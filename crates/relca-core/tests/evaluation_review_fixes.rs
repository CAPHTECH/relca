#[path = "support/evaluation_review_acceptance.rs"]
mod h;

#[test]
fn pareto_no_shared_observations_has_an_explicit_nonempty_statement() {
    let source =
        include_str!("../../../fixtures/eval-shared-source.broken-one-objective/model.relca");
    let out =
        relca_core::compare::compare(source, h::PLANS, relca_core::compare::Analysis::Exhaustive);
    h::pareto_hand(&serde_json::to_value(out).unwrap());
}
