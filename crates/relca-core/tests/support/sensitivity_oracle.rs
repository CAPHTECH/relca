//! Independent acceptance predicates, not a sensitivity implementation.
#![allow(dead_code)]
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const BASE: &str = include_str!("../data/sensitivity/base.relca");
pub const HIGH: &str = include_str!("../data/sensitivity/high.relca");
pub const MISS: &str = include_str!("../data/sensitivity/miss.relca");
pub const ZERO: &str = include_str!("../data/sensitivity/zero.relca");
pub const ABORT: &str = include_str!("../data/sensitivity/abort.relca");
pub const QUERY: &str = "probability reached #1";
pub fn hand() -> Value {
    serde_json::from_str(include_str!("../data/sensitivity/expected.json")).unwrap()
}
pub fn keys(v: &Value, names: &[&str]) -> Result<(), String> {
    let actual: BTreeSet<_> = v
        .as_object()
        .ok_or("expected object")?
        .keys()
        .map(String::as_str)
        .collect();
    let want: BTreeSet<_> = names.iter().copied().collect();
    if actual != want {
        return Err(format!("closed keys: {actual:?} != {want:?}"));
    }
    Ok(())
}
pub fn property<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["properties"]
        .as_array()
        .expect("properties")
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("missing {id}: {}", v["diagnostics"]))
}
pub fn uncertainty(v: &Value, expected: [(&str, Option<&str>); 3]) -> Result<(), String> {
    let u = v
        .get("uncertainty")
        .ok_or("E9b unimplemented: mandatory uncertainty is missing")?;
    keys(u, &["rules", "aleatory", "estimation", "model"])?;
    if u["rules"] != "uncertainty-p2-1" {
        return Err("unknown uncertainty rules".into());
    }
    for (name, (status, reason)) in ["aleatory", "estimation", "model"]
        .into_iter()
        .zip(expected)
    {
        let c = &u[name];
        keys(c, &["status", "reason", "items"])?;
        if c["status"] != status || c["reason"] != json!(reason) {
            return Err(format!("{name} status/reason: {c}"));
        }
        let items = c["items"].as_array().ok_or("items must be array")?;
        if matches!(status, "available" | "references") && items.is_empty() {
            return Err(format!("{name} empty evidence"));
        }
        if status == "references" {
            for item in items {
                let p = item.as_str().ok_or("reference must be pointer")?;
                if !p.ends_with(&format!("/uncertainty/{name}")) || v.pointer(p).is_none() {
                    return Err(format!("unresolved {name} reference {p}"));
                }
            }
        } else if status == "available" {
            for item in items {
                if !item["kind"].as_str().is_some_and(|s| !s.is_empty()) {
                    return Err(format!("{name} evidence kind absent"));
                }
                // Payload coverage is deliberately narrower than full wire closure.
                if item.as_object().is_none_or(|o| o.len() < 2) {
                    return Err(format!("{name} evidence has no provenance/value"));
                }
                if let Some(at) = item.get("at") {
                    let p = at.as_str().ok_or("evidence pointer must be string")?;
                    if !p.starts_with('/') || v.pointer(p).is_none() {
                        return Err(format!("unresolved evidence {p}"));
                    }
                }
            }
        } else if !items.is_empty() {
            return Err(format!("{name} unavailable value fabricated"));
        }
    }
    Ok(())
}
pub fn references(v: &Value, children: &[String]) -> Result<(), String> {
    uncertainty(v, [("references", Some("aggregate_per_source")); 3])?;
    for name in ["aleatory", "estimation", "model"] {
        let expected: Vec<_> = children
            .iter()
            .map(|p| format!("{p}/uncertainty/{name}"))
            .collect();
        if v["uncertainty"][name]["items"] != json!(expected) {
            return Err(format!("wrong child ownership for {name}"));
        }
    }
    Ok(())
}
pub fn suite(
    v: &Value,
    ids: &[&str],
    counts: [u64; 3],
    verdict: &str,
    extrema: Option<(&str, &str)>,
) -> Result<(), String> {
    if v["schema"] != "relca-sensitivity-result/v1" {
        return Err(format!(
            "runner unimplemented: schema={}; mode={}; verdict={}",
            v["schema"], v["mode"], v["verdict"]
        ));
    }
    let scope = &v["scope"];
    if scope["kind"] != "enumerated_models"
        || scope["point_ids"] != json!(ids)
        || scope["planned"] != ids.len()
        || scope["evaluated"] != counts[0]
        || scope["refused"] != counts[1]
        || scope["not_run"] != counts[2]
        || counts.iter().sum::<u64>() != ids.len() as u64
        || scope["all_points_evaluated"] != (counts[0] == ids.len() as u64)
        || scope["continuous_region_checked"] != false
        || scope["robust_guarantee"] != false
    {
        return Err(format!("finite scope or accounting: {scope}"));
    }
    if v["verdict"] != verdict {
        return Err(format!("wrong suite verdict {}", v["verdict"]));
    }
    let points = v["points"].as_array().ok_or("missing points")?;
    if points
        .iter()
        .map(|p| p["id"].as_str().unwrap_or(""))
        .collect::<Vec<_>>()
        != ids
    {
        return Err("invented, missing, reordered or duplicated point".into());
    }
    for status in ["evaluated", "refused", "not_run"] {
        let n = points.iter().filter(|p| p["status"] == status).count() as u64;
        if scope[status] != n {
            return Err(format!("status count {status}"));
        }
    }
    let summaries = v["summaries"].as_array().ok_or("missing summaries")?;
    let s = summaries
        .iter()
        .find(|s| s["id"] == QUERY)
        .ok_or("target summary missing")?;
    if s["point_ids"] != json!(ids) {
        return Err("summary silently omitted a point".into());
    }
    if let Some((min, max)) = extrema {
        if s["kind"] != "finite_point_extrema"
            || s["status"] != "available"
            || !s["reason"].is_null()
            || s["value"] != json!({"min":min,"max":max})
        {
            return Err(format!("wrong finite extrema {s}"));
        }
    } else if !s["value"].is_null()
        || s["status"] != "unavailable"
        || !["incomplete_points", "simulation_pointwise"]
            .iter()
            .any(|r| s["reason"] == *r)
    {
        return Err(format!(
            "unknown or simulation interval promoted to extrema {s}"
        ));
    }
    Ok(())
}
pub fn zero_statistics(
    v: &Value,
    rows: u64,
    alpha: &str,
    upper: &str,
    verdict: &str,
) -> Result<(), String> {
    if v["simulation"]["replications"] != 4
        || v["simulation"]["completed_replications"] != 4
        || v["simulation"]["multiplicity"]["rows"] != rows
        || v["simulation"]["multiplicity"]["alpha_after"] != alpha
    {
        return Err(format!(
            "planned Bonferroni allocation changed: {}",
            v["simulation"]
        ));
    }
    let c = &property(v, QUERY)["cases"][0];
    if c["hits"] != 0
        || c["unknown"] != 0
        || c["trials"] != 4
        || c["interval"] != json!({"lower":"0","upper":upper})
        || c["verdict"] != verdict
        || v["verdict"] != verdict
    {
        return Err(format!("zero accidents/statistical verdict {c}"));
    }
    Ok(())
}
pub fn finite_control() -> Value {
    json!({"schema":"relca-sensitivity-result/v1","verdict":"satisfied",
        "scope":{"kind":"enumerated_models","point_ids":["base","high","miss"],
            "planned":3,"evaluated":3,"refused":0,"not_run":0,"all_points_evaluated":true,
            "continuous_region_checked":false,"robust_guarantee":false},
        "points":[{"id":"base","status":"evaluated"},{"id":"high","status":"evaluated"},
                  {"id":"miss","status":"evaluated"}],
        "summaries":[{"id":QUERY,"point_ids":["base","high","miss"],"kind":"finite_point_extrema",
            "status":"available","reason":null,"value":{"min":"1/250","max":"1/125"}}]})
}
pub fn zero_control() -> Value {
    json!({"verdict":"inconclusive","simulation":{"replications":4,"completed_replications":4,
        "multiplicity":{"rows":2,"alpha_after":"1/32"}},
        "properties":[{"id":QUERY,"cases":[{"hits":0,"unknown":0,"trials":4,
            "interval":{"lower":"0","upper":"607705/1048576"},"verdict":"inconclusive"}]}]})
}
pub fn evidence_text(host: &Value, items: &Value) -> String {
    fn visit(host: &Value, value: &Value, seen: &mut BTreeSet<String>, out: &mut String) {
        match value {
            Value::String(s) if s.starts_with('/') => {
                if seen.insert(s.clone())
                    && let Some(target) = host.pointer(s)
                {
                    visit(host, target, seen, out);
                }
            }
            Value::Object(fields) => {
                for (k, v) in fields {
                    out.push_str(k);
                    visit(host, v, seen, out);
                }
            }
            Value::Array(xs) => {
                for v in xs {
                    visit(host, v, seen, out);
                }
            }
            _ => out.push_str(&value.to_string()),
        }
    }
    let mut out = String::new();
    visit(host, items, &mut BTreeSet::new(), &mut out);
    out
}
