use super::*;
use crate::integer_tests::{latent, model};

#[test]
fn integer_frame_snapshots_lazy_initial_tuples_and_limits() {
    let m = model(&latent(2));
    let f0 = replay::initial(&m);
    let mut f1 = f0.clone();
    f1.state_values[0] = StateValue::Integer(1);
    assert_ne!(f0.state_values, f1.state_values);
    assert_eq!(
        std::collections::HashSet::from([f0.state_values.clone(), f1.state_values]).len(),
        2
    );
    assert_ne!(StateValue::Integer(0), StateValue::Value(ValueId(0)));
    let r = explore(&m);
    assert_eq!(r.stop, StopReason::Complete);
    assert_eq!(
        r.statistics
            .initial_tuples
            .iter()
            .map(|t| t[0])
            .collect::<std::collections::HashSet<_>>(),
        std::collections::HashSet::from([
            StateValue::Integer(0),
            StateValue::Integer(1),
            StateValue::Integer(2)
        ])
    );
    for frames in [0, 1, 2, 3, 4] {
        let r = explore_with_limits(
            &m,
            Limits {
                frames,
                ..Limits::default()
            },
        );
        assert!(r.statistics.initial_copies <= frames);
        assert!(
            r.statistics
                .initial_tuples
                .iter()
                .all(|t| matches!(t[0], StateValue::Integer(0..=2)))
        );
    }
    let huge = model(&latent(u64::MAX));
    assert_eq!(
        explore_with_limits(
            &huge,
            Limits {
                frames: 1,
                ..Limits::default()
            }
        )
        .stop,
        StopReason::EnumerationLimit
    );
}

#[test]
fn integer_independent_range_bundle_keeps_frame_resources_results_and_counters() {
    let m = model(include_str!(
        "../../../../fixtures/int-atomic.broken-overflow/model.relca"
    ));
    let mut i = replay::interpreter(&m);
    let mut f = replay::initial(&m);
    let Stmt::Call(call) = &m.process.body[0].kind else {
        panic!()
    };
    i.create(&mut f, std::slice::from_ref(call), None).unwrap();
    let before = f.state_values.clone();
    assert!(i.range_attempt(&f, 0, false, None).unwrap());
    assert_eq!(f.state_values, before);
    assert!(f.tasks[0].person.is_none());
    assert!(f.tasks[0].start.is_none());
    assert!(f.trace.is_empty());
    assert_eq!(i.statistics.normal_completions, 0);
}

#[test]
fn integer_replay_independently_rejects_each_range_payload_and_source_tamper() {
    for src in [
        include_str!("../../../../fixtures/int-upper.broken-overflow/model.relca"),
        include_str!("../../../../fixtures/int-underflow.broken-underflow/model.relca"),
        include_str!("../../../../fixtures/int-atomic.broken-overflow/model.relca"),
    ] {
        let m = model(src);
        let explained = crate::explain::explain(src);
        let control = explained
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id == "state_in_range")
            .unwrap();
        assert_eq!(control.status, crate::explain::ReplayStatus::Verified);
        assert_eq!(control.steps.len(), 5);
        replay::replay(&m, &control.steps, "state_in_range", control.case_id).unwrap();
        for fault in 0..10 {
            let mut bad = control.steps.clone();
            let last = bad.last_mut().unwrap();
            let w = last.state_range_violation.as_mut().unwrap();
            match fault {
                0 => w.before = "19".into(),
                1 => w.operator = "=".into(),
                2 => w.operand = "7".into(),
                3 => w.attempted = "99".into(),
                4 => w.time += 1,
                5 => w.case_id = Some(2),
                6 => w.write_span.start += 1,
                7 => w.declaration_span.end -= 1,
                8 => last.time += 1,
                _ => last.case_id = Some(2),
            }
            assert!(
                replay::replay(&m, &bad, "state_in_range", control.case_id).is_err(),
                "fault {fault}"
            );
            assert!(
                replay::replay_choices(
                    &m,
                    &bad,
                    "state_in_range",
                    control.case_id,
                    &vec![None; bad.len()]
                )
                .is_err()
            );
        }
    }
}

#[test]
fn integer_arrivals_range_replays_without_legacy_case_aliasing() {
    let src = include_str!("../../../../fixtures/int-arrivals.broken-overflow/model.relca");
    let x = crate::explain::explain(src);
    let c = x
        .explanation
        .counterexamples
        .iter()
        .find(|c| c.id == "state_in_range")
        .unwrap();
    assert_eq!(
        c.status,
        crate::explain::ReplayStatus::Verified,
        "{:?}",
        c.reason
    );
}

#[test]
fn integer_sample_range_keeps_known_hits_and_unknown_continuation_independently() {
    let src = r#"model M version "0.2" {
        type T = enum { ok }
        observed case { n: 0..1 = 1; seen: bool = true }
        actor A { capacity 1; capability c() -> T { duration fixed 1min; result nondet { ok }; writes n; finish { n += 1 } } }
        process P { a = A.c(); end done }
        bounds { cases 1; live_tasks 1; clock 5min }
        simulation replications 2 seed 17 confidence 0.9
        verify probability reached seen == true >= 1
        verify probability reached n == 0 >= 0
    }"#;
    let m = model(src);
    let a = crate::engine::simulate(&m);
    let r = simulate(&m);
    assert_eq!(r.stop, StopReason::Complete);
    assert_eq!(a.replications, r.outcome.replications);
    for rep in &r.outcome.replications {
        assert_eq!(
            rep.cases[0].queries,
            vec![QueryTruth::Hit, QueryTruth::Unknown]
        );
    }
    assert_eq!(a.range_stops.len(), 2);
    assert_eq!(r.outcome.range_stops.len(), 2);
    for split in ["-\n0", "+\n0", "- 0"] {
        let s = latent(1).replace("h: 0..1", &format!("h: 0..1 = {split}"));
        let errors = crate::syntax::parse(&s).unwrap_err();
        assert!(errors.iter().any(|e| e.code == diag::U_SYN_0));
    }
}

// Review fix 3: a trace that claims a successful Start or Finish whose write
// leaves the declared range is an illegal transition, never a panic and never
// a silent out-of-range state.
#[test]
fn review_fix_replay_rejects_out_of_range_successful_start_and_finish() {
    let mut seen = Vec::new();
    let start = "model Count version \"0.2\" {\n  type T = enum { ok, no }\n  observed case { n: 0..1 = 1 }\n  actor A { capacity 1; capability inc() -> T { duration fixed 1min; result nondet { ok }; start { n += 1 } } }\n  process P { a = A.inc(); end done }\n  bounds { cases 1; live_tasks 2; clock 10min }\n  verify reachable end.done\n}\n";
    for (src, kind, phase) in [
        (
            include_str!("../../../../fixtures/int-underflow.broken-underflow/model.relca"),
            crate::explain::Kind::Finish,
            "finish",
        ),
        (
            include_str!("../../../../fixtures/int-upper.broken-overflow/model.relca"),
            crate::explain::Kind::Finish,
            "finish",
        ),
        (start, crate::explain::Kind::Start, "start"),
    ] {
        let m = model(src);
        let explained = crate::explain::explain(src);
        let control = explained
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id == "state_in_range")
            .unwrap();
        assert_eq!(control.status, crate::explain::ReplayStatus::Verified);
        replay::replay(&m, &control.steps, "state_in_range", control.case_id).unwrap();
        let mut bad = control.steps.clone();
        let last = bad.last_mut().unwrap();
        assert_eq!(last.kind, crate::explain::Kind::RangeViolation);
        assert_eq!(last.state_range_violation.as_ref().unwrap().phase, phase);
        // A well-formed task stage: located at the call, with its one
        // business event, so the replay reaches the write itself.
        let Stmt::Call(call) = &m.process.body[0].kind else {
            panic!()
        };
        let attempted = last
            .state_range_violation
            .as_ref()
            .unwrap()
            .attempted
            .clone();
        let n = last
            .values
            .observed
            .iter_mut()
            .find(|v| v.name == "n")
            .unwrap();
        assert_ne!(n.value, attempted);
        n.value = attempted.into();
        last.kind = kind;
        last.state_range_violation = None;
        last.location.span = call.span;
        if kind == crate::explain::Kind::Finish {
            last.result = Some("ok".into());
        }
        last.events = vec![crate::explain::Event::Business(
            crate::explain::BusinessEvent {
                repetition: None,
                kind,
                case_id: last.case_id,
                binding: last.binding.clone(),
                actor: last.actor.clone(),
                capability: last.capability.clone(),
                executor: last.executor.clone(),
                result: last.result.clone(),
                location: last.location.clone(),
                ack_at: None,
            },
        )];
        let got = std::panic::catch_unwind(|| {
            replay::replay(&m, &bad, "state_in_range", control.case_id)
        });
        seen.push(format!("{phase}: {:?}", got.map_err(|_| "panic")));
    }
    assert_eq!(
        seen,
        [
            "finish: Ok(Err(IllegalTransition))",
            "finish: Ok(Err(IllegalTransition))",
            "start: Ok(Err(IllegalTransition))"
        ]
    );
}
