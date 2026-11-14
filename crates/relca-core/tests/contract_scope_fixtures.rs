//! Separate twenty-input scope cohort; never counted as legacy 124/148 inputs.
use relca_core::{Verdict, engine, ir, lower, outcome, reference, syntax};
use relca_test_support::fixture_match::{CONTRACT_SCOPE_FIXTURES, contract_scope_fixture};
use std::path::Path;

#[test]
fn scope_population_is_closed_and_unknown_names_cannot_be_excluded() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut actual: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("scope-"))
        .collect();
    actual.sort();
    let mut expected = CONTRACT_SCOPE_FIXTURES.map(str::to_owned).to_vec();
    expected.sort();
    assert_eq!(actual, expected);
    assert!(std::panic::catch_unwind(|| contract_scope_fixture("scope-unknown")).is_err());
}

fn hand(name: &str, plan: &str) -> (u64, Option<Verdict>) {
    let active = if name == "scope-a4.broken-global" {
        true
    } else if name == "scope-a3.broken-outside" {
        plan == "Old"
    } else {
        plan == "New"
    };
    let time =
        if name == "scope-a1.broken-called" || (plan == "New" && !name.starts_with("scope-a3")) {
            2
        } else {
            1
        };
    let contract = active.then_some(
        if (name.starts_with("scope-a3") && plan == "New") || plan == "Old" {
            Verdict::Vacuous
        } else if name == "scope-a2.broken-grant" {
            Verdict::Violated
        } else {
            Verdict::Satisfied
        },
    );
    (time, contract)
}

fn verify(out: &outcome::Outcome, minutes: u64, contract: Option<Verdict>, context: &str) {
    assert!(!out.truncated, "{context}: no computational cut");
    assert!(
        !out.diagnostics
            .iter()
            .any(|d| d.severity == relca_core::diag::Severity::Error),
        "{context}: no hidden abort"
    );
    let rows: Vec<_> = out
        .properties
        .iter()
        .filter(|p| matches!(p.property, ir::Property::Contract(_)))
        .collect();
    assert_eq!(
        rows.len(),
        usize::from(contract.is_some()),
        "{context}: hand applicable inventory"
    );
    if let Some(want) = contract {
        assert_eq!(rows[0].verdict, want, "{context}: hand contract verdict");
    }
    assert!(
        out.properties
            .iter()
            .any(|p| matches!(p.property, ir::Property::ReachableEnd(_))
                && p.verdict == Verdict::Satisfied),
        "{context}: reachable done"
    );
    let range = outcome::Range {
        min: minutes,
        max: minutes,
    };
    if !out.terminals.is_empty() {
        assert_eq!(out.terminals.len(), 1);
        assert_eq!(
            (out.terminals[0].elapsed, out.terminals[0].work),
            (range, range),
            "{context}: hand time/work"
        );
    } else {
        let cases = out.case_outcomes.as_ref().unwrap();
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].terminals.len(), 1);
        assert_eq!(
            (cases[0].terminals[0].elapsed, cases[0].terminals[0].work),
            (Some(range), Some(range)),
            "{context}: hand case time/work"
        );
    }
}

#[test]
fn all_twenty_scope_fixture_inputs_meet_hand_values_in_both_interpreters() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut count = 0;
    for name in CONTRACT_SCOPE_FIXTURES {
        let source = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
        let ast = syntax::parse(&source).unwrap();
        for plan in ["Old", "New"] {
            let model = lower::lower_with_selection(&ast, Some(plan), None).unwrap();
            let (time, contract) = hand(name, plan);
            let e = engine::explore(&model);
            let r = reference::explore(&model);
            assert_eq!(r.stop, reference::StopReason::Complete, "{name}/{plan}");
            verify(&e, time, contract, &format!("engine {name}/{plan}"));
            verify(
                &r.outcome,
                time,
                contract,
                &format!("reference {name}/{plan}"),
            );
            assert_eq!(e.properties, r.outcome.properties);
            count += 1;
        }
    }
    assert_eq!(count, 20);
}
