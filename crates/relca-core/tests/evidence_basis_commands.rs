use relca_core::{Verdict, diff};

#[test]
fn diff_refusal_keeps_violated_side_facts_without_claiming_a_supported_comparison() {
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
        assert_eq!(result.verdict, Verdict::Unsupported, "{target}");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == "U-SEM-0" && d.message == "evidence_basis_unsupported_command")
        );
        assert!(result.diagnostics.iter().all(|d| d.code != "U-LEDGER-0"));
        assert!(result.checked.is_empty());
    }
}
