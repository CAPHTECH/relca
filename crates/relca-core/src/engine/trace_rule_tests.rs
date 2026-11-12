use super::*;

#[test]
fn rule_replay_rejects_poisoned_values_owners_versions_and_short_capture() {
    let src = include_str!("../../../../fixtures/rule-explain/model.relca");
    let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
    let (_, capture) = explore(src, &m);
    assert_eq!(capture.failure, None);
    let target = capture
        .edges
        .iter()
        .flatten()
        .flat_map(|e| &e.targets)
        .find(|t| t.starts_with("contract "))
        .unwrap()
        .clone();
    let (mut steps, case, choices) = capture.shortest(&target).unwrap();
    let last = steps.last_mut().unwrap();
    last.violation = Some(Violation {
        id: target.clone(),
        property_location: None,
        at_step: last.index,
        trigger_step: None,
        deadline: None,
        related_steps: Vec::new(),
        state_invariant: None,
    });
    assert_eq!(
        crate::reference::replay::replay_choices(&m, &steps, &target, case, &choices),
        Ok(())
    );
    for mode in 0..3 {
        let mut poisoned = steps.clone();
        let value = poisoned[0]
            .values
            .observed
            .iter_mut()
            .find(|v| v.name == "ApprovalRule.approval_required")
            .unwrap();
        match mode {
            0 => value.value = "true".into(),
            1 => value.name = "AnotherRule.approval_required".into(),
            _ => value.location.span.start += 1,
        }
        assert!(
            crate::reference::replay::replay_choices(&m, &poisoned, &target, case, &choices)
                .is_err(),
            "poison mode {mode}"
        );
    }
    let mut forged = m.clone();
    forged.actors[0]
        .metadata
        .rule
        .as_mut()
        .unwrap()
        .effective_version = "forged".into();
    assert!(
        crate::reference::replay::replay_choices(&forged, &steps, &target, case, &choices).is_err()
    );
    let k = capture.recorded_edges;
    assert!(k > 0);
    for (limit, expected) in [(k, None), (k - 1, Some(ReplayReason::CaptureLimit))] {
        let mut capture = Capture::new(src, &m);
        capture.capture_limit = limit;
        let (_, capture) = explore_captured(&m, DEFAULT_STATE_BUDGET, capture);
        assert_eq!(capture.failure, expected);
        if expected.is_some() {
            assert!(capture.shortest(&target).is_none());
        }
    }
    let n = steps.len();
    assert!(
        crate::reference::replay::replay_choices(
            &m,
            &steps[..n - 1],
            &target,
            case,
            &choices[..n - 1]
        )
        .is_err(),
        "a prefix is not a verified full counterexample"
    );
}

#[test]
fn rule_multi_case_replay_keeps_qualified_fixed_initial_values() {
    let mut src =
        include_str!("../../../../fixtures/rule-explain/model.relca").replace("cases 1", "cases 2");
    let end = src.rfind('}').unwrap();
    src.insert_str(end, "arrivals fixed [0min,0min]\n");
    let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
    let (_, capture) = explore(&src, &m);
    assert_eq!(capture.failure, None);
    let target = capture
        .edges
        .iter()
        .flatten()
        .flat_map(|e| &e.targets)
        .find(|t| t.starts_with("contract "))
        .unwrap()
        .clone();
    let (mut steps, case, choices) = capture.shortest(&target).unwrap();
    let last = steps.last_mut().unwrap();
    last.violation = Some(Violation {
        id: target.clone(),
        property_location: None,
        at_step: last.index,
        trigger_step: None,
        deadline: None,
        related_steps: Vec::new(),
        state_invariant: None,
    });
    assert_eq!(
        crate::reference::replay::replay_choices(&m, &steps, &target, case, &choices),
        Ok(())
    );
    let value = steps
        .iter_mut()
        .flat_map(|s| &mut s.values.observed)
        .find(|v| v.name == "ApprovalRule.approval_required")
        .unwrap();
    value.value = "true".into();
    assert!(crate::reference::replay::replay_choices(&m, &steps, &target, case, &choices).is_err());
}
