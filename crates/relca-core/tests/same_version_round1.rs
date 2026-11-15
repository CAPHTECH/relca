use relca_core::{Verdict, engine, ir, lower, reference, report, syntax};

const MIXED: &str = include_str!("same_version_round1/mixed-cases.relca");
const ATOMIC: &str = include_str!("same_version_round1/atomic-rejected-check.relca");
const VERSIONED: &str = include_str!("same_version_round1/version-check-rejected-check.relca");
const CLAUSE: &str = "same_version Checker.check(d), Approver.approve(d)";
const BODY: &str = "    d = Writer.draft()\n    s = Checker.seed(case.id)\n    k = Checker.check(case.id,d)\n    p = Approver.approve(d)\n    end done";

fn check_both(source: &str, want: Verdict) {
    let plain = source
        .lines()
        .filter(|s| !s.trim_start().starts_with("contract V {"))
        .collect::<Vec<_>>()
        .join("\n");
    let old = report::check(&plain);
    assert_eq!(
        old.verdict,
        Verdict::Satisfied,
        "legal old substrate: {:?}",
        old.diagnostics
    );
    let m = lower::lower(&syntax::parse(source).unwrap()).unwrap();
    let actual = engine::explore(&m);
    let independent = reference::explore(&m);
    assert_eq!(independent.stop, reference::StopReason::Complete);
    for out in [&actual, &independent.outcome] {
        assert!(!out.truncated);
        let row = out
            .properties
            .iter()
            .find(|p| matches!(p.property, ir::Property::Contract(_)))
            .unwrap();
        assert_eq!(row.verdict, want, "{:?}", out.diagnostics);
    }
    assert_eq!(actual.properties, independent.outcome.properties);
}

#[test]
fn staggered_cases_keep_satisfied_and_violated_histories_separate() {
    // Case 1: draft 0-1, check 1-2, approve 2-3, mark 3-4.
    // Case 2 arrives at10 after the mark, skips check, approves at11-12.
    // A shared last check would incorrectly satisfy the second case as well.
    let m = lower::lower(&syntax::parse(MIXED).unwrap()).unwrap();
    let actual = engine::explore(&m);
    let independent = reference::explore(&m);
    assert_eq!(independent.stop, reference::StopReason::Complete);
    for out in [&actual, &independent.outcome] {
        let row = out
            .properties
            .iter()
            .find(|p| matches!(p.property, ir::Property::Contract(_)))
            .unwrap();
        assert_eq!(row.verdict, Verdict::Violated);
        assert!(!out.truncated);
        let cases = row.cases.as_ref().unwrap();
        assert_eq!(
            cases
                .iter()
                .map(|c| (c.case_id.0 + 1, c.arrival, c.verdict))
                .collect::<Vec<_>>(),
            vec![(1, 0, Verdict::Satisfied), (2, 10, Verdict::Violated)]
        );
    }
    assert_eq!(actual.properties, independent.outcome.properties);
}

#[test]
fn an_atomic_rejection_is_not_an_examination() {
    // draft 0-1, seed 1-2, rejected duplicate check 2-3, approve 3-4.
    check_both(ATOMIC, Verdict::Violated);
}

fn rejected_recheck() -> String {
    ATOMIC.replace(BODY, "    d = Writer.draft(); f = Writer.draft();\n    k = Checker.check(case.id,d); k2 = Checker.check(case.id,f);\n    p = Approver.approve(d); end done")
}

#[test]
fn a_rejected_recheck_does_not_replace_the_last_normal_examination() {
    // d 0-1, f 1-2, normal check(d) 2-3, rejected check(f) 3-4,
    // approve(d) 4-5 still compares with the normal check(d).
    check_both(&rejected_recheck(), Verdict::Satisfied);
}

#[test]
fn a_rejected_use_does_not_activate_the_contract() {
    let source = ATOMIC.replace(BODY, "    d = Writer.draft(); s = Checker.seed(case.id);\n    p = Approver.approve(d); k = Checker.check(case.id,d); end done")
        .replace(CLAUSE, "same_version Approver.approve(d), Checker.check(d)");
    check_both(&source, Verdict::Vacuous);
}

#[test]
fn an_ordinary_result_named_duplicate_is_still_a_normal_examination() {
    let atomic = "capability check(key:CaseId, d:Doc) -> Ack {\n      commits; writes keys; duration fixed 1min\n      atomic { if key in keys { result duplicate } else { keys += key; result accepted } }\n      cancel forbidden_after_start\n    }";
    assert_eq!(ATOMIC.matches(atomic).count(), 1);
    let source = ATOMIC.replace(atomic, "capability check(key:CaseId, d:Doc) -> Ack { duration fixed 1min; result nondet { duplicate } }");
    check_both(&source, Verdict::Satisfied);
}

#[test]
fn a_version_check_rejection_is_not_an_examination() {
    // Two cases can start their checks before either writes holder. The first
    // write makes the other return conflict; that case has no normal check.
    check_both(VERSIONED, Verdict::Violated);
}

#[test]
fn sampled_atomic_rejection_and_recheck_match_the_independent_interpreter() {
    for (source, broken) in [(ATOMIC.to_owned(), true), (rejected_recheck(), false)] {
        let close = source.rfind('}').unwrap();
        let source = format!(
            "{} simulation replications 2 seed 7 confidence 0.5 }}",
            &source[..close]
        );
        let m = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
        let actual = engine::simulate(&m);
        let independent = reference::simulate(&m);
        assert_eq!(independent.stop, reference::StopReason::Complete);
        assert_eq!(actual.completed_replications, 2);
        assert_eq!(actual.replications, independent.outcome.replications);
        assert!(actual.replications.iter().all(|r| {
            r.cases
                .iter()
                .all(|c| c.violations.contains(&true) == broken)
        }));
    }
}
