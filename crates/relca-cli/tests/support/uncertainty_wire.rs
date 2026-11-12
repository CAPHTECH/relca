//! Wire shape checks, not an independent numerical oracle.
//! No production uncertainty factory/decoder is called by these checks.
#![allow(dead_code)]
use serde_json::{Value, json};

fn need(ok: bool, message: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("uncertainty: {message}"))
    }
}
fn keys(v: &Value, expected: &[&str]) -> Result<(), String> {
    need(
        v.as_object().is_some_and(|o| {
            o.len() == expected.len() && expected.iter().all(|k| o.contains_key(*k))
        }),
        "closed keys",
    )
}
fn text(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.is_empty())
}
fn rows(v: &Value) -> Result<&Vec<Value>, String> {
    v.as_array().ok_or("uncertainty: array".into())
}
fn strings(v: &Value) -> bool {
    v.as_array().is_some_and(|a| a.iter().all(text))
}
fn span(v: &Value) -> Result<(), String> {
    keys(v, &["start", "end"])?;
    need(
        matches!((v["start"].as_u64(), v["end"].as_u64()), (Some(a),Some(b)) if a<=b),
        "source span",
    )
}
fn probability(v: &Value) -> bool {
    let Some(s) = v.as_str() else {
        return false;
    };
    let (n, d) = s.split_once('/').unwrap_or((s, "1"));
    let integer = |s: &str| {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0'))
    };
    if !integer(n) || !integer(d) || d == "0" || (s.contains('/') && d == "1") {
        return false;
    }
    n.len() < d.len() || n.len() == d.len() && n <= d
}
fn assignment(v: &Value) -> Result<(), String> {
    keys(v, &["variable", "value"])?;
    need(text(&v["variable"]) && text(&v["value"]), "assignment")
}
fn evidence(v: &Value, category: &str) -> Result<(), String> {
    let kind = v["kind"].as_str().ok_or("uncertainty: kind")?;
    let allowed = match category {
        "aleatory" => &[
            "initial_distribution",
            "kernel",
            "duration_distribution",
            "response_channel",
            "arrival_distribution",
        ][..],
        "estimation" => &["sampling_method", "statistical_query", "point_estimate"][..],
        _ => &["assumption", "model_context", "model_assumption"][..],
    };
    need(allowed.contains(&kind), "classification evidence kind")?;
    match kind {
        "initial_distribution" => {
            keys(v, &["kind", "variable", "instance", "span", "entries"])?;
            need(
                text(&v["variable"])
                    && matches!(v["instance"].as_str(), Some("per_case" | "single_case")),
                "initial identity",
            )?;
            span(&v["span"])?;
            need(!rows(&v["entries"])?.is_empty(), "initial entries")?;
            for e in rows(&v["entries"])? {
                keys(e, &["value", "probability"])?;
                need(
                    text(&e["value"]) && probability(&e["probability"]),
                    "initial mass",
                )?;
            }
        }
        "kernel" => {
            keys(
                v,
                &[
                    "kind",
                    "capability",
                    "name",
                    "basis",
                    "given",
                    "span",
                    "rows",
                ],
            )?;
            need(
                text(&v["capability"])
                    && text(&v["name"])
                    && (v["basis"].is_null() || v["basis"].is_string())
                    && strings(&v["given"]),
                "kernel identity",
            )?;
            span(&v["span"])?;
            need(!rows(&v["rows"])?.is_empty(), "kernel rows")?;
            for row in rows(&v["rows"])? {
                keys(row, &["conditions", "branches"])?;
                for c in rows(&row["conditions"])? {
                    assignment(c)?;
                }
                need(!rows(&row["branches"])?.is_empty(), "kernel branches")?;
                for b in rows(&row["branches"])? {
                    keys(b, &["probability", "result", "writes"])?;
                    need(
                        probability(&b["probability"]) && text(&b["result"]),
                        "kernel mass",
                    )?;
                    for w in rows(&b["writes"])? {
                        assignment(w)?;
                    }
                }
            }
        }
        "duration_distribution" | "arrival_distribution" => {
            let arrival = kind == "arrival_distribution";
            let identity = if arrival { "basis" } else { "capability" };
            keys(v, &["kind", identity, "span", "entries"])?;
            need(text(&v[identity]), "distribution identity")?;
            span(&v["span"])?;
            need(!rows(&v["entries"])?.is_empty(), "distribution entries")?;
            for e in rows(&v["entries"])? {
                let key = if arrival { "times" } else { "minutes" };
                keys(e, &[key, "probability"])?;
                need(probability(&e["probability"]), "distribution mass")?;
                if arrival {
                    need(
                        !rows(&e[key])?.is_empty() && rows(&e[key])?.iter().all(Value::is_u64),
                        "arrival times",
                    )?;
                } else {
                    need(e[key].is_u64(), "duration minutes")?;
                }
            }
        }
        "response_channel" => {
            keys(
                v,
                &[
                    "kind",
                    "binding",
                    "attempt",
                    "max_attempts",
                    "loss_probability",
                    "basis",
                    "span",
                ],
            )?;
            need(
                text(&v["binding"])
                    && text(&v["basis"])
                    && probability(&v["loss_probability"])
                    && matches!((v["attempt"].as_u64(), v["max_attempts"].as_u64()), (Some(a), Some(b)) if a>0 && a<=b),
                "response channel",
            )?;
            span(&v["span"])?;
        }
        "model_assumption" => {
            keys(v, &["kind", "name", "subjects"])?;
            need(
                matches!(
                    v["name"].as_str(),
                    Some(
                        "analyst_assumptions_not_exhaustive"
                            | "initial_product_independence"
                            | "duration_result_conditional_independence"
                            | "response_channel_independence"
                    )
                ) && strings(&v["subjects"]),
                "model assumption",
            )?;
        }
        "point_estimate" => {
            keys(v, &["kind", "at", "reason"])?;
            need(
                v["reason"] == "interval_not_configured",
                "point-estimate interval not invented",
            )?;
        }
        _ => keys(v, &["kind", "at"])?,
    }
    if let Some(at) = v.get("at") {
        need(
            at.as_str().is_some_and(|s| s.starts_with('/')),
            "evidence pointer",
        )?;
    }
    Ok(())
}
pub fn validate(v: &Value) -> Result<(), String> {
    keys(v, &["rules", "aleatory", "estimation", "model"])?;
    need(v["rules"] == "uncertainty-p2-1", "rules")?;
    for name in ["aleatory", "estimation", "model"] {
        let c = &v[name];
        keys(c, &["status", "reason", "items"])?;
        let items = rows(&c["items"])?;
        match (c["status"].as_str(), c["reason"].as_str()) {
            (Some("available"), None) if c["reason"].is_null() => {
                need(!items.is_empty(), "available evidence")?;
                for item in items {
                    evidence(item, name)?;
                }
            }
            (Some("references"), Some("aggregate_per_source")) => {
                need(
                    !items.is_empty()
                        && items.iter().all(|v| {
                            v.as_str().is_some_and(|p| {
                                p.starts_with('/') && p.ends_with(&format!("/uncertainty/{name}"))
                            })
                        }),
                    "child references",
                )?;
            }
            (Some("not_configured"), Some("no_probabilistic_model"))
            | (
                Some("not_applicable"),
                Some("exhaustive_analysis" | "no_analysis" | "observable_trace_only"),
            )
            | (
                Some("unavailable"),
                Some(
                    "no_analysis"
                    | "not_lowered"
                    | "input_unavailable"
                    | "effective_inputs_unavailable"
                    | "no_source_reports",
                ),
            ) => need(items.is_empty(), "unavailable has values")?,
            _ => return Err("uncertainty: status/reason".into()),
        }
    }
    Ok(())
}
pub fn validate_host(host: &Value) -> Result<(), String> {
    validate(&host["uncertainty"])?;
    for name in ["aleatory", "estimation", "model"] {
        for item in rows(&host["uncertainty"][name]["items"])? {
            if let Some(at) = item.as_str().or_else(|| item["at"].as_str()) {
                need(host.pointer(at).is_some(), "unresolved evidence")?;
            }
        }
    }
    Ok(())
}

fn absent(status: &str, reason: &str) -> Value {
    json!({"status":status,"reason":reason,"items":[]})
}
fn available(items: Vec<Value>) -> Value {
    json!({"status":"available","reason":null,"items":items})
}

pub fn for_mode(host: &Value, mode: &str) -> Value {
    let mut u = project(host);
    if matches!(mode, "diff" | "compare" | "mutation") {
        let sources = super::hand::links::sources(host, mode);
        for category in ["aleatory", "estimation", "model"] {
            u[category] = if sources.is_empty() {
                absent("unavailable", "no_source_reports")
            } else {
                json!({"status":"references","reason":"aggregate_per_source",
                    "items":sources.iter().map(|p| format!("{}/uncertainty/{category}",p.strip_suffix("/ledger").unwrap())).collect::<Vec<_>>()})
            };
        }
    } else if matches!(mode, "verify" | "cli_refusal") {
        u["aleatory"] = absent("unavailable", "input_unavailable");
        u["estimation"] = absent("not_applicable", "no_analysis");
        u["model"] = absent("unavailable", "input_unavailable");
    } else if mode == "log_conformance" {
        u["aleatory"] = absent("not_applicable", "observable_trace_only");
        u["estimation"] = absent("not_applicable", "observable_trace_only");
    } else if mode == "simulation" && host["simulation"]["confidence"].is_null() {
        let reason = if host["model"]["sha256"].is_null() {
            "input_unavailable"
        } else if host["actors"].is_array() || host["ledger"]["actor_versions"]["status"] == "known"
        {
            "no_analysis"
        } else {
            "not_lowered"
        };
        u["estimation"] = absent("unavailable", reason);
    }
    u
}

/// Hand wire controls, and rebasing metadata only in an already-validated
/// temporary old-field DTO. This does not analyze source or change live output.
pub fn project(host: &Value) -> Value {
    let input_missing = host["model"]["sha256"].is_null();
    let lowered =
        host["actors"].is_array() || host["ledger"]["actor_versions"]["status"] == "known";
    let aleatory = if input_missing {
        absent("unavailable", "input_unavailable")
    } else {
        host.get("uncertainty")
            .map(|u| u["aleatory"].clone())
            .unwrap_or_else(|| {
                if lowered {
                    absent("not_configured", "no_probabilistic_model")
                } else {
                    absent("unavailable", "not_lowered")
                }
            })
    };
    let mut items = vec![
        json!({"kind":"model_assumption","name":"analyst_assumptions_not_exhaustive","subjects":[]}),
    ];
    for (i, _) in host["assumptions"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        items.push(json!({"kind":"assumption","at":format!("/assumptions/{i}")}));
    }
    for key in [
        "cut",
        "cut_context",
        "bounds",
        "agency",
        "via_model_paths",
        "unchecked",
        "not_checked",
        "scope",
        "limits",
        "resource_time",
        "priority_policy",
        "scenario",
        "diagnostics",
        "response_analysis",
        "simulation",
        "approval_burden",
    ] {
        if host.get(key).is_some() {
            items.push(json!({"kind":"model_context","at":format!("/{key}")}));
        }
    }
    if host.pointer("/ledger/execution/limits").is_some() {
        items.push(json!({"kind":"model_context","at":"/ledger/execution/limits"}));
    }
    for (kind, field, name) in [
        (
            "initial_distribution",
            "variable",
            "initial_product_independence",
        ),
        (
            "duration_distribution",
            "capability",
            "duration_result_conditional_independence",
        ),
        (
            "response_channel",
            "binding",
            "response_channel_independence",
        ),
    ] {
        let subjects: Vec<_> = aleatory["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|i| i["kind"] == kind)
            .map(|i| i[field].clone())
            .collect();
        if !subjects.is_empty() {
            items.push(json!({"kind":"model_assumption","name":name,"subjects":subjects}));
        }
    }
    fn quantities(v: &Value, path: &str, out: &mut Vec<Value>) {
        if let Some(o) = v.as_object() {
            for (key, v) in o {
                if path.is_empty() && ["uncertainty", "ledger"].contains(&key.as_str()) {
                    continue;
                }
                let at = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                if ["calendars", "calendar", "arrival_time", "wait_causes"].contains(&key.as_str())
                {
                    out.push(json!({"kind":"model_context","at":at}));
                }
                quantities(v, &at, out);
            }
        } else if let Some(a) = v.as_array() {
            for (i, v) in a.iter().enumerate() {
                quantities(v, &format!("{path}/{i}"), out);
            }
        }
    }
    quantities(host, "", &mut items);
    let model = if input_missing {
        absent("unavailable", "input_unavailable")
    } else if lowered {
        available(items)
    } else {
        absent("unavailable", "not_lowered")
    };
    json!({"rules":"uncertainty-p2-1","aleatory":aleatory,
        "estimation":absent("not_applicable","exhaustive_analysis"),"model":model})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_classes_and_effective_mass_payloads_reject_single_faults() {
        let mut control = project(&json!({"model":{"sha256":"hand-input"}}));
        control["aleatory"] = available(vec![json!({"kind":"initial_distribution",
            "variable":"case.bad","instance":"per_case","span":{"start":0,"end":1},
            "entries":[{"value":"bad","probability":"1/2"},{"value":"good","probability":"1/2"}]})]);
        validate(&control).unwrap();
        for (at, value) in [
            ("/rules", json!("future")),
            ("/aleatory/reason", json!("no_probabilistic_model")),
            (
                "/aleatory/items/0/span",
                json!({"start":0,"end":1,"extra":true}),
            ),
            (
                "/aleatory/items/0/entries/0",
                json!({"value":"bad","probability":"3/2"}),
            ),
            ("/aleatory/items/0/kind", json!("model_context")),
        ] {
            let mut bad = control.clone();
            *bad.pointer_mut(at).unwrap() = value;
            assert!(validate(&bad).is_err(), "escaped {at}");
        }
        for category in ["aleatory", "estimation", "model"] {
            let mut bad = control.clone();
            bad.as_object_mut().unwrap().remove(category);
            assert!(validate(&bad).is_err());
        }
    }
}
