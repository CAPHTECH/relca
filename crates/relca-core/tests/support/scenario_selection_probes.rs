//! Scenario-selection probes, linked from the scenario integration tests.
// Planned signatures: lower_with_selection(ast, configuration, scenario),
// report::{check_with_selection,simulate_with_selection}(src, configuration,scenario).
// No test-only source projection is allowed in these semantic tests.
use relca_core::{engine, ir, lower, outcome, reference, report, syntax};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
fn source(n: &str) -> String {
    fs::read_to_string(root().join(n).join("model.relca")).unwrap()
}
fn selected(n: &str, config: Option<&str>, scenario: Option<&str>) -> ir::Model {
    let ast = syntax::parse(&source(n)).unwrap_or_else(|ds| panic!("{n}: {ds:?}"));
    lower::lower_with_selection(&ast, config, scenario).unwrap_or_else(|ds| panic!("{n}: {ds:?}"))
}
fn range(r: outcome::Range) -> Value {
    json!({"min":r.min,"max":r.max})
}
fn prob(p: &relca_core::probability::ProbabilityRange) -> Value {
    json!({"min":p.min.to_string(),"max":p.max.to_string()})
}
fn partial(expected: &Value, actual: &Value, path: &str) {
    match expected {
        Value::Object(obj) => {
            for (k, v) in obj {
                if k == "absent_diagnostics" {
                    continue;
                }
                partial(
                    v,
                    actual
                        .get(k)
                        .unwrap_or_else(|| panic!("missing {path}/{k}")),
                    &format!("{path}/{k}"),
                );
            }
        }
        Value::Array(xs) => {
            assert_eq!(
                xs.len(),
                actual.as_array().unwrap().len(),
                "{path}: complete array membership"
            );
            for (i, (x, y)) in xs.iter().zip(actual.as_array().unwrap()).enumerate() {
                partial(x, y, &format!("{path}/{i}"));
            }
        }
        _ => assert_eq!(expected, actual, "{path}"),
    }
}
fn hand(model: &ir::Model, out: &outcome::Outcome, expected: &Value, label: &str) {
    if let Some(t) = expected["bounds"].get("truncated") {
        assert_eq!(json!(out.truncated), *t, "{label}: cut classification");
    }
    if let Some(rows) = expected["case_outcomes"].as_array() {
        let actual: Vec<_> = out.case_outcomes.as_ref().unwrap().iter().map(|c| {
            let ts: Vec<_> = c.terminals.iter().map(|t| {
                let (kind,end)=match t.kind {
                    outcome::CaseTerminalKind::End(id) => ("end",json!(model.ends[id.0])),
                    outcome::CaseTerminalKind::Rejected => ("rejected",Value::Null),
                    outcome::CaseTerminalKind::CutOff => ("cut_off",Value::Null),
                    #[allow(unreachable_patterns)]
                    _ => panic!("unexpected terminal class in scn hand population"),
                };
                let mut v=json!({"kind":kind,"end":end,"completed_at":t.completed_at.map(range),
                    "elapsed":t.elapsed.map(range),"work":t.work.map(range),"result_at":t.result_at.map(range)});
                if let Some(p)=&t.probability { v["probability"]=prob(p); }
                v
            }).collect();
            json!({"case_id":c.case_id.0+1,"arrival":c.arrival,"terminals":ts})
        }).collect();
        partial(&json!(rows), &json!(actual), label);
    }
    if let Some(ps) = expected["properties"].as_array() {
        assert_eq!(
            ps.len(),
            out.properties.len(),
            "{label}: property population"
        );
        for (p, e) in out.properties.iter().zip(ps) {
            assert_eq!(json!(p.verdict), e["verdict"], "{label}: property verdict");
            if let Some(cs) = e["cases"].as_array() {
                let actual = p.cases.as_ref().unwrap();
                assert_eq!(actual.len(), cs.len());
                for (a, e) in actual.iter().zip(cs) {
                    assert_eq!(json!(a.verdict), e["verdict"], "{label}: case verdict");
                    if let Some(p) = e.get("probability") {
                        assert_eq!(
                            &prob(a.probability.as_ref().unwrap()),
                            p,
                            "{label}: probability"
                        );
                    }
                }
            }
        }
    }
    if let Some(m) = expected.get("metrics") {
        partial(m, &serde_json::to_value(&out.metrics).unwrap(), label);
    }
}

#[test]
fn scenario_engine_and_independent_reference_each_match_hand_oracles_then_each_other() {
    let rows: Vec<Value> = serde_json::from_str(include_str!("scenario_cases.json")).unwrap();
    let mut runtime = 0;
    for row in rows {
        let n = row["name"].as_str().unwrap();
        let inv = &row["invocation"];
        if inv["command"] != "check" || row["kind"] == "static" || row["kind"] == "invocation" {
            continue;
        }
        let h: Value =
            serde_json::from_slice(&fs::read(root().join(n).join("expected.json")).unwrap())
                .unwrap();
        if h["verdict"] == "invalid" || h["verdict"] == "unsupported" {
            continue;
        }
        let m = selected(n, inv["configuration"].as_str(), inv["scenario"].as_str());
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "{n}: no runtime exclusions"
        );
        hand(&m, &e, &h, &format!("{n}/engine"));
        hand(&m, &r.outcome, &h, &format!("{n}/reference"));
        assert_eq!(e.case_outcomes, r.outcome.case_outcomes, "{n}");
        assert_eq!(e.properties, r.outcome.properties, "{n}");
        assert_eq!(e.metrics, r.outcome.metrics, "{n}");
        assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
        assert_eq!(e.diagnostics, r.outcome.diagnostics[m.diagnostics.len()..]);
        let dto = serde_json::to_value(report::check_with_selection(
            &source(n),
            inv["configuration"].as_str(),
            inv["scenario"].as_str(),
        ))
        .unwrap();
        partial(&h, &dto, n);
        runtime += 1;
    }
    assert_eq!(
        runtime, 77,
        "closed authored runtime population must not silently disappear"
    );
}

#[test]
fn selected_and_unselected_static_errors_are_never_partial_ir_or_zero_filled_results() {
    let rows: Vec<Value> = serde_json::from_str(include_str!("scenario_cases.json")).unwrap();
    for row in rows {
        if row["kind"] != "static" {
            continue;
        }
        let n = row["name"].as_str().unwrap();
        let inv = &row["invocation"];
        let h: Value =
            serde_json::from_slice(&fs::read(root().join(n).join("expected.json")).unwrap())
                .unwrap();
        let code = h["diagnostics"][0]["code"].as_str().unwrap();
        // Missing selected simulation is a report-entry refusal, not an
        // invalid executable model for check/lower. Multi-scenario compare is
        // likewise owned by compare rather than generic name resolution.
        if inv["command"] == "simulate" {
            let v = serde_json::to_value(report::simulate_with_selection(
                &source(n),
                inv["configuration"].as_str(),
                inv["scenario"].as_str(),
            ))
            .unwrap();
            assert!(
                v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == code),
                "{n}: {v}"
            );
            assert!(v["bounds"]["truncated"].is_null());
            continue;
        }
        if inv["command"] == "compare" {
            let plans: Vec<_> = inv["plans"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_str().unwrap())
                .collect();
            let v = serde_json::to_value(relca_core::compare::compare_with_scenario(
                &source(n),
                &plans,
                relca_core::compare::Analysis::Exhaustive,
                inv["scenario"].as_str(),
            ))
            .unwrap();
            assert!(
                v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == code),
                "{n}: {v}"
            );
            continue;
        }
        let ds = syntax::parse(&source(n))
            .and_then(|a| {
                lower::lower_with_selection(
                    &a,
                    inv["configuration"].as_str(),
                    inv["scenario"].as_str(),
                )
            })
            .err()
            .unwrap_or_else(|| panic!("{n}: accepted invalid scenario"));
        assert!(ds.iter().any(|d| d.code == code), "{n}: {ds:?}");
    }
}

#[test]
fn horizon_simulation_replication_audit_matches_independent_reference_without_false_misses() {
    for n in [
        "scn-sim",
        "scn-sim.broken-no-horizon",
        "scn-sim-clock30",
        "scn-sim-clock30.broken-earlier",
        "scn-simulation-fifo",
        "scn-simulation-fifo.broken-no-policy",
    ] {
        let scenario = if n.starts_with("scn-simulation-fifo") {
            "Busy"
        } else {
            "Day"
        };
        let m = selected(n, None, Some(scenario));
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "{n}: independent reference must finish"
        );
        let ej = serde_json::to_value(&e).unwrap();
        let rj = serde_json::to_value(&r.outcome).unwrap();
        assert_eq!(
            ej["replications"], rj["replications"],
            "{n}: full per-path query/draw/cut audit"
        );
        if n == "scn-sim" {
            assert_eq!(ej["truncated_replications"], 0);
            for rep in ej["replications"].as_array().unwrap() {
                assert_eq!(rep["cut"], "horizon");
                assert_eq!(rep["cases"][2]["queries"], json!(["unknown"]));
                assert!(rep["cases"][2]["terminal"].is_null());
            }
        }
        let inv = serde_json::from_slice::<Value>(
            &fs::read(root().join(n).join("expected.json")).unwrap(),
        )
        .unwrap();
        let dto = serde_json::to_value(report::simulate_with_selection(
            &source(n),
            None,
            Some(scenario),
        ))
        .unwrap();
        partial(&inv, &dto, n);
    }
}

#[test]
fn scenario_name_is_not_a_random_key_component_in_either_interpreter() {
    let x = selected("scn-random-name", None, Some("X"));
    let y = selected("scn-random-name", None, Some("Y"));
    let a = engine::simulate(&x);
    let b = engine::simulate(&y);
    let ar = reference::simulate(&x);
    let br = reference::simulate(&y);
    assert_eq!(a.replications, b.replications);
    assert_eq!(a.replications, ar.outcome.replications);
    assert_eq!(a.replications, br.outcome.replications);
    for rep in &a.replications {
        for d in &rep.draws {
            assert!(!d.key.contains("scenario"));
            assert!(d.key.starts_with("relca-sim-v1|seed=0|rep="));
        }
    }
}

#[test]
fn independently_forged_public_ir_is_refused_by_both_backends() {
    let good = selected("scn-horizon", None, Some("Day"));
    let mut bads = Vec::new();
    let mut m = good.clone();
    m.horizon = Some(0);
    bads.push(m);
    let mut m = good.clone();
    m.horizon = Some(3);
    bads.push(m);
    let mut m = good.clone();
    m.scenario = None;
    bads.push(m);
    let mut m = good.clone();
    m.scenario.as_mut().unwrap().name.clear();
    bads.push(m);
    let mut m = good.clone();
    m.arrival_every.as_mut().unwrap().every = 0;
    bads.push(m);
    let mut m = good.clone();
    m.arrival_every.as_mut().unwrap().count += 1;
    bads.push(m);
    let mut m = good.clone();
    m.arrival_every.as_mut().unwrap().until = 4;
    bads.push(m);
    let mut m = good.clone();
    m.arrivals.as_mut().unwrap()[1] += 1;
    bads.push(m);
    for (i, m) in bads.iter().enumerate() {
        let e = engine::explore(m);
        let r = reference::explore(m);
        assert!(
            e.diagnostics.iter().any(|d| d.code == "U-SEM-0"),
            "IR fault{i}"
        );
        assert_eq!(r.stop, reference::StopReason::SemanticAbort, "IR fault{i}");
        assert!(e.case_outcomes.is_none());
    }
}

#[test]
fn expansion_limit_is_checked_before_allocation_and_exact_boundary_lowers_only() {
    let s = include_str!("scenario_limit_10000.relca");
    let a = syntax::parse(s).unwrap();
    let m = lower::lower_with_selection(&a, None, Some("Light")).unwrap();
    assert_eq!(m.arrivals.as_ref().unwrap().len(), 10000);
    assert_eq!(m.arrivals.as_ref().unwrap().last(), Some(&9999));
    // Never explore10000 cases as a substitute for validating the front end.
}

#[test]
fn computational_state_budget_wins_over_the_horizon_complete_claim() {
    let m = selected("scn-horizon", None, Some("Day"));
    let e = engine::explore_with_budget(&m, 0);
    assert!(e.truncated);
    let metrics = serde_json::to_value(e.metrics).unwrap();
    assert_eq!(metrics["status"], "truncated");
    assert!(metrics["overall"]["work"]["min"].is_null());
}
