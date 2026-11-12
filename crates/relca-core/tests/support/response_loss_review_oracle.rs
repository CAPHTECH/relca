//! Review-contract predicates. They do not call production analysis or load captures.
#![allow(dead_code)]
use serde_json::{Value, json};

pub fn rows(v: &Value) -> &[Value] {
    v.as_array().expect("required array")
}

pub fn property<'a>(v: &'a Value, id: &str) -> &'a Value {
    let found: Vec<_> = rows(&v["properties"])
        .iter()
        .filter(|p| p["id"] == id)
        .collect();
    assert_eq!(found.len(), 1, "one property {id}");
    found[0]
}

pub fn warning(v: &Value, code: &str) -> bool {
    rows(&v["diagnostics"]).iter().any(|d| d["code"] == code)
}

fn strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|v| strings(v, out)),
        Value::Object(o) => o.iter().for_each(|(k, v)| {
            out.push(k.clone());
            strings(v, out);
        }),
        _ => {}
    }
}

fn explicit_mark(v: &Value, subject: &str) -> bool {
    let mut text = Vec::new();
    strings(v, &mut text);
    text.iter().any(|s| s.contains(subject))
        && text.iter().any(|s| s.contains("response_control"))
        && text
            .iter()
            .any(|s| s.contains("not_derived") || s.contains("unchecked"))
}

/// A general response_control notice cannot stand in for a subject-specific mark.
pub fn has_mark(v: &Value, subject: &str) -> bool {
    v["assumptions"]
        .as_array()
        .is_some_and(|a| a.iter().any(|r| explicit_mark(r, subject)))
        || ["agency", "response_analysis"].iter().any(|key| {
            v[key]["unchecked"]
                .as_array()
                .is_some_and(|a| a.iter().any(|r| explicit_mark(r, subject)))
        })
}

pub fn suppressed(v: &Value, code: &str) {
    assert!(
        !warning(v, code),
        "old warning is not response-aware: {code}"
    );
    assert!(
        has_mark(v, code),
        "missing reasoned unchecked mark for {code}: response_control"
    );
}

fn underived(row: &Value) -> bool {
    row["status"] == "not_derived" && row["reason"] == "response_control"
}

fn mentions_alias(v: &Value, alias: &str) -> bool {
    match v {
        Value::String(s) => s == alias,
        Value::Array(a) => a.iter().any(|v| mentions_alias(v, alias)),
        Value::Object(o) => o.iter().any(|(k, v)| {
            ["call", "targets", "judgements"].contains(&k.as_str()) && mentions_alias(v, alias)
                || v.is_object() && mentions_alias(v, alias)
                || v.is_array() && mentions_alias(v, alias)
        }),
        _ => false,
    }
}

/// Check every candidate row, including model-uncertainty target/correlation links.
pub fn alias_free(v: &Value, alias: &str) -> bool {
    for (table, list) in [
        ("lineage", ""),
        ("common_causes", "causes"),
        ("model_uncertainties", "items"),
    ] {
        let t = &v["agency"][table];
        if t.is_null() || underived(t) {
            continue;
        }
        let Some(rs) = (if list.is_empty() { t } else { &t[list] }).as_array() else {
            return false;
        };
        if rs.iter().any(|r| mentions_alias(r, alias) && !underived(r)) {
            return false;
        }
    }
    true
}

pub fn m1(v: &Value, kind: &str) {
    assert_eq!(
        v["verdict"], "satisfied",
        "front-end gate: {}",
        v["diagnostics"]
    );
    assert_eq!(v["bounds"]["truncated"], false);
    let (alias, time) = match kind {
        "common" => ("c1", 2),
        "judgement" => ("bV", 4),
        "target" => ("e", 4),
        _ => panic!("unknown M1 kind"),
    };
    let done = rows(&v["terminals"])
        .iter()
        .find(|t| t["end"] == "done")
        .unwrap();
    assert_eq!(done["elapsed"], json!({"min": time, "max": time}));
    assert_eq!(done["work"], json!({"min": time, "max": time}));
    assert!(
        alias_free(v, alias),
        "M1: non-executable alias {alias} leaked into Agency judgement/target"
    );
    if kind == "common" {
        let t = &v["agency"]["common_causes"];
        if !underived(t) {
            let causes = rows(&t["causes"]);
            assert_eq!(causes.len(), 1, "keep the real common cause");
            assert_eq!(causes[0]["latent"], "Invoice.difficulty");
            assert_eq!(
                rows(&causes[0]["targets"]).len(),
                2,
                "one real response attempt plus c2, not three with alias"
            );
        }
    } else {
        let table = &v["agency"]["model_uncertainties"];
        assert!(
            underived(table) || !rows(&table["items"]).is_empty(),
            "do not silently drop all real basis rows"
        );
        assert!(
            !rows(&v["agency"]["lineage"]).is_empty(),
            "keep real judgements"
        );
    }
}

pub fn via(v: &Value) {
    assert_eq!(v["verdict"], "satisfied", "{}", v["diagnostics"]);
    assert!(
        v.get("via_model_paths").is_none_or(Value::is_null),
        "review boundary has not derived response paths"
    );
    assert!(
        has_mark(v, "via_model_paths"),
        "missing reasoned not_derived via_model_paths: response_control"
    );
}

pub fn reachable(v: &Value, receipt: &str, fail: bool, violation: bool) -> i32 {
    let unknown = receipt == "unknown";
    let want = if unknown {
        "inconclusive"
    } else if fail {
        "violated"
    } else {
        "satisfied"
    };
    assert_eq!(
        property(v, "reachable end.done")["verdict"],
        want,
        "ReceiptUnknown is unresolved, not known unreachable"
    );
    let root = if violation { "violated" } else { want };
    assert_eq!(v["verdict"], root);
    assert_eq!(v["bounds"]["truncated"], false);
    assert!(!warning(v, "W-DEADLOCK-0"));
    if unknown {
        assert_eq!(property(v, "receipt_resolution")["verdict"], "inconclusive");
        assert!(rows(&v["terminals"]).is_empty());
        let cases = rows(&v["case_outcomes"]);
        assert_eq!(cases.len(), 1);
        let ts = rows(&cases[0]["terminals"]);
        assert_eq!(ts.len(), 1);
        assert_eq!(ts[0]["kind"], "receipt_unknown");
        for key in ["end", "completed_at", "elapsed", "work"] {
            assert!(ts[0].get(key).expect("required null").is_null());
        }
        for key in ["observed_until", "observed_elapsed", "observed_work"] {
            assert_eq!(ts[0][key], json!({"min":1,"max":1}));
        }
        let receipts = rows(&v["response_receipts"]);
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0]["receipt"], "unknown");
        assert!(receipts[0].get("observed_result").unwrap().is_null());
    } else {
        let ts = rows(&v["terminals"]);
        assert_eq!(ts.len(), 1);
        assert_eq!(ts[0]["end"], if fail { "rejected" } else { "done" });
        assert_eq!(ts[0]["elapsed"], json!({"min":1,"max":1}));
        assert_eq!(ts[0]["work"], json!({"min":1,"max":1}));
    }
    match root {
        "violated" => 1,
        "inconclusive" => 2,
        "satisfied" => 0,
        _ => unreachable!(),
    }
}
