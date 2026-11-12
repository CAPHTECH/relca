//! Fixed implementation-side seam assertions. This module NEVER solves graphs.
//! Engine and independent reference adapters must each call these functions.
#![allow(dead_code)]
use serde_json::Value;

fn rows(group: &str) -> Vec<Value> {
    let v: Value = serde_json::from_str(include_str!("quantile_private_oracles.json")).unwrap();
    v[group].as_array().unwrap().clone()
}
fn contract(group: &str, mut production: impl FnMut(&Value) -> Value) {
    for mut row in rows(group) {
        let expected = row.as_object_mut().unwrap().remove("expected").unwrap();
        let got = production(&row);
        for (key, value) in expected.as_object().unwrap() {
            assert_eq!(
                &got[key], value,
                "{group}/{}: {key}; independently hand-authored oracle",
                row["id"]
            );
        }
    }
}
/// Translate the tree into YOUR production first-hit graph/reducer. The adapter
/// returns A/B/D four-endpoint rows (where requested), plus flattened quantile
/// values including lower_bound. Do not emulate a CDF in this assertion module.
pub fn graph_contract(production: impl FnMut(&Value) -> Value) {
    contract("graphs", production);
}
/// Feed the actual Hit/Never/Censored observer and sample aggregator. The result
/// is a flattened sample value with lower_bound. Empty/aborted are real inputs.
pub fn sample_contract(production: impl FnMut(&Value) -> Value) {
    contract("samples", production);
}
/// Observe every accepted case's view at each SELECTED world transition, not
/// only its own trace. Pre-receipt and unselected candidates must be ignored.
pub fn world_contract(production: impl FnMut(&Value) -> Value) {
    contract("worlds", production);
}
/// Parse a valid quantile source ONCE, then forge IR directly for each mutation.
/// Test engine and reference entrances separately: errors cannot come from lower.
pub fn ir_contract(production: impl FnMut(&Value) -> Value) {
    contract("ir_mutants", production);
}
pub fn numeric_isolation_contract(production: impl FnOnce(u64) -> Value) {
    let got = production(1);
    assert_eq!(got["quantile_reason"], "truncated");
    assert_eq!(got["old_outcome_unchanged"], true);
    assert_eq!(got["old_diagnostics_unchanged"], true);
    assert_eq!(got["old_census_unchanged"], true);
}
pub fn sparse_graph_contract(production: impl FnOnce(u64) -> Value) {
    let got = production(1_000_000_000_000);
    assert_eq!(
        got["graph_builds"], 1,
        "one graph,not one graph per x or per minute"
    );
    assert_eq!(got["quantile"], "3");
}

fn empty_inventory(reference: bool) {
    let m = crate::quantile_test_support::model();
    let q = if reference {
        crate::reference::quality_empty(&m, false)
    } else {
        crate::quality::simulation(&m, &[], false)
    };
    let projection = crate::comparison_projection::quality_rows(Some(&q), &m, true);
    assert!(
        projection.is_ok(),
        "no_samples projection rejected: {:?}",
        projection.err()
    );
    assert!(q.unchecked.iter().any(|s| s == "quantiles"));
    assert!(!q.checked.iter().any(|s| s == "quantiles"));
    let quantiles = q.quantiles.as_ref().unwrap();
    assert!(
        quantiles
            .iter()
            .all(|r| r["value"]["reason"] == "no_samples")
    );
}
#[test]
fn m2_engine_no_samples_inventory() {
    empty_inventory(false);
}
#[test]
fn m2_reference_no_samples_inventory() {
    empty_inventory(true);
}

#[test]
fn m3_same_forged_public_ir_is_rejected_by_both_simulation_entrances() {
    let mut base = crate::quantile_test_support::model();
    // The sample scaffold has no simulation declaration. Use the existing
    // valid simulation fixture only for its declaration; forge after lower.
    let s = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/qtl-sample-sim/model.relca"
    ))
    .unwrap();
    base.simulation = crate::lower::lower(&crate::syntax::parse(&s).unwrap())
        .unwrap()
        .simulation;
    for row in rows("ir_mutants") {
        let m = crate::quantile_test_support::forge(&base, &row);
        let e = crate::engine::simulate(&m);
        let r = crate::reference::simulate(&m);
        assert_eq!(
            e.diagnostics.iter().any(|d| d.code == crate::diag::U_SEM_0),
            row["expected"]["engine_unsupported"] == true,
            "engine simulate {}",
            row["id"]
        );
        assert_eq!(
            format!("{:?}", r.stop),
            row["expected"]["reference_stop"].as_str().unwrap(),
            "reference simulate {}",
            row["id"]
        );
        assert_eq!(
            r.outcome
                .diagnostics
                .iter()
                .any(|d| d.code == crate::diag::U_SEM_0),
            row["expected"]["engine_unsupported"] == true,
            "reference simulate diagnostic {}",
            row["id"]
        );
        let ex = crate::engine::explore(&m);
        let rx = crate::reference::explore(&m);
        assert_eq!(
            e.diagnostics.iter().any(|d| d.code == crate::diag::U_SEM_0),
            ex.diagnostics
                .iter()
                .any(|d| d.code == crate::diag::U_SEM_0)
        );
        assert_eq!(r.stop, rx.stop);
    }
}

#[test]
fn f15b_reference_sources_do_not_depend_on_engine_reducers() {
    fn inspect(path: &std::path::Path) {
        if path.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                inspect(&entry.unwrap().path());
            }
        } else if path.extension().is_some_and(|s| s == "rs") {
            let source = std::fs::read_to_string(path).unwrap();
            // Identifier-level check also catches imports renamed with `as`,
            // not just a direct qualified call. Shared data types are allowed.
            let tokens = source
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .collect::<Vec<_>>();
            for reducer in [
                "sample_quantile",
                "inverse_quantile",
                "first_curves",
                "first_reduce",
            ] {
                assert!(
                    !tokens.contains(&reducer),
                    "{} depends on engine reducer {reducer}",
                    path.display()
                );
            }
        }
    }
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    inspect(&src.join("reference.rs"));
    inspect(&src.join("reference"));
}

#[test]
fn f38_sparse_clock_censor_evaluates_three_breakpoints_not_every_minute() {
    use std::cell::Cell;
    // A chance fork: Hit(2) with 1/2; a clock-censored leaf with 1/2.
    // The ONLY breakpoints are 0, 2, clock+1; one D and two solves per x.
    let clock = 100_000u64;
    let row = serde_json::json!({"tree":{"chance":[["1/2",2],["1/2",{"censored":{"earliest":clock+1,"cause":"clock"}}]]}});
    let g = crate::quantile_test_support::graph(&row);
    let calls = Cell::new(0usize);
    let actions = |i: usize| {
        // Root has neither hit nor censor: it is visited exactly once per
        // actual first_reduce call. Counting this callback needs no src hook.
        if i == 0 {
            calls.set(calls.get() + 1);
        }
        g.actions[i].clone()
    };
    let order = (0..g.views.len()).collect::<Vec<_>>();
    let f = crate::quality::first_curves(&order, actions, std::slice::from_ref(&g.views), 4096);
    assert_eq!(
        calls.get(),
        7,
        "one D plus A and B at three points, not O(clock)"
    );
    assert_eq!(f.xs, vec![0, 2, 100_001]);
    let query = crate::ir::QuantileMetric {
        name: "P95".into(),
        p: crate::quantile_test_support::rational(&serde_json::json!("19/20")),
        property: 0,
        given_reached: false,
    };
    let (value, lb) = crate::quality::inverse_quantile(&query, &f);
    assert_eq!(
        value,
        serde_json::json!({"lower":null,"upper":null,"basis":null,"reason":"truncated"})
    );
    assert_eq!(lb, "100001");
}

#[test]
fn world_adapter_preserves_actor_case_and_empty_case2_self_trace() {
    let row = rows("worlds")
        .into_iter()
        .find(|r| r["id"] == "T24-transient-other-case")
        .unwrap();
    assert_eq!(row["case2_trace"], serde_json::json!([]));
    assert_eq!(row["views"][2]["actor_case"], 1);
    let mut cross_view_seen = false;
    let got =
        crate::quantile_test_support::worlds(&row, |first, arrivals, now, selected, views| {
            if now == 11 {
                cross_view_seen = true;
                assert_eq!(views[0], (true, true, Some(11)), "case1 owns the event");
                assert_eq!(
                    views[1],
                    (true, true, None),
                    "case2 observes shared state without a self event"
                );
            }
            crate::quality::observe_first_world(first, arrivals, now, selected, views);
        });
    assert!(cross_view_seen);
    assert_eq!(got["first_hits"], serde_json::json!([11, 1]));
}
