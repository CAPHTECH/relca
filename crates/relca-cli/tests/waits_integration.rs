//! Real binary and closed fixture matcher coverage for integrated A7 obligations.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/waits_integration_inputs.rs"]
mod i;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::Value;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn cli(s: &str, command: &str, cfg: Option<&str>, day: bool) -> Value {
    let mut args = vec![command];
    if let Some(cfg) = cfg {
        args.extend(["--configuration", cfg]);
    }
    if day {
        args.extend(["--scenario", "Day"]);
    }
    invoke(s, &args).0
}
fn invoke(s: &str, args: &[&str]) -> (Value, std::process::Output) {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let path = test_tmp::root().join(format!(
        "a7int-{}-{}.relca",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, s).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_relca"));
    c.arg(args[0]).arg(&path).args(&args[1..]);
    let out = c.output().unwrap();
    std::fs::remove_file(path).unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let diagnostics = |rows: &Value, prefix: &str| -> String {
        rows.as_array()
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
    };
    let stderr = if args[0] == "diff" {
        diagnostics(&v["from"]["report"]["diagnostics"], "from: ")
            + &diagnostics(&v["to"]["report"]["diagnostics"], "to: ")
            + &diagnostics(&v["diagnostics"], "diff: ")
    } else {
        diagnostics(&v["diagnostics"], "")
    };
    assert_eq!(
        String::from_utf8(out.stderr.clone()).unwrap(),
        stderr,
        "closed ordered stderr diagnostics"
    );
    let verdict: relca_core::Verdict = serde_json::from_value(v["verdict"].clone()).unwrap();
    assert_eq!(out.status.code(), Some(verdict.exit_code()));
    (v, out)
}
#[test]
fn b6_horizon_and_clock_keep_quality_separate_from_observed_waits_in_cli() {
    for clock in [false, true] {
        let s = i::hq(clock, false);
        let a = cli(&s, "check", None, true);
        let b = cli(&i::masked(&s), "check", None, true);
        assert_eq!(a["quality"], b["quality"]);
        assert_eq!(
            i::ratio(&a)["value"]["reason"],
            if clock { "truncated" } else { "undetermined" }
        );
        assert_eq!(
            a["metrics"]["status"],
            if clock { "truncated" } else { "complete" }
        );
        if !clock {
            assert_eq!(
                a["metrics"]["actors"][0]["queue"]["wait_total"]["stats"],
                i::range("3")
            );
        }
    }
}
#[test]
fn f36_exhaustive_cli_keeps_four_quality_rows_before_waiting() {
    i::assumption_order(&cli(&i::hq(false, false), "check", None, true), false);
}
#[test]
fn f36_sample_cli_keeps_four_quality_rows_before_waiting() {
    i::assumption_order(&cli(&i::hq(false, true), "simulate", None, true), true);
}
#[test]
fn f36_early_refusal_cli_keeps_waiting_after_quality() {
    let a = cli(&i::refusal(), "simulate", None, true);
    assert_eq!(a["verdict"], "unsupported");
    i::assumption_order(&a, true);
}

fn missing_simulation_clause_cli(day: bool) {
    let s = if day {
        i::hq(false, false)
    } else {
        i::quality(&i::fixture("wait-intentional"))
    };
    let a = cli(&s, "simulate", None, day);
    assert_eq!(a["verdict"], "invalid");
    i::missing_simulation_disclosure(&a);
}
#[test]
fn f36_missing_simulation_clause_cli_without_scenario_has_no_waiting() {
    missing_simulation_clause_cli(false);
}
#[test]
fn f36_missing_simulation_clause_cli_with_scenario_has_no_waiting() {
    missing_simulation_clause_cli(true);
}
fn combined_quality_and_waiting_wire_guard(sim: bool) {
    let s = i::hq(false, sim);
    let a = cli(&s, if sim { "simulate" } else { "check" }, None, true);
    let checked = if sim {
        fixture_match::validate_simulation_wire(&a)
    } else {
        fixture_match::validate_business_metrics_wire(&a)
    };
    assert!(checked.is_ok(), "{checked:?}");
    let mut bad = a;
    bad["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["field"] != "quality.form");
    let checked = if sim {
        fixture_match::validate_simulation_wire(&bad)
    } else {
        fixture_match::validate_business_metrics_wire(&bad)
    };
    assert!(
        checked.is_err(),
        "missing quality row is not excused by waiting projection"
    );
}
#[test]
fn combined_quality_and_waiting_exhaustive_wire_keeps_old_guards() {
    combined_quality_and_waiting_wire_guard(false);
}
#[test]
fn combined_quality_and_waiting_sample_wire_keeps_old_guards() {
    combined_quality_and_waiting_wire_guard(true);
}
#[test]
fn rule_check_configurations_keep_hand_partition_and_authority_disclosure() {
    for (cfg, lead, wait) in [("Before", "10", "2"), ("After", "8", "0")] {
        let s = i::rule();
        let a = cli(&s, "check", Some(cfg), false);
        let b = cli(&i::masked(&s), "check", Some(cfg), false);
        assert_eq!(a["verdict"], "satisfied");
        assert_eq!(i::disclosure(&a), i::disclosure(&b));
        assert_eq!(
            a["metrics"]["cases"][0]["terminals"][0]["lead_time"],
            i::range(lead)
        );
        assert_eq!(
            a["metrics"]["cases"][0]["terminals"][0]["breakdown"]["intentional"],
            i::range(wait)
        );
        assert_eq!(
            a["metrics"]["cases"][0]["terminals"][0]["breakdown"]["in_service"],
            i::range("8")
        );
        assert_eq!(
            a["metrics"]["overall"]["queue_wait_observed"],
            i::range("0")
        );
    }
}
#[test]
fn record_conflict_and_c5_paths_keep_hand_service_partitions_in_real_cli() {
    for (name, ends) in [
        (
            "rec-approval",
            vec![("posted", "4", "7"), ("returned", "3", "6")],
        ),
        (
            "agy2-jue",
            vec![("posted", "3", "3"), ("returned", "2", "2")],
        ),
        (
            "cf-version-atomic",
            vec![("done", "1", "1"), ("conflict_end", "1", "1")],
        ),
    ] {
        let s = i::crossing(name);
        let a = cli(&s, "check", None, false);
        for (end, min, max) in ends {
            i::partition_equals_service(&a["metrics"], end, min, max);
        }
        let b = cli(&i::masked(&s), "check", None, false);
        assert_eq!(i::disclosure(&a), i::disclosure(&b));
        fixture_match::validate_business_metrics_wire(&a).unwrap();
    }
}

fn a7fix_revocation_cli(quality_on: bool, rule_on: bool, revocable: bool, waits: bool) {
    let s = i::revocation(quality_on, rule_on, revocable);
    let s = if waits { s } else { i::masked(&s) };
    let v = cli(&s, "simulate", None, false);
    i::refusal_status(&v, "invalid", "E-SIM-0", "explicit simulation clause");
    i::legacy_revocation(&v, revocable);
    if waits {
        let control = cli(&i::masked(&s), "simulate", None, false);
        i::legacy_refusal_equal(&v, &control);
    }
    i::no_metric_disclosure(&v, true, quality_on, rule_on);
}
macro_rules! revocation_cases {
    ($on:ident, $off:ident, $q:expr, $rule:expr, $rev:expr) => {
        #[test]
        fn $on() {
            a7fix_revocation_cli($q, $rule, $rev, true);
        }
        #[test]
        fn $off() {
            a7fix_revocation_cli($q, $rule, $rev, false);
        }
    };
}
revocation_cases!(
    a7fix_revocable_no_quality_waits_cli,
    a7fix_revocable_no_quality_control_cli,
    false,
    false,
    true
);
revocation_cases!(
    a7fix_revocable_quality_waits_cli,
    a7fix_revocable_quality_control_cli,
    true,
    false,
    true
);
revocation_cases!(
    a7fix_revocable_rule_quality_waits_cli,
    a7fix_revocable_rule_quality_control_cli,
    true,
    true,
    true
);
revocation_cases!(
    a7fix_no_revoker_no_quality_waits_cli,
    a7fix_no_revoker_no_quality_control_cli,
    false,
    false,
    false
);
revocation_cases!(
    a7fix_no_revoker_quality_waits_cli,
    a7fix_no_revoker_quality_control_cli,
    true,
    false,
    false
);
revocation_cases!(
    a7fix_no_revoker_rule_quality_waits_cli,
    a7fix_no_revoker_rule_quality_control_cli,
    true,
    true,
    false
);

fn a7fix_completion_cli(question: &str, rule_on: bool, waits: bool) {
    let s = i::completion(question, rule_on);
    let s = if waits { s } else { i::masked(&s) };
    let v = cli(&s, "simulate", None, true);
    i::refusal_status(&v, "unsupported", "U-SEM-0", "structural completion");
    i::no_metric_disclosure(&v, true, true, rule_on);
}
macro_rules! completion_cases {
    ($on:ident, $off:ident, $question:expr, $rule:expr) => {
        #[test]
        fn $on() {
            a7fix_completion_cli($question, $rule, true);
        }
        #[test]
        fn $off() {
            a7fix_completion_cli($question, $rule, false);
        }
    };
}
completion_cases!(
    a7fix_terminating_waits_cli,
    a7fix_terminating_control_cli,
    "terminating",
    false
);
completion_cases!(
    a7fix_recoverable_waits_cli,
    a7fix_recoverable_control_cli,
    "recoverable",
    false
);
completion_cases!(
    a7fix_almost_sure_waits_cli,
    a7fix_almost_sure_control_cli,
    "almost_sure",
    false
);
completion_cases!(
    a7fix_terminating_rule_waits_cli,
    a7fix_terminating_rule_control_cli,
    "terminating",
    true
);

fn a7fix_rule_order_cli(sim: bool) {
    for cfg in ["Before", "After"] {
        let s = i::rule_quality(sim);
        for waits in [false, true] {
            let s = if waits { s.clone() } else { i::masked(&s) };
            let v = cli(&s, if sim { "simulate" } else { "check" }, Some(cfg), false);
            assert!(v.get("metrics").is_some());
            i::completed_rule_quality(&v, sim);
            i::quality_block(&v, sim, true);
            if waits {
                i::assumption_order(&v, sim);
            } else {
                assert!(!i::fields(&v).contains(&"metrics.waiting"));
            }
        }
    }
}
#[test]
fn a7fix_rule_quality_waits_exhaustive_order_cli() {
    a7fix_rule_order_cli(false);
}
#[test]
fn a7fix_rule_quality_waits_simulation_order_cli() {
    a7fix_rule_order_cli(true);
}

#[test]
fn a7fix_diff_unchecked_literal_inventory_cli() {
    for contest in [false, true] {
        for mismatch in [false, true] {
            let s = i::diff_input(contest, mismatch);
            for waits in [false, true] {
                let s = if waits { s.clone() } else { i::masked(&s) };
                let (v, _) = invoke(&s, &["diff", "--from", "Left", "--to", "Right"]);
                assert_eq!(v["verdict"], if mismatch { "invalid" } else { "satisfied" });
                assert_eq!(
                    v["unchecked"],
                    i::diff_unchecked(contest, mismatch, waits),
                    "contest={contest} mismatch={mismatch} waits={waits}"
                );
            }
        }
    }
}

#[test]
fn a7fix_diff_aborted_legacy_unchecked_places_waiting_before_terminals_cli() {
    let s = i::diff_aborted_input();
    for waits in [false, true] {
        let s = if waits { s.clone() } else { i::masked(&s) };
        let (v, _) = invoke(&s, &["diff", "--from", "Left", "--to", "Right"]);
        assert_eq!(v["verdict"], "unsupported");
        for side in ["from", "to"] {
            assert_eq!(v[side]["report"]["metrics"]["status"], "aborted");
            assert!(v[side]["report"]["bounds"]["truncated"].is_null());
            assert!(
                v[side]["report"]["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "U-SEM-0")
            );
        }
        assert_eq!(v["unchecked"], i::diff_aborted_unchecked(waits));
    }
}

#[test]
fn a7fix_frontend_refusals_never_publish_quality_or_waiting_cli() {
    for sim in [false, true] {
        let s = i::hq(false, sim);
        for waits in [false, true] {
            let s = if waits { s.clone() } else { i::masked(&s) };
            for parse_error in [false, true] {
                let bad = if parse_error {
                    format!("{s}\n)")
                } else {
                    i::quality(&s)
                };
                let v = cli(&bad, if sim { "simulate" } else { "check" }, None, true);
                assert!(
                    v["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d["severity"] == "error")
                );
                assert!(v.get("quality").is_none());
                i::no_metric_disclosure(&v, sim, false, false);
            }
        }
    }
}

#[test]
fn a7fix_metrics_undeclared_normal_and_runtime_refusal_have_no_waiting_cli() {
    for (sim, refuse) in [(false, false), (true, false), (true, true)] {
        let s = if refuse {
            i::refusal()
        } else {
            i::hq(false, sim)
        };
        let s = s.replace(
            "metrics { window 20min; currency \"JPY\"; breakdown waits }",
            "",
        );
        assert!(!s.contains("metrics {"));
        let v = cli(&s, if sim { "simulate" } else { "check" }, None, true);
        i::no_metric_disclosure(&v, sim, true, false);
    }
}

#[test]
fn a7fix_structural_refusal_must_not_leave_waiting_before_quality_cli() {
    let v = cli(&i::completion("terminating", false), "simulate", None, true);
    i::refusal_status(&v, "unsupported", "U-SEM-0", "structural completion");
    if i::fields(&v).contains(&"metrics.waiting") {
        i::assumption_order(&v, true);
    }
    i::no_metric_disclosure(&v, true, true, false);
}

#[test]
fn a7fix_selected_configuration_frontend_returns_have_no_quality_or_waiting_cli() {
    for sim in [false, true] {
        let s = i::rule_quality(sim);
        for waits in [false, true] {
            let s = if waits { s.clone() } else { i::masked(&s) };
            for parse_error in [false, true] {
                let s = if parse_error {
                    format!("{s}\n)")
                } else {
                    i::quality(&s)
                };
                let v = cli(
                    &s,
                    if sim { "simulate" } else { "check" },
                    Some("Before"),
                    false,
                );
                assert!(
                    v["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d["severity"] == "error")
                );
                assert!(v.get("quality").is_none());
                i::no_metric_disclosure(&v, sim, false, false);
            }
        }
    }
}

#[test]
fn a7fix_input_read_failure_cli_has_no_metrics_quality_or_waiting() {
    let path = test_tmp::root().join(format!("a7fix-no-such-input-{}.relca", std::process::id()));
    assert!(!path.exists(), "test must exercise a real read failure");
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("simulate")
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["model"]["sha256"], Value::Null);
    assert_eq!(v["diagnostics"][0]["code"], "E-SYN-0");
    assert!(!out.stderr.is_empty());
    i::no_metric_disclosure(&v, true, false, false);
}

#[test]
fn a7fix_missing_clause_rejected_lowering_copies_no_rows_cli() {
    let s = i::quality(&i::revocation(true, true, true));
    for waits in [false, true] {
        let s = if waits { s.clone() } else { i::masked(&s) };
        let v = cli(&s, "simulate", None, false);
        i::refusal_status(&v, "invalid", "E-SIM-0", "explicit simulation clause");
        assert!(v.get("quality").is_none());
        assert!(i::fields(&v).is_empty());
        i::no_metric_disclosure(&v, true, false, false);
    }
}

#[test]
fn a7fix_missing_clause_scenario_keeps_rule_after_quality_without_waiting_cli() {
    let s = i::missing_clause_rule_scenario();
    for waits in [false, true] {
        let s = if waits { s.clone() } else { i::masked(&s) };
        let v = cli(&s, "simulate", None, true);
        i::refusal_status(&v, "invalid", "E-SIM-0", "explicit simulation clause");
        i::no_metric_disclosure(&v, true, true, true);
    }
}
