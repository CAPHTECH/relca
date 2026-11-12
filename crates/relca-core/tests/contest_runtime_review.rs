//! Regression oracles for the ADR 0022 D2 boundary-review rulings.
use relca_core::{engine, ir, lower, reference, syntax};

const FIFO: &str = r#"model ContestRuntime version "0.2" {
  type T = enum { pass, fail }
  people [j,p]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 4min; result nondet { fail } } }
  actor Route { executors [p]; capability respond() -> T { duration fixed 2min; result nondet { pass } } }
  actor Party { control party; standing over [end.returned] basis "claim" }
  permit j to Judge.judge
  permit p to Route.respond
  authority G { basis "decision"; grants j to Judge.judge }
  process Flow { d = Judge.judge(); if d == fail { end returned } else { end posted } }
  cut C { include [Judge,Route,Party] }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 4min }
}"#;
const CONTEST: &str =
    "contest Party on end.returned route Route.respond reversible_by [j] respond_by 4min";

fn model(s: &str) -> ir::Model {
    lower::lower(&syntax::parse(s).unwrap()).unwrap()
}

#[test]
fn m1_fifo_business_times_do_not_depend_on_contest_declaration() {
    for source in [FIFO.to_owned(), FIFO.replace(CONTEST, "")] {
        let m = model(&source);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for out in [&e, &r.outcome] {
            let cases = out.case_outcomes.as_ref().unwrap();
            assert_eq!(cases.len(), 2);
            for (case, expected) in cases.iter().zip([4, 8]) {
                assert_eq!(case.terminals.len(), 1);
                let t = case.terminals[0].completed_at.unwrap();
                assert_eq!((t.min, t.max), (expected, expected));
            }
        }
    }
}

#[test]
fn m1_unreached_contest_does_not_make_simulation_management_nondeterministic() {
    let source = FIFO.replace("nondet { fail }", "nondet { pass }").replace(
        "verify safety",
        "simulation replications 2 seed 0 confidence 0.5\nverify safety",
    );
    let m = model(&source);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    let ev = serde_json::to_value(&e).unwrap();
    let rv = serde_json::to_value(&r.outcome).unwrap();
    for out in [&ev, &rv] {
        assert_eq!(out["completed_replications"], 2, "{out:#}");
        assert_eq!(out["truncated_replications"], 0);
        assert!(
            !out["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SEM-0")
        );
    }
    for key in ["replications", "case_outcomes", "properties"] {
        assert_eq!(ev[key], rv[key], "{key}");
    }
}

#[test]
fn m2_revised_ruling_preserves_same_tick_finish_dispatch_interleaving() {
    for (source, expected, first, second) in [
        (
            include_str!("../../../fixtures/ctx-contest-resource/model.relca"),
            include_str!("../../../fixtures/ctx-contest-resource/expected.json"),
            6,
            8,
        ),
        (
            include_str!("../../../fixtures/ctx-contest-resource.broken-slow-route/model.relca"),
            include_str!("../../../fixtures/ctx-contest-resource.broken-slow-route/expected.json"),
            7,
            10,
        ),
    ] {
        let hand: serde_json::Value = serde_json::from_str(expected).unwrap();
        let m = model(source);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for rows in [
            hand["contest_runtime"].clone(),
            serde_json::to_value(e.contest_runtime.unwrap()).unwrap(),
            serde_json::to_value(r.outcome.contest_runtime.unwrap()).unwrap(),
        ] {
            for case in rows.as_array().unwrap() {
                let responses = vec![first, second];
                let filed: Vec<_> = case["alternatives"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|a| a["filed"] == true)
                    .collect();
                assert_eq!(
                    filed
                        .iter()
                        .map(|a| a["response_at"].as_u64().unwrap())
                        .collect::<Vec<_>>(),
                    responses
                );
                for a in filed {
                    assert_eq!(a["target_at"], 4);
                    assert_eq!(a["deadline"], 8);
                    assert_eq!(
                        a["status"],
                        if a["response_at"].as_u64().unwrap() > 8 {
                            "violated"
                        } else {
                            "responded"
                        }
                    );
                }
            }
        }
    }
}

fn both(source: &str) -> Vec<relca_core::outcome::Outcome> {
    let m = model(source);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    vec![e, r.outcome]
}

#[test]
fn minor1_uncreated_route_cost_is_not_missing_or_free() {
    let priced = include_str!("../../../fixtures/ctx-contest-metrics/model.relca");
    let deadlock = priced
        .replace("actor Route { executors [p];", "actor Route { capacity 0;")
        .replace("permit p to Route.respond", "");
    let missing = priced.replace(
        "include [Judge,Route,Party]",
        "include [Judge,Party] exclude Route reason \"outside\" loses \"response\"",
    );
    for source in [priced.to_owned(), deadlock, missing] {
        for out in both(&source) {
            let rows = out.contest_runtime.unwrap();
            for a in &rows[0].alternatives {
                if !a.filed || a.response_at.is_none() {
                    let cost = serde_json::to_value(&a.route_cost).unwrap();
                    assert_eq!(
                        cost,
                        serde_json::json!({"min":null,"max":null,"reason":"never_created"})
                    );
                }
            }
        }
    }
}

#[test]
fn minor2_cut_unresolved_route_work_and_wait_are_not_exact_extrema() {
    let source = include_str!("../../../fixtures/ctx-contest-metrics/model.relca")
        .replace("clock 10min", "clock 2min");
    for source in [
        source.clone(),
        source.replace("bounds {", "arrivals fixed [0min]\nbounds {"),
    ] {
        for out in both(&source) {
            assert!(out.truncated);
            let rows = out.contest_runtime.unwrap();
            let a = rows[0].alternatives.iter().find(|a| a.filed).unwrap();
            assert_eq!(serde_json::to_value(a.status).unwrap(), "unresolved");
            assert_eq!(a.route_work, None);
            assert_eq!(a.route_wait, None);
            assert_eq!(
                serde_json::to_value(&a.route_cost).unwrap(),
                serde_json::json!({"min":null,"max":null,"reason":"truncated"})
            );
        }
    }
}

#[test]
fn minor3_overdue_cut_retains_unresolved_status_and_explains_violation() {
    let source = include_str!("../../../fixtures/ctx-contest-metrics/model.relca")
        .replace("fixed 2min", "fixed 5min");
    for clock in [3, 4] {
        let source = source.replace("clock 10min", &format!("clock {clock}min"));
        for source in [
            source.clone(),
            source.replace("bounds {", "arrivals fixed [0min]\nbounds {"),
        ] {
            for out in both(&source) {
                assert!(out.truncated);
                assert_eq!(
                    out.properties.last().unwrap().verdict,
                    relca_core::Verdict::Violated
                );
                let rows = out.contest_runtime.unwrap();
                let a = rows[0].alternatives.iter().find(|a| a.filed).unwrap();
                assert_eq!(serde_json::to_value(a.status).unwrap(), "unresolved");
                assert_eq!(a.response_at, None);
                assert_eq!(a.closed_at, None);
                assert_eq!(
                    a.violation_reason.as_deref(),
                    Some("deadline_passed_unresolved")
                );
            }
        }
    }
}
