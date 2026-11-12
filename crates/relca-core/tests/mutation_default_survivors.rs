//! ADR 0046 (c6) mutation survivors with a public input: F11,
//! X-sum-other-unprojected, X-auth-order and X-fresh-no-actor-state. Inputs
//! live in this file; the closed fixture population is not extended. Every
//! expectation is hand-derived first (the derivation is above each test).
//! The four function-only survivors are private cases of
//! tests/support/mutation_default_private.rs.
use relca_core::mutation;
use serde_json::{Value, json};

const SUMMARY: &str = include_str!("../../../fixtures/mut2-summary/model.relca");

fn replace_once(src: &str, from: &str, to: &str) -> String {
    assert_eq!(src.matches(from).count(), 1, "{from}");
    src.replacen(from, to, 1)
}

fn mutate(src: &str) -> Value {
    let v = serde_json::to_value(mutation::mutate(src)).unwrap();
    assert_eq!(
        v["original"]["verdict"], "satisfied",
        "{}",
        v["original"]["diagnostics"]
    );
    v
}

fn row(v: &Value, i: usize, kind: &str) -> Value {
    assert_eq!(v["mutations"][i]["kind"], kind);
    v["mutations"][i].clone()
}

/// mut2-summary with the Invoice read moved one call up: x = Reader.read()
/// reads [Invoice]; e = Extract.make(x) reads nothing. e's evidence sources
/// come only from the argument ancestry of x.
const ANCESTRY: &str = r#"model Summary version "0.2" {
  type Entry = enum { entry }
  type Verdict = enum { pass, fail }
  type Ack = enum { posted }
  actor Invoice { source document }
  actor PurchaseOrder { source record }
  actor Reader { capacity 1; capability read() -> Entry { reads [Invoice]; duration fixed 1min; result nondet { entry } } }
  actor Extract { capacity 1; capability make(x:Entry) -> Entry { duration fixed 1min; result nondet { entry } } }
  actor Clerk { capacity 1; capability match_po(e:Entry) -> Verdict { reads [PurchaseOrder]; duration fixed 1min; result nondet { pass, fail } } }
  actor Approver { capacity 1; capability approve(e:Entry) -> Verdict { duration fixed 1min; result nondet { pass, fail } } }
  actor Ledger { capacity 1; capability post(p:Verdict,a:Verdict) -> Ack { commits; duration fixed 1min; result nondet { posted } } }
  process Flow {
    x = Reader.read(); e = Extract.make(x); p = Clerk.match_po(e); a = Approver.approve(e)
    if p == pass && a == pass {} else { end rejected }
    z = Ledger.post(p,a); end done
  }
  cut Scope { include [Invoice,PurchaseOrder,Reader,Extract,Clerk,Approver,Ledger] }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify reachable end.done
  verify safety no_resource_overcommit
}"#;

/// F11 (sources must include argument ancestry) and X-sum-other-unprojected
/// (other_changes must drop the leaf an assigned row already reports).
///
/// Producer sources: x = {Invoice} (reads); e = reads {} plus x's {Invoice} =
/// {Invoice}. Consumers: x -> e only; e -> p, a; p -> z; a -> z. One site,
/// the second consumer of e: summary_reused:Flow:body/1:e->a, e_dup.
/// Judgements (ancestors or guard support of the effect z with a non-case
/// argument): original e, p, a; mutant e, e_dup, p, a. All have subject
/// {Invoice}, so group ["Invoice"] goes 3 -> 4 (not reduced). a keeps subject
/// [Invoice] and independent [] -> held, detected, three held rows.
/// With direct reads only (F11), e's sources are empty -> no_assigned_check.
/// The group row 3 -> 4 is assigned, so the agency leaf
/// /lineage_groups/subject=["Invoice"]/judgements must not stay in
/// other_changes (it would, without the projection).
#[test]
fn f11_ancestry_sources_and_assigned_group_leaf_not_in_other_changes() {
    let v = mutate(ANCESTRY);
    let r = row(&v, 4, "summary_reused");
    assert_eq!(r["status"], "generated");
    assert_eq!(r["mutants"].as_array().unwrap().len(), 1);
    let m = &r["mutants"][0];
    assert_eq!(m["id"], "summary_reused:Flow:body/1:e->a");
    assert_eq!(m["site"]["statement_path"], "body/1");
    assert_eq!(m["site"]["affected_bindings"], json!(["e", "a"]));
    assert_eq!(m["site"]["duplicate_binding"], "e_dup");
    assert_eq!(m["result"], "detected", "{}", m["assigned_changes"]);
    assert!(m["reason"].is_null());
    assert_eq!(
        m["assigned_changes"],
        json!([
            {"check":"agency.lineage","key":"a:Approver.approve","field":"subject_sources","before":["Invoice"],"after":["Invoice"],"direction":"held"},
            {"check":"agency.lineage","key":"a:Approver.approve","field":"independent_sources","before":[],"after":[],"direction":"held"},
            {"check":"agency.lineage_groups","key":"[\"Invoice\"]","field":"judgements","before":3,"after":4,"direction":"held"}
        ])
    );
    let leaf: Vec<_> = m["other_changes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| {
            c["check"] == "agency" && c["key"] == "/lineage_groups/subject=[\"Invoice\"]/judgements"
        })
        .collect();
    assert_eq!(leaf, Vec::<&Value>::new());
}

/// Two named Authorities on one Approver (executor a1, issued by cfo).
const TWO_AUTHORITIES: &str = r#"model Expiry version "0.2" {
  type Verdict = enum { pass, fail }
  people [a1,cfo]
  actor Approver { executors [a1]; capability approve() -> Verdict { duration fixed 1min; result nondet { pass } } capability review() -> Verdict { duration fixed 1min; result nondet { pass } } }
  authority ApprovalGrant { basis "approval table"; grants a1 to Approver.approve; issued_by cfo }
  authority ReviewGrant { basis "review table"; grants a1 to Approver.review; issued_by cfo }
  process Flow { v = Approver.approve(); w = Approver.review(); end done }
  cut Scope { include [Approver] }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify safety no_resource_overcommit
  contract Controls { authority_basis Approver.approve ApprovalGrant; authority_basis Approver.review ReviewGrant }
}"#;

/// X-auth-order: authority_expired sites follow declaration order.
/// Both Authorities are named by authority_basis and have no expires, so two
/// sites: ApprovalGrant (v) then ReviewGrant (w). v runs [0,1], w [1,2]
/// (live_tasks 1). Expiry 0min: finish 1 > 0 violates #1 only in the first
/// mutant, finish 2 > 0 violates #2 only in the second -> both detected, one
/// worsened row each (the other authority_basis is unchanged, so no row).
#[test]
fn authority_sites_follow_declaration_order() {
    let v = mutate(TWO_AUTHORITIES);
    let r = row(&v, 1, "authority_expired");
    assert_eq!(r["status"], "generated");
    let ms = r["mutants"].as_array().unwrap();
    let ids: Vec<_> = ms.iter().map(|m| m["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        [
            "authority_expired:ApprovalGrant:expires",
            "authority_expired:ReviewGrant:expires"
        ]
    );
    for (m, (authority, binding, n)) in ms
        .iter()
        .zip([("ApprovalGrant", "v", 1), ("ReviewGrant", "w", 2)])
    {
        assert_eq!(m["site"]["authority"], authority);
        assert_eq!(m["site"]["affected_bindings"], json!([binding]));
        assert_eq!(m["result"], "detected");
        assert_eq!(
            m["assigned_changes"],
            json!([{"check":"properties","key":format!("contract Controls #{n} authority_basis"),"field":"verdict","before":"satisfied","after":"violated","direction":"worsened"}])
        );
    }
}

/// X-fresh-no-actor-state: the first unused `<b>_dup` counts Actor state and
/// latent names as used (ADR 0046 U6, ruled for c6: the reserved names include
/// Actor state, latent, names, people, types, Authorities and fork names).
/// mut2-summary has the one site e->a (p keeps e). With an Actor variable
/// named e_dup the copy is e_dup2; the rest is mut2-summary's held row
/// (a: subject [Invoice], independent []; group ["Invoice"] p, a = 2 -> 2,
/// the copy has no argument and is no judgement) -> detected.
#[test]
fn fresh_name_skips_actor_state_and_latent_names() {
    let state = replace_once(
        SUMMARY,
        "actor Clerk { capacity 1;",
        "actor Clerk { state e_dup: bool = false; capacity 1;",
    );
    let latent = replace_once(
        SUMMARY,
        "  actor Invoice { source document }\n",
        "  type Difficulty = enum { routine, difficult }\n  actor Invoice { source document; latent e_dup: Difficulty ~ { routine: 0.8, difficult: 0.2 } }\n",
    );
    for src in [state, latent] {
        let v = mutate(&src);
        let r = row(&v, 4, "summary_reused");
        assert_eq!(r["status"], "generated");
        assert_eq!(r["mutants"].as_array().unwrap().len(), 1);
        let m = &r["mutants"][0];
        assert_eq!(m["id"], "summary_reused:Flow:body/0:e->a");
        assert_eq!(m["site"]["duplicate_binding"], "e_dup2");
        assert_eq!(m["result"], "detected");
        assert!(
            m["assigned_changes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["direction"] == "held")
        );
    }
}
