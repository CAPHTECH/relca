//! Test-first ADR 0073: old-language substrate checked at the real parser/lower
//! boundary; new wire expectations are authored by hand in the Python suite.
use std::{path::Path, process::Command};

#[test]
fn old_language_intent_substrates_parse_and_lower() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("python3")
        .args(["-B", "crates/relca-cli/tests/intent_pairs/inputs.py"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let values: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for (name, value) in values.as_object().unwrap() {
        let source = value.as_str().unwrap();
        let ast = relca_core::syntax::parse(source)
            .unwrap_or_else(|d| panic!("{name}: old-language parse: {d:?}"));
        for configuration in ["A", "B"] {
            relca_core::lower::lower_configuration(&ast, configuration)
                .unwrap_or_else(|d| panic!("{name}/{configuration}: old-language lower: {d:?}"));
        }
    }
}

fn acceptance(stage: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("python3")
        .args([
            "-B",
            "crates/relca-cli/tests/intent_pairs/cli.py",
            "--binary",
            env!("CARGO_BIN_EXE_relca"),
            "--stage",
            stage,
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn old_inputs_and_hand_substrate_geometry() {
    acceptance("legacy");
}

#[test]
fn stage_one_pair_intent_acceptance() {
    acceptance("pairs");
}

#[test]
fn stage_two_replace_acceptance() {
    acceptance("replace");
}

#[test]
fn hand_wire_fault_calibration() {
    acceptance("calibration");
}

#[test]
fn new_diff_wire_roundtrips_in_core_and_fixture_consumers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("python3")
        .args([
            "-B",
            "crates/relca-cli/tests/intent_pairs/inputs.py",
            "--new-wire-inputs",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(out.status.success());
    let inputs: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for (name, source) in inputs.as_object().unwrap() {
        let report = relca_core::diff::diff(source.as_str().unwrap(), "A", "B");
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["verdict"], "satisfied", "{name}: {value}");
        let decoded: relca_core::diff::DiffReport = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), value);
        relca_test_support::fixture_match::compare_diff_report(&serde_json::json!({}), &value)
            .unwrap_or_else(|e| panic!("{name}: fixture consumer must accept new wire: {e}"));
        if name == "replace" {
            let mut corrupt = value;
            corrupt["differences"][0]["covered_by"][0]["via"] = serde_json::json!("wildcard");
            assert!(
                serde_json::from_value::<relca_core::diff::DiffReport>(corrupt.clone()).is_err()
            );
            assert!(
                relca_test_support::fixture_match::compare_diff_report(
                    &serde_json::json!({}),
                    &corrupt
                )
                .is_err()
            );
        }
    }
}
