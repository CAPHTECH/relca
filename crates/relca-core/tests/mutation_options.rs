//! Additional enumerator boundaries for ADR 0068. No captured output is an oracle.
use relca_core::{
    Verdict,
    mutation::{self, Options},
};
#[path = "support/response_loss_inputs.rs"]
mod response;

fn run(src: &str, limit: &str, kinds: &str) -> mutation::MutationReport {
    mutation::mutate_with_options(
        src,
        "bpr-default",
        None,
        &Options::parse(Some(limit), Some(kinds)).unwrap(),
    )
}

#[test]
fn guard_summary_and_authority_counts_are_lexical_not_executed_counts() {
    let src = include_str!("../../../fixtures/mut2-summary/model.relca");
    // Two reads for each of kinds 0 and 9; one removable terminal guard for
    // each of kinds 2 and 8; one second consumer of e for kind 4. 2+2+1+1+1=7.
    for limit in ["1", "7"] {
        let r = run(src, limit, "summary_reused");
        assert_eq!(r.run.candidates_total, Some(7));
        assert_eq!(r.mutations[4].candidates, Some(1));
        assert_eq!(r.mutations[4].mutants.len(), 1);
        assert_eq!(r.mutations[4].mutants[0].result, "detected");
        assert_eq!(r.verdict, Verdict::Satisfied);
        assert_eq!(r.run.completion, "complete");
    }
    let src = include_str!("../../../fixtures/mut2-expiry/model.relca");
    let r = run(src, "1", "authority_expired");
    assert_eq!(r.run.candidates_total, Some(1));
    assert_eq!(r.mutations[1].candidates, Some(1));
    assert_eq!(r.mutations[1].mutants[0].result, "detected");
    assert_eq!(r.verdict, Verdict::Satisfied);
}

#[test]
fn response_kinds_enumerate_excluded_candidates_without_spending_budget() {
    // One explicitly declared response-loss site makes one candidate per kind.
    let src = response::service("g", response::NONE);
    let r = run(&src, "1", "receipt_unknown");
    assert_eq!(r.mutations[3].status, "not_selected");
    assert_eq!(r.mutations[3].candidates, Some(1));
    assert!(r.mutations[3].mutants.is_empty());
    assert_eq!(r.mutations[6].candidates, Some(1));
    assert_eq!(r.mutations[6].mutants[0].result, "detected");
    assert_eq!(r.run.completion, "complete");
    assert!(!r.run.limit_reached);
    assert!(!r.checked.iter().any(|s| s == "command_resend"));
    assert!(r.checked.iter().any(|s| s == "receipt_unknown"));
    let r = run(&src, "1", "command_resent,receipt_unknown");
    // Atomic key deduplication keeps at_most_once satisfied on the resend;
    // no assigned worsening occurs (the existing comparison is unchanged).
    assert_eq!(r.mutations[3].mutants[0].result, "undetected");
    assert_eq!(r.mutations[6].mutants[0].result, "not_run");
    assert_eq!(r.mutations[6].candidates, Some(1));
    assert_eq!(r.verdict, Verdict::Inconclusive);
    assert_eq!(r.run.reason, Some("mutant_limit"));
    let id = format!("receipt_unknown:Flow:{}", src.find("ack =").unwrap());
    let selected = Options::parse(Some("1"), Some("command_resent,receipt_unknown"))
        .unwrap()
        .with_sites(Some(&id))
        .unwrap();
    let r = mutation::mutate_with_options(&src, "bpr-default", None, &selected);
    assert_eq!(r.mutations[3].mutants[0].result, "not_selected");
    assert_eq!(r.mutations[6].mutants[0].result, "detected");
    assert_eq!(r.verdict, Verdict::Satisfied);
    assert!(!r.run.limit_reached);
}

#[test]
fn denominator_counts_include_unselected_and_not_run_terminals() {
    let src = r#"model Denominator version "0.2" {
        type T = enum { a,b }
        actor A { capacity 1; capability choose() -> T { duration fixed 1min; result nondet { a,b } } }
        process P { v = A.choose(); if v == a { end yes } else { end no } }
        cut C { include [A] }
        bounds { cases 1; live_tasks 1; clock 5min }
        verify safety no_resource_overcommit
        property case.Done = reached(end.yes || end.no)
        metric cohort.Rate = expected(count(case.Done)) / arrivals
    }"#;
    // The denominator consists of two distinct lexical ends: remove yes or no.
    let r = run(src, "1", "old_promise_denominator_removed");
    assert_eq!(r.mutations[7].candidates, Some(2));
    assert_eq!(r.mutations[7].mutants.len(), 2);
    assert_eq!(r.mutations[7].mutants[0].result, "detected");
    assert_eq!(r.mutations[7].mutants[1].result, "not_run");
    assert_eq!(r.verdict, Verdict::Inconclusive);
    let r = run(src, "2", "old_promise_denominator_removed");
    assert_eq!(r.verdict, Verdict::Satisfied);
    assert_eq!(r.run.completion, "complete");
    let r = run(src, "1", "authority_expired");
    assert_eq!(r.mutations[7].candidates, Some(2));
    assert_eq!(r.mutations[7].status, "not_selected");
    assert!(r.mutations[7].mutants.is_empty());
    assert!(!r.checked.iter().any(|s| s == "quality_denominator"));
    assert_eq!(r.verdict, Verdict::Vacuous);
    assert_eq!(r.run.reason, Some("no_selected_mutant"));
    let selected = Options::parse(Some("1"), Some("old_promise_denominator_removed"))
        .unwrap()
        .with_sites(Some(
            "old_promise_denominator_removed:cohort.Rate:remove:no",
        ))
        .unwrap();
    let r = mutation::mutate_with_options(src, "bpr-default", None, &selected);
    assert_eq!(r.mutations[7].mutants[0].result, "not_selected");
    assert_eq!(r.mutations[7].mutants[1].result, "detected");
    assert_eq!(r.verdict, Verdict::Satisfied);
    assert!(!r.run.limit_reached);
}

#[test]
fn blocked_and_unreadable_runs_keep_the_declared_scope_and_unknown_counts() {
    let options = Options::parse(Some("17"), Some("evidence_missing")).unwrap();
    for r in [
        mutation::mutate_with_options("not a model", "bpr-default", None, &options),
        mutation::input_failure_with_options("missing", Some("Day"), &options),
    ] {
        assert_eq!(r.verdict, Verdict::Invalid);
        assert_eq!(r.run.mutant_limit, mutation::MutantLimit::Used(17));
        assert_eq!(r.run.candidates_total, None);
        assert_eq!(r.mutations[0].status, "blocked");
        assert!(r.mutations.iter().all(|k| k.candidates.is_none()));
        assert!(r.mutations[1..].iter().all(|k| k.status == "not_selected"));
        assert_eq!(r.ledger.execution["limits"]["mutant_limit"]["value"], 17);
    }
}

#[test]
fn ledger_rejects_a_different_mutant_limit_from_the_executed_run() {
    let src = include_str!("../../relca-cli/tests/support/mutation_limits.relca");
    let mut r = run(src, "1", "original_access_removed");
    let host = serde_json::to_value(&r).unwrap();
    r.ledger.validate(&host).unwrap();
    r.ledger.execution["limits"]["mutant_limit"]["value"] = serde_json::json!(128);
    assert!(r.ledger.validate(&host).is_err());
}

#[test]
fn site_census_does_not_leak_its_zero_budget_into_a_blocked_run() {
    let options = Options::default().with_sites(Some("unknown")).unwrap();
    let r = mutation::mutate_with_options("not a model", "bpr-default", None, &options);
    assert_eq!(r.run.mutant_limit, mutation::MutantLimit::Used(128));
    assert_eq!(r.run.candidates_total, None);
    assert_eq!(
        r.assumptions
            .iter()
            .find(|a| a.field == "mutation.limit")
            .unwrap()
            .value,
        "computational_128"
    );
}
