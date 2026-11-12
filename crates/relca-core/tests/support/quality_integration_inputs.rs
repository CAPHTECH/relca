//! ADR0045 integration: sources and hand oracles, never captured quality output.
#![allow(dead_code)]
use serde_json::{Value, json};

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
pub fn append(src: &str, items: &str) -> String {
    let i = src.rfind('}').unwrap();
    format!("{}\n{items}\n{}", &src[..i], &src[i..])
}
pub fn deferred(name: &str) -> Value {
    let all: Value = serde_json::from_str(include_str!("quality_deferred.json")).unwrap();
    assert!(all[name].is_object());
    all[name].clone()
}
pub fn scenario(src: &str, horizon: u64) -> String {
    let mut body = String::new();
    let mut scope = String::new();
    for line in src.lines() {
        if line.trim_start().starts_with("bounds {")
            || line.trim_start().starts_with("arrivals fixed ")
        {
            scope.push_str(line);
            scope.push('\n');
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    if !scope.contains("arrivals fixed") {
        scope.push_str("arrivals fixed [0min]\n");
    }
    append(
        &body,
        &format!("scenario Day {{ {scope} horizon {horizon}min }}"),
    )
}
pub fn ratio(q: &Value, name: &str) -> Value {
    q["cohort"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == name)
        .unwrap()
        .clone()
}
pub fn property(q: &Value, name: &str) -> Value {
    q["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == name)
        .unwrap()
        .clone()
}
pub fn outcome(q: &Value, kind: &str) -> Value {
    q["outcomes_per_arrival"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == kind)
        .unwrap_or_else(|| panic!("missing {kind}: {q}"))
        .clone()
}
pub fn exact(n: &str) -> Value {
    json!({"lower":n,"upper":n,"basis":"exact","reason":null})
}
pub fn range(l: &str, u: &str, reason: Option<&str>) -> Value {
    json!({"lower":l,"upper":u,"basis":"per_case_relaxation","reason":reason})
}
pub fn unknown(q: &Value, name: &str, reason: &str, l: &str, u: &str) {
    let r = ratio(q, name);
    assert_eq!(
        r["value"]["reason"], reason,
        "{name}: actual cut provenance"
    );
    assert!(r["value"]["lower"].is_null());
    assert!(r["value"]["upper"].is_null());
    assert_eq!(r["bounds_with_undetermined"], json!({"lower":l,"upper":u}));
}
pub const HELD: &str = r#"model HeldQuality version "0.2" {
 type T = enum { ok }
 observed case { hit: bool = false }
 actor A { capacity 1; capability run() -> T { duration fixed 5min; result nondet { ok }; writes hit; finish { hit = true } } }
 process P { r = A.run(); end done }
 cut C { include [A] }
 policy fifo tie_by case_id
 property case.Hit = reached(hit == true)
 metric cohort.Rate = expected(count(case.Hit)) / arrivals
 scenario Day { arrivals fixed [0min,1min,2min]; concurrent cases 1 on_overflow wait_external fifo; horizon 3min; bounds { cases 3; live_tasks 1; clock 20min } }
 verify probability reached hit == true >= 1
}"#;

pub const MIXED: &str = r#"model MixedQuality version "0.2" {
 type Choice = enum { waiting, forked }
 type T = enum { ok }
 observed case { hit: bool = false }
 actor A { capacity 2;
 capability choose() -> Choice { duration fixed 1min; result nondet { waiting, forked } }
 capability run() -> T { duration fixed 5min; result nondet { ok }; writes hit; finish { hit = true } }
 capability other() -> T { duration fixed 5min; result nondet { ok } }
 }
 process P { c = A.choose(); if c == waiting { single = A.run() } else { fork jobs { first = A.run(); second = A.other() }; join all jobs }; end done }
 cut C { include [A] }
 property case.Hit = reached(hit == true)
 metric cohort.Rate = expected(count(case.Hit)) / arrivals
 scenario Day { arrivals fixed [0min]; horizon 3min; bounds { cases 1; live_tasks 1; clock 10min } }
 verify reachable end.done
}"#;

/// Record is observed only; the scalar latent is allowed for evaluation but
/// not for decisions. At t1 pass/correct=false =1/50, pass/correct=true=9/10,
/// fail=2/25. Record field pass=23/25, Wrong=1/50, Wrong/Registered=1/46.
pub const RECORD: &str = r#"model RecordQuality version "0.2" {
 type Verdict = record { confidence: enum { pass, fail } }
 observed case { entry: optional<Verdict>; registered: bool = false }
 latent case { correct: bool = false }
 kernel K { * -> { 0.9: Verdict { confidence = pass } with correct = true; 0.02: Verdict { confidence = pass } with correct = false; 0.08: Verdict { confidence = fail } with correct = false } }
 actor A { capacity 1; capability run() -> Verdict { duration fixed 1min; outcomes kernel K; writes [entry,correct] } }
 type Ack = enum { ok }
 actor Ledger { capacity 1; capability post() -> Ack { duration fixed 1min; result nondet { ok }; writes registered; finish { registered = true } } }
 process P { v = A.run(); if v.confidence == pass { a = Ledger.post(); end posted } else { end returned } }
 cut C { include [A,Ledger] }
 bounds { cases 1; live_tasks 1; clock 10min }
 verify safety no_resource_overcommit
 verify probability reached entry.confidence == pass >= 0.92
 property case.Registered = reached(registered == true)
 property case.Wrong = reached(registered == true && correct == false)
 property case.Pass = reached(entry.confidence == pass)
 metric cohort.WrongRate = expected(count(case.Wrong)) / arrivals
 metric cohort.Conditional = expected(count(case.Wrong)) / expected(count(case.Registered))
 metric cohort.PassRate = expected(count(case.Pass)) / arrivals
}"#;

pub fn strip_quality(src: &str) -> String {
    src.split_inclusive('\n')
        .map(|line| {
            if line.trim_start().starts_with("property case.")
                || line.trim_start().starts_with("metric cohort.")
            {
                line.chars()
                    .map(|c| if c == '\n' { '\n' } else { ' ' })
                    .collect::<String>()
            } else {
                line.to_owned()
            }
        })
        .collect()
}
#[path = "../../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
pub fn old_projection(v: Value) -> Value {
    old_projection_inner(ledger_shape::original_document(&v))
}
pub fn old_outcome_projection(v: Value) -> Value {
    // Internal SimulationOutcome is not a public Report and has no ledger.
    assert!(v.get("mode").is_none() && v.get("ledger").is_none());
    assert!(v["replications"].is_array());
    old_projection_inner(v)
}
fn old_projection_inner(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("quality");
        if let Some(a) = o.get_mut("assumptions").and_then(Value::as_array_mut) {
            a.retain(|a| !a["field"].as_str().unwrap().starts_with("quality."));
        }
        if let Some(m) = o.get_mut("model").and_then(Value::as_object_mut) {
            m.remove("sha256");
        }
        for x in o.values_mut() {
            *x = old_projection_inner(x.take());
        }
    } else if let Some(a) = v.as_array_mut() {
        for x in a {
            *x = old_projection_inner(x.take());
        }
    }
    v
}
