//! Review findings on ADR 0015 (addendum 2): each test pins one ruling.
use relca_core::{
    diff, engine, ir, lower,
    outcome::{CaseTerminalKind, Outcome},
    reference::{self, StopReason},
    report, syntax,
};

fn model(src: &str) -> ir::Model {
    let ast = syntax::parse(src).unwrap_or_else(|d| panic!("parse {d:?}"));
    lower::lower(&ast).unwrap_or_else(|d| panic!("lower {d:?}"))
}

/// Both runtimes, and the reference must agree with the engine exactly.
fn both(src: &str) -> Outcome {
    let m = model(src);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, StopReason::Complete);
    // Model diagnostics (W-CUT-0) come from lower; compare the runtime facts.
    let (mut x, mut y) = (e.clone(), r.outcome);
    x.diagnostics.clear();
    y.diagnostics.clear();
    assert_eq!(x, y);
    e
}

fn range(p: &Option<relca_core::probability::ProbabilityRange>) -> (String, String) {
    let p = p.as_ref().expect("probability");
    (p.min.to_string(), p.max.to_string())
}

fn s(a: &str, b: &str) -> (String, String) {
    (a.into(), b.into())
}

// ADR 0017 replaces MAJOR-1 refusal with a nonempty case-local comparison.
#[test]
fn review_single_case_arrivals_route_compares_configurations() {
    let base = include_str!("../../../fixtures/config-diff-parallel/model.relca");
    let invocation: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/config-diff-parallel/invocation.json"
    ))
    .unwrap();
    let from = invocation["from"].as_str().unwrap();
    let to = invocation["to"].as_str().unwrap();
    for clause in [
        "arrivals fixed [0min]",
        "policy fifo tie_by case_id",
        "queue capacity 1 on_overflow reject",
    ] {
        let src = base.replace("  bounds", &format!("  {clause}\n  bounds"));
        let v = serde_json::to_value(diff::diff(&src, from, to)).unwrap();
        assert_eq!(v["verdict"], "satisfied", "{clause}: {v}");
        assert_eq!(v["terminals"], serde_json::json!([]));
        assert_eq!(v["case_outcomes"].as_array().unwrap().len(), 1);
        assert!(
            !v["case_outcomes"][0]["terminals"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let v = serde_json::to_value(report::check_configuration(&src, from)).unwrap();
        assert_eq!(v["verdict"], "satisfied", "{clause}");
        assert_eq!(v["case_outcomes"].as_array().unwrap().len(), 1);
        let v = serde_json::to_value(report::check_configuration(&src, "Unknown")).unwrap();
        assert_eq!(v["verdict"], "invalid");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-NAME-0")
        );
    }
    // The legacy route still compares.
    let v = serde_json::to_value(diff::diff(base, from, to)).unwrap();
    assert_ne!(v["verdict"], "unsupported");
}

// MAJOR-2: FIFO compares the tasks queued at each start. A task that enters
// the queue later in the same instant (after a same-time finish) is not
// compared, so case 1 may wait behind case 2 here.
#[test]
fn review_fifo_compares_only_tasks_queued_at_each_start() {
    let src = r#"model FifoSameTick version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false }
  actor B { capacity 2; capability prep() -> T { duration fixed 1min; result nondet { ok } } }
  actor A { capacity 1; capability run() -> T { duration fixed 2min; result nondet { ok }; writes hit; finish { hit = true } } }
  process Flow { p = B.prep(); r = A.run(); end done }
  arrivals fixed [0min, 0min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 1; clock 20min }
  verify probability reached hit == true within 3min >= 1
}"#;
    let out = both(src);
    let cases = out.case_outcomes.as_ref().unwrap();
    for c in cases {
        let t = &c.terminals[0];
        assert!(matches!(t.kind, CaseTerminalKind::End(_)));
        let at = t.completed_at.unwrap();
        assert_eq!((at.min, at.max), (3, 5));
    }
    let rows = out.properties[0].cases.as_ref().unwrap();
    for row in rows {
        assert_eq!(range(&row.probability), s("0", "1"));
        assert_eq!(row.verdict, relca_core::Verdict::Violated);
    }
    let v = serde_json::to_value(report::check(src)).unwrap();
    let fifo = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["field"] == "policy.queue")
        .unwrap();
    assert!(
        fifo["affects"]
            .as_str()
            .unwrap()
            .contains("tasks entering the queue later in the same instant are not compared")
    );
}

// minor-1: an admitted case never becomes rejected, so a later clock cut
// leaves the rejection mass exact (case 3 is rejected only when D1 = 3).
#[test]
fn review_rejection_mass_is_exact_under_a_later_cut() {
    let src = r#"model RejectCut version "0.2" {
  type T = enum { ok }
  actor A { capacity 1; capability run() -> T { duration {1min:0.5,3min:0.5}; result nondet { ok } } }
  process Flow { r = A.run(); end done }
  arrivals fixed [0min, 1min, 2min]
  policy fifo tie_by case_id
  queue capacity 1 on_overflow reject
  bounds { cases 3; live_tasks 1; clock 3min }
  verify safety no_resource_overcommit
}"#;
    let out = both(src);
    assert!(out.truncated);
    let case3 = &out.case_outcomes.as_ref().unwrap()[2];
    let rejected = case3
        .terminals
        .iter()
        .find(|t| t.kind == CaseTerminalKind::Rejected)
        .unwrap();
    assert_eq!(range(&rejected.probability), s("1/2", "1/2"));
    assert!(rejected.completed_at.is_none() && rejected.work.is_none());
}

// minor-2: a case that ended without starting a task no longer waits, so a
// later arrival is admitted.
#[test]
fn review_ended_case_without_tasks_is_not_waiting() {
    let src = r#"model EndThenArrive version "0.2" {
  type T = enum { ok }
  process Flow { end done }
  arrivals fixed [0min, 1min]
  queue capacity 1 on_overflow reject
  bounds { cases 2; live_tasks 1; clock 5min }
  verify reachable end.done
}"#;
    let out = both(src);
    let a = out.admission.unwrap().rejected_count;
    assert_eq!((a.min, a.max), (0, 0));
    for c in out.case_outcomes.as_ref().unwrap() {
        assert_eq!(c.terminals.len(), 1);
        assert!(matches!(c.terminals[0].kind, CaseTerminalKind::End(_)));
    }
    assert_eq!(out.properties[0].verdict, relca_core::Verdict::Satisfied);
}

// minor-3: the reference is compared beyond the fixture population, on
// cross-case join any with cancellation, a shared person, and FIFO forks.
#[test]
fn review_reference_matches_engine_on_any_cancel_people_and_forks() {
    let any_cancel = r#"model AnyCancel version "0.2" {
  type R = enum { pass }
  actor F { capacity 1; capability run() -> R { duration fixed 1min; result nondet { pass }; cancel cooperative ack_within 2min } }
  actor S { capacity 1; capability run() -> R { duration fixed 3min; result nondet { pass }; cancel cooperative ack_within 2min } }
  process Flow {
    fork f { a = F.run(); b = S.run() }
    join any f where result == pass tie by [a, b] remaining cancel_then_drain if none -> done
    end done
  }
  arrivals fixed [0min, 1min]
  bounds { cases 2; live_tasks 2; clock 20min }
  verify reachable end.done
  verify safety no_resource_overcommit
}"#;
    let people = r#"model SharedPerson version "0.2" {
  type T = enum { ok }
  people [p]
  actor A { capacity 1; executors [p]; capability run() -> T { duration fixed 1min; result nondet { ok } } }
  actor B { capacity 1; executors [p]; capability run() -> T { duration fixed 1min; result nondet { ok } } }
  process Flow { fork f { a = A.run(); b = B.run() } join all f; end done }
  arrivals fixed [0min, 0min]
  bounds { cases 2; live_tasks 2; clock 20min }
  verify reachable end.done
  verify safety no_resource_overcommit
}"#;
    let forks = r#"model FifoForks version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false }
  actor A { capacity 1; capability short() -> T { duration fixed 1min; result nondet { ok }; writes hit; finish { hit = true } }
            capability long() -> T { duration {1min:0.5,3min:0.5}; result nondet { ok } } }
  process Flow { fork f { x = A.short(); y = A.long() } join all f; end done }
  arrivals fixed [0min, 1min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 2; clock 20min }
  verify probability reached hit == true within 2min >= 0.5
}"#;
    let out = both(any_cancel);
    assert_eq!(out.case_outcomes.as_ref().unwrap().len(), 2);
    let out = both(people);
    // One person serves A and B of both cases: four one-minute tasks in series.
    for c in out.case_outcomes.as_ref().unwrap() {
        let at = c.terminals[0].completed_at.unwrap();
        assert_eq!((at.min, at.max), (2, 4));
    }
    both(forks);
}
