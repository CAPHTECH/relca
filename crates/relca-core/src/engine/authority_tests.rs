use super::*;
fn model(src: &str) -> ir::Model {
    crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap()
}
const DELEGATION: &str = include_str!("../../../../fixtures/auth-delegation/model.relca");

#[test]
fn malformed_authority_ir_is_rejected_independently_by_both_interpreters() {
    let base = model(DELEGATION);
    let child = base
        .authorities
        .iter()
        .position(|a| a.name == "Child")
        .unwrap();
    let parent = base.authorities[child]
        .restrictions
        .delegated_from
        .unwrap()
        .0;
    let scope = model(include_str!(
        "../../../../fixtures/auth-scope-latent/model.relca"
    ));
    for fault in 0..14 {
        let mut m = if (2..=6).contains(&fault) {
            scope.clone()
        } else {
            base.clone()
        };
        match fault {
            0 => {
                m.authorities[child].restrictions.delegated_from = Some(ir::AuthorityId(usize::MAX))
            }
            1 => m.authorities[parent].restrictions.delegated_from = Some(ir::AuthorityId(child)),
            2 => m.state_variables[0].kind = ir::StateKind::LatentCase,
            3 => {
                m.authorities[0].restrictions.scope = Some(ir::Cond::Eq(ir::ResultMatch {
                    binding: ir::BindingId(0),
                    value: ir::ValueId(0),
                }))
            }
            4 => {
                m.authorities[0].restrictions.scope = Some(ir::Cond::StateEq(ir::StateMatch {
                    variable: ir::StateVarId(0),
                    value: ir::StateValue::Value(ir::ValueId(usize::MAX)),
                }))
            }
            5 => m.authorities[0].restrictions.scope = Some(ir::Cond::And(vec![])),
            6 => m.authorities[0].restrictions.scope_span = None,
            7 => {
                m.authorities[child].restrictions.revocable_by =
                    Some(ir::AuthorityRevoker::Person(ir::PersonId(usize::MAX)));
                m.authorities[child].restrictions.revocable_span = Some(m.authorities[child].span);
            }
            8 => {
                m.authorities[child].restrictions.revocable_by =
                    Some(ir::AuthorityRevoker::Actor(ir::ActorId(usize::MAX)));
                m.authorities[child].restrictions.revocable_span = Some(m.authorities[child].span);
            }
            9 => m.authorities[child].restrictions.effective_expires = None,
            10 => m.authorities[child].not_by.clear(),
            11 => m.authorities[child].issued_by = None,
            12 => m.authorities[child].grants[0].persons = vec![ir::PersonId(0)],
            13 => {
                m.authorities[child].restrictions.expires = Some(11);
                m.authorities[child].restrictions.effective_expires = Some(11);
            }
            _ => unreachable!(),
        }
        let engine = explore(&m);
        assert!(
            engine.diagnostics.iter().any(|d| d.code == diag::U_SEM_0),
            "engine fault {fault}"
        );
        assert!(engine.terminals.is_empty());
        let reference = crate::reference::explore(&m);
        assert_eq!(
            reference.verdict(),
            Verdict::Unsupported,
            "reference fault {fault}"
        );
        assert!(reference.outcome.terminals.is_empty());
    }
}

#[test]
fn direct_ir_restricted_waive_and_contest_cannot_publish_static_success() {
    for source in [
        include_str!("../../../../fixtures/auth-waive/model.relca"),
        include_str!("../../../../fixtures/auth-contest/model.relca"),
    ] {
        let mut m = model(source);
        m.authorities[0].restrictions.expires = Some(100);
        m.authorities[0].restrictions.effective_expires = Some(100);
        m.authorities[0].restrictions.expires_span = Some(m.authorities[0].span);
        assert!(
            explore(&m)
                .diagnostics
                .iter()
                .any(|d| d.code == diag::U_SEM_0)
        );
        assert_eq!(
            crate::reference::explore(&m).verdict(),
            Verdict::Unsupported
        );
        // Neither interpreter may return an independently successful contest row
        // merely because a direct builder retained lower's old static tables.
        for o in [explore(&m), crate::reference::explore(&m).outcome] {
            assert!(
                o.properties
                    .iter()
                    .filter(|p| matches!(p.property, ir::Property::Contest(_)))
                    .all(|p| p.verdict == Verdict::Unsupported)
            );
        }
    }
}

#[test]
fn same_tick_scope_reads_each_interleaving_without_pruning_tasks() {
    let src = r#"model M version "0.2" {
      type T = enum { ok }
      observed case { enabled: bool = false }
      people [p,r]
      actor A { executors [p]; capability run() -> T { duration fixed 1min; result nondet { ok } } }
      actor W { capacity 1; capability write() -> T { duration fixed 1min; result nondet { ok }; writes enabled; finish { enabled = true } } }
      authority G { basis "record"; grants p to A.run; issued_by r; scope enabled == true }
      process P { fork f { a = A.run(); w = W.write() }; join all f; end done }
      bounds { cases 1; live_tasks 2; clock 3min }
      contract C { authority_basis A.run G }
    }"#;
    let m = model(src);
    for o in [explore(&m), crate::reference::explore(&m).outcome] {
        assert_eq!(o.properties[0].verdict, Verdict::Violated);
        assert_eq!(o.terminals.len(), 1);
        assert_eq!(
            (o.terminals[0].elapsed.min, o.terminals[0].work.min),
            (1, 2)
        );
    }
}

#[test]
fn authority_metadata_does_not_enter_state_keys_room_or_generation() {
    use std::hash::{Hash, Hasher};
    let restricted = model(include_str!("../../../../fixtures/auth-expiry/model.relca"));
    let mut plain = restricted.clone();
    for a in &mut plain.authorities {
        a.restrictions = ir::AuthorityRestrictions::default();
    }
    let s = State::initial(&restricted, 0);
    let other = State::initial(&plain, 0);
    assert_eq!(s, other);
    let hash = |s: &State| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(&s), hash(&other));
    let counts = |m: &ir::Model| {
        let mut e = Exploration::default();
        explore_into(
            m,
            true,
            DEFAULT_STATE_BUDGET,
            std::convert::identity,
            &mut e,
        )
        .unwrap();
        e.census
    };
    assert_eq!(counts(&restricted), counts(&plain));
    for budget in 0..=counts(&plain).2 + 1 {
        assert_eq!(
            explore_with_budget(&restricted, budget),
            explore_with_budget(&plain, budget)
        );
    }
}

#[test]
fn revocation_record_survives_budget_zero_and_one() {
    let src = include_str!("../../../../fixtures/auth-revocation/model.relca");
    for budget in [0, 1] {
        let report = crate::report::check_using(src, |m| explore_with_budget(m, budget));
        assert_eq!(
            report.unchecked.last().map(String::as_str),
            Some("dynamic_authority_revocation")
        );
        assert!(
            report
                .assumptions
                .iter()
                .any(|a| a.field == "authority.G.revocable_by")
        );
    }
}

#[test]
fn sampled_authority_history_uses_completion_snapshot_in_both_drivers() {
    let base = include_str!("../../../../fixtures/auth-scope-post/model.relca");
    for (src, broken) in [
        (base.to_owned(), false),
        (
            base.replace("finish { enabled = true }", "finish { enabled = false }"),
            true,
        ),
    ] {
        let src = src.replace(
            "  bounds",
            "  simulation replications 2 seed 7 confidence 0.95\n  bounds",
        );
        let m = model(&src);
        for sampled in [simulate(&m), crate::reference::simulate(&m).outcome] {
            assert_eq!(
                sampled.completed_replications, 2,
                "{:?}",
                sampled.diagnostics
            );
            for replication in sampled.replications {
                assert_eq!(replication.cases[0].violations, vec![broken]);
            }
        }
    }
}

#[test]
fn revocation_assumption_precedes_kernel_and_simulation_tail() {
    let src = include_str!("../../../../fixtures/auth-revocation/model.relca")
        .replace("result nondet { pass }", "outcomes kernel K")
        .replace(
            "process Flow",
            "kernel K { * -> { 1: pass } }\nprocess Flow",
        )
        .replace(
            "  bounds",
            "  simulation replications 2 seed 7 confidence 0.95\n  bounds",
        );
    let report = crate::report::simulate(&src);
    let index = |field: &str| {
        report
            .assumptions
            .iter()
            .position(|a| a.field == field)
            .unwrap()
    };
    assert!(index("authority.G.revocable_by") < index("kernel.K"));
    assert!(index("authority.G.revocable_by") < index("simulation.random"));
}

#[test]
fn authority_nonrequires_population_complete29_static9_integration_pending2() {
    // Additional source-owned control, NOT a replacement for the external
    // Complete31 acceptance. A19 must still run after ADR 0026 integration.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let (mut complete, mut rejected, mut pending) = (0, 0, 0);
    for name in authority_population::NAMES {
        if name.starts_with("auth-requires-integration") {
            pending += 1;
            continue;
        }
        let source = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join(name).join("expected.json")).unwrap())
                .unwrap();
        let lowered = crate::lower::lower(&crate::syntax::parse(&source).unwrap());
        if expected["verdict"] == "invalid" || expected["verdict"] == "unsupported" {
            assert!(lowered.is_err(), "static rejection {name}");
            rejected += 1;
            continue;
        }
        let m = lowered.unwrap();
        let reference = crate::reference::explore(&m);
        assert_eq!(
            reference.stop,
            crate::reference::StopReason::Complete,
            "{name}"
        );
        assert_eq!(
            serde_json::json!(reference.verdict()),
            expected["verdict"],
            "hand root {name}"
        );
        let engine = explore(&m);
        for outcome in [&engine, &reference.outcome] {
            for (actual, want) in outcome
                .properties
                .iter()
                .zip(expected["properties"].as_array().unwrap())
            {
                assert_eq!(
                    serde_json::json!(actual.verdict),
                    want["verdict"],
                    "hand property {name}"
                );
            }
        }
        let mut reference = reference.outcome;
        assert!(reference.diagnostics.starts_with(&m.diagnostics));
        reference.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(engine, reference, "all fields {name}");
        complete += 1;
    }
    assert_eq!((complete, rejected, pending), (29, 9, 2));
}
