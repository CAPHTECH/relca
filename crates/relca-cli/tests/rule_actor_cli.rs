//! Independent CLI bridge and closed new-wire calibration for ADR0044.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/rule_acceptance.rs"]
mod h;
use serde_json::{Value, json};
use std::process::Command;
include!("../../relca-core/tests/support/rule_cases.rs");
fn fixture(name: &str) {
    let row = h::row(name);
    let explicit = (row["invocation"] != json!({"command":"check"})).then_some(&row["invocation"]);
    let invocation = fixture_match::Invocation::parse(explicit).unwrap();
    let path = h::root().join("fixtures").join(name).join("model.relca");
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(invocation.args(&path))
        .output()
        .unwrap();
    let actual: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        out.status.code(),
        Some(
            serde_json::from_value::<relca_core::Verdict>(row["expected"]["verdict"].clone())
                .unwrap()
                .exit_code()
        ),
        "{name}"
    );
    invocation
        .compare(&row["expected"], &actual)
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    h::verify(name, &actual);
}
#[test]
fn rule_judgement_union_is_closed_and_never_invents_process_or_person_fields() {
    let valid = json!({"kind":"rule","actor":"ApprovalRule","version":"r1","reads":["ApprovalRule.approval_required"]});
    fixture_match::validate_rule_judgement_wire(&valid).unwrap();
    for key in ["kind", "actor", "version", "reads"] {
        let mut bad = valid.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(
            fixture_match::validate_rule_judgement_wire(&bad).is_err(),
            "missing {key}"
        );
    }
    for (key, value) in [
        ("call", json!("a")),
        ("capability", json!("A.c")),
        ("stops", json!("all_paths")),
        ("bypasses", json!(0)),
        ("people", json!(["cfo"])),
        ("unknown", json!(true)),
    ] {
        let mut bad = valid.clone();
        bad[key] = value;
        assert!(
            fixture_match::validate_rule_judgement_wire(&bad).is_err(),
            "extra {key}"
        );
    }
    for (key, value) in [
        ("kind", json!("process")),
        ("actor", json!("")),
        ("reads", json!([])),
        ("reads", json!(["Other.x"])),
        ("reads", json!(["ApprovalRule.x", "ApprovalRule.x"])),
    ] {
        let mut bad = valid.clone();
        bad[key] = value;
        assert!(
            fixture_match::validate_rule_judgement_wire(&bad).is_err(),
            "bad {key}"
        );
    }
}
#[test]
fn actor_rule_and_diff_cells_refuse_nulls_numbers_missing_extra_and_wrong_direction() {
    assert!(!fixture_match::rule_actor_fixture("rule-unregistered"));
    assert!(fixture_match::rule_actor_fixture("rule-basic"));
    let valid = json!({"version":{"declared":"r1","effective":"r2"},"values":[{"name":"approval_required","declared":true,"effective":false}],"amendment":{"from":"r1","to":"r2","by":"cfo","under":"RuleChange","under_version":null},"read_by":["Ledger.post.requires"]});
    fixture_match::validate_rule_actor_wire(&valid).unwrap();
    for key in ["version", "values", "amendment", "read_by"] {
        let mut bad = valid.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(
            fixture_match::validate_rule_actor_wire(&bad).is_err(),
            "missing {key}"
        );
    }
    for (pointer, value) in [
        ("/values/0/effective", json!(1)),
        ("/amendment/from", json!("r2")),
        ("/amendment/to", json!("r0")),
        ("/version/effective", json!(null)),
        ("/values", json!([])),
    ] {
        let mut bad = valid.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            fixture_match::validate_rule_actor_wire(&bad).is_err(),
            "bad {pointer}"
        );
    }
    let mut extra = valid.clone();
    extra["score"] = json!(1);
    assert!(fixture_match::validate_rule_actor_wire(&extra).is_err());
    let cell = json!({"version":"r1","values":[{"name":"approval_required","value":true}],"amendment":null});
    fixture_match::validate_rule_diff_cell_wire(&cell).unwrap();
    for key in ["version", "values", "amendment"] {
        let mut bad = cell.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(fixture_match::validate_rule_diff_cell_wire(&bad).is_err());
    }
    assert!(fixture_match::validate_rule_diff_cell_wire(&Value::Null).is_err());
}

#[test]
fn check_explain_diff_and_mutation_keep_derived_rule_items() {
    let src = h::source("rule-agency");
    let check = h::actual(&src, &json!({"command":"check"}));
    let explain = h::actual(&src, &json!({"command":"explain"}));
    assert_eq!(
        check["agency"], explain["agency"],
        "explain shares Rule derivation, no configuration selector"
    );
    assert_eq!(
        check["agency"]["refusal"][0]["judgements"][0]["kind"],
        "rule"
    );
    // Existing mutation contract remains unsupported overall; A6 does not own
    // mutation generation. Its original Report must nevertheless retain Rule.
    let path = h::root().join("fixtures/rule-agency/model.relca");
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("mutate")
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
    let mutation: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(mutation["original"]["agency"], check["agency"]);
    assert!(
        mutation["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["kind"] == "stale_definition_write" && m["status"] == "unsupported")
    );
}
