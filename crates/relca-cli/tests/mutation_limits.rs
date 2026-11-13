//! ADR 0068 acceptance. Expectations come from the lexical model below,
//! not from captured CLI output. See mutation_limits.md for the calculation.
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};
#[path = "../src/fixture_match.rs"]
mod fixture_match;

const KINDS: [&str; 10] = [
    "evidence_missing",
    "authority_expired",
    "hold_delayed",
    "command_resent",
    "summary_reused",
    "stale_definition_write",
    "receipt_unknown",
    "old_promise_denominator_removed",
    "refusal_path_cut",
    "original_access_removed",
];

fn model() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/mutation_limits.relca")
}
fn invoke(options: &[&str]) -> (i32, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("mutate")
        .arg(model())
        .args(options)
        .output()
        .unwrap();
    let v = serde_json::from_slice(&output.stdout).unwrap();
    fixture_match::validate_mutation_wire(&v).unwrap();
    (output.status.code().unwrap(), v)
}
fn counts(v: &Value) {
    // Two reads, no guard, authority, repeated consumer, commit or cohort metric.
    for (i, row) in v["mutations"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            row["candidates"],
            if i == 0 || i == 9 {
                json!(2)
            } else if i == 5 || i == 7 {
                Value::Null
            } else {
                json!(0)
            },
            "kind {}",
            KINDS[i]
        );
    }
    assert_eq!(v["run"]["candidates_total"], 4);
}

#[test]
fn explicit_limit_is_accepted_and_recorded() {
    let (exit, v) = invoke(&["--mutant-limit", "4"]);
    assert_eq!(exit, 4);
    assert_eq!(v["run"]["completion"], "complete");
    assert_eq!(
        v["ledger"]["execution"]["limits"]["mutant_limit"]["value"],
        4
    );
}

#[test]
fn explicit_defaults_and_reordered_kind_lists_preserve_the_default_output() {
    let default = invoke(&[]);
    assert_eq!(default, invoke(&["--mutant-limit", "128"]));
    assert_eq!(default, invoke(&["--kinds", &KINDS.join(",")]));
    let reversed: Vec<_> = KINDS.iter().rev().copied().collect();
    assert_eq!(default, invoke(&["--kinds", &reversed.join(",")]));
}

#[test]
fn hand_candidate_counts_survive_truncation_and_default_limit() {
    let (exit, v) = invoke(&[]);
    assert_eq!(exit, 4); // unchanged unsupported-kind priority
    counts(&v);
    assert_eq!(v["run"]["mutant_limit"], 128);
    assert_eq!(
        v["ledger"]["execution"]["limits"]["mutant_limit"]["value"],
        128
    );
    for limit in ["3", "4", "5", "4096"] {
        let (exit, v) = invoke(&["--mutant-limit", limit]);
        assert_eq!(exit, 4);
        counts(&v);
        let n: u64 = limit.parse().unwrap();
        assert_eq!(v["run"]["mutant_limit"], n);
        assert_eq!(
            v["ledger"]["execution"]["limits"]["mutant_limit"]["value"],
            n
        );
        assert_eq!(
            v["run"]["completion"],
            if n == 3 { "inconclusive" } else { "complete" }
        );
        assert_eq!(v["run"]["limit_reached"], n == 3);
        assert_eq!(
            v["run"]["reason"],
            if n == 3 {
                json!("mutant_limit")
            } else {
                Value::Null
            }
        );
        let mutants: Vec<_> = v["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|r| r["mutants"].as_array().unwrap())
            .collect();
        assert_eq!(mutants.len(), 4);
        assert_eq!(
            mutants.iter().filter(|m| m["result"] != "not_run").count(),
            n.min(4) as usize
        );
        if n == 3 {
            assert_eq!(mutants[3]["result"], "not_run");
            assert_eq!(mutants[3]["reason"], "mutant_limit");
        }
    }
}

#[test]
fn selected_scope_has_exact_budget_and_visible_exclusions() {
    for (limit, expected_exit, completion) in [
        ("1", 2, "inconclusive"),
        ("2", 1, "complete"),
        ("3", 1, "complete"),
    ] {
        // The two read removals have no assigned changing check: undetected.
        let (exit, v) = invoke(&[
            "--kinds",
            "original_access_removed",
            "--mutant-limit",
            limit,
        ]);
        assert_eq!(exit, expected_exit);
        counts(&v);
        assert_eq!(v["run"]["completion"], completion);
        for (i, row) in v["mutations"].as_array().unwrap().iter().enumerate() {
            let excluded = json!(format!("mutation.kind.{}", KINDS[i]));
            assert_eq!(
                v["unchecked"].as_array().unwrap().contains(&excluded),
                i != 9
            );
            if i != 9 {
                assert_eq!(row["status"], "not_selected");
                assert_eq!(row["reason"], "not_selected");
                assert_eq!(row["mutants"], json!([]));
            } else {
                assert_eq!(row["status"], "generated");
                assert_eq!(row["mutants"].as_array().unwrap().len(), 2);
            }
        }
        assert!(
            !v["checked"]
                .as_array()
                .unwrap()
                .contains(&json!("authority_expiry"))
        );
        assert!(
            !v["checked"]
                .as_array()
                .unwrap()
                .contains(&json!("command_resend"))
        );
        assert!(
            !v["checked"]
                .as_array()
                .unwrap()
                .contains(&json!("receipt_unknown"))
        );
    }
    let (exit, v) = invoke(&["--kinds", "authority_expired"]);
    assert_eq!(exit, 5); // no executed mutant: vacuous, not a computational cut
    assert_eq!(v["run"]["reason"], "no_selected_mutant");
    assert_eq!(v["run"]["completion"], "complete");
    assert_eq!(v["mutations"][1]["status"], "not_applicable");
    let (exit, v) = invoke(&[
        "--kinds",
        "evidence_missing,original_access_removed",
        "--mutant-limit",
        "4",
    ]);
    assert_eq!(exit, 1);
    assert_eq!(v["run"]["completion"], "complete");
    assert_eq!(v["mutations"][0]["mutants"].as_array().unwrap().len(), 2);
    assert_eq!(v["mutations"][9]["mutants"].as_array().unwrap().len(), 2);
    let (exit, v) = invoke(&["--kinds", "stale_definition_write"]);
    assert_eq!(exit, 4); // Unsupported keeps precedence even with no executed mutant.
    assert_eq!(v["mutations"][5]["status"], "unsupported");
}

#[test]
fn invalid_limit_and_kind_arguments_fail_before_model_check() {
    for options in [
        vec!["--mutant-limit", "0"],
        vec!["--mutant-limit", "-1"],
        vec!["--mutant-limit", "1.5"],
        vec!["--mutant-limit", "NaN"],
        vec!["--mutant-limit", "4097"],
        vec!["--mutant-limit", "99999999999999999999999999999"],
        vec!["--mutant-limit", ""],
        vec!["--mutant-limit", "+1"],
        vec!["--mutant-limit", " 2"],
        vec!["--mutant-limit"],
        vec!["--mutant-limit", "2", "--mutant-limit", "3"],
        vec!["--kinds", "unknown"],
        vec!["--kinds", ""],
        vec!["--kinds"],
        vec!["--kinds", "evidence_missing,"],
        vec!["--kinds", ",evidence_missing"],
        vec!["--kinds", "evidence_missing,,hold_delayed"],
        vec!["--kinds", "evidence_missing,evidence_missing"],
        vec!["--kinds", " evidence_missing"],
        vec!["--kinds", "evidence_missing", "--kinds", "hold_delayed"],
    ] {
        let (exit, v) = invoke(&options);
        assert_eq!(exit, 3, "{options:?}");
        assert_eq!(v["verdict"], "invalid");
        assert_eq!(v["run"]["reason"], "invalid_arguments");
        assert!(v["original"].is_null());
    }
}

#[test]
fn count_and_selection_wire_rejects_false_success_and_lost_candidates() {
    let (_, v) = invoke(&["--kinds", "original_access_removed", "--mutant-limit", "1"]);
    for (pointer, value) in [
        ("/run/candidates_total", json!(1)),
        ("/mutations/9/candidates", json!(1)),
        ("/mutations/0/status", json!("satisfied")),
        ("/mutations/0/reason", Value::Null),
        ("/run/mutant_limit", json!(0)),
        ("/run/mutant_limit", json!(4097)),
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            fixture_match::validate_mutation_wire(&bad).is_err(),
            "{pointer}"
        );
    }
    let mut bad = v.clone();
    bad["mutations"][0]
        .as_object_mut()
        .unwrap()
        .remove("candidates");
    assert!(fixture_match::validate_mutation_wire(&bad).is_err());
    let mut bad = v;
    bad["unchecked"]
        .as_array_mut()
        .unwrap()
        .retain(|w| w != "mutation.kind.evidence_missing");
    assert!(fixture_match::validate_mutation_wire(&bad).is_err());
}

#[test]
fn selected_denominator_kind_uses_the_same_closed_transport() {
    let source = r#"model Denominator version "0.2" {
        type T = enum { a,b }
        actor A { capacity 1; capability choose() -> T { duration fixed 1min; result nondet { a,b } } }
        process P { v = A.choose(); if v == a { end yes } else { end no } }
        cut C { include [A] }
        bounds { cases 1; live_tasks 1; clock 5min }
        verify safety no_resource_overcommit
        property case.Done = reached(end.yes || end.no)
        metric cohort.Rate = expected(count(case.Done)) / arrivals
    }"#;
    for limit in ["1", "2"] {
        let r = relca_core::mutation::mutate_with_options(
            source,
            "bpr-default",
            None,
            &relca_core::mutation::Options::parse(
                Some(limit),
                Some("old_promise_denominator_removed"),
            )
            .unwrap(),
        );
        let v = serde_json::to_value(r).unwrap();
        fixture_match::validate_mutation_wire(&v).unwrap();
        let selected = relca_core::mutation::Options::parse(
            Some(limit),
            Some("old_promise_denominator_removed"),
        )
        .unwrap()
        .with_sites(Some(
            "old_promise_denominator_removed:cohort.Rate:remove:no",
        ))
        .unwrap();
        let r = relca_core::mutation::mutate_with_options(source, "bpr-default", None, &selected);
        fixture_match::validate_mutation_wire(&serde_json::to_value(r).unwrap()).unwrap();
    }
}

#[test]
fn excluded_site_transport_has_no_report_and_exact_unchecked_membership() {
    let (_, v) = invoke(&[
        "--kinds",
        "original_access_removed",
        "--sites",
        "original_access_removed:Probe.read:reads:B",
        "--mutant-limit",
        "1",
    ]);
    for (pointer, value) in [
        ("/mutations/9/mutants/0/result", json!("detected")),
        ("/mutations/9/mutants/0/reason", Value::Null),
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(fixture_match::validate_mutation_wire(&bad).is_err());
    }
}

#[test]
fn zero_time_expiry_reason_is_closed_and_not_a_false_time_claim() {
    let src = include_str!("../../../fixtures/mut2-expiry/model.relca")
        .replace("duration fixed 1min", "duration fixed 0min");
    let r = relca_core::mutation::mutate_with_options(
        &src,
        "bpr-default",
        None,
        &relca_core::mutation::Options::parse(None, Some("authority_expired")).unwrap(),
    );
    let v = serde_json::to_value(r).unwrap();
    fixture_match::validate_mutation_wire(&v).unwrap();
    assert_eq!(v["mutations"][1]["reason"], "no_time_elapse");
    let mut bad = v;
    bad["original"]["terminals"][0]["elapsed"]["max"] = json!(1);
    assert!(fixture_match::validate_mutation_wire(&bad).is_err());
}
