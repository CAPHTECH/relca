#[path = "../../tests/support/q9_review_oracle.rs"]
mod hand;
#[path = "../../tests/support/q9_review_inputs.rs"]
mod input;
#[path = "../../tests/support/q9_review_reference_aggregate.rs"]
mod reference_aggregate;

#[test]
fn q9_review_reference_aggregate_uses_production_pipeline() {
    reference_aggregate::assert_reference_aggregate(|src| {
        serde_json::to_value(super::simulate_using(src, None, |m| {
            crate::reference::simulate(m).outcome
        }))
        .unwrap()
    });
}
