//! Actual subprocess stdout/stderr/exit; each invocation has its own directory.
#[path = "../../relca-core/tests/support/r2_hand.rs"]
mod h;
#[path = "../../relca-core/tests/support/r2_inputs.rs"]
mod i;
use serde_json::{Value, json};
use std::{fs, process::Command};
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
fn run(source: &str, mode: &str, args: &[&str]) -> (Value, i32) {
    let path = i::dir().join("model.relca");
    fs::write(&path, source).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(mode)
        .arg(&path)
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        o.stdout.iter().filter(|b| **b == b'\n').count(),
        1,
        "one complete JSON line, not progress chatter"
    );
    let v: Value = serde_json::from_slice(&o.stdout).expect("JSON stdout");
    let stderr = match mode {
        "compare" => {
            let mut s = String::new();
            for p in h::rows(&v["plans"]) {
                s += &diagnostics(
                    &p["report"],
                    &format!("configuration {}: ", p["configuration"].as_str().unwrap()),
                );
            }
            s + &diagnostics(&v, "compare: ")
        }
        "diff" => {
            diagnostics(&v["from"]["report"], "from: ")
                + &diagnostics(&v["to"]["report"], "to: ")
                + &diagnostics(&v, "diff: ")
        }
        _ => diagnostics(&v, ""),
    };
    assert_eq!(o.stderr, stderr.as_bytes(), "full stderr transport");
    let exit = o.status.code().unwrap();
    (v, exit)
}
fn exit(verdict: &Value) -> i32 {
    match verdict.as_str().unwrap() {
        "satisfied" => 0,
        "violated" => 1,
        "inconclusive" => 2,
        "invalid" => 3,
        "unsupported" => 4,
        "vacuous" => 5,
        _ => panic!(),
    }
}
macro_rules! check {
    ($name:ident,$id:literal) => {
        #[test]
        fn $name() {
            let (v, code) = run(&i::source($id), "check", &[]);
            h::check(&v, $id);
            h::waiting(&v, $id);
            assert_eq!(code, exit(&v["verdict"]));
            h::strict_roundtrip(&v);
        }
    };
}
check!(cli_r01, "R01");
check!(cli_r02, "R02");
check!(cli_r03_legacy, "R03");
check!(cli_r04, "R04");
check!(cli_r05, "R05");
check!(cli_r06, "R06");
check!(cli_r07, "R07");
check!(cli_r08, "R08");
check!(cli_r09, "R09");
#[test]
fn cli_simulate_known_recovery_keeps_q1_unproved_safety_and_replication_unit() {
    for (id, t, w, unknown) in [
        ("R01", 7, 7, 0),
        ("R02", 3, 3, 4),
        ("R07", 9, 7, 0),
        ("R08", 8, 8, 0),
    ] {
        let (v, code) = run(&i::source(id), "simulate", &[]);
        assert_eq!(code, 2, "{}", v["diagnostics"]);
        h::verdict(&v, "inconclusive");
        assert_eq!(v["simulation"]["completed_replications"], 4);
        assert_eq!(v["case_outcomes"][0]["unfinished_count"], unknown);
        assert_eq!(v["response_recovery"]["coverage"], "sampled");
        let scalar = |n: &str| json!({"mean":n,"min":n,"max":n,"samples":4,"reason":null});
        assert_eq!(v["metrics"]["overall"]["work"], scalar(&w.to_string()));
        assert_eq!(
            v["metrics"]["overall"]["counts"]["received"]["stats"],
            scalar("1")
        );
        if unknown == 0 {
            let r = h::row(&v["metrics"]["cases"][0]["terminals"], "end", "shipped");
            assert_eq!(r["lead_time"], scalar(&t.to_string()));
        } else {
            assert_eq!(
                h::row(
                    &v["case_outcomes"][0]["terminals"],
                    "kind",
                    "receipt_unknown"
                )["count"],
                4
            );
        }
    }
}
#[test]
fn cli_explain_replays_query_evidence_decision_and_resolution_as_distinct_events() {
    let source = i::edit(&i::source("T01"), "by 7min", "by 6min");
    let (v, code) = run(&source, "explain", &[]);
    assert_eq!(code, 1, "{}", v["diagnostics"]);
    let cs = h::rows(&v["explanation"]["counterexamples"]);
    assert_eq!(
        v["explanation"]["version"],
        "invoice-counterexample-p3a-recovery-1"
    );
    assert!(!cs.is_empty());
    let c = cs
        .iter()
        .find(|c| c["id"].as_str().unwrap().contains("Deadline"))
        .expect("deadline counterexample");
    assert_eq!(c["status"], "verified");
    assert_eq!(c["replay"]["status"], "verified");
    let steps = h::rows(&c["steps"]);
    let finished = |binding: &str, time: u64| {
        steps
            .iter()
            .any(|s| s["binding"] == binding && s["time"] == time && s["kind"] == "finish")
    };
    assert!(finished("status", 3));
    assert!(finished("decision", 6));
    let q = steps
        .iter()
        .find(|s| s["kind"] == "receipt_query")
        .expect("query evidence event");
    assert_eq!(q["time"], 3);
    assert_eq!(q["command"], "pay");
    let r = steps
        .iter()
        .find(|s| s["kind"] == "resolve_effect")
        .expect("epistemic resolve event");
    assert_eq!(r["time"], 6);
    assert_eq!(r["command"], "pay");
    for s in [q, r] {
        let span = &s["location"]["span"];
        let text = &source
            [span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize];
        assert!(!text.is_empty());
    }
    let unknown = steps
        .iter()
        .find(|s| s["observed_receipt"] == "unknown")
        .unwrap();
    assert_eq!(unknown["result"], Value::Null);
    assert_eq!(unknown["actual_result"], "charged");
}
#[test]
fn cli_compare_and_diff_keep_children_and_full_diagnostic_prefixes() {
    let source = i::paired("R01", "R07");
    for (mode, args) in [
        ("compare", vec!["--plans", "A,B", "--mode", "exhaustive"]),
        ("diff", vec!["--from", "A", "--to", "B"]),
    ] {
        let (v, code) = run(&source, mode, &args);
        assert_eq!(code, 5, "{v}");
        let children = if mode == "compare" {
            h::rows(&v["plans"])
                .iter()
                .map(|p| &p["report"])
                .collect::<Vec<_>>()
        } else {
            vec![&v["from"]["report"], &v["to"]["report"]]
        };
        for (r, id) in children.iter().zip(["R01", "R07"]) {
            h::check(r, id);
        }
    }
}
#[test]
fn cli_golden_is_hand_authored_semantic_projection_not_captured_unsupported_stdout() {
    let golden: Value =
        serde_json::from_str(include_str!("support/r2_semantic_golden.json")).unwrap();
    for id in [
        "R01", "R02", "R03", "R04", "R05", "R06", "R07", "R08", "R09",
    ] {
        let (v, code) = run(&i::source(id), "check", &[]);
        h::check(&v, id);
        let t = &v["case_outcomes"][0]["terminals"][0];
        assert_eq!(
            json!({"exit":code,"kind":t["kind"],"end":t["end"],"elapsed":t["elapsed"],"work":t["work"],"observed_until":t["observed_until"],"observed_work":t["observed_work"],"receipt_resolution":h::row(&v["properties"],"id","receipt_resolution")["verdict"]}),
            golden[id],
            "{id}"
        );
    }
}
#[test]
fn cli_old_bindings_keep_exact_exit_and_diagnostic_transport() {
    for word in i::WORDS {
        let (v, code) = run(&i::old_binding(word), "check", &[]);
        assert_eq!(code, 0);
        h::verdict(&v, "satisfied");
    }
}
