//! Link under mutation.rs with cfg(test), path ../tests/support/evaluation_review_private_mutation.rs.
use super::*;

#[test]
fn kind7_forbids_all_seven_separately_calibrated_entry_points() {
    let source = include_str!("../../../../fixtures/eval-mut7/model.relca");
    let ast = syntax::parse(source).unwrap();
    let original = report::check(source);
    assert_eq!(original.verdict, Verdict::Satisfied);
    let mut out = uncomputed("original_not_decidable", 128);
    out.original = Some(original);
    let (_, counts) = crate::evaluation_spy::observe(|| {
        kind7(source, &ast, &mut out, &mut 0, &Options::default(), None)
    });
    assert_eq!(out.mutations[7].mutants.len(), 4);
    assert!(
        out.mutations[7]
            .mutants
            .iter()
            .all(|m| m.result == "detected" && m.report.is_none() && m.other_changes.is_empty())
    );
    assert_eq!(
        counts.as_slice(),
        &[0; 7],
        "no lower, explore, simulate, compare or explain trace for structural mutants"
    );
}
