//! Test-first intake: no production semantic repairs in this target.
#[path = "support/quantile_integration_inputs.rs"]
mod i;
use serde_json::{Value, json};
fn deferred(name: &str, r: bool) {
    let v = i::deferred(name);
    let s = v["source"].as_str().unwrap();
    let sim = v["invocation"]["command"] == "simulate";
    let scenario = v["invocation"]["scenario"].as_str();
    i::assert_backend(
        s,
        None,
        scenario,
        sim,
        r,
        &v["expected"]["quality"]["quantiles"],
    );
    if r {
        return;
    }
    let actual = i::run(s, None, scenario, sim);
    i::assert_report(
        &actual,
        &v["expected"]["quality"]["quantiles"],
        v["expected"]["verdict"].as_str().unwrap(),
    );
    assert_eq!(
        i::h::old_projection(actual),
        i::h::old_projection(i::run(&i::h::no_quantiles(s), None, scenario, sim))
    );
}
macro_rules! pair {
    ($e:ident,$r:ident,$f:expr) => {
        #[test]
        fn $e() {
            ($f)(false)
        }
        #[test]
        fn $r() {
            ($f)(true)
        }
    };
}
pair!(t14_pooling_engine, t14_pooling_reference, |r| deferred(
    "qtl-pooling",
    r
));
pair!(t14_horizon_engine, t14_horizon_reference, |r| deferred(
    "qtl-pooling.broken-horizon",
    r
));
pair!(t18_hit_engine, t18_hit_reference, |r| deferred(
    "qtl-denominator-horizon",
    r
));
pair!(t18_censor_engine, t18_censor_reference, |r| deferred(
    "qtl-denominator-horizon.broken-horizon",
    r
));
pair!(t25_within_engine, t25_within_reference, |r| deferred(
    "qtl-censor-within",
    r
));
pair!(t25_unbounded_engine, t25_unbounded_reference, |r| deferred(
    "qtl-censor-within.broken-no-within",
    r
));
pair!(x_unident_g_engine, x_unident_g_reference, |r| deferred(
    "mutation-X-unident-g",
    r
));
fn ordinary(s: &str, expected: Value, scenario: Option<&str>, r: bool) {
    let sim = false;
    i::assert_backend(s, None, scenario, sim, r, &expected);
    let actual = i::run(s, None, scenario, sim);
    let old = i::run(&i::h::no_quantiles(s), None, scenario, sim);
    assert!(actual["quality"]["quantiles"].is_array());
    assert_eq!(actual["quality"]["quantiles"], expected);
    i::h::assumptions(&actual);
    assert_eq!(i::h::old_projection(actual), i::h::old_projection(old));
}
fn clock_before_horizon(r: bool) {
    let d = i::deferred("qtl-denominator-horizon");
    let s = i::h::replace(
        &i::h::replace(
            d["source"].as_str().unwrap(),
            "horizon 2min",
            "horizon 30min",
        ),
        "clock 10min",
        "clock 1min",
    );
    let mut q =
        i::deferred("qtl-denominator-horizon.broken-horizon")["expected"]["quality"]["quantiles"]
            .clone();
    for row in q.as_array_mut().unwrap() {
        assert_eq!(row["value"]["reason"], "undetermined");
        row["value"]["reason"] = json!("truncated");
    }
    i::assert_backend(&s, None, Some("Day"), false, r, &q);
    if !r {
        i::assert_report(&i::run(&s, None, Some("Day"), false), &q, "inconclusive");
    }
}
pair!(
    clock_before_horizon_engine,
    clock_before_horizon_reference,
    clock_before_horizon
);
pair!(record_engine, record_reference, |r| ordinary(
    &i::record(),
    i::record_expected(false),
    None,
    r
));
pair!(record_wrong_engine, record_wrong_reference, |r| ordinary(
    &i::record_wrong(),
    i::record_expected(true),
    None,
    r
));
pair!(held_engine, held_reference, |r| ordinary(
    &i::held(),
    i::held_expected(),
    Some("Day"),
    r
));
pair!(held_initial_engine, held_initial_reference, |r| ordinary(
    &i::held_initial(),
    i::held_initial_expected(),
    Some("Day"),
    r
));
pair!(overflow_engine, overflow_reference, |r| ordinary(
    &i::overflow(),
    i::overflow_expected(),
    Some("Day"),
    r
));
pair!(atomic_route_engine, atomic_route_reference, |r| ordinary(
    &i::atomic(true),
    i::atomic_expected(true),
    None,
    r
));
pair!(atomic_miss_engine, atomic_miss_reference, |r| ordinary(
    &i::atomic(false),
    i::atomic_expected(false),
    None,
    r
));
pair!(version_check_engine, version_check_reference, |r| ordinary(
    &i::conflict(true),
    i::conflict_expected(true),
    None,
    r
));
pair!(version_unchecked_engine, version_unchecked_reference, |r| {
    ordinary(&i::conflict(false), i::conflict_expected(false), None, r)
});
fn held_samples(r: bool) {
    for (s, q, c) in [
        (i::held(), i::held_expected(), 3),
        (i::held_initial(), i::held_initial_expected(), 0),
        (i::overflow(), i::overflow_expected(), 0),
    ] {
        let s = i::b6::append(&s, "simulation replications 1 seed 1 confidence 0.5");
        let expected = i::deterministic_sample(q, 3, c);
        i::assert_backend(&s, None, Some("Day"), true, r, &expected);
        if !r {
            let v = i::run(&s, None, Some("Day"), true);
            assert_eq!(v["quality"]["quantiles"], expected);
            i::h::assumptions(&v);
        }
    }
}
pair!(
    held_overflow_samples_engine,
    held_overflow_samples_reference,
    held_samples
);
fn rule(sim: bool, r: bool) {
    let s = i::rule();
    let mut failures = vec![];
    for c in ["AsIs", "ToBeB", "ToBeB", "AsIs"] {
        for day in ["Light", "Later"] {
            if std::panic::catch_unwind(|| {
                i::assert_rule_route(&s, c, Some(day), false);
                i::assert_backend(
                    &s,
                    Some(c),
                    Some(day),
                    sim,
                    r,
                    &i::rule_expected(sim, c == "AsIs"),
                );
                // Reference tests must not accidentally exercise the engine report.
                if !r {
                    i::assert_rule_report(&i::run(&s, Some(c), Some(day), sim), sim, c == "AsIs");
                }
            })
            .is_err()
            {
                failures.push((c, day));
            }
        }
    }
    assert!(failures.is_empty(), "Rule hand failures {failures:?}");
}
pair!(rule_exhaustive_engine, rule_exhaustive_reference, |r| rule(
    false, r
));
pair!(rule_simulation_engine, rule_simulation_reference, |r| rule(
    true, r
));
fn rule_legacy(r: bool) {
    let s = i::rule_legacy();
    for c in ["AsIs", "ToBeB"] {
        i::assert_rule_route(&s, c, None, true);
        i::assert_backend(
            &s,
            Some(c),
            None,
            false,
            r,
            &i::rule_expected(false, c == "AsIs"),
        );
        if !r {
            let v = i::run(&s, Some(c), None, false);
            i::assert_rule_report(&v, false, c == "AsIs");
            assert_eq!(
                i::h::old_projection(v),
                i::h::old_projection(i::run(&i::h::no_quantiles(&s), Some(c), None, false))
            );
        }
    }
}
pair!(rule_legacy_engine, rule_legacy_reference, rule_legacy);
#[test]
fn c6_all_six_kinds_preserve_quantiles_and_comparison_sets_with_scenario() {
    for &(name, kind) in i::MUTATIONS {
        for day in [false, true] {
            let s = i::mutation_source(name, day);
            let scenario = day.then_some("Day");
            let v = json!(relca_core::mutation::mutate_set_with_scenario(
                &s,
                "bpr-default",
                scenario
            ));
            i::assert_mutation(&v, kind);
            let old = json!(relca_core::mutation::mutate_set_with_scenario(
                &i::h::no_quantiles(&s),
                "bpr-default",
                scenario
            ));
            assert_eq!(
                i::wrapper_old(v),
                i::wrapper_old(old),
                "{name} scenario {day}: no new detection meaning"
            );
        }
    }
}
#[test]
fn c6_missing_selector_is_a_located_gate_and_does_not_publish_quantiles() {
    let s = i::mutation_source("mut2-expiry", true);
    let v = json!(relca_core::mutation::mutate(&s));
    assert_eq!(v["verdict"], "invalid");
    assert!(v["original"].get("quality").is_none());
    let d = v["original"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "E-CONF-0")
        .unwrap();
    assert!(d["span"].is_object());
    for &(_, kind) in i::MUTATIONS {
        assert_eq!(v["mutations"][kind]["status"], "blocked");
    }
}
#[test]
fn c6_horizon_child_is_inconclusive_not_detected_and_retains_quantiles() {
    let s = i::mutation_horizon();
    i::assert_mutation_horizon(&json!(relca_core::mutation::mutate_set_with_scenario(
        &s,
        "bpr-default",
        Some("Day")
    )));
}
#[test]
fn record_latent_decision_is_statically_located_not_a_runtime_ir_refusal() {
    let s = i::h::replace(&i::record(), "v.confidence == pass", "correct == true");
    let v = i::run(&s, None, None, false);
    assert_eq!(v["verdict"], "invalid");
    let ds = v["diagnostics"].as_array().unwrap();
    let d = ds.iter().find(|d| d["code"] == "E-POL-LATENT").unwrap();
    assert!(d["span"].is_object());
    assert!(v.get("quality").is_none());
}
#[test]
fn record_binding_property_is_a_located_static_stateonly_refusal() {
    let s = i::h::replace(
        &i::record(),
        "property case.Pass = reached(entry.confidence == pass)",
        "property case.Pass = reached(v.confidence == pass)",
    );
    let v = i::run(&s, None, None, false);
    assert_eq!(v["verdict"], "unsupported");
    let ds: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert_eq!(ds.len(), 1);
    assert_eq!(ds[0]["code"], "U-SEM-0");
    assert!(ds[0]["span"].is_object());
    assert!(v.get("quality").is_none());
}
#[test]
fn structural_completion_refusal_places_quantile_after_old_quality_estimation() {
    let s = i::completion_refusal();
    let v = i::run(&s, None, None, true);
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(
        i::h::old_projection(v.clone()),
        i::h::old_projection(i::run(&i::h::no_quantiles(&s), None, None, true))
    );
    i::assert_aborted_quantiles(&v);
}
#[test]
fn both_missing_simulation_routes_retain_quantiles_without_fake_metrics() {
    let s = i::h::source("qtl-one");
    for day in [false, true] {
        let s = if day {
            i::b6::append(&s, "scenario Day { arrivals fixed [0min] }")
        } else {
            s.clone()
        };
        let v = i::run(&s, None, day.then_some("Day"), true);
        assert_eq!(v["verdict"], "invalid");
        assert!(v.get("metrics").is_none());
        i::assert_aborted_quantiles(&v);
    }
}
#[test]
fn late_sample_refusal_keeps_reasoned_quantiles_and_old_fields() {
    let s = i::b6::append(
        &i::conflict(true),
        "simulation replications 1 seed 1 confidence 0.5",
    );
    let v = i::run(&s, None, None, true);
    assert_eq!(v["verdict"], "unsupported");
    let rows = v["quality"]["quantiles"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    for row in rows {
        assert_eq!(
            row["value"],
            json!({"point":null,"observations":0,"censored":0,"reason":"aborted"})
        );
        assert!(row["lower_bound"].is_null());
    }
    i::h::assumptions(&v);
    assert_eq!(
        i::h::old_projection(v),
        i::h::old_projection(i::run(&i::h::no_quantiles(&s), None, None, true))
    );
}
#[test]
fn declared_quantiles_stay_out_of_metrics_and_old_compare_table() {
    let s = i::metrics(&i::record());
    let v = i::run(&s, None, None, false);
    i::assert_report(&v, &i::record_expected(false), "satisfied");
    assert!(
        v["metrics"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("quantiles"))
    );
}
#[test]
fn record_and_horizon_compare_and_diff_transport_nested_quantiles_not_old_table_rows() {
    for (src, want, day) in [
        (i::record_wrong(), i::record_expected(true), None),
        (i::held(), i::held_expected(), Some("Day")),
    ] {
        let src = i::b6::append(
            &src,
            "configuration One { cut C; process P }\nconfiguration Two { cut C; process P }",
        );
        let cmp = json!(relca_core::compare::compare_with_scenario(
            &src,
            &["One", "Two"],
            relca_core::compare::Analysis::Exhaustive,
            day
        ));
        assert_eq!(cmp["plans"].as_array().unwrap().len(), 2);
        for p in cmp["plans"].as_array().unwrap() {
            assert_eq!(p["report"]["quality"]["quantiles"], want);
        }
        assert!(
            !cmp["tables"]["quality"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["key"]["scope"] == "quantile")
        );
        let diff = json!(relca_core::diff::diff_with_scenario(
            &src, "One", "Two", day
        ));
        for side in ["from", "to"] {
            assert_eq!(diff[side]["report"]["quality"]["quantiles"], want);
        }
    }
}
