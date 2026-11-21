use super::*;
use serde_json::{Value, json};

const MODEL: &str = include_str!("../../../fixtures/mean-compare/model.relca");

fn both(s: &str) -> [CompareReport; 2] {
    crate::syntax::parse(s).unwrap();
    let plans = ["Fast", "Slow"];
    [
        compare(s, &plans, Analysis::Exhaustive),
        compare_using(&plans, Analysis::Exhaustive, |name, _| {
            let (r, m) = report::check_selected_scenario_using(s, name, None, |m| {
                crate::reference::explore(m).outcome
            });
            (SelectedReport::Exhaustive(r), m)
        }),
    ]
}

#[test]
fn hand_mean_pareto_and_exclusions() {
    let want: Value =
        serde_json::from_str(include_str!("../../../fixtures/mean-compare/expected.json")).unwrap();
    for r in both(MODEL) {
        let v = json!(r);
        let e = &v["evaluation"];
        assert_eq!(
            e["objectives"][0]["objective"],
            want["evaluation"]["objectives"][0]["objective"]
        );
        assert_eq!(
            e["objectives"][0]["values"],
            want["evaluation"]["objectives"][0]["values"]
        );
        assert_eq!(
            e["pareto"]["non_dominated"],
            want["evaluation"]["pareto"]["non_dominated"]
        );
        assert_eq!(
            e["pareto"]["dominated"],
            want["evaluation"]["pareto"]["dominated"]
        );
        for plan in e["plans"].as_array().unwrap() {
            assert_eq!(plan["status"], "admissible");
        }
    }
    for r in both(include_str!(
        "../../../fixtures/mean-compare.broken-tight/model.relca"
    )) {
        let v = json!(r);
        assert_eq!(v["evaluation"]["plans"][0]["status"], "admissible");
        assert_eq!(v["evaluation"]["plans"][1]["status"], "excluded");
        assert_eq!(
            v["evaluation"]["hard"][0]["values"][1]["verdict"],
            "violated"
        );
    }
    for r in both(&MODEL.replace(
        "  compare [",
        "  verify metric mean.Lead <= 3min\n  compare [",
    )) {
        let v = json!(r);
        assert_eq!(v["evaluation"]["plans"][1]["verify_verdict"], "violated");
        assert_eq!(v["evaluation"]["plans"][1]["hard_verdict"], "satisfied");
        assert_eq!(v["evaluation"]["plans"][1]["status"], "excluded");
    }
}

#[test]
fn mean_objective_does_not_use_a_lower_bound() {
    for r in both(include_str!(
        "../../../fixtures/mean-compare.broken-clock/model.relca"
    )) {
        let v = json!(r);
        assert_eq!(
            v["plans"][1]["report"]["quality"]["means"][0]["lower_bound"],
            "5"
        );
        assert_eq!(
            v["evaluation"]["objectives"][0]["values"][1],
            json!({"configuration":"Slow","lower":null,"upper":null,"basis":null,"reason":"truncated"})
        );
        let row = &v["evaluation"]["hard"][0]["values"][1];
        assert_eq!(row["verdict"], "inconclusive");
        assert_eq!(row["interval"], json!({"lower":"5","upper":null}));
    }
}

#[test]
fn compare_names_shared_mean_observations() {
    let s = MODEL.replace("  compare [", "  metric quantile.Median = quantile(0.5, case.Done)\n  metric cohort.Reach = expected(count(case.Done)) / arrivals\n  metric mean.Cost = mean(cost)\n  compare [")
        .replace("minimize mean.Lead }", "minimize mean.Lead; minimize quantile.Median; maximize cohort.Reach; minimize mean.Cost; minimize metrics.overall.observed_cost_per_arrival }");
    for r in both(&s) {
        let assumption = &r
            .assumptions
            .iter()
            .find(|a| a.field == "compare.pareto")
            .unwrap()
            .affects;
        for (a, b) in [
            ("mean.Lead", "quantile.Median"),
            ("mean.Lead", "cohort.Reach"),
            ("mean.Cost", "metrics.overall.observed_cost_per_arrival"),
        ] {
            assert!(assumption.contains(&format!("({a}, {b})")), "{assumption}");
        }
    }
}

#[test]
fn sampled_comparison_does_not_use_mean_points_as_hard_evidence() {
    let s = MODEL.replace("  compare [", "  metric cohort.Reach = expected(count(case.Done)) / arrivals\n  simulation replications 2 seed 3 confidence 0.95\n  compare [")
        .replace("{ mean.Lead <= 6min }", "{ mean.Lead <= 6min; cohort.Reach >= 0 }");
    let r = compare(&s, &["Fast", "Slow"], Analysis::Simulation);
    let v = json!(r);
    let e = &v["evaluation"];
    assert_eq!(e["objectives"][0]["values"][0]["point"], "2");
    assert_eq!(e["objectives"][0]["values"][1]["point"], "5");
    assert_eq!(e["statistics"]["multiplicity"]["m"], 2); // Two plans, one cohort test each.
    assert_eq!(e["hard"][0]["values"][0]["reason"], "no_statistical_bound");
    assert_eq!(e["hard"][0]["values"][0]["verdict"], "unsupported");
    assert_eq!(e["hard"][1]["values"][0]["verdict"], "satisfied");
    assert_eq!(e["pareto"]["reason"], "simulation_estimates");
}

#[test]
fn overlapping_mean_objectives_and_hard_equality() {
    let once = |s: &str, from: &str, to: &str| {
        assert_eq!(s.matches(from).count(), 1, "{from}");
        s.replacen(from, to, 1)
    };
    let s = once(
        MODEL,
        "type T = enum { ok }",
        "type T = enum { ok }; type Choice = enum { a, b }",
    );
    let s = once(
        &s,
        "actor A { capacity 1",
        "actor A { capacity 1; capability pick() -> Choice { duration fixed 1min; result nondet { a, b } }",
    );
    let s = once(
        &s,
        "process Quick { x=A.quick(); end done }",
        "process Quick { c=A.pick(); if c == a { x=A.quick() } else { y=A.slow() }; end done }",
    );
    let s = once(
        &s,
        "process Long { x=A.slow(); end done }",
        "process Long { c=A.pick(); if c == a { x=A.quick() } else { y=A.slow() }; end done }",
    );
    // Both plans take 1+2 or 1+5, so [3,6]. <=6 holds even at the
    // upper endpoint; strict <6 straddles it and cannot certify a plan.
    for r in both(&s) {
        let e = &json!(r)["evaluation"];
        assert_eq!(e["hard"][0]["values"][0]["verdict"], "satisfied");
        assert_eq!(
            e["pareto"]["undetermined_pairs"],
            json!([{"from":"Fast","to":"Slow","reason":"overlapping_intervals"}])
        );
    }
    for r in both(&once(&s, "<= 6min", "< 6min")) {
        let e = &json!(r)["evaluation"];
        assert_eq!(e["hard"][0]["values"][0]["verdict"], "inconclusive");
        assert_eq!(
            e["hard"][0]["values"][0]["interval"],
            json!({"lower":"3","upper":"6"})
        );
    }
}
