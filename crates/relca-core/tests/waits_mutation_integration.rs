//! Test-first C6/A7 crossings; arithmetic precedes execution in the adjacent md.
#[path = "support/waits_mutation_inputs.rs"]
mod i;
use relca_core::{engine, lower, mutation, reference, report, syntax};
use serde_json::{Value, json};

fn mutate(s: &str) -> Value {
    json!(mutation::mutate_set_with_scenario(
        s,
        "bpr-default",
        Some("Day")
    ))
}
fn independent(s: &str, ref_side: bool) -> Value {
    let m = lower::lower_with_selection(&syntax::parse(s).unwrap(), None, Some("Day")).unwrap();
    let outcome = if ref_side {
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "zero exclusions");
        r.outcome
    } else {
        engine::explore(&m)
    };
    assert!(!outcome.truncated);
    json!({"metrics":outcome.metrics,"quality":outcome.quality})
}

#[test]
fn hq_horizon12_mutation_selection_metadata_and_empty_six_species() {
    let s = i::hq();
    let v = mutate(&s);
    i::envelope_with_ends(&v, &mutate(&i::masked(&s)), &["done"]);
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
    for ref_side in [false, true] {
        let o = independent(&s, ref_side);
        i::cases(&o, ["5", "10"], ["5", "5"], ["0", "5"], "A", "5", "1/2");
        assert_eq!(o["metrics"], v["original"]["metrics"]);
    }
}
#[test]
fn missing_selector_gate_has_no_metrics_or_waiting_assumption() {
    let v = json!(mutation::mutate(&i::hq()));
    assert_eq!(v["verdict"], "invalid");
    assert_eq!(v["original"]["diagnostics"][0]["code"], "E-CONF-0");
    assert!(v["original"].get("metrics").is_none());
    assert!(
        !v["original"]["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "metrics.waiting")
    );
    for k in [0, 1, 2, 4, 8, 9] {
        assert_eq!(v["mutations"][k]["status"], "blocked");
    }
}
#[test]
fn summary_queue_arithmetic_is_independent_in_both_collectors() {
    let s = i::summary(12, 20, false);
    for ref_side in [false, true] {
        i::cases(
            &independent(&s, ref_side),
            ["4", "5"],
            ["4", "4"],
            ["0", "1"],
            "Extract",
            "1",
            "1/5",
        );
        i::cases(
            &independent(&i::manual_summary(&s), ref_side),
            ["6", "7"],
            ["5", "5"],
            ["1", "2"],
            "Extract",
            "3",
            "3/7",
        );
    }
}
#[test]
fn summary_held_lineage_not_changed_waiting_is_detection() {
    let s = i::summary(12, 20, false);
    let v = mutate(&s);
    i::envelope(&v, &mutate(&i::masked(&s)));
    let m = i::only(&v, 4);
    i::held(m);
    i::cases(
        &m["report"],
        ["6", "7"],
        ["5", "5"],
        ["1", "2"],
        "Extract",
        "3",
        "3/7",
    );
    assert!(m["other_changes"].as_array().unwrap().iter().any(|c| {
        c["check"] == "metrics"
            && c["key"]
                .as_str()
                .unwrap()
                .contains("queue/wait_total/stats/min")
            && c["before"] == "1"
            && c["after"] == "3"
            && c["direction"] == "changed"
    }));
    assert_eq!(v["run"]["completion"], "complete");
}
#[test]
fn all_six_species_relower_and_transport_waiting_without_using_it_as_assignment() {
    let s = i::summary(12, 20, true);
    let v = mutate(&s);
    i::envelope(&v, &mutate(&i::masked(&s)));
    for (k, n) in [(0, 2), (1, 1), (2, 1), (4, 1), (8, 1), (9, 2)] {
        assert_eq!(v["mutations"][k]["status"], "generated");
        assert_eq!(v["mutations"][k]["mutants"].as_array().unwrap().len(), n);
    }
    i::held(i::only(&v, 4));
    assert_eq!(i::only(&v, 1)["result"], "detected");
    assert_eq!(v["run"]["completion"], "complete");
}
#[test]
fn summary_horizon_is_incomplete_not_detection_and_keeps_observed_waiting_lower() {
    let s = i::summary(6, 20, false);
    let v = mutate(&s);
    i::envelope(&v, &mutate(&i::masked(&s)));
    let m = i::only(&v, 4);
    assert_eq!(m["result"], "inconclusive");
    assert_eq!(m["reason"], "assigned_check_unavailable");
    assert_eq!(m["assigned_changes"], json!([]));
    assert_eq!(m["report"]["bounds"]["truncated"], false);
    assert_eq!(m["report"]["metrics"]["status"], "complete");
    assert_eq!(
        m["report"]["metrics"]["cases"][1]["dwell"]["bound"],
        "lower"
    );
    assert_eq!(v["run"]["completion"], "inconclusive");
    assert_eq!(v["run"]["reason"], "mutant_incomplete");
    for ref_side in [false, true] {
        let o = independent(&i::manual_summary(&s), ref_side);
        assert_eq!(o["metrics"], m["report"]["metrics"]);
    }
}
#[test]
fn summary_computational_cut_invalidates_waiting_instead_of_lower_bound() {
    let s = i::summary(12, 6, false);
    let v = mutate(&s);
    i::envelope(&v, &mutate(&i::masked(&s)));
    let m = i::only(&v, 4);
    assert_eq!(m["result"], "inconclusive");
    assert_eq!(m["reason"], "truncated");
    assert_eq!(m["report"]["metrics"]["status"], "truncated");
    assert_eq!(
        m["report"]["metrics"]["overall"]["queue_wait_observed"],
        json!({"min":null,"max":null,"reason":"truncated"})
    );
    assert!(
        m["report"]["metrics"]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c.get("dwell").is_none())
    );
    assert_eq!(v["run"]["completion"], "inconclusive");
}
#[test]
fn changed_intentional_wait_and_quality_never_detect_unassigned_guard_mutants() {
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
                .is_none(),
            "the sole lexical Wait was removed"
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
        assert_eq!(m["assigned_changes"], json!([]));
        assert!(
            m["other_changes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["check"] == "metrics"
                    && c["key"].as_str().unwrap().contains("intentional_wait"))
        );
    }
    for ref_side in [false, true] {
        assert_eq!(
            independent(&s, ref_side)["metrics"],
            v["original"]["metrics"]
        );
    }
}
#[test]
fn legacy_guard_unfinished_rows_use_horizon_classification_even_with_waits() {
    // Root remains decidable (the original rejects at2); mutant needs4 > horizon3.
    let s = i::waiting_only()
        .replace("horizon 8min", "horizon 3min")
        .replace("wait 2min; end rejected", "wait 0min; end rejected");
    let v = mutate(&s);
    for k in [2, 8] {
        let m = i::only(&v, k);
        assert_eq!(m["result"], "inconclusive");
        assert_eq!(m["reason"], "assigned_check_unavailable");
        assert_eq!(m["report"]["bounds"]["truncated"], false);
        assert_eq!(m["report"]["metrics"]["status"], "complete");
    }
    assert_eq!(v["run"]["reason"], "mutant_incomplete");
}
#[test]
fn comparison_projection_accepts_the_relowered_summary_shape_but_omits_waiting_tables() {
    for s in [
        i::summary(12, 20, false),
        i::manual_summary(&i::summary(12, 20, false)),
    ] {
        let s = i::configurations(&s);
        let v = json!(relca_core::compare::compare_with_scenario(
            &s,
            &["Left", "Right"],
            relca_core::compare::Analysis::Exhaustive,
            Some("Day")
        ));
        let c = json!(relca_core::compare::compare_with_scenario(
            &i::masked(&s),
            &["Left", "Right"],
            relca_core::compare::Analysis::Exhaustive,
            Some("Day")
        ));
        assert_eq!(v["verdict"], "satisfied");
        assert_eq!(v["tables"], c["tables"]);
        assert_eq!(
            v["unchecked"].as_array().unwrap().last(),
            Some(&json!("waiting_tables"))
        );
        let d = json!(relca_core::diff::diff_with_scenario(
            &s,
            "Left",
            "Right",
            Some("Day")
        ));
        assert_eq!(d["verdict"], "satisfied");
        assert!(
            d["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("waiting_tables"))
        );
    }
}
#[test]
fn simulate_refusal_has_quality_but_no_metrics_or_waiting_explanation() {
    let s = i::summary(12, 20, false);
    let v = json!(report::simulate_with_selection(&s, None, Some("Day")));
    assert_eq!(v["verdict"], "invalid");
    assert!(v.get("metrics").is_none());
    let fields: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["field"].as_str().unwrap())
        .collect();
    assert_eq!(
        fields,
        vec![
            "quality.definition",
            "quality.denominator",
            "quality.form",
            "quality.estimation"
        ]
    );
}

#[test]
fn restricted_rule_expiry_mutant_has_no_metrics_or_waiting_explanation() {
    let v = json!(mutation::mutate(&i::rule_refusal()));
    assert_eq!(v["original_verdict"], "satisfied");
    i::order(&v["original"]);
    let m = i::only(&v, 1);
    assert_eq!(m["result"], "invalid_mutant");
    assert_eq!(m["reason"], "static_unsupported");
    assert_eq!(m["assigned_changes"], json!([]));
    i::no_metrics_no_waiting(&m["report"]);
    assert!(
        m["report"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| { d["message"] == "restricted rule-change Authority is unsupported" })
    );
    assert_eq!(v["run"]["completion"], "inconclusive");
    assert_eq!(v["run"]["reason"], "mutant_incomplete");
    i::order(&i::only(&v, 4)["report"]);
}
