//! Hand-only intake oracles. No product output is used to construct expectations.
#![allow(dead_code)]
#[path = "quality_integration_inputs.rs"]
pub mod b6;
#[path = "quantile_acceptance.rs"]
pub mod h;
use serde_json::{Value, json};

pub const HORIZON: &[&str] = &[
    "qtl-pooling",
    "qtl-pooling.broken-horizon",
    "qtl-denominator-horizon",
    "qtl-denominator-horizon.broken-horizon",
    "qtl-censor-within",
    "qtl-censor-within.broken-no-within",
    "mutation-X-unident-g",
];
pub fn deferred(name: &str) -> Value {
    let v: Value = serde_json::from_str(include_str!("quantile_deferred.json")).unwrap();
    assert!(v[name].is_object(), "closed deferred member {name}");
    v[name].clone()
}
pub fn point(t: Option<&str>, reason: Option<&str>) -> Value {
    json!({"lower":t,"upper":t,"basis":t.map(|_| "exact"),"reason":reason})
}
pub fn rows(property: &str, all: Value, given: Value, al: Value, gl: Value) -> Value {
    json!([
        {"name":"All","p":"1/2","property":property,"form":"all_arrivals","value":all,"lower_bound":al},
        {"name":"Given","p":"1","property":property,"form":"reached_only","value":given,"lower_bound":gl}
    ])
}
pub fn deterministic_sample(mut rows: Value, n: u64, c: u64) -> Value {
    for row in rows.as_array_mut().unwrap() {
        let v = &row["value"];
        assert_eq!(v["lower"], v["upper"]);
        row["value"] =
            json!({"point":v["lower"],"observations":n,"censored":c,"reason":v["reason"]});
    }
    rows
}
pub fn add(src: &str, property: &str) -> String {
    b6::append(
        src,
        &format!(
            "metric quantile.All = quantile(0.5, case.{property})\nmetric quantile.Given = quantile(1, case.{property} given reached)"
        ),
    )
}
pub fn record() -> String {
    add(b6::RECORD, "Pass")
}
pub fn record_wrong() -> String {
    add(b6::RECORD, "Wrong")
}
pub fn record_expected(wrong: bool) -> Value {
    rows(
        if wrong { "Wrong" } else { "Pass" },
        point(
            if wrong { None } else { Some("1") },
            if wrong {
                Some("beyond_reached_mass")
            } else {
                None
            },
        ),
        point(Some(if wrong { "2" } else { "1" }), None),
        Value::Null,
        Value::Null,
    )
}
pub fn held() -> String {
    add(b6::HELD, "Hit")
}
pub fn held_expected() -> Value {
    rows(
        "Hit",
        point(None, Some("undetermined")),
        point(None, Some("undetermined")),
        json!("3"),
        Value::Null,
    )
}
pub fn held_initial() -> String {
    h::replace(&held(), "hit: bool = false", "hit: bool = true")
}
pub fn held_initial_expected() -> Value {
    rows(
        "Hit",
        point(Some("0"), None),
        point(Some("0"), None),
        Value::Null,
        Value::Null,
    )
}
pub fn conflict(checked: bool) -> String {
    let s = b6::fixture("cf-version-atomic");
    let s = if checked {
        s
    } else {
        h::replace(&s, "version_check [keys] else conflict", "")
    };
    b6::append(
        &add(
            &b6::append(&s, "property case.Registered = reached(registered == true)"),
            "Registered",
        ),
        "metric quantile.Whole = quantile(1, case.Registered)",
    )
}
pub fn conflict_expected(checked: bool) -> Value {
    let mut v = rows(
        "Registered",
        point(
            (!checked).then_some("1"),
            checked.then_some("beyond_reached_mass"),
        ),
        point(
            (!checked).then_some("1"),
            checked.then_some("denominator_may_be_zero"),
        ),
        if checked { json!("1") } else { Value::Null },
        Value::Null,
    );
    v.as_array_mut().unwrap().push(json!({"name":"Whole","p":"1","property":"Registered","form":"all_arrivals","value":point((!checked).then_some("1"),checked.then_some("beyond_reached_mass")),"lower_bound":if checked {json!("1")} else {Value::Null}}));
    v
}
pub fn overflow() -> String {
    // ADR0042 U10 forbids scenario-local overflow terminal declarations.
    let s = b6::HELD.replace(
        "concurrent cases 1 on_overflow wait_external fifo; horizon 3min;",
        "",
    );
    assert_ne!(s, b6::HELD);
    b6::append(
        &add(
            &b6::append(
                &s,
                "concurrent cases 1 on_overflow end overflow\nproperty case.Overflow = reached(end.overflow)",
            ),
            "Hit",
        ),
        "metric quantile.OverflowAll = quantile(0.5, case.Overflow)\nmetric quantile.OverflowGiven = quantile(1, case.Overflow given reached)",
    )
}
pub fn overflow_expected() -> Value {
    let mut v = rows(
        "Hit",
        point(None, Some("beyond_reached_mass")),
        point(Some("5"), None),
        Value::Null,
        Value::Null,
    );
    for (name, p, form) in [
        ("OverflowAll", "1/2", "all_arrivals"),
        ("OverflowGiven", "1", "reached_only"),
    ] {
        v.as_array_mut().unwrap().push(json!({"name":name,"p":p,"property":"Overflow","form":form,"value":point(Some("0"),None),"lower_bound":null}));
    }
    v
}
pub fn atomic(allow: bool) -> String {
    let s = include_str!("../../../../fixtures/sov-atomic/model.relca")
        .replace("result nondet { pass, fail }", "result nondet { pass }");
    let s = if allow {
        s
    } else {
        h::replace(&s, "allow: bool = true", "allow: bool = false")
    };
    add(
        &b6::append(&s, "property case.Registered = reached(registered == true)"),
        "Registered",
    )
}
pub fn atomic_expected(allow: bool) -> Value {
    rows(
        "Registered",
        point(
            allow.then_some("3"),
            (!allow).then_some("beyond_reached_mass"),
        ),
        point(allow.then_some("3"), (!allow).then_some("zero_denominator")),
        Value::Null,
        Value::Null,
    )
}
pub fn rule() -> String {
    // Deterministic pass substrate isolates Rule withdrawal from sample nondet refusal.
    let s =
        b6::fixture("rule-basic").replace("result nondet { pass, fail }", "result nondet { pass }");
    add(
        &b6::append(
            &s,
            "property case.Required = reached(ApprovalRule.approval_required == true)\nscenario Light { arrivals fixed [0min] }\nscenario Later { arrivals fixed [5min] }\nsimulation replications 1 seed 1 confidence 0.5",
        ),
        "Required",
    )
}
pub fn rule_expected(sim: bool, enabled: bool) -> Value {
    let value = |reason: &str| {
        if sim {
            json!({"point":if enabled {json!("0")}else{Value::Null},"observations":1,"censored":0,"reason":if enabled {Value::Null}else{json!(reason)}})
        } else {
            point(enabled.then_some("0"), (!enabled).then_some(reason))
        }
    };
    rows(
        "Required",
        value("beyond_reached_mass"),
        value("zero_denominator"),
        Value::Null,
        Value::Null,
    )
}
pub fn rule_legacy() -> String {
    let mut s = rule();
    for line in [
        "scenario Light { arrivals fixed [0min] }\n",
        "scenario Later { arrivals fixed [5min] }\n",
    ] {
        assert_eq!(s.matches(line).count(), 1);
        s = h::replace(&s, line, "");
    }
    s
}
pub fn assert_rule_route(src: &str, configuration: &str, scenario: Option<&str>, legacy: bool) {
    let a = relca_core::syntax::parse(src).unwrap();
    let m = relca_core::lower::lower_with_selection(&a, Some(configuration), scenario).unwrap();
    assert_eq!(m.legacy_route(), legacy, "selected Rule execution route");
}
pub fn assert_withdrawn_inventory(quality: &Value) {
    for (key, count) in [("checked", 0), ("unchecked", 1)] {
        assert_eq!(
            quality[key]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| *s == "quantiles")
                .count(),
            count,
            "withdrawn quantile inventory: {key}"
        );
    }
}
pub fn assert_rule_report(v: &Value, sim: bool, enabled: bool) {
    assert_eq!(v["quality"]["quantiles"], rule_expected(sim, enabled));
    assert!(
        v["quality"]["checked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "quantiles")
    );
    assert!(
        !v["quality"]["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "quantiles")
    );
    assert!(
        !v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "quality.rule")
    );
    h::assumptions(v);
}
pub fn metrics(src: &str) -> String {
    b6::append(src, "metrics { window 20min; currency \"JPY\" }")
}
pub const MUTATIONS: &[(&str, usize)] = &[
    ("mut-evidence", 0),
    ("mut2-expiry", 1),
    ("mut-guard", 2),
    ("mut2-summary", 4),
    ("mut-guard", 8),
    ("mut-origin", 9),
];
pub fn mutation_source(name: &str, scenario: bool) -> String {
    let mut s = b6::append(
        &b6::fixture(name),
        "observed case { qtl_ready: bool = true }\nproperty case.Initial = reached(qtl_ready == true)",
    );
    if scenario {
        let line = s
            .lines()
            .find(|l| l.trim_start().starts_with("bounds {"))
            .unwrap()
            .to_owned();
        s = h::replace(
            &s,
            &line,
            &format!("scenario Day {{ arrivals fixed [0min]; {line} }}"),
        );
    }
    add(&s, "Initial")
}
pub fn mutation_rows() -> Value {
    rows(
        "Initial",
        point(Some("0"), None),
        point(Some("0"), None),
        Value::Null,
        Value::Null,
    )
}
pub fn mutation_horizon() -> String {
    let s = b6::fixture("mut2-summary")
        .replace("result nondet { pass, fail }", "result nondet { pass }");
    let line = s
        .lines()
        .find(|l| l.trim_start().starts_with("bounds {"))
        .unwrap()
        .to_owned();
    let s = h::replace(
        &s,
        &line,
        &format!("scenario Day {{ arrivals fixed [0min]; horizon 4min; {line} }}"),
    );
    add(
        &b6::append(&s, "property case.Done = reached(end.done)"),
        "Done",
    )
}
pub fn assert_mutation_horizon(v: &Value) {
    assert_eq!(v["original"]["verdict"], "satisfied");
    assert_eq!(
        v["original"]["quality"]["quantiles"],
        rows(
            "Done",
            point(Some("4"), None),
            point(Some("4"), None),
            Value::Null,
            Value::Null
        )
    );
    let ms = v["mutations"][4]["mutants"].as_array().unwrap();
    assert_eq!(ms.len(), 1);
    assert_eq!(ms[0]["result"], "inconclusive");
    assert_eq!(ms[0]["reason"], "assigned_check_unavailable");
    assert_eq!(ms[0]["report"]["verdict"], "inconclusive");
    assert_eq!(ms[0]["report"]["bounds"]["truncated"], false);
    assert_eq!(
        ms[0]["report"]["quality"]["quantiles"],
        rows(
            "Done",
            point(None, Some("undetermined")),
            point(None, Some("undetermined")),
            json!("5"),
            Value::Null
        )
    );
    assert!(ms[0]["assigned_changes"].as_array().unwrap().is_empty());
}
pub fn completion_refusal() -> String {
    b6::append(
        &h::source("qtl-one"),
        "simulation replications 1 seed 1 confidence 0.5\nverify terminating end",
    )
}
pub fn assert_aborted_quantiles(v: &Value) {
    let rows = v["quality"]["quantiles"].as_array().unwrap();
    assert!(!rows.is_empty());
    for r in rows {
        assert_eq!(
            r["value"],
            json!({"point":null,"observations":0,"censored":0,"reason":"aborted"})
        );
        assert!(r["lower_bound"].is_null());
    }
    assert!(
        v["quality"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("quantiles"))
    );
    h::assumptions(v);
}
pub fn wrapper_old(mut v: Value) -> Value {
    v = h::without_uncertainty(v);
    if let Some(original) = v.get_mut("original").filter(|v| v.is_object()) {
        *original = h::old_projection(original.take());
    }
    for k in v["mutations"].as_array_mut().unwrap() {
        for m in k["mutants"].as_array_mut().unwrap() {
            if let Some(report) = m.get_mut("report").filter(|v| v.is_object()) {
                *report = h::old_projection(report.take());
            }
        }
    }
    v
}
pub fn no_samples_hand_quality() -> Value {
    let mut q = h::hand_quality(true);
    let sample = json!({"mean":null,"min":null,"max":null,"samples":0,"reason":"no_samples"});
    for p in q["properties"].as_array_mut().unwrap() {
        p["expected_count"] = sample.clone();
        p["unknown_count"] = sample.clone();
        for c in p["cases"].as_array_mut().unwrap() {
            c["hits"] = json!(0);
            c["unknown"] = json!(0);
            c["trials"] = json!(0);
        }
    }
    for m in q["cohort"].as_array_mut().unwrap() {
        m["value"] = json!({"point":null,"numerator":sample,"denominator":sample,"samples":0,"reason":"no_samples"});
        m["bounds_with_undetermined"] = Value::Null;
    }
    for o in q["outcomes_per_arrival"].as_array_mut().unwrap() {
        o["expected_count"] = sample.clone();
        o["rate"] = json!({"point":null,"samples":0,"reason":"no_samples"});
    }
    q["quantiles"][0]["value"] =
        json!({"point":null,"observations":0,"censored":0,"reason":"no_samples"});
    let checked = std::mem::replace(&mut q["checked"], json!([]));
    q["unchecked"]
        .as_array_mut()
        .unwrap()
        .extend(checked.as_array().unwrap().iter().cloned());
    q
}
pub fn assert_mutation(v: &Value, kind: usize) {
    assert_eq!(v["original"]["quality"]["quantiles"], mutation_rows());
    assert_eq!(
        v["mutations"][kind]["status"], "generated",
        "exercise kind {kind}"
    );
    let mutants = v["mutations"][kind]["mutants"].as_array().unwrap();
    assert!(!mutants.is_empty());
    for m in mutants {
        if m["report"].get("quality").is_some() {
            assert_eq!(m["report"]["quality"]["quantiles"], mutation_rows());
        } else {
            assert_eq!(
                m["result"], "invalid_mutant",
                "no silent missing child quality"
            );
        }
        for key in ["assigned_changes", "other_changes"] {
            assert!(!m[key].as_array().unwrap().iter().any(|r| {
                r["check"]
                    .as_str()
                    .is_some_and(|s| s.contains("quality") || s.contains("quantile"))
            }));
        }
    }
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("quality_changes"))
    );
}
pub fn run(src: &str, configuration: Option<&str>, scenario: Option<&str>, sim: bool) -> Value {
    if sim {
        json!(relca_core::report::simulate_with_selection(
            src,
            configuration,
            scenario
        ))
    } else {
        json!(relca_core::report::check_with_selection(
            src,
            configuration,
            scenario
        ))
    }
}
pub fn assert_report(v: &Value, expected: &Value, root: &str) {
    assert_eq!(v["verdict"], root, "old verdict: {v}");
    assert_eq!(v["quality"]["quantiles"], *expected, "hand first inverse");
    h::assumptions(v);
    let aborted = expected
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["value"]["reason"] == "aborted");
    assert!(
        v["quality"][if aborted { "unchecked" } else { "checked" }]
            .as_array()
            .unwrap()
            .contains(&json!("quantiles"))
    );
    if let Some(metrics) = v.get("metrics") {
        assert!(
            metrics["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("quantiles")),
            "quality never becomes a business metric"
        );
    }
}

pub fn assert_backend(
    src: &str,
    configuration: Option<&str>,
    scenario: Option<&str>,
    sim: bool,
    r: bool,
    expected: &Value,
) {
    let a = relca_core::syntax::parse(src).expect("new syntax must parse");
    let m = relca_core::lower::lower_with_selection(&a, configuration, scenario)
        .expect("new lower must accept");
    let plain = h::no_quantiles(src);
    let a = relca_core::syntax::parse(&plain).unwrap();
    let p = relca_core::lower::lower_with_selection(&a, configuration, scenario).unwrap();
    if !sim {
        let (mut v, mut old) = if r {
            let a = relca_core::reference::explore(&m);
            let b = relca_core::reference::explore(&p);
            assert_eq!(a.stop, relca_core::reference::StopReason::Complete);
            assert_eq!(b.stop, relca_core::reference::StopReason::Complete);
            (a.outcome, b.outcome)
        } else {
            (
                relca_core::engine::explore(&m),
                relca_core::engine::explore(&p),
            )
        };
        assert_eq!(
            json!(v.quality.as_ref().unwrap().quantiles),
            *expected,
            "{} vs hand",
            if r { "REFERENCE" } else { "ENGINE" }
        );
        if expected
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["value"]["reason"] == "aborted")
        {
            assert_withdrawn_inventory(&json!(v.quality.as_ref().unwrap()));
        }
        for value in [&mut v, &mut old] {
            let q = value.quality.as_mut().unwrap();
            q.quantiles = None;
            q.checked.retain(|s| s != "quantiles");
            q.unchecked.retain(|s| s != "quantiles");
        }
        assert_eq!(
            v, old,
            "complete typed old Outcome, including non-serialized fields"
        );
        return;
    }
    let (mut v, mut old) = {
        if r {
            let a = relca_core::reference::simulate(&m);
            let b = relca_core::reference::simulate(&p);
            assert_eq!(a.stop, relca_core::reference::StopReason::Complete);
            assert_eq!(b.stop, relca_core::reference::StopReason::Complete);
            assert_eq!(
                a.outcome.replications, b.outcome.replications,
                "whole selected paths and draws"
            );
            (json!(a.outcome), json!(b.outcome))
        } else {
            let a = relca_core::engine::simulate(&m);
            let b = relca_core::engine::simulate(&p);
            assert_eq!(
                a.replications, b.replications,
                "whole selected paths and draws"
            );
            (json!(a), json!(b))
        }
    };
    assert_eq!(
        v["quality"]["quantiles"],
        *expected,
        "{} vs hand",
        if r { "REFERENCE" } else { "ENGINE" }
    );
    if expected
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["value"]["reason"] == "aborted")
    {
        assert_withdrawn_inventory(&v["quality"]);
    }
    for value in [&mut v, &mut old] {
        let q = value["quality"].as_object_mut().unwrap();
        q.remove("quantiles");
        for key in ["checked", "unchecked"] {
            q[key].as_array_mut().unwrap().retain(|s| s != "quantiles");
        }
    }
    assert_eq!(v, old, "complete old Outcome fields, not just quantities");
}
