use super::*;

pub(super) fn fixture(name: &str) -> ir::Model {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap();
    crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap()
}

pub(super) fn range_witness() -> outcome::StateRangeViolation {
    let src = include_str!("../../../../fixtures/rsrc-over-demand/model.relca")
        .replace("  actor", "  observed case { n: 0..0 = 0 }\n  actor")
        .replace(
            "result nondet { ok }",
            "result nondet { ok }; start { n += 1 }",
        );
    let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
    explore(&m).state_range_violations.remove(0)
}

pub(super) fn queued(m: &ir::Model) -> (State, Vec<Instruction>) {
    let mut program = vec![Instruction::Stop];
    let pc = compile(&m.process.body, 0, &mut program);
    let s = State::initial(m, pc);
    let Step::Next(mut xs) = step(s, &program, m, &mut Room(100)).unwrap() else {
        panic!("queue edge");
    };
    (xs.remove(0), program)
}

#[test]
fn resource_same_tick_normal_finish_and_ack_both_release_once() {
    let mut m = fixture("rsrc-cooperative");
    m.capabilities[1].duration = 3;
    let (s, p) = queued(&m);
    let mut pending = vec![s];
    let mut seen = HashSet::new();
    let mut endings = BTreeSet::new();
    while let Some(s) = pending.pop() {
        if !seen.insert(s.clone()) {
            continue;
        }
        if s.time == 3
            && s.tasks
                .get(1)
                .is_some_and(|t| t.lifecycle == Lifecycle::Done)
        {
            endings.insert(s.results[1].is_some());
            assert_eq!(s.resource_occupancy(&m).unwrap(), [0]);
        }
        if let Step::Next(xs) = step(s, &p, &m, &mut Room(1000)).unwrap() {
            pending.extend(xs);
        }
    }
    assert_eq!(endings, BTreeSet::from([false, true]));
}

#[test]
fn resource_h01_h08_legacy_observer_overcommit_and_checked_actual_overflow() {
    for capacity in [2, 1] {
        let mut m = fixture("rsrc-exact");
        m.resources[0].capacity = capacity;
        let (mut s, _) = queued(&m);
        for t in &mut s.tasks {
            t.lifecycle = Lifecycle::Running {
                finish: 5,
                ack: None,
            };
        }
        assert_eq!(s.resource_occupancy(&m).unwrap(), [2]);
        for cut in [false, true] {
            let mut e = Exploration {
                truncated: cut,
                ..Exploration::default()
            };
            e.observe(&s, &m).unwrap();
            let o = e.outcome(&m);
            assert_eq!(
                o.properties[0].verdict,
                if capacity == 1 {
                    Verdict::Violated
                } else if cut {
                    Verdict::Inconclusive
                } else {
                    Verdict::Satisfied
                }
            );
        }
        if capacity == 1 {
            let mut e = Exploration::default();
            e.ranges.push(range_witness());
            e.observe(&s, &m).unwrap();
            assert_eq!(e.outcome(&m).properties[0].verdict, Verdict::Violated);
            let out = run_initialized(&m, true, 1000, |mut initial| {
                initial.tasks = s.tasks.clone();
                initial
            });
            assert_eq!(out.properties[0].verdict, Verdict::Violated);
        }
    }
    let m = fixture("rsrc-huge");
    let (mut s, _) = queued(&m);
    for t in &mut s.tasks {
        t.lifecycle = Lifecycle::Running {
            finish: 1,
            ack: None,
        };
    }
    assert!(matches!(
        s.resource_occupancy(&m),
        Err(Halt::Unsupported(_))
    ));
    assert!(matches!(
        Exploration::default().observe(&s, &m),
        Err(Halt::Unsupported(_))
    ));
    let out = run_initialized(&m, true, 1000, |mut initial| {
        initial.tasks = s.tasks;
        initial
    });
    assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
}

#[test]
fn resource_h02_h03_h04_h06_h09_actual_lifecycle_seams_and_half_open_peak() {
    for name in [
        "rsrc-release-fork",
        "rsrc-cooperative",
        "rsrc-immediate",
        "rsrc-queued-cancel",
        "rsrc-zero-time",
    ] {
        let m = fixture(name);
        let (s, p) = queued(&m);
        let mut pending = vec![s];
        let mut seen = HashSet::new();
        let (mut requested, mut acked, mut released, mut zero_started, mut queued_cancelled) =
            (false, false, false, false, false);
        while let Some(s) = pending.pop() {
            if !seen.insert(s.clone()) {
                continue;
            }
            let occupied = s.resource_occupancy(&m).unwrap()[0];
            if name == "rsrc-cooperative" || name == "rsrc-immediate" {
                if s.time == 2
                    && s.tasks.get(1).is_some_and(|t| {
                        matches!(t.lifecycle, Lifecycle::Running { ack: Some(_), .. })
                    })
                {
                    assert_eq!(occupied, 1);
                    requested = true;
                }
                let ack_time = if name == "rsrc-immediate" { 2 } else { 3 };
                if s.time == ack_time
                    && s.tasks
                        .get(1)
                        .is_some_and(|t| t.lifecycle == Lifecycle::Done)
                    && s.results[1].is_none()
                {
                    assert_eq!(occupied, 0);
                    assert!(resources_fit(
                        &m.capabilities[2],
                        &s.resource_occupancy(&m).unwrap(),
                        &m
                    ));
                    acked = true;
                }
            }
            if name == "rsrc-release-fork"
                && s.tasks.len() == 2
                && s.tasks.iter().any(|t| t.lifecycle == Lifecycle::Done)
                && s.tasks.iter().any(|t| t.lifecycle == Lifecycle::Queued)
            {
                assert_eq!(occupied, 0);
                released = true;
                let Step::Next(xs) = step(s.clone(), &p, &m, &mut Room(100)).unwrap() else {
                    panic!("released sibling must dispatch");
                };
                assert!(
                    xs.iter()
                        .any(|n| n.tasks.iter().any(|t| t.lifecycle.running()))
                );
            }
            if name == "rsrc-queued-cancel" {
                assert_eq!(occupied, 0);
                let mut e = Exploration::default();
                e.observe(&s, &m).unwrap();
                assert!(!e.overcommit);
                if s.tasks
                    .get(1)
                    .is_some_and(|t| t.lifecycle == Lifecycle::Done)
                {
                    queued_cancelled = true;
                }
            }
            if name == "rsrc-zero-time" && s.tasks.len() == 2 && s.tasks[0].lifecycle.running() {
                assert_eq!(occupied, 1);
                zero_started |= s.time == 0;
            }
            if let Step::Next(xs) = step(s, &p, &m, &mut Room(1000)).unwrap() {
                pending.extend(xs);
            }
        }
        match name {
            "rsrc-release-fork" => assert!(released),
            "rsrc-cooperative" | "rsrc-immediate" => assert!(requested && acked),
            "rsrc-zero-time" => assert!(zero_started),
            "rsrc-queued-cancel" => assert!(queued_cancelled),
            _ => unreachable!(),
        }
    }
    let m = fixture("rsrc-half-open");
    let mut s = State::initial(&m, 0);
    s.time = 2;
    s.tasks = vec![
        Task {
            frozen_performance: None,
            stale: Vec::new(),
            binding: ir::BindingId(0),
            capability: ir::CapabilityId(0),
            executor: None,
            lifecycle: Lifecycle::Running {
                finish: 2,
                ack: None,
            },
        },
        Task {
            frozen_performance: None,
            stale: Vec::new(),
            binding: ir::BindingId(1),
            capability: ir::CapabilityId(1),
            executor: None,
            lifecycle: Lifecycle::Running {
                finish: 3,
                ack: None,
            },
        },
    ];
    let mut history = metrics::History::new(&m);
    history.tasks[0].start = Some(0);
    history.tasks[1].start = Some(2);
    let mut peak = metrics::Peak::new(&m);
    peak.resources[0] = 2.into();
    peak.observe(std::iter::once((&s, &history.tasks[..])), &m);
    assert_eq!(peak.resources[0], 2.into());
}

#[test]
fn resource_h05_atomic_candidate_set_in_both_use_orders() {
    let mut m = fixture("rsrc-atomic");
    // a=S holder; x=R,S; y=R. Queued x must not reserve its free R.
    m.actors[0].capacity = Some(3);
    m.capabilities.push(m.capabilities[0].clone());
    m.capabilities[2].uses.retain(|u| u.resource.0 == 0);
    m.capabilities[1].uses.retain(|u| u.resource.0 == 1);
    for reverse in [false, true] {
        if reverse {
            m.capabilities[0].uses.reverse();
        }
        m.metrics = None;
        let (mut s, p) = queued(&m);
        s.tasks.clear();
        s.results.resize(3, None);
        for (i, c) in [1, 0, 2].into_iter().enumerate() {
            s.tasks.push(Task {
                frozen_performance: None,
                stale: Vec::new(),
                binding: ir::BindingId(i),
                capability: ir::CapabilityId(c),
                executor: None,
                lifecycle: if i == 0 {
                    Lifecycle::Running {
                        finish: 3,
                        ack: None,
                    }
                } else {
                    Lifecycle::Queued
                },
            });
        }
        m.metrics = None;
        s.metric_history = None;
        assert_eq!(s.resource_occupancy(&m).unwrap(), [0, 1]);
        let Step::Next(xs) = step(s.clone(), &p, &m, &mut Room(100)).unwrap() else {
            panic!("y dispatch");
        };
        assert_eq!(xs.len(), 1);
        assert!(xs[0].tasks[2].lifecycle.running());
        assert_eq!(xs[0].tasks[1].lifecycle, Lifecycle::Queued);
        assert_eq!(s.resource_occupancy(&m).unwrap(), [0, 1]);
    }
}

#[test]
fn resource_h07_public_ir_guards_all_entries_and_replay() {
    let mut original = fixture("rsrc-over-demand");
    let span = original.process.span;
    original.simulation = Some(ir::SimulationSpec {
        replications: 2,
        seed: 1,
        confidence: Rational::decimal("0.95").unwrap(),
        span,
        replications_span: span,
        seed_span: span,
        confidence_span: span,
    });
    for fault in 0..7 {
        let mut m = original.clone();
        match fault {
            0 => m.capabilities[0].uses[0].resource = ir::ResourceId(999),
            1 => m.capabilities[0].uses[0].amount = 0,
            2 => {
                let u = m.capabilities[0].uses[0].clone();
                m.capabilities[0].uses.push(u);
            }
            3 => m.resources[0].name.clear(),
            4 => m.resources.push(m.resources[0].clone()),
            5 => m.resources[0].name = m.actors[0].name.clone(),
            _ => m.capabilities[0].uses[0].resource = ir::ResourceId(usize::MAX),
        }
        for global in [false, true] {
            if global {
                m.arrivals = Some(vec![0]);
            }
            assert!(
                explore(&m)
                    .diagnostics
                    .iter()
                    .any(|d| d.code == diag::U_SEM_0),
                "fault {fault} global {global}"
            );
            assert!(
                simulate(&m)
                    .diagnostics
                    .iter()
                    .any(|d| d.code == diag::U_SEM_0)
            );
            assert_eq!(
                crate::reference::explore(&m).stop,
                crate::reference::StopReason::SemanticAbort
            );
            assert_eq!(
                crate::reference::simulate(&m).stop,
                crate::reference::StopReason::SemanticAbort
            );
        }
    }
}

#[test]
fn resource_h13_reference_and_engine_refuse_unresolved_schedulers_without_completed_samples() {
    for (name, remove_fifo) in [("rsrc-global", true), ("rsrc-competition", false)] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca");
        let mut src = std::fs::read_to_string(path)
            .unwrap()
            .replace("verify safety no_resource_overcommit", "")
            .replace(
                "  bounds",
                "  simulation replications 2 seed 1 confidence 0.95\n  bounds",
            );
        if remove_fifo {
            src = src.replace("policy fifo tie_by case_id", "");
        }
        let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        let e = simulate(&m);
        let r = crate::reference::simulate(&m);
        for out in [e, r.outcome] {
            assert_eq!(out.completed_replications, 0);
            assert!(out.replications.is_empty());
            assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
        }
    }
}

#[test]
fn resource_deadlock_outcome_matches_reference_by_field() {
    for name in ["rsrc-atomic.broken-excess-demand", "rsrc-gate-after-wait"] {
        let m = fixture(name);
        let e = explore(&m);
        let mut r = crate::reference::explore(&m).outcome;
        r.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e.properties, r.properties);
        assert_eq!(e.diagnostics, r.diagnostics);
        assert_eq!(e.postprocessing_diagnostics, r.postprocessing_diagnostics);
        assert_eq!(e.unchecked, r.unchecked);
        fn compare(a: &serde_json::Value, b: &serde_json::Value, path: String) {
            match (a, b) {
                (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
                    for (k, v) in a {
                        compare(v, &b[k], format!("{path}/{k}"));
                    }
                }
                (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
                    assert_eq!(a.len(), b.len(), "{path}");
                    for (i, (a, b)) in a.iter().zip(b).enumerate() {
                        compare(a, b, format!("{path}/{i}"));
                    }
                }
                _ => assert_eq!(a, b, "{path}"),
            }
        }
        compare(
            &serde_json::to_value(e.metrics).unwrap(),
            &serde_json::to_value(r.metrics).unwrap(),
            name.into(),
        );
    }
}

#[test]
fn resource_review_shortages_without_a_row_fail_closed_instead_of_indexing() {
    let m = fixture("rsrc-over-demand");
    let cap = &m.capabilities[0];
    assert!(!cap.uses.is_empty());
    assert_eq!(
        trace::Capture::shortages(cap, &[]),
        Err(crate::explain::ReplayReason::StateMismatch)
    );
    let mut s = State::initial(&m, 0);
    s.tasks.clear();
    let rows = trace::Capture::new("", &m).model_resources(&s, &m).unwrap();
    assert_eq!(rows.len(), 1);
    // R capacity 1, nobody running: available 1 >= required 1, no shortage.
    assert_eq!(trace::Capture::shortages(cap, &rows), Ok(Vec::new()));
}
