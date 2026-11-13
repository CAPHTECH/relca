use super::super::arrivals_tests::{tree, world};
use super::*;

#[test]
fn ack_only_due_task_still_checks_frozen_performance_consistency() {
    let m = crate::lower::lower(
        &crate::syntax::parse(include_str!(
            "../../../../../fixtures/serial-time/model.relca"
        ))
        .unwrap(),
    )
    .unwrap();
    let mut t = tree(&m, Limits::default(), vec![0]);
    let mut w = world(&m, 1);
    let Stmt::Call(call) = &m.process.body[0].kind else {
        panic!("serial first call")
    };
    let f = &mut w.cases[0].payload_mut().unwrap().frame;
    f.control.clear();
    t.cases[0]
        .create(f, std::slice::from_ref(call), None)
        .unwrap();
    let task = &mut f.tasks[0];
    task.phase = Phase::Running;
    task.start = Some(0);
    task.finish = None;
    task.ack = Some(0);
    // This ordinary capability has no performance profile. A frozen selector
    // is therefore corrupt, even when this transition is cancellation only.
    task.frozen_performance = Some(FrozenPerformance {
        profile: 0,
        row: 0,
        executor: None,
    });
    assert!(matches!(t.walk(w), Err(StopReason::SemanticAbort)));
    assert_eq!(t.stats.cancellation_completions, 0);
}
