//! ADR 0076 F9. Register as a child of production mutation.rs; never copy the
//! classifier into this oracle. Registration is a separate implementation step.
use crate::{Verdict, agency, report};

const ID: &str = "contract V #1 same_version";

fn reports() -> (report::Report, report::Report) {
    // Only the DTO's public contract-row shape is needed for the isolated
    // classification test. The ordinary CLI separately tests real new syntax.
    let mut before = report::check(include_str!(
        "../../../../fixtures/mut2-summary/model.relca"
    ));
    assert_eq!(before.verdict, Verdict::Satisfied);
    let mut property = before.properties[0].clone();
    property.id = ID.into();
    property.verdict = Verdict::Satisfied;
    property.cases = None;
    before.properties.push(property);
    let mut after = before.clone();
    after.properties.last_mut().unwrap().verdict = Verdict::Violated;
    after.verdict = Verdict::Violated;
    (before, after)
}

fn lineage(report: &mut report::Report) -> &mut agency::Lineage {
    report
        .agency
        .as_mut()
        .unwrap()
        .as_derived_mut()
        .unwrap()
        .lineage
        .iter_mut()
        .find(|row| row.call == "a")
        .unwrap()
}

#[test]
fn same_version_f9_rule_failure_dominates_contract_worsening() {
    let (mut before, mut after) = reports();
    lineage(&mut before).independent_sources = vec!["OriginalCheck".into()];
    lineage(&mut after).independent_sources = vec!["InventedIndependentCheck".into()];
    let got = super::compare_summary(
        &before,
        &after,
        &std::collections::BTreeSet::from(["Invoice".into()]),
        "a",
        "Approver.approve",
    );
    assert_eq!(got.result, "undetected");
    assert_eq!(got.reason, Some("independence_credited"));
    assert!(
        got.assigned
            .iter()
            .any(|change| { change.key == ID && change.direction == "worsened" })
    );
    assert!(!got.other.iter().any(|change| change.key == ID));
}

#[test]
fn same_version_worsening_is_assigned_even_without_a_lineage_row() {
    let (mut before, mut after) = reports();
    for report in [&mut before, &mut after] {
        report
            .agency
            .as_mut()
            .unwrap()
            .as_derived_mut()
            .unwrap()
            .lineage
            .retain(|row| row.call != "a");
    }
    let got = super::compare_summary(
        &before,
        &after,
        &std::collections::BTreeSet::from(["Invoice".into()]),
        "a",
        "Approver.approve",
    );
    assert_eq!(got.result, "detected");
    assert_eq!(got.reason, None);
    assert!(
        got.assigned
            .iter()
            .any(|change| { change.key == ID && change.direction == "worsened" })
    );
}
