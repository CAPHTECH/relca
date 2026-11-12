//! Real CLI stdout/stderr/exit, checked against independent hand sets, not snapshots.
#[path = "../../relca-core/tests/support/q6b_oracle.rs"]
mod hand;
#[path = "../../relca-core/tests/support/q6b_inputs.rs"]
mod input;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static INVOCATION: AtomicU64 = AtomicU64::new(0);

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
    let dir = root().join(".tmp/q6b-cli").join(format!(
        "{}-{}-{name}-{mode}",
        std::process::id(),
        INVOCATION.fetch_add(1, Ordering::Relaxed)
    ));
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
        out.stdout.iter().filter(|x| **x == b'\n').count(),
        1,
        "one JSON document"
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let exit = out.status.code().expect("normal exit");
    assert_eq!(
        exit,
        hand::exit(&v),
        "{name}/{mode}: classification and exit agree"
    );
    let stderr = match mode {
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
        stderr,
        "diagnostic stderr mirrors JSON"
    );
    (v, exit)
}
fn invoke(name: &str, mode: &str) -> (Value, i32) {
    let args = if let Some(c) = input::configuration(name) {
        vec!["--configuration", c]
    } else {
        vec![]
    };
    invoke_source(name, mode, &input::source(name), &args)
}

#[test]
fn green_a9_keep_only_and_single_intent_removal_keep_unknown_dependency() {
    for (name, kept) in [("keep", true), ("no-keep", false)] {
        let (v, exit) = invoke(name, "check");
        assert_eq!(exit, 0);
        hand::legacy(&v, kept).unwrap();
    }
}
#[test]
fn green_wire_single_faults_reject_without_production_arithmetic() {
    hand::validate_table(&hand::expected("a8")).unwrap();
    for (label, bad) in hand::wire_faults() {
        assert!(hand::validate_table(&bad).is_err(), "{label}");
    }
}
#[test]
fn green_old_if_check_and_simulate_preserve_nondeterministic_boundary() {
    let src = input::substrate("a7");
    let (v, exit) = invoke_source("old-if", "check", &src, &[]);
    assert_eq!(exit, 0);
    hand::validate_times("a7", &v).unwrap();
    assert!(v["agency"].get("degradation").is_none());
    let (s, exit) = invoke_source("old-if", "simulate", &src, &[]);
    assert_eq!(exit, 4);
    hand::diagnostic(&s, "U-SEM-0", "").unwrap();
}

macro_rules! table {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            let (v, _) = invoke($name, "check");
            hand::validate_report($name, &v)
                .unwrap_or_else(|d| panic!("{d}; diagnostics={}", v["diagnostics"]));
            if !matches!($name, "clock" | "defer") {
                hand::validate_times($name, &v).unwrap();
            }
        }
    };
}
table!(a7_clerk_dependency_and_direct_doc, "a7");
table!(a7_one_changed_call_removes_doc_and_clerk, "other");
table!(a8_m8_bypass_gives_may_not_must, "a8");
table!(a8_m8_clerk_in_both_branches_is_must, "must");
table!(a8_guard_conjunction_and_end_partition, "guarded");
table!(
    a8_one_guard_opens_previously_unreachable_end,
    "guarded-open"
);
table!(i14_suffix_is_not_double_counted_as_prefix, "suffix");
table!(a11_known_none_retains_branch_metadata, "known-none");
table!(a11_kernel_latent_is_allowed_not_a_policy_read, "latent-ok");
table!(i15_requires_cannot_be_reported_as_start_one, "requires");
table!(i15_state_guard_not_derived_not_zero, "state-guard");
table!(
    i15_calendar_availability_not_promoted_to_guarantee,
    "calendar"
);
table!(
    i15_priority_availability_not_promoted_to_guarantee,
    "priority"
);
table!(
    i15_structural_qualified_permission_sets_are_separate,
    "qualified"
);
table!(i15_m8_keep_does_not_create_manual_dependency, "keep-route");
table!(i15_clock_does_not_truncate_structural_table, "clock");
table!(i15_defer_does_not_remove_structural_table, "defer");

macro_rules! rejection {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            let (_, code, reason) = input::STATIC.iter().find(|r| r.0 == $name).unwrap();
            let (control,_) = invoke("a7","check");
            assert!(!control["diagnostics"].to_string().contains("unsupported syntax: degrade"),"positive degrade grammar must be recognized before a negative syntax classification is evidence");
            for mode in ["check", "simulate", "explain"] {
                let (v, exit) = invoke($name, mode);
                hand::diagnostic(&v, code, reason).unwrap();
                assert_eq!(exit, if code.starts_with("E-") { 3 } else { 4 });
            }
        }
    };
}
rejection!(a11_latent_degrade_rejected_before_execution, "latent");
rejection!(
    a11_unknown_binding_is_not_implicit_failure,
    "unknown-binding"
);
rejection!(
    i12_state_condition_outside_known_binding_is_unsupported,
    "state-condition"
);
rejection!(a11_receipt_unknown_cannot_choose_failure_side, "unknown");
rejection!(i12_retry_intersection_stays_unsupported, "retry");
rejection!(i12_branch_wait_stays_unsupported, "branch-wait");
rejection!(i12_branch_fork_stays_unsupported, "branch-fork");
rejection!(i12_nested_route_stays_unsupported, "nested");
rejection!(
    i12_second_route_capacity_is_not_prefix_execution,
    "two-routes"
);

#[test]
fn i13_simulate_never_invents_uniform_degradation_probability() {
    let (v, exit) = invoke("a7", "simulate");
    assert_eq!(exit, 4);
    hand::diagnostic(&v, "U-SEM-0", "").unwrap();
    assert_eq!(v["simulation"]["completed_replications"], 0);
}

#[test]
fn i13_simulate_known_kernel_has_two_minute_samples_not_an_invented_failure_rate() {
    let (v, _) = invoke("latent-ok", "simulate");
    hand::validate_report("latent-ok", &v).unwrap();
    assert_eq!(v["simulation"]["completed_replications"], 4);
    assert_eq!(
        v["metrics"]["overall"]["work"],
        json!({"mean":"2","min":"2","max":"2","samples":4,"reason":null})
    );
    let cost = &v["metrics"]["overall"]["observed_cost_total"];
    assert_eq!(cost["reason"], "not_configured");
    assert!(cost["mean"].is_null());
}

#[test]
fn i13_explain_replays_real_degraded_branch() {
    let (v, exit) = invoke("explain", "explain");
    assert_eq!(exit, 1, "{}", v["diagnostics"]);
    let rows = v["explanation"]["counterexamples"].as_array().unwrap();
    let row = rows
        .iter()
        .find(|r| r["id"] == "contract C #1 invariant")
        .unwrap();
    assert_eq!(row["replay"]["status"], "verified");
    assert!(row.to_string().contains("Clerk.enter"));
}

// Q6c裁定1・補足4によりi12_compare_diff_not_connected_does_not_silently_run_ifの
// compare/diff未接続期待を撤去。接続後の意味と未導出の保持はq6c_connectの独立手値が検査する。

#[test]
fn i13_effect_contract_is_not_rolled_back_on_degradation() {
    let (v, exit) = invoke("effect", "check");
    assert_eq!(exit, 0, "{}", v["diagnostics"]);
    let p = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "contract C #1 invariant")
        .unwrap();
    assert_eq!(p["verdict"], json!("satisfied"));
}

#[test]
fn i15_cut_defers_time_and_clock_is_inconclusive_with_structure_intact() {
    for (name, reason, exit) in [("clock", "truncated", 2), ("defer", "not_configured", 0)] {
        let (v, actual) = invoke(name, "check");
        assert_eq!(actual, exit, "{}", v["diagnostics"]);
        hand::validate_report(name, &v).unwrap();
        let work = &v["metrics"]["overall"]["work"];
        assert_eq!(work["reason"], reason);
        assert!(work["min"].is_null() && work["max"].is_null());
    }
}
