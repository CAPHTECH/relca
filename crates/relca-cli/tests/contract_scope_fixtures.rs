#[path = "support/contract_scope_snapshot_oracle.rs"]
mod hand;
use serde_json::Value;
use std::{path::Path, process::Command};

#[test]
fn all_ten_scope_fixture_jobs_have_independent_time_and_classification_oracles() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut jobs = Vec::new();
    for name in hand::NAMES {
        let path = format!("fixtures/{name}/model.relca");
        let invocation: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{name}/invocation.json"))).unwrap(),
        )
        .unwrap();
        let parsed =
            relca_test_support::fixture_match::Invocation::parse(Some(&invocation)).unwrap();
        let args = parsed.args(Path::new(&path));
        let id = format!("fixture-{name}");
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .current_dir(&root)
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        hand::check(&id, &args, &value, out.status.code().unwrap())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut broken = value.clone();
        broken["verdict"] = serde_json::json!("unsupported");
        assert!(
            hand::check(&id, &args, &broken, out.status.code().unwrap()).is_err(),
            "{name}: classification fault"
        );
        jobs.push(serde_json::json!({"id":id,"args":args}));
    }
    hand::validate_set(&jobs).unwrap();
    let mut lost = jobs.clone();
    lost.pop();
    assert!(hand::validate_set(&lost).is_err());
    let mut duplicate = jobs.clone();
    duplicate.push(jobs[0].clone());
    assert!(hand::validate_set(&duplicate).is_err());
}

#[test]
fn mutation_scope_copy_has_closed_wire_and_rejects_transport_faults() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args([
            "mutate",
            "fixtures/scope-a1/model.relca",
            "--configuration",
            "Old",
            "--kinds",
            "authority_expired",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(5));
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    relca_test_support::fixture_match::validate_mutation_wire(&value).unwrap();
    assert_eq!(value["contract_scope"], value["original"]["contract_scope"]);
    assert_eq!(value["run"]["reason"], "no_selected_mutant");
    let mut bad = value.clone();
    bad.as_object_mut().unwrap().remove("contract_scope");
    assert!(relca_test_support::fixture_match::validate_mutation_wire(&bad).is_err());
    let mut bad = value;
    bad["unchecked"]
        .as_array_mut()
        .unwrap()
        .retain(|s| !s.as_str().unwrap().starts_with("contract.scope."));
    assert!(relca_test_support::fixture_match::validate_mutation_wire(&bad).is_err());
}
