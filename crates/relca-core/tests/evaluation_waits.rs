#[path = "support/evaluation_waits_acceptance.rs"]
mod acceptance;
use acceptance::Op;
use relca_core::{compare, engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn run(s: &str, op: Op<'_>, day: bool) -> Value {
    let scenario = day.then_some("Day");
    match op {
        Op::Compare(plans, sample) => json!(compare::compare_with_scenario(
            s,
            plans,
            if sample {
                compare::Analysis::Simulation
            } else {
                compare::Analysis::Exhaustive
            },
            scenario
        )),
        Op::Single(config, true) => {
            json!(report::simulate_with_selection(s, Some(config), scenario))
        }
        Op::Single(config, false) => json!(report::check_with_selection(s, Some(config), scenario)),
    }
}

#[test]
fn both_collectors_match_the_hand_queue_and_work_before_the_comparator() {
    for config in ["Left", "Right"] {
        let model = lower::lower_with_selection(
            &syntax::parse(acceptance::W).unwrap(),
            Some(config),
            Some("Day"),
        )
        .unwrap();
        for ref_side in [false, true] {
            let outcome = if ref_side {
                let r = reference::explore(&model);
                assert_eq!(
                    r.stop,
                    reference::StopReason::Complete,
                    "zero reference exclusions"
                );
                r.outcome
            } else {
                engine::explore(&model)
            };
            assert!(!outcome.truncated);
            acceptance::hand_metrics(&json!({"metrics":outcome.metrics}), false, false);
            assert_eq!(
                json!(outcome.quality.unwrap())["cohort"][0]["value"],
                acceptance::quality_table::exact("1")
            );
        }
    }
}
