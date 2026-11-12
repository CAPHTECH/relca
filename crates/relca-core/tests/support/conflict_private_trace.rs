//! Internal trace tests linked from src/engine/trace.rs:
//! #[cfg(test)] #[path = "../../tests/support/conflict_private_trace.rs"] mod conflict_private;
use super::*;
use serde_json::{Value as Json, json};

fn source(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
fn capture(name: &str) -> (ir::Model, Capture) {
    let s = source(name);
    let m = crate::syntax::parse(&s)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap();
    let (_, c) = explore(&s, &m);
    assert_eq!(c.failure, None);
    (m, c)
}
fn value(stage: &Stage, name: &str) -> Json {
    json!(stage)["values"]["observed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == name)
        .unwrap()["value"]
        .clone()
}
fn commits(s: &Stage) -> usize {
    json!(s)["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "commit")
        .count()
}
fn paths(c: &Capture) -> Vec<Vec<Stage>> {
    fn walk(
        c: &Capture,
        node: usize,
        path: &mut Vec<Stage>,
        active: &mut BTreeSet<usize>,
        out: &mut Vec<Vec<Stage>>,
    ) {
        assert!(active.insert(node), "unexpected same-path cycle");
        assert!(
            out.len() < 100_000,
            "explicit path enumeration budget, never silent skip"
        );
        if c.edges[node].is_empty() {
            out.push(path.clone());
        }
        for edge in &c.edges[node] {
            path.push(edge.stage.clone());
            walk(c, c.canonical(edge.to), path, active, out);
            path.pop();
        }
        active.remove(&node);
    }
    let mut out = vec![];
    walk(c, 0, &mut vec![], &mut BTreeSet::new(), &mut out);
    out
}

#[test]
fn cf_each_joint_two_and_three_case_path_has_exactly_one_commit() {
    for (name, n) in [("cf-version", 2), ("cf-three", 3), ("cf-wide-state", 2)] {
        let (_, c) = capture(name);
        let paths = paths(&c);
        assert!(!paths.is_empty());
        let mut winners = BTreeSet::new();
        for path in paths {
            let finishes: Vec<_> = path
                .iter()
                .filter(|s| s.kind == Kind::Finish && s.actor.as_deref() == Some("Ledger"))
                .collect();
            assert_eq!(finishes.len(), n);
            assert_eq!(path.iter().map(commits).sum::<usize>(), 1);
            assert_eq!(
                finishes
                    .iter()
                    .filter(|s| s.result.as_deref() == Some("conflict"))
                    .count(),
                n - 1
            );
            for s in finishes {
                assert_eq!(s.time, 1);
                assert_eq!(value(s, "Ledger.claims"), json!("1"));
                if commits(s) == 1 {
                    winners.insert(s.case_id.unwrap());
                }
            }
        }
        assert_eq!(winners, (1..=n as u64).collect());
    }
}

#[test]
fn cf_no_ghost_commit_or_checked_finish_writes_on_conflict() {
    for name in ["cf-version", "cf-no-ghost-commit", "cf-version-atomic"] {
        let (_, c) = capture(name);
        let mut count = 0;
        for edge in c.edges.iter().flatten().filter(|e| {
            e.stage.kind == Kind::Finish && e.stage.result.as_deref() == Some("conflict")
        }) {
            count += 1;
            assert_eq!(commits(&edge.stage), 0);
            assert_eq!(edge.stage.time, 1);
            if name == "cf-version-atomic" {
                let keys = value(&edge.stage, "Ledger.keys");
                assert_eq!(keys.as_array().unwrap().len(), 1);
            } else {
                assert_eq!(value(&edge.stage, "Ledger.claims"), json!("1"));
            }
        }
        assert!(count > 0);
    }
}

#[test]
fn cf_unchecked_start_receipts_survive_version_rejection() {
    for (name, wanted) in [
        ("cf-start-marker", "2"),
        ("cf-start-marker.broken-no-start", "0"),
    ] {
        let (_, c) = capture(name);
        let mut count = 0;
        for e in c.edges.iter().flatten().filter(|e| {
            e.stage.kind == Kind::Finish && e.stage.result.as_deref() == Some("conflict")
        }) {
            count += 1;
            assert_eq!(value(&e.stage, "Ledger.started"), json!(wanted));
            assert_eq!(value(&e.stage, "Ledger.claims"), json!("1"));
        }
        assert!(count > 0);
    }
}

#[test]
fn cf_second_check_variable_and_aba_history_have_independent_path_witnesses() {
    for (name, long_time, variable) in [
        ("cf-check-vector", 6, "Ledger.y"),
        ("cf-aba", 6, "Ledger.holder"),
    ] {
        let (_, c) = capture(name);
        let mut found = false;
        for path in paths(&c) {
            let Some(start) = path.iter().find(|s| {
                s.kind == Kind::Start && s.capability.as_deref() == Some("long") && s.time == 1
            }) else {
                continue;
            };
            let who = start.case_id;
            let foreign = |s: &Stage, cap: &str, time: u64| {
                s.case_id != who
                    && s.kind == Kind::Finish
                    && s.capability.as_deref() == Some(cap)
                    && s.time == time
            };
            if !path.iter().any(|s| foreign(s, "short", 2)) {
                continue;
            }
            if name == "cf-aba"
                && !path
                    .iter()
                    .any(|s| foreign(s, "release", 3) && value(s, variable) == json!("free"))
            {
                continue;
            }
            let Some(long) = path.iter().find(|s| {
                s.kind == Kind::Finish
                    && s.case_id == who
                    && s.capability.as_deref() == Some("long")
            }) else {
                continue;
            };
            assert_eq!(long.time, long_time);
            assert_eq!(long.result.as_deref(), Some("conflict"));
            assert_eq!(commits(long), 0);
            if name == "cf-aba" {
                assert_eq!(value(long, variable), json!("free"));
            }
            found = true;
        }
        assert!(
            found,
            "required legal prefix must actually be reachable: {name}"
        );
    }
}

#[test]
fn cf_cancel_ack_does_not_notify_but_drain_finish_does() {
    for (name, wanted) in [
        ("cf-cancel", "committed"),
        ("cf-cancel.broken-drain", "conflict"),
    ] {
        let (_, c) = capture(name);
        let mut found = false;
        for path in paths(&c) {
            let Some(start) = path.iter().find(|s| {
                s.kind == Kind::Start && s.capability.as_deref() == Some("long") && s.time == 1
            }) else {
                continue;
            };
            let who = start.case_id;
            let Some(short) = path.iter().find(|s| {
                s.kind == Kind::Start
                    && s.capability.as_deref() == Some("slow")
                    && s.time == 1
                    && s.case_id != who
            }) else {
                continue;
            };
            if !path
                .iter()
                .any(|s| s.kind == Kind::CancelRequest && s.time == 2 && s.case_id == short.case_id)
            {
                continue;
            }
            let Some(long) = path.iter().find(|s| {
                s.kind == Kind::Finish
                    && s.capability.as_deref() == Some("long")
                    && s.case_id == who
            }) else {
                continue;
            };
            assert_eq!(long.time, 6);
            assert_eq!(long.result.as_deref(), Some(wanted));
            found = true;
            if name == "cf-cancel" {
                let ack = path
                    .iter()
                    .find(|s| {
                        s.kind == Kind::CancelAck && s.time == 2 && s.case_id == short.case_id
                    })
                    .unwrap();
                assert_eq!(commits(ack), 0);
                assert_eq!(value(ack, "Ledger.holder"), json!("free"));
            }
        }
        assert!(
            found,
            "cancel prefix must be reached, not inferred from marginal ends"
        );
    }
}

#[test]
fn cf_cancel_does_not_undo_an_actual_prior_start_receipt() {
    for (name, wanted) in [
        ("cf-cancel-start", "conflict"),
        ("cf-cancel-start.broken-no-start", "committed"),
    ] {
        let (_, c) = capture(name);
        let mut found = false;
        for path in paths(&c) {
            let Some((start_at, start)) = path.iter().enumerate().find(|(_, s)| {
                s.kind == Kind::Start && s.capability.as_deref() == Some("long") && s.time == 1
            }) else {
                continue;
            };
            let Some((slow_at, slow)) = path.iter().enumerate().find(|(_, s)| {
                s.kind == Kind::Start
                    && s.capability.as_deref() == Some("slow")
                    && s.time == 1
                    && s.case_id != start.case_id
            }) else {
                continue;
            };
            if start_at >= slow_at {
                continue;
            }
            let Some(ack) = path
                .iter()
                .find(|s| s.kind == Kind::CancelAck && s.case_id == slow.case_id && s.time == 2)
            else {
                continue;
            };
            let Some(long) = path.iter().find(|s| {
                s.kind == Kind::Finish
                    && s.capability.as_deref() == Some("long")
                    && s.case_id == start.case_id
            }) else {
                continue;
            };
            assert_eq!(value(ack, "Ledger.holder"), json!("free"));
            assert_eq!(
                value(ack, "Ledger.marker"),
                json!(if wanted == "conflict" {
                    "taken"
                } else {
                    "free"
                })
            );
            assert_eq!(long.time, 6);
            assert_eq!(long.result.as_deref(), Some(wanted));
            found = true;
        }
        assert!(
            found,
            "start-before-cancel prefix must actually be enumerated"
        );
    }
}

#[test]
fn cf_loss_witness_is_independent_bfs_minimum_and_replay_rejects_removed_write() {
    let (m, c) = capture("cf-lww.broken-overlap");
    let id = "no_lost_update Ledger.holder";
    let mut queue = std::collections::VecDeque::from([(0, 0usize)]);
    let mut seen = BTreeSet::from([0]);
    let mut minimum = None;
    while let Some((node, d)) = queue.pop_front() {
        for e in &c.edges[node] {
            if e.targets.iter().any(|t| t == id) {
                minimum = Some(d + 1);
                break;
            }
            let next = c.canonical(e.to);
            if seen.insert(next) {
                queue.push_back((next, d + 1));
            }
        }
        if minimum.is_some() {
            break;
        }
    }
    let (mut stages, case, choices) = c.shortest(id).unwrap();
    assert_eq!(stages.len() - 1, minimum.unwrap());
    for (i, s) in stages.iter_mut().enumerate() {
        s.index = i;
    }
    let variable = m
        .state_variables
        .iter()
        .find(|v| v.name == "holder")
        .unwrap();
    let last = stages.last_mut().unwrap();
    last.violation = Some(Violation {
        state_invariant: None,
        id: id.into(),
        property_location: Some(c.location(variable.span).unwrap()),
        at_step: last.index,
        trigger_step: None,
        deadline: None,
        related_steps: vec![],
    });
    assert!(crate::reference::replay::replay_choices(&m, &stages, id, case, &choices).is_ok());
    let at = stages.iter().position(|s| s.kind == Kind::Finish).unwrap();
    assert!(at < stages.len() - 1);
    let mut tampered = stages.clone();
    tampered.remove(at);
    let mut bad_choices = choices.clone();
    bad_choices.remove(at);
    for (i, s) in tampered.iter_mut().enumerate() {
        s.index = i;
    }
    let last = tampered.last_mut().unwrap();
    last.violation.as_mut().unwrap().at_step = last.index;
    assert!(
        crate::reference::replay::replay_choices(&m, &tampered, id, case, &bad_choices).is_err()
    );
}

#[test]
fn cf_replay_rejects_loss_claim_on_an_otherwise_legal_serial_prefix() {
    // Unlike removing a stage, this prefix has truthful results, values,
    // resources, control and contiguous indices. ONLY its claimed loss is false.
    let (m, c) = capture("cf-lww");
    let id = "no_lost_update Ledger.holder";
    let (_,real_loss)=capture("cf-lww.broken-overlap");
    // Preserve the established global-witness case convention. When a concrete
    // writer case is carried, this truthful prefix's last writer is case2.
    let case=real_loss.shortest(id).unwrap().1.map(|_|2);
    let mut queue =
        std::collections::VecDeque::from([(0, vec![c.initial.clone().unwrap()], vec![None])]);
    let mut seen = BTreeSet::from([0]);
    let mut selected = None;
    while let Some((node, path, choices)) = queue.pop_front() {
        for edge in &c.edges[node] {
            let mut next = path.clone();
            next.push(edge.stage.clone());
            let mut draw = choices.clone();
            draw.push(edge.choice.clone());
            if edge.stage.kind == Kind::Finish
                && edge.stage.time == 2
                && edge.stage.case_id == Some(2)
            {
                selected = Some((next, draw));
                break;
            }
            let to = c.canonical(edge.to);
            if seen.insert(to) {
                queue.push_back((to, next, draw));
            }
        }
        if selected.is_some() {
            break;
        }
    }
    let (mut path, choices) = selected.expect("serial second finish must actually be recorded");
    for (i, s) in path.iter_mut().enumerate() {
        s.index = i;
    }
    let variable = m
        .state_variables
        .iter()
        .find(|v| v.name == "holder")
        .unwrap();
    let last = path.last_mut().unwrap();
    assert_eq!(last.result.as_deref(), Some("committed"));
    last.violation = Some(Violation {
        state_invariant: None,
        id: id.into(),
        property_location: Some(c.location(variable.span).unwrap()),
        at_step: last.index,
        trigger_step: None,
        deadline: None,
        related_steps: vec![],
    });
    assert!(
        crate::reference::replay::replay_choices(&m, &path, id, case, &choices).is_err(),
        "a truthful serial prefix has no stale overwrite; replay must not trust a forged loss marker"
    );
}

/// F35. Ledger.claim is version-checked on holder (capacity 2) and every case
/// then calls Counter.bump (`bumps: 0..1`, `+= 1`), two cases at 0. Calc: the
/// claims run 0..1; the first finish is unmarked and commits (holder = taken),
/// the second is marked by that receipt and must take the reject value
/// `conflict`; both cases bump, and the second bump leaves 0..1, so a real
/// `state_in_range` witness exists and the reference replays it. Forgery: the
/// UNMARKED committed finish is relabelled `conflict` with its values (the
/// write of taken) untouched. An unmarked task has only `committed` as a legal
/// result, so replay must refuse; choosing the branch by "the claimed result
/// is the reject value" instead of by the mark would accept it.
#[test]
fn cf_replay_refuses_a_reject_value_claimed_by_an_unmarked_checked_finish() {
    let s = "model F version \"0.2\" {
  type R = enum { committed, conflict }
  type H = enum { free, taken }
  actor Ledger {
    capacity 2
    state holder: H = free
    capability claim() -> R {
      version_check [holder] else conflict
      commits
      duration fixed 1min
      result nondet { committed }
      writes holder
      finish { holder = taken }
    }
  }
  actor Counter {
    capacity 2
    state bumps: 0..1 = 0
    capability bump() -> R { duration fixed 1min; result nondet { committed }; writes bumps; finish { bumps += 1 } }
  }
  process P { r = Ledger.claim(); b = Counter.bump(); end done }
  cut C { include [Ledger, Counter] }
  bounds { cases 2; live_tasks 1; clock 10min }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  verify reachable end.done
}";
    let m = crate::syntax::parse(s).and_then(|a| crate::lower::lower(&a)).unwrap();
    let (_, c) = explore(s, &m);
    assert_eq!(c.failure, None);
    let id = "state_in_range";
    let (mut stages, case, choices) = c.shortest(id).expect("second bump leaves 0..1");
    for (i, st) in stages.iter_mut().enumerate() {
        st.index = i;
    }
    let last = stages.last_mut().unwrap();
    last.violation = Some(Violation {
        state_invariant: None,
        id: id.into(),
        property_location: None,
        at_step: last.index,
        trigger_step: None,
        deadline: None,
        related_steps: vec![],
    });
    let claims: Vec<_> = stages
        .iter()
        .enumerate()
        .filter(|(_, st)| st.kind == Kind::Finish && st.capability.as_deref() == Some("claim"))
        .map(|(i, st)| (i, st.result.clone()))
        .collect();
    assert_eq!(
        claims.iter().map(|(_, r)| r.as_deref()).collect::<Vec<_>>(),
        vec![Some("committed"), Some("conflict")],
        "first claim commits, the marked second is rejected"
    );
    assert_eq!(
        crate::reference::replay::replay_choices(&m, &stages, id, case, &choices),
        Ok(())
    );
    // The forgery is otherwise well-formed: the stage and its Finish event carry
    // the same result, and the Commit event goes because `conflict` does not
    // commit, so the event shape check alone cannot refuse it.
    let mut forged = stages.clone();
    let st = &mut forged[claims[0].0];
    st.result = Some("conflict".into());
    st.events.retain(|e| !matches!(e, crate::explain::Event::Business(b) if b.kind == Kind::Commit));
    assert_eq!(st.events.len(), 1);
    for e in &mut st.events {
        if let crate::explain::Event::Business(b) = e {
            assert_eq!(b.kind, Kind::Finish);
            b.result = Some("conflict".into());
        }
    }
    assert!(
        crate::reference::replay::replay_choices(&m, &forged, id, case, &choices).is_err(),
        "an unmarked checked finish cannot claim the reject value"
    );
}
