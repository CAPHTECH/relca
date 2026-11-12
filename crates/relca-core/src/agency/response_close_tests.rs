//! Boundary inputs for response postprocessing, including an absent Agency.
#[test]
fn alias_only_uncertainty_becomes_none_not_empty_table() {
    let source = include_str!("../../../../fixtures/lin-basis/model.relca");
    let source = source.replace("bV = B.check(e)", "bV = B.check(e) response_loss none");
    let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    // The boundary receives one old suppression row for a nonexecuting alias.
    // Keep the production-derived payload; only select that row explicitly.
    let mut r = crate::report::check(&source);
    r.agency = Some(m.agency.clone());
    let u = r
        .agency
        .as_mut()
        .unwrap()
        .as_derived_mut()
        .unwrap()
        .model_uncertainties
        .as_mut()
        .unwrap();
    u.items.retain(|row| row.call == "bV");
    assert_eq!(u.items.len(), 1, "precondition: alias row exists");
    super::response::apply(&mut r, &m, None);
    assert!(
        r.agency
            .unwrap()
            .as_derived()
            .unwrap()
            .model_uncertainties
            .is_none()
    );
}

#[test]
fn absent_agency_still_withdraws_legacy_via_paths_with_reason() {
    let source = include_str!("../../../../fixtures/act-via/model.relca").replace(
        "e = Extract.make()",
        "e = Extract.make() response_loss none",
    );
    let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    let mut r = crate::report::check(&source);
    r.agency = None;
    r.via_model_paths = super::via_model_paths(&m);
    assert!(!r.via_model_paths.as_ref().unwrap().paths.is_empty());
    r.assumptions.retain(|a| a.field != "via_model_paths");
    super::response::apply(&mut r, &m, None);
    assert!(r.agency.is_none());
    assert!(
        r.via_model_paths.is_none(),
        "old paths must not survive absent Agency"
    );
    assert!(
        r.assumptions
            .iter()
            .any(|a| a.field == "via_model_paths" && a.value == "not_derived(response_control)")
    );
}
