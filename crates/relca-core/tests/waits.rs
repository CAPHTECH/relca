//! ADR0048 Stage C. Hand oracles precede the collector; no new API imports are
//! required to express RED. Shared with CLI so both exercise the same arithmetic.
use relca_core::{compare, diff, engine, lower, reference, report, syntax};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
#[path = "support/waits_private_oracles.rs"]
mod private_oracles;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
pub fn rows() -> Value {
    serde_json::from_str(include_str!("support/waits_oracles.json")).unwrap()
}

#[test]
fn private_oracle_inventory_compiles_but_does_not_claim_collector_linkage() {
    let probes = private_oracles::probes();
    let names: std::collections::BTreeSet<_> = probes.iter().map(|p| &p.name[..3]).collect();
    assert_eq!(
        names,
        std::collections::BTreeSet::from(["P01", "P02", "P03", "P04", "P05", "P06", "P07", "P08"])
    );
    assert!(
        probes
            .iter()
            .all(|p| p.input.is_object() && p.expected.is_object())
    );
    let review = private_oracles::review_probes();
    let ids: std::collections::BTreeSet<_> = review.iter().map(|p| &p.name[..3]).collect();
    assert_eq!(
        ids,
        std::collections::BTreeSet::from(["P09", "P10", "P11", "P12"])
    );
    assert_eq!(review.len(), 10);
    let mutation = private_oracles::mutation_probes();
    assert_eq!(mutation.len(), 2, "P13 history and single-interval control");
    assert!(
        mutation
            .iter()
            .all(|p| p.name == "P13_wait_interval_not_total"
                && p.input.is_object()
                && p.expected.is_object())
    );
}
pub fn source(name: &str) -> String {
    fs::read_to_string(root().join("fixtures").join(name).join("model.relca")).unwrap()
}
pub fn masked(s: &str) -> String {
    let out = s.replace("breakdown waits", "               ");
    assert_ne!(out, s, "control must actually remove the opt-in");
    out
}
pub fn run(s: &str, r: &Value, cfg: Option<&str>) -> Value {
    let inv = &r["invocation"];
    let scenario = inv["scenario"].as_str();
    if inv["command"] == "compare" && cfg.is_none() {
        serde_json::to_value(compare::compare_with_scenario(
            s,
            &["Left", "Right"],
            compare::Analysis::Exhaustive,
            scenario,
        ))
        .unwrap()
    } else if inv["command"] == "simulate" {
        serde_json::to_value(report::simulate_with_selection(s, cfg, scenario)).unwrap()
    } else {
        serde_json::to_value(report::check_with_selection(s, cfg, scenario)).unwrap()
    }
}

pub fn subset(want: &Value, actual: &Value, path: &str) {
    match want {
        Value::Object(o) => {
            let a = actual
                .as_object()
                .unwrap_or_else(|| panic!("{path}: missing object: {actual}"));
            for (key, value) in o {
                subset(
                    value,
                    a.get(key)
                        .unwrap_or_else(|| panic!("{path}/{key}: missing")),
                    &format!("{path}/{key}"),
                );
            }
        }
        Value::Array(xs) => {
            let a = actual
                .as_array()
                .unwrap_or_else(|| panic!("{path}: missing array"));
            assert_eq!(xs.len(), a.len(), "{path}: closed inventory");
            for (i, (x, y)) in xs.iter().zip(a).enumerate() {
                subset(x, y, &format!("{path}/{i}"));
            }
        }
        _ => assert_eq!(want, actual, "{path}"),
    }
}

/// The case terminal oracle lists business Ends only. The rejected inventory is
/// independently checked against its old lead_time reason, never omitted silently.
pub fn hand(name: &str, m: &Value, want: &Value) {
    assert!(m.is_object(), "{name}: metrics must exist after opt-in");
    assert_eq!(
        m["cases"].as_array().unwrap().len(),
        want["cases"].as_array().unwrap().len(),
        "{name}: cases"
    );
    subset(&want["overall"], &m["overall"], &format!("{name}/overall"));
    subset(&want["actors"], &m["actors"], &format!("{name}/actors"));
    for (actual, expected) in m["cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(want["cases"].as_array().unwrap())
    {
        assert_eq!(actual["case_id"], expected["case_id"], "{name}: case order");
        subset(
            &expected["tasks"],
            &actual["tasks"],
            &format!("{name}/case{}/tasks", actual["case_id"]),
        );
        assert_eq!(
            actual.get("joins"),
            expected.get("joins"),
            "{name}: joins (including mode, fork and branch order)"
        );
        assert_eq!(
            actual.get("dwell"),
            expected.get("dwell"),
            "{name}: conditional dwell presence and exact shape"
        );
        for t in expected["terminals"].as_array().unwrap() {
            let found: Vec<_> = actual["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|a| a["end"] == t["end"])
                .collect();
            assert_eq!(found.len(), 1, "{name}: unique end {}", t["end"]);
            assert_eq!(
                found[0].get("breakdown"),
                t.get("breakdown"),
                "{name}/case{}/end{}",
                actual["case_id"],
                t["end"]
            );
        }
        for t in actual["terminals"].as_array().unwrap() {
            if t["kind"] == "rejected" {
                let b = t["breakdown"]
                    .as_object()
                    .expect("rejected still needs breakdown");
                for (key, s) in b {
                    let scalars: Vec<_> = if key == "queue" {
                        s.as_object().unwrap().values().collect()
                    } else {
                        vec![s]
                    };
                    for scalar in scalars {
                        assert_eq!(scalar, &t["lead_time"], "{name}: null rejected reason");
                    }
                }
            }
        }
    }
}

pub fn assumptions(v: &Value) {
    let rows = v["assumptions"].as_array().unwrap();
    let positions: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r["field"] == "metrics.waiting")
        .collect();
    assert_eq!(positions.len(), 1, "one explicit opt-in assumption");
    let (index, row) = positions[0];
    assert_eq!(row["value"], "exclusive_interval_partition_v1");
    assert_eq!(row["affects"], WAITING_AFFECTS);
    for (i, r) in rows.iter().enumerate() {
        let f = r["field"].as_str().unwrap();
        if f == "quality.quantile" || f == "probability.scheduler" {
            assert!(i > index, "waiting precedes {f}");
        } else if f != "metrics.waiting" {
            assert!(
                i < index,
                "all other existing assumption rows precede waiting: {f}"
            );
        }
    }
}
pub const WAITING_AFFECTS: &str = "区分は時刻ごとに一つ。external、in_service、contest_route、queue、intentional、unattributedの順で決める。原因は全案件のTaskの容量・資源・人の占有から読む。異なる原因又は既知と不明の混在はcombinedとし、説明できない時間を隠さない。前工程待ちは受入から、first_start_waitは到着から数える。網羅の区分の範囲の和はlead timeの範囲と一致するとは限らない。dwellとhorizonで止めたqueueの和・時間平均のlowerは固定した観測期間内の観測量の印であり、打切り後まで延長した期間の平均の下界を主張しない。";

pub fn erase_new_metrics(m: &mut Value) {
    if !m.is_object() {
        return;
    }
    for c in m["cases"].as_array_mut().unwrap() {
        for key in ["dwell", "joins"] {
            c.as_object_mut().unwrap().remove(key);
        }
        for t in c["terminals"].as_array_mut().unwrap() {
            t.as_object_mut().unwrap().remove("breakdown");
        }
        for t in c["tasks"].as_array_mut().unwrap() {
            for key in ["wait_causes", "predecessor_wait"] {
                t.as_object_mut().unwrap().remove(key);
            }
        }
    }
    for a in m["actors"].as_array_mut().unwrap() {
        a.as_object_mut().unwrap().remove("queue");
    }
    m["overall"]
        .as_object_mut()
        .unwrap()
        .remove("queue_wait_observed");
    if let Some(a) = m["checked"].as_array_mut() {
        a.retain(|x| x != "waiting_decomposition" && x != "queue_length_max_and_average");
    }
    if let Some(a) = m["unchecked"].as_array_mut()
        && let Some(i) = a.iter().position(|x| x == "queue_length_quantiles")
    {
        a.splice(
            i..=i,
            [
                json!("waiting_decomposition"),
                json!("queue_length_statistics"),
            ],
        );
    }
}

pub fn check_name(name: &str) {
    let rows = rows();
    let row = &rows[name];
    let s = source(name);
    let v = run(&s, row, None);
    assert_eq!(
        v["verdict"], row["verdict"],
        "{name}: expected acceptance, not legacy parser refusal: {}",
        v["diagnostics"]
    );
    if let Some(code) = row["static"].as_str() {
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code),
            "{name}: {code}"
        );
        return;
    }
    if row["invocation"]["command"] == "compare" {
        if !row["hand"].is_null() {
            for plan in v["plans"].as_array().unwrap() {
                hand(name, &plan["report"]["metrics"], &row["hand"]);
                assumptions(&plan["report"]);
            }
        }
    } else if !row["hand"].is_null() {
        hand(name, &v["metrics"], &row["hand"]);
        assumptions(&v);
    }
}

macro_rules! acceptance {
    ($test:ident, $base:literal) => {
        #[test]
        fn $test() {
            let data = rows();
            let names: Vec<_> = data
                .as_object()
                .unwrap()
                .keys()
                .filter(|n| *n == $base || n.starts_with(concat!($base, ".broken-")))
                .collect();
            assert!(names.len() >= 2);
            for name in names {
                check_name(name);
            }
        }
    };
}
acceptance!(
    t01_fifo_task_causes_predecessors_and_actor_integral,
    "wait-fifo"
);
acceptance!(t02_person_priority_and_same_leaf_correlation, "wait-person");
acceptance!(
    t03_external_admission_origin_and_horizon_dwell,
    "wait-external"
);
acceptance!(t04_resource_and_capacity_two_control, "wait-resource");
acceptance!(
    t05_distinct_queued_task_causes_are_combined,
    "wait-combined"
);
acceptance!(
    t06_intentional_intervals_not_unattributed,
    "wait-intentional"
);
acceptance!(
    t07_join_any_is_null_join_any_and_all_is_two_zero,
    "wait-any"
);
acceptance!(
    t09_contest_route_excludes_intentional_and_service,
    "wait-contest"
);
acceptance!(t10_actor_declaration_order_not_names, "wait-label");
acceptance!(
    t12_real_horizon_queue_is_lower_clock_is_null,
    "wait-horizon-queue"
);
acceptance!(
    t13_samples_are_replications_and_horizon_is_not_failure,
    "wait-sample"
);
acceptance!(
    t14_one_task_two_shortages_are_combined,
    "wait-one-task-combined"
);
acceptance!(t15_rejection_and_zero_time_overflow_differ, "wait-reject");
acceptance!(
    t16_zero_observation_average_has_no_samples,
    "wait-zero-window-observation"
);
acceptance!(t17_compare_embedded_reports_only, "wait-compare-diff");
acceptance!(
    t18_sample_does_not_randomize_nondeterminism,
    "wait-no-policy-sample"
);
acceptance!(
    t19_only_positive_duration_support_not_mean,
    "wait-duration-support"
);

fn legacy_arithmetic(name: &str, report: &Value, oracle: &Value) {
    let actual = report["metrics"]["cases"]
        .as_array()
        .expect("legacy metrics");
    let wanted = oracle["cases"].as_array().unwrap();
    assert_eq!(actual.len(), wanted.len(), "{name}: legacy cases");
    for (c, w) in actual.iter().zip(wanted) {
        for task in w["tasks"].as_array().unwrap() {
            let t = c["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["binding"] == task["binding"])
                .unwrap();
            let causes: Vec<_> = task["wait_causes"].as_object().unwrap().values().collect();
            if causes.iter().all(|s| s["min"].is_string()) {
                for endpoint in ["min", "max"] {
                    let sum: u64 = causes
                        .iter()
                        .map(|s| s[endpoint].as_str().unwrap().parse::<u64>().unwrap())
                        .sum();
                    assert_eq!(
                        t["wait"][endpoint],
                        sum.to_string(),
                        "{name}: old wait equals hand cause sum"
                    );
                }
            } else {
                assert_eq!(
                    t["wait"]["reason"], causes[0]["reason"],
                    "{name}: old task null reason"
                );
            }
        }
        for expected in w["terminals"].as_array().unwrap() {
            let t = c["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["end"] == expected["end"])
                .unwrap();
            let b = expected["breakdown"].as_object().unwrap();
            let scalars: Vec<_> = b
                .iter()
                .filter(|(k, _)| *k != "queue")
                .map(|(_, v)| v)
                .chain(b["queue"].as_object().unwrap().values())
                .collect();
            if scalars
                .iter()
                .all(|s| s["min"].is_string() && s["min"] == s["max"])
            {
                let sum: u64 = scalars
                    .iter()
                    .map(|s| s["min"].as_str().unwrap().parse::<u64>().unwrap())
                    .sum();
                assert_eq!(
                    t["lead_time"]["min"],
                    sum.to_string(),
                    "{name}: old lead equals hand partition length"
                );
                assert_eq!(
                    t["lead_time"]["max"],
                    sum.to_string(),
                    "{name}: old lead equals hand partition length"
                );
            } else if scalars.iter().all(|s| s["min"].is_null()) {
                assert_eq!(
                    t["lead_time"]["reason"], scalars[0]["reason"],
                    "{name}: old lead null reason"
                );
            }
        }
    }
}

#[test]
fn every_runtime_substrate_is_valid_without_new_syntax_and_meets_legacy_hand_sums() {
    let data = rows();
    let mut failures = Vec::new();
    for (name, r) in data.as_object().unwrap() {
        if r["static"].is_string() || r["hand"].is_null() {
            continue;
        }
        let s = source(name);
        let old = run(&masked(&s), r, None);
        if old["verdict"] != r["verdict"] {
            failures.push(format!("{name}: {} {}", old["verdict"], old["diagnostics"]));
        } else {
            let reports: Vec<_> = if r["invocation"]["command"] == "compare" {
                old["plans"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| &p["report"])
                    .collect()
            } else {
                vec![&old]
            };
            for report in reports {
                legacy_arithmetic(name, report, &r["hand"]);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "unrelated old-language defects:\n{}",
        failures.join("\n")
    );
}

#[test]
fn all_exhaustive_inputs_meet_hand_values_independently_in_both_interpreters() {
    let mut complete = 0;
    let data = rows();
    for (name, r) in data.as_object().unwrap() {
        if r["static"].is_string()
            || r["hand"].is_null()
            || r["invocation"]["command"] == "simulate"
        {
            continue;
        }
        let s = source(name);
        let selections: &[Option<&str>] = if r["invocation"]["command"] == "compare" {
            &[Some("Left"), Some("Right")]
        } else {
            &[None]
        };
        for &cfg in selections {
            let model = lower::lower_with_selection(
                &syntax::parse(&s).expect("breakdown waits syntax"),
                cfg,
                r["invocation"]["scenario"].as_str(),
            )
            .expect("valid fixture lower");
            let e = engine::explore(&model);
            let mut reference = reference::explore(&model);
            assert_eq!(
                reference.stop,
                reference::StopReason::Complete,
                "{name}: zero exclusions"
            );
            let prefix = model.diagnostics.len();
            assert_eq!(
                &reference.outcome.diagnostics[..prefix],
                model.diagnostics.as_slice(),
                "reference alone prefixes lower diagnostics"
            );
            reference.outcome.diagnostics.drain(..prefix);
            hand(name, &serde_json::to_value(&e.metrics).unwrap(), &r["hand"]);
            hand(
                name,
                &serde_json::to_value(&reference.outcome.metrics).unwrap(),
                &r["hand"],
            );
            assert_eq!(e, reference.outcome, "{name}: full independent Outcome");
            let old = lower::lower_with_selection(
                &syntax::parse(&masked(&s)).unwrap(),
                cfg,
                r["invocation"]["scenario"].as_str(),
            )
            .unwrap();
            let mut enabled = e;
            let mut old_outcome = engine::explore(&old);
            let mut enabled_metrics = serde_json::to_value(enabled.metrics.take()).unwrap();
            erase_new_metrics(&mut enabled_metrics);
            assert_eq!(
                enabled_metrics,
                serde_json::to_value(old_outcome.metrics.take()).unwrap(),
                "{name}: no old-metric interference"
            );
            assert_eq!(
                enabled, old_outcome,
                "{name}: full state/budget/verdict noninterference"
            );
            complete += 1;
        }
    }
    assert_eq!(
        complete, 32,
        "32 exhaustive selections, reference Complete32, exclusions0"
    );
}

#[test]
fn t11_duplicate_span_missing_words_and_identifiers_are_contextual() {
    let s = source("wait-fifo");
    let good = s
        .replace(" A ", " breakdown ")
        .replace("[A, B]", "[breakdown, B]")
        .replace("A.", "breakdown.")
        .replace("a =", "waits =");
    assert_ne!(good, s);
    assert_eq!(report::check(&good).verdict, relca_core::Verdict::Satisfied);
    for suffix in ["}", "; }", ""] {
        let bad = format!("model M version \"0.2\" {{ metrics {{ breakdown {suffix}");
        let diagnostics = syntax::parse(&bad).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == "U-SYN-0" && d.message == "unsupported syntax: breakdown"),
            "{diagnostics:?}"
        );
    }
    let dup = source("wait-fifo.broken-duplicate");
    let ds = lower::lower(&syntax::parse(&dup).expect("duplicate is lower, not parser, error"))
        .unwrap_err();
    let d = ds.iter().find(|d| d.code == "U-SEM-0").unwrap();
    assert_eq!(d.message, "duplicate metrics breakdown waits");
    assert_eq!(d.span.unwrap().start, dup.rfind("breakdown waits").unwrap());
    assert_eq!(
        d.span.unwrap().end,
        dup.rfind("breakdown waits").unwrap() + "breakdown waits".len()
    );
}

#[test]
fn t17_compare_and_diff_tables_exclude_only_new_scope_fields_including_resources() {
    let s = source("wait-compare-diff");
    let resource = s
        .replace(
            "actor A { capacity 1",
            "resource Desk capacity 1; actor A { capacity 2",
        )
        .replace(
            "capability prep() -> T {",
            "capability prep() -> T { uses Desk:1;",
        );
    assert_ne!(resource, s);
    assert!(resource.contains("uses Desk:1;"));
    for s in [s.clone(), resource] {
        let plain = masked(&s);
        for (new, old) in [
            (
                serde_json::to_value(compare::compare(
                    &s,
                    &["Left", "Right"],
                    compare::Analysis::Exhaustive,
                ))
                .unwrap(),
                serde_json::to_value(compare::compare(
                    &plain,
                    &["Left", "Right"],
                    compare::Analysis::Exhaustive,
                ))
                .unwrap(),
            ),
            (
                serde_json::to_value(diff::diff(&s, "Left", "Right")).unwrap(),
                serde_json::to_value(diff::diff(&plain, "Left", "Right")).unwrap(),
            ),
        ] {
            assert_eq!(new["verdict"], "satisfied");
            assert_eq!(
                new["metrics"], old["metrics"],
                "new quantities stay out of comparison table"
            );
        }
    }
}

#[test]
fn t13_completion_exactly_at_horizon_does_not_add_lower() {
    let s = source("wait-sample");
    let exact = s.replace("horizon 12min", "horizon 10min");
    assert_ne!(exact, s);
    let data = rows();
    let v = run(&exact, &data["wait-sample"], None);
    hand("exact-D", &v["metrics"], &data["wait-sample"]["hand"]);
}

#[test]
fn invalid_public_ir_still_fails_closed_in_each_interpreter() {
    let s = source("wait-fifo");
    let model = lower::lower(&syntax::parse(&s).expect("new syntax")).unwrap();
    for actor in [true, false] {
        let mut m = model.clone();
        if actor {
            m.metrics.as_mut().unwrap().required_actors[0] = relca_core::ir::ActorId(usize::MAX);
        } else {
            m.metrics.as_mut().unwrap().required_capabilities[0] =
                relca_core::ir::CapabilityId(usize::MAX);
        }
        let e = engine::explore(&m);
        assert!(e.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
        assert!(e.metrics.is_none());
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::SemanticAbort);
        assert!(r.outcome.metrics.is_none());
    }
}

#[test]
fn declaration_is_an_explicit_ir_boolean_and_does_not_enable_without_opt_in() {
    let s = source("wait-fifo");
    let enabled = lower::lower(&syntax::parse(&s).expect("breakdown waits syntax")).unwrap();
    let plain = lower::lower(&syntax::parse(&masked(&s)).unwrap()).unwrap();
    // Debug avoids importing a speculative new field before implementation;
    // the public contract still fixes its exact name and true/false values.
    assert!(format!("{:?}", enabled.metrics).contains("breakdown_waits: true"));
    assert!(format!("{:?}", plain.metrics).contains("breakdown_waits: false"));
    assert!(
        enabled
            .assumptions
            .iter()
            .any(|r| r.field == "metrics.waiting")
    );
    assert!(
        !plain
            .assumptions
            .iter()
            .any(|r| r.field == "metrics.waiting")
    );
}

fn per_replication_oracle(sample: &Value) -> Value {
    let mut out = sample.clone();
    fn walk(v: &mut Value) {
        match v {
            Value::Object(o) if o.contains_key("min") => {
                o.remove("mean");
                o.remove("samples");
                if o["reason"] == "no_samples" {
                    o.insert("reason".into(), json!("no_terminal"));
                }
            }
            Value::Object(o) => {
                for x in o.values_mut() {
                    walk(x);
                }
            }
            Value::Array(a) => {
                for x in a {
                    walk(x);
                }
            }
            _ => (),
        }
    }
    walk(&mut out);
    out
}

#[test]
fn sample_histories_and_draw_keys_meet_hand_values_in_each_independent_interpreter() {
    let data = rows();
    for name in [
        "wait-sample",
        "wait-sample.broken-short-horizon",
        "wait-no-policy-sample",
    ] {
        let r = &data[name];
        let s = source(name);
        let model =
            lower::lower_with_selection(&syntax::parse(&s).expect("new syntax"), None, Some("Day"))
                .unwrap();
        let plain =
            lower::lower_with_selection(&syntax::parse(&masked(&s)).unwrap(), None, Some("Day"))
                .unwrap();
        let e = engine::simulate(&model);
        let other = reference::simulate(&model);
        assert_eq!(other.stop, reference::StopReason::Complete);
        let mut ev = serde_json::to_value(&e).unwrap();
        let rv = serde_json::to_value(&other.outcome).unwrap();
        assert_eq!(
            ev["replications"], rv["replications"],
            "{name}: full histories and draw keys"
        );
        for v in [&ev, &rv] {
            let reps = v["replications"].as_array().unwrap();
            assert_eq!(reps.len(), 4);
            for rep in reps {
                hand(name, &rep["metrics"], &per_replication_oracle(&r["hand"]));
            }
        }
        for rep in ev["replications"].as_array_mut().unwrap() {
            erase_new_metrics(&mut rep["metrics"]);
        }
        let old = serde_json::to_value(engine::simulate(&plain)).unwrap();
        assert_eq!(
            ev["replications"], old["replications"],
            "{name}: draw/decision/old scalar noninterference"
        );
    }
}

#[test]
fn explain_runs_only_configuration_free_models_and_carries_waiting_assumption() {
    let s = source("wait-fifo");
    let v = serde_json::to_value(relca_core::explain::explain(&s)).unwrap();
    assert_eq!(v["verdict"], "satisfied");
    assumptions(&v);
    hand("explain-fifo", &v["metrics"], &rows()["wait-fifo"]["hand"]);
}

#[test]
fn new_wire_round_trips_losslessly_and_nested_dtos_reject_unknown_keys() {
    let data = rows();
    for name in ["wait-fifo", "wait-any", "wait-horizon-queue", "wait-sample"] {
        let v = run(&source(name), &data[name], None);
        assert!(v["metrics"].is_object(), "{name}: opt-in metrics required");
        let m = &v["metrics"];
        let dto: relca_core::metrics::BusinessMetrics =
            serde_json::from_value(m.clone()).expect("new closed wire DTO");
        assert_eq!(
            serde_json::to_value(dto).unwrap(),
            *m,
            "{name}: no field lost by DTO"
        );
        for path in [
            "/cases/0/tasks/0/wait_causes",
            "/cases/0/terminals/0/breakdown",
            "/actors/0/queue",
            "/actors/0/queue/length_max",
        ] {
            let mut corrupt = m.clone();
            corrupt.pointer_mut(path).expect("required new field")["extra"] = json!(0);
            assert_ne!(corrupt, *m);
            assert!(
                serde_json::from_value::<relca_core::metrics::BusinessMetrics>(corrupt).is_err(),
                "{name}{path}: nested fields must deny unknown keys"
            );
        }
    }
}

#[test]
fn nondeterministic_sample_refusal_never_publishes_successful_wait_samples() {
    let data = rows();
    let name = "wait-no-policy-sample.broken-no-policy";
    let v = run(&source(name), &data[name], None);
    assert_eq!(v["verdict"], "unsupported");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0"),
        "must reach the existing nondeterminism gate, not fail new syntax"
    );
    fn aborted(v: &Value) {
        if let Some(o) = v.as_object() {
            assert!(!o.contains_key("bound"), "abort retracts lower");
            if o.contains_key("min") {
                assert_eq!(
                    v,
                    &json!({"mean":null,"min":null,"max":null,"samples":0,"reason":"aborted"})
                );
            } else {
                for child in o.values() {
                    aborted(child);
                }
            }
        } else if let Some(a) = v.as_array() {
            for child in a {
                aborted(child);
            }
        }
    }
    if v["metrics"].is_object() {
        let m = &v["metrics"];
        aborted(&m["overall"]["queue_wait_observed"]);
        for a in m["actors"].as_array().unwrap() {
            aborted(&a["queue"]);
        }
        for c in m["cases"].as_array().unwrap() {
            assert!(c.get("dwell").is_none());
            aborted(&c["joins"]);
            for t in c["terminals"].as_array().unwrap() {
                aborted(&t["breakdown"]);
            }
            for t in c["tasks"].as_array().unwrap() {
                aborted(&t["wait_causes"]);
                aborted(&t["predecessor_wait"]);
            }
        }
    }
}

#[test]
fn t10_only_independently_enumerated_name_and_source_metadata_leaves_change() {
    let locations: Vec<Value> =
        serde_json::from_str(include_str!("support/waits_label_locations.json")).unwrap();
    assert!(locations.len() > 1);
    let mut original =
        serde_json::to_value(report::check(&source("wait-label.broken-original-labels"))).unwrap();
    let mut labelled = serde_json::to_value(report::check(&source("wait-label"))).unwrap();
    assert_eq!(original["verdict"], "satisfied");
    assert_eq!(labelled["verdict"], "satisfied");
    for location in locations {
        let path = location["path"].as_str().unwrap();
        for n in 0..2 {
            if path == format!("/actors/{n}/name") {
                let p = format!("/ledger/actor_versions/entries/{n}/actor");
                let a = original.pointer_mut(&p).unwrap();
                let b = labelled.pointer_mut(&p).unwrap();
                assert_eq!(*a, location["original"]);
                assert_eq!(*b, location["labelled"]);
                *a = Value::Null;
                *b = Value::Null;
            }
        }
        let a = original.pointer_mut(path).unwrap();
        let b = labelled.pointer_mut(path).unwrap();
        if location["kind"] == "source_sha256" {
            assert_eq!(path, "/model/sha256");
            assert_ne!(a, b);
            assert_eq!(a.as_str().unwrap().len(), 64);
            assert_eq!(b.as_str().unwrap().len(), 64);
        } else {
            assert_eq!(
                a, &location["original"],
                "{path}: independent original control value"
            );
            assert_eq!(
                b, &location["labelled"],
                "{path}: independent labelled control value"
            );
        }
        *a = Value::Null;
        *b = Value::Null;
    }
    assert_eq!(
        original, labelled,
        "all numbers, verdicts, non-name text and order must agree"
    );
}

#[test]
fn unreachable_fork_retains_lexical_join_inventory_with_never_created_reasons() {
    let s = source("wait-person");
    let observed = s.replace("type T", "observed case { flag: bool = false }\n type T");
    assert_ne!(observed, s);
    let source=observed.replace("process P { fork f { a = A.a(); b = B.b() }; join all f; end done }",
        "process P { if flag == true { fork f { a = A.a(); b = B.b() }; join all f } else { }; end done }");
    assert_ne!(source, observed);
    assert_eq!(
        report::check(&masked(&source)).verdict,
        relca_core::Verdict::Satisfied,
        "independent old grammar control"
    );
    let m = lower::lower(&syntax::parse(&source).expect("new opt-in syntax")).unwrap();
    let expected = json!([{"fork":"f","mode":"all","branches":[
        {"binding":"a","join_wait":{"min":null,"max":null,"reason":"never_created"}},
        {"binding":"b","join_wait":{"min":null,"max":null,"reason":"never_created"}}]}]);
    // flag=false reaches End at0; neither lexical branch was created. No time
    // is fabricated and the lexical fork/branch inventory is not erased.
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for metrics in [
        serde_json::to_value(e.metrics).unwrap(),
        serde_json::to_value(r.outcome.metrics).unwrap(),
    ] {
        assert_eq!(metrics["cases"][0]["joins"], expected);
        assert_eq!(
            metrics["overall"]["queue_wait_observed"],
            json!({"min":"0","max":"0","reason":null})
        );
    }
}

#[test]
fn horizon_unfinished_all_join_does_not_use_future_declared_stop() {
    let s = source("wait-person");
    let source=s.replace("bounds { cases 1; live_tasks 2; clock 10min }",
        "scenario Day { arrivals fixed [0min]; horizon 4min; bounds { cases 1; live_tasks 2; clock 10min } }");
    assert_ne!(source, s);
    assert_eq!(
        report::check_with_selection(&masked(&source), None, Some("Day")).verdict,
        relca_core::Verdict::Inconclusive
    );
    let m = lower::lower_with_selection(
        &syntax::parse(&source).expect("new opt-in syntax"),
        None,
        Some("Day"),
    )
    .unwrap();
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for metrics in [
        serde_json::to_value(e.metrics).unwrap(),
        serde_json::to_value(r.outcome.metrics).unwrap(),
    ] {
        let c = &metrics["cases"][0];
        for b in c["joins"][0]["branches"].as_array().unwrap() {
            assert_eq!(
                b["join_wait"],
                json!({"min":null,"max":null,"reason":"unfinished"})
            );
        }
        // Short task ends3, long task is still running at4. The observation
        // contains service[0,4), not a lead5 forecast or join2 computed at5.
        assert_eq!(
            c["dwell"]["until"],
            json!({"min":"4","max":"4","reason":null})
        );
        assert_eq!(
            c["dwell"]["breakdown"]["in_service"],
            json!({"min":"4","max":"4","reason":null})
        );
        assert_eq!(c["dwell"]["bound"], "lower");
    }
}

fn review_waiting_unchecked(diff_mode: bool) {
    let src = source("wait-compare-diff");
    let plain = masked(&src);
    let run = |s: &str| {
        if diff_mode {
            serde_json::to_value(diff::diff(s, "Left", "Right")).unwrap()
        } else {
            serde_json::to_value(compare::compare(
                s,
                &["Left", "Right"],
                compare::Analysis::Exhaustive,
            ))
            .unwrap()
        }
    };
    let old = run(&plain);
    let new = run(&src);
    for v in [&old, &new] {
        assert_eq!(v["verdict"], "satisfied");
    }
    let count = |v: &Value| {
        v["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| *x == "waiting_tables")
            .count()
    };
    assert_eq!(count(&old), 0, "masked control retains legacy unchecked");
    assert_eq!(
        count(&new),
        1,
        "declared waiting table exclusion must be named exactly once"
    );
    assert_eq!(
        new["metrics"], old["metrics"],
        "waiting quantities remain excluded from table"
    );
}

#[test]
fn review_compare_names_waiting_tables_once() {
    review_waiting_unchecked(false);
}
#[test]
fn review_diff_names_waiting_tables_once() {
    review_waiting_unchecked(true);
}

#[test]
fn review_nontrivial_duration_draws_preserve_both_interpreters_outcomes() {
    // Hand input: a single run, d=2 or4 with probability1/2 each; no queue,
    // predecessors or other intervals. Sampling is not exhaustive verification.
    let src = r#"model ReviewSample version "0.2" {
      type T = enum { ok }
      observed case { hit: bool = false }
      actor A { capacity 1; capability run() -> T { duration { 2min:0.5, 4min:0.5 }; result nondet { ok }; writes hit; finish { hit = true } } }
      process P { r = A.run(); end done }
      cut C { include [A] }
      policy fifo tie_by case_id
      scenario Day { arrivals fixed [0min]; simulation replications 4 seed 1 confidence 0.5; bounds { cases 1; live_tasks 1; clock 10min } }
      metrics { window 10min; currency "JPY"; breakdown waits }
      verify probability reached hit == true >= 0.5
    }"#;
    let model =
        lower::lower_with_selection(&syntax::parse(src).unwrap(), None, Some("Day")).unwrap();
    let mut plain = model.clone();
    plain.metrics.as_mut().unwrap().breakdown_waits = false;
    let e = engine::simulate(&model);
    let eoff = engine::simulate(&plain);
    let r = reference::simulate(&model);
    let roff = reference::simulate(&plain);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(roff.stop, reference::StopReason::Complete);
    let zero = json!({"min":"0","max":"0","reason":null});
    let mut sum = 0;
    for (label, on, off) in [
        ("engine", &e, &eoff),
        ("reference", &r.outcome, &roff.outcome),
    ] {
        assert_eq!(on.replications.len(), 4);
        // Nonempty draws alone could be a deterministic endpoint draw. Require
        // the actual first 1/2 hazard, with its independent exact key inventory.
        assert!(
            on.replications.iter().any(|rep| rep
                .draws
                .iter()
                .any(|d| d.key.ends_with("site=duration.P.r|n=1"))),
            "{label}: must observe a nontrivial duration draw"
        );
        for rep in &on.replications {
            let key = format!(
                "relca-sim-v1|seed=1|rep={}|case=1|site=duration.P.r|n=1",
                rep.replication
            );
            // Independently calculated SHA256 first bytes for reps1..4 of
            // these exact keys: 60,bb,ef,8b. Half/half selects finish2 when
            // k<2^63; otherwise finish4 is certain at the next endpoint.
            // No production sampler/hash/collector supplies this oracle.
            let first_byte = [0x60, 0xbb, 0xef, 0x8b][rep.replication as usize - 1];
            let d = if first_byte < 128 { 2 } else { 4 };
            if label == "engine" {
                sum += d;
            }
            assert_eq!(rep.draws[0].key, key);
            assert_eq!(rep.draws.len(), if d == 2 { 1 } else { 2 });
            if d == 4 {
                assert_eq!(rep.draws[1].key, key.replace("|n=1", "|n=2"));
            }
            assert_eq!(
                rep.cases[0].queries,
                vec![relca_core::outcome::QueryTruth::Hit]
            );
            assert!(rep.cut.is_none());
            let p = serde_json::to_value(rep.metrics.as_ref().unwrap()).unwrap();
            let c = &p["cases"][0];
            let amount = json!({"min":d.to_string(),"max":d.to_string(),"reason":null});
            assert_eq!(c["terminals"][0]["lead_time"], amount);
            let b = &c["terminals"][0]["breakdown"];
            assert_eq!(b["in_service"], amount);
            for k in ["external", "contest_route", "intentional", "unattributed"] {
                assert_eq!(b[k], zero);
            }
            for k in ["actor_capacity", "resource", "executor", "combined"] {
                assert_eq!(b["queue"][k], zero);
            }
            for k in [
                "actor_capacity",
                "resource",
                "executor",
                "combined",
                "unattributed",
            ] {
                assert_eq!(c["tasks"][0]["wait_causes"][k], zero);
            }
            assert_eq!(c["tasks"][0]["predecessor_wait"], zero);
            let q = &p["actors"][0]["queue"];
            assert_eq!(q["wait_total"]["stats"], zero);
            assert_eq!(q["length_max"], zero);
            assert_eq!(q["length_time_average"]["stats"], zero);
            assert_eq!(p["overall"]["queue_wait_observed"], zero);
        }
        let without_metrics = |v: &relca_core::outcome::SimulationOutcome| {
            let mut v = v.clone();
            v.metric_template = None;
            for rep in &mut v.replications {
                rep.metrics = None;
            }
            v
        };
        assert_eq!(
            without_metrics(on),
            without_metrics(off),
            "{label}: full non-metric Outcome, choices and keys invariant"
        );
        for (a, b) in on.replications.iter().zip(&off.replications) {
            let mut a = serde_json::to_value(&a.metrics).unwrap();
            erase_new_metrics(&mut a);
            assert_eq!(
                a,
                serde_json::to_value(&b.metrics).unwrap(),
                "{label}: legacy metrics invariant"
            );
        }
    }
    assert_eq!(
        e.replications, r.outcome.replications,
        "independent histories agree after each met the hand oracle"
    );
    // Four values from {2,4}: the sum is even, so only integer or half-integer
    // means are possible. Reduce the rational without depending on a reducer.
    let expected_mean = if sum % 4 == 0 {
        (sum / 4).to_string()
    } else {
        format!("{}/2", sum / 2)
    };
    let aggregate =
        serde_json::to_value(report::simulate_with_selection(src, None, Some("Day"))).unwrap();
    let service = &aggregate["metrics"]["cases"][0]["terminals"][0]["breakdown"]["in_service"];
    assert_eq!(service["mean"], expected_mean);
    assert_eq!(service["samples"], 4);
}
