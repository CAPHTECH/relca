//! Internal state tests linked from src/engine.rs:
//! #[cfg(test)] #[path = "../tests/support/conflict_private_state.rs"] mod conflict_private_state;
//! Field spellings may be mechanically adapted, never the identity assertions.
use super::*;

#[test]
fn cf_stale_only_distinguishes_direct_eq_hashset_and_finish_result() {
    let src = include_str!("../../../../fixtures/cf-version/model.relca");
    let m = crate::syntax::parse(src)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap();
    let holder = ir::StateVarId(
        m.state_variables
            .iter()
            .position(|v| v.name == "holder")
            .unwrap(),
    );
    let mut a = State::initial(&m, 0);
    a.time = 1;
    a.work = 1;
    a.tasks.push(Task {
        frozen_performance: None,
        executor: None,
        binding: ir::BindingId(0),
        capability: ir::CapabilityId(0),
        lifecycle: Lifecycle::Running {
            finish: 1,
            ack: None,
        },
        stale: vec![],
    });
    let mut b = a.clone();
    b.tasks[0].stale = vec![holder];
    assert_eq!(a.state_values, b.state_values);
    assert_eq!(a.time, b.time);
    assert_eq!(a.pc, b.pc);
    assert_eq!(a.results, b.results);
    assert_ne!(
        a, b,
        "direct Eq must include stale even if Hash was already updated"
    );
    assert_eq!(
        std::collections::HashSet::from([a.clone(), b.clone()]).len(),
        2
    );
    let finish = |s: &State| {
        complete(
            s,
            Due {
                i: 0,
                finish: 1,
                ack: None,
            },
            &m,
            &mut Room(DEFAULT_STATE_BUDGET),
        )
        .unwrap()
    };
    let fresh = finish(&a);
    let stale = finish(&b);
    assert_eq!(fresh.len(), 1);
    assert_eq!(stale.len(), 1);
    assert_eq!(fresh[0].results[0], Some(m.capabilities[0].results[0]));
    assert_eq!(
        stale[0].results[0],
        Some(m.capabilities[0].version_check.as_ref().unwrap().reject)
    );
    let claims = m
        .state_variables
        .iter()
        .position(|v| v.name == "claims")
        .unwrap();
    assert_eq!(fresh[0].state_values[claims], ir::StateValue::Integer(1));
    assert_eq!(stale[0].state_values[claims], ir::StateValue::Integer(0));
}

#[test]
fn cf_loss_only_remains_semantic_after_writer_task_is_removed() {
    let src = include_str!("../../../../fixtures/cf-lww/model.relca");
    let m = crate::syntax::parse(src)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap();
    let holder = ir::StateVarId(
        m.state_variables
            .iter()
            .position(|v| v.name == "holder")
            .unwrap(),
    );
    let a = State::initial(&m, 0);
    let mut b = a.clone();
    b.lost_updates = vec![holder];
    assert!(a.tasks.is_empty() && b.tasks.is_empty());
    assert_eq!(a.state_values, b.state_values);
    assert_ne!(a, b);
    assert_eq!(std::collections::HashSet::from([a, b]).len(), 2);
}
