//! Site IDs reuse the existing mutant ID: kind + the existing lexical site.
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

fn invoke(options: &[&str]) -> (i32, Value) {
    let model =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/mutation_limits.relca");
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("mutate")
        .arg(model)
        .args(options)
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        serde_json::from_slice(&out.stdout).unwrap(),
    )
}

#[test]
fn one_of_two_hand_read_sites_is_excluded_not_truncated() {
    let id = "original_access_removed:Probe.read:reads:B";
    let (exit, v) = invoke(&[
        "--kinds",
        "original_access_removed",
        "--sites",
        id,
        "--mutant-limit",
        "1",
    ]);
    assert_eq!(exit, 1);
    assert_eq!(v["run"]["completion"], "complete");
    assert_eq!(v["run"]["limit_reached"], false);
    assert_eq!(v["run"]["candidates_total"], 4);
    let row = &v["mutations"][9];
    assert_eq!(row["candidates"], 2);
    assert_eq!(
        row["mutants"][0]["id"],
        "original_access_removed:Probe.read:reads:A"
    );
    assert_eq!(row["mutants"][0]["result"], "not_selected");
    assert_eq!(row["mutants"][0]["reason"], "not_selected");
    assert!(row["mutants"][0]["report"].is_null());
    assert_eq!(row["mutants"][1]["id"], id);
    assert_eq!(row["mutants"][1]["result"], "undetected");
    assert!(v["unchecked"].as_array().unwrap().contains(&json!(
        "mutation.site.original_access_removed:Probe.read:reads:A"
    )));
    assert!(
        !v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!(format!("mutation.site.{id}")))
    );
    // An identifier is valid even if --kinds excludes its kind: the intersection
    // is empty, so neither read deletion runs and this is not a resource cut.
    let (exit, v) = invoke(&["--kinds", "evidence_missing", "--sites", id]);
    // Round 1 decision: an empty intersection is not a successful mutation test.
    assert_eq!(exit, 5);
    assert_eq!(v["verdict"], "vacuous");
    assert_eq!(v["run"]["reason"], "no_selected_mutant");
    assert_eq!(v["run"]["completion"], "complete");
    assert!(
        v["mutations"][0]["mutants"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["result"] == "not_selected")
    );
}

#[test]
fn unknown_empty_duplicate_and_malformed_site_lists_are_invalid() {
    for sites in [
        "",
        "unknown",
        "original_access_removed:Probe.read:reads:Z",
        ",x",
        "x,",
        "x,x",
    ] {
        let (exit, v) = invoke(&["--sites", sites]);
        assert_eq!(exit, 3, "{sites}");
        assert_eq!(v["verdict"], "invalid");
    }
}
