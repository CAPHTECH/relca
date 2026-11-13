//! Time-source audit controls; expected times follow the declarations below.
use relca_core::{
    Verdict,
    mutation::{self, Options},
};

const SOURCE: &str = include_str!("../../../fixtures/mut2-expiry/model.relca");

fn run(source: &str, scenario: Option<&str>) -> mutation::MutationReport {
    mutation::mutate_with_options(
        source,
        "bpr-default",
        scenario,
        &Options::parse(None, Some("authority_expired")).unwrap(),
    )
}

fn calendar_source(target: &str, opens: u64) -> String {
    let source = SOURCE.replace("duration fixed 1min", "duration fixed 0min");
    let intervals = if opens == 0 {
        "[0min, 11min) capacity 1".to_owned()
    } else {
        format!("[0min, {opens}min) capacity 0; [{opens}min, 11min) capacity 1")
    };
    let source = source.replace(
        "  people",
        &format!("  calendar Day through 11min on_close continue {{ {intervals} }}\n  people"),
    );
    match target {
        "actor" => source.replace(
            "actor Approver {",
            "actor Approver { capacity 1; calendar Day;",
        ),
        "resource" => source
            .replace(
                "  actor Approver",
                "  resource Desk capacity 1 calendar Day\n  actor Approver",
            )
            .replace("duration fixed 0min", "uses Desk: 1; duration fixed 0min"),
        "person" => source.replace(
            "  actor Approver",
            "  person_calendar a1 Day\n  actor Approver",
        ),
        _ => panic!("unknown calendar target"),
    }
}

fn completions(r: &mutation::MutationReport) -> Vec<u64> {
    let original = r.original.as_ref().unwrap();
    assert_eq!(
        r.original_verdict,
        Some(Verdict::Satisfied),
        "{:?}",
        original.diagnostics
    );
    assert_eq!(original.bounds.truncated, Some(false));
    original
        .case_outcomes
        .as_ref()
        .unwrap()
        .iter()
        .map(|c| {
            assert_eq!(c.terminals.len(), 1);
            let time = c.terminals[0].completed_at.unwrap();
            assert_eq!(time.min, time.max);
            time.max
        })
        .collect()
}

#[test]
fn calendars_are_conservative_time_sources_even_with_zero_time_completions() {
    for target in ["actor", "resource", "person"] {
        let r = run(&calendar_source(target, 0), None);
        assert_eq!(completions(&r), [0], "{target}");
        // A start-slot calendar is a separate time source, not a service duration.
        // Conservatively retain the mutant even if this run needs no calendar wait.
        assert_eq!(r.mutations[1].status, "generated", "{target}");
        assert_eq!(r.mutations[1].candidates, Some(1));
        assert_eq!(r.mutations[1].mutants.len(), 1);
        assert_eq!(r.mutations[1].mutants[0].result, "undetected");
        assert_eq!(r.verdict, Verdict::Violated);
    }
    let r = run(
        &SOURCE.replace("duration fixed 1min", "duration fixed 0min"),
        None,
    );
    assert_eq!(r.original_verdict, Some(Verdict::Satisfied));
    assert_eq!(r.mutations[1].reason, Some("no_time_elapse"));
    assert_eq!(r.mutations[1].candidates, Some(0));
}

#[test]
fn calendar_opening_advances_zero_duration_actor_resource_and_person_calls() {
    for target in ["actor", "resource", "person"] {
        let r = run(&calendar_source(target, 3), None);
        // Arrival 0 + calendar wait 3 + service 0 = absolute completion 3.
        assert_eq!(completions(&r), [3], "{target}");
        assert_eq!(r.mutations[1].candidates, Some(1));
        assert_eq!(r.mutations[1].mutants[0].result, "detected");
        assert_eq!(r.verdict, Verdict::Satisfied);
    }
}

#[test]
fn fixed_every_expands_arrivals_but_horizon_and_admission_are_not_timers() {
    for (until, cases, expected) in [(1, 1, vec![0]), (3, 2, vec![0, 2])] {
        let source = SOURCE
            .replace("duration fixed 1min", "duration fixed 0min")
            .replace("issued_by cfo", "issued_by cfo; expires 5min")
            .replace("contract Controls {", "contract Controls { response case.received -> one_of(end.done) by 6min after case.received otherwise unresolved;")
            .replace("  bounds", &format!(
                "  policy fifo tie_by case_id\n  queue capacity 1 on_overflow wait_external fifo\n  concurrent cases 1 on_overflow wait_external fifo\n  scenario Window {{ arrivals fixed_every 2min during [0min, {until}min); horizon 8min }}\n  bounds"))
            .replace("cases 1;", &format!("cases {cases};"));
        let r = run(&source, Some("Window"));
        assert_eq!(completions(&r), expected);
        let arrivals: Vec<_> = r
            .original
            .as_ref()
            .unwrap()
            .case_outcomes
            .as_ref()
            .unwrap()
            .iter()
            .map(|c| c.arrival)
            .collect();
        assert_eq!(arrivals, expected);
        if cases == 1 {
            // every=2, expires=5, response by=6, horizon=8 and clock=10 are not timers.
            assert_eq!(r.mutations[1].reason, Some("no_time_elapse"));
            assert_eq!(r.mutations[1].candidates, Some(0));
        } else {
            assert_eq!(r.mutations[1].candidates, Some(1));
            assert_eq!(r.mutations[1].mutants[0].result, "detected");
        }
    }
}
