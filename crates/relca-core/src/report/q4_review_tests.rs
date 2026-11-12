#[path = "../../tests/support/q4_review.rs"]
mod input;

#[test]
fn scenario_distribution_metadata_is_a_declaration_envelope_in_both_row_orders() {
    for (last, reverse) in [(4, false), (4, true), (9, false)] {
        let report = crate::report::simulate_with_selection(
            &input::scenario_arrivals(last, reverse),
            None,
            Some("Day"),
        );
        assert_eq!(report.verdict, crate::Verdict::Satisfied);
        let scenario = report.scenario.unwrap();
        assert_eq!(scenario["arrivals"]["form"], "distribution");
        assert_eq!(scenario["arrivals"]["from"], 0);
        assert_eq!(scenario["arrivals"]["until"], last);
        assert_eq!(scenario["arrivals"]["count"], 2);
        assert_eq!(scenario["drain"]["from"], last);
    }
}
