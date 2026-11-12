#![allow(dead_code)]
use serde_json::{Value, json};
#[path = "evaluation_integration_inputs.rs"]
pub mod i;
#[path = "evaluation_quality_table_oracle.rs"]
pub mod quality_table;

#[derive(Clone, Copy)]
pub enum Op<'a> {
    Compare(&'a [&'a str], bool),
    Single(&'a str, bool),
}
pub const W: &str = r#"model WaitingEvaluation version "0.2" {
  type T = enum { ok }
  actor A { capacity 1; capability work() -> T { duration fixed 2min; result nondet { ok } } }
  process P { a = A.work(); wait 1min; end done }
  process Q { a = A.work(); wait 1min; end done }
  cut C { include [A] }
  configuration Left { cut C; process P }
  configuration Right { cut C; process Q }
  policy fifo tie_by case_id
  scenario Day { arrivals fixed [0min,0min]; bounds { cases 2; live_tasks 2; clock 20min }; simulation replications 4 seed 7 confidence 0.5 }
  metrics { window 20min; currency "JPY"; rate A per_minute 1; rate A.work per_start 0; breakdown waits }
  property case.Done = reached(end.done)
  metric cohort.DoneRate = expected(count(case.Done)) / arrivals
  verify reachable end.done
  compare [Left,Right] under [Day] cut C hard { cohort.DoneRate >= 0.5 } objectives { minimize metrics.overall.work }
}"#;

pub fn masked(s: &str) -> String {
    assert!(s.contains("breakdown waits"));
    s.replace("breakdown waits", "               ")
}
pub fn compared(s: &str, sample: bool) -> Value {
    super::run(s, Op::Compare(&["Left", "Right"], sample), true)
}
pub fn scalar(n: &str, sample: bool) -> Value {
    if sample {
        json!({"mean":n,"min":n,"max":n,"samples":4,"reason":null})
    } else {
        json!({"min":n,"max":n,"reason":null})
    }
}
pub fn assumption_order(r: &Value, sample: bool) {
    let fields: Vec<_> = r["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["field"].as_str().unwrap())
        .collect();
    let fourth = if sample {
        "quality.estimation"
    } else {
        "quality.scheduler"
    };
    let q = fields
        .iter()
        .position(|s| *s == "quality.definition")
        .unwrap();
    assert_eq!(
        &fields[q..q + 5],
        &[
            "quality.definition",
            "quality.denominator",
            "quality.form",
            fourth,
            "metrics.waiting"
        ]
    );
    if let Some(p) = fields.iter().position(|s| *s == "probability.scheduler") {
        assert!(q + 4 < p);
    }
    assert_eq!(
        fields.iter().filter(|&&s| s == "metrics.waiting").count(),
        1
    );
}
pub fn hand_metrics(r: &Value, sample: bool, fault: bool) {
    let (service, leads, wait, avg) = if fault {
        ("3", ["4", "7"], "3", "3/7")
    } else {
        ("2", ["3", "5"], "2", "2/5")
    };
    let metrics = &r["metrics"];
    assert_eq!(metrics["status"], "complete");
    for (ix, case) in metrics["cases"].as_array().unwrap().iter().enumerate() {
        let end = case["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["end"] == "done")
            .unwrap();
        assert_eq!(end["lead_time"], scalar(leads[ix], sample));
        assert_eq!(end["breakdown"]["in_service"], scalar(service, sample));
        assert_eq!(end["breakdown"]["intentional"], scalar("1", sample));
        assert_eq!(
            end["breakdown"]["queue"]["actor_capacity"],
            scalar(if ix == 0 { "0" } else { wait }, sample)
        );
        for k in ["resource", "executor", "combined"] {
            assert_eq!(end["breakdown"]["queue"][k], scalar("0", sample));
        }
        assert_eq!(end["breakdown"]["unattributed"], scalar("0", sample));
        assert_eq!(case["tasks"][0]["predecessor_wait"], scalar("0", sample));
        assert_eq!(
            case["tasks"][0]["wait_causes"]["actor_capacity"],
            scalar(if ix == 0 { "0" } else { wait }, sample)
        );
        assert!(case.get("dwell").is_none());
    }
    let queue = &metrics["actors"][0]["queue"];
    assert_eq!(queue["wait_total"], json!({"stats":scalar(wait,sample)}));
    assert_eq!(queue["length_max"], scalar("1", sample));
    assert_eq!(
        queue["length_time_average"],
        json!({"stats":scalar(avg,sample)})
    );
    assert_eq!(
        metrics["overall"]["work"],
        scalar(if fault { "6" } else { "4" }, sample)
    );
}
pub fn arrays(v: &Value, sample: bool, waits: bool) {
    let mut checked = vec![
        "configuration_selection",
        "cut_compatibility",
        "terminal_compatibility",
    ];
    if sample {
        checked.extend(["simulation_clause", "random_key_separation"]);
    }
    checked.extend([
        "property_tables",
        "terminal_tables",
        "metric_tables",
        "hard_filter",
        "admissibility",
    ]);
    checked.push(if sample {
        "hard_cross_plan_bonferroni"
    } else {
        "pareto_sure_dominance"
    });
    let raw = if sample {
        br#"["ranking","pareto","weighted_score","differences","difference_intervals","cross_plan_confidence","operation_coupling","terminal_meaning","metric_differences","metric_ranking","metric_semantic_alignment"]"#.as_slice()
    } else {
        br#"["ranking","weighted_score","differences","difference_intervals","cross_plan_confidence","operation_coupling","terminal_meaning","metric_differences","metric_ranking","metric_semantic_alignment"]"#.as_slice()
    };
    let mut unchecked: Value = serde_json::from_slice(raw).unwrap();
    if waits {
        unchecked
            .as_array_mut()
            .unwrap()
            .push(json!("waiting_tables"));
    }
    assert_eq!(v["checked"], json!(checked));
    assert_eq!(v["unchecked"], unchecked);
    if !waits {
        assert_eq!(serde_json::to_vec(&v["unchecked"]).unwrap(), raw);
    }
}
pub fn transport(s: &str, v: &Value, sample: bool) {
    for p in v["plans"].as_array().unwrap() {
        let one = super::run(
            s,
            Op::Single(p["configuration"].as_str().unwrap(), sample),
            true,
        );
        assert_eq!(
            p["report"], one,
            "all single-command report bytes/fields including waiting and quality"
        );
    }
}
pub fn check_metrics_table(v: &Value, control: &Value) {
    assert_eq!(
        v["tables"]["metrics"], control["tables"]["metrics"],
        "A7 new leaves must not leak into B7 tables"
    );
    assert!(!v["tables"]["metrics"].as_array().unwrap().is_empty());
    for row in v["tables"]["metrics"].as_array().unwrap() {
        let k = row["key"].to_string();
        for excluded in [
            "queue_wait_observed",
            "breakdown",
            "wait_causes",
            "predecessor_wait",
            "join_wait",
            "dwell",
            "length_time_average",
            "wait_total",
        ] {
            assert!(
                !k.contains(excluded),
                "new waiting leaf in old metric table: {k}"
            );
        }
        assert_ne!(row["key"]["scope"], "queue");
    }
}

#[test]
fn d05_exact_checked_unchecked_and_no_waiting_byte_control_both_modes() {
    for sample in [false, true] {
        let v = compared(W, sample);
        assert_eq!(v["diagnostics"], json!([]));
        assert_eq!(v["verdict"], "satisfied");
        arrays(&v, sample, true);
        let control = compared(&masked(W), sample);
        arrays(&control, sample, false);
        assert_eq!(v["evaluation"], control["evaluation"]);
        check_metrics_table(&v, &control);
        for cell in v["evaluation"]["hard"][0]["values"].as_array().unwrap() {
            assert_eq!(cell["verdict"], "satisfied");
            assert_eq!(
                cell["interval"],
                i::interval(if sample { "1/2" } else { "1" }, "1")
            );
        }
        assert_eq!(
            v["evaluation"]["plans"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p["status"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["admissible", "admissible"]
        );
        let pareto = &v["evaluation"]["pareto"];
        assert_eq!(
            pareto["status"],
            if sample { "not_computed" } else { "computed" }
        );
        assert_eq!(
            pareto["reason"],
            if sample {
                json!("simulation_estimates")
            } else {
                Value::Null
            }
        );
        transport(W, &v, sample);
        for p in v["plans"].as_array().unwrap() {
            hand_metrics(&p["report"], sample, false);
            assumption_order(&p["report"], sample);
        }
    }
}

#[test]
fn waiting_and_quality_transport_horizon_cutoff_and_computational_clock() {
    {
        let sample = false;
        for h in [14, 30] {
            let source = i::horizon(h, "0.3", true);
            let source = i::edit(
                &source,
                "currency \"JPY\";",
                "currency \"JPY\"; breakdown waits;",
            );
            let v = super::run(&source, Op::Compare(&["Check", "Fast"], sample), true);
            assert_eq!(v["diagnostics"], json!([]));
            transport(&source, &v, sample);
            let control = super::run(
                &masked(&source),
                Op::Compare(&["Check", "Fast"], sample),
                true,
            );
            check_metrics_table(&v, &control);
            assert_eq!(
                v["tables"]["quality"], control["tables"]["quality"],
                "all quality rows, values and denominators"
            );
            let cutoff: Vec<_> = v["tables"]["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["kind"] == "cut_off")
                .collect();
            assert_eq!(
                cutoff.len(),
                3,
                "rectangle follows declared horizon even when no cutoff is observed"
            );
            for (ix, row) in cutoff.iter().enumerate() {
                assert_eq!(row["case_id"], ix + 1);
                assert_eq!(
                    row["values"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| c["configuration"].as_str().unwrap())
                        .collect::<Vec<_>>(),
                    ["Check", "Fast"]
                );
            }
            if h == 14 {
                assert_eq!(v["tables"]["quality"], quality_table::horizon14());
                for c in v["evaluation"]["hard"][0]["values"].as_array().unwrap() {
                    assert_eq!(c["interval"], i::interval("1/75", "26/75"));
                    assert_eq!(c["reason"], "undetermined");
                    assert_eq!(c["verdict"], "inconclusive");
                }
            }
        }
    }
    for h in [1, 5] {
        let source = i::simulation(h, false);
        let source = i::edit(
            &source,
            "currency \"JPY\";",
            "currency \"JPY\"; breakdown waits;",
        );
        let v = super::run(&source, Op::Compare(&["Good", "Bad"], true), true);
        assert_eq!(v["diagnostics"], json!([]));
        transport(&source, &v, true);
        let control = super::run(&masked(&source), Op::Compare(&["Good", "Bad"], true), true);
        check_metrics_table(&v, &control);
        assert_eq!(v["tables"]["quality"], control["tables"]["quality"]);
        let cutoff: Vec<_> = v["tables"]["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "cut_off")
            .collect();
        assert_eq!(cutoff.len(), 1);
        assert_eq!(
            cutoff[0]["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["configuration"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["Good", "Bad"]
        );
        for c in v["evaluation"]["hard"][0]["values"].as_array().unwrap() {
            assert_eq!(
                c["interval"],
                i::interval("0", if h == 1 { "1" } else { "1/2" })
            );
            assert_eq!(
                c["verdict"],
                if h == 1 { "inconclusive" } else { "satisfied" }
            );
        }
    }
    let s = i::horizon(30, "0.3", true);
    let s = i::edit(
        &s,
        "currency \"JPY\";",
        "currency \"JPY\"; breakdown waits;",
    );
    let s = i::edit(&s, "clock 30min", "clock 14min");
    let v = super::run(&s, Op::Compare(&["Check", "Fast"], false), true);
    for p in v["plans"].as_array().unwrap() {
        assert_eq!(p["report"]["bounds"]["truncated"], true);
        assert_eq!(
            p["report"]["metrics"]["actors"][0]["queue"]["wait_total"]["stats"]["reason"],
            "truncated"
        );
    }
    for c in v["evaluation"]["hard"][0]["values"].as_array().unwrap() {
        assert_eq!(c["reason"], "truncated");
    }
}

pub const WAIT_PATHS: &[&str] = &[
    "metrics.overall.queue_wait_observed",
    "metrics.actors.A.queue.wait_total",
    "metrics.actors.A.queue.length_max",
    "metrics.actors.A.queue.length_time_average",
    "metrics.cases.terminals.breakdown.in_service",
    "metrics.cases.terminals.breakdown.queue.actor_capacity",
    "metrics.cases.terminals.breakdown.queue.resource",
    "metrics.cases.terminals.breakdown.queue.executor",
    "metrics.cases.terminals.breakdown.queue.combined",
    "metrics.cases.terminals.breakdown.intentional",
    "metrics.cases.terminals.breakdown.external",
    "metrics.cases.terminals.breakdown.contest_route",
    "metrics.cases.terminals.breakdown.unattributed",
    "metrics.cases.intentional_wait",
    "metrics.cases.tasks.wait_causes.actor_capacity",
    "metrics.cases.tasks.wait_causes.resource",
    "metrics.cases.tasks.wait_causes.executor",
    "metrics.cases.tasks.wait_causes.combined",
    "metrics.cases.tasks.wait_causes.unattributed",
    "metrics.cases.tasks.predecessor_wait",
    "metrics.cases.joins.branches.join_wait",
    "metrics.cases.dwell.breakdown.intentional",
];
#[test]
fn every_waiting_leaf_is_still_an_unsupported_objective_or_hard_not_a_sixth_overall() {
    for path in WAIT_PATHS {
        for hard in [false, true] {
            let source = if hard {
                i::edit(W, "cohort.DoneRate >= 0.5", &format!("{path} >= 0.5"))
            } else {
                i::edit(W, "metrics.overall.work", path)
            };
            let v = compared(&source, false);
            assert_eq!(v["verdict"], "unsupported", "{path}");
            assert!(v["evaluation"].is_null());
            for p in v["plans"].as_array().unwrap() {
                if p["report"].get("metrics").is_none() {
                    assert!(
                        p["report"]["assumptions"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|a| a["field"] != "metrics.waiting")
                    );
                }
            }
            assert!(
                v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .chain(v["plans"][0]["report"]["diagnostics"].as_array().unwrap())
                    .any(|d| d["code"] == "U-SYN-0"),
                "{path}"
            );
        }
    }
}

#[test]
fn missing_simulation_and_structural_refusal_have_no_metrics_waiting_disclosure() {
    let no_clause = i::edit(W, "; simulation replications 4 seed 7 confidence 0.5", "");
    let structural = i::edit(W, "verify reachable end.done", "verify terminating end");
    for source in [no_clause, structural] {
        for config in ["Left", "Right"] {
            let v = super::run(&source, Op::Single(config, true), true);
            assert_ne!(v["verdict"], "satisfied");
            assert!(v.get("metrics").is_none());
            let fields: Vec<_> = v["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a["field"].as_str().unwrap())
                .collect();
            assert!(!fields.contains(&"metrics.waiting"));
            let q = fields
                .iter()
                .position(|s| *s == "quality.definition")
                .unwrap();
            assert_eq!(
                &fields[q..q + 4],
                &[
                    "quality.definition",
                    "quality.denominator",
                    "quality.form",
                    "quality.estimation"
                ]
            );
        }
    }
}

#[test]
fn one_input_fault_changes_the_precomputed_queue_oracle_not_the_expected_data() {
    let source = i::edit(W, "duration fixed 2min", "duration fixed 3min");
    let v = compared(&source, false);
    for p in v["plans"].as_array().unwrap() {
        assert!(
            std::panic::catch_unwind(|| hand_metrics(&p["report"], false, false)).is_err(),
            "the unchanged hand oracle must reject the single input fault"
        );
        hand_metrics(&p["report"], false, true);
        assert_ne!(
            p["report"]["metrics"]["actors"][0]["queue"]["length_time_average"],
            json!({"stats":scalar("2/5",false)})
        );
    }
}

#[test]
fn waiting_does_not_change_any_of_the_five_overall_objectives_or_pareto() {
    let control = i::fixture("cmp7-overall");
    let source = i::edit(
        &control,
        "currency \"JPY\";",
        "currency \"JPY\"; breakdown waits;",
    );
    let v = super::run(&source, Op::Compare(&["AsIs", "ToBeP"], false), false);
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/cmp7-overall/expected.json"
    ))
    .unwrap();
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(
        v["evaluation"], expected["evaluation"],
        "hand oracle: work8/8, named0/0, peak1/2, cost805/805, per-arrival805/805; only AsIs sure-dominates"
    );
    let baseline = super::run(&control, Op::Compare(&["AsIs", "ToBeP"], false), false);
    check_metrics_table(&v, &baseline);
    assert_eq!(
        v["unchecked"].as_array().unwrap().last(),
        Some(&json!("waiting_tables"))
    );
    assert_eq!(
        v["plans"][1]["report"]["metrics"]["cases"][0]["joins"][0]["branches"][0]["join_wait"],
        scalar("2", false)
    );
    for p in v["plans"].as_array().unwrap() {
        assert_eq!(
            p["report"],
            super::run(
                &source,
                Op::Single(p["configuration"].as_str().unwrap(), false),
                false
            )
        );
    }
}
