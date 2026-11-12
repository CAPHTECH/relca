//! Actual CLI seams and independent wire-negative controls for ADR0041.
#[path = "../src/fixture_match.rs"]
mod wire;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};
fn run(name: &str, command: &str) -> (Value, i32) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
        .join("model.relca");
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(path)
        .output()
        .unwrap();
    (
        serde_json::from_slice(&out.stdout).unwrap(),
        out.status.code().unwrap(),
    )
}
#[test]
fn all_authored_expectations_use_the_closed_wire_and_explicit_invocation() {
    for name in wire::CONFLICT_FIXTURES {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name);
        let e: Value =
            serde_json::from_slice(&std::fs::read(path.join("expected.json")).unwrap()).unwrap();
        let ip = path.join("invocation.json");
        let inv: Option<Value> = ip
            .exists()
            .then(|| serde_json::from_slice(&std::fs::read(ip).unwrap()).unwrap());
        let invocation = wire::Invocation::parse(inv.as_ref()).unwrap();
        invocation.validate_expected(&e).unwrap();
    }
}
#[test]
fn global_loss_table_has_exact_identity_and_no_sample_proof_or_fake_case_rows() {
    let raw = json!({"mode":"exhaustive","properties":[{"id":"no_lost_update Ledger.holder","verdict":"violated"}]});
    assert!(wire::validate_conflict_properties(&raw).is_ok());
    for (key, value) in [
        ("cases", json!([])),
        ("probability", Value::Null),
        ("notes", json!([])),
    ] {
        let mut bad = raw.clone();
        bad["properties"][0][key] = value;
        assert_ne!(raw, bad);
        assert!(wire::validate_conflict_properties(&bad).is_err());
    }
    for id in [
        "no_lost_update ",
        "no_lost_update Ledger.holder.extra",
        "no_lost_update Ledger.1holder",
    ] {
        let mut bad = raw.clone();
        bad["properties"][0]["id"] = json!(id);
        assert_ne!(raw, bad);
        assert!(wire::validate_conflict_properties(&bad).is_err());
    }
    let mut duplicate = raw.clone();
    duplicate["properties"]
        .as_array_mut()
        .unwrap()
        .push(raw["properties"][0].clone());
    assert!(wire::validate_conflict_properties(&duplicate).is_err());
    let sample = json!({"mode":"simulation","simulation":{"completed_replications":10},"properties":[{"id":"no_lost_update Ledger.holder","verdict":"inconclusive","basis":"sample","witness":null}]});
    assert!(wire::validate_conflict_properties(&sample).is_ok());
    for (key, value) in [
        ("verdict", json!("satisfied")),
        ("basis", json!("witness")),
        ("witness", json!({"replication":1})),
    ] {
        let mut bad = sample.clone();
        bad["properties"][0][key] = value;
        assert_ne!(sample, bad);
        assert!(wire::validate_conflict_properties(&bad).is_err());
    }
    let mut found = sample.clone();
    found["properties"][0]["verdict"] = json!("violated");
    found["properties"][0]["basis"] = json!("witness");
    found["properties"][0]["witness"] = json!({"replication":10});
    assert!(wire::validate_conflict_properties(&found).is_ok());
    found["properties"][0]["witness"]["replication"] = json!(11);
    assert!(wire::validate_conflict_properties(&found).is_err());
}
#[test]
fn cli_version_and_lww_exit_are_not_interchanged_and_sample_is_not_exhaustive() {
    for (name, command, code, verdict) in [
        ("cf-version", "check", 0, "satisfied"),
        ("cf-version.broken-no-check", "check", 4, "unsupported"),
        ("cf-lww.broken-overlap", "check", 1, "violated"),
        ("cf-lww-clock.broken-clock", "check", 2, "inconclusive"),
        ("cf-simulation", "simulate", 2, "inconclusive"),
        ("cf-same-tick.broken-simulate", "simulate", 4, "unsupported"),
        ("cf-latent.broken-latent", "check", 3, "invalid"),
    ] {
        let (v, exit) = run(name, command);
        assert_eq!(exit, code, "{name}");
        assert_eq!(v["verdict"], verdict, "{name}");
        assert!(v.get("versions").is_none() && v.get("lost_updates").is_none());
    }
}
#[test]
fn cli_explain_loss_uses_finish_and_verified_replay_not_a_new_stage_kind() {
    let (v, exit) = run("cf-lww.broken-overlap", "explain");
    assert_eq!(exit, 1);
    wire::validate_explain_wire(&v).unwrap();
    let c = v["explanation"]["counterexamples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "no_lost_update Ledger.holder")
        .unwrap();
    assert_eq!(c["status"], "verified");
    assert_eq!(c["replay"]["status"], "verified");
    let steps = c["steps"].as_array().unwrap();
    let last = steps.last().unwrap();
    assert_eq!(last["time"], 1);
    assert_eq!(last["kind"], "finish");
    assert_eq!(last["result"], "committed");
    assert!(c["property_location"].is_object());
    assert!(last["violation"]["related_steps"].as_array().unwrap().len() >= 2);
}
