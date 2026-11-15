use relca_core::{ast, syntax};

#[test]
fn same_version_clause_is_contextual_and_ends_a_bare_invariant() {
    let src = "model M version \"0.2\" { contract V { invariant ready\nsame_version A.check(x), B.use(y) } }";
    let m = syntax::parse(src).unwrap();
    let ast::Item::Contract(c) = &m.items[0] else {
        panic!()
    };
    assert_eq!(c.clauses.len(), 2);
    let ast::ContractClause::SameVersion { first, second } = &c.clauses[1].value else {
        panic!()
    };
    assert_eq!(
        (
            &first.0.actor.value,
            &first.0.capability.value,
            &first.1.value
        ),
        (&"A".into(), &"check".into(), &"x".into())
    );
    assert_eq!(
        (
            &second.0.actor.value,
            &second.0.capability.value,
            &second.1.value
        ),
        (&"B".into(), &"use".into(), &"y".into())
    );
    for clause in [
        "same_version A.check(), B.use(y)",
        "same_version A.check(x), B.use(y,z)",
    ] {
        assert!(
            syntax::parse(&format!(
                "model M version \"0.2\" {{ contract V {{ {clause} }} }}"
            ))
            .is_err()
        );
    }
}
