use relca_core::{Verdict, engine, ir, lower, reference, syntax};

fn model(src: &str) -> ir::Model {
    lower::lower(&syntax::parse(src).unwrap()).unwrap()
}

fn basis(src: &str, clause: &str) -> String {
    let end = src.rfind('}').unwrap();
    format!("{} contract Basis {{ {clause} }} }}", &src[..end])
}

#[test]
fn reference_reconstructs_without_the_engine_table() {
    let src = basis(
        include_str!("evidence_basis/m0.relca"),
        "evidence_basis Clerk.match_po [Invoice]",
    );
    let mut m = model(&src);
    let ir::ContractKind::EvidenceBasis { calls, .. } = &mut m.contracts[0].kind else {
        panic!()
    };
    calls.clear();
    let independent = reference::explore(&m);
    assert_eq!(independent.stop, reference::StopReason::Complete);
    assert!(
        independent
            .outcome
            .properties
            .iter()
            .all(|p| p.verdict == Verdict::Satisfied)
    );
    assert!(
        engine::explore(&m)
            .diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0")
    );
}

#[test]
fn receipt_attempts_and_delivered_aliases_reconstruct_their_inputs() {
    let m0 = include_str!("evidence_basis/m0.relca");
    for suffix in [
        "response_loss none",
        "response_loss sequence [unknown]",
        "response_loss sequence [unknown,delivered] retry max_attempts 2 when receipt unknown backoff fixed 1min exhausted end returned",
        "response_loss none retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end returned",
    ] {
        let src = m0.replace(
            "p = Clerk.match_po(e)",
            &format!("p = Clerk.match_po(e) {suffix}"),
        );
        for clause in [
            "evidence_basis Clerk.match_po [Invoice,PurchaseOrder]",
            "evidence_basis Ledger.post [Invoice,PurchaseOrder]",
        ] {
            let m = model(&basis(&src, clause));
            let actual = engine::explore(&m);
            let independent = reference::explore(&m);
            assert_eq!(independent.stop, reference::StopReason::Complete);
            assert_eq!(actual.properties, independent.outcome.properties);
        }
    }
}

#[test]
fn sampled_contract_witnesses_match_the_independent_interpreter() {
    let m0 = include_str!("evidence_basis/m0.relca")
        .replace("result nondet { pass, fail }", "result nondet { pass }");
    for (src, broken) in [
        (m0.clone(), false),
        (m0.replace("reads [Invoice]; ", ""), true),
    ] {
        let src = basis(&src, "evidence_basis Clerk.match_po [Invoice]");
        let end = src.rfind('}').unwrap();
        let src = format!(
            "{} simulation replications 4 seed 7 confidence 0.5 }}",
            &src[..end]
        );
        let m = model(&src);
        let actual = engine::simulate(&m);
        let independent = reference::simulate(&m);
        assert_eq!(independent.stop, reference::StopReason::Complete);
        assert_eq!(actual.replications, independent.outcome.replications);
        assert_eq!(actual.replications.len(), 4);
        assert!(actual.replications.iter().all(|r| {
            r.cases
                .iter()
                .all(|c| c.violations.contains(&true) == broken)
        }));
    }
}
