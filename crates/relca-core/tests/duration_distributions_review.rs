//! Review findings of ADR 0014 (2026-10-22): cuts after the deadline,
//! static certainty with a distributed rival, and reference-only checks.
//! Expected values are hand calculations, not copied from either runtime.

use relca_core::{engine, lower, reference, report, syntax};
use serde_json::Value;

fn check(src: &str) -> Value {
    serde_json::to_value(report::check(src)).unwrap()
}

fn model(src: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(src).expect("parse")).expect("lower")
}

fn range(p: &Option<relca_core::probability::ProbabilityRange>) -> (String, String) {
    let p = p.as_ref().expect("probability");
    (p.min.to_string(), p.max.to_string())
}

const LIVE: &str = r#"model M version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false }
  actor A { capacity 1; capability sample() -> T { duration {1min:0.5,3min:0.5}; result nondet { ok }; writes hit; finish { hit = true } } }
  actor B { capacity 3; capability c() -> T { duration fixed 1min; result nondet { ok } } }
  process Flow { s = A.sample(); fork f { x = B.c(); y = B.c(); z = B.c() }; join all f; end done }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify probability reached hit == true within 2min >= 0.5
}"#;

// The fork exceeds live_tasks right after A finishes. If A takes 1min the
// hit is already observed (1); if A takes 3min the cut is at time 3 > 2, so
// the deadline has passed without a hit (0, decided). Both runtimes: [1/2,1/2].
#[test]
fn review_live_tasks_cut_after_deadline_is_decided_in_both_runtimes() {
    let m = model(LIVE);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert!(e.truncated && r.outcome.truncated);
    let half = ("1/2".to_string(), "1/2".to_string());
    assert_eq!(range(&e.properties[0].probability), half);
    assert_eq!(range(&r.outcome.properties[0].probability), half);
    let v = check(LIVE);
    assert_eq!(v["properties"][0]["verdict"], "satisfied", "{v}");
}

// Fixed 3min only: the cut is at time 3 > 2 with no hit, so [0,0] and
// within 2min <= 0 is satisfied in both runtimes even though truncated.
#[test]
fn review_fixed_only_live_cut_after_deadline() {
    let src = LIVE
        .replace("duration {1min:0.5,3min:0.5}", "duration fixed 3min")
        .replace(">= 0.5", "<= 0");
    let m = model(&src);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    let zero = ("0".to_string(), "0".to_string());
    assert_eq!(range(&e.properties[0].probability), zero);
    assert_eq!(range(&r.outcome.properties[0].probability), zero);
    let v = check(&src);
    assert_eq!(v["properties"][0]["verdict"], "satisfied", "{v}");
    assert_eq!(v["bounds"]["truncated"], true);
}

// The queried flag is never written, so only the cut decides. A cut at time
// 2 (= D) is not decided: later bundles at time 2 could still hit, so [0,1].
// A cut at time 3 (> D) is decided: [0,0].
#[test]
fn review_live_cut_at_the_deadline_stays_uncertain() {
    let base = LIVE
        .replace(
            "hit: bool = false",
            "hit: bool = false; never: bool = false",
        )
        .replace(
            "hit == true within 2min >= 0.5",
            "never == true within 2min <= 0",
        );
    for (a, max, verdict) in [("2", "1", "inconclusive"), ("3", "0", "satisfied")] {
        let src = base.replace(
            "duration {1min:0.5,3min:0.5}",
            &format!("duration fixed {a}min"),
        );
        let m = model(&src);
        let expected = ("0".to_string(), max.to_string());
        assert_eq!(
            range(&engine::explore(&m).properties[0].probability),
            expected
        );
        assert_eq!(
            range(&reference::explore(&m).outcome.properties[0].probability),
            expected
        );
        assert_eq!(check(&src)["properties"][0]["verdict"], verdict);
    }
}

// State budget: hit is written at time 1, the query asks within 0min.
// With budget 0 nothing is expanded: [0,1]. With budget 5 every state at
// time 0 is expanded (seed, receipt, start of A) and later cuts are after
// the deadline: [0,0].
#[test]
fn review_state_budget_cut_after_deadline() {
    let src = r#"model M version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false }
  actor A { capacity 1; capability sample() -> T { duration fixed 1min; result nondet { ok }; writes hit; finish { hit = true } } }
  actor B { capacity 1; capability c() -> T { duration {3min:0.5,5min:0.5}; result nondet { ok } } }
  process Flow { s = A.sample(); x = B.c(); y = B.c(); z = B.c(); end done }
  bounds { cases 1; live_tasks 2; clock 40min }
  verify probability reached hit == true within 0min <= 0
}"#;
    let m = model(src);
    let e = engine::explore_with_budget(&m, 0);
    assert_eq!(
        range(&e.properties[0].probability),
        ("0".to_string(), "1".to_string())
    );
    let e = engine::explore_with_budget(&m, 5);
    assert!(e.truncated);
    assert_eq!(
        range(&e.properties[0].probability),
        ("0".to_string(), "0".to_string())
    );
}

// m1: the rival (Fast) has a distribution and Slow is fixed 3min with a
// cooperative cancel. Fast may finish at 1, Slow is then acknowledged
// without a result, and reading slow is uncertain: lower rejects it with a
// span (not only the runtime defense).
#[test]
fn review_static_certainty_with_distributed_rival() {
    let s = include_str!("../../../fixtures/duration-cancel/model.relca")
        .replace("duration fixed 1min", "duration {1min:0.5,5min:0.5}")
        .replace("duration {2min:0.5,4min:0.5}", "duration fixed 3min")
        .replace(
            "end done\n",
            "if slow == ok { end done } else { end other }\n",
        );
    let ast = syntax::parse(&s).expect("parse");
    let diags = lower::lower(&ast).expect_err("lower must reject");
    assert!(
        diags
            .iter()
            .any(|d| d.code == "U-SEM-0" && d.span.is_some()),
        "{diags:?}"
    );
}

// m2: the reference interpreter checks the absolute finish time of every
// positive candidate at start, independently of the engine.
#[test]
fn review_reference_overflow_aborts() {
    let s = r#"model M version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false }
  actor First { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }
  actor A { capacity 1; capability sample() -> T { duration {1min:0.5,18446744073709551615min:0.5}; result nondet { ok }; writes hit; finish { hit = true } } }
  process Flow { f = First.run(); s = A.sample(); end done }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify probability reached hit == true within 3min <= 0.5
}"#;
    let r = reference::explore(&model(s));
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
}
