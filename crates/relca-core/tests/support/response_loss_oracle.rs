//! Specification-derived assertions, shared by core and real CLI, not snapshots.
#![allow(dead_code)]
use serde_json::{Value, json};

pub fn required<'a>(v: &'a Value, key: &str) -> &'a Value {
    v.get(key)
        .unwrap_or_else(|| panic!("missing required key {key}"))
}
pub fn rows(v: &Value) -> &[Value] {
    v.as_array().expect("required array")
}
pub fn row<'a>(v: &'a Value, key: &str, value: &str) -> &'a Value {
    let found: Vec<_> = rows(v)
        .iter()
        .filter(|r| required(r, key) == value)
        .collect();
    assert_eq!(found.len(), 1, "unique row {key}={value}");
    found[0]
}
pub fn point(n: u64) -> Value {
    json!({"min":n,"max":n})
}
pub fn metric(n: &str) -> Value {
    json!({"min":n,"max":n,"reason":null})
}
pub fn probability(v: &Value, id: &str, lo: &str, hi: &str) {
    let p = row(required(v, "properties"), "id", id);
    assert_eq!(
        required(p, "probability"),
        &json!({"min":lo,"max":hi}),
        "{id}"
    );
}
pub fn verdict(v: &Value, expected: &str) {
    assert_eq!(
        required(v, "verdict"),
        expected,
        "front-end diagnostics: {}",
        v["diagnostics"]
    );
}
pub fn once(v: &Value, expected: &str) {
    let found: Vec<_> = rows(required(v, "properties"))
        .iter()
        .filter(|p| p["id"].as_str().unwrap().ends_with(" at_most_once"))
        .collect();
    assert_eq!(found.len(), 1, "exactly one at_most_once contract");
    assert_eq!(required(found[0], "verdict"), expected);
}
pub fn terminal(v: &Value, end: &str, elapsed: u64, work: u64) {
    let t = row(required(v, "terminals"), "end", end);
    assert_eq!(required(t, "elapsed"), &point(elapsed), "{end} elapsed");
    assert_eq!(required(t, "work"), &point(work), "{end} work");
}
pub fn receipt_resolution(v: &Value, expected: &str) {
    let p = row(required(v, "properties"), "id", "receipt_resolution");
    assert_eq!(required(p, "verdict"), expected);
}

pub fn receipt(v: &Value, case: u64, attempt: u64, status: &str, result: Option<&str>, at: u64) {
    let hits: Vec<_> = rows(required(v, "response_receipts"))
        .iter()
        .filter(|r| {
            r["case_id"] == case
                && r["attempt"] == attempt
                && r["receipt"] == status
                && r["observed_result"] == json!(result)
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "receipt ({case},{attempt},{status},{result:?})"
    );
    let r = hits[0];
    assert_eq!(required(r, "at"), &point(at));
    assert!(required(r, "command").is_string());
    assert!(required(r, "capability").is_string());
    assert!(required(r, "resolved_effect").is_boolean());
    if status == "unknown" {
        assert!(
            required(r, "observed_result").is_null(),
            "hidden actual result leaked"
        );
    }
}

pub fn unknown(v: &Value, at: u64, work: u64) {
    assert!(
        rows(required(v, "terminals")).is_empty(),
        "unknown became a normal End"
    );
    let cases = rows(required(v, "case_outcomes"));
    assert_eq!(cases.len(), 1);
    let ts = rows(required(&cases[0], "terminals"));
    assert_eq!(ts.len(), 1);
    let t = &ts[0];
    assert_eq!(required(t, "kind"), "receipt_unknown");
    for k in ["end", "completed_at", "elapsed", "work"] {
        assert!(required(t, k).is_null(), "unknown has completed {k}");
    }
    assert_eq!(required(t, "observed_until"), &point(at));
    assert_eq!(required(t, "observed_elapsed"), &point(at));
    assert_eq!(required(t, "observed_work"), &point(work));
    assert_eq!(
        v["bounds"]["truncated"], false,
        "unknown is not a computational cut"
    );
    receipt_resolution(v, "inconclusive");
    assert!(
        !rows(required(v, "diagnostics"))
            .iter()
            .any(|d| d["code"] == "W-DEADLOCK-0")
    );
}

pub fn check(v: &Value, name: &str) {
    let expected = match name {
        "modeled-resend" | "correlated-tight" | "known-fail" | "requires-refusal" => "violated",
        "g-unknown" | "single-unknown" | "g-nondet" | "g-default" | "g-probability"
        | "g-probability-one" | "g-clock" | "k8-g-unknown" | "k8-g-clock" | "max-one"
        | "requires-allowed" => "inconclusive",
        _ => "satisfied",
    };
    verdict(v, expected);
    if ["g-clock", "k8-g-clock"].contains(&name) {
        assert_eq!(v["bounds"]["truncated"], true);
        return;
    }
    assert_eq!(v["bounds"]["truncated"], false);
    match name {
        "correlated" | "correlated-tight" | "backoff" | "independent" | "max-thirty-two" => {
            let max = if name == "max-thirty-two" { 32 } else { 2 };
            let elapsed = if name == "backoff" { 5 } else { max };
            if name == "independent" {
                let ok = row(&v["terminals"], "end", "ok");
                assert_eq!(ok["elapsed"], json!({"min":1,"max":2}));
                assert_eq!(ok["work"], json!({"min":1,"max":2}));
            } else {
                terminal(v, "ok", 1, 1);
            }
            terminal(v, "failed", elapsed, max);
            let failure = if name == "independent" {
                "1/100"
            } else {
                "1/10"
            };
            probability(v, "probability reached #1", failure, failure);
            let failed = row(&v["terminals"], "end", "failed");
            assert_eq!(
                required(failed, "probability"),
                &json!({"min":failure,"max":failure})
            );
        }
        "known-fail" => {
            terminal(v, "failed", 2, 2);
            assert_eq!(rows(&v["terminals"]).len(), 1);
            probability(v, "probability reached #1", "1", "1");
        }
        "known-pass" => {
            terminal(v, "ok", 1, 1);
            assert_eq!(rows(&v["terminals"]).len(), 1);
            probability(v, "probability reached #1", "0", "0");
        }
        "requires-refusal" => {
            once(v, "vacuous");
            terminal(v, "duplicate", 0, 0);
            probability(v, "probability reached #1", "0", "0");
        }
        "k8-g-resend" | "k8-g-unknown" => {
            once(v, "satisfied");
            probability(v, "probability reached #1", "1/50", "1/50");
            probability(v, "probability reached #2", "1", "1");
            receipt(v, 1, 1, "unknown", None, 2);
            if name == "k8-g-unknown" {
                unknown(v, 3, 3);
                receipt(v, 1, 2, "unknown", None, 3);
            } else {
                terminal(v, "done", 3, 3);
                receipt(v, 1, 2, "delivered", Some("rejected_duplicate"), 3);
            }
        }
        "g-nondet" | "g-default" | "g-probability" => {
            once(v, "satisfied");
            receipt_resolution(v, "inconclusive");
            terminal(v, "posted", 1, 1);
            terminal(v, "duplicate", 2, 2);
            receipt(v, 1, 1, "delivered", Some("committed"), 1);
            receipt(v, 1, 1, "unknown", None, 1);
            receipt(v, 1, 2, "delivered", Some("rejected_duplicate"), 2);
            receipt(v, 1, 2, "unknown", None, 2);
            if name == "g-probability" {
                assert_eq!(
                    row(&v["terminals"], "end", "posted")["probability"],
                    json!({"min":"9/10","max":"9/10"})
                );
                assert_eq!(
                    row(&v["terminals"], "end", "duplicate")["probability"],
                    json!({"min":"9/100","max":"9/100"})
                );
                let ts = &v["case_outcomes"][0]["terminals"];
                assert_eq!(
                    row(ts, "kind", "receipt_unknown")["probability"],
                    json!({"min":"1/100","max":"1/100"})
                );
                assert!(
                    rows(&v["assumptions"])
                        .iter()
                        .any(|a| a.to_string().contains("independent receipt loss")),
                    "basis lost"
                );
            } else {
                assert!(
                    rows(&v["response_receipts"])
                        .iter()
                        .all(|r| r.get("probability").is_none()),
                    "nondeterminism assigned a distribution"
                );
            }
        }
        "g-arrivals" => {
            once(v, "satisfied");
            let cs = rows(required(v, "case_outcomes"));
            assert_eq!(cs.len(), 2);
            for (id, arrival) in [(1, 0), (2, 4)] {
                let c = cs.iter().find(|c| c["case_id"] == id).unwrap();
                assert_eq!(c["arrival"], arrival);
                let t = row(&c["terminals"], "end", "duplicate");
                assert_eq!(t["completed_at"], point(arrival + 2));
                assert_eq!(t["elapsed"], point(2));
                assert_eq!(t["work"], point(2));
                receipt(v, id, 1, "unknown", None, arrival + 1);
                receipt(
                    v,
                    id,
                    2,
                    "delivered",
                    Some("rejected_duplicate"),
                    arrival + 2,
                );
            }
        }
        _ => {
            once(
                v,
                if name == "modeled-resend" {
                    "violated"
                } else {
                    "satisfied"
                },
            );
            let is_unknown = [
                "g-unknown",
                "single-unknown",
                "max-one",
                "g-probability-one",
                "modeled-resend",
                "requires-allowed",
            ]
            .contains(&name);
            let one = [
                "single-unknown",
                "max-one",
                "requires-allowed",
                "single-delivered",
                "g-first-delivered",
                "modeled-first-delivered",
                "g-probability-zero",
            ]
            .contains(&name);
            let n = if one { 1 } else { 2 };
            if is_unknown {
                unknown(v, n, n);
            } else {
                terminal(v, if one { "posted" } else { "duplicate" }, n, n);
            }
            probability(v, "probability reached #1", "1", "1");
            probability(
                v,
                "probability reached #2",
                if name == "modeled-resend" { "1" } else { "0" },
                if name == "modeled-resend" { "1" } else { "0" },
            );
            if !one {
                receipt(v, 1, 1, "unknown", None, 1);
                receipt(
                    v,
                    1,
                    2,
                    if is_unknown && name != "modeled-resend" {
                        "unknown"
                    } else {
                        "delivered"
                    },
                    if name == "modeled-resend" {
                        Some("committed")
                    } else if is_unknown {
                        None
                    } else {
                        Some("rejected_duplicate")
                    },
                    2,
                );
            }
        }
    }
}

pub fn static_failure(v: &Value, code: &str, reason: &str) {
    verdict(
        v,
        if code.starts_with("U-") {
            "unsupported"
        } else {
            "invalid"
        },
    );
    assert!(
        rows(required(v, "diagnostics"))
            .iter()
            .any(|d| d["code"] == code),
        "missing {code}: {}",
        v["diagnostics"]
    );
    if !reason.is_empty() {
        assert!(
            v.to_string().contains(reason),
            "missing semantic reason {reason}"
        );
    }
    assert_eq!(required(&v["bounds"], "truncated"), &Value::Null);
    assert!(rows(required(v, "terminals")).is_empty());
}

pub fn mutation(v: &Value, kind: &str, result: &str, reason: Option<&str>) {
    let r = row(required(v, "mutations"), "kind", kind);
    assert_eq!(r["status"], "generated");
    assert_eq!(rows(&r["mutants"]).len(), 1);
    let m = &r["mutants"][0];
    assert_eq!(m["result"], result, "{kind}");
    assert_eq!(required(m, "reason"), &json!(reason));
    assert_eq!(
        m["operator"],
        if kind == "command_resent" {
            "lose_response_and_resend"
        } else {
            "lose_response"
        }
    );
    if kind == "receipt_unknown" && result == "detected" {
        assert!(
            rows(&m["assigned_changes"])
                .iter()
                .any(|c| c["direction"] == "held" && c["check"] == "receipt_unknown")
        );
        unknown(&m["report"], 1, 1);
    }
}

pub fn indicators(v: &Value, unknown_result: bool) {
    verdict(
        v,
        if unknown_result {
            "inconclusive"
        } else {
            "satisfied"
        },
    );
    let overall = required(required(v, "metrics"), "overall");
    assert_eq!(overall["work"], metric("1"));
    assert_eq!(overall["observed_cost_total"], metric("3")); // 1 minute + one start at 2
    let counts = required(overall, "counts");
    for key in ["offered", "received"] {
        assert_eq!(counts[key]["stats"], metric("1"));
    }
    assert_eq!(
        counts["completed"]["stats"],
        metric(if unknown_result { "0" } else { "1" })
    );
    assert_eq!(
        counts["unfinished"]["stats"],
        metric(if unknown_result { "1" } else { "0" })
    );
    assert_eq!(
        required(counts, "receipt_unknown_count")["stats"],
        metric(if unknown_result { "1" } else { "0" })
    );
    assert_eq!(counts["deadlocked"]["stats"], metric("0"));
    let q = required(v, "quality");
    let registered = row(&q["properties"], "name", "Registered");
    assert_eq!(registered["expected_count"]["lower"], "1");
    assert_eq!(registered["expected_count"]["upper"], "1");
    let rate = row(&q["cohort"], "name", "RegisteredPerArrival");
    assert_eq!(rate["value"]["lower"], "1");
    assert_eq!(rate["value"]["upper"], "1");
    let unknown_row = row(&q["outcomes_per_arrival"], "kind", "receipt_unknown");
    let n = if unknown_result { "1" } else { "0" };
    assert_eq!(unknown_row["rate"]["lower"], n);
    assert_eq!(unknown_row["rate"]["upper"], n);
    if unknown_result {
        unknown(v, 1, 1);
    }
}
