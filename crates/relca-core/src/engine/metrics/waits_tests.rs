use super::*;
use crate::metrics::wait_oracles as oracle;
use crate::metrics::wait_test_support;
use serde_json::{Value, json};

fn model(same_actor: bool, renamed: bool) -> ir::Model {
    let mut source = String::from(
        "model Private version \"0.2\" { type T = enum { ok } actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } } actor B { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } } process P { fork f { a=A.run(); b=B.run() }; join all f; end done } bounds { cases 1; live_tasks 2; clock 20min } metrics { window 20min; currency \"JPY\"; breakdown waits } }",
    );
    if same_actor {
        source = source.replace("b=B.run()", "b=A.run()");
    }
    if renamed {
        source = source
            .replace("a=A.run()", "blocker=A.run()")
            .replace("b=A.run()", "target=A.run()");
    }
    source = source.replace(
        "metrics {",
        "verify safety no_resource_overcommit metrics {",
    );
    let mut m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    m.horizon = Some(10);
    m.scenario = Some(ir::ScenarioRecord {
        name: "Private".into(),
        span: m.process.span,
    });
    m
}

fn point(name: &str, input: &Value) -> (ir::Model, crate::metrics::BusinessMetrics) {
    let sample_ratio = name.starts_with("P07");
    let mut tasks = input["tasks"].as_array().cloned().unwrap_or_default();
    let until = input["T"]
        .as_u64()
        .or_else(|| input["end"].as_u64())
        .unwrap_or(3);
    if input.get("A_total").is_some() {
        tasks = vec![
            json!({"ready":0,"start":input["A_total"],"stop":until}),
            json!({"ready":0,"start":input["B_total"],"stop":until}),
        ];
    } else if let Some(total) = input["total"].as_u64() {
        tasks.push(json!({"ready":0,"start":total.min(until),"stop":until}));
        if total > until {
            tasks.push(json!({"ready":0,"start":total-until,"stop":until}));
        }
    } else if input.get("service").is_some() {
        tasks.push(json!({"ready":0,"start":0,"stop":until}));
    }
    let m = model(
        sample_ratio || name.starts_with("P03"),
        name.starts_with("P03"),
    );
    let mut state = super::super::State::initial(&m, 0);
    state.time = until;
    for (h, t) in state
        .metric_history
        .as_mut()
        .unwrap()
        .tasks
        .iter_mut()
        .zip(&tasks)
    {
        h.ready = t["ready"].as_u64();
        h.start = t["start"].as_u64();
        h.stop = t["stop"].as_u64();
        h.status = if t["status"] == "cancelled_before_start" {
            Status::CancelledBeforeStart
        } else if h.start.is_some() && h.stop.is_some() {
            Status::Finished
        } else {
            Status::Queued
        };
    }
    let is_record = input.get("kind").is_none();
    let terminal = if is_record {
        if input.get("end").is_some() {
            input["end"].as_u64().map(|_| ir::EndId(0))
        } else {
            Some(ir::EndId(0))
        }
    } else {
        input["end"].as_u64().map(|_| ir::EndId(0))
    };
    let horizon = input["cut_off"] == true || input["horizon_stopped"] == true;
    let o = CaseObservation {
        admitted_at: Some(input["admitted"].as_u64().unwrap_or(0)),
        state: Some(&state),
        arrival: 0,
        end: terminal,
        rejected: false,
        deadlocked: false,
        not_arrived: false,
    };
    let result = snapshot_with_horizon(&m, &[o], &Peak::new(&m), until, horizon);
    (m, result)
}

#[test]
fn engine_waits_private_p01_p08() {
    oracle::run(|name, input| {
        if input["kind"] == "history" {
            return wait_test_support::select(name, point(name, input).1);
        }
        let records = input["records"].as_array().unwrap();
        let mut acc = None;
        let mut model = None;
        let mut points = Vec::new();
        for r in records {
            let (m, p) = point(name, r);
            model = Some(m);
            if input["kind"] == "sample_reduce" {
                points.push(p);
            } else {
                merge(&mut acc, p);
            }
        }
        let out = if input["kind"] == "sample_reduce" {
            wait_test_support::sample(points, false)
        } else {
            let m = model.unwrap();
            finish(
                acc,
                &m,
                if input["coverage"] == "truncated" {
                    crate::obligation::Coverage::Truncated
                } else {
                    crate::obligation::Coverage::Complete
                },
            )
            .unwrap_or_else(|| panic!("{name}: {:?}", super::super::validate(&m)))
        };
        wait_test_support::select(name, out)
    });
}

#[test]
fn engine_waits_identity_and_forged_inventory() {
    use std::hash::{Hash, Hasher};
    let m = model(false, false);
    let mut no_assumption = m.clone();
    no_assumption
        .assumptions
        .retain(|a| a.field != "metrics.waiting");
    assert!(super::super::explore(&no_assumption).metrics.is_some());
    let mut plain = m.clone();
    plain.metrics.as_mut().unwrap().breakdown_waits = false;
    let a = super::super::State::initial(&m, 0);
    let b = super::super::State::initial(&plain, 0);
    assert_eq!(a, b);
    let hash = |s: &super::super::State| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(&a), hash(&b));
    for budget in 0..40 {
        let mut a = super::super::explore_with_budget(&m, budget);
        let mut b = super::super::explore_with_budget(&plain, budget);
        a.metrics = None;
        b.metrics = None;
        assert_eq!(a, b, "budget{budget}");
    }
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
                    branches[0].binding = ir::BindingId(usize::MAX);
                }
            }
            _ => unreachable!(),
        }
        let out = super::super::explore(&bad);
        assert!(out.metrics.is_none(), "fault{fault}");
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.code == crate::diag::U_SEM_0),
            "fault{fault}"
        );
    }
}

#[test]
fn engine_waits_sample_prefix_and_aborted_withdrawal() {
    let (_, observed) = point(
        "P05",
        &json!({"end":null,"T":3,"service":3,"cut_off":false}),
    );
    let mut prefix = wait_test_support::sample(vec![observed], false);
    let raw = serde_json::to_value(&prefix).unwrap();
    assert_eq!(raw["cases"][0]["tasks"][0]["predecessor_wait"]["mean"], "0");
    assert_eq!(
        raw["cases"][0]["tasks"][0]["wait_causes"]["actor_capacity"]["mean"],
        "0"
    );
    assert!(raw["cases"][0].get("dwell").is_none());
    // Actual horizon record, then abort: no conditional dwell/lower survives.
    let (_, cut) = point("P05", &json!({"end":null,"T":3,"service":3,"cut_off":true}));
    prefix = wait_test_support::sample(vec![cut], true);
    let raw = serde_json::to_value(prefix).unwrap();
    assert!(raw["cases"][0].get("dwell").is_none());
    assert!(
        raw["actors"][0]["queue"]["wait_total"]
            .get("bound")
            .is_none()
    );
    assert_eq!(
        raw["cases"][0]["tasks"][0]["predecessor_wait"]["reason"],
        "aborted"
    );
}

/// Full lexical inventory, with a distinct History for every input case. In
/// particular, the completed cross-case blocker is not dropped at the snapshot.
fn review_history(name: &str, input: &Value) -> Value {
    let cases = input["cases"].as_array().unwrap();
    let mut source = String::from("model EngineReview version \"0.2\" { type T = enum { ok } ");
    if let Some(people) = input["people"].as_array() {
        source.push_str(&format!(
            "people [{}] ",
            people
                .iter()
                .map(|p| p.as_str().unwrap())
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    for actor in input["actors"].as_array().unwrap() {
        source.push_str(&format!(
            "actor {} {{ capacity {}; ",
            actor["name"].as_str().unwrap(),
            actor["capacity"]
        ));
        if let Some(people) = actor["executors"].as_array() {
            source.push_str(&format!(
                "executors [{}]; ",
                people
                    .iter()
                    .map(|p| p.as_str().unwrap())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        source.push_str("capability run() -> T { duration fixed 1min; result nondet { ok } } } ");
    }
    source.push_str("process P { ");
    let mut bindings = Vec::new();
    for case in cases {
        for task in case["tasks"].as_array().unwrap() {
            let binding = task["binding"].as_str().unwrap();
            if !bindings.contains(&binding) {
                bindings.push(binding);
                source.push_str(&format!(
                    "{}={}.run(); ",
                    binding,
                    task["actor"].as_str().unwrap()
                ));
            }
        }
    }
    if cases.iter().any(|c| c["waits"].is_array()) {
        // Enable the ordinary raw wait sum; actual intervals come from History.
        source.push_str("wait 1min; ");
    }
    source.push_str(&format!("end done }} arrivals fixed [{}] policy fifo tie_by case_id bounds {{ cases {}; live_tasks {}; clock 20min }} metrics {{ window 20min; currency \"JPY\"; breakdown waits }} verify safety no_resource_overcommit }}",
        cases.iter().map(|c| format!("{}min", c["arrival"])).collect::<Vec<_>>().join(","), cases.len(), bindings.len()));
    let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    assert!(super::super::validate(&m).is_ok());
    let until = input["T"].as_u64().unwrap();
    let states: Vec<_> = cases
        .iter()
        .map(|case| {
            let mut state = super::super::State::initial(&m, 0);
            state.time = case["end"].as_u64().unwrap();
            let history = state.metric_history.as_mut().unwrap();
            history.waits = case["waits"].as_array().map(|waits| {
                waits
                    .iter()
                    .map(|w| (w[0].as_u64().unwrap(), w[1].as_u64().unwrap()))
                    .collect()
            });
            for task in case["tasks"].as_array().unwrap() {
                let id = m
                    .process
                    .bindings
                    .iter()
                    .position(|b| b.name == task["binding"])
                    .unwrap();
                let h = &mut history.tasks[id];
                h.ready = task["ready"].as_u64();
                h.start = task["start"].as_u64();
                h.stop = task["stop"].as_u64();
                h.executor = task["executor"].as_str().map(|person| {
                    ir::PersonId(m.people.iter().position(|p| p.name == person).unwrap())
                });
                h.status = Status::Finished;
            }
            state
        })
        .collect();
    let observations: Vec<_> = cases
        .iter()
        .zip(&states)
        .map(|(case, state)| CaseObservation {
            admitted_at: case["admitted"].as_u64(),
            state: Some(state),
            arrival: case["arrival"].as_u64().unwrap(),
            end: Some(ir::EndId(0)),
            rejected: false,
            deadlocked: false,
            not_arrived: false,
        })
        .collect();
    let collect = || snapshot(&m, &observations, &Peak::new(&m), until);
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
        let text = include_str!("../../../../../fixtures/wait-person/model.relca");
        let m = crate::lower::lower(&crate::syntax::parse(text).unwrap()).unwrap();
        let point = super::super::explore(&m).metrics.unwrap();
        wait_test_support::review_malformed(input, point)
    } else {
        review_history(name, input)
    }
}

#[test]
fn engine_waits_review_p09_p12() {
    oracle::run_review(review_observe);
}

#[test]
fn engine_waits_review_p09() {
    wait_test_support::review_family("P09", review_observe);
}
#[test]
fn engine_waits_review_p10() {
    wait_test_support::review_family("P10", review_observe);
}
#[test]
fn engine_waits_review_p11() {
    wait_test_support::review_family("P11", review_observe);
}
#[test]
fn engine_waits_review_p12() {
    wait_test_support::review_family("P12", review_observe);
}

#[test]
fn engine_waits_mutation_p13() {
    oracle::run_mutation(review_history);
}
