//! Independent ADR 0058 expectations. No production ledger factory is used.
#![allow(dead_code)]
use serde_json::{Value, json};
#[path = "ledger_links.rs"]
pub mod links;

pub fn state(status: &str, value: Value, reason: Option<&str>) -> Value {
    json!({"status":status,"value":value,"reason":reason})
}
pub fn known(value: impl Into<Value>) -> Value {
    state("known", value.into(), None)
}
pub fn na(reason: &str) -> Value {
    state("not_applicable", Value::Null, Some(reason))
}
pub fn unavailable(reason: &str) -> Value {
    state("unavailable", Value::Null, Some(reason))
}
pub fn undeclared() -> Value {
    state("not_declared", Value::Null, Some("not_declared"))
}
pub fn actor(name: &str) -> Value {
    json!({"actor":name,"via_model":undeclared(),"rule":undeclared()})
}
pub fn actors(names: &[&str]) -> Value {
    json!({"status":"known","entries":names.iter().map(|n|actor(n)).collect::<Vec<_>>(),"reason":null})
}
pub fn actor_status(status: &str, reason: &str) -> Value {
    json!({"status":status,"entries":[],"reason":reason})
}
#[derive(Clone)]
pub struct Spec {
    pub mode: &'static str,
    pub builtin: &'static str,
    pub status: &'static str,
    pub selection: Value,
    pub seed: Value,
    pub replications: Value,
    pub confidence: Value,
    pub random: Value,
    pub bounds: Value,
    pub limits: Value,
    pub actors: Value,
}
pub fn selection(config: Option<&str>, scenario: Option<&str>, process: &str) -> Value {
    json!({"requested":{"configuration":config,"scenario":scenario},
        "effective":{"configuration":config.map_or_else(undeclared,known),
            "scenario":scenario.map_or_else(undeclared,known),"process":known(process)}})
}
pub fn limits() -> Value {
    let mut value = json!({});
    for key in [
        "state_budget",
        "capture_limit",
        "mutant_limit",
        "max_events",
        "max_line_bytes",
        "max_order_states",
    ] {
        value[key] = na("no_analysis");
    }
    value
}
impl Spec {
    pub fn finite(process: &str, names: &[&str]) -> Self {
        let mut cap = limits();
        cap["state_budget"] = known(1_000_000);
        Self {
            mode: "exhaustive",
            builtin: "builtin_finite",
            status: "ran",
            selection: selection(None, None, process),
            seed: na("exhaustive_analysis"),
            replications: na("exhaustive_analysis"),
            confidence: na("exhaustive_analysis"),
            random: na("exhaustive_analysis"),
            bounds: known("/bounds"),
            limits: cap,
            actors: actors(names),
        }
    }
    pub fn simulation(
        process: &str,
        names: &[&str],
        seed: u64,
        reps: u64,
        confidence: &str,
    ) -> Self {
        Self {
            mode: "simulation",
            builtin: "builtin_simulation",
            seed: known(seed),
            replications: known(reps),
            confidence: known(confidence),
            random: known("sha256-counter-v1"),
            limits: limits(),
            ..Self::finite(process, names)
        }
    }
    pub fn prelower(mode: &'static str, input: bool) -> Self {
        let reason = if input {
            "input_unavailable"
        } else {
            "not_lowered"
        };
        let mut s = Self::finite("", &[]);
        s.mode = mode;
        s.builtin = if mode == "simulation" {
            "builtin_simulation"
        } else {
            "builtin_finite"
        };
        s.status = "not_run";
        s.bounds = unavailable(reason);
        s.actors = actor_status("unavailable", "not_lowered");
        s.selection["effective"] = json!({"configuration":unavailable(reason),
            "scenario":unavailable(reason),"process":unavailable(reason)});
        s.limits["state_budget"] = unavailable("not_lowered");
        if mode == "simulation" {
            s.seed = unavailable(reason);
            s.replications = unavailable(reason);
            s.confidence = unavailable(reason);
            s.random = unavailable(reason);
            s.limits = limits();
        }
        s
    }
    pub fn aggregate(mode: &'static str) -> Self {
        let mut s = Self::finite("", &[]);
        s.mode = mode;
        s.builtin = "none";
        s.status = "not_applicable";
        s.seed = na("aggregate_per_source");
        s.replications = s.seed.clone();
        s.confidence = s.seed.clone();
        s.random = s.seed.clone();
        s.bounds = s.seed.clone();
        s.limits = limits();
        s.actors = actor_status("not_applicable", "aggregate_per_source");
        s.selection["effective"] =
            json!({"configuration":s.seed,"scenario":s.seed,"process":s.seed});
        if mode == "mutation" {
            s.limits["mutant_limit"] = known(128);
        }
        s
    }
    pub fn minimal(mode: &'static str) -> Self {
        let mut s = Self::aggregate(mode);
        s.seed = na("no_analysis");
        s.replications = s.seed.clone();
        s.confidence = s.seed.clone();
        s.random = s.seed.clone();
        s.bounds = unavailable("input_unavailable");
        s.actors = actor_status("unavailable", "not_lowered");
        s.selection["effective"] = json!({"configuration":unavailable("input_unavailable"),
            "scenario":unavailable("input_unavailable"),"process":unavailable("input_unavailable")});
        s
    }
    pub fn conform() -> Self {
        let mut s = Self::finite("Flow", &["Clerk", "Approver", "Poster"]);
        s.mode = "log_conformance";
        s.builtin = "builtin_event_contracts";
        s.seed = na("no_analysis");
        s.replications = s.seed.clone();
        s.confidence = s.seed.clone();
        s.random = s.seed.clone();
        s.bounds = known(json!({"cases":2,"live_tasks":2,"clock":20000}));
        s.limits = limits();
        for (key, n) in [
            ("max_events", 10000),
            ("max_line_bytes", 65536),
            ("max_order_states", 100000),
        ] {
            s.limits[key] = known(n);
        }
        s
    }
}
fn reference(report: &Value, at: &str) -> Value {
    if report.pointer(at).is_some() {
        known(at)
    } else {
        na("no_analysis")
    }
}
fn references(report: &Value, paths: &[&str]) -> Value {
    json!(
        paths
            .iter()
            .filter(|p| report.pointer(p).is_some())
            .map(|p| known(*p))
            .collect::<Vec<_>>()
    )
}
pub fn expected(report: &Value, s: &Spec) -> Result<Value, String> {
    Ok(json!({"schema":"relca-conclusion-ledger/v1",
        "producer":{"name":"relca","version":env!("CARGO_PKG_VERSION"),"version_source":"cargo_package",
            "build_identity":state("not_recorded",Value::Null,Some("package_version_only"))},
        "backends":{"builtin":{"name":s.builtin,"status":s.status,
            "version":if s.builtin=="none" {na("no_analysis")} else {known(env!("CARGO_PKG_VERSION"))}},
            "external":{"status":"not_connected","name":null,"version":null,"reason":"external_backend_not_connected"}},
        "derivation":{"bundle":"relca-conclusions-p2-uncertainty-2","components":links::components(report)?},
        "execution":{"mode":s.mode,"status":s.status,"selection":s.selection,
            "seed":s.seed,"replications":s.replications,"confidence":s.confidence,
            "random_method":s.random,"bounds":s.bounds,"limits":s.limits},
        "actor_versions":s.actors,
        "context":{"model":reference(report,"/model"),"mode":reference(report,"/mode"),
            "cut":reference(report,"/cut"),"log":reference(report,"/log"),
            "mutation":if report.get("original").is_some_and(|v|!v.is_null()) {known("/original/model")} else {na("no_analysis")},
            "assumptions":references(report,&["/assumptions","/explanation/assumptions"]),
            "boundaries":references(report,&["/bounds","/cut_context","/limits","/log/window"]),
            "scope":references(report,&["/unchecked","/checked","/scope","/simulation","/not_checked","/mismatch","/explanation"])},
        "subjects":links::subjects(report,s.mode)?}))
}
pub fn validate(report: &Value, spec: &Spec) -> Result<(), String> {
    let actual = report.get("ledger").ok_or("LEDGER_MISSING at /ledger")?;
    let wanted = expected(report, spec)?;
    difference(&wanted, actual, "/ledger").map_or(Ok(()), Err)
}
pub fn difference(a: &Value, b: &Value, p: &str) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        let ak: Vec<_> = a.keys().collect();
        let bk: Vec<_> = b.keys().collect();
        if ak != bk {
            return Some(format!("KEYS at {p}: expected {ak:?}, actual {bk:?}"));
        }
        for k in a.keys() {
            if let Some(e) = difference(&a[k], &b[k], &format!("{p}/{k}")) {
                return Some(e);
            }
        }
    } else if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        if a.len() != b.len() {
            return Some(format!("LENGTH at {p}: {} != {}", a.len(), b.len()));
        }
        for (n, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(e) = difference(a, b, &format!("{p}/{n}")) {
                return Some(e);
            }
        }
    }
    Some(format!("VALUE at {p}: expected {a}, actual {b}"))
}
