//! Link as a cfg(test) child of engine/trace.rs; see actor_observed_obligations.md.
//! Deliberately not an integration test: Capture/replay are crate-private.
use super::*;

fn source(name: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join("fixtures").join(name).join("model.relca")).unwrap()
}
fn model(src: &str) -> ir::Model {
    crate::syntax::parse(src)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap()
}
fn changed(src: &str, from: &str, to: &str) -> String {
    let s = src.replace(from, to);
    assert_ne!(src, s);
    s
}
fn numbered(
    mut path: Vec<Stage>,
    case: Option<u64>,
    choices: Vec<Option<KernelChoice>>,
    target: Option<&str>,
) -> WitnessPath {
    for (i, s) in path.iter_mut().enumerate() {
        s.index = i;
    }
    if let Some(target) = target {
        let last = path.last_mut().unwrap();
        last.violation = Some(Violation {
            id: target.into(),
            property_location: None,
            at_step: last.index,
            trigger_step: None,
            deadline: None,
            related_steps: vec![],
            state_invariant: None,
        });
    }
    (path, case, choices)
}
fn selected(c: &Capture, target: &str, case: u64) -> WitnessPath {
    let mut queue = VecDeque::from([(0, vec![c.initial.clone().unwrap()], vec![None])]);
    let mut seen = HashSet::new();
    while let Some((node, path, choices)) = queue.pop_front() {
        if !seen.insert(node) {
            continue;
        }
        for edge in &c.edges[node] {
            let mut next = path.clone();
            next.push(edge.stage.clone());
            let mut draws = choices.clone();
            draws.push(edge.choice.clone());
            if edge.cases.get(target) == Some(&case) {
                return numbered(next, Some(case), draws, Some(target));
            }
            queue.push_back((c.canonical(edge.to), next, draws));
        }
    }
    panic!("missing target {target} case{case}");
}
fn leaves(c: &Capture) -> Vec<WitnessPath> {
    // Finite positive-time examples. Do not globally deduplicate nodes: distinct
    // predecessor histories are precisely the joint-order oracle being tested.
    let mut queue = VecDeque::from([(0, vec![c.initial.clone().unwrap()], vec![None], vec![0])]);
    let mut result = vec![];
    let mut visited = 0;
    while let Some((node, path, choices, nodes)) = queue.pop_front() {
        visited += 1;
        assert!(
            visited < 200_000,
            "acceptance path enumeration exceeded its explicit bound"
        );
        if c.edges[node].is_empty() {
            result.push(numbered(path, None, choices, None));
            continue;
        }
        for edge in &c.edges[node] {
            let at = c.canonical(edge.to);
            assert!(
                !nodes.contains(&at),
                "unexpected cycle in finite straight-line acceptance model"
            );
            let mut next = path.clone();
            next.push(edge.stage.clone());
            let mut draws = choices.clone();
            draws.push(edge.choice.clone());
            let mut ns = nodes.clone();
            ns.push(at);
            queue.push_back((at, next, draws, ns));
        }
    }
    result
}
fn replay(m: &ir::Model, path: &WitnessPath, target: &str) -> Result<(), ReplayReason> {
    crate::reference::replay::replay_choices(m, &path.0, target, path.1, &path.2)
}

#[test]
fn aobs_path_enumerator_control_runs_the_existing_global_graph() {
    let src = source("arrivals-staggered");
    let m = model(&src);
    let (_, c) = explore(&src, &m);
    assert_eq!(c.failure, None);
    let paths = leaves(&c);
    assert!(!paths.is_empty());
    for path in paths {
        assert_eq!(path.0.iter().filter(|s| s.kind == Kind::End).count(), 3);
    }
}

#[test]
fn aobs_start_is_visible_in_the_other_case_branch_snapshot() {
    let src = source("aobs-start-visible");
    let m = model(&src);
    let (_, c) = explore(&src, &m);
    assert_eq!(c.failure, None);
    for path in leaves(&c) {
        let start = path
            .0
            .iter()
            .find(|s| {
                s.kind == Kind::Start && s.case_id == Some(1) && s.actor.as_deref() == Some("Desk")
            })
            .unwrap();
        assert!(
            start
                .values
                .observed
                .iter()
                .any(|v| v.name == "Desk.busy" && v.value == "true")
        );
        let branch = path
            .0
            .iter()
            .find(|s| s.kind == Kind::Branch && s.case_id == Some(2))
            .unwrap();
        assert_eq!(branch.time, 2);
        assert!(
            branch
                .values
                .observed
                .iter()
                .any(|v| v.name == "Desk.busy" && v.value == "true")
        );
    }
}

#[test]
fn aobs_cancel_ack_preserves_start_and_omits_finish_in_the_actual_snapshot() {
    let src = source("aobs-cancel");
    let m = model(&src);
    let (_, c) = explore(&src, &m);
    assert_eq!(c.failure, None);
    for path in leaves(&c) {
        let ack = path
            .0
            .iter()
            .find(|s| s.kind == Kind::CancelAck && s.actor.as_deref() == Some("Slow"))
            .unwrap();
        assert_eq!(ack.time, 1);
        assert!(
            ack.values
                .observed
                .iter()
                .any(|v| v.name == "Slow.started" && v.value == "true")
        );
        assert!(
            ack.values
                .observed
                .iter()
                .any(|v| v.name == "Slow.finished" && v.value == "false")
        );
        assert!(
            !path
                .0
                .iter()
                .any(|s| s.kind == Kind::Finish && s.actor.as_deref() == Some("Slow"))
        );
    }
}

#[test]
fn aobs_per_case_observations_do_not_exist_before_each_admission() {
    let src = source("aobs-per-case");
    let m = model(&src);
    let (_, c) = explore(&src, &m);
    assert_eq!(c.failure, None);
    assert!(c.initial.as_ref().unwrap().values.observed.is_empty());
    let mut seen = BTreeSet::new();
    for edge in c.edges.iter().flatten() {
        let mut case = None;
        for e in &edge.stage.events {
            match e {
                Event::Business(b) if b.kind == Kind::Initialize => case = b.case_id,
                Event::Value(v) if v.name.starts_with("Doc.") => {
                    let k = case.expect("local initialization event needs a case identity");
                    assert_eq!(edge.stage.time, if k == 1 { 0 } else { 3 });
                    assert_eq!(v.visibility, Visibility::Observed);
                    seen.insert((k, v.name.clone(), v.value.clone()));
                }
                _ => {}
            }
        }
    }
    assert_eq!(
        seen,
        BTreeSet::from([
            (1, "Doc.flag".into(), "true".into()),
            (1, "Doc.answer".into(), "none".into()),
            (2, "Doc.flag".into(), "true".into()),
            (2, "Doc.answer".into(), "none".into())
        ])
    );
}

#[test]
fn aobs_joint_complete_paths_are_exactly_posted_denied_and_denied_posted() {
    let src = source("aobs-shared-order");
    let m = model(&src);
    let (_, c) = explore(&src, &m);
    assert_eq!(c.failure, None);
    let mut joint = BTreeSet::new();
    for (path, _, _) in leaves(&c) {
        let mut ends = [None, None];
        let mut normal = vec![];
        let mut refusals = vec![];
        for s in &path {
            if s.kind == Kind::End {
                let case = s.case_id.unwrap() as usize - 1;
                assert!(case < 2);
                assert!(ends[case].is_none());
                let text = &src[s.location.span.start..s.location.span.end];
                ends[case] = Some(if text.contains("posted") {
                    "posted"
                } else if text.contains("denied") {
                    "denied"
                } else {
                    panic!("bad end location {text}")
                });
                assert_eq!(s.time, 1);
            }
            if s.actor.as_deref() == Some("Ledger") {
                if s.kind == Kind::Finish {
                    normal.push(s.case_id.unwrap());
                    assert_eq!(s.time, 1);
                    assert_eq!(s.result.as_deref(), Some("committed"));
                }
                if s.kind == Kind::Refuse {
                    refusals.push(s.case_id.unwrap());
                    assert_eq!(s.time, 1);
                    assert_eq!(s.result.as_deref(), Some("rejected"));
                }
            }
        }
        assert_eq!(normal.len(), 1);
        assert_eq!(refusals.len(), 1);
        assert_ne!(normal[0], refusals[0]);
        joint.insert((ends[0].unwrap(), ends[1].unwrap()));
        let last = path.last().unwrap();
        assert!(
            last.values
                .observed
                .iter()
                .any(|v| v.name == "Ledger.posted" && v.value == "1")
        );
    }
    assert_eq!(
        joint,
        BTreeSet::from([("posted", "denied"), ("denied", "posted")])
    );
}

#[test]
fn aobs_read_write_same_tick_has_both_complete_joint_histories() {
    let src = source("aobs-read-write-order");
    let m = model(&src);
    let (_, c) = explore(&src, &m);
    assert_eq!(c.failure, None);
    let mut joint = BTreeSet::new();
    let mut order = BTreeSet::new();
    for (path, _, _) in leaves(&c) {
        let ends: Vec<_> = path.iter().filter(|s| s.kind == Kind::End).collect();
        assert_eq!(ends.len(), 2);
        let first = ends.iter().find(|s| s.case_id == Some(1)).unwrap();
        let second = ends.iter().find(|s| s.case_id == Some(2)).unwrap();
        joint.insert((first.time, second.time));
        let finish = path
            .iter()
            .position(|s| {
                s.kind == Kind::Finish && s.case_id == Some(1) && s.actor.as_deref() == Some("Desk")
            })
            .unwrap();
        let branch = path
            .iter()
            .position(|s| s.kind == Kind::Branch && s.case_id == Some(2))
            .unwrap();
        assert_eq!(path[finish].time, 2);
        assert_eq!(path[branch].time, 2);
        assert_eq!(second.time, if finish < branch { 3 } else { 4 });
        order.insert(finish < branch);
    }
    assert_eq!(joint, BTreeSet::from([(2, 3), (2, 4)]));
    assert_eq!(order, BTreeSet::from([false, true]));
}

#[test]
fn aobs_range_prefixes_for_both_writer_orders_replay_independently() {
    let src = source("aobs-shared-order.broken-no-requires");
    let m = model(&src);
    let (_, c) = explore(&src, &m);
    assert_eq!(c.failure, None);
    for case in [1, 2] {
        let path = selected(&c, "state_in_range", case);
        assert_eq!(replay(&m, &path, "state_in_range"), Ok(()));
        let w = path
            .0
            .last()
            .unwrap()
            .state_range_violation
            .as_ref()
            .unwrap();
        assert_eq!(w.case_id, Some(case));
        assert_eq!(w.time, 2);
        assert_eq!(w.before, "1");
        assert_eq!(w.attempted, "2");
        let finish: Vec<_> = path
            .0
            .iter()
            .filter(|s| s.kind == Kind::Finish && s.actor.as_deref() == Some("Ledger"))
            .collect();
        assert_eq!(finish.len(), 1);
        assert_eq!(finish[0].case_id, Some(3 - case));
    }
}

#[test]
fn aobs_replay_rejects_single_value_visibility_name_span_missing_duplicate_and_case_tamper() {
    for arrivals in [false, true] {
        let src = source("aobs-counter.broken-overflow");
        let src = if arrivals {
            changed(&src, "cut C", "arrivals fixed [0min]; cut C")
        } else {
            src
        };
        let m = model(&src);
        let (_, c) = explore(&src, &m);
        let path = selected(&c, "state_in_range", 1);
        assert_eq!(replay(&m, &path, "state_in_range"), Ok(()));
        let index = path
            .0
            .iter()
            .position(|s| s.values.observed.iter().any(|v| v.name == "Desk.handled"))
            .unwrap();
        let vi = path.0[index]
            .values
            .observed
            .iter()
            .position(|v| v.name == "Desk.handled")
            .unwrap();
        for fault in [
            "value",
            "visibility",
            "name",
            "span",
            "missing",
            "duplicate",
            "case",
        ] {
            let mut bad = path.clone();
            let s = &mut bad.0[index];
            match fault {
                "value" => s.values.observed[vi].value = "1".into(),
                "visibility" => s.values.observed[vi].visibility = Visibility::Latent,
                "name" => s.values.observed[vi].name = "handled".into(),
                "span" => s.values.observed[vi].location.span.start += 1,
                "missing" => {
                    s.values.observed.remove(vi);
                }
                "duplicate" => s.values.observed.push(s.values.observed[vi].clone()),
                "case" => s.case_id = Some(99),
                _ => unreachable!(),
            }
            assert!(
                replay(&m, &bad, "state_in_range").is_err(),
                "accepted {fault} arrivals={arrivals}"
            );
        }
    }
}

#[test]
fn aobs_late_admission_keeps_current_shared_snapshot_and_rejects_stale_other_frame() {
    let src = source("aobs-late");
    let src = changed(
        &src,
        "process P",
        "actor Probe { capacity 1; capability check() -> R { duration fixed 1min; result nondet { committed } } }; process P",
    );
    let src = changed(
        &src,
        "else { end denied }",
        "else { p = Probe.check(); end denied }",
    );
    let src = changed(&src, "include [Ledger]", "include [Ledger,Probe]");
    let src = changed(
        &src,
        "verify safety",
        "contract Observe { invariant Probe.check.finish -> Ledger.posted == 0 }; verify safety",
    );
    let m = model(&src);
    let target = m.contracts[0].property_id(ir::ClauseId(0));
    let (_, c) = explore(&src, &m);
    let path = selected(&c, &target, 2);
    assert_eq!(replay(&m, &path, &target), Ok(()));
    assert_eq!(
        path.0.last().unwrap().time,
        4,
        "Probe3..4 after zero-work refusal3"
    );
    let admission = path
        .0
        .iter()
        .position(|s| s.time == 3 && s.values.observed.iter().any(|v| v.name == "Ledger.posted"))
        .unwrap();
    let value = path.0[admission]
        .values
        .observed
        .iter()
        .position(|v| v.name == "Ledger.posted")
        .unwrap();
    assert_eq!(path.0[admission].values.observed[value].value, "1");
    let mut bad = path.clone();
    bad.0[admission].values.observed[value].value = "0".into();
    assert!(replay(&m, &bad, &target).is_err());
    let mut bad = path.clone();
    let first_finish = bad
        .0
        .iter_mut()
        .find(|s| {
            s.kind == Kind::Finish && s.case_id == Some(1) && s.actor.as_deref() == Some("Ledger")
        })
        .unwrap();
    first_finish
        .values
        .observed
        .iter_mut()
        .find(|v| v.name == "Ledger.posted")
        .unwrap()
        .value = "0".into();
    assert!(replay(&m, &bad, &target).is_err());
    let mut bad = path.clone();
    let start = bad
        .0
        .iter()
        .position(|s| s.kind == Kind::Start && s.actor.as_deref() == Some("Probe"))
        .unwrap();
    bad.0.swap(start, start + 1);
    assert!(
        replay(&m, &bad, &target).is_err(),
        "illegal order must not replay"
    );
}
