//! ADR 0044 new-public-IR obligations, independent entry assertions.
use super::*;

fn base() -> ir::Model {
    crate::lower::lower_configuration(
        &crate::syntax::parse(include_str!("../../../../fixtures/rule-basic/model.relca")).unwrap(),
        "ToBeB",
    )
    .unwrap()
    .model
}

#[test]
fn rule_metadata_and_authority_ir_are_closed_in_both_entries() {
    let m = base();
    assert!(
        !explore(&m)
            .diagnostics
            .iter()
            .any(|d| d.code == crate::diag::U_SEM_0)
    );
    assert_eq!(
        crate::reference::explore(&m).stop,
        crate::reference::StopReason::Complete
    );
    let owner = m
        .actors
        .iter()
        .position(|a| a.metadata.rule.is_some())
        .unwrap();
    let var = m.actors[owner].metadata.rule.as_ref().unwrap().values[0].variable;
    let authority = m.actors[owner]
        .metadata
        .rule
        .as_ref()
        .unwrap()
        .amendment
        .as_ref()
        .unwrap()
        .under
        .0;
    let mut faults: Vec<(&str, ir::Model)> = Vec::new();
    macro_rules! fault {
        ($name:expr, $body:expr) => {{
            let mut n = m.clone();
            ($body)(&mut n);
            faults.push(($name, n));
        }};
    }
    fault!("missing RuleInfo", |n: &mut ir::Model| n.actors[owner]
        .metadata
        .rule = None);
    fault!("owner", |n: &mut ir::Model| n.state_variables[var.0].kind =
        ir::StateKind::Rule(ir::ActorId(usize::MAX)));
    fault!("StateKind", |n: &mut ir::Model| n.state_variables[var.0]
        .kind =
        ir::StateKind::ActorObserved(ir::ActorId(owner)));
    fault!("variable index", |n: &mut ir::Model| n.actors[owner]
        .metadata
        .rule
        .as_mut()
        .unwrap()
        .values[0]
        .variable =
        ir::StateVarId(usize::MAX));
    fault!("duplicate value", |n: &mut ir::Model| {
        let r = n.actors[owner].metadata.rule.as_mut().unwrap();
        r.values.push(r.values[0].clone());
    });
    fault!("type index", |n: &mut ir::Model| n.state_variables[var.0]
        .base =
        ir::TypeId(usize::MAX));
    fault!("optional", |n: &mut ir::Model| n.state_variables[var.0]
        .optional = true);
    fault!("integer", |n: &mut ir::Model| n.state_variables[var.0]
        .integer_max = Some(1));
    fault!("set", |n: &mut ir::Model| n.state_variables[var.0]
        .case_set_size = Some(1));
    fault!("declared type", |n: &mut ir::Model| n.actors[owner]
        .metadata
        .rule
        .as_mut()
        .unwrap()
        .values[0]
        .declared =
        ir::StateValue::Integer(0));
    fault!("effective version", |n: &mut ir::Model| n.actors[owner]
        .metadata
        .rule
        .as_mut()
        .unwrap()
        .effective_version =
        "forged".into());
    fault!("declared version", |n: &mut ir::Model| n.actors[owner]
        .metadata
        .rule
        .as_mut()
        .unwrap()
        .declared_version =
        "forged".into());
    fault!("amender index", |n: &mut ir::Model| n.actors[owner]
        .metadata
        .rule
        .as_mut()
        .unwrap()
        .amendment
        .as_mut()
        .unwrap()
        .by =
        ir::PersonId(usize::MAX));
    fault!("Authority index", |n: &mut ir::Model| n.actors[owner]
        .metadata
        .rule
        .as_mut()
        .unwrap()
        .amendment
        .as_mut()
        .unwrap()
        .under =
        ir::AuthorityId(usize::MAX));
    fault!("grant non-Rule", |n: &mut ir::Model| n.authorities
        [authority]
        .rule_grants[0]
        .rule =
        ir::ActorId(1));
    fault!("grant Actor index", |n: &mut ir::Model| n.authorities
        [authority]
        .rule_grants[0]
        .rule =
        ir::ActorId(usize::MAX));
    fault!("grant person index", |n: &mut ir::Model| n.authorities
        [authority]
        .rule_grants[0]
        .persons[0] =
        ir::PersonId(usize::MAX));
    fault!("under non-Rule", |n: &mut ir::Model| n.authorities
        [authority]
        .under =
        Some(ir::RuleBasis {
            rule: ir::ActorId(1),
            version: "r1".into(),
            span: n.actors[owner].span
        }));
    fault!("under plus Capability", |n: &mut ir::Model| {
        n.authorities[authority].under = Some(ir::RuleBasis {
            rule: ir::ActorId(owner),
            version: "r1".into(),
            span: n.actors[owner].span,
        });
        n.authorities[authority].grants.push(ir::Grant {
            capability: ir::CapabilityId(0),
            persons: vec![ir::PersonId(0)],
        });
    });
    for (label, n) in faults {
        let out = explore(&n);
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.code == crate::diag::U_SEM_0),
            "engine {label}"
        );
        assert!(out.terminals.is_empty(), "engine numeric result {label}");
        assert_eq!(
            crate::reference::explore(&n).stop,
            crate::reference::StopReason::SemanticAbort,
            "reference {label}"
        );
    }
}

#[test]
fn rule_metadata_does_not_add_runtime_state_or_choices() {
    let m = base();
    let out = explore(&m);
    let mut n = m.clone();
    for a in &mut n.actors {
        a.metadata.grain = Some("another-label".into());
    }
    assert_eq!(out, explore(&n));
    let census = |m: &ir::Model| {
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
    assert_eq!(census(&m), census(&n));
    eprintln!("Rule actual engine census: {:?}", census(&m));
}

#[test]
fn rule_qualified_writes_keep_the_static_refusal_even_from_other_actors() {
    for (src, old, new) in [
        (
            include_str!("../../../../fixtures/rule-write-result.broken-s04result/model.relca"),
            "writes bit",
            "writes ApprovalRule.bit",
        ),
        (
            include_str!("../../../../fixtures/rule-write-start.broken-s04start/model.relca"),
            "start { bit",
            "start { ApprovalRule.bit",
        ),
        (
            include_str!("../../../../fixtures/rule-kernel-with.broken-s21w/model.relca"),
            "with approval_required",
            "with ApprovalRule.approval_required",
        ),
    ] {
        let changed = src.replace(old, new);
        assert_ne!(src, changed);
        let ast = crate::syntax::parse(&changed).unwrap();
        let errors = crate::lower::lower(&ast).unwrap_err();
        assert!(
            errors.iter().any(|e| e.code == crate::diag::U_SEM_0
                && e.message == "rule values cannot be written during a run"),
            "{errors:?}"
        );
    }
    let src = r#"model M version "0.2" { type T = enum { pass, fail } actor R { version "r1"; decides bit: T = pass } actor W { capacity 1; capability write() -> T { duration fixed 1min; result nondet { pass }; writes R.bit } } process P { end done } cut C { include [R,W] } bounds { cases 1; live_tasks 1; clock 3min } verify safety no_resource_overcommit }"#;
    let errors = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap_err();
    assert!(
        errors.iter().any(|e| e.code == crate::diag::U_SEM_0
            && e.message == "rule values cannot be written during a run"),
        "{errors:?}"
    );
}
