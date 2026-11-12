//! ADR 0029 review fix 1 on the wire: the closed validators accept
//! `range_stopped` only beside an integer range stop without a computational
//! cut, and require it there.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/integer_acceptance.rs"]
mod hand;
#[path = "../../relca-core/tests/support/integer_review_models.rs"]
mod models;
use models::{CONTEST, METRICS, REQUIRES, with_arrivals, with_cut};
use serde_json::{Value, json};
use std::process::Command;

fn run(source: &str) -> (Value, i32) {
    static ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = hand::root().join("target").join(format!(
        "int-review-fix-{}-{}.relca",
        std::process::id(),
        ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("check")
        .arg(&path)
        .current_dir(hand::root())
        .output()
        .unwrap();
    (
        serde_json::from_slice(&output.stdout).unwrap(),
        output.status.code().unwrap(),
    )
}

const SUMMARY: &str = "/contest_summary/business_ended_open";

/// Writes one word at a pointer; the summary count takes the whole range.
fn set(v: &mut Value, pointer: &str, word: &str) {
    *v.pointer_mut(pointer).unwrap() = match (pointer, word) {
        (SUMMARY, "complete") => json!({"min":"0","max":"0","reason":null}),
        (SUMMARY, w) => json!({"min":null,"max":null,"reason":w}),
        (_, w) => json!(w),
    };
}

/// The coverage-like words of one report, by JSON pointer.
fn words(v: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for (i, r) in v["contest_runtime"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        out.push((
            format!("/contest_runtime/{i}/coverage"),
            r["coverage"].clone(),
        ));
    }
    if v.get("contest_summary").is_some() {
        // The summary's word is its null reason; an exact count is complete.
        let reason = &v["contest_summary"]["business_ended_open"]["reason"];
        out.push((
            SUMMARY.into(),
            if reason.is_null() {
                json!("complete")
            } else {
                reason.clone()
            },
        ));
    }
    if v.get("metrics").is_some() {
        out.push(("/metrics/status".into(), v["metrics"]["status"].clone()));
    }
    for (i, r) in v["capability_refusals"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        out.push((
            format!("/capability_refusals/{i}/coverage"),
            r["coverage"].clone(),
        ));
    }
    for (i, r) in v["obligations"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        out.push((format!("/obligations/{i}/coverage"), r["coverage"].clone()));
    }
    out
}

fn sources() -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for s in [CONTEST, METRICS, REQUIRES] {
        out.push((s.to_string(), "range_stopped"));
        out.push((with_arrivals(s), "range_stopped"));
        out.push((with_cut(s), "truncated"));
        out.push((with_arrivals(&with_cut(s)), "truncated"));
    }
    out
}

#[test]
fn range_stop_reports_pass_the_closed_validators_with_the_range_word() {
    for (s, word) in sources() {
        let (v, exit) = run(&s);
        assert_eq!(exit, 1, "{v}");
        assert_eq!(v["bounds"]["range_violation_stops"], true, "{v}");
        let ws = words(&v);
        assert!(!ws.is_empty(), "{s}");
        assert!(ws.iter().all(|(_, w)| w == word), "{ws:?}\n{s}");
        fixture_match::compare_report(&json!({}), &v).unwrap_or_else(|e| panic!("{e}\n{s}\n{v}"));
    }
}

#[test]
fn validators_gate_range_stopped_to_integer_range_stops() {
    // A range stop must not be reported in the cut word or as complete.
    for (s, word) in sources().into_iter().filter(|(_, w)| *w == "range_stopped") {
        let (v, _) = run(&s);
        fixture_match::compare_report(&json!({}), &v).unwrap();
        for (pointer, _) in words(&v) {
            for other in ["complete", "truncated"] {
                let mut bad = v.clone();
                set(&mut bad, &pointer, other);
                assert_ne!(bad, v);
                assert!(
                    fixture_match::compare_report(&json!({}), &bad).is_err(),
                    "{pointer} = {other} accepted beside a range stop (was {word})"
                );
            }
        }
    }
    // Without a range stop the new word is not admitted anywhere.
    for s in [CONTEST, METRICS, REQUIRES] {
        let wide = s.replace("n: 0..0 = 0", "n: 0..1 = 0");
        assert_ne!(wide, s);
        let (v, _) = run(&wide);
        assert!(v["bounds"].get("range_violation_stops").is_none(), "{v}");
        fixture_match::compare_report(&json!({}), &v).unwrap();
        let ws = words(&v);
        assert!(
            !ws.is_empty() && ws.iter().all(|(_, w)| w == "complete"),
            "{ws:?}"
        );
        for (pointer, _) in ws {
            let mut bad = v.clone();
            set(&mut bad, &pointer, "range_stopped");
            assert_ne!(bad, v);
            assert!(
                fixture_match::compare_report(&json!({}), &bad).is_err(),
                "{pointer} = range_stopped accepted without a range stop"
            );
        }
    }
}

/// Rewrites every word of one table consistently (the table's own null
/// reasons too), so only the bounds/word gate can reject it.
fn rewrite(v: &mut Value, table: &str, word: &str) {
    // Metrics null every range (as the engine does); a contest route keeps
    // the never_created fact of a branch that did not file.
    fn ranges(v: &mut Value, word: &str) {
        match v {
            Value::Object(o) if o.contains_key("min") && o.contains_key("max") => {
                if o["reason"] != "never_created" {
                    o.insert("min".into(), Value::Null);
                    o.insert("max".into(), Value::Null);
                    o.insert("reason".into(), json!(word));
                }
            }
            Value::Object(o) => o.values_mut().for_each(|x| ranges(x, word)),
            Value::Array(a) => a.iter_mut().for_each(|x| ranges(x, word)),
            _ => {}
        }
    }
    match table {
        "contest" => {
            for row in v["contest_runtime"].as_array_mut().unwrap() {
                row["coverage"] = json!(word);
                for a in row["alternatives"].as_array_mut().unwrap() {
                    if a["filed"] == true {
                        ranges(&mut a["route_cost"], word);
                    }
                }
            }
        }
        _ => {
            v["metrics"]["status"] = json!(word);
            ranges(&mut v["metrics"], word);
        }
    }
}

#[test]
fn validators_reject_consistent_rewrites_of_the_range_word() {
    // Beside a range stop, a contest table rewritten wholly into the cut word.
    let (v, _) = run(CONTEST);
    fixture_match::compare_report(&json!({}), &v).unwrap();
    let mut bad = v.clone();
    rewrite(&mut bad, "contest", "truncated");
    assert_ne!(bad, v);
    assert_eq!(
        fixture_match::compare_report(&json!({}), &bad),
        Err("contest_runtime: range_stopped coverage/bounds mismatch".into())
    );
    // Without a range stop, a metrics table rewritten wholly into the range word.
    let wide = METRICS.replace("n: 0..0 = 0", "n: 0..1 = 0");
    assert_ne!(wide, METRICS);
    let (v, _) = run(&wide);
    fixture_match::compare_report(&json!({}), &v).unwrap();
    let mut bad = v.clone();
    rewrite(&mut bad, "metrics", "range_stopped");
    assert_ne!(bad, v);
    assert_eq!(
        fixture_match::compare_report(&json!({}), &bad),
        Err("metric range-stop status".into())
    );
    // Control: the same rewrite into the true word is accepted.
    let (v, _) = run(METRICS);
    let mut same = v.clone();
    rewrite(&mut same, "metrics", "range_stopped");
    fixture_match::compare_report(&json!({}), &same).unwrap();
}
