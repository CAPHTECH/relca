use relca_core::{
    Verdict,
    mutation::{self, Options},
};
#[path = "mutation_review_round1.rs"]
mod earlier;

fn run(src: &str, options: &Options) -> mutation::MutationReport {
    mutation::mutate_with_options(src, "bpr-default", None, options)
}
fn absent(r: &mutation::MutationReport, stage: &str) {
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
fn unsupported_zero_trials_keep_precedence_but_supported_empty_scope_is_vacuous() {
    let src = include_str!("../../relca-cli/tests/support/mutation_limits.relca");
    let r = run(
        src,
        &Options::parse(None, Some("stale_definition_write")).unwrap(),
    );
    assert_eq!(r.verdict, Verdict::Unsupported);
    assert_eq!(r.run.reason, None);
    let r = run(
        src,
        &Options::parse(None, Some("authority_expired")).unwrap(),
    );
    assert_eq!(r.verdict, Verdict::Vacuous);
    assert_eq!(r.run.reason, Some("no_selected_mutant"));
}

#[test]
fn kind_exclusion_and_zero_trials_do_not_claim_whole_run_or_comparison_stages() {
    let src = include_str!("../../relca-cli/tests/support/mutation_limits.relca");
    let r = run(
        src,
        &Options::parse(None, Some("original_access_removed")).unwrap(),
    );
    assert_eq!(r.mutations[9].mutants.len(), 2); // both actually checked
    for stage in ["single_fault_generation", "assigned_check_comparison"] {
        absent(&r, stage);
    }
    for r in [
        run(
            include_str!("../../../fixtures/mut-empty/model.relca"),
            &Options::default(),
        ),
        run(
            src,
            &Options::parse(None, Some("authority_expired")).unwrap(),
        ),
    ] {
        assert!(r.mutations.iter().all(|row| row.mutants.is_empty()));
        for stage in [
            "single_fault_generation",
            "assigned_check_comparison",
            "authority_expiry",
            "command_resend",
            "receipt_unknown",
            "quality_denominator",
        ] {
            absent(&r, stage);
        }
    }
}

#[test]
fn positive_wait_and_wait_until_after_end_prevent_zero_time_expiry_classification() {
    let base = include_str!("../../../fixtures/mut2-expiry/model.relca")
        .replace("duration fixed 1min", "duration fixed 0min")
        .replace(
            "  type Verdict",
            "  observed case { ready: bool = false }\n  type Verdict",
        );
    for statement in ["wait 1min", "wait until ready == true"] {
        let source = base.replace("end done }", &format!("end done; {statement} }}"));
        let r = run(
            &source,
            &Options::parse(None, Some("authority_expired")).unwrap(),
        );
        assert_eq!(
            r.original_verdict,
            Some(Verdict::Satisfied),
            "{statement}: {:?}",
            r.original.as_ref().map(|r| &r.diagnostics)
        );
        assert_eq!(r.mutations[1].status, "generated", "{statement}");
        assert_eq!(r.mutations[1].mutants[0].result, "undetected");
    }
}

#[test]
fn authority_called_only_by_other_process_is_not_an_expiry_candidate() {
    let src = earlier::MULTI
        .replace("actor A { capacity 1;", "people [p,cfo]\n actor A { capacity 1; executors [p];")
        .replace("actor B { capacity 1;", "actor B { capacity 1; executors [p];")
        .replace(" bounds {", " authority GA { basis \"a\"; grants p to A.read; issued_by cfo }\n authority GB { basis \"b\"; grants p to B.read; issued_by cfo }\n contract Auth { authority_basis A.read GA; authority_basis B.read GB; invariant Shared.read.finish -> hit == true }\n bounds {");
    let r = run(
        &src,
        &Options::parse(None, Some("authority_expired"))
            .unwrap()
            .with_configuration(Some("Second")),
    );
    assert_eq!(
        r.original_verdict,
        Some(Verdict::Violated),
        "{:?}",
        r.original.as_ref().map(|r| &r.diagnostics)
    );
    assert_eq!(r.mutations[1].candidates, Some(1));
    let m = &r.mutations[1].mutants[0];
    assert_eq!(m.site.authority.as_deref(), Some("GB"));
    assert_eq!(m.site.affected_bindings, ["x2"]);
}

#[test]
fn explicit_denominator_keeps_whole_definition_but_only_selected_end_removals() {
    let src = earlier::MULTI.replace(" metric cohort.Rate = expected(count(case.Hit)) / arrivals",
        " property case.Denom = reached(end.old_no || end.new_no || end.new_yes)\n metric cohort.Rate = expected(count(case.Hit)) / expected(count(case.Denom))");
    let r = run(
        &src,
        &Options::parse(None, Some("old_promise_denominator_removed"))
            .unwrap()
            .with_configuration(Some("Second")),
    );
    assert_eq!(
        r.original_verdict,
        Some(Verdict::Satisfied),
        "{:?}",
        r.original.as_ref().map(|r| &r.diagnostics)
    );
    assert_eq!(r.mutations[7].candidates, Some(2));
    for m in &r.mutations[7].mutants {
        assert!(m.site.removed_end.as_deref().unwrap().starts_with("new_"));
        assert_eq!(m.assigned_changes.len(), 2);
        assert_eq!(
            serde_json::to_value(&m.assigned_changes[0].before).unwrap(),
            serde_json::json!(["old_no", "new_no", "new_yes"])
        );
    }
}

#[test]
fn recovery_gate_ignores_recovery_only_in_an_unselected_process() {
    let source = include_str!("../../relca-cli/tests/snapshots/inputs/r2-comparison.relca");
    let start = source.find("  process Flow {").unwrap();
    let end = source.find("  process Other {").unwrap();
    let source = format!("{}  process Flow {{ pay = Psp.charge_keyed(case.id) response_loss none; normal = Shop.ship_known(pay); end shipped }}\n{}", &source[..start], &source[end..])
        .replace("receipt_query charge_keyed key key availability available", "result nondet {committed_once}");
    let r = run(
        &source,
        &Options::parse(None, Some("authority_expired"))
            .unwrap()
            .with_configuration(Some("A")),
    );
    assert_eq!(
        r.original_verdict,
        Some(Verdict::Satisfied),
        "{:?}",
        r.original.as_ref().map(|r| &r.diagnostics)
    );
    assert_ne!(r.run.completion, "blocked", "{:?}", r.diagnostics);
    assert!(
        !r.diagnostics
            .iter()
            .any(|d| d.message == "recovery_feature_combination")
    );
    let selected_recovery = run(&source, &Options::default().with_configuration(Some("B")));
    assert_eq!(selected_recovery.verdict, Verdict::Unsupported);
    assert!(
        selected_recovery
            .diagnostics
            .iter()
            .any(|d| d.message == "recovery_feature_combination")
    );
}
