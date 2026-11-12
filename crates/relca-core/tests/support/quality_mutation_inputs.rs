//! ADR 0045 mutation-survivor acceptance inputs (P13, P35, P35r, O15, P34,
//! P34r). Shared by the core test (engine and the independent reference) and
//! the CLI test (real binary). Every expectation below is written by hand from
//! the model text; the derivation is in the comment of each input. The inputs
//! live here so the closed fixture population is unchanged.
#![allow(dead_code)]
use serde_json::{Value, json};

pub struct Input {
    pub name: &'static str,
    pub source: String,
    pub command: &'static str,
    pub verdict: &'static str,
    pub exit: i32,
    pub quality: Value,
}

fn range(l: &str, u: &str, basis: &str, reason: Value) -> Value {
    json!({"lower":l,"upper":u,"basis":basis,"reason":reason})
}
fn exact(v: &str) -> Value {
    range(v, v, "exact", Value::Null)
}
fn relaxed(l: &str, u: &str) -> Value {
    range(l, u, "per_case_relaxation", Value::Null)
}
fn absent(reason: &str) -> Value {
    json!({"lower":null,"upper":null,"basis":null,"reason":reason})
}
fn case(min: &str, max: &str, reason: Value) -> Value {
    json!({"case_id":1,"arrival":0,"min":min,"max":max,"reason":reason})
}
fn property(name: &str, condition: &str, cases: Value, count: Value) -> Value {
    json!({"name":name,"condition":condition,"within":null,"cases":cases,"expected_count":count})
}
fn per_arrival(name: &str, numerator: &str, count: u64, value: Value, bounds: Value) -> Value {
    json!({"name":name,"form":"ratio_of_expected_counts","numerator":numerator,
           "denominator":{"kind":"arrivals","count":count},"value":value,
           "bounds_with_undetermined":bounds})
}
fn outcome(kind: &str, end: Value, count: Value, rate: Value) -> Value {
    json!({"kind":kind,"end":end,"expected_count":count,"rate":rate})
}
/// rejected, unfinished and deadlock all exactly 0 (one case, exhaustive).
fn no_other_outcomes() -> Vec<Value> {
    ["rejected", "unfinished", "deadlock"]
        .into_iter()
        .map(|k| outcome(k, Value::Null, exact("0"), exact("0")))
        .collect()
}
/// No deadline in any input: deadline_attainment stays unchecked.
fn exhaustive(properties: Value, cohort: Value, outcomes: Vec<Value>) -> Value {
    json!({
        "rules":"quality-cohort-p1-1",
        "analysis":"exhaustive",
        "properties":properties,
        "cohort":cohort,
        "outcomes_per_arrival":outcomes,
        "checked":["case_reachability","expected_counts","ratio_of_expected_counts",
                   "denominator_disclosure","outcomes_per_arrival"],
        "unchecked":["expectation_of_ratio","cohort_any_probability","metric_intervals",
                     "metric_constraints","numerator_subset_of_denominator","business_value",
                     "rework_loss","deadline_attainment"]
    })
}

/// P13: fixtures/qual-union-nondet plus `arrivals fixed [0min]`, so the union
/// of two ends goes through the arrivals path. A.choose ends at time 1 with a
/// nondeterministic left/right, reaching end.a or end.b, never both, never
/// neither: P = reached(end.a || end.b) is 1 under every schedule, case
/// [1, 1] exact. Per end: [0, 1] per_case_relaxation (the scheduler picks).
/// A sum of per-end endpoints would give P [0, 2].
pub fn union_arrivals() -> Input {
    let source = r#"model Union version "0.2" {
  type Choice = enum { left, right }
  actor A { capacity 1; capability choose() -> Choice { duration fixed 1min; result nondet { left, right } } }
  process P { c = A.choose(); if c == left { end a } else { end b } }
  cut C { include [A] }
  arrivals fixed [0min]
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.a
  property case.P = reached(end.a || end.b)
  metric cohort.Rate = expected(count(case.P)) / arrivals
}
"#;
    let mut outcomes = vec![
        outcome("end", json!("a"), relaxed("0", "1"), relaxed("0", "1")),
        outcome("end", json!("b"), relaxed("0", "1"), relaxed("0", "1")),
    ];
    outcomes.extend(no_other_outcomes());
    Input {
        name: "union-arrivals",
        source: source.into(),
        command: "check",
        verdict: "satisfied",
        exit: 0,
        quality: exhaustive(
            json!([property(
                "P",
                "end.a || end.b",
                json!([case("1", "1", Value::Null)]),
                exact("1")
            )]),
            json!([per_arrival("Rate", "P", 1, exact("1"), Value::Null)]),
            outcomes,
        ),
    }
}

const POST_END: &str = r#"model ContestRuntime version "0.2" {
  type T = enum { pass, fail }
  people [j,p]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 1min; result nondet { fail } } }
  actor Route { executors [p]; capability respond() -> T { duration fixed 2min; result nondet { pass }; writes reviewed; finish { reviewed = true } } }
  actor Party { control party; standing over [end.returned] basis "claim" }
  permit j to Judge.judge
  permit p to Route.respond
  authority G { basis "decision"; grants j to Judge.judge }
  process Flow { d = Judge.judge(); if d == fail { end returned } else { end posted } }
  cut C { include [Judge,Route,Party] }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify safety no_resource_overcommit
  contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 2min }
  observed case { reviewed: bool = false }
  property case.Reviewed = reached(reviewed == true)
  property case.Returned = reached(end.returned)
  metric cohort.ReviewedRate = expected(count(case.Reviewed)) / arrivals
  metric cohort.ReturnedRate = expected(count(case.Returned)) / arrivals
}
"#;

fn post_end_quality(reviewed_case: Value, reviewed: Value, rate: Value, bounds: Value) -> Value {
    let mut outcomes = vec![
        outcome("end", json!("returned"), exact("1"), exact("1")),
        outcome("end", json!("posted"), exact("0"), exact("0")),
    ];
    outcomes.extend(no_other_outcomes());
    exhaustive(
        json!([
            property(
                "Reviewed",
                "reviewed == true",
                json!([reviewed_case]),
                reviewed
            ),
            property(
                "Returned",
                "end.returned",
                json!([case("1", "1", Value::Null)]),
                exact("1")
            )
        ]),
        json!([
            per_arrival("ReviewedRate", "Reviewed", 1, rate, bounds),
            per_arrival("ReturnedRate", "Returned", 1, exact("1"), Value::Null)
        ]),
        outcomes,
    )
}

/// P35: fixtures/qual-post-end-state plus `arrivals fixed [0min]`. Judge
/// fails at time 1, end.returned. The contest is filed or not
/// (nondeterministic); when filed Route.respond writes reviewed = true at time
/// 3, inside the 10min clock. The state property keeps observing after End:
/// Reviewed [0, 1] per_case_relaxation. Returned and end.returned [1, 1].
/// Closing the state observation at End would give Reviewed [0, 0].
pub fn post_end_arrivals() -> Input {
    Input {
        name: "post-end-arrivals",
        source: POST_END.replace("  bounds {", "  arrivals fixed [0min]\n  bounds {"),
        command: "check",
        verdict: "satisfied",
        exit: 0,
        quality: post_end_quality(
            case("0", "1", Value::Null),
            relaxed("0", "1"),
            relaxed("0", "1"),
            Value::Null,
        ),
    }
}

/// P35r: fixtures/qual-post-end-state with `clock 2min` (legacy route). The
/// filed branch starts Route.respond at time 1, due at 3, cut by the clock at
/// 2: reviewed is not yet known there (a computational cut, truncated). The
/// unfiled branch is closed with reviewed false. Endpoints b0 = b1 = b2 = 0,
/// b3 = 1: case {0, 1, truncated}, count {0, 1, per_case_relaxation,
/// truncated}, the rate withdrawn as truncated with bounds 0/1 and 1/1.
/// end.returned was reached at time 1 on both branches: [1, 1] exact. The
/// clock cut makes the root inconclusive (exit 2). Closing the state
/// observation at End would give Reviewed [0, 0] with no reason.
pub fn post_end_short_clock() -> Input {
    Input {
        name: "post-end-short-clock",
        source: POST_END.replace("clock 10min", "clock 2min"),
        command: "check",
        verdict: "inconclusive",
        exit: 2,
        quality: post_end_quality(
            case("0", "1", json!("truncated")),
            range("0", "1", "per_case_relaxation", json!("truncated")),
            absent("truncated"),
            json!({"lower":"0","upper":"1"}),
        ),
    }
}

/// O15: fixtures/qual-deadlock.broken-zero-capacity plus a simulation clause.
/// Checker has capacity 0: neither case ever starts check, no future event,
/// so both cases deadlock at time 0 in all 4 replications regardless of the
/// seed. No property is hit. Per replication: arrivals 2, deadlock 2, every
/// other outcome 0. WrongPerArrival = 0/8 = 0; WrongPerRegistered has a zero
/// denominator in every replication (point null, zero_denominator, samples
/// kept). A sampled deadlock is W-DEADLOCK-0 and the root is violated (as in
/// fixtures/sim-deadlock.broken-zero-capacity), exit 1.
pub fn deadlock_simulation() -> Input {
    let source = r#"model QualityBase version "0.2" {
  type V = enum { pass, fail }
  type Ack = enum { committed }
  observed case { registered: bool = false }
  latent case { correct: bool = false }
  kernel judge { * -> { 0.9: pass with correct = true; 0.02: pass with correct = false; 0.08: fail with correct = false } }
  actor Checker { capacity 0; capability check() -> V { duration fixed 3min; outcomes kernel judge; writes correct } }
  actor Ledger { capacity 2; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed }; writes registered; finish { registered = true } } }
  process Flow { v = Checker.check(); if v == pass {} else { end returned }; a = Ledger.post(); end posted }
  cut C { include [Checker, Ledger] }
  arrivals fixed [0min, 0min]
  bounds { cases 2; live_tasks 2; clock 30min }
  verify reachable end.posted
  simulation replications 4 seed 1 confidence 0.5
  property case.Wrong = reached(registered == true && correct == false)
  property case.Registered = reached(registered == true)
  metric cohort.WrongPerArrival = expected(count(case.Wrong)) / arrivals
  metric cohort.WrongPerRegistered = expected(count(case.Wrong)) / expected(count(case.Registered))
  policy fifo tie_by case_id
}
"#;
    let s = |v: &str| json!({"mean":v,"min":v,"max":v,"samples":4,"reason":null});
    let cases = json!([
        {"case_id":1,"arrival":0,"hits":0,"unknown":0,"trials":4},
        {"case_id":2,"arrival":0,"hits":0,"unknown":0,"trials":4}
    ]);
    let prop = |name: &str, condition: &str| {
        json!({"name":name,"condition":condition,"within":null,"cases":cases,
               "expected_count":s("0"),"unknown_count":s("0")})
    };
    let rate = |p: &str| json!({"point":p,"samples":4,"reason":null});
    let out = |kind: &str, end: Value, n: &str, p: &str| json!({"kind":kind,"end":end,"expected_count":s(n),"rate":rate(p)});
    let quality = json!({
        "rules":"quality-cohort-p1-1",
        "analysis":"simulation",
        "properties":[
            prop("Wrong","registered == true && correct == false"),
            prop("Registered","registered == true")
        ],
        "cohort":[
            {"name":"WrongPerArrival","form":"ratio_of_expected_counts","numerator":"Wrong",
             "denominator":{"kind":"arrivals","count":2},
             "value":{"point":"0","numerator":s("0"),"denominator":s("2"),"samples":4,"reason":null},
             "bounds_with_undetermined":null},
            {"name":"WrongPerRegistered","form":"ratio_of_expected_counts","numerator":"Wrong",
             "denominator":{"kind":"property","property":"Registered","covers":null,"reason":"state_condition"},
             "value":{"point":null,"numerator":s("0"),"denominator":s("0"),"samples":4,"reason":"zero_denominator"},
             "bounds_with_undetermined":null}
        ],
        "outcomes_per_arrival":[
            out("end", json!("returned"), "0", "0"),
            out("end", json!("posted"), "0", "0"),
            out("rejected", Value::Null, "0", "0"),
            out("unfinished", Value::Null, "0", "0"),
            out("deadlock", Value::Null, "2", "1")
        ],
        "checked":["case_reachability","expected_counts","ratio_of_expected_counts",
                   "denominator_disclosure","outcomes_per_arrival"],
        "unchecked":["expectation_of_ratio","cohort_any_probability","metric_intervals",
                     "metric_constraints","numerator_subset_of_denominator","business_value",
                     "rework_loss","deadline_attainment"]
    });
    Input {
        name: "deadlock-simulation",
        source: source.into(),
        command: "simulate",
        verdict: "violated",
        exit: 1,
        quality,
    }
}

/// P34 / P34r: a quality-only numeric cut on the public path (legacy route).
/// p = 0.(63 threes) = 33..3 / 10^63, coprime to 10. Twenty checks in a row;
/// any `no` ends the case at end.done, the twentieth `yes` writes hit = true.
/// Hit = p^20, denominator 10^1260 > 2^4096 (1260 * log2 10 ~ 4186 bits),
/// while p^19 (~3976 bits) still fits: only the quality slot exceeds 4096
/// bits. end.done is reached on every branch, so its slot is 1 at every level
/// and no probability query exists: the existing facts carry no cut. All is
/// {0, 1, truncated} and its rate withdrawn with bounds 0 and 1; Done and
/// end.done [1, 1] exact. The verdict (reachable end.done) is satisfied with
/// truncated false, exit 0.
pub fn quality_only_numeric() -> Input {
    let p = format!("0.{}", "3".repeat(63));
    let q = format!("0.{}7", "6".repeat(62));
    let steps: String = (0..19)
        .map(|i| format!("x{i} = A.c(); if x{i} == yes {{}} else {{ end done }}; "))
        .collect();
    let source = format!(
        r#"model Digits version "0.2" {{
  type V = enum {{ yes, no }}
  latent case {{ hit: bool = false }}
  kernel k {{ * -> {{ {p}: yes; {q}: no }} }}
  kernel last {{ * -> {{ {p}: yes with hit = true; {q}: no with hit = false }} }}
  actor A {{ capacity 1; capability c() -> V {{ duration fixed 1min; outcomes kernel k }}; capability l() -> V {{ duration fixed 1min; outcomes kernel last; writes hit }} }}
  process Flow {{ {steps}z = A.l(); end done }}
  cut C {{ include [A] }}
  bounds {{ cases 1; live_tasks 1; clock 30min }}
  verify reachable end.done
  property case.All = reached(hit == true)
  property case.Done = reached(end.done)
  metric cohort.AllRate = expected(count(case.All)) / arrivals
  metric cohort.DoneRate = expected(count(case.Done)) / arrivals
}}
"#
    );
    let mut outcomes = vec![outcome("end", json!("done"), exact("1"), exact("1"))];
    outcomes.extend(no_other_outcomes());
    Input {
        name: "quality-only-numeric",
        source,
        command: "check",
        verdict: "satisfied",
        exit: 0,
        quality: exhaustive(
            json!([
                property(
                    "All",
                    "hit == true",
                    json!([case("0", "1", json!("truncated"))]),
                    range("0", "1", "per_case_relaxation", json!("truncated"))
                ),
                property(
                    "Done",
                    "end.done",
                    json!([case("1", "1", Value::Null)]),
                    exact("1")
                )
            ]),
            json!([
                per_arrival(
                    "AllRate",
                    "All",
                    1,
                    absent("truncated"),
                    json!({"lower":"0","upper":"1"})
                ),
                per_arrival("DoneRate", "Done", 1, exact("1"), Value::Null)
            ]),
            outcomes,
        ),
    }
}

/// P34r: the same chain on the arrivals route (`arrivals fixed [0min]`, one
/// case at time 0). On the legacy route the reference takes only its quality
/// from a separate arrivals run, so a quality slot flag cannot reach the
/// existing outcome there; here the existing outcome is that run's. Same hand
/// values: one case arriving at 0.
pub fn quality_only_numeric_arrivals() -> Input {
    let mut input = quality_only_numeric();
    input.name = "quality-only-numeric-arrivals";
    input.source = input
        .source
        .replace("  bounds {", "  arrivals fixed [0min]\n  bounds {");
    assert!(input.source.contains("arrivals fixed"));
    input
}

pub fn all() -> Vec<Input> {
    vec![
        union_arrivals(),
        post_end_arrivals(),
        post_end_short_clock(),
        deadlock_simulation(),
        quality_only_numeric(),
        quality_only_numeric_arrivals(),
    ]
}
