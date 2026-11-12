//! Boundary review fixes for ADR 0029 (bounded integers). Every expected word
//! is derived by hand from the U2 ruling: a branch stopped by an out-of-range
//! write is not a computational cut, so its consumers say `range_stopped`;
//! with a computational cut as well they keep `truncated`.
#[path = "support/integer_acceptance.rs"]
mod hand;
#[path = "support/integer_review_models.rs"]
mod models;
use models::{CONTEST, METRICS, REQUIRES, with_arrivals, with_cut};
use relca_core::{engine, ir, lower, outcome, reference, report, syntax};
use serde_json::{Value, json};

fn model(s: &str) -> ir::Model {
    lower::lower(&syntax::parse(s).unwrap_or_else(|e| panic!("{e:?}\n{s}")))
        .unwrap_or_else(|e| panic!("{e:?}\n{s}"))
}

fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}

/// Engine and reference on the legacy route and on the arrivals route.
fn four(s: &str) -> Vec<(&'static str, outcome::Outcome)> {
    let a = with_arrivals(s);
    let (legacy, arrivals) = (model(s), model(&a));
    let mut out = Vec::new();
    for (label, m, routed) in [
        ("engine legacy", &legacy, false),
        ("engine arrivals", &arrivals, true),
    ] {
        let o = engine::explore(m);
        assert_eq!(o.case_outcomes.is_some(), routed, "{label}");
        out.push((label, o));
    }
    for (label, m, routed) in [
        ("reference legacy", &legacy, false),
        ("reference arrivals", &arrivals, true),
    ] {
        let r = reference::explore(m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{label}");
        assert_eq!(r.outcome.case_outcomes.is_some(), routed, "{label}");
        out.push((label, r.outcome));
    }
    for (label, o) in &out {
        assert!(
            !o.state_range_violations.is_empty(),
            "{label}: no range stop"
        );
    }
    out
}

fn bounds(s: &str, truncated: bool) {
    for src in [s.to_string(), with_arrivals(s)] {
        let v = check(&src);
        assert_eq!(v["verdict"], "violated", "{v}");
        assert_eq!(v["bounds"]["truncated"], truncated, "{v}");
        assert_eq!(v["bounds"]["range_violation_stops"], true, "{v}");
        assert_eq!(hand::property(&v, "state_in_range")["verdict"], "violated");
    }
}

/// Every route's words side by side, so one failure shows all four routes.
fn inventory(
    runs: &[(&'static str, outcome::Outcome)],
    f: impl Fn(&outcome::Outcome) -> Vec<Value>,
    word: &str,
) {
    let actual: Vec<_> = runs.iter().map(|(l, o)| (*l, f(o))).collect();
    let expected: Vec<_> = actual
        .iter()
        .map(|(l, ws)| (*l, vec![json!(word); ws.len()]))
        .collect();
    assert!(actual.iter().all(|(_, ws)| !ws.is_empty()), "{actual:?}");
    assert_eq!(actual, expected);
}

fn contest_words(s: &str, word: &str) {
    let runs = four(s);
    inventory(
        &runs,
        |o| {
            let rows = serde_json::to_value(o.contest_runtime.as_ref().unwrap()).unwrap();
            let summary = serde_json::to_value(o.contest_summary.as_ref().unwrap()).unwrap();
            let mut ws: Vec<_> = rows
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["coverage"].clone())
                .collect();
            ws.push(summary["business_ended_open"]["reason"].clone());
            ws
        },
        word,
    );
    for (label, o) in runs {
        let rows = serde_json::to_value(o.contest_runtime.as_ref().unwrap()).unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 1, "{label}");
        let mut filed = 0;
        for row in rows {
            assert_eq!(row["coverage"], word, "{label}: {row}");
            for a in row["alternatives"].as_array().unwrap() {
                if a["filed"] == true {
                    filed += 1;
                    assert_eq!(
                        a["route_cost"],
                        json!({"min":null,"max":null,"reason":word}),
                        "{label}: {a}"
                    );
                } else {
                    assert_eq!(a["route_cost"]["reason"], "never_created", "{label}");
                }
            }
        }
        assert!(filed > 0, "{label}: no filed alternative");
        assert_eq!(
            serde_json::to_value(o.contest_summary.as_ref().unwrap()).unwrap(),
            json!({"business_ended_open":{"min":null,"max":null,"reason":word}}),
            "{label}"
        );
    }
}

#[test]
fn range_stop_only_contest_rows_and_summary_say_range_stopped_on_every_route() {
    bounds(CONTEST, false);
    contest_words(CONTEST, "range_stopped");
}

#[test]
fn range_stop_with_clock_cut_contest_rows_and_summary_keep_truncated() {
    let s = with_cut(CONTEST);
    bounds(&s, true);
    contest_words(&s, "truncated");
}

fn metric_words(s: &str, word: &str) {
    fn ranges(v: &Value, word: &str, n: &mut usize) {
        match v {
            Value::Object(o) if o.contains_key("min") && o.contains_key("max") => {
                *n += 1;
                assert_eq!(v["min"], Value::Null, "{v}");
                assert_eq!(v["max"], Value::Null, "{v}");
                assert_eq!(v["reason"], word, "{v}");
            }
            Value::Object(o) => o.values().for_each(|x| ranges(x, word, n)),
            Value::Array(a) => a.iter().for_each(|x| ranges(x, word, n)),
            _ => {}
        }
    }
    let runs = four(s);
    inventory(
        &runs,
        |o| vec![serde_json::to_value(o.metrics.as_ref().unwrap()).unwrap()["status"].clone()],
        word,
    );
    for (label, o) in runs {
        let v = serde_json::to_value(o.metrics.as_ref().unwrap()).unwrap();
        assert_eq!(v["status"], word, "{label}");
        let mut n = 0;
        ranges(&v, word, &mut n);
        assert!(n > 0, "{label}: no metric range");
    }
}

#[test]
fn range_stop_only_metrics_say_range_stopped_on_every_route() {
    bounds(METRICS, false);
    metric_words(METRICS, "range_stopped");
}

#[test]
fn range_stop_with_clock_cut_metrics_keep_truncated() {
    let s = with_cut(METRICS);
    bounds(&s, true);
    metric_words(&s, "truncated");
}

fn requires_words(s: &str, word: &str) {
    let runs = four(s);
    inventory(
        &runs,
        |o| {
            let rows = serde_json::to_value(o.obligations.as_ref().unwrap()).unwrap();
            let mut ws: Vec<_> = o
                .capability_refusals
                .as_ref()
                .unwrap()
                .iter()
                .map(|r| json!(r.coverage))
                .collect();
            ws.extend(
                rows.as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r["coverage"].clone()),
            );
            ws
        },
        word,
    );
    for (label, o) in runs {
        let refusals = o.capability_refusals.as_ref().unwrap();
        assert_eq!(refusals.len(), 1, "{label}");
        for r in refusals {
            assert_eq!(r.capability, "Worker.run", "{label}");
            assert_eq!(r.coverage, word, "{label}");
        }
        let rows = serde_json::to_value(o.obligations.as_ref().unwrap()).unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 1, "{label}");
        for row in rows {
            assert_eq!(row["coverage"], word, "{label}: {row}");
            // The bump branch stopped before its deadline: unknown, not success.
            assert!(
                row["outcomes"].as_array().unwrap().iter().any(|x| x
                    == &json!({"status":"unresolved","triggered_at":0,"deadline":3,"resolved_at":null,"reason":"state_range_violation","by_clause":null,"executor":null,"authority":null})),
                "{label}: {row}"
            );
        }
    }
}

#[test]
fn range_stop_only_refusals_and_obligations_say_range_stopped_on_every_route() {
    bounds(REQUIRES, false);
    requires_words(REQUIRES, "range_stopped");
}

#[test]
fn range_stop_with_clock_cut_refusals_and_obligations_keep_truncated() {
    let s = with_cut(REQUIRES);
    bounds(&s, true);
    requires_words(&s, "truncated");
}

/// Without a range stop the same tables stay complete (the new word is not a
/// default): the bump write is made to fit by widening the range.
#[test]
fn without_range_stop_the_same_tables_stay_complete() {
    for (s, field) in [
        (CONTEST, "contest"),
        (METRICS, "metrics"),
        (REQUIRES, "requires"),
    ] {
        let wide = s.replace("n: 0..0 = 0", "n: 0..1 = 0");
        assert_ne!(wide, s);
        for src in [wide.clone(), with_arrivals(&wide)] {
            let v = check(&src);
            assert_eq!(v["bounds"]["truncated"], false, "{field}: {v}");
            assert!(v["bounds"].get("range_violation_stops").is_none(), "{v}");
            let words: Vec<_> = match field {
                "contest" => vec![v["contest_runtime"][0]["coverage"].clone()],
                "metrics" => vec![v["metrics"]["status"].clone()],
                _ => vec![
                    v["capability_refusals"][0]["coverage"].clone(),
                    v["obligations"][0]["coverage"].clone(),
                ],
            };
            assert!(words.iter().all(|w| w == "complete"), "{field}: {words:?}");
        }
    }
}

// Review fix 4: a variable on the right of an integer comparison is outside
// the ADR 0029 range (like a variable on the right of an assignment).
fn program(cond_site: &str) -> String {
    format!(
        r#"model Cmp version "0.2" {{ type T = enum {{ ok, no }}; observed case {{ n: 0..2 = 0; m: 0..2 = 1 }}; actor A {{ capacity 1; capability run() -> T {{ duration fixed 1min; result nondet {{ ok }}{} }} }}; process P {{ {} }}; bounds {{ cases 1; live_tasks 2; clock 10min }}; verify safety no_resource_overcommit }}"#,
        if cond_site.starts_with("requires") {
            format!("; {cond_site} else no")
        } else {
            String::new()
        },
        if cond_site.starts_with("requires") {
            "a = A.run(); end done".to_string()
        } else {
            format!("if {cond_site} {{ end done }} else {{ end other }}")
        }
    )
}

#[test]
fn integer_comparison_with_a_variable_on_the_right_is_unsupported() {
    for cond in ["n < m", "n == m", "n != m", "n >= m", "requires n <= m"] {
        hand::rejection(&program(cond), "U-SYN-0");
    }
    // Controls: a constant stays supported; a member name stays a type error.
    let ok = program("n < 1");
    assert_eq!(
        hand::property(&check(&ok), "state_in_range")["verdict"],
        "satisfied"
    );
    hand::rejection(&program("n == ok"), "E-TYPE-0");
}

// Review fix 5 [U]: an integer in a kernel Capability's writes without a
// finish assignment is stopped statically, never run.
#[test]
fn kernel_capability_writing_an_integer_without_finish_is_rejected_statically() {
    let base = r#"model KW version "0.2" {
  type T = enum { ok, no }
  observed case { n: 0..2 = 0; f: T = ok }
  kernel K { * -> { 1: ok } }
  actor A { capacity 1; capability c() -> T { duration fixed 1min; outcomes kernel K; WRITES } }
  process P { a = A.c(); end done }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.done
}
"#;
    for (writes, code) in [
        ("writes n", "E-TYPE-0"),
        ("writes [n]", "E-TYPE-0"),
        ("writes n; start { n += 1 }", "E-TYPE-0"),
        ("writes [f,n]; finish { f = no }", "E-TYPE-0"),
    ] {
        let s = base.replace("WRITES", writes);
        assert_ne!(s, base);
        hand::rejection(&s, code);
        assert!(lower::lower(&syntax::parse(&s).unwrap()).is_err(), "{s}");
    }
    let with = base
        .replace("{ 1: ok }", "{ 1: ok with n = 1 }")
        .replace("WRITES", "writes n");
    assert_ne!(with, base);
    hand::rejection(&with, "U-SEM-0");
    // Control: the finish assignment makes the same Capability runnable.
    let finished = base.replace("WRITES", "writes n; finish { n += 1 }");
    assert_eq!(
        hand::property(&check(&finished), "state_in_range")["verdict"],
        "satisfied"
    );
}

/// Engine and reference verdicts (and probability) per property, with the
/// reference required to finish its own exploration.
fn engine_and_reference(m: &ir::Model) -> Vec<(&'static str, outcome::Outcome)> {
    let r = reference::explore(m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    vec![("engine", engine::explore(m)), ("reference", r.outcome)]
}

fn rows(o: &outcome::Outcome) -> Vec<(Value, Value)> {
    o.properties
        .iter()
        .map(|p| {
            (
                serde_json::to_value(p.verdict).unwrap(),
                serde_json::to_value(&p.probability).unwrap(),
            )
        })
        .collect()
}

/// Mutation N12 (completion graph forgets that a node was range-stopped).
/// Hand: n starts at its upper bound 2; the only path finishes `n += 1` at
/// 1min and attempts 3, so the branch stops there and no end is reached. What
/// follows a range stop is unknown (U2), so the end properties have no
/// witness either way: inconclusive, and almost_sure keeps the stopped mass as
/// [0, 1]. Only state_in_range is decided. Clock 10min and live_tasks 2 are
/// never reached, so this is not a computational cut.
#[test]
fn completion_graph_keeps_a_range_stopped_node_unknown() {
    let s = r#"model Count version "0.2" {
  type T = enum { ok, no }
  observed case { n: 0..2 = 2 }
  actor A { capacity 1; capability inc() -> T { duration fixed 1min; result nondet { ok }; writes n; finish { n += 1 } } }
  process P { a = A.inc(); if n == 2 { end done } else { end wrong } }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify recoverable end
  verify terminating end
  verify almost_sure end
}"#;
    let v = check(s);
    assert_eq!(v["verdict"], "violated", "{v}");
    let root: relca_core::Verdict = serde_json::from_value(v["verdict"].clone()).unwrap();
    assert_eq!(root.exit_code(), 1);
    assert_eq!(v["bounds"]["truncated"], false, "{v}");
    assert_eq!(v["bounds"]["range_violation_stops"], true, "{v}");
    assert_eq!(v["terminals"], json!([]));
    let want = [
        ("recoverable end", "inconclusive", Value::Null),
        ("terminating end", "inconclusive", Value::Null),
        (
            "almost_sure end",
            "inconclusive",
            json!({"min":"0","max":"1"}),
        ),
        ("state_in_range", "violated", Value::Null),
    ];
    let public: Vec<_> = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["id"].as_str().unwrap(),
                p["verdict"].as_str().unwrap(),
                p.get("probability").cloned().unwrap_or(Value::Null),
            )
        })
        .collect();
    assert_eq!(public, want);
    let expected: Vec<_> = want
        .iter()
        .map(|(_, verdict, p)| (json!(verdict), p.clone()))
        .collect();
    let m = model(s);
    for (label, o) in engine_and_reference(&m) {
        assert!(!o.truncated, "{label}");
        assert_eq!(o.state_range_violations.len(), 1, "{label}");
        assert_eq!(o.state_range_violations[0].phase, "finish", "{label}");
        assert_eq!(o.state_range_violations[0].attempted, "3", "{label}");
        assert!(o.terminals.is_empty(), "{label}");
        assert_eq!(rows(&o), expected, "{label}");
    }
}

/// Mutation N07 on the arrivals route (engine::arrivals start records the
/// range stop but still publishes the start). Hand: n starts at its upper
/// bound 1 and the start bundle `flag = true; n += 1` attempts 2 at 0min, so
/// case 1 stops before its Start and never reaches end.done. reachable
/// end.done has no witness (U2): inconclusive for the case and the root row,
/// not satisfied; no resource is ever acquired, but the exploration is not
/// complete, so the safety row is inconclusive as well.
#[test]
fn arrivals_start_range_stop_does_not_publish_the_start() {
    let s = with_arrivals(
        r#"model Boundary version "0.2" {
  type T = enum { ok, no }
  observed case { n: 0..1 = 1; flag: bool = false }
  actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok }; start { flag = true; n += 1 } } }
  process P { a = A.run(); end done }
  bounds { cases 1; live_tasks 4; clock 10min }
  verify safety no_resource_overcommit
  verify reachable end.done
}"#,
    );
    let v = check(&s);
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(v["bounds"]["truncated"], false, "{v}");
    assert_eq!(v["bounds"]["range_violation_stops"], true, "{v}");
    let w = &v["state_range_violations"];
    assert_eq!(w.as_array().unwrap().len(), 1, "{v}");
    assert_eq!(
        (&w[0]["case_id"], &w[0]["phase"], &w[0]["time"]),
        (&json!(1), &json!("start"), &json!(0))
    );
    assert_eq!(
        hand::property(&v, "reachable end.done")["cases"],
        json!([{"case_id":1,"arrival":0,"verdict":"inconclusive"}])
    );
    let expected = vec![
        (json!("inconclusive"), Value::Null),
        (json!("inconclusive"), Value::Null),
        (json!("violated"), Value::Null),
    ];
    let m = model(&s);
    for (label, o) in engine_and_reference(&m) {
        assert!(o.case_outcomes.is_some(), "{label}: arrivals route");
        assert!(!o.truncated, "{label}");
        assert_eq!(rows(&o), expected, "{label}");
        let reach = o.properties[1].cases.as_ref().unwrap();
        assert_eq!(reach.len(), 1, "{label}");
        assert_eq!(
            serde_json::to_value(reach[0].verdict).unwrap(),
            "inconclusive",
            "{label}"
        );
    }
}
