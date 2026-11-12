#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/quantile_onto_inputs.rs"]
mod h;
#[path = "support/test_tmp.rs"]
mod test_tmp;
#[path = "../../relca-core/tests/support/waits_integration_inputs.rs"]
mod w;
use serde_json::{Value, json};

fn cli(source: &str, args: &[&str]) -> Value {
    let dir = test_tmp::root().join(format!(
        "relca-c7onto-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, source).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(args[0])
        .arg(&path)
        .args(&args[1..])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let verdict: relca_core::Verdict = serde_json::from_value(v["verdict"].clone()).unwrap();
    assert_eq!(out.status.code(), Some(verdict.exit_code()));
    std::fs::remove_dir_all(&dir).unwrap();
    v
}
#[test]
fn f35_all_real_command_assumption_paths_and_old_fields() {
    let mut failures = vec![];
    for (route, sim) in [
        ("normal", false),
        ("normal", true),
        ("refusal", true),
        ("late_abort", true),
        ("missing_clause", true),
        ("structural", true),
    ] {
        if std::panic::catch_unwind(|| {
            let s = h::wait_source(route);
            let args = [if sim { "simulate" } else { "check" }];
            let v = cli(&s, &args);
            eprintln!("waiting route {route} sim {sim} fields {:?}", h::fields(&v));
            h::assert_wait(&v, sim, route);
            h::assert_old(&v, &cli(&h::i::h::no_quantiles(&s), &args));
            fixture_match::Invocation::parse(Some(&json!({"command":args[0]})))
                .unwrap()
                .compare(&json!({"verdict":v["verdict"]}), &v)
                .expect("independent closed wire including A7 waiting");
        })
        .is_err()
        {
            failures.push((route, sim));
        }
    }
    assert!(failures.is_empty(), "waiting cross failures {failures:?}");
}
#[test]
fn f35_probability_scheduler_is_observed_after_quantile() {
    let s = h::scheduler_source();
    let v = cli(&s, &["check"]);
    h::assert_scheduler(&v);
}
fn objective(name: &str) {
    let (s, ps) = h::compare_source(name);
    let plans = ps.join(",");
    let v = cli(&s, &["compare", "--plans", &plans, "--mode", "exhaustive"]);
    h::assert_objective(&v, name);
    fixture_match::Invocation::parse(Some(
        &json!({"command":"compare","plans":ps,"mode":"exhaustive"}),
    ))
    .unwrap()
    .compare(&json!({"verdict":v["verdict"]}), &v)
    .expect("B7 real objective wire");
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
    h::assert_hard_refusal(&cli(
        &s,
        &["compare", "--plans", &ps.join(","), "--mode", "exhaustive"],
    ));
}
#[test]
fn b7_d04_rule_hard_unsupported_and_c7_quantiles_withdrawn() {
    for sim in [false, true] {
        h::assert_rule(
            &cli(
                &h::rule_source(),
                &[
                    "compare",
                    "--plans",
                    "AsIs,ToBeB",
                    "--mode",
                    if sim { "simulation" } else { "exhaustive" },
                ],
            ),
            sim,
        );
    }
}

#[test]
fn b7_existing_objective_transports_c7_quantile_reports() {
    h::assert_existing_objective(&cli(
        &h::existing_objective_source(),
        &["compare", "--plans", "AsIs,ToBeP", "--mode", "exhaustive"],
    ));
}

#[test]
fn simulation_quality_tail_allows_only_the_six_closed_old_rows() {
    let v = cli(&h::wait_source("normal"), &["simulate"]);
    fixture_match::validate_simulation_wire(&v).unwrap();
    let at = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .position(|r| r["field"] == "simulation.random")
        .unwrap();
    for fault in 0..3 {
        let mut bad = v.clone();
        let rows = bad["assumptions"].as_array_mut().unwrap();
        match fault {
            0 => {
                rows.remove(at + 2);
            }
            1 => {
                rows.swap(at, at + 1);
            }
            _ => {
                rows.insert(
                    at + 1,
                    json!({"field":"simulation.extra","value":"unexpected","affects":"unexpected"}),
                );
            }
        }
        assert!(
            fixture_match::validate_simulation_wire(&bad).is_err(),
            "closed old simulation tail fault {fault}"
        );
    }
}

#[test]
fn r1_quality_block_before_simulation_random_is_rejected() {
    for source in [
        h::wait_source("normal"),
        h::i::h::no_quantiles(&h::wait_source("normal")),
    ] {
        let v = cli(&source, &["simulate"]);
        fixture_match::validate_simulation_wire(&v).unwrap();
        let mut bad = v.clone();
        let rows = bad["assumptions"].as_array_mut().unwrap();
        let start = rows
            .iter()
            .position(|r| r["field"] == "quality.definition")
            .unwrap();
        let block: Vec<_> = rows.drain(start..start + 4).collect();
        let at = rows
            .iter()
            .position(|r| r["field"] == "simulation.random")
            .unwrap();
        rows.splice(at..at, block);
        assert_eq!(
            fixture_match::validate_simulation_wire(&bad).unwrap_err(),
            "quality assumptions must follow existing rows"
        );
    }
}

#[test]
fn r2_structural_refusal_rejects_independently_valid_sampled_quality() {
    let source = |route| {
        h::i::b6::append(
            &h::wait_source(route),
            "metric cohort.DoneRate = expected(count(case.Done)) / arrivals",
        )
    };
    let v = cli(&source("structural"), &["simulate"]);
    let sampled = cli(&source("normal"), &["simulate"]);
    fixture_match::validate_simulation_wire(&v).unwrap();
    fixture_match::validate_simulation_wire(&sampled).unwrap();
    fixture_match::validate_quality_wire(&sampled["quality"], true).unwrap();
    let mut bad = v.clone();
    bad["quality"] = sampled["quality"].clone();
    // Quality itself is valid: only the refusal root contradicts its samples.
    fixture_match::validate_quality_wire(&bad["quality"], true).unwrap();
    assert_eq!(
        fixture_match::validate_simulation_wire(&bad).unwrap_err(),
        "structural refusal quality must be fully withdrawn"
    );
    // Each family is also guarded independently: no reliance on the first
    // leaked trials field in the full-copy negative control above.
    for pointer in [
        "/quality/properties/0/cases",
        "/quality/properties/0/expected_count",
        "/quality/properties/0/unknown_count",
        "/quality/cohort/0/value",
        "/quality/outcomes_per_arrival/0/expected_count",
        "/quality/outcomes_per_arrival/0/rate",
        "/quality/quantiles",
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(pointer).unwrap() = sampled.pointer(pointer).unwrap().clone();
        if pointer == "/quality/quantiles" {
            for key in ["checked", "unchecked"] {
                bad["quality"][key] = sampled["quality"][key].clone();
            }
        }
        fixture_match::validate_quality_wire(&bad["quality"], true)
            .expect("independent wire-valid family transplant");
        assert_eq!(
            fixture_match::validate_simulation_wire(&bad).unwrap_err(),
            "structural refusal quality must be fully withdrawn",
            "{pointer}"
        );
    }
}

#[test]
fn no_metrics_legacy_five_siblings_are_not_a_waiting_exemption() {
    let revoker = h::i::b6::append(
        &w::revocation(true, false, true),
        "metric quantile.Median = quantile(0.5, case.Done)",
    );
    for s in [h::wait_source("structural"), revoker] {
        let v = cli(&s, &["simulate"]);
        assert!(v.get("metrics").is_none());
        fixture_match::validate_simulation_wire(&v).unwrap();
        let at = v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .position(|r| r["field"] == "metrics.window")
            .unwrap();
        for fault in 0..4 {
            let mut bad = v.clone();
            let rows = bad["assumptions"].as_array_mut().unwrap();
            match fault {
                0 => {
                    rows.remove(at + 2);
                }
                1 => {
                    rows.swap(at, at + 1);
                }
                2 => {
                    rows.insert(at+1,json!({"field":"metrics.extra","value":"unexpected","affects":"unexpected"}));
                }
                _ => {
                    rows.push(json!({"field":"metrics.waiting","value":"exclusive_interval_partition_v1","affects":"not computed"}));
                }
            }
            let err = fixture_match::validate_simulation_wire(&bad).unwrap_err();
            if fault == 3 {
                assert_eq!(err, "waiting assumption without metrics");
            }
        }
    }
}

#[test]
fn structural_refusal_shape_is_closed_to_the_exact_diagnostic_and_no_samples() {
    let v = cli(&h::wait_source("structural"), &["simulate"]);
    // These expected refusal fields are hand-authored, not copied from a run.
    assert!(v.get("metrics").is_none());
    assert_eq!(v["properties"], json!([]));
    assert_eq!(v["case_outcomes"], json!([]));
    assert_eq!(v["simulation"]["attempted_replications"], 0);
    assert_eq!(v["simulation"]["completed_replications"], 0);
    let msg = "simulation cannot check structural completion questions";
    let d = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .position(|d| d["code"] == "U-SEM-0" && d["message"] == msg)
        .unwrap();
    fixture_match::validate_simulation_wire(&v).unwrap();
    let sampled = cli(&h::wait_source("normal"), &["simulate"]);
    for fault in 0..4 {
        let mut bad = v.clone();
        match fault {
            0 => bad["diagnostics"][d]["message"] = json!("another U-SEM-0 refusal"),
            1 => bad["simulation"]["attempted_replications"] = json!(1),
            2 => bad["case_outcomes"]
                .as_array_mut()
                .unwrap()
                .push(sampled["case_outcomes"][0].clone()),
            _ => bad["assumptions"].as_array_mut().unwrap().push(
                sampled["assumptions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["field"] == "metrics.waiting")
                    .unwrap()
                    .clone(),
            ),
        }
        assert!(
            fixture_match::validate_simulation_wire(&bad).is_err(),
            "structural refusal fault {fault}"
        );
    }
}

fn review_compare(source: &str, plans: &[&str], sim: bool) -> Value {
    let mode = if sim { "simulation" } else { "exhaustive" };
    let names = plans.join(",");
    let mut args = vec!["compare", "--plans", &names, "--mode", mode];
    let mut invocation = json!({"command":"compare","plans":plans,"mode":mode});
    if source.contains("scenario Day") {
        args.extend(["--scenario", "Day"]);
        invocation["scenario"] = json!("Day");
    }
    let v = cli(source, &args);
    fixture_match::Invocation::parse(Some(&invocation))
        .unwrap()
        .compare(&json!({"verdict":v["verdict"]}), &v)
        .expect("real compare closed wire before semantic expectations");
    v
}
fn review(name: &str, sim: bool) {
    let s = h::review_source(name);
    let v = review_compare(&s, &["One", "Two"], sim);
    h::assert_review(&v, name, sim);
    let control = review_compare(&h::objective_free(&s), &["One", "Two"], sim);
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
        let s = h::rule_quantile_source();
        let v = review_compare(&s, &["AsIs", "ToBeB"], sim);
        h::assert_rule_quantile(&v, sim);
        let control = review_compare(&h::objective_free(&s), &["AsIs", "ToBeB"], sim);
        h::assert_objective_noninterference(&v, &control);
    }
}
fn shared(kind: &str, sim: bool) {
    h::assert_shared(
        &review_compare(&h::shared_source(kind), &["One", "Two"], sim),
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
