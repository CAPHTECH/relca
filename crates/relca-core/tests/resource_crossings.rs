//! Explicit cross-surface obligations; arithmetic is in resource_obligations.md.
#[path = "support/resource_acceptance.rs"]
mod hand;
use relca_core::{engine, explain, lower, reference, report, syntax};
use serde_json::{Value, json};
fn m(s: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(s).unwrap()).unwrap()
}
fn v(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
fn preflight(s: &str) {
    lower::lower(&syntax::parse(&hand::old_skeleton(s)).unwrap()).unwrap();
}
fn explanation(s: &str) -> Value {
    preflight(s);
    serde_json::to_value(explain::explain(s)).unwrap()
}
#[test]
fn refusal_and_queued_cancel_do_not_create_work_start_or_resource_acquisition() {
    for (name, binding, starts, work, refused, cancelled) in [
        ("rsrc-refusal", "x", "0", "0", Some("1"), "0"),
        ("rsrc-refusal.broken-enabled", "x", "1", "3", Some("0"), "0"),
        ("rsrc-queued-cancel", "b", "0", "0", None, "1"),
    ] {
        let r = v(&hand::source(name));
        let task = r["metrics"]["cases"][0]["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["binding"] == binding)
            .unwrap();
        assert_eq!(
            task["starts"],
            json!({"min":starts,"max":starts,"reason":null})
        );
        assert_eq!(task["work"], json!({"min":work,"max":work,"reason":null}));
        if let Some(count) = refused {
            assert_eq!(
                task["refused_count"],
                json!({"min":count,"max":count,"reason":null})
            );
        }
        assert_eq!(
            task["cancelled_before_start_count"]["stats"],
            json!({"min":cancelled,"max":cancelled,"reason":null})
        );
    }
}
#[test]
fn explain_cancel_snapshots_are_post_transition_and_request_is_not_release() {
    for (name, ack) in [("rsrc-cooperative", 3), ("rsrc-immediate", 2)] {
        let s = hand::edit(
            &hand::source(name),
            "  bounds",
            "  observed case { flag: bool = false }\n  contract C { invariant A.c.finish -> flag == true }\n  bounds",
        );
        let r = explanation(&s);
        let c = r["explanation"]["counterexamples"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "contract C #1 invariant")
            .unwrap();
        assert_eq!(c["replay"]["status"], "verified");
        let steps = c["steps"].as_array().unwrap();
        for (kind, binding, time, occupied, amount) in [
            ("cancel_request", "b", 2, 1, Some(1)),
            ("cancel_ack", "b", ack, 0, None),
            ("start", "c", ack, 2, Some(2)),
            ("finish", "c", ack + 1, 0, None),
        ] {
            let stage = steps
                .iter()
                .find(|s| s["kind"] == kind && s["binding"] == binding)
                .unwrap();
            assert_eq!(stage["time"], time);
            assert_eq!(stage["resource"]["actor"], "A");
            let shared = &stage["shared_resources"];
            assert_eq!(shared.as_array().unwrap().len(), 1);
            let r = &shared[0];
            assert_eq!(r["resource"], "R");
            assert_eq!(r["capacity"], 2);
            assert_eq!(r["occupied"], occupied);
            assert_eq!(
                r["holders"],
                match amount {
                    Some(n) => json!([{"case_id":1,"binding":binding,"amount":n}]),
                    None => json!([]),
                }
            );
        }
    }
}
#[test]
fn global_explain_holders_include_both_cases_sorted_by_case_and_binding() {
    let s = hand::edit(
        &hand::source("rsrc-global.broken-two-units"),
        "  bounds",
        "  observed case { flag: bool = false }\n  contract C { invariant A.x.finish -> flag == true }\n  bounds",
    );
    let r = explanation(&s);
    let c = r["explanation"]["counterexamples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "contract C #1 invariant")
        .unwrap();
    assert_eq!(c["replay"]["status"], "verified");
    assert!(c["steps"].as_array().unwrap().iter().any(|s|s["kind"]=="start"&&s["shared_resources"]==json!([{"resource":"R","capacity":2,"occupied":2,"holders":[{"case_id":1,"binding":"x","amount":1},{"case_id":2,"binding":"x","amount":1}]}])),"{c}");
}
#[test]
fn static_resource_diagnostics_pin_exact_code_count_and_source_byte_spans() {
    for (name, token, last) in [
        ("rsrc-collision.broken-same-name", "A", false),
        ("rsrc-zero-amount.broken-zero", "0", false),
        ("rsrc-unknown.broken-unknown", "Missing", false),
        ("rsrc-duplicate.broken-duplicate", "R", true),
        ("rsrc-second-use.broken-second-use", "uses R: 1", true),
    ] {
        let s = hand::source(name);
        let v = v(&s);
        let ds = v["diagnostics"].as_array().unwrap();
        assert_eq!(ds.len(), 1, "{v}");
        assert_eq!(
            ds[0]["code"],
            hand::expected(name)["diagnostics"][0]["code"]
        );
        let start = if name.starts_with("rsrc-collision") {
            s.find("resource A").unwrap() + "resource ".len()
        } else if name.starts_with("rsrc-zero-amount") {
            s.find("uses R: 0").unwrap() + "uses R: ".len()
        } else if last {
            s.rfind(token).unwrap()
        } else {
            s.find(token).unwrap()
        };
        assert_eq!(
            ds[0]["span"],
            json!({"start":start,"end":start+token.len()}),
            "{name}"
        );
    }
}
#[test]
fn range_stop_and_state_budget_withdraw_all_resource_scalar_claims() {
    let s = hand::edit(
        &hand::source("rsrc-over-demand"),
        "  bounds",
        "  observed case { n: 0..0 = 0 }\n  bounds",
    );
    let s = hand::edit(
        &s,
        "result nondet { ok };",
        "result nondet { ok }; start { n += 1 };",
    );
    preflight(&s);
    let r = v(&s);
    assert_eq!(r["verdict"], "violated");
    for key in ["occupancy_time", "peak_units", "utilization"] {
        assert_eq!(
            r["metrics"]["resources"][0][key],
            json!({"min":null,"max":null,"reason":"range_stopped"})
        );
    }
    let model = m(&hand::source("rsrc-exact"));
    let o = engine::explore_with_budget(&model, 1);
    assert!(o.truncated);
    let metrics = serde_json::to_value(o.metrics.unwrap()).unwrap();
    for key in ["occupancy_time", "peak_units", "utilization"] {
        assert_eq!(
            metrics["resources"][0][key],
            json!({"min":null,"max":null,"reason":"truncated"})
        );
    }
}
const CONTEST: &str = r#"model ContestResource version "0.2" {
type T = enum { ok }
resource R capacity 1
people [j1,j2,p]
actor Judge { executors [j1,j2]; capability work() -> T { uses R: 1; duration fixed 2min; result nondet { ok } } }
actor Route { executors [p]; capability respond() -> T { uses R: 1; duration fixed 1min; result nondet { ok } } }
actor Party { control party; standing over [end.done] basis "claim" }
permit j1 to Judge.work
permit j2 to Judge.work
permit p to Route.respond
authority G { basis "decision"; grants j1 to Judge.work; grants j2 to Judge.work }
process P { x = Judge.work(); end done }
cut K { include [Judge,Route,Party] }
arrivals fixed [0min,0min]
policy fifo tie_by case_id
bounds { cases 2; live_tasks 3; clock 10min }
metrics { window 10min; currency "JPY" }
verify safety no_resource_overcommit
verify reachable end.done
contract C { contest Party on end.done route Route.respond reversible_by [j1] respond_by 8min }
}"#;
#[test]
fn contest_route_resource_work_counts_globally_after_business_terminal_freezes() {
    preflight(CONTEST);
    for (s, high, case2) in [
        (CONTEST.to_owned(), "6", 5),
        (
            hand::edit(CONTEST, "duration fixed 1min", "duration fixed 2min"),
            "8",
            6,
        ),
    ] {
        let model = m(&s);
        let e = engine::explore(&model);
        let mut r = reference::explore(&model);
        assert_eq!(r.stop, reference::StopReason::Complete);
        r.outcome.diagnostics.drain(..model.diagnostics.len());
        assert_eq!(e, r.outcome);
        let r = v(&s);
        // The process has no result-dependent guard, so the static contest
        // property finds no decision to reverse (ADR 0018, reason no_decision)
        // and is violated, as it is without resources.
        assert_eq!(r["verdict"], "violated", "{r}");
        let verdicts: Vec<(String, String)> = r["properties"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["id"].as_str().unwrap().to_owned(),
                    p["verdict"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(
            verdicts,
            [
                ("safety no_resource_overcommit", "satisfied"),
                ("reachable end.done", "satisfied"),
                ("contract C #1 contest", "violated"),
                ("contract C #1 contest_response", "satisfied"),
            ]
            .map(|(i, v)| (i.to_owned(), v.to_owned()))
        );
        let row = &r["metrics"]["resources"][0];
        assert_eq!(
            row["occupancy_time"],
            json!({"min":"4","max":high,"reason":null})
        );
        assert_eq!(
            row["peak_units"],
            json!({"min":"1","max":"1","reason":null})
        );
        let cs = r["case_outcomes"].as_array().unwrap();
        assert_eq!(cs[0]["terminals"][0]["elapsed"], json!({"min":2,"max":2}));
        assert_eq!(
            cs[1]["terminals"][0]["elapsed"],
            json!({"min":4,"max":case2})
        );
        for c in cs {
            assert_eq!(
                c["terminals"][0]["work"],
                json!({"min":2,"max":2}),
                "route work must not thaw business work"
            );
        }
    }
}

#[test]
fn every_additional_old_language_control_is_well_formed_before_resource_implementation() {
    preflight(CONTEST);
    preflight(&hand::edit(
        CONTEST,
        "duration fixed 1min",
        "duration fixed 2min",
    ));
    for name in [
        "rsrc-cooperative",
        "rsrc-immediate",
        "rsrc-global.broken-two-units",
    ] {
        let target = if name.starts_with("rsrc-global") {
            "A.x"
        } else {
            "A.c"
        };
        let s = hand::edit(
            &hand::source(name),
            "  bounds",
            &format!(
                "  observed case {{ flag: bool = false }}\n  contract C {{ invariant {target}.finish -> flag == true }}\n  bounds"
            ),
        );
        preflight(&s);
    }
    for duration in ["duration { 3min:1 }", "duration { 1min:0.5,3min:0.5 }"] {
        preflight(&hand::edit(
            &hand::source("rsrc-weighted"),
            "duration fixed 3min",
            duration,
        ));
    }
    let s = hand::edit(
        &hand::source("rsrc-over-demand"),
        "  bounds",
        "  observed case { n: 0..0 = 0 }\n  bounds",
    );
    preflight(&hand::edit(
        &s,
        "result nondet { ok };",
        "result nondet { ok }; start { n += 1 };",
    ));
    let s = hand::edit(
        &hand::source("rsrc-global"),
        "  bounds",
        "  configuration Before { cut K; process P }\n  configuration After { cut K; process P }\n  bounds",
    );
    let a = syntax::parse(&hand::old_skeleton(&s)).unwrap();
    lower::lower_configuration(&a, "Before").unwrap();
    lower::lower_configuration(&a, "After").unwrap();
    let s = hand::edit(
        &hand::source("rsrc-global"),
        "  bounds",
        "  simulation replications 2 seed 1 confidence 0.95\n  bounds",
    );
    preflight(&s);
}

#[test]
fn all_resource_syntax_limits_and_unselected_uses_fail_closed_with_exact_classification() {
    let base = hand::source("rsrc-over-demand");
    for (from, to, code) in [
        ("uses R: 1", "uses", "E-SYN-0"),
        ("uses R: 1", "uses R 1", "E-SYN-0"),
        ("uses R: 1", "uses R:", "E-SYN-0"),
        ("uses R: 1", "uses R: 1,", "E-SYN-0"),
        ("uses R: 1", "uses R: -1", "E-SYN-0"),
        ("uses R: 1", "uses R: 18446744073709551616", "U-SYN-0"),
        ("uses R: 1", "uses R: 1.5", "U-SYN-0"),
        ("uses R: 1", "uses R: 1min", "U-SYN-0"),
        ("uses R: 1", "uses R: 1+1", "U-SYN-0"),
        ("R capacity 1", "R capacity", "E-SYN-0"),
        ("R capacity 1", "R capacity -1", "E-SYN-0"),
        ("R capacity 1", "R capacity 18446744073709551616", "U-SYN-0"),
    ] {
        let s = hand::edit(&base, from, to);
        let r = v(&s);
        assert_eq!(r["diagnostics"].as_array().unwrap().len(), 2, "{r}");
        assert_eq!(r["diagnostics"][0]["code"], code, "{to}: {r}");
        assert_eq!(r["diagnostics"][1]["code"], "W-CUT-0");
        assert_eq!(
            r["verdict"],
            if code == "E-SYN-0" {
                "invalid"
            } else {
                "unsupported"
            }
        );
    }
    let s = hand::edit(
        &base,
        "actor A {",
        "actor A { capability dormant() -> T { uses Missing: 1; duration fixed 1min; result nondet { ok } }",
    );
    let r = v(&s);
    assert_eq!(r["verdict"], "invalid");
    assert_eq!(r["diagnostics"][0]["code"], "E-NAME-0");
}

#[test]
fn forward_resources_reversed_use_order_and_capacity_without_metrics_have_the_same_meaning() {
    let base = hand::source("rsrc-atomic");
    let moved = hand::edit(&base, "resource R capacity 1; resource S capacity 1", "");
    let moved = hand::edit(
        &moved,
        "process P",
        "resource R capacity 1; resource S capacity 1; process P",
    );
    let reversed = hand::edit(&base, "uses S: 1, R: 1", "uses R: 1, S: 1");
    let old = v(&base);
    for s in [&moved, &reversed] {
        preflight(s);
        let r = v(s);
        assert_eq!(r["verdict"], "satisfied");
        assert_eq!(r["terminals"], old["terminals"]);
        assert_eq!(r["metrics"]["resources"], old["metrics"]["resources"]);
    }
    let s = hand::edit(&base, "metrics { window 10min; currency \"JPY\" }", "");
    let r = v(&s);
    assert_eq!(r["terminals"], old["terminals"]);
    assert!(r.get("metrics").is_none());
    assert_eq!(r["verdict"], "satisfied");
}
