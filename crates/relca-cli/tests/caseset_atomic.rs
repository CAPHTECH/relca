#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn run(name: &str, command: &str, extra: &[&str]) -> (Value, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args([command, &format!("fixtures/{name}/model.relca")])
        .args(extra)
        .current_dir(root())
        .output()
        .unwrap();
    (
        serde_json::from_slice(&output.stdout).unwrap(),
        output.status.code().unwrap(),
    )
}

#[test]
fn notes_vocabulary_is_closed_and_no_wrong_property_or_empty_annotation_is_accepted() {
    let good = json!({"properties":[{"id":"contract C #1 at_most_once","verdict":"satisfied","notes":["mechanism_unmodeled"]}]});
    assert!(fixture_match::validate_caseset_notes(&good).is_ok());
    for notes in [
        json!([]),
        Value::Null,
        json!(["unknown"]),
        json!(["mechanism_unmodeled", "mechanism_unmodeled"]),
    ] {
        let mut bad = good.clone();
        bad["properties"][0]["notes"] = notes;
        assert!(fixture_match::validate_caseset_notes(&bad).is_err());
    }
    let mut bad = good;
    bad["properties"][0]["id"] = json!("safety no_resource_overcommit");
    assert!(fixture_match::validate_caseset_notes(&bad).is_err());
}

#[test]
fn check_simulate_explain_and_compare_transport_notes_without_table_annotations() {
    for (command, args, exit) in [
        (
            "compare",
            vec!["--plans", "Left,Right", "--mode", "exhaustive"],
            0,
        ),
        (
            "compare",
            vec!["--plans", "Left,Right", "--mode", "simulation"],
            2,
        ),
        ("check", vec!["--configuration", "Left"], 0),
        ("simulate", vec!["--configuration", "Left"], 2),
    ] {
        let (v, code) = run("aset-compare-notes", command, &args);
        assert_eq!(code, exit, "{command}: {v}");
        let reports: Vec<_> = if command == "compare" {
            v["plans"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| &p["report"])
                .collect()
        } else {
            vec![&v]
        };
        for r in reports {
            let row = r["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"] == "contract C #1 at_most_once")
                .unwrap();
            assert_eq!(row["notes"], json!(["mechanism_unmodeled"]));
        }
        if command == "compare" {
            assert!(!v["tables"].to_string().contains("notes"));
        }
    }
    // Explain cannot select configuration: use a single-process source instead.
    let (v, code) = run("aset-dup.broken-no-check", "explain", &[]);
    assert_eq!(code, 1);
    let row = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "contract C #1 at_most_once")
        .unwrap();
    assert_eq!(row["notes"], json!(["mechanism_unmodeled"]));
    fixture_match::validate_explain_wire(&v).unwrap();
}

#[test]
fn counterexample_set_value_is_an_array_not_a_string_and_person_is_named() {
    let (v, exit) = run("aset-replay-display", "explain", &[]);
    assert_eq!(exit, 1);
    assert_eq!(v["explanation"]["status"], "complete");
    let paths = v["explanation"]["counterexamples"].as_array().unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0]["replay"]["status"], "verified");
    let last = paths[0]["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(last["time"], 5);
    assert!(
        last["values"]["observed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["name"] == "Ledger.keys" && x["value"] == json!([1, 2]))
    );
    fixture_match::validate_explain_wire(&v).unwrap();
    for bad in [json!([2, 1]), json!([1, 1]), json!([0, 2]), json!([1, 3])] {
        let mut forged = v.clone();
        let stages = forged["explanation"]["counterexamples"][0]["steps"]
            .as_array_mut()
            .unwrap();
        let row = stages.last_mut().unwrap()["values"]["observed"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|x| x["name"] == "Ledger.keys")
            .unwrap();
        row["value"] = bad;
        assert!(fixture_match::validate_explain_wire(&forged).is_err());
    }
}

#[test]
fn alpha_and_grain_changes_preserve_every_unjustified_output_byte() {
    let output = Command::new("python3")
        .args([
            "-B",
            "crates/relca-core/tests/support/caseset_alpha.py",
            env!("CARGO_BIN_EXE_relca"),
        ])
        .current_dir(root())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
