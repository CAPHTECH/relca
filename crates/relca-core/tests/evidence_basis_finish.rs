//! Finish guarantees for observed-snapshot writers, independently of call order.
use relca_core::{Verdict, engine, ir, lower, outcome, reference, syntax};

fn edit(src: &str, old: &str, new: &str) -> String {
    assert_eq!(src.matches(old).count(), 1, "{old}");
    src.replace(old, new)
}

fn add(src: &str, text: &str) -> String {
    let end = src.rfind('}').unwrap();
    format!("{}\n{text}\n}}", &src[..end])
}

fn dominating() -> String {
    edit(
        include_str!("evidence_basis/m5.relca"),
        "if e == pass { p = Clerk.match_po(e) } else {}",
        "p = Clerk.match_po(e)",
    )
}

fn basis(src: &str, sources: &str) -> String {
    add(
        src,
        &format!("contract Basis {{ evidence_basis Approver.approve [{sources}] }}"),
    )
}

fn definite_missing(src: &str) -> String {
    let src = edit(
        src,
        "actor PurchaseOrder",
        "actor Missing { source record }\n  actor PurchaseOrder",
    );
    let src = edit(&src, "include [", "include [Missing,");
    basis(&src, "PurchaseOrder,Missing")
}

fn model(src: &str) -> ir::Model {
    lower::lower(&syntax::parse(src).unwrap()).unwrap()
}

fn pair(src: &str, expected: Verdict) -> (outcome::Outcome, reference::ReferenceRun) {
    let m = model(src);
    pair_model(&m, expected)
}

fn pair_model(m: &ir::Model, expected: Verdict) -> (outcome::Outcome, reference::ReferenceRun) {
    let e = engine::explore(m);
    let r = reference::explore(m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(
        e.properties, r.outcome.properties,
        "independent path interpretation"
    );
    let rows: Vec<_> = e
        .properties
        .iter()
        .filter(|p| matches!(p.property, ir::Property::Contract(_)))
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].verdict, expected);
    (e, r)
}

fn conditional(src: &str) {
    let ast = syntax::parse(src).unwrap();
    match lower::lower(&ast) {
        Err(ds) => assert!(
            ds.iter()
                .any(|d| d.code == "U-SEM-0" && d.message == "evidence_basis_conditional_snapshot")
        ),
        Ok(m) => {
            // On the unfixed implementation the requires case reaches both
            // interpreters and exposes satisfied versus violated, before this
            // test checks the required stage-one refusal.
            let e = engine::explore(&m);
            let r = reference::explore(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            assert_eq!(
                e.properties, r.outcome.properties,
                "a non-Finish writer is not a must source"
            );
            assert_eq!(
                e.properties
                    .iter()
                    .find(|p| matches!(p.property, ir::Property::Contract(_)))
                    .unwrap()
                    .verdict,
                Verdict::Unsupported
            );
            assert!(e.diagnostics.iter().any(|d| d.code == "U-SEM-0"
                && d.message.contains("evidence_basis_conditional_snapshot")));
        }
    }
}

fn refused_writer() -> String {
    let src = edit(
        &dominating(),
        "observed case {",
        "observed case { allowed: bool = false;",
    );
    edit(
        &src,
        "capability match_po(e:V) -> V {",
        "capability match_po(e:V) -> V { requires allowed == true else fail;",
    )
}

#[test]
fn dominating_requires_refusal_is_not_a_must_finish() {
    let src = refused_writer();
    let witness = add(
        &src,
        "contract Witness { invariant Approver.approve.finish -> po == pass }",
    );
    let (e, r) = pair(&witness, Verdict::Violated);
    assert!(!e.capability_refusals.as_ref().unwrap().is_empty());
    assert!(!r.outcome.capability_refusals.as_ref().unwrap().is_empty());
    conditional(&basis(&src, "PurchaseOrder"));
    let direct_alternative = edit(&src, "reads [po]", "reads [po,PurchaseOrder]");
    pair(
        &basis(&direct_alternative, "PurchaseOrder"),
        Verdict::Satisfied,
    );
    pair(&definite_missing(&src), Verdict::Violated);
}

#[test]
fn definite_missing_source_still_violates_after_a_refusal_and_a_later_clock_cut() {
    let src = edit(&refused_writer(), "clock 10min", "clock 2min");
    let (e, r) = pair(&definite_missing(&src), Verdict::Violated);
    assert!(e.truncated && r.outcome.truncated);
}

fn cancelled_writer(cancel: &str, queued: bool, distributed: bool) -> String {
    let src = edit(&dominating(), "live_tasks 1", "live_tasks 2");
    let duration = if distributed {
        "duration {3min:0.5,4min:0.5}"
    } else {
        "duration fixed 3min"
    };
    let src = edit(
        &src,
        "duration fixed 1min; result nondet { pass }; writes po",
        &format!("{duration}; result nondet {{ pass }}; writes po; cancel {cancel}"),
    );
    let (src, fast) = if queued {
        (
            edit(
                &src,
                "capability enter()",
                "capability quick() -> V { duration fixed 1min; result nondet { pass }; cancel immediate }\n    capability enter()",
            ),
            "Clerk.quick()",
        )
    } else {
        let src = add(
            &src,
            "actor Fast { capacity 1; capability run() -> V { duration fixed 1min; result nondet { pass }; cancel immediate } }",
        );
        (
            edit(
                &src,
                "include [PurchaseOrder",
                "include [Fast,PurchaseOrder",
            ),
            "Fast.run()",
        )
    };
    edit(
        &src,
        "p = Clerk.match_po(e)",
        &format!(
            "fork f {{ p = Clerk.match_po(e); fast = {fast} }}\n    join any f where result == pass tie by [fast,p] remaining cancel_then_drain if none -> posted"
        ),
    )
}

fn cancellation(cancel: &str, queued: bool, distributed: bool) {
    let src = cancelled_writer(cancel, queued, distributed);
    let witness = add(
        &src,
        "contract Witness { invariant Approver.approve.finish -> po == pass }",
    );
    pair(&witness, Verdict::Violated);
    conditional(&basis(&src, "PurchaseOrder"));
    pair(&definite_missing(&src), Verdict::Violated);
}

#[test]
fn queued_cancellation_is_a_may_writer() {
    cancellation("immediate", true, false);
}

#[test]
fn running_immediate_cancellation_is_a_may_writer() {
    cancellation("immediate", false, false);
}

#[test]
fn cooperative_cancellation_is_a_may_writer() {
    cancellation("cooperative ack_within 1min", false, false);
}

#[test]
fn distributed_cancellation_is_a_may_writer() {
    cancellation("immediate", false, true);
}

fn response(src: &str, suffix: &str) -> String {
    edit(
        src,
        "p = Clerk.match_po(e)",
        &format!("p = Clerk.match_po(e) {suffix}"),
    )
}

#[test]
fn lost_receipt_does_not_finish_a_later_reader() {
    pair(
        &basis(
            &response(&dominating(), "response_loss sequence [unknown]"),
            "PurchaseOrder",
        ),
        Verdict::Vacuous,
    );
}

#[test]
fn unknown_retry_exhaustion_does_not_finish_a_later_reader() {
    let src = response(
        &dominating(),
        "response_loss sequence [unknown,unknown] retry max_attempts 2 when receipt unknown backoff fixed 1min exhausted end posted",
    );
    pair(&basis(&src, "PurchaseOrder"), Verdict::Vacuous);
}

#[test]
fn result_retry_exhaustion_does_not_finish_a_later_reader() {
    let src = edit(
        &dominating(),
        "result nondet { pass }; writes po",
        "result nondet { fail }; writes po",
    );
    let src = response(
        &src,
        "response_loss none retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end posted",
    );
    pair(&basis(&src, "PurchaseOrder"), Verdict::Vacuous);
}

#[test]
fn response_requires_refusal_is_a_may_writer() {
    let src = response(&refused_writer(), "response_loss none");
    conditional(&basis(&src, "PurchaseOrder"));
    pair(&definite_missing(&src), Verdict::Violated);
}

#[test]
fn an_ordinary_fail_result_is_still_a_normal_finish_source() {
    let src = edit(
        &dominating(),
        "result nondet { pass }; writes po",
        "result nondet { fail }; writes po",
    );
    pair(&basis(&src, "PurchaseOrder"), Verdict::Satisfied);
    pair(
        &basis(&response(&src, "response_loss none"), "PurchaseOrder"),
        Verdict::Satisfied,
    );
}

#[test]
fn response_refusal_can_bind_a_result_without_a_normal_finish() {
    let src = response(&refused_writer(), "response_loss none");
    let src = add(
        &src,
        "contract Basis { evidence_basis direct Clerk.match_po [PurchaseOrder] }",
    );
    let (e, r) = pair(&src, Verdict::Vacuous);
    assert!(!e.capability_refusals.as_ref().unwrap().is_empty());
    assert!(!r.outcome.capability_refusals.as_ref().unwrap().is_empty());
}

#[test]
fn optional_retry_attempts_are_not_must_snapshot_writers() {
    let src = edit(
        &dominating(),
        "result nondet { pass }; writes po",
        "result nondet { pass, fail }; writes po",
    );
    let src = response(
        &src,
        "response_loss none retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end posted",
    );
    let src = basis(&src, "PurchaseOrder");
    pair(&src, Verdict::Satisfied);
    let m = model(&src);
    let optional = m
        .process
        .body
        .iter()
        .find_map(|s| match &s.kind {
            ir::Stmt::Response(c, r) if r.attempt > 1 => Some(c.binding),
            _ => None,
        })
        .unwrap();
    let ir::ContractKind::EvidenceBasis { calls, .. } = &m.contracts[0].kind else {
        panic!()
    };
    let reader = calls
        .iter()
        .find(|c| m.process.bindings[c.binding.0].name == "a")
        .unwrap();
    assert!(reader.excluded_edges.contains(&format!(
        "{} -> {}",
        m.process.bindings[optional.0].name, m.process.bindings[reader.binding.0].name
    )));
}

#[test]
fn clock_cut_withholds_unfinished_snapshot_contracts() {
    let src = edit(&dominating(), "clock 10min", "clock 1min");
    let (e, r) = pair(&basis(&src, "PurchaseOrder"), Verdict::Inconclusive);
    assert!(e.truncated && r.outcome.truncated);
}

#[test]
fn live_task_cut_withholds_unfinished_snapshot_contracts() {
    let src = edit(&dominating(), "live_tasks 1", "live_tasks 0");
    let (e, r) = pair(&basis(&src, "PurchaseOrder"), Verdict::Inconclusive);
    assert!(e.truncated && r.outcome.truncated);
}

#[test]
fn state_budget_cut_is_not_a_finish_or_a_satisfied_source() {
    let src = basis(
        &edit(&dominating(), "clock 10min", "clock 0min"),
        "PurchaseOrder",
    );
    let m = model(&src);
    let e = engine::explore_with_budget(&m, 1);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert!(e.truncated && r.outcome.truncated);
    assert_eq!(e.properties, r.outcome.properties);
    assert!(
        e.properties
            .iter()
            .all(|p| p.verdict == Verdict::Inconclusive)
    );
}

#[test]
fn state_range_stop_does_not_promote_a_writer_to_finish() {
    let src = edit(
        &dominating(),
        "observed case {",
        "observed case { n: 0..1 = 1;",
    );
    let src = edit(&src, "writes po", "writes [po,n]; finish { n += 1 }");
    let (e, r) = pair(&basis(&src, "PurchaseOrder"), Verdict::Inconclusive);
    assert!(!e.state_range_violations.is_empty());
    assert!(!r.outcome.state_range_violations.is_empty());
}

#[test]
fn blocked_resource_does_not_manufacture_an_activation() {
    let src = edit(
        &dominating(),
        "actor Clerk { executors [c1]",
        "actor Clerk { capacity 0",
    );
    pair(&basis(&src, "PurchaseOrder"), Verdict::Vacuous);
}

#[test]
fn business_horizon_does_not_manufacture_an_activation() {
    let src = add(
        &dominating(),
        "scenario Window { arrivals fixed [0min]; horizon 1min }",
    );
    let src = basis(&src, "PurchaseOrder");
    let m =
        lower::lower_with_selection(&syntax::parse(&src).unwrap(), None, Some("Window")).unwrap();
    let (e, r) = pair_model(&m, Verdict::Inconclusive);
    assert!(!e.truncated && !r.outcome.truncated);
}

#[test]
fn arithmetic_abort_does_not_fabricate_a_writer_finish() {
    let src = edit(
        &dominating(),
        "duration fixed 1min; result nondet { pass }; writes po",
        "duration fixed 18446744073709551615min; result nondet { pass }; writes po",
    );
    let src = edit(&src, "clock 10min", "clock 18446744073709551615min");
    let m = model(&basis(&src, "PurchaseOrder"));
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    assert_eq!(e.properties, r.outcome.properties);
    assert!(
        e.properties
            .iter()
            .all(|p| p.verdict == Verdict::Unsupported)
    );
}
