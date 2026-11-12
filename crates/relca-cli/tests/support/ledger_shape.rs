//! Independent closed ADR 0058 vocabulary for the pre-existing wire validators.
//! This checks shape, not the input-specific hand values in the Q3 acceptance.
#![allow(dead_code)]
// Self-contained old core acceptance files are also included by CLI targets.
// Both parents intentionally share this stateless wire vocabulary, not state.
#![allow(clippy::duplicate_mod)]
use serde_json::{Value, json};
#[path = "ledger_expected.rs"]
mod hand;
#[path = "uncertainty_wire.rs"]
pub mod uncertainty_wire;
pub use hand::Spec;

fn need(ok: bool, what: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("ledger: {what}"))
    }
}
fn keys(v: &Value, names: &[&str]) -> Result<(), String> {
    let fields = v.as_object().ok_or("ledger: object required")?;
    need(
        fields.len() == names.len() && names.iter().all(|n| fields.contains_key(*n)),
        "closed keys",
    )
}
fn text(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.is_empty())
}
fn array(v: &Value) -> Result<&Vec<Value>, String> {
    v.as_array().ok_or("ledger: array required".into())
}
fn sv(v: &Value) -> Result<(), String> {
    keys(v, &["status", "value", "reason"])?;
    let ok = match v["status"].as_str() {
        Some("known") => !v["value"].is_null() && v["reason"].is_null(),
        Some("not_declared") => v["value"].is_null() && v["reason"] == "not_declared",
        Some("not_applicable") => {
            v["value"].is_null()
                && matches!(
                    v["reason"].as_str(),
                    Some("no_analysis" | "aggregate_per_source" | "exhaustive_analysis")
                )
        }
        Some("unavailable") => {
            v["value"].is_null()
                && matches!(
                    v["reason"].as_str(),
                    Some("input_unavailable" | "not_lowered" | "execution_settings_unavailable")
                )
        }
        _ => false,
    };
    need(ok, "state/value/reason")
}
fn reference(v: &Value) -> Result<(), String> {
    sv(v)?;
    need(
        v == &hand::na("no_analysis")
            || (v["status"] == "known" && v["value"].as_str().is_some_and(|p| p.starts_with('/'))),
        "pointer",
    )
}
pub fn validate(v: &Value) -> Result<(), String> {
    keys(
        v,
        &[
            "schema",
            "producer",
            "backends",
            "derivation",
            "execution",
            "actor_versions",
            "context",
            "subjects",
        ],
    )?;
    need(v["schema"] == "relca-conclusion-ledger/v1", "schema")?;
    need(
        v["producer"]
            == json!({"name":"relca","version":"0.0.0","version_source":"cargo_package",
        "build_identity":{"status":"not_recorded","value":null,"reason":"package_version_only"}}),
        "producer",
    )?;
    keys(&v["backends"], &["builtin", "external"])?;
    need(
        v["backends"]["external"]
            == json!({"status":"not_connected","name":null,"version":null,"reason":"external_backend_not_connected"}),
        "external",
    )?;
    let b = &v["backends"]["builtin"];
    keys(b, &["name", "status", "version"])?;
    need(
        matches!(
            b["name"].as_str(),
            Some("none" | "builtin_finite" | "builtin_simulation" | "builtin_event_contracts")
        ),
        "builtin name",
    )?;
    need(
        matches!(
            b["status"].as_str(),
            Some("ran" | "not_run" | "not_applicable" | "unavailable")
        ),
        "builtin status",
    )?;
    sv(&b["version"])?;
    need(
        b["version"]
            == if b["name"] == "none" {
                hand::na("no_analysis")
            } else {
                hand::known("0.0.0")
            },
        "builtin version",
    )?;
    keys(&v["derivation"], &["bundle", "components"])?;
    need(
        v["derivation"]["bundle"] == "relca-conclusions-p2-uncertainty-2",
        "bundle",
    )?;
    for c in array(&v["derivation"]["components"])? {
        keys(c, &["name", "version", "at"])?;
        need(
            text(&c["version"])
                && c["name"] == c["at"]
                && c["at"].as_str().is_some_and(|p| p.starts_with('/')),
            "component",
        )?;
    }
    let e = &v["execution"];
    keys(
        e,
        &[
            "mode",
            "status",
            "selection",
            "seed",
            "replications",
            "confidence",
            "random_method",
            "bounds",
            "limits",
        ],
    )?;
    need(
        matches!(
            e["mode"].as_str(),
            Some(
                "exhaustive"
                    | "simulation"
                    | "diff"
                    | "compare"
                    | "mutation"
                    | "log_conformance"
                    | "verify"
                    | "cli_refusal"
            )
        ) && e["status"] == b["status"],
        "execution",
    )?;
    keys(&e["selection"], &["requested", "effective"])?;
    keys(&e["selection"]["requested"], &["configuration", "scenario"])?;
    keys(
        &e["selection"]["effective"],
        &["configuration", "scenario", "process"],
    )?;
    for key in ["configuration", "scenario"] {
        need(
            e["selection"]["requested"][key].is_null() || text(&e["selection"]["requested"][key]),
            "requested selection",
        )?;
    }
    for key in ["configuration", "scenario", "process"] {
        sv(&e["selection"]["effective"][key])?;
        need(
            e["selection"]["effective"][key]["status"] != "known"
                || text(&e["selection"]["effective"][key]["value"]),
            "effective selection text",
        )?;
    }
    for key in [
        "seed",
        "replications",
        "confidence",
        "random_method",
        "bounds",
    ] {
        sv(&e[key])?;
    }
    for key in ["seed", "replications"] {
        need(
            e[key]["status"] != "known" || e[key]["value"].is_u64(),
            "integer setting",
        )?;
    }
    for key in ["confidence", "random_method"] {
        need(
            e[key]["status"] != "known" || text(&e[key]["value"]),
            "text setting",
        )?;
    }
    if e["bounds"]["value"].is_object() {
        keys(&e["bounds"]["value"], &["cases", "live_tasks", "clock"])?;
        for key in ["cases", "live_tasks", "clock"] {
            need(e["bounds"]["value"][key].is_u64(), "bound")?;
        }
    } else if e["bounds"]["status"] == "known" {
        need(e["bounds"]["value"] == "/bounds", "bounds pointer")?;
    }
    keys(
        &e["limits"],
        &[
            "state_budget",
            "capture_limit",
            "mutant_limit",
            "max_events",
            "max_line_bytes",
            "max_order_states",
        ],
    )?;
    for value in e["limits"].as_object().unwrap().values() {
        sv(value)?;
        need(value["status"] != "not_declared", "limit availability")?;
        need(
            value["status"] != "known" || value["value"].is_u64(),
            "limit",
        )?;
    }
    let a = &v["actor_versions"];
    keys(a, &["status", "entries", "reason"])?;
    need(
        matches!(
            a["status"].as_str(),
            Some("known" | "unavailable" | "not_applicable")
        ),
        "actor status",
    )?;
    need(
        match a["status"].as_str() {
            Some("known") => a["reason"].is_null(),
            Some("unavailable") => a["reason"] == "not_lowered",
            _ => a["reason"] == "aggregate_per_source",
        },
        "actor reason",
    )?;
    let entries = array(&a["entries"])?;
    need(
        a["status"] == "known" || entries.is_empty(),
        "unknown actors",
    )?;
    for row in entries {
        keys(row, &["actor", "via_model", "rule"])?;
        need(text(&row["actor"]), "actor name")?;
        sv(&row["via_model"])?;
        sv(&row["rule"])?;
        need(
            matches!(
                row["via_model"]["status"].as_str(),
                Some("known" | "not_declared")
            ) && matches!(
                row["rule"]["status"].as_str(),
                Some("known" | "not_declared")
            ),
            "actor version availability",
        )?;
        if row["via_model"]["status"] == "known" {
            need(text(&row["via_model"]["value"]), "via model")?;
        }
        if row["rule"]["status"] == "known" {
            keys(&row["rule"]["value"], &["declared", "effective"])?;
            need(
                text(&row["rule"]["value"]["declared"]) && text(&row["rule"]["value"]["effective"]),
                "rule version",
            )?;
        }
    }
    let c = &v["context"];
    keys(
        c,
        &[
            "model",
            "mode",
            "assumptions",
            "cut",
            "boundaries",
            "scope",
            "log",
            "mutation",
        ],
    )?;
    for key in ["model", "mode", "cut", "log", "mutation"] {
        reference(&c[key])?;
    }
    for key in ["assumptions", "boundaries", "scope"] {
        for r in array(&c[key])? {
            reference(r)?;
        }
    }
    for s in array(&v["subjects"])? {
        keys(s, &["target", "source_ledgers"])?;
        need(
            s["target"].as_str().is_some_and(|p| p.starts_with('/')),
            "subject",
        )?;
        for p in array(&s["source_ledgers"])? {
            need(
                p.as_str()
                    .is_some_and(|p| p.starts_with('/') && p.ends_with("/ledger")),
                "source ledger",
            )?;
        }
    }
    Ok(())
}

/// New metadata only: this hand control does not replace any pre-existing value.
pub fn control(host: &Value) -> Value {
    hand::expected(host, &hand::Spec::prelower("exhaustive", false)).unwrap()
}
pub fn validate_expected(host: &Value, spec: &Spec) -> Result<(), String> {
    hand::validate(host, spec)
}

pub fn original_field_count(host: &Value) -> usize {
    validate(&host["ledger"]).expect("mandatory closed ledger");
    uncertainty_wire::validate(&host["uncertainty"]).expect("mandatory closed uncertainty");
    host.as_object().unwrap().len() - 2
}
pub fn original_report(report: &Value) -> Value {
    let mut host = report.clone();
    validate(&host["ledger"]).expect("mandatory closed ledger before old-field comparison");
    uncertainty_wire::validate(&host["uncertainty"])
        .expect("mandatory closed uncertainty before old-field comparison");
    host.as_object_mut().unwrap().remove("ledger");
    host.as_object_mut().unwrap().remove("uncertainty");
    host
}
/// Only type-owned Report locations, never arbitrary payload members named ledger.
pub fn original_document(report: &Value) -> Value {
    let mut host = original_report(report);
    if host.get("from").is_some() && host.get("to").is_some() {
        for side in ["from", "to"] {
            host[side]["report"] = original_report(&host[side]["report"]);
        }
    }
    if let Some(plans) = host.get_mut("plans").and_then(Value::as_array_mut) {
        for plan in plans {
            plan["report"] = original_report(&plan["report"]);
        }
    }
    if host.get("original").is_some_and(|r| !r.is_null()) {
        host["original"] = original_report(&host["original"]);
    }
    if let Some(kinds) = host.get_mut("mutations").and_then(Value::as_array_mut) {
        for kind in kinds {
            for mutant in kind["mutants"].as_array_mut().unwrap() {
                if mutant.get("report").is_some_and(|r| !r.is_null()) {
                    mutant["report"] = original_report(&mutant["report"]);
                }
            }
        }
    }
    host
}
pub fn extend_value(mut host: Value, spec: &Spec) -> Value {
    assert!(host.get("ledger").is_none());
    host["ledger"] = hand::expected(&host, spec).unwrap();
    host["uncertainty"] = uncertainty_wire::for_mode(&host, spec.mode);
    host["ledger"] = hand::expected(&host, spec).unwrap();
    host
}

/// The old validator projects later features away before its DTO round-trip.
/// Rebase only metadata links in that temporary DTO, never the emitted report
/// or any old evidence/setting. Full original links have separate Q3 acceptance.
pub fn dto_projection(host: &mut Value) -> Result<(), String> {
    validate(&host["ledger"])?;
    let mut s = Spec::prelower("exhaustive", false);
    let l = &host["ledger"];
    s.builtin = match l["backends"]["builtin"]["name"].as_str() {
        Some("none") => "none",
        Some("builtin_finite") => "builtin_finite",
        _ => return Err("not an exhaustive DTO".into()),
    };
    s.status = match l["execution"]["status"].as_str() {
        Some("ran") => "ran",
        Some("not_run") => "not_run",
        Some("unavailable") => "unavailable",
        _ => return Err("not an exhaustive DTO status".into()),
    };
    let e = &l["execution"];
    s.selection = e["selection"].clone();
    s.seed = e["seed"].clone();
    s.replications = e["replications"].clone();
    s.confidence = e["confidence"].clone();
    s.random = e["random_method"].clone();
    s.bounds = e["bounds"].clone();
    s.limits = e["limits"].clone();
    s.actors = l["actor_versions"].clone();
    host["uncertainty"] = uncertainty_wire::project(host);
    let expected = hand::expected(host, &s)?;
    for key in ["derivation", "context", "subjects"] {
        host["ledger"][key] = expected[key].clone();
    }
    Ok(())
}

/// Preserve every old byte and prepend the independently specified metadata.
pub fn extend_raw(raw: &str, spec: &Spec) -> String {
    let mut host: Value = serde_json::from_str(raw).unwrap();
    assert!(host.get("ledger").is_none());
    // These legacy raw controls are refusals, not a source of new stochastic
    // expectations. Never infer an effective distribution from a captured value.
    assert_ne!(
        spec.actors["status"], "known",
        "use metadata stripping for lowered old-field comparisons"
    );
    host["ledger"] = hand::expected(&host, spec).unwrap();
    let uncertainty = uncertainty_wire::for_mode(&host, spec.mode);
    uncertainty_wire::validate(&uncertainty).unwrap();
    host["uncertainty"] = uncertainty.clone();
    uncertainty_wire::validate_host(&host).unwrap();
    let expected = hand::expected(&host, spec).unwrap();
    let prefix = raw_metadata_prefix(&expected, &uncertainty);
    let result = format!("{prefix}{}", raw.strip_prefix('{').unwrap());
    assert_eq!(
        format!("{{{}", &result[prefix.len()..]),
        raw,
        "old stdout bytes must remain identical"
    );
    result
}

fn raw_metadata_prefix(ledger: &Value, uncertainty: &Value) -> String {
    let mut prefix = raw_ledger_prefix(ledger);
    let mut u = format!("{{\"rules\":{}", uncertainty["rules"]);
    for name in ["aleatory", "estimation", "model"] {
        let c = &uncertainty[name];
        assert!(
            c["items"].as_array().unwrap().iter().all(Value::is_string),
            "raw refusal controls must not invent available effective inputs"
        );
        u.push_str(&format!(
            ",\"{name}\":{{\"status\":{},\"reason\":{},\"items\":{}}}",
            c["status"], c["reason"], c["items"]
        ));
    }
    u.push('}');
    prefix.push_str(&format!("\"uncertainty\":{u},"));
    prefix
}

fn raw_ledger_prefix(ledger: &Value) -> String {
    let mut prefix = String::from("{\"ledger\":{");
    for (i, key) in [
        "schema",
        "producer",
        "backends",
        "derivation",
        "execution",
        "actor_versions",
        "context",
        "subjects",
    ]
    .iter()
    .enumerate()
    {
        if i > 0 {
            prefix.push(',');
        }
        prefix.push_str(&format!(
            "\"{key}\":{}",
            serde_json::to_string(&ledger[*key]).unwrap()
        ));
    }
    prefix.push_str("},");
    prefix
}

/// The hash-only legacy oracle retains its old digest. Check the new metadata
/// independently, then remove exactly its first member and separator bytes.
pub fn remove_expected_prefix(raw: &[u8], spec: &Spec) -> Vec<u8> {
    let host: Value = serde_json::from_slice(raw).unwrap();
    hand::validate(&host, spec).unwrap();
    uncertainty_wire::validate_host(&host).unwrap();
    let expected = hand::expected(&host, spec).unwrap();
    let ledger_prefix = raw_ledger_prefix(&expected);
    let prefix = ledger_prefix.as_bytes();
    assert!(
        raw.starts_with(prefix),
        "only the exact canonical ledger member can be removed"
    );
    let key = b"\"uncertainty\":";
    let rest = raw[prefix.len()..]
        .strip_prefix(key)
        .expect("mandatory second uncertainty member");
    let mut values = serde_json::Deserializer::from_slice(rest).into_iter::<Value>();
    assert_eq!(values.next().unwrap().unwrap(), host["uncertainty"]);
    let prefix_len = prefix.len() + key.len() + values.byte_offset() + 1;
    assert_eq!(raw[prefix_len - 1], b',');
    let mut result = vec![b'{'];
    result.extend_from_slice(&raw[prefix_len..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_ledger_migration_rejects_missing_and_extra_key() {
        let v = control(&json!({"verdict":"invalid"}));
        validate(&v).unwrap();
        let mut bad = v.clone();
        bad.as_object_mut().unwrap().remove("producer");
        assert!(validate(&bad).is_err());
        let mut bad = v;
        bad["execution"]["limits"]["surplus"] = json!(0);
        assert!(validate(&bad).is_err());
    }
    #[test]
    fn raw_refusal_metadata_does_not_reencode_or_hide_old_stdout_bytes() {
        let raw = "{\"verdict\":\"invalid\",\"model\":{\"name\":null,\"sha256\":\"hand-source\"},\"n\":8,\"text\":\"原文\"}\n";
        let spec = Spec::prelower("exhaustive", false);
        let encoded = extend_raw(raw, &spec);
        assert_eq!(
            remove_expected_prefix(encoded.as_bytes(), &spec),
            raw.as_bytes()
        );
        let host: Value = serde_json::from_str(&encoded).unwrap();
        // Uncertainty's object order is not an old-data byte oracle. Parsing its
        // exact span must work without serializing or changing the old body.
        let reordered = format!(
            "{}\"uncertainty\":{},{}",
            raw_ledger_prefix(&host["ledger"]),
            serde_json::to_string(&host["uncertainty"]).unwrap(),
            raw.strip_prefix('{').unwrap()
        );
        assert_eq!(
            remove_expected_prefix(reordered.as_bytes(), &spec),
            raw.as_bytes()
        );
        let faulty = encoded.replacen("\"n\":8", "\"n\":9", 1);
        assert_ne!(
            remove_expected_prefix(faulty.as_bytes(), &spec),
            raw.as_bytes()
        );
    }
}
