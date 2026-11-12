//! A constant calendar must not turn known physical/FIFO deadlock into a cut.
use crate::{Verdict, engine, ir, lower, reference, syntax};

const SOURCE: &str = r#"model StaticFifoCalendar version "0.2" {
  type Role = enum { owner, waiter }
  type R = enum { ok }
  calendar One through 21min on_close continue { [0min,21min) capacity 1 }
  calendar Two through 21min on_close continue { [0min,21min) capacity 2 }
  resource Slot capacity 1 calendar One
  actor Board { capacity 2; calendar Two
    state requests: 0..2 = 0
    capability claim() -> Role {
      requires requests == 0 else waiter
      duration fixed 1min
      result nondet { owner }
      start { requests += 1 }
    }
  }
  actor A { capacity 1; calendar One
    capability heavy() -> R { uses Slot: 2; duration fixed 1min; result nondet { ok } }
    capability light() -> R { uses Slot: 1; duration fixed 1min; result nondet { ok } }
  }
  process Flow {
    role = Board.claim()
    if role == owner { h = A.heavy() }
    else { wait 2min; l = A.light() }
    end done
  }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 2; clock 20min }
  verify reachable end.done
  verify safety no_resource_overcommit
}"#;

fn check(source: &str) {
    let model = lower::lower(&syntax::parse(source).unwrap()).unwrap();
    let engine = engine::explore(&model);
    let mut reference = reference::explore(&model);
    assert_eq!(reference.stop, reference::StopReason::Complete);
    for out in [&engine, &reference.outcome] {
        assert!(
            !out.truncated,
            "a table cannot release the over-demand FIFO head"
        );
        assert!(out.diagnostics.iter().any(|d| d.code == "W-DEADLOCK-0"));
        assert!(
            out.case_outcomes
                .as_ref()
                .unwrap()
                .iter()
                .all(|c| c.terminals.is_empty())
        );
        assert!(
            out.properties
                .iter()
                .any(|p| matches!(p.property, ir::Property::NoResourceOvercommit)
                    && p.verdict == Verdict::Satisfied)
        );
        assert!(
            out.properties
                .iter()
                .any(|p| matches!(p.property, ir::Property::ReachableEnd(_))
                    && p.verdict == Verdict::Violated)
        );
    }
    reference
        .outcome
        .diagnostics
        .drain(..model.diagnostics.len());
    assert_eq!(engine, reference.outcome);
    let explained = crate::explain::explain(source);
    let witness = explained
        .explanation
        .counterexamples
        .iter()
        .find(|c| c.id == "diagnostic:W-DEADLOCK-0")
        .unwrap();
    assert_eq!(
        witness.replay.status,
        crate::explain::ReplayStatus::Verified
    );
    assert!(
        witness
            .steps
            .iter()
            .any(|s| s.kind == crate::explain::Kind::Deadlock && s.time == 2)
    );
}

#[test]
fn constant_calendar_preserves_known_fifo_deadlock_in_both_interpreters_and_replay() {
    // Hand order: claim owner [0,1), refused waiter sleeps [0,2). At 1,
    // heavy queues with demand 2 > physical 1. At 2, light cannot bypass it.
    check(SOURCE);
    let legacy = SOURCE
        .lines()
        .filter(|l| !l.trim_start().starts_with("calendar "))
        .collect::<Vec<_>>()
        .join("\n")
        .replace(" calendar One", "")
        .replace(" calendar Two", "");
    check(&legacy);
}

#[test]
fn fixed_call_order_does_not_invent_a_future_calendar_release() {
    let ordered = SOURCE.replace(
        "policy fifo tie_by case_id",
        "policy observable fifo tie_by case_id then call_order assign first_idle",
    );
    check(&ordered);
    // A different Actor removes the per-Actor FIFO edge; only the declared
    // common-resource call order still prevents bypass of the impossible head.
    let separate = ordered.replace("  process Flow {", "  actor B { capacity 1; calendar One; capability light() -> R { uses Slot: 1; duration fixed 1min; result nondet { ok } } }\n  process Flow {")
        .replace("l = A.light()", "l = B.light()");
    check(&separate);
}
