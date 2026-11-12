use super::*;
use crate::metrics::wait_oracles as oracle;
use crate::metrics::wait_test_support;
use serde_json::{Value, json};

fn model(shared: bool, target: bool) -> ir::Model {
    let owner = if shared { "A" } else { "B" };
    let (a, b) = if target {
        ("blocker", "target")
    } else {
        ("a", "b")
    };
    let text = format!(
        "model RefPrivate version \"0.2\" {{ type T = enum {{ ok }} actor A {{ capacity 1; capability run() -> T {{ duration fixed 1min; result nondet {{ ok }} }} }} actor B {{ capacity 1; capability run() -> T {{ duration fixed 1min; result nondet {{ ok }} }} }} process P {{ fork f {{ {a}=A.run(); {b}={owner}.run() }}; join all f; end done }} bounds {{ cases 1; live_tasks 2; clock 20min }} metrics {{ window 20min; currency \"JPY\"; breakdown waits }} }}"
    );
    let text = text.replace(
        "metrics {",
        "verify safety no_resource_overcommit metrics {",
    );
    let mut m = crate::lower::lower(&crate::syntax::parse(&text).unwrap()).unwrap();
    m.horizon = Some(10);
    m.scenario = Some(ir::ScenarioRecord {
        name: "Private".into(),
        span: m.process.span,
    });
    m
}

fn project_record(name: &str, record: &Value) -> w::BusinessMetrics {
    let until = record["T"]
        .as_u64()
        .or_else(|| record["end"].as_u64())
        .unwrap_or(3);
    let m = model(
        name.starts_with("P03") || name.starts_with("P07"),
        name.starts_with("P03"),
    );
    let mut histories = record["tasks"].as_array().cloned().unwrap_or_default();
    if record.get("A_total").is_some() {
        histories = vec![
            json!({"ready":0,"start":record["A_total"],"stop":until}),
            json!({"ready":0,"start":record["B_total"],"stop":until}),
        ];
    } else if let Some(total) = record["total"].as_u64() {
        histories.push(json!({"ready":0,"start":total.min(until),"stop":until}));
        if total > until {
            histories.push(json!({"ready":0,"start":total-until,"stop":until}));
        }
    } else if record.get("service").is_some() {
        histories.push(json!({"ready":0,"start":0,"stop":until}));
    }
    let mut calls = Vec::new();
    let mut ends = Vec::new();
    inventory(&m.process.body, &mut calls, &mut ends);
    let tasks: Vec<_> = histories
        .iter()
        .zip(calls)
        .map(|(h, c)| Task {
            frozen_performance: None,
            stale: Default::default(),
            metric_ready: h["ready"].as_u64(),
            checked_through: None,
            person: None,
            call: c.clone(),
            phase: if h["status"] == "cancelled_before_start" {
                Phase::Cancelled
            } else if h["start"].is_number() {
                Phase::Finished(ir::ValueId(0))
            } else {
                Phase::Queued
            },
            start: h["start"].as_u64(),
            stop: h["stop"].as_u64(),
            finish: None,
            ack: None,
        })
        .collect();
    let has_end = if record["kind"] == "history" || record.get("end").is_some() {
        record["end"].is_number()
    } else {
        true
    };
    let observations = [Observation {
        condition_intervals: Vec::new(),
        admitted_at: Some(record["admitted"].as_u64().unwrap_or(0)),
        wait_intervals: vec![],
        intentional_wait: 0,
        tasks: &tasks,
        arrival: 0,
        terminal: has_end.then_some((ir::EndId(0), until)),
        rejected: false,
        deadlock: false,
        future: false,
    }];
    project_with_horizon(
        &m,
        &observations,
        until,
        record["cut_off"] == true || record["horizon_stopped"] == true,
    )
}

#[test]
fn reference_waits_private_p01_p08() {
    oracle::run(|name, input| {
        if input["kind"] == "history" {
            return wait_test_support::select(name, project_record(name, input));
        }
        let mut envelope = Envelope::default();
        let mut samples = Vec::new();
        for record in input["records"].as_array().unwrap() {
            let point = project_record(name, record);
            if input["kind"] == "sample_reduce" {
                samples.push(point);
            } else {
                envelope.add(point);
            }
        }
        let value = if input["kind"] == "sample_reduce" {
            wait_test_support::sample(samples, false)
        } else {
            envelope
                .finish(input["coverage"].as_str().unwrap_or("complete"))
                .unwrap()
        };
        wait_test_support::select(name, value)
    });
}

#[test]
fn reference_waits_forged_inventory() {
    let m = model(false, false);
    let mut no_assumption = m.clone();
    no_assumption
        .assumptions
        .retain(|a| a.field != "metrics.waiting");
    assert!(
        crate::reference::explore(&no_assumption)
            .outcome
            .metrics
            .is_some()
    );
    assert!(crate::reference::explore(&m).outcome.metrics.is_some());
    for fault in 0..3 {
        let mut bad = m.clone();
        match fault {
            0 => bad.capabilities[0].uses.push(ir::ResourceUse {
                resource: ir::ResourceId(usize::MAX),
                amount: 1,
                span: bad.process.span,
            }),
            1 => bad.actors[0].executors = Some(vec![ir::PersonId(usize::MAX)]),
            2 => {
                if let ir::Stmt::Fork { branches, .. } = &mut bad.process.body[0].kind {
                    branches[1].binding = ir::BindingId(usize::MAX);
                }
            }
            _ => unreachable!(),
        }
        let result = crate::reference::explore(&bad);
        assert_eq!(
            result.stop,
            crate::reference::StopReason::SemanticAbort,
            "fault{fault}"
        );
        assert!(result.outcome.metrics.is_none(), "fault{fault}");
    }
}

#[test]
fn reference_waits_sample_prefix_and_abort() {
    let prefix = project_record(
        "P05",
        &json!({"end":null,"T":3,"service":3,"cut_off":false}),
    );
    let raw = serde_json::to_value(wait_test_support::sample(vec![prefix], false)).unwrap();
    assert_eq!(raw["cases"][0]["tasks"][0]["predecessor_wait"]["mean"], "0");
    assert_eq!(
        raw["cases"][0]["tasks"][1]["wait_causes"]["actor_capacity"]["reason"],
        "never_created"
    );
    assert!(raw["cases"][0].get("dwell").is_none());
    let cutoff = project_record("P05", &json!({"end":null,"T":3,"service":3,"cut_off":true}));
    let raw = serde_json::to_value(wait_test_support::sample(vec![cutoff], true)).unwrap();
    assert!(raw["cases"][0].get("dwell").is_none());
    assert!(
        raw["actors"][0]["queue"]["wait_total"]
            .get("bound")
            .is_none()
    );
    assert_eq!(raw["overall"]["queue_wait_observed"]["reason"], "aborted");
}

fn review_history(name: &str, input: &Value) -> Value {
    let records = input["cases"].as_array().unwrap();
    let mut text = String::from("model ReferenceReview version \"0.2\" { type T=enum{ok} ");
    if let Some(people) = input["people"].as_array() {
        text.push_str("people [");
        text.push_str(
            &people
                .iter()
                .map(|p| p.as_str().unwrap())
                .collect::<Vec<_>>()
                .join(","),
        );
        text.push_str("] ");
    }
    for a in input["actors"].as_array().unwrap() {
        text.push_str(&format!(
            "actor {} {{ capacity {}; ",
            a["name"].as_str().unwrap(),
            a["capacity"]
        ));
        if let Some(executors) = a["executors"].as_array() {
            text.push_str(&format!(
                "executors [{}]; ",
                executors
                    .iter()
                    .map(|p| p.as_str().unwrap())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        text.push_str("capability run()->T {duration fixed 1min; result nondet{ok}} } ");
    }
    let mut declarations = std::collections::BTreeMap::new();
    for r in records {
        for t in r["tasks"].as_array().unwrap() {
            declarations.insert(t["binding"].as_str().unwrap(), t["actor"].as_str().unwrap());
        }
    }
    text.push_str("process P { ");
    for (binding, actor) in &declarations {
        text.push_str(&format!("{binding}={actor}.run(); "));
    }
    if records.iter().any(|r| r["waits"].is_array()) {
        // The trace supplies the actual intervals, not this inventory marker.
        text.push_str("wait 1min; ");
    }
    text.push_str(&format!("end done }} arrivals fixed [{}] policy fifo tie_by case_id bounds {{ cases {}; live_tasks {}; clock 20min }} metrics {{window 20min; currency \"JPY\"; breakdown waits}} verify safety no_resource_overcommit }}",
        records.iter().map(|r| format!("{}min", r["arrival"])).collect::<Vec<_>>().join(","), records.len(), declarations.len()));
    let m = crate::lower::lower(&crate::syntax::parse(&text).unwrap()).unwrap();
    assert!(validate(&m));
    let mut calls = Vec::new();
    let mut ends = Vec::new();
    inventory(&m.process.body, &mut calls, &mut ends);
    // An absent lexical binding is represented by no reference Task, so the
    // real projector emits never_created, not a forged zero-valued history.
    let tasks: Vec<Vec<_>> = records
        .iter()
        .map(|record| {
            record["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| {
                    let call = calls
                        .iter()
                        .find(|c| m.process.bindings[c.binding.0].name == t["binding"])
                        .unwrap();
                    Task {
                        frozen_performance: None,
                        call: (*call).clone(),
                        metric_ready: t["ready"].as_u64(),
                        start: t["start"].as_u64(),
                        stop: t["stop"].as_u64(),
                        person: t["executor"].as_str().map(|person| {
                            ir::PersonId(m.people.iter().position(|p| p.name == person).unwrap())
                        }),
                        phase: Phase::Finished(ir::ValueId(0)),
                        stale: Default::default(),
                        checked_through: None,
                        finish: None,
                        ack: None,
                    }
                })
                .collect()
        })
        .collect();
    let observations: Vec<_> = records
        .iter()
        .zip(&tasks)
        .map(|(r, tasks)| {
            let wait_intervals: Vec<_> = r["waits"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|w| (w[0].as_u64().unwrap(), w[1].as_u64().unwrap()))
                .collect();
            let until = input["T"].as_u64().unwrap();
            // Raw clipped sum, deliberately not the exclusive intentional part.
            let intentional_wait = wait_intervals
                .iter()
                .map(|&(begin, end)| end.min(until).saturating_sub(begin))
                .sum();
            Observation {
                condition_intervals: Vec::new(),
                tasks,
                admitted_at: r["admitted"].as_u64(),
                wait_intervals,
                intentional_wait,
                arrival: r["arrival"].as_u64().unwrap(),
                terminal: Some((ir::EndId(0), r["end"].as_u64().unwrap())),
                rejected: false,
                deadlock: false,
                future: false,
            }
        })
        .collect();
    let collect = || project(&m, &observations, input["T"].as_u64().unwrap());
    if name.starts_with("P11") {
        return wait_test_support::rejected(
            || {
                let point = collect();
                assert_eq!(
                    serde_json::to_value(&point.cases[0].tasks[0].predecessor_wait).unwrap(),
                    json!({"min":"0","max":"0","reason":null})
                );
            },
            "metric invariant: Task.ready precedes admission",
        );
    }
    if name.starts_with("P13") {
        serde_json::to_value(collect()).unwrap()["cases"][0]["terminals"][0]["breakdown"].clone()
    } else {
        wait_test_support::review_select(name, collect())
    }
}

fn review_observe(name: &str, input: &Value) -> Value {
    if input["kind"] == "review_malformed_point" {
        let source = include_str!("../../../../../fixtures/wait-person/model.relca");
        let model = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        let point = crate::reference::explore(&model).outcome.metrics.unwrap();
        wait_test_support::review_malformed(input, point)
    } else {
        review_history(name, input)
    }
}

#[test]
fn reference_waits_review_p09_p12() {
    oracle::run_review(review_observe);
}

#[test]
fn reference_waits_review_p09() {
    wait_test_support::review_family("P09", review_observe);
}
#[test]
fn reference_waits_review_p10() {
    wait_test_support::review_family("P10", review_observe);
}
#[test]
fn reference_waits_review_p11() {
    wait_test_support::review_family("P11", review_observe);
}
#[test]
fn reference_waits_review_p12() {
    wait_test_support::review_family("P12", review_observe);
}

#[test]
fn reference_waits_mutation_p13() {
    oracle::run_mutation(review_history);
}
