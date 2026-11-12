//! Independent raw-result checks. No production conformance/parser/monitor calls.
use serde_json::{Value, json};
use std::collections::BTreeSet;
#[path = "ledger_shape.rs"]
pub mod ledger_shape;

const REQUIRED: &[&str] = &[
    "ledger",
    "uncertainty",
    "verdict",
    "conformance",
    "mode",
    "schema",
    "scope",
    "model",
    "log",
    "configuration",
    "scenario",
    "cut",
    "cut_context",
    "assumptions",
    "checks",
    "cases",
    "summary",
    "unmapped_actor",
    "unmapped_operation",
    "not_checked",
    "limits",
    "diagnostics",
];
const LOG_CODES: &[&str] = &[
    "E-LOG-0", "E-LOG-1", "E-LOG-2", "U-LOG-0", "U-LOG-1", "W-LOG-0", "W-LOG-1", "W-LOG-2",
    "W-LOG-3", "W-LOG-4",
];
fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    v[key]
        .as_array()
        .ok_or_else(|| format!("{key}: required array"))
}
fn text(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.is_empty())
}
fn tri(v: &Value) -> bool {
    matches!(v.as_str(), Some("conformant" | "violating" | "unknown"))
}
fn span(v: &Value) -> bool {
    v.is_null() || matches!((v["start"].as_u64(),v["end"].as_u64()),(Some(a),Some(b)) if a<=b)
}
fn nullable_text(v: &Value) -> bool {
    v.is_null() || text(v)
}

pub fn validate(v: &Value, exit: i32) -> Result<(), String> {
    let object = v.as_object().ok_or("report: object required")?;
    for key in REQUIRED {
        require(object.contains_key(*key), format!("missing root {key}"))?;
    }
    ledger_shape::validate(&v["ledger"])?;
    ledger_shape::uncertainty_wire::validate_host(v)?;
    require(v["mode"] == "log_conformance", "wrong mode")?;
    require(v["schema"] == "relca-conformance/v1", "wrong result schema")?;
    require(
        v["scope"] == "observable_event_contracts_v1",
        "wrong result scope",
    )?;
    require(tri(&v["conformance"]), "conformance is not three-valued")?;
    let (state, expected_exit) = match v["verdict"].as_str() {
        Some("satisfied") => ("conformant", 0),
        Some("violated") => ("violating", 1),
        Some("inconclusive") => ("unknown", 2),
        Some("invalid") => ("unknown", 3),
        Some("unsupported") => ("unknown", 4),
        _ => return Err("bad verdict, including vacuous".into()),
    };
    require(
        v["conformance"] == state && exit == expected_exit,
        "verdict/three-value/exit mismatch",
    )?;
    require(v["model"].is_object(), "model object")?;
    for k in ["name", "sha256"] {
        require(
            v["model"].get(k).is_some() && nullable_text(&v["model"][k]),
            format!("model.{k}"),
        )?;
    }
    require(v["log"].is_object(), "log object")?;
    require(
        v["log"].get("sha256").is_some() && nullable_text(&v["log"]["sha256"]),
        "log.sha256",
    )?;
    require(
        nullable_text(&v["configuration"]) && nullable_text(&v["scenario"]),
        "selection",
    )?;
    require(
        matches!(v["cut"].as_str(), Some("undeclared" | "unresolved")) || v["cut"].is_object(),
        "cut",
    )?;
    for k in ["notices", "reentry", "horizon"] {
        array(&v["cut_context"], k)?;
    }
    require(
        !array(&v["cut_context"], "notices")?.is_empty(),
        "Cut notices cannot disappear",
    )?;
    for key in ["assumptions", "not_checked", "unmapped_operation"] {
        array(v, key)?;
    }
    require(
        v["summary"].is_object() && !v["summary"].as_object().unwrap().is_empty(),
        "summary counts",
    )?;
    for (k, n) in [
        ("max_events", 10000),
        ("max_line_bytes", 65536),
        ("max_order_states", 100000),
    ] {
        require(v["limits"][k] == n, format!("fixed limit {k}"))?;
    }
    for k in ["event_limit", "line_limit", "order_limit", "truncated"] {
        require(v["limits"][k].is_boolean(), format!("limits.{k}"))?;
    }
    if v["limits"]["truncated"] == true {
        require(v["conformance"] != "conformant", "truncated success")?;
    }
    let mut rows = BTreeSet::new();
    for row in array(v, "checks")? {
        for k in [
            "id",
            "case_id",
            "conformance",
            "reason",
            "evidence",
            "model_span",
        ] {
            require(row.get(k).is_some(), format!("check missing {k}"))?;
        }
        require(
            text(&row["id"]) && nullable_text(&row["case_id"]),
            "check identity",
        )?;
        require(
            tri(&row["conformance"]) && span(&row["model_span"]),
            "check state/span",
        )?;
        require(nullable_text(&row["reason"]), "check reason type")?;
        if row["conformance"] != "conformant" {
            require(text(&row["reason"]), "non-success needs reason")?;
        }
        require(
            rows.insert((row["id"].to_string(), row["case_id"].to_string())),
            "duplicate check key",
        )?;
        for evidence in array(row, "evidence")? {
            require(
                text(&evidence["event_id"])
                    && text(&evidence["case_id"])
                    && text(&evidence["task_id"]),
                "evidence identity",
            )?;
            require(
                evidence["line"].as_u64().is_some_and(|n| n > 1),
                "evidence JSONL line",
            )?;
        }
    }
    let mut cases = BTreeSet::new();
    for c in array(v, "cases")? {
        require(
            text(&c["case_id"]) && tri(&c["conformance"]),
            "case identity/state",
        )?;
        require(cases.insert(c["case_id"].to_string()), "duplicate case")?;
    }
    for row in array(v, "unmapped_actor")? {
        for k in [
            "source_actor_id",
            "source_executor_id",
            "mapped_actor",
            "reason",
            "case_ids",
            "event_ids",
        ] {
            require(row.get(k).is_some(), format!("unmapped missing {k}"))?;
        }
        require(
            text(&row["reason"]) && nullable_text(&row["mapped_actor"]),
            "unmapped reason/actor",
        )?;
        require(
            !array(row, "event_ids")?.is_empty() && !array(row, "case_ids")?.is_empty(),
            "unmapped evidence lost",
        )?;
    }
    for d in array(v, "diagnostics")? {
        let code = d["code"].as_str().ok_or("diagnostic code")?;
        require(
            LOG_CODES.contains(&code) || relca_core::diag::CODES.contains(&code),
            "unknown diagnostic",
        )?;
        let severity = if code.starts_with("W-") {
            "warning"
        } else {
            "error"
        };
        require(
            d["severity"] == severity
                && text(&d["message"])
                && d.get("span").is_some()
                && span(&d["span"]),
            "diagnostic shape/severity",
        )?;
    }
    if v["verdict"] == "invalid" {
        require(
            array(v, "checks")?.is_empty() && array(v, "cases")?.is_empty(),
            "invalid leaked conclusions",
        )?;
    }
    Ok(())
}

pub fn has_code(v: &Value, code: &str) -> bool {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == code)
}
pub fn has_reason(v: &Value, reason: &str) -> bool {
    v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["reason"] == reason)
}
pub fn evidence_ids(v: &Value) -> BTreeSet<String> {
    v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["evidence"].as_array().unwrap())
        .map(|e| e["event_id"].as_str().unwrap().to_owned())
        .collect()
}

/// A complete, manually written control for the validator, not a CLI snapshot.
pub fn control() -> Value {
    json!({"ledger":ledger_shape::control(&json!({})),"uncertainty":ledger_shape::uncertainty_wire::project(&json!({})),"verdict":"satisfied","conformance":"conformant","mode":"log_conformance",
        "schema":"relca-conformance/v1","scope":"observable_event_contracts_v1",
        "model":{"name":"M","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
        "log":{"sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
        "configuration":null,"scenario":null,"cut":"undeclared",
        "cut_context":{"notices":["under this Cut only"],"reentry":[],"horizon":[]},
        "assumptions":[],"not_checked":[{"item":"process_path","reason":"outside_scope"}],
        "checks":[{"id":"task:t1","case_id":"c","conformance":"conformant","reason":null,"model_span":null,
            "evidence":[{"event_id":"t1-start","case_id":"c","task_id":"t1","line":2},
                        {"event_id":"t1-finish","case_id":"c","task_id":"t1","line":3}]}],
        "cases":[{"case_id":"c","conformance":"conformant"}],
        "summary":{"cases":{"conformant":1,"violating":0,"unknown":0},"checks":{"conformant":1,"violating":0,"unknown":0}},
        "unmapped_actor":[],"unmapped_operation":[],
        "limits":{"max_events":10000,"max_line_bytes":65536,"max_order_states":100000,
            "event_limit":false,"line_limit":false,"order_limit":false,"truncated":false},
        "diagnostics":[{"code":"W-CUT-0","severity":"warning","message":"Cut undeclared","span":null}]})
}

#[cfg(test)]
mod ledger_migration_tests {
    use super::*;
    #[test]
    fn ledger_migration_rejects_missing_and_extra_metadata() {
        let good = control();
        validate(&good, 0).unwrap();
        let mut missing = good.clone();
        missing["ledger"].as_object_mut().unwrap().remove("schema");
        assert!(validate(&missing, 0).is_err());
        let mut extra = good;
        extra["ledger"]["extra"] = json!(0);
        assert!(validate(&extra, 0).is_err());
    }
}
