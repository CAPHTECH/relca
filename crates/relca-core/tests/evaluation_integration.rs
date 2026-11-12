#[path = "support/evaluation_integration_acceptance.rs"]
mod acceptance;
use acceptance::{Op, i, q};
use relca_core::{compare, report};
use relca_core::{engine, lower, reference, syntax};
use serde_json::{Value, json};

fn run(src: &str, op: Op<'_>, day: bool) -> Value {
    let scenario = day.then_some("Day");
    match op {
        Op::Compare(plans, sampled) => json!(compare::compare_with_scenario(
            src,
            plans,
            if sampled {
                compare::Analysis::Simulation
            } else {
                compare::Analysis::Exhaustive
            },
            scenario
        )),
        Op::Single(config, true) => {
            json!(report::simulate_with_selection(src, Some(config), scenario))
        }
        Op::Single(config, false) => {
            json!(report::check_with_selection(src, Some(config), scenario))
        }
    }
}

fn model(s: &str, config: &str) -> relca_core::ir::Model {
    lower::lower_with_selection(&syntax::parse(s).unwrap(), Some(config), Some("Day")).unwrap()
}

fn independent_quality(s: &str, config: &str, sampled: bool, reference_side: bool) -> Value {
    let m = model(s, config);
    if reference_side {
        if sampled {
            let r = reference::simulate(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            json!(r.outcome.quality.unwrap())
        } else {
            let r = reference::explore(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            json!(r.outcome.quality.unwrap())
        }
    } else if sampled {
        json!(engine::simulate(&m).quality.unwrap())
    } else {
        json!(engine::explore(&m).quality.unwrap())
    }
}

#[test]
fn independent_reference_and_engine_horizon_endpoints_precede_evaluation() {
    for r in [false, true] {
        for h in [14, 30] {
            let s = i::horizon(h, "0.3", false);
            for config in ["Check", "Fast"] {
                let quality = independent_quality(&s, config, false, r);
                let rate = i::row(&quality, "cohort", "WrongPerArrival");
                if h == 14 {
                    q::unknown(&quality, "WrongPerArrival", "undetermined", "1/75", "26/75");
                } else {
                    assert_eq!(rate["value"], i::exact("1/50"));
                }
                // Third case is unknown only at the business cut. Its unknown
                // mass is not transformed into the deterministic false case.
                let c = &i::row(&quality, "properties", "Wrong")["cases"][2];
                assert_eq!(
                    c["reason"],
                    if h == 14 {
                        json!("undetermined")
                    } else {
                        Value::Null
                    }
                );
            }
        }
    }
}

#[test]
fn independent_reference_and_engine_simulation_observation_counts() {
    for r in [false, true] {
        for h in [1, 5] {
            let s = i::simulation(h, false);
            for config in ["Good", "Bad"] {
                let quality = independent_quality(&s, config, true, r);
                let c = &i::row(&quality, "properties", "Wrong")["cases"][0];
                assert_eq!(
                    (
                        c["hits"].as_u64(),
                        c["unknown"].as_u64(),
                        c["trials"].as_u64()
                    ),
                    (Some(0), Some(if h == 1 { 3 } else { 0 }), Some(3))
                );
            }
        }
    }
}

#[test]
fn computational_state_budget_is_not_business_horizon() {
    let s = i::horizon(30, "0.3", false);
    let m = model(&s, "Check");
    let o = engine::explore_with_budget(&m, 0);
    assert!(o.truncated);
    let v = json!(o.quality.unwrap());
    q::unknown(&v, "WrongPerArrival", "truncated", "0", "1");
}

#[test]
fn computational_live_task_cut_wins_in_both_independent_reducers() {
    let s = i::with_plans(q::MIXED, true);
    for r in [false, true] {
        let quality = independent_quality(&s, "Left", false, r);
        q::unknown(&quality, "Rate", "truncated", "0", "1");
    }
}
