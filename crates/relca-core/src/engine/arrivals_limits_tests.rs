//! Private ADR 0039 state/clock/forged-IR obligations, independent hand oracles.
use super::*;
use serde_json::json;

fn model(src: &str) -> ir::Model {
    crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap()
}
fn readmit() -> ir::Model {
    model(include_str!("../../../../fixtures/lim-readmit/model.relca"))
}
fn concurrent() -> ir::Model {
    model(include_str!(
        "../../../../fixtures/lim-concurrent-wait/model.relca"
    ))
}
fn states(m: &ir::Model) -> (Vec<Node>, Facts, Vec<u64>) {
    let a = arrivals(m).unwrap();
    let mut f = Facts::new(a.len());
    let nodes = graph(m, &a, 100_000, &mut f).unwrap();
    (nodes, f, a)
}
fn hash(s: &Global) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

#[test]
fn b01_readmit_after_immediates_before_clock_keeps_arrival_origin() {
    let m = readmit();
    let (nodes, _, a) = states(&m);
    let done = nodes
        .iter()
        .find(|n| {
            n.state
                .cases
                .iter()
                .all(|s| s.payload().is_some_and(|c| c.terminal.is_some()))
        })
        .unwrap();
    let s = &done.state;
    assert_eq!(
        s.cases
            .iter()
            .map(|c| c.payload().unwrap().admitted_at)
            .collect::<Vec<_>>(),
        [Some(0), Some(0), Some(2)]
    );
    assert_eq!(
        s.cases
            .iter()
            .map(|c| c.payload().unwrap().local.time)
            .collect::<Vec<_>>(),
        [2, 4, 6]
    );
    let v = serde_json::to_value(metric_snapshot(s, &m, &a, s.time, false)).unwrap();
    for (k, (external, start)) in [(0, 0), (0, 2), (2, 4)].into_iter().enumerate() {
        assert_eq!(v["cases"][k]["external_wait"]["min"], external.to_string());
        assert_eq!(v["cases"][k]["first_start_wait"]["min"], start.to_string());
    }
}

#[test]
fn b02_joint_initial_product_includes_held_at_arrival_and_never_reinitializes() {
    let src = include_str!("../../../../fixtures/lim-concurrent-wait/model.relca");
    let src = src.replace(
        "  observed case",
        "  latent case { flag:bool ~ {true:0.5,false:0.5} }\n  observed case",
    );
    let m = model(&src);
    let (nodes, _, _) = states(&m);
    let chance = nodes
        .iter()
        .find(|n| n.state.phase == Phase::Chance)
        .unwrap();
    assert_eq!(chance.actions.len(), 1);
    assert_eq!(chance.actions[0].len(), 8);
    let flag = m
        .state_variables
        .iter()
        .position(|v| v.name == "flag")
        .unwrap();
    let mut tuples = std::collections::HashSet::new();
    for (weight, id) in &chance.actions[0] {
        assert_eq!(*weight, Rational::decimal("0.125").unwrap());
        let s = &nodes[*id].state;
        assert_eq!(s.time, 0);
        assert!(matches!(s.cases[1], Slot::Held(_)) && matches!(s.cases[2], Slot::Held(_)));
        tuples.insert(
            s.cases
                .iter()
                .map(|c| c.payload().unwrap().local.state_values[flag])
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(tuples.len(), 8);
    for n in &nodes {
        if n.state.time > 0 && n.state.phase == Phase::Chance {
            assert_eq!(n.actions[0].len(), 1, "no second initial probability layer");
        }
        for c in n.state.cases.iter().filter_map(Slot::payload) {
            if c.received && c.admitted_at.is_none() {
                assert!(c.local.tasks.is_empty());
                assert_eq!(c.local.results, vec![None]);
                assert_ne!(c.local.state_values[flag], ir::StateValue::None);
            }
        }
    }
}

#[test]
fn b03_slot_admitted_time_peak_and_monitor_are_distinct_state_keys() {
    let m = concurrent();
    let (nodes, _, _) = states(&m);
    let s = nodes
        .iter()
        .find(|n| {
            n.state.phase == Phase::Mixed
                && n.state.time == 0
                && n.state.cases[0].payload().is_some_and(|c| c.received)
        })
        .unwrap()
        .state
        .clone();
    let c = s.cases[0].payload().unwrap().clone();
    let mut variants = Vec::new();
    for slot in [
        Slot::Held(c.clone().into()),
        Slot::Overflow(c.clone().into()),
    ] {
        let mut n = (*s).clone();
        n.cases[0] = slot;
        variants.push(n);
    }
    let mut at = (*s).clone();
    at.cases[0].payload_mut().unwrap().admitted_at = Some(1);
    variants.push(at);
    let mut peak = (*s).clone();
    peak.external_peak = Some(3);
    variants.push(peak);
    let mut received = (*s).clone();
    received.cases[0].payload_mut().unwrap().received = false;
    variants.push(received);
    let mut monitor = (*s).clone();
    monitor.cases[0].payload_mut().unwrap().local.monitors[0].violated = true;
    variants.push(monitor);
    for n in variants {
        assert_ne!(*s, n);
        assert_ne!(hash(&s), hash(&n));
    }
}

#[test]
fn b04_clock_snapshot_does_not_turn_held_into_rejection() {
    let mut m = concurrent();
    m.bounds.clock = 3;
    let (nodes, f, a) = states(&m);
    assert!(f.truncated);
    let n = nodes
        .iter()
        .find(|n| n.cut == ProbabilityCut::Clock)
        .unwrap();
    assert_eq!(
        n.state
            .cases
            .iter()
            .map(|c| c.payload().unwrap().admitted_at)
            .collect::<Vec<_>>(),
        [Some(0), Some(2), None]
    );
    assert!(matches!(n.state.cases[2], Slot::Held(_)));
    let v = serde_json::to_value(metric_snapshot(&n.state, &m, &a, 3, false)).unwrap();
    for (key, count) in [
        ("received", 3),
        ("admitted", 2),
        ("completed", 1),
        ("unfinished", 2),
        ("rejected", 0),
        ("deadlocked", 0),
        ("not_arrived", 0),
    ] {
        assert_eq!(
            v["overall"]["counts"][key]["stats"]["min"],
            count.to_string(),
            "{key}"
        );
    }
    let out = explore(&m, 100_000);
    let v = serde_json::to_value(out.metrics.unwrap()).unwrap();
    assert_eq!(
        v["overall"]["counts"]["admitted"]["stats"],
        json!({"min":null,"max":null,"reason":"truncated"})
    );
}

#[test]
fn b05_clock_equal_readmission_and_taskless_clock_zero_are_exhausted() {
    let mut m = concurrent();
    m.bounds.clock = 2;
    let (nodes, _, _) = states(&m);
    let n = nodes
        .iter()
        .find(|n| n.cut == ProbabilityCut::Clock)
        .unwrap();
    assert_eq!(n.state.cases[1].payload().unwrap().admitted_at, Some(2));
    let m = model(include_str!(
        "../../../../fixtures/lim-instant.broken-clock-zero/model.relca"
    ));
    let (nodes, f, _) = states(&m);
    assert!(!f.truncated && !f.deadlock);
    assert!(nodes.len() < 50);
    assert!(nodes.iter().any(|n| n.state.cases.iter().all(|s| {
        s.payload()
            .is_some_and(|c| c.terminal.is_some() && c.local.time == 0)
    })));
}

#[test]
fn b06_old_rejections_have_no_payload_initialization_or_receipt() {
    let m = model(include_str!(
        "../../../../fixtures/arrivals-zero-queue/model.relca"
    ));
    let (nodes, f, _) = states(&m);
    assert!(!m.explicit_limits() && !f.chance);
    for n in &nodes {
        assert!(n.state.external_peak.is_none());
        assert!(
            n.state
                .cases
                .iter()
                .all(|s| matches!(s, Slot::Future | Slot::Rejected))
        );
    }
    for budget in 0..=nodes.len() + 1 {
        let mut f = Facts::new(2);
        let a = arrivals(&m).unwrap();
        let ns = graph(&m, &a, budget, &mut f).unwrap();
        assert!(
            ns.iter()
                .all(|n| n.state.cases.iter().all(|s| s.payload().is_none()))
        );
    }
}

#[test]
fn b07_malformed_public_limit_ir_is_refused_independently() {
    let wait = concurrent();
    let end = model(include_str!(
        "../../../../fixtures/lim-overflow/model.relca"
    ));
    let mut bad = Vec::new();
    let mut m = wait.clone();
    m.concurrent_cases.as_mut().unwrap().capacity = 0;
    bad.push(m);
    let mut m = wait.clone();
    m.admission_queue = Some(ir::AdmissionQueue {
        capacity: 0,
        overflow: ir::Overflow::WaitExternalFifo,
    });
    bad.push(m);
    let mut m = wait.clone();
    m.admission_queue = Some(ir::AdmissionQueue {
        capacity: 1,
        overflow: ir::Overflow::Reject,
    });
    bad.push(m);
    let mut m = end.clone();
    m.admission_queue.as_mut().unwrap().overflow = ir::Overflow::End(ir::EndId(usize::MAX));
    bad.push(m);
    let mut m = end.clone();
    m.overflow_end = None;
    bad.push(m);
    let mut m = wait.clone();
    m.overflow_end = Some(ir::EndId(0));
    bad.push(m);
    let mut m = end.clone();
    m.concurrent_cases = Some(ir::AdmissionQueue {
        capacity: 2,
        overflow: ir::Overflow::End(ir::EndId(0)),
    });
    bad.push(m);
    let mut m = end.clone();
    m.process.body.push(ir::Statement {
        kind: ir::Stmt::End(m.overflow_end.unwrap()),
        span: m.process.span,
    });
    bad.push(m);
    let mut m = model(include_str!(
        "../../../../fixtures/lim-admitted-deadline/model.relca"
    ));
    m.concurrent_cases.as_mut().unwrap().overflow = ir::Overflow::Reject;
    bad.push(m);
    let mut m = wait.clone();
    m.arrivals = Some(vec![1, 0, 0]);
    bad.push(m);
    let mut m = wait;
    m.arrivals = Some(vec![0, 0]);
    bad.push(m);
    for (i, m) in bad.iter().enumerate() {
        let out = super::super::explore(m);
        assert!(
            out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0),
            "engine {i}"
        );
        assert_eq!(
            crate::reference::explore(m).stop,
            crate::reference::StopReason::SemanticAbort,
            "reference {i}"
        );
    }
}

#[test]
fn b08_admitted_matches_only_admission_and_deadline_addition_is_checked() {
    let m = model(include_str!(
        "../../../../fixtures/lim-admitted-deadline/model.relca"
    ));
    assert!(!Bundle::Received.trigger(ir::Trigger::Admitted, &m));
    assert!(!Bundle::Admitted.trigger(ir::Trigger::Received, &m));
    assert!(Bundle::Admitted.trigger(ir::Trigger::Admitted, &m));
    assert!(!Bundle::Start(ir::CapabilityId(0), None).trigger(ir::Trigger::Admitted, &m));
    let mut s = State::initial(&m, 0);
    s.time = u64::MAX;
    assert!(emit(&mut s, Bundle::Admitted, &m, &mut Exploration::default()).is_err());
}

#[test]
fn b09_overflow_never_creates_process_tasks_or_resource_work() {
    let m = model(include_str!(
        "../../../../fixtures/lim-counts-bare/model.relca"
    ));
    let (nodes, _, _) = states(&m);
    for n in &nodes {
        for slot in &n.state.cases {
            if let Slot::Overflow(c) = slot {
                assert!(c.local.tasks.is_empty());
                assert!(!c.ever_started && c.admitted_at.is_none());
                assert_eq!(c.local.work, 0);
                assert!(
                    c.local
                        .metric_history
                        .as_ref()
                        .unwrap()
                        .tasks
                        .iter()
                        .all(|t| t.status == metrics::Status::NeverCreated)
                );
            }
        }
    }
}

#[test]
fn b10_concurrent_room_returns_while_contest_route_still_occupies_resource() {
    let src = include_str!("../../../../fixtures/ctx-contest-origin/model.relca")
        .replace("duration fixed 4min", "duration fixed 2min")
        .replace(
            "actor Route { executors [p]; capability respond() -> T { duration fixed 2min",
            "actor Route { executors [p]; capability respond() -> T { duration fixed 5min",
        )
        .replace("[10min]", "[0min, 0min]")
        .replace("cases 1", "cases 2")
        .replace("respond_by 2min", "respond_by 5min")
        .replace(
            "  bounds {",
            "  concurrent cases 1 on_overflow wait_external fifo\n  bounds {",
        );
    let m = model(&src);
    let (nodes, _, _) = states(&m);
    let n = nodes.iter().find(|n| n.state.time == 2 && matches!(&n.state.cases[0],Slot::Admitted(c) if c.terminal.is_some() && c.local.tasks.iter().any(|t| t.lifecycle.running())) && matches!(n.state.cases[1],Slot::Held(_))).unwrap();
    assert!(n.state.room(&m));
}

#[test]
fn b11_cut_bounds_and_whole_path_extrema_do_not_sum_case_maxima() {
    let m = readmit();
    let a = arrivals(&m).unwrap();
    let mut f = Facts::new(3);
    let nodes = graph(&m, &a, 0, &mut f).unwrap();
    assert_eq!(admission_range(&nodes, 3, true), Range { min: 0, max: 3 });
    assert_eq!(admission_range(&nodes, 3, false), Range { min: 0, max: 3 });
    let (nodes, _, _) = states(&m);
    assert_eq!(admission_range(&nodes, 3, true), Range { min: 2, max: 2 });
    let s = nodes
        .iter()
        .find(|n| {
            n.state
                .cases
                .iter()
                .all(|c| c.payload().is_some_and(|c| c.terminal.is_some()))
        })
        .unwrap()
        .state
        .clone();
    let mut combined = None;
    for selected in [1, 2] {
        let mut path = (*s).clone();
        for (k, slot) in path.cases.iter_mut().enumerate() {
            if k != selected {
                *slot = Slot::Future;
            }
        }
        metrics::merge(&mut combined, metric_snapshot(&path, &m, &a, 6, false));
    }
    let v = serde_json::to_value(combined.unwrap()).unwrap();
    assert_eq!(v["overall"]["counts"]["admitted"]["stats"]["max"], "1");
    assert_eq!(v["cases"][1]["counts"]["admitted"]["stats"]["max"], "1");
    assert_eq!(v["cases"][2]["counts"]["admitted"]["stats"]["max"], "1");
}

#[test]
fn b13_nullable_admission_survives_roundtrip_and_forged_projection_fails_closed() {
    let raw = json!({"case_id":3,"arrival":0,"admitted_at":null,"terminals":[]});
    let row: crate::report::CaseOutcome = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(row).unwrap(), raw);
    let old = json!({"case_id":3,"arrival":0,"terminals":[]});
    let row: crate::report::CaseOutcome = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(row).unwrap(), old);
    let src = include_str!("../../../../fixtures/lim-counts-bare/model.relca");
    let m = model(src);
    let report = crate::report::check(src);
    assert!(crate::comparison_projection::exhaustive(&report, &m).is_ok());
    let mut forged = report.clone();
    forged.metrics.as_mut().unwrap().overall.counts.admitted = None;
    assert!(crate::comparison_projection::exhaustive(&forged, &m).is_err());
    let mut forged = report.clone();
    forged.metrics.as_mut().unwrap().cases[1]
        .terminals
        .iter_mut()
        .find(|t| t.kind == crate::metrics::TerminalKind::Overflow)
        .unwrap()
        .kind = crate::metrics::TerminalKind::End;
    assert!(crate::comparison_projection::exhaustive(&forged, &m).is_err());
    let mut forged = report;
    forged.case_outcomes.as_mut().unwrap()[1].terminals[0].kind =
        crate::report::CaseTerminalKind::End;
    assert!(crate::comparison_projection::exhaustive(&forged, &m).is_err());
}

#[test]
fn b14_readmission_reserves_room_before_clone_or_evidence_and_old_overflow_is_not_reemitted() {
    let m = readmit();
    let (nodes, _, a) = states(&m);
    let before = nodes
        .iter()
        .find(|n| {
            n.actions.iter().flatten().any(|(_, id)| {
                n.state
                    .cases
                    .iter()
                    .zip(&nodes[*id].state.cases)
                    .any(|(a, b)| matches!((a, b), (Slot::Held(_), Slot::Admitted(_))))
            })
        })
        .unwrap();
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    let mut facts = Facts::new(a.len());
    assert!(matches!(
        transition(
            &before.state,
            &m,
            &a,
            &program,
            entry,
            &mut Room(0),
            &mut facts
        ),
        Err(Halt::Budget)
    ));
    assert!(facts.per.iter().all(|f| f.state_events.is_empty()));
    let src = include_str!("../../../../fixtures/lim-overflow.broken-hidden/model.relca")
        .replace("[0min, 0min, 0min]", "[0min, 0min, 3min]");
    let m = model(&src);
    let (nodes, _, _) = states(&m);
    let mut capture = trace::Capture::new(&src, &m);
    for n in &nodes {
        if n.state.phase != Phase::Activate || n.state.time != 3 {
            continue;
        }
        for (_, id) in n.actions.iter().flatten() {
            let label = capture
                .global_label(&n.state, &nodes[*id].state, &m)
                .unwrap();
            assert!(!label.events.iter().any(|e| matches!(e,crate::explain::Event::Business(e) if e.kind==crate::explain::Kind::End && e.case_id==Some(2))));
        }
    }
    let explained = crate::explain::explain(&src);
    assert!(
        explained
            .explanation
            .counterexamples
            .iter()
            .all(|c| c.replay.status == crate::explain::ReplayStatus::Verified)
    );
}
