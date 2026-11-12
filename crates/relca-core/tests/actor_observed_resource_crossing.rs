//! ADR 0032 x ADR 0033: one input with a Task that uses a shared resource and
//! a Task that writes and reads shared Actor observed state. Both the start
//! conditions (Actor slot, resource; the input has no executors) and the full
//! expansion of the write/read order must hold in the engine and in the
//! independent reference. Every run also keeps `no_resource_overcommit`
//! satisfied in both interpreters: Seat and the Actor slots are never
//! exceeded. `posted` is a bounded integer, so the implicit `state_in_range`
//! (ADR 0029 U2) is also checked: satisfied, since no variant writes past
//! its upper bound (requires keeps 0..1; the variants without it use 0..2).
//!
//! Hand calculation (cases 2, both arrive at 0; Seat capacity 1):
//! each case runs hold (3min, Seat 1) then post (1min, Seat 1, requires
//! posted == 0, finish posted += 1). Seat serialises all three occupying
//! segments: the first post finishes at 4 (Ha, Pa) or 7 (Ha, Hb, Pa); the
//! other case's post is refused only once its hold is done and Seat is free,
//! which is always after 3 + 3 + 1 = 7 occupied ticks: denied at 7.
//! Either case can be the committer: both ends are reachable for each case.
//! Work counts occupancy only: posted 3 + 1 = 4, denied 3 (refusal is no work).
//! Control without `uses`: both holds run at 0..3 (Clerk capacity 2), Ledger
//! capacity 1 admits one post at 3..4, the other is refused at 4: both ends
//! complete at exactly 4.
//! Control without `requires`: no refusal, so `denied` is unreachable.
//!
//! BRANCH (a process `if` that reads shared `Ledger.posted`, with Seat):
//! each case runs hold (3min, Seat) then `if Ledger.posted == 0 { post }
//! else { end skipped }`; posted is 0..2 so two posts do not overflow. The
//! first holder X ends its hold at 3 and reads posted 0. At 3 Seat goes to
//! X's post (3..4, posted 1 at 4; Y holds 4..7, reads 1 at 7: skipped at 7)
//! or to Y's hold (3..6; Y reads 0 at 6, then the two posts run 6..7 and
//! 7..8 in either order). Per case: posted 4..8 with work 3 + 1 = 4, skipped
//! 7..7 with work 3. No read coincides with a write, so the order of the
//! Seat grants alone produces the spread.
use relca_core::{engine, ir, lower, outcome, reference, syntax};

const SOURCE: &str = r#"model M version "0.2" {
  type R = enum { committed, rejected }
  resource Seat capacity 1
  actor Ledger {
    capacity 1
    state posted: 0..1 = 0
    capability post() -> R {
      uses Seat: 1
      requires posted == 0 else rejected
      duration fixed 1min
      result nondet { committed }
      writes posted
      finish { posted += 1 }
    }
  }
  actor Clerk { capacity 2; capability hold() -> R { uses Seat: 1; duration fixed 3min; result nondet { committed } } }
  process P { h = Clerk.hold(); r = Ledger.post(); if r == committed { end posted } else { end denied } }
  cut C { include [Ledger, Clerk] }
  bounds { cases 2; live_tasks 1; clock 20min }
  arrivals fixed [0min,0min]
  verify safety no_resource_overcommit
}"#;

/// Per case: (end name, completed_at min..max, work).
type Ends = Vec<Vec<(String, (u64, u64), u64)>>;

fn model(src: &str) -> ir::Model {
    lower::lower(&syntax::parse(src).unwrap()).unwrap()
}

fn change(src: &str, from: &str, to: &str) -> String {
    assert_eq!(src.matches(from).count(), 1, "{from}");
    src.replacen(from, to, 1)
}

/// (end name, completed_at min..max, work) per case, engine == reference.
fn ends(src: &str) -> Ends {
    let m = model(src);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
    assert_eq!(e.terminals, r.outcome.terminals);
    assert_eq!(e.properties, r.outcome.properties);
    for (name, properties) in [
        ("engine", &e.properties),
        ("reference", &r.outcome.properties),
    ] {
        assert_eq!(
            properties
                .iter()
                .map(|p| (p.property, p.verdict))
                .collect::<Vec<_>>(),
            vec![
                (
                    ir::Property::NoResourceOvercommit,
                    relca_core::Verdict::Satisfied
                ),
                (ir::Property::StateInRange, relca_core::Verdict::Satisfied),
            ],
            "{name}"
        );
    }
    assert_eq!(r.verdict(), relca_core::Verdict::Satisfied);
    e.case_outcomes
        .unwrap()
        .iter()
        .map(|c| {
            c.terminals
                .iter()
                .map(|t| {
                    let outcome::CaseTerminalKind::End(id) = t.kind else {
                        panic!("unexpected rejection")
                    };
                    let done = t.completed_at.unwrap();
                    let work = t.work.unwrap();
                    assert_eq!(work.min, work.max);
                    (m.ends[id.0].clone(), (done.min, done.max), work.min)
                })
                .collect()
        })
        .collect()
}

fn both(posted: (u64, u64), denied: (u64, u64)) -> Ends {
    let case = vec![
        ("posted".to_string(), posted, 4),
        ("denied".to_string(), denied, 3),
    ];
    vec![case.clone(), case]
}

#[test]
fn resource_gate_and_shared_state_order_are_both_expanded() {
    assert_eq!(ends(SOURCE), both((4, 7), (7, 7)));
}

#[test]
fn removing_the_resource_uses_changes_only_the_resource_serialisation() {
    let src = change(SOURCE, "      uses Seat: 1\n", "");
    let src = change(&src, "uses Seat: 1; ", "");
    assert_eq!(ends(&src), both((4, 4), (4, 4)));
}

#[test]
fn removing_the_state_guard_removes_the_order_dependent_refusal() {
    let src = change(SOURCE, "      requires posted == 0 else rejected\n", "");
    let src = change(&src, "0..1 = 0", "0..2 = 0");
    let got = ends(&src);
    for case in &got {
        assert!(case.iter().all(|(end, _, _)| end == "posted"), "{got:?}");
    }
}

const BRANCH: &str = r#"model M version "0.2" {
  type R = enum { committed, rejected }
  resource Seat capacity 1
  actor Ledger {
    capacity 1
    state posted: 0..2 = 0
    capability post() -> R {
      uses Seat: 1
      duration fixed 1min
      result nondet { committed }
      writes posted
      finish { posted += 1 }
    }
  }
  actor Clerk { capacity 2; capability hold() -> R { uses Seat: 1; duration fixed 3min; result nondet { committed } } }
  process P { h = Clerk.hold(); if Ledger.posted == 0 { r = Ledger.post(); end posted } else { end skipped } }
  cut C { include [Ledger, Clerk] }
  bounds { cases 2; live_tasks 1; clock 20min }
  arrivals fixed [0min,0min]
  verify safety no_resource_overcommit
}"#;

#[test]
fn process_branch_on_shared_state_with_resource_expands_both_seat_orders() {
    let case = vec![
        ("posted".to_string(), (4, 8), 4),
        ("skipped".to_string(), (7, 7), 3),
    ];
    assert_eq!(ends(BRANCH), vec![case.clone(), case]);
}
