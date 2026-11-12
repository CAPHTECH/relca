//! Private connection to the fixed, independently authored Q6a acceptance.
#[path = "../tests/support/q6a_oracle.rs"]
pub(crate) mod hand;
#[path = "../tests/support/q6a_inputs.rs"]
pub(crate) mod input;
#[path = "../tests/support/q6a_private.rs"]
pub(crate) mod private;

#[derive(Default)]
pub(crate) struct Observations {
    pub events: Vec<(u64, String, &'static str)>,
    pub draws: Vec<(u64, String)>,
    pub replayed: Vec<Option<crate::ir::FrozenPerformance>>,
}
thread_local! {
    static OBSERVATIONS: std::cell::RefCell<Option<Observations>> = const { std::cell::RefCell::new(None) };
}
pub(crate) fn event(
    at: u64,
    m: &crate::ir::Model,
    cap: crate::ir::CapabilityId,
    kind: &'static str,
) {
    OBSERVATIONS.with_borrow_mut(|slot| {
        if let Some(o) = slot {
            let c = &m.capabilities[cap.0];
            o.events
                .push((at, format!("{}.{}", m.actors[c.actor.0].name, c.name), kind));
        }
    });
}
pub(crate) fn draw(at: u64, site: String) {
    OBSERVATIONS.with_borrow_mut(|slot| {
        if let Some(o) = slot {
            o.draws.push((at, site));
        }
    });
}

pub(crate) fn begin_observations() {
    OBSERVATIONS.with_borrow_mut(|s| *s = Some(Observations::default()));
}
pub(crate) fn take_observations() -> Observations {
    OBSERVATIONS
        .with_borrow_mut(Option::take)
        .expect("observer is active")
}
pub(crate) fn replayed(selector: Option<crate::ir::FrozenPerformance>) {
    OBSERVATIONS.with_borrow_mut(|s| {
        if let Some(s) = s {
            s.replayed.push(selector)
        }
    });
}
pub(crate) fn corrupt(selector: &mut Option<crate::ir::FrozenPerformance>, fault: &str) {
    match fault {
        "missing" => *selector = None,
        "profile_out_of_range" => selector.as_mut().unwrap().profile = usize::MAX,
        "executor_out_of_range" => {
            selector.as_mut().unwrap().executor = Some(crate::ir::PersonId(usize::MAX))
        }
        "band_out_of_range" => selector.as_mut().unwrap().row = usize::MAX,
        _ => panic!("unknown selector fault"),
    }
}
pub(crate) fn private_source(name: &str) -> String {
    input::append(
        &input::source(name).replace("duration fixed 4min;", "commits; duration fixed 4min;"),
        "contract PrivateReplay { invariant Desk.run.commit -> r == denied }",
    )
}
fn replay_material(
    name: &str,
) -> (
    crate::ir::Model,
    crate::explain::Counterexample,
    Vec<Option<crate::explain::KernelChoice>>,
) {
    let source = private_source(name);
    let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    let explained = crate::explain::explain(&source);
    let counter = explained
        .explanation
        .counterexamples
        .into_iter()
        .find(|c| c.id.ends_with(" invariant"))
        .expect("finish-triggered invariant witness");
    assert_eq!(
        counter.replay.status,
        crate::explain::ReplayStatus::Verified
    );
    let (_, capture) = crate::engine::trace::explore(&source, &m);
    let choices = counter
        .steps
        .iter()
        .map(|step| {
            capture.edges.iter().flatten().find_map(|edge| {
                (edge.stage.kind == step.kind
                    && edge.stage.time == step.time
                    && edge.stage.case_id == step.case_id
                    && edge.stage.binding == step.binding
                    && edge.stage.result == step.result)
                    .then(|| edge.choice.clone())
                    .flatten()
            })
        })
        .collect::<Vec<_>>();
    assert!(choices.last().is_some_and(Option::is_some));
    (m, counter, choices)
}
pub(crate) fn finish_material() -> (crate::explain::Stage, crate::explain::KernelChoice) {
    let (_, counter, choices) = replay_material("single");
    (
        counter.steps.last().unwrap().clone(),
        choices.last().unwrap().clone().unwrap(),
    )
}
pub(crate) fn replay_selector(
    name: &str,
    fault: Option<&str>,
) -> Result<Option<crate::ir::FrozenPerformance>, crate::explain::ReplayReason> {
    let (m, counter, mut choices) = replay_material(name);
    if let Some(fault) = fault {
        corrupt(
            &mut choices
                .last_mut()
                .unwrap()
                .as_mut()
                .unwrap()
                .frozen_performance,
            fault,
        );
    }
    begin_observations();
    crate::reference::replay::replay_choices(
        &m,
        &counter.steps,
        &counter.id,
        counter.case_id,
        &choices,
    )?;
    Ok(OBSERVATIONS.with_borrow(|s| {
        s.as_ref()
            .unwrap()
            .replayed
            .last()
            .cloned()
            .expect("actual replayed Finish")
    }))
}

#[test]
fn selector_identity() {
    private::selector_identity(|side| match side {
        "engine" => crate::engine::q6_observe::identity(),
        "reference" => crate::reference::q6_observe::identity(),
        _ => panic!(),
    });
}
#[test]
fn backlog_projection() {
    private::backlog_projection(|side, name| match side {
        "engine" => crate::engine::q6_observe::backlog(name),
        "reference" => crate::reference::q6_observe::backlog(name),
        _ => panic!(),
    });
}
#[test]
fn selector_corruption() {
    private::selector_corruption(|side, mode, fault| match side {
        "engine" => crate::engine::q6_observe::corruption(mode, fault),
        "reference" => crate::reference::q6_observe::corruption(mode, fault),
        _ => panic!(),
    });
}

#[test]
fn q6_contract_public_ir_keeps_declaration_and_bound_kernel_consistent() {
    for fault in ["probability", "duplicate_name", "input_contract"] {
        let mut m =
            crate::lower::lower(&crate::syntax::parse(&input::source("people")).unwrap()).unwrap();
        match fault {
            "probability" => {
                let k = &mut m.capabilities[0].performance.as_mut().unwrap().kernels[0];
                k.rows[0].branches[0].probability =
                    crate::probability::Rational::decimal("0.8").unwrap();
                k.rows[0].branches[1].probability =
                    crate::probability::Rational::decimal("0.2").unwrap();
            }
            "duplicate_name" => m.kernels.push(m.kernels[0].clone()),
            "input_contract" => {
                let mut cap = m.capabilities[0].clone();
                cap.name = "other".into();
                cap.params.push(crate::ir::Parameter {
                    name: "x".into(),
                    ty: cap.return_type,
                    span: cap.span,
                });
                m.capabilities.push(cap);
            }
            _ => panic!(),
        }
        let e = crate::engine::explore(&m);
        let r = crate::reference::explore(&m);
        assert!(
            e.diagnostics
                .iter()
                .any(|d| d.message.contains("invalid_q6_ir")),
            "{fault}: {:?}",
            e.diagnostics
        );
        assert_eq!(
            r.stop,
            crate::reference::StopReason::SemanticAbort,
            "{fault}"
        );
    }
}

#[test]
fn calendar_close_does_not_change_the_frozen_performance_kernel() {
    use serde_json::json;
    let source = input::edit(
        &input::source("single"),
        "capacity 1",
        "capacity 1; calendar Late",
    );
    let source = input::append(
        &source,
        "calendar Late through 21min on_close continue { [0min,3min) capacity 0; [3min,4min) capacity 1; [4min,21min) capacity 0 }",
    );
    let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    let reference = crate::reference::explore(&m);
    assert_eq!(reference.stop, crate::reference::StopReason::Complete);
    // One task: start3, close4, finish3+4=7, work4, q=0 so bad=1/10.
    for (side, o) in [
        ("engine", crate::engine::explore(&m)),
        ("reference", reference.outcome),
    ] {
        assert!(!o.truncated, "{side}: {:?}", o.diagnostics);
        let t = &o.case_outcomes.as_ref().unwrap()[0].terminals[0];
        assert_eq!(
            t.completed_at,
            Some(crate::outcome::Range { min: 7, max: 7 })
        );
        assert_eq!(t.work, Some(crate::outcome::Range { min: 4, max: 4 }));
        let q = json!(o.quality);
        let p = hand::by(&q["properties"], "name", &json!("Bad")).unwrap();
        hand::subset(&hand::exact("1/10"), &p["expected_count"], side).unwrap();
    }
}

#[test]
fn priority_and_calendar_order_the_actual_performance_backlog() {
    use serde_json::json;
    for reverse in [false, true] {
        let source = input::source(if reverse {
            "priority-reverse"
        } else {
            "priority"
        })
        .replace("type R = enum { ok }", "type R = enum { ok, bad }")
        .replace("result nondet { ok }", "outcomes performance P; writes bad");
        let source = input::append(
            &source,
            &format!(
                "latent case {{ bad: bool = false }} {} performance P basis \"fixture assumption; not measured\" {{ load queued_same_actor_at_start; executor q {{ [0,1): kernel Low; [1,infinity): kernel Half }} }} property case.Bad = reached(bad == true) metric cohort.BadPerArrival = expected(count(case.Bad)) / arrivals",
                input::KERNELS
            ),
        );
        let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
        let r = crate::reference::explore(&m);
        assert_eq!(r.stop, crate::reference::StopReason::Complete);
        // Both are queued before q's shift opens3. High rank starts3 at q=1
        // (bad1/2), low rank starts5 at q=0 (bad1/10). E=3/5; ratio=3/10.
        for (side, o) in [
            ("engine", crate::engine::explore(&m)),
            ("reference", r.outcome),
        ] {
            assert!(!o.truncated, "{side}: {:?}", o.diagnostics);
            let q = json!(o.quality);
            let p = hand::by(&q["properties"], "name", &json!("Bad")).unwrap();
            hand::subset(&hand::exact("3/5"), &p["expected_count"], side).unwrap();
            let ratio = hand::by(&q["cohort"], "name", &json!("BadPerArrival")).unwrap();
            hand::subset(&hand::exact("3/10"), &ratio["value"], side).unwrap();
            for case in 0..2 {
                let high = case == usize::from(!reverse);
                let probability = if high { "1/2" } else { "1/10" };
                let finish = if high { 5 } else { 7 };
                let terminal = &o.case_outcomes.as_ref().unwrap()[case].terminals[0];
                assert_eq!(
                    terminal.completed_at,
                    Some(crate::outcome::Range {
                        min: finish,
                        max: finish
                    })
                );
                let row = hand::by(&p["cases"], "case_id", &json!(case + 1)).unwrap();
                hand::subset(
                    &json!({"min":probability,"max":probability,"reason":null}),
                    row,
                    side,
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn draw_lifecycle() {
    use serde_json::json;
    private::draw_lifecycle(|side, case| {
        let mut source = if case == "performance_refused" {
            input::source("refused")
        } else {
            input::source("single")
        };
        if case.ends_with("cancel") {
            source = source.replace("duration fixed 4min;", "cancel cooperative ack_within 1min; duration fixed 4min;")
                .replace("process Flow { r = Desk.run(); end done }", "process Flow { fork f { r = Desk.run(); fast = Fast.win() }; join any f where result == ok tie by [fast,r] remaining cancel_then_drain if none -> failed; if fast == ok { end done } else { end failed } }")
                .replace("include [Desk]", "include [Desk,Fast]");
            source = input::append(
                &source,
                "actor Fast { control policy; capacity 1; capability win() -> R { cancel forbidden_after_start; duration fixed 1min; result nondet { ok } } }",
            );
            // Force actual ack bundles to remain observable; without any
            // obligations the interpreters legitimately elide those bundles.
            source = input::append(
                &source,
                "contract Monitor { response monitoring: case.received -> one_of(end.done) by 20min after case.received otherwise unresolved }",
            );
        }
        if case == "ordinary_cancel" {
            source = input::substrate(&source);
        }
        source = source.replace("simulation replications 16", "simulation replications 1");
        let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
        OBSERVATIONS.with_borrow_mut(|slot| *slot = Some(Observations::default()));
        let o = if side == "engine" {
            crate::engine::simulate(&m)
        } else {
            crate::reference::simulate(&m).outcome
        };
        let observed = OBSERVATIONS.with_borrow_mut(Option::take).unwrap();
        assert_eq!(
            o.completed_replications, 1,
            "{side}/{case}: {:?}",
            o.diagnostics
        );
        let events: Vec<_> = observed
            .events
            .iter()
            .filter(|(_, cap, _)| cap == "Desk.run")
            .collect();
        let starts: Vec<_> = events
            .iter()
            .filter(|(_, _, kind)| *kind == "start")
            .collect();
        let finishes = events
            .iter()
            .filter(|(_, _, kind)| *kind == "finish")
            .count();
        let acks = events.iter().filter(|(_, _, kind)| *kind == "ack").count();
        let draws = o.replications[0]
            .draws
            .iter()
            .filter(|d| d.key.contains("site=kernel.Flow.r|"))
            .count();
        let early = observed
            .draws
            .iter()
            .filter(|(at, site)| {
                site == "kernel.Flow.r" && starts.iter().any(|start| start.0 == *at)
            })
            .count();
        let work: u64 = starts
            .iter()
            .map(|start| {
                events
                    .iter()
                    .find(|(at, _, kind)| *at >= start.0 && matches!(*kind, "finish" | "ack"))
                    .expect("started Task closes")
                    .0
                    - start.0
            })
            .sum();
        json!({"starts":starts.len(),"normal_finishes":finishes,"cancel_acks":acks,"kernel_draws":draws,"draws_at_start":early,"work":work})
    });
}

#[test]
fn public_ir() {
    use crate::{engine, ir, lower, reference, syntax};
    use serde_json::json;
    private::public_ir(|source, fault, mode| {
        let mut m = lower::lower(&syntax::parse(source).unwrap()).unwrap();
        let cap = m
            .capabilities
            .iter()
            .position(|c| c.performance.is_some())
            .unwrap();
        let profile = m.capabilities[cap].performance.as_ref().unwrap().profile;
        match fault {
            "qualified_add_outside_actor" => {
                let id = ir::PersonId(m.people.len());
                m.people.push(ir::Person {
                    name: "outside".into(),
                    span: m.capabilities[cap].span,
                });
                m.capabilities[cap]
                    .qualified_executors
                    .as_mut()
                    .unwrap()
                    .push(id);
            }
            "profile_missing_executor" => {
                m.performances[profile].rows.pop();
            }
            "band_gap" => {
                m.performances[profile].rows[1].from += 1;
            }
            "kernel_id_out_of_range" => {
                m.performances[profile].rows[0].kernel = usize::MAX;
            }
            "ordinary_and_performance_sources" => {
                m.capabilities[cap].kernel =
                    Some(m.capabilities[cap].performance.as_ref().unwrap().kernels[0].clone());
            }
            _ => unreachable!(),
        }
        let (ds, et, stop, rt) = if mode == "explore" {
            let e = engine::explore(&m);
            let r = reference::explore(&m);
            (
                e.diagnostics,
                e.terminals.len(),
                r.stop,
                r.outcome.terminals.len(),
            )
        } else {
            let e = engine::simulate(&m);
            let r = reference::simulate(&m);
            let terminals = |o: &crate::outcome::SimulationOutcome| {
                o.replications
                    .iter()
                    .flat_map(|r| &r.cases)
                    .filter(|c| c.terminal.is_some())
                    .count()
            };
            let et = terminals(&e);
            let rt = terminals(&r.outcome);
            (e.diagnostics, et, r.stop, rt)
        };
        let d = ds.iter().find(|d| d.code == "U-SEM-0").unwrap();
        json!({"engine":{"code":d.code,"reason":d.message.split(':').next().unwrap(),"terminals":et},
            "reference":{"stop":format!("{stop:?}"),"terminals":rt}})
    });
}
