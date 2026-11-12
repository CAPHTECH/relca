//! Independent JSON decoder and finite hand arithmetic. No relca-core imports.
#![allow(dead_code)]
use serde_json::{Value, json};
use std::collections::BTreeSet;

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
        Value::Array(rows) => {
            let a = actual
                .as_array()
                .ok_or_else(|| format!("{path}: array required"))?;
            if a.len() != rows.len() {
                return Err(format!("{path}: row count {} != {}", a.len(), rows.len()));
            }
            for (i, w) in rows.iter().enumerate() {
                subset(w, &a[i], &format!("{path}/{i}"))?;
            }
        }
        _ if want != actual => return Err(format!("{path}: expected {want}, got {actual}")),
        _ => (),
    }
    Ok(())
}
pub fn exit(v: &Value) -> i32 {
    match v["verdict"].as_str().expect("verdict") {
        "satisfied" => 0,
        "violated" => 1,
        "inconclusive" => 2,
        "invalid" => 3,
        "unsupported" => 4,
        "vacuous" => 5,
        _ => panic!("unknown verdict"),
    }
}
pub fn diagnostic(v: &Value, code: &str, reason: &str) -> Result<(), String> {
    let want = if code.starts_with("E-") {
        "invalid"
    } else {
        "unsupported"
    };
    subset(&json!({"verdict":want}), v, "")
        .map_err(|e| format!("{e}; diagnostics={}", v["diagnostics"]))?;
    if !v["diagnostics"]
        .as_array()
        .ok_or("diagnostics")?
        .iter()
        .any(|d| d["code"] == code && d["message"].as_str().is_some_and(|s| s.contains(reason)))
    {
        return Err(format!(
            "expected {code}/{reason}; diagnostics={}",
            v["diagnostics"]
        ));
    }
    if !v["terminals"].as_array().is_some_and(Vec::is_empty) {
        return Err("static refusal executed".into());
    }
    Ok(())
}
pub fn point(n: u64) -> Value {
    json!({"min":n,"max":n})
}
pub fn metric(n: impl ToString) -> Value {
    let n = n.to_string();
    json!({"min":n,"max":n,"reason":null})
}
pub fn exact(n: &str) -> Value {
    interval(n, n, "exact")
}
pub fn interval(l: &str, u: &str, basis: &str) -> Value {
    json!({"lower":l,"upper":u,"basis":basis,"reason":null})
}
pub fn by<'a>(rows: &'a Value, key: &str, value: &Value) -> Result<&'a Value, String> {
    let found: Vec<_> = rows
        .as_array()
        .ok_or("rows missing")?
        .iter()
        .filter(|r| r[key] == *value)
        .collect();
    if found.len() != 1 {
        return Err(format!("expected unique {key}={value} in {rows}"));
    }
    Ok(found[0])
}

/// (arrival, absolute finish, task wait, task work), in public CaseId order.
pub fn schedule(name: &str) -> Vec<(u64, u64, u64, u64)> {
    match name {
        "qualified" | "uncontrolled" | "forbidden" | "people" | "reverse" | "unassigned"
        | "known-none" | "label" | "unused" => vec![(0, 2, 0, 2)],
        "shift" | "shift-unassigned" => vec![(0, 5, 3, 2)],
        "burst" | "lighter" | "defer" => vec![(0, 2, 0, 2), (0, 4, 2, 2), (0, 6, 4, 2)],
        // External waiting is not Task queue waiting.
        "held" => vec![(0, 2, 0, 2), (0, 4, 0, 2), (0, 6, 0, 2)],
        "later" => vec![(0, 4, 0, 4), (1, 8, 3, 4)],
        "simultaneous" => vec![(0, 4, 0, 4), (0, 8, 4, 4)],
        "single" => vec![(0, 4, 0, 4)],
        "priority" => vec![(0, 7, 5, 2), (0, 5, 3, 2)],
        "priority-reverse" => vec![(0, 5, 3, 2), (0, 7, 5, 2)],
        "refused" => vec![(0, 0, 0, 0)],
        "admitted" => vec![(0, 2, 0, 2)],
        _ => vec![],
    }
}
pub fn probabilities(name: &str) -> Vec<(&'static str, &'static str)> {
    match name {
        "people" | "single" | "known-none" | "label" | "unused" => vec![("1/10", "1/10")],
        "reverse" | "admitted" => vec![("2/5", "2/5")],
        "unassigned" => vec![("1/10", "2/5")],
        "burst" => vec![("1/2", "1/2"), ("1/2", "1/2"), ("1/10", "1/10")],
        "lighter" => vec![("1/4", "1/4"), ("1/4", "1/4"), ("1/10", "1/10")],
        "held" => vec![("1/10", "1/10"); 3],
        "later" => vec![("1/10", "1/10"); 2],
        "simultaneous" => vec![("1/2", "1/2"), ("1/10", "1/10")],
        "refused" => vec![("0", "0")],
        _ => vec![],
    }
}
pub fn count_and_ratio(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "burst" => Some(("11/10", "11/30")),
        "lighter" => Some(("3/5", "1/5")),
        "held" => Some(("3/10", "1/10")),
        "later" => Some(("1/5", "1/10")),
        "simultaneous" => Some(("3/5", "3/10")),
        "people" | "single" | "known-none" | "label" | "unused" => Some(("1/10", "1/10")),
        "reverse" | "admitted" => Some(("2/5", "2/5")),
        "refused" => Some(("0", "0")),
        _ => None,
    }
}
pub fn quality(name: &str, q: &Value) -> Result<(), String> {
    let p = by(&q["properties"], "name", &json!("Bad"))?;
    let c = by(&q["cohort"], "name", &json!("BadPerArrival"))?;
    let n = if name == "defer" {
        3
    } else if matches!(name, "clock" | "horizon") {
        1
    } else {
        probabilities(name).len()
    };
    subset(
        &json!({"denominator":{"kind":"arrivals","count":n}}),
        c,
        "quality/cohort",
    )?;
    if matches!(name, "clock" | "horizon" | "defer") {
        let reason = match name {
            "clock" => "truncated",
            "horizon" => "undetermined",
            _ => "not_configured",
        };
        subset(
            &json!({"lower":null,"upper":null,"reason":reason}),
            &c["value"],
            "unknown ratio",
        )?;
        if name == "defer" {
            subset(
                &json!({"lower":null,"upper":null,"reason":reason}),
                &p["expected_count"],
                "deferred count",
            )?;
        } else {
            subset(
                &json!({"lower":"0","upper":"1","basis":"per_case_relaxation","reason":reason}),
                &p["expected_count"],
                "unknown count bounds",
            )?;
        }
        if name != "defer" {
            subset(
                &json!({"lower":"0","upper":"1"}),
                &c["bounds_with_undetermined"],
                "unknown bounds",
            )?;
            let kind = if name == "horizon" {
                "cut_off"
            } else {
                "unfinished"
            };
            let row = by(&q["outcomes_per_arrival"], "kind", &json!(kind))?;
            if name == "horizon" {
                subset(&exact("1"), &row["rate"], "cut_off rate")?;
            }
        }
        return Ok(());
    }
    let ps = probabilities(name);
    let cases = p["cases"].as_array().ok_or("quality cases")?;
    if cases.len() != ps.len() {
        return Err("quality case inventory".into());
    }
    for (i, (lo, hi)) in ps.iter().enumerate() {
        subset(
            &json!({"case_id":i+1,"min":lo,"max":hi,"reason":null}),
            &cases[i],
            "quality/case",
        )?;
    }
    if name == "unassigned" {
        for v in [&p["expected_count"], &c["value"]] {
            subset(
                &interval("1/10", "2/5", "per_case_relaxation"),
                v,
                "relaxation",
            )?;
        }
    } else if let Some((count, ratio)) = count_and_ratio(name) {
        subset(&exact(count), &p["expected_count"], "expected bad count")?;
        subset(&exact(ratio), &c["value"], "bad per offered arrival")?;
    }
    Ok(())
}

pub fn execution(name: &str, cases: &Value, metrics: &Value, q: &Value) -> Result<(), String> {
    if name == "empty" {
        let t = by(&metrics["cases"], "case_id", &json!(1))?;
        let task = by(&t["tasks"], "binding", &json!("r"))?;
        for k in ["starts", "work"] {
            subset(&metric(0), &task[k], "empty candidates")?;
        }
        if cases.as_array().ok_or("cases")?.iter().any(|c| {
            c["terminals"]
                .as_array()
                .is_some_and(|ts| ts.iter().any(|t| t["kind"] == "end"))
        }) {
            return Err("empty qualification invented a normal End".into());
        }
        return Ok(());
    }
    if matches!(name, "clock" | "horizon") {
        quality(name, q)?;
        if name == "horizon" {
            subset(
                &json!([{"case_id":1,"terminals":[{"kind":"cut_off","end":null,"completed_at":null}]}]),
                cases,
                "horizon",
            )?;
        }
        return Ok(());
    }
    let sched = schedule(name);
    if cases.as_array().ok_or("case outcomes")?.len() != sched.len() {
        return Err("case inventory".into());
    }
    for (i, (arrival, finish, wait, work)) in sched.iter().enumerate() {
        let c = by(cases, "case_id", &json!(i + 1))?;
        subset(
            &json!({"arrival":arrival,"terminals":[{"kind":"end","end":"done","completed_at":point(*finish),
            "elapsed":point(finish-arrival),"work":point(*work)}]}),
            c,
            "case",
        )?;
        let m = by(&metrics["cases"], "case_id", &json!(i + 1))?;
        if name == "defer" {
            continue;
        }
        let task = by(
            &m["tasks"],
            "binding",
            &json!(if name == "known-none" {
                "@response:0:1"
            } else {
                "r"
            }),
        )?;
        if name == "refused" {
            subset(
                &json!({"min":null,"max":null,"reason":"refused"}),
                &task["wait"],
                "refused has no Start wait",
            )?;
            subset(
                &metric(0),
                &task["refusal_wait"],
                "refusal observed at ready",
            )?;
            subset(&metric(1), &task["refused_count"], "one refusal")?;
        } else {
            subset(&metric(*wait), &task["wait"], "task/wait")?;
        }
        for (k, w) in [("work", *work), ("starts", u64::from(name != "refused"))] {
            subset(&metric(w), &task[k], &format!("task/{k}"))?;
        }
        if matches!(name, "shift" | "shift-unassigned") {
            for (cause, minutes) in [
                ("calendar", 3),
                ("actor_capacity", 0),
                ("executor", 0),
                ("resource", 0),
                ("combined", 0),
                ("unattributed", 0),
            ] {
                subset(
                    &metric(minutes),
                    &task["wait_causes"][cause],
                    "qualified wait cause",
                )?;
            }
        }
        if name == "held" {
            subset(&metric((i * 2) as u64), &m["external_wait"], "Held elapsed")?;
        }
    }
    if name == "defer" {
        for k in ["work", "observed_cost_total"] {
            subset(
                &json!({"min":null,"max":null,"reason":"not_configured"}),
                &metrics["overall"][k],
                "defer",
            )?;
        }
    } else {
        let total: u64 = sched.iter().map(|r| r.3).sum();
        subset(&metric(total), &metrics["overall"]["work"], "overall work")?;
        subset(
            &json!({"min":null,"max":null,"reason":"not_configured"}),
            &metrics["overall"]["observed_cost_total"],
            "missing unit cost",
        )?;
    }
    if !probabilities(name).is_empty() || name == "defer" {
        quality(name, q)?;
    }
    if matches!(
        name,
        "qualified"
            | "uncontrolled"
            | "forbidden"
            | "shift"
            | "shift-unassigned"
            | "priority"
            | "priority-reverse"
            | "refused"
            | "admitted"
    ) {
        let total: u64 = sched.iter().map(|r| r.3).sum();
        for (person, work) in [("p", 0), ("q", total)] {
            subset(
                &metric(work),
                &by(&metrics["people"], "person", &json!(person))?["work"],
                "qualified person",
            )?;
        }
    }
    Ok(())
}

fn qualification(executors: Value, qualified: Value, permitted: Value) -> Value {
    json!({"capability":"Desk.run","actor":"Desk","executors":executors,
        "qualified_executors":qualified,"permitted":permitted})
}
fn row(executor: &str, lower: Value, upper: Value, kernel: &str) -> Value {
    json!({"executor":executor,"lower":lower,"upper":upper,"kernel":kernel})
}
pub fn table(name: &str) -> Value {
    let only_qualification = matches!(
        name,
        "qualified"
            | "uncontrolled"
            | "forbidden"
            | "shift"
            | "shift-unassigned"
            | "empty"
            | "priority"
            | "priority-reverse"
    );
    let people = matches!(
        name,
        "people"
            | "reverse"
            | "unassigned"
            | "refused"
            | "admitted"
            | "known-none"
            | "label"
            | "unused"
    );
    let ex = if people || only_qualification {
        if name == "reverse" {
            json!(["q", "p"])
        } else {
            json!(["p", "q"])
        }
    } else {
        Value::Null
    };
    let qualified = if name == "empty" {
        json!([])
    } else if only_qualification || matches!(name, "refused" | "admitted") {
        json!(["q"])
    } else if people {
        json!(["p", "q"])
    } else {
        Value::Null
    };
    let permitted = if only_qualification {
        if name == "forbidden" {
            json!(["p"])
        } else {
            json!(["q"])
        }
    } else {
        ex.clone()
    };
    let qs = if name == "unused" {
        vec![]
    } else {
        vec![qualification(ex, qualified, permitted)]
    };
    let mut profiles = vec![];
    let mut bindings = vec![];
    if !only_qualification {
        let rows = if people {
            let mut rows = vec![];
            if !matches!(name, "refused" | "admitted") {
                rows.push(row("p", Value::Null, Value::Null, "Low"));
            }
            rows.push(row("q", Value::Null, Value::Null, "Q40"));
            rows
        } else {
            vec![
                row("anonymous", json!(0), json!(1), "Low"),
                row(
                    "anonymous",
                    json!(1),
                    Value::Null,
                    if name == "lighter" { "Quarter" } else { "Half" },
                ),
            ]
        };
        profiles.push(json!({"name":"P","basis":"fixture assumption; not measured","evidence_status":"analyst_declared_not_validated",
            "load":if people {"none"} else {"queued_same_actor_at_start"},"snapshot":"start","duration_effect":"not_supported","rows":rows}));
        if name != "unused" {
            bindings.push(json!({"capability":"Desk.run","profile":"P"}));
        }
    }
    json!({"rules":"work-profiles-v1","qualifications":qs,"profiles":profiles,"bindings":bindings})
}

fn keys(v: &Value, names: &[&str]) -> Result<(), String> {
    let fields = v.as_object().ok_or("object required")?;
    if fields.len() != names.len() || names.iter().any(|k| !fields.contains_key(*k)) {
        return Err(format!("closed keys {names:?}: {v}"));
    }
    Ok(())
}
fn names(v: &Value) -> Result<Vec<&str>, String> {
    let mut seen = BTreeSet::new();
    v.as_array()
        .ok_or("name array")?
        .iter()
        .map(|x| {
            let s = x.as_str().filter(|s| !s.is_empty()).ok_or("name string")?;
            if !seen.insert(s) {
                return Err("duplicate name".into());
            }
            Ok(s)
        })
        .collect()
}
/// Strict independent decoder for the new subtree, including relational checks.
/// Null anonymous executors, undeclared qualifications and infinity are distinct
/// by their fields; load none requires BOTH bounds null, not an invented zero.
pub fn decode_table(t: &Value) -> Result<(), String> {
    keys(t, &["rules", "qualifications", "profiles", "bindings"])?;
    if t["rules"] != "work-profiles-v1" {
        return Err("rules version".into());
    }
    let mut qs = std::collections::BTreeMap::new();
    for q in t["qualifications"].as_array().ok_or("qualifications")? {
        keys(
            q,
            &[
                "capability",
                "actor",
                "executors",
                "qualified_executors",
                "permitted",
            ],
        )?;
        let cap = q["capability"].as_str().ok_or("capability")?;
        let actor = q["actor"].as_str().ok_or("actor")?;
        if !cap.starts_with(&format!("{actor}.")) || qs.insert(cap, q).is_some() {
            return Err("capability/actor or duplicate".into());
        }
        if q["executors"].is_null() {
            if !q["qualified_executors"].is_null() || !q["permitted"].is_null() {
                return Err("anonymous qualification".into());
            }
        } else {
            let es = names(&q["executors"])?;
            if !q["qualified_executors"].is_null()
                && names(&q["qualified_executors"])?
                    .iter()
                    .any(|p| !es.contains(p))
            {
                return Err("qualification outside actor".into());
            }
            // Permission is not intersected with qualification (A1).
            if !q["permitted"].is_null() {
                names(&q["permitted"])?;
            }
        }
    }
    let mut profiles = std::collections::BTreeMap::new();
    for p in t["profiles"].as_array().ok_or("profiles")? {
        keys(
            p,
            &[
                "name",
                "basis",
                "evidence_status",
                "load",
                "snapshot",
                "duration_effect",
                "rows",
            ],
        )?;
        let name = p["name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("profile name")?;
        if profiles.insert(name, p).is_some() {
            return Err("duplicate profile".into());
        }
        if p["basis"].as_str().is_none_or(|s| s.trim().is_empty())
            || p["evidence_status"] != "analyst_declared_not_validated"
            || p["snapshot"] != "start"
            || p["duration_effect"] != "not_supported"
        {
            return Err("profile declaration contract".into());
        }
        let load = p["load"].as_str().ok_or("load")?;
        if !matches!(load, "none" | "queued_same_actor_at_start") {
            return Err("unsupported load".into());
        }
        let rows = p["rows"]
            .as_array()
            .filter(|r| !r.is_empty())
            .ok_or("rows")?;
        let mut ends = std::collections::BTreeMap::new();
        for r in rows {
            keys(r, &["executor", "lower", "upper", "kernel"])?;
            let ex = r["executor"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("executor")?;
            if r["kernel"].as_str().is_none_or(|s| s.is_empty()) {
                return Err("kernel".into());
            }
            if load == "none" {
                if !r["lower"].is_null() || !r["upper"].is_null() || ends.insert(ex, None).is_some()
                {
                    return Err("load none row".into());
                }
            } else {
                let lo = r["lower"].as_u64().ok_or("lower")?;
                let hi = if r["upper"].is_null() {
                    None
                } else {
                    Some(r["upper"].as_u64().ok_or("upper")?)
                };
                if ends.get(ex).copied().unwrap_or(Some(0)) != Some(lo)
                    || hi.is_some_and(|h| h <= lo)
                {
                    return Err("partition gap/overlap/empty/tail".into());
                }
                ends.insert(ex, hi);
            }
        }
        if ends.values().any(Option::is_some) {
            return Err("finite final tail".into());
        }
    }
    let mut bound = BTreeSet::new();
    for b in t["bindings"].as_array().ok_or("bindings")? {
        keys(b, &["capability", "profile"])?;
        let cap = b["capability"].as_str().ok_or("binding capability")?;
        if !bound.insert(cap) {
            return Err("duplicate binding".into());
        }
        let q = qs.get(cap).ok_or("binding qualification missing")?;
        let p = profiles
            .get(b["profile"].as_str().ok_or("binding profile")?)
            .ok_or("unknown profile")?;
        let wanted = if q["executors"].is_null() {
            vec!["anonymous"]
        } else if q["qualified_executors"].is_null() {
            names(&q["executors"])?
        } else {
            names(&q["qualified_executors"])?
        };
        if wanted.is_empty() {
            return Err("empty performance qualification".into());
        }
        let actual: BTreeSet<_> = p["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["executor"].as_str().unwrap())
            .collect();
        if actual != wanted.into_iter().collect() {
            return Err("performance coverage".into());
        }
    }
    Ok(())
}
pub fn wire_faults() -> Vec<(&'static str, &'static str, Value)> {
    let mut faults = vec![];
    for (label, path, value) in [
        ("version", "/rules", json!("work-profiles-v2")),
        (
            "unknown qualification",
            "/qualifications/0/qualified_executors",
            json!(["outside"]),
        ),
        (
            "duplicate qualification",
            "/qualifications/0/qualified_executors",
            json!(["p", "p"]),
        ),
        (
            "empty coverage",
            "/qualifications/0/qualified_executors",
            json!([]),
        ),
        (
            "missing person row",
            "/profiles/0/rows",
            json!([row("p", Value::Null, Value::Null, "Low")]),
        ),
        (
            "evidence promoted",
            "/profiles/0/evidence_status",
            json!("validated"),
        ),
        ("empty basis", "/profiles/0/basis", json!("")),
        ("finish snapshot", "/profiles/0/snapshot", json!("finish")),
        (
            "duration inferred",
            "/profiles/0/duration_effect",
            json!("skill_based"),
        ),
        ("missing profile", "/bindings/0/profile", json!("Missing")),
    ] {
        let mut v = table("people");
        *v.pointer_mut(path).unwrap() = value;
        faults.push(("people", label, v));
    }
    for (label, path, value) in [
        ("band gap", "/profiles/0/rows/1/lower", json!(2)),
        ("band overlap", "/profiles/0/rows/1/lower", json!(0)),
        ("empty band", "/profiles/0/rows/0/upper", json!(0)),
        ("finite final band", "/profiles/0/rows/1/upper", json!(20)),
        ("fractional band", "/profiles/0/rows/0/lower", json!(0.5)),
    ] {
        let mut v = table("burst");
        *v.pointer_mut(path).unwrap() = value;
        faults.push(("burst", label, v));
    }
    let mut v = table("people");
    v["profiles"][0]["skill_score"] = json!(100);
    faults.push(("people", "unknown key", v));
    faults
}
pub fn ledger(v: &Value) -> Result<(), String> {
    let rows = v
        .pointer("/ledger/derivation/components")
        .and_then(Value::as_array)
        .ok_or("ledger components")?;
    let matching: Vec<_> = rows
        .iter()
        .filter(|r| r["at"] == "/work_profiles/rules")
        .collect();
    if matching.len() != 1
        || matching[0]
            != &json!({"name":"/work_profiles/rules","at":"/work_profiles/rules","version":"work-profiles-v1"})
    {
        return Err("work profiles local rule link".into());
    }
    Ok(())
}
pub fn uncertainty(name: &str, v: &Value) -> Result<(), String> {
    let u = &v["uncertainty"];
    keys(u, &["rules", "aleatory", "estimation", "model"])?;
    if u["rules"] != "uncertainty-p2-1" {
        return Err("uncertainty rules".into());
    }
    for axis in ["aleatory", "estimation", "model"] {
        keys(&u[axis], &["status", "reason", "items"])?;
    }
    let table = table(name);
    let mut kernels: BTreeSet<&str> = table["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["rows"].as_array().unwrap())
        .map(|r| r["kernel"].as_str().unwrap())
        .collect();
    if name == "unused" {
        kernels = BTreeSet::from(["Low"]);
    }
    if table["profiles"].as_array().unwrap().is_empty() {
        kernels.clear();
    }
    let actual: Vec<_> = u["aleatory"]["items"]
        .as_array()
        .ok_or("aleatory items")?
        .iter()
        .filter(|r| r["kind"] == "kernel")
        .collect();
    if actual.len() != kernels.len() {
        return Err("aleatory must include each effective performance kernel exactly once".into());
    }
    for kernel in kernels {
        let found: Vec<_> = actual
            .iter()
            .filter(|r| r["name"] == kernel && r["capability"] == "Desk.run")
            .collect();
        if found.len() != 1 {
            return Err(format!("aleatory missing {kernel}"));
        }
        let bad = match kernel {
            "Low" => "1/10",
            "Q40" => "2/5",
            "Half" => "1/2",
            "Quarter" => "1/4",
            _ => return Err("unknown kernel".into()),
        };
        let good = match kernel {
            "Low" => "9/10",
            "Q40" => "3/5",
            "Half" => "1/2",
            "Quarter" => "3/4",
            _ => unreachable!(),
        };
        subset(
            &json!({"kind":"kernel","capability":"Desk.run","name":kernel,"given":[],"basis":null,
            "rows":[{"conditions":[],"branches":[
                {"probability":good,"result":"ok","writes":[{"variable":"bad","value":"false"}]},
                {"probability":bad,"result":"bad","writes":[{"variable":"bad","value":"true"}]}]}]}),
            found[0],
            "effective kernel",
        )?;
    }
    let refs = u["model"]["items"]
        .as_array()
        .ok_or("model uncertainty items")?;
    if refs
        .iter()
        .filter(|r| **r == json!({"kind":"model_context","at":"/work_profiles"}))
        .count()
        != 1
    {
        return Err("work profile model assumption reference".into());
    }
    Ok(())
}
pub fn validate(name: &str, v: &Value) -> Result<(), String> {
    let verdict = if matches!(name, "forbidden" | "empty") {
        "violated"
    } else if matches!(name, "clock" | "horizon") {
        "inconclusive"
    } else {
        "satisfied"
    };
    subset(
        &json!({"verdict":verdict,"bounds":{"truncated":name=="clock"}}),
        v,
        name,
    )?;
    execution(name, &v["case_outcomes"], &v["metrics"], &v["quality"])?;
    if name == "known-none" {
        subset(
            &json!({"verdict":"satisfied"}),
            by(&v["properties"], "id", &json!("receipt_resolution"))?,
            "complete exhaustive receipt",
        )?;
    }
    if name == "forbidden" {
        let p = v["properties"]
            .as_array()
            .ok_or("properties")?
            .iter()
            .find(|p| {
                p["id"]
                    .as_str()
                    .is_some_and(|s| s.ends_with(" #1 permitted"))
            })
            .ok_or("permitted property")?;
        if p["verdict"] != "violated" {
            return Err("permission violation hidden by eligibility".into());
        }
    }
    if name == "empty"
        && !v["diagnostics"]
            .as_array()
            .ok_or("diagnostics")?
            .iter()
            .any(|d| d["code"] == "W-DEADLOCK-0")
    {
        return Err("permanent qualification absence not deadlock".into());
    }
    let t = v.get("work_profiles").ok_or("work_profiles missing")?;
    decode_table(t)?;
    if t != &table(name) {
        return Err(format!("work_profiles: expected {}, got {t}", table(name)));
    }
    ledger(v)?;
    uncertainty(name, v)?;
    Ok(())
}

/// Bit r-1 is the bad-result observation in replication r, seed1, kernel.Flow.r.
/// Fixed from independent SHA256 integer CDF arithmetic before running Relca.
pub fn bank(name: &str) -> &'static [u16] {
    match name {
        "people" | "label" | "single" => &[0x0821],
        "known-none" => &[0x4200],
        "reverse" | "admitted" => &[0x2d21],
        "burst" => &[0x2d31, 0xd38e, 0x0200],
        "lighter" => &[0x0821, 0x408c, 0x0200],
        "held" => &[0x0821, 0x400c, 0x0200],
        "later" => &[0x0821, 0x400c],
        "simultaneous" => &[0x2d31, 0x400c],
        "refused" => &[0],
        _ => panic!("no sample bank {name}"),
    }
}
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
pub fn sample(mean: &str, min: u64, max: u64) -> Value {
    json!({"mean":mean,"min":min.to_string(),"max":max.to_string(),"samples":16,"reason":null})
}
pub fn sample_quality(name: &str, q: &Value) -> Result<(), String> {
    let masks = bank(name);
    let p = by(&q["properties"], "name", &json!("Bad"))?;
    let c = by(&q["cohort"], "name", &json!("BadPerArrival"))?;
    if p["cases"].as_array().ok_or("sample cases")?.len() != masks.len() {
        return Err("sample case inventory".into());
    }
    for (i, mask) in masks.iter().enumerate() {
        subset(
            &json!({"case_id":i+1,"hits":mask.count_ones(),"unknown":0,"trials":16}),
            &p["cases"][i],
            "per-case sample",
        )?;
    }
    let per_rep: Vec<u64> = (0..16)
        .map(|i| masks.iter().map(|m| u64::from((m >> i) & 1)).sum())
        .collect();
    let total: u64 = per_rep.iter().sum();
    let counts = sample(
        &fraction(total, 16),
        *per_rep.iter().min().unwrap(),
        *per_rep.iter().max().unwrap(),
    );
    subset(
        &counts,
        &p["expected_count"],
        "expected count sample, unit=replication",
    )?;
    subset(&sample("0", 0, 0), &p["unknown_count"], "unknown sample")?;
    subset(
        &json!({"point":fraction(total,16*masks.len() as u64),"samples":16,"reason":null,
        "numerator":counts,"denominator":sample(&masks.len().to_string(),masks.len() as u64,masks.len() as u64)}),
        &c["value"],
        "sample ratio",
    )?;
    Ok(())
}
pub fn validate_sim(name: &str, v: &Value) -> Result<(), String> {
    if name == "unassigned" {
        return diagnostic(v, "U-SEM-0", "unresolved");
    }
    subset(
        &json!({"mode":"simulation","verdict":if name == "known-none" {"inconclusive"} else {"satisfied"},"simulation":{"status":"completed","completed_replications":16}}),
        v,
        "simulation",
    )?;
    sample_quality(name, &v["quality"])?;
    if name == "known-none" {
        subset(
            &json!({"verdict":"inconclusive"}),
            by(&v["properties"], "id", &json!("receipt_resolution"))?,
            "sampled receipt is not a proof",
        )?;
        by(
            &v["metrics"]["cases"][0]["tasks"],
            "binding",
            &json!("@response:0:1"),
        )?;
    }
    let t = v.get("work_profiles").ok_or("work_profiles missing")?;
    decode_table(t)?;
    if t != &table(name) {
        return Err("sample declaration table changed".into());
    }
    ledger(v)?;
    uncertainty(name, v)?;
    let work: u64 = schedule(name).iter().map(|r| r.3).sum();
    subset(
        &sample(&work.to_string(), work, work),
        &v["metrics"]["overall"]["work"],
        "sample work",
    )?;
    Ok(())
}

pub fn quantiles(minutes: &str) -> Value {
    json!([
        {"name":"DoneMedian","p":"1/2","property":"Done","form":"all_arrivals",
            "value":{"lower":minutes,"upper":minutes,"basis":"exact","reason":null},"lower_bound":null},
        {"name":"BadGiven","p":"1","property":"Bad","form":"reached_only",
            "value":{"lower":minutes,"upper":minutes,"basis":"exact","reason":null},"lower_bound":null}
    ])
}
