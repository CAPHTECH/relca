use super::*;

pub(super) fn source(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
pub(super) fn model(name: &str) -> Model {
    crate::lower::lower(&crate::syntax::parse(&source(name)).unwrap()).unwrap()
}

pub(super) fn range_witness() -> StateRangeViolation {
    let src = source("rsrc-over-demand")
        .replace("  actor", "  observed case { n: 0..0 = 0 }\n  actor")
        .replace(
            "result nondet { ok }",
            "result nondet { ok }; start { n += 1 }",
        );
    let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
    explore(&m).outcome.state_range_violations.remove(0)
}
pub(super) fn tasks(m: &Model) -> Vec<Task> {
    fn calls(block: &Block, out: &mut Vec<Call>) {
        for s in block {
            match &s.kind {
                Stmt::Call(c) => out.push(c.clone()),
                Stmt::Response(_, _) => {
                    panic!("resource test frame builder does not cover retry control")
                }
                Stmt::Fork { branches, .. } => out.extend(branches.clone()),
                _ => {}
            }
        }
    }
    let mut cs = Vec::new();
    calls(&m.process.body, &mut cs);
    cs.into_iter()
        .map(|call| Task {
            frozen_performance: None,
            stale: BTreeSet::new(),
            call,
            phase: Phase::Queued,
            metric_ready: Some(0),
            checked_through: None,
            person: None,
            start: None,
            stop: None,
            finish: None,
            ack: None,
        })
        .collect()
}

#[test]
fn resource_h01_h08_reference_legacy_observer_actual_checked_occupancy_and_cut_precedence() {
    for capacity in [2, 1] {
        let mut m = model("rsrc-exact");
        m.resources[0].capacity = capacity;
        let mut f = replay::initial(&m);
        f.tasks = tasks(&m);
        for t in &mut f.tasks {
            t.phase = Phase::Running;
            t.start = Some(0);
            t.finish = Some(5);
        }
        assert_eq!(reference_resource_units(f.tasks.iter(), &m).unwrap(), [2]);
        let mut i = replay::interpreter(&m);
        i.observe_capacity(&f).unwrap();
        for cut in [false, true] {
            i.evidence.truncated = cut;
            if cut {
                i.statistics.ranges.push(range_witness());
            }
            assert_eq!(
                i.evidence
                    .verdict(Property::NoResourceOvercommit, StopReason::Complete),
                if capacity == 1 {
                    Verdict::Violated
                } else if !cut {
                    Verdict::Satisfied
                } else {
                    Verdict::Inconclusive
                }
            );
        }
    }
    let m = model("rsrc-huge");
    let mut f = replay::initial(&m);
    f.tasks = tasks(&m);
    for t in &mut f.tasks {
        t.phase = Phase::Running;
        t.start = Some(0);
        t.finish = Some(1);
    }
    assert!(matches!(
        reference_resource_units(f.tasks.iter(), &m),
        Err(StopReason::SemanticAbort)
    ));
    assert_eq!(
        replay::interpreter(&m).observe_capacity(&f),
        Err(StopReason::SemanticAbort)
    );
}

#[test]
fn resource_h02_h05_reference_candidates_ignore_done_and_never_partially_acquire() {
    let m = model("rsrc-release-fork");
    let mut f = replay::initial(&m);
    f.tasks = tasks(&m);
    f.tasks[0].phase = Phase::Finished(ValueId(0));
    assert_eq!(reference_resource_units(f.tasks.iter(), &m).unwrap(), [0]);
    assert!(replay::interpreter(&m).can_start(&f, 1).unwrap());
    let mut m = model("rsrc-atomic");
    m.actors[0].capacity = Some(3);
    m.capabilities.push(m.capabilities[0].clone());
    m.capabilities[2].uses.retain(|u| u.resource.0 == 0);
    m.capabilities[1].uses.retain(|u| u.resource.0 == 1);
    for reverse in [false, true] {
        if reverse {
            m.capabilities[0].uses.reverse();
        }
        let mut f = replay::initial(&m);
        f.tasks = tasks(&m);
        let mut third = f.tasks[0].clone();
        third.call.capability = CapabilityId(2);
        third.call.binding = BindingId(2);
        f.tasks.push(third);
        f.tasks[1].phase = Phase::Running;
        f.tasks[1].start = Some(0);
        f.tasks[1].finish = Some(3);
        let i = replay::interpreter(&m);
        assert!(!i.can_start(&f, 0).unwrap());
        assert_eq!(
            reference_resource_units(f.tasks.iter(), &m).unwrap(),
            [0, 1]
        );
        assert!(i.can_start(&f, 2).unwrap());
        assert_eq!(
            reference_resource_units(f.tasks.iter(), &m).unwrap(),
            [0, 1]
        );
    }
}

#[test]
fn resource_h03_h04_h06_reference_actual_replayed_lifecycle_and_h16_independent_sweep() {
    for (name, time) in [("rsrc-cooperative", 3), ("rsrc-immediate", 2)] {
        let src = source(name).replace("  bounds","  observed case { flag: bool = false }\n  contract C { invariant A.c.finish -> flag == true }\n  bounds");
        let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        let report = crate::explain::explain(&src);
        let c = report
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id == "contract C #1 invariant")
            .unwrap();
        let mut f = replay::initial(&m);
        let mut i = replay::interpreter(&m);
        let (mut request, mut ack) = (false, false);
        for s in c.steps.iter().skip(1) {
            replay::apply(&m, &mut i, &mut f, s, None).unwrap();
            let used = reference_resource_units(f.tasks.iter(), &m).unwrap();
            if s.kind == crate::explain::Kind::CancelRequest {
                assert_eq!(used, [1]);
                assert_eq!(f.tasks[1].ack, Some(time));
                request = true;
            }
            if s.kind == crate::explain::Kind::CancelAck {
                assert_eq!(used, [0]);
                ack = true;
                let mut probe = f.tasks[0].clone();
                probe.phase = Phase::Queued;
                probe.call.capability = CapabilityId(2);
                probe.call.binding = BindingId(2);
                probe.start = None;
                probe.stop = None;
                let mut g = f.clone();
                g.tasks.push(probe);
                assert!(i.can_start(&g, g.tasks.len() - 1).unwrap());
            }
        }
        assert!(request && ack);
    }
    let m = model("rsrc-queued-cancel");
    let mut f = replay::initial(&m);
    f.tasks = tasks(&m);
    assert_eq!(reference_resource_units(f.tasks.iter(), &m).unwrap(), [0]);
    f.tasks[1].phase = Phase::Cancelled;
    assert_eq!(reference_resource_units(f.tasks.iter(), &m).unwrap(), [0]);
    let mut i = replay::interpreter(&m);
    i.observe_capacity(&f).unwrap();
    assert!(!i.evidence.overcommitted);
    let src = source("rsrc-queued-cancel").replace("; end done", "; z = A.b(); end done");
    let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
    let report = crate::explain::explain(&src);
    let c = report
        .explanation
        .counterexamples
        .iter()
        .find(|c| c.id == "diagnostic:W-DEADLOCK-0")
        .unwrap();
    replay::replay(&m, &c.steps, &c.id, c.case_id).unwrap();
    let mut f = replay::initial(&m);
    let mut i = replay::interpreter(&m);
    let mut saw_cancel = false;
    for s in c.steps.iter().skip(1) {
        replay::apply(&m, &mut i, &mut f, s, None).unwrap();
        if f.tasks.get(1).is_some_and(|t| t.phase == Phase::Cancelled) {
            saw_cancel = true;
            assert_eq!(reference_resource_units(f.tasks.iter(), &m).unwrap(), [0]);
            i.observe_capacity(&f).unwrap();
            assert!(!i.evidence.overcommitted);
        }
    }
    assert!(saw_cancel);
    let m = model("rsrc-half-open");
    let mut ts = tasks(&m);
    ts[0].phase = Phase::Finished(ValueId(0));
    ts[0].start = Some(0);
    ts[0].stop = Some(2);
    ts[1].phase = Phase::Finished(ValueId(0));
    ts[1].start = Some(2);
    ts[1].stop = Some(3);
    let metric = metrics::project(
        &m,
        &[metrics::Observation {
            condition_intervals: Vec::new(),
            admitted_at: Some(0),
            wait_intervals: Vec::new(),
            intentional_wait: 0,
            tasks: &ts,
            arrival: 0,
            terminal: Some((EndId(0), 3)),
            rejected: false,
            deadlock: false,
            future: false,
        }],
        3,
    );
    let row = serde_json::to_value(&metric.resources.unwrap()[0]).unwrap();
    assert_eq!(row["occupancy_time"]["min"], "5");
    assert_eq!(row["peak_units"]["min"], "2");
    assert_eq!(row["utilization"]["min"], "1/6");
}

#[test]
fn resource_h11_replay_rejects_shortage_and_nonempty_global_holder_tampering_and_h07_guard() {
    let src = source("rsrc-over-demand.broken-too-much");
    let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
    let report = crate::explain::explain(&src);
    let c = report
        .explanation
        .counterexamples
        .iter()
        .find(|c| c.id == "diagnostic:W-DEADLOCK-0")
        .unwrap();
    replay::replay(&m, &c.steps, &c.id, c.case_id).unwrap();
    for fault in 0..2 {
        let mut steps = c.steps.clone();
        let d = steps.last_mut().unwrap().deadlock_detail.as_mut().unwrap();
        if fault == 0 {
            d.waiting_tasks[0].resource_shortages[0].missing = 0;
        } else {
            d.shared_resources[0].capacity = 2;
        }
        assert!(matches!(
            replay::replay(&m, &steps, &c.id, c.case_id),
            Err(crate::explain::ReplayReason::StateMismatch
                | crate::explain::ReplayReason::IllegalTransition)
        ));
    }
    for fault in 0..6 {
        let mut bad = m.clone();
        match fault {
            0 => bad.capabilities[0].uses[0].resource = ResourceId(999),
            1 => bad.capabilities[0].uses[0].amount = 0,
            2 => {
                let u = bad.capabilities[0].uses[0].clone();
                bad.capabilities[0].uses.push(u);
            }
            3 => bad.resources[0].name.clear(),
            4 => bad.resources.push(bad.resources[0].clone()),
            _ => bad.resources[0].name = bad.actors[0].name.clone(),
        }
        assert!(replay::replay(&bad, &c.steps, &c.id, c.case_id).is_err());
    }
    let src = source("rsrc-global.broken-two-units").replace("  bounds","  observed case { flag: bool = false }\n  contract C { invariant A.x.finish -> flag == true }\n  bounds");
    let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
    let report = crate::explain::explain(&src);
    let c = report
        .explanation
        .counterexamples
        .iter()
        .find(|c| c.id == "contract C #1 invariant")
        .unwrap();
    replay::replay(&m, &c.steps, &c.id, c.case_id).unwrap();
    for fault in 0..2 {
        let mut steps = c.steps.clone();
        let row = steps
            .iter_mut()
            .find(|s| {
                s.shared_resources
                    .first()
                    .is_some_and(|r| r.holders.len() == 2)
            })
            .unwrap();
        let holder = &mut row.shared_resources[0].holders[1];
        if fault == 0 {
            holder.case_id = 1;
        } else {
            holder.amount = 2;
        }
        assert!(matches!(
            replay::replay(&m, &steps, &c.id, c.case_id),
            Err(crate::explain::ReplayReason::StateMismatch
                | crate::explain::ReplayReason::IllegalTransition)
        ));
    }
}
