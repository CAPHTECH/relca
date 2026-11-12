//! Independent closed Q8 wire and arithmetic validator. No production evaluators.
use serde_json::{Value, json};
use std::collections::BTreeSet;

type Result<T> = std::result::Result<T, String>;
fn need(test: bool, why: &str) -> Result<()> {
    if test { Ok(()) } else { Err(why.into()) }
}
fn keys(v: &Value, expected: &[&str]) -> Result<()> {
    let o = v.as_object().ok_or("object required")?;
    need(
        o.len() == expected.len() && expected.iter().all(|k| o.contains_key(*k)),
        "missing/unknown key",
    )
}
fn text(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.is_empty())
}
fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("array required".into())
}
fn count(v: &Value) -> Result<u64> {
    v.as_u64().ok_or("nonnegative integer required".into())
}
fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}
pub fn fraction(v: &Value) -> Result<(u128, u128)> {
    let s = v.as_str().ok_or("fraction string required")?;
    let (n, d) = s.split_once('/').unwrap_or((s, "1"));
    need(
        !n.is_empty() && !d.is_empty() && n.bytes().chain(d.bytes()).all(|b| b.is_ascii_digit()),
        "fraction digits",
    )?;
    let n: u128 = n.parse().map_err(|_| "numerator range")?;
    let d: u128 = d.parse().map_err(|_| "denominator range")?;
    need(d > 0 && gcd(n, d) == 1, "canonical fraction")?;
    let canonical = if d == 1 {
        n.to_string()
    } else {
        format!("{n}/{d}")
    };
    need(s == canonical, "canonical representation")?;
    Ok((n, d))
}
fn coverage(v: &Value) -> Result<()> {
    need(
        matches!(v.as_str(), Some("partial" | "observed_scope_only")),
        "coverage",
    )
}
fn measure(v: &Value, null_allowed: bool) -> Result<()> {
    if v.is_null() {
        need(null_allowed, "unexpected null")
    } else {
        fraction(v).map(|_| ())
    }
}
pub fn has_reason(v: &Value, reason: &str) -> bool {
    v["diagnostics"]
        .as_array()
        .is_some_and(|a| a.iter().any(|d| d["reason"] == reason))
        || v["checks"]
            .as_array()
            .is_some_and(|a| a.iter().any(|d| d["reason"] == reason))
        || v["approval_burden"]["episodes"]
            .as_array()
            .is_some_and(|a| {
                a.iter().any(|e| {
                    ["rate", "reinvestigation"].iter().any(|k| {
                        e[k]["reasons"]
                            .as_array()
                            .is_some_and(|r| r.iter().any(|s| s == reason))
                    })
                })
            })
}
fn reasons(v: &Value, known: bool) -> Result<()> {
    let a = array(v)?;
    need(a.iter().all(text), "reason text")?;
    need(
        if known { a.is_empty() } else { !a.is_empty() },
        "known/unknown reasons",
    )
}
pub fn metrics(v: &Value) -> Result<()> {
    keys(
        v,
        &[
            "schema",
            "rules",
            "spec",
            "scope_notice",
            "episodes",
            "cases",
            "unassigned_reinvestigation_tasks",
            "rate",
            "reinvestigation",
        ],
    )?;
    need(
        v["schema"] == "relca-approval-burden/v1" && v["rules"] == "approval-burden-observed-p2-1",
        "metric version",
    )?;
    need(text(&v["scope_notice"]), "scope notice")?;
    keys(
        &v["spec"],
        &[
            "sha256",
            "profile",
            "status",
            "max_bytes",
            "max_episodes",
            "processed_episodes",
            "truncated",
        ],
    )?;
    need(
        v["spec"]["status"] == "declared_not_certified",
        "declaration is not certified",
    )?;
    need(
        v["spec"]["sha256"].as_str().is_some_and(|s| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        }) || (v["spec"]["truncated"] == true && v["spec"]["sha256"].is_null()),
        "spec hash",
    )?;
    need(
        v["spec"]["max_bytes"] == 1048576 && v["spec"]["max_episodes"] == 10000,
        "spec limits",
    )?;
    count(&v["spec"]["processed_episodes"])?;
    need(v["spec"]["truncated"].is_boolean(), "truncation bool")?;
    let truncated = v["spec"]["truncated"] == true || v["rate"]["reason"] == "truncated";
    let r = &v["rate"];
    let t = &v["reinvestigation"];
    keys(
        r,
        &[
            "numerator",
            "denominator",
            "known_ineligible",
            "unknown_episodes",
            "no_observed_approval_cases",
            "value",
            "reason",
            "coverage",
        ],
    )?;
    keys(
        t,
        &[
            "total",
            "mean",
            "known_episodes",
            "unknown_episodes",
            "reason",
            "unit",
            "coverage",
        ],
    )?;
    coverage(&r["coverage"])?;
    coverage(&t["coverage"])?;
    need(t["unit"] == "person_min", "person minutes")?;
    if truncated {
        need(r["reason"] == "truncated", "input cut rate reason")?;
        need(
            r["numerator"].is_null() && r["denominator"].is_null() && r["value"].is_null(),
            "prefix must not become full rate",
        )?;
        need(
            t["total"].is_null() && t["mean"].is_null() && t["reason"] == "truncated",
            "prefix time withdrawal",
        )?;
        for key in [
            "known_ineligible",
            "unknown_episodes",
            "no_observed_approval_cases",
        ] {
            need(r[key].is_null(), "unobserved rate census must be null")?;
        }
        for key in ["known_episodes", "unknown_episodes"] {
            need(t[key].is_null(), "unobserved time census must be null")?;
        }
        need(v["cases"].is_null(), "unobserved cases must be null")?;
        need(
            r["coverage"] == "partial" && t["coverage"] == "partial",
            "input cut coverage",
        )?;
    } else {
        let n = count(&r["numerator"])? as u128;
        let d = count(&r["denominator"])? as u128;
        need(n <= d, "N exceeds D")?;
        if d == 0 {
            need(
                r["value"].is_null()
                    && matches!(
                        r["reason"].as_str(),
                        Some("zero_denominator" | "not_configured")
                    ),
                "zero denominator",
            )?;
        } else {
            let (a, b) = fraction(&r["value"])?;
            need(
                a * d == n * b && r["reason"].is_null(),
                "ratio disagrees with N/D",
            )?;
        }
        for key in [
            "known_ineligible",
            "unknown_episodes",
            "no_observed_approval_cases",
        ] {
            count(&r[key])?;
        }
        if count(&r["unknown_episodes"])? + count(&r["no_observed_approval_cases"])? > 0 {
            need(r["coverage"] == "partial", "unknown rate coverage")?;
        }
        let known = count(&t["known_episodes"])? as u128;
        count(&t["unknown_episodes"])?;
        if t["total"].is_null() {
            need(
                t["mean"].is_null() && text(&t["reason"]),
                "null time reason",
            )?;
        } else {
            let (a, b) = fraction(&t["total"])?;
            let (x, y) = fraction(&t["mean"])?;
            need(
                known > 0 && a * y == x * b * known && t["reason"].is_null(),
                "time mean denominator",
            )?;
        }
    }
    let episodes = array(&v["episodes"])?;
    let mut last = None;
    let mut n = 0;
    let mut d = 0;
    let mut ineligible = 0;
    let mut unknown = 0;
    let mut kt = 0;
    let mut ut = 0;
    let mut time_sum = (0u128, 1u128);
    for e in episodes {
        keys(
            e,
            &[
                "case_id",
                "approval_task_id",
                "spec_pointer",
                "rate",
                "reinvestigation",
                "evidence",
            ],
        )?;
        need(
            text(&e["case_id"]) && text(&e["approval_task_id"]),
            "episode identity",
        )?;
        // Missing-link observations have no real sidecar pointer.
        need(
            e["spec_pointer"].is_null()
                || e["spec_pointer"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("/episodes/")),
            "spec pointer",
        )?;
        let key = (
            e["case_id"].as_str().unwrap(),
            e["approval_task_id"].as_str().unwrap(),
        );
        need(last.is_none_or(|p| p < key), "episode duplicate/order")?;
        last = Some(key);
        keys(&e["rate"], &["status", "classification", "reasons"])?;
        let known = e["rate"]["status"] == "known";
        reasons(&e["rate"]["reasons"], known)?;
        match e["rate"]["classification"].as_str() {
            Some("endorsed") if known => {
                n += 1;
                d += 1;
            }
            Some("not_endorsed") if known => d += 1,
            Some("ineligible") if known => ineligible += 1,
            Some("unknown") if e["rate"]["status"] == "unknown" => unknown += 1,
            Some("unsupported") if e["rate"]["status"] == "unsupported" => unknown += 1,
            _ => return Err("episode rate classification/status".into()),
        }
        let z = &e["reinvestigation"];
        keys(z, &["status", "value", "reasons"])?;
        let known = matches!(
            z["status"].as_str(),
            Some("known" | "declared_empty_observation")
        );
        reasons(&z["reasons"], known)?;
        measure(&z["value"], !known)?;
        if known {
            let (a, b) = fraction(&z["value"])?;
            time_sum = (time_sum.0 * b + a * time_sum.1, time_sum.1 * b);
            let g = gcd(time_sum.0, time_sum.1);
            time_sum = (time_sum.0 / g, time_sum.1 / g);
            kt += 1;
            if z["status"] == "declared_empty_observation" {
                need(z["value"] == "0", "empty observed time")?;
            }
        } else {
            need(
                z["value"].is_null()
                    && matches!(z["status"].as_str(), Some("unknown" | "unsupported")),
                "unknown time",
            )?;
            ut += 1;
        }
        let mut ids = BTreeSet::new();
        for evidence in array(&e["evidence"])? {
            keys(
                evidence,
                &["event_id", "line", "case_id", "task_id", "time", "after"],
            )?;
            need(
                text(&evidence["event_id"]) && ids.insert(evidence["event_id"].as_str().unwrap()),
                "evidence duplicate",
            )?;
            need(
                count(&evidence["line"])? > 0 && evidence["case_id"] == e["case_id"],
                "evidence identity",
            )?;
            need(
                evidence["time"]["earliest"]
                    .as_u64()
                    .zip(evidence["time"]["latest"].as_u64())
                    .is_some_and(|(a, b)| a <= b),
                "evidence time",
            )?;
            array(&evidence["after"])?;
        }
    }
    if !truncated {
        if episodes.iter().any(|e| {
            e["rate"]["reasons"]
                .as_array()
                .is_some_and(|a| a.contains(&json!("not_configured")))
        }) {
            need(
                r["reason"] == "not_configured",
                "deferred rate aggregate reason",
            )?;
        }
        need(
            r["numerator"] == n
                && r["denominator"] == d
                && r["known_ineligible"] == ineligible
                && r["unknown_episodes"] == unknown,
            "episode rate census",
        )?;
        // A deferred/cross-episode aggregate may withdraw values independently.
        if t["reason"].is_null() || t["reason"] == "no_known_observations" {
            need(
                t["known_episodes"] == kt && t["unknown_episodes"] == ut,
                "episode time census",
            )?;
        }
        if !t["total"].is_null() {
            let (a, b) = fraction(&t["total"])?;
            need(a * time_sum.1 == time_sum.0 * b, "episode time sum")?;
        }
    }
    let mut previous = None;
    for case in if truncated {
        &[][..]
    } else {
        array(&v["cases"])?.as_slice()
    } {
        keys(
            case,
            &[
                "case_id",
                "episodes",
                "rate_unknown_episodes",
                "time_unknown_episodes",
                "no_observed_approval",
                "coverage",
            ],
        )?;
        let id = case["case_id"].as_str().ok_or("case id")?;
        need(previous.is_none_or(|p| p < id), "case duplicate/order")?;
        previous = Some(id);
        count(&case["episodes"])?;
        count(&case["rate_unknown_episodes"])?;
        count(&case["time_unknown_episodes"])?;
        need(
            case["no_observed_approval"].is_boolean(),
            "no approval bool",
        )?;
        coverage(&case["coverage"])?;
    }
    for orphan in array(&v["unassigned_reinvestigation_tasks"])? {
        keys(orphan, &["case_id", "task_id", "event_ids"])?;
        need(
            text(&orphan["case_id"])
                && text(&orphan["task_id"])
                && !array(&orphan["event_ids"])?.is_empty(),
            "orphan identity",
        )?;
    }
    Ok(())
}
pub fn report(v: &Value, exit: i32) -> Result<()> {
    need(
        v["schema"] == "relca-conformance-approval/v1",
        "Q8_UNIMPLEMENTED: new scope/sidecar/result schema missing (not ledger missing)",
    )?;
    need(
        v["scope"] == "observable_event_contracts_with_approval_burden_v1"
            && v["mode"] == "log_conformance",
        "Q8 scope/mode",
    )?;
    need(
        v.get("approval_burden").is_some(),
        "Q8_UNIMPLEMENTED: approval_burden field missing",
    )?;
    let (verdict, tri) = match exit {
        0 => ("satisfied", "conformant"),
        1 => ("violated", "violating"),
        2 => ("inconclusive", "unknown"),
        3 => ("invalid", "unknown"),
        4 => ("unsupported", "unknown"),
        _ => return Err("exit range".into()),
    };
    need(
        v["verdict"] == verdict && v["conformance"] == tri,
        "verdict/exit",
    )?;
    need(
        v["ledger"]["schema"] == "relca-conclusion-ledger/v1",
        "existing ledger lost",
    )?;
    if v["approval_burden"].is_null() {
        need(
            matches!(exit, 3 | 4),
            "missing metric result on measured run",
        )?;
        if exit == 3 {
            need(
                array(&v["checks"])?.is_empty() && array(&v["cases"])?.is_empty(),
                "invalid prefix conclusions",
            )?;
        }
        return Ok(());
    }
    metrics(&v["approval_burden"])?;
    need(
        array(&v["ledger"]["subjects"])?
            .iter()
            .any(|s| s["target"] == "/approval_burden"),
        "metric conclusions not linked",
    )?;
    need(
        array(&v["ledger"]["derivation"]["components"])?
            .iter()
            .any(|s| {
                s["at"] == "/approval_burden/rules"
                    && s["version"] == "approval-burden-observed-p2-1"
            }),
        "metric rule version not linked",
    )?;
    Ok(())
}

/// Hand-assembled wire control; this is not a capture from any Relca build.
pub fn control() -> Value {
    json!({"schema":"relca-approval-burden/v1","rules":"approval-burden-observed-p2-1",
      "spec":{"sha256":"0000000000000000000000000000000000000000000000000000000000000000","profile":{},"status":"declared_not_certified","max_bytes":1048576,"max_episodes":10000,"processed_episodes":1,"truncated":false},
      "scope_notice":"synthetic observation only","episodes":[{"case_id":"c","approval_task_id":"a","spec_pointer":"/episodes/0",
       "rate":{"status":"known","classification":"endorsed","reasons":[]},
       "reinvestigation":{"status":"known","value":"4","reasons":[]},"evidence":[]}],
      "cases":[{"case_id":"c","episodes":1,"rate_unknown_episodes":0,"time_unknown_episodes":0,"no_observed_approval":false,"coverage":"observed_scope_only"}],
      "unassigned_reinvestigation_tasks":[],
      "rate":{"numerator":1,"denominator":1,"known_ineligible":0,"unknown_episodes":0,"no_observed_approval_cases":0,"value":"1","reason":null,"coverage":"observed_scope_only"},
      "reinvestigation":{"total":"4","mean":"4","known_episodes":1,"unknown_episodes":0,"reason":null,"unit":"person_min","coverage":"observed_scope_only"}})
}

/// No candidate census was observed; zero would falsely claim an observation.
pub fn truncated_control(spec_cut: bool) -> Value {
    let mut v = control();
    v["spec"]["truncated"] = json!(spec_cut);
    if spec_cut {
        v["spec"]["sha256"] = Value::Null;
    }
    v["spec"]["processed_episodes"] = json!(0);
    v["episodes"] = json!([]);
    v["cases"] = Value::Null;
    for key in [
        "numerator",
        "denominator",
        "value",
        "known_ineligible",
        "unknown_episodes",
        "no_observed_approval_cases",
    ] {
        v["rate"][key] = Value::Null;
    }
    for key in ["total", "mean", "known_episodes", "unknown_episodes"] {
        v["reinvestigation"][key] = Value::Null;
    }
    for key in ["rate", "reinvestigation"] {
        v[key]["reason"] = json!("truncated");
        v[key]["coverage"] = json!("partial");
    }
    v
}
