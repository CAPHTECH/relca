//! ADR 0022 D2: hand arithmetic, independent of the runtime implementation.
use relca_core::report;
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::path::PathBuf;

pub const NAMES: [&str; 12] = [
    "ctx-contest",
    "ctx-contest-branches",
    "ctx-contest-branches.broken-zero-capacity",
    "ctx-contest-clock",
    "ctx-contest-clock.broken-short-clock",
    "ctx-contest-metrics",
    "ctx-contest-metrics.broken-missing-route-rate",
    "ctx-contest-origin",
    "ctx-contest-origin.broken-arrival-clock",
    "ctx-contest-resource",
    "ctx-contest-resource.broken-slow-route",
    "ctx-contest.broken-late",
];
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(root().join(name).join("expected.json")).unwrap())
        .unwrap()
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                let actual = a.get(k).unwrap_or_else(|| panic!("missing {k}: {a}"));
                if k == "alternatives" {
                    let actual = actual.as_array().unwrap();
                    let expected = v.as_array().unwrap();
                    assert_eq!(expected.len(), actual.len(), "alternative inventory");
                    // These hand alternatives have unique filed/status/result/time tuples.
                    for hand in expected {
                        let found = actual.iter().find(|x| {
                            hand.as_object()
                                .unwrap()
                                .iter()
                                .all(|(k, v)| x.get(k) == Some(v))
                        });
                        assert!(
                            found.is_some(),
                            "missing hand alternative {hand}: {actual:?}"
                        );
                    }
                } else {
                    subset(v, actual);
                }
            }
        }
        Value::Array(es) => {
            let actual = a.as_array().unwrap_or_else(|| panic!("not array: {a}"));
            assert_eq!(es.len(), actual.len(), "exact inventory: {a}");
            for (x, y) in es.iter().zip(actual) {
                subset(x, y);
            }
        }
        _ => assert_eq!(e, a),
    }
}

#[test]
fn twelve_reports_have_static_then_runtime_rows_and_frozen_business_end() {
    for name in NAMES {
        let actual = serde_json::to_value(report::check(&source(name))).unwrap();
        subset(&expected(name), &actual);
        let rows = actual["contest_runtime"]
            .as_array()
            .expect("D2 runtime table");
        assert!(!rows.is_empty(), "{name}");
        for row in rows {
            let alts = row["alternatives"].as_array().unwrap();
            assert!(
                alts.iter().any(|a| a["filed"] == false),
                "declined branch {name}"
            );
            assert!(
                alts.iter().any(|a| a["filed"] == true),
                "filed branch {name}"
            );
            for alt in alts {
                if alt["filed"] == false {
                    assert_eq!(alt["status"], "not_filed");
                    for key in ["filed_at", "deadline", "response_at", "executor", "result"] {
                        assert!(alt[key].is_null(), "not filed must not invent {key}");
                    }
                } else if let Some(response) = alt["response_at"].as_u64() {
                    let duration = if name == "ctx-contest.broken-late"
                        || name == "ctx-contest-resource.broken-slow-route"
                    {
                        3
                    } else {
                        2
                    };
                    let wait = response - alt["target_at"].as_u64().unwrap() - duration;
                    assert_eq!(
                        alt["route_work"],
                        json!({"min":duration,"max":duration}),
                        "{name}: actual route work"
                    );
                    assert_eq!(
                        alt["route_wait"],
                        json!({"min":wait,"max":wait}),
                        "{name}: actual shared-resource wait"
                    );
                    if alt["status"] == "violated" {
                        assert_eq!(alt["violation_reason"], "late_response");
                    }
                    if name == "ctx-contest-metrics" {
                        assert_eq!(
                            alt["route_cost"],
                            json!({"min":"2","max":"2","reason":null})
                        );
                    }
                    if name == "ctx-contest-metrics.broken-missing-route-rate" {
                        assert_eq!(
                            alt["route_cost"],
                            json!({"min":null,"max":null,"reason":"not_configured"})
                        );
                    }
                } else if name == "ctx-contest-branches.broken-zero-capacity" {
                    assert_eq!(alt["violation_reason"], "deadlock");
                }
            }
        }
    }
}

fn check(src: &str) -> Value {
    serde_json::to_value(report::check(src)).unwrap()
}
fn contract(v: &Value, suffix: &str) -> String {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"].as_str().unwrap().ends_with(suffix))
        .unwrap()["verdict"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn simulation(src: &str, seed: u64) -> String {
    src.replacen(
        "\n}",
        &format!("\n  simulation replications 2 seed {seed} confidence 0.5\n}}"),
        1,
    )
}

#[test]
fn all_twelve_old_syntax_skeletons_have_legal_bounds_warnings_and_end_arithmetic() {
    for name in NAMES {
        let src = source(name)
            .lines()
            .filter(|l| !l.trim_start().starts_with("contract Controls"))
            .collect::<Vec<_>>()
            .join("\n");
        let a = check(&src);
        assert_eq!(
            a["verdict"], "satisfied",
            "old syntax preflight {name}: {a}"
        );
        assert_eq!(
            a["bounds"]["truncated"], false,
            "ordinary process finishes before clock {name}"
        );
        // Removing Controls also removes the only route call. Judge only
        // returns fail: else/posted are dead. p belongs only to Route;
        // the branches pair instead uses anonymous route capacity.
        let mut codes = vec!["W-CONTEST-0", "W-UNREACH-0", "W-UNREACH-1", "W-UNREACH-2"];
        if !name.starts_with("ctx-contest-branches") {
            codes.push("W-UNREACH-3");
        }
        assert_eq!(
            a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d["code"].as_str().unwrap())
                .collect::<Vec<_>>(),
            codes,
            "{name}"
        );
        assert_eq!(
            a["diagnostics"][1]["message"],
            "process Flow: Route.respond not_called"
        );
        assert_eq!(
            a["diagnostics"][2]["message"],
            "process Flow: if else arm is never taken"
        );
        assert_eq!(
            a["diagnostics"][3]["message"],
            "process Flow: end.posted is never reached"
        );
        if codes.len() == 5 {
            assert_eq!(
                a["diagnostics"][4]["message"],
                "process Flow: executor p is never assigned"
            );
        }
        assert!(a.get("contest_runtime").is_none());
        assert!(a.get("obligations").is_none());
        subset(&expected(name)["terminals"], &a["terminals"]);
    }
}

#[test]
fn six_pairs_are_one_explicit_source_edit() {
    for (good, bad, from, to) in [
        (
            "ctx-contest",
            "ctx-contest.broken-late",
            "duration fixed 2min",
            "duration fixed 3min",
        ),
        (
            "ctx-contest-clock",
            "ctx-contest-clock.broken-short-clock",
            "clock 3min",
            "clock 2min",
        ),
        (
            "ctx-contest-origin",
            "ctx-contest-origin.broken-arrival-clock",
            "respond_by 2min",
            "respond_by 1min",
        ),
        (
            "ctx-contest-resource",
            "ctx-contest-resource.broken-slow-route",
            "duration fixed 2min",
            "duration fixed 3min",
        ),
        (
            "ctx-contest-metrics",
            "ctx-contest-metrics.broken-missing-route-rate",
            "rate Route per_minute 1; ",
            "",
        ),
        (
            "ctx-contest-branches",
            "ctx-contest-branches.broken-zero-capacity",
            "capacity 1",
            "capacity 0",
        ),
    ] {
        assert_eq!(source(good).matches(from).count(), 1, "{good}");
        assert_eq!(source(good).replacen(from, to, 1), source(bad), "{bad}");
    }
}

#[test]
fn a6_unreached_contest_is_vacuous_and_simulation_refuses_only_on_encounter() {
    let src = source("ctx-contest");
    let unreachable = src.replace("result nondet { fail }", "result nondet { pass }");
    let a = check(&unreachable);
    assert_eq!(a["verdict"], "vacuous");
    assert_eq!(contract(&a, " contest"), "satisfied");
    assert_eq!(contract(&a, " contest_response"), "vacuous");
    assert_eq!(a["contest_runtime"][0]["alternatives"], json!([]));
    for seed in [0, 1, 999] {
        let unseen =
            serde_json::to_value(report::simulate(&simulation(&unreachable, seed))).unwrap();
        assert_ne!(
            unseen["verdict"], "unsupported",
            "unreached contest must not refuse: {unseen}"
        );
        let seen = serde_json::to_value(report::simulate(&simulation(&src, seed))).unwrap();
        assert_eq!(
            seen["verdict"], "unsupported",
            "never sample filed/declined with seed"
        );
        assert!(
            seen["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SEM-0")
        );
    }
}

#[test]
fn a7_route_parameters_cut_zero_deadline_and_actual_executor_permission() {
    let src = source("ctx-contest");
    let parameterized = src.replace("capability respond()", "capability respond(x:T)");
    let unavailable = check(&parameterized);
    assert_eq!(
        unavailable["verdict"], "unsupported",
        "filed route cannot invent an argument"
    );
    assert_eq!(contract(&unavailable, " contest"), "satisfied");
    assert!(
        unavailable["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0")
    );
    let unreachable = parameterized.replace("result nondet { fail }", "result nondet { pass }");
    assert_eq!(
        check(&unreachable)["verdict"],
        "vacuous",
        "unused route parameters are not encountered"
    );
    let excluded = src.replace(
        "include [Judge,Route,Party]",
        "include [Judge,Party]; exclude Route reason \"external\" loses \"response\"",
    );
    let a = check(&excluded);
    assert_eq!(contract(&a, " contest"), "violated");
    assert_eq!(contract(&a, " contest_response"), "violated");
    assert!(
        a["contest_runtime"][0]["alternatives"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["violation_reason"] == "missing_route")
    );
    let zero = check(&src.replace("respond_by 2min", "respond_by 0min"));
    assert_eq!(contract(&zero, " contest"), "violated");
    assert_eq!(contract(&zero, " contest_response"), "violated");
    let permission = src.replace("people [j,p]", "people [j,p,q]").replace(
        "actor Route { executors [p]",
        "actor Route { executors [p,q]",
    );
    let a = check(&permission);
    assert_eq!(contract(&a, " contest"), "satisfied");
    assert_eq!(contract(&a, " contest_response"), "violated");
    let alternatives = a["contest_runtime"][0]["alternatives"].as_array().unwrap();
    assert!(
        alternatives
            .iter()
            .any(|a| a["executor"] == "p" && a["status"] == "responded")
    );
    assert!(
        alternatives
            .iter()
            .any(|a| a["executor"] == "q" && a["status"] == "violated"),
        "do not prune unauthorized allocations"
    );
}

#[test]
fn new_assumptions_and_unchecked_distinguish_model_runtime_from_real_world() {
    let a = check(&source("ctx-contest"));
    for (field, value) in [
        ("contest.occurrence", "nondeterministic_at_target_end"),
        (
            "contest.response",
            "route_normal_finish_by_target_time_plus_deadline",
        ),
        (
            "contest.closure",
            "business_end_distinct_from_execution_close",
        ),
        (
            "contest.scope",
            "model_structure_and_runtime_not_acceptance",
        ),
        ("obligations.scope", "per_case_response_instances"),
        ("obligations.outcomes", "possible_outcomes_not_path_counts"),
    ] {
        assert!(
            a["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["field"] == field && r["value"] == value),
            "{field}: {a}"
        );
    }
    for word in [
        "contest_real_world_occurrence",
        "contest_real_world_response_time",
        "contest_acceptance",
        "contest_remedy",
        "contest_route_handoff",
        "contest_decision_coverage",
    ] {
        assert!(
            a["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!(word)),
            "{word}"
        );
    }
}

fn row<'a>(v: &'a Value, field: &str, key: &str, name: &str) -> &'a Value {
    v[field]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r[key] == name)
        .unwrap()
}
fn scalar(a: &Value, min: &str, max: &str) {
    subset(&json!({"min":min,"max":max,"reason":null}), a);
}

#[test]
fn f11_route_work_people_and_cost_do_not_change_terminal_snapshot() {
    for name in [
        "ctx-contest-metrics",
        "ctx-contest-metrics.broken-missing-route-rate",
    ] {
        let a = check(&source(name));
        let m = &a["metrics"];
        scalar(&m["overall"]["work"], "1", "3");
        scalar(&m["overall"]["named_people_work"], "1", "3");
        let route = row(m, "actors", "actor", "Route");
        scalar(&route["work"], "0", "2");
        scalar(&route["utilization"], "0", "1/5");
        scalar(&row(m, "people", "person", "p")["work"], "0", "2");
        scalar(
            &row(m, "terminals", "end", "returned")["cost_per_case"],
            "1",
            "1",
        );
        if name.contains("broken") {
            subset(
                &json!({"min":null,"max":null,"reason":"not_configured"}),
                &m["overall"]["observed_cost_total"],
            );
        } else {
            scalar(&m["overall"]["observed_cost_total"], "1", "3");
        }
    }
}

#[test]
fn n3_end_closes_old_instances_but_route_can_create_new_instances_until_execution_close() {
    let src=source("ctx-contest").replace("contract Controls {", "contract Controls { response old: case.received -> one_of(Route.respond.finish) by 9min after case.received otherwise unresolved; response later: Route.respond.start -> one_of(Route.respond.finish) by 2min after Route.respond.start otherwise unresolved;");
    let a = check(&src);
    let obligations = a["obligations"].as_array().unwrap();
    let old = obligations
        .iter()
        .find(|r| r["response_label"] == "old")
        .unwrap();
    assert!(
        old["outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|o| o["status"] == "violated"
                && o["reason"] == "terminal_unresolved"
                && o["resolved_at"] == 1)
    );
    let later = obligations
        .iter()
        .find(|r| r["response_label"] == "later")
        .unwrap();
    assert!(
        later["outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|o| o["status"] == "fulfilled"
                && o["triggered_at"] == 1
                && o["deadline"] == 3
                && o["resolved_at"] == 3)
    );
    assert_ne!(old["id"], later["id"]);
}

#[test]
fn diff_compare_transport_is_not_an_obligation_or_runtime_table_diff() {
    for name in ["ctx-contest", "ctx-waive"] {
        let mut src = source(name);
        if name == "ctx-contest" {
            src=src.replace("  bounds", "  configuration A { cut C; process Flow }\n  configuration B { cut C; process Flow }\n  bounds");
        } else {
            src=src.replace("  bounds", "  cut Scope { include [Officer,B,Wait] }\n  configuration A { cut Scope; process Flow }\n  configuration B { cut Scope; process Flow }\n  bounds");
        }
        let d = serde_json::to_value(relca_core::diff::diff(&src, "A", "B")).unwrap();
        let c = serde_json::to_value(relca_core::compare::compare(
            &src,
            &["A", "B"],
            relca_core::compare::Analysis::Exhaustive,
        ))
        .unwrap();
        for value in [&d, &c] {
            assert!(
                value["unchecked"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("obligation_tables")),
                "{name}: {value}"
            );
            assert_eq!(
                value["unchecked"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("contest_runtime_tables")),
                name == "ctx-contest"
            );
        }
    }
    let src = source("resp-diff")
        .lines()
        .filter(|l| !l.trim_start().starts_with("contest Vendor"))
        .collect::<Vec<_>>()
        .join("\n");
    let d = serde_json::to_value(relca_core::diff::diff(&src, "A", "B")).unwrap();
    let c = serde_json::to_value(relca_core::compare::compare(
        &src,
        &["A", "B"],
        relca_core::compare::Analysis::Exhaustive,
    ))
    .unwrap();
    for value in [&d, &c] {
        for word in ["obligation_tables", "contest_runtime_tables"] {
            assert!(
                !value["unchecked"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(word)),
                "legacy changed"
            );
        }
    }
}

#[test]
fn a8_runtime_explanation_replays_and_has_authored_clause_location() {
    for (name, line) in [
        ("ctx-contest.broken-late", 14),
        ("ctx-contest-branches.broken-zero-capacity", 13),
    ] {
        let src = source(name);
        let a = serde_json::to_value(relca_core::explain::explain(&src)).unwrap();
        let mut projection = a.clone();
        projection.as_object_mut().unwrap().remove("explanation");
        assert_eq!(
            ledger_shape::original_report(&projection),
            ledger_shape::original_report(&check(&src)),
            "explain must not change root or raw tables"
        );
        let cs = a["explanation"]["counterexamples"].as_array().unwrap();
        let c = cs
            .iter()
            .find(|c| c["id"] == "contract Controls #1 contest_response")
            .expect("runtime counterexample");
        assert_eq!(c["status"], "verified");
        assert_eq!(c["replay"]["status"], "verified");
        let location = &c["property_location"];
        assert_eq!(location["line"], line);
        assert_eq!(location["column"], 23);
        let text =
            "contest Party on end.returned route Route.respond reversible_by [j] respond_by 2min";
        let start = src.find(text).unwrap();
        assert_eq!(
            location["span"],
            json!({"start":start,"end":start+text.len()})
        );
        assert!(
            !cs.iter().any(|c| c["id"] == "contract Controls #1 contest"),
            "static contest is omitted, not replayed"
        );
    }
}

#[test]
fn a9_clock_cut_keeps_business_reachability_but_nulls_all_new_metric_extrema() {
    let src = source("ctx-contest-metrics")
        .replace("clock 10min", "clock 2min")
        .replace(
            "verify safety",
            "verify reachable end.returned\n  verify safety",
        );
    let a = check(&src);
    assert_eq!(contract(&a, "contest_response"), "inconclusive");
    assert!(
        a["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "reachable end.returned" && p["verdict"] == "satisfied")
    );
    assert_eq!(a["metrics"]["status"], "truncated");
    fn null_extrema(v: &Value) {
        if let Some(o) = v.as_object() {
            if o.contains_key("min") && o.contains_key("max") {
                assert_eq!(v["min"], Value::Null);
                assert_eq!(v["max"], Value::Null);
                assert_eq!(v["reason"], "truncated");
            } else {
                for child in o.values() {
                    null_extrema(child);
                }
            }
        } else if let Some(a) = v.as_array() {
            for child in a {
                null_extrema(child);
            }
        }
    }
    null_extrema(&a["metrics"]);
    subset(
        &json!([{"end":"returned","elapsed":{"min":1,"max":1},"work":{"min":1,"max":1},"result_at":null}]),
        &a["terminals"],
    );
}

#[test]
fn a9_post_end_route_uses_shared_person_and_delays_a_later_arrival() {
    // Same p executes Judge and Route. Case 1 ends at 1. When it files,
    // route occupies p until3, so case2 arriving2 starts3, not2. Without
    // that filing case2 starts2. These are schedule alternatives, not rates.
    let src = source("ctx-contest-metrics")
        .replace("people [j,p]", "people [p]")
        .replace("executors [j]", "executors [p]")
        .replace("permit j to Judge.judge", "permit p to Judge.judge")
        .replace("grants j to Judge.judge", "grants p to Judge.judge")
        .replace("reversible_by [j]", "reversible_by [p]")
        .replace(
            "  bounds { cases 1",
            "  arrivals fixed [0min,2min]\n  policy fifo tie_by case_id\n  bounds { cases 2",
        );
    let a = check(&src);
    assert_eq!(a["verdict"], "satisfied", "{a}");
    let cases = a["case_outcomes"].as_array().unwrap();
    subset(
        &json!({"completed_at":{"min":1,"max":1},"elapsed":{"min":1,"max":1},"work":{"min":1,"max":1}}),
        &cases[0]["terminals"][0],
    );
    subset(
        &json!({"completed_at":{"min":3,"max":4},"elapsed":{"min":1,"max":2},"work":{"min":1,"max":1}}),
        &cases[1]["terminals"][0],
    );
    let m = &a["metrics"];
    scalar(&m["cases"][1]["first_start_wait"], "0", "1");
    scalar(&m["overall"]["work"], "2", "6");
    scalar(&m["overall"]["observed_cost_total"], "2", "6");
    scalar(&row(m, "people", "person", "p")["work"], "2", "6");
    scalar(&row(m, "people", "person", "p")["peak_tasks"], "1", "1");
    scalar(
        &row(m, "terminals", "end", "returned")["cost_per_case"],
        "1",
        "1",
    );
}
