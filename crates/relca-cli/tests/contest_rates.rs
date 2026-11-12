//! ADR0063 actual stdout/stderr/exit. Match JSON paths, never whole-output bytes.
#[path = "../../relca-core/tests/support/q9_oracle.rs"]
mod hand;
#[path = "../../relca-core/tests/support/q9_inputs.rs"]
mod input;
use serde_json::Value;
use std::{path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn diagnostics(v: &Value, prefix: &str) -> String {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{prefix}{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect()
}
fn invoke_source(name: &str, mode: &str, src: &str, args: &[&str]) -> (Value, i32) {
    let dir = root()
        .join(".tmp/q9-cli")
        .join(format!("{}-{name}-{mode}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(mode)
        .arg(path)
        .args(args)
        .current_dir(root())
        .env("CARGO_NET_OFFLINE", "true")
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert_eq!(
        out.stdout.iter().filter(|b| **b == b'\n').count(),
        1,
        "one JSON document"
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let expected_stderr = match mode {
        "compare" => {
            let mut text = String::new();
            for p in v["plans"].as_array().unwrap() {
                text.push_str(&diagnostics(
                    &p["report"],
                    &format!("configuration {}: ", p["configuration"].as_str().unwrap()),
                ));
            }
            text.push_str(&diagnostics(&v, "compare: "));
            text
        }
        "diff" => {
            let mut text = diagnostics(&v["from"]["report"], "from: ");
            text.push_str(&diagnostics(&v["to"]["report"], "to: "));
            text.push_str(&diagnostics(&v, "diff: "));
            text
        }
        _ => diagnostics(&v, ""),
    };
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        expected_stderr,
        "diagnostic stderr must mirror the JSON paths"
    );
    let exit = out.status.code().expect("normal process exit");
    assert_eq!(
        exit,
        hand::exit(&v),
        "exit agrees with classification for {name}/{mode}"
    );
    (v, exit)
}
fn invoke(name: &str, mode: &str) -> (Value, i32) {
    invoke_source(name, mode, &input::source(name), &[])
}
fn sim(name: &str) {
    let (v, _) = invoke(name, "simulate");
    hand::validate_sim(name, &v)
        .unwrap_or_else(|e| panic!("{name}: {e}; diagnostics={}", v["diagnostics"]));
}
macro_rules! run {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            sim($name);
        }
    };
}
run!(c_bernoulli_and_post_end_route, "c");
run!(
    a1_changed_probability_changes_samples_not_business_end,
    "half"
);
run!(a2_on_time_equal_deadline, "deadline");
run!(a2_late_witness, "late");
run!(a6_basis_cannot_clear_cut, "defer");
run!(a6_certain_rate_cannot_clear_cut, "defer-fixed");
run!(a7_clock_is_unresolved_not_success, "clock");
run!(a8_zero_capacity_is_deadlock_not_response, "deadlock");
run!(a9_spaced_arrivals_replication_unit, "two-spaced");
run!(a9_burst_queue_not_service, "two-burst");
run!(a10_calendar_wait, "calendar");
run!(a10_later_calendar_wait, "calendar-later");
run!(a11_conditional_target_denominator, "conditional-target");
run!(a11_synthetic_site_is_not_clause_ordinal, "ordinal");
run!(a12_same_tick_choice_is_not_uniform, "same-tick");
run!(a12_two_clauses_keep_dispatch_nondeterminism, "two-clauses");
run!(a14_no_metrics_no_new_table, "no-metrics");
run!(
    a15_mixed_rate_and_unconfigured_refuses_on_encounter,
    "mixed"
);
run!(
    a15_unreached_unconfigured_does_not_block_known_rate,
    "mixed-unreached"
);
run!(a15_route_result_choice_still_refuses, "route-nondet");
run!(a15_normal_call_is_not_route_response, "normal-call");

macro_rules! invalid {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            let (_, code, reason) = input::STATIC.iter().find(|r| r.0 == $name).unwrap();
            for mode in ["check", "simulate"] {
                let (v, exit) = invoke($name, mode);
                hand::diagnostic(&v, code, reason).unwrap();
                assert_eq!(exit, if code.starts_with("E-") { 3 } else { 4 });
            }
        }
    };
}
invalid!(a3_zero_probability_invalid_not_silent_cut, "zero");
invalid!(a4_above_one, "above-one");
invalid!(a4_occurrence_required, "missing-occurrence");
invalid!(a4_occurrence_basis_required, "missing-occurrence-basis");
invalid!(a4_occurrence_basis_nonblank, "empty-occurrence-basis");
invalid!(a4_service_basis_required, "missing-service-basis");
invalid!(a4_service_basis_nonempty, "empty-service-basis");
invalid!(a4_independence_required, "missing-independence");
invalid!(a4_actor_is_not_independence, "unknown-independence");
invalid!(a4_duplicate_rates_not_last_wins, "duplicate-rates");
invalid!(a4_empty_rates_no_default, "empty-rates");

#[test]
fn c_a1_a3_check_uses_positive_support_and_keeps_static_authority() {
    for (name, want) in [
        ("c", "violated"),
        ("half", "violated"),
        ("tiny", "violated"),
        ("tiny-no-authority", "violated"),
        ("deadline", "satisfied"),
        ("late", "violated"),
    ] {
        let (v, _) = invoke(name, "check");
        assert_eq!(v["verdict"], want, "{name}: {}", v["diagnostics"]);
        hand::assumptions(&v, 1).unwrap();
        assert_eq!(
            hand::property(&v, " #1 contest")["verdict"],
            if name == "tiny-no-authority" {
                "violated"
            } else {
                "satisfied"
            }
        );
        if name != "tiny-no-authority" {
            assert_eq!(hand::property(&v, " #1 contest_response")["verdict"], want);
        }
    }
}

#[test]
fn green_legacy_no_rates_json_paths_and_nonzero_simulation_exit_stay_fixed() {
    let (c, exit) = invoke("legacy-fixed", "check");
    assert_eq!(exit, 0);
    assert_eq!(
        hand::property(&c, " #1 contest_response")["verdict"],
        "satisfied"
    );
    assert!(c.pointer("/metrics/contests").is_none());
    hand::subset(
        &hand::range("1", "3"),
        &c["metrics"]["overall"]["work"],
        "legacy/work",
    )
    .unwrap();
    hand::subset(
        &hand::range("0", "22"),
        &c["metrics"]["overall"]["observed_cost_total"],
        "legacy/cost",
    )
    .unwrap();
    let (s, exit) = invoke("legacy-fixed", "simulate");
    assert_eq!(exit, 4);
    assert_eq!(s["simulation"]["status"], "unsupported");
    assert_eq!(hand::property(&s, " #1 contest")["verdict"], "satisfied");
    assert!(s.pointer("/metrics/contests").is_none());
    let (u, exit) = invoke("legacy-unreached", "simulate");
    assert_eq!(exit, 2);
    assert_eq!(u["simulation"]["completed_replications"], 16);
}

#[test]
fn green_invoice_defer_and_not_configured_survive_without_output_byte_freeze() {
    let src = include_str!("../../../examples/invoice/to-be-a-phase1.relca");
    let (v, exit) = invoke_source("invoice", "check", src, &[]);
    assert_eq!(exit, 1);
    assert!(src.contains("defer Vendor.contest_rate") && src.contains("defer Resubmission"));
    for k in ["work", "observed_cost_total"] {
        let q = &v["metrics"]["overall"][k];
        assert_eq!(q["reason"], "not_configured");
        assert!(q["min"].is_null() && q["max"].is_null());
    }
    assert!(v.pointer("/metrics/contests").is_none());
}

#[test]
fn green_independent_wire_fault_controls_are_effective() {
    hand::validate_table(&hand::expected_table("fixed")).unwrap();
    for (description, bad) in hand::wire_faults() {
        assert!(hand::validate_table(&bad).is_err(), "{description}");
    }
}

#[test]
fn a14_compare_and_diff_transport_child_tables_without_new_objectives() {
    let s = input::edit(
        &input::source("fixed"),
        "  process Flow",
        "  configuration A { cut Scope; process Flow }\n  configuration B { cut Scope; process FlowB }\n  process FlowB { d = Judge.decide(); if d == fail { end returned } else { end posted } }\n  process Flow",
    );
    let (c, exit) = invoke_source(
        "compare",
        "compare",
        &s,
        &["--plans", "A,B", "--mode", "simulation"],
    );
    assert_eq!(exit, 2, "compare diagnostics: {}", c["plans"]);
    for row in c["plans"].as_array().unwrap() {
        hand::validate_sim("fixed", &row["report"]).unwrap();
    }
    // diff is exhaustive: propagate existing contest structure and explicitly
    // refrain from manufacturing simulation sample tables or numeric differences.
    let (d, exit) = invoke_source("diff", "diff", &s, &["--from", "A", "--to", "B"]);
    assert_eq!(exit, 0);
    for side in ["from", "to"] {
        assert_eq!(
            hand::property(&d[side]["report"], " #1 contest_response")["verdict"],
            "satisfied"
        );
        assert!(d[side]["report"].pointer("/metrics/contests").is_none());
    }
}
