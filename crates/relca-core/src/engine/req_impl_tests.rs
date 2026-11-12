//! ADR 0026 private obligations, independent of acceptance oracle constants.
use super::*;

fn model(source: &str) -> ir::Model {
    crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap()
}

#[test]
fn req_simulation_metric_columns_are_independently_equal() {
    fn equal(a: &serde_json::Value, b: &serde_json::Value, path: &str) {
        match (a, b) {
            (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
                assert_eq!(a.len(), b.len(), "keys at {path}");
                for (k, a) in a {
                    equal(a, &b[k], &format!("{path}/{k}"));
                }
            }
            (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
                assert_eq!(a.len(), b.len(), "length at {path}");
                for (k, (a, b)) in a.iter().zip(b).enumerate() {
                    equal(a, b, &format!("{path}/{k}"));
                }
            }
            _ => assert_eq!(a, b, "at {path}"),
        }
    }
    let source = include_str!("../../../../fixtures/req-wait-arrivals/model.relca").replacen(
        "  bounds",
        "  simulation replications 4 seed 0 confidence 0.5\n  bounds",
        1,
    );
    let m = model(&source);
    let reference = crate::reference::simulate(&m).outcome;
    let optimized = simulate(&m);
    equal(
        &serde_json::to_value(&reference.metric_template).unwrap(),
        &serde_json::to_value(&optimized.metric_template).unwrap(),
        "template",
    );
    equal(
        &serde_json::to_value(&reference).unwrap(),
        &serde_json::to_value(&optimized).unwrap(),
        "sample",
    );
}

fn start_model() -> ir::Model {
    model(include_str!("../../../../fixtures/req-start/model.relca"))
}

#[test]
fn req_cancel_availability_rejects_during_lower_not_late_runtime() {
    let source = r#"model M version "0.2" { type T=enum {ok,denied} observed case {flag:bool=false} actor A {capacity 1; capability fast() -> T {duration fixed 1min; result nondet {ok}; requires flag == true else denied; cancel forbidden_after_start}} actor B {capacity 1; capability slow() -> T {duration fixed 5min; result nondet {ok}; cancel immediate}} process P {fork f {a=A.fast(); b=B.slow()}; join any f where result == denied tie by [a,b] remaining cancel_then_drain if none end rejected; if b == ok {end done} else {end rejected}} bounds {cases 1; live_tasks 2; clock 8min} verify safety no_resource_overcommit }"#;
    let source = source.replace("if none end", "if none ->");
    let ast = crate::syntax::parse(&source).unwrap();
    let errors =
        crate::lower::lower(&ast).expect_err("cancelled binding must be statically unavailable");
    assert!(
        errors
            .iter()
            .any(|d| d.code == diag::U_SEM_0 && d.message.contains("not definitely available"))
    );
}

#[test]
fn req_public_ir_new_fields_are_independently_rejected_without_panics() {
    let base = start_model();
    for fault in 0..9 {
        let mut m = base.clone();
        match fault {
            0 => m.capabilities[0].requires.as_mut().unwrap().otherwise = ir::ValueId(999),
            1 => {
                m.capabilities[0].requires.as_mut().unwrap().condition =
                    ir::Cond::StateEq(ir::StateMatch {
                        variable: ir::StateVarId(999),
                        value: ir::StateValue::None,
                    })
            }
            2 => {
                m.capabilities[0].requires.as_mut().unwrap().condition =
                    ir::Cond::Eq(ir::ResultMatch {
                        binding: ir::BindingId(0),
                        value: ir::ValueId(0),
                    })
            }
            3 => m.state_variables[0].kind = ir::StateKind::LatentCase,
            4 => m.capabilities[0].start_writes[0].variable = ir::StateVarId(999),
            5 => m.capabilities[0].start_writes[0].source = ir::WriteSource::Result,
            6 => {
                let w = m.capabilities[0].start_writes[0];
                m.capabilities[0].start_writes.push(w);
            }
            7 => {
                m.capabilities[0].start_writes[0].source =
                    ir::WriteSource::Constant(ir::StateValue::Value(ir::ValueId(999)))
            }
            _ => m.capabilities[0].requires.as_mut().unwrap().condition = ir::Cond::And(vec![]),
        }
        let optimized = explore(&m);
        assert!(
            optimized
                .diagnostics
                .iter()
                .any(|d| d.code == diag::U_SEM_0),
            "fault {fault}"
        );
        assert_eq!(
            crate::reference::explore(&m).stop,
            crate::reference::StopReason::SemanticAbort,
            "fault {fault}"
        );
    }
}

#[test]
fn req_sleep_identity_none_preserves_prior_hash_and_debug_stream() {
    use std::hash::{Hash, Hasher};
    fn hash(s: &State) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    }
    let m = start_model();
    let a = State::initial(&m, 0);
    assert!(!format!("{a:?}").contains("sleep_until"));
    let mut h = std::collections::hash_map::DefaultHasher::new();
    a.state_values.hash(&mut h);
    a.initialized.hash(&mut h);
    a.monitors.hash(&mut h);
    a.time.hash(&mut h);
    a.pc.hash(&mut h);
    a.results.hash(&mut h);
    a.tasks.hash(&mut h);
    a.waiting.hash(&mut h);
    a.adopted.hash(&mut h);
    a.work.hash(&mut h);
    a.result_at.hash(&mut h);
    if let Some(x) = &a.metric_history {
        x.hash(&mut h);
    }
    if let Some(x) = &a.obligations {
        x.hash(&mut h);
    }
    if let Some(x) = &a.contest {
        x.hash(&mut h);
    }
    assert_eq!(hash(&a), h.finish());
    let mut b = a.clone();
    b.sleep_until = Some(2);
    let mut c = a.clone();
    c.sleep_until = Some(3);
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(hash(&a), hash(&b));
    assert_ne!(hash(&b), hash(&c));
    assert!(format!("{b:?}").contains("sleep_until: 2"));
}

#[test]
fn req_sleep_enter_wake_refuse_room_zero_one_and_overflow() {
    let source = r#"model M version "0.2" { process P { wait 2min; end done } bounds { cases 1; live_tasks 0; clock 5min } verify reachable end.done }"#;
    let m = model(source);
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    let initial = State::initial(&m, entry);
    assert!(matches!(
        step_observed(
            initial.clone(),
            &program,
            &m,
            &mut Room(0),
            &mut Exploration::default()
        ),
        Err(Halt::Budget)
    ));
    let Step::Next(mut xs) = step_observed(
        initial.clone(),
        &program,
        &m,
        &mut Room(1),
        &mut Exploration::default(),
    )
    .unwrap() else {
        panic!()
    };
    let mut asleep = xs.remove(0);
    assert_eq!(asleep.sleep_until, Some(2));
    assert_eq!((asleep.time, asleep.work), (0, 0));
    assert!(matches!(
        step_observed(
            asleep.clone(),
            &program,
            &m,
            &mut Room(0),
            &mut Exploration::default()
        ),
        Err(Halt::Budget)
    ));
    asleep.time = 2;
    assert!(matches!(
        step_observed(
            asleep.clone(),
            &program,
            &m,
            &mut Room(0),
            &mut Exploration::default()
        ),
        Err(Halt::Budget)
    ));
    assert!(matches!(
        step_observed(
            asleep,
            &program,
            &m,
            &mut Room(1),
            &mut Exploration::default()
        ),
        Ok(Step::Next(_))
    ));
    let mut overflow = initial;
    overflow.time = u64::MAX;
    assert!(matches!(
        step_observed(
            overflow,
            &program,
            &m,
            &mut Room(1),
            &mut Exploration::default()
        ),
        Err(Halt::Unsupported("wait time overflow"))
    ));
    let m = model(include_str!(
        "../../../../fixtures/req-start.broken-false/model.relca"
    ));
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    let Step::Next(mut xs) = step_observed(
        State::initial(&m, entry),
        &program,
        &m,
        &mut Room(1),
        &mut Exploration::default(),
    )
    .unwrap() else {
        panic!()
    };
    let queued = xs.remove(0);
    let mut e = Exploration::default();
    assert!(matches!(
        step_observed(queued.clone(), &program, &m, &mut Room(0), &mut e),
        Err(Halt::Budget)
    ));
    assert!(e.refusals.is_empty());
    let Step::Next(xs) = step_observed(queued, &program, &m, &mut Room(1), &mut e).unwrap() else {
        panic!()
    };
    assert_eq!(xs.len(), 1);
    assert_eq!(e.refusals.len(), 1);
    assert!(!xs[0].tasks[0].lifecycle.running());
    // Full public exploration finds its exact held-state threshold rather than
    // changing the expected state count to fit the implementation.
    let k = (1..128)
        .find(|b| !explore_with_budget(&m, *b).truncated)
        .unwrap();
    assert!(explore_with_budget(&m, k - 1).truncated);
    assert!(!explore_with_budget(&m, k).truncated);
    assert_eq!(explore_with_budget(&m, k), explore_with_budget(&m, k + 1));
}

#[test]
fn req_immediate_ack_reserves_now_and_expands_both_ack_orders() {
    let m = model(
        r#"model M version "0.2" { type T = enum { ok } actor A { capacity 2; capability run() -> T { duration fixed 10min; result nondet { ok }; cancel immediate } } process P { fork f { a=A.run(); b=A.run() }; join all f; end done } bounds { cases 1; live_tasks 2; clock 12min } verify reachable end.done }"#,
    );
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    let Step::Next(mut xs) = step_observed(
        State::initial(&m, entry),
        &program,
        &m,
        &mut Room(1),
        &mut Exploration::default(),
    )
    .unwrap() else {
        panic!()
    };
    let mut s = xs.remove(0);
    s.time = 3;
    s.work = 6;
    for t in &mut s.tasks {
        t.lifecycle = Lifecycle::Running {
            finish: 10,
            ack: None,
        };
    }
    let s = request_cancel(vec![s], 0, &m, &mut Room(0))
        .unwrap()
        .remove(0);
    let s = request_cancel(vec![s], 1, &m, &mut Room(0))
        .unwrap()
        .remove(0);
    assert!(
        s.tasks
            .iter()
            .all(|t| matches!(t.lifecycle, Lifecycle::Running { ack: Some(3), .. }))
    );
    let Step::Next(xs) = step_observed(
        s.clone(),
        &program,
        &m,
        &mut Room(2),
        &mut Exploration::default(),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(xs.len(), 2);
    assert_eq!(
        xs.iter()
            .map(|s| s
                .tasks
                .iter()
                .position(|t| t.lifecycle == Lifecycle::Done)
                .unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1])
    );
    for s in xs {
        assert_eq!((s.time, s.work), (3, 6));
        let Step::Next(xs) =
            step_observed(s, &program, &m, &mut Room(1), &mut Exploration::default()).unwrap()
        else {
            panic!()
        };
        assert!(xs[0].tasks.iter().all(|t| t.lifecycle == Lifecycle::Done));
        assert_eq!(xs[0].occupancy(&m), vec![0]);
    }
    assert!(matches!(
        step_observed(s, &program, &m, &mut Room(1), &mut Exploration::default()),
        Err(Halt::Budget)
    ));
}
