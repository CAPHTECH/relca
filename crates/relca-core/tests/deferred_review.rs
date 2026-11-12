#[path = "support/deferred_review_inputs.rs"]
mod h;
#[path = "support/deferred_metrics_inputs.rs"]
mod previous;
#[path = "support/deferred_second_inputs.rs"]
mod second;
use relca_core::{compare, engine, lower, reference, syntax};
use serde_json::{Value, json};

fn quality(name: &str, sim: bool, independent: bool, deferred: bool) -> Value {
    let s = h::source(name, deferred);
    let m =
        lower::lower_with_selection(&syntax::parse(&s).unwrap(), Some("Left"), h::scenario(name))
            .unwrap();
    if sim {
        if independent {
            let r = reference::simulate(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            json!(r.outcome.quality)
        } else {
            json!(engine::simulate(&m).quality)
        }
    } else if independent {
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        json!(r.outcome.quality)
    } else {
        json!(engine::explore(&m).quality)
    }
}
macro_rules! quality_case {
    ($test:ident, $name:literal, $sim:literal) => {
        #[test]
        fn $test() {
            let mut failures = Vec::new();
            for independent in [false, true] {
                h::assert_quality(
                    &quality($name, $sim, independent, false),
                    $name,
                    $sim,
                    false,
                );
                let q = quality($name, $sim, independent, true);
                if std::panic::catch_unwind(|| h::assert_quality(&q, $name, $sim, true)).is_err() {
                    failures.push(independent);
                }
            }
            assert!(
                failures.is_empty(),
                "failed engine=false/reference=true: {failures:?}"
            );
        }
    };
}
quality_case!(truncated_exhaustive, "truncated", false);
quality_case!(truncated_sample_parent, "truncated", true);
quality_case!(undetermined_exhaustive, "undetermined", false);
quality_case!(undetermined_sample_parent, "undetermined", true);
quality_case!(zero_denominator_exhaustive, "zero_denominator", false);
quality_case!(zero_denominator_sample_parent, "zero_denominator", true);
quality_case!(
    denominator_may_be_zero_exhaustive,
    "denominator_may_be_zero",
    false
);
quality_case!(beyond_reached_mass_quantile, "beyond_reached_mass", false);

macro_rules! compare_case {
    ($test:ident, $name:literal, $sim:literal) => {
        #[test]
        fn $test() {
            for deferred in [false, true] {
                let s = h::source($name, deferred);
                let v = json!(compare::compare_with_scenario(
                    &s,
                    &["Left", "Right"],
                    if $sim {
                        compare::Analysis::Simulation
                    } else {
                        compare::Analysis::Exhaustive
                    },
                    h::scenario($name)
                ));
                h::assert_comparison(&v, $name, $sim, deferred);
            }
        }
    };
}
compare_case!(truncated_compare, "truncated", false);
compare_case!(truncated_compare_sample_hard, "truncated", true);
compare_case!(horizon_compare, "undetermined", false);
compare_case!(horizon_compare_sample_hard, "undetermined", true);
compare_case!(zero_compare_sample, "zero_denominator", true);
compare_case!(maybe_zero_compare, "denominator_may_be_zero", false);
#[test]
fn deferred_hard_does_not_reconstruct_from_bounds_or_hits_even_with_prior_reason() {
    let mut failures = Vec::new();
    for name in ["truncated", "undetermined"] {
        for sim in [false, true] {
            let v = json!(compare::compare_with_scenario(
                &h::source(name, true),
                &["Left", "Right"],
                if sim {
                    compare::Analysis::Simulation
                } else {
                    compare::Analysis::Exhaustive
                },
                h::scenario(name)
            ));
            for row in v["evaluation"]["hard"].as_array().unwrap() {
                for cell in row["values"].as_array().unwrap() {
                    if !cell["interval"].is_null()
                        || cell["verdict"] != "inconclusive"
                        || !cell["basis"].is_null()
                    {
                        failures.push((name, sim, cell.clone()));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "hard numeric resurrection: {failures:?}"
    );
}
#[test]
fn existing_clock_fixture_with_hard_is_not_decidable_after_defer() {
    for deferred in [false, true] {
        let v = json!(compare::compare(
            &h::clock_fixture(deferred),
            &["Left", "Right"],
            compare::Analysis::Exhaustive
        ));
        let q = &v["plans"][0]["report"]["quality"];
        assert_eq!(q["cohort"][0]["value"]["reason"], "truncated");
        if deferred {
            assert!(q["cohort"][0]["bounds_with_undetermined"].is_null());
            assert_eq!(
                v["evaluation"]["hard"][0]["values"][0]["verdict"],
                "inconclusive"
            );
            assert!(v["evaluation"]["hard"][0]["values"][0]["interval"].is_null());
        } else {
            assert_eq!(
                q["cohort"][0]["bounds_with_undetermined"],
                json!({"lower":"1/75","upper":"26/75"})
            );
            assert_eq!(
                v["evaluation"]["hard"][0]["values"][0]["verdict"],
                "satisfied"
            );
        }
    }
}
macro_rules! dependency_case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            for deferred in [false, true] {
                let s = h::dependency_source($name, deferred);
                let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
                let e = engine::explore(&m);
                let r = reference::explore(&m);
                assert_eq!(r.stop, reference::StopReason::Complete);
                let mut failures = Vec::new();
                for (side, metrics) in [
                    ("engine", json!(e.metrics)),
                    ("reference", json!(r.outcome.metrics)),
                ] {
                    if std::panic::catch_unwind(|| h::assert_dependency(&metrics, $name, deferred))
                        .is_err()
                    {
                        failures.push(side);
                    }
                    if deferred
                        && std::panic::catch_unwind(|| second::assert_all_quantities(&metrics))
                            .is_err()
                    {
                        failures.push(side);
                    }
                }
                assert!(failures.is_empty(), "{failures:?}");
                assert_eq!(e.metrics, r.outcome.metrics);
            }
        }
    };
}
dependency_case!(contest_route_only_dependency, "contest");
dependency_case!(shared_resource_all_time_quantities, "shared_resource");
dependency_case!(shared_person_all_time_quantities, "shared_person");

#[test]
fn materialized_review_input_table_is_closed() {
    let mut cases = Vec::new();
    for (name, src) in previous::all_sources() {
        cases.push(json!({"name":format!("previous_{name}"),"source":src,"plans":name.starts_with("compare_").then_some("Included,Unrelated"),"scenario":null}));
    }
    for deferred in [false, true] {
        for name in h::QUALITY_CASES {
            cases.push(json!({"name":format!("{name}_{deferred}"),"source":h::source(name,deferred),"plans":"Left,Right","scenario":h::scenario(name)}));
        }
        cases.push(json!({"name":format!("clock_fixture_{deferred}"),"source":h::clock_fixture(deferred),"plans":"Left,Right","scenario":null}));
        for name in h::DEPENDENCY_CASES {
            cases.push(json!({"name":format!("{name}_{deferred}"),"source":h::dependency_source(name,deferred),"plans":null,"scenario":null}));
        }
    }
    assert_eq!(cases.len(), 37);
    assert_eq!(
        cases
            .iter()
            .map(|c| c["source"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        36
    );
    for case in &cases {
        let src = case["source"].as_str().unwrap();
        let ast = syntax::parse(src).unwrap();
        let selection = case["plans"].as_str().map(|s| s.split(',').next().unwrap());
        let result = lower::lower_with_selection(&ast, selection, case["scenario"].as_str());
        if case["name"] == "previous_static_cut_error" {
            assert!(result.is_err());
        } else {
            assert!(result.is_ok(), "{}: {:?}", case["name"], result.err());
        }
    }
}

#[test]
fn no_defer_review_controls_match_hand_values_in_both_collectors() {
    for name in h::QUALITY_CASES {
        for sim in [false, true] {
            if sim && ["denominator_may_be_zero", "beyond_reached_mass"].contains(&name) {
                continue;
            }
            for independent in [false, true] {
                h::assert_quality(&quality(name, sim, independent, false), name, sim, false);
            }
        }
    }
}
