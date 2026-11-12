//! Materializations and live CLI remain bound to separately authored hand inputs.
#[path = "support/p2c_snapshot_oracle.rs"]
mod hand;
#[path = "../../relca-core/tests/support/p2_cleanup_inputs.rs"]
mod input;
use serde_json::Value;
use std::path::Path;

#[test]
fn materialized_cleanup_models_are_exactly_the_fixed_hand_inputs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/inputs");
    for (id, source) in [
        (hand::IDS[0], input::KNOWN_FAILURE.to_owned()),
        (hand::IDS[1], input::prefix_diff()),
        (hand::IDS[2], input::withdrawn_diff()),
    ] {
        let actual = std::fs::read_to_string(root.join(format!("{id}.relca"))).unwrap();
        assert_eq!(
            actual.split_whitespace().collect::<Vec<_>>(),
            source.split_whitespace().collect::<Vec<_>>(),
            "{id}"
        );
    }
}
#[test]
fn cleanup_snapshot_jobs_connect_live_cli_to_the_fixed_hand_values() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let jobs: Vec<Value> =
        serde_json::from_str(include_str!("../../../scripts/snapshot-extra-jobs.json")).unwrap();
    hand::validate_set(&jobs);
    for id in hand::IDS {
        let job = jobs.iter().find(|j| j["id"] == id).unwrap();
        let args: Vec<String> = serde_json::from_value(job["args"].clone()).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .current_dir(&root)
            .output()
            .unwrap();
        hand::check(
            id,
            &args,
            &serde_json::from_slice(&output.stdout).unwrap(),
            output.status.code().unwrap(),
            &output.stderr,
        );
    }
}
