//! Private obligations supplied with ADR 0040 acceptance, not new fixture oracles.
use super::*;
use crate::contest::ContestTarget;

fn model() -> ir::Model {
    crate::lower::lower(
        &crate::syntax::parse(include_str!("../../../../fixtures/sov-basic/model.relca")).unwrap(),
    )
    .unwrap()
}

#[test]
fn target_time_and_end_snapshot_are_state_identity_not_only_hash_inputs() {
    let m = model();
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    assert!(!program.is_empty());
    let mut a = State::initial(&m, entry);
    a.time = 10;
    a.contest.as_mut().unwrap().routes[0].target_at = Some(3);
    let mut b = a.clone();
    b.contest.as_mut().unwrap().routes[0].target_at = Some(5);
    assert_ne!(a, b);
    assert_eq!(std::collections::HashSet::from([a.clone(), b]).len(), 2);
    let mut states = std::collections::HashSet::new();
    for open in [None, Some(false), Some(true)] {
        let mut s = a.clone();
        s.contest.as_mut().unwrap().end_open = open;
        states.insert(s);
    }
    assert_eq!(states.len(), 3);
    // The frozen legacy debug vocabulary must not be widened by these fields.
    let debug = format!("{a:?}");
    assert!(!debug.contains("target_at") && !debug.contains("end_open"));
}

#[test]
fn filing_reserves_two_before_cloning_and_expands_one_choice_at_a_time() {
    let src = include_str!("../../../../fixtures/sov-basic/model.relca");
    let line = "contest Vendor on Ledger.post.commit route Clerk.recheck reversible_by [a1] respond_by 5min";
    for n in [2, 12] {
        let src = src.replace(line, &format!("{line};").repeat(n));
        let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        let entry = compile(&m.process.body, 0, &mut vec![Instruction::Stop]);
        let mut s = State::initial(&m, entry);
        s.time = 3;
        for route in &mut s.contest.as_mut().unwrap().routes {
            route.target_at = Some(3);
        }
        for budget in [0, 1] {
            let mut room = Room(budget);
            let mut facts = Exploration::default();
            assert!(matches!(
                contest_control(&s, &m, &mut room, &mut facts),
                Err(Halt::Budget)
            ));
            assert_eq!(facts.contest_clones, 0);
        }
        let mut room = Room(2);
        let mut facts = Exploration::default();
        let Some(Step::Next(first)) = contest_control(&s, &m, &mut room, &mut facts).unwrap()
        else {
            panic!("filing choice");
        };
        assert_eq!(room.0, 0);
        assert_eq!(facts.contest_clones, 2);
        assert_eq!(first.len(), 2);
        assert!(first.iter().all(|s| {
            s.contest
                .as_ref()
                .unwrap()
                .routes
                .iter()
                .filter(|r| r.filed.is_some())
                .count()
                == 1
        }));
        if n == 2 {
            let mut joint = std::collections::BTreeSet::new();
            for state in first {
                let Some(Step::Next(second)) =
                    contest_control(&state, &m, &mut Room(2), &mut Exploration::default()).unwrap()
                else {
                    panic!("second choice");
                };
                for s in second {
                    joint.insert(
                        s.contest
                            .unwrap()
                            .routes
                            .iter()
                            .map(|r| r.filed.unwrap())
                            .collect::<Vec<_>>(),
                    );
                }
            }
            assert_eq!(
                joint,
                std::collections::BTreeSet::from([
                    vec![false, false],
                    vec![false, true],
                    vec![true, false],
                    vec![true, true]
                ])
            );
        }
    }
}

#[test]
fn isolated_new_target_and_forged_certificates_fail_before_exploration() {
    let good = model();
    let cap = match good.contests()[0].target {
        ContestTarget::Commit(c) => c,
        _ => unreachable!(),
    };
    let assert_rejected = |m: ir::Model| {
        let result = std::panic::catch_unwind(|| {
            let out = explore(&m);
            assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
            assert_eq!(
                crate::reference::explore(&m).stop,
                crate::reference::StopReason::SemanticAbort
            );
        });
        assert!(result.is_ok());
    };
    let mut m = good.clone();
    m.responsibility.as_mut().unwrap().contests[0].target =
        ContestTarget::Commit(ir::CapabilityId(usize::MAX));
    assert_rejected(m);
    let mut m = good.clone();
    m.responsibility.as_mut().unwrap().standings[0].target =
        ContestTarget::Commit(ir::CapabilityId(usize::MAX));
    assert_rejected(m);
    let mut m = good.clone();
    let standing = m.responsibility.as_ref().unwrap().standings[0].clone();
    m.responsibility.as_mut().unwrap().standings.push(standing);
    assert_rejected(m);
    let mut m = good.clone();
    m.responsibility.as_mut().unwrap().contest_paths[0].rules = "forged".into();
    assert_rejected(m);
    let mut m = good.clone();
    m.responsibility.as_mut().unwrap().contests[0].route = cap;
    m.process.body.clear();
    assert_rejected(m);
    let mut m = good.clone();
    let call = (**crate::responsibility::calls(&good)
        .values()
        .find(|c| c.capability == cap)
        .unwrap())
    .clone();
    m.process.body.push(ir::Statement {
        kind: ir::Stmt::Call(call),
        span: good.process.body[0].span,
    });
    assert_rejected(m);
}

#[test]
fn route_finish_and_requires_refusal_never_write_ordinary_result_slots() {
    let mut m = model();
    let entry = compile(&m.process.body, 0, &mut vec![Instruction::Stop]);
    let mut s = State::initial(&m, entry);
    let ordinary = s.results.clone();
    s.time = 4;
    finish_site(&mut s, m.contest_site(0), Some(ir::ValueId(0)), &m);
    assert_eq!(s.results, ordinary);
    assert_eq!(s.contest.as_ref().unwrap().routes[0].response_at, Some(4));
    let route = m.contests()[0].route;
    let src = include_str!("../../../../fixtures/sov-basic/model.relca");
    let rejected = src.replace(
        "capability recheck() -> Verdict {",
        "capability recheck() -> Verdict { requires false else fail;",
    );
    // The DSL's condition fragment does not admit a bare boolean; use an
    // observed boolean rather than weakening that syntax boundary.
    let rejected = rejected
        .replace("requires false", "requires ready == true")
        .replace(
            "  people",
            "  observed case { ready: bool = false }\n  people",
        );
    m = crate::lower::lower(&crate::syntax::parse(&rejected).unwrap()).unwrap();
    s = State::initial(&m, entry);
    s.initialized = true;
    s.time = 3;
    s.contest.as_mut().unwrap().routes[0] = RouteState {
        target_at: Some(3),
        filed: Some(true),
        queued: true,
        ..RouteState::default()
    };
    s.tasks = vec![Task {
        frozen_performance: None,
        stale: Vec::new(),
        binding: m.contest_site(0),
        capability: route,
        executor: None,
        lifecycle: Lifecycle::Queued,
    }];
    // Sleep allows only the queued route to execute without inventing a
    // process result or moving a normal Wait's pc/adoption.
    s.sleep_until = Some(8);
    let program = vec![
        Instruction::Sleep {
            duration: 5,
            next: 1,
        },
        Instruction::Stop,
    ];
    s.pc = 0;
    let Step::Next(next) =
        step_observed(s, &program, &m, &mut Room(100), &mut Exploration::default()).unwrap()
    else {
        panic!("route refusal");
    };
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].results.len(), m.process.bindings.len());
    assert!(next[0].results.iter().all(Option::is_none));
    assert_eq!(
        next[0].contest.as_ref().unwrap().routes[0].violation,
        Some("refused")
    );
    assert_eq!(next[0].pc, 0);
}

#[test]
fn normal_contest1_name_cannot_alias_synthetic_contest_colon1() {
    let src =
        include_str!("../../../../fixtures/sov-basic/model.relca").replace("approval", "contest1");
    let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
    assert_eq!(m.site_name(ir::BindingId(0)), "contest1");
    assert_eq!(m.site_name(m.contest_site(0)), "contest:1");
    let e = explore(&m);
    let mut r = crate::reference::explore(&m);
    assert_eq!(r.stop, crate::reference::StopReason::Complete);
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    assert_eq!(e, r.outcome);
    assert_eq!(e.terminals[1].elapsed.min, 3);
}

#[test]
fn independent_reference_simulation_refuses_the_observed_commit_choice() {
    let src = include_str!("../../../../fixtures/sov-sim/model.relca");
    let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
    let r = crate::reference::simulate(&m);
    // Reference Complete also denotes a completely classified nondeterminism
    // refusal. Inspect diagnostics, not StopReason alone.
    assert_eq!(r.stop, crate::reference::StopReason::Complete);
    assert!(r.outcome.diagnostics.iter().any(|d| d.code == diag::U_SEM_0
        && d.message.contains("time 3")
        && d.message.contains("contest")));
}

#[test]
fn legacy_end_only_pre_end_deadlock_keeps_zero_open_cases() {
    let src = r#"model M version "0.2" {
      type R = enum { ok }
      people [p]
      actor A { capacity 0; capability go() -> R { duration fixed 1min; result nondet { ok } } }
      actor Review { executors [p]; capability reply() -> R { duration fixed 1min; result nondet { ok } } }
      actor V { control party; standing over [end.done] basis "old" }
      process P { x = A.go(); end done }
      bounds { cases 1; live_tasks 2; clock 5min }
      verify safety no_resource_overcommit
      contract C { contest V on end.done route Review.reply reversible_by [p] respond_by 2min }
    }"#;
    let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
    let e = explore(&m);
    let mut r = crate::reference::explore(&m);
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    assert_eq!(e, r.outcome);
    let summary = serde_json::to_value(e.contest_summary).unwrap();
    assert_eq!(
        summary["business_ended_open"],
        serde_json::json!({"min":"0","max":"0","reason":null})
    );
}

#[test]
fn legacy_zero_capacity_route_is_deadlock_not_successful_execution_close() {
    let src =
        include_str!("../../../../fixtures/ctx-contest-branches.broken-zero-capacity/model.relca");
    let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
    let e = explore(&m);
    let mut r = crate::reference::explore(&m);
    assert!(
        r.outcome
            .diagnostics
            .iter()
            .any(|d| d.code == diag::W_DEADLOCK_0)
    );
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    assert_eq!(e, r.outcome);
}
