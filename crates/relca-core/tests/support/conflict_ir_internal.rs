//! Internal IR tests linked from src/lib.rs:
//! #[cfg(test)] #[path = "../tests/support/conflict_ir_internal.rs"] mod conflict_ir_internal;
//! These tests do not share lower's validator with either interpreter.
use crate::{engine, ir, lower, reference, syntax};
fn model(name: &str) -> ir::Model {
    let s = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap();
    syntax::parse(&s).and_then(|a| lower::lower(&a)).unwrap()
}
fn refused(m: &ir::Model) {
    let e = engine::explore(m);
    let r = reference::explore(m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    for o in [&e, &r.outcome] {
        assert!(o.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
        assert!(o.terminals.is_empty());
    }
}
#[test]
fn cf_public_ir_checks_version_domain_ids_receipts_and_resolution_evidence() {
    let good = model("cf-version");
    for fault in 0..7 {
        let mut m = good.clone();
        let checked = m.capabilities[0].version_check.as_ref().unwrap().variables[0];
        match fault {
            0 => m.capabilities[0]
                .version_check
                .as_mut()
                .unwrap()
                .variables
                .clear(),
            1 => m.capabilities[0]
                .version_check
                .as_mut()
                .unwrap()
                .variables
                .push(checked),
            2 => {
                m.capabilities[0].version_check.as_mut().unwrap().variables[0] =
                    ir::StateVarId(usize::MAX)
            }
            3 => m.capabilities[0].version_check.as_mut().unwrap().reject = ir::ValueId(usize::MAX),
            4 => {
                m.capabilities[0].version_check.as_mut().unwrap().reject =
                    m.capabilities[0].results[0]
            }
            5 => {
                let write = m.capabilities[0].writes[0];
                m.capabilities[0].start_writes.push(write);
            }
            6 => m.concurrency_resolutions.clear(),
            _ => unreachable!(),
        }
        refused(&m);
    }
    let mut m = model("cf-agency");
    m.actors[0].capacity = Some(2);
    m.capabilities[0].version_check = None;
    // Serialized evidence was computed for capacity1 and is now stale.
    refused(&m);
}
#[test]
fn cf_public_ir_requires_exact_implicit_loss_properties_and_observed_shared_placement() {
    let good = model("cf-lww");
    let holder = ir::StateVarId(
        good.state_variables
            .iter()
            .position(|v| v.name == "holder")
            .unwrap(),
    );
    for fault in 0..5 {
        let mut m = good.clone();
        match fault {
            0 => m
                .verify
                .retain(|v| !matches!(v.property, ir::Property::NoLostUpdate(_))),
            1 => {
                let verify = m.verify.last().unwrap().clone();
                m.verify.push(verify);
            }
            2 => m.state_variables[holder.0].last_write_wins = false,
            3 => m.state_variables[holder.0].kind = ir::StateKind::LatentCase,
            4 => m.state_variables[holder.0].kind = ir::StateKind::ObservedCase,
            _ => unreachable!(),
        }
        refused(&m);
    }
}

#[test]
fn cf_commutative_fork_pair_remains_structurally_refused_at_capacity_two() {
    // S20's fourth control: the L/V/W controls are cf fixtures; the unchanged
    // aset-static-fork-target fixture additionally preserves InsertCase at1.
    // Here two Add writers are shared Actor observations at capacity2, cases1.
    let source = "model CommutativeFork version \"0.2\" {type T=enum{ok};actor Ledger{capacity 2;state n:0..2=0;capability add()->T{writes n;finish{n+=1};duration fixed 1min;result nondet{ok}}};process P{a=Ledger.add();b=Ledger.add();end done};cut C{include[Ledger]};bounds{cases 1;live_tasks 2;clock 5min};verify reachable end.done}";
    let good = crate::report::check(source);
    assert_eq!(good.verdict, crate::Verdict::Satisfied);
    let v = serde_json::to_value(good).unwrap();
    assert_eq!(
        v["terminals"][0]["elapsed"],
        serde_json::json!({"min":2,"max":2})
    );
    assert_eq!(
        v["terminals"][0]["work"],
        serde_json::json!({"min":2,"max":2})
    );
    let bad = source.replace(
        "a=Ledger.add();b=Ledger.add();",
        "fork f{a=Ledger.add();b=Ledger.add()};join all f;",
    );
    assert_ne!(source, bad);
    let checked = crate::report::check(&bad);
    assert_eq!(checked.verdict, crate::Verdict::Unsupported);
    assert!(
        checked.diagnostics.iter().any(|d| d.code == "U-SEM-0"
            && d.message == "fork siblings write the same observed variable")
    );
}
