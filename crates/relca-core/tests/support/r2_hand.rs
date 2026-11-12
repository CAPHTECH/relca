//! Numbers fixed from ADR 0065, not captured stdout. Also used by the real CLI.
#![allow(dead_code)]
use serde_json::{Value, json};
pub const RULES: &str = "response-recovery-p3a-1";
pub const SCOPE: &str = "joint modeled recovery paths; query evidence is not a business decision or a received original acknowledgement";
pub const BUNDLE: &str = "relca-conclusions-p3a-recovery-1";
pub fn rows(v: &Value) -> &[Value] {
    v.as_array().expect("required array")
}
pub fn row<'a>(v: &'a Value, key: &str, name: &str) -> &'a Value {
    let rs: Vec<_> = rows(v).iter().filter(|r| r[key] == name).collect();
    assert_eq!(rs.len(), 1, "unique {key}={name}: {v}");
    rs[0]
}
pub fn point(n: u64) -> Value {
    json!({"min":n,"max":n})
}
pub fn scalar(n: &str) -> Value {
    json!({"min":n,"max":n,"reason":null})
}
pub fn verdict(v: &Value, n: &str) {
    assert_eq!(v["verdict"], n, "{}", v["diagnostics"]);
}
pub fn admitted(v: &Value) {
    assert!(
        !matches!(v["verdict"].as_str(), Some("invalid" | "unsupported")),
        "positive control refused: {}",
        v["diagnostics"]
    );
}
pub fn rejection(v: &Value, code: &str, reason: &str) {
    verdict(
        v,
        if code.starts_with("U-") {
            "unsupported"
        } else {
            "invalid"
        },
    );
    let errors: Vec<_> = rows(&v["diagnostics"])
        .iter()
        .filter(|r| r["severity"] == "error")
        .collect();
    assert_eq!(errors.len(), 1, "one intended gate: {}", v["diagnostics"]);
    assert_eq!(errors[0]["code"], code, "{}", v["diagnostics"]);
    if !reason.is_empty() {
        let msg = errors[0]["message"].as_str().unwrap();
        assert!(
            msg == reason || msg.starts_with(&format!("{reason}:")),
            "closed reason prefix {reason}: {msg}"
        );
    }
    assert!(errors[0]["span"].is_object(), "source-local rejection");
}
/// (observed time, work, normal End, evidence, decision, resolve).
pub type Timeline = (
    u64,
    u64,
    Option<&'static str>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
);
pub fn timeline(id: &str) -> Timeline {
    match id {
        "R01" => (7, 7, Some("shipped"), Some(3), Some(6), Some(6)),
        "R02" => (3, 3, None, None, None, None),
        "R03" => (1, 1, None, None, None, None),
        "R04" => (9, 9, Some("refunded"), Some(3), Some(6), Some(9)),
        "R05" => (10, 10, Some("refunded"), Some(3), Some(6), Some(10)),
        "R06" => (4, 4, None, None, None, None),
        "R07" => (9, 7, Some("shipped"), Some(5), Some(8), Some(8)),
        "R08" => (8, 8, Some("shipped"), Some(4), Some(7), Some(7)),
        "R09" => (8, 8, None, Some(3), Some(6), None),
        _ => panic!("unknown hand row {id}"),
    }
}
pub fn count(v: &Value, name: &str, n: &str) {
    assert_eq!(
        v["metrics"]["overall"]["counts"][name]["stats"],
        scalar(n),
        "{name}"
    );
}
pub fn check(v: &Value, id: &str) {
    let (t, w, end, evidence, decision, resolved) = timeline(id);
    let complete = end.is_some();
    // Refund-only contracts are vacuous in the proceed model. Preserve the
    // existing root fold instead of silently removing those contracts.
    verdict(
        v,
        if !complete {
            "inconclusive"
        } else if matches!(id, "R04" | "R05") {
            "satisfied"
        } else {
            "vacuous"
        },
    );
    assert_eq!(v["bounds"]["truncated"], false);
    assert_eq!(
        row(&v["properties"], "id", "receipt_resolution")["verdict"],
        if complete {
            "satisfied"
        } else {
            "inconclusive"
        }
    );
    let cases = rows(&v["case_outcomes"]);
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0]["case_id"], 1);
    let ts = rows(&cases[0]["terminals"]);
    assert_eq!(ts.len(), 1);
    let term = &ts[0];
    assert_eq!(
        term["kind"],
        if complete { "end" } else { "receipt_unknown" }
    );
    assert_eq!(term.get("end"), Some(&json!(end)));
    if complete {
        for (k, n) in [("completed_at", t), ("elapsed", t), ("work", w)] {
            assert_eq!(term[k], point(n), "{id} {k}");
        }
    } else {
        for k in ["completed_at", "elapsed", "work", "result_at"] {
            assert_eq!(term.get(k), Some(&Value::Null));
        }
        for (k, n) in [
            ("observed_until", t),
            ("observed_elapsed", t),
            ("observed_work", w),
        ] {
            assert_eq!(term[k], point(n));
        }
    }
    for (k, n) in [
        ("offered", "1"),
        ("received", "1"),
        ("completed", if complete { "1" } else { "0" }),
        ("unfinished", if complete { "0" } else { "1" }),
        ("receipt_unknown_count", if complete { "0" } else { "1" }),
    ] {
        count(v, k, n);
    }
    assert_eq!(v["metrics"]["overall"]["work"], scalar(&w.to_string()));
    assert_eq!(
        v["metrics"]["overall"]["observed_cost_total"]["reason"],
        "not_configured"
    );
    assert_eq!(
        row(&v["quality"]["cohort"], "name", "ChargedPerArrival")["denominator"]["count"],
        1
    );
    assert_eq!(
        row(&v["quality"]["cohort"], "name", "ChargedPerArrival")["value"]["lower"],
        "1"
    );
    let receipts = rows(&v["response_receipts"]);
    assert_eq!(receipts.len(), if id == "R08" { 2 } else { 1 });
    for (index, r) in receipts.iter().enumerate() {
        assert_eq!(r["command"], "pay");
        assert_eq!(r["receipt"], "unknown");
        assert_eq!(r.get("observed_result"), Some(&Value::Null));
        assert_eq!(r["resolved_effect"], false);
        assert_eq!(r["at"], point(index as u64 + 1));
    }
    if id == "R03" {
        assert!(v.get("response_recovery").is_none_or(Value::is_null));
        return;
    }
    let recovery = &v["response_recovery"];
    assert_eq!(recovery["rules"], RULES);
    assert_eq!(recovery["scope"], SCOPE);
    assert_eq!(recovery["coverage"], "complete");
    assert_eq!(v["response_analysis"]["rules"], "response-p3a-recovery-1");
    for checked in [
        "exact_g_query",
        "recovery_evidence_matching",
        "recovery_incomplete_case",
    ] {
        assert!(
            rows(&v["response_analysis"]["checked"])
                .iter()
                .any(|v| v == checked)
        );
    }
    for unchecked in [
        "general_status_query",
        "asynchronous_external_task",
        "correlated_response_loss",
        "unbounded_retry",
        "fairness",
        "non_idempotent_recovery",
    ] {
        assert!(
            rows(&v["response_analysis"]["unchecked"])
                .iter()
                .any(|v| v == unchecked)
        );
    }
    assert!(!rows(&recovery["paths"]).is_empty());
    for path in rows(&recovery["paths"]) {
        let cs = rows(&path["commands"]);
        assert_eq!(cs.len(), 1);
        let c = &cs[0];
        assert_eq!(c["case_id"], 1);
        assert_eq!(c["saved_key"], 1);
        assert_eq!(c["command"], "pay");
        assert_eq!(c["provider"], "Psp");
        assert_eq!(c["capability"], "Psp.charge_keyed");
        assert_eq!(c["unresolved"], !complete);
        assert_eq!(
            c["stop_reason"],
            if complete { "end" } else { "receipt_unknown" }
        );
        let qs = rows(&c["queries"]);
        assert_eq!(qs.len(), 1);
        let q = &qs[0];
        assert_eq!(
            q["at"],
            if id == "R07" {
                5
            } else if id == "R08" {
                4
            } else {
                3
            }
        );
        assert_eq!(
            q["status"],
            if evidence.is_some() {
                "committed_once"
            } else {
                "still_unknown"
            }
        );
        assert_eq!(q["evidence"].is_null(), evidence.is_none());
        if let Some(at) = decision {
            assert_eq!(row(&c["tasks"], "binding", "decision")["finished_at"], at);
        } else {
            assert!(!rows(&c["tasks"]).iter().any(|r| r["binding"] == "decision"));
        }
        let rs = rows(&c["resolutions"]);
        assert_eq!(rs.len(), usize::from(resolved.is_some()));
        if let Some(at) = resolved {
            assert_eq!(rs[0]["at"], at);
            assert_eq!(rs[0]["fact"], "committed_once");
            assert_eq!(rs[0]["query_ref"], q["id"]);
        }
        if id == "R05" {
            assert_eq!(row(&c["tasks"], "binding", "note")["finished_at"], 10);
        }
        let expected: Vec<(&str, u64, u64, Option<&str>)> = match id {
            "R01" => vec![
                ("status", 1, 3, None),
                ("decision", 3, 6, Some("c")),
                ("sh", 6, 7, None),
            ],
            "R02" => vec![("status", 1, 3, None)],
            "R04" => vec![
                ("status", 1, 3, None),
                ("decision", 3, 6, Some("c")),
                ("d", 6, 7, Some("c")),
                ("v", 7, 8, Some("f")),
                ("r", 8, 9, None),
            ],
            "R05" => vec![
                ("status", 1, 3, None),
                ("decision", 3, 6, Some("c")),
                ("d", 6, 7, Some("c")),
                ("v", 7, 8, Some("f")),
                ("r", 8, 9, None),
                ("note", 9, 10, Some("c")),
            ],
            "R06" => vec![("status", 1, 3, None), ("note", 3, 4, Some("c"))],
            "R07" => vec![
                ("status", 3, 5, None),
                ("decision", 5, 8, Some("c")),
                ("sh", 8, 9, None),
            ],
            "R08" => vec![
                ("status", 2, 4, None),
                ("decision", 4, 7, Some("c")),
                ("sh", 7, 8, None),
            ],
            "R09" => vec![
                ("status", 1, 3, None),
                ("decision", 3, 6, Some("c")),
                ("d", 6, 7, Some("c")),
                ("v", 7, 8, Some("f")),
            ],
            _ => unreachable!(),
        };
        assert_eq!(
            rows(&c["tasks"]).len(),
            expected.len(),
            "actual Task inventory, no aliases or resolve Task"
        );
        for (name, start, finish, executor) in expected {
            let task = row(&c["tasks"], "binding", name);
            assert_eq!(task["started_at"], start);
            assert_eq!(task["finished_at"], finish);
            assert_eq!(task.get("executor"), Some(&json!(executor)));
        }
    }
    assert_eq!(v["ledger"]["derivation"]["bundle"], BUNDLE);
    assert!(
        v["ledger"]["subjects"]
            .to_string()
            .contains("/response_recovery")
    );
    assert!(
        rows(&v["ledger"]["derivation"]["components"])
            .iter()
            .any(|c| c.to_string().contains("/response_recovery/rules")
                && c.to_string().contains(RULES))
    );
}
pub fn strict_roundtrip(v: &Value) {
    let r: relca_core::report::Report =
        serde_json::from_value(v.clone()).expect("typed valid control");
    assert_eq!(json!(r), *v);
}
pub fn waiting(v: &Value, id: &str) {
    let (t, w, end, _, _, _) = timeline(id);
    let ts = &v["metrics"]["cases"][0]["terminals"];
    if let Some(end) = end {
        let b = &row(ts, "end", end)["breakdown"];
        assert_eq!(b["in_service"], scalar(&w.to_string()));
        assert_eq!(b["intentional"], scalar(&(t - w).to_string()));
        for k in ["external", "contest_route", "unattributed"] {
            assert_eq!(b[k], scalar("0"));
        }
        for k in ["actor_capacity", "resource", "executor", "combined"] {
            assert_eq!(b["queue"][k], scalar("0"));
        }
    } else {
        for r in rows(ts) {
            assert_eq!(r["lead_time"]["reason"], "no_terminal");
            assert!(r["lead_time"]["min"].is_null());
        }
    }
}
