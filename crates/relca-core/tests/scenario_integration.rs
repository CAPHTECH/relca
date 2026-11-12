//! Scenarios with records, limits, Standing and shared updates.
//!
//! (b) scenario-path boundary cases:
//!   G1/G2: at the horizon, Held cases (ADR 0039 wait_external) must get the
//!          cut_off row and the per-case "suffix not inspected" mark like
//!          admitted cases (ADR 0042: 「外部待ち(ADR 0039)の案件も同じ」, U1).
//!   G3:    a scenario accepts `queue`/`concurrent` in the existing item form
//!          (ADR 0042 構文 scen_item).
//!   G4:    a scenario-local `on_overflow end <名前>` is U-SEM-0 (U10).
//! and the integration items D01–D05 (limits/Standing/records), with inputs kept
//! in the test (no new fixtures, closed populations unchanged).
//!
//! Every expected value is hand arithmetic from the model text and the ADR
//! rules, written before running the integrated binary; nothing is copied from
//! output.
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn check(src: &str, scenario: &str) -> Value {
    serde_json::to_value(report::check_with_selection(src, None, Some(scenario))).unwrap()
}
fn simulate(src: &str, scenario: &str) -> Value {
    serde_json::to_value(report::simulate_with_selection(src, None, Some(scenario))).unwrap()
}

/// Engine and the independent reference agree on the whole runtime outcome.
fn both_backends(src: &str, scenario: &str) {
    let ast = syntax::parse(src).unwrap_or_else(|d| panic!("{d:?}"));
    let m =
        lower::lower_with_selection(&ast, None, Some(scenario)).unwrap_or_else(|d| panic!("{d:?}"));
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes, "case outcomes");
    assert_eq!(e.properties, r.outcome.properties, "properties");
    assert_eq!(e.metrics, r.outcome.metrics, "metrics");
    assert_eq!(e.truncated, r.outcome.truncated);
    assert_eq!(e.unchecked, r.outcome.unchecked);
}

/// Every key of `expected` must be in `actual` with the same value; arrays
/// must have the same length.
fn partial(expected: &Value, actual: &Value, path: &str) {
    match expected {
        Value::Object(o) => {
            for (k, v) in o {
                partial(
                    v,
                    actual
                        .get(k)
                        .unwrap_or_else(|| panic!("missing {path}/{k} in {actual}")),
                    &format!("{path}/{k}"),
                );
            }
        }
        Value::Array(xs) => {
            let ys = actual
                .as_array()
                .unwrap_or_else(|| panic!("{path}: not an array: {actual}"));
            assert_eq!(xs.len(), ys.len(), "{path}: length of {actual}");
            for (i, (x, y)) in xs.iter().zip(ys).enumerate() {
                partial(x, y, &format!("{path}/{i}"));
            }
        }
        _ => assert_eq!(expected, actual, "{path}"),
    }
}

fn r(a: u64, b: u64) -> Value {
    json!({"min": a, "max": b})
}
fn q(a: &str, b: &str) -> Value {
    json!({"min": a, "max": b, "reason": null})
}
fn errors(v: &Value) -> Vec<(String, String)> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| {
            (
                d["code"].as_str().unwrap().to_owned(),
                d["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// D01 (P20): three receipts 0, 1, 2; one concurrent case with external wait
/// declared in the scenario (G3); A runs 5 minutes, capacity 1.
fn d01(horizon: u64, extra: &str, verify: &str) -> String {
    format!(
        r#"model M version "0.2" {{
  type T = enum {{ ok }}
  observed case {{ hit: bool = false }}
  actor A {{ capacity 1; capability run() -> T {{
    duration fixed 5min
    result nondet {{ ok }}
    writes hit
    finish {{ hit = true }}
  }} }}
  process Flow {{ r = A.run(); end done }}
  cut C {{ include [A] }}
  policy fifo tie_by case_id
  scenario Day {{
    arrivals fixed [0min, 1min, 2min]
    concurrent cases 1 on_overflow wait_external fifo
    horizon {horizon}min
    bounds {{ cases 3; live_tasks 1; clock 20min }}
{extra}  }}
  metrics {{ window 30min; currency "JPY"; rate A per_minute 1; rate A.run per_start 0 }}
  {verify}
}}
"#
    )
}
const PROB1: &str = "verify probability reached hit == true >= 1";

fn cut_off(work: u64) -> Value {
    json!({"kind": "cut_off", "end": null, "completed_at": null, "elapsed": null,
           "work": r(work, work), "result_at": null, "probability": {"min": "1", "max": "1"}})
}

#[test]
fn d01_held_cases_at_the_horizon_are_cut_off_with_receipt_but_no_admission() {
    // Case1 runs 0..5; case2 received 1, case3 received 2, both held by the
    // concurrent limit. At D3 all three are unfinished: cut_off, work [3,0,0],
    // admission [0,null,null]. Cost 3*1, observed until 3, utilization 3/30.
    let src = d01(3, "", PROB1);
    let v = check(&src, "Day");
    assert_eq!(errors(&v), vec![]);
    let expected = json!({
        "verdict": "inconclusive",
        "bounds": {"cases": 3, "truncated": false},
        "case_outcomes": [
            {"case_id": 1, "arrival": 0, "admitted_at": r(0, 0), "terminals": [cut_off(3)]},
            {"case_id": 2, "arrival": 1, "admitted_at": null, "terminals": [cut_off(0)]},
            {"case_id": 3, "arrival": 2, "admitted_at": null, "terminals": [cut_off(0)]},
        ],
        "properties": [{"id": "probability reached #1", "verdict": "inconclusive", "cases": [
            {"case_id": 1, "verdict": "inconclusive", "probability": {"min": "0", "max": "1"}},
            {"case_id": 2, "verdict": "inconclusive", "probability": {"min": "0", "max": "1"}},
            {"case_id": 3, "verdict": "inconclusive", "probability": {"min": "0", "max": "1"}},
        ]}],
        "admission": {"rejected_count": r(0, 0), "external_wait_max": r(2, 2)},
        "metrics": {
            "status": "complete",
            "overall": {
                "counts": {
                    "received": {"stats": q("3", "3")},
                    "admitted": {"stats": q("1", "1")},
                    "completed": {"stats": q("0", "0")},
                    "cut_off": {"stats": q("3", "3")},
                    "unfinished": {"stats": q("0", "0")},
                    "rejected": {"stats": q("0", "0")},
                    "not_arrived": {"stats": q("0", "0")},
                },
                "observed_until": q("3", "3"),
                "work": q("3", "3"),
                "observed_cost_total": q("3", "3"),
            },
            "actors": [{"actor": "A", "utilization": q("1/10", "1/10")}],
        },
        "scenario": {"horizon": 3, "drain": {"from": 2, "until": 3}, "warmup": 0},
    });
    partial(&expected, &v, "$");
    both_backends(&src, "Day");
}

#[test]
fn d01_full_drain_admits_held_cases_in_receipt_order() {
    // D16 > last finish 15: admissions [0,5,10], finishes [5,10,15], elapsed
    // from receipt [5,9,13] (ADR 0039: external wait is in elapsed), external
    // wait [0,4,8], work 5 each; cost 15, utilization 15/30.
    let src = d01(16, "", PROB1);
    let v = check(&src, "Day");
    let done = |c: u64, e: u64| {
        json!({"kind": "end", "end": "done", "completed_at": r(c, c), "elapsed": r(e, e),
               "work": r(5, 5), "result_at": null, "probability": {"min": "1", "max": "1"}})
    };
    let expected = json!({
        "verdict": "satisfied",
        "bounds": {"truncated": false},
        "case_outcomes": [
            {"case_id": 1, "admitted_at": r(0, 0), "terminals": [done(5, 5)]},
            {"case_id": 2, "admitted_at": r(5, 5), "terminals": [done(10, 9)]},
            {"case_id": 3, "admitted_at": r(10, 10), "terminals": [done(15, 13)]},
        ],
        "properties": [{"verdict": "satisfied"}],
        "metrics": {
            "status": "complete",
            "overall": {
                "counts": {"completed": {"stats": q("3", "3")}, "cut_off": {"stats": q("0", "0")},
                           "unfinished": {"stats": q("0", "0")}},
                "observed_until": q("15", "15"),
                "observed_cost_total": q("15", "15"),
            },
            "actors": [{"actor": "A", "utilization": q("1/2", "1/2")}],
            "cases": [{"external_wait": q("0", "0")}, {"external_wait": q("4", "4")},
                      {"external_wait": q("8", "8")}],
        },
        "scenario": {"horizon": 16, "drain": {"from": 2, "until": 16}},
    });
    partial(&expected, &v, "$");
    both_backends(&src, "Day");
}

#[test]
fn d01_sampled_held_cutoff_is_unknown_not_a_miss() {
    // Four replications, threshold 1/2. D3: every case is cut off in every
    // replication (cut_off_count 4, unknown 4). D16: every case hits 4/4.
    let sim = "    simulation replications 4 seed 1 confidence 0.5\n";
    let verify = "verify probability reached hit == true >= 0.5";
    let v = simulate(&d01(3, sim, verify), "Day");
    let row = |k: u64| json!({"case_id": k, "hits": 0, "unknown": 4, "trials": 4});
    let co = |k: u64| {
        json!({"case_id": k, "terminals": [], "unfinished_count": 0,
                             "deadlocked_count": 0, "cut_off_count": 4})
    };
    partial(
        &json!({
            "verdict": "inconclusive",
            "properties": [{"cases": [row(1), row(2), row(3)]}],
            "case_outcomes": [co(1), co(2), co(3)],
            "simulation": {"truncated_replications": 0, "cut_off_replications": 4},
        }),
        &v,
        "$",
    );
    let v = simulate(&d01(16, sim, verify), "Day");
    let row = |k: u64| json!({"case_id": k, "verdict": "satisfied", "hits": 4, "unknown": 0});
    partial(
        &json!({
            "verdict": "satisfied",
            "properties": [{"cases": [row(1), row(2), row(3)]}],
            "simulation": {"truncated_replications": 0, "cut_off_replications": 0},
        }),
        &v,
        "$",
    );
}

#[test]
fn g2_held_cases_cut_at_the_horizon_are_inconclusive_like_the_clock_cut() {
    // reachable end.done: case1 is cut while running, cases 2 and 3 are held.
    // None has reached done by D3 and every one could after it: all three
    // rows inconclusive, root inconclusive (never violated). Same as clock 3.
    let src = d01(3, "", "verify reachable end.done");
    let v = check(&src, "Day");
    let rows: Vec<_> = v["properties"][0]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["verdict"].clone())
        .collect();
    assert_eq!(rows, vec![json!("inconclusive"); 3]);
    assert_eq!(v["verdict"], "inconclusive");
    both_backends(&src, "Day");
    // A response due at receipt+5 (5, 6, 7 > D3) is not yet decided for any
    // case: inconclusive rows, never satisfied for the held case 3.
    let src = d01(
        3,
        "",
        "contract K { response case.received -> one_of(end.done) by 5min after case.received otherwise unresolved }",
    );
    let v = check(&src, "Day");
    let rows: Vec<_> = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"].as_str().unwrap().starts_with("contract K"))
        .unwrap()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["verdict"].clone())
        .collect();
    assert_eq!(rows, vec![json!("inconclusive"); 3]);
    assert_eq!(v["verdict"], "inconclusive");
    both_backends(&src, "Day");
}

/// Post-integration boundary review m2: the held case's re-admission falls
/// exactly at D. Receipts 0 and 1; one concurrent case with external wait;
/// A runs 5 minutes; horizon 5.
fn readmit_at_d() -> String {
    r#"model M version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false }
  actor A { capacity 1; capability run() -> T {
    duration fixed 5min
    result nondet { ok }
    writes hit
    finish { hit = true }
  } }
  process Flow { r = A.run(); end done }
  cut C { include [A] }
  policy fifo tie_by case_id
  scenario Day {
    arrivals fixed [0min, 1min]
    concurrent cases 1 on_overflow wait_external fifo
    horizon 5min
    bounds { cases 2; live_tasks 1; clock 20min }
  }
  metrics { window 30min; currency "JPY"; rate A per_minute 1; rate A.run per_start 0 }
  verify probability reached hit == true >= 1
}
"#
    .to_owned()
}

#[test]
fn m2_held_case_readmitted_exactly_at_the_horizon_is_cut_off_with_admission_at_d() {
    // Events at time <= D are processed in the same-time layers (ADR 0042).
    // Case1 admitted 0, runs 0..5, end done at 5 = D: elapsed 5, work 5.
    // Case2 received 1, held; at 5 the finish frees the slot and, after the
    // immediate events are exhausted and before time advances, the held case
    // is re-admitted at 5 using the slot freed at 5 (ADR 0039 U1 and its
    // exception to the ADR 0015 rule). Nothing after D: case2 is cut off
    // with admission 5, external wait 5 - 1 = 4, work 0. Property: case1
    // reached hit (1), case2 undecided [0,1]: root inconclusive. Counts:
    // received 2, admitted 2, completed 1, cut_off 1. Work 5, cost 5*1,
    // observed until 5, utilization 5/30 = 1/6. Drain from the last
    // receipt 1 until 5.
    let src = readmit_at_d();
    let v = check(&src, "Day");
    assert_eq!(errors(&v), vec![]);
    let expected = json!({
        "verdict": "inconclusive",
        "bounds": {"cases": 2, "truncated": false},
        "case_outcomes": [
            {"case_id": 1, "arrival": 0, "admitted_at": r(0, 0), "terminals": [
                {"kind": "end", "end": "done", "completed_at": r(5, 5), "elapsed": r(5, 5),
                 "work": r(5, 5), "result_at": null, "probability": {"min": "1", "max": "1"}}]},
            {"case_id": 2, "arrival": 1, "admitted_at": r(5, 5), "terminals": [cut_off(0)]},
        ],
        "properties": [{"id": "probability reached #1", "verdict": "inconclusive", "cases": [
            {"case_id": 1, "verdict": "satisfied", "probability": {"min": "1", "max": "1"}},
            {"case_id": 2, "verdict": "inconclusive", "probability": {"min": "0", "max": "1"}},
        ]}],
        // external_wait_max is the largest number of held cases on a path
        // (ADR 0039 report: 経路ごとの最大の外部待ちの件数), not a duration:
        // only case2 is ever held, so 1. The duration 4 is metrics.cases.
        "admission": {"rejected_count": r(0, 0), "external_wait_max": r(1, 1)},
        "metrics": {
            "status": "complete",
            "overall": {
                "counts": {
                    "received": {"stats": q("2", "2")},
                    "admitted": {"stats": q("2", "2")},
                    "completed": {"stats": q("1", "1")},
                    "cut_off": {"stats": q("1", "1")},
                    "unfinished": {"stats": q("0", "0")},
                    "rejected": {"stats": q("0", "0")},
                    "not_arrived": {"stats": q("0", "0")},
                },
                "observed_until": q("5", "5"),
                "work": q("5", "5"),
                "observed_cost_total": q("5", "5"),
            },
            "actors": [{"actor": "A", "utilization": q("1/6", "1/6")}],
            "cases": [{"external_wait": q("0", "0")}, {"external_wait": q("4", "4")}],
        },
        "scenario": {"horizon": 5, "drain": {"from": 1, "until": 5}, "warmup": 0},
    });
    partial(&expected, &v, "$");
    both_backends(&src, "Day");
}

fn d03(initial: bool) -> String {
    d01(3, "", "contract I { invariant good }").replace(
        "observed case { hit: bool = false }",
        &format!("observed case {{ hit: bool = false; good: bool = {initial} }}"),
    )
}

#[test]
fn d03_receipt_time_invariant_violation_of_held_cases_survives_the_cutoff() {
    // good = false at receipt: every received case violates the unconditional
    // invariant at its receipt (0, 1, 2 <= D3), held ones included; the
    // violation is not excused by the cutoff. Admission stays null for 2, 3.
    let v = check(&d03(false), "Day");
    let p = &v["properties"][0];
    assert_eq!(p["id"], "contract I #1 invariant");
    assert_eq!(p["verdict"], "violated");
    let rows: Vec<_> = p["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["arrival"].clone(), c["verdict"].clone()))
        .collect();
    assert_eq!(
        rows,
        vec![
            (json!(0), json!("violated")),
            (json!(1), json!("violated")),
            (json!(2), json!("violated"))
        ]
    );
    assert_eq!(v["verdict"], "violated");
    let admitted: Vec<_> = v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["admitted_at"].clone())
        .collect();
    assert_eq!(admitted, vec![r(0, 0), Value::Null, Value::Null]);
    both_backends(&d03(false), "Day");
    // The single edit good = true: nothing violated up to D3, but the states
    // after the cutoff are not inspected: inconclusive, never satisfied.
    let v = check(&d03(true), "Day");
    let rows: Vec<_> = v["properties"][0]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["verdict"].clone())
        .collect();
    assert_eq!(rows, vec![json!("inconclusive"); 3]);
    assert_eq!(v["verdict"], "inconclusive");
    both_backends(&d03(true), "Day");
}

/// D02 (P13 U10): the scenario owns the limit item.
fn d02(item: &str, common: &str) -> String {
    format!(
        r#"model M version "0.2" {{
  type T = enum {{ ok }}
  actor A {{ capacity 1; capability run() -> T {{ duration fixed 5min; result nondet {{ ok }} }} }}
  process Flow {{ r = A.run(); end done }}
  cut C {{ include [A] }}
  policy fifo tie_by case_id
{common}  scenario Day {{
    arrivals fixed [0min, 1min, 2min]
{item}    horizon 16min
    bounds {{ cases 3; live_tasks 1; clock 20min }}
  }}
  verify safety no_resource_overcommit
}}
"#
    )
}

fn kinds(v: &Value) -> Vec<(Value, Value)> {
    v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let t = &c["terminals"][0];
            (t["kind"].clone(), t["end"].clone())
        })
        .collect()
}

#[test]
fn d02_scenario_queue_and_concurrent_reject_lower_and_run() {
    // queue capacity 1: case2 waits (1 waiting), case3 finds the queue full at
    // 2 and is rejected. Cases 1, 2 end at 5, 10.
    let v = check(&d02("    queue capacity 1 on_overflow reject\n", ""), "Day");
    assert_eq!(errors(&v), vec![]);
    assert_eq!(
        kinds(&v),
        vec![
            (json!("end"), json!("done")),
            (json!("end"), json!("done")),
            (json!("rejected"), Value::Null)
        ]
    );
    assert_eq!(v["verdict"], "satisfied");
    // concurrent cases 1: case1 occupies the only slot until 5; cases 2 and 3
    // arrive at 1 and 2 and are rejected.
    let v = check(
        &d02("    concurrent cases 1 on_overflow reject\n", ""),
        "Day",
    );
    assert_eq!(errors(&v), vec![]);
    assert_eq!(
        kinds(&v),
        vec![
            (json!("end"), json!("done")),
            (json!("rejected"), Value::Null),
            (json!("rejected"), Value::Null)
        ]
    );
}

#[test]
fn d02_scenario_local_overflow_end_is_unsupported_but_the_common_item_is_not() {
    for item in ["queue capacity 1", "concurrent cases 1"] {
        // The only edit: reject -> end spill inside the scenario. U-SEM-0
        // before runtime, and no invalid (E-) diagnostic (exit 4, not 3).
        let v = check(
            &d02(&format!("    {item} on_overflow end spill\n"), ""),
            "Day",
        );
        assert_eq!(v["verdict"], "unsupported", "{item}");
        let es = errors(&v);
        assert!(
            es.iter().all(|(c, _)| c.starts_with("U-")),
            "{item}: {es:?}"
        );
        assert!(
            es.contains(&(
                "U-SEM-0".into(),
                "scenario overflow terminal is unsupported".into()
            )),
            "{item}: {es:?}"
        );
        assert!(v["properties"].as_array().unwrap().is_empty());
        // Control: the same item at model scope (ADR 0039 permits it).
        let v = check(
            &d02("", &format!("  {item} on_overflow end spill\n")),
            "Day",
        );
        assert_eq!(errors(&v), vec![], "{item}");
        let k = kinds(&v);
        assert_eq!(k[0], (json!("end"), json!("done")));
        assert_eq!(k[2], (json!("overflow"), json!("spill")), "{item}");
    }
}

#[test]
fn d02_concurrent_in_both_the_model_and_the_scenario_is_never_an_override() {
    let item = "concurrent cases 1 on_overflow wait_external fifo";
    let v = check(
        &d02(&format!("    {item}\n"), &format!("  {item}\n")),
        "Day",
    );
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(
        errors(&v),
        // The same pair of refusals as the existing queue item
        // (scn-duplicate-common-queue.broken-both): the selection check and
        // the effective model's own duplicate check.
        vec![
            (
                "U-SEM-0".to_owned(),
                "concurrent declared both in model and scenario Day".to_owned()
            ),
            ("U-SEM-0".to_owned(), "duplicate concurrent".to_owned())
        ]
    );
}

/// D04: ctx-contest-metrics with its bounds moved into scenario Day.
fn d04(horizon: u64, respond_by: u64, extra: &str) -> String {
    format!(
        r#"model ContestRuntime version "0.2" {{
  type T = enum {{ pass, fail }}
  people [j,p]
  actor Judge {{ executors [j]; capability judge() -> T {{ duration fixed 1min; result nondet {{ fail }} }} }}
  actor Route {{ executors [p]; capability respond() -> T {{ duration fixed 2min; result nondet {{ pass }} }} }}
  actor Party {{ control party; standing over [end.returned] basis "claim" }}
  permit j to Judge.judge
  permit p to Route.respond
  authority G {{ basis "decision"; grants j to Judge.judge }}
  process Flow {{ d = Judge.judge(); if d == fail {{ end returned }} else {{ end posted }} }}
  cut C {{ include [Judge,Route,Party] }}
  scenario Day {{
{extra}    bounds {{ cases 1; live_tasks 2; clock 10min }}
    horizon {horizon}min
  }}
  metrics {{ window 10min; currency "JPY"; rate Judge per_minute 1; rate Route per_minute 1; rate Judge.judge per_start 0; rate Route.respond per_start 0 }}
  verify safety no_resource_overcommit
  contract Controls {{ contest Party on end.returned route Route.respond reversible_by [j] respond_by {respond_by}min }}
}}
"#
    )
}

fn property<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("no {id}"))
}

#[test]
fn d04_business_end_before_the_horizon_is_not_a_cutoff_and_the_route_stays_open() {
    // Judge 0..1, End returned at 1. Not filed: execution closes at 1, work 1.
    // Filed at 1: the route occupies 1..3, cut at D2 after 1 minute; deadline
    // 1+2 = 3 > D is undecided. End row stays an End (lead 1, work 1), no
    // cut_off case, metrics work/cost 1..2, observed until 1..2.
    let src = d04(2, 2, "");
    let v = check(&src, "Day");
    partial(
        &json!({
            "verdict": "inconclusive",
            "bounds": {"truncated": false},
            "case_outcomes": [{"case_id": 1, "terminals": [
                {"kind": "end", "end": "returned", "completed_at": r(1, 1), "elapsed": r(1, 1),
                 "work": r(1, 1)}]}],
            "metrics": {"status": "complete", "overall": {
                "counts": {"cut_off": {"stats": q("0", "0")}},
                "work": q("1", "2"), "observed_cost_total": q("1", "2"),
                "observed_until": q("1", "2")}},
        }),
        &v,
        "$",
    );
    assert_eq!(
        property(&v, "contract Controls #1 contest_response")["verdict"],
        "inconclusive"
    );
    both_backends(&src, "Day");
    // respond_by 1: the filed route's deadline 2 <= D2 is unmet: violated.
    let v = check(&d04(2, 1, ""), "Day");
    assert_eq!(
        property(&v, "contract Controls #1 contest_response")["verdict"],
        "violated"
    );
    assert_eq!(v["verdict"], "violated");
    assert_eq!(
        v["case_outcomes"][0]["terminals"].as_array().unwrap().len(),
        1
    );
    assert_eq!(v["case_outcomes"][0]["terminals"][0]["kind"], "end");
    // horizon 3: the route answers at 3 = D, inside the deadline: satisfied,
    // business End still 1, total work 1..3 (as the original fixture).
    let v = check(&d04(3, 2, ""), "Day");
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(
        v["case_outcomes"][0]["terminals"][0]["completed_at"],
        r(1, 1)
    );
    assert_eq!(v["metrics"]["overall"]["work"], q("1", "3"));
}

#[test]
fn d04_sampling_still_refuses_nondeterministic_filing() {
    let v = simulate(
        &d04(
            2,
            2,
            "    simulation replications 4 seed 1 confidence 0.5\n",
        ),
        "Day",
    );
    assert_eq!(v["verdict"], "unsupported");
    assert!(
        errors(&v).iter().any(|(c, _)| c == "U-SEM-0"),
        "{:?}",
        errors(&v)
    );
}

#[test]
fn d05_unused_scenario_fails_only_its_own_case_dependent_constraint_with_a_fresh_table() {
    // A record result type and a set<CaseId> state. Small (selected) is valid;
    // Large has 65 cases, beyond the 64-element set limit (ADR 0035). Each
    // scenario is lowered with a fresh table: the record type is not seen as
    // a duplicate, and only Large's own refusal appears.
    let large = format!(
        "  scenario Large {{\n    arrivals fixed [{}]\n    bounds {{ cases 65; live_tasks 1; clock 30min }}\n  }}\n",
        vec!["0min"; 65].join(", ")
    );
    let src = |extra: &str| {
        format!(
            r#"model M version "0.2" {{
  type Entry = record {{ confidence: enum {{ high, low }} }}
  actor L {{ state keys: set<CaseId> }}
  actor A {{ capacity 1; capability c() -> Entry {{ duration fixed 1min; result nondet {{ Entry {{ confidence = high }}, Entry {{ confidence = low }} }} }} }}
  process P {{ e = A.c(); if e.confidence == low {{ end low }} else {{ end high }} }}
  cut C {{ include [L,A] }}
  scenario Small {{
    arrivals fixed [0min, 0min]
    bounds {{ cases 2; live_tasks 1; clock 30min }}
  }}
{extra}  verify reachable end.low
}}
"#
        )
    };
    let v = check(&src(&large), "Small");
    assert_eq!(v["verdict"], "unsupported");
    let es = errors(&v);
    assert_eq!(es.len(), 1, "{es:?}");
    assert_eq!(es[0].0, "U-SEM-0");
    let ok = check(&src(""), "Small");
    assert_eq!(errors(&ok), vec![]);
    assert_eq!(ok["verdict"], "satisfied");
    // Two valid scenarios with the same record declarations both lower.
    let twin = "  scenario Twin {\n    arrivals fixed [0min]\n    bounds { cases 1; live_tasks 1; clock 30min }\n  }\n";
    for s in ["Small", "Twin"] {
        let v = check(&src(twin), s);
        assert_eq!(errors(&v), vec![], "{s}");
        assert_eq!(v["verdict"], "satisfied", "{s}");
    }
}
