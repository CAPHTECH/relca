//! Snapshot materializations stay bound to the fixed, separately authored Q6c inputs.
#[path = "../../relca-core/tests/support/q6b_inputs.rs"]
mod b_input;
#[path = "../../relca-core/tests/support/q9_inputs.rs"]
mod contest_input;
#[path = "../../relca-core/tests/support/q6c_inputs.rs"]
mod input;
#[path = "support/q6c_snapshot_oracle.rs"]
mod snapshot;
use serde_json::{Value, json};
use std::path::Path;
fn tokens(source: &str) -> Vec<&str> {
    source.split_whitespace().collect()
}

#[test]
fn materialized_q6c_models_are_the_fixed_acceptance_inputs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/inputs");
    for (file, source) in [
        ("q6c-work.relca", input::work()),
        ("q6c-degrade.relca", input::degradation()),
        ("q6c-base.relca", input::point("base")),
        ("q6c-probability.relca", input::point("probability")),
    ] {
        assert_eq!(
            tokens(&std::fs::read_to_string(root.join(file)).unwrap()),
            tokens(&source),
            "{file}"
        );
    }
}
#[test]
fn materialized_suite_is_the_explicit_two_hand_points() {
    let v: Value =
        serde_json::from_str(include_str!("snapshots/inputs/q6c-sensitivity.json")).unwrap();
    assert_eq!(
        v,
        json!({"schema":"relca-sensitivity/v1",
        "scope":{"kind":"enumerated_models","rationale":"Explicit Q6c hand points, no implicit Cartesian product"},
        "points":[{"id":"base","model":"q6c-base.relca","configuration":null,"scenario":null},
                  {"id":"probability","model":"q6c-probability.relca","configuration":null,"scenario":null}],
        "targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":null})
    );
}

#[test]
fn q6c_snapshot_jobs_connect_live_cli_to_the_fixed_hand_oracle() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let jobs: Vec<Value> =
        serde_json::from_str(include_str!("../../../scripts/snapshot-extra-jobs.json")).unwrap();
    snapshot::validate_set(&jobs);
    for id in snapshot::IDS {
        let job = jobs.iter().find(|j| j["id"] == id).unwrap();
        let args: Vec<String> = serde_json::from_value(job["args"].clone()).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .current_dir(&root)
            .output()
            .unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        snapshot::check(
            id,
            &args,
            &doc,
            output.status.code().unwrap(),
            &output.stderr,
        );
    }
}
