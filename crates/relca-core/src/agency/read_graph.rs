//! Observed snapshots contribute lexical edges, never execution state.
use super::*;

pub(crate) fn lexical_has_capability(body: &ir::Block, id: ir::CapabilityId) -> bool {
    body.iter().any(|s| match &s.kind {
        ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => c.capability == id,
        ir::Stmt::Fork { branches, .. } => branches.iter().any(|c| c.capability == id),
        ir::Stmt::If {
            then_body,
            else_body,
            ..
        } => lexical_has_capability(then_body, id) || lexical_has_capability(else_body, id),
        _ => false,
    })
}

fn writes(c: &ir::Capability, v: ir::StateVarId) -> bool {
    crate::evidence_basis::writes(c, v)
}

pub(crate) fn concurrent_read(body: &ir::Block, caps: &[ir::Capability]) -> Option<diag::Span> {
    for s in body {
        match &s.kind {
            ir::Stmt::Response(_, _) => {} // This query concerns fork-sibling reads only.
            ir::Stmt::Fork { branches, .. } => {
                for c in branches {
                    let cap = caps.get(c.capability.0)?;
                    if cap.read_variables.iter().any(|v| {
                        branches.iter().any(|w| {
                            w.binding != c.binding
                                && caps.get(w.capability.0).is_some_and(|cap| writes(cap, *v))
                        })
                    }) {
                        return Some(c.span);
                    }
                }
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                if let Some(span) =
                    concurrent_read(then_body, caps).or_else(|| concurrent_read(else_body, caps))
                {
                    return Some(span);
                }
            }
            _ => {}
        }
    }
    None
}

/// A separate ledger keeps old, no-read-edge accounting byte-for-byte stable.
pub(super) fn read_parents(m: &ir::Model, calls: &[&ir::Call]) -> Result<Vec<Vec<ir::BindingId>>> {
    with_limits(m, calls, DEFAULT)
}

fn with_limits(
    m: &ir::Model,
    calls: &[&ir::Call],
    limits: Limits,
) -> Result<Vec<Vec<ir::BindingId>>> {
    if m.has_merges() {
        let mut out = vec![Vec::new(); m.process.bindings.len()];
        let mut b = viewpoints::budget(limits);
        for (i, c) in calls.iter().enumerate() {
            for w in &calls[..i] {
                b.work()?;
                if m.capabilities[c.capability.0]
                    .read_variables
                    .iter()
                    .any(|v| writes(&m.capabilities[w.capability.0], *v))
                {
                    b.copy(1)?;
                    out[c.binding.0].push(w.binding);
                }
            }
        }
        return Ok(out);
    }
    if m.capabilities.iter().all(|c| c.read_variables.is_empty()) {
        return Ok(vec![Vec::new(); calls.len()]);
    }
    let mut b = viewpoints::budget(limits);
    b.location = Location::model(m, "read_graph");
    b.copy(calls.len())?;
    let mut parents = vec![Vec::new(); calls.len()];
    for cap in &m.capabilities {
        for v in &cap.read_variables {
            b.work()?;
            if m.state_variables
                .get(v.0)
                .is_none_or(|v| v.kind != ir::StateKind::ObservedCase)
            {
                return Err(Error::InvalidIr(
                    "read_graph requires an observed case variable".into(),
                ));
            }
        }
    }
    if concurrent_read(&m.process.body, &m.capabilities).is_some() {
        return Err(Error::InvalidIr(
            "read_graph has a concurrent writer".into(),
        ));
    }
    for (i, c) in calls.iter().enumerate() {
        b.location.span = Some(c.span);
        b.location.call = Some(m.process.bindings[c.binding.0].name.clone());
        let cap = m.capabilities.get(c.capability.0).ok_or_else(invalid)?;
        for w in &calls[..i] {
            b.work()?;
            let writer = m.capabilities.get(w.capability.0).ok_or_else(invalid)?;
            if cap.read_variables.iter().any(|v| writes(writer, *v)) {
                b.copy(1)?;
                parents[i].push(w.binding);
            }
        }
    }
    Ok(parents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_graph_limits_keep_their_phase_and_counter() {
        let m = crate::lower::lower(
            &crate::syntax::parse(include_str!(
                "../../../../fixtures/agy2-reads-observed/model.relca"
            ))
            .unwrap(),
        )
        .unwrap();
        let mut indexed = vec![None; m.process.bindings.len()];
        collect(
            &m.process.body,
            &mut indexed,
            &m,
            0,
            &mut viewpoints::budget(DEFAULT),
        )
        .unwrap();
        let calls: Vec<_> = indexed.into_iter().collect::<Option<_>>().unwrap();
        let expected = read_parents(&m, &calls).unwrap();
        for field in ["work", "cells"] {
            let run = |k| {
                let mut l = DEFAULT;
                if field == "work" {
                    l.work = k;
                } else {
                    l.cells = k;
                }
                with_limits(&m, &calls, l)
            };
            let k = (1..10000).find(|k| run(*k).is_ok()).unwrap();
            assert_eq!(run(k).unwrap(), expected);
            let Error::Limit(l) = run(k - 1).unwrap_err() else {
                panic!("expected limit");
            };
            assert_eq!(l.counter, field);
            assert_eq!(l.phase, "read_graph");
            assert_eq!(l.process, m.process.name);
            assert!(l.requested.unwrap() >= k);
            assert!(l.span.is_some());
        }
    }
}
