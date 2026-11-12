use super::*;

#[test]
fn state_invariant_all_global_handoffs_keep_received_monitors_and_inactive_slots_separate() {
    let mut phases = BTreeSet::new();
    let mut labels = BTreeSet::new();
    for name in [
        "sinv-shared",
        "sinv-ended-clean.broken-increment",
        "sinv-rejected",
        "sinv-rejected.broken-zero-queue",
        "sinv-future",
        "sinv-seed-mass",
    ] {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures")
                .join(name)
                .join("model.relca"),
        )
        .unwrap();
        let mut m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        if m.arrivals.is_none() {
            m.arrivals = Some(vec![0]);
        }
        let times = m.arrivals.as_ref().unwrap();
        let mut facts = Facts::new(times.len());
        facts.trace = Some(trace::Capture::new(&src, &m));
        let nodes = graph(&m, times, 10000, &mut facts).unwrap();
        for node in nodes {
            let s = &node.state;
            phases.insert(format!("{:?}", s.phase));
            for (k, slot) in s.cases.iter().enumerate() {
                if let Slot::Admitted(c) = slot {
                    if !c.received {
                        assert!(c.local.monitors.iter().all(|m| !m.activated && !m.violated));
                    } else {
                        let view = s.view(k, &m).unwrap();
                        let mut normalized = view.clone();
                        update_state_invariants(&mut normalized, &m, k).unwrap();
                        assert_eq!(
                            view, normalized,
                            "global key inserted before normalization: {name}/{k}"
                        );
                    }
                }
            }
        }
        for edge in facts.trace.as_ref().unwrap().edges.iter().flatten() {
            labels.insert(format!("{:?}", edge.stage.kind));
        }
    }
    for phase in [
        Phase::Duration,
        Phase::Admission,
        Phase::Chance,
        Phase::Nondet,
        Phase::Receipt,
        Phase::Mixed,
    ] {
        assert!(
            phases.contains(&format!("{phase:?}")),
            "missing global boundary {phase:?}"
        );
    }
    for label in [
        "Receive",
        "Start",
        "Finish",
        "Refuse",
        "End",
        "Advance",
        "Initialize",
    ] {
        assert!(
            labels.contains(label),
            "missing global hook {label}: {labels:?}"
        );
    }
}

#[test]
fn state_invariant_passive_normalization_never_stores_or_retimes_ended_case() {
    let src = include_str!("../../../../fixtures/sinv-ended-clean.broken-increment/model.relca");
    let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
    let times = m.arrivals.as_ref().unwrap();
    let mut facts = Facts::new(times.len());
    let nodes = graph(&m, times, 10000, &mut facts).unwrap();
    let mut observed = 0;
    for node in nodes {
        let s = &node.state;
        if s.time != 3 {
            continue;
        }
        for slot in &s.cases {
            if let Slot::Admitted(c) = slot
                && c.terminal.is_some()
                && c.local.monitors[0].violated
            {
                assert_eq!(c.local.time, 2);
                assert_eq!(c.local.work, 2);
                assert!(
                    c.local
                        .state_values
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| m.is_shared_state(m.state_variables[*i].kind))
                        .all(|(_, v)| *v == ir::StateValue::None)
                );
                observed += 1;
            }
        }
    }
    assert!(
        observed > 0,
        "no ended-reader violation at the writer's t3 poststate"
    );
}

#[test]
fn state_invariant_passive_witness_survives_another_clause_aborting_the_same_bundle() {
    let src = include_str!("../../../../fixtures/sinv-ended-clean.broken-increment/model.relca");
    let mut m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
    let times = m.arrivals.clone().unwrap();
    let nodes = graph(&m, &times, 10000, &mut Facts::new(2)).unwrap();
    let parent = nodes.iter().map(|n| &*n.state).find(|s| s.time == 3 && s.phase == Phase::Mixed
        && s.shared.contains(&ir::StateValue::Integer(0))
        && s.cases.iter().any(|slot| matches!(slot, Slot::Admitted(c) if c.terminal.is_some()))
        && s.cases.iter().any(|slot| matches!(slot, Slot::Admitted(c) if c.local.tasks.iter().any(|t| matches!(t.lifecycle, Lifecycle::Running { finish: 3, .. }))))).unwrap();
    let mut parent = parent.clone();
    let capability = ir::CapabilityId(
        m.capabilities
            .iter()
            .position(|c| c.name == "post")
            .unwrap(),
    );
    let event = ir::Event {
        capability,
        kind: ir::EventKind::Finish(None),
    };
    m.contracts.push(ir::ContractClause {
        name: Some("Overflow".into()),
        ordinal: None,
        span: m.process.span,
        kind: ir::ContractKind::Response {
            trigger: ir::Trigger::Event(event),
            targets: vec![ir::Target::End(ir::EndId(0))],
            within: u64::MAX,
        },
    });
    m.verify.push(ir::Verify {
        property: ir::Property::Contract(ir::ClauseId(1)),
        span: m.process.span,
    });
    for slot in &mut parent.cases {
        if let Slot::Admitted(c) = slot {
            c.local.monitors.push(Monitor::default());
        }
    }
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    let mut facts = Facts::new(2);
    let result = transition(
        &parent,
        &m,
        &times,
        &program,
        entry,
        &mut Room(10000),
        &mut facts,
    );
    assert!(
        result.is_err(),
        "the response deadline must overflow at the finish"
    );
    let ended = parent
        .cases
        .iter()
        .position(|slot| matches!(slot, Slot::Admitted(c) if c.terminal.is_some()))
        .unwrap();
    assert!(
        facts.per[ended].violation[0],
        "independent known ended-reader violation must survive response abort"
    );
}
