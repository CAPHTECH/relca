use relca_core::{ast, syntax};

#[test]
fn evidence_clause_is_contextual_and_preserves_source_paths() {
    for (text, direct, count) in [
        ("Clerk.run [Invoice, Invoice.difficulty]", false, 2),
        ("direct Clerk.run [Invoice]", true, 1),
        ("direct.run []", false, 0),
        ("direct\n.run []", false, 0),
    ] {
        let src =
            format!("model M version \"0.2\" {{ contract Basis {{ evidence_basis {text} }} }}");
        let model = syntax::parse(&src).unwrap();
        let ast::Item::Contract(contract) = &model.items[0] else {
            panic!()
        };
        let ast::ContractClause::EvidenceBasis {
            direct: actual,
            sources,
            ..
        } = &contract.clauses[0].value
        else {
            panic!()
        };
        assert_eq!(*actual, direct);
        assert_eq!(sources.len(), count);
    }
}

#[test]
fn evidence_clause_follows_a_bare_state_invariant_without_a_semicolon() {
    let src = "model M version \"0.2\" { contract Basis { invariant ready\nevidence_basis Clerk.run [Invoice] } }";
    let model = syntax::parse(src).unwrap();
    assert!(model.has_evidence_basis());
    let ast::Item::Contract(c) = &model.items[0] else {
        panic!()
    };
    assert_eq!(c.clauses.len(), 2);
}
