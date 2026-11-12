//! ADR 0009 binary acceptance. Oracles are hand-written fixture literals.

use serde_json::{Value, json};
use std::{path::Path, process::Command};
#[path = "support/ledger_shape.rs"]
mod ledger_shape;

fn path(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
        .join("model.relca")
        .to_str()
        .unwrap()
        .to_owned()
}

fn run(args: &[&str], exit: i32) -> (Value, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let value: Value = serde_json::from_str(&stdout).expect("exactly one JSON value on stdout");
    assert_eq!(output.status.code(), Some(exit), "{args:?}: {value}");
    (value, String::from_utf8(output.stderr).unwrap())
}

#[test]
fn warning_only_agency_results_keep_exit_zero_and_one_stdout_json() {
    for (name, code) in [
        ("agency-any", "W-AGY-1"),
        ("agency-any.broken-single-valued-a", "W-AGY-0"),
    ] {
        let (actual, stderr) = run(&["check", &path(name)], 0);
        assert_eq!(actual["verdict"], "satisfied");
        assert_eq!(ledger_shape::original_field_count(&actual), 13);
        assert_eq!(actual["agency"]["rules"], "agency-p0-1");
        assert!(
            actual["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code && d["severity"] == "warning")
        );
        assert!(stderr.contains(code), "stderr={stderr}");
    }
}

#[test]
fn diff_agency_is_non_symmetric_and_identical_to_both_selected_reports() {
    let model = path("agency-refusal");
    let (actual, _) = run(&["diff", &model, "--from", "AsIs", "--to", "ToBe"], 0);
    assert_eq!(ledger_shape::original_field_count(&actual), 11);
    assert_ne!(actual["agency"]["from"], actual["agency"]["to"]);
    for (side, name) in [("from", "AsIs"), ("to", "ToBe")] {
        let (selected, _) = run(&["check", &model, "--configuration", name], 0);
        assert_eq!(actual["agency"][side], selected["agency"]);
        assert_eq!(actual["agency"][side], actual[side]["report"]["agency"]);
    }
    assert_eq!(
        actual["agency"]["from"]["refusal"][0]["judgements"][0]["stops"],
        "all_paths"
    );
    assert_eq!(
        actual["agency"]["to"]["refusal"][0]["judgements"][0]["stops"],
        "conditional"
    );
    assert!(
        !actual["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("agency"))
    );
}

#[test]
fn input_and_selection_failures_have_explicit_null_agency() {
    let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/agency-no-such-input.relca");
    assert!(!missing.exists(), "I/O fault input must really be absent");
    let (actual, _) = run(&["check", missing.to_str().unwrap()], 3);
    assert_eq!(actual.get("agency"), Some(&Value::Null));
    assert_eq!(ledger_shape::original_field_count(&actual), 13);
    let (actual, _) = run(
        &[
            "check",
            &path("agency-refusal"),
            "--configuration",
            "Unknown",
        ],
        3,
    );
    assert_eq!(actual.get("agency"), Some(&Value::Null));
}
