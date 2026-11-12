#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/rule_quality_inputs.rs"]
mod h;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::{Value, json};
use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn run(src: &str, command: &str, args: &[&str]) -> (Value, i32) {
    let dir = test_tmp::root().join(format!("k3-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{}.relca", NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&p, src).unwrap();
    let x = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(p)
        .args(args)
        .output()
        .unwrap();
    (
        serde_json::from_slice(&x.stdout).unwrap(),
        x.status.code().unwrap(),
    )
}
#[test]
fn real_check_simulate_rule_quantiles_and_closed_wire() {
    for sim in [false, true] {
        for (c, enabled) in [("AsIs", true), ("ToBeB", false)] {
            let (v, exit) = run(
                &h::source(),
                if sim { "simulate" } else { "check" },
                &["--configuration", c],
            );
            assert_eq!(exit, if sim { 2 } else { 0 });
            h::assert_report(&v, enabled, sim);
            if sim {
                fixture_match::validate_simulation_wire(&v).unwrap()
            } else {
                fixture_match::compare_report(&json!({}), &v).unwrap()
            }
            let mut bad = v.clone();
            bad["assumptions"]
                .as_array_mut()
                .unwrap()
                .push(h::old::quality_rule_assumption());
            assert!(
                if sim {
                    fixture_match::validate_simulation_wire(&bad)
                } else {
                    fixture_match::compare_report(&json!({}), &bad)
                }
                .is_err()
            );
        }
    }
}
#[test]
fn real_compare_cp_boundary_and_exit() {
    for sim in [false, true] {
        for tight in [false, true] {
            let s = if tight {
                h::old::replace(&h::source(), "<= 0.6", "<= 0.4")
            } else {
                h::source()
            };
            let (v, exit) = run(
                &s,
                "compare",
                &[
                    "--plans",
                    "AsIs,ToBeB",
                    "--mode",
                    if sim { "simulation" } else { "exhaustive" },
                ],
            );
            assert_eq!(exit, if sim && !tight { 2 } else { 1 });
            h::assert_compare(&v, sim, tight);
        }
    }
}
#[test]
fn real_revocable_rule_keeps_numeric_quality_and_unchecked_revocation() {
    let (v, exit) = run(
        &h::revocable_source(),
        "check",
        &["--configuration", "ToBeB"],
    );
    assert_eq!(exit, 0);
    h::assert_revocable_report(&v);
    fixture_match::compare_report(&json!({}), &v).unwrap();
}
#[test]
fn real_invoice_rule_risks_and_contract_violation_remain_separate() {
    for (c, risk) in [
        ("AsIs", "93/125000"),
        ("ToBeP", "93/125000"),
        ("ToBeA", "231/125000"),
        ("ToBeB", "81/12500"),
    ] {
        let s = h::invoice_rule();
        let without_defer = h::without_defer(include_str!("../../../examples/invoice/rules.relca"));
        let input = if c == "ToBeB" { &without_defer } else { &s };
        let (v, exit) = run(input, "check", &["--configuration", c]);
        assert_eq!(v["quality"]["cohort"][0]["value"], h::exact(risk));
        assert!(
            !v["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["field"] == "quality.rule")
        );
        if c == "ToBeB" {
            assert_eq!(exit, 1);
            assert_eq!(
                v["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["id"] == "contract InvoiceControls #1 invariant")
                    .unwrap()["verdict"],
                "violated"
            );
        }
    }
}
#[test]
fn no_rule_positive_control_keeps_existing_risk() {
    let (v, exit) = run(
        &h::without_defer(include_str!("../../../examples/invoice/phase1.relca")),
        "check",
        &["--configuration", "AsIs"],
    );
    assert_eq!(exit, 0);
    assert_eq!(v["quality"]["cohort"][0]["value"], h::exact("93/125000"));
}
#[test]
fn approval_fail_is_not_silently_counted_as_success() {
    for sim in [false, true] {
        for pass in [false, true] {
            for c in ["AsIs", "ToBeB"] {
                let (v, exit) = run(
                    &h::approval_source(pass),
                    if sim { "simulate" } else { "check" },
                    &["--configuration", c],
                );
                h::assert_approval(&v["quality"], pass || c == "ToBeB", sim);
                if !pass && c == "ToBeB" {
                    assert_eq!(exit, 1);
                }
            }
        }
    }
}
#[test]
fn matcher_has_hand_positive_and_two_independent_withdrawal_fault_controls() {
    let (mut good, exit) = run(h::old::QUALITY, "check", &["--configuration", "AsIs"]);
    assert_eq!(exit, 0);
    good["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["field"] != "quality.rule");
    let q = &mut good["quality"];
    q["checked"] = json!([
        "case_reachability",
        "expected_counts",
        "ratio_of_expected_counts",
        "denominator_disclosure",
        "outcomes_per_arrival"
    ]);
    q["unchecked"] = json!([
        "expectation_of_ratio",
        "cohort_any_probability",
        "metric_intervals",
        "metric_constraints",
        "numerator_subset_of_denominator",
        "business_value",
        "rework_loss",
        "deadline_attainment"
    ]);
    q["properties"][0]["expected_count"] = h::exact("1");
    q["properties"][0]["cases"] =
        json!([{"case_id":1,"arrival":0,"min":"1","max":"1","reason":null}]);
    q["cohort"][0]["value"] = h::exact("1");
    for row in q["outcomes_per_arrival"].as_array_mut().unwrap() {
        let n = if row["end"] == "done" { "1" } else { "0" };
        row["expected_count"] = h::exact(n);
        row["rate"] = h::exact(n);
    }
    fixture_match::compare_report(&json!({}), &good).unwrap();
    let mut row_left = good.clone();
    row_left["assumptions"]
        .as_array_mut()
        .unwrap()
        .push(h::old::quality_rule_assumption());
    assert!(fixture_match::compare_report(&json!({}), &row_left).is_err());
    let mut null_left = good;
    // Old aborted/null table is independently well formed, but a successful Rule
    // execution cannot use it. No obsolete row is present in this negative.
    let q = &mut null_left["quality"];
    let checked = q["checked"].as_array().unwrap().clone();
    q["unchecked"].as_array_mut().unwrap().extend(checked);
    q["checked"] = json!([]);
    q["properties"][0]["expected_count"] = h::absent("aborted", false);
    q["properties"][0]["cases"] =
        json!([{"case_id":1,"arrival":0,"min":null,"max":null,"reason":"aborted"}]);
    q["cohort"][0]["value"] = h::absent("aborted", false);
    for row in q["outcomes_per_arrival"].as_array_mut().unwrap() {
        row["expected_count"] = h::absent("aborted", false);
        row["rate"] = h::absent("aborted", false);
    }
    serde_json::from_value::<relca_core::quality::Quality>(q.clone()).unwrap();
    assert!(fixture_match::compare_report(&json!({}), &null_left).is_err());
    // Hand wire control: invalid plus an E- error is a real semantic refusal.
    let mut invalid = null_left;
    invalid["verdict"] = json!("invalid");
    invalid["diagnostics"].as_array_mut().unwrap().push(json!({
        "code":"E-CON-0", "severity":"error", "message":"hand invalid refusal", "span":null
    }));
    fixture_match::compare_report(&json!({}), &invalid).unwrap();
    invalid["verdict"] = json!("satisfied");
    assert!(fixture_match::compare_report(&json!({}), &invalid).is_err());
}
#[test]
fn matcher_preserves_true_semantic_refusal_with_reasoned_null_quality() {
    let s = h::old::replace(
        h::old::QUALITY,
        "type T = enum { ok }",
        "type T = enum { ok, bad }",
    );
    let s = h::old::replace(&s, "result nondet { ok }", "result nondet { ok, bad }");
    let (mut v, exit) = run(&s, "simulate", &["--configuration", "AsIs"]);
    assert_eq!(exit, 4);
    assert_eq!(v["quality"]["cohort"][0]["value"]["reason"], "aborted");
    v["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|a| a["field"] != "quality.rule");
    fixture_match::validate_simulation_wire(&v).unwrap();
}
