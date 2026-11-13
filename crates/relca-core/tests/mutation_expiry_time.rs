use relca_core::{
    Verdict,
    mutation::{self, Options},
    report,
};
const SOURCE: &str = include_str!("../../../fixtures/mut2-expiry/model.relca");
fn selected(src: &str) -> mutation::MutationReport {
    mutation::mutate_with_options(
        src,
        "bpr-default",
        None,
        &Options::parse(None, Some("authority_expired")).unwrap(),
    )
}

#[test]
fn all_zero_duration_at_time_zero_has_no_observable_expiry_mutant() {
    let src = SOURCE.replace("duration fixed 1min", "duration fixed 0min");
    let r = selected(&src);
    assert_eq!(r.original_verdict, Some(Verdict::Satisfied));
    assert_eq!(r.mutations[1].status, "not_applicable");
    assert_eq!(r.mutations[1].reason, Some("no_time_elapse"));
    assert_eq!(r.mutations[1].candidates, Some(0));
    assert!(r.mutations[1].mutants.is_empty());
    assert_eq!(r.run.candidates_total, Some(0));
    assert_eq!(r.run.completion, "complete");
    assert_eq!(r.verdict, Verdict::Vacuous);
    assert_eq!(r.run.reason, Some("no_selected_mutant"));
    // The underlying Authority semantics still include the expiry instant.
    assert_eq!(
        report::check(&src.replace("issued_by cfo", "issued_by cfo; expires 0min")).verdict,
        Verdict::Satisfied
    );
}

#[test]
fn positive_duration_or_late_arrival_remains_applicable() {
    let zero_late = SOURCE
        .replace("duration fixed 1min", "duration fixed 0min")
        .replace("  bounds", "  arrivals fixed [3min]\n  bounds");
    let distribution = SOURCE.replace("duration fixed 1min", "duration {1min:0.5,2min:0.5}");
    for src in [SOURCE, zero_late.as_str(), distribution.as_str()] {
        let r = selected(src);
        assert_eq!(
            r.mutations[1].status,
            "generated",
            "{:?}",
            r.original.as_ref().map(|r| &r.diagnostics)
        );
        assert_eq!(r.mutations[1].candidates, Some(1));
        assert_eq!(r.mutations[1].mutants[0].result, "detected");
    }
    let blocked = selected(&zero_late.replace("clock 10min", "clock 2min"));
    assert_eq!(blocked.run.completion, "blocked");
    assert_eq!(blocked.mutations[1].status, "blocked");
    assert_eq!(blocked.mutations[1].candidates, None);
    let zero_mass =
        selected(&SOURCE.replace("duration fixed 1min", "duration {0min:0.5,1min:0.5}"));
    assert_eq!(zero_mass.original_verdict, Some(Verdict::Unsupported));
    assert_eq!(zero_mass.mutations[1].status, "blocked");
}
