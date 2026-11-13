use relca_core::{engine, lower, reference, report, syntax};

#[test]
fn expanded_n32_uses_the_standard_test_thread_stack() {
    let src = include_str!("bounded_repetition/models/L2-32.relca");
    let ast = syntax::parse(src).unwrap();
    let m = lower::lower(&ast).unwrap();
    let e = engine::explore(&m);
    assert!(!e.truncated);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    let _ = report::check(src);
}

#[test]
fn a_long_expansion_returns_the_existing_logical_limit_not_stack_overflow() {
    let src = r#"model LongBody version "0.2" {
        type V = enum { ok }
        actor A { capacity 1; capability c() -> V { duration fixed 1min; result nondet { ok } } }
        process P { repeat work at_most 32 {
            a = A.c(); b = A.c(); c = A.c(); d = A.c(); e = A.c(); again
        } on_limit { end done } }
        cut C { include [A] }
        bounds { cases 1; live_tasks 1; clock 1000min }
        verify safety no_resource_overcommit
    }"#;
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    let e = engine::explore(&m);
    assert!(!e.truncated);
    assert_eq!(e.terminals[0].elapsed.min, 160);
    let r = reference::explore_with_limits(&m, reference::Limits::default());
    assert_eq!(r.stop, reference::StopReason::EnumerationLimit);
    assert!(r.outcome.terminals.is_empty());
}
