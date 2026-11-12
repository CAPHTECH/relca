//! ADR0039 real CLI, independently hand-derived fixture values and closed wire.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/limits_acceptance.rs"]
mod h;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

fn run(args: &[&str]) -> (Value, i32) {
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(h::root())
        .output()
        .unwrap();
    // CLI intentionally prints diagnostics (including W-CUT-0) to stderr.
    (
        serde_json::from_slice(&o.stdout).unwrap(),
        o.status.code().unwrap(),
    )
}
fn input(name: &str, s: &str) -> PathBuf {
    let dir = h::root()
        .join("target")
        .join(format!("limits-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{name}.relca"));
    std::fs::write(&p, s).unwrap();
    p
}
#[test]
fn every_new_fixture_invocation_reports_all_failures_together() {
    let mut failures = Vec::new();
    for name in h::names() {
        let dir = h::root().join("fixtures").join(&name);
        let inv = dir.join("invocation.json");
        let iv: Option<Value> = inv
            .exists()
            .then(|| serde_json::from_slice(&std::fs::read(inv).unwrap()).unwrap());
        let invocation = fixture_match::Invocation::parse(iv.as_ref()).unwrap();
        let expected = h::expected(&name);
        invocation
            .validate_expected(&expected)
            .unwrap_or_else(|e| panic!("invalid hand schema {name}: {e}"));
        let o = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(invocation.args(&dir.join("model.relca")))
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        if let Err(e) = invocation.compare(&expected, &v) {
            failures.push(format!("{name}: {e}"));
        }
        let verdict: relca_core::Verdict =
            serde_json::from_value(expected["verdict"].clone()).unwrap();
        if o.status.code() != Some(verdict.exit_code()) {
            failures.push(format!(
                "{name}: exit {:?}, expected {}",
                o.status.code(),
                verdict.exit_code()
            ));
        }
    }
    // The fixed corpus simulates external wait; this derived old-metrics input
    // independently crosses overflow through the simulation CLI as well.
    let s = h::edit(
        &h::source("lim-counts-bare"),
        "  bounds {",
        "  simulation replications 10 seed 1 confidence 0.95\n  bounds {",
    );
    let path = input("overflow-simulation", &s);
    let (v, exit) = run(&["simulate", path.to_str().unwrap()]);
    if exit != 2 {
        failures.push(format!("overflow simulation: exit {exit}, expected2"));
    }
    if let Err(e) = fixture_match::validate_simulation_wire(&v) {
        failures.push(format!("overflow simulation wire: {e}"));
    }
    for (key, n) in [
        ("received", 3),
        ("admitted", 1),
        ("completed", 3),
        ("overflowed", 2),
        ("rejected", 0),
    ] {
        if v["metrics"]["overall"]["counts"][key]["observed_total"] != n * 10 {
            failures.push(format!(
                "overflow simulation count {key}: expected total {}",
                n * 10
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "all limits failures:\n{}",
        failures.join("\n")
    );
}
#[test]
fn explain_has_received_and_admitted_and_independent_verified_replay() {
    let path = h::root().join("fixtures/lim-admitted-deadline.broken-receipt/model.relca");
    let (v, exit) = run(&["explain", path.to_str().unwrap()]);
    assert_eq!(exit, 1);
    fixture_match::validate_explain_wire(&v).unwrap();
    assert_eq!(v["explanation"]["status"], "complete");
    let mut kinds = std::collections::BTreeSet::new();
    for c in v["explanation"]["counterexamples"].as_array().unwrap() {
        assert_eq!(c["replay"]["status"], "verified");
        for step in c["steps"].as_array().unwrap() {
            for e in step["events"].as_array().unwrap() {
                if let Some(k) = e["kind"].as_str() {
                    kinds.insert(k.to_string());
                }
            }
        }
    }
    assert!(kinds.contains("received") && kinds.contains("admitted"));
}
#[test]
fn comparison_overflow_rows_and_normal_end_mismatch_use_real_cli() {
    let s = h::compare_source();
    let path = input("compare", &s);
    let (v, exit) = run(&[
        "compare",
        path.to_str().unwrap(),
        "--plans",
        "Left,Right",
        "--mode",
        "exhaustive",
    ]);
    assert_eq!(exit, 0);
    assert_eq!(v["verdict"], "satisfied");
    fixture_match::validate_compare_wire(&v).unwrap();
    assert_eq!(
        v["tables"]["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "overflow")
            .count(),
        3
    );
    let path = input(
        "compare-mismatch",
        &h::edit(
            &s,
            "process Other { r = A.run(); end done }",
            "process Other { r = A.run(); end other }",
        ),
    );
    let (v, exit) = run(&[
        "compare",
        path.to_str().unwrap(),
        "--plans",
        "Left,Right",
        "--mode",
        "exhaustive",
    ]);
    assert_eq!(exit, 3);
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CMP-0")
    );
}
#[test]
fn wire_rejects_each_new_single_fault_and_old_models_cannot_gain_new_fields() {
    let path = h::root().join("fixtures/lim-readmit/model.relca");
    let (v, exit) = run(&["check", path.to_str().unwrap()]);
    assert_eq!(exit, 1);
    fixture_match::compare_report(&json!({}), &v).unwrap();
    for (pointer, value) in [
        ("/case_outcomes/2/admitted_at", json!({"min":3,"max":2})),
        ("/admission/external_wait_max", json!({"min":2,"max":1})),
        ("/admission/overflow_count", json!({"min":0,"max":0})),
        ("/case_outcomes/2/terminals/0/kind", json!("overflow")),
        (
            "/metrics/cases/2/external_wait/reason",
            json!("not_a_reason"),
        ),
    ] {
        let mut broken = v.clone();
        if let Some(slot) = broken.pointer_mut(pointer) {
            *slot = value;
        } else {
            let (parent, key) = pointer.rsplit_once('/').unwrap();
            broken
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), value);
        }
        assert_ne!(v, broken);
        assert!(
            fixture_match::compare_report(&json!({}), &broken).is_err(),
            "wire fault survived {pointer}"
        );
    }
    let path = h::root().join("fixtures/arrivals-contracts/model.relca");
    let (old, _) = run(&["check", path.to_str().unwrap()]);
    for field in ["admitted_at", "external_wait"] {
        let mut broken = old.clone();
        broken["case_outcomes"][0][field] = json!({"min":0,"max":0});
        assert!(fixture_match::compare_report(&json!({}), &broken).is_err());
    }
}

#[test]
fn independent_new_wire_positive_and_negative_controls_do_not_need_limit_parser() {
    let path = h::root().join("fixtures/arrivals-count/model.relca");
    let (old, exit) = run(&["check", path.to_str().unwrap()]);
    assert_eq!(exit, 0);
    let mut good = old.clone();
    let affects = concat!(
        "admitted nonterminal cases with no task ever started are counted before same-time starts; ",
        "Admission limits and overflow handling are model rules, not an Actor. Counts are tested before same-time starts and finishes. Concurrent cases stop counting at the business end, not contest closure. ",
        "Receipt and initial values occur at arrival; admission occurs when both limits have room. External FIFO may reuse slots released at the same time after immediate events settle; new arrivals do not. External wait is included in elapsed, within and receipt-based response deadlines. first_start_wait remains arrival-based and overlaps external_wait; do not add them."
    );
    good["assumptions"].as_array_mut().unwrap().push(json!({"field":"queue.capacity","value":"2 on_overflow wait_external fifo","affects":affects}));
    good["admission"]["external_wait_max"] = json!({"min":0,"max":0});
    for c in good["case_outcomes"].as_array_mut().unwrap() {
        let a = c["arrival"].clone();
        c["admitted_at"] = json!({"min":a,"max":a});
    }
    fixture_match::compare_report(&json!({}), &good)
        .expect("independent complete new wait wire must be admitted");
    for pointer in [
        "/admission/external_wait_max",
        "/case_outcomes/0/admitted_at",
    ] {
        let mut broken = good.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        broken
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            fixture_match::compare_report(&json!({}), &broken).is_err(),
            "new mandatory key survived removal {pointer}"
        );
    }
    let mut wrong = old;
    wrong["case_outcomes"][0]["admitted_at"] = json!({"min":0,"max":0});
    assert!(
        fixture_match::compare_report(&json!({}), &wrong).is_err(),
        "old source cannot gain new fields"
    );
    let mut inverted = good.clone();
    inverted["admission"]["external_wait_max"] = json!({"min":2,"max":1});
    assert!(fixture_match::compare_report(&json!({}), &inverted).is_err());
    let mut unknown = good;
    unknown["admission"]["future_limit"] = json!(0);
    assert!(fixture_match::compare_report(&json!({}), &unknown).is_err());
}
