//! Actual CLI stdout/stderr/exit for P3a. One unique directory per invocation.
#[path = "../../relca-core/tests/support/p3a_hand.rs"]
mod h;
#[path = "../../relca-core/tests/support/p3a_inputs.rs"]
mod i;
#[path = "../../relca-core/tests/support/response_loss_inputs.rs"]
mod q1;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn diagnostics(v: &Value, prefix: &str) -> String {
    h::rows(&v["diagnostics"])
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
fn checked(out: Output, mode: &str) -> (Value, i32) {
    assert_eq!(
        out.stdout.iter().filter(|b| **b == b'\n').count(),
        1,
        "one complete JSON line"
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut stderr = String::new();
    match mode {
        "compare" => {
            for p in h::rows(&v["plans"]) {
                stderr += &diagnostics(
                    &p["report"],
                    &format!("configuration {}: ", p["configuration"].as_str().unwrap()),
                );
            }
            stderr += &diagnostics(&v, "compare: ");
        }
        "diff" => {
            stderr += &diagnostics(&v["from"]["report"], "from: ");
            stderr += &diagnostics(&v["to"]["report"], "to: ");
            stderr += &diagnostics(&v, "diff: ");
        }
        "sensitivity" => {
            for p in h::rows(&v["points"]) {
                if p["status"] == "evaluated" {
                    stderr += &diagnostics(
                        &p["report"],
                        &format!("point {}: ", p["id"].as_str().unwrap()),
                    );
                }
            }
            stderr += &diagnostics(&v, "sensitivity: ");
        }
        _ => stderr += &diagnostics(&v, ""),
    }
    assert_eq!(
        out.stderr,
        stderr.as_bytes(),
        "complete diagnostic transport, not a substring"
    );
    (v, out.status.code().unwrap())
}
fn run(s: &str, mode: &str, args: &[&str]) -> (Value, i32) {
    let path = i::dir().join("model.relca");
    fs::write(&path, s).unwrap();
    checked(
        Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg(mode)
            .arg(path)
            .args(args)
            .output()
            .unwrap(),
        mode,
    )
}
fn suite(path: &Path) -> (Value, i32) {
    checked(
        Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("sensitivity")
            .arg(path)
            .args(["--mode", "exhaustive"])
            .output()
            .unwrap(),
        "sensitivity",
    )
}
fn code(n: &str) -> i32 {
    match n {
        "satisfied" => 0,
        "violated" => 1,
        "inconclusive" => 2,
        _ => panic!(),
    }
}
macro_rules! comparisons {
    ($test:ident,$name:literal,$mode:literal)=>{#[test]fn $test(){let mode=$mode;
        let args=if mode=="compare" {vec!["--plans","A,B","--mode","exhaustive"]}else{vec!["--from","A","--to","B"]};
        let (v,exit)=run(&i::pair($name),mode,&args);assert_eq!(exit,code(h::leaf($name)),"{}",v["diagnostics"]);h::comparison(&v,$name,mode=="compare");
        if mode=="diff" {let typed:relca_core::diff::DiffReport=serde_json::from_value(v.clone()).unwrap();assert_eq!(json!(typed),v);}
        let children=h::children(&v,mode=="compare");
        for (child,configuration) in children.into_iter().zip(["A","B"]) {
            assert_eq!(child["ledger"]["execution"]["selection"]["requested"]["configuration"],configuration);
            assert_eq!(child["ledger"]["execution"]["selection"]["effective"]["configuration"]["value"],configuration);
            assert_eq!(child["ledger"]["derivation"]["bundle"],v["ledger"]["derivation"]["bundle"]);
        }
    }};
}
comparisons!(r1_cli_compare_g_resend, "g-resend", "compare");
comparisons!(r1_cli_diff_g_resend, "g-resend", "diff");
comparisons!(r1_cli_compare_modeled_resend, "modeled-resend", "compare");
comparisons!(r1_cli_diff_modeled_resend, "modeled-resend", "diff");
comparisons!(r1_cli_compare_g_unknown, "g-unknown", "compare");
comparisons!(r1_cli_diff_g_unknown, "g-unknown", "diff");
comparisons!(r1_cli_compare_g_probability, "g-probability", "compare");
comparisons!(r1_cli_diff_g_probability, "g-probability", "diff");
comparisons!(r1_cli_compare_k8_resend, "k8-g-resend", "compare");
comparisons!(r1_cli_diff_k8_resend, "k8-g-resend", "diff");
comparisons!(r1_cli_compare_k8_unknown, "k8-g-unknown", "compare");
comparisons!(r1_cli_diff_k8_unknown, "k8-g-unknown", "diff");
comparisons!(r1_cli_compare_k8_clock, "k8-g-clock", "compare");
comparisons!(r1_cli_diff_k8_clock, "k8-g-clock", "diff");
macro_rules! wait {
    ($test:ident,$path:literal,$sim:literal) => {
        #[test]
        fn $test() {
            let (v, exit) = run(
                &i::wait($path),
                if $sim { "simulate" } else { "check" },
                &[],
            );
            assert_eq!(exit, if $sim || $path == "UU" { 2 } else { 0 });
            h::waiting(&v, $path, $sim);
        }
    };
}
wait!(r1c_cli_check_d, "D", false);
wait!(r1c_cli_check_ud, "UD", false);
wait!(r1c_cli_check_uu, "UU", false);
wait!(r1c_cli_simulate_d, "D", true);
wait!(r1c_cli_simulate_ud, "UD", true);
wait!(r1c_cli_simulate_uu, "UU", true);
#[test]
fn r1_cli_probability_endpoints_compare_samples_unknown_is_unfinished_subset() {
    for (name, kind) in [
        ("g-probability-zero", "end"),
        ("g-probability-one", "receipt_unknown"),
    ] {
        let s = i::append(
            &i::pair(name),
            "simulation replications 4 seed 7 confidence 0.5",
        );
        let (v, exit) = run(&s, "compare", &["--plans", "A,B", "--mode", "simulation"]);
        assert_eq!(exit, 2);
        h::verdict(&v, "inconclusive");
        for p in h::children(&v, true) {
            assert_eq!(
                p["case_outcomes"][0]["unfinished_count"],
                if kind == "receipt_unknown" { 4 } else { 0 }
            );
            assert_eq!(
                h::row(&p["case_outcomes"][0]["terminals"], "kind", kind)["count"],
                4
            );
        }
        {
            let u = h::row(&v["tables"]["terminals"], "kind", "receipt_unknown");
            for cell in h::rows(&u["values"]) {
                assert_eq!(cell["status"], "reported");
                assert_eq!(
                    cell["value"]["count"],
                    if kind == "receipt_unknown" { 4 } else { 0 }
                );
            }
        }
    }
}
#[test]
fn r4_cli_loss_points_execute_and_retry_policy_refuses_before_execution() {
    let (v, exit) = suite(&i::suite(&[
        ("zero", i::point("0", 1)),
        ("tenth", i::point("0.1", 1)),
        ("fifth", i::point("0.2", 1)),
    ]));
    assert_eq!(exit, 1);
    h::verdict(&v, "violated");
    for (p, n) in h::rows(&v["points"]).iter().zip(["1", "99/100", "24/25"]) {
        assert_eq!(p["status"], "evaluated");
        h::probability(
            &h::row(&p["report"]["properties"], "id", "probability reached #1")["probability"],
            n,
        );
    }
    let a = i::point("0.1", 1);
    let b = i::edit(&a, "backoff fixed 0min", "backoff fixed 3min");
    let (v, exit) = suite(&i::suite(&[("a", a), ("b", b)]));
    assert_eq!(exit, 3);
    h::verdict(&v, "invalid");
    assert_eq!(v["points"][1]["reason"], "sensitivity_contract_mismatch");
    assert!(
        h::rows(&v["points"])
            .iter()
            .all(|p| p["status"] != "evaluated")
    );
}
#[test]
fn r4_cli_duration_points_and_responsibility_are_connected() {
    let a = i::responsibility();
    let b = i::edit(&a, "fixed 1min", "fixed 3min");
    let (v, exit) = suite(&i::suite(&[("one", a), ("three", b)]));
    assert_eq!(exit, 1);
    for (p, n) in h::rows(&v["points"]).iter().zip(["1", "0"]) {
        assert_eq!(p["status"], "evaluated");
        h::probability(
            &h::row(&p["report"]["properties"], "id", "probability reached #1")["probability"],
            n,
        );
        assert!(!h::rows(&p["report"]["responsibilities"]).is_empty());
    }
}
#[test]
fn r4_usage_lists_sensitivity_and_conform_without_enabling_verify() {
    let out = Command::new(env!("CARGO_BIN_EXE_relca")).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8(out.stderr).unwrap();
    let usage = stderr.lines().next().unwrap();
    let commands = usage
        .strip_prefix("usage: relca <")
        .unwrap()
        .strip_suffix("> ...")
        .unwrap()
        .split('|')
        .collect::<Vec<_>>();
    for name in ["sensitivity", "conform"] {
        assert_eq!(
            commands.iter().filter(|c| **c == name).count(),
            1,
            "{usage}"
        );
    }
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("verify")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
}
#[test]
fn r1_cli_ec_example_no_longer_refuses_exhaustive_comparison() {
    let (v, exit) = run(
        include_str!("../../relca-core/tests/support/p3a_ec.relca"),
        "compare",
        &["--plans", "Keyed,Plain", "--mode", "exhaustive"],
    );
    assert_eq!(exit, 1);
    h::verdict(&v, "violated");
    let u = h::row(&v["tables"]["terminals"], "kind", "receipt_unknown");
    for cell in h::rows(&u["values"]) {
        h::observed(&cell["value"], 4, 4);
    }
}
