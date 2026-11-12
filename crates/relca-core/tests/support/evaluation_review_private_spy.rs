//! Link under lib.rs with cfg(test), path ../tests/support/evaluation_review_private_spy.rs.
//! Slot contract: lower=0, engine explore=1, reference explore=2, compare=3,
//! engine simulate=4, reference simulate=5, explain trace explore=6.
use crate::{compare, engine, evaluation_spy, explain, lower, reference, syntax};

const SOURCE: &str = include_str!("../../../../fixtures/eval-mut7/model.relca");

fn model() -> crate::ir::Model {
    lower::lower(&syntax::parse(SOURCE).unwrap()).unwrap()
}

fn simulation_model() -> crate::ir::Model {
    let source = include_str!("../../../../fixtures/met-sample/model.relca");
    lower::lower(&syntax::parse(source).unwrap()).unwrap()
}

fn one(slot: usize, f: impl FnOnce()) {
    let (_, counts) = evaluation_spy::observe(f);
    let mut expected = vec![0; 7];
    expected[slot] = 1;
    assert_eq!(counts.as_slice(), expected, "distinct entry slot {slot}");
}

#[test]
fn lower_has_its_own_spy_slot() {
    let ast = syntax::parse(SOURCE).unwrap();
    one(0, || {
        lower::lower(&ast).unwrap();
    });
}
#[test]
fn engine_explore_has_its_own_spy_slot() {
    let m = model();
    one(1, || {
        engine::explore(&m);
    });
}
#[test]
fn reference_explore_has_its_own_spy_slot() {
    let m = model();
    one(2, || {
        reference::explore(&m);
    });
}
#[test]
fn compare_has_its_own_spy_slot() {
    one(3, || {
        compare::compare(SOURCE, &[], compare::Analysis::Exhaustive);
    });
}
#[test]
fn engine_simulate_has_its_own_spy_slot() {
    let m = simulation_model();
    one(4, || {
        assert_eq!(engine::simulate(&m).completed_replications, 4);
    });
}
#[test]
fn reference_simulate_has_its_own_spy_slot() {
    let m = simulation_model();
    one(5, || {
        assert_eq!(reference::simulate(&m).outcome.completed_replications, 4);
    });
}
#[test]
fn explain_counts_the_trace_explore_entry_not_an_ordinary_explore() {
    let source = include_str!("../../../../fixtures/explain-invariant/model.relca");
    let (out, counts) = evaluation_spy::observe(|| explain::explain(source));
    assert_ne!(
        out.report.verdict,
        crate::Verdict::Invalid,
        "must reach the trace explorer"
    );
    let counts = counts.as_slice();
    assert_eq!(counts.len(), 7, "all execution entries must be represented");
    assert!(counts[0] > 0, "explain lowers its model");
    assert_eq!(
        &counts[1..],
        &[0, 0, 0, 0, 0, 1],
        "trace is counted separately, exactly once"
    );
}
