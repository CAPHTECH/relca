//! Hand oracles independent of engine/independent reference; Stage C RED.
#![allow(dead_code)]
use relca_core::{
    engine, ir, lower,
    outcome::{CaseTerminalKind, Outcome},
    reference, report, syntax,
};
use serde_json::{Value, json};
use std::path::PathBuf;
#[path = "limits_population.rs"]
mod population;
pub use population::names;
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn source(n: &str) -> String {
    std::fs::read_to_string(root().join("fixtures").join(n).join("model.relca")).unwrap()
}
pub fn expected(n: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(root().join("fixtures").join(n).join("expected.json")).unwrap(),
    )
    .unwrap()
}
pub fn pairs() -> Vec<Value> {
    serde_json::from_str(include_str!("limits_pairs.json")).unwrap()
}
pub fn edit(s: &str, a: &str, b: &str) -> String {
    let t = s.replace(a, b);
    assert_ne!(s, t, "control edit {a}");
    t
}
pub fn selected(s: &str) -> ir::Model {
    syntax::parse(s)
        .and_then(|a| lower::lower(&a))
        .unwrap_or_else(|d| panic!("limits parse/lower: {d:?}\n{s}"))
}
pub fn checked(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
pub fn sampled(s: &str) -> Value {
    serde_json::to_value(report::simulate(s)).unwrap()
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                if k == "absent_diagnostics" {
                    for code in v.as_array().unwrap() {
                        assert!(
                            !a["diagnostics"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|d| d["code"] == *code),
                            "forbidden diagnostic {code}: {a}"
                        );
                    }
                } else {
                    subset(v, a.get(k).unwrap_or_else(|| panic!("missing {k}: {a}")));
                }
            }
        }
        Value::Array(es) => {
            let xs = a.as_array().unwrap_or_else(|| panic!("not array: {a}"));
            // Diagnostics are intentionally partial and may include lower warnings.
            // All numerical, case, task and property arrays retain exact cardinality.
            let diagnostic = es.first().is_some_and(|r| r.get("code").is_some());
            if diagnostic {
                for e in es {
                    assert!(
                        xs.iter().any(|a| is_subset(e, a)),
                        "missing diagnostic {e}: {a}"
                    );
                }
            } else {
                assert_eq!(es.len(), xs.len(), "closed array {e} vs {a}");
                for (e, a) in es.iter().zip(xs) {
                    subset(e, a);
                }
            }
        }
        _ => assert_eq!(e, a, "hand oracle"),
    }
}
fn is_subset(e: &Value, a: &Value) -> bool {
    match e {
        Value::Object(o) => o
            .iter()
            .all(|(k, v)| a.get(k).is_some_and(|a| is_subset(v, a))),
        _ => e == a,
    }
}
pub fn skeleton(s: &str) -> String {
    s.lines()
        .filter(|line| {
            let l = line.trim_start();
            !l.starts_with("queue capacity")
                && !l.starts_with("concurrent cases")
                && !l.starts_with("contract C {")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn range(r: relca_core::outcome::Range) -> Value {
    json!({"min":r.min,"max":r.max})
}
pub fn outcome_hand(m: &ir::Model, o: &Outcome, h: &Value) {
    if h.get("bounds").is_some() {
        assert_eq!(json!(o.truncated), h["bounds"]["truncated"]);
    }
    if let Some(cs) = h.get("case_outcomes") {
        let actual = o.case_outcomes.as_ref().expect("all case IDs retained");
        assert_eq!(actual.len(), cs.as_array().unwrap().len());
        for (c, h) in actual.iter().zip(cs.as_array().unwrap()) {
            assert_eq!(json!(c.case_id.0 + 1), h["case_id"]);
            assert_eq!(json!(c.arrival), h["arrival"]);
            assert_eq!(c.terminals.len(), h["terminals"].as_array().unwrap().len());
            for (t, h) in c.terminals.iter().zip(h["terminals"].as_array().unwrap()) {
                if let CaseTerminalKind::End(id) = t.kind {
                    assert_eq!(h["kind"], "end");
                    assert_eq!(h["end"], m.ends[id.0]);
                } else {
                    let d = format!("{:?}", t.kind);
                    assert!(
                        if h["kind"] == "overflow" {
                            d.starts_with("Overflow")
                        } else {
                            d == "Rejected"
                        },
                        "terminal identity {d} vs {h}"
                    );
                }
                for (key, value) in [
                    ("completed_at", t.completed_at),
                    ("elapsed", t.elapsed),
                    ("work", t.work),
                    ("result_at", t.result_at),
                ] {
                    assert_eq!(
                        json!(value.map(range)),
                        h[key],
                        "engine/reference hand time {key}"
                    );
                }
                if let Some(p) = h.get("probability") {
                    let actual = t.probability.as_ref().unwrap();
                    assert_eq!(
                        p,
                        &json!({"min":actual.min.to_string(),"max":actual.max.to_string()})
                    );
                }
            }
        }
    }
    if let Some(metrics) = h.get("metrics") {
        subset(
            metrics,
            &serde_json::to_value(o.metrics.as_ref().unwrap()).unwrap(),
        );
    }
    if let Some(ps) = h.get("properties") {
        assert_eq!(o.properties.len(), ps.as_array().unwrap().len());
        for (p, h) in o.properties.iter().zip(ps.as_array().unwrap()) {
            assert_eq!(json!(p.verdict), h["verdict"]);
            if let Some(cs) = h.get("cases") {
                let actual = p.cases.as_ref().unwrap();
                assert_eq!(actual.len(), cs.as_array().unwrap().len());
                for (c, h) in actual.iter().zip(cs.as_array().unwrap()) {
                    assert_eq!(json!(c.case_id.0 + 1), h["case_id"]);
                    assert_eq!(json!(c.verdict), h["verdict"]);
                    if let Some(p) = h.get("probability") {
                        let actual = c.probability.as_ref().unwrap();
                        assert_eq!(
                            p,
                            &json!({"min":actual.min.to_string(),"max":actual.max.to_string()})
                        );
                    }
                }
            }
        }
    }
}
pub fn both(s: &str, h: &Value) {
    let m = selected(s);
    let e = engine::explore(&m);
    let mut r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "no silent reference exclusion"
    );
    // Apply independent oracle BEFORE differential equality.
    outcome_hand(&m, &e, h);
    outcome_hand(&m, &r.outcome, h);
    assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    assert_eq!(
        e, r.outcome,
        "full Outcome including admission/time/slot-derived statistics"
    );
}
pub fn accept(base: &str) {
    let p = pairs().into_iter().find(|p| p["good"] == base).unwrap();
    for name in [p["good"].as_str().unwrap(), p["bad"].as_str().unwrap()] {
        let s = source(name);
        let h = expected(name);
        let sampled_mode = p["invocation"]["command"] == "simulate";
        let v = if sampled_mode {
            sampled(&s)
        } else {
            checked(&s)
        };
        subset(&h, &v);
        if sampled_mode {
            // Runtime nondeterminism refusal is lowerable: the bad simulation
            // input must also be rejected by the independent sampling driver.
            let m = selected(&s);
            let e = engine::simulate(&m);
            let r = reference::simulate(&m);
            assert_eq!(
                e, r.outcome,
                "independent simulation, including runtime refusal"
            );
        }
        if h["verdict"] == "invalid" || h["verdict"] == "unsupported" {
            continue;
        }
        if !sampled_mode {
            both(&s, &h);
        }
    }
}
pub fn row<'a>(v: &'a Value, f: &str) -> &'a Value {
    let a: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["field"] == f)
        .collect();
    assert_eq!(a.len(), 1, "assumption {f}");
    a[0]
}
pub fn compare_source() -> String {
    let s = source("lim-counts-bare");
    edit(
        &s,
        "  arrivals fixed",
        "  process Other { r = A.run(); end done }\n  cut C { include [A] }\n  configuration Left { cut C; process Flow }\n  configuration Right { cut C; process Other }\n  arrivals fixed",
    )
}
pub fn contest_source() -> String {
    let s =
        std::fs::read_to_string(root().join("fixtures/ctx-contest-origin/model.relca")).unwrap();
    let s = edit(&s, "duration fixed 4min", "duration fixed 2min");
    // The route must stay busy after business end2; its resource is independent.
    let s = edit(
        &s,
        "actor Route { executors [p]; capability respond() -> T { duration fixed 2min",
        "actor Route { executors [p]; capability respond() -> T { duration fixed 5min",
    );
    let s = edit(&s, "[10min]", "[0min, 0min]");
    let s = edit(&s, "cases 1", "cases 2");
    let s = edit(&s, "respond_by 2min", "respond_by 5min");
    edit(
        &s,
        "  bounds {",
        "  concurrent cases 1 on_overflow wait_external fifo\n  metrics { window 30min; currency \"JPY\" }\n  bounds {",
    )
}
