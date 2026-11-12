//! ADR0045 public acceptance. RED must be unsupported new syntax, not a compiler error.
#[path = "support/quality_acceptance.rs"]
mod h;
use relca_core::{engine, lower, reference, syntax};
use serde_json::Value;

#[test]
fn closed_edit_witnesses_reconstruct_every_installed_pair_without_noop_replacements() {
    let pairs: Value = serde_json::from_str(include_str!("support/quality_pairs.json")).unwrap();
    let installed = h::names();
    assert_eq!(
        installed.len(),
        129,
        "closed current population,not deferred upstream inputs"
    );
    for pair in pairs.as_array().unwrap() {
        let base = pair["base"].as_str().unwrap();
        let broken = pair["broken"].as_str().unwrap();
        if !installed.iter().any(|n| n == base) {
            continue;
        }
        assert!(installed.iter().any(|n| n == broken));
        let source = h::source(base);
        let mut changed = source.clone();
        for e in pair["edits"].as_array().unwrap().iter().rev() {
            let a = e["start"].as_u64().unwrap() as usize;
            let b = e["end"].as_u64().unwrap() as usize;
            assert_eq!(&source[a..b], e["old"].as_str().unwrap());
            changed.replace_range(a..b, e["new"].as_str().unwrap());
        }
        assert_ne!(changed, source, "{base}: no-op pair is not a control");
        assert_eq!(
            changed,
            h::source(broken),
            "{base}: exact authored edit witnesses"
        );
    }
}

#[test]
fn all_quality_sources_and_hand_oracles_collect_every_failure() {
    let mut failures = vec![];
    for n in h::names() {
        if std::panic::catch_unwind(|| h::hand(&n, &h::run(&n, &h::source(&n)))).is_err() {
            failures.push(n);
        }
    }
    assert!(
        failures.is_empty(),
        "all quality assertion failures: {failures:?}"
    );
}

#[test]
fn every_runtime_substrate_is_legal_without_new_quality_syntax() {
    let inventory = h::inventory();
    let mut failures = vec![];
    for n in h::names() {
        if inventory[&n]["code"].is_string() {
            continue;
        }
        let s = h::no_quality(&h::source(&n));
        let v = h::run(&n, &s);
        if v["verdict"] != h::expected(&n)["verdict"] {
            failures.push((n, v["verdict"].clone(), v["diagnostics"].clone()));
        }
    }
    assert!(
        failures.is_empty(),
        "unrelated substrate failures: {failures:?}"
    );
}

#[test]
fn engine_and_independent_reference_full_outcomes_no_runtime_exclusions() {
    let inventory = h::inventory();
    let mut failures = vec![];
    let mut complete = 0;
    for n in h::names() {
        if inventory[&n]["code"].is_string() {
            continue;
        }
        let s = h::source(&n);
        let choices: Vec<Option<&str>> =
            if n.starts_with("qual-compare") || n.starts_with("qual-diff") {
                vec![Some("Check"), Some("Fast")]
            } else if n.starts_with("qual-invoice") {
                vec![Some("AsIs")]
            } else {
                vec![None]
            };
        for cfg in choices {
            let result = std::panic::catch_unwind(|| {
                let a = syntax::parse(&s).expect("new quality syntax must be accepted");
                let m = match cfg {
                    Some(c) => lower::lower_configuration(&a, c).map(|l| l.model),
                    None => lower::lower(&a),
                }
                .expect("valid quality declarations must lower");
                if inventory[&n]["invocation"]["command"] == "simulate"
                    || inventory[&n]["invocation"]["mode"] == "simulation"
                {
                    let e = engine::simulate(&m);
                    let r = reference::simulate(&m);
                    assert_eq!(
                        r.stop,
                        reference::StopReason::Complete,
                        "{n}: no reference exclusions"
                    );
                    let ev = serde_json::to_value(&e).unwrap();
                    let rv = serde_json::to_value(&r.outcome).unwrap();
                    assert_eq!(
                        ev["quality"], rv["quality"],
                        "{n}: simulation quality independent aggregate"
                    );
                    if let Some(cfg) = cfg {
                        assert_eq!(
                            rv["quality"], inventory[&n]["plans"][cfg],
                            "{n}/{cfg}: reference hand oracle"
                        );
                    } else if let Some(q) = h::expected(&n).get("quality") {
                        assert_eq!(&rv["quality"], q, "{n}: reference hand oracle");
                    }
                    assert_eq!(
                        ev["replications"], rv["replications"],
                        "{n}: RNG keys and all per-rep observations"
                    );
                } else {
                    let e = engine::explore(&m);
                    let mut r = reference::explore(&m);
                    assert_eq!(
                        r.stop,
                        reference::StopReason::Complete,
                        "{n}: no reference exclusions"
                    );
                    // engine Outcomes do not prepend lowering diagnostics.
                    assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
                    r.outcome.diagnostics.drain(..m.diagnostics.len());
                    assert_eq!(
                        e, r.outcome,
                        "{n}/{cfg:?}: every runtime field, including quality"
                    );
                }
            });
            if result.is_ok() {
                complete += 1;
            } else {
                failures.push(format!("{n}/{cfg:?}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "reference incomplete/unequal: {failures:?}; completed={complete}"
    );
    assert_eq!(
        complete, 78,
        "75 runtime inputs plus3 extra configuration selections; no reference exclusions"
    );
}

#[test]
fn quality_added_only_explicit_legacy_and_explain_pairs_preserve_old_values() {
    for n in ["qual-legacy", "qual-explain"] {
        let s = h::source(n);
        let v = h::run(n, &s);
        assert!(
            v.get("quality").is_some(),
            "must exercise actual quality before projecting"
        );
        let old = h::run(n, &h::no_quality(&s));
        assert!(old.get("quality").is_none());
        assert_eq!(
            h::old_projection(v),
            h::old_projection(old),
            "{n}: all old fields remain unchanged"
        );
    }
}

#[test]
fn explicit_quality_additions_do_not_change_old_reports_samples_rng_or_agency() {
    let inventory = h::inventory();
    let mut failures = vec![];
    for n in h::names() {
        if inventory[&n]["code"].is_string()
            || matches!(
                inventory[&n]["invocation"]["command"].as_str(),
                Some("compare" | "diff")
            )
        {
            continue;
        }
        let s = h::source(&n);
        if !s
            .lines()
            .any(|l| l.trim_start().starts_with("property case."))
        {
            continue;
        }
        let result = std::panic::catch_unwind(|| {
            let with = h::run(&n, &s);
            assert!(
                with.get("quality").is_some(),
                "{n}: exercise actual quality,not two static errors"
            );
            let old = h::run(&n, &h::no_quality(&s));
            assert!(old.get("quality").is_none());
            assert_eq!(
                h::old_projection(with),
                h::old_projection(old),
                "{n}: every old report field"
            );
        });
        if result.is_err() {
            failures.push(n);
        }
    }
    assert!(
        failures.is_empty(),
        "old report changes in explicit quality pairs: {failures:?}"
    );
}

#[test]
fn single_unknown_min_and_max_are_not_scheduler_width_or_exact() {
    for n in [
        "qual-max-only-unknown",
        "qual-max-only-unknown.broken-short-clock",
        "qual-min-only-unknown.broken-short-clock",
        "qual-contention.broken-no-policy",
    ] {
        h::hand(n, &h::run(n, &h::source(n)));
    }
}

#[test]
fn within_zero_hours_days_and_initial_hits_are_normalized_without_blanking_other_queries() {
    let n = "qual-closed-within";
    let base = h::source(n);
    for (duration, ticks) in [("0min", 0), ("1h", 60), ("1d", 1440)] {
        let s = h::replace(&base, "within 1min", &format!("within {duration}"));
        let mut q = h::expected(n)["quality"].clone();
        q["properties"][2]["within"] = ticks.into();
        let actual = h::run(n, &s);
        assert_eq!(actual["verdict"], "inconclusive");
        assert_eq!(
            actual["quality"], q,
            "initial false hit survives any later cut; normalized {duration}"
        );
    }
}

#[test]
fn within_property_without_its_ratio_does_not_claim_deadline_attainment_rate_checked() {
    let n = "qual-contention";
    let s = h::replace(
        &h::source(n),
        "metric cohort.OnTimePerArrival = expected(count(case.OnTime)) / arrivals",
        "",
    );
    let mut q = h::expected(n)["quality"].clone();
    q["cohort"].as_array_mut().unwrap().pop();
    q["checked"]
        .as_array_mut()
        .unwrap()
        .retain(|v| v != "deadline_attainment");
    q["unchecked"]
        .as_array_mut()
        .unwrap()
        .push("deadline_attainment".into());
    let actual = h::run(n, &s);
    assert_eq!(actual["verdict"], "satisfied");
    assert_eq!(
        actual["quality"], q,
        "within alone is reachability,not a declared cohort deadline rate"
    );
}

#[test]
fn simulation_zero_denominator_is_not_zero_probability_or_no_samples() {
    let s = h::replace(
        &h::source("qual-zero-denominator"),
        "\n}\n",
        "\n  simulation replications 4 seed 1 confidence 0.5\n}\n",
    );
    let v = serde_json::to_value(relca_core::report::simulate(&s)).unwrap();
    assert_eq!(v["verdict"], "satisfied");
    let q = v.get("quality").expect("declared quality must be reported");
    assert_eq!(q["cohort"][0]["value"]["point"], "0");
    assert!(q["cohort"][1]["value"]["point"].is_null());
    assert_eq!(q["cohort"][1]["value"]["reason"], "zero_denominator");
    assert_eq!(q["cohort"][1]["value"]["samples"], 4);
    for side in ["numerator", "denominator"] {
        assert_eq!(
            q["cohort"][1]["value"][side],
            serde_json::json!({"mean":"0","min":"0","max":"0","samples":4,"reason":null})
        );
    }
    assert!(q["cohort"][1]["bounds_with_undetermined"].is_null());
}

#[test]
fn a_known_end_of_an_unselected_process_is_zero_not_an_unknown_name() {
    let s = h::replace(
        &h::source("qual-compare"),
        "\n}\n",
        "\n  process Unselected { end other }\n  property case.Other = reached(end.other)\n}\n",
    );
    let v = serde_json::to_value(relca_core::report::check_configuration(&s, "Check")).unwrap();
    assert_eq!(v["verdict"], "satisfied");
    let properties = v["quality"]["properties"]
        .as_array()
        .expect("all declared properties must be reported");
    let p = properties.iter().find(|p| p["name"] == "Other").unwrap();
    assert_eq!(
        p,
        &serde_json::json!({
            "name":"Other","condition":"end.other","within":null,
            "cases":[{"case_id":1,"arrival":0,"min":"0","max":"0","reason":null},{"case_id":2,"arrival":10,"min":"0","max":"0","reason":null}],
            "expected_count":{"lower":"0","upper":"0","basis":"exact","reason":null}
        })
    );
}

#[test]
fn simulation_compare_runtime_abort_keeps_reported_reasoned_null_cells() {
    let s = h::replace(
        &h::source("qual-compare-sim"),
        "outcomes kernel lenient; writes correct",
        "result nondet { pass, fail }",
    );
    let run = |src: &str| {
        serde_json::to_value(relca_core::compare::compare(
            src,
            &["Check", "Fast"],
            relca_core::compare::Analysis::Simulation,
        ))
        .unwrap()
    };
    let old = run(&h::no_quality(&s));
    assert_eq!(
        old["plans"][0]["report"]["verdict"], "satisfied",
        "independent old-syntax control"
    );
    assert_eq!(
        old["plans"][1]["report"]["verdict"], "unsupported",
        "intended runtime nondeterminism,not malformed quality substrate"
    );
    let v = run(&s);
    assert_eq!(v["verdict"], "unsupported");
    let rows = v["tables"]["quality"]
        .as_array()
        .expect("quality table must not disappear for one aborted plan");
    for field in ["value", "bounds_with_undetermined"] {
        let row = rows
            .iter()
            .find(|r| {
                r["key"]["scope"] == "cohort"
                    && r["key"]["subject"] == "WrongPerArrival"
                    && r["key"]["field"] == field
            })
            .unwrap();
        let cells = row["values"].as_array().unwrap();
        assert_eq!(cells[1]["configuration"], "Fast");
        assert_eq!(
            cells[1]["status"], "reported",
            "not_applicable is not computation failure"
        );
        if field == "value" {
            assert!(cells[1]["value"]["point"].is_null());
            assert_eq!(cells[1]["value"]["reason"], "aborted");
            assert_eq!(cells[1]["value"]["samples"], 0);
            assert_eq!(cells[0]["value"]["point"], "0");
        } else {
            assert!(cells[1]["value"].is_null());
        }
    }
}

#[test]
fn computational_cut_before_last_receipt_keeps_future_case_in_arrival_denominator() {
    let n = "qual-closed-within";
    let s = h::replace(&h::source(n), "[0min, 6min, 12min]", "[0min, 6min, 20min]");
    assert_eq!(
        h::run(n, &h::no_quality(&s))["verdict"],
        "inconclusive",
        "independent future-arrival substrate"
    );
    let v = h::run(n, &s);
    assert_eq!(v["verdict"], "inconclusive");
    let q = &v["quality"];
    assert_eq!(
        q["cohort"][0]["denominator"],
        serde_json::json!({"kind":"arrivals","count":3})
    );
    assert_eq!(
        q["cohort"][0]["bounds_with_undetermined"],
        serde_json::json!({"lower":"1/75","upper":"26/75"})
    );
    assert_eq!(
        q["properties"][2]["cases"][2],
        serde_json::json!({"case_id":3,"arrival":20,"min":"0","max":"1","reason":"truncated"}),
        "Future is not an initialized business valuation or a false-state hit"
    );
    assert_eq!(q["cohort"][2]["value"]["reason"], "truncated");
    assert_eq!(
        q["cohort"][2]["bounds_with_undetermined"],
        serde_json::json!({"lower":"2/3","upper":"1"})
    );
}

#[test]
fn no_quality_does_not_change_legacy_probability_or_query_count() {
    let n = "qual-legacy-route";
    let s = h::source(n);
    let with = h::run(n, &s);
    let without = h::run(n, &h::no_quality(&s));
    assert!(with.get("quality").is_some());
    assert_eq!(with["terminals"], without["terminals"]);
    assert!(with["terminals"][0].get("probability").is_none());
    assert_eq!(with["properties"], without["properties"]);
    let _: Value = with;
}
