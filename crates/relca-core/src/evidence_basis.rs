//! Contract evidence is separate from the may-lineage used by Agency.
use crate::{diag, ir};
use std::collections::BTreeSet;

pub(crate) enum SourceInput<'a> {
    Parent(ir::BindingId),
    Direct(&'a [ir::ActorId]),
}

/// One lineage rule for Agency's may graph and the contract's must graph.
/// Only snapshot edges differ. Visiting, rather than allocating a new union,
/// preserves Agency's exact insert/copy/work accounting and traversal order.
pub(crate) fn derive_sources<E>(
    call: &ir::Call,
    cap: &ir::Capability,
    snapshots: &[ir::BindingId],
    mut visit: impl FnMut(SourceInput<'_>) -> Result<(), E>,
) -> Result<(), E> {
    for &parent in call.args.iter().chain(snapshots) {
        if parent != ir::CURRENT_CASE {
            visit(SourceInput::Parent(parent))?;
        }
    }
    visit(SourceInput::Direct(&cap.reads))
}

fn union(
    call: &ir::Call,
    cap: &ir::Capability,
    snapshots: &[ir::BindingId],
    inherited: &[BTreeSet<ir::ActorId>],
) -> BTreeSet<ir::ActorId> {
    let mut set = BTreeSet::new();
    let result: Result<(), std::convert::Infallible> =
        derive_sources(call, cap, snapshots, |input| {
            match input {
                SourceInput::Parent(p) => set.extend(&inherited[p.0]),
                SourceInput::Direct(sources) => set.extend(sources),
            }
            Ok(())
        });
    result.unwrap();
    set
}

pub(crate) fn always_writes(c: &ir::Capability, v: ir::StateVarId) -> bool {
    c.start_writes.iter().any(|w| w.variable == v)
        || (c.atomic.is_none()
            && c.version_check.is_none()
            && (c.writes.iter().any(|w| w.variable == v)
                || (!c.result_kernels().collect::<Vec<_>>().is_empty()
                    && c.result_kernels().all(|k| {
                        k.rows.iter().all(|r| {
                            r.branches
                                .iter()
                                .all(|b| b.overrides.iter().any(|(x, _)| *x == v))
                        })
                    }))))
}

pub(crate) fn present(m: &ir::Model) -> bool {
    m.contracts
        .iter()
        .any(|c| matches!(c.kind, ir::ContractKind::EvidenceBasis { .. }))
}

fn unsupported(reason: &str, span: diag::Span) -> Vec<diag::Diagnostic> {
    vec![diag::Diagnostic {
        code: diag::U_SEM_0.into(),
        severity: diag::Severity::Error,
        message: reason.into(),
        span: Some(span),
    }]
}

pub(crate) fn writes(c: &ir::Capability, v: ir::StateVarId) -> bool {
    c.start_writes
        .iter()
        .chain(c.finish_writes())
        .any(|w| w.variable == v)
        || c.kernel_writes.contains(&v)
        || c.structural_kernel().is_some_and(|k| {
            k.rows
                .iter()
                .flat_map(|r| &r.branches)
                .flat_map(|b| &b.overrides)
                .any(|(x, _)| *x == v)
        })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SiteKind {
    Call,
    OptionalAttempt,
    ResultAlias,
}

struct Site<'a> {
    call: &'a ir::Call,
    prefix: BTreeSet<ir::BindingId>,
    kind: SiteKind,
}

fn exclusive(body: &ir::Block, a: diag::Span, b: diag::Span) -> bool {
    let contains = |block: &ir::Block, p: diag::Span| {
        block
            .iter()
            .any(|s| s.span.start <= p.start && p.end <= s.span.end)
    };
    body.iter().any(|s| match &s.kind {
        ir::Stmt::If {
            then_body,
            else_body,
            ..
        } => {
            (contains(then_body, a) && contains(else_body, b))
                || (contains(else_body, a) && contains(then_body, b))
                || exclusive(then_body, a, b)
                || exclusive(else_body, a, b)
        }
        _ => false,
    })
}

/// The same/outer block prefix dominates; a completed fork is deliberately not
/// promoted into that prefix in stage one. Each branch still inherits its outer
/// prefix. No condition, requires, wait, or latent kernel edge is an input.
fn collect<'a>(body: &'a ir::Block, prefix: &BTreeSet<ir::BindingId>, out: &mut Vec<Site<'a>>) {
    let mut prefix = prefix.clone();
    for s in body {
        match &s.kind {
            ir::Stmt::Call(c) => {
                out.push(Site {
                    call: c,
                    prefix: prefix.clone(),
                    kind: SiteKind::Call,
                });
                prefix.insert(c.binding);
            }
            ir::Stmt::Response(c, r) => {
                if r.attempt == 1 {
                    // This binding carries an adopted result. It is never a
                    // Task and cannot itself emit a Finish or write a snapshot.
                    out.push(Site {
                        call: &r.command_call,
                        prefix: prefix.clone(),
                        kind: SiteKind::ResultAlias,
                    });
                    prefix.insert(r.command);
                }
                out.push(Site {
                    call: c,
                    prefix: prefix.clone(),
                    kind: if r.attempt == 1 {
                        SiteKind::Call
                    } else {
                        SiteKind::OptionalAttempt
                    },
                });
                prefix.insert(c.binding);
            }
            ir::Stmt::Fork { branches, .. } => {
                for c in branches {
                    out.push(Site {
                        call: c,
                        prefix: prefix.clone(),
                        kind: SiteKind::Call,
                    });
                }
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                collect(then_body, &prefix, out);
                collect(else_body, &prefix, out);
            }
            _ => {}
        }
    }
}

pub(crate) fn prepare(m: &mut ir::Model) -> Result<(), Vec<diag::Diagnostic>> {
    if !present(m) {
        return Ok(());
    }
    let span = m
        .contracts
        .iter()
        .find(|c| matches!(c.kind, ir::ContractKind::EvidenceBasis { .. }))
        .unwrap()
        .span;
    if m.has_loops() {
        return Err(unsupported("loop_feature_combination", span));
    }
    if !m.process.degradations.is_empty() || m.recovery().is_some() {
        return Err(unsupported("evidence_basis_feature_combination", span));
    }
    for c in &m.contracts {
        if let ir::ContractKind::EvidenceBasis { sources, .. } = &c.kind
            && sources
                .iter()
                .any(|s| !crate::responsibility::included(m, *s))
        {
            return Err(vec![diag::Diagnostic {
                code: diag::E_CUT_1.into(),
                severity: diag::Severity::Error,
                message: "evidence_basis references a source Actor excluded by the cut".into(),
                span: Some(c.span),
            }]);
        }
    }
    let mut calls = Vec::new();
    collect(&m.process.body, &BTreeSet::new(), &mut calls);
    // Retry Task slots are allocated after the public result bindings. Their
    // numeric IDs are not lexical order; only the process traversal orders
    // writers before readers and source propagation before dependent calls.
    let n = m.process.bindings.len();
    let mut must = vec![BTreeSet::new(); n];
    let mut may = must.clone();
    let mut bound_must = must.clone();
    let mut bound_may = must.clone();
    let mut conditional = must.clone();
    let mut excluded: Vec<BTreeSet<String>> = vec![BTreeSet::new(); n];
    // A merge is not an evidence-producing call. Track only whether one flows
    // into a target's inputs, never invent a union/intersection of its roots.
    // Control conditions are deliberately not evidence inputs (ADR 0070).
    let mut merged: Vec<_> = m
        .process
        .bindings
        .iter()
        .map(|b| b.merge.is_some())
        .collect();
    for (i, site) in calls.iter().enumerate() {
        let call = site.call;
        let prefix = &site.prefix;
        let cap = &m.capabilities[call.capability.0];
        let mut certain = union(call, cap, &[], &bound_must);
        let mut possible = union(call, cap, &[], &bound_may);
        let mut uncertain = BTreeSet::new();
        let mut edges = BTreeSet::new();
        let mut merge_input = false;
        for arg in call.args.iter().filter(|a| **a != ir::CURRENT_CASE) {
            edges.extend(excluded[arg.0].iter().cloned());
            merge_input |= merged[arg.0];
        }
        for &variable in &cap.read_variables {
            let mut candidates = Vec::new();
            let mut guaranteed = false;
            for parent in calls[..i].iter().rev() {
                if parent.kind == SiteKind::ResultAlias {
                    continue;
                }
                if exclusive(&m.process.body, parent.call.span, call.span) {
                    continue;
                }
                let writer = parent.call;
                let wc = &m.capabilities[writer.capability.0];
                if !writes(wc, variable) {
                    continue;
                }
                candidates.push(writer.binding);
                if prefix.contains(&writer.binding)
                    && parent.kind == SiteKind::Call
                    && wc.requires.is_none()
                    && always_writes(wc, variable)
                {
                    guaranteed = true;
                    break;
                }
            }
            let mut shared = if guaranteed {
                candidates
                    .first()
                    .map(|b| must[b.0].clone())
                    .unwrap_or_default()
            } else {
                BTreeSet::new()
            };
            let mut all = BTreeSet::new();
            for &writer in &candidates {
                merge_input |= merged[writer.0];
                shared = shared.intersection(&must[writer.0]).copied().collect();
                all.extend(&may[writer.0]);
                edges.extend(excluded[writer.0].iter().cloned());
            }
            if candidates.len() != 1 || !guaranteed {
                uncertain.extend(all.difference(&shared).copied());
                for writer in &candidates {
                    edges.insert(format!(
                        "{} -> {}",
                        m.process.bindings[writer.0].name, m.process.bindings[call.binding.0].name
                    ));
                }
            }
            certain.extend(&shared);
            possible.extend(all);
        }
        uncertain.retain(|s| !certain.contains(s));
        must[call.binding.0] = certain;
        may[call.binding.0] = possible;
        conditional[call.binding.0] = uncertain;
        bound_must[call.binding.0] = if cap.requires.is_none() {
            must[call.binding.0].clone()
        } else {
            BTreeSet::new()
        };
        bound_may[call.binding.0] = may[call.binding.0].clone();
        excluded[call.binding.0] = edges;
        merged[call.binding.0] = merge_input;
    }
    for clause in &m.contracts {
        if let ir::ContractKind::EvidenceBasis { capability, .. } = &clause.kind
            && calls
                .iter()
                .any(|s| s.call.capability == *capability && merged[s.call.binding.0])
        {
            return Err(unsupported(
                "merge_control: evidence_basis target has merged inputs",
                clause.span,
            ));
        }
    }
    for clause in &mut m.contracts {
        if let ir::ContractKind::EvidenceBasis {
            capability,
            direct,
            calls: table,
            ..
        } = &mut clause.kind
        {
            table.clear();
            for site in &calls {
                let call = site.call;
                let direct_target = *direct && call.capability == *capability;
                let actual = if direct_target {
                    m.capabilities[capability.0].reads.iter().copied().collect()
                } else {
                    must[call.binding.0].clone()
                };
                table.push(ir::EvidenceCall {
                    binding: call.binding,
                    sources: actual.into_iter().collect(),
                    possible_sources: if direct_target {
                        m.capabilities[capability.0].reads.clone()
                    } else {
                        may[call.binding.0].iter().copied().collect()
                    },
                    conditional_sources: conditional[call.binding.0].iter().copied().collect(),
                    excluded_edges: if direct_target {
                        vec![]
                    } else {
                        excluded[call.binding.0].iter().cloned().collect()
                    },
                });
            }
        }
    }
    Ok(())
}

/// Execution must not trust a forged public-IR proof table. Ordinary models
/// return before cloning or doing any evidence graph work.
pub(crate) fn validate(m: &ir::Model) -> Result<(), &'static str> {
    if !present(m) {
        return Ok(());
    }
    let bad = "invalid evidence_basis table";
    for cap in &m.capabilities {
        if cap
            .reads
            .iter()
            .any(|s| m.actors.get(s.0).is_none_or(|a| a.source.is_none()))
            || cap.read_variables.iter().any(|v| {
                m.state_variables
                    .get(v.0)
                    .is_none_or(|v| v.kind != ir::StateKind::ObservedCase)
            })
        {
            return Err(bad);
        }
    }
    for c in &m.contracts {
        if let ir::ContractKind::EvidenceBasis {
            capability,
            sources,
            ..
        } = &c.kind
            && (m.capabilities.get(capability.0).is_none()
                || sources.is_empty()
                || sources.iter().collect::<BTreeSet<_>>().len() != sources.len()
                || sources
                    .iter()
                    .any(|s| m.actors.get(s.0).is_none_or(|a| a.source.is_none())))
        {
            return Err(bad);
        }
    }
    let mut expected = m.clone();
    prepare(&mut expected).map_err(|_| bad)?;
    if expected.contracts != m.contracts {
        return Err(bad);
    }
    Ok(())
}
