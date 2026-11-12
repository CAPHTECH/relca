//! ADR0056 stage-C acceptance. RED tests are not ignored or feature-gated.
#[path = "support/response_loss_oracle.rs"]
mod hand;
#[path = "support/response_loss_inputs.rs"]
mod input;

use relca_core::{Verdict, engine, ir, lower, mutation, reference, report, syntax};
use serde_json::json;

fn check(name: &str) {
    let s = input::source(name);
    hand::check(&json!(report::check(&s)), name);
}

macro_rules! runtime {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check($name);
        }
    };
}
runtime!(
    v0109_g_keeps_one_commit_and_returns_duplicate_not_committed,
    "g-resend"
);
runtime!(
    v0109_concrete_non_idempotent_effect_commits_twice,
    "modeled-resend"
);
runtime!(
    v0109_one_fault_first_delivery_prevents_second_task,
    "modeled-first-delivered"
);
runtime!(
    v0109_g_first_delivery_does_not_spuriously_retry,
    "g-first-delivered"
);
runtime!(
    v0112_fixed_bad_input_stays_one_tenth_after_two_attempts,
    "correlated"
);
runtime!(
    v0112_tight_upper_is_violated_not_independent_one_hundredth,
    "correlated-tight"
);
runtime!(v0112_backoff_changes_elapsed_not_work, "backoff");
runtime!(
    v0112_explicit_independent_kernel_has_one_hundredth_final_failure,
    "independent"
);
runtime!(
    v0214_g_matches_one_fiftieth_with_three_minutes,
    "k8-g-resend"
);
runtime!(
    v0214_final_unknown_is_not_success_or_failure,
    "k8-g-unknown"
);
runtime!(v0214_clock_cut_is_not_a_receipt_unknown_stop, "k8-g-clock");
runtime!(
    receipt_unknown_keeps_effect_and_has_dedicated_outcome,
    "single-unknown"
);
runtime!(receipt_delivered_control_has_normal_end, "single-delivered");
runtime!(receipt_two_unknowns_keep_commit_once, "g-unknown");
runtime!(nondeterminism_is_not_half_probability, "g-nondet");
runtime!(
    omitted_loss_declaration_on_retry_defaults_to_nondeterminism,
    "g-default"
);
runtime!(basis_probability_has_d_ud_uu_masses, "g-probability");
runtime!(
    probability_zero_is_reliable_not_missing_configuration,
    "g-probability-zero"
);
runtime!(
    probability_one_is_unknown_not_known_failure,
    "g-probability-one"
);
runtime!(finite_limit_one_includes_the_first_attempt, "max-one");
runtime!(
    finite_expansion_limit_thirty_two_is_accepted,
    "max-thirty-two"
);
runtime!(clock_cut_does_not_turn_into_known_failure, "g-clock");
runtime!(arrivals_keep_case_keys_and_receipts_separate, "g-arrivals");
runtime!(actor_name_and_grain_do_not_create_idempotency, "g-label");
runtime!(known_fail_uses_exhausted_destination, "known-fail");
runtime!(known_pass_stops_on_first_attempt, "known-pass");
runtime!(
    requires_refusal_has_no_task_or_response_loss,
    "requires-refusal"
);
runtime!(
    removing_refusal_enables_effect_and_unknown,
    "requires-allowed"
);

macro_rules! refusal {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            let (_, code, reason) = input::STATIC.iter().find(|(n, _, _)| *n == $name).unwrap();
            hand::static_failure(&json!(report::check(&input::source($name))), code, reason);
        }
    };
}
refusal!(
    r_resend_is_semantically_unsupported_not_note_only_success,
    "r-resend"
);
refusal!(
    r_without_contract_cannot_evade_unsupported,
    "r-without-once"
);
refusal!(v0214_r_with_response_loss_is_unsupported, "k8-r-resend");
refusal!(
    concrete_counter_requires_explicit_effect_scope,
    "modeled-missing-scope"
);
refusal!(
    idempotency_declaration_without_atomic_is_not_a_guarantee,
    "idempotency-without-atomic"
);
refusal!(
    idempotency_checks_same_set_as_insert,
    "idempotency-wrong-set"
);
refusal!(
    idempotency_does_not_cover_repeatable_start_effects,
    "idempotency-start-write"
);
refusal!(zero_total_attempts_is_invalid, "max-zero");
refusal!(
    thirty_three_attempts_is_unsupported_not_silently_capped,
    "max-thirty-three"
);
refusal!(
    short_sequence_is_not_filled_with_delivered,
    "sequence-short"
);
refusal!(
    extra_sequence_entry_is_not_silently_dropped,
    "sequence-extra"
);
refusal!(
    missing_failure_target_is_a_syntax_error,
    "missing-exhausted"
);
refusal!(missing_backoff_is_not_zero_by_default, "missing-backoff");
refusal!(probability_requires_basis, "missing-basis");
refusal!(empty_basis_is_not_evidence, "empty-basis");
refusal!(probability_above_one_is_invalid, "invalid-probability");
refusal!(
    fork_retry_crossing_is_semantically_unsupported,
    "fork-retry"
);
refusal!(
    version_check_retry_crossing_is_semantically_unsupported,
    "version-retry"
);

#[test]
fn pairs_are_exactly_one_predeclared_edit_each() {
    for &(a, b, from, to) in input::PAIRS {
        assert_eq!(
            input::replace(&input::source(a), from, to),
            input::source(b),
            "{a} -> {b}"
        );
    }
}

#[test]
fn finite_grammar_parses_every_declared_runtime_input_before_engine_work() {
    assert_eq!(input::RUNTIME.len(), 28);
    assert_eq!(input::STATIC.len(), 18);
    assert_eq!(input::PAIRS.len(), 22);
    for name in input::RUNTIME {
        syntax::parse(&input::source(name))
            .unwrap_or_else(|d| panic!("{name}: new grammar not admitted: {d:?}"));
    }
}

#[test]
fn bare_retry_remains_unsupported_outside_the_new_finite_grammar() {
    let s = input::replace(
        &input::substrate("correlated"),
        "v = Check.run()",
        "v = Check.run(); retry 3",
    );
    let v = json!(report::check(&s));
    hand::verdict(&v, "unsupported");
    assert!(
        hand::rows(&v["diagnostics"])
            .iter()
            .any(|d| d["code"] == "U-SYN-0")
    );
}

fn model(s: &str) -> ir::Model {
    let a =
        syntax::parse(s).unwrap_or_else(|d| panic!("front-end has not admitted ADR0056: {d:?}"));
    lower::lower(&a).unwrap_or_else(|d| panic!("ADR0056 lower rejection: {d:?}"))
}

fn terminal_outcome(
    m: &ir::Model,
    o: &relca_core::outcome::Outcome,
    end: &str,
    min: u64,
    max: u64,
    work_min: u64,
    work_max: u64,
) {
    let ts: Vec<_> = o
        .terminals
        .iter()
        .filter(|t| m.ends[t.end.0] == end)
        .collect();
    assert_eq!(ts.len(), 1, "one reachable {end}");
    assert_eq!((ts[0].elapsed.min, ts[0].elapsed.max), (min, max));
    assert_eq!((ts[0].work.min, ts[0].work.max), (work_min, work_max));
}

fn raw_hand(name: &str, m: &ir::Model, o: &relca_core::outcome::Outcome) {
    assert!(!o.truncated);
    let q = |i| {
        o.properties
            .iter()
            .find(|p| p.property == ir::Property::ProbabilityReached(i))
            .unwrap()
    };
    let ratio = |i, n: &str| {
        let p = q(i).probability.as_ref().unwrap();
        assert_eq!(
            (p.min.to_string(), p.max.to_string()),
            (n.to_owned(), n.to_owned())
        );
    };
    if ["correlated", "backoff", "independent", "max-thirty-two"].contains(&name) {
        let n = if name == "max-thirty-two" { 32 } else { 2 };
        terminal_outcome(
            m,
            o,
            "ok",
            1,
            if name == "independent" { 2 } else { 1 },
            1,
            if name == "independent" { 2 } else { 1 },
        );
        terminal_outcome(
            m,
            o,
            "failed",
            if name == "backoff" { 5 } else { n },
            if name == "backoff" { 5 } else { n },
            n,
            n,
        );
        ratio(
            0,
            if name == "independent" {
                "1/100"
            } else {
                "1/10"
            },
        );
    } else {
        let contract = o
            .properties
            .iter()
            .find(|p| matches!(p.property, ir::Property::Contract(_)))
            .unwrap();
        assert_eq!(
            contract.verdict,
            if name == "modeled-resend" {
                Verdict::Violated
            } else {
                Verdict::Satisfied
            }
        );
        if name.starts_with("k8-") {
            ratio(0, "1/50");
            ratio(1, "1");
        } else {
            ratio(0, "1");
            ratio(1, if name == "modeled-resend" { "1" } else { "0" });
        }
        if [
            "single-unknown",
            "g-unknown",
            "modeled-resend",
            "k8-g-unknown",
        ]
        .contains(&name)
        {
            assert!(
                o.terminals.is_empty(),
                "unreceived result must not reach a normal End"
            );
            assert!(
                o.case_outcomes.as_ref().is_some_and(|c| !c.is_empty()),
                "must retain unknown case outcome"
            );
        } else if name == "g-arrivals" {
            let cases = o.case_outcomes.as_ref().unwrap();
            assert_eq!(cases.len(), 2);
            for c in cases {
                assert_eq!(c.terminals.len(), 1);
                assert_eq!(c.terminals[0].elapsed.unwrap().min, 2);
            }
        } else {
            terminal_outcome(
                m,
                o,
                if name == "k8-g-resend" {
                    "done"
                } else {
                    "duplicate"
                },
                if name == "k8-g-resend" { 3 } else { 2 },
                if name == "k8-g-resend" { 3 } else { 2 },
                if name == "k8-g-resend" { 3 } else { 2 },
                if name == "k8-g-resend" { 3 } else { 2 },
            );
        }
    }
}

#[test]
fn independent_reference_and_engine_each_match_hand_not_just_each_other() {
    for name in [
        "g-resend",
        "modeled-resend",
        "single-unknown",
        "g-unknown",
        "correlated",
        "backoff",
        "independent",
        "max-thirty-two",
        "k8-g-resend",
        "k8-g-unknown",
        "g-arrivals",
    ] {
        let m = model(&input::source(name));
        let e = engine::explore(&m);
        raw_hand(name, &m, &e);
        let r = reference::explore(&m);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "no runtime reference exclusions: {name}"
        );
        raw_hand(name, &m, &r.outcome);
    }
}

#[test]
fn old_syntax_substrates_are_green_before_new_semantics() {
    for name in [
        "g-resend",
        "modeled-resend",
        "r-resend",
        "correlated",
        "independent",
    ] {
        let s = input::substrate(name);
        let m = model(&s);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for o in [&e, &r.outcome] {
            assert!(!o.truncated);
            if ["correlated", "independent"].contains(&name) {
                terminal_outcome(&m, o, "ok", 1, 1, 1, 1);
                terminal_outcome(&m, o, "failed", 1, 1, 1, 1);
            } else {
                terminal_outcome(&m, o, "posted", 1, 1, 1, 1);
            }
        }
        assert_eq!(report::check(&s).verdict, Verdict::Satisfied, "{name}");
    }
}

#[test]
fn old_syntax_indicator_and_version_substrates_are_green() {
    let known = input::indicators(false)
        .replace("idempotency key key", "")
        .replace("response_loss sequence [delivered]", "");
    let v = json!(report::check(&known));
    hand::verdict(&v, "satisfied");
    assert_eq!(
        v["metrics"]["overall"]["observed_cost_total"],
        hand::metric("3")
    );
    assert_eq!(
        hand::row(&v["quality"]["cohort"], "name", "RegisteredPerArrival")["value"]["lower"],
        "1"
    );
    let version = include_str!("../../../fixtures/cf-version/model.relca");
    hand::verdict(&json!(report::check(version)), "satisfied");
    let early = input::replace(
        &input::substrate("g-resend"),
        "duration fixed 1min",
        "duration fixed 3min",
    );
    let early = input::append(
        &early,
        "contract Early { invariant registered == false -> registered == true }",
    );
    for (s, cut) in [
        (&early, false),
        (&input::replace(&early, "clock 40min", "clock 1min"), true),
    ] {
        let v = json!(report::check(s));
        hand::verdict(&v, "violated");
        assert_eq!(v["bounds"]["truncated"], cut);
    }
}

#[test]
fn failure_free_k8_r_and_g_keep_two_minutes_and_one_fiftieth() {
    for (s, note) in [
        (include_str!("../../../fixtures/k8-cut-r/model.relca"), true),
        (
            include_str!("../../../fixtures/k8-cut-g/model.relca"),
            false,
        ),
    ] {
        let v = json!(report::check(s));
        hand::verdict(&v, "satisfied");
        hand::terminal(&v, "done", 2, 2);
        hand::probability(&v, "probability reached #1", "1/50", "1/50");
        let p = hand::row(&v["properties"], "id", "contract C #1 at_most_once");
        assert_eq!(p.get("notes").is_some(), note);
        let m = model(s);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        terminal_outcome(&m, &r.outcome, "done", 2, 2, 2, 2);
    }
}

#[test]
fn unknown_blocks_downstream_task_instead_of_leaking_actual_binding() {
    let v = json!(report::check(&input::no_binding_leak_source(true)));
    hand::verdict(&v, "inconclusive");
    hand::unknown(&v, 1, 1);
    let known = json!(report::check(&input::no_binding_leak_source(false)));
    hand::verdict(&known, "satisfied");
    hand::terminal(&known, "posted", 8, 8);
}

#[test]
fn quality_and_metrics_keep_unknown_in_arrivals_and_do_not_count_completion() {
    hand::indicators(&json!(report::check(&input::indicators(true))), true);
    hand::indicators(&json!(report::check(&input::indicators(false))), false);
}

#[test]
fn quality_and_metrics_reference_are_independent_of_public_report() {
    for unknown in [true, false] {
        let m = model(&input::indicators(unknown));
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        let e = engine::explore(&m);
        for o in [&e, &r.outcome] {
            let v = json!({"metrics":o.metrics,"quality":o.quality});
            assert_eq!(
                v["metrics"]["overall"]["counts"]["completed"]["stats"],
                hand::metric(if unknown { "0" } else { "1" })
            );
            assert_eq!(
                v["metrics"]["overall"]["observed_cost_total"],
                hand::metric("3")
            );
            let q = hand::row(
                &v["quality"]["outcomes_per_arrival"],
                "kind",
                "receipt_unknown",
            );
            assert_eq!(q["rate"]["lower"], if unknown { "1" } else { "0" });
        }
    }
}

#[test]
fn explain_duplicate_has_independently_verified_attempt_and_receipt_history() {
    let s = input::source("modeled-resend");
    let v = json!(relca_core::explain::explain(&s));
    hand::check(&v, "modeled-resend");
    let c = hand::row(
        &v["explanation"]["counterexamples"],
        "id",
        "contract C #1 at_most_once",
    );
    assert_eq!(c["status"], "verified");
    let steps = hand::rows(&c["steps"]);
    let attempts: std::collections::BTreeSet<_> =
        steps.iter().filter_map(|s| s["attempt"].as_u64()).collect();
    assert_eq!(attempts, [1, 2].into_iter().collect());
    assert!(steps.iter().any(|s| s["observed_receipt"] == "unknown"));
    assert!(steps.iter().any(|s| {
        s["attempt"] == 2
            && (s["kind"] == "commit"
                || hand::rows(&s["events"])
                    .iter()
                    .any(|e| e["kind"] == "commit"))
    }));
    for step in steps.iter().filter(|s| s["attempt"].as_u64().is_some()) {
        assert!(step.get("command").is_some());
        assert!(
            step.get("location").is_some(),
            "DSL source location retained"
        );
    }
}

#[test]
fn deterministic_probability_endpoints_simulate_without_inventing_schedules() {
    for (name, kind, count) in [
        ("g-probability-zero", "end", 200),
        ("g-probability-one", "receipt_unknown", 200),
    ] {
        let v = json!(report::simulate(&input::simulation_source(name)));
        assert_ne!(v["verdict"], "unsupported", "{}", v["diagnostics"]);
        assert_ne!(v["verdict"], "invalid", "{}", v["diagnostics"]);
        let row = hand::row(&v["case_outcomes"][0]["terminals"], "kind", kind);
        assert_eq!(row["count"], count);
        if kind == "receipt_unknown" {
            assert!(row["end"].is_null());
        }
        assert_eq!(v["bounds"]["truncated"], false);
    }
}

#[test]
fn simulation_never_randomizes_unresolved_receipt_nondeterminism() {
    let v = json!(report::simulate(&input::simulation_source("g-nondet")));
    hand::verdict(&v, "unsupported");
    assert!(
        hand::rows(&v["diagnostics"])
            .iter()
            .any(|d| d["code"] == "U-SEM-0")
    );
}

#[test]
fn compare_and_diff_keep_receipts_end_mismatch_and_confirmed_child_violation() {
    let s = input::comparison_source("g-resend");
    let c = json!(relca_core::compare::compare(
        &s,
        &["A", "B"],
        relca_core::compare::Analysis::Exhaustive
    ));
    // P3a: Q1 itself is comparable, but BFlow's lexical End inventory differs.
    hand::verdict(&c, "invalid");
    assert!(c["mismatch"].is_object());
    assert!(
        hand::rows(&c["diagnostics"])
            .iter()
            .any(|d| d["code"] == "E-CMP-0")
    );
    assert_eq!(
        hand::rows(&c["plans"][0]["report"]["response_receipts"]).len(),
        2
    );
    let d = json!(relca_core::diff::diff(&s, "A", "B"));
    // Diff preserves unlike End inventories. B ends at zero without registering,
    // so its registered>=1 query violates and folds into the root.
    hand::verdict(&d, "violated");
    hand::verdict(&d["from"]["report"], "satisfied");
    hand::verdict(&d["to"]["report"], "violated");
    assert_eq!(
        hand::rows(&d["from"]["report"]["response_receipts"]).len(),
        2
    );
}

#[test]
fn mutation_g_preserves_once_without_calling_that_a_worsening_detection() {
    let v = json!(mutation::mutate(&input::mutation_source("g")));
    hand::mutation(&v, "command_resent", "undetected", Some("no_worsening"));
    hand::mutation(&v, "receipt_unknown", "detected", None);
    let resend = &hand::row(&v["mutations"], "kind", "command_resent")["mutants"][0];
    hand::check(&resend["report"], "g-resend");
    assert!(
        hand::rows(&resend["assigned_changes"])
            .iter()
            .any(|c| c["direction"] == "held")
    );
    assert_eq!(v["run"]["completion"], "complete");
}

#[test]
fn mutation_concrete_counter_detects_real_duplicate_but_r_is_invalid_not_detected() {
    let v = json!(mutation::mutate(&input::mutation_source("modeled")));
    hand::mutation(&v, "command_resent", "detected", None);
    hand::check(
        &hand::row(&v["mutations"], "kind", "command_resent")["mutants"][0]["report"],
        "modeled-resend",
    );
    let r = json!(mutation::mutate(&input::mutation_source("r")));
    hand::mutation(
        &r,
        "command_resent",
        "invalid_mutant",
        Some("static_unsupported"),
    );
    let m = &hand::row(&r["mutations"], "kind", "command_resent")["mutants"][0];
    hand::static_failure(
        &m["report"],
        "U-SEM-0",
        "response_loss_resend_mechanism_unmodeled",
    );
    assert_eq!(r["run"]["completion"], "inconclusive");
}

#[test]
fn mutation_declared_sites_and_reason_counts_are_closed_and_not_double_counted() {
    let v = json!(mutation::mutate(&input::skipped_source()));
    for kind in ["command_resent", "receipt_unknown"] {
        let r = hand::row(&v["mutations"], "kind", kind);
        assert_eq!(r["status"], "generated");
        assert_eq!(hand::rows(&r["mutants"]).len(), 1);
        assert_eq!(
            r["skipped_counts"],
            json!({"undeclared_site":1,"existing_response_loss":1,"existing_retry":1,"fork_branch":1})
        );
        let mut counts = std::collections::BTreeMap::<&str, u64>::new();
        for s in hand::rows(&r["skipped_sites"]) {
            *counts.entry(s["reason"].as_str().unwrap()).or_default() += 1;
        }
        assert_eq!(json!(counts), r["skipped_counts"]);
    }
}

#[test]
fn mutation_undeclared_call_is_skipped_even_with_idempotency_declaration() {
    let s = input::replace(&input::mutation_source("g"), input::NONE, "");
    let v = json!(mutation::mutate(&s));
    for kind in ["command_resent", "receipt_unknown"] {
        let r = hand::row(&v["mutations"], "kind", kind);
        assert!(hand::rows(&r["mutants"]).is_empty());
        assert_eq!(r["skipped_counts"], json!({"undeclared_site":1}));
    }
}

#[test]
fn mutation_clock_cut_cannot_use_receipt_held_exception() {
    // A normal post fits; the injected retry does not. One computational cut only.
    let s = input::replace(&input::mutation_source("g"), "clock 40min", "clock 1min");
    let v = json!(mutation::mutate(&s));
    let m = &hand::row(&v["mutations"], "kind", "command_resent")["mutants"][0];
    assert_eq!(m["result"], "inconclusive");
    assert_eq!(m["report"]["bounds"]["truncated"], true);
    assert_eq!(v["run"]["completion"], "inconclusive");
}

#[test]
fn receipt_held_exception_does_not_erase_a_real_computational_cut() {
    // A false state invariant supplies a real violation witness before the
    // clock cut, so the original is decidable/violated, not blocked as merely
    // inconclusive. Neither operator reaches the three-minute completion.
    let s = input::replace(
        &input::mutation_source("g"),
        "duration fixed 1min",
        "duration fixed 3min",
    );
    let s = input::append(
        &s,
        "contract Early { invariant registered == false -> registered == true }",
    );
    let before = json!(report::check(&s));
    hand::verdict(&before, "violated");
    assert_eq!(before["bounds"]["truncated"], false);
    // One fault relative to that common three-minute/invariant substrate.
    let s = input::replace(&s, "clock 40min", "clock 1min");
    let v = json!(mutation::mutate(&s));
    assert_eq!(v["original"]["verdict"], "violated");
    assert_eq!(v["original"]["bounds"]["truncated"], true);
    let m = &hand::row(&v["mutations"], "kind", "receipt_unknown")["mutants"][0];
    assert_eq!(m["result"], "inconclusive");
    assert_eq!(m["report"]["bounds"]["truncated"], true);
    assert!(
        !hand::rows(&m["assigned_changes"])
            .iter()
            .any(|c| c["direction"] == "held")
    );
}

#[test]
fn mutation_unreachable_site_is_not_a_receipt_held_detection() {
    let s = input::replace(
        &input::mutation_source("g"),
        "process Flow {",
        "process Flow { end untouched;",
    );
    let v = json!(mutation::mutate(&s));
    hand::mutation(
        &v,
        "receipt_unknown",
        "undetected",
        Some("no_assigned_check"),
    );
}

#[test]
fn mutation_budget_keeps_old_trials_before_new_operators_and_lists_not_run() {
    let mut s = input::mutation_source("g");
    let body: String = (0..129)
        .map(|i| format!("a{i} = Ledger.post(case.id) response_loss none;\n"))
        .collect();
    s = input::replace(&s, "ack = Ledger.post(case.id) response_loss none;", &body);
    // Make every mutation cheap to run, but keep a violated original probability
    // query so the original gate is decidable (not a vacuous-only root).
    s = input::replace(
        &s,
        "if ack == rejected_duplicate { end duplicate } else { end posted }",
        "end posted",
    );
    s = input::replace(&s, "process Flow {", "process Flow { end untouched;");
    s = input::replace(&s, "commits; writes", "reads [Original]; commits; writes");
    s = input::replace(&s, "include [Ledger]", "include [Ledger, Original]");
    s = input::append(&s, "actor Original { source document }");
    let v = json!(mutation::mutate(&s));
    assert_eq!(v["run"]["mutant_limit"], 128);
    assert_eq!(v["run"]["limit_reached"], true);
    assert_eq!(v["run"]["completion"], "inconclusive");
    let old_trials: usize = hand::rows(&v["mutations"])
        .iter()
        .filter(|r| !["command_resent", "receipt_unknown"].contains(&r["kind"].as_str().unwrap()))
        .flat_map(|r| hand::rows(&r["mutants"]).iter())
        .filter(|m| !m["report"].is_null())
        .count();
    assert_eq!(old_trials, 2);
    let resent = hand::rows(&hand::row(&v["mutations"], "kind", "command_resent")["mutants"]);
    assert_eq!(resent.len(), 129);
    assert_eq!(
        resent.iter().filter(|m| !m["report"].is_null()).count(),
        126
    );
    assert_eq!(
        resent.iter().filter(|m| m["result"] == "not_run").count(),
        3
    );
    let unknown = hand::rows(&hand::row(&v["mutations"], "kind", "receipt_unknown")["mutants"]);
    assert_eq!(unknown.len(), 129);
    assert!(unknown.iter().all(|m| m["result"] == "not_run"));
}

#[test]
fn probability_hand_checker_rejects_independent_product_and_wrong_receipt_mass() {
    let v = json!({"properties":[{"id":"probability reached #1","probability":{"min":"1/10","max":"1/10"}}]});
    hand::probability(&v, "probability reached #1", "1/10", "1/10");
    let mut broken = v.clone();
    broken["properties"][0]["probability"] = json!({"min":"1/100","max":"1/100"});
    assert!(
        std::panic::catch_unwind(|| hand::probability(
            &broken,
            "probability reached #1",
            "1/10",
            "1/10"
        ))
        .is_err()
    );
    let mut receipt = v;
    receipt["properties"][0]["probability"] = json!({"min":"1/2","max":"1/2"});
    assert!(
        std::panic::catch_unwind(|| hand::probability(
            &receipt,
            "probability reached #1",
            "1/100",
            "1/100"
        ))
        .is_err()
    );
}

#[test]
fn hand_unknown_checker_rejects_single_field_lies_missing_null_and_wrong_kind() {
    let witness = json!({"terminals":[],"bounds":{"truncated":false},"diagnostics":[],
        "properties":[{"id":"receipt_resolution","verdict":"inconclusive"}],
        "case_outcomes":[{"terminals":[{"kind":"receipt_unknown","end":null,"completed_at":null,
        "elapsed":null,"work":null,"observed_until":{"min":1,"max":1},
        "observed_elapsed":{"min":1,"max":1},"observed_work":{"min":1,"max":1}}]}]});
    hand::unknown(&witness, 1, 1);
    for (pointer, lie) in [
        ("/case_outcomes/0/terminals/0/kind", json!("end")),
        (
            "/case_outcomes/0/terminals/0/completed_at",
            json!({"min":1,"max":1}),
        ),
        ("/bounds/truncated", json!(true)),
        ("/case_outcomes/0/terminals/0/observed_work/min", json!(0)),
    ] {
        let mut bad = witness.clone();
        *bad.pointer_mut(pointer).unwrap() = lie;
        assert!(
            std::panic::catch_unwind(|| hand::unknown(&bad, 1, 1)).is_err(),
            "{pointer}"
        );
    }
    let mut bad = witness;
    bad["case_outcomes"][0]["terminals"][0]
        .as_object_mut()
        .unwrap()
        .remove("completed_at");
    assert!(std::panic::catch_unwind(|| hand::unknown(&bad, 1, 1)).is_err());
}
