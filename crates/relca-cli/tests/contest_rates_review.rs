//! Real CLI coverage for the Q9 review. No old snapshot or fixture is changed.
#[path = "../../relca-core/tests/support/q9_review_oracle.rs"]
mod hand;
#[path = "../../relca-core/tests/support/q9_review_inputs.rs"]
mod input;
use serde_json::Value;
use std::{path::PathBuf, process::Command};

fn invoke(name: &str, mode: &str) -> (Value, i32) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root
        .join(".tmp/q9-review-cli")
        .join(format!("{}-{name}-{mode}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, input::source(name)).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(mode)
        .arg(path)
        .current_dir(root)
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.stdout.iter().filter(|b| **b == b'\n').count(), 1);
    let stderr: String = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(String::from_utf8(out.stderr).unwrap(), stderr);
    (v, out.status.code().unwrap())
}
#[test]
fn mj1_before_target_unknown_not_absent() {
    let (v, exit) = invoke("range-before-target", "simulate");
    assert_eq!(exit, 1);
    hand::range_report(&v, false);
}
#[test]
fn mj1_in_route_unresolved_not_failed() {
    let (v, exit) = invoke("range-in-route", "simulate");
    assert_eq!(exit, 1);
    hand::range_report(&v, true);
}
#[test]
fn m1_running_permission_violation_is_unresolved() {
    let (v, exit) = invoke("permission-cut", "simulate");
    assert_ne!(exit, 0);
    let r = &v["metrics"]["contests"]["rows"][0];
    assert_eq!(r["unresolved"], 16);
    assert_eq!(r["route_failed"], 0);
}
#[test]
fn m3_check_discloses_no_sample_aggregation() {
    let (v, exit) = invoke("fixed", "check");
    assert_eq!(exit, 0);
    hand::check_assumption(&v, true);
    let (v, exit) = invoke("legacy", "check");
    assert_eq!(exit, 0);
    hand::check_assumption(&v, false);
}
#[test]
fn m4_legacy_encounter_wins_over_deadline_overflow() {
    let (v, exit) = invoke("legacy-overflow", "simulate");
    assert_eq!(exit, 4);
    hand::legacy_refusal(&v, 1);
}
#[test]
fn u2_legacy_encounter_wins_over_other_cases_live_cut() {
    for name in ["legacy-live-control", "legacy-live"] {
        let (v, exit) = invoke(name, "simulate");
        assert_eq!(exit, 4);
        hand::legacy_refusal(&v, 3);
    }
}
#[test]
fn u1_no_contest_end_with_unstarted_cancelled_task() {
    let (v, exit) = invoke("end-unstarted", "simulate");
    assert_eq!(exit, 0, "existential reachable End has a sampled witness");
    assert_eq!(v["simulation"]["completed_replications"], 16);
    assert_eq!(v["simulation"]["truncated_replications"], 0);
    assert!(
        v["simulation"]["deadlock_witnesses"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let tasks = v["metrics"]["cases"][0]["tasks"].as_array().unwrap();
    let b = tasks.iter().find(|t| t["binding"] == "b").unwrap();
    assert_eq!(b["starts"]["mean"], "0");
    assert_eq!(b["cancelled_before_start_count"]["stats"]["mean"], "1");
}
