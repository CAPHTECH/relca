//! ADR 0014 public acceptance. Oracles are fractions derived before implementation.
use relca_core::{diff, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::{collections::BTreeMap, fs, path::Path};

const SUM: &str = include_str!("../../../fixtures/duration-sum/model.relca");
const CLOCK: &str = include_str!("../../../fixtures/duration-clock/model.relca");
const CANCEL: &str = include_str!("../../../fixtures/duration-cancel/model.relca");
const PARALLEL: &str = include_str!("../../../fixtures/duration-parallel/model.relca");
const ZERO: &str = include_str!("../../../fixtures/duration-zero/model.relca");
const AFFECTS: &str = "duration is drawn from the declared distribution, independently of outcomes and of other durations; the distribution is an analyst assumption";

fn check(source: &str) -> Value {
    serde_json::to_value(report::check(source)).unwrap()
}
fn probability(v: &Value, n: usize, min: &str, max: &str, verdict: &str) {
    let id = format!("probability reached #{n}");
    let p = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id);
    assert_eq!(
        p,
        Some(&json!({"id":id,"verdict":verdict,"probability":{"min":min,"max":max}})),
        "{v}"
    );
}
fn query(source: &str, queries: &str) -> String {
    let prefix = source.split("  verify").next().unwrap();
    format!("{prefix}  {queries}\n}}\n")
}
fn failure(source: &str, code: &str) {
    let v = check(source);
    assert_eq!(
        v["verdict"],
        if code.starts_with('E') {
            "invalid"
        } else {
            "unsupported"
        },
        "{source}\n{v}"
    );
    let errors: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert!(!errors.is_empty(), "{v}");
    for d in errors {
        assert_eq!(d["code"], code, "single fault: {source}\n{v}");
        let start = d["span"]["start"].as_u64().unwrap() as usize;
        let end = d["span"]["end"].as_u64().unwrap() as usize;
        assert!(
            start <= end
                && end <= source.len()
                && source.is_char_boundary(start)
                && source.is_char_boundary(end)
        );
    }
    assert_eq!(v["properties"], json!([]));
    assert_eq!(v["terminals"], json!([]));
    assert!(v["agency"].is_null());
    for key in ["cases", "live_tasks", "clock", "truncated"] {
        assert!(v["bounds"][key].is_null(), "{v}");
    }
}
fn rows(v: &Value, key: &str, id: &str) -> BTreeMap<String, Value> {
    let items = v[key].as_array().unwrap();
    let rows: BTreeMap<_, _> = items
        .iter()
        .map(|r| (r[id].as_str().unwrap().to_owned(), r.clone()))
        .collect();
    assert_eq!(rows.len(), items.len());
    rows
}
fn hand(v: &Value, h: &Value, name: &str) {
    let mut keys: Vec<_> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort();
    let cut = h["bounds"]["truncated"] == true;
    let mut expected_keys = vec![
        "actors",
        "agency",
        "assumptions",
        "bounds",
        "cut",
        "cut_context",
        "diagnostics",
        "ledger",
        "mode",
        "model",
        "properties",
        "terminals",
        "uncertainty",
        "unit",
        "verdict",
    ];
    if cut {
        expected_keys.push("unchecked");
        expected_keys.sort();
    }
    assert_eq!(keys, expected_keys);
    assert_eq!(
        v.get("unchecked"),
        cut.then(|| json!(["unreachable"])).as_ref()
    );
    for key in ["verdict", "mode", "unit", "cut", "bounds"] {
        assert_eq!(v[key], h[key], "{key}: {v}");
    }
    for (key, id) in [("properties", "id"), ("terminals", "end")] {
        assert_eq!(rows(v, key, id), rows(h, key, id), "{key}: {v}");
    }
    if h["verdict"] == "invalid" {
        assert!(v["agency"].is_null());
    }
    let actual: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (&d["code"], &d["severity"]))
        .collect();
    for d in h["diagnostics"].as_array().unwrap() {
        assert!(actual.contains(&(&d["code"], &d["severity"])), "{v}");
    }
    if h["verdict"] != "invalid" {
        // Fast.win always supplies pass, so join any cannot take if_none.
        // The lexical rejected suffix after end done is also unreachable.
        let cancel = matches!(
            name,
            "duration-cancel" | "duration-cancel.broken-ignore-cancel"
        );
        assert_eq!(
            actual.len(),
            if cancel { 2 } else { 1 },
            "exact runtime inventory: {v}"
        );
        if cancel {
            assert_eq!(v["diagnostics"][1]["code"], "W-UNREACH-2");
            assert_eq!(v["diagnostics"][1]["severity"], "warning");
            assert_eq!(
                v["diagnostics"][1]["message"],
                "process Flow: end.rejected is never reached"
            );
        }
    }
    for code in h["absent_diagnostics"].as_array().unwrap() {
        assert!(!actual.iter().any(|(c, _)| *c == code), "{v}");
    }
}
fn fixture(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    let source = fs::read_to_string(dir.join("model.relca")).unwrap();
    let h: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
    let v = check(&source);
    hand(&v, &h, name);
    if h["verdict"] != "invalid" {
        let (caps, value): (&[&str], &str) = match name.split('.').next().unwrap() {
            "duration-serial" => (&["A.first", "A.second"], "dist(3min=1/2, 5min=1/2)"),
            "duration-parallel" | "duration-contention" => {
                (&["A.left", "A.right"], "dist(3min=1/2, 5min=1/2)")
            }
            "duration-cancel" => (&["Slow.work"], "dist(2min=1/2, 4min=1/2)"),
            "duration-sum" => (&["A.sample"], "dist(3min=1/2, 5min=1/2)"),
            "duration-duplicate" => (&["A.sample"], "dist(3min=1/4, 5min=3/4)"),
            "duration-zero" if name.contains("broken") => {
                (&["A.sample"], "dist(1min=0, 4min=1, 100min=0)")
            }
            "duration-zero" => (&["A.sample"], "dist(1min=0, 3min=1, 100min=0)"),
            "duration-clock" => (&["A.sample"], "dist(1min=1/2, 3min=1/2)"),
            "duration-no-lookahead" => (&["Random.sample"], "dist(3min=1/2, 5min=1/2)"),
            _ => unreachable!(),
        };
        let expected: Vec<_> = caps
            .iter()
            .map(|cap| json!({"field":format!("duration.{cap}"),"value":value,"affects":AFFECTS}))
            .collect();
        let actual: Vec<_> = v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["field"].as_str().unwrap().starts_with("duration."))
            .cloned()
            .collect();
        assert_eq!(actual, expected, "{name}: exact duration assumptions");
    }
}
macro_rules! fixtures { ($($test:ident => $name:literal),+ $(,)?) => {$(#[test] fn $test(){fixture($name);})+}; }
fixtures! {
    f1_serial => "duration-serial", f1_tight => "duration-serial.broken-tight-upper",
    f2_parallel => "duration-parallel", f2_tight => "duration-parallel.broken-tight-upper",
    f3_contention => "duration-contention", f3_capacity => "duration-contention.broken-extra-capacity",
    f4_cancel => "duration-cancel", f4_ignore => "duration-cancel.broken-ignore-cancel",
    f5_sum => "duration-sum", f5_mass => "duration-sum.broken-mass",
    f6_duplicate => "duration-duplicate", f6_value => "duration-duplicate.broken-duplicate-value",
    f7_zero => "duration-zero", f7_late => "duration-zero.broken-late-positive",
    f8_clock => "duration-clock", f8_short => "duration-clock.broken-short-clock",
    f9_no_lookahead => "duration-no-lookahead", f9_late_choice => "duration-no-lookahead.broken-late-choice",
}

#[test]
fn pairs_are_exactly_one_source_replacement_each() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for (base, fault, from, to) in [
        (
            "serial",
            "tight-upper",
            "within 6min <= 0.25",
            "within 6min <= 0.2",
        ),
        ("parallel", "tight-upper", "<= 0.25", "<= 0.2"),
        ("contention", "extra-capacity", "capacity 1", "capacity 2"),
        (
            "cancel",
            "ignore-cancel",
            "cancel cooperative ack_within 1min",
            "cancel forbidden_after_start",
        ),
        ("sum", "mass", "5min:0.5", "5min:0.4"),
        ("duplicate", "duplicate-value", "5min", "3min"),
        ("zero", "late-positive", "3min:1", "4min:1"),
        ("clock", "short-clock", "clock 2min", "clock 0min"),
        (
            "no-lookahead",
            "late-choice",
            "duration fixed 1min",
            "duration fixed 3min",
        ),
    ] {
        let s = fs::read_to_string(root.join(format!("duration-{base}/model.relca"))).unwrap();
        let b =
            fs::read_to_string(root.join(format!("duration-{base}.broken-{fault}/model.relca")))
                .unwrap();
        assert_eq!(s.matches(from).count(), 1, "{base}");
        assert_eq!(s.replacen(from, to, 1), b, "{base}");
    }
}

macro_rules! matrix { ($($name:ident: $from:literal => $to:literal, $code:literal;)+) => {$ (
    #[test] fn $name(){assert_eq!(SUM.matches($from).count(),1);failure(&SUM.replacen($from,$to,1),$code);}
)+}; }
matrix! {
    s_mass_low: "5min:0.5" => "5min:0.4", "E-PROB-0";
    s_mass_high: "5min:0.5" => "5min:0.6", "E-PROB-0";
    s_mass_float_trap: "3min:0.5,5min:0.5" => "3min:0.999999999999999999,5min:0.000000000000000002", "E-PROB-0";
    s_negative: "3min:0.5" => "3min:-0.1", "E-PROB-0";
    s_range: "3min:0.5" => "3min:1.1", "E-PROB-0";
    s_duplicate: "5min:0.5" => "03 min:0.5", "E-PROB-0";
    s_duplicate_zero: "3min:0.5,5min:0.5" => "3min:1,5min:0,5min:0", "E-PROB-0";
    s_empty: "{3min:0.5,5min:0.5}" => "{}", "E-SYN-0";
    s_no_colon: "3min:0.5" => "3min 0.5", "E-SYN-0";
    s_no_comma: "3min:0.5,5min:0.5" => "3min:0.5 5min:0.5", "E-SYN-0";
    s_trailing_comma: "5min:0.5}" => "5min:0.5,}", "E-SYN-0";
    s_no_close: "5min:0.5}" => "5min:0.5", "E-SYN-0";
    s_no_probability: "3min:0.5" => "3min:", "E-SYN-0";
    s_missing_within_time: "within 3min" => "within", "E-SYN-0";
    s_positive_zero: "3min:0.5" => "0min:0.5", "U-SEM-0";
    s_all_zero_time: "{3min:0.5,5min:0.5}" => "{0min:1}", "U-SEM-0";
    s_no_capacity: "capacity 1;" => "", "E-CAP-0";
    s_fixed_dist: "duration {3min:0.5,5min:0.5}" => "duration fixed 3min; duration {3min:0.5,5min:0.5}", "U-SEM-0";
    s_dist_fixed: "duration {3min:0.5,5min:0.5}" => "duration {3min:0.5,5min:0.5}; duration fixed 3min", "U-SEM-0";
    s_dist_dist: "duration {3min:0.5,5min:0.5}" => "duration {3min:0.5,5min:0.5}; duration {3min:0.5,5min:0.5}", "U-SEM-0";
    s_fixed_fixed: "duration {3min:0.5,5min:0.5}" => "duration fixed 3min; duration fixed 5min", "U-SEM-0";
    s_missing_duration: "duration {3min:0.5,5min:0.5};" => "", "U-SEM-0";
    s_unknown: "duration {3min:0.5,5min:0.5}" => "duration unknown", "U-SYN-0";
    s_continuous: "duration {3min:0.5,5min:0.5}" => "duration lognormal(3,5)", "U-SYN-0";
    s_time_unit: "3min:0.5" => "3sec:0.5", "U-SYN-0";
    s_zero_time_unit: "3min:0.5,5min:0.5" => "3min:1,5sec:0", "U-SYN-0";
    s_fractional_time: "3min:0.5" => "3.5min:0.5", "U-SYN-0";
    s_huge_time: "3min:0.5" => "18446744073709551616min:0.5", "U-SYN-0";
    s_named_time: "3min:0.5" => "delay:0.5", "U-SYN-0";
    s_exponent: "3min:0.5" => "3min:5e-1", "U-SYN-0";
    s_fraction: "3min:0.5" => "3min:1/2", "U-SYN-0";
    s_positive_sign: "3min:0.5" => "3min:+0.5", "U-SYN-0";
    s_within_unit: "within 3min" => "within 3sec", "U-SYN-0";
    s_within_fraction: "within 3min" => "within 3.5min", "U-SYN-0";
    s_within_huge: "within 3min" => "within 18446744073709551616min", "U-SYN-0";
    s_query_range: "<= 0.5" => "<= 1.1", "E-PROB-0";
    s_query_negative: "<= 0.5" => "<= -0.1", "E-PROB-0";
    s_query_unknown: "reached hit == true" => "reached missing == true", "E-NAME-0";
    s_query_binding: "reached hit == true" => "reached s == ok", "U-SEM-0";
    s_query_type: "reached hit == true" => "reached hit == ok", "E-TYPE-0";
    s_query_end: "reached hit == true" => "reached end.done && hit == true", "U-SYN-0";
    s_query_lt: "<= 0.5" => "< 0.5", "U-SYN-0";
    s_query_gt: "<= 0.5" => "> 0.5", "U-SYN-0";
    s_query_eq: "<= 0.5" => "== 0.5", "U-SYN-0";
    s_duplicate_query: "<= 0.5" => "<= 0.5; verify probability reached hit == true within 3min <= 0.50", "U-SEM-0";
}

#[test]
fn d_hazard_three_and_inclusive_deadline() {
    // q1=1/4, q2=(1/4)/(3/4)=1/3, q3=1. Cumulative 1/4,1/2,1.
    let s = SUM.replace("3min:0.5,5min:0.5", "1min:0.25,2min:0.25,3min:0.5");
    for (deadline, p) in [(0, "0"), (1, "1/4"), (2, "1/2"), (3, "1")] {
        let v = check(&query(
            &s,
            &format!("verify probability reached hit == true within {deadline}min >= 0"),
        ));
        probability(&v, 1, p, p, "satisfied");
        assert_eq!(
            v["terminals"][0]["probability"],
            json!({"min":"1","max":"1"})
        );
    }
}

#[test]
fn d_zero_time_and_initial_receipt() {
    let s = SUM.replace("3min:0.5,5min:0.5", "0min:0,2min:1");
    for (initial, deadline, expected) in [(false, 0, "0"), (false, 2, "1"), (true, 0, "1")] {
        let s = s.replace("hit: bool = false", &format!("hit: bool = {initial}"));
        let v = check(&query(
            &s,
            &format!("verify probability reached hit == true within {deadline}min >= 0"),
        ));
        probability(&v, 1, expected, expected, "satisfied");
        assert_eq!(v["terminals"][0]["elapsed"], json!({"min":2,"max":2}));
        assert_eq!(v["terminals"][0]["work"], json!({"min":2,"max":2}));
    }
}

#[test]
fn d_clock_deadline_cut_reason_and_safety_are_separate() {
    for (d, min, max) in [
        (0, "0", "0"),
        (1, "1/2", "1/2"),
        (2, "1/2", "1/2"),
        (3, "1/2", "1"),
    ] {
        let v = check(&query(
            CLOCK,
            &format!("verify probability reached hit == true within {d}min <= 1"),
        ));
        probability(&v, 1, min, max, "satisfied");
        assert_eq!(v["verdict"], "satisfied");
        assert_eq!(v["bounds"]["truncated"], true);
    }
    let q = "verify probability reached hit == true within 2min <= 0.5";
    let v = check(&query(CLOCK, q));
    probability(&v, 1, "1/2", "1/2", "satisfied");
    assert_eq!(v["verdict"], "satisfied");
    let v = check(&query(
        CLOCK,
        &format!("verify safety no_resource_overcommit; {q}"),
    ));
    probability(&v, 1, "1/2", "1/2", "satisfied");
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["properties"][0]["verdict"], "inconclusive");
    let v = check(&query(
        CLOCK,
        "verify probability reached hit == true <= 0.5",
    ));
    probability(&v, 1, "1/2", "1", "inconclusive");
    let v = check(&CLOCK.replace("clock 2min", "clock 3min"));
    probability(&v, 2, "1", "1", "satisfied");
    assert_eq!(v["bounds"]["truncated"], false);
    let v = check(&query(
        &CLOCK.replace("hit: bool = false", "hit: bool = true"),
        "verify probability reached hit == true within 0min >= 1",
    ));
    probability(&v, 1, "1", "1", "satisfied");
    // live_tasks cut is not a clock cut, even though D <= clock.
    let v = check(&query(&CLOCK.replace("live_tasks 2", "live_tasks 0"), q));
    probability(&v, 1, "0", "1", "inconclusive");
}

#[test]
fn d_cancel_extrema_and_ack_after_survival() {
    let v = check(&query(
        CANCEL,
        "verify probability reached changed == true within 2min <= 0",
    ));
    probability(&v, 1, "0", "1/4", "violated");
    let s = CANCEL
        .replace("4min:0.5", "5min:0.5")
        .replace("ack_within 1min", "ack_within 3min");
    let v = check(&s);
    probability(&v, 1, "0", "1/4", "satisfied");
    // ack2 and ack4 both remain possible; survival at2 must not clamp at3.
    assert_eq!(v["terminals"][0]["elapsed"], json!({"min":2,"max":4}));
    assert_eq!(v["terminals"][0]["work"], json!({"min":3,"max":5}));
    assert_eq!(v["terminals"][0]["result_at"], json!({"min":1,"max":1}));
}

#[test]
fn d_transient_within_and_fixed_only() {
    let reset = "actor Reset { capacity 1; capability clear() -> T { duration fixed 1min; result nondet { ok }; writes hit; finish { hit = false } } }; process Flow";
    let s = SUM.replace("process Flow", reset).replace(
        "s = A.sample(); end done",
        "s = A.sample(); r = Reset.clear(); end done",
    );
    for (d, p) in [(3, "1/2"), (5, "1")] {
        let v = check(&query(
            &s,
            &format!("verify probability reached hit == true within {d}min >= 0"),
        ));
        probability(&v, 1, p, p, "satisfied");
        assert_eq!(v["terminals"][0]["elapsed"], json!({"min":4,"max":6}));
        assert_eq!(v["terminals"][0]["work"], json!({"min":4,"max":6}));
    }
    let s = SUM.replace("duration {3min:0.5,5min:0.5}", "duration fixed 3min");
    for (suffix, p) in [("within 2min", "0"), ("within 3min", "1"), ("", "1")] {
        let v = check(&query(
            &s,
            &format!("verify probability reached hit == true {suffix} >= 0"),
        ));
        probability(&v, 1, p, p, "satisfied");
        assert!(
            !v["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["field"].as_str().unwrap().starts_with("duration."))
        );
    }
}

#[test]
fn d_duration_outcome_product_and_terminal_metrics() {
    let s = SUM
        .replace("3min:0.5,5min:0.5", "1min:0.5,3min:0.5")
        .replace(
            "actor A",
            "kernel K { * -> { 0.25: ok with hit = true; 0.75: ok with hit = false } }; actor A",
        )
        .replace(
            "result nondet { ok }; writes hit; finish { hit = true }",
            "outcomes kernel K; writes hit",
        )
        .replace(
            "s = A.sample(); end done",
            "s = A.sample(); if hit == true { end bad } else { end good }",
        );
    // Independent product: (1/2)(1/4)=1/8 at1, plus another1/8 at3.
    for (d, p) in [(1, "1/8"), (3, "1/4")] {
        let v = check(&query(
            &s,
            &format!("verify probability reached hit == true within {d}min >= 0"),
        ));
        probability(&v, 1, p, p, "satisfied");
        let terminals = rows(&v, "terminals", "end");
        for (end, p) in [("bad", "1/4"), ("good", "3/4")] {
            assert_eq!(terminals[end]["probability"], json!({"min":p,"max":p}));
            for metric in ["elapsed", "work"] {
                assert_eq!(terminals[end][metric], json!({"min":1,"max":3}));
            }
        }
    }
}

#[test]
fn d_contention_start_order_is_not_uniform_random() {
    let s = PARALLEL
        .replace("capacity 2", "capacity 1")
        .replacen(
            "duration {3min:0.5,5min:0.5}",
            "duration {1min:0.5,3min:0.5}",
            1,
        )
        .replacen("duration {3min:0.5,5min:0.5}", "duration fixed 2min", 1);
    let v = check(&query(
        &s,
        "verify probability reached leftDone == true within 1min >= 0; verify probability reached leftDone == true && rightDone == true within 3min >= 0",
    ));
    probability(&v, 1, "0", "1/2", "satisfied");
    probability(&v, 2, "1/2", "1/2", "satisfied");
}

#[test]
fn d_zero_extrema_and_result_at() {
    let v = check(&ZERO.replace("clock 120min", "clock 10min"));
    assert_eq!(v["bounds"]["truncated"], false);
    assert_eq!(v["terminals"][0]["elapsed"], json!({"min":3,"max":3}));
    let s = r#"model ResultAt version "0.2" {
      type T = enum { pass, ok }
      actor Matcher { capacity 1; capability sample() -> T { duration {1min:0,3min:1,100min:0}; result nondet { pass }; cancel forbidden_after_start } }
      actor Nonmatch { capacity 1; capability work() -> T { duration fixed 2min; result nondet { ok }; cancel forbidden_after_start } }
      process Flow { fork f { m = Matcher.sample(); n = Nonmatch.work() }; join any f where result == pass tie by [m,n] remaining drain if none -> rejected; end done; end rejected }
      bounds { cases 1; live_tasks 2; clock 120min }
      verify safety no_resource_overcommit
    }"#;
    let v = check(s);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(
        v["terminals"],
        json!([{"end":"done","elapsed":{"min":3,"max":3},"work":{"min":5,"max":5},"result_at":{"min":3,"max":3},"probability":{"min":"1","max":"1"}}])
    );
}

#[test]
fn d_wire_assumptions_unused_caps_order_and_duration_only() {
    let s = SUM.replace("3min:0.5,5min:0.5", "5min:0.3,3min:0.7")
        .replace("actor A", "kernel Unused { * -> { 1: ok } }; actor A")
        .replace("process Flow", "actor B { capacity 1; capability unused() -> T { duration {0min:0,2min:1}; result nondet { ok } } }; process Flow");
    let v = check(&query(&s, "verify safety no_resource_overcommit"));
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(ledger_shape::original_field_count(&v), 13);
    assert!(v["properties"][0].get("probability").is_none());
    assert_eq!(
        v["terminals"][0]["probability"],
        json!({"min":"1","max":"1"})
    );
    let assumptions = v["assumptions"].as_array().unwrap();
    let duration: Vec<_> = assumptions
        .iter()
        .filter(|a| a["field"].as_str().unwrap().starts_with("duration."))
        .cloned()
        .collect();
    assert_eq!(
        duration,
        vec![
            json!({"field":"duration.A.sample","value":"dist(3min=7/10, 5min=3/10)","affects":AFFECTS}),
            json!({"field":"duration.B.unused","value":"dist(0min=0, 2min=1)","affects":AFFECTS})
        ]
    );
    let position = |field: &str| {
        assumptions
            .iter()
            .position(|a| a["field"] == field)
            .unwrap()
    };
    assert!(position("kernel.Unused") < position("duration.A.sample"));
    if let Some(p) = assumptions
        .iter()
        .position(|a| a["field"] == "probability.scheduler")
    {
        assert!(position("duration.B.unused") < p);
        assert_eq!(p + 1, assumptions.len());
    }
    // No query/kernel: the duration declaration alone still activates probability.
    let v = check(&query(SUM, "verify safety no_resource_overcommit"));
    assert_eq!(
        v["terminals"][0]["probability"],
        json!({"min":"1","max":"1"})
    );
    assert!(
        !v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "kernel_correlations")
    );
}

#[test]
fn d_legacy_no_duration_or_within_has_no_new_keys_or_assumptions() {
    let s = include_str!("../../../fixtures/serial-time/model.relca");
    let v = check(s);
    assert_eq!(ledger_shape::original_field_count(&v), 13);
    for key in ["properties", "terminals"] {
        for row in v[key].as_array().unwrap() {
            assert!(row.get("probability").is_none());
        }
    }
    assert!(
        !v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"].as_str().unwrap().starts_with("duration."))
    );
    // Literal baseline observed at fcd526e, not regenerated from the new engine.
    assert_eq!(
        v["assumptions"],
        json!([
            {"field":"Office.boundary","value":"Office","affects":"Actor boundary is represented by its name only"},
            {"field":"Office.executors","value":"anonymous×1","affects":"executor identity, separation of duties and permission checks are unsupported"},
            {"field":"Office.control","value":"policy","affects":"classification of policy and environment choices"},
            {"field":"Office.permission","value":"default(executors)","affects":"Actor executors may execute their own Capabilities; authority is not inferred"},
            {"field":"schedule","value":"work-conserving","affects":"enabled tasks start without intentional idle; competing starts remain nondeterministic"}
        ])
    );
}

#[test]
fn d_queries_distinguish_deadline_and_absence_and_contextual_within() {
    let s = query(
        SUM,
        "verify probability reached hit == true within 3min <= 0.5; verify probability reached hit == true within 5min <= 0.5; verify probability reached hit == true <= 0.5",
    );
    let v = check(&s);
    probability(&v, 1, "1/2", "1/2", "satisfied");
    probability(&v, 2, "1", "1", "violated");
    probability(&v, 3, "1", "1", "violated");
    let s = SUM
        .replace("duration {3min:0.5,5min:0.5}", "duration fixed 3min")
        .replace("actor A", "actor within")
        .replace("A.sample()", "within.within()")
        .replace("capability sample()", "capability within()")
        .replace("enum { ok }", "enum { within }")
        .replace("nondet { ok }", "nondet { within }")
        .replace("s = within", "within = within");
    assert_eq!(check(&s)["verdict"], "violated"); // deterministic hit at3 > upper1/2
}

#[test]
fn d_unused_and_unselected_capabilities_are_statically_validated() {
    let unused = SUM.replace("s = A.sample();", "");
    for (from, to, code) in [
        ("5min:0.5", "5min:0.4", "E-PROB-0"),
        ("3min:0.5", "0min:0.5", "U-SEM-0"),
        ("capacity 1;", "", "E-CAP-0"),
    ] {
        failure(&unused.replace(from, to), code);
    }
    let s = SUM.replace("process Flow", "actor Unused { capacity 1; capability bad() -> T { duration {3min:0.5,5min:0.4}; result nondet { ok } } }; process Flow")
        .replace("  bounds", "  cut C { include [A,Unused] }; configuration Selected { cut C; process Flow }; configuration Other { cut C; process Flow }; bounds");
    let v = serde_json::to_value(report::check_configuration(&s, "Selected")).unwrap();
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-PROB-0")
    );
}

#[test]
fn d_overflow_positive_support_and_zero_support_contrast() {
    let s = SUM.replace("duration {3min:0.5,5min:0.5}", "duration {1min:0.5,18446744073709551615min:0.5}")
        .replace("process Flow", "actor First { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }; process Flow")
        .replace("s = A.sample()", "f = First.run(); s = A.sample()");
    for clock in ["1", "18446744073709551615"] {
        let v = check(&s.replace("clock 20min", &format!("clock {clock}min")));
        assert_eq!(v["verdict"], "unsupported", "{v}");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SEM-0")
        );
    }
    let s = s
        .replace(
            "1min:0.5,18446744073709551615min:0.5",
            "1min:1,18446744073709551615min:0",
        )
        .replace("clock 20min", "clock 2min");
    let v = check(&query(
        &s,
        "verify probability reached hit == true within 2min >= 1",
    ));
    probability(&v, 1, "1", "1", "satisfied");
    assert_eq!(v["bounds"]["truncated"], false);
    assert_eq!(v["terminals"][0]["elapsed"], json!({"min":2,"max":2}));
    assert_eq!(v["terminals"][0]["work"], json!({"min":2,"max":2}));
}

#[test]
fn d_configuration_and_diff_keep_probability_inside_reports() {
    let s = SUM.replace("  bounds", "  cut C { include [A] }; configuration Before { cut C; process Flow }; configuration After { cut C; process Flow }; bounds");
    let d = serde_json::to_value(diff::diff(&s, "Before", "After")).unwrap();
    assert_eq!(d["verdict"], "satisfied", "{d}");
    assert_eq!(ledger_shape::original_field_count(&d), 11);
    assert_eq!(d["differences"], json!([]));
    assert!(d["mismatch"].is_null());
    for (side, name) in [("from", "Before"), ("to", "After")] {
        let v = serde_json::to_value(report::check_configuration(&s, name)).unwrap();
        assert_eq!(d[side]["report"], v);
        probability(&v, 1, "1/2", "1/2", "satisfied");
    }
    assert!(d.get("probability").is_none());
}

#[test]
fn d_static_certainty_uses_all_positive_support_not_minimum() {
    let s = CANCEL
        .replace("duration fixed 1min", "duration fixed 3min")
        .replace("2min:0.5,4min:0.5", "1min:0.5,5min:0.5")
        .replace(
            "end done\n",
            "if slow == ok { end done } else { end other }\n",
        );
    // Slow can survive Fast3, be acknowledged without a result, then be read.
    failure(&s, "U-SEM-0");
    // All positive durations finish before the rival: a binding really exists.
    let safe = s.replace("1min:0.5,5min:0.5", "1min:0.5,2min:0.5");
    assert!(check(&safe)["bounds"]["truncated"].is_boolean());
    // Forbidden after start plus separate capacities guarantees a normal result.
    let safe = s.replace(
        "cancel cooperative ack_within 1min",
        "cancel forbidden_after_start",
    );
    assert!(check(&safe)["bounds"]["truncated"].is_boolean());
}

#[test]
fn d_serial_boundaries_parallel_marginals_and_cancel_lower_bound() {
    let serial = include_str!("../../../fixtures/duration-serial/model.relca");
    for (d, p, bound) in [
        (5, "0", "0"),
        (6, "1/4", "0.25"),
        (8, "3/4", "0.75"),
        (10, "1", "1"),
    ] {
        let v = check(&query(
            serial,
            &format!(
                "verify probability reached done == true within {d}min >= {bound}; verify probability reached done == true within {d}min <= {bound}"
            ),
        ));
        probability(&v, 1, p, p, "satisfied");
        probability(&v, 2, p, p, "satisfied");
    }
    for condition in ["leftDone == true", "rightDone == true"] {
        let v = check(&query(
            PARALLEL,
            &format!("verify probability reached {condition} within 3min >= 0.5"),
        ));
        probability(&v, 1, "1/2", "1/2", "satisfied");
    }
    let v = check(&query(
        CANCEL,
        "verify probability reached changed == true within 2min >= 0",
    ));
    probability(&v, 1, "0", "1/4", "satisfied");
}

#[test]
fn d_public_reentry_same_time_does_not_resample_survivor() {
    let s = SUM.replace("process Flow", "actor Other { capacity 1; capability work() -> T { duration fixed 3min; result nondet { ok } } }; process Flow")
        .replace("s = A.sample(); end done", "fork f { s = A.sample(); o = Other.work() }; join all f; end done");
    let v = check(&s);
    probability(&v, 1, "1/2", "1/2", "satisfied");
    assert_eq!(v["terminals"][0]["elapsed"], json!({"min":3,"max":5}));
    assert_eq!(v["terminals"][0]["work"], json!({"min":6,"max":8}));
    // This public oracle complements, but cannot replace, the cursor/clone seam.
}

#[test]
fn d_lower_matrix_latent_cut_fixed_zero_and_short_circuit() {
    let s = SUM.replace("observed case", "latent case");
    let s = s.replace(
        "result nondet { ok }; writes hit; finish { hit = true }",
        "result nondet { ok }",
    );
    // Property may read latent; business control may not, even behind a true atom.
    let bad = s.replace(
        "s = A.sample(); end done",
        "s = A.sample(); if hit == true { end done } else { end other }",
    );
    failure(&bad, "E-POL-LATENT");
    let bad = SUM.replace(
        "hit == true within",
        "hit == true || missing == true within",
    );
    failure(&bad, "E-NAME-0");
    let cut = SUM
        .replace(
            "actor A",
            "actor Hidden { latent secret: bool = false }; actor A",
        )
        .replace(
            "  bounds",
            "  cut C { include [A]; exclude Hidden reason \"outside\" loses \"truth\" }; bounds",
        )
        .replace("reached hit == true", "reached Hidden.secret == true");
    failure(&cut, "E-CUT-1");
    let old = SUM
        .replace("duration {3min:0.5,5min:0.5}", "duration fixed 0min")
        .replace("capacity 1;", "");
    failure(&old, "U-SEM-0");
    // A fixed0 unused operation remains legal; do not globally reject fixed0.
    let old = old.replace("s = A.sample();", "");
    assert_eq!(check(&old)["verdict"], "satisfied");
}

#[test]
fn d_probability_lexical_64_digits_and_within_line_breaks() {
    let sixty_four = format!("0.5{}", "0".repeat(62));
    let sixty_five = format!("0.5{}", "0".repeat(63));
    for (from, replacement) in [
        ("3min:0.5", format!("3min:{sixty_four}")),
        ("<= 0.5", format!("<= {sixty_four}")),
    ] {
        let v = check(&SUM.replace(from, &replacement));
        probability(&v, 1, "1/2", "1/2", "satisfied");
    }
    failure(
        &SUM.replace("3min:0.5", &format!("3min:{sixty_five}")),
        "U-SYN-0",
    );
    failure(
        &SUM.replace("<= 0.5", &format!("<= {sixty_five}")),
        "U-SYN-0",
    );
    for source in [
        SUM.replace("within 3min", "within\n3 min"),
        SUM.replace("{3min:0.5,5min:0.5}", "{\n3min : 0.5,\n5min : 0.5\n}"),
    ] {
        probability(&check(&source), 1, "1/2", "1/2", "satisfied");
    }
    // Optional close and comparison characters must keep their old tokenization.
    let source = SUM.replace(
        "observed case {",
        "observed case { optionalValue: optional<T>= none;",
    );
    probability(&check(&source), 1, "1/2", "1/2", "satisfied");
}
