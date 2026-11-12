//! ADR0063 acceptance. Future syntax is tested through APIs that already compile.
#[path = "support/q9_oracle.rs"]
mod hand;
#[path = "support/q9_inputs.rs"]
mod input;
use relca_core::{Verdict, engine, ir, lower, reference, report, syntax};
use serde_json::json;
use sha2::{Digest, Sha256};

fn model(name: &str) -> ir::Model {
    let src = input::source(name);
    let ast = syntax::parse(&src)
        .unwrap_or_else(|e| panic!("Q9 rates syntax not yet implemented for {name}: {e:?}"));
    lower::lower(&ast)
        .unwrap_or_else(|e| panic!("Q9 rates lowering not yet implemented for {name}: {e:?}"))
}
fn simulation(name: &str) {
    let actual = json!(report::simulate(&input::source(name)));
    hand::validate_sim(name, &actual)
        .unwrap_or_else(|e| panic!("{name}: {e}; diagnostics={}", actual["diagnostics"]));
}
macro_rules! sim {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            simulation($name);
        }
    };
}
sim!(c_a1_nonuniform_quarter_samples_not_analytic_mean, "c");
sim!(
    a1_half_probability_changes_occurrence_and_late_count,
    "half"
);
sim!(a2_fixed_response_is_model_only_not_proof, "fixed");
sim!(a2_exact_deadline_is_on_time, "deadline");
sim!(a2_shorter_deadline_keeps_time_but_finds_witness, "late");
sim!(a6_cut_defers_new_rate_and_time_not_counts, "defer");
sim!(a6_certain_input_does_not_clear_cut, "defer-fixed");
sim!(a7_clock_after_end_is_not_execution_close, "clock");
sim!(a8_deadlock_is_not_zero_time_response, "deadlock");
sim!(
    a9_spaced_arrivals_use_replications_not_case_trials,
    "two-spaced"
);
sim!(a9_burst_keeps_service_and_adds_queue, "two-burst");
sim!(a10_calendar_wait_is_not_service, "calendar");
sim!(
    a10_later_opening_only_moves_wait_and_response,
    "calendar-later"
);
sim!(a11_ordinal_and_synthetic_site_are_distinct, "ordinal");
sim!(a12_same_tick_finish_choice_remains_unresolved, "same-tick");
sim!(
    a12_two_routes_do_not_get_implicit_dispatch_order,
    "two-clauses"
);
sim!(a14_metrics_are_opt_in, "no-metrics");
sim!(a15_unconfigured_clause_is_not_given_a_probability, "mixed");
sim!(
    a15_unreached_unconfigured_clause_is_not_preflight_refused,
    "mixed-unreached"
);
sim!(
    a15_route_result_nondeterminism_is_not_sampled_uniformly,
    "route-nondet"
);
sim!(
    a15_normal_call_to_same_capability_is_not_the_contest_response,
    "normal-call"
);
sim!(
    a11_conditioned_denominator_is_six_targets_not_sixteen_arrivals,
    "conditional-target"
);

macro_rules! invalid {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            let (_, code, reason) = input::STATIC.iter().find(|r| r.0 == $name).unwrap();
            for v in [
                json!(report::check(&input::source($name))),
                json!(report::simulate(&input::source($name))),
            ] {
                hand::diagnostic(&v, code, reason).unwrap();
                assert!(v["terminals"].as_array().unwrap().is_empty());
            }
        }
    };
}
invalid!(
    a3_zero_probability_is_invalid_and_names_cut_or_excludes,
    "zero"
);
invalid!(a4_probability_above_one_is_invalid, "above-one");
invalid!(a4_occurrence_is_mandatory, "missing-occurrence");
invalid!(a4_occurrence_basis_is_mandatory, "missing-occurrence-basis");
invalid!(
    a4_blank_occurrence_basis_is_not_evidence,
    "empty-occurrence-basis"
);
invalid!(a4_service_basis_is_mandatory, "missing-service-basis");
invalid!(
    a4_empty_service_basis_is_not_evidence,
    "empty-service-basis"
);
invalid!(a4_independence_is_mandatory, "missing-independence");
invalid!(a4_actor_name_is_not_independence, "unknown-independence");
invalid!(a4_duplicate_rates_is_not_last_wins, "duplicate-rates");
invalid!(a4_empty_rates_is_not_default_half, "empty-rates");

#[test]
fn i1_rates_syntax_parses_before_lowering() {
    syntax::parse(&input::base())
        .expect("rates syntax is the new grammar, not invalid old substrate");
}

#[test]
fn c_a1_check_and_each_interpreter_keep_positive_support_and_frozen_end() {
    for name in ["c", "half", "tiny", "fixed", "deadline", "late", "defer"] {
        let m = model(name);
        assert!(
            m.uses_probability(),
            "rates alone opt into probability even with fixed time"
        );
        assert!(!m.legacy_route(), "new rates use the arrivals interpreter");
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        let e = engine::explore(&m);
        let response = if matches!(name, "fixed" | "deadline") {
            Verdict::Satisfied
        } else {
            Verdict::Violated
        };
        for (side, o) in [("reference", &r.outcome), ("engine", &e)] {
            assert!(!o.truncated, "{side}/{name}");
            let c = m
                .contracts
                .iter()
                .position(|c| matches!(c.kind, ir::ContractKind::ContestResponse { .. }))
                .unwrap();
            let p = o
                .properties
                .iter()
                .find(|p| p.property == ir::Property::Contract(ir::ClauseId(c)))
                .unwrap();
            assert_eq!(
                p.verdict, response,
                "{side}/{name}: not a statistical tolerance"
            );
            let cases = o.case_outcomes.as_ref().expect("arrivals path");
            let end = &cases[0].terminals[0];
            assert_eq!(
                (end.completed_at.unwrap().min, end.completed_at.unwrap().max),
                (1, 1)
            );
            assert_eq!((end.work.unwrap().min, end.work.unwrap().max), (1, 1));
            let rows = o
                .contest_runtime
                .as_ref()
                .expect("exhaustive possibilities");
            let a = &rows[0].alternatives;
            assert!(a.iter().any(|x| x.filed));
            if !matches!(name, "fixed" | "deadline" | "late") {
                assert!(a.iter().any(|x| !x.filed));
            }
            let metrics = json!(o.metrics);
            if name != "defer" {
                hand::subset(
                    &hand::range(
                        if matches!(name, "fixed" | "deadline" | "late") {
                            "3"
                        } else {
                            "1"
                        },
                        if matches!(name, "fixed" | "deadline" | "late") {
                            "3"
                        } else {
                            "5"
                        },
                    ),
                    &metrics["overall"]["work"],
                    side,
                )
                .unwrap();
            } else {
                assert_eq!(metrics["overall"]["work"]["reason"], "not_configured");
                assert!(metrics["overall"]["work"]["min"].is_null());
            }
        }
        let public = json!(report::check(&input::source(name)));
        assert_eq!(
            hand::property(&public, " #1 contest_response")["verdict"],
            json!(response)
        );
        hand::assumptions(&public, 1).unwrap();
    }
}

#[test]
fn a3_tiny_positive_rate_does_not_erase_missing_authority() {
    let v = json!(report::check(&input::source("tiny-no-authority")));
    assert_eq!(v["verdict"], "violated", "diagnostics={}", v["diagnostics"]);
    assert_eq!(hand::property(&v, " #1 contest")["verdict"], "violated");
    assert_eq!(
        v["contest_paths"][0]["checks"]["reversal_authority"]["satisfied"],
        false
    );
}

fn draw(rep: u64, case: u64, site: &str, n: u64) -> u64 {
    let key = format!("relca-sim-v1|seed=1|rep={rep}|case={case}|site={site}|n={n}");
    let bytes = Sha256::digest(key.as_bytes());
    u64::from_be_bytes(bytes[..8].try_into().unwrap())
}
fn key(rep: u64, case: u64, site: &str, n: u64) -> String {
    format!("relca-sim-v1|seed=1|rep={rep}|case={case}|site={site}|n={n}")
}

#[test]
fn green_independent_sha_bank_certifies_the_predeclared_vectors() {
    for (bank, ordinal, threshold) in [
        (hand::C, 1, 3u128 << 62),
        (hand::HALF, 1, 1u128 << 63),
        (hand::ORDINAL, 2, 3u128 << 62),
    ] {
        for rep in 1..=16 {
            let filed = u128::from(draw(
                rep,
                1,
                &format!("contest.Flow.clause{ordinal}.occurrence"),
                1,
            )) >= threshold;
            let duration = if !filed {
                0
            } else if u128::from(draw(rep, 1, "duration.Flow.contest:1", 1)) < (3u128 << 62) {
                2
            } else {
                4
            };
            assert_eq!(
                bank[(rep - 1) as usize],
                duration,
                "rep {rep}, clause {ordinal}"
            );
        }
    }
    for rep in 1..=16 {
        assert_eq!(
            draw(rep, 1, "kernel.Flow.d", 1) < (1u64 << 63),
            hand::TARGETS[(rep - 1) as usize]
        );
    }
}

#[test]
fn a11_both_interpreters_match_each_replication_before_aggregation() {
    for (name, bank, ordinal) in [
        ("c", hand::C, 1),
        ("half", hand::HALF, 1),
        ("ordinal", hand::ORDINAL, 2),
    ] {
        let m = model(name);
        let reference = reference::simulate(&m);
        assert_eq!(reference.stop, reference::StopReason::Complete);
        let engine = engine::simulate(&m);
        for (side, o) in [("reference", reference.outcome), ("engine", engine)] {
            assert_eq!(o.completed_replications, 16, "{side}/{name}");
            assert_eq!(o.replications.len(), 16);
            for (i, r) in o.replications.iter().enumerate() {
                let duration = bank[i];
                let rep = i as u64 + 1;
                let mut keys = vec![key(
                    rep,
                    1,
                    &format!("contest.Flow.clause{ordinal}.occurrence"),
                    1,
                )];
                if duration != 0 {
                    keys.push(key(rep, 1, "duration.Flow.contest:1", 1));
                }
                if duration == 4 {
                    keys.push(key(rep, 1, "duration.Flow.contest:1", 2));
                }
                let mut actual: Vec<_> = r.draws.iter().map(|d| d.key.clone()).collect();
                actual.sort();
                keys.sort();
                assert_eq!(
                    actual, keys,
                    "{side}/{name}/rep{rep}: no draws for an uncreated route"
                );
                assert_eq!(
                    r.cases[0].terminal.as_ref().unwrap().end.as_deref(),
                    Some("returned")
                );
                let metrics = json!(r.metrics);
                hand::subset(
                    &hand::range(&(1 + duration).to_string(), &(1 + duration).to_string()),
                    &metrics["overall"]["work"],
                    side,
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn a13_real_metrics_decoder_rejects_single_faults_not_only_the_hand_checker() {
    let v = json!(report::simulate(&input::source("fixed")));
    hand::validate_sim("fixed", &v).unwrap();
    let good = v["metrics"].clone();
    serde_json::from_value::<relca_core::metrics::BusinessMetrics>(good.clone())
        .expect("positive decode control");
    for (description, bad) in hand::wire_faults() {
        let mut metrics = good.clone();
        metrics["contests"] = bad;
        assert!(
            serde_json::from_value::<relca_core::metrics::BusinessMetrics>(metrics).is_err(),
            "{description}"
        );
    }
}

#[test]
fn a14_root_uncertainty_and_ledger_link_the_declared_not_validated_evidence() {
    let v = json!(report::simulate(&input::source("fixed")));
    hand::validate_sim("fixed", &v).unwrap();
    let u = v.get("uncertainty").expect("Q7 root uncertainty");
    assert!(
        u.to_string().contains("analyst-declared-not-validated"),
        "model uncertainty retains declaration limitation"
    );
    assert!(v["ledger"].to_string().contains("/metrics/contests/rules"));
}

#[test]
fn green_oracle_wire_single_faults_really_fail_without_production() {
    for name in [
        "c",
        "half",
        "fixed",
        "conditional-target",
        "two-spaced",
        "two-burst",
        "defer",
        "clock",
        "deadlock",
    ] {
        hand::validate_table(&hand::expected_table(name)).unwrap();
    }
    for (description, bad) in hand::wire_faults() {
        assert!(hand::validate_table(&bad).is_err(), "{description}");
    }
}

#[test]
fn green_abort_oracle_rejects_empty_and_partial_sample_tables() {
    let mut table = hand::expected_table("fixed");
    let row = &mut table["rows"][0];
    for k in [
        "target_observed",
        "target_not_reached",
        "target_unknown",
        "filed",
        "not_filed",
        "choice_unknown",
        "responded",
        "late_responded",
        "unresolved",
        "route_failed",
    ] {
        row[k] = serde_json::Value::Null;
    }
    for k in ["service_time", "queue_wait", "response_elapsed"] {
        row[k] = hand::absent("aborted");
    }
    row["filing_fraction"] =
        json!({"value":null,"numerator":null,"denominator":null,"reason":"aborted"});
    let good = json!({"mode":"simulation","verdict":"unsupported","diagnostics":[{"code":"U-SEM-0"}],"metrics":{"contests":table}});
    hand::validate_sim("route-nondet", &good).unwrap();
    let mut empty = good.clone();
    empty["metrics"]["contests"]["rows"] = json!([]);
    assert!(hand::validate_sim("route-nondet", &empty).is_err());
    let mut partial = good.clone();
    partial["metrics"]["contests"]["rows"][0]["responded"] = json!(0);
    assert!(hand::validate_sim("route-nondet", &partial).is_err());
    let mut leak = good;
    leak["metrics"]["contests"]["rows"][0]["queue_wait"]["mean"] = json!(0);
    assert!(hand::validate_sim("route-nondet", &leak).is_err());
}

#[test]
fn green_all_old_language_substrates_lower_and_single_edit_witnesses_are_real() {
    for name in input::SIM {
        let src = input::substrate(name);
        let ast = syntax::parse(&src).unwrap_or_else(|d| panic!("substrate {name}: {d:?}\n{src}"));
        lower::lower(&ast).unwrap_or_else(|d| panic!("substrate {name}: {d:?}\n{src}"));
    }
    assert_eq!(
        input::edit(&input::source("c"), "probability 0.25", "probability 0.5"),
        input::source("half")
    );
    assert_eq!(
        input::edit(
            &input::source("deadline"),
            "respond_by 2min",
            "respond_by 1min"
        ),
        input::source("late")
    );
    assert_eq!(
        input::edit(&input::source("two-spaced"), "[0min,4min]", "[0min,1min]"),
        input::source("two-burst")
    );
}

#[test]
fn green_legacy_contest_paths_metrics_and_refusal_do_not_change() {
    let src = input::source("legacy-fixed");
    let c = json!(report::check(&src));
    assert_eq!(c["verdict"], "satisfied");
    assert_eq!(c["contest_paths"][0]["basis"], "構造上存在");
    assert_eq!(
        hand::property(&c, " #1 contest_response")["verdict"],
        "satisfied"
    );
    assert!(c.pointer("/metrics/contests").is_none());
    hand::subset(
        &hand::range("1", "3"),
        &c["metrics"]["overall"]["work"],
        "legacy/work",
    )
    .unwrap();
    hand::subset(
        &hand::range("0", "22"),
        &c["metrics"]["overall"]["observed_cost_total"],
        "legacy/cost",
    )
    .unwrap();
    let a = c["contest_runtime"][0]["alternatives"].as_array().unwrap();
    assert_eq!(a.len(), 2);
    assert!(
        a.iter()
            .any(|r| r["filed"] == false && r["route_cost"]["reason"] == "never_created")
    );
    assert!(
        a.iter()
            .any(|r| r["filed"] == true && r["response_at"] == 3 && r["target_at"] == 1)
    );
    let s = json!(report::simulate(&src));
    assert_eq!(s["verdict"], "unsupported");
    assert_eq!(hand::property(&s, " #1 contest")["verdict"], "satisfied");
    assert!(s.pointer("/metrics/contests").is_none());
}

#[test]
fn green_invoice_retains_opaque_defer_and_reasoned_null() {
    let src = include_str!("../../../examples/invoice/to-be-a-phase1.relca");
    assert!(src.contains("defer Vendor.contest_rate"));
    assert!(src.contains("defer Resubmission"));
    let v = json!(report::check(src));
    assert_eq!(
        v["verdict"], "violated",
        "A wrong posting 231/125000 exceeds 1/1000"
    );
    let work = &v["metrics"]["overall"]["work"];
    assert_eq!(work["reason"], "not_configured");
    assert!(work["min"].is_null() && work["max"].is_null());
    assert!(v.pointer("/metrics/contests").is_none());
}
