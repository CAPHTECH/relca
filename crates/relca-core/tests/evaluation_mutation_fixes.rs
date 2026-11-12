#[path = "support/evaluation_mutation_acceptance.rs"]
mod h;

fn run(case: &str) {
    let out = relca_core::compare::compare_with_scenario(
        &h::source(case),
        h::plans(case),
        h::analysis(case),
        h::scenario(case),
    );
    h::hand(case, &serde_json::to_value(out).unwrap());
}
h::mutation_acceptance_tests!();
