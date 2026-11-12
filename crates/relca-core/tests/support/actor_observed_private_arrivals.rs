//! Link as a cfg(test) child of engine/arrivals.rs, not engine.rs.
use super::*;

fn input() -> ir::Model {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = std::fs::read_to_string(root.join("fixtures/aobs-shared-order/model.relca")).unwrap();
    crate::syntax::parse(&src)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap()
}

#[test]
fn aobs_global_shared_slot_alone_changes_eq_hash_identity() {
    let m = input();
    let a = initial(&m, 2);
    let mut b = a.clone();
    let id = m
        .state_variables
        .iter()
        .position(|v| v.name == "posted")
        .unwrap();
    assert_eq!(a.shared[id], ir::StateValue::Integer(0));
    b.shared[id] = ir::StateValue::Integer(1);
    assert_ne!(a, b);
    let mut set = HashSet::new();
    set.insert(a.clone());
    set.insert(b.clone());
    assert_eq!(set.len(), 2, "shared payload must enter Eq/Hash");
    use std::hash::{Hash, Hasher};
    let hash = |s: &Global| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    };
    assert_ne!(
        hash(&a),
        hash(&b),
        "M22 removing shared from Hash must be detected even if Eq retains it"
    );
}

#[test]
fn aobs_graph_root_has_one_fixed_bank_not_per_case_seed_product() {
    let m = input();
    let a = initial(&m, 2);
    assert_eq!(a.shared, vec![ir::StateValue::Integer(0)]);
    assert!(a.cases.iter().all(|s| matches!(s, Slot::Future)));
    let mut control = m.clone();
    control.state_variables.clear();
    // Only the initial control state is compared: the process/writes are not
    // executed after removing their variable declarations.
    let b = initial(&control, 2);
    assert_eq!(a.time, b.time);
    assert_eq!(a.cursor, b.cursor);
    assert_eq!(a.phase, b.phase);
    assert_eq!(a.cases, b.cases);
}

#[test]
fn aobs_fixed_bank_adds_zero_nodes_generated_or_held_to_control_only_graph() {
    let src = "model M version \"0.2\" { actor A {} process P {end done} cut C {include [A]} bounds {cases 2;live_tasks 1;clock 4min} arrivals fixed [0min,2min] verify reachable end.done }";
    let with = src.replace(
        "actor A {}",
        "actor A {state x:bool=true;state answer:optional<bool>}",
    );
    assert_ne!(src, with);
    let lower = |s: &str| {
        crate::syntax::parse(s)
            .and_then(|a| crate::lower::lower(&a))
            .unwrap()
    };
    let plain = lower(src);
    let state = lower(&with);
    assert_eq!(
        compatibility_counts(&plain),
        compatibility_counts(&state),
        "Fixed initialization is payload only, not an extra transition or seed"
    );
}
