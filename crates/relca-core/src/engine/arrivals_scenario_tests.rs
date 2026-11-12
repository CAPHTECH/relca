//! Implementation-only obligations: graph keys, near-horizon Room and forged IR.
use super::*;

const SOURCE: &str = include_str!("../../../../fixtures/scn-horizon/model.relca");

fn selected() -> ir::Model {
    crate::lower::lower_with_selection(&crate::syntax::parse(SOURCE).unwrap(), None, Some("Day"))
        .unwrap()
}

#[test]
fn horizon_graph_leaf_keeps_time_work_phase_and_stop_class_separate() {
    let m = selected();
    let a = arrivals(&m).unwrap();
    let mut f = Facts::new(a.len());
    let nodes = graph(&m, &a, DEFAULT_STATE_BUDGET, &mut f).unwrap();
    let horizon: Vec<_> = nodes
        .iter()
        .filter(|n| n.cut == ProbabilityCut::Horizon)
        .collect();
    assert_eq!(horizon.len(), 1);
    let n = horizon[0];
    assert_eq!(n.state.time, 12);
    assert!(n.unknown && n.actions.is_empty());
    assert!(!f.truncated && f.horizon && !f.budget);
    let Slot::Admitted(c) = &n.state.cases[2] else {
        panic!("admitted")
    };
    assert_eq!(c.local.work, 2);
    assert!(c.terminal.is_none());
    let mut earlier = (*n.state).clone();
    earlier.time = 11;
    let mut phase = (*n.state).clone();
    phase.phase = Phase::Duration;
    if phase == *n.state {
        phase.phase = Phase::Admission;
    }
    let states = std::collections::HashSet::from([(*n.state).clone(), earlier, phase]);
    assert_eq!(
        states.len(),
        3,
        "time and same-tick phase are execution keys"
    );
    assert!(ProbabilityCut::Horizon != ProbabilityCut::Clock);
    assert!(ProbabilityCut::Horizon != ProbabilityCut::Pending);
    assert!(ProbabilityCut::Horizon != ProbabilityCut::Range);
}

#[test]
fn near_horizon_room_refusal_remains_computational_and_trace_is_noninterfering() {
    let m = selected();
    let count = compatibility_counts(&m).0;
    let mut cut = false;
    let mut closed = false;
    for budget in count.saturating_sub(5)..=count + 2 {
        trace_compatibility_at_budget(SOURCE, &m, budget);
        let out = explore(&m, budget);
        if out.truncated {
            cut = true;
            assert_eq!(
                out.metrics.as_ref().unwrap().status,
                crate::metrics::Status::Truncated
            );
            assert!(
                matches!(&out.scenario_observed_until, Some(crate::metrics::Scalar::Range(r)) if r.reason == Some(crate::metrics::Reason::Truncated))
            );
        } else {
            closed = true;
            assert!(
                out.case_outcomes.as_ref().unwrap()[2]
                    .terminals
                    .iter()
                    .any(|t| t.kind == Kind::CutOff)
            );
            assert_eq!(
                out.metrics.as_ref().unwrap().status,
                crate::metrics::Status::Complete
            );
        }
    }
    assert!(
        cut && closed,
        "inspect both sides of the actual near-D Room boundary"
    );
}

#[test]
fn forged_scenario_records_are_refused_by_exhaustive_and_sampled_entries() {
    let base = selected();
    let mut bad = base.clone();
    bad.scenario = None;
    bad.arrivals = None;
    bad.arrival_every = None;
    bad.bounds.cases = 1;
    bad.start_policy = None;
    // Even a malformed IR which would otherwise select the legacy route must
    // not smuggle in a horizon without its declaration record.
    assert!(bad.legacy_route());
    for m in [&bad, &{
        let mut m = base.clone();
        m.arrival_every.as_mut().unwrap().count = 0;
        m
    }] {
        assert!(
            crate::engine::explore(m)
                .diagnostics
                .iter()
                .any(|d| d.code == diag::U_SEM_0)
        );
        assert!(
            crate::engine::simulate(m)
                .diagnostics
                .iter()
                .any(|d| d.code == diag::U_SEM_0)
        );
        assert_eq!(
            crate::reference::explore(m).stop,
            crate::reference::StopReason::SemanticAbort
        );
        assert_eq!(
            crate::reference::simulate(m).stop,
            crate::reference::StopReason::SemanticAbort
        );
    }
}

#[test]
fn projection_refuses_forged_scenario_metadata_and_cutoff_nullability() {
    let m = selected();
    let r = crate::report::check_with_selection(SOURCE, None, Some("Day"));
    assert!(crate::comparison_projection::exhaustive(&r, &m).is_ok());
    let mut bad = r.clone();
    bad.scenario.as_mut().unwrap()["drain"]["from"] = 6.into();
    assert!(crate::comparison_projection::exhaustive(&bad, &m).is_err());
    let mut bad = r.clone();
    bad.scenario.as_mut().unwrap()["extra"] = true.into();
    assert!(crate::comparison_projection::exhaustive(&bad, &m).is_err());
    let mut bad = r;
    bad.case_outcomes.as_mut().unwrap()[2].terminals[0].completed_at =
        Some(crate::report::Range { min: 12, max: 12 });
    assert!(crate::comparison_projection::exhaustive(&bad, &m).is_err());
}

#[test]
fn sampled_projection_keeps_cutoff_counts_separate_and_checks_observation_coverage() {
    let source = include_str!("../../../../fixtures/scn-sim/model.relca");
    let m = crate::lower::lower_with_selection(
        &crate::syntax::parse(source).unwrap(),
        None,
        Some("Day"),
    )
    .unwrap();
    let report = crate::report::simulate_with_selection(source, None, Some("Day"));
    let view = crate::comparison_projection::simulation(&report, &m).unwrap();
    assert!(
        view.terminals
            .contains_key(&(3, crate::comparison_projection::TerminalKey::CutOff))
    );
    let mut bad = report.clone();
    bad.case_outcomes[2]["cut_off_count"] = 0.into();
    assert!(crate::comparison_projection::simulation(&bad, &m).is_err());
    let mut bad = report.clone();
    bad.scenario.as_mut().unwrap()["aggregation"]["observed_until"]["samples"] = 3.into();
    assert!(crate::comparison_projection::simulation(&bad, &m).is_err());
    let mut bad = report.clone();
    bad.metrics.as_mut().unwrap().overall.counts.cut_off = None;
    assert!(crate::comparison_projection::simulation(&bad, &m).is_err());
    let mut bad = report;
    bad.simulation["cut_off_replications"] = 5.into();
    assert!(crate::comparison_projection::simulation(&bad, &m).is_err());
}

#[test]
fn copy_on_write_payload_preserves_old_value_hash_debug_and_isolates_successors() {
    use std::hash::{Hash, Hasher};
    #[derive(Debug, Hash)]
    #[allow(clippy::large_enum_variant)] // Deliberately mirror the former inline layout.
    enum InlineSlot {
        Future,
        Rejected,
        Admitted(Case),
    }
    fn hash(value: &impl Hash) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut h);
        h.finish()
    }
    let m = selected();
    let a = arrivals(&m).unwrap();
    let mut facts = Facts::new(a.len());
    let nodes = graph(&m, &a, DEFAULT_STATE_BUDGET, &mut facts).unwrap();
    let node = nodes
        .iter()
        .find(|n| n.cut == ProbabilityCut::Horizon)
        .unwrap();
    for slot in &node.state.cases {
        let Slot::Admitted(shared) = slot else {
            panic!("all cases admitted")
        };
        let plain = (**shared).clone();
        assert_eq!(hash(shared), hash(&plain));
        assert_eq!(format!("{shared:?}"), format!("{plain:?}"));
        let mirrors = [
            (Slot::Future, InlineSlot::Future),
            (Slot::Rejected, InlineSlot::Rejected),
            (slot.clone(), InlineSlot::Admitted(plain)),
        ];
        for (stored, inline) in mirrors {
            assert_eq!(hash(&stored), hash(&inline));
            assert_eq!(format!("{stored:?}"), format!("{inline:?}"));
        }
        let mut sibling = shared.clone();
        assert!(Rc::ptr_eq(&sibling.0, &shared.0));
        sibling.received = !shared.received;
        assert!(!Rc::ptr_eq(&sibling.0, &shared.0));
        assert_ne!(sibling.received, shared.received);
    }
}

#[test]
fn horizon_stop_does_not_swallow_an_unprocessed_event_at_the_horizon_time() {
    // Boundary review m1: the horizon leaf is legal only when the next event
    // lies after D. A state at D=12 with an event still due at 12 is the same
    // non-progressing clock as without a horizon, never a quiet horizon stop.
    let m = selected();
    let a = arrivals(&m).unwrap();
    let mut f = Facts::new(a.len());
    let nodes = graph(&m, &a, DEFAULT_STATE_BUDGET, &mut f).unwrap();
    let leaf = nodes
        .iter()
        .find(|n| n.cut == ProbabilityCut::Horizon)
        .unwrap();
    assert_eq!(leaf.state.time, 12);
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    let honest = transition(
        &leaf.state,
        &m,
        &a,
        &program,
        entry,
        &mut Room(1000),
        &mut Facts::new(a.len()),
    )
    .unwrap();
    assert_eq!(
        honest.map(|x| x.len()),
        Some(0),
        "control: next event 15 > D"
    );
    // Forge one unprocessed event at D: an arrival at 12 still behind the cursor
    // (the Mixed layer never admits; admission belongs to its own phase).
    let mut forged = (*leaf.state).clone();
    forged.cursor = 2;
    let late = [a[0], a[1], 12];
    let r = transition(
        &forged,
        &m,
        &late,
        &program,
        entry,
        &mut Room(1000),
        &mut Facts::new(a.len()),
    );
    assert_eq!(
        r.err(),
        Some(Halt::Unsupported("non-progressing multi-case clock"))
    );
}
