//! Actual CLI channels, ADR0059 oracle, existing raw goldens, no src edits.
#[path = "../../relca-core/tests/support/q4_oracle.rs"]
mod hand;
#[path = "../../relca-core/tests/support/q4_inputs.rs"]
mod input;
#[path = "support/ledger_shape.rs"]
mod ledger;
use serde_json::Value;
use std::{path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn invoke(name: &str) -> (Value, i32) {
    // All writing stays inside this worktree, including during parallel tests.
    let dir = root()
        .join(".tmp/q4-cli")
        .join(format!("{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, input::source(name)).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_relca"));
    command
        .arg(input::command(name))
        .arg(path)
        .current_dir(root())
        .env("LC_ALL", "C")
        .env("CARGO_NET_OFFLINE", "true");
    if let Some(s) = input::scenario(name) {
        command.args(["--scenario", s]);
    }
    let out = command.output().unwrap();
    let exit = out.status.code().expect("normal CLI exit");
    assert_eq!(
        out.stdout.iter().filter(|b| **b == b'\n').count(),
        1,
        "one JSON line"
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let stderr: String = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        stderr,
        "{name}: exact diagnostic mirror, including warnings"
    );
    ledger::validate(&v["ledger"]).unwrap();
    (v, exit)
}

fn runtime(name: &str) {
    let (v, exit) = invoke(name);
    hand::validate(name, &v)
        .unwrap_or_else(|e| panic!("{name}: {e}; diagnostics {}", v["diagnostics"]));
    let expected = match v["verdict"].as_str().unwrap() {
        "satisfied" => 0,
        "violated" => 1,
        "inconclusive" => 2,
        "invalid" => 3,
        "unsupported" => 4,
        "vacuous" => 5,
        _ => panic!("unknown verdict"),
    };
    assert_eq!(
        exit, expected,
        "{name}: exit agrees with hand-checked verdict"
    );
}
macro_rules! run {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            runtime($name);
        }
    };
}
run!(c01_open_calendar_wait, "open");
run!(c01_later_open, "open-later");
run!(c02_continue_and_unset_rate, "continue");
run!(c02_earlier_close, "continue-earlier-close");
run!(c03_shift_identity, "shift");
run!(c03_later_shift, "shift-later");
run!(c04_atomic_resource_start, "resource");
run!(c04_later_resource, "resource-later");
run!(c05_ack_after_close, "ack");
run!(c05_ack_delay_is_interval, "ack-later");
run!(c06_horizon, "horizon");
run!(c06_clock, "clock");
run!(c07_condition_not_task_wait, "condition");
run!(c07_later_condition, "condition-later");
run!(c08_true, "condition-true");
run!(c08_deadlock, "condition-deadlock");
run!(c09_same_tick_orders, "condition-race");
run!(c09_simulate_rejects_race, "condition-race-sim");
run!(c09_simulation_without_race, "condition-sim");
run!(c10_disjoint_calendar_and_condition, "condition-open");
run!(c10_later_calendar, "condition-open-later");
run!(c11_c12_arrival_time_not_first_sample, "arrivals");
run!(c11_changed_arrival_deadline, "arrivals-burst");
run!(c11_calendar_arrival_crossing, "arrivals-calendar");
run!(green_fixed_burst, "fixed-burst");
run!(green_fixed_spaced, "fixed-spaced");
run!(c13_explicit_rate, "explicit-rate");
run!(c13_explicit_zero_rate, "free-overtime");
run!(c15_retry, "retry");
run!(c15_later_retry, "retry-later");
run!(c17_drop_not_overcommit, "drop");
run!(c17_zero_capacity_rate, "drop-zero");
run!(c17_zero_capacity_missing_rate, "drop-zero-missing-rate");

#[test]
fn c16_every_static_case_has_its_hand_diagnostic_and_nonzero_exit() {
    let mut failures = Vec::new();
    for (name, code, reason) in input::STATIC {
        let (v, exit) = invoke(name);
        if let Err(e) = hand::diagnostic(&v, code, reason) {
            failures.push(format!("{name}: {e}"));
        }
        let wanted = if code.starts_with("E-") { 3 } else { 4 };
        if exit != wanted {
            failures.push(format!("{name}: exit {exit}, wanted {wanted}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn green_six_existing_calendar_free_jobs_match_all_three_raw_channels() {
    let out = Command::new("python3")
        .args([
            "-B",
            "crates/relca-cli/tests/support/q4_legacy_raw.py",
            env!("CARGO_BIN_EXE_relca"),
        ])
        .current_dir(root())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        "Q4 legacy raw controls: 6 jobs, stdout/stderr/exit unchanged"
    );
}

#[test]
fn green_hand_wire_faults_are_not_successes_of_production() {
    for (name, description, bad) in hand::mutation_controls() {
        hand::validate(name, &hand::expected(name)).unwrap();
        assert!(hand::validate(name, &bad).is_err(), "{description}");
    }
}
