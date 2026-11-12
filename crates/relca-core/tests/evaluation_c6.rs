#[path = "support/evaluation_c6_acceptance.rs"]
mod acceptance;
fn run(src: &str, mutate: bool, day: bool) -> serde_json::Value {
    let scenario = day.then_some("Day");
    if mutate {
        serde_json::to_value(relca_core::mutation::mutate_set_with_scenario(
            src,
            "bpr-default",
            scenario,
        ))
        .unwrap()
    } else {
        serde_json::to_value(relca_core::report::check_with_selection(
            src, None, scenario,
        ))
        .unwrap()
    }
}

#[test]
fn manual_summary_edit_is_undetermined_or_truncated_in_both_independent_reducers() {
    for (h, c, reason) in [(4, 10, "undetermined"), (10, 4, "truncated"), (5, 10, "")] {
        let src = acceptance::summary(h, c, true)
            .replace(
                "e = Extract.make();",
                "e = Extract.make(); e_dup = Extract.make();",
            )
            .replace("a = Approver.approve(e)", "a = Approver.approve(e_dup)");
        let m = relca_core::lower::lower_with_selection(
            &relca_core::syntax::parse(&src).unwrap(),
            None,
            Some("Day"),
        )
        .unwrap();
        let reference = relca_core::reference::explore(&m);
        assert_eq!(reference.stop, relca_core::reference::StopReason::Complete);
        for o in [relca_core::engine::explore(&m), reference.outcome] {
            assert_eq!(o.truncated, c < h);
            let q = serde_json::to_value(o.quality.unwrap()).unwrap();
            let rate = &q["cohort"][0];
            assert_eq!(
                rate["value"]["reason"],
                if reason.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::json!(reason)
                }
            );
            if !reason.is_empty() {
                assert!(rate["value"]["lower"].is_null());
                assert_eq!(rate["bounds_with_undetermined"]["lower"], "0");
                assert_eq!(rate["bounds_with_undetermined"]["upper"], "1");
            } else {
                assert_eq!(rate["value"]["lower"], "0");
                assert_eq!(rate["value"]["upper"], "1");
            }
        }
    }
}
