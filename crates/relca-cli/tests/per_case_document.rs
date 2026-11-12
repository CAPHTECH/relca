//! Real CLI acceptance, exact fixture inventory, and independent closed wire.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/percase_acceptance.rs"]
mod p;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn run(command: &str, name: &str) -> (Value, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args([command, &format!("fixtures/{name}/model.relca")])
        .current_dir(root())
        .output()
        .unwrap();
    (
        serde_json::from_slice(&out.stdout).unwrap(),
        out.status.code().unwrap(),
    )
}
#[test]
fn pcase_34_closed_expectations_and_single_process_explain_replay() {
    assert_eq!(fixture_match::PERCASE_FIXTURES, p::NAMES);
    for name in p::NAMES {
        let (v, exit) = run("check", name);
        let h = p::expected(name);
        fixture_match::compare_report(&h, &v).unwrap_or_else(|e| panic!("{name}: {e}"));
        let verdict: relca_core::Verdict = serde_json::from_value(h["verdict"].clone()).unwrap();
        assert_eq!(exit, verdict.exit_code(), "{name}");
    }
    for name in ["pcase-explain", "pcase-explain.broken-invariant"] {
        let (v, exit) = run("explain", name);
        assert_eq!(exit, 1);
        fixture_match::validate_explain_wire(&v).unwrap();
        assert_eq!(
            v["explanation"]["counterexamples"][0]["replay"]["status"],
            "verified"
        );
    }
}
#[test]
fn per_case_wire_rejects_unknown_null_type_duplicates_and_false_shared() {
    let (v, _) = run("check", "pcase-common");
    fixture_match::validate_common_cause_agency(&v["agency"]).unwrap();
    assert_eq!(
        v["agency"]["common_causes"]["causes"][0]["instance"],
        "per_case"
    );
    p::assumptions(&v, &["Invoice"], true);
    for value in [
        json!("shared"),
        json!("per_process"),
        Value::Null,
        json!(true),
        json!(1),
        json!([]),
    ] {
        let mut a = v["agency"].clone();
        a["common_causes"]["causes"][0]["instance"] = value;
        assert!(fixture_match::validate_common_cause_agency(&a).is_err());
    }
    let mut a = v["agency"].clone();
    let row = a["common_causes"]["causes"][0].clone();
    a["common_causes"]["causes"]
        .as_array_mut()
        .unwrap()
        .push(row);
    assert!(fixture_match::validate_common_cause_agency(&a).is_err());
    // Wire cannot know the source: bind required ownership to an expectation.
    let mut missing = v.clone();
    missing["agency"]["common_causes"]["causes"][0]
        .as_object_mut()
        .unwrap()
        .remove("instance");
    assert!(fixture_match::compare_report(&json!({"agency":{"common_causes":{"causes":[{"actor":"Invoice","latent":"Invoice.difficulty","instance":"per_case"}]}}}),&missing).is_err());
    for name in [
        "pcase-capacity.broken-capacity",
        "pcase-optin.broken-unmarked",
    ] {
        let (bad, _) = run("check", name);
        assert!(bad["agency"].is_null());
        assert!(
            !bad["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["field"] == "initial.actor_case_independence")
        );
    }
}

#[test]
fn optional_instance_wire_table_has_parser_independent_negative_controls() {
    let old = include_str!("../../../fixtures/lin-shared-kernel/model.relca");
    let mut report = serde_json::to_value(relca_core::report::check(old)).unwrap();
    fixture_match::validate_common_cause_agency(&report["agency"]).unwrap();
    report["agency"]["common_causes"]["causes"][0]["instance"] = json!("per_case");
    fixture_match::validate_common_cause_agency(&report["agency"]).unwrap();
    let expected = json!({"agency":{"common_causes":{"causes":[{"actor":"Invoice","latent":"Invoice.difficulty","instance":"per_case"}]}}});
    fixture_match::validate_expected(&expected).unwrap();
    for value in [
        json!("shared"),
        json!("other"),
        Value::Null,
        json!(false),
        json!(1),
        json!({}),
    ] {
        let mut bad = report.clone();
        bad["agency"]["common_causes"]["causes"][0]["instance"] = value;
        assert!(fixture_match::validate_common_cause_agency(&bad["agency"]).is_err());
    }
    let mut missing = report.clone();
    missing["agency"]["common_causes"]["causes"][0]
        .as_object_mut()
        .unwrap()
        .remove("instance");
    fixture_match::validate_common_cause_agency(&missing["agency"]).unwrap();
    fixture_match::compare_report(&json!({}), &missing).unwrap();
    assert!(
        fixture_match::compare_report(&expected, &missing).is_err(),
        "source-bound required instance cannot silently vanish"
    );
    let row = report["agency"]["common_causes"]["causes"][0].clone();
    report["agency"]["common_causes"]["causes"]
        .as_array_mut()
        .unwrap()
        .push(row);
    assert!(fixture_match::validate_common_cause_agency(&report["agency"]).is_err());
}
#[test]
fn cli_simulation_preserves_key_scope_assumptions_and_unmarked_case0() {
    for name in ["pcase-keys", "pcase-keys.broken-unmarked-single"] {
        let (v, exit) = run("simulate", name);
        assert_eq!(exit, 0, "statistical <=1 is satisfied, not a safety proof");
        fixture_match::validate_simulation_wire(&v).unwrap();
        p::assumptions(
            &v,
            if name.contains("broken") {
                &[]
            } else {
                &["Invoice"]
            },
            !name.contains("broken"),
        );
    }
}
