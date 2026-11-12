//! ADR 0032 mutation survivors (bbc444c), closed in the core.
//! The CLI `explain` entries live in
//! `crates/relca-cli/tests/actor_observed_mutation_fixes.rs`.
//!
//! Hand calculations:
//! - START_ONLY (A04, A12, A23): Desk writes shared `busy` only in `start`.
//!   With `capacity 2` and `cases 2` two Tasks could write at once, so lower
//!   refuses with U-SEM-0 G5 spanning the Capability declaration (from
//!   `capability` to its closing brace). Forging `capacity 2` into the IR of
//!   the accepted `capacity 1` control must be refused by both interpreters.
//! - AND_GUARD (A10, A19) and INT_GUARD (A11, A19, A20): case 1 arrives at 0,
//!   reads `ready` false (`n` 0), runs Desk 0..2 and writes true (1) at 2.
//!   Case 2 arrives at 2: read before the write gives Desk 2..4, read after
//!   gives Alt 2..3. Case 1 completes at 2..2, case 2 at 3..4. The `hit`
//!   atom is per-case and always true for `hit == false`; it must not hide
//!   the shared atom of the conjunction.
//! - EXTERNAL (A13): aobs-owner with B at `capacity 1`; moving A's Capability
//!   to B makes B write A's observed state. Only the owner may write: U-SEM-0.
//! - START_OVERFLOW (A18): `n: 0..1`, `start { n += 1 }`, arrivals 0 and 2.
//!   Case 1 starts at 0 (n 0 -> 1), ends at 1. Case 2 starts at 2 and reads
//!   the shared value 1 written by case 1: attempted 2 > 1 at time 2, phase
//!   start, one range violation in each interpreter.
use relca_core::{diag, engine, ir, lower, outcome, reference, report, syntax};
use serde_json::{Value, json};

const START_ONLY: &str = "model M version \"0.2\" {\n  type R = enum { ok }\n  actor Desk { capacity 2; state busy: bool = false; capability work() -> R { start { busy = true }; duration fixed 1min; result nondet { ok } } }\n  process P { b = Desk.work(); end done }\n  cut C { include [Desk] }\n  bounds { cases 2; live_tasks 2; clock 10min }\n  arrivals fixed [0min,0min]\n  verify safety no_resource_overcommit\n}\n";

const AND_GUARD: &str = "model M version \"0.2\" {\n  type R = enum { ok }\n  observed case { hit: bool = false }\n  actor Desk { capacity 1; state ready: bool = false; capability work() -> R { writes ready; finish { ready = true }; duration fixed 2min; result nondet { ok } } }\n  actor Alt { capacity 1; capability other() -> R { duration fixed 1min; result nondet { ok } } }\n  process P { if Desk.ready == true && hit == false { a = Alt.other() } else { b = Desk.work() }; end done }\n  cut C { include [Desk,Alt] }\n  bounds { cases 2; live_tasks 1; clock 10min }\n  arrivals fixed [0min,2min]\n  verify safety no_resource_overcommit\n}\n";

const INT_GUARD: &str = "model M version \"0.2\" {\n  type R = enum { ok }\n  actor Desk { capacity 1; state n: 0..1 = 0; capability work() -> R { writes n; finish { n = 1 }; duration fixed 2min; result nondet { ok } } }\n  actor Alt { capacity 1; capability other() -> R { duration fixed 1min; result nondet { ok } } }\n  process P { if Desk.n >= 1 { a = Alt.other() } else { b = Desk.work() }; end done }\n  cut C { include [Desk,Alt] }\n  bounds { cases 2; live_tasks 1; clock 10min }\n  arrivals fixed [0min,2min]\n  verify safety no_resource_overcommit\n}\n";

const START_OVERFLOW: &str = "model M version \"0.2\" {\n  type R = enum { ok }\n  actor A { capacity 1; state n: 0..1 = 0; capability run() -> R { start { n += 1 }; duration fixed 1min; result nondet { ok } } }\n  process P { a = A.run(); end done }\n  cut C { include [A] }\n  bounds { cases 2; live_tasks 1; clock 6min }\n  arrivals fixed [0min,2min]\n  verify reachable end.done\n}\n";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}

fn change(s: &str, from: &str, to: &str) -> String {
    let t = s.replacen(from, to, 1);
    assert_ne!(s, t, "replacement must affect the control: {from}");
    t
}

fn model(s: &str) -> ir::Model {
    lower::lower(&syntax::parse(s).unwrap()).unwrap_or_else(|d| panic!("{d:?}\n{s}"))
}

fn refused_by_both(m: &ir::Model) {
    let e = engine::explore(m);
    let r = reference::explore(m).outcome;
    for (name, out) in [("engine", &e), ("reference", &r)] {
        assert!(
            out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0),
            "{name} accepted forged IR: {out:?}"
        );
        assert!(out.terminals.is_empty(), "{name}: {out:?}");
    }
}

/// (case id, completed_at min..max) of every case terminal.
fn completions(o: &outcome::Outcome) -> Vec<(usize, (u64, u64))> {
    o.case_outcomes
        .iter()
        .flatten()
        .flat_map(|c| {
            c.terminals.iter().map(move |t| {
                let r = t.completed_at.unwrap();
                (c.case_id.0, (r.min, r.max))
            })
        })
        .collect()
}

/// Both interpreters give case 1 2..2 and case 2 3..4, and agree on the rest.
fn both_orders_kept(s: &str) {
    let m = model(s);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    let want = vec![(0, (2, 2)), (1, (3, 4))];
    assert_eq!(completions(&e), want, "engine");
    assert_eq!(completions(&r.outcome), want, "reference");
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
    assert_eq!(e.terminals, r.outcome.terminals);
    assert_eq!(e.properties, r.outcome.properties);
    assert!(
        !e.diagnostics
            .iter()
            .any(|d| d.code.starts_with("W-UNREACH")),
        "Alt.other is reachable after the write: {:?}",
        e.diagnostics
    );
}

/// A04: the start-only writer is refused at the Capability declaration.
#[test]
fn a04_start_only_shared_writer_is_refused_at_the_capability_span() {
    let v = serde_json::to_value(report::check(START_ONLY)).unwrap();
    assert_eq!(v["verdict"], "unsupported", "{v}");
    let start = START_ONLY.find("capability work()").unwrap();
    let tail = "result nondet { ok } }";
    let end = START_ONLY.find(tail).unwrap() + tail.len();
    let ds: Vec<&Value> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert_eq!(
        ds,
        vec![&json!({
            "code": "U-SEM-0",
            "severity": "error",
            "message": "unresolved concurrent writes to Desk.busy by Desk.work and Desk.work (G5)",
            "span": {"start": start, "end": end}
        })],
        "{v}"
    );
}

/// A12 + A23: forged capacity 2 on a start-only writer at the cases 2 boundary.
#[test]
fn a12_a23_forged_capacity_two_start_writer_is_refused_by_both_interpreters() {
    let good = model(&change(START_ONLY, "capacity 2", "capacity 1"));
    assert!(good.capabilities[0].writes.is_empty());
    assert_eq!(good.capabilities[0].start_writes.len(), 1);
    assert_eq!(good.bounds.cases, 2);
    let e = engine::explore(&good);
    assert!(
        e.diagnostics.iter().all(|d| d.code != diag::U_SEM_0),
        "{e:?}"
    );
    let mut m = good.clone();
    m.actors[0].capacity = Some(2);
    refused_by_both(&m);
}

/// A10 (+ A19): a conjunction with a per-case atom still reads shared state.
#[test]
fn a10_conjunction_with_a_case_atom_keeps_both_read_write_orders() {
    both_orders_kept(AND_GUARD);
}

/// A11 + A20 (+ A19): an integer comparison of shared state is order-sensitive
/// in the engine and in the reference interpreter alike.
#[test]
fn a11_a20_integer_comparison_keeps_both_orders_in_engine_and_reference() {
    both_orders_kept(INT_GUARD);
}

/// A13: through the public IR entry, a Capability of B (capacity 1) writing
/// A's observed state is refused.
#[test]
fn a13_writer_of_another_actors_observed_state_is_refused_via_public_ir() {
    let src = change(
        &fixture("aobs-owner"),
        "actor B { state x: R = ok }",
        "actor B { capacity 1; state x: R = ok }",
    );
    let good = model(&src);
    assert_eq!(good.actors[1].name, "B");
    assert_eq!(good.actors[1].capacity, Some(1));
    let ir::StateKind::ActorObserved(owner) =
        good.state_variables[good.capabilities[0].writes[0].variable.0].kind
    else {
        panic!("A.x must be Actor observed state")
    };
    assert_eq!(owner, ir::ActorId(0));
    assert!(engine::explore(&good).diagnostics.is_empty());
    let mut m = good.clone();
    m.capabilities[0].actor = ir::ActorId(1);
    refused_by_both(&m);
}

/// A18: the start range preflight reads the shared value of the other case.
#[test]
fn a18_start_overflow_reads_the_shared_value_written_by_the_earlier_case() {
    let m = model(START_OVERFLOW);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for (name, out) in [("engine", &e), ("reference", &r.outcome)] {
        assert_eq!(out.state_range_violations.len(), 1, "{name}: {out:?}");
        let v = &out.state_range_violations[0];
        assert_eq!(
            (
                v.time,
                v.phase.as_str(),
                v.before.as_str(),
                v.attempted.as_str(),
                v.upper.as_str()
            ),
            (2, "start", "1", "2", "1"),
            "{name}"
        );
    }
    assert_eq!(e.state_range_violations, r.outcome.state_range_violations);
}
