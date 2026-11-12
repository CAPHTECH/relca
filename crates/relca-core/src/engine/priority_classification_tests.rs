//! Supplemental Q5 classification probes, separate from the fixed acceptance.
use crate::{engine, lower, reference, report, syntax};

const FINISH: &str = r#"model FinishTie version "0.2" {
    type R = enum { committed, conflict } type H = enum { free, taken }
    latent case { coin: bool ~ { true: 0.5, false: 0.5 } }
    actor A { capacity 2; state holder: H = free
        capability claim() -> R { duration fixed 2min; result nondet { committed };
            version_check [holder] else conflict resolve priority descending; commits;
            writes holder; finish { holder = taken } } }
    process Flow { r = A.claim(); if r == committed { end won } else { end lost } }
    cut C { include [A] } bounds { cases 2; live_tasks 2; clock 20min }
    arrivals fixed [0min, 0min] case_priority fixed [5, 5]
    policy observable priority descending tie fifo tie_by case_id
    verify safety no_resource_overcommit
    verify probability reached end.won >= 0
}"#;

const EXTERNAL: &str = r#"model ExternalTie version "0.2" {
    type R = enum { ok }
    latent case { coin: bool ~ { true: 0.5, false: 0.5 } }
    actor A { capacity 1; capability run() -> R { duration fixed 4min; result nondet { ok } } }
    process Flow { r = A.run(); end done }
    cut C { include [A] } bounds { cases 3; live_tasks 2; clock 20min }
    arrivals fixed [0min, 1min, 2min] case_priority fixed [0, 5, 5]
    policy observable priority descending tie fifo tie_by case_id
    concurrent cases 1 on_overflow wait_external priority descending
    verify safety no_resource_overcommit
    verify probability reached end.done within 10min >= 0
}"#;

fn classification(source: &str, unresolved: bool) {
    let m = lower::lower(&syntax::parse(source).unwrap()).unwrap();
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for (which, out) in [("engine", &e), ("reference", &r.outcome)] {
        assert_eq!(
            out.probability_policy_unresolved,
            Some(unresolved),
            "{which}: unresolved priority scope must not be labelled fixed-policy"
        );
    }
    if unresolved {
        let wire = report::check(source);
        let row = wire
            .assumptions
            .iter()
            .find(|a| a.field == "probability.scheduler")
            .unwrap();
        assert_eq!(row.value, "full-information");
    }
}

#[test]
fn priority_finish_tie_is_full_information_with_fixed_starts_and_initial_chance() {
    classification(FINISH, true);
    // An explicit CaseId tie resolves this choice, not the chance input.
    classification(
        &FINISH.replace(
            "resolve priority descending;",
            "resolve priority descending tie_by case_id;",
        ),
        false,
    );
    // An equal-rank future finish is not a currently unresolved due tie.
    classification(
        &FINISH.replace("arrivals fixed [0min, 0min]", "arrivals fixed [0min, 1min]"),
        false,
    );
}

#[test]
fn priority_external_tie_is_full_information_with_fixed_starts_and_initial_chance() {
    classification(EXTERNAL, true);
    classification(
        &EXTERNAL.replace(
            "wait_external priority descending",
            "wait_external priority descending tie fifo tie_by case_id",
        ),
        false,
    );
}
