use super::*;
use crate::integer_tests::{latent, model};

#[test]
fn integer_state_key_and_lazy_initial_room_endpoints() {
    let m = model(&latent(2));
    let seed = State::initial(&m, 0);
    let mut other = seed.clone();
    other.state_values[0] = ir::StateValue::Integer(1);
    assert_ne!(seed, other);
    assert_eq!(HashSet::from([seed.clone(), other]).len(), 2);
    let mut enumeration = seed.clone();
    enumeration.state_values[0] = ir::StateValue::Value(ir::ValueId(0));
    assert_ne!(seed, enumeration);
    for budget in [0, 1, 2, 3, 4] {
        let mut e = Exploration::default();
        let mut room = Room(budget);
        let actual = initialize_latents(&seed, &m, &mut room, &mut e);
        if budget < 3 {
            assert!(matches!(actual, Err(Halt::Budget)));
        } else {
            let xs = actual.unwrap();
            assert_eq!(
                xs.iter().map(|s| s.state_values[0]).collect::<HashSet<_>>(),
                HashSet::from([
                    ir::StateValue::Integer(0),
                    ir::StateValue::Integer(1),
                    ir::StateValue::Integer(2)
                ])
            );
            assert!(xs.iter().all(|s| s.initialized));
            assert_eq!(room.0, budget - 3);
        }
        assert!(e.initial_clones <= budget);
    }
    let singleton = model(&latent(0));
    let mut e = Exploration::default();
    assert_eq!(
        initialize_latents(
            &State::initial(&singleton, 0),
            &singleton,
            &mut Room(1),
            &mut e
        )
        .unwrap()
        .len(),
        1
    );
    assert_eq!(e.initial_clones, 1);
    let fixed = model(&latent(0).replace("h: 0..0", "h: 0..0 = 0"));
    assert!(State::initial(&fixed, 0).initialized);
}

#[test]
fn integer_start_finish_and_kernel_fail_atomically_without_events_or_acquisition() {
    for start in [false, true] {
        let src = include_str!("../../../../fixtures/int-atomic.broken-overflow/model.relca");
        let src = if start {
            src.replace("finish {", "start {")
                .replace("writes [flag,n];", "")
        } else {
            src.into()
        };
        let m = model(&src);
        let mut p = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut p);
        let mut s = State::initial(&m, entry);
        let mut e = Exploration::default();
        let mut room = Room(100);
        let mut attempts = 0;
        for _ in 0..10 {
            let before = s.clone();
            let Step::Next(xs) = step_observed(s.clone(), &p, &m, &mut room, &mut e).unwrap()
            else {
                panic!("must stop on attempted write");
            };
            if xs.is_empty() {
                assert_eq!(before, s);
                assert_eq!(e.ranges.len(), 1);
                assert_eq!(s.occupancy(&m).iter().sum::<u64>(), u64::from(!start));
                assert!(s.results.iter().all(Option::is_none));
                assert!(
                    !e.state_events
                        .iter()
                        .any(|(_, b, _)| matches!(b, Bundle::Finish(..)))
                );
                if start {
                    assert!(
                        !e.state_events
                            .iter()
                            .any(|(_, b, _)| matches!(b, Bundle::Start(..)))
                    );
                }
                attempts += 1;
                break;
            }
            s = xs.into_iter().next().unwrap();
        }
        assert_eq!(attempts, 1);
    }
    let m = model(include_str!(
        "../../../../fixtures/int-kernel-finish.broken-overflow/model.relca"
    ));
    assert_eq!(
        explore(&m).properties.last().unwrap().verdict,
        Verdict::Violated
    );
}

#[test]
fn integer_direct_ir_rejects_mixed_values_duplicate_property_and_kernel() {
    let good = model(include_str!("../../../../fixtures/int-upper/model.relca"));
    for fault in 0..7 {
        let mut m = good.clone();
        match fault {
            0 => m.state_variables[0].initial = ir::InitialValue::Fixed(ir::StateValue::Integer(3)),
            1 => {
                m.state_variables[0].initial =
                    ir::InitialValue::Fixed(ir::StateValue::Value(ir::ValueId(0)))
            }
            2 => m.verify.push(m.verify.last().unwrap().clone()),
            3 => {
                m.verify.pop();
            }
            4 => {
                m.kernels.push(ir::Kernel {
                    name: "bad".into(),
                    basis: None,
                    given: vec![ir::StateVarId(0)],
                    rows: vec![],
                });
            }
            5 => {
                if let ir::WriteSource::Integer { operand, .. } =
                    &mut m.capabilities[0].writes[0].source
                {
                    *operand = i128::MAX;
                }
            }
            _ => {
                m.kernels.push(ir::Kernel {
                    name: "numeric".into(),
                    basis: None,
                    given: vec![],
                    rows: vec![ir::KernelRow {
                        values: vec![],
                        branches: vec![ir::KernelBranch {
                            probability: Rational::one(),
                            result: ir::ValueId(0),
                            overrides: vec![(ir::StateVarId(0), ir::StateValue::Integer(1))],
                        }],
                    }],
                });
            }
        }
        assert!(validate(&m).is_err(), "fault {fault}");
        assert_eq!(
            crate::reference::explore(&m).stop,
            crate::reference::StopReason::SemanticAbort
        );
    }
    let m = model(include_str!(
        "../../../../fixtures/int-set-stop.broken-outward/model.relca"
    ));
    assert!(validate(&m).is_ok());
    assert_eq!(
        explore(&m).properties.last().unwrap().verdict,
        Verdict::Violated
    );
}

#[test]
fn integer_failed_finish_preserves_simultaneous_cancellation_alternative() {
    let m = model(include_str!(
        "../../../../fixtures/int-upper.broken-overflow/model.relca"
    ));
    let mut p = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut p);
    let mut s = State::initial(&m, entry);
    s.time = 1;
    s.waiting = true;
    s.tasks.push(Task {
        frozen_performance: None,
        stale: Vec::new(),
        executor: None,
        binding: ir::BindingId(0),
        capability: ir::CapabilityId(0),
        lifecycle: Lifecycle::Running {
            finish: 1,
            ack: Some(1),
        },
    });
    let mut e = Exploration::default();
    let Step::Next(xs) = step_observed(s.clone(), &p, &m, &mut Room(10), &mut e).unwrap() else {
        panic!()
    };
    assert_eq!(xs.len(), 1);
    assert_eq!(xs[0].tasks[0].lifecycle, Lifecycle::Done);
    assert!(xs[0].results[0].is_none());
    assert_eq!(xs[0].state_values, s.state_values);
    assert_eq!(e.ranges.len(), 1);
}

// Review fix 2: an engine abort after a range witness was found keeps the
// witness. state_in_range stays violated (as in arrivals and the reference);
// the other properties stay unsupported.
#[test]
fn review_fix_legacy_abort_keeps_known_state_in_range_witness() {
    let m = model(include_str!(
        "../../../../fixtures/int-upper.broken-overflow/model.relca"
    ));
    let found = explore(&m);
    assert_eq!(found.state_range_violations.len(), 1);
    let at = |o: &outcome::Outcome, p: ir::Property| {
        o.properties
            .iter()
            .find(|x| x.property == p)
            .unwrap()
            .verdict
    };
    let e = Exploration {
        ranges: found.state_range_violations.clone(),
        ..Exploration::default()
    };
    let out = e.abort("test abort", &m);
    assert_eq!(at(&out, ir::Property::StateInRange), Verdict::Violated);
    assert_eq!(
        at(&out, ir::Property::NoResourceOvercommit),
        Verdict::Unsupported
    );
    assert_eq!(out.state_range_violations, found.state_range_violations);
    // Without a witness the abort decides nothing about the range property.
    let none = Exploration::default().abort("test abort", &m);
    assert_eq!(at(&none, ir::Property::StateInRange), Verdict::Unsupported);
}
