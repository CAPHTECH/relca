//! Event contracts use only positively matched observations, never event counts
//! from raw redeliveries or Actor names as a proxy for the identity of a person.
use super::{
    checks::{Check, Tri},
    input::Event,
    lifecycle::{self, Task},
};
use crate::ir;

pub fn observed(
    pattern: ir::Event,
    case: &str,
    tasks: &[Task],
    m: &ir::Model,
    events: &[Event],
) -> Vec<usize> {
    lifecycle::matching(pattern, tasks, m)
        .into_iter()
        .filter(|i| events[*i].case_id == case)
        .collect()
}
fn incomplete(pattern: ir::Event, case: &str, tasks: &[Task]) -> bool {
    tasks
        .iter()
        .filter(|t| t.case_id == case && t.observed_capabilities.contains(&pattern.capability))
        .any(|t| {
            !t.trusted
                || match pattern.kind {
                    ir::EventKind::Start => t.start.is_none(),
                    ir::EventKind::Finish(_) | ir::EventKind::Commit => !t.complete,
                    ir::EventKind::CancelAck => t.ack.is_none(),
                }
        })
}
pub fn independent(m: &ir::Model, events: &[Event], tasks: &[Task], case: &str) -> Vec<Check> {
    independent_with_q1(m, events, tasks, case, &std::collections::BTreeSet::new())
}

pub(super) fn independent_with_q1(
    m: &ir::Model,
    events: &[Event],
    tasks: &[Task],
    case: &str,
    q1: &std::collections::BTreeSet<ir::CapabilityId>,
) -> Vec<Check> {
    let mut rows = Vec::new();
    for (i, c) in m.contracts.iter().enumerate() {
        if !supported_contract_with_q1(&c.kind, m, q1)
            && !matches!(c.kind, ir::ContractKind::Separation { .. })
        {
            continue;
        }
        let (state, reason, ids) = match c.kind {
            ir::ContractKind::Separation { left, right } => {
                let a = observed(left, case, tasks, m, events);
                let b = observed(right, case, tasks, m, events);
                let person = |i| {
                    tasks
                        .iter()
                        .find(|t| t.indices.contains(&i))
                        .and_then(|t| t.person)
                };
                let mut evidence = Vec::new();
                let mut missing = false;
                for &x in &a {
                    for &y in &b {
                        match (person(x), person(y)) {
                            (Some(p), Some(q)) if p == q => evidence.extend([x, y]),
                            (Some(_), Some(_)) => (),
                            _ => missing = true,
                        }
                    }
                }
                if !evidence.is_empty() {
                    (Tri::Violating, Some("separation"), evidence)
                } else if a.is_empty() || b.is_empty() {
                    (Tri::Unknown, Some("no_observed_trigger"), [a, b].concat())
                } else if missing || incomplete(left, case, tasks) || incomplete(right, case, tasks)
                {
                    (Tri::Unknown, Some("task_correspondence"), [a, b].concat())
                } else {
                    (Tri::Conformant, None, [a, b].concat())
                }
            }
            ir::ContractKind::AtMostOnce { event } => {
                let ids = observed(event, case, tasks, m, events);
                if ids.len() > 1 {
                    (Tri::Violating, Some("at_most_once"), ids)
                } else if ids.is_empty() {
                    (Tri::Unknown, Some("no_observed_trigger"), ids)
                } else if incomplete(event, case, tasks) {
                    (Tri::Unknown, Some("task_correspondence"), ids)
                } else {
                    (Tri::Conformant, None, ids)
                }
            }
            ir::ContractKind::RequiresEvent { .. }
            | ir::ContractKind::Excludes { .. }
            | ir::ContractKind::Response { .. }
            | ir::ContractKind::NamedResponse { .. }
            | ir::ContractKind::Waive { .. }
            | ir::ContractKind::ContestResponse { .. }
            | ir::ContractKind::Invariant { .. }
            | ir::ContractKind::StateInvariant { .. }
            | ir::ContractKind::Permitted { .. }
            | ir::ContractKind::AuthorityBasis { .. } => continue,
        };
        rows.push(Check::new(
            format!(
                "{}contract:{}",
                if supported_contract_with_q1(&c.kind, m, q1) {
                    ""
                } else {
                    "independent:"
                },
                c.ordinal.unwrap_or(i + 1)
            ),
            Some(case),
            state,
            reason,
            &ids,
            events,
            Some(c.span),
        ));
    }
    rows
}

/// Exhaustive match: a new model contract variant cannot silently become a
/// successful log check. Dependent semantics must be supported as well.
pub fn supported_contract(kind: &ir::ContractKind, m: &ir::Model) -> bool {
    supported_contract_with_q1(kind, m, &std::collections::BTreeSet::new())
}

pub(super) fn supported_contract_with_q1(
    kind: &ir::ContractKind,
    m: &ir::Model,
    q1: &std::collections::BTreeSet<ir::CapabilityId>,
) -> bool {
    let event = |e: ir::Event| {
        !q1.contains(&e.capability) && lifecycle::supported(&m.capabilities[e.capability.0], m)
    };
    match *kind {
        ir::ContractKind::Separation { left, right } => event(left) && event(right),
        ir::ContractKind::AtMostOnce { event: e } => event(e),
        ir::ContractKind::RequiresEvent { before, start }
        | ir::ContractKind::Excludes { before, start } => event(before) && event(start),
        ir::ContractKind::Response { .. }
        | ir::ContractKind::NamedResponse { .. }
        | ir::ContractKind::Waive { .. }
        | ir::ContractKind::ContestResponse { .. }
        | ir::ContractKind::Invariant { .. }
        | ir::ContractKind::StateInvariant { .. }
        | ir::ContractKind::Permitted { .. }
        | ir::ContractKind::AuthorityBasis { .. } => false,
    }
}

pub struct Ordered {
    pub checks: Vec<Check>,
    pub truncated: bool,
}
pub fn ordered(
    m: &ir::Model,
    events: &[Event],
    tasks: &[Task],
    case: &str,
    order: &super::order::Order,
) -> Ordered {
    ordered_with_q1(
        m,
        events,
        tasks,
        case,
        order,
        &std::collections::BTreeSet::new(),
    )
}

pub(super) fn ordered_with_q1(
    m: &ir::Model,
    events: &[Event],
    tasks: &[Task],
    case: &str,
    order: &super::order::Order,
    q1: &std::collections::BTreeSet<ir::CapabilityId>,
) -> Ordered {
    let mut out = Ordered {
        checks: Vec::new(),
        truncated: false,
    };
    for (i, c) in m.contracts.iter().enumerate() {
        let (before, start, excludes) = match c.kind {
            ir::ContractKind::RequiresEvent { before, start } => (before, start, false),
            ir::ContractKind::Excludes { before, start } => (before, start, true),
            ir::ContractKind::Separation { .. }
            | ir::ContractKind::AtMostOnce { .. }
            | ir::ContractKind::Response { .. }
            | ir::ContractKind::NamedResponse { .. }
            | ir::ContractKind::Waive { .. }
            | ir::ContractKind::ContestResponse { .. }
            | ir::ContractKind::Invariant { .. }
            | ir::ContractKind::StateInvariant { .. }
            | ir::ContractKind::Permitted { .. }
            | ir::ContractKind::AuthorityBasis { .. } => continue,
        };
        let a = observed(before, case, tasks, m, events);
        let b = observed(start, case, tasks, m, events);
        let mut ids = [a.clone(), b.clone()].concat();
        // A single strict-time pair is a schedule-independent local witness and
        // remains available even after a different order query exhausted its
        // budget. Other reachability probes share the run-wide order budget.
        let mut forced = None;
        let mut limited = false;
        if excludes && !a.is_empty() && !b.is_empty() {
            let x = *a
                .iter()
                .min_by_key(|i| events[**i].time.latest)
                .expect("nonempty");
            let y = *b
                .iter()
                .max_by_key(|i| events[**i].time.earliest)
                .expect("nonempty");
            if events[x].time.latest < events[y].time.earliest {
                forced = Some((x, y));
            }
            if forced.is_none() {
                'pairs: for &x in &a {
                    for &y in &b {
                        if x == y {
                            continue;
                        }
                        match order.precedes(x, y) {
                            Some(true) => {
                                forced = Some((x, y));
                                break 'pairs;
                            }
                            Some(false) => (),
                            None => {
                                limited = true;
                                break 'pairs;
                            }
                        }
                    }
                }
            }
        }
        let (state, reason) = if a.is_empty() || (!excludes && b.is_empty()) {
            (Tri::Unknown, Some("no_observed_trigger"))
        } else if excludes && b.is_empty() {
            if incomplete(before, case, tasks) || incomplete(start, case, tasks) {
                (Tri::Unknown, Some("task_correspondence"))
            } else {
                (Tri::Conformant, None)
            }
        } else if let Some((x, y)) = forced {
            // A forced violating pair is independent of the other schedules.
            ids.clear();
            ids.extend([x, y]);
            (Tri::Violating, Some("excludes"))
        } else if limited {
            out.truncated = true;
            (Tri::Unknown, Some("order_limit"))
        } else {
            let alternatives = order.alternatives(&ids, |path| {
                let pos = |x| path.iter().position(|i| *i == x).expect("relevant event");
                if excludes {
                    a.iter().any(|&x| b.iter().any(|&y| pos(x) < pos(y)))
                } else {
                    b.iter().all(|&y| a.iter().any(|&x| pos(x) < pos(y)))
                }
            });
            if alternatives.truncated {
                out.truncated = true;
                (Tri::Unknown, Some("order_limit"))
            } else if alternatives.yes && alternatives.no {
                (Tri::Unknown, Some("ambiguous_order"))
            } else if excludes && alternatives.yes {
                (Tri::Violating, Some("excludes"))
            } else if !excludes && !alternatives.yes {
                (Tri::Unknown, Some("no_observed_trigger"))
            } else if incomplete(before, case, tasks) || incomplete(start, case, tasks) {
                (Tri::Unknown, Some("task_correspondence"))
            } else {
                (Tri::Conformant, None)
            }
        };
        out.checks.push(Check::new(
            format!(
                "{}contract:{}",
                if supported_contract_with_q1(&c.kind, m, q1) {
                    ""
                } else {
                    "independent:"
                },
                c.ordinal.unwrap_or(i + 1)
            ),
            Some(case),
            state,
            reason,
            &ids,
            events,
            Some(c.span),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commit_match_requires_a_normal_completion_not_a_result_name() {
        let src = "model M version \"0.2\" { type V = enum { committed } actor A { capacity 1; capability x() -> V { commits; duration fixed 1min; result nondet { committed }; cancel cooperative ack_within 1min } } process P { a = A.x(); end done } contract C { at_most_once A.x.commit per case.id } bounds { cases 1; live_tasks 1; clock 3min } }";
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let t = Task {
            q1_unsupported: false,
            case_id: "c".into(),
            task_id: "t".into(),
            indices: vec![0],
            capability: Some(ir::CapabilityId(0)),
            observed_capabilities: vec![ir::CapabilityId(0)],
            person: None,
            start: Some(0),
            finish: None,
            ack: Some(0),
            result: Some(ir::ValueId(0)),
            trusted: true,
            complete: false,
        };
        assert!(
            lifecycle::matching(
                ir::Event {
                    capability: ir::CapabilityId(0),
                    kind: ir::EventKind::Commit
                },
                &[t],
                &m
            )
            .is_empty()
        );
    }
}
