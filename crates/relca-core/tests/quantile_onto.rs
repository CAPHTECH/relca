//! New hand expectations are recorded before this target's first run.
#[path = "support/quantile_onto_inputs.rs"]
mod h;
use relca_core::{compare, engine, lower, reference, syntax};
use serde_json::json;

fn waiting(route: &str, sim: bool) {
    let s = h::wait_source(route);
    // Input must lower; a parser/selection defect is not the intended refusal.
    lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    let v = h::i::run(&s, None, None, sim);
    h::assert_wait(&v, sim, route);
    h::assert_old(&v, &h::i::run(&h::i::h::no_quantiles(&s), None, None, sim));
}
#[test]
fn f35_check_old_waiting_quantile_scheduler_order() {
    waiting("normal", false);
}
#[test]
fn f35_simulate_old_waiting_quantile_scheduler_order() {
    waiting("normal", true);
}
#[test]
fn f35_unresolved_scheduler_refusal_order() {
    waiting("refusal", true);
}
#[test]
fn f35_late_result_abort_order() {
    waiting("late_abort", true);
}
#[test]
fn f35_probability_scheduler_is_observed_after_quantile() {
    let s = h::scheduler_source();
    let v = h::i::run(&s, None, None, false);
    h::assert_scheduler(&v);
}
#[test]
fn a7_missing_clause_has_no_metrics_or_waiting_but_retains_quantile() {
    waiting("missing_clause", true);
}
#[test]
fn a7_structural_refusal_has_no_metrics_or_waiting_but_retains_quantile() {
    waiting("structural", true);
}

fn collector(sim: bool, r: bool) {
    let s = h::wait_source("normal");
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    let v = if sim {
        if r {
            let x = reference::simulate(&m);
            assert_eq!(x.stop, reference::StopReason::Complete);
            json!({"quality":x.outcome.quality,"metrics":x.outcome.replications[0].metrics})
        } else {
            let x = engine::simulate(&m);
            json!({"quality":x.quality,"metrics":x.replications[0].metrics})
        }
    } else if r {
        let x = reference::explore(&m);
        assert_eq!(x.stop, reference::StopReason::Complete);
        json!({"quality":x.outcome.quality,"metrics":x.outcome.metrics})
    } else {
        let x = engine::explore(&m);
        json!({"quality":x.quality,"metrics":x.metrics})
    };
    assert_eq!(v["metrics"]["unchecked"], json!(h::WAIT_UNCHECKED));
    let q = &v["quality"]["quantiles"][0]["value"];
    assert_eq!(q[if sim { "point" } else { "lower" }], "3");
    let v = &v["metrics"];
    assert_eq!(v["cases"].as_array().unwrap().len(), 2);
    for (j, c) in v["cases"].as_array().unwrap().iter().enumerate() {
        let scalar = &c["terminals"][0]["breakdown"]["queue"]["actor_capacity"];
        assert_eq!(scalar["min"], if j == 0 { "0" } else { "3" });
    }
}
#[test]
fn a7_engine_exhaustive_twelve_unchecked_words_and_hand_waits() {
    collector(false, false);
}
#[test]
fn a7_reference_exhaustive_twelve_unchecked_words_and_hand_waits() {
    collector(false, true);
}
#[test]
fn a7_engine_sample_twelve_unchecked_words_and_hand_waits() {
    collector(true, false);
}
#[test]
fn a7_reference_sample_twelve_unchecked_words_and_hand_waits() {
    collector(true, true);
}

fn objective(name: &str) {
    let (s, plans) = h::compare_source(name);
    let ps: Vec<_> = plans.iter().map(String::as_str).collect();
    let v = json!(compare::compare(&s, &ps, compare::Analysis::Exhaustive));
    h::assert_objective(&v, name);
}
macro_rules! objectives {
    ($($test:ident => $name:literal),+ $(,)?) => {$(#[test] fn $test(){objective($name);})+};
}
objectives! {
    t08_quantile_objective_and_hard_admission => "qtl-objective",
    t08_unresolved_hard_keeps_objective_but_excludes_plan => "qtl-objective.broken-no-policy",
    t09_quantile_and_cost_frontier => "qtl-pareto",
    t09_single_fee_edit_tradeoff => "qtl-pareto.broken-pricier-parallel",
    t10_configured_price => "qtl-missing-rate",
    t10_missing_price_is_reasoned_null => "qtl-missing-rate.broken-missing-rate",
    t21_lower_bound_is_not_objective_value => "qtl-null-objective",
    t21_pass_only_restores_finite_dominance => "qtl-null-objective.broken-pass-only",
    t21_overlapping_interval_does_not_dominate => "qtl-interval-objective",
    t21_fifo_restores_certain_dominance => "qtl-interval-objective.broken-fifo",
}
#[test]
fn t12_quantile_hard_keeps_b7_exact_diagnostic() {
    let (s, ps) = h::compare_source("qtl-objective.broken-quantile-hard");
    let ps: Vec<_> = ps.iter().map(String::as_str).collect();
    h::assert_hard_refusal(&json!(compare::compare(
        &s,
        &ps,
        compare::Analysis::Exhaustive
    )));
}
#[test]
fn b7_d04_rule_hard_unsupported_and_c7_quantiles_withdrawn() {
    for sim in [false, true] {
        let v = json!(compare::compare(
            &h::rule_source(),
            &["AsIs", "ToBeB"],
            if sim {
                compare::Analysis::Simulation
            } else {
                compare::Analysis::Exhaustive
            }
        ));
        h::assert_rule(&v, sim);
    }
}
#[test]
fn a7_projection_accepts_both_declarations_without_a_new_metric_word() {
    let s = h::i::b6::append(
        &h::wait_source("normal"),
        "process Q { job = A.run(); end done }\nconfiguration One { cut C; process P }\nconfiguration Two { cut C; process Q }",
    );
    for mode in [compare::Analysis::Exhaustive, compare::Analysis::Simulation] {
        let v = json!(compare::compare(&s, &["One", "Two"], mode));
        assert!(
            v["diagnostics"].as_array().unwrap().is_empty(),
            "validate_waits crossing: {v}"
        );
        assert_eq!(v["plans"].as_array().unwrap().len(), 2);
        for p in v["plans"].as_array().unwrap() {
            assert_eq!(
                p["report"]["metrics"]["unchecked"],
                json!(h::WAIT_UNCHECKED)
            );
        }
    }
}

#[test]
fn b7_existing_objective_transports_c7_quantile_reports() {
    let v = json!(compare::compare(
        &h::existing_objective_source(),
        &["AsIs", "ToBeP"],
        compare::Analysis::Exhaustive
    ));
    h::assert_existing_objective(&v);
}

fn review(name: &str, sim: bool) {
    let s = h::review_source(name);
    let mode = if sim {
        compare::Analysis::Simulation
    } else {
        compare::Analysis::Exhaustive
    };
    let scenario = if name == "horizon" { Some("Day") } else { None };
    let v = json!(compare::compare_with_scenario(
        &s,
        &["One", "Two"],
        mode,
        scenario
    ));
    h::assert_review(&v, name, sim);
    let control = json!(compare::compare_with_scenario(
        &h::objective_free(&s),
        &["One", "Two"],
        mode,
        scenario
    ));
    h::assert_objective_noninterference(&v, &control);
}
macro_rules! review_cases {
    ($($test:ident => ($name:literal, $sim:literal)),+ $(,)?) => {$(#[test] fn $test(){review($name, $sim);})+};
}
review_cases! {
    r3_simulation_censored_null_objective => ("sample_censor", true),
    r3_zero_denominator_exhaustive => ("zero", false),
    r3_zero_denominator_simulation => ("zero", true),
    r3_denominator_may_be_zero_admissible_missing_pair => ("maybe", false),
    r3_horizon_undetermined_exhaustive_not_evaluated => ("horizon", false),
    r3_horizon_undetermined_simulation_not_evaluated => ("horizon", true),
    r3_reached_exhaustive_positive_control => ("reached", false),
    r3_reached_simulation_positive_control => ("reached", true),
}

#[test]
fn r3_rule_quantile_objective_both_modes() {
    for sim in [false, true] {
        let mode = if sim {
            compare::Analysis::Simulation
        } else {
            compare::Analysis::Exhaustive
        };
        let s = h::rule_quantile_source();
        let v = json!(compare::compare(&s, &["AsIs", "ToBeB"], mode));
        h::assert_rule_quantile(&v, sim);
        let control = json!(compare::compare(
            &h::objective_free(&s),
            &["AsIs", "ToBeB"],
            mode
        ));
        h::assert_objective_noninterference(&v, &control);
    }
}

fn shared(kind: &str, sim: bool) {
    let mode = if sim {
        compare::Analysis::Simulation
    } else {
        compare::Analysis::Exhaustive
    };
    h::assert_shared(
        &json!(compare::compare(
            &h::shared_source(kind),
            &["One", "Two"],
            mode
        )),
        kind,
        sim,
    );
}
macro_rules! shared_cases {
    ($($test:ident => ($kind:literal, $sim:literal)),+ $(,)?) => {$(#[test] fn $test(){shared($kind, $sim);})+};
}
shared_cases! {
    m1_shared_quantiles_exhaustive => ("quantiles", false),
    m1_shared_quantiles_simulation => ("quantiles", true),
    m1_shared_numerator_exhaustive => ("numerator", false),
    m1_shared_numerator_simulation => ("numerator", true),
    m1_shared_denominator_exhaustive => ("denominator", false),
    m1_shared_denominator_simulation => ("denominator", true),
    m1_shared_reversed_exhaustive => ("reversed", false),
    m1_shared_reversed_simulation => ("reversed", true),
}

#[test]
fn m1_unshared_closed_table_keeps_existing_text() {
    for kind in ["different_quantiles", "different_cohort", "metric"] {
        for sim in [false, true] {
            shared(kind, sim);
        }
    }
}
