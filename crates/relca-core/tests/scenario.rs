//! Scenario selection and old-grammar substrate controls.
//! Selection-API probes are linked below; hand inputs are not execution evidence.
use relca_core::{lower, syntax};
#[path = "support/scenario_selection_probes.rs"]
mod scenario_selection_probes;
use serde_json::Value;
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::{path::PathBuf, process::Command};

#[test]
fn every_intended_valid_scenario_has_an_independently_lowered_old_grammar_substrate() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("support/scenario_substrates.json")).unwrap();
    assert!(!rows.is_empty());
    let mut failures = Vec::new();
    for r in &rows {
        let source = r["source"].as_str().unwrap();
        let result = syntax::parse(source).and_then(|ast| {
            if let Some(c) = r["configuration"].as_str() {
                lower::lower_configuration(&ast, c).map(|s| s.model)
            } else {
                lower::lower(&ast)
            }
        });
        if let Err(ds) = result {
            failures.push(format!("{}: {ds:?}", r["name"]));
        }
    }
    assert!(
        failures.is_empty(),
        "unrelated old-grammar defects:\n{}",
        failures.join("\n")
    );
}

#[test]
fn materializer_is_reproducible_and_has_no_implementation_oracle_input() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("python3")
        .args([
            "-B",
            "crates/relca-core/tests/support/generate_scenario_fixtures.py",
            "--check",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// Boundary review of c9cefb5 (M1, M2, m2, m3). Every expectation below is hand
// arithmetic or the old rule read from the source, written before the repair.
mod boundary_review {
    use super::ledger_shape;
    use relca_core::{engine, lower, outcome, reference, report, syntax};
    use serde_json::{Value, json};

    /// Removes only the scenario shell (its header line and its closing brace).
    fn flat(src: &str) -> String {
        let mut out = Vec::new();
        let mut inside = false;
        for line in src.lines() {
            let t = line.trim();
            if t.starts_with("scenario ") && t.ends_with('{') {
                inside = true;
                continue;
            }
            if inside && t == "}" {
                inside = false;
                continue;
            }
            out.push(line);
        }
        assert!(!inside);
        out.join("\n")
    }
    fn without_horizon(src: &str) -> String {
        src.lines()
            .filter(|l| !l.trim().starts_with("horizon "))
            .collect::<Vec<_>>()
            .join("\n")
    }
    /// Scenario shell positions: the scenario
    /// record, the source hash, the four scenario assumption rows and the
    /// simulation.random note. Everything else must be the old-rule bytes.
    fn without_shell(mut v: Value) -> Value {
        v = ledger_shape::original_report(&v);
        let o = v.as_object_mut().unwrap();
        assert!(o.remove("scenario").is_some(), "shell report has a record");
        o["model"].as_object_mut().unwrap().remove("sha256");
        for row in o["assumptions"].as_array_mut().unwrap().iter_mut() {
            if row["field"] == "simulation.random" {
                let text = row["affects"].as_str().unwrap();
                row["affects"] = json!(text.trim_end_matches("; scenario の名前は鍵に入らない"));
            }
        }
        o["assumptions"]
            .as_array_mut()
            .unwrap()
            .retain(|r| !r["field"].as_str().unwrap().starts_with("scenario"));
        v
    }
    fn flat_without_hash(mut v: Value) -> Value {
        v = ledger_shape::original_report(&v);
        assert!(v.get("scenario").is_none());
        v["model"].as_object_mut().unwrap().remove("sha256");
        v
    }
    fn simulate_pair(src: &str) -> (Value, Value) {
        let shell =
            serde_json::to_value(report::simulate_with_selection(src, None, Some("Day"))).unwrap();
        let old = serde_json::to_value(report::simulate(&flat(src))).unwrap();
        (shell, old)
    }
    fn check_pair(src: &str) -> (Value, Value) {
        let shell =
            serde_json::to_value(report::check_with_selection(src, None, Some("Day"))).unwrap();
        let old = serde_json::to_value(report::check(&flat(src))).unwrap();
        (shell, old)
    }

    const LIVE: &str = r#"model M version "0.2" {
type T = enum { ok }
observed case { hit: bool = false }
actor A {
  capacity 1
  capability run() -> T {
    duration fixed 3min
    result nondet { ok }
    writes hit
    finish { hit = true }
  }
}
actor B {
  capacity 1
  capability run() -> T {
    duration fixed 3min
    result nondet { ok }
  }
}
process Flow { fork f { a = A.run(); b = B.run() } join all f; end done }
cut C { include [A, B] }
policy fifo tie_by case_id
scenario Day {
  simulation replications 4 seed 1 confidence 0.5
  arrivals fixed [0min, 2min]
  bounds { cases 2; live_tasks 1; clock 30min }
}
metrics {
  window 30min
  currency "JPY"
  rate A per_minute 1
  rate A.run per_start 0
  rate B per_minute 1
  rate B.run per_start 0
}
verify probability reached hit == true >= 0.5
}"#;

    #[test]
    fn m1_clock_cut_under_a_scenario_shell_is_a_truncated_metric_like_the_old_model() {
        // Clock 8: finishes 3, 6 and 9 > 8, so all four replications stop at
        // the clock with case 3 open. truncated_replications=4 > 0 => truncated.
        let src = include_str!("../../../fixtures/scn-sim.broken-no-horizon/model.relca");
        let (shell, old) = simulate_pair(src);
        for v in [&shell, &old] {
            assert_eq!(v["bounds"]["truncated"], true);
            assert_eq!(v["simulation"]["truncated_replications"], 4);
            assert_eq!(v["metrics"]["status"], "truncated");
            // m2: case 3 never ends in any replication: no sample (ADR 0019).
            let case3 = &v["metrics"]["cases"][2]["terminals"][0];
            assert_eq!(case3["lead_time"]["reason"], "no_samples");
            assert_eq!(case3["cost_per_case"]["reason"], "no_samples");
        }
        assert_eq!(without_shell(shell), flat_without_hash(old));
    }

    #[test]
    fn m1_live_tasks_cut_under_a_scenario_shell_is_a_truncated_metric_like_the_old_model() {
        // The fork starts two tasks of case 1 at time 0 > live_tasks 1: every
        // replication is cut at once, so truncated_replications=4 and no case
        // ends (both lead times have no sample).
        let (shell, old) = simulate_pair(LIVE);
        for v in [&shell, &old] {
            assert_eq!(v["bounds"]["truncated"], true);
            assert_eq!(v["simulation"]["truncated_replications"], 4);
            assert_eq!(v["metrics"]["status"], "truncated");
            for k in 0..2 {
                assert_eq!(
                    v["metrics"]["cases"][k]["terminals"][0]["lead_time"]["reason"],
                    "no_samples"
                );
            }
        }
        assert_eq!(without_shell(shell), flat_without_hash(old));
    }

    #[test]
    fn m3_scenario_shell_keeps_the_old_agency_marker_order_and_ledger() {
        // Two cases and a named response: the ledger has a row per case and,
        // by the old rule, obligation_debtor is appended after
        // multi_case_interference.
        let src = r#"model M version "0.2" {
type T = enum { ok }
actor A {
  capacity 1
  capability run() -> T {
    duration fixed 5min
    result nondet { ok }
  }
}
process Flow { r = A.run(); end done }
cut C { include [A] }
policy fifo tie_by case_id
scenario Day {
  arrivals fixed [0min, 2min]
  bounds { cases 2; live_tasks 1; clock 30min }
}
contract K { response delivery: case.received -> one_of(end.done) by 9min after case.received otherwise unresolved }
}"#;
        let (shell, old) = check_pair(src);
        for v in [&shell, &old] {
            // ADR 0043 (plan C04): the model declares no people, so lower
            // appends responsibility_people_undeclared after its markers;
            // report.rs still appends obligation_debtor once and last (ADR
            // 0042/0043 integration order).
            let u = v["agency"]["unchecked"].as_array().unwrap();
            assert_eq!(
                u[u.len() - 3..],
                [
                    json!("multi_case_interference"),
                    json!("responsibility_people_undeclared"),
                    json!("obligation_debtor")
                ]
            );
            assert_eq!(v["obligations"].as_array().unwrap().len(), 2);
        }
        assert_eq!(without_shell(shell), flat_without_hash(old));
    }

    #[test]
    fn anonymous_response_ledger_is_not_added_by_a_scenario_shell() {
        // Old rule (Model::tracks_obligations): an anonymous response of a
        // multi-case model has no ledger. Without D the deadlines [9,11,13]
        // meet finishes [5,10,15]: cases 1 and 2 satisfied, case 3 violated.
        let src = without_horizon(include_str!("../../../fixtures/scn-obligation/model.relca"));
        let (shell, old) = check_pair(&src);
        for v in [&shell, &old] {
            assert!(v.get("obligations").is_none());
            let cases: Vec<_> = v["properties"][0]["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["verdict"].clone())
                .collect();
            assert_eq!(
                cases,
                [json!("satisfied"), json!("satisfied"), json!("violated")]
            );
        }
        assert_eq!(without_shell(shell), flat_without_hash(old));
        // With D12 the case-3 deadline 13 > 12 stays inconclusive (P04), still
        // without a ledger.
        let v = serde_json::to_value(report::check_with_selection(
            include_str!("../../../fixtures/scn-obligation/model.relca"),
            None,
            Some("Day"),
        ))
        .unwrap();
        assert!(v.get("obligations").is_none());
        assert_eq!(v["properties"][0]["cases"][2]["verdict"], "inconclusive");
    }

    pub(super) fn contest(horizon: u64) -> String {
        format!(
            r#"model ContestRuntime version "0.2" {{
  type T = enum {{ pass, fail }}
  people [j,p]
  actor Judge {{ executors [j]; capability judge() -> T {{ duration fixed 4min; result nondet {{ fail }} }} }}
  actor Route {{ executors [p]; capability respond() -> T {{ duration fixed 2min; result nondet {{ pass }} }} }}
  actor Party {{ control party; standing over [end.returned] basis "claim" }}
  permit j to Judge.judge
  permit p to Route.respond
  authority G {{ basis "decision"; grants j to Judge.judge }}
  process Flow {{ d = Judge.judge(); if d == fail {{ end returned }} else {{ end posted }} }}
  cut C {{ include [Judge,Route,Party] }}
  scenario Day {{
    arrivals fixed [0min]
    policy fifo tie_by case_id
    bounds {{ cases 1; live_tasks 2; clock 30min }}
    horizon {horizon}min
  }}
  verify safety no_resource_overcommit
  contract Controls {{ contest Party on end.returned route Route.respond reversible_by [j] respond_by 2min }}
}}
"#
        )
    }
    fn selected(src: &str) -> relca_core::ir::Model {
        lower::lower_with_selection(&syntax::parse(src).unwrap(), None, Some("Day")).unwrap()
    }
    fn unreach(ds: &[relca_core::diag::Diagnostic]) -> bool {
        ds.iter().any(|d| d.code.starts_with("W-UNREACH-"))
    }

    #[test]
    fn m2_horizon_after_business_end_with_only_the_contest_route_open_is_unchecked_in_both_interpreters()
     {
        // Judge 0..4 -> fail -> end returned at 4. Not filed: closed at 4.
        // Filed at 4: route 4..6, deadline 6. D=5 stops this branch at 5 with
        // the route running: no business cutoff row, but the suffix after 5 is
        // not inspected, so reachability is unchecked and no W-UNREACH is made.
        let m = selected(&contest(5));
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for (label, o) in [("engine", &e), ("reference", &r.outcome)] {
            assert!(!o.truncated, "{label}: horizon is not a computational cut");
            assert_eq!(o.unchecked, ["unreachable"], "{label}");
            assert!(!unreach(&o.diagnostics), "{label}: no W-UNREACH after D");
            let c = &o.case_outcomes.as_ref().unwrap()[0];
            assert_eq!(c.terminals.len(), 1, "{label}: no cutoff row after End");
            assert!(matches!(
                c.terminals[0].kind,
                outcome::CaseTerminalKind::End(_)
            ));
            assert_eq!(
                c.terminals[0].completed_at.as_ref().map(|r| (r.min, r.max)),
                Some((4, 4))
            );
        }
        assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
        assert_eq!(e.properties, r.outcome.properties);
        let v = serde_json::to_value(report::check_with_selection(&contest(5), None, Some("Day")))
            .unwrap();
        assert_eq!(v["verdict"], "inconclusive");
        assert_eq!(v["bounds"]["truncated"], false);
        assert_eq!(v["unchecked"], json!(["unreachable"]));
        let filed = &v["contest_runtime"][0]["alternatives"][1];
        assert_eq!(
            (
                &filed["filed"],
                &filed["deadline"],
                &filed["status"],
                &filed["closed_at"]
            ),
            (&json!(true), &json!(6), &json!("unresolved"), &Value::Null)
        );
        assert_eq!(
            v["scenario"]["aggregation"]["observed_until"],
            json!({"min":"4","max":"5","reason":null})
        );
    }

    #[test]
    fn m2_control_route_closing_exactly_at_the_horizon_leaves_no_horizon_leaf() {
        // D=6: the route finish at 6 is processed in the layer at D, all
        // executions close, and no leaf stops at the horizon. The graph is
        // complete: no unchecked marker; response 6 <= deadline 6 is satisfied.
        let m = selected(&contest(6));
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        for (label, o) in [("engine", &e), ("reference", &r.outcome)] {
            assert!(!o.truncated, "{label}");
            assert!(o.unchecked.is_empty(), "{label}");
        }
        let v = serde_json::to_value(report::check_with_selection(&contest(6), None, Some("Day")))
            .unwrap();
        assert_eq!(v["verdict"], "satisfied");
        assert!(v.get("unchecked").is_none());
        assert_eq!(
            v["scenario"]["aggregation"]["observed_until"],
            json!({"min":"4","max":"6","reason":null})
        );
    }
}

// Mutation survivors of c9cefb5 (F14b, F17a, F17b, F17r, F22c, G01c, G03c,
// G03d, G04a, G06c, G09b, G09bb, G09c). Every expectation is FIFO interval
// arithmetic or ADR 0042 read before running the binary: a business horizon D
// advances to D, processes the events at D, then judges deadlines <= D
// (ADR 0042 "止めるときの期限の判定", M3). Arrival + W <= D fixes a within
// row; the horizon is not a computational cut (truncated stays false).
mod mutation_survivors {
    use relca_core::{compare, engine, ir, lower, outcome, reference, report, syntax};
    use serde_json::{Value, json};

    const BODY: &str = r#"model M version "0.2" {
type T = enum { ok }
observed case { hit: bool = false }
actor A {
  capacity 1
  capability run() -> T {
    duration fixed 5min
    result nondet { ok }
    writes hit
    finish { hit = true }
  }
}
process Flow { r = A.run(); end done }
cut C { include [A] }
policy fifo tie_by case_id
"#;

    /// Arrivals 0, 2, 4 (fixed_every 2min during [0, 6)), service 5, one
    /// server: starts 0, 5, 10 and finishes 5, 10, 15.
    fn day(extra_scenario: &str, horizon: u64, clock: u64, tail: &str) -> String {
        format!(
            "{BODY}scenario Day {{\n  arrivals fixed_every 2min during [0min, 6min)\n  horizon {horizon}min\n  bounds {{ cases 3; live_tasks 1; clock {clock}min }}\n{extra_scenario}}}\n{tail}\n}}\n"
        )
    }
    const SIM: &str = "  simulation replications 4 seed 1 confidence 0.5\n";
    const BY8: &str = "contract K { response case.received -> one_of(end.done) by 8min after case.received otherwise unresolved }";

    fn selected(src: &str, scenario: &str) -> ir::Model {
        lower::lower_with_selection(&syntax::parse(src).unwrap(), None, Some(scenario))
            .unwrap_or_else(|ds| panic!("{ds:?}"))
    }
    fn partial(expected: &Value, actual: &Value, path: &str) {
        match expected {
            Value::Object(o) => {
                for (k, v) in o {
                    let a = actual
                        .get(k)
                        .unwrap_or_else(|| panic!("missing {path}/{k} in {actual}"));
                    partial(v, a, &format!("{path}/{k}"));
                }
            }
            Value::Array(xs) => {
                let ys = actual.as_array().unwrap_or_else(|| panic!("{path}: array"));
                assert_eq!(xs.len(), ys.len(), "{path}: complete array membership");
                for (i, (x, y)) in xs.iter().zip(ys).enumerate() {
                    partial(x, y, &format!("{path}/{i}"));
                }
            }
            _ => assert_eq!(expected, actual, "{path}"),
        }
    }
    fn range(r: Option<outcome::Range>) -> Value {
        r.map_or(Value::Null, |r| json!({"min":r.min,"max":r.max}))
    }
    fn prob(p: &Option<relca_core::probability::ProbabilityRange>) -> Value {
        p.as_ref().map_or(
            Value::Null,
            |p| json!({"min":p.min.to_string(),"max":p.max.to_string()}),
        )
    }
    /// Per-case property rows of one interpreter (verdict and probability).
    fn props(o: &outcome::Outcome) -> Value {
        json!(
            o.properties
                .iter()
                .map(
                    |p| json!({"verdict":p.verdict,"cases":p.cases.as_ref().unwrap().iter()
                    .map(|c| json!({"verdict":c.verdict,"probability":prob(&c.probability)}))
                    .collect::<Vec<_>>()})
                )
                .collect::<Vec<_>>()
        )
    }
    fn terminals(o: &outcome::Outcome) -> Value {
        json!(
            o.case_outcomes
                .as_ref()
                .unwrap()
                .iter()
                .map(|c| c
                    .terminals
                    .iter()
                    .map(|t| json!({
                        "kind": match t.kind {
                            outcome::CaseTerminalKind::ReceiptUnknown => panic!("scenario terminal oracle does not cover receipt observations"),
                            outcome::CaseTerminalKind::End(_) => "end",
                            outcome::CaseTerminalKind::Rejected => "rejected",
                            outcome::CaseTerminalKind::Overflow(_) => "overflow",
                            outcome::CaseTerminalKind::CutOff => "cut_off",
                        },
                        "completed_at": range(t.completed_at),
                        "elapsed": range(t.elapsed),
                        "work": range(t.work),
                        "probability": prob(&t.probability),
                    }))
                    .collect::<Vec<_>>())
                .collect::<Vec<_>>()
        )
    }
    fn observed_until(o: &outcome::Outcome) -> Value {
        serde_json::to_value(&o.scenario_observed_until).unwrap()
    }
    /// Runs both interpreters; the reference must finish without exclusions.
    fn both(m: &ir::Model) -> [(&'static str, outcome::Outcome); 2] {
        let r = reference::explore(m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        [("engine", engine::explore(m)), ("reference", r.outcome)]
    }
    fn p(v: &str) -> Value {
        json!({"min":v,"max":v})
    }
    fn rg(a: u64, b: u64) -> Value {
        json!({"min":a,"max":b})
    }

    #[test]
    fn f17_within_window_closing_exactly_at_the_horizon_is_a_definite_miss_in_both_interpreters() {
        // within 8min: windows [0,8], [2,10], [4,12]. Hits at 5 <= 8 and
        // 10 <= 10. Case 3 finishes at 15 > D=12 and its window ends at
        // 4 + 8 = 12 <= D: the row is decided (probability 0), not [0,1].
        let src = day(
            "",
            12,
            30,
            "verify probability reached hit == true within 8min >= 1",
        );
        let m = selected(&src, "Day");
        let hand = json!([{"verdict":"violated","cases":[
            {"verdict":"satisfied","probability":p("1")},
            {"verdict":"satisfied","probability":p("1")},
            {"verdict":"violated","probability":p("0")}]}]);
        for (label, o) in both(&m) {
            assert!(!o.truncated, "{label}: D is not a computational cut");
            assert_eq!(props(&o), hand, "{label}");
            // Case 3 is cut off at D with work 12 - 10 = 2.
            assert_eq!(terminals(&o)[2][0]["kind"], "cut_off", "{label}");
            assert_eq!(terminals(&o)[2][0]["work"], rg(2, 2), "{label}");
        }
        let v =
            serde_json::to_value(report::check_with_selection(&src, None, Some("Day"))).unwrap();
        partial(
            &json!({"verdict":"violated","bounds":{"truncated":false},
                "properties":[{"verdict":"violated","cases":[
                    {"case_id":1,"verdict":"satisfied","probability":p("1")},
                    {"case_id":2,"verdict":"satisfied","probability":p("1")},
                    {"case_id":3,"verdict":"violated","probability":p("0")}]}]}),
            &v,
            "check",
        );
    }

    #[test]
    fn f17b_sampled_within_window_closing_exactly_at_the_horizon_is_a_miss_not_unknown() {
        // The same deterministic times in all 4 replications: case 3's window
        // [4,12] closes at D=12 without a hit, so 0 hits and 0 unknown. Three
        // rows, confidence 1/2: alpha 1/12, G(4;1/2)=1/16 < 1/12, so 4 hits
        // satisfy >= 0.5 and 4 misses violate it.
        let src = day(
            SIM,
            12,
            30,
            "verify probability reached hit == true within 8min >= 0.5",
        );
        let v =
            serde_json::to_value(report::simulate_with_selection(&src, None, Some("Day"))).unwrap();
        partial(
            &json!({"verdict":"violated","bounds":{"truncated":false},
                "properties":[{"verdict":"violated","cases":[
                    {"case_id":1,"verdict":"satisfied","hits":4,"unknown":0,"trials":4},
                    {"case_id":2,"verdict":"satisfied","hits":4,"unknown":0,"trials":4},
                    {"case_id":3,"verdict":"violated","hits":0,"unknown":0,"trials":4}]}],
                "simulation":{"truncated_replications":0,"cut_off_replications":4}}),
            &v,
            "simulate",
        );
    }

    fn clock_before_horizon() -> String {
        // Service 3: finishes 3, 6, 9. Clock 8 < horizon 20: the run stops at
        // the clock with case 3 running, a computational cut.
        day(SIM, 20, 8, "verify probability reached hit == true >= 0.5")
            .replace("fixed 5min", "fixed 3min")
    }

    #[test]
    fn f22c_clock_before_a_later_horizon_is_a_computational_cut_in_simulation() {
        let src = clock_before_horizon();
        assert!(src.contains("fixed 3min") && !src.contains("fixed 5min"));
        let v =
            serde_json::to_value(report::simulate_with_selection(&src, None, Some("Day"))).unwrap();
        partial(
            &json!({"verdict":"inconclusive","bounds":{"truncated":true},
                "properties":[{"verdict":"inconclusive","cases":[
                    {"case_id":1,"verdict":"satisfied","hits":4,"unknown":0},
                    {"case_id":2,"verdict":"satisfied","hits":4,"unknown":0},
                    {"case_id":3,"verdict":"inconclusive","hits":0,"unknown":4}]}],
                "case_outcomes":[
                    {"case_id":1,"unfinished_count":0,"cut_off_count":0},
                    {"case_id":2,"unfinished_count":0,"cut_off_count":0},
                    {"case_id":3,"terminals":[],"unfinished_count":4,"cut_off_count":0}],
                "simulation":{"truncated_replications":4,"cut_off_replications":0},
                "scenario":{"horizon":20,"aggregation":{"observed_until":
                    {"min":"8","max":"8","mean":"8","samples":4}}}}),
            &v,
            "simulate",
        );
    }

    #[test]
    fn g01c_reference_counts_only_computational_cuts_as_truncated_replications() {
        // scn-sim: D=8=clock, case 3 is cut off at the horizon in all 4
        // replications: a business cutoff, so 0 truncated. The clock-before-
        // horizon input stops at the clock 8 in all 4: 4 truncated.
        for (src, scenario, hand) in [
            (
                include_str!("../../../fixtures/scn-sim/model.relca").to_owned(),
                "Day",
                0,
            ),
            (clock_before_horizon(), "Day", 4),
        ] {
            let m = selected(&src, scenario);
            let e = engine::simulate(&m);
            let r = reference::simulate(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            assert_eq!(e.truncated_replications, Some(hand), "engine");
            assert_eq!(r.outcome.truncated_replications, Some(hand), "reference");
            assert_eq!(e.replications, r.outcome.replications);
        }
    }

    #[test]
    fn g03c_sampled_response_deadline_equal_to_the_horizon_is_violated() {
        // by 8: deadlines 8, 10, 12; finishes 5, 10, 15. Case 3 is unresolved
        // at D=12 and its deadline 12 <= D: an expiry violation in every
        // replication, never excused by the cutoff (P24).
        let src = day(SIM, 12, 30, BY8);
        let v =
            serde_json::to_value(report::simulate_with_selection(&src, None, Some("Day"))).unwrap();
        assert_eq!(v["verdict"], "violated", "{v}");
        let k = &v["properties"][0];
        assert_eq!(k["id"], "contract K #1 response");
        assert_eq!(k["verdict"], "violated");
        assert_eq!(k["cases"][2]["case_id"], 3);
        assert_eq!(k["cases"][2]["verdict"], "violated");
    }

    #[test]
    fn g03d_exhaustive_response_deadline_equal_to_the_horizon_is_violated_in_both_interpreters() {
        // Same times as g03c: cases 1 and 2 resolve at 5 <= 8 and 10 <= 10;
        // case 3 is pending at D=12 with deadline 12 <= D: violated.
        let src = day("", 12, 30, BY8);
        let m = selected(&src, "Day");
        for (label, o) in both(&m) {
            assert!(!o.truncated, "{label}");
            let k = &o.properties[0];
            let cases: Vec<_> = k
                .cases
                .as_ref()
                .unwrap()
                .iter()
                .map(|c| json!(c.verdict))
                .collect();
            assert_eq!(
                cases,
                [json!("satisfied"), json!("satisfied"), json!("violated")],
                "{label}"
            );
            assert_eq!(json!(k.verdict), "violated", "{label}");
        }
        let v =
            serde_json::to_value(report::check_with_selection(&src, None, Some("Day"))).unwrap();
        assert_eq!(v["verdict"], "violated");
        assert_eq!(v["bounds"]["truncated"], false);
    }

    const BRANCHES: &str = r#"model M version "0.2" {
type T = enum { ok }
observed case { hit: bool = false }
actor A {
  capacity 1
  capability run() -> T {
    duration {1min:0.25,2min:0.25,4min:0.5}
    result nondet { ok }
    writes hit
    finish { hit = true }
  }
}
process Flow { r = A.run(); end done }
cut C { include [A] }
policy fifo tie_by case_id
scenario Day {
  arrivals fixed [0min, 0min]
  horizon 5min
  bounds { cases 2; live_tasks 1; clock 30min }
}
verify probability reached hit == true >= 0.5
}
"#;

    #[test]
    fn g09_cutoff_row_work_spans_every_branch_and_observed_until_every_leaf_in_both_interpreters() {
        // Case 1 runs 0..d1, d1 in {1,2,4} (mass 1/4, 1/4, 1/2), all < D=5.
        // Case 2 runs d1..d1+d2. End at <= 5: (1,1)2 (1,2)3 (1,4)5 (2,1)3
        // (2,2)4 (4,1)5, mass 1/16+1/16+1/8+1/16+1/16+1/8 = 1/2, work d2 in
        // {1,4}. Cutoff at 5: (2,4) work 5-2=3 mass 1/8, (4,2) work 1 mass
        // 1/8, (4,4) work 1 mass 1/4: mass 1/2, work {1,3}. Leaves stop at
        // 2, 3, 5, 3, 4, 5 and three horizon leaves at 5: observed 2..5.
        let m = selected(BRANCHES, "Day");
        let hand = json!([
            [{"kind":"end","completed_at":rg(1,4),"elapsed":rg(1,4),"work":rg(1,4),"probability":p("1")}],
            [{"kind":"end","completed_at":rg(2,5),"elapsed":rg(2,5),"work":rg(1,4),"probability":p("1/2")},
             {"kind":"cut_off","completed_at":null,"elapsed":null,"work":rg(1,3),"probability":p("1/2")}]
        ]);
        for (label, o) in both(&m) {
            assert!(!o.truncated, "{label}");
            assert_eq!(terminals(&o), hand, "{label}");
            assert_eq!(
                observed_until(&o),
                json!({"min":"2","max":"5","reason":null}),
                "{label}"
            );
            // Case 2's unknown cutoff mass 1/2 leaves reached-hit in [1/2, 1].
            assert_eq!(
                props(&o)[0]["cases"][1]["probability"],
                json!({"min":"1/2","max":"1"}),
                "{label}"
            );
        }
        let v = serde_json::to_value(report::check_with_selection(BRANCHES, None, Some("Day")))
            .unwrap();
        partial(
            &json!({"bounds":{"truncated":false},"case_outcomes":[
                {"case_id":1,"terminals":[{"kind":"end","work":rg(1,4)}]},
                {"case_id":2,"terminals":[{"kind":"end","work":rg(1,4)},
                    {"kind":"cut_off","work":rg(1,3),"probability":p("1/2")}]}]}),
            &v,
            "check",
        );
    }

    #[test]
    fn g09c_observed_until_without_a_horizon_leaf_is_the_last_finish_in_both_interpreters() {
        // scn-drain: finishes 5, 10, 15 < D=20, so no leaf stops at the
        // horizon and every leaf stops at 15.
        let m = selected(
            include_str!("../../../fixtures/scn-drain/model.relca"),
            "Day",
        );
        for (label, o) in both(&m) {
            assert_eq!(
                observed_until(&o),
                json!({"min":"15","max":"15","reason":null}),
                "{label}"
            );
        }
    }

    #[test]
    fn g06c_declared_drain_assumption_starts_at_the_last_arrival() {
        // scn-drain: arrivals 0, 2, 4, horizon 20. Drain runs from the last
        // actual arrival 4 (not the first 0, not the right edge 6) to D.
        let v = serde_json::to_value(report::check_with_selection(
            include_str!("../../../fixtures/scn-drain/model.relca"),
            None,
            Some("Day"),
        ))
        .unwrap();
        let rows: Vec<_> = v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["field"] == "scenario.drain")
            .collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["value"], "4min..20min");
    }

    #[test]
    fn g04a_unselected_scenario_overlapping_the_model_policy_is_named_with_its_span() {
        // Spare repeats the model's policy. ADR 0042 checks every scenario,
        // selected or not, and forbids overriding a common item: the lowering
        // reports the overlap on Spare (lower.rs "{field} declared both in
        // model and scenario {name}" at the scenario span), and the effective
        // model then holds two policies ("duplicate policy" at Spare's line).
        let light = "scenario Light {\n  arrivals fixed_every 4min during [0min, 12min)\n  bounds { cases 3; live_tasks 1; clock 30min }\n}\n";
        let spare = "scenario Spare {\n  arrivals fixed_every 4min during [0min, 12min)\n  policy fifo tie_by case_id\n  bounds { cases 3; live_tasks 1; clock 30min }\n}\n";
        let src = format!(
            "{}{light}{spare}verify probability reached hit == true within 4min >= 1\n}}\n",
            BODY.replace("fixed 5min", "fixed 3min")
        );
        let at = src.find("scenario Spare").unwrap();
        let close = at + spare.len() - 1;
        let line = at + spare.find("policy fifo").unwrap();
        let ds = syntax::parse(&src)
            .and_then(|a| lower::lower_with_selection(&a, None, Some("Light")))
            .expect_err("an unselected overlap is still refused");
        let got: Vec<_> = ds
            .iter()
            .map(|d| {
                let s = d.span.unwrap();
                (d.code.as_str(), d.message.as_str(), s.start, s.end)
            })
            .collect();
        assert_eq!(
            got,
            [
                (
                    "U-SEM-0",
                    "policy declared both in model and scenario Spare",
                    at,
                    close
                ),
                (
                    "U-SEM-0",
                    "duplicate policy",
                    line,
                    line + "policy fifo tie_by case_id".len()
                ),
            ]
        );
    }

    #[test]
    fn f14b_simulation_compare_passes_the_selector_to_every_plan() {
        // Light arrivals 0, 4, 8. Slow (3min) finishes 3, 7, 11; Fast (1min)
        // finishes 1, 5, 9: elapsed 3 and 1 <= 4, all hits in 4 replications,
        // which satisfy >= 0.5 (G(4;1/2)=1/16 < 1/12).
        let src = r#"model M version "0.2" {
type T = enum { ok }
observed case { hit: bool = false }
actor A {
  capacity 1
  capability run() -> T {
    duration fixed 3min
    result nondet { ok }
    writes hit
    finish { hit = true }
  }
  capability fast() -> T { duration fixed 1min; result nondet { ok }; writes hit; finish { hit = true } }
}
process Q { q = A.fast(); end done }
process Flow { r = A.run(); end done }
cut C { include [A] }
policy fifo tie_by case_id
scenario Light {
  arrivals fixed_every 4min during [0min, 12min)
  bounds { cases 3; live_tasks 1; clock 30min }
}
scenario Busy {
  arrivals fixed_every 1min during [0min, 3min)
  bounds { cases 3; live_tasks 1; clock 30min }
}
configuration Slow { cut C; process Flow }
configuration Fast { cut C; process Q }
verify probability reached hit == true within 4min >= 0.5
simulation replications 4 seed 1 confidence 0.5
}
"#;
        let v = serde_json::to_value(compare::compare_with_scenario(
            src,
            &["Slow", "Fast"],
            compare::Analysis::Simulation,
            Some("Light"),
        ))
        .unwrap();
        let rows = json!([
            {"case_id":1,"arrival":0,"verdict":"satisfied","hits":4},
            {"case_id":2,"arrival":4,"verdict":"satisfied","hits":4},
            {"case_id":3,"arrival":8,"verdict":"satisfied","hits":4}]);
        let plan = |c: &str| {
            json!({"configuration":c,"report":{"verdict":"satisfied","scenario":{"name":"Light"},
                "properties":[{"verdict":"satisfied","cases":rows}]}})
        };
        partial(
            &json!({"verdict":"satisfied","scenario":"Light","analysis":"simulation",
                "plans":[plan("Slow"),plan("Fast")]}),
            &v,
            "compare",
        );
    }
}
