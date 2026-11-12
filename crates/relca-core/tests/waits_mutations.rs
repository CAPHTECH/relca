//! ADR0048 survivor regressions. Expectations were derived first in
//! support/waits_mutation_calc.md. Inline inputs do not grow the fixture census.
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

pub fn scalar(x: &str) -> Value {
    span(x, x)
}
fn span(lo: &str, hi: &str) -> Value {
    json!({"min":lo,"max":hi,"reason":null})
}
fn partition(service: Value, actor: Value, route: Value, intentional: Value) -> Value {
    json!({"external":scalar("0"),"in_service":service,"contest_route":route,
        "queue":{"actor_capacity":actor,"resource":scalar("0"),"executor":scalar("0"),"combined":scalar("0")},
        "intentional":intentional,"unattributed":scalar("0")})
}
fn base(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
fn replace_once(s: &str, old: &str, new: &str) -> String {
    assert_eq!(s.matches(old).count(), 1, "closed inline-input edit: {old}");
    s.replace(old, new)
}

pub const IDS: [&str; 8] = ["F15a", "F26", "A01a", "A01b", "A05b", "A23", "A10", "A12b"];

pub fn diagnostic_codes(id: &str) -> Vec<&'static str> {
    if matches!(id, "A01a" | "A01b") {
        vec!["W-RESP-0"]
    } else {
        vec![]
    }
}

pub fn diagnostics(id: &str, v: &Value) {
    let rows = v["diagnostics"].as_array().unwrap();
    let mut codes: Vec<_> = rows
        .iter()
        .map(|d| {
            assert_eq!(d["severity"], "warning", "{id}: expected warnings only");
            d["code"].as_str().unwrap()
        })
        .collect();
    codes.sort_unstable();
    assert_eq!(
        codes,
        diagnostic_codes(id),
        "{id}: closed diagnostic multiset"
    );
}

pub fn input(id: &str) -> (String, Option<&'static str>, &'static str, i32) {
    match id {
        "F15a" => (
            replace_once(&base("wait-person"), "a = A.a(); b = B.b()", "z = A.a(); a = B.b()"),
            None, "satisfied", 0,
        ),
        "F26" => (r#"model ExactHorizon version "0.2" {
            type T = enum { ok }
            actor A { capacity 1; capability run() -> T { duration fixed 5min; result nondet { ok } } }
            process P { r = A.run(); end done }
            cut C { include [A] }
            policy fifo tie_by case_id
            scenario Day { arrivals fixed [0min, 0min]; horizon 10min; bounds { cases 2; live_tasks 1; clock 20min } }
            metrics { window 20min; currency "JPY"; breakdown waits }
            verify reachable end.done
        }"#.into(), Some("Day"), "satisfied", 0),
        "A01a" => (
            replace_once(&base("wait-contest"), "wait 5min", "fin = Approver.approve()"),
            None, "satisfied", 0,
        ),
        "A01b" => {
            let s = replace_once(&base("wait-contest"), "duration fixed 1min; result nondet { pass, fail }", "duration fixed 4min; result nondet { pass, fail }");
            let s = replace_once(&s, "bounds { cases 1; live_tasks 2; clock 20min }", "arrivals fixed [0min, 0min]; policy fifo tie_by case_id; bounds { cases 2; live_tasks 4; clock 40min }");
            let s = replace_once(&s, "window 20min", "window 40min");
            (s, None, "violated", 1)
        },
        "A05b" => {
            let s = replace_once(&base("wait-combined"), "arrivals fixed [0min, 0min]", "arrivals fixed [0min, 0min, 1min]");
            let s = replace_once(&s, "cases 2;", "cases 3;");
            (s, None, "satisfied", 0)
        },
        "A23" => (r#"model SpareResource version "0.2" {
            type T = enum { ok }
            resource Desk capacity 2;
            actor A { capacity 1; capability run() -> T { uses Desk:1; duration fixed 3min; result nondet { ok } } }
            process P { r = A.run(); end done }
            cut C { include [A] }
            arrivals fixed [0min, 0min]
            policy fifo tie_by case_id
            bounds { cases 2; live_tasks 2; clock 20min }
            metrics { window 20min; currency "JPY"; breakdown waits }
            verify reachable end.done
            verify safety no_resource_overcommit
        }"#.into(), None, "satisfied", 0),
        "A10" => (r#"model EarlyTerminal version "0.2" {
            type T = enum { ok }
            actor A { capacity 1; capability run() -> T { duration fixed 2min; result nondet { ok } } }
            process P { r = A.run(); end done }
            cut C { include [A] }
            policy fifo tie_by case_id
            scenario Day { arrivals fixed [0min, 0min, 0min]; horizon 3min; bounds { cases 3; live_tasks 1; clock 20min } }
            metrics { window 20min; currency "JPY"; breakdown waits }
            verify reachable end.done
        }"#.into(), Some("Day"), "inconclusive", 2),
        "A12b" => (r#"model RejectedNotCutOff version "0.2" {
            type T = enum { ok }
            actor A { capacity 1; capability run() -> T { duration fixed 5min; result nondet { ok } } }
            process P { r = A.run(); end done }
            cut C { include [A] }
            policy fifo tie_by case_id
            queue capacity 1 on_overflow reject
            scenario Day { arrivals fixed [0min, 0min, 0min]; horizon 3min; bounds { cases 3; live_tasks 1; clock 20min } }
            metrics { window 20min; currency "JPY"; breakdown waits }
            verify safety no_resource_overcommit
        }"#.into(), Some("Day"), "inconclusive", 2),
        _ => panic!("unknown survivor {id}"),
    }
}

fn terminal<'a>(m: &'a Value, case: usize, end: &str) -> &'a Value {
    let rows: Vec<_> = m["cases"][case]["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["end"] == end)
        .collect();
    assert_eq!(rows.len(), 1, "unique case{case}/{end}");
    rows[0]
}
fn queue(m: &Value, actor: usize, total: &str, max: &str, average: &str, lower: bool) {
    let quantity = |x: &str| {
        if lower {
            json!({"stats":scalar(x),"bound":"lower"})
        } else {
            json!({"stats":scalar(x)})
        }
    };
    assert_eq!(
        m["actors"][actor]["queue"],
        json!({
        "wait_total":quantity(total),"length_max":scalar(max),"length_time_average":quantity(average)})
    );
}
fn no_dwell(m: &Value) {
    for c in m["cases"].as_array().unwrap() {
        assert!(c.get("dwell").is_none(), "only actual cut_off has dwell");
    }
}

/// Apply the same independent arithmetic to engine, reference and real CLI.
pub fn hand(id: &str, m: &Value) {
    assert!(m.is_object(), "{id}: real metrics required");
    match id {
        "F15a" => {
            assert_eq!(
                m["cases"][0]["joins"],
                json!([{"fork":"f","mode":"all","branches":[
                {"binding":"z","join_wait":scalar("2")},{"binding":"a","join_wait":scalar("0")}]}])
            );
            assert_eq!(
                terminal(m, 0, "done")["breakdown"],
                partition(scalar("5"), scalar("0"), scalar("0"), scalar("0"))
            );
        }
        "F26" => {
            assert_eq!(m["overall"]["observed_until"], scalar("10"));
            assert_eq!(m["overall"]["queue_wait_observed"], scalar("5"));
            queue(m, 0, "5", "1", "1/2", false);
            no_dwell(m);
            for (i, q, lead) in [(0, "0", "5"), (1, "5", "10")] {
                let t = terminal(m, i, "done");
                assert_eq!(t["lead_time"], scalar(lead));
                assert_eq!(
                    t["breakdown"],
                    partition(scalar("5"), scalar(q), scalar("0"), scalar("0"))
                );
            }
        }
        "A01a" => {
            assert_eq!(
                terminal(m, 0, "posted")["breakdown"],
                partition(scalar("5"), scalar("0"), scalar("0"), scalar("0"))
            );
            assert_eq!(terminal(m, 0, "posted")["lead_time"], scalar("5"));
            assert_eq!(
                terminal(m, 0, "returned")["breakdown"],
                partition(scalar("2"), scalar("0"), scalar("0"), scalar("0"))
            );
        }
        "A01b" => {
            assert_eq!(
                terminal(m, 0, "posted")["breakdown"],
                partition(scalar("3"), scalar("0"), span("0", "4"), span("1", "5"))
            );
            assert_eq!(
                terminal(m, 1, "posted")["breakdown"],
                partition(scalar("3"), span("2", "4"), span("0", "4"), span("0", "5"))
            );
            for (i, q, lead) in [(0, "0", "2"), (1, "2", "4")] {
                assert_eq!(
                    terminal(m, i, "returned")["breakdown"],
                    partition(scalar("2"), scalar(q), scalar("0"), scalar("0"))
                );
                assert_eq!(terminal(m, i, "returned")["lead_time"], scalar(lead));
            }
            assert_eq!(terminal(m, 0, "posted")["lead_time"], scalar("8"));
            assert_eq!(terminal(m, 1, "posted")["lead_time"], scalar("10"));
        }
        "A05b" => {
            for (i, service, combined, lead, a, b, j) in [
                (0, "5", "0", "5", "0", "0", "2"),
                (1, "7", "3", "10", "3", "5", "4"),
                (2, "8", "5", "14", "5", "9", "6"),
            ] {
                let t = terminal(m, i, "done");
                let mut p = partition(scalar(service), scalar("0"), scalar("0"), scalar("0"));
                p["queue"]["combined"] = scalar(combined);
                if i == 2 {
                    p["queue"]["resource"] = scalar("1");
                }
                assert_eq!(
                    t["breakdown"],
                    p,
                    "case{} combined sums every segment",
                    i + 1
                );
                assert_eq!(t["lead_time"], scalar(lead));
                let tasks = &m["cases"][i]["tasks"];
                assert_eq!(tasks[0]["wait_causes"]["actor_capacity"], scalar(a));
                assert_eq!(tasks[1]["wait_causes"]["resource"], scalar(b));
                assert_eq!(
                    m["cases"][i]["joins"][0]["branches"][0]["join_wait"],
                    scalar(j)
                );
            }
            queue(m, 0, "8", "2", "8/15", false);
            queue(m, 1, "14", "2", "14/15", false);
            assert_eq!(m["overall"]["queue_wait_observed"], scalar("22"));
        }
        "A23" => {
            assert_eq!(
                m["cases"][1]["tasks"][0]["wait_causes"],
                json!({
                "actor_capacity":scalar("3"),"resource":scalar("0"),"executor":scalar("0"),"combined":scalar("0"),"unattributed":scalar("0")})
            );
            assert_eq!(
                terminal(m, 1, "done")["breakdown"],
                partition(scalar("3"), scalar("3"), scalar("0"), scalar("0"))
            );
            assert_eq!(terminal(m, 1, "done")["lead_time"], scalar("6"));
            queue(m, 0, "3", "1", "1/2", false);
        }
        "A10" => {
            assert_eq!(m["overall"]["observed_until"], scalar("3"));
            assert_eq!(m["overall"]["queue_wait_observed"], scalar("5"));
            queue(m, 0, "5", "2", "5/3", true);
            assert!(m["cases"][0].get("dwell").is_none());
            for (i, service, q) in [(1, "1", "2"), (2, "0", "3")] {
                assert_eq!(
                    m["cases"][i]["dwell"],
                    json!({"until":scalar("3"),"bound":"lower",
                    "breakdown":partition(scalar(service),scalar(q),scalar("0"),scalar("0"))})
                );
            }
        }
        "A12b" => {
            assert_eq!(
                m["cases"][0]["dwell"],
                json!({"until":scalar("3"),"bound":"lower",
                "breakdown":partition(scalar("3"),scalar("0"),scalar("0"),scalar("0"))})
            );
            for c in &m["cases"].as_array().unwrap()[1..] {
                assert!(c.get("dwell").is_none(), "rejected is not cut_off");
                let t: Vec<_> = c["terminals"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|t| t["kind"] == "rejected")
                    .collect();
                assert_eq!(t.len(), 1);
                let n = json!({"min":null,"max":null,"reason":"rejected"});
                assert_eq!(
                    t[0]["breakdown"],
                    json!({"external":n,"in_service":n,"contest_route":n,
                    "queue":{"actor_capacity":n,"resource":n,"executor":n,"combined":n},
                    "intentional":n,"unattributed":n})
                );
            }
            queue(m, 0, "0", "0", "0", true);
        }
        _ => panic!("unknown survivor {id}"),
    }
    if !matches!(id, "A10" | "A12b") {
        no_dwell(m);
    }
}

fn check(id: &str) {
    let (src, scenario, verdict, _) = input(id);
    let m = lower::lower_with_selection(&syntax::parse(&src).unwrap(), None, scenario)
        .expect("valid inline acceptance input before executing either interpreter");
    let r = report::check_with_selection(&src, None, scenario);
    diagnostics(id, &serde_json::to_value(&r).unwrap());
    assert_eq!(
        serde_json::to_value(r.verdict).unwrap(),
        verdict,
        "{id}: root verdict"
    );
    hand(id, &serde_json::to_value(&r.metrics).unwrap());
    let e = engine::explore(&m);
    let mut r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "{id}: reference exclusions zero"
    );
    assert_eq!(
        &r.outcome.diagnostics[..m.diagnostics.len()],
        m.diagnostics.as_slice()
    );
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    for metrics in [&e.metrics, &r.outcome.metrics] {
        hand(id, &serde_json::to_value(metrics).unwrap());
    }
    assert_eq!(e, r.outcome, "{id}: independent full Outcomes");
    if id == "F26" {
        let control = replace_once(&src, "horizon 10min", "horizon 12min");
        let r = report::check_with_selection(&control, None, scenario);
        assert_eq!(r.verdict, relca_core::Verdict::Satisfied);
        hand(id, &serde_json::to_value(&r.metrics).unwrap());
    }
}

#[test]
fn survivor_f15a_lexical_branch_order() {
    check("F15a");
}
#[test]
fn survivor_f26_exhaustive_exact_horizon() {
    check("F26");
}
#[test]
fn survivor_a01a_service_precedes_route() {
    check("A01a");
}
#[test]
fn survivor_a01b_route_queue_precedes_wait() {
    check("A01b");
}
#[test]
fn survivor_a05b_combined_sums_split_intervals() {
    check("A05b");
}
#[test]
fn survivor_a23_exactly_enough_resource() {
    check("A23");
}
#[test]
fn survivor_a10_observation_end_not_last_terminal() {
    check("A10");
}
#[test]
fn survivor_a12b_rejected_never_gets_dwell() {
    check("A12b");
}
