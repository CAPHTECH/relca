use super::*;
#[test]
fn command_fault_edits_match_public_dsl_and_enumerate_only_declared_sites() {
    let source = include_str!("../../../../fixtures/k8-cut-g/model.relca").replace(
        "p = Ledger.post(case.id)",
        "p = Ledger.post(case.id) response_loss none",
    );
    let ast = syntax::parse(&source).unwrap();
    let m = lower::lower(&ast).unwrap();
    let (sites, skipped) = enumerate(&source, &ast, &m);
    assert_eq!(sites.len(), 1);
    assert!(skipped.is_empty());
    let candidate = &sites[0];
    for resend in [true, false] {
        let edited = candidate.apply(&ast, resend);
        let suffix = if resend {
            format!(
                "response_loss sequence [unknown,delivered] retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end {}",
                candidate.exhausted
            )
        } else {
            "response_loss sequence [unknown]".into()
        };
        let hand = source.replace("response_loss none", &suffix);
        let generated =
            report::check_ast_with_scenario_using(&source, &edited, None, engine::explore);
        let expected = report::check(&hand);
        assert_eq!(generated.verdict, expected.verdict);
        assert_eq!(generated.terminals, expected.terminals);
        assert_eq!(generated.case_outcomes, expected.case_outcomes);
        assert_eq!(generated.response_receipts, expected.response_receipts);
    }
    let undeclared = source.replace("response_loss none", "");
    let a = syntax::parse(&undeclared).unwrap();
    let m = lower::lower(&a).unwrap();
    let (sites, skipped) = enumerate(&undeclared, &a, &m);
    assert!(sites.is_empty());
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].reason, "undeclared_site");
}

#[test]
fn receipt_held_refuses_computational_cut_abort_and_missing_wire() {
    let source = include_str!("../../../../fixtures/k8-cut-g/model.relca").replace(
        "p = Ledger.post(case.id)",
        "p = Ledger.post(case.id) response_loss none",
    );
    let ast = syntax::parse(&source).unwrap();
    let context = lower::lower(&ast).unwrap();
    let before = report::check(&source);
    let (sites, _) = enumerate(&source, &ast, &context);
    let candidate = &sites[0];
    let edited = candidate.apply(&ast, false);
    let after = report::check_ast_with_scenario_using(&source, &edited, None, engine::explore);
    assert_eq!(
        compare_fault(6, &candidate.site, &context, &before, &after, &edited).result,
        "detected"
    );
    for fault in 0..7 {
        let mut bad = after.clone();
        match fault {
            0 => {
                bad.bounds.truncated = Some(true);
                bad.verdict = Verdict::Violated;
            }
            1 => {
                bad.bounds.truncated = None;
                bad.verdict = Verdict::Violated;
            }
            2 => bad.response_receipts = None,
            3 => bad.case_outcomes = None,
            4 => bad.verdict = Verdict::Unsupported,
            5 => {
                bad.case_outcomes.as_mut().unwrap()[0].terminals[0].kind =
                    report::CaseTerminalKind::CutOff;
                bad.verdict = Verdict::Violated;
            }
            _ => bad.response_analysis = None,
        }
        let c = compare_fault(6, &candidate.site, &context, &before, &bad, &edited);
        assert!(matches!(c.result, "inconclusive" | "invalid_mutant"));
        assert!(c.assigned.iter().all(|c| c.direction != "held"));
    }
}
