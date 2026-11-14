use relca_core::{ir, lower, syntax};

fn source(body: &str, clause: &str) -> String {
    let i = body.rfind('}').unwrap();
    format!("{} contract Basis {{ {clause} }} }}", &body[..i])
}

fn model(src: &str) -> ir::Model {
    lower::lower(&syntax::parse(src).unwrap()).unwrap()
}

#[test]
fn must_snapshot_and_argument_tables_do_not_use_control_or_latent() {
    let m0 = include_str!("evidence_basis/m0.relca");
    for (target, direct, expected) in [
        ("Clerk.match_po", "", vec!["Invoice", "PurchaseOrder"]),
        ("Clerk.match_po", "direct ", vec!["PurchaseOrder"]),
        ("Approver.approve", "", vec!["Invoice"]),
    ] {
        let m = model(&source(
            m0,
            &format!("evidence_basis {direct}{target} [Invoice]"),
        ));
        let ir::ContractKind::EvidenceBasis { calls, .. } = &m.contracts[0].kind else {
            panic!()
        };
        let binding = if target == "Approver.approve" {
            "a"
        } else {
            "p"
        };
        let call = calls
            .iter()
            .find(|c| m.process.bindings[c.binding.0].name == binding)
            .unwrap();
        let mut names: Vec<_> = call
            .sources
            .iter()
            .map(|s| m.actors[s.0].name.as_str())
            .collect();
        names.sort();
        assert_eq!(names, expected);
    }
}

#[test]
fn conditional_snapshot_refuses_but_dominating_snapshot_is_must() {
    let m5 = include_str!("evidence_basis/m5.relca");
    let src = source(m5, "evidence_basis Approver.approve [PurchaseOrder]");
    let out = relca_core::engine::explore(&model(&src));
    let ds = out.diagnostics;
    assert!(
        ds.iter()
            .any(|d| d.code == "U-SEM-0"
                && d.message.contains("evidence_basis_conditional_snapshot"))
    );
    let src = src.replace(
        "if e == pass { p = Clerk.match_po(e) } else {}",
        "p = Clerk.match_po(e)",
    );
    let m = model(&src);
    let ir::ContractKind::EvidenceBasis { calls, .. } = &m.contracts[0].kind else {
        panic!()
    };
    assert!(
        calls
            .iter()
            .find(|c| m.process.bindings[c.binding.0].name == "a")
            .unwrap()
            .sources
            .iter()
            .any(|s| m.actors[s.0].name == "PurchaseOrder")
    );
}

#[test]
fn engine_refuses_forged_evidence_table_and_target() {
    let src = source(
        include_str!("evidence_basis/m0.relca"),
        "evidence_basis Clerk.match_po [Invoice]",
    );
    for target in [false, true] {
        let mut m = model(&src);
        let ir::ContractKind::EvidenceBasis {
            capability, calls, ..
        } = &mut m.contracts[0].kind
        else {
            panic!()
        };
        if target {
            *capability = ir::CapabilityId(usize::MAX);
        } else {
            calls[0].sources.clear();
        }
        let out = relca_core::engine::explore(&m);
        assert!(out.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
        assert!(
            out.properties
                .iter()
                .all(|p| p.verdict != relca_core::Verdict::Satisfied)
        );
    }
}
