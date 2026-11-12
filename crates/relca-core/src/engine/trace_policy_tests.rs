use super::*;

fn source(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
fn model(src: &str) -> ir::Model {
    crate::syntax::parse(src)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap()
}
fn leaves(c: &Capture) -> Vec<Vec<Stage>> {
    let mut pending = vec![(0, vec![c.initial.clone().unwrap()])];
    let mut out = Vec::new();
    while let Some((node, path)) = pending.pop() {
        assert!(path.len() < 200);
        if c.edges[node].is_empty() {
            out.push(path);
            continue;
        }
        for edge in &c.edges[node] {
            let mut next = path.clone();
            next.push(edge.stage.clone());
            pending.push((c.canonical(edge.to), next));
        }
    }
    out
}

#[test]
fn policy_latent_each_joint_tuple_has_case1_before_case2_even_false_true() {
    let src = source("pol-latent");
    let m = model(&src);
    let (_, capture) = explore(&src, &m);
    assert_eq!(capture.failure, None);
    let paths = leaves(&capture);
    assert_eq!(paths.len(), 4);
    let mut tuples = BTreeSet::new();
    for path in paths {
        let initial = path
            .iter()
            .find(|s| {
                s.kind == Kind::Initialize && s.events.iter().any(|e| matches!(e, Event::Value(_)))
            })
            .unwrap();
        let tuple: Vec<_> = initial
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Value(v) if v.name == "hard" => Some(v.value.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(tuple.len(), 2);
        tuples.insert(tuple);
        let task_events: Vec<_> = path
            .iter()
            .filter(|s| matches!(s.kind, Kind::Start | Kind::Finish))
            .map(|s| (s.case_id, s.kind, s.time))
            .collect();
        assert_eq!(
            task_events,
            [
                (Some(1), Kind::Start, 0),
                (Some(1), Kind::Finish, 2),
                (Some(2), Kind::Start, 2),
                (Some(2), Kind::Finish, 4)
            ]
        );
    }
    assert_eq!(
        tuples,
        BTreeSet::from([
            vec!["true".into(), "true".into()],
            vec!["true".into(), "false".into()],
            vec!["false".into(), "true".into()],
            vec!["false".into(), "false".into()]
        ])
    );
}

#[test]
fn policy_projection_consistent_open_policy_traces_fail_only_at_new_dispatch_rule() {
    for name in ["pol-separation", "pol-tie-actor", "pol-resource-hol"] {
        let mut src = source(name);
        if name == "pol-resource-hol" {
            let at = src.rfind('}').unwrap();
            src.insert_str(at,"contract Due {response case.received -> one_of(end.done) by 2min after case.received otherwise unresolved}\n");
        }
        let m = model(&src);
        let mut open = m.clone();
        open.start_policy = Some(ir::StartPolicy::default());
        let (_, capture) = explore(&src, &open);
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
            crate::reference::replay::replay_choices(&open, &steps, &target, case, &choices),
            Ok(()),
            "open trace is otherwise completely valid: {name}"
        );
        assert_eq!(
            crate::reference::replay::replay_choices(&m, &steps, &target, case, &choices),
            Err(ReplayReason::IllegalTransition),
            "same source, spans, holders, values, initial state; not ModelMismatch: {name}"
        );
    }
}

#[test]
fn policy_named_start_range_stop_replays_before_executor_acquisition() {
    let src = r#"model M version "0.2" {type T=enum {ok} people [p] actor A {executors [p];state x:0..1=1;capability work()->T {duration fixed 1min;result nondet {ok};writes x;start {x+=1};finish {x=1}}} process P {a=A.work();end done} bounds {cases 1;live_tasks 1;clock 3min} policy fifo tie_by case_id then call_order assign first_idle verify safety no_resource_overcommit}"#;
    let m = model(src);
    let (_, capture) = explore(src, &m);
    assert_eq!(capture.failure, None);
    let target = capture
        .edges
        .iter()
        .flatten()
        .flat_map(|e| &e.targets)
        .find(|t| t.contains("state_in_range"))
        .unwrap()
        .clone();
    let (mut steps, case, choices) = capture.shortest(&target).unwrap();
    let last = steps.last_mut().unwrap();
    assert_eq!(last.kind, Kind::RangeViolation);
    assert_eq!(
        last.executor, None,
        "range stop precedes start and person acquisition"
    );
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
}
