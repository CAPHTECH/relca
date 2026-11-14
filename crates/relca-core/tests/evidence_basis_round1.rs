use relca_core::{Verdict, engine, ir, lower, reference, report, syntax};

fn edit(src: &str, old: &str, new: &str) -> String {
    assert_eq!(src.matches(old).count(), 1, "{old}");
    src.replace(old, new)
}
fn add(src: &str, text: &str) -> String {
    let end = src.rfind('}').unwrap();
    format!("{}\n{text}\n}}", &src[..end])
}
fn basis(src: &str, target: &str, sources: &str) -> String {
    add(
        src,
        &format!("contract Basis {{ evidence_basis {target} [{sources}] }}"),
    )
}
fn argref() -> String {
    let src = include_str!("evidence_basis/m0.relca");
    let src = edit(src, "if p == pass {} else { end returned }", "");
    let src = edit(&src, "Approver.approve(e)", "Approver.approve(p)");
    let src = edit(
        &src,
        "capability match_po(e:V) -> V {",
        "capability match_po(e:V) -> V { requires allowed == true else fail;",
    );
    add(&src, "observed case { allowed: bool = false }")
}
fn dominating() -> String {
    edit(
        include_str!("evidence_basis/m5.relca"),
        "if e == pass { p = Clerk.match_po(e) } else {}",
        "p = Clerk.match_po(e)",
    )
}
fn pair(src: &str, expected: Verdict) {
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for (name, out) in [("engine", &e), ("reference", &r.outcome)] {
        let row = out
            .properties
            .iter()
            .find(|p| p.property == ir::Property::Contract(ir::ClauseId(0)))
            .unwrap();
        assert_eq!(row.verdict, expected, "{name}: hand-computed provenance");
    }
    assert_eq!(e.properties, r.outcome.properties);
}

#[test]
fn refused_else_argument_has_no_producer_reads() {
    pair(
        &basis(&argref(), "Approver.approve", "PurchaseOrder"),
        Verdict::Violated,
    );
}

#[test]
fn accepted_argument_and_ordinary_fail_result_keep_normal_producer_reads() {
    let src = edit(&argref(), "allowed: bool = false", "allowed: bool = true");
    pair(
        &basis(&src, "Approver.approve", "PurchaseOrder"),
        Verdict::Satisfied,
    );
    let src = edit(
        &src,
        "result nondet { pass, fail }",
        "result nondet { fail }",
    );
    pair(
        &basis(&src, "Approver.approve", "PurchaseOrder"),
        Verdict::Satisfied,
    );
}

#[test]
fn refusal_can_happen_on_one_path_and_missing_source_must_violate() {
    let src = add(&argref(), "type B = enum { yes,no }");
    let src = edit(&src, "allowed: bool = false", "allowed: B = no");
    let src = edit(&src, "allowed == true", "allowed == yes");
    let src = edit(
        &src,
        "capability enter() -> V { reads [Invoice]; duration fixed 1min; result nondet { pass } }",
        "capability enter() -> B { reads [Invoice]; duration fixed 1min; result nondet { yes,no }; writes allowed }",
    );
    let src = edit(&src, "match_po(e:V)", "match_po(e:B)");
    pair(
        &basis(&src, "Approver.approve", "PurchaseOrder"),
        Verdict::Violated,
    );
}

#[test]
fn a_normal_forwarder_cannot_restore_reads_of_a_refused_ancestor() {
    let src = edit(
        &argref(),
        "capability enter()",
        "capability forward(x:V) -> V { duration fixed 1min; result nondet { pass } }\n    capability enter()",
    );
    let src = edit(
        &src,
        "a = Approver.approve(p)",
        "q = Clerk.forward(p); a = Approver.approve(q)",
    );
    pair(
        &basis(&src, "Approver.approve", "PurchaseOrder"),
        Verdict::Violated,
    );
}

#[test]
fn another_real_argument_or_direct_read_is_not_erased_by_refusal() {
    let src = edit(&argref(), "approve(e:V)", "approve(e:V,p:V)");
    let src = edit(&src, "Approver.approve(p)", "Approver.approve(e,p)");
    pair(
        &basis(&src, "Approver.approve", "Invoice"),
        Verdict::Satisfied,
    );
    let src = edit(
        &src,
        "capability approve(e:V,p:V) -> V {",
        "capability approve(e:V,p:V) -> V { reads [PurchaseOrder];",
    );
    pair(
        &basis(&src, "Approver.approve", "Invoice,PurchaseOrder"),
        Verdict::Satisfied,
    );
}

#[test]
fn adopted_response_refusal_does_not_import_producer_reads() {
    let src = edit(
        &argref(),
        "p = Clerk.match_po(e)",
        "p = Clerk.match_po(e) response_loss none",
    );
    pair(
        &basis(&src, "Approver.approve", "PurchaseOrder"),
        Verdict::Violated,
    );
}

#[test]
fn delivered_normal_response_keeps_the_normal_value_sources() {
    let src = edit(&argref(), "allowed: bool = false", "allowed: bool = true");
    let src = edit(
        &src,
        "p = Clerk.match_po(e)",
        "p = Clerk.match_po(e) response_loss none retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end returned",
    );
    pair(
        &basis(&src, "Approver.approve", "PurchaseOrder"),
        Verdict::Satisfied,
    );
}

#[test]
fn retry_exhaustion_does_not_publish_a_value_to_a_later_consumer() {
    let src = edit(
        &argref(),
        "p = Clerk.match_po(e)",
        "p = Clerk.match_po(e) response_loss none retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end returned",
    );
    pair(
        &basis(&src, "Approver.approve", "PurchaseOrder"),
        Verdict::Vacuous,
    );
}

#[test]
fn unconditional_last_write_replaces_old_source_even_when_value_is_equal() {
    for result in ["pass", "fail"] {
        let src = edit(
            &dominating(),
            "capability enter()",
            &format!(
                "capability overwrite() -> V {{ duration fixed 1min; result nondet {{ {result} }}; writes po }}\n    capability enter()"
            ),
        );
        let src = edit(
            &src,
            "a = Approver.approve(e)",
            "w = Clerk.overwrite(); a = Approver.approve(e)",
        );
        pair(
            &basis(&src, "Approver.approve", "PurchaseOrder"),
            Verdict::Violated,
        );
    }
}

#[test]
fn conditional_overwrite_is_unknown_unless_a_direct_source_suffices() {
    let src = edit(
        &dominating(),
        "capability enter()",
        "capability overwrite() -> V { duration fixed 1min; result nondet { fail }; writes po }\n    capability enter()",
    );
    let src = edit(
        &src,
        "a = Approver.approve(e)",
        "if e == pass { w = Clerk.overwrite() } else {}; a = Approver.approve(e)",
    );
    let rejected = report::check(&basis(&src, "Approver.approve", "PurchaseOrder"));
    assert_eq!(rejected.verdict, Verdict::Unsupported);
    assert!(
        rejected
            .diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0"
                && d.message.contains("evidence_basis_conditional_snapshot"))
    );
    let src = edit(&src, "reads [po]", "reads [po,PurchaseOrder]");
    pair(
        &basis(&src, "Approver.approve", "PurchaseOrder"),
        Verdict::Satisfied,
    );
}

#[test]
fn definite_violation_at_another_call_survives_conditional_snapshot() {
    let src = edit(
        include_str!("evidence_basis/m5.relca"),
        "e = Clerk.enter()",
        "e = Clerk.enter(); a0 = Approver.approve(e)",
    );
    pair(
        &basis(&src, "Approver.approve", "PurchaseOrder"),
        Verdict::Violated,
    );
}

#[test]
fn definite_violation_on_another_branch_survives_conditional_snapshot() {
    let src = edit(
        include_str!("evidence_basis/m5.relca"),
        "else {}",
        "else { a0 = Approver.approve(e); end returned }",
    );
    pair(
        &basis(&src, "Approver.approve", "PurchaseOrder"),
        Verdict::Violated,
    );
}

#[test]
fn contest_route_only_withholds_the_evidence_property() {
    // Use the real invoice source with its executable contest route; unrelated
    // properties must keep the native check results rather than globally abort.
    let src = include_str!("../../../examples/invoice/as-is.relca");
    let before = report::check(src);
    let after = report::check(&basis(src, "Clerk.recheck", "Invoice"));
    assert_eq!(after.verdict, Verdict::Unsupported);
    assert!(
        after
            .diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0" && d.message.contains("evidence_basis_contest_route"))
    );
    for row in &before.properties {
        let new = after.properties.iter().find(|p| p.id == row.id).unwrap();
        assert_eq!(new.verdict, row.verdict, "{}", row.id);
    }
}

#[test]
fn required_source_outside_cut_uses_existing_contract_cut_error() {
    let src = add(
        include_str!("evidence_basis/m0.relca"),
        "actor Spare { source record }",
    );
    let src = edit(
        &src,
        "include [PurchaseOrder,Invoice,Clerk,Approver,Ledger]",
        "include [PurchaseOrder,Invoice,Clerk,Approver,Ledger]; exclude Spare reason \"out\" loses \"source access\"",
    );
    let ds =
        lower::lower(&syntax::parse(&basis(&src, "Clerk.match_po", "Spare")).unwrap()).unwrap_err();
    assert!(
        ds.iter()
            .any(|d| d.code == "E-CUT-1" && d.message.contains("evidence_basis"))
    );
}

#[test]
fn cancelled_result_is_not_definitely_available_as_an_argument() {
    let src = edit(&argref(), "requires allowed == true else fail; ", "");
    let src = edit(
        &src,
        "reads [PurchaseOrder]; duration fixed 1min; result nondet { pass, fail }",
        "reads [PurchaseOrder]; duration fixed 3min; result nondet { pass }; cancel immediate",
    );
    let src = add(
        &src,
        "actor Fast { capacity 1; capability run(e:V) -> V { duration fixed 1min; result nondet { pass }; cancel immediate } }",
    );
    let src = edit(
        &src,
        "include [PurchaseOrder",
        "include [Fast,PurchaseOrder",
    );
    let src = edit(&src, "live_tasks 1", "live_tasks 2");
    let src = edit(
        &src,
        "p = Clerk.match_po(e)",
        "fork f { p = Clerk.match_po(e); q = Fast.run(e) }; join any f where result == pass tie by [q,p] remaining cancel_then_drain if none -> returned",
    );
    let ds =
        lower::lower(&syntax::parse(&basis(&src, "Approver.approve", "PurchaseOrder")).unwrap())
            .unwrap_err();
    assert!(ds.iter().any(|d| {
        d.message
            .contains("argument result is not definitely available")
    }));
}

#[test]
fn fallback_degradation_and_repetition_aliases_remain_explicitly_unsupported() {
    let base = include_str!("evidence_basis/m0.relca");
    let src = edit(
        base,
        "a = Approver.approve(e)",
        "degrade Manual when p == pass { a = Approver.approve(e) } else { end returned }",
    );
    let src = edit(&src, "x = Ledger.post(p,a)", "");
    let ds =
        lower::lower(&syntax::parse(&basis(&src, "Approver.approve", "PurchaseOrder")).unwrap())
            .unwrap_err();
    assert!(
        ds.iter()
            .any(|d| d.message.contains("evidence_basis_feature_combination"))
    );
    let src = edit(
        base,
        "e = Clerk.enter()",
        "repeat attempt at_most 1 { e = Clerk.enter(); leave } on_limit { end returned }",
    );
    let ds =
        lower::lower(&syntax::parse(&basis(&src, "Approver.approve", "PurchaseOrder")).unwrap())
            .unwrap_err();
    assert!(
        ds.iter()
            .any(|d| d.message.contains("loop_feature_combination"))
    );
}
