//! Review findings on ADR 0013 (2026-10-21), each pinned to the reviewer's
//! reproduction input. Oracles come from the ADR, not from the implementation.
use relca_core::report;
use serde_json::Value;

fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}

fn codes(v: &Value) -> Vec<String> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap().to_string())
        .collect()
}

fn message(v: &Value, code: &str) -> String {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == code)
        .unwrap_or_else(|| panic!("missing {code}: {v}"))["message"]
        .as_str()
        .unwrap()
        .to_string()
}

const OPTIONAL_TIGHT: &str = r#"model L1 version "0.2" {
  type V = enum { pass }
  observed case { e: optional<V>= none }
  actor A { capacity 1; capability c() -> V { duration fixed 1min; result nondet { pass }; writes e } }
  process P { x = A.c(); end done }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify reachable end.done
}"#;
const ALL_VERIFY_FAIL: &str = r#"model L1 version "0.2" {
  type V = enum { pass }
  observed case { e: optional<V> = none }
  actor A { capacity 1; capability c() -> V { duration fixed 1min; result nondet { pass }; writes e } }
  process P { x = A.c(); end done }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify reachable end.nope
}"#;
const QUALIFIED_BRACKET_WRITES: &str = r#"model D version "0.2" {
  type Difficulty = enum { routine, difficult }
  type T = enum { ok }
  observed case { hit: bool = false }
  actor Invoice { latent difficulty: Difficulty ~ { routine: 0.5, difficult: 0.5 }; latent wrong: bool = false }
  kernel K given [Invoice.difficulty] {
    routine -> { 0.2: ok with hit = true; 0.8: ok with hit = false }
    difficult -> { 0.6: ok with hit = true; 0.4: ok with hit = false }
  }
  actor A { capacity 1; capability sample() -> T { duration fixed 1min; outcomes kernel K; writes [hit, Invoice.wrong]; kernel_reads latent [Invoice.difficulty] } }
  process Flow { s = A.sample(); if hit == true { end bad } else { end good } }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify probability reached hit == true <= 0.4
}"#;
const TWO_GIVEN: &str = r#"model S1 version "0.2" {
  type D = enum { a, b }
  type T = enum { ok }
  observed case { hit: bool = false }
  latent case { d: D ~ { a: 0.5, b: 0.5 }; n: bool }
  kernel K given [d, n] {
    a, true -> { 0.5: ok with hit = true; 0.5: ok with hit = false }
    a, false -> { 1: ok with hit = false }
    b, true -> { 1: ok with hit = false }
    b, false -> { 1: ok with hit = true }
  }
  actor A { capacity 1; capability sample() -> T { duration fixed 1min; outcomes kernel K; writes hit; kernel_reads latent [d, n] } }
  process Flow { s = A.sample(); end done }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify probability reached hit == true <= 0.75
}"#;
const CUT_QUERY: &str = r#"model D version "0.2" {
  type Difficulty = enum { routine, difficult }
  type T = enum { ok }
  observed case { hit: bool = false }
  latent case { wrong: bool = false }
  actor Invoice { latent difficulty: Difficulty ~ { routine: 0.5, difficult: 0.5 }; }
  kernel K {
    * -> { 0.2: ok with hit = true; 0.8: ok with hit = false }
  }
  actor A { capacity 1; capability sample() -> T { duration fixed 1min; outcomes kernel K; writes hit } }
  process Flow { s = A.sample(); if hit == true { end bad } else { end good } }
  cut C { question "q"
    include [A]
    exclude Invoice reason "r" loses "l"
    reentry [review cut quarterly by cfo] }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify probability reached Invoice.difficulty == routine <= 0.4
}"#;
const OPTIONAL_LATENT_DIST: &str = r#"model T1 version "0.2" {
  type Difficulty = enum { routine, difficult }
  type T = enum { ok }
  observed case { hit: bool = false }
  latent case { d: optional<Difficulty> ~ { routine: 0.5, difficult: 0.25, none: 0.25 } }
  kernel K given [d] {
    routine -> { 0.2: ok with hit = true; 0.8: ok with hit = false }
    difficult -> { 0.6: ok with hit = true; 0.4: ok with hit = false }
  }
  actor A { capacity 1; capability sample() -> T { duration fixed 1min; outcomes kernel K; writes hit; kernel_reads latent [d] } }
  process Flow { s = A.sample(); if hit == true { end bad } else { end good } }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify probability reached hit == true <= 0.4
}"#;

// F1: <= and >= are read only at the probability comparison; a model without
// probability keeps optional<V>= none (ADR 0013 "保つもの").
#[test]
fn review_optional_close_then_default_without_space_is_unchanged() {
    let v = check(OPTIONAL_TIGHT);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    let spaced = check(&OPTIONAL_TIGHT.replace("<V>= none", "<V> = none"));
    for key in [
        "verdict",
        "properties",
        "terminals",
        "assumptions",
        "agency",
    ] {
        assert_eq!(v[key], spaced[key], "{key}");
    }
    let le = check(&TWO_GIVEN.replace("<= 0.75", "< = 0.75"));
    assert_eq!(le["verdict"], "unsupported", "{le}");
}

// F3: a model without probability queries keeps the ADR 0001 U-SEM-0 when
// every verify fails to lower.
#[test]
fn review_all_failed_verify_keeps_required_request_diagnostic() {
    let v = check(ALL_VERIFY_FAIL);
    assert_eq!(codes(&v), ["E-NAME-0", "U-SEM-0", "W-CUT-0"], "{v}");
}

// F4: ADR 0013 addendum, Actor.variable in writes is U-SYN-0 inside brackets too.
#[test]
fn review_qualified_writes_in_brackets_is_unsupported_syntax() {
    let v = check(QUALIFIED_BRACKET_WRITES);
    assert_eq!(v["verdict"], "unsupported", "{v}");
    assert!(codes(&v).contains(&"U-SYN-0".to_string()), "{v}");
    assert!(!codes(&v).contains(&"E-SYN-0".to_string()), "{v}");
}

// F5: the numeric limit names its own cause.
#[test]
fn review_numeric_limit_message_names_the_denominator() {
    let v = check(include_str!("review_inputs/chain20.relca"));
    assert_eq!(v["verdict"], "inconclusive", "{v}");
    assert_eq!(
        message(&v, "W-BUDGET-0"),
        "probability denominator exceeded the exact arithmetic limit"
    );
}

// F7: kernel assumption separates references with a comma and one space.
#[test]
fn review_kernel_assumption_lists_two_references_with_comma_space() {
    let v = check(TWO_GIVEN);
    let row = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["field"] == "kernel.K")
        .unwrap();
    assert_eq!(row["value"], "given[d, n]");
}

// F9: E-CUT-1 from a probability query does not call it an invariant only.
#[test]
fn review_cut_query_message_covers_probability_queries() {
    let v = check(CUT_QUERY);
    assert!(message(&v, "E-CUT-1").contains("probability query"), "{v}");
}

// F10: an optional latent stays unsupported; listing none is not a coverage fault.
#[test]
fn review_optional_latent_distribution_with_none_is_only_unsupported() {
    let v = check(OPTIONAL_LATENT_DIST);
    assert_eq!(v["verdict"], "unsupported", "{v}");
    assert!(!codes(&v).contains(&"E-PROB-0".to_string()), "{v}");
}
