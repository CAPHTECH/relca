//! Hand controls from the independent stage-observation review.
use relca_core::report;
use serde_json::{Value, json};

const FALLBACK: &str = r#"model F version "0.2" {
  type V = enum { pass, fail }
  people [c1]
  actor Doc { source document }
  actor Probe { executors [c1]; capability run() -> V { reads [Doc]; duration fixed 1min; result nondet { pass, fail } } }
  actor Worker { executors [c1]; capability confirm(e: V) -> V { duration fixed 1min; result nondet { pass } } capability read() -> V { reads [Doc]; duration fixed 1min; result nondet { pass } } }
  cut C { include [Doc,Probe,Worker] }
  process Main { p = Probe.run();
    degrade Manual when p == fail { f = Worker.confirm(p); end done }
    else { n = Worker.read(); end done }
  }
  bounds { cases 1; live_tasks 1; clock 10min }
  responsible c1 for Worker.confirm
  verify reachable end.done
}"#;

const MIXED: &str = r#"model M version "0.2" {
  type V = enum { pass, fail }
  people [c1, c2]
  actor Invoice { source document }
  actor Doc { source document }
  actor Clerk { executors [c2]; capability enter() -> V { reads [Invoice]; duration fixed 1min; result nondet { pass } } }
  actor Checker { executors [c1]
    capability look() -> V { reads [Doc]; duration fixed 1min; result nondet { pass } }
    capability confirm(e:V, d:V) -> V { duration fixed 1min; result nondet { pass, fail } } }
  cut C { include [Invoice,Doc,Clerk,Checker] }
  process Main { e = Clerk.enter(); d = Checker.look(); v = Checker.confirm(e, d); if v == pass { end posted } else { end returned } }
  bounds { cases 1; live_tasks 1; clock 10min }
  responsible c1 for Checker.confirm
  verify reachable end.posted
}"#;

const CONTEST: &str = r#"model AppealOnly version "0.2" {
  type V = enum { pass, fail }
  type Ack = enum { ok }
  people [c1,c2]
  actor Invoice { source document }
  actor Extractor { capacity 1; capability extract() -> V { reads [Invoice]; duration fixed 1min; result nondet { pass } } }
  actor Clerk { executors [c1]
    capability recheck() -> V { reads [Invoice]; duration fixed 1min; result nondet { pass } }
    capability confirm(e:V) -> V { duration fixed 1min; result nondet { pass, fail } }
  }
  actor Ledger { capacity 1; capability post(v:V) -> Ack { commits; duration fixed 1min; result nondet { ok } } }
  actor Vendor { control party; standing over [end.returned] basis "claim" }
  cut C { include [Invoice,Extractor,Clerk,Ledger,Vendor] }
  process Main { e=Extractor.extract(); v=Clerk.confirm(e); if v==pass {} else {end returned}; x=Ledger.post(v); end posted }
  responsible c1 for Clerk.recheck
  authority ReviewGrant { basis "test delegation"; grants c1 to Clerk.confirm; issued_by c2 }
  contract Appeal { contest Vendor on end.returned route Clerk.recheck reversible_by [c1] respond_by 2min }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.posted
}"#;

fn check(source: &str) -> Value {
    let value = serde_json::to_value(report::check(source)).unwrap();
    assert_eq!(value["verdict"], "satisfied", "{value}");
    value
}
fn stage_unexamined(value: &Value, reason: &str) {
    let observation = &value["responsibilities"][0]["observation"];
    assert_eq!(observation["stages_status"], "not_derived");
    assert_eq!(observation["stages_reason"], reason);
    assert_eq!(observation["stages"], Value::Null);
}

#[test]
fn fallback_only_capability_is_unexamined_not_a_nonexecutor() {
    let value = check(FALLBACK);
    assert_eq!(
        value["responsibilities"][0]["observation"]["status"],
        "present"
    );
    stage_unexamined(&value, "fallback_path");
    assert!(
        value["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("stage_observation(fallback_path)"))
    );
    assert!(
        !value["responsibilities"][0]["warnings"]
            .as_array()
            .unwrap()
            .contains(&json!("W-RESP-3"))
    );
}

#[test]
fn fallback_only_state_writer_retains_the_path_reason() {
    let source = FALLBACK
        .replace(
            "people [c1]",
            "people [c1]\n observed case { confirmed: optional<V> }",
        )
        .replace(
            "capability confirm(e: V) -> V {",
            "capability confirm(e: V) -> V { writes confirmed;",
        )
        .replace(
            "responsible c1 for Worker.confirm",
            "responsible c1 for (confirmed == pass)",
        );
    assert_ne!(source, FALLBACK);
    stage_unexamined(&check(&source), "fallback_path");
}

#[test]
fn a_real_nonexecutor_on_fallback_remains_not_applicable() {
    let source = FALLBACK.replace("people [c1]", "people [c1,c2]").replace(
        "responsible c1 for Worker.confirm",
        "responsible c2 for Worker.confirm",
    );
    let value = check(&source);
    let observation = &value["responsibilities"][0]["observation"];
    assert_eq!(observation["stages_status"], "not_applicable");
    assert_eq!(observation["stages_reason"], "person_not_executor");
    assert_eq!(observation["stages"], Value::Null);
}

#[test]
fn uncalled_capability_with_the_person_as_executor_is_not_derived() {
    let source = FALLBACK.replace("capability read()", "capability unused() -> V { duration fixed 1min; result nondet { pass } } capability read()")
        .replace("responsible c1 for Worker.confirm", "responsible c1 for Worker.unused");
    stage_unexamined(&check(&source), "target_not_called");
}

#[test]
fn contest_only_capability_is_unexamined_not_a_nonexecutor() {
    let value = check(CONTEST);
    stage_unexamined(&value, "contest_path");
    assert!(
        value["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("responsibility_stage(contest_path)"))
    );
}

#[test]
fn contest_only_state_writer_is_unexamined() {
    let source = CONTEST
        .replace(
            "people [c1,c2]",
            "people [c1,c2]\n observed case { verified: optional<V> }",
        )
        .replace(
            "capability recheck() -> V {",
            "capability recheck() -> V { writes verified;",
        )
        .replace(
            "responsible c1 for Clerk.recheck",
            "responsible c1 for (verified == pass)",
        );
    stage_unexamined(&check(&source), "contest_path");
}

#[test]
fn person_missing_and_stage_only_missing_coexist_without_double_warning_for_the_same_source() {
    let value = check(MIXED);
    let row = &value["responsibilities"][0];
    assert_eq!(row["observation"]["missing_sources"], json!(["Invoice"]));
    assert_eq!(row["observation"]["direct"], json!(["Doc"]));
    assert_eq!(
        row["observation"]["stages"],
        json!([{"call":"v","required":["Doc","Invoice"],"direct":[],"missing":["Doc","Invoice"]}])
    );
    assert_eq!(row["warnings"], json!(["W-RESP-1", "W-RESP-3"]));
    let warnings: Vec<_> = value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-RESP-3")
        .collect();
    assert_eq!(warnings.len(), 1);
    let message = warnings[0]["message"].as_str().unwrap();
    assert!(message.contains("v: Doc"));
    assert!(!message.contains("Invoice"));
}

#[test]
fn rule_capability_is_an_ordinary_call_but_decides_evaluation_is_not_a_call() {
    let source = r#"model RR version "0.2" {
      type V = enum { pass, fail }
      people [c1]
      actor Invoice { source document }
      actor Clerk { executors [c1]; capability enter() -> V { reads [Invoice]; duration fixed 1min; result nondet { pass } } }
      actor ApprovalRule { control policy; capacity 1; version "r1"; decides needed: bool = true
        capability judge(e:V) -> V { duration fixed 1min; result nondet { pass, fail } } }
      cut C { include [Invoice,Clerk,ApprovalRule] }
      process Main { e = Clerk.enter(); j = ApprovalRule.judge(e); if j == pass { end posted } else { end returned } }
      bounds { cases 1; live_tasks 1; clock 10min }
      verify reachable end.posted
    }"#;
    let value = check(source);
    let stages = value["agency"]["viewpoints"]["observation"]["stages"]
        .as_array()
        .unwrap();
    assert_eq!(
        stages
            .iter()
            .map(|r| r["call"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["e", "j"]
    );
    assert_eq!(stages[1]["capability"], "ApprovalRule.judge");
    assert_eq!(stages[1]["direct"], json!([]));
    assert_eq!(stages[1]["via_outputs"], json!(["Invoice"]));
    assert!(
        value["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("stage_observation(rule)"))
    );
}
