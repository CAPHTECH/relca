//! Link under compare.rs with cfg(test), path ../tests/support/evaluation_review_private_compare.rs.
//! Acceptance owns this file; implementation owns the one-line linkage.
use super::*;
use serde_json::json;

// Reuse the existing cfg(test) compare seam without editing implementation src.
#[path = "evaluation_horizon_private_compare.rs"]
mod horizon_inventory;

#[test]
fn no_defer_cohort_reason_alone_not_configured_keeps_hard_inconclusive() {
    let src = include_str!("../../../../fixtures/eval-sim-ratio/model.relca");
    let result = compare_using(&["Good", "Bad"], Analysis::Simulation, |name, _| {
        let (mut report, metadata) = report::simulate_selected_metadata(src, name);
        assert!(
            metadata
                .as_ref()
                .unwrap()
                .model
                .deferred_metrics()
                .is_none()
        );
        for cohort in &mut report.quality.as_mut().unwrap().cohort {
            // Only the cohort estimate is unavailable; retain structural trials
            // and all other wire fields. This was already supported before K2.
            cohort["value"]["reason"] = json!("not_configured");
            cohort["value"]["point"] = serde_json::Value::Null;
        }
        let _: crate::quality::Quality =
            serde_json::from_value(json!(report.quality.as_ref().unwrap()))
                .expect("hand-edited input remains a valid quality wire");
        (SelectedReport::Simulation(report), metadata)
    });
    let wire = json!(result);
    for hard in wire["evaluation"]["hard"]
        .as_array()
        .expect("must evaluate")
    {
        for cell in hard["values"].as_array().unwrap() {
            assert_eq!(cell["verdict"], "inconclusive");
            assert_eq!(cell["reason"], "not_configured");
            assert!(cell["interval"].is_null());
            assert!(cell["basis"].is_null());
            assert!(cell["point"].is_null());
        }
    }
}

#[test]
fn evaluate_error_clears_checked_claims_for_all_discarded_tables() {
    let src = include_str!("../../../../fixtures/eval-sim-ratio/model.relca");
    let run = |bad: bool| {
        compare_using(&["Good", "Bad"], Analysis::Simulation, |name, _| {
            let (r, mut selected) = report::simulate_selected_metadata(src, name);
            if bad && name == "Bad" {
                // Reports stay well-formed: only selected IR confidence disagrees.
                selected
                    .as_mut()
                    .unwrap()
                    .model
                    .simulation
                    .as_mut()
                    .unwrap()
                    .confidence = crate::probability::Rational("3/4".parse().unwrap());
            }
            (SelectedReport::Simulation(r), selected)
        })
    };
    let normal = run(false);
    assert!(
        normal.evaluation.is_some(),
        "positive control reaches evaluation"
    );
    for word in ["metric_tables", "property_tables", "terminal_tables"] {
        assert!(
            normal.checked.iter().any(|s| s == word),
            "normal computes {word}"
        );
    }
    let bad = run(true);
    assert_eq!(bad.verdict, Verdict::Unsupported);
    assert_eq!(bad.verdict.exit_code(), 4);
    assert!(
        bad.diagnostics
            .iter()
            .any(|d| d.code == diag::U_SEM_0 && d.message == "simulation clauses differ"),
        "must reach evaluate, not fail projection: {:?}",
        bad.diagnostics
    );
    assert!(bad.evaluation.is_none());
    assert_eq!(json!(bad.tables), json!(Tables::default()));
    for word in [
        "metric_tables",
        "property_tables",
        "terminal_tables",
        "hard_filter",
        "admissibility",
        "hard_cross_plan_bonferroni",
    ] {
        assert!(
            !bad.checked.iter().any(|s| s == word),
            "discarded table/evaluation is not checked: {word}"
        );
    }
}
