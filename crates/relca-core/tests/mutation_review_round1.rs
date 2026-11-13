//! Round 1 acceptance: lexical arithmetic, not captured mutation output.
use relca_core::{
    Verdict,
    mutation::{self, Options},
};
#[path = "support/response_loss_inputs.rs"]
mod response;

pub const MULTI: &str = r#"model Scope version "0.2" {
 type V = enum { pass, fail }
 observed case { hit: bool = false }
 actor D1 { source document }
 actor D2 { source record }
 actor Shared { capacity 1; capability read() -> V { reads [D1]; duration fixed 1min; result nondet { pass } } }
 actor A { capacity 1; capability read() -> V { reads [D1]; duration fixed 1min; result nondet { pass, fail } } }
 actor B { capacity 1; capability read() -> V { reads [D2]; duration fixed 1min; result nondet { pass, fail } } }
 actor Finish { capacity 1; capability finish() -> V { duration fixed 1min; result nondet { pass }; writes hit; finish { hit = true } } }
 process One { s1 = Shared.read(); x1 = A.read(); if x1 == pass {} else { end old_no }; y1 = Finish.finish(); end old_yes }
 process Two { s2 = Shared.read(); x2 = B.read(); if x2 == pass {} else { end new_no }; y2 = Finish.finish(); end new_yes }
 cut C { include [D1,D2,Shared,A,B,Finish] }
 configuration First { cut C; process One }
 configuration Second { cut C; process Two }
 bounds { cases 1; live_tasks 2; clock 20min }
 verify safety no_resource_overcommit
 property case.Hit = reached(hit == true)
 metric cohort.Rate = expected(count(case.Hit)) / arrivals
}"#;

fn run(src: &str, kinds: &str, sites: Option<&str>) -> mutation::MutationReport {
    let options = Options::parse(None, Some(kinds))
        .unwrap()
        .with_sites(sites)
        .unwrap();
    mutation::mutate_with_options(src, "bpr-default", None, &options)
}
fn unchecked(r: &mutation::MutationReport, stage: &str) {
    assert!(
        !r.checked.iter().any(|s| s == stage),
        "{stage}: {:?}",
        r.checked
    );
    assert!(
        r.unchecked.iter().any(|s| s == stage),
        "{stage}: {:?}",
        r.unchecked
    );
}

#[test]
fn m1_effective_process_controls_every_candidate_family() {
    let options = Options::default().with_configuration(Some("Second"));
    let r = mutation::mutate_with_options(MULTI, "bpr-default", None, &options);
    assert_eq!(
        r.original_verdict,
        Some(Verdict::Satisfied),
        "{:?}",
        r.original.as_ref().map(|r| &r.diagnostics)
    );
    // Shared + B: 2 reads twice; one guard twice; 2 distinct ends = 8.
    assert_eq!(r.run.candidates_total, Some(8));
    for i in [0, 9] {
        assert_eq!(r.mutations[i].candidates, Some(2));
        for m in &r.mutations[i].mutants {
            assert_ne!(m.site.actor.as_deref(), Some("A"));
            assert!(
                m.site
                    .affected_bindings
                    .iter()
                    .all(|b| b == "s2" || b == "x2")
            );
        }
    }
    for i in [2, 8] {
        assert_eq!(r.mutations[i].candidates, Some(1));
        assert_eq!(
            r.mutations[i].mutants[0].site.process.as_deref(),
            Some("Two")
        );
    }
    assert_eq!(r.mutations[7].candidates, Some(2));
    for m in &r.mutations[7].mutants {
        assert_eq!(m.site.process.as_deref(), Some("Two"));
        assert!(m.site.removed_end.as_deref().unwrap().starts_with("new_"));
    }
    // Round 2: one configuration still chooses one effective process.
    // Shared + A: 2 reads twice; one guard twice; 2 old_* ends = 8.
    let one = MULTI.replace(" configuration Second { cut C; process Two }", "");
    let r = mutation::mutate_with_options(
        &one,
        "bpr-default",
        None,
        &Options::default().with_configuration(Some("First")),
    );
    assert_eq!(r.original_verdict, Some(Verdict::Satisfied));
    assert_eq!(r.run.candidates_total, Some(8));
    for i in [0, 9] {
        assert_eq!(r.mutations[i].candidates, Some(2));
        assert_eq!(r.mutations[i].mutants.len(), 2);
        for (m, (actor, binding)) in r.mutations[i]
            .mutants
            .iter()
            .zip([("Shared", "s1"), ("A", "x1")])
        {
            // Reads are declaration sites, not statement sites: process is null.
            assert_eq!(m.site.process, None);
            assert_eq!(m.site.actor.as_deref(), Some(actor));
            assert_eq!(m.site.affected_bindings, [binding]);
        }
    }
    for i in [2, 8] {
        assert_eq!(r.mutations[i].candidates, Some(1));
        assert_eq!(r.mutations[i].mutants.len(), 1);
        assert_eq!(
            r.mutations[i].mutants[0].site.process.as_deref(),
            Some("One")
        );
    }
    assert_eq!(r.mutations[7].candidates, Some(2));
    assert_eq!(r.mutations[7].mutants.len(), 2);
    let mut ends = Vec::new();
    for m in &r.mutations[7].mutants {
        assert_eq!(m.site.process.as_deref(), Some("One"));
        ends.push(m.site.removed_end.as_deref().unwrap());
    }
    ends.sort();
    assert_eq!(ends, ["old_no", "old_yes"]);
    for i in [1, 4] {
        assert_eq!(r.mutations[i].candidates, Some(0));
        assert!(r.mutations[i].mutants.is_empty());
    }
}

#[test]
fn m2_partial_and_empty_site_selections_keep_kind_and_whole_run_stages_unchecked() {
    let src = include_str!("../../../fixtures/mut2-expiry/model.relca")
  .replace("  contract Controls", "  authority H { basis \"second\"; grants a1 to Approver.approve; issued_by cfo }\n  contract Controls")
  .replace("authority_basis Approver.approve ApprovalGrant", "authority_basis Approver.approve ApprovalGrant; authority_basis Approver.approve H");
    let r = run(
        &src,
        "authority_expired",
        Some("authority_expired:ApprovalGrant:expires"),
    );
    assert_eq!(r.mutations[1].mutants.len(), 2);
    for s in [
        "authority_expiry",
        "single_fault_generation",
        "assigned_check_comparison",
    ] {
        unchecked(&r, s);
    }
    let r = mutation::mutate_with_options(
        MULTI,
        "bpr-default",
        None,
        &Options::parse(None, Some("old_promise_denominator_removed"))
            .unwrap()
            .with_sites(Some(
                "old_promise_denominator_removed:cohort.Rate:remove:new_yes",
            ))
            .unwrap()
            .with_configuration(Some("Second")),
    );
    unchecked(&r, "quality_denominator");
    for s in ["single_fault_generation", "assigned_check_comparison"] {
        unchecked(&r, s);
    }
    let qualified = MULTI.replace("actor Shared { capacity 1;", "people [p,cfo]\n actor Shared { capacity 1; executors [p];")
        .replace(" bounds {", " authority G { basis \"b\"; grants p to Shared.read; issued_by cfo }\n contract Auth { authority_basis Shared.read G }\n bounds {");
    for (kind, id, i, stage) in [
        (
            "authority_expired",
            "old_promise_denominator_removed:cohort.Rate:remove:new_yes",
            1,
            "authority_expiry",
        ),
        (
            "old_promise_denominator_removed",
            "authority_expired:G:expires",
            7,
            "quality_denominator",
        ),
    ] {
        let options = Options::parse(None, Some(kind))
            .unwrap()
            .with_sites(Some(id))
            .unwrap()
            .with_configuration(Some("Second"));
        let r = mutation::mutate_with_options(&qualified, "bpr-default", None, &options);
        assert!(!r.mutations[i].mutants.is_empty(), "{:?}", r.diagnostics);
        assert!(
            r.mutations[i]
                .mutants
                .iter()
                .all(|m| m.result == "not_selected")
        );
        unchecked(&r, stage);
    }
    let src = response::service("g", response::NONE).replace(
        ";\n        if ack",
        "; next = Ledger.post(case.id) response_loss none;\n        if ack",
    );
    for (kind, stage) in [
        ("command_resent", "command_resend"),
        ("receipt_unknown", "receipt_unknown"),
    ] {
        let id = format!("{kind}:Flow:{}", src.find("ack =").unwrap());
        let r = run(&src, kind, Some(&id));
        let i = if kind == "command_resent" { 3 } else { 6 };
        assert_eq!(r.mutations[i].mutants.len(), 2);
        unchecked(&r, stage);
        let other = if kind == "command_resent" {
            "receipt_unknown"
        } else {
            "command_resent"
        };
        let id = format!("{other}:Flow:{}", src.find("ack =").unwrap());
        let r = run(&src, kind, Some(&id));
        assert!(
            r.mutations[i]
                .mutants
                .iter()
                .all(|m| m.result == "not_selected")
        );
        unchecked(&r, stage);
    }
}

#[test]
fn minor1_no_executed_mutant_is_vacuous() {
    let src = include_str!("../../relca-cli/tests/support/mutation_limits.relca");
    for r in [
        run(src, "authority_expired", None),
        run(
            src,
            "evidence_missing",
            Some("original_access_removed:Probe.read:reads:B"),
        ),
    ] {
        assert_eq!(r.verdict, Verdict::Vacuous);
        assert_eq!(r.run.reason, Some("no_selected_mutant"));
        assert_eq!(r.run.completion, "complete");
    }
}

#[test]
fn minor2_positive_backoff_prevents_zero_time_inapplicability_even_after_end() {
    let src = r#"model Backoff version "0.2" {
 type T = enum { ok }
 people [a1,cfo]
 actor A { executors [a1]; capability approve() -> T { duration fixed 0min; result nondet { ok } } }
 actor L { capacity 1; capability post(key:CaseId) -> T { commits; effect_scope modeled; duration fixed 0min; result nondet { ok }; cancel forbidden_after_start } }
 authority G { basis "b"; grants a1 to A.approve; issued_by cfo }
 process P { v = A.approve(); end done; x = L.post(case.id) response_loss none retry max_attempts 2 when receipt unknown backoff fixed 1min exhausted end failed }
 cut C { include [A,L] }
 bounds { cases 1; live_tasks 1; clock 10min }
 verify safety no_resource_overcommit
 contract C { authority_basis A.approve G }
 }"#;
    let r = run(src, "authority_expired", None);
    assert_eq!(
        r.original_verdict,
        Some(Verdict::Satisfied),
        "{:?}",
        r.original.as_ref().map(|r| &r.diagnostics)
    );
    assert_eq!(
        r.original.as_ref().unwrap().case_outcomes.as_ref().unwrap()[0].terminals[0]
            .completed_at
            .unwrap()
            .max,
        0
    );
    assert_eq!(r.mutations[1].status, "generated");
    assert_eq!(r.mutations[1].mutants[0].result, "undetected");
}

#[test]
fn minor3_advancing_time_pair_keeps_an_undetected_expiry_fault() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for (name, result, at, original) in [
        ("mut3-expiry-window", "detected", 1, Verdict::Satisfied),
        (
            "mut3-expiry-window.broken-late-arrival",
            "undetected",
            11,
            Verdict::Violated,
        ),
    ] {
        let dir = root.join(name);
        assert!(
            dir.join("model.relca").exists(),
            "required advancing-time fixture is missing: {name}"
        );
        let src = std::fs::read_to_string(dir.join("model.relca")).unwrap();
        // New fixtures do not alter the frozen legacy migration populations;
        // both originals and both expiry edits still receive full reference checks.
        for source in [&src, &src.replace("expires 5min", "expires 0min")] {
            let ast = relca_core::syntax::parse(source).unwrap();
            let model = relca_core::lower::lower(&ast).unwrap();
            let engine = relca_core::engine::explore(&model);
            let mut reference = relca_core::reference::explore(&model);
            assert_eq!(reference.stop, relca_core::reference::StopReason::Complete);
            assert!(
                reference
                    .outcome
                    .diagnostics
                    .starts_with(&model.diagnostics)
            );
            reference
                .outcome
                .diagnostics
                .drain(..model.diagnostics.len());
            assert_eq!(engine, reference.outcome);
        }
        let r = run(&src, "authority_expired", None);
        assert_eq!(r.original_verdict, Some(original));
        assert_eq!(r.mutations[1].candidates, Some(1));
        assert_eq!(r.mutations[1].mutants[0].result, result);
        assert_eq!(
            r.original.as_ref().unwrap().case_outcomes.as_ref().unwrap()[0].terminals[0]
                .completed_at
                .unwrap()
                .max,
            at
        );
        for file in ["calc.md", "expected.json", "invocation.json"] {
            assert!(dir.join(file).exists());
        }
    }
}
