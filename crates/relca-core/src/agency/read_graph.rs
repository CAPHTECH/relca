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
    if m.capabilities.iter().all(|c| c.read_variables.is_empty()) {
        return Ok(vec![Vec::new(); calls.len()]);
    }
    let mut b = viewpoints::budget(DEFAULT);
    b.copy(calls.len())?;
    let mut parents = vec![Vec::new(); calls.len()];
    for cap in &m.capabilities {
        for v in &cap.read_variables {
            b.work()?;
            if m.state_variables
                .get(v.0)
                .is_none_or(|v| v.kind != ir::StateKind::ObservedCase)
            {
                return Err(invalid());
            }
        }
    }
    if concurrent_read(&m.process.body, &m.capabilities).is_some() {
        return Err(invalid());
    }
    for (i, c) in calls.iter().enumerate() {
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
