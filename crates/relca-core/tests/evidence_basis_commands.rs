use relca_core::{Verdict, diff};

#[test]
fn diff_transports_side_verdicts_without_comparing_witnesses() {
    let m0 = include_str!("evidence_basis/m0.relca");
    let start = m0.find("  process Main {").unwrap();
    let end = m0.find("  bounds {").unwrap();
    let alternative = m0[start..end].replacen("process Main", "process Alternative", 1);
    let close = m0.rfind('}').unwrap();
    for (target, source, side) in [
        ("Clerk.match_po", "Invoice", Verdict::Satisfied),
        ("Approver.approve", "PurchaseOrder", Verdict::Violated),
    ] {
        let src = format!(
            "{}\n{alternative}\nconfiguration A {{ cut S; process Main }}\nconfiguration B {{ cut S; process Alternative }}\ncontract Basis {{ evidence_basis {target} [{source}] }}\n}}",
            &m0[..close]
        );
        let result = diff::diff(&src, "A", "B");
        assert_eq!(result.from.report.verdict, side);
        assert_eq!(result.to.report.verdict, side);
        assert_eq!(result.verdict, side, "{target}");
        assert!(
            result
                .unchecked
                .iter()
                .any(|item| item == "contract_witnesses")
        );
        assert!(result.diagnostics.iter().all(|d| d.code != "U-LEDGER-0"));
        assert!(result.checked.iter().any(|item| item == "relation"));
        let row = result
            .properties
            .iter()
            .find(|p| p.id == "contract Basis #1 evidence_basis")
            .unwrap();
        assert_eq!((row.from, row.to), (Some(side), Some(side)));
    }
}

#[test]
fn lower_failure_seals_after_scope_and_rule_withdrawals() {
    let src = include_str!("bounded_repetition/models/K1.relca")
        .replace(
            "  cut S {",
            "  actor Rule { version \"r1\"; decides active: bool = true }\n  cut S {",
        )
        .replace("Clerk,Approver,Ledger]", "Clerk,Approver,Ledger,Rule]");
    let result = diff::diff(&src, "A", "Missing");
    assert_eq!(result.verdict, Verdict::Invalid);
    assert_eq!(result.from.report.verdict, Verdict::Violated);
    assert_eq!(result.to.report.verdict, Verdict::Invalid);
    for w in ["contract_witnesses", "rule"] {
        assert_eq!(result.unchecked.iter().filter(|x| *x == w).count(), 1);
    }
    result
        .ledger
        .validate(&serde_json::to_value(&result).unwrap())
        .unwrap();
}
