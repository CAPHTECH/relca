//! Actual production validator, not fixture_match's test-only validator.
use crate::metrics::{Range, Scalar};
use crate::{comparison_projection, lower, report, syntax};

#[test]
fn review_any_numeric_join_is_rejected() {
    let src = include_str!("../../../../fixtures/wait-any/model.relca");
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    let mut point = report::check(src).metrics.unwrap();
    assert!(comparison_projection::validate_metrics(Some(&point), &m, false).is_ok());
    let joins = point.cases[0].joins.as_mut().unwrap();
    assert_eq!(joins[0].mode, crate::metrics::JoinMode::Any);
    for b in &joins[0].branches {
        assert_eq!(
            b.join_wait,
            Scalar::Range(Range {
                min: None,
                max: None,
                reason: Some(crate::metrics::Reason::JoinAny)
            })
        );
    }
    // any's drain at5 does not turn its early branch's stop3 into join wait.
    // Even numeric0 is forbidden: every any branch is null/join_any.
    joins[0].branches[0].join_wait = Scalar::Range(Range {
        min: Some(0.into()),
        max: Some(0.into()),
        reason: None,
    });
    assert!(
        comparison_projection::validate_metrics(Some(&point), &m, false).is_err(),
        "mode any must reject numeric join_wait, including zero"
    );
}

#[test]
fn review_actual_cutoff_requires_dwell_and_queue_lower() {
    let src = include_str!("../../../../fixtures/wait-horizon-queue/model.relca");
    let m = lower::lower_with_selection(&syntax::parse(src).unwrap(), None, Some("Day")).unwrap();
    let mut point = report::check_with_selection(src, None, Some("Day"))
        .metrics
        .unwrap();
    assert!(comparison_projection::validate_metrics(Some(&point), &m, false).is_ok());
    let raw = serde_json::to_value(&point).unwrap();
    for c in raw["cases"].as_array().unwrap() {
        assert_eq!(c["counts"]["cut_off"]["stats"]["min"], "1");
        assert_eq!(c["dwell"]["until"]["min"], "3");
        assert_eq!(c["dwell"]["bound"], "lower");
    }
    assert_eq!(
        raw["cases"][0]["dwell"]["breakdown"]["in_service"]["min"],
        "3"
    );
    assert_eq!(
        raw["cases"][1]["dwell"]["breakdown"]["queue"]["actor_capacity"]["min"],
        "3"
    );
    let q = &raw["actors"][0]["queue"];
    assert_eq!(q["wait_total"]["stats"]["min"], "3");
    assert_eq!(q["length_time_average"]["stats"]["min"], "1");
    assert_eq!(q["length_max"]["min"], "1");
    // Preserve the observed cutoff counts and everything else. Remove only
    // conditional presence; a horizon declaration alone cannot reconstruct it.
    for c in &mut point.cases {
        c.dwell = None;
    }
    let q = point.actors[0].queue.as_mut().unwrap();
    q.wait_total.bound = None;
    q.length_time_average.bound = None;
    assert!(
        comparison_projection::validate_metrics(Some(&point), &m, false).is_err(),
        "observed cut_off must require dwell and lower fields"
    );
}
