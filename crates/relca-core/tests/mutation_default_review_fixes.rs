//! Boundary review of ADR 0046 (c6): M1, m5 and m7. Inputs live in this file;
//! the closed fixture population is not extended. Every expectation below is
//! hand-derived first (the derivation is in the comment above each assert).
use relca_core::{mutation, report};
use serde_json::{Value, json};

const TO_BE_P: &str = include_str!("../../../examples/invoice/to-be-p.relca");

fn wlin2(r: &report::Report) -> Vec<String> {
    r.diagnostics
        .iter()
        .filter(|d| d.code == "W-LIN-2")
        .map(|d| d.message.clone())
        .collect()
}

fn replace_once(src: &str, from: &str, to: &str) -> String {
    assert_eq!(src.matches(from).count(), 1, "{from}");
    src.replacen(from, to, 1)
}

fn summary_row(src: &str) -> Value {
    let v = serde_json::to_value(mutation::mutate(src)).unwrap();
    assert_eq!(v["mutations"][4]["kind"], "summary_reused");
    v["mutations"][4].clone()
}

const APPROVAL_WLIN2: &str = "approvalV Approver.approve kernel approval_check targets [entry]: conditional independence under given assumes detection without a new evidence source";

/// M1 (fixes the current behaviour, does not change it). W-LIN-2 groups the
/// consumers of a latent by the producer *binding* (ADR 0024), so a hand copy
/// of the producer of receiptV splits one group into two.
///
/// Original To-Be P: entry = Clerk.enter() kernel-writes extraction_correct;
/// po_match, receipt_match and approval_check are all given it. Group of entry =
/// {poV, receiptV, approvalV}. approvalV has no reads; its subject sources
/// {Invoice, PurchaseOrder, ReceiptRecord} are all covered by the peers poV
/// ({Invoice, PurchaseOrder}) and receiptV ({Invoice, ReceiptRecord}), and no
/// peer result is in its given -> 1 W-LIN-2 targets [entry]. poV and receiptV
/// each bring a new explicit record -> none. Total 1.
///
/// Hand copy (entry_dup = Clerk.enter() right after entry, receiptV reads
/// entry_dup): group(entry) = {poV, approvalV}, group(entry_dup) = {receiptV,
/// approvalV}. For approvalV, peers of entry cover {Invoice, PurchaseOrder}
/// only (ReceiptRecord is new) and peers of entry_dup cover {Invoice,
/// ReceiptRecord} only (PurchaseOrder is new) -> no target. poV: its only
/// group peer approvalV descends from it -> Invoice is new -> none. Total 0,
/// although the lineage of every judgement is the same Invoice.
///
/// The other copy (entry_dup feeds approvalV): group(entry) = {poV, receiptV,
/// approvalV} as before, group(entry_dup) = {approvalV} has size 1 -> 1, same
/// message.
#[test]
fn m1_to_be_p_hand_copy_of_receipt_producer_drops_wlin2_by_binding_identity() {
    let original = report::check(TO_BE_P);
    assert_eq!(wlin2(&original), vec![APPROVAL_WLIN2.to_string()]);

    let copied = replace_once(
        TO_BE_P,
        "    entry = Clerk.enter()\n",
        "    entry = Clerk.enter()\n    entry_dup = Clerk.enter()\n",
    );
    let receipt = replace_once(
        &copied,
        "receiptV = Clerk.match_receipt(entry)",
        "receiptV = Clerk.match_receipt(entry_dup)",
    );
    let r = report::check(&receipt);
    // The second draw replaces extraction_correct before every check reads it
    // and is drawn from the same row given the same difficulty, so the
    // decided verdict cannot move; only the W-LIN-2 count does.
    assert_eq!(r.verdict, original.verdict, "{:?}", r.diagnostics);
    assert_eq!(wlin2(&r), Vec::<String>::new());

    let approval = replace_once(
        &copied,
        "approvalV = Approver.approve(entry,poV,receiptV)",
        "approvalV = Approver.approve(entry_dup,poV,receiptV)",
    );
    let r = report::check(&approval);
    assert_eq!(r.verdict, original.verdict, "{:?}", r.diagnostics);
    assert_eq!(wlin2(&r), vec![APPROVAL_WLIN2.to_string()]);

    // The generated mutant is the same edit: lineage held -> detected, and the
    // lost W-LIN-2 shows only as one diagnostics line of other_changes.
    let row = summary_row(TO_BE_P);
    let m = &row["mutants"][0];
    assert_eq!(m["id"], "summary_reused:ToBePFlow:body/0:entry->receiptV");
    assert_eq!(m["result"], "detected");
    assert!(
        m["assigned_changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["direction"] == "held")
    );
    let lost: Vec<_> = m["other_changes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["check"] == "diagnostics" && c["key"].as_str().unwrap().contains("W-LIN-2"))
        .collect();
    assert_eq!(
        lost,
        vec![
            &json!({"check":"diagnostics","key":"/code=\"W-LIN-2\";severity=\"warning\"","field":"value","before":1,"after":null,"direction":"changed"})
        ]
    );
    let n = m["report"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-LIN-2")
        .count();
    assert_eq!(n, 0);
}

/// m5 base: one case. A.extract kernel-writes its own Actor latent `hidden`
/// (lower accepts this for one case; multi-case rejects it). e feeds y and z,
/// which guard the commit L.post, so y and z are judgements with lineage rows.
/// e is the only producer with two consumers (y, z each feed only w).
const LATENT_WRITER: &str = r#"model M5 version "0.2" {
  type R = enum { ok }
  type V = enum { pass, fail }
  type Ack = enum { posted }
  kernel K { * -> { 1: ok with hidden = ok } }
  actor Doc { source document }
  actor Ref1 { source record }
  actor Ref2 { source record }
  actor A { capacity 1; latent hidden: R = ok
    capability extract() -> R { reads [Doc]; writes hidden; duration fixed 1min; outcomes kernel K }
  }
  actor C { capacity 2
    capability check1(x:R) -> V { reads [Ref1]; duration fixed 1min; result nondet { pass, fail } }
    capability check2(x:R) -> V { reads [Ref2]; duration fixed 1min; result nondet { pass, fail } }
  }
  actor L { capacity 1; capability post(p:V,q:V) -> Ack { commits; duration fixed 1min; result nondet { posted } } }
  cut Scope { include [Doc,Ref1,Ref2,A,C,L] }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify reachable end.done
  process P {
    e = A.extract(); y = C.check1(e); z = C.check2(e)
    if y == pass && z == pass {} else { end rejected }
    w = L.post(y,z); end done
  }
}"#;

/// m5 (fix): a producer that writes Actor latent state is a shared-state
/// writer like an ActorObserved writer; copying it would add a second write
/// besides the reused summary, so it is not a site (fail closed). The only
/// candidate e has two consumers -> without the exclusion one mutant e->z.
#[test]
fn m5_actor_latent_writer_is_not_a_summary_producer() {
    let original = report::check(LATENT_WRITER);
    // end.done is reached at 4min <= 10min (y and z pass).
    assert_eq!(
        serde_json::to_value(original.verdict).unwrap(),
        "satisfied",
        "{:?}",
        original.diagnostics
    );
    let row = summary_row(LATENT_WRITER);
    assert_eq!(row["status"], "not_applicable");
    assert_eq!(row["reason"], "no_matching_site");
    assert_eq!(row["mutants"], json!([]));
}

/// Control of m5: the same model without the latent write still yields the
/// one site e->z (y keeps e), so the exclusion is the latent write only.
/// z: subject {Doc} via e, independent {Ref2}; after the copy e_dup reads Doc
/// too, so both sets and the {Doc} group (judgements y and z = 2) are kept ->
/// detected. (First hand version had no effect, hence no judgements and an
/// empty lineage -> undetected/no_assigned_check; the commit was added.)
#[test]
fn m5_control_without_latent_write_keeps_the_site() {
    let control = replace_once(
        LATENT_WRITER,
        "writes hidden; duration fixed 1min; outcomes kernel K",
        "duration fixed 1min; result nondet { ok }",
    );
    let row = summary_row(&control);
    assert_eq!(row["status"], "generated");
    let ids: Vec<_> = row["mutants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["summary_reused:P:body/0:e->z"]);
    assert_eq!(row["mutants"][0]["site"]["duplicate_binding"], "e_dup");
    assert_eq!(row["mutants"][0]["result"], "detected");
}

/// m7 base: the original is violated and cut by the clock.
/// make [0,1], match_po [1,2] writes po, approve [2,3]. At approve.finish (3min
/// <= clock 3min) po may be pass -> invariant K violated. If both pass, post
/// runs [3,4] and the 4min state is cut -> truncated. Otherwise end.rejected
/// at 3min.
const TRUNCATED_VIOLATED: &str = r#"model Trunc version "0.2" {
  type Entry = enum { entry }
  type Verdict = enum { pass, fail }
  type Ack = enum { posted }
  observed case { po: optional<Verdict> }
  actor Invoice { source document }
  actor PurchaseOrder { source record }
  actor Extract { via model "doc@v3"; capacity 1; capability make() -> Entry { reads [Invoice]; duration fixed 1min; result nondet { entry } } }
  actor Clerk { capacity 1; capability match_po(e:Entry) -> Verdict { reads [PurchaseOrder]; duration fixed 1min; result nondet { pass, fail }; writes po } }
  actor Approver { capacity 1; capability approve(e:Entry) -> Verdict { duration fixed 1min; result nondet { pass, fail } } }
  actor Ledger { capacity 1; capability post(p:Verdict,a:Verdict) -> Ack { commits; duration fixed 1min; result nondet { posted } } }
  contract K { invariant Approver.approve.finish -> po == fail }
  process Flow {
    e = Extract.make(); p = Clerk.match_po(e); a = Approver.approve(e)
    if p == pass && a == pass {} else { end rejected }
    z = Ledger.post(p,a); end done
  }
  cut Scope { include [Invoice,PurchaseOrder,Extract,Clerk,Approver,Ledger] }
  bounds { cases 1; live_tasks 2; clock 3min }
  verify safety no_resource_overcommit
}"#;

/// m7 (fixes the current behaviour). The only site is e->a (p keeps e; p and a
/// have one consumer each). The copy adds e_dup [1,2] before match_po, so
/// approve ends at 4min > 3min: every path of the mutant is cut as well. The
/// original truncation is not consulted by the summary comparison; the mutant
/// truncation is, so the row is inconclusive/truncated, not detected and not
/// original_truncated. A copied producer only adds time on each path, so an
/// original cut by the clock always yields a cut mutant here [R].
#[test]
fn m7_truncated_violated_original_summary_row_is_mutant_truncated() {
    let original = report::check(TRUNCATED_VIOLATED);
    assert_eq!(
        serde_json::to_value(original.verdict).unwrap(),
        "violated",
        "{:?}",
        original.diagnostics
    );
    assert_eq!(original.bounds.truncated, Some(true));
    let row = summary_row(TRUNCATED_VIOLATED);
    assert_eq!(row["status"], "generated");
    assert!(row["reason"].is_null());
    assert_eq!(row["mutants"].as_array().unwrap().len(), 1);
    let m = &row["mutants"][0];
    assert_eq!(m["id"], "summary_reused:Flow:body/0:e->a");
    assert_eq!(m["site"]["duplicate_binding"], "e_dup");
    assert_eq!(m["site"]["affected_bindings"], json!(["e", "a"]));
    assert_eq!(m["result"], "inconclusive");
    assert_eq!(m["reason"], "truncated");
    assert_eq!(m["assigned_changes"], json!([]));
    assert_eq!(m["other_changes"], json!([]));
    assert_eq!(m["report"]["bounds"]["truncated"], true);
}
