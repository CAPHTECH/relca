//! New hand inputs only; arithmetic is in sensitivity_calculations_a2a3.md.
#![allow(dead_code)]
use serde_json::{Value, json};

pub fn a2(kind: &str) -> String {
    let independent = "{ * -> { 0.1: miss; 0.9: detect } }";
    let common = "given [Cause.bad] { true -> { 1: miss }; false -> { 1: detect } }";
    let ordered = "given [a] { none -> { 0.1: miss; 0.9: detect }; miss -> { 1: miss }; detect -> { 1: detect } }";
    let (ka, kb, latent) = match kind {
        "independent" => (independent, independent, ""),
        "common" => (common, common, "kernel_reads latent [Cause.bad]"),
        "ordered" => (independent, ordered, ""),
        _ => panic!("unknown hand input"),
    };
    format!(
        r#"model ReviewA2 version "0.2" {{
  type Verdict = enum {{ miss, detect }}
  type Entry = enum {{ item }}
  latent case {{ wrong: bool ~ {{ true: 0.02, false: 0.98 }} }}
  observed case {{ a, b: optional<Verdict> }}
  actor Cause {{ instance per_case; grain document; latent bad: bool ~ {{ true: 0.1, false: 0.9 }} }}
  actor Record {{ source record }}
  actor Reader {{ capacity 1
    capability read() -> Entry {{ reads [Record]; duration fixed 1min; result nondet {{ item }} }}
  }}
  kernel KA {ka}
  kernel KB {kb}
  actor A {{ capacity 1
    capability check(e: Entry) -> Verdict {{ reads [Record]; duration fixed 1min; outcomes kernel KA; {latent}; writes a }}
  }}
  actor B {{ capacity 1
    capability check(e: Entry) -> Verdict {{ reads [Record]; duration fixed 1min; outcomes kernel KB; {latent}; writes b }}
  }}
  actor Poster {{ capacity 1
    capability post() -> Entry {{ duration fixed 1min; result nondet {{ item }}; commits }}
  }}
  process Flow {{ e = Reader.read(); av = A.check(e); bv = B.check(e);
    if av == miss && bv == miss {{ ack = Poster.post(); end done }} else {{ end done }}
  }}
  cut C {{ include [Cause, Record, Reader, A, B, Poster] }}
  bounds {{ cases 1; live_tasks 2; clock 6min }}
  verify probability reached wrong == true && a == miss && b == miss <= 0.002
  verify probability reached a == miss && b == miss <= 0.1
  verify probability reached a == miss <= 0.1
  verify probability reached b == miss <= 0.1
}}"#
    )
}

pub fn a3(busy: bool, degraded: bool) -> String {
    let (p, complement) = if degraded {
        ("0.04", "0.96")
    } else {
        ("0.01", "0.99")
    };
    let (times, cases) = if busy { ("0min, 0min", 2) } else { ("0min", 1) };
    format!(
        r#"model ReviewA3 version "0.2" {{
  type Result = enum {{ ok }}
  observed case {{ wrong: bool = false }}
  kernel K {{ * -> {{ {p}: ok with wrong = true; {complement}: ok with wrong = false }} }}
  actor Desk {{ capacity 1
    capability run() -> Result {{ duration fixed 1min; outcomes kernel K; writes wrong }}
  }}
  process Flow {{ r = Desk.run(); end done }}
  cut C {{ include [Desk] }}
  arrivals fixed [{times}]
  policy fifo tie_by case_id
  bounds {{ cases {cases}; live_tasks 2; clock 4min }}
  property case.Wrong = reached(wrong == true)
  metric cohort.WrongPerArrival = expected(count(case.Wrong)) / arrivals
  verify probability reached wrong == true <= 0.04
  simulation replications 4 seed 0 confidence 0.875
}}"#
    )
}

pub const ARRIVAL_DECL: &str = r#"arrivals distribution basis "hand assumption; not measured" {
    0.25: fixed [2min, 2min]
    0.75: fixed [2min, 5min]
  }"#;

pub fn traffic() -> String {
    format!(
        r#"model ReviewTraffic version "0.2" {{
  type Result = enum {{ ok }}
  actor Desk {{ capacity 1
    capability run() -> Result {{ duration fixed 1min; result nondet {{ ok }} }}
  }}
  process Flow {{ r = Desk.run(); end done }}
  cut C {{ include [Desk] }}
  {ARRIVAL_DECL}
  policy fifo tie_by case_id
  bounds {{ cases 2; live_tasks 2; clock 8min }}
  verify probability reached end.done <= 1
  simulation replications 4 seed 0 confidence 0.875
}}"#
    )
}

pub fn with_quality(source: &str) -> String {
    source.replace("verify probability", "property case.Done = reached(end.done)\n  metric cohort.DoneRate = expected(count(case.Done)) / arrivals\n  verify probability")
}

pub fn property<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap()
}

pub fn exact(p: &str) -> Value {
    json!({"lower":p,"upper":p,"basis":"exact","reason":null})
}

pub fn assert_a3(v: &Value, count: &str, cases: usize, rate: &str) {
    assert_eq!(v["verdict"], "satisfied", "{}", v["diagnostics"]);
    assert_eq!(v["bounds"]["truncated"], false);
    let q = &v["quality"];
    let p = q["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Wrong")
        .unwrap();
    assert_eq!(
        p["expected_count"],
        exact(count),
        "expected count, NOT per-arrival rate"
    );
    assert_eq!(p["cases"].as_array().unwrap().len(), cases);
    let c = q["cohort"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "WrongPerArrival")
        .unwrap();
    assert_eq!(c["denominator"], json!({"kind":"arrivals","count":cases}));
    assert_eq!(c["value"], exact(rate));
}
