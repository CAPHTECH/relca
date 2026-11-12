//! Closed wire and hand sets. No parser, scheduler, production Agency or arithmetic.
#![allow(dead_code)]
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const CAPS: &[&str] = &[
    "Probe.check",
    "Clerk.enter",
    "Fast.run",
    "Other.enter",
    "Review.choice",
    "Finish.close",
];
pub fn counts(nonzero: &[(&str, u64, u64)]) -> Value {
    Value::Array(
        CAPS.iter()
            .map(|cap| {
                let (_, min, max) = nonzero
                    .iter()
                    .find(|x| x.0 == *cap)
                    .copied()
                    .unwrap_or((cap, 0, 0));
                json!({"capability":cap,"min":min,"max":max})
            })
            .collect(),
    )
}
fn end(name: &str, reachable: bool, starts: &[(&str, u64, u64)]) -> Value {
    let mut may: Vec<_> = starts.iter().filter(|x| x.2 > 0).map(|x| x.0).collect();
    let mut must: Vec<_> = starts.iter().filter(|x| x.1 > 0).map(|x| x.0).collect();
    may.sort();
    must.sort();
    json!({"end":name,"reachable":reachable,"counts":counts(starts),"may_require":may,"must_require":must})
}
fn observation(actor: &str, cap: &str, source: &str, executor: &str) -> Value {
    json!({"actor":actor,"capability":cap,"source":source,"executors":[executor],"qualified":null,"permitted":[executor]})
}
fn side(ends: Vec<Value>, observation: Vec<Value>) -> Value {
    json!({"status":"derived","reason":null,"ends":ends,"observation":observation})
}
fn absent(reason: &str) -> Value {
    json!({"status":"not_derived","reason":reason,"ends":null,"observation":null})
}

pub fn expected(name: &str) -> Value {
    let normal = side(
        vec![end("done", true, &[("Fast.run", 1, 1)])],
        vec![observation("Fast", "Fast.run", "Summary", "f")],
    );
    let degraded = side(
        vec![end("done", true, &[("Clerk.enter", 1, 1)])],
        vec![observation("Clerk", "Clerk.enter", "Doc", "c")],
    );
    let mut route = json!({"name":"Manual","condition":{"binding":"status","value":"fail"},"producer":"Probe.check",
        "common_prefix":counts(&[("Probe.check",1,1)]),"normal":normal,"degraded":degraded});
    match name {
        "a7" | "known-none" | "latent-ok" | "clock" | "defer" | "effect" | "explain" => (),
        "other" | "keep-route" => {
            route["degraded"] = side(vec![end("done", true, &[("Other.enter", 1, 1)])], vec![])
        }
        "keep-other" => {
            route["degraded"] = side(
                vec![end("done", true, &[("Fast.run", 1, 1)])],
                vec![observation("Fast", "Fast.run", "Summary", "f")],
            )
        }
        "a8" => {
            route["degraded"] = side(
                vec![end(
                    "done",
                    true,
                    &[
                        ("Clerk.enter", 0, 1),
                        ("Other.enter", 0, 1),
                        ("Review.choice", 1, 1),
                    ],
                )],
                vec![observation("Clerk", "Clerk.enter", "Doc", "c")],
            )
        }
        "must" => {
            route["degraded"] = side(
                vec![end(
                    "done",
                    true,
                    &[("Clerk.enter", 1, 1), ("Review.choice", 1, 1)],
                )],
                vec![observation("Clerk", "Clerk.enter", "Doc", "c")],
            )
        }
        "guarded" | "guarded-open" => {
            let open = name == "guarded-open";
            route["degraded"] = side(
                vec![
                    end(
                        "alternate",
                        true,
                        &[("Other.enter", 1, 1), ("Review.choice", 1, 1)],
                    ),
                    end(
                        "done",
                        !open,
                        if open {
                            &[]
                        } else {
                            &[("Clerk.enter", 1, 1), ("Review.choice", 1, 1)]
                        },
                    ),
                    end(
                        "impossible",
                        open,
                        if open {
                            &[("Other.enter", 1, 1), ("Review.choice", 1, 1)]
                        } else {
                            &[]
                        },
                    ),
                ],
                if open {
                    vec![]
                } else {
                    vec![observation("Clerk", "Clerk.enter", "Doc", "c")]
                },
            );
        }
        "suffix" => {
            route["normal"]["ends"][0] =
                end("done", true, &[("Fast.run", 1, 1), ("Finish.close", 1, 1)]);
            route["degraded"]["ends"][0] = end(
                "done",
                true,
                &[("Clerk.enter", 1, 1), ("Finish.close", 1, 1)],
            );
        }
        "requires" => route["degraded"] = absent("requires"),
        "state-guard" => route["degraded"] = absent("shared_state_guard"),
        "calendar" => route["degraded"] = absent("calendar"),
        "priority" => {
            route["normal"] = absent("priority");
            route["degraded"] = absent("priority");
        }
        "qualified" => {
            route["degraded"]["observation"][0] = json!({"actor":"Clerk","capability":"Clerk.enter","source":"Doc","executors":["c","z"],"qualified":["z"],"permitted":["c"]})
        }
        _ => panic!("no hand table for {name}"),
    }
    json!({"rules":"agency-degradation-v1","status":"derived","scope":"structural_guard_paths","reason":null,
        "basis":"unknown","unchecked":["retention_basis","actual_skill"],
        "notice":"行使回数から不要とは判定しない","routes":[route]})
}

pub fn bounds(name: &str) -> (u64, u64) {
    match name {
        "other" | "keep-route" => (2, 3),
        "keep-other" => (2, 2),
        "a8" | "must" | "guarded" => (2, 5),
        "guarded-open" => (2, 4),
        "suffix" => (4, 6),
        "latent-ok" => (2, 2),
        "requires" | "state-guard" => (1, 2),
        "calendar" => (2, 6),
        _ => (2, 4),
    }
}

pub fn subset(want: &Value, actual: &Value, path: &str) -> Result<(), String> {
    match want {
        Value::Object(fields) => {
            let a = actual
                .as_object()
                .ok_or_else(|| format!("{path}: object required, got {actual}"))?;
            for (k, v) in fields {
                subset(
                    v,
                    a.get(k).ok_or_else(|| format!("{path}/{k}: missing"))?,
                    &format!("{path}/{k}"),
                )?;
            }
        }
        Value::Array(fields) => {
            let a = actual
                .as_array()
                .ok_or_else(|| format!("{path}: array required"))?;
            if a.len() != fields.len() {
                return Err(format!(
                    "{path}: array length {} != {}",
                    a.len(),
                    fields.len()
                ));
            }
            for (i, (w, a)) in fields.iter().zip(a).enumerate() {
                subset(w, a, &format!("{path}/{i}"))?;
            }
        }
        _ if want != actual => return Err(format!("{path}: expected {want}, got {actual}")),
        _ => (),
    }
    Ok(())
}
fn keys(v: &Value, names: &[&str]) -> Result<(), String> {
    let actual: BTreeSet<_> = v
        .as_object()
        .ok_or("object required")?
        .keys()
        .map(String::as_str)
        .collect();
    if actual != names.iter().copied().collect() {
        return Err(format!("closed keys {names:?}: {actual:?}"));
    }
    Ok(())
}
fn closed_counts(v: &Value) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for row in v.as_array().ok_or("counts array")? {
        keys(row, &["capability", "min", "max"])?;
        let cap = row["capability"].as_str().ok_or("capability string")?;
        if !seen.insert(cap) || !CAPS.contains(&cap) {
            return Err("count identity".into());
        }
        let min = row["min"].as_u64().ok_or("min unsigned")?;
        let max = row["max"].as_u64().ok_or("max unsigned")?;
        if min > max {
            return Err("count interval".into());
        }
    }
    if seen.len() != CAPS.len() {
        return Err("incomplete counts".into());
    }
    Ok(())
}
pub fn validate_table(v: &Value) -> Result<(), String> {
    keys(
        v,
        &[
            "rules",
            "status",
            "scope",
            "reason",
            "basis",
            "unchecked",
            "notice",
            "routes",
        ],
    )?;
    if v["status"] == "not_derived" {
        if *v != expected_limit() {
            return Err("derivation_limit must have null routes, not a partial table".into());
        }
        return Ok(());
    }
    subset(
        &json!({"rules":"agency-degradation-v1","status":"derived","scope":"structural_guard_paths","reason":null,
        "basis":"unknown","unchecked":["retention_basis","actual_skill"],"notice":"行使回数から不要とは判定しない"}),
        v,
        "",
    )?;
    let mut seen = BTreeSet::new();
    for route in v["routes"].as_array().ok_or("routes")? {
        keys(
            route,
            &[
                "name",
                "condition",
                "producer",
                "common_prefix",
                "normal",
                "degraded",
            ],
        )?;
        if !seen.insert(route["name"].as_str().ok_or("route name")?) {
            return Err("duplicate route".into());
        }
        keys(&route["condition"], &["binding", "value"])?;
        if route["condition"]["binding"] != "status"
            || route["condition"]["value"] != "fail"
            || route["producer"] != "Probe.check"
        {
            return Err("condition producer mismatch".into());
        }
        closed_counts(&route["common_prefix"])?;
        for key in ["normal", "degraded"] {
            let side = &route[key];
            keys(side, &["status", "reason", "ends", "observation"])?;
            if side["status"] == "not_derived" {
                if !side["reason"].as_str().is_some_and(|s| !s.is_empty())
                    || !side["ends"].is_null()
                    || !side["observation"].is_null()
                {
                    return Err("not_derived leaks values".into());
                }
                continue;
            }
            if side["status"] != "derived" || !side["reason"].is_null() {
                return Err("side status".into());
            }
            let mut ends = BTreeSet::new();
            for row in side["ends"].as_array().ok_or("ends")? {
                keys(
                    row,
                    &["end", "reachable", "counts", "may_require", "must_require"],
                )?;
                if !ends.insert(row["end"].as_str().ok_or("end name")?) {
                    return Err("duplicate end".into());
                }
                closed_counts(&row["counts"])?;
                let mut may = Vec::new();
                let mut must = Vec::new();
                for c in row["counts"].as_array().unwrap() {
                    if c["max"].as_u64().unwrap() > 0 {
                        may.push(c["capability"].clone());
                    }
                    if c["min"].as_u64().unwrap() > 0 {
                        must.push(c["capability"].clone());
                    }
                }
                may.sort_by_key(Value::to_string);
                must.sort_by_key(Value::to_string);
                if row["may_require"] != json!(may) || row["must_require"] != json!(must) {
                    return Err("may/must count mismatch".into());
                }
                let reachable = row["reachable"].as_bool().ok_or("reachable bool")?;
                if !reachable && !may.is_empty() {
                    return Err("vacuous dependency".into());
                }
            }
            for o in side["observation"].as_array().ok_or("observation")? {
                keys(
                    o,
                    &[
                        "actor",
                        "capability",
                        "source",
                        "executors",
                        "qualified",
                        "permitted",
                    ],
                )?;
                for k in ["executors", "permitted"] {
                    let _ = o[k].as_array().ok_or("person array")?;
                }
                if !o["qualified"].is_null() {
                    for q in o["qualified"].as_array().ok_or("qualified array")? {
                        if !o["executors"].as_array().unwrap().contains(q) {
                            return Err("qualification outside structural executors".into());
                        }
                    }
                }
            }
        }
    }
    if seen.len() != 1 {
        return Err("route inventory".into());
    }
    Ok(())
}

pub fn validate_report(name: &str, v: &Value) -> Result<(), String> {
    let t = v
        .pointer("/agency/degradation")
        .ok_or("missing /agency/degradation")?;
    validate_table(t)?;
    if *t != expected(name) {
        return Err(format!("{name}: hand dependency mismatch: {t}"));
    }
    let ledger = v.get("ledger").ok_or("ledger")?.to_string();
    if !ledger.contains("/agency/degradation/rules") {
        return Err("missing local rules ledger reference".into());
    }
    if !v["uncertainty"]["model"]
        .to_string()
        .contains("/agency/degradation")
    {
        return Err("missing model uncertainty reference".into());
    }
    let retention = &v["agency"]["viewpoints"]["retention"];
    for flag in ["retention_basis", "actual_skill", "fallback_dependency"] {
        if !retention["unchecked"]
            .as_array()
            .ok_or("retention unchecked")?
            .contains(&json!(flag))
        {
            return Err(format!("lost legacy unchecked {flag}"));
        }
    }
    Ok(())
}

pub fn validate_times(name: &str, v: &Value) -> Result<(), String> {
    let (min, max) = if matches!(name, "keep" | "no-keep") {
        (1, 1)
    } else {
        bounds(name)
    };
    let mut rows: Vec<_> = v["terminals"]
        .as_array()
        .ok_or("terminals")?
        .iter()
        .collect();
    let cases = rows.is_empty();
    if cases {
        for c in v["case_outcomes"].as_array().ok_or("case outcomes")? {
            rows.extend(c["terminals"].as_array().ok_or("case terminals")?.iter());
        }
    }
    for (field, lo, hi) in [
        ("elapsed", min, max),
        ("work", min, if name == "calendar" { 4 } else { max }),
    ] {
        let actual_field = if field == "elapsed" && cases {
            "completed_at"
        } else {
            field
        };
        let a = rows
            .iter()
            .filter_map(|r| r[actual_field]["min"].as_u64())
            .min();
        let b = rows
            .iter()
            .filter_map(|r| r[actual_field]["max"].as_u64())
            .max();
        if (a, b) != (Some(lo), Some(hi)) {
            return Err(format!(
                "{name}/{field}: expected [{lo},{hi}], got {a:?}..{b:?}"
            ));
        }
    }
    Ok(())
}
pub fn diagnostic(v: &Value, code: &str, reason: &str) -> Result<(), String> {
    let verdict = if code.starts_with("E-") {
        "invalid"
    } else {
        "unsupported"
    };
    if v["verdict"] != verdict {
        return Err(format!(
            "expected {verdict}/{code}/{reason}: {}",
            v["diagnostics"]
        ));
    }
    if !v["diagnostics"]
        .as_array()
        .ok_or("diagnostics")?
        .iter()
        .any(|d| d["code"] == code && d["message"].as_str().is_some_and(|s| s.contains(reason)))
    {
        return Err(format!("missing {code}/{reason}: {}", v["diagnostics"]));
    }
    if !v["terminals"].as_array().ok_or("terminals")?.is_empty() {
        return Err("static refusal ran prefix".into());
    }
    Ok(())
}
pub fn legacy(v: &Value, kept: bool) -> Result<(), String> {
    if v["verdict"] != "satisfied" {
        return Err(format!("legacy diagnostics {}", v["diagnostics"]));
    }
    if v["agency"].get("degradation").is_some() {
        return Err("keep fabricated degradation".into());
    }
    let retention = &v["agency"]["viewpoints"]["retention"];
    let clerk = retention["capabilities"]
        .as_array()
        .ok_or("retention")?
        .iter()
        .find(|c| c["capability"] == "Clerk.enter")
        .ok_or("Clerk retention")?;
    subset(
        &json!({"normal":{"min":0,"max":0},"fallback":if kept {"kept_by_intent"} else {"unchecked"},"basis":"unknown"}),
        clerk,
        "Clerk",
    )?;
    for flag in ["fallback_dependency", "retention_basis", "actual_skill"] {
        if !retention["unchecked"]
            .as_array()
            .ok_or("unchecked")?
            .contains(&json!(flag))
        {
            return Err(format!("lost {flag}"));
        }
    }
    Ok(())
}
pub fn wire_faults() -> Vec<(&'static str, Value)> {
    let good = expected("a8");
    let mut out = Vec::new();
    for (label, path, value) in [
        ("wrong rules", "/rules", json!("agency-degradation-v2")),
        ("runtime scope", "/scope", json!("actual_skill")),
        ("wrong producer", "/routes/0/producer", json!("Other.enter")),
        (
            "negative count",
            "/routes/0/degraded/ends/0/counts/1/min",
            json!(-1),
        ),
        (
            "inverted count",
            "/routes/0/degraded/ends/0/counts/1/min",
            json!(2),
        ),
        (
            "vacuous reachable",
            "/routes/0/degraded/ends/0/reachable",
            json!(false),
        ),
        ("measured basis", "/basis", json!("measured")),
        (
            "not derived with values",
            "/routes/0/degraded/status",
            json!("not_derived"),
        ),
        (
            "unknown person",
            "/routes/0/degraded/observation/0/qualified",
            json!(["alien"]),
        ),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(path).expect("single existing field") = value;
        out.push((label, bad));
    }
    let mut extra = good.clone();
    extra["score"] = json!(100);
    out.push(("unknown field", extra));
    let mut dup = good.clone();
    dup["routes"]
        .as_array_mut()
        .unwrap()
        .push(good["routes"][0].clone());
    out.push(("duplicate route", dup));
    let mut may = good;
    may["routes"][0]["degraded"]["ends"][0]["must_require"] =
        json!(["Clerk.enter", "Review.choice"]);
    out.push(("may promoted to must", may));
    out
}

pub fn exit(v: &Value) -> i32 {
    match v["verdict"].as_str().unwrap() {
        "satisfied" => 0,
        "violated" => 1,
        "inconclusive" => 2,
        "invalid" => 3,
        "unsupported" => 4,
        "vacuous" => 5,
        _ => panic!("verdict"),
    }
}

/// Implementation-owned cfg(test) adapter must call this with its real derivation
/// budget, never a public fake Report. Return the Agency degradation value.
pub fn budget_contract(mut derive: impl FnMut(usize) -> Value) {
    assert_eq!(derive(1_000_000), expected("a8"));
    let zero = derive(0);
    assert_eq!(zero, expected_limit());
}

pub fn expected_limit() -> Value {
    json!({"rules":"agency-degradation-v1","status":"not_derived","scope":"structural_guard_paths",
        "reason":"derivation_limit","basis":"unknown","unchecked":["retention_basis","actual_skill"],
        "notice":"行使回数から不要とは判定しない","routes":null})
}

/// Private adapter: lower q6b_inputs::source("a7") afresh for every case,
/// apply exactly the named metadata fault, then run both real IR entrances.
/// The second tuple member is format!("{:?}", reference::explore(&model).stop).
/// No new production API/IR field is referenced before it exists.
pub fn route_ir_contract(mut run: impl FnMut(&str) -> (Vec<(String, String)>, String)) {
    let (good, stop) = run("valid");
    assert_eq!(stop, "Complete");
    assert!(
        !good
            .iter()
            .any(|(code, _)| code.starts_with("U-") || code.starts_with("E-"))
    );
    for fault in [
        "producer_outside_model",
        "condition_producer_mismatch",
        "degraded_calls_disagree_with_ir",
    ] {
        let (diagnostics, stop) = run(fault);
        assert!(
            diagnostics
                .iter()
                .any(|(code, message)| code == "U-SEM-0" && message.contains("invalid_q6_ir")),
            "{fault}: engine must reject inconsistent route metadata, not execute a fallback"
        );
        assert_eq!(
            stop, "SemanticAbort",
            "{fault}: independent reference validation"
        );
    }
}
