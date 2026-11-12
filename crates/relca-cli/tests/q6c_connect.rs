//! Real CLI hand acceptance. Every invocation gets a new atomic-nonce directory.
#[path = "../../relca-core/tests/support/q6b_inputs.rs"]
mod b_input;
#[path = "../../relca-core/tests/support/q9_inputs.rs"]
mod contest_input;
#[path = "../../relca-core/tests/support/q6c_oracle.rs"]
mod h;
#[path = "../../relca-core/tests/support/q6c_inputs.rs"]
mod i;
use relca_core::{conformance, mutation, report};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
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
fn checked(out: Output, mode: &str) -> (Value, i32, Vec<u8>) {
    let v: Value = serde_json::from_slice(&out.stdout).expect("one JSON stdout");
    assert_eq!(out.stdout.iter().filter(|b| **b == b'\n').count(), 1);
    let mut stderr = String::new();
    match mode {
        "compare" => {
            for p in v["plans"].as_array().unwrap() {
                stderr.push_str(&diagnostics(
                    &p["report"],
                    &format!("configuration {}: ", p["configuration"].as_str().unwrap()),
                ));
            }
            stderr.push_str(&diagnostics(&v, "compare: "));
        }
        "diff" => {
            stderr.push_str(&diagnostics(&v["from"]["report"], "from: "));
            stderr.push_str(&diagnostics(&v["to"]["report"], "to: "));
            stderr.push_str(&diagnostics(&v, "diff: "));
        }
        "sensitivity" => {
            for p in v["points"].as_array().unwrap() {
                if p["status"] == "evaluated" {
                    stderr.push_str(&diagnostics(
                        &p["report"],
                        &format!("point {}: ", p["id"].as_str().unwrap()),
                    ));
                }
            }
            stderr.push_str(&diagnostics(&v, "sensitivity: "));
        }
        "mutate" => {
            if !v["original"].is_null() {
                stderr.push_str(&diagnostics(&v["original"], "original: "));
            }
            for row in v["mutations"].as_array().unwrap() {
                for m in row["mutants"].as_array().unwrap() {
                    if !m["report"].is_null() {
                        stderr.push_str(&diagnostics(
                            &m["report"],
                            &format!("{}: ", m["id"].as_str().unwrap()),
                        ));
                    }
                }
            }
            stderr.push_str(&diagnostics(&v, "mutate: "));
        }
        _ => stderr.push_str(&diagnostics(&v, "")),
    }
    assert_eq!(
        out.stderr,
        stderr.as_bytes(),
        "complete stderr with public prefixes"
    );
    (v, out.status.code().unwrap(), out.stdout)
}
fn run(s: &str, mode: &str, args: &[&str]) -> (Value, i32, Vec<u8>) {
    let d = i::dir();
    let path = d.join("model.relca");
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
fn run_suite(path: &Path) -> (Value, i32, Vec<u8>) {
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
macro_rules! comparisons {
    ($name:ident,$kind:literal,$mode:literal) => {
        #[test]
        fn $name() {
            let s = match $kind {
                "work" => i::work(),
                "bands" => i::bands(),
                "degrade" => i::degradation(),
                _ => unreachable!(),
            };
            let args = if $mode == "compare" {
                vec!["--plans", "From,To", "--mode", "exhaustive"]
            } else {
                vec!["--from", "From", "--to", "To"]
            };
            let (v, exit, _) = run(&s, $mode, &args);
            assert_eq!(exit, 0, "{}", v["diagnostics"]);
            if $mode == "compare" {
                h::compare(&v, $kind, false);
            } else {
                h::diff(&v, $kind, false);
            }
        }
    };
}
comparisons!(i17_cli_diff_work, "work", "diff");
comparisons!(i17_cli_compare_work, "work", "compare");
comparisons!(i17_cli_diff_band, "bands", "diff");
comparisons!(i17_cli_compare_band, "bands", "compare");
comparisons!(i17_cli_diff_degrade, "degrade", "diff");
comparisons!(i17_cli_compare_degrade, "degrade", "compare");
#[test]
fn i17_cli_engine_unconnected_comparisons_refuse_q6() {
    for kind in ["s1", "retry"] {
        for one in [false, true] {
            for mode in ["compare", "diff"] {
                let args = if mode == "compare" {
                    vec!["--plans", "From,To", "--mode", "exhaustive"]
                } else {
                    vec!["--from", "From", "--to", "To"]
                };
                let (v, exit, _) = run(&i::refused_comparison(kind, one), mode, &args);
                assert_eq!(exit, 4);
                assert_eq!(v["verdict"], "unsupported");
                let children: Vec<_> = if mode == "compare" {
                    v["plans"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| &p["report"])
                        .collect()
                } else {
                    vec![&v["from"]["report"], &v["to"]["report"]]
                };
                let mut refused = 0;
                for child in children {
                    if child["verdict"] == "unsupported" {
                        h::refusal(child, false, false);
                        refused += 1;
                    } else {
                        h::no_errors(child);
                        assert_eq!(child["verdict"], "satisfied");
                    }
                }
                assert!(refused > 0);
            }
        }
    }
}
#[test]
fn i18_cli_accepted_points_have_hand_values_and_complete_kernel_evidence() {
    let kinds = ["base", "probability", "value", "basis"];
    let path = i::suite(&kinds.iter().map(|k| (*k, i::point(k))).collect::<Vec<_>>());
    let (v, exit, _) = run_suite(&path);
    assert_eq!(exit, 0, "{}", v["diagnostics"]);
    h::suite(&v, &kinds);
}
#[test]
fn i18_cli_qualification_and_band_are_contract_refusals_not_q6_disconnection() {
    for kind in ["qualification", "band"] {
        let path = i::suite(&[("base", i::point("base")), (kind, i::point(kind))]);
        let (v, exit, _) = run_suite(&path);
        assert_eq!(exit, 3, "{v}");
        assert_eq!(v["verdict"], "invalid");
        assert_eq!(v["points"][1]["reason"], "sensitivity_contract_mismatch");
        assert_eq!(v["diagnostics"].as_array().unwrap().len(), 1);
        assert_eq!(v["diagnostics"][0]["code"], "E-CMP-0");
        for p in v["points"].as_array().unwrap() {
            assert_ne!(p["status"], "evaluated");
        }
        // Even refused, successfully lowered points retain their actual inputs.
        h::effective(&v["points"][1]["effective_inputs"], kind);
    }
}
#[test]
fn i18_cli_degrade_is_still_q6_feature_combination() {
    let s = i::edit(
        &b_input::base(),
        "verify reachable end.done",
        "verify probability reached end.done >= 1",
    );
    let (v, exit, _) = run_suite(&i::suite(&[("a", s.clone()), ("b", s)]));
    assert_eq!(exit, 4);
    assert_eq!(v["diagnostics"].as_array().unwrap().len(), 2);
    for p in v["points"].as_array().unwrap() {
        assert_eq!(p["reason"], "sensitivity_feature_combination");
        assert_eq!(p["status"], "refused");
        h::sensitivity_refusal(p);
    }
}
fn log() -> String {
    let rows = [
        json!({"kind":"header","schema":"relca-log/v1","clock":"shared","time_unit":"min",
        "window":{"from":0,"to":20},"cases":[{"case_id":"c","from_case_start":true,"through_case_end":true}],
        "actor_map":[{"source_id":"desk","actor":"Desk"}],"executor_map":[{"source_id":"person","person":"p"}]}),
        json!({"kind":"event","event_id":"start","case_id":"c","task_id":"r","operation":"Desk.run","lifecycle":"start","cancel_phase":null,
            "time":{"earliest":0,"latest":0},"after":[],"actor_id":"desk","executor_id":"person","input_version":"v1","output_version":null,"result":null}),
        json!({"kind":"event","event_id":"finish","case_id":"c","task_id":"r","operation":"Desk.run","lifecycle":"finish","cancel_phase":null,
            "time":{"earliest":2,"latest":2},"after":["start"],"actor_id":"desk","executor_id":"person","input_version":"v1","output_version":"v1","result":"ok"}),
    ];
    rows.iter().map(|v| format!("{v}\n")).collect()
}
fn mode_run(kind: &str, s: &str) -> (Value, i32, Vec<u8>) {
    if kind == "conform" {
        let d = i::dir();
        let path = d.join("events.jsonl");
        fs::write(&path, log()).unwrap();
        run(
            s,
            "conform",
            &[
                "--log",
                path.to_str().unwrap(),
                "--scope",
                "event-contracts",
            ],
        )
    } else {
        run(
            s,
            match kind {
                "mutate" => "mutate",
                "s1" => "simulate",
                _ => "check",
            },
            &[],
        )
    }
}
fn old_bytes(kind: &str, s: &str) -> (Vec<u8>, i32) {
    let (text, exit) = match kind {
        "mutate" => {
            let r = mutation::mutate(s);
            (serde_json::to_string(&r).unwrap(), r.verdict.exit_code())
        }
        "conform" => {
            let r = conformance::conform(s, log().as_bytes(), None, None);
            (serde_json::to_string(&r).unwrap(), r.verdict.exit_code())
        }
        "s1" => {
            let r = report::simulate(s);
            (serde_json::to_string(&r).unwrap(), r.verdict.exit_code())
        }
        _ => {
            let r = report::check(s);
            (serde_json::to_string(&r).unwrap(), r.verdict.exit_code())
        }
    };
    (format!("{text}\n").into_bytes(), exit)
}
fn strict_mode(kind: &str) {
    let old = i::unconnected(kind, "old");
    let (v, exit, stdout) = mode_run(kind, &old);
    let (expected, expected_exit) = old_bytes(kind, &old);
    assert_eq!(stdout, expected, "old model raw stdout");
    assert_eq!(exit, expected_exit);
    assert!(!v.to_string().contains("q6_feature_combination"));
    assert_eq!(
        v["verdict"],
        if kind == "mutate" {
            "unsupported"
        } else {
            "satisfied"
        }
    );
    if kind == "mutate" {
        assert_eq!(v["original_verdict"], "satisfied");
    }
    assert_eq!(
        exit,
        if kind == "mutate" { 4 } else { 0 },
        "old model is a positive substrate: {v}"
    );
    for feature in ["qualification", "performance"] {
        let (v, exit, _) = mode_run(kind, &i::unconnected(kind, feature));
        assert_eq!(exit, 4, "{kind}/{feature}: {v}");
        h::refusal(&v, kind == "mutate", kind == "conform");
    }
}
#[test]
fn m5_mutate_exact_diagnostics_and_old_stdout() {
    strict_mode("mutate");
}
#[test]
fn m5_conform_exact_diagnostics_and_old_stdout() {
    strict_mode("conform");
}
#[test]
fn m5_contest_exact_diagnostics_and_old_stdout() {
    strict_mode("contest");
}
#[test]
fn m5_s1_exact_diagnostics_and_old_stdout() {
    strict_mode("s1");
}
#[test]
fn m5_retry_exact_diagnostics_and_old_stdout() {
    strict_mode("retry");
}
