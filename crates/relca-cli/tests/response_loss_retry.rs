//! Actual stdout/stderr/exit, not a call to the core report as a substitute.
#[path = "../../relca-core/tests/support/response_loss_oracle.rs"]
mod hand;
#[path = "../../relca-core/tests/support/response_loss_inputs.rs"]
mod input;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::{Value, json};
#[path = "support/ledger_shape.rs"]
mod ledger_shape;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
fn run(command: &str, name: &str, source: &str, extra: &[&str]) -> (Value, i32, Vec<u8>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let tmp = test_tmp::root();
    let dir = tmp.join(format!("q1t-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!(
        "{name}-{}.relca",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&file, source).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .current_dir(root)
        .arg(command)
        .arg(file)
        .args(extra)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap();
    assert!(o.stdout.ends_with(b"\n"), "complete JSON stdout line");
    let v: Value = serde_json::from_slice(&o.stdout).expect("CLI JSON stdout");
    (
        v,
        o.status.code().expect("ordinary exit, not signal"),
        o.stderr,
    )
}
fn live(name: &str) {
    let (v, code, _) = run("check", name, &input::source(name), &[]);
    hand::check(&v, name);
    let expected = match v["verdict"].as_str().unwrap() {
        "satisfied" => 0,
        "violated" => 1,
        "inconclusive" => 2,
        other => panic!("unexpected {other}"),
    };
    assert_eq!(code, expected);
    // Deserialization must preserve new required nulls, not discard the receipt table.
    let typed: relca_core::report::Report = serde_json::from_value(v.clone()).unwrap();
    let round = json!(typed);
    assert_eq!(round.get("response_receipts"), v.get("response_receipts"));
    assert_eq!(round.get("case_outcomes"), v.get("case_outcomes"));
}
macro_rules! runtime {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            live($name);
        }
    };
}
runtime!(cli_v0109_g_resend, "g-resend");
runtime!(cli_v0109_concrete_duplicate, "modeled-resend");
runtime!(cli_v0109_reliable_control, "modeled-first-delivered");
runtime!(cli_v0112_correlated_one_tenth, "correlated");
runtime!(cli_v0112_broken_tight_upper, "correlated-tight");
runtime!(cli_v0112_backoff_not_work, "backoff");
runtime!(cli_v0112_independent_control, "independent");
runtime!(cli_v0214_g_resend, "k8-g-resend");
runtime!(cli_v0214_g_unknown, "k8-g-unknown");
runtime!(cli_v0214_g_cut, "k8-g-clock");
runtime!(cli_receipt_unknown_exit_two_not_success, "single-unknown");
runtime!(cli_receipt_delivered_control, "single-delivered");
runtime!(cli_nondeterminism_not_uniform, "g-nondet");
runtime!(cli_default_is_nondeterminism, "g-default");
runtime!(cli_probability_basis_and_mass, "g-probability");
runtime!(cli_probability_zero, "g-probability-zero");
runtime!(cli_probability_one, "g-probability-one");
runtime!(cli_arrivals, "g-arrivals");
runtime!(cli_labels, "g-label");
runtime!(cli_one_total_attempt, "max-one");
runtime!(cli_thirty_two_accepted, "max-thirty-two");
runtime!(cli_known_exhaustion, "known-fail");

macro_rules! refusal {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            let (_, c, r) = input::STATIC.iter().find(|(n, _, _)| *n == $name).unwrap();
            let (v, code, stderr) = run("check", $name, &input::source($name), &[]);
            hand::static_failure(&v, c, r);
            assert_eq!(code, if c.starts_with("U-") { 4 } else { 3 });
            assert!(
                String::from_utf8(stderr).unwrap().contains(c),
                "diagnostic must reach stderr"
            );
        }
    };
}
refusal!(cli_cut_r_is_unsupported_not_only_annotated, "r-resend");
refusal!(cli_r_cannot_remove_once_to_evade_refusal, "r-without-once");
refusal!(cli_k8_r_resend, "k8-r-resend");
refusal!(cli_missing_effect_scope, "modeled-missing-scope");
refusal!(
    cli_idempotency_without_mechanism,
    "idempotency-without-atomic"
);
refusal!(cli_idempotency_wrong_set, "idempotency-wrong-set");
refusal!(cli_max_zero_invalid, "max-zero");
refusal!(cli_max_thirty_three_unsupported, "max-thirty-three");
refusal!(cli_sequence_short, "sequence-short");
refusal!(cli_sequence_extra, "sequence-extra");
refusal!(cli_missing_exhaustion, "missing-exhausted");
refusal!(cli_missing_backoff, "missing-backoff");
refusal!(cli_missing_probability_basis, "missing-basis");
refusal!(cli_empty_probability_basis, "empty-basis");
refusal!(cli_probability_out_of_range, "invalid-probability");
refusal!(cli_fork_crossing, "fork-retry");
refusal!(cli_version_crossing, "version-retry");

#[test]
fn cli_failure_free_k8_controls_preserve_existing_values() {
    for (name, s) in [
        (
            "k8-r",
            include_str!("../../../fixtures/k8-cut-r/model.relca"),
        ),
        (
            "k8-g",
            include_str!("../../../fixtures/k8-cut-g/model.relca"),
        ),
    ] {
        let (v, code, _) = run("check", name, s, &[]);
        assert_eq!(code, 0);
        hand::verdict(&v, "satisfied");
        hand::terminal(&v, "done", 2, 2);
        hand::probability(&v, "probability reached #1", "1/50", "1/50");
    }
}

#[test]
fn cli_old_syntax_substrate_controls_are_green() {
    for name in ["g-resend", "r-resend", "modeled-resend", "correlated"] {
        let (v, code, _) = run("check", name, &input::substrate(name), &[]);
        assert_eq!(code, 0);
        hand::verdict(&v, "satisfied");
    }
}

#[test]
fn cli_quality_metrics_unknown_is_unfinished_not_completed_with_cost_preserved() {
    for unknown in [true, false] {
        let (v, code, _) = run("check", "indicators", &input::indicators(unknown), &[]);
        hand::indicators(&v, unknown);
        assert_eq!(code, if unknown { 2 } else { 0 });
    }
}

#[test]
fn cli_explain_preserves_check_and_requires_verified_duplicate_replay() {
    let s = input::source("modeled-resend");
    let (check, code, _) = run("check", "duplicate", &s, &[]);
    let (mut explain, xcode, _) = run("explain", "duplicate", &s, &[]);
    assert_eq!(code, 1);
    assert_eq!(xcode, code);
    let x = explain
        .as_object_mut()
        .unwrap()
        .remove("explanation")
        .unwrap();
    assert_eq!(
        ledger_shape::original_report(&check),
        ledger_shape::original_report(&explain)
    );
    assert_eq!(
        hand::row(&x["counterexamples"], "id", "contract C #1 at_most_once")["status"],
        "verified"
    );
}

#[test]
fn cli_mutation_two_operators_and_r_invalid_not_detected() {
    for (kind, result, reason) in [
        ("g", "undetected", Some("no_worsening")),
        ("modeled", "detected", None),
        ("r", "invalid_mutant", Some("static_unsupported")),
    ] {
        let (v, code, _) = run("mutate", kind, &input::mutation_source(kind), &[]);
        assert_eq!(code, 4); // stale_definition_write remains unsupported; not a detection oracle
        hand::mutation(&v, "command_resent", result, reason);
        hand::mutation(&v, "receipt_unknown", "detected", None);
        if kind == "r" {
            let m = &hand::row(&v["mutations"], "kind", "command_resent")["mutants"][0];
            hand::static_failure(
                &m["report"],
                "U-SEM-0",
                "response_loss_resend_mechanism_unmodeled",
            );
        }
    }
}

#[test]
fn cli_mutation_reports_skipped_counts_by_reason() {
    let (v, code, _) = run("mutate", "skipped", &input::skipped_source(), &[]);
    assert_eq!(code, 4);
    for kind in ["command_resent", "receipt_unknown"] {
        let r = hand::row(&v["mutations"], "kind", kind);
        assert_eq!(
            r["skipped_counts"],
            json!({"undeclared_site":1,"existing_response_loss":1,"existing_retry":1,"fork_branch":1})
        );
        assert_eq!(hand::rows(&r["mutants"]).len(), 1);
    }
}

#[test]
fn cli_simulate_refuses_nondeterminism_and_comparisons_preserve_mismatch_and_violation() {
    let (v, code, _) = run(
        "simulate",
        "nondet",
        &input::simulation_source("g-nondet"),
        &[],
    );
    assert_eq!(code, 4);
    hand::verdict(&v, "unsupported");
    assert!(v.to_string().contains("U-SEM-0"));
    for (command, args) in [
        ("compare", vec!["--plans", "A,B", "--mode", "exhaustive"]),
        ("diff", vec!["--from", "A", "--to", "B"]),
    ] {
        let (v, code, _) = run(
            command,
            "crossing",
            &input::comparison_source("g-resend"),
            &args,
        );
        // P3a accepts Q1; this old pair still has unlike lexical Ends.
        // compare refuses that mismatch, whereas diff preserves B's query violation.
        assert_eq!(code, if command == "compare" { 3 } else { 1 });
        hand::verdict(
            &v,
            if command == "compare" {
                "invalid"
            } else {
                "violated"
            },
        );
        let source = if command == "compare" {
            &v["plans"][0]["report"]
        } else {
            &v["from"]["report"]
        };
        assert_eq!(hand::rows(&source["response_receipts"]).len(), 2);
    }
}
