//! Internal cfg(test) child of engine/trace.rs (no public API change).
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
fn model(s: &str) -> ir::Model {
    crate::syntax::parse(s)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap()
}
fn change(s: &str, a: &str, b: &str) -> String {
    let t = s.replace(a, b);
    assert_ne!(s, t);
    t
}
fn value(stage: &Stage, name: &str) -> Json {
    let v = json!(stage);
    v["values"]["observed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == name)
        .unwrap()["value"]
        .clone()
}
fn path(c: &Capture, target: &str) -> WitnessPath {
    let (mut stages, case, choices) = c.shortest(target).unwrap();
    for (i, s) in stages.iter_mut().enumerate() {
        s.index = i;
    }
    let last = stages.last_mut().unwrap();
    last.violation = Some(Violation {
        state_invariant: None,
        id: target.into(),
        property_location: None,
        at_step: last.index,
        trigger_step: None,
        deadline: None,
        related_steps: vec![],
    });
    (stages, case, choices)
}
fn replay(m: &ir::Model, p: &WitnessPath, target: &str) -> Result<(), ReplayReason> {
    crate::reference::replay::replay_choices(m, &p.0, target, p.1, &p.2)
}

#[test]
fn aset_shared_final_bank_and_late_arrival_never_reinitialize_existing_keys() {
    for late in [false, true] {
        let s = source("aset-cases");
        let s = if late {
            change(&s, "[0min,0min]", "[0min,3min]")
        } else {
            s
        };
        let m = model(&s);
        let (_, c) = explore(&s, &m);
        assert_eq!(c.failure, None);
        assert_eq!(value(c.initial.as_ref().unwrap(), "Ledger.keys"), json!([]));
        let finals: Vec<_> = c
            .edges
            .iter()
            .flatten()
            .filter(|e| e.stage.kind == Kind::Finish && e.stage.case_id == Some(2))
            .collect();
        assert!(!finals.is_empty());
        for edge in finals {
            assert_eq!(edge.stage.time, if late { 4 } else { 2 });
            assert_eq!(value(&edge.stage, "Ledger.keys"), json!([1, 2]));
        }
        if late {
            for edge in c
                .edges
                .iter()
                .flatten()
                .filter(|e| e.stage.kind == Kind::Received && e.stage.case_id == Some(2))
            {
                assert_eq!(value(&edge.stage, "Ledger.keys"), json!([1]));
            }
        }
    }
}

#[test]
fn aset_cancel_ack_has_no_atomic_effect_but_start_insert_survives() {
    for (name, at_ack) in [
        ("aset-cancel", json!([])),
        ("aset-start-insert", json!([1])),
    ] {
        let s = source(name);
        let m = model(&s);
        let (_, c) = explore(&s, &m);
        assert_eq!(c.failure, None);
        let acks: Vec<_> = c
            .edges
            .iter()
            .flatten()
            .filter(|e| {
                e.stage.kind == Kind::CancelAck && e.stage.actor.as_deref() == Some("Ledger")
            })
            .collect();
        assert!(!acks.is_empty());
        for ack in acks {
            assert_eq!(ack.stage.time, 2);
            assert_eq!(value(&ack.stage, "Ledger.keys"), at_ack);
            assert!(!json!(ack.stage.events).to_string().contains("commit"));
        }
    }
}

#[test]
fn aset_selected_integer_bundle_rolls_back_all_values_before_any_commit() {
    let s = source("aset-atomic-rollback.broken-initial-one");
    let m = model(&s);
    let (_, c) = explore(&s, &m);
    let target = "state_in_range";
    let p = path(&c, target);
    assert_eq!(replay(&m, &p, target), Ok(()));
    let last = p.0.last().unwrap();
    assert_eq!(last.time, 1);
    assert_eq!(value(last, "Ledger.keys"), json!([]));
    assert_eq!(value(last, "registered"), json!("false"));
    assert_eq!(value(last, "n"), json!("1"));
    assert!(!json!(last.events).to_string().contains("commit"));
}

#[test]
fn aset_replay_rejects_order_duplicates_zero_out_of_range_and_stringified_sets() {
    let s = source("aset-replay-display");
    let m = model(&s);
    let (_, c) = explore(&s, &m);
    let target = m.contracts[1].property_id(ir::ClauseId(1));
    let original = path(&c, &target);
    assert_eq!(replay(&m, &original, &target), Ok(()));
    let index = original.0.len() - 1;
    let faults: Vec<Json> = serde_json::from_str(include_str!(
        "../../../../fixtures/aset-replay-display.broken-shorter-wait/replay_faults.json"
    ))
    .unwrap();
    for bad in faults {
        let mut altered = original.clone();
        let row = altered.0[index]
            .values
            .observed
            .iter_mut()
            .find(|v| v.name == "Ledger.keys")
            .unwrap();
        assert_eq!(json!(row.value), json!([1, 2]));
        // A closed display decoder may refuse malformed arrays before replay.
        // If it accepts a value, replay must independently refuse the forged
        // record; sorting/deduplication must not turn it into a valid prefix.
        if let Ok(value) = serde_json::from_value(bad) {
            row.value = value;
            assert!(
                replay(&m, &altered, &target).is_err(),
                "never sort/dedup a forged snapshot"
            );
        }
    }
    let mut altered = original.clone();
    let row = altered.0[0]
        .values
        .observed
        .iter_mut()
        .find(|v| v.name == "Ledger.keys")
        .unwrap();
    assert_eq!(json!(row.value), json!([]));
    row.value = serde_json::from_value(json!([1])).unwrap();
    assert!(
        replay(&m, &altered, &target).is_err(),
        "initial set is empty exactly once, not any valid bitset"
    );
}

#[test]
fn aset_rejected_finish_has_no_commit_in_capture_and_reference_event_predicate() {
    let s = source("aset-dup");
    let s = change(
        &s,
        "contract C",
        "contract I{invariant Ledger.post.finish(rejected_duplicate) -> registered==false};contract C",
    );
    let m = model(&s);
    let (_, c) = explore(&s, &m);
    let target = m.contracts[0].property_id(ir::ClauseId(0));
    let p = path(&c, &target);
    assert_eq!(replay(&m, &p, &target), Ok(()));
    let last = p.0.last().unwrap();
    assert_eq!(last.time, 2);
    assert_eq!(last.kind, Kind::Finish);
    assert_eq!(value(last, "Ledger.keys"), json!([1]));
    assert!(!json!(last.events).to_string().contains("commit"));
    let commits =
        p.0.iter()
            .flat_map(|s| s.events.iter())
            .filter(|e| matches!(e,Event::Business(b) if b.kind==Kind::Commit))
            .count();
    assert_eq!(commits, 1);
    assert_eq!(p.0.iter().filter(|s| s.kind == Kind::Finish).count(), 2);
}

#[test]
fn aset_bit63_has_case64_display_and_state_eq_tracks_the_entire_bitset() {
    let source = source("aset-limit");
    let m = model(&source);
    let c = Capture::new(&source, &m);
    let base = State::initial(&m, 0);
    let id = m
        .state_variables
        .iter()
        .position(|v| v.name == "keys")
        .unwrap();
    let mut states = HashSet::new();
    for (bits, want) in [
        (0, json!([])),
        (1, json!([1])),
        (2, json!([2])),
        (3, json!([1, 2])),
        (1u64 << 63, json!([64])),
    ] {
        let mut s = base.clone();
        s.state_values[id] = ir::StateValue::Set(bits);
        let values = json!(c.values(&s, &m).unwrap());
        let row = values["observed"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == "Ledger.keys")
            .unwrap();
        assert_eq!(row["value"], want);
        states.insert(s);
    }
    assert_eq!(states.len(), 5);
}

/// R06: a late arrival's Initialize snapshot of a shared set is decoded only
/// from a strictly ascending, duplicate-free array. Hand derivation (unit
/// min): Ledger capacity 1, case 1 posts [0,1), case 2 [1,2), so when case 3
/// arrives at 3 the shared keys are [1,2]. The earliest invariant
/// counterexample is case 1's Probe finish at 5 (post 1, wait 3, Probe [4,5)),
/// after case 3's arrival, so the path carries case 3's Initialize at 3.
/// [2,1] and [1,1,2] name the same bitset; replay must refuse them, not sort or
/// deduplicate them into the real value.
#[test]
fn aset_late_arrival_initialize_snapshot_refuses_unsorted_or_duplicated_sets() {
    let s = source("aset-replay-display");
    let s = change(
        &s,
        "arrivals fixed [0min,0min]",
        "arrivals fixed [0min,0min,3min]",
    );
    let s = change(&s, "cases 2;", "cases 3;");
    let m = model(&s);
    let (_, c) = explore(&s, &m);
    let target = m.contracts[1].property_id(ir::ClauseId(1));
    let original = path(&c, &target);
    assert_eq!(replay(&m, &original, &target), Ok(()));
    assert_eq!(original.0.last().unwrap().time, 5);
    let at = original
        .0
        .iter()
        .position(|st| {
            st.kind == Kind::Initialize
                && st.time == 3
                && st
                    .events
                    .iter()
                    .any(|e| matches!(e, Event::Value(v) if v.name == "Ledger.keys"))
        })
        .expect("case 3 Initialize snapshot at 3");
    for bad in [vec![2u64, 1], vec![1, 1, 2]] {
        let mut altered = original.clone();
        let mut hit = 0;
        for e in &mut altered.0[at].events {
            if let Event::Value(v) = e
                && v.name == "Ledger.keys"
            {
                assert_eq!(v.value, crate::explain::DisplayValue::CaseSet(vec![1, 2]));
                v.value = crate::explain::DisplayValue::CaseSet(bad.clone());
                hit += 1;
            }
        }
        assert_eq!(hit, 1);
        assert!(
            replay(&m, &altered, &target).is_err(),
            "forged {bad:?} must not be normalised"
        );
    }
}
