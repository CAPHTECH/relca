//! Independent JSON oracle transport: no production quality arithmetic or types.
#![allow(dead_code)]
use relca_core::{compare, diff, explain, report};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn names() -> Vec<String> {
    serde_json::from_str(include_str!("quality_names.json")).unwrap()
}
pub fn inventory() -> Value {
    serde_json::from_str(include_str!("quality_inventory.json")).unwrap()
}
pub fn source(n: &str) -> String {
    fs::read_to_string(root().join("fixtures").join(n).join("model.relca")).unwrap()
}
pub fn expected(n: &str) -> Value {
    serde_json::from_slice(
        &fs::read(root().join("fixtures").join(n).join("expected.json")).unwrap(),
    )
    .unwrap()
}

pub fn no_quality(s: &str) -> String {
    // Preserve byte positions of every old token. Only the explicit new lines
    // in the closed runtime fixtures are removed, not an arbitrary substring.
    s.split_inclusive('\n')
        .map(|l| {
            if l.trim_start().starts_with("property case.")
                || l.trim_start().starts_with("metric cohort.")
            {
                l.chars()
                    .map(|c| if c == '\n' { '\n' } else { ' ' })
                    .collect::<String>()
            } else {
                l.to_owned()
            }
        })
        .collect()
}
pub fn replace(s: &str, from: &str, to: &str) -> String {
    assert!(s.contains(from), "missing replacement witness {from}");
    let changed = s.replacen(from, to, 1);
    assert_ne!(s, changed);
    changed
}
pub fn run(n: &str, s: &str) -> Value {
    let inv = &inventory()[n]["invocation"];
    match inv["command"].as_str() {
        Some("check") => serde_json::to_value(report::check_configuration(
            s,
            inv["configuration"].as_str().unwrap(),
        )),
        Some("simulate") => serde_json::to_value(report::simulate(s)),
        Some("compare") => serde_json::to_value(compare::compare(
            s,
            &["Check", "Fast"],
            if inv["mode"] == "simulation" {
                compare::Analysis::Simulation
            } else {
                compare::Analysis::Exhaustive
            },
        )),
        Some("diff") => serde_json::to_value(diff::diff(s, "Check", "Fast")),
        Some("explain") => serde_json::to_value(explain::explain(s)),
        None => serde_json::to_value(report::check(s)),
        _ => panic!("unknown quality invocation {inv}"),
    }
    .unwrap()
}

pub fn projection_rows(q: &Value) -> Vec<Value> {
    let mut rows = vec![];
    let mut push = |scope: &str,
                    subject: &Value,
                    end: Value,
                    field: &str,
                    value: Value,
                    den: Option<&Value>| {
        let mut row =
            json!({"key":{"scope":scope,"subject":subject,"end":end,"field":field},"value":value});
        if let Some(d) = den {
            row["denominator"] = d.clone();
        }
        rows.push(row);
    };
    for p in q["properties"].as_array().unwrap() {
        push(
            "property",
            &p["name"],
            Value::Null,
            "expected_count",
            p["expected_count"].clone(),
            None,
        );
        if q["analysis"] == "simulation" {
            push(
                "property",
                &p["name"],
                Value::Null,
                "unknown_count",
                p["unknown_count"].clone(),
                None,
            );
        }
    }
    for m in q["cohort"].as_array().unwrap() {
        for field in ["value", "bounds_with_undetermined"] {
            push(
                "cohort",
                &m["name"],
                Value::Null,
                field,
                m[field].clone(),
                Some(&m["denominator"]),
            );
        }
    }
    for o in q["outcomes_per_arrival"].as_array().unwrap() {
        for field in ["expected_count", "rate"] {
            push(
                "outcome",
                &o["kind"],
                o["end"].clone(),
                field,
                o[field].clone(),
                None,
            );
        }
    }
    rows
}

pub fn tables(n: &str, actual: &Value) {
    let inv = inventory();
    let compared = inv[n]["invocation"]["command"] == "compare";
    let table = if compared {
        &actual["tables"]["quality"]
    } else {
        &actual["quality"]
    };
    if n.contains(".broken-") {
        assert_eq!(table, &json!([]), "mismatch must carry empty quality table");
        return;
    }
    let check = &inv[n]["plans"]["Check"];
    let fast = &inv[n]["plans"]["Fast"];
    if compared {
        let plans = actual["plans"].as_array().unwrap();
        assert_eq!(plans.len(), 2);
        for (plan, want, cfg) in [(&plans[0], check, "Check"), (&plans[1], fast, "Fast")] {
            assert_eq!(plan["configuration"], cfg);
            assert_eq!(
                &plan["report"]["quality"], want,
                "{n}/{cfg}: full hand quality"
            );
        }
    } else {
        assert_eq!(&actual["from"]["report"]["quality"], check);
        assert_eq!(&actual["to"]["report"]["quality"], fast);
    }
    let mut rows = vec![];
    for (left, right) in projection_rows(check).iter().zip(projection_rows(fast)) {
        assert_eq!(left["key"], right["key"]);
        let mut r = json!({"key":left["key"]});
        if let Some(d) = left.get("denominator") {
            r["denominator"] = d.clone();
        }
        if compared {
            r["values"] = json!([
                {"configuration":"Check","status":"reported","value":left["value"]},
                {"configuration":"Fast","status":"reported","value":right["value"]}
            ]);
        } else {
            r["from"] = json!({"status":"reported","value":left["value"]});
            r["to"] = json!({"status":"reported","value":right["value"]});
        }
        rows.push(r);
    }
    assert_eq!(
        table,
        &json!(rows),
        "{n}: exact ordered closed quality rows; no difference/rank/verdict"
    );
}

pub fn hand(n: &str, actual: &Value) {
    let h = expected(n);
    assert_eq!(
        actual["verdict"], h["verdict"],
        "{n}: quality must not set verification verdict"
    );
    if n == "qual-diff" {
        // ADR0051 hand: both Flow and Quick test v; its producer moves
        // Checker.check -> Checker.quick. No intent/annotation: W1/W3 +1.
        let row = json!({"kind":"relation","target":"v","change":"modified","reason":null,"loses":null,
                "from":{"producer":"Checker.check"},"to":{"producer":"Checker.quick"}});
        let producers: Vec<_> = actual["differences"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "relation" && r["change"] == "modified")
            .cloned()
            .collect();
        assert_eq!(producers, vec![row]);
        // These two warnings are source/ADR0051 hand consequences, unlike the
        // removed pre-quality capture. Preserve their payloads and group order.
        let diagnostics = actual["diagnostics"].as_array().unwrap();
        let mut positions = Vec::new();
        for (code, message) in [
            ("W-DIFF-1", "computed change for v has no intent"),
            (
                "W-DIFF-3",
                "computed change for relation v has no preserve, override, maintain or loses annotation",
            ),
        ] {
            let rows: Vec<_> = diagnostics
                .iter()
                .enumerate()
                .filter(|(_, d)| d["code"] == code && d["message"] == message)
                .collect();
            assert_eq!(rows.len(), 1, "one source-derived producer warning: {code}");
            assert_eq!(rows[0].1["severity"], "warning");
            assert!(rows[0].1["span"].is_null());
            positions.push(rows[0].0);
        }
        assert!(
            positions[0] < positions[1],
            "intent warning precedes annotation warning"
        );
    }
    let inv = inventory();
    if matches!(
        inv[n]["invocation"]["command"].as_str(),
        Some("compare" | "diff")
    ) {
        tables(n, actual);
        return;
    }
    if let Some(code) = inv[n]["code"].as_str() {
        let errors: Vec<_> = actual["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(errors.len(), 1, "{n}: exactly one static fault");
        assert_eq!(errors[0]["code"], code, "{n}: classification");
        if code == "E-NAME-0" {
            let source = source(n);
            let span = &errors[0]["span"];
            let a = span["start"].as_u64().expect("located unknown reference") as usize;
            let b = span["end"].as_u64().unwrap() as usize;
            assert!(
                a <= b
                    && b <= source.len()
                    && source.is_char_boundary(a)
                    && source.is_char_boundary(b)
            );
            let wanted = if n.ends_with("unknown-property") {
                "Nope"
            } else {
                "nope"
            };
            assert_eq!(
                &source[a..b],
                wanted,
                "{n}: point at the reference name,not the whole declaration"
            );
        }
        for (suffix, prefix) in [
            ("duplicate-property", "duplicate quality property"),
            ("duplicate-metric", "duplicate cohort metric"),
            ("binding-condition", "probability conditions require state"),
            (
                "expectation-of-ratio",
                "expectation of ratio is unsupported",
            ),
            ("mixed", "end union cannot mix with state conditions"),
        ] {
            if n.ends_with(suffix) {
                assert!(
                    errors[0]["message"].as_str().unwrap().starts_with(prefix),
                    "{n}: diagnostic message {prefix}"
                );
            }
        }
        assert!(
            actual.get("quality").is_none(),
            "static failure must not emit a quality estimate"
        );
        if n.ends_with("old-property") || n.ends_with("old-metric") {
            let word = if n.ends_with("old-property") {
                "property"
            } else {
                "metric"
            };
            assert_eq!(errors[0]["message"], format!("unsupported syntax: {word}"));
        }
        for (suffix, message) in [
            (
                "cohort-probability",
                "unsupported syntax: cohort probability",
            ),
            (
                "verify-union",
                "unsupported syntax: end probability condition must be standalone",
            ),
            ("broken-hard", "unsupported syntax: hard"),
        ] {
            if n.ends_with(suffix) {
                assert_eq!(
                    errors[0]["message"], message,
                    "{n}: old parser diagnostic must remain byte-identical"
                );
            }
        }
    } else if let Some(q) = h.get("quality") {
        assert_eq!(
            actual.get("quality"),
            Some(q),
            "{n}: full hand quality, required null fields and order"
        );
    } else if !n.starts_with("qual-late-abort") {
        assert!(
            actual.get("quality").is_none(),
            "{n}: undeclared quality key must be omitted"
        );
    }
    if n == "qual-late-abort" {
        let q = actual
            .get("quality")
            .expect("quality is required even when no statistical verify is added");
        assert_eq!(actual["simulation"]["completed_replications"], 4);
        assert_eq!(
            q["properties"][0]["cases"],
            json!([{"case_id":1,"arrival":0,"hits":4,"unknown":0,"trials":4}])
        );
        assert_eq!(
            q["properties"][0]["expected_count"],
            json!({"mean":"1","min":"1","max":"1","samples":4,"reason":null})
        );
        assert_eq!(
            q["properties"][0]["unknown_count"],
            json!({"mean":"0","min":"0","max":"0","samples":4,"reason":null})
        );
        assert_eq!(q["cohort"][0]["value"]["point"], "1");
        assert_eq!(q["cohort"][0]["value"]["samples"], 4);
    }
    if n == "qual-late-abort.broken-nondeterminism" {
        assert_eq!(actual["simulation"]["attempted_replications"], 3);
        assert_eq!(actual["simulation"]["completed_replications"], 2);
        let q = actual
            .get("quality")
            .expect("aborted quality must remain disclosed");
        assert_eq!(q["checked"], json!([]));
        for p in q["properties"].as_array().unwrap() {
            for f in ["expected_count", "unknown_count"] {
                assert_eq!(
                    p[f],
                    json!({"mean":null,"min":null,"max":null,"samples":0,"reason":"aborted"})
                );
            }
            for c in p["cases"].as_array().unwrap() {
                assert_eq!(
                    (
                        c["hits"].as_u64(),
                        c["unknown"].as_u64(),
                        c["trials"].as_u64()
                    ),
                    (Some(0), Some(0), Some(0))
                );
            }
        }
        assert_eq!(q["cohort"][0]["value"]["reason"], "aborted");
        assert_eq!(q["cohort"][0]["value"]["samples"], 0);
        assert!(q["cohort"][0]["value"]["point"].is_null());
        fn withdrawn(v: &Value) {
            match v {
                Value::Object(fields) => {
                    if fields.contains_key("samples") {
                        assert_eq!(v["samples"], 0, "all prior samples must be retracted");
                        assert_eq!(v["reason"], "aborted");
                        for key in ["mean", "min", "max", "point"] {
                            if fields.contains_key(key) {
                                assert!(v[key].is_null());
                            }
                        }
                    }
                    for v in fields.values() {
                        withdrawn(v);
                    }
                }
                Value::Array(values) => {
                    for v in values {
                        withdrawn(v);
                    }
                }
                _ => {}
            }
        }
        withdrawn(q);
    }
}

#[path = "../../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
pub fn old_projection(mut v: Value) -> Value {
    v = ledger_shape::original_document(&v);
    // Only for the explicit T13/T20 quality-added pairs. The historical byte
    // checker never calls this function or removes any quality/assumption.
    v.as_object_mut().unwrap().remove("quality");
    v["model"]["sha256"] = Value::Null;
    v["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|a| !a["field"].as_str().unwrap().starts_with("quality."));
    v
}
