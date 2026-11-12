//! Connect the newly registered raw inputs to their declared hand derivation.
#[path = "support/r2_snapshot_oracle.rs"]
mod oracle;
use serde_json::{Value, json};
use std::{path::Path, process::Command};

fn run(id: &str) -> Value {
    let jobs: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/snapshot-extra-jobs.json"
    )))
    .unwrap();
    let job = jobs
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["id"] == id)
        .unwrap();
    let args: Vec<_> = job["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    assert_eq!(
        out.status.code().map(i64::from),
        job["expected_exit"].as_i64(),
        "{id}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["verdict"], job["expected_verdict"]);
    oracle::check(id, &oracle::args(id), &value, out.status.code().unwrap()).unwrap();
    value
}
#[test]
fn raw_recovery_input_keeps_the_hand_timeline_and_stop_distinctions() {
    let v = run("r2-check-proceed");
    serde_json::from_value::<relca_core::report::Report>(v.clone()).unwrap();
    assert_eq!(v["terminals"][0]["elapsed"], json!({"min":7,"max":7}));
    assert_eq!(v["terminals"][0]["work"], json!({"min":7,"max":7}));
    let c = &v["response_recovery"]["paths"][0]["commands"][0];
    assert_eq!(c["queries"][0]["at"], 3);
    assert_eq!(c["resolutions"][0]["at"], 6);
    assert_eq!(c["unresolved"], false);
    let decision = c["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["binding"] == "decision")
        .unwrap();
    assert_eq!(decision["finished_at"], 6);
    assert_eq!(v["response_receipts"][0]["receipt"], "unknown");
    assert!(v["response_receipts"][0]["observed_result"].is_null());
    for (id, truncated, stop) in [
        ("r2-check-clock", true, "truncated"),
        ("r2-check-horizon", false, "cut_off"),
    ] {
        let v = run(id);
        serde_json::from_value::<relca_core::report::Report>(v.clone()).unwrap();
        assert_eq!(v["bounds"]["truncated"], truncated);
        let c = &v["response_recovery"]["paths"][0]["commands"][0];
        assert_eq!(c["stop_reason"], stop);
        assert!(c["queries"].as_array().unwrap().is_empty());
        assert!(c["resolutions"].as_array().unwrap().is_empty());
    }
    let v = run("r2-check-unresolved");
    assert_eq!(
        v["response_recovery"]["paths"][0]["commands"][0]["unresolved"],
        true
    );
    assert_eq!(
        v["case_outcomes"][0]["terminals"][0]["kind"],
        "receipt_unknown"
    );
}
#[test]
fn raw_recovery_sampling_replay_and_closed_mutation_match_their_scopes() {
    let sampled = run("r2-simulate");
    serde_json::from_value::<relca_core::report::SimulationReport>(sampled.clone()).unwrap();
    assert_eq!(sampled["response_analysis"]["coverage"], "sampled");
    assert!(
        sampled["response_analysis"]["checked"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s.as_str().unwrap().starts_with("sampled_"))
    );
    let explained = run("r2-explain");
    let c = explained["explanation"]["counterexamples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"].as_str().unwrap().contains("Deadline"))
        .unwrap();
    assert_eq!(c["replay"]["status"], "verified");
    for (kind, time) in [("receipt_query", 3), ("resolve_effect", 6)] {
        assert!(
            c["steps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["kind"] == kind && s["time"] == time)
        );
    }
    let mutation = run("r2-mutate-refusal");
    assert!(
        mutation["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"] == "recovery_feature_combination")
    );
    let conform = run("r2-conform-refusal");
    assert!(conform.to_string().contains("recovery_feature_combination"));
}
#[test]
fn raw_recovery_hand_oracle_rejects_single_financial_and_scope_faults() {
    let valid = run("r2-check-resolved");
    for (pointer, value) in [
        (
            "/response_recovery/paths/0/commands/0/queries/0/at",
            json!(4),
        ),
        (
            "/response_recovery/paths/0/commands/0/resolutions",
            json!([]),
        ),
        (
            "/response_recovery/paths/0/commands/0/unresolved",
            json!(true),
        ),
        ("/terminals/0/work/min", json!(6)),
    ] {
        let mut bad = valid.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            oracle::check(
                "r2-check-resolved",
                &oracle::args("r2-check-resolved"),
                &bad,
                0
            )
            .is_err(),
            "{pointer}"
        );
    }
    let mut sampled = run("r2-simulate");
    sampled["response_analysis"]["coverage"] = json!("complete");
    assert!(oracle::check("r2-simulate", &oracle::args("r2-simulate"), &sampled, 1).is_err());
}
#[test]
fn recovery_bundle_audit_has_independent_hand_fault_pairs() {
    let out = Command::new("python3")
        .arg("-B")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/ledger_delta.py"))
        .arg("--calibrate-recovery")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "recovery bundle calibration: positive=4 rejected_faults=4\n"
    );
}
#[test]
fn every_new_recovery_raw_job_runs_its_closed_hand_classifier() {
    for id in oracle::IDS {
        run(id);
    }
}
