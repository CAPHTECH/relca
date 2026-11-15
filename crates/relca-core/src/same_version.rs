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

pub(crate) fn supported(m: &ir::Model) -> Result<(), Vec<diag::Diagnostic>> {
    if !present(m) {
        return Ok(());
    }
    let Some(clause) = m
        .contracts
        .iter()
        .find(|c| matches!(c.kind, ir::ContractKind::SameVersion { .. }))
    else {
        return Ok(());
    };
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
    fn unsupported_block(m: &ir::Model, body: &ir::Block) -> bool {
        body.iter().any(|s| match &s.kind {
            ir::Stmt::Call(c) => non_result_call(m, c),
            ir::Stmt::Response(..) | ir::Stmt::Query(..) | ir::Stmt::ResolveEffect { .. } => true,
            ir::Stmt::Fork {
                join: ir::Join::Any { .. },
                ..
            } => true,
            ir::Stmt::Fork { branches, .. } => branches.iter().any(|c| non_result_call(m, c)),
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => unsupported_block(m, then_body) || unsupported_block(m, else_body),
            _ => false,
        })
    }
    let reason = if m.has_loops() {
        "loop_feature_combination"
    } else if !m.process.degradations.is_empty()
        || m.recovery().is_some()
        || unsupported_block(m, &m.process.body)
    {
        "same_version_feature_combination"
    } else {
        return Ok(());
    };
    Err(vec![diag::Diagnostic {
        code: diag::U_SEM_0.into(),
        severity: diag::Severity::Error,
        message: reason.into(),
        span: Some(clause.span),
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
    if first == second || a.ty != b.ty || supported(m).is_err() {
        return Err(bad);
    }
    Ok(())
}
