//! Real CLI transport + recursively closed C6/A7 wire, not a synthetic Report.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/waits_mutation_inputs.rs"]
mod i;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn invoke(s: &str, command: &str, args: &[&str], exit: i32) -> Value {
    static ID: AtomicUsize = AtomicUsize::new(0);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tmp = test_tmp::root();
    std::fs::create_dir_all(&tmp).unwrap();
    // Keep the checked absolute directory when changing the child's cwd.
    let tmp = tmp.canonicalize().unwrap();
    assert!(tmp.starts_with(root.canonicalize().unwrap()));
    let path = tmp.join(format!(
        "a7onto-cli-{}-{}.relca",
        std::process::id(),
        ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, s).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(&path)
        .args(args)
        .current_dir(&root)
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        o.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    serde_json::from_slice(&o.stdout).expect("single JSON, never panic")
}
fn mutate(s: &str) -> Value {
    let v = invoke(s, "mutate", &["--scenario", "Day"], 4);
    fixture_match::validate_mutation_wire(&v).unwrap_or_else(|e| panic!("{e}"));
    v
}
#[test]
fn hq_horizon12_cli_keeps_selected_metadata_and_waiting_metrics() {
    let s = i::hq();
    let v = invoke(&s, "mutate", &["--scenario", "Day"], 4);
    let control = invoke(&i::masked(&s), "mutate", &["--scenario", "Day"], 4);
    fixture_match::validate_mutation_wire(&v).unwrap();
    fixture_match::validate_mutation_wire(&control).unwrap();
    i::envelope_with_ends(&v, &control, &["done"]);
    assert_eq!(v["original"]["scenario"]["horizon"], 12);
    i::cases(
        &v["original"],
        ["5", "10"],
        ["5", "5"],
        ["0", "5"],
        "A",
        "5",
        "1/2",
    );
    for k in [0, 1, 2, 4, 8, 9] {
        assert_eq!(v["mutations"][k]["status"], "not_applicable");
    }
    assert_eq!(v["run"]["completion"], "complete");
}
#[test]
fn all_six_cli_report_paths_keep_closed_wait_wire_and_assignment_identity() {
    let s = i::summary(12, 20, true);
    let v = mutate(&s);
    i::envelope(&v, &mutate(&i::masked(&s)));
    for (k, n) in [(0, 2), (1, 1), (2, 1), (4, 1), (8, 1), (9, 2)] {
        assert_eq!(v["mutations"][k]["mutants"].as_array().unwrap().len(), n);
    }
    i::held(i::only(&v, 4));
}
#[test]
fn summary_cli_observes_queue_change_but_detection_only_uses_held_lineage() {
    let s = i::summary(12, 20, false);
    let v = mutate(&s);
    i::envelope(&v, &mutate(&i::masked(&s)));
    i::held(i::only(&v, 4));
    i::cases(
        &v["original"],
        ["4", "5"],
        ["4", "4"],
        ["0", "1"],
        "Extract",
        "1",
        "1/5",
    );
    i::cases(
        &i::only(&v, 4)["report"],
        ["6", "7"],
        ["5", "5"],
        ["1", "2"],
        "Extract",
        "3",
        "3/7",
    );
}
#[test]
fn business_and_computational_incompleteness_are_never_detected_cli_rows() {
    for (h, c, reason, status) in [
        (6, 20, "assigned_check_unavailable", "complete"),
        (12, 6, "truncated", "truncated"),
    ] {
        let s = i::summary(h, c, false);
        let v = mutate(&s);
        i::envelope(&v, &mutate(&i::masked(&s)));
        let m = i::only(&v, 4);
        assert_eq!(m["result"], "inconclusive");
        assert_eq!(m["reason"], reason);
        assert_eq!(m["report"]["metrics"]["status"], status);
        assert_eq!(v["run"]["completion"], "inconclusive");
        let mut forged = v.clone();
        forged["mutations"][4]["mutants"][0]["result"] = json!("detected");
        forged["mutations"][4]["mutants"][0]["reason"] = Value::Null;
        assert!(
            fixture_match::validate_mutation_wire(&forged).is_err(),
            "unfinished detected report must fail closed"
        );
    }
}
#[test]
fn changed_intentional_wait_and_quality_do_not_detect_unassigned_cli_mutants() {
    let s = i::waiting_only();
    let v = mutate(&s);
    i::envelope(&v, &mutate(&i::masked(&s)));
    for k in [2, 8] {
        let m = i::only(&v, k);
        assert_eq!(m["result"], "undetected");
        assert_eq!(m["reason"], "no_assigned_check");
        assert!(
            m["report"]["metrics"]["cases"][0]
                .get("intentional_wait")
                .is_none()
        );
        let done = m["report"]["metrics"]["cases"][0]["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["end"] == "done")
            .unwrap();
        assert_eq!(done["breakdown"]["intentional"], i::range("0"));
        assert_eq!(
            v["original"]["metrics"]["cases"][0]["intentional_wait"],
            i::range("2")
        );
    }
}
#[test]
fn missing_selector_and_missing_simulation_refuse_without_waiting_explanation() {
    let v = invoke(&i::hq(), "mutate", &[], 3);
    fixture_match::validate_mutation_wire(&v).unwrap();
    i::no_metrics_no_waiting(&v["original"]);
    let v = invoke(
        &i::summary(12, 20, false),
        "simulate",
        &["--scenario", "Day"],
        3,
    );
    assert!(v.get("metrics").is_none());
    assert!(
        !v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "metrics.waiting")
    );
    let fs: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["field"].as_str().unwrap())
        .collect();
    assert_eq!(
        fs,
        vec![
            "quality.definition",
            "quality.denominator",
            "quality.form",
            "quality.estimation"
        ]
    );
}
#[test]
fn comparator_rejects_missing_assumption_unknown_queue_and_changed_population() {
    let v = mutate(&i::summary(12, 20, true));
    for k in [0, 1, 2, 4, 8, 9] {
        for surface in ["original", "mutant"] {
            let mut bad = v.clone();
            let r = if surface == "original" {
                &mut bad["original"]
            } else {
                &mut bad["mutations"][k]["mutants"][0]["report"]
            };
            r["assumptions"]
                .as_array_mut()
                .unwrap()
                .retain(|a| a["field"] != "metrics.waiting");
            assert!(
                fixture_match::validate_mutation_wire(&bad).is_err(),
                "kind{k} {surface} missing assumption"
            );
            let mut bad = v.clone();
            bad["mutations"][k]["mutants"][0]["report"]["metrics"]["actors"][0]["queue"]["arbitrary"] =
                json!(0);
            assert!(
                fixture_match::validate_mutation_wire(&bad).is_err(),
                "unknown queue key kind{k}"
            );
        }
    }
    let mut bad = v.clone();
    bad["mutations"].as_array_mut().unwrap().remove(7);
    assert!(
        fixture_match::validate_mutation_wire(&bad).is_err(),
        "structural kind7 cannot vanish"
    );
}

#[test]
fn restricted_rule_expiry_cli_mutant_has_no_metrics_or_waiting_explanation() {
    let v = invoke(&i::rule_refusal(), "mutate", &[], 4);
    fixture_match::validate_mutation_wire(&v).unwrap();
    assert_eq!(v["original_verdict"], "satisfied");
    i::order(&v["original"]);
    let m = i::only(&v, 1);
    assert_eq!(m["result"], "invalid_mutant");
    assert_eq!(m["reason"], "static_unsupported");
    i::no_metrics_no_waiting(&m["report"]);
    assert_eq!(v["run"]["completion"], "inconclusive");
    assert_eq!(v["run"]["reason"], "mutant_incomplete");
    i::order(&i::only(&v, 4)["report"]);
}

#[test]
fn rule_metric_tail_rejects_single_missing_or_reordered_rows_before_projection() {
    let v = invoke(&i::rule_refusal(), "mutate", &[], 4);
    fixture_match::validate_mutation_wire(&v).unwrap();
    for original in [true, false] {
        for fault in [
            "missing_rule",
            "rule_grant_swap",
            "rule_metrics_swap",
            "quality_rule_swap",
        ] {
            let mut bad = v.clone();
            let report = if original {
                &mut bad["original"]
            } else {
                &mut bad["mutations"][4]["mutants"][0]["report"]
            };
            let rows = report["assumptions"].as_array_mut().unwrap();
            let rule = rows
                .iter()
                .position(|a| a["field"] == "ApprovalRule.rule")
                .unwrap();
            let grant = rows
                .iter()
                .position(|a| a["field"] == "authority.RuleChange.change.ApprovalRule.cfo")
                .unwrap();
            let metrics = rows
                .iter()
                .position(|a| a["field"] == "metrics.observation")
                .unwrap();
            // Expected adjacency comes from the Rule tail contract, not sorting.
            assert_eq!(rule, metrics + 1);
            assert_eq!(grant, rule + 1);
            match fault {
                "missing_rule" => {
                    rows.remove(rule);
                }
                "rule_grant_swap" => rows.swap(rule, grant),
                "rule_metrics_swap" => rows.swap(rule, metrics),
                "quality_rule_swap" => {
                    let rule = rows
                        .iter()
                        .position(|a| a["field"] == "quality.form")
                        .unwrap();
                    let scheduler = rows
                        .iter()
                        .position(|a| a["field"] == "quality.scheduler")
                        .unwrap();
                    rows.swap(rule, scheduler);
                }
                _ => unreachable!(),
            }
            assert!(
                fixture_match::validate_mutation_wire(&bad).is_err(),
                "{fault} original={original}"
            );
        }
    }
}
