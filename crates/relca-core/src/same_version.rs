//! Value identity contracts never participate in Agency or evidence derivation.
use crate::{diag, ir};

pub(crate) fn present(m: &ir::Model) -> bool {
    m.contracts
        .iter()
        .any(|c| matches!(c.kind, ir::ContractKind::SameVersion { .. }))
}

/// A Finish carries an observed result even when the operation rejected its
/// work. Such results still have an identity, but do not examine or use a
/// version successfully. Ordinary enum names have no success/failure meaning.
pub(crate) fn normally_completed(cap: &ir::Capability, result: ir::ValueId) -> bool {
    cap.version_check
        .as_ref()
        .is_none_or(|v| v.reject != result)
        && cap.atomic.as_ref().is_none_or(|a| a.commit == result)
}

enum Refusal<'a> {
    Degradation(&'a ir::Degradation),
    Recovery(ir::BindingId, &'a ir::Block),
    Response(ir::BindingId, diag::Span),
    JoinAny(ir::ForkId, diag::Span),
    NonResult(&'a ir::Call),
    // The public IR still rejects these even though the DSL gate does not
    // admit them out of a recovery handler.
    InternalRecoveryStatement,
}

fn visit_refusals<'a>(
    m: &'a ir::Model,
    record: &mut impl FnMut(Refusal<'a>) -> Option<Refusal<'a>>,
) -> Option<Refusal<'a>> {
    fn non_result_argument(m: &ir::Model, arg: ir::BindingId) -> bool {
        let mut pending = vec![arg];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(arg) = pending.pop() {
            if arg == ir::CURRENT_CASE {
                return true;
            }
            if !seen.insert(arg) {
                continue;
            }
            let Some(binding) = m.process.bindings.get(arg.0) else {
                return true;
            };
            if m.state_variables
                .iter()
                .any(|v| v.kind == ir::StateKind::ObservedCase && v.name == binding.name)
            {
                return true;
            }
            if let Some(merge) = &binding.merge {
                pending.extend(&merge.sources);
            }
        }
        false
    }
    fn non_result_call(m: &ir::Model, call: &ir::Call) -> bool {
        m.contracts.iter().any(|c| match c.kind {
            ir::ContractKind::SameVersion { first, second } => {
                [first, second].iter().any(|&(cap, param)| {
                    cap == call.capability
                        && call
                            .args
                            .get(param)
                            .is_none_or(|&a| non_result_argument(m, a))
                })
            }
            _ => false,
        })
    }
    fn unsupported_block<'a>(
        m: &ir::Model,
        body: &'a ir::Block,
        record: &mut impl FnMut(Refusal<'a>) -> Option<Refusal<'a>>,
    ) -> Option<Refusal<'a>> {
        body.iter().find_map(|s| match &s.kind {
            ir::Stmt::Call(c) => non_result_call(m, c)
                .then(|| record(Refusal::NonResult(c)))
                .flatten(),
            ir::Stmt::Response(_, r) => {
                record(Refusal::Response(r.command, s.span)).or_else(|| {
                    non_result_call(m, &r.command_call)
                        .then(|| record(Refusal::NonResult(&r.command_call)))
                        .flatten()
                })
            }
            ir::Stmt::Query(..) | ir::Stmt::ResolveEffect { .. } => {
                record(Refusal::InternalRecoveryStatement)
            }
            ir::Stmt::Fork { id, join, branches } => {
                if matches!(join, ir::Join::Any { .. })
                    && let Some(found) = record(Refusal::JoinAny(*id, s.span))
                {
                    return Some(found);
                }
                branches
                    .iter()
                    .filter(|c| non_result_call(m, c))
                    .find_map(|c| record(Refusal::NonResult(c)))
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => unsupported_block(m, then_body, record)
                .or_else(|| unsupported_block(m, else_body, record)),
            _ => None,
        })
    }

    m.process
        .degradations
        .iter()
        .find_map(|d| record(Refusal::Degradation(d)))
        .or_else(|| {
            m.recovery()
                .and_then(|(b, body)| record(Refusal::Recovery(b, body)))
        })
        .or_else(|| unsupported_block(m, &m.process.body, record))
}

fn refusal(m: &ir::Model) -> Option<(diag::Span, Refusal<'_>)> {
    let clause = m
        .contracts
        .iter()
        .find(|c| matches!(c.kind, ir::ContractKind::SameVersion { .. }))?;
    Some((clause.span, visit_refusals(m, &mut Some)?))
}

impl Refusal<'_> {
    fn detail(self, m: &ir::Model, reason: &str) -> Option<diag::Detail> {
        use diag::{Blocker, DetailKind};
        let at = |role: &str, name: &str, span| Blocker::at(role, Some(name.into()), Some(span));
        let binding_name = |b: ir::BindingId| m.process.bindings.get(b.0).map(|b| b.name.as_str());
        let mut kind = DetailKind::AnalysisNotConnected;
        let (cause, blockers) = match self {
            Self::Degradation(d) => ("degradation", vec![at("degradation", &d.name, d.span)]),
            Self::Recovery(binding, handler) => {
                let name = binding_name(binding)?;
                let handler_span = diag::Span {
                    start: handler.first()?.span.start,
                    end: handler.last()?.span.end,
                };
                let mut blockers = vec![at("recovery_handler", name, handler_span)];
                if let Some((call, _)) = m
                    .response_attempts()
                    .into_iter()
                    .find(|(_, r)| r.attempt == 1 && r.command == binding && r.recovery.is_some())
                {
                    blockers.push(at("response", name, call.span));
                }
                ("recovery", blockers)
            }
            Self::Response(b, span) => ("response", vec![at("response", binding_name(b)?, span)]),
            Self::JoinAny(id, span) => (
                "join_any",
                vec![at("join_any", m.process.forks.get(id.0)?, span)],
            ),
            Self::NonResult(call) => {
                kind = DetailKind::Rewrite;
                (
                    "non_result_argument",
                    vec![at(
                        "non_result_call",
                        binding_name(call.binding)?,
                        call.span,
                    )],
                )
            }
            Self::InternalRecoveryStatement => return None,
        };
        Some(diag::Detail {
            reason: reason.into(),
            kind,
            cause: Some(cause.into()),
            blockers,
            see: "docs/guide/dsl.md#確かめた版と使う版を揃える".into(),
        })
    }
}

pub(crate) fn supported(m: &ir::Model) -> Result<(), Vec<diag::Diagnostic>> {
    let Some((span, refusal)) = refusal(m) else {
        return Ok(());
    };
    let reason = "same_version_feature_combination";
    let detail = refusal.detail(m, reason).and_then(|mut summary| {
        let mut complete = true;
        summary.blockers.clear();
        visit_refusals(m, &mut |cause| {
            if let Some(row) = cause.detail(m, reason) {
                if row.kind == diag::DetailKind::AnalysisNotConnected {
                    summary.kind = row.kind;
                }
                summary.blockers.extend(row.blockers);
            } else {
                complete = false;
            }
            None
        });
        complete.then_some(summary)
    });
    Err(vec![diag::Diagnostic {
        presentation: diag::Presentation {
            detail: detail.map(Box::new),
            ..Default::default()
        },
        code: diag::U_SEM_0.into(),
        severity: diag::Severity::Error,
        message: reason.into(),
        span: Some(span),
    }])
}

pub(crate) fn validate(
    m: &ir::Model,
    first: (ir::CapabilityId, usize),
    second: (ir::CapabilityId, usize),
) -> Result<(), &'static str> {
    let bad = "invalid same_version target";
    let a = m
        .capabilities
        .get(first.0.0)
        .and_then(|c| c.params.get(first.1))
        .ok_or(bad)?;
    let b = m
        .capabilities
        .get(second.0.0)
        .and_then(|c| c.params.get(second.1))
        .ok_or(bad)?;
    if first == second || a.ty != b.ty || refusal(m).is_some() {
        return Err(bad);
    }
    Ok(())
}
