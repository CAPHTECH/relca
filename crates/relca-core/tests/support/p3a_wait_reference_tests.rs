//! Include as cfg(test) child of reference::metrics. Reconstruct reference tasks.
use super::*;
use crate::metrics::p3a_wait_hand as hand;
fn observe(kind: &str, delay: u64, stop: &str) -> serde_json::Value {
    let mut m = crate::lower::lower(&crate::syntax::parse(&hand::source(kind)).unwrap()).unwrap();
    let end = 5 + delay;
    if stop == "horizon" {
        m.horizon = Some(end);
    }
    let (mut calls, mut ends) = (Vec::new(), Vec::new());
    inventory(&m.process.body, &mut calls, &mut ends);
    let tasks: Vec<Vec<Task>> = (0..2)
        .map(|case| {
            let specs = if case == 0 {
                vec![("a1", 0, 0, 1), ("a2", 4, 4 + delay, end)]
            } else {
                vec![("blocker", 4, 4, 4 + delay)]
            };
            specs
                .into_iter()
                .map(|(name, ready, start, finish)| {
                    let call = calls
                        .iter()
                        .find(|c| m.process.bindings[c.binding.0].name == name)
                        .unwrap();
                    Task {
                        frozen_performance: None,
                        call: (*call).clone(),
                        metric_ready: Some(ready),
                        start: Some(start),
                        stop: Some(finish),
                        person: (kind == "executor").then_some(ir::PersonId(0)),
                        phase: Phase::Finished(ir::ValueId(0)),
                        stale: Default::default(),
                        checked_through: None,
                        finish: None,
                        ack: None,
                    }
                })
                .collect()
        })
        .collect();
    let observations: Vec<_> = tasks
        .iter()
        .enumerate()
        .map(|(case, tasks)| Observation {
            condition_intervals: Vec::new(),
            admitted_at: Some(if case == 0 { 0 } else { 4 }),
            wait_intervals: if case == 0 { vec![(1, 4)] } else { vec![] },
            intentional_wait: if case == 0 { 3 } else { 0 },
            tasks,
            arrival: if case == 0 { 0 } else { 4 },
            terminal: if case == 0 && matches!(stop, "unknown" | "horizon") {
                None
            } else {
                Some((ir::EndId(0), if case == 0 { end } else { 4 + delay }))
            },
            rejected: false,
            deadlock: false,
            future: false,
        })
        .collect();
    let point = project_with_horizon(&m, &observations, end, stop == "horizon");
    let point = if stop == "truncated" {
        let mut env = Envelope::default();
        env.add(point);
        env.finish("truncated").unwrap()
    } else {
        point
    };
    serde_json::to_value(point).unwrap()
}
#[test]
fn p3a_wait_reference_two_cases_half_open_capacity_person_resource_and_stops() {
    for kind in ["actor_capacity", "executor", "resource"] {
        let source_kind = if kind == "actor_capacity" {
            "actor"
        } else {
            kind
        };
        for delay in [1, 2] {
            for stop in ["complete", "unknown", "horizon", "truncated"] {
                hand::check(&observe(source_kind, delay, stop), kind, delay, stop);
            }
        }
    }
}
