//! ADR 0042/0043 marker order and horizon semantics on current raw Reports.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(s: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = root()
        .join("target")
        .join(format!("scenario-integration-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{}.relca", N.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&p, s).unwrap();
    p
}

fn run(args: &[&str], src: &str) -> (i32, Vec<u8>, Vec<u8>) {
    let path = scratch(src);
    let mut all = vec![args[0], path.to_str().unwrap()];
    all.extend(&args[1..]);
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(all)
        .output()
        .unwrap();
    (o.status.code().unwrap(), o.stdout, o.stderr)
}

const COMMON_HEAD: &str = r#"model M version "0.2" {
type T = enum { ok }
actor A {
  capacity 1
  capability run() -> T {
    duration fixed 5min
    result nondet { ok }
  }
}
process Flow { r = A.run(); end done }
cut C { include [A] }
policy fifo tie_by case_id
"#;
const RESPONSE: &str = "contract K { response delivery: case.received -> one_of(end.done) by 9min after case.received otherwise unresolved }\n}\n";

fn plain() -> String {
    format!(
        "{COMMON_HEAD}arrivals fixed [0min, 2min]\nbounds {{ cases 2; live_tasks 1; clock 30min }}\n{RESPONSE}"
    )
}

fn shell() -> String {
    format!(
        "{COMMON_HEAD}scenario Day {{\n  arrivals fixed [0min, 2min]\n  bounds {{ cases 2; live_tasks 1; clock 30min }}\n}}\n{RESPONSE}"
    )
}

fn markers(v: &Value) -> Vec<String> {
    v["agency"]["unchecked"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn a_multi_case_named_response_keeps_declared_marker_order_with_and_without_scenario() {
    // Two cases, a named response: two ledger rows, obligation_debtor last.
    let (code, out, err) = run(&["check"], &plain());
    assert_eq!(code, 0);
    assert!(err.is_empty());
    let old: Value = serde_json::from_slice(&out).unwrap();
    let tail = [
        "multi_case_interference",
        "responsibility_people_undeclared",
        "obligation_debtor",
    ];
    let u = markers(&old);
    assert_eq!(u[u.len() - 3..], tail);
    assert_eq!(old["obligations"].as_array().unwrap().len(), 2);

    let (code, out, err) = run(&["check", "--scenario", "Day"], &shell());
    assert_eq!(code, 0);
    assert!(err.is_empty());
    let new: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(markers(&new), u, "the same marker list and order");
    // Only the scenario rows are added, after the existing rows.
    let fields = |v: &Value| -> Vec<String> {
        v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["field"].as_str().unwrap().to_owned())
            .collect()
    };
    let mut expected = fields(&old);
    expected.extend(["scenario", "scenario.drain", "scenario.aggregation"].map(String::from));
    assert_eq!(fields(&new), expected);

    for v in [&old, &new] {
        fixture_match::compare_report(&json!({}), v)
            .unwrap_or_else(|e| panic!("the wire matcher must accept the old order: {e}"));
    }
}

fn assert_marker_refused(v: &Value) {
    let e = fixture_match::compare_report(&json!({}), v).unwrap_err();
    assert!(e.contains("Agency unchecked marker"), "{e}");
}

#[test]
fn a_matcher_accepts_obligation_debtor_after_the_interference_marker_only_with_ledger_rows() {
    let (_, out, _) = run(&["check"], &plain());
    let v: Value = serde_json::from_slice(&out).unwrap();
    // Swapped order: never accepted.
    let mut swapped = v.clone();
    let u = swapped["agency"]["unchecked"].as_array_mut().unwrap();
    let n = u.len();
    u.swap(n - 2, n - 1);
    assert_marker_refused(&swapped);
    // A trailing obligation_debtor without ledger rows: never accepted.
    let mut rowless = v.clone();
    rowless["obligations"] = json!([]);
    assert_marker_refused(&rowless);
    // A duplicated interference marker: never accepted.
    let mut twice = v.clone();
    let u = twice["agency"]["unchecked"].as_array_mut().unwrap();
    let n = u.len();
    u.insert(n - 1, json!("multi_case_interference"));
    assert_marker_refused(&twice);
}

/// Post-integration boundary review m1: obligation_debtor appears at most
/// once, only last, and only when the report carries obligation rows. Before
/// the fix the matcher only closed the tail, so both wires below were
/// accepted.
#[test]
fn a_matcher_refuses_obligation_debtor_before_the_tail_or_twice() {
    let (_, out, _) = run(&["check"], &plain());
    let v: Value = serde_json::from_slice(&out).unwrap();
    // No ledger rows, obligation_debtor moved before the interference marker.
    // The current people marker remains between those two positions.
    let mut rowless_before = v.clone();
    rowless_before["obligations"] = json!([]);
    let u = rowless_before["agency"]["unchecked"]
        .as_array_mut()
        .unwrap();
    let n = u.len();
    let interference = u
        .iter()
        .position(|w| w == "multi_case_interference")
        .unwrap();
    u.swap(interference, n - 1);
    assert_eq!(u[interference], json!("obligation_debtor"));
    assert_eq!(u[n - 1], json!("multi_case_interference"));
    assert_marker_refused(&rowless_before);
    // Ledger rows, obligation_debtor added at the head as well:
    // [obligation_debtor, .., multi_case_interference, obligation_debtor].
    let mut head_too = v.clone();
    assert!(!head_too["obligations"].as_array().unwrap().is_empty());
    let u = head_too["agency"]["unchecked"].as_array_mut().unwrap();
    u.insert(0, json!("obligation_debtor"));
    assert_marker_refused(&head_too);
}

/// (c) G11b, undecided at b5 and not equivalent on the integrated binary: the
/// trace's global horizon leaf must expire debts at D, not at the clock.
/// Two cases, arrivals 0 and 1; Judge (one person j) 0..1 and 1..2, End
/// returned at 1 and 2; filing at the End; route (person p) 2 minutes,
/// respond_by 1. Case 1's deadline 2 <= D2 is unmet (route ends at 3): the
/// violation. Case 2's deadline 3 > D2 is undecided at the horizon and must
/// not be chosen as the counterexample (its replay would not verify).
const G11B: &str = r#"model ContestRuntime version "0.2" {
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
  scenario Day {
    arrivals fixed [0min, 1min]
    bounds { cases 2; live_tasks 2; clock 10min }
    horizon 2min
  }
  verify safety no_resource_overcommit
  contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 1min }
  contract R { response case.received -> one_of(end.returned, end.posted) by 4min after case.received otherwise unresolved }
}
"#;

#[test]
fn c_g11b_horizon_counterexample_is_the_case_violated_by_d_and_replays() {
    let (code, out, _) = run(&["explain", "--scenario", "Day"], G11B);
    assert_eq!(code, 1);
    let v: Value = serde_json::from_slice(&out).unwrap();
    let e = &v["explanation"];
    assert_eq!(e["status"], "complete");
    let cs = e["counterexamples"].as_array().unwrap();
    assert_eq!(cs.len(), 1);
    let c = &cs[0];
    assert_eq!(c["id"], "contract Controls #1 contest_response");
    assert_eq!(c["case_id"], 1);
    assert_eq!(c["status"], "verified");
    assert_eq!(c["replay"]["status"], "verified");
    let last = c["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(last["kind"], "horizon");
    assert_eq!(last["time"], 2);
    assert_eq!(last["violation"]["deadline"], 2);
}
