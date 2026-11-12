//! ADR0062 real stdout JSON paths, stderr diagnostics and actual exit codes.
#[path = "../../relca-core/tests/support/q5_oracle.rs"]
mod hand;
#[path = "../../relca-core/tests/support/q5_inputs.rs"]
mod input;
use serde_json::Value;
use std::{path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn invoke(name: &str, source: &str) -> (Value, i32) {
    static INVOCATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let invocation = INVOCATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let directory = root()
        .join(".tmp/q5-cli")
        .join(format!("{}-{name}-{invocation}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("model.relca");
    std::fs::write(&path, source).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_relca"));
    cmd.arg(input::command(name))
        .arg(path)
        .current_dir(root())
        .env("LC_ALL", "C")
        .env("CARGO_NET_OFFLINE", "true");
    if let Some(scenario) = input::scenario(name) {
        cmd.args(["--scenario", scenario]);
    }
    let out = cmd.output().unwrap();
    let exit = out.status.code().expect("normal exit");
    let v: Value = serde_json::from_slice(&out.stdout).expect("stdout JSON");
    assert_eq!(
        out.stdout.iter().filter(|b| **b == b'\n').count(),
        1,
        "single report line"
    );
    let ds = v["diagnostics"].as_array().unwrap();
    let mirror = ds
        .iter()
        .map(|d| {
            format!(
                "{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect::<String>();
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        mirror,
        "diagnostic mirror {name}"
    );
    (v, exit)
}
fn accept(name: &str) {
    let (v, exit) = invoke(name, &input::source(name));
    hand::validate(name, &v)
        .unwrap_or_else(|e| panic!("{name}: {e}; diagnostics {}", v["diagnostics"]));
    assert_eq!(exit, hand::exit(name), "{name}: hand exit");
}
macro_rules! run{($($test:ident=>$name:literal),+$(,)?)=>{$(#[test]fn $test(){accept($name);})+};}
run! {
    p01_start=>"start",p01_equal_tail=>"start-tie",p02_omitted_tie=>"equal",
    p02_explicit_fifo=>"equal-fifo",p02_call_order=>"call-order",p02_reverse_call_order=>"call-order-reverse",
    p02_simulation_refusal=>"equal-sim",p03_people=>"people",p03_environment=>"people-environment",
    p04_calendar=>"calendar",p04_later=>"calendar-later",p05_external=>"external",
    p05_fifo_control=>"external-fifo",p06_external_tie=>"external-equal",p06_external_fifo=>"external-equal-fifo",
    p06_external_simulation_refusal=>"external-equal-sim",p06_initial_case_id=>"initial",
    p07_version_winner=>"conflict",p07_without_resolve=>"conflict-no-resolve",p08_nondet=>"conflict-equal",
    p08_explicit_id=>"conflict-id",p09_no_future_wait=>"future",p09_same_due=>"future-tied",
    p10_keep_stale=>"stale",p11_unrelated_orders=>"unrelated",p11_ack=>"ack",p12_clock=>"clock",
    p12_horizon=>"horizon",p13_actor_hol=>"actor-hol",p13_resource_hol=>"resource-hol",
    p13_independent_frontier=>"independent-hol",p13_unattributed=>"unattributed",p13_physical=>"physical",
    p14_explain_start=>"explain-start",p14_explain_external=>"explain-external",p14_explain_finish=>"explain-conflict",
    p15_retry=>"retry",p15_unknown=>"retry-unknown",p16_unused=>"unused",p16_maximum=>"max-rank",
    p16_label=>"label",p16_scenario=>"scenario"
}

#[test]
fn p16_all_static_reasons_and_exit_are_checked() {
    let mut errors = Vec::new();
    for &(name, code, reason) in input::STATIC {
        let (v, exit) = invoke(name, &input::source(name));
        if let Err(e) = hand::diagnostic(&v, code, reason) {
            errors.push(format!("{name}: {e}"));
        }
        let wanted = if code.starts_with("E-") { 3 } else { 4 };
        if exit != wanted {
            errors.push(format!("{name}: exit {exit}, expected {wanted}"));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn green_five_broken_outputs_are_rejected_without_byte_snapshot() {
    for (name, description, bad) in hand::faults() {
        hand::validate(name, &hand::witness(name)).unwrap();
        assert!(hand::validate(name, &bad).is_err(), "{description}");
    }
}

#[test]
fn green_legacy_models_without_priority_keep_hand_paths_and_exit() {
    // Hand-fixed current contract, not a capture of new priority output. Q7 is
    // free to append uncertainty without invalidating these selected paths.
    for (name, cases, expected_exit) in [
        ("equal", vec![(1, 2), (2, 4)], 0),
        ("external", vec![(1, 4), (2, 8), (3, 12)], 0),
    ] {
        let (v, exit) = invoke(name, &input::substrate(name));
        assert_eq!(exit, expected_exit);
        assert_eq!(v["verdict"], "satisfied");
        assert!(
            v.get("priority_policy").is_none(),
            "new field leaked into legacy input"
        );
        for (id, at) in cases {
            let c = v["case_outcomes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["case_id"] == id)
                .unwrap();
            assert_eq!(c["terminals"][0]["completed_at"], hand::point(at));
        }
    }
    let (v, exit) = invoke("version-retry", &input::substrate("version-retry"));
    assert_eq!(exit, 4);
    hand::diagnostic(&v, "U-SEM-0", "retry_feature_combination").unwrap();
}

#[test]
fn green_three_existing_fixture_contracts_keep_paths_and_exit() {
    let fixtures = [
        (
            "legacy-met",
            include_str!("../../../fixtures/met-fifo/model.relca"),
            vec![8, 13, 18],
        ),
        (
            "legacy-limits",
            include_str!("../../../fixtures/lim-concurrent-wait/model.relca"),
            vec![2, 4, 6],
        ),
        (
            "legacy-version",
            include_str!("../../../fixtures/cf-version/model.relca"),
            vec![1, 1],
        ),
    ];
    for (name, source, completions) in fixtures {
        let (v, exit) = invoke(name, source);
        assert_eq!(exit, 0, "{name}");
        assert_eq!(v["verdict"], "satisfied");
        assert!(v.get("priority_policy").is_none());
        let rows = v["case_outcomes"].as_array().unwrap();
        assert_eq!(rows.len(), completions.len());
        for (i, at) in completions.iter().enumerate() {
            let row = rows.iter().find(|r| r["case_id"] == i + 1).unwrap();
            let ts = row["terminals"].as_array().unwrap();
            assert_eq!(ts.len(), if name == "legacy-version" { 2 } else { 1 });
            for t in ts {
                assert_eq!(t["completed_at"], hand::point(*at));
                assert_eq!(
                    t["work"],
                    hand::point(if name == "legacy-met" {
                        8
                    } else if name == "legacy-limits" {
                        2
                    } else {
                        1
                    })
                );
            }
        }
        if name == "legacy-met" {
            assert_eq!(
                v.pointer("/metrics/overall/work").unwrap(),
                &hand::scalar(24)
            );
            assert_eq!(
                v.pointer("/metrics/overall/observed_cost_total").unwrap(),
                &hand::scalar(78)
            );
        }
    }
}
