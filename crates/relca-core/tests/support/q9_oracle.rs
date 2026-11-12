//! Independent finite hand values. No production arithmetic, lowerer or scheduler.
#![allow(dead_code)]
use serde_json::{Value, json};

pub const C: [u64; 16] = [0, 0, 2, 0, 0, 0, 0, 2, 2, 0, 2, 0, 0, 0, 0, 0];
pub const HALF: [u64; 16] = [0, 2, 2, 0, 0, 4, 2, 2, 2, 0, 2, 0, 4, 0, 0, 0];
pub const ORDINAL: [u64; 16] = [2, 2, 2, 0, 0, 0, 0, 2, 0, 2, 0, 0, 0, 0, 0, 0];
pub const TARGETS: [bool; 16] = [
    false, true, true, false, true, true, true, false, false, false, true, false, false, false,
    false, false,
];

pub fn fraction(n: u64, d: u64) -> String {
    assert!(d > 0);
    let (mut a, mut b) = (n, d);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    if d / a == 1 {
        (n / a).to_string()
    } else {
        format!("{}/{}", n / a, d / a)
    }
}
pub fn sample(mean: &str, min: u64, max: u64, n: u64) -> Value {
    json!({"mean":mean,"min":min.to_string(),"max":max.to_string(),"samples":n,"reason":null})
}
pub fn absent(reason: &str) -> Value {
    json!({"mean":null,"min":null,"max":null,"samples":0,"reason":reason})
}
pub fn range(a: &str, b: &str) -> Value {
    json!({"min":a,"max":b,"reason":null})
}
pub fn table(rows: Vec<Value>) -> Value {
    json!({"rules":"contest-rates-sample-p2-1","basis":"sample","unit":"replication",
        "scope":"declared-model-not-party-representation","rows":rows})
}
#[allow(clippy::too_many_arguments)]
pub fn row(
    case: u64,
    p: &str,
    target: u64,
    filed: u64,
    late: u64,
    work: Value,
    wait: Value,
    elapsed: Value,
) -> Value {
    json!({"id":"contract Controls #1 contest","case_id":case,"party":"Party","target":"returned","route":"Reviewer.reply",
        "configured_probability":p,"occurrence_basis":"synthetic occurrence fixture; not measured",
        "service_time_basis":"synthetic service fixture; not measured","independence":"per_case_per_clause",
        "cohort":"normal_response_observed","trials":16,"target_observed":target,"target_not_reached":16-target,"target_unknown":0,
        "filed":filed,"not_filed":target-filed,"choice_unknown":0,"responded":filed-late,"late_responded":late,"unresolved":0,"route_failed":0,
        "filing_fraction":{"value":if target==0 {None} else {Some(fraction(filed,target))},"numerator":filed,"denominator":target,"reason":if target==0 {Some("no_samples")} else {None}},
        "service_time":work,"queue_wait":wait,"response_elapsed":elapsed})
}
pub fn point_row(case: u64, wait: u64) -> Value {
    row(
        case,
        "1",
        16,
        16,
        if wait > 1 { 16 } else { 0 },
        sample("2", 2, 2, 16),
        sample(&wait.to_string(), wait, wait, 16),
        sample(&(2 + wait).to_string(), 2 + wait, 2 + wait, 16),
    )
}

pub fn expected_table(name: &str) -> Value {
    let zero = |n| sample("0", 0, 0, n);
    let mut c = row(
        1,
        "1/4",
        16,
        4,
        0,
        sample("2", 2, 2, 4),
        zero(4),
        sample("2", 2, 2, 4),
    );
    match name {
        "c" | "mixed-unreached" => table(vec![c]),
        "half" => table(vec![row(
            1,
            "1/2",
            16,
            8,
            2,
            sample("5/2", 2, 4, 8),
            zero(8),
            sample("5/2", 2, 4, 8),
        )]),
        "conditional-target" => table(vec![row(
            1,
            "1/4",
            6,
            2,
            0,
            sample("2", 2, 2, 2),
            zero(2),
            sample("2", 2, 2, 2),
        )]),
        "ordinal" => {
            c = row(
                1,
                "1/4",
                16,
                5,
                0,
                sample("2", 2, 2, 5),
                zero(5),
                sample("2", 2, 2, 5),
            );
            c["id"] = json!("contract Controls #2 contest");
            table(vec![c])
        }
        "fixed" | "deadline" | "normal-call" => table(vec![point_row(1, 0)]),
        "late" => {
            c = point_row(1, 0);
            c["responded"] = json!(0);
            c["late_responded"] = json!(16);
            table(vec![c])
        }
        "clock" | "deadlock" => {
            c = point_row(1, 0);
            c["responded"] = json!(0);
            c[if name == "clock" {
                "unresolved"
            } else {
                "route_failed"
            }] = json!(16);
            for k in ["service_time", "queue_wait", "response_elapsed"] {
                c[k] = absent(if name == "clock" {
                    "truncated"
                } else {
                    "no_samples"
                });
            }
            table(vec![c])
        }
        "defer" | "defer-fixed" => {
            if name == "defer-fixed" {
                c = point_row(1, 0);
            }
            c["filing_fraction"]["value"] = Value::Null;
            c["filing_fraction"]["reason"] = json!("not_configured");
            for k in ["service_time", "queue_wait", "response_elapsed"] {
                c[k] = absent("not_configured");
            }
            table(vec![c])
        }
        "two-spaced" => table(vec![point_row(1, 0), point_row(2, 0)]),
        "two-burst" | "calendar" => table(vec![point_row(1, 0), point_row(2, 1)]),
        "calendar-later" => table(vec![point_row(1, 0), point_row(2, 2)]),
        "two-clauses" => {
            let mut second = point_row(1, 2);
            second["id"] = json!("contract Controls #2 contest");
            // Same-tick dispatch may choose either clause, so this ordered hand
            // object is used for independent wire controls only, not runtime.
            table(vec![point_row(1, 0), second])
        }
        _ => panic!("no deterministic table {name}"),
    }
}

/// Compare required JSON paths, tolerating unrelated Q7 uncertainty additions.
/// Scalar shape remains closed: null/reason/sample count cannot be hidden.
pub fn subset(want: &Value, actual: &Value, path: &str) -> Result<(), String> {
    match want {
        Value::Object(fields) => {
            let a = actual
                .as_object()
                .ok_or_else(|| format!("{path}: expected object, got {actual}"))?;
            if fields.contains_key("mean") || fields.contains_key("min") {
                if actual != want {
                    return Err(format!("{path}: expected scalar {want}, got {actual}"));
                }
            } else {
                for (key, v) in fields {
                    subset(
                        v,
                        a.get(key).ok_or_else(|| format!("{path}/{key}: missing"))?,
                        &format!("{path}/{key}"),
                    )?;
                }
            }
        }
        Value::Array(rows) => {
            let a = actual
                .as_array()
                .ok_or_else(|| format!("{path}: expected array"))?;
            if a.len() != rows.len() {
                return Err(format!(
                    "{path}: expected {} rows, got {}",
                    rows.len(),
                    a.len()
                ));
            }
            for (i, row) in rows.iter().enumerate() {
                subset(row, &a[i], &format!("{path}/{i}"))?;
            }
        }
        _ if want != actual => return Err(format!("{path}: expected {want}, got {actual}")),
        _ => (),
    }
    Ok(())
}
pub fn property<'a>(v: &'a Value, suffix: &str) -> &'a Value {
    let rows = v["properties"].as_array().expect("property array");
    let found: Vec<_> = rows
        .iter()
        .filter(|p| p["id"].as_str().is_some_and(|s| s.ends_with(suffix)))
        .collect();
    assert_eq!(found.len(), 1, "unique property {suffix}: {rows:?}");
    found[0]
}
pub fn diagnostic(v: &Value, code: &str, reason: &str) -> Result<(), String> {
    let want = if code.starts_with("E-") {
        "invalid"
    } else {
        "unsupported"
    };
    subset(&json!({"verdict":want}), v, "")?;
    if !v["diagnostics"]
        .as_array()
        .ok_or("diagnostics missing")?
        .iter()
        .any(|d| d["code"] == code && d["message"].as_str().is_some_and(|s| s.contains(reason)))
    {
        return Err(format!(
            "diagnostics: expected {code}/{reason}, got {}",
            v["diagnostics"]
        ));
    }
    Ok(())
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

pub fn assumptions(v: &Value, ordinal: u64) -> Result<(), String> {
    let rows = v["assumptions"].as_array().ok_or("assumptions missing")?;
    for (field, value) in [
        (
            "occurrence_basis",
            "synthetic occurrence fixture; not measured",
        ),
        (
            "service_time_basis",
            "synthetic service fixture; not measured",
        ),
        ("independence", "per_case_per_clause"),
        ("validation", "analyst-declared-not-validated"),
    ] {
        let key = format!("contest.{ordinal}.{field}");
        let found: Vec<_> = rows.iter().filter(|a| a["field"] == key).collect();
        if found.len() != 1 || found[0]["value"] != value {
            return Err(format!("assumptions/{key}: expected unique {value}"));
        }
    }
    Ok(())
}

pub fn validate_sim(name: &str, v: &Value) -> Result<(), String> {
    let aborted = matches!(name, "mixed" | "route-nondet" | "same-tick" | "two-clauses");
    let verdict = if aborted {
        "unsupported"
    } else if matches!(name, "half" | "late" | "deadlock" | "calendar-later") {
        "violated"
    } else {
        "inconclusive"
    };
    subset(&json!({"mode":"simulation","verdict":verdict}), v, "")?;
    if aborted {
        if !v["diagnostics"]
            .as_array()
            .ok_or("diagnostics")?
            .iter()
            .any(|d| d["code"] == "U-SEM-0")
        {
            return Err("missing nondeterminism refusal".into());
        }
        // A prefix must never become a set of complete sample observations.
        if let Some(rows) = v
            .pointer("/metrics/contests/rows")
            .and_then(Value::as_array)
        {
            let wanted = if matches!(name, "same-tick" | "two-clauses") {
                2
            } else {
                1
            };
            if rows.len() != wanted {
                return Err(format!("aborted row inventory: expected {wanted}"));
            }
            for r in rows {
                for k in [
                    "target_observed",
                    "target_not_reached",
                    "target_unknown",
                    "filed",
                    "not_filed",
                    "choice_unknown",
                    "responded",
                    "late_responded",
                    "unresolved",
                    "route_failed",
                ] {
                    if !r.get(k).is_some_and(Value::is_null) {
                        return Err(format!("aborted rows/{k}: required null"));
                    }
                }
                for k in ["service_time", "queue_wait", "response_elapsed"] {
                    subset(&absent("aborted"), &r[k], &format!("aborted/{k}"))?;
                }
                subset(
                    &json!({"value":null,"numerator":null,"denominator":null,"reason":"aborted"}),
                    &r["filing_fraction"],
                    "aborted/filing_fraction",
                )?;
                if r["trials"] != 16 {
                    return Err("aborted trials must retain the input".into());
                }
            }
        } else {
            return Err("aborted metrics.contests shape missing".into());
        }
        return Ok(());
    }
    assumptions(v, if name == "ordinal" { 2 } else { 1 })?;
    if name == "no-metrics" {
        if v.get("metrics").is_some() {
            return Err("undeclared metrics".into());
        }
        return Ok(());
    }
    let t = v
        .pointer("/metrics/contests")
        .ok_or("metrics/contests: missing Q9 sample table")?;
    subset(&expected_table(name), t, "/metrics/contests")?;
    let runtime = property(
        v,
        if name == "ordinal" {
            " #2 contest_response"
        } else {
            " #1 contest_response"
        },
    );
    let expected_runtime = if matches!(name, "half" | "late" | "deadlock" | "calendar-later") {
        "violated"
    } else {
        "inconclusive"
    };
    if runtime["verdict"] != expected_runtime {
        return Err(format!(
            "contest_response: expected {expected_runtime}, got {runtime}"
        ));
    }
    // These are per-replication totals over both cases, not 32 independent trials.
    if name == "c" {
        subset(
            &sample("3/2", 1, 3, 16),
            &v["metrics"]["overall"]["work"],
            "overall/work",
        )?;
        subset(
            &sample("11/2", 0, 22, 16),
            &v["metrics"]["overall"]["observed_cost_total"],
            "overall/cost",
        )?;
    }
    if name == "half" {
        subset(
            &sample("9/4", 1, 5, 16),
            &v["metrics"]["overall"]["work"],
            "overall/work",
        )?;
        subset(
            &sample("27/2", 0, 42, 16),
            &v["metrics"]["overall"]["observed_cost_total"],
            "overall/cost",
        )?;
    }
    if matches!(
        name,
        "two-spaced" | "two-burst" | "calendar" | "calendar-later"
    ) {
        subset(
            &sample("6", 6, 6, 16),
            &v["metrics"]["overall"]["work"],
            "overall/work unit=replication",
        )?;
    }
    if name == "normal-call" {
        subset(
            &sample("5", 5, 5, 16),
            &v["metrics"]["overall"]["work"],
            "normal-call/overall/work",
        )?;
        subset(
            &sample("44", 44, 44, 16),
            &v["metrics"]["overall"]["observed_cost_total"],
            "normal-call/overall/cost",
        )?;
        let terminal = v["metrics"]["terminals"]
            .as_array()
            .ok_or("terminal metrics")?
            .iter()
            .find(|r| r["end"] == "returned")
            .ok_or("returned terminal")?;
        subset(
            &sample("22", 22, 22, 16),
            &terminal["cost_per_case"],
            "normal-call/end excludes route",
        )?;
    }
    Ok(())
}

pub fn validate_table(t: &Value) -> Result<(), String> {
    subset(
        &json!({"rules":"contest-rates-sample-p2-1","basis":"sample","unit":"replication","scope":"declared-model-not-party-representation"}),
        t,
        "table",
    )?;
    if t.as_object().ok_or("table object")?.len() != 5 {
        return Err("unknown table field".into());
    }
    let rows = t["rows"].as_array().ok_or("rows")?;
    let expected_keys = expected_table("fixed")["rows"][0]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut seen = std::collections::BTreeSet::new();
    for r in rows {
        let fields = r.as_object().ok_or("row object")?;
        if fields
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            != expected_keys
        {
            return Err("unknown/missing row field".into());
        }
        let n = |k: &str| {
            r[k].as_u64()
                .ok_or_else(|| format!("row/{k}: unsigned count"))
        };
        if !seen.insert((r["id"].as_str().ok_or("id")?, n("case_id")?)) {
            return Err("duplicate row".into());
        }
        if r["route"] != "Reviewer.reply" || r["cohort"] != "normal_response_observed" {
            return Err("route/cohort mismatch".into());
        }
        if n("target_observed")? + n("target_not_reached")? + n("target_unknown")? != n("trials")? {
            return Err("target partition".into());
        }
        if n("filed")? + n("not_filed")? + n("choice_unknown")? != n("target_observed")? {
            return Err("filing partition".into());
        }
        if n("responded")? + n("late_responded")? + n("unresolved")? + n("route_failed")?
            != n("filed")?
        {
            return Err("response partition".into());
        }
        let f = &r["filing_fraction"];
        if f["numerator"] != r["filed"] || f["denominator"] != r["target_observed"] {
            return Err("conditional denominator".into());
        }
        if f["reason"].is_null()
            && (n("target_observed")? == 0
                || f["value"] != fraction(n("filed")?, n("target_observed")?))
        {
            return Err("fraction mismatch".into());
        }
        let mut sample_count = None;
        for k in ["service_time", "queue_wait", "response_elapsed"] {
            let s = &r[k];
            let count = s["samples"].as_u64().ok_or("samples")?;
            if sample_count.replace(count).is_some_and(|old| old != count) {
                return Err("different time cohorts".into());
            }
            if s["reason"].is_null() {
                if count == 0 || count != n("responded")? + n("late_responded")? {
                    return Err("time count".into());
                }
                let _ = s["mean"].as_str().ok_or("mean")?;
            } else if count != 0
                || !s["mean"].is_null()
                || !s["min"].is_null()
                || !s["max"].is_null()
            {
                return Err("null sample leak".into());
            }
        }
        // The deterministic controls have integer means. Reject even a coherent
        // but false partition of service/queue/elapsed via the fixed hand oracle.
        if r["configured_probability"] == "1"
            && r["service_time"]["reason"].is_null()
            && r["service_time"]["mean"] != "2"
        {
            return Err("fixed service must be two".into());
        }
    }
    Ok(())
}

pub fn wire_faults() -> Vec<(&'static str, Value)> {
    let good = expected_table("fixed");
    let mut out = Vec::new();
    for (label, path, bad) in [
        (
            "wrong denominator",
            "/rows/0/filing_fraction/denominator",
            json!(32),
        ),
        (
            "missing case counted as zero",
            "/rows/0/target_observed",
            json!(0),
        ),
        ("filed frequency from dedup", "/rows/0/filed", json!(1)),
        ("negative count", "/rows/0/responded", json!(-1)),
        ("wrong route", "/rows/0/route", json!("Judge.decide")),
        ("wrong cohort", "/rows/0/cohort", json!("all_arrivals")),
        (
            "time sample uses case count",
            "/rows/0/service_time/samples",
            json!(32),
        ),
        (
            "service includes wait",
            "/rows/0/service_time/mean",
            json!("3"),
        ),
        (
            "null leaks number",
            "/rows/0/service_time/reason",
            json!("not_configured"),
        ),
    ] {
        let mut v = good.clone();
        *v.pointer_mut(path).unwrap() = bad;
        out.push((label, v));
    }
    let mut unknown = good.clone();
    unknown["rows"][0]["accepted_by_party"] = json!(true);
    out.push(("unknown field", unknown));
    let mut duplicate = good.clone();
    duplicate["rows"]
        .as_array_mut()
        .unwrap()
        .push(good["rows"][0].clone());
    out.push(("duplicate identity", duplicate));
    out
}
