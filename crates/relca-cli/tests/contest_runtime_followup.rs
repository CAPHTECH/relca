//! ADR 0022 追記 3 の再レビューの受入の穴: 同じ時刻に 2 案件が終端に達するときの
//! 起票の安定化と FIFO、切断の下で起票しなかった枝の費用。期待値は手計算。
use serde_json::Value;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use std::{fs, process::Command};

const SAME_END: &str = r#"model SameEnd version "0.2" {
  type T = enum { pass, fail }
  people [j,p]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 4min; result nondet { fail } } }
  actor Route { executors [p]; capability respond() -> T { duration fixed 2min; result nondet { pass } } }
  actor Party { control party; standing over [end.returned] basis "claim" }
  permit j to Judge.judge
  permit p to Route.respond
  authority G { basis "decision"; grants j to Judge.judge }
  process Flow { end returned }
  cut C { include [Judge,Route,Party] }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 4min }
}"#;

const CUT: &str = r#"model ContestRuntime version "0.2" {
  type T = enum { pass, fail }
  people [j,p]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 1min; result nondet { fail } } }
  actor Route { executors [p]; capability respond() -> T { duration fixed 2min; result nondet { pass } } }
  actor Party { control party; standing over [end.returned] basis "claim" }
  permit j to Judge.judge
  permit p to Route.respond
  authority G { basis "decision"; grants j to Judge.judge }
  process Flow { d = Judge.judge(); if d == fail { end returned } else { end posted } }
  cut C { include [Judge,Route,Party] }
  bounds { cases 1; live_tasks 2; clock 2min }
  metrics { window 10min; currency "JPY"; rate Judge per_minute 1; rate Route per_minute 1; rate Judge.judge per_start 0; rate Route.respond per_start 0 }
  verify safety no_resource_overcommit
  contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 2min }
}"#;

fn check(name: &str, src: &str) -> Value {
    let dir = test_tmp::root().join(format!(
        "relca-contest-followup-{}-{name}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["check", path.to_str().unwrap()])
        .output()
        .unwrap();
    fs::remove_file(&path).unwrap();
    serde_json::from_slice(&out.stdout).unwrap()
}

fn responses(row: &Value) -> Vec<(String, Value)> {
    row["alternatives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["status"].as_str().unwrap().to_owned(),
                a["response_at"].clone(),
            )
        })
        .collect()
}

#[test]
fn same_time_ends_stabilize_filing_before_dispatch_so_case1_is_first() {
    // Both cases arrive at 0 and end at 0. Each files or not; all filings and
    // queues happen before dispatch, so FIFO tie_by case_id starts case1's
    // route first: case1 responds at 2 only. Case2 responds at 2 (case1 did
    // not file) or 4 (case1 filed, 0..2 then 2..4). Deadline 0+4=4.
    let v = check("same-end", SAME_END);
    let rows = v["contest_runtime"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let n = |s: &str, t: Option<u64>| (s.to_owned(), t.map_or(Value::Null, Value::from));
    assert_eq!(
        responses(&rows[0]),
        [n("not_filed", None), n("responded", Some(2))]
    );
    assert_eq!(
        responses(&rows[1]),
        [
            n("not_filed", None),
            n("responded", Some(2)),
            n("responded", Some(4))
        ]
    );
    let response = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "contract Controls #1 contest_response")
        .unwrap();
    assert_eq!(response["verdict"], "satisfied");
}

#[test]
fn a_branch_that_did_not_file_keeps_never_created_under_a_cut() {
    // Judge 0..1, end returned at 1. Declined closes at 1; filed route runs
    // 1..3 but the clock is 2, so the filed branch is unresolved.
    let v = check("cut", CUT);
    assert_eq!(v["verdict"], "inconclusive");
    let alts = v["contest_runtime"][0]["alternatives"].as_array().unwrap();
    assert_eq!(alts[0]["status"], "not_filed");
    assert_eq!(alts[0]["route_cost"]["reason"], "never_created");
    assert_eq!(alts[1]["status"], "unresolved");
    assert_eq!(alts[1]["route_cost"]["reason"], "truncated");
    assert_eq!(alts[1]["route_work"], Value::Null);
}

#[path = "../src/fixture_match.rs"]
mod fixture_match;

#[test]
fn the_wire_ties_deadline_passed_unresolved_to_unresolved_branches() {
    let v = check("cut-wire", CUT);
    fixture_match::validate_contest_runtime_wire(&v).unwrap();
    let mut bad = v.clone();
    bad["contest_runtime"][0]["alternatives"][1]["violation_reason"] = "late_response".into();
    assert!(fixture_match::validate_contest_runtime_wire(&bad).is_err());
    let mut bad = v.clone();
    let alt = &mut bad["contest_runtime"][0]["alternatives"][1];
    alt["status"] = "violated".into();
    alt["violation_reason"] = "deadline_passed_unresolved".into();
    assert!(fixture_match::validate_contest_runtime_wire(&bad).is_err());
    for cost in [
        serde_json::json!({"min":"0","max":"0","reason":null}),
        serde_json::json!({"min":null,"max":null,"reason":"truncated"}),
    ] {
        let mut bad = v.clone();
        bad["contest_runtime"][0]["alternatives"][0]["route_cost"] = cost;
        assert!(fixture_match::validate_contest_runtime_wire(&bad).is_err());
    }
}
