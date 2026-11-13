use super::*;

#[test]
fn observations_resolve_named_variables_after_declaration_reordering() {
    let source = include_str!("../../tests/bounded_repetition/models/L1-FFF.relca");
    let mut m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
    // Only query construction is under test: do not execute the original
    // process with its now deliberately stale state-variable IDs.
    m.state_variables.reverse();
    let (q, loops) = queries(&m);
    assert!(!loops.is_empty());
    for l in loops {
        for query in &q.probability_queries[l.first..l.limit] {
            let ir::ProbabilityCondition::State(ir::Cond::And(cs)) = &query.condition else {
                panic!("round observation")
            };
            let [
                ir::Cond::IntCompare {
                    variable: started, ..
                },
                ir::Cond::IntCompare {
                    variable: round, ..
                },
            ] = cs.as_slice()
            else {
                panic!("round variables")
            };
            assert_eq!(
                q.state_variables[started.0].name,
                format!("@loop:{}:started", l.name)
            );
            assert_eq!(
                q.state_variables[round.0].name,
                format!("@loop:{}:round", l.name)
            );
        }
        let ir::ProbabilityCondition::State(ir::Cond::IntCompare { variable, .. }) =
            &q.probability_queries[l.limit].condition
        else {
            panic!("limit observation")
        };
        assert_eq!(
            q.state_variables[variable.0].name,
            format!("@loop:{}:limit", l.name)
        );
    }
}
