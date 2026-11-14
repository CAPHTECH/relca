//! ADR 0072 CLI acceptance. Semantic expectations are hand-written, never captured.
#[path = "../../relca-core/tests/support/agency_viewpoint_limit_inputs.rs"]
mod input;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn run(cmd: &str, source: &str, flags: &[&str]) -> Output {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = root().join(".tmp/viewpoint-limit-acceptance");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "{}-{}.relca",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(cmd)
        .arg(path)
        .args(flags)
        .current_dir(root())
        .output()
        .unwrap()
}
fn wire(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&o.stderr)))
}
fn scale(v: &Value, n: u64) {
    assert_eq!(
        v["ledger"]["execution"]["limits"]["agency_limit_scale"],
        json!({"status":"known","value":n,"reason":null})
    );
}
fn warning(v: &Value, block: &str) {
    let b = &v["agency"]["viewpoints"][block];
    assert_eq!(b["status"], "not_derived", "{b}");
    assert_eq!(b["reason"], "derivation_limit");
    let limit = &b["limit"];
    assert_eq!(limit["counter"], "work", "{b}");
    assert_eq!(limit["limit"], 1_000_000);
    assert!(limit["requested"].as_u64().unwrap() > 1_000_000);
    let phase = limit["phase"].as_str().expect("phase");
    assert!(!phase.is_empty());
    let diagnostics = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| {
            d["code"] == "W-AGY-5" && d["message"].as_str().is_some_and(|s| s.contains(block))
        })
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1, "one warning per failed block: {v}");
    assert_eq!(diagnostics[0]["severity"], "warning");
    let message = diagnostics[0]["message"].as_str().unwrap();
    for word in ["work", "1000000", phase] {
        assert!(message.contains(word), "{message}");
    }
    assert!(
        message.contains(&limit["requested"].as_u64().unwrap().to_string()),
        "{message}"
    );
}

#[test]
fn control_gate_ten_and_stage_hand_values_survive_cli_serialization() {
    for (source, stages) in [
        (input::gate(10), None),
        (input::stage(5, 2, "and"), Some((5, 2))),
        (input::stage(2, 4, "and"), Some((2, 4))),
    ] {
        let o = run("check", &source, &[]);
        assert_eq!(o.status.code(), Some(0));
        let v = wire(&o);
        assert_eq!(v["verdict"], "satisfied");
        if let Some((s, w)) = stages {
            input::assert_stage(&v["agency"]["viewpoints"], s, w);
        } else {
            input::assert_gate(&v["agency"]["viewpoints"], 10);
        }
        assert!(!String::from_utf8_lossy(&o.stderr).contains("W-AGY-5"));
    }
}

#[test]
fn s1_s2_gate_twenty_cli_is_derived_without_changing_verdict() {
    let o = run("check", &input::gate(20), &[]);
    assert_eq!(o.status.code(), Some(0));
    let v = wire(&o);
    assert_eq!(v["verdict"], "satisfied");
    input::assert_gate(&v["agency"]["viewpoints"], 20);
    assert!(!String::from_utf8_lossy(&o.stderr).contains("W-AGY-5"));
}

#[test]
fn cli_default_and_explicit_one_are_the_same_except_the_recorded_assumption() {
    let source = input::gate(4);
    let plain = run("check", &source, &[]);
    let explicit = run("check", &source, &["--agency-limit-scale", "1"]);
    assert_eq!(explicit.status.code(), Some(0));
    let mut p = wire(&plain);
    let mut e = wire(&explicit);
    scale(&p, 1);
    scale(&e, 1);
    for value in [&mut p, &mut e] {
        value["assumptions"]
            .as_array_mut()
            .unwrap()
            .retain(|a| a["field"] != "agency.limit");
    }
    assert_eq!(p, e);
    assert_eq!(plain.stderr, explicit.stderr);
}

#[test]
fn cli_scale_one_hundred_derives_connected_and_and_records_the_actual_scale() {
    // No fixed assertion about all_and(11)'s post-S2 default work boundary.
    // The high-budget result, however, has an unambiguous semantic hand value.
    let o = run(
        "check",
        &input::joined(11, " && "),
        &["--agency-limit-scale", "100"],
    );
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let v = wire(&o);
    scale(&v, 100);
    let assumptions = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["field"] == "agency.limit")
        .collect::<Vec<_>>();
    assert_eq!(assumptions.len(), 1);
    assert_eq!(assumptions[0]["value"], "computational_x100");
    for (end, selection) in [("done", "all_contexts"), ("rejected", "some_contexts")] {
        let want = (1..=11)
            .map(|i| (format!("j{i}"), json!(selection)))
            .collect::<serde_json::Map<_, _>>();
        assert_eq!(
            input::framing(&v["agency"]["viewpoints"], end),
            Value::Object(want)
        );
    }
    assert_eq!(
        input::count(&v["agency"]["viewpoints"], "Judge.check"),
        json!({"min":11,"max":11})
    );
}

#[test]
fn cli_accepts_only_ascii_integers_one_through_one_thousand() {
    for n in ["1", "5", "100", "1000", "0001", "001000"] {
        let o = run("check", &input::gate(2), &["--agency-limit-scale", n]);
        assert_eq!(
            o.status.code(),
            Some(0),
            "{n}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        scale(&wire(&o), n.parse().unwrap());
    }
}

#[test]
fn cli_bad_scale_is_invalid_and_has_no_analysis_limit_value() {
    for cmd in ["check", "simulate", "explain"] {
        for n in [
            "0",
            "1001",
            "-1",
            "+5",
            "1.5",
            "",
            "99999999999999999999",
            "１２",
            " 1",
            "1 ",
            "1e2",
        ] {
            let o = run(cmd, &input::simulation(), &["--agency-limit-scale", n]);
            assert_eq!(o.status.code(), Some(3), "{cmd} {n}");
            let v = wire(&o);
            assert_eq!(v["verdict"], "invalid");
            assert_eq!(
                v["ledger"]["execution"]["limits"]["agency_limit_scale"],
                json!({"status":"not_applicable","reason":"no_analysis","value":null}),
                "{cmd} {n}: {v}"
            );
            assert!(v["agency"].is_null());
            assert!(
                v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "E-SYN-0"
                        && d["severity"] == "error"
                        && d["message"]
                            .as_str()
                            .is_some_and(|m| m.contains("--agency-limit-scale"))),
                "{v}"
            );
        }
    }
}

#[test]
fn cli_missing_and_duplicate_scale_are_not_silently_accepted() {
    for cmd in ["check", "simulate", "explain"] {
        for flags in [
            vec!["--agency-limit-scale"],
            vec!["--agency-limit-scale", "2", "--agency-limit-scale", "3"],
        ] {
            let o = run(cmd, &input::simulation(), &flags);
            assert_eq!(o.status.code(), Some(3));
            assert_eq!(
                wire(&o)["ledger"]["execution"]["limits"]["agency_limit_scale"],
                json!({"status":"not_applicable","reason":"no_analysis","value":null})
            );
        }
    }
}

#[test]
fn cli_scale_is_accepted_by_check_simulate_and_explain_only() {
    for (cmd, exit, verdict) in [
        ("check", 0, "satisfied"),
        ("simulate", 2, "inconclusive"),
        ("explain", 0, "satisfied"),
    ] {
        let o = run(cmd, &input::simulation(), &["--agency-limit-scale", "2"]);
        assert_eq!(
            o.status.code(),
            Some(exit),
            "{cmd}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        let v = wire(&o);
        assert_eq!(v["verdict"], verdict);
        scale(&v, 2);
        if cmd == "simulate" {
            assert_eq!(v["simulation"]["completed_replications"], 4);
            let p = v["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"] == "safety no_resource_overcommit")
                .unwrap();
            assert_eq!(p["verdict"], "inconclusive");
            assert_eq!(p["basis"], "sample");
        }
    }
}

#[test]
fn cli_unaccepted_commands_reject_the_flag_before_reading_the_model() {
    // Use the same model and otherwise well-formed command arguments. These
    // are invalid argv, not unsupported analysis, even for the verify backend.
    for (cmd, rest) in [
        ("mutate", vec![]),
        ("diff", vec!["--from", "A", "--to", "B"]),
        ("compare", vec!["--plans", "A,B", "--mode", "exhaustive"]),
        ("sensitivity", vec!["--mode", "exhaustive"]),
        (
            "conform",
            vec!["--log", "missing.jsonl", "--scope", "event-contracts"],
        ),
        ("verify", vec![]),
    ] {
        let mut flags = rest;
        flags.extend(["--agency-limit-scale", "2"]);
        let o = run(cmd, &input::gate(2), &flags);
        assert_eq!(
            o.status.code(),
            Some(3),
            "{cmd}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        if cmd != "diff" {
            assert_eq!(wire(&o)["verdict"], "invalid", "{cmd}");
        }
        let stderr = String::from_utf8_lossy(&o.stderr);
        assert!(
            stderr.contains("usage:") || stderr.contains("argument"),
            "{cmd}: {stderr}"
        );
    }
}

#[test]
fn cli_verdict_and_explain_replay_are_independent_of_scale() {
    for n in ["1", "100"] {
        for cmd in ["check", "explain"] {
            let o = run(cmd, &input::violation(), &["--agency-limit-scale", n]);
            assert_eq!(o.status.code(), Some(1));
            let v = wire(&o);
            assert_eq!(v["verdict"], "violated");
            let p = v["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"].as_str().unwrap().starts_with("contract Once #1"))
                .unwrap();
            assert_eq!(p["verdict"], "violated");
            if cmd == "explain" {
                assert_eq!(
                    v["explanation"]["counterexamples"][0]["replay"]["status"],
                    "verified"
                );
                let commits = v["explanation"]["counterexamples"][0]["steps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|s| s["events"].as_array().unwrap())
                    .filter(|e| {
                        e["kind"] == "commit" && e["actor"] == "Ledger" && e["capability"] == "post"
                    })
                    .count();
                assert_eq!(commits, 2);
            }
        }
    }
}

#[test]
fn d_warning_contract_rejects_missing_warning_and_missing_limit() {
    // Calibration of the shared warning predicate without inventing a future
    // post-S2 default boundary. Real injected-limit tests call it via the
    // private acceptance adapter; see the calculations document.
    let good = json!({"agency":{"viewpoints":{"framing":{"status":"not_derived","reason":"derivation_limit","limit":{"counter":"work","limit":1000000,"requested":1000001,"phase":"framing contexts"}}}},"diagnostics":[{"code":"W-AGY-5","severity":"warning","message":"framing work 1000000 requested 1000001 in framing contexts"}]});
    warning(&good, "framing");
    for missing in ["warning", "limit"] {
        let mut bad = good.clone();
        if missing == "warning" {
            bad["diagnostics"] = json!([]);
        } else {
            bad["agency"]["viewpoints"]["framing"]
                .as_object_mut()
                .unwrap()
                .remove("limit");
        }
        assert!(std::panic::catch_unwind(|| warning(&bad, "framing")).is_err());
    }
}
