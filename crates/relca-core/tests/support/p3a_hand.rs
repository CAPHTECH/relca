//! Independent arithmetic assertions. No captured outputs or snapshot lookup.
#![allow(dead_code)]
use serde_json::{Value, json};
pub fn rows(v: &Value) -> &[Value] {
    v.as_array().expect("required array")
}
pub fn row<'a>(v: &'a Value, k: &str, n: &str) -> &'a Value {
    let rs: Vec<_> = rows(v).iter().filter(|v| v[k] == n).collect();
    assert_eq!(rs.len(), 1, "unique {k}={n}: {v}");
    rs[0]
}
pub fn range(a: u64, b: u64) -> Value {
    json!({"min":a,"max":b})
}
pub fn scalar(n: &str) -> Value {
    json!({"min":n,"max":n,"reason":null})
}
pub fn probability(v: &Value, n: &str) {
    assert_eq!(v, &json!({"min":n,"max":n}));
}
pub fn verdict(v: &Value, n: &str) {
    assert_eq!(v["verdict"], n, "{}", v["diagnostics"]);
}
pub fn leaf(name: &str) -> &'static str {
    match name {
        "modeled-resend" => "violated",
        "g-unknown" | "g-probability" | "k8-g-unknown" | "k8-g-clock" => "inconclusive",
        _ => "satisfied",
    }
}
pub fn observed(v: &Value, t: u64, w: u64) {
    for k in ["completed_at", "elapsed", "work", "result_at"] {
        assert_eq!(v.get(k), Some(&Value::Null), "unknown {k}");
    }
    assert_eq!(v["observed_until"], range(t, t));
    assert_eq!(v["observed_elapsed"], range(t, t));
    assert_eq!(v["observed_work"], range(w, w));
}
pub fn children(v: &Value, cmp: bool) -> Vec<&Value> {
    if cmp {
        rows(&v["plans"]).iter().map(|p| &p["report"]).collect()
    } else {
        vec![&v["from"]["report"], &v["to"]["report"]]
    }
}
pub fn comparison(v: &Value, name: &str, cmp: bool) {
    let cs = children(v, cmp);
    assert_eq!(cs.len(), 2);
    for c in cs {
        verdict(c, leaf(name));
        assert_eq!(c["bounds"]["truncated"], name == "k8-g-clock");
        assert_eq!(
            row(&c["properties"], "id", "receipt_resolution")["verdict"],
            if matches!(name, "g-resend" | "k8-g-resend") {
                "satisfied"
            } else {
                "inconclusive"
            }
        );
        let receipts = rows(&c["response_receipts"]);
        for r in receipts {
            assert_eq!(r["case_id"], 1);
            assert!(r["command"].is_string());
            assert!([1, 2].contains(&r["attempt"].as_u64().unwrap()));
        }
        if name != "k8-g-clock" {
            assert!(receipts.iter().any(|r| r["attempt"] == 2));
        }
    }
    verdict(v, leaf(name));
    let properties = if cmp {
        &v["tables"]["properties"]
    } else {
        &v["properties"]
    };
    assert_eq!(
        rows(properties)
            .iter()
            .filter(|p| p["id"] == "receipt_resolution")
            .count(),
        1
    );
    if name == "k8-g-clock" {
        return;
    }
    let unknown = matches!(
        name,
        "g-unknown" | "modeled-resend" | "g-probability" | "k8-g-unknown"
    );
    if cmp {
        let ts = rows(&v["tables"]["terminals"]);
        assert_eq!(
            ts.iter().filter(|t| t["kind"] == "receipt_unknown").count(),
            1,
            "Q1 applicability retains the row even when no unknown was reported"
        );
        if !unknown {
            for c in rows(&row(&v["tables"]["terminals"], "kind", "receipt_unknown")["values"]) {
                assert_eq!(c["status"], "not_reported");
                assert_eq!(c["value"], Value::Null);
            }
        }
        if unknown {
            let u = row(&v["tables"]["terminals"], "kind", "receipt_unknown");
            assert_eq!(u["case_id"], 1);
            assert_eq!(u["end"], Value::Null);
            for c in rows(&u["values"]) {
                assert_eq!(c["status"], "reported");
                observed(
                    &c["value"],
                    if name.starts_with("k8-") { 3 } else { 2 },
                    if name.starts_with("k8-") { 3 } else { 2 },
                );
                if name == "g-probability" {
                    probability(&c["value"]["probability"], "1/100");
                }
            }
        }
        // The declared exhausted End must survive even when no path reaches it.
        assert!(ts.iter().any(|t| t["end"] == "failed"));
        for (end, t, w, p) in match name {
            "g-resend" => vec![("duplicate", 2, 2, None)],
            "g-probability" => vec![
                ("posted", 1, 1, Some("9/10")),
                ("duplicate", 2, 2, Some("9/100")),
            ],
            "k8-g-resend" => vec![("done", 3, 3, None)],
            _ => vec![],
        } {
            let r = row(&v["tables"]["terminals"], "end", end);
            for c in rows(&r["values"]) {
                assert_eq!(c["value"]["elapsed"], range(t, t));
                assert_eq!(c["value"]["work"], range(w, w));
                if let Some(p) = p {
                    probability(&c["value"]["probability"], p);
                }
            }
        }
    } else {
        let cases = rows(&v["case_outcomes"]);
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0]["case_id"], 1);
        let us: Vec<_> = rows(&cases[0]["terminals"])
            .iter()
            .filter(|r| r["kind"] == "receipt_unknown")
            .collect();
        assert_eq!(us.len(), usize::from(unknown));
        if unknown {
            for side in ["from", "to"] {
                observed(
                    &us[0][side],
                    if name.starts_with("k8-") { 3 } else { 2 },
                    if name.starts_with("k8-") { 3 } else { 2 },
                );
            }
        }
    }
}
pub fn waiting(v: &Value, path: &str, sample: bool) {
    assert_ne!(v["verdict"], "unsupported", "{}", v["diagnostics"]);
    assert_ne!(v["verdict"], "invalid");
    let m = &v["metrics"];
    assert!(m.is_object());
    let c = &m["cases"][0];
    assert!(
        c.get("dwell").is_none(),
        "ReceiptUnknown is not a horizon dwell"
    );
    let count = |n: &str| {
        if sample {
            json!({"mean":n,"min":n,"max":n,"samples":4,"reason":null})
        } else {
            scalar(n)
        }
    };
    for (key, n) in [
        ("offered", "1"),
        ("received", "1"),
        ("completed", if path == "UU" { "0" } else { "1" }),
        ("unfinished", if path == "UU" { "1" } else { "0" }),
        (
            "receipt_unknown_count",
            if path == "UU" { "1" } else { "0" },
        ),
    ] {
        assert_eq!(
            m["overall"]["counts"][key]["stats"],
            count(n),
            "count {key}"
        );
    }
    let tasks = rows(&c["tasks"]);
    assert_eq!(tasks.len(), 2, "declared attempt inventory, not alias");
    assert!(tasks.iter().all(|r| r["binding"] != "ack"));
    assert_eq!(
        row(&m["capabilities"], "capability", "Ledger.post")["starts"],
        count(if path == "D" { "1" } else { "2" })
    );
    if path != "UU" {
        let t = row(
            &c["terminals"],
            "end",
            if path == "D" { "posted" } else { "duplicate" },
        );
        assert_eq!(t["lead_time"], count(if path == "D" { "1" } else { "5" }));
        let b = &t["breakdown"];
        assert_eq!(b["in_service"], count(if path == "D" { "1" } else { "2" }));
        assert_eq!(b["intentional"], count(if path == "D" { "0" } else { "3" }));
        for k in ["external", "contest_route", "unattributed"] {
            assert_eq!(b[k], count("0"));
        }
        for k in ["actor_capacity", "resource", "executor", "combined"] {
            assert_eq!(b["queue"][k], count("0"));
        }
    } else {
        for t in rows(&c["terminals"]) {
            assert!(t["lead_time"]["min"].is_null());
            assert!(t["breakdown"]["in_service"]["min"].is_null());
            assert_eq!(
                t["lead_time"]["reason"],
                if sample { "no_samples" } else { "no_terminal" }
            );
            assert_eq!(
                t["breakdown"]["in_service"]["reason"],
                if sample { "no_samples" } else { "no_terminal" }
            );
        }
        if !sample {
            let u = row(
                &v["case_outcomes"][0]["terminals"],
                "kind",
                "receipt_unknown",
            );
            observed(u, 5, 2);
        } else {
            assert_eq!(v["case_outcomes"][0]["unfinished_count"], 4);
            assert_eq!(
                row(
                    &v["case_outcomes"][0]["terminals"],
                    "kind",
                    "receipt_unknown"
                )["count"],
                4
            );
        }
    }
    assert_eq!(
        m["overall"]["work"],
        count(if path == "D" { "1" } else { "2" })
    );
    assert_eq!(
        m["overall"]["observed_cost_total"],
        count(if path == "D" { "3" } else { "6" })
    );
}
