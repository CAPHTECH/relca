//! Actual CLI boundary checks for the Q4 review rulings.
#[path = "../../relca-core/tests/support/q4_oracle.rs"]
mod hand;
#[path = "../../relca-core/tests/support/q4_inputs.rs"]
mod input;
#[path = "../../relca-core/tests/support/q4_review.rs"]
mod review;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

fn invoke(name: &str, source: &str, simulation: bool, scenario: Option<&str>, exit: i32) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root
        .join(".tmp/q4-review-cli")
        .join(format!("{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, source).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_relca"));
    command
        .arg(if simulation { "simulate" } else { "check" })
        .arg(path)
        .current_dir(root);
    if let Some(s) = scenario {
        command.args(["--scenario", s]);
    }
    let out = command.output().unwrap();
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        out.status.code(),
        Some(exit),
        "{name}: {}",
        value["diagnostics"]
    );
    assert_eq!(
        value["verdict"],
        if exit == 0 { "satisfied" } else { "violated" }
    );
    let stderr: String = value["diagnostics"]
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
    assert_eq!(out.stderr, stderr.as_bytes(), "{name}: diagnostic mirror");
    value
}

#[test]
fn m1_cli_property_arrivals_are_null_with_hand_samples() {
    let mut failures = Vec::new();
    for name in ["arrivals", "arrivals-burst", "arrivals-calendar"] {
        let value = invoke(name, &input::source(name), true, None, 0);
        if let Err(error) = hand::subset(
            &hand::expected(name)["properties"],
            &value["properties"],
            name,
        ) {
            failures.push(error);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn m2_cli_condition_wait_does_not_certify_decision_support() {
    let mut failures = Vec::new();
    for dataflow in [false, true] {
        let value = invoke(
            &format!("condition-duty-{dataflow}"),
            &review::condition_source(dataflow),
            false,
            None,
            1,
        );
        if let Err(error) = review::condition_withdrawal(&value) {
            failures.push(error);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn green_m2_cli_if_is_still_a_control_dependency() {
    for dataflow in [false, true] {
        let value = invoke(
            &format!("if-duty-{dataflow}"),
            &review::branch_control(dataflow),
            false,
            None,
            0,
        );
        let rows = value["responsibilities"].as_array().unwrap();
        let row = rows
            .iter()
            .find(|r| r["target"]["value"] == "Approver.approve")
            .unwrap();
        hand::subset(&json!({"status":"present","reason":null,"effects":[{"capability":"Ledger.post","status":"all_paths"}]}),
        &row["refusal"], "if control").unwrap();
    }
}

#[test]
fn m3_cli_scenario_drain_is_support_envelope() {
    let mut failures = Vec::new();
    for (last, reverse) in [(4, false), (4, true), (9, false)] {
        let value = invoke(
            &format!("drain-{last}-{reverse}"),
            &review::scenario_arrivals(last, reverse),
            true,
            Some("Day"),
            0,
        );
        if let Err(error) = review::drain(&value, last) {
            failures.push(error);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn green_cli_review_oracle_single_fault_pairs() {
    review::control_pairs();
}
