//! ADR0049 Stage C acceptance: compile on PRE-C7, fail on missing declarations.
#[path = "support/quantile_acceptance.rs"]
mod h;
use relca_core::{engine, lower, reference, syntax};
use serde_json::{Value, json};

#[test]
fn every_quantile_command_matches_hand_values_collect_all_red() {
    let mut failures = vec![];
    for name in h::names() {
        if std::panic::catch_unwind(|| h::hand(&name, &h::run(&name, &h::source(&name)))).is_err() {
            failures.push(name);
        }
    }
    assert!(
        failures.is_empty(),
        "quantile hand assertion failures: {failures:?}"
    );
}

#[test]
fn all_runtime_substrates_are_legal_without_quantile_declarations() {
    let mut failures = vec![];
    for name in h::names().into_iter().filter(|n| h::runtime(n)) {
        let s = h::no_quantiles(&h::source(&name));
        let actual = h::run(&name, &s);
        if actual["verdict"] != h::expected(&name)["verdict"] {
            failures.push((
                name,
                actual["verdict"].clone(),
                actual["diagnostics"].clone(),
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "unrelated substrate defects, not C7 RED: {failures:#?}"
    );
}

#[test]
fn both_independent_interpreters_match_hand_inverse_and_whole_outcomes() {
    let mut failures = vec![];
    let mut completed = 0;
    for name in h::names().into_iter().filter(|n| h::runtime(n)) {
        let result = std::panic::catch_unwind(|| {
            let s = h::source(&name);
            let m = lower::lower(&syntax::parse(&s).expect("valid new quantile syntax"))
                .expect("valid quantile lower");
            let expected = h::expected(&name)["quality"]["quantiles"].clone();
            if h::inventory()[&name]["invocation"]["command"] == "simulate" {
                let e = engine::simulate(&m);
                let r = reference::simulate(&m);
                assert_eq!(
                    r.stop,
                    reference::StopReason::Complete,
                    "{name}: no reference exclusions"
                );
                let ev = serde_json::to_value(&e).unwrap();
                let rv = serde_json::to_value(&r.outcome).unwrap();
                assert_eq!(
                    ev["quality"]["quantiles"], expected,
                    "{name}: ENGINE vs hand"
                );
                assert_eq!(
                    rv["quality"]["quantiles"], expected,
                    "{name}: REFERENCE vs hand"
                );
                assert_eq!(ev["quality"], rv["quality"], "{name}: independent quality");
                assert_eq!(
                    ev["replications"], rv["replications"],
                    "{name}: observations and RNG audits"
                );
            } else {
                let e = engine::explore(&m);
                let mut r = reference::explore(&m);
                assert_eq!(
                    r.stop,
                    reference::StopReason::Complete,
                    "{name}: no reference exclusions"
                );
                assert_eq!(
                    serde_json::to_value(&e.quality).unwrap()["quantiles"],
                    expected,
                    "{name}: ENGINE vs hand"
                );
                assert_eq!(
                    serde_json::to_value(&r.outcome.quality).unwrap()["quantiles"],
                    expected,
                    "{name}: REFERENCE vs hand"
                );
                assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
                r.outcome.diagnostics.drain(..m.diagnostics.len());
                assert_eq!(
                    e, r.outcome,
                    "{name}: complete outcomes,not just quantities"
                );
            }
        });
        if result.is_err() {
            failures.push(name);
        } else {
            completed += 1;
        }
    }
    assert!(
        failures.is_empty(),
        "runtime reference failures: {failures:?}; completed={completed}"
    );
    assert_eq!(
        completed,
        h::names().iter().filter(|n| h::runtime(n)).count()
    );
}

#[test]
fn declaration_addition_preserves_old_reports_census_metrics_diagnostics_and_draws() {
    for name in h::names().into_iter().filter(|n| h::runtime(n)) {
        let s = h::source(&name);
        let with = h::run(&name, &s);
        assert!(
            with["quality"]["quantiles"].is_array(),
            "exercise actual new wire before projection: {name}"
        );
        let old = h::run(&name, &h::no_quantiles(&s));
        assert!(
            old["quality"].get("quantiles").is_none(),
            "absence,not null/empty"
        );
        assert_eq!(
            h::old_projection(with),
            h::old_projection(old),
            "{name}: only explicitly added positions may change"
        );
        if h::inventory()[&name]["invocation"]["command"] == "simulate" {
            let full = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
            let plain = lower::lower(&syntax::parse(&h::no_quantiles(&s)).unwrap()).unwrap();
            assert_eq!(
                engine::simulate(&full).replications,
                engine::simulate(&plain).replications,
                "{name}: every selected observation and full RNG seed/site/occurrence key, not only summary means"
            );
            let full = reference::simulate(&full);
            let plain = reference::simulate(&plain);
            assert_eq!(full.stop, reference::StopReason::Complete);
            assert_eq!(plain.stop, reference::StopReason::Complete);
            assert_eq!(
                full.outcome.replications, plain.outcome.replications,
                "{name}: independent observer must not consume or reseed randomness"
            );
        }
    }
}

#[test]
fn all_installed_pairs_have_exact_non_noop_edit_witnesses() {
    let pairs: Value = serde_json::from_str(include_str!("support/quantile_pairs.json")).unwrap();
    let names = h::names();
    let mut covered = std::collections::BTreeSet::new();
    for pair in pairs.as_array().unwrap() {
        let a = pair["base"].as_str().unwrap();
        let b = pair["broken"].as_str().unwrap();
        if !names.iter().any(|n| n == a) {
            continue;
        }
        covered.insert(b.to_owned());
        if pair.get("edits").is_some() {
            assert_eq!(pair["edits"], "bijection");
            continue;
        }
        assert_eq!(
            h::replace(
                &h::source(a),
                pair["old"].as_str().unwrap(),
                pair["new"].as_str().unwrap()
            ),
            h::source(b),
            "{b}"
        );
    }
    for name in names.iter().filter(|n| n.contains(".broken-")) {
        assert!(covered.contains(name), "no witness: {name}");
    }
}

#[test]
fn shared_metric_namespace_and_forward_property_reference_have_precise_spans() {
    for name in [
        "qtl-static.broken-duplicate",
        "qtl-static.broken-cohort-first",
        "qtl-static.broken-quantile-first",
    ] {
        let s = h::source(name);
        let v = h::run(name, &s);
        h::hand(name, &v);
        let d = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["severity"] == "error")
            .unwrap();
        let a = d["span"]["start"].as_u64().unwrap() as usize;
        let b = d["span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&s[a..b], "Median", "later duplicate name owns span");
        assert_eq!(
            a,
            s.rfind(".Median =").unwrap() + 1,
            "later declaration,not property/cohort array iteration order"
        );
    }
    let s = h::source("qtl-contention");
    let line = "  property case.Done = reached(end.done)\n";
    let s = h::replace(&s, line, "");
    let s = h::replace(&s, "\n}\n", &format!("\n{line}}}\n"));
    h::hand("qtl-contention", &h::run("qtl-contention", &s));
}

#[test]
fn engine_budget_cut_mid_tick_keeps_zero_lower_bound_and_unknown_denominator() {
    let s = h::source("qtl-same-tick");
    let m = lower::lower(&syntax::parse(&s).expect("quantile syntax")).unwrap();
    let out = engine::explore_with_budget(&m, 0);
    assert!(out.truncated);
    assert_eq!(out.properties[0].verdict, relca_core::Verdict::Inconclusive);
    assert_eq!(out.properties[0].verdict.exit_code(), 2);
    let v = serde_json::to_value(&out.quality).unwrap();
    let q = &v["quantiles"];
    assert_eq!(
        q[0]["value"],
        json!({"lower":null,"upper":null,"basis":null,"reason":"truncated"})
    );
    assert_eq!(
        q[0]["lower_bound"], "0",
        "unfinished same-tick start is still possible NOW,not at1"
    );
    assert_eq!(q[1]["value"]["reason"], "truncated");
    assert!(
        q[1]["lower_bound"].is_null(),
        "unknown Dlo0 cannot promise positive denominator"
    );
    assert!(out.diagnostics.iter().any(|d| d.code == "W-BUDGET-0"));
}

#[test]
fn simulation_clock_control_preserves_the_duration_hazard_key_at_two() {
    let before = h::run("qtl-sample-sim", &h::source("qtl-sample-sim"));
    let after = h::run(
        "qtl-sample-sim.broken-short-clock",
        &h::source("qtl-sample-sim.broken-short-clock"),
    );
    h::hand("qtl-sample-sim", &before);
    h::hand("qtl-sample-sim.broken-short-clock", &after);
    let outcomes: Vec<_> = ["qtl-sample-sim", "qtl-sample-sim.broken-short-clock"]
        .iter()
        .map(|n| {
            let m = lower::lower(&syntax::parse(&h::source(n)).unwrap()).unwrap();
            serde_json::to_value(engine::simulate(&m)).unwrap()
        })
        .collect();
    let reps = outcomes[0]["replications"]
        .as_array()
        .expect("real sample observations,not summary report");
    let shortened = outcomes[1]["replications"].as_array().unwrap();
    assert_eq!(reps.len(), 4);
    assert_eq!(shortened.len(), 4);
    for (a, b) in reps.iter().zip(shortened) {
        let early: Vec<_> = a["draws"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["key"].as_str().unwrap().contains("duration"))
            .take(1)
            .collect();
        assert!(!early.is_empty(), "not an empty key comparison");
        for d in early {
            assert!(
                b["draws"].as_array().unwrap().contains(d),
                "clock must not reseed or change site/occurrence"
            );
        }
    }
}

#[test]
fn sparse_large_clock_is_not_a_new_state_space_or_uses_probability_flag() {
    let s = h::source("qtl-contention");
    let s = h::replace(&s, "clock 10min", "clock 1000000000000min");
    let m = lower::lower(&syntax::parse(&s).expect("quantile syntax")).unwrap();
    let plain = lower::lower(&syntax::parse(&h::no_quantiles(&s)).unwrap()).unwrap();
    assert_eq!(m.legacy_route(), plain.legacy_route());
    assert_eq!(m.verify, plain.verify);
    assert_eq!(m.probability_queries, plain.probability_queries);
    let v = h::run("qtl-contention", &s);
    assert_eq!(
        v["quality"]["quantiles"],
        h::expected("qtl-contention")["quality"]["quantiles"]
    );
    assert_eq!(
        h::old_projection(v),
        h::old_projection(h::run("qtl-contention", &h::no_quantiles(&s)))
    );
}

#[test]
fn existing_consumers_transport_quantiles_without_new_comparison_or_mutation_meaning() {
    let s = h::source("qtl-first-hit");
    let want = h::expected("qtl-first-hit")["quality"]["quantiles"].clone();
    let explained = serde_json::to_value(relca_core::explain::explain(&s)).unwrap();
    assert_eq!(
        explained["quality"]["quantiles"], want,
        "flat explain: no configuration selector invented"
    );
    let configured = h::replace(
        &s,
        "\n}\n",
        "\n  configuration One { cut C; process P }\n  configuration Two { cut C; process P }\n}\n",
    );
    for name in ["One", "Two"] {
        let v = serde_json::to_value(relca_core::report::check_configuration(&configured, name))
            .unwrap();
        assert_eq!(v["quality"]["quantiles"], want, "configuration {name}");
    }
    for mode in [
        relca_core::compare::Analysis::Exhaustive,
        relca_core::compare::Analysis::Simulation,
    ] {
        let compared = serde_json::to_value(relca_core::compare::compare(
            &configured,
            &["One", "Two"],
            mode,
        ))
        .unwrap();
        let expected = if matches!(mode, relca_core::compare::Analysis::Simulation) {
            h::expected("qtl-first-hit-sim")["quality"]["quantiles"].clone()
        } else {
            want.clone()
        };
        for plan in compared["plans"]
            .as_array()
            .expect("existing compare plans")
        {
            assert_eq!(plan["report"]["quality"]["quantiles"], expected);
        }
        assert!(
            compared.get("evaluation").is_none(),
            "no compare declaration, no ranking/evaluation"
        );
        assert!(
            !compared["tables"]["quality"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["key"]["scope"] == "quantile"),
            "old table remains closed"
        );
    }
    let diff = serde_json::to_value(relca_core::diff::diff(&configured, "One", "Two")).unwrap();
    for side in ["from", "to"] {
        assert_eq!(diff[side]["report"]["quality"]["quantiles"], want);
    }
    let mutated = serde_json::to_value(relca_core::mutation::mutate(&s)).unwrap();
    assert_eq!(
        mutated["original"]["quality"]["quantiles"], want,
        "original report preserves observation"
    );
}

#[test]
fn relabeling_only_changes_independently_enumerated_old_and_quantile_name_positions() {
    use sha2::{Digest, Sha256};
    let locations: Value =
        serde_json::from_str(include_str!("support/quantile_label_locations.json")).unwrap();
    assert_eq!(locations["positions"].as_array().unwrap().len(), 13);
    assert_eq!(locations["quantile_positions"].as_array().unwrap().len(), 4);
    let mut values: Vec<Value> = ["qtl-label", "qtl-label.broken-renamed"]
        .iter()
        .map(|n| {
            let s = h::source(n);
            let mut v = h::run(n, &s);
            h::hand(n, &v);
            assert_eq!(
                v["model"]["sha256"],
                Sha256::digest(s.as_bytes())
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                "source hash is not blindly ignored"
            );
            v["model"]["sha256"] = Value::Null;
            v
        })
        .collect();
    for row in locations["positions"]
        .as_array()
        .unwrap()
        .iter()
        .chain(locations["quantile_positions"].as_array().unwrap())
    {
        let path = row["path"].as_str().unwrap();
        for (i, side) in ["left", "right"].iter().enumerate() {
            if path == "/actors/0/name" {
                let at = &mut values[i]["ledger"]["actor_versions"]["entries"][0]["actor"];
                assert_eq!(*at, row[*side]);
                *at = Value::Null;
            }
            assert_eq!(
                values[i].pointer(path).unwrap(),
                &row[*side],
                "exact frozen relabel location {path}"
            );
            *values[i].pointer_mut(path).unwrap() = Value::Null;
        }
    }
    assert_eq!(
        values[0], values[1],
        "all numbers,reasons,order,verdict,counters and unregistered fields unchanged"
    );
}
