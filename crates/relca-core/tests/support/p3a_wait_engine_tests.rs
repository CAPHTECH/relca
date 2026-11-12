//! Include as cfg(test) child of engine::metrics. No cause calculator here.
use super::*;
use crate::metrics::p3a_wait_hand as hand;
fn observe(kind: &str, delay: u64, stop: &str) -> serde_json::Value {
    let mut m = crate::lower::lower(&crate::syntax::parse(&hand::source(kind)).unwrap()).unwrap();
    let end = 5 + delay;
    if stop == "horizon" {
        m.horizon = Some(end);
    }
    let mut states = [State::initial(&m, 0), State::initial(&m, 4)];
    for (case, state) in states.iter_mut().enumerate() {
        state.time = if case == 0 { end } else { 4 + delay };
        let history = state.metric_history.as_mut().unwrap();
        for h in &mut history.tasks {
            h.status = Status::NeverCreated;
            h.ready = None;
            h.start = None;
            h.stop = None;
        }
        history.waits = Some(if case == 0 { vec![(1, 4)] } else { vec![] });
        for (name, ready, start, finish) in if case == 0 {
            vec![("a1", 0, 0, 1), ("a2", 4, 4 + delay, end)]
        } else {
            vec![("blocker", 4, 4, 4 + delay)]
        } {
            let id = m
                .process
                .bindings
                .iter()
                .position(|b| b.name == name)
                .unwrap();
            let h = &mut history.tasks[id];
            h.ready = Some(ready);
            h.start = Some(start);
            h.stop = Some(finish);
            h.status = Status::Finished;
            h.executor = (kind == "executor").then_some(ir::PersonId(0));
        }
    }
    let observations: Vec<_> = states
        .iter()
        .enumerate()
        .map(|(case, state)| CaseObservation {
            admitted_at: Some(if case == 0 { 0 } else { 4 }),
            state: Some(state),
            arrival: if case == 0 { 0 } else { 4 },
            end: if case == 0 && matches!(stop, "unknown" | "horizon") {
                None
            } else {
                Some(ir::EndId(0))
            },
            rejected: false,
            deadlocked: false,
            not_arrived: false,
        })
        .collect();
    let point = snapshot_with_horizon(&m, &observations, &Peak::new(&m), end, stop == "horizon");
    let point = if stop == "truncated" {
        finish(Some(point), &m, crate::obligation::Coverage::Truncated).unwrap()
    } else {
        point
    };
    serde_json::to_value(point).unwrap()
}
#[test]
fn p3a_wait_engine_two_cases_half_open_capacity_person_resource_and_stops() {
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
