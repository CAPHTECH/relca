//! Public-IR refusal probes, not another numerical acceptance oracle.
use super::*;
fn model() -> ir::Model {
    let source = r#"model IR version "0.2" {
        type R = enum { ok, conflict } type H = enum { free, taken }
        actor A { capacity 2; state holder: H = free
            capability claim() -> R { duration fixed 1min; result nondet { ok };
                version_check [holder] else conflict resolve priority descending;
                writes holder; finish { holder = taken } }
            capability peer() -> R { duration fixed 1min; result nondet { ok };
                version_check [holder] else conflict resolve priority descending;
                writes holder; finish { holder = taken } } }
        process Flow { r = A.claim(); end done }
        bounds { cases 2; live_tasks 2; clock 5min }
        arrivals fixed [0min, 0min] case_priority fixed [0, 9]
        policy observable priority descending
        concurrent cases 2 on_overflow wait_external priority descending
        verify safety no_resource_overcommit
    }"#;
    crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap()
}
fn refuses(m: &ir::Model) {
    let out = crate::engine::explore(m);
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0" && d.message.contains("invalid_priority_ir")),
        "{:?}",
        out.diagnostics
    );
    assert_eq!(
        crate::reference::explore(m).stop,
        crate::reference::StopReason::SemanticAbort
    );
}
#[test]
fn priority_ir_refuses_count_class_scope_target_and_pair_forgery_in_both_entries() {
    let m = model();
    assert!(validate_priority(&m).is_ok());
    assert_eq!(
        crate::reference::explore(&m).stop,
        crate::reference::StopReason::Complete
    );
    let mut bad = m.clone();
    bad.case_priority.as_mut().unwrap().ranks.pop();
    refuses(&bad);
    let mut bad = m.clone();
    bad.case_priority
        .as_mut()
        .unwrap()
        .classes
        .policy
        .push(ir::ActorId(usize::MAX));
    refuses(&bad);
    let mut bad = m.clone();
    bad.case_priority = None;
    refuses(&bad);
    let mut bad = m.clone();
    bad.capabilities[0]
        .version_check
        .as_mut()
        .unwrap()
        .variables[0] = ir::StateVarId(usize::MAX);
    refuses(&bad);
    let mut bad = m.clone();
    let classes = &mut bad.case_priority.as_mut().unwrap().classes;
    classes.environment.append(&mut classes.policy);
    refuses(&bad);
    let mut bad = m.clone();
    bad.capabilities[1].version_check.as_mut().unwrap().resolve = None;
    refuses(&bad);
    let mut bad = m.clone();
    bad.start_policy.as_mut().unwrap().call_order = true;
    refuses(&bad);
    let mut bad = m;
    bad.concurrent_cases.as_mut().unwrap().capacity = 0;
    refuses(&bad);
}
