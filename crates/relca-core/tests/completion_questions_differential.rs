#[path = "completion_questions.rs"]
mod acceptance;
use relca_core::{engine, lower, reference, syntax};

#[test]
fn twenty_four_complete_zero_exclusions_each_interpreter_first_matches_hand_oracle() {
    let mut complete = 0;
    for n in acceptance::NAMES {
        let m = lower::lower(&syntax::parse(&acceptance::source(n)).unwrap()).unwrap();
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{n}");
        let e = engine::explore(&m);
        acceptance::outcome_hand(n, &m, &r.outcome);
        acceptance::outcome_hand(n, &m, &e);
        let mut rr = r.outcome;
        assert!(rr.diagnostics.starts_with(&m.diagnostics));
        rr.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, rr, "all runtime fields {n}");
        complete += 1;
    }
    assert_eq!(complete, 24, "reference Complete24; exclusions0");
}
