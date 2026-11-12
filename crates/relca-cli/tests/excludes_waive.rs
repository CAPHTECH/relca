//! D1 real-binary and independent closed-wire acceptance.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
fn expected(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(root().join(name).join("expected.json")).unwrap())
        .unwrap()
}

#[test]
fn twelve_real_binary_reports_match_hand_table_and_exit_not_their_own_verdict() {
    let mut failures = Vec::new();
    for name in fixture_match::EXCLUDES_WAIVE_FIXTURES {
        let e = expected(name);
        fixture_match::validate_expected(&e).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("check")
            .arg(root().join(name).join("model.relca"))
            .output()
            .unwrap();
        let a: Value = serde_json::from_slice(&output.stdout).unwrap();
        let verdict: relca_core::Verdict = serde_json::from_value(e["verdict"].clone()).unwrap();
        if let Err(error) = fixture_match::compare_report(&e, &a) {
            failures.push(format!(
                "{name}: {error}; observed {} exit {:?}",
                a["verdict"],
                output.status.code()
            ));
        }
        if output.status.code() != Some(verdict.exit_code()) {
            failures.push(format!(
                "{name}: exit {:?}, hand exit {}",
                output.status.code(),
                verdict.exit_code()
            ));
        }
        if e["verdict"] == "invalid" && a.get("obligations").is_some() {
            failures.push(format!("{name}: static rejection must omit obligations"));
        }
        // The existing CLI renders warnings as well as errors on stderr.
        // W-CUT-0 is deliberately expected, not an empty-stderr success case.
        let diagnostic_text: String = a["diagnostics"]
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
            String::from_utf8(output.stderr).unwrap(),
            diagnostic_text,
            "{name}: stderr projection"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn hand_wire() -> Value {
    let mut e = expected("ctx-waive");
    e.as_object_mut().unwrap().remove("absent_diagnostics");
    e["obligations"][0]["id"] = json!("case1:response1:received");
    e
}

#[test]
fn a11_closed_obligation_schema_has_independent_positive_and_single_fault_controls() {
    let good = hand_wire();
    fixture_match::validate_obligations_wire(&good).unwrap();
    // A complete positive hand table is necessary: a validator rejecting all
    // new reports would otherwise appear to detect every mutation.
    for name in fixture_match::EXCLUDES_WAIVE_FIXTURES {
        let mut e = expected(name);
        if let Some(rows) = e.get_mut("obligations").and_then(Value::as_array_mut) {
            for r in rows {
                let ordinal = r["response_id"]
                    .as_str()
                    .unwrap()
                    .split('#')
                    .nth(1)
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap();
                r["id"] = json!(format!("case1:response{ordinal}:received"));
            }
        }
        fixture_match::validate_obligations_wire(&e).unwrap();
    }
    // Extra and omitted keys at both new object levels, independently.
    for pointer in ["/obligations/0", "/obligations/0/outcomes/0"] {
        let mut bad = good.clone();
        bad.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("surplus".into(), json!(true));
        assert!(
            fixture_match::validate_obligations_wire(&bad).is_err(),
            "unknown {pointer}"
        );
        let keys: Vec<_> = good
            .pointer(pointer)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        for key in keys {
            let mut bad = good.clone();
            bad.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(&key);
            assert!(
                fixture_match::validate_obligations_wire(&bad).is_err(),
                "missing {pointer}/{key}"
            );
        }
    }
    for (pointer, value) in [
        ("/obligations", Value::Null),
        ("/obligations/0/id", json!(0)),
        ("/obligations/0/case_id", json!(0)),
        ("/obligations/0/case_id", json!(2)),
        (
            "/obligations/0/response_id",
            json!("contract Controls #2 waive"),
        ),
        ("/obligations/0/trigger_site", json!("")),
        ("/obligations/0/targets", json!([])),
        ("/obligations/0/debtor", json!("Officer")),
        ("/obligations/0/debtor_status", json!("inferred")),
        ("/obligations/0/non_waivable", json!(true)),
        ("/obligations/0/coverage", json!("truncated")),
        ("/obligations/0/outcomes", json!([])),
        ("/obligations/0/outcomes/0/status", json!("cancelled")),
        ("/obligations/0/outcomes/0/status", json!("fulfilled")),
        ("/obligations/0/outcomes/0/status", json!("unresolved")),
        ("/obligations/0/outcomes/0/deadline", json!(0)),
        ("/obligations/0/outcomes/0/resolved_at", Value::Null),
        ("/obligations/0/outcomes/0/resolved_at", json!(3)),
        ("/obligations/0/outcomes/0/reason", Value::Null),
        ("/obligations/0/outcomes/0/by_clause", json!("absent")),
        ("/obligations/0/outcomes/0/executor", Value::Null),
        ("/obligations/0/outcomes/0/authority", Value::Null),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            fixture_match::validate_obligations_wire(&bad).is_err(),
            "single fault {pointer}: {bad}"
        );
    }
    for pointer in ["/obligations", "/obligations/0/outcomes"] {
        let mut bad = good.clone();
        let rows = bad.pointer_mut(pointer).unwrap().as_array_mut().unwrap();
        rows.push(rows[0].clone());
        assert!(
            fixture_match::validate_obligations_wire(&bad).is_err(),
            "duplicate {pointer}"
        );
    }
    let mut violation = good.clone();
    violation["obligations"][0]["outcomes"][0] = json!({"status":"violated","triggered_at":0,"deadline":2,"resolved_at":1,"reason":"deadlock_unresolved","by_clause":null,"executor":null,"authority":null});
    fixture_match::validate_obligations_wire(&violation).unwrap();
    violation["obligations"][0]["outcomes"][0]["reason"] = json!("expiry");
    assert!(fixture_match::validate_obligations_wire(&violation).is_err());
    let mut cut = good.clone();
    cut["bounds"]["truncated"] = json!(true);
    cut["obligations"][0]["coverage"] = json!("truncated");
    cut["obligations"][0]["outcomes"][0] = json!({"status":"unresolved","triggered_at":0,"deadline":2,"resolved_at":null,"reason":"clock","by_clause":null,"executor":null,"authority":null});
    fixture_match::validate_obligations_wire(&cut).unwrap();
}

#[test]
fn a11_report_containing_commands_do_not_bypass_nested_obligation_validation() {
    // On implemented D1 this starts with a real valid DTO, not a malformed
    // synthetic outer comparison. Corrupt only one nested obligation field.
    let src = std::fs::read_to_string(root().join("ctx-waive/model.relca")).unwrap();
    let src = src.replace("  bounds", "  cut C { include [Officer,B,Wait] }\n  configuration A { cut C; process Flow }\n  configuration BPlan { cut C; process Flow }\n  bounds");
    let d = serde_json::to_value(relca_core::diff::diff(&src, "A", "BPlan")).unwrap();
    fixture_match::compare_diff_report(&json!({}), &d).unwrap();
    assert!(
        d["from"]["report"].get("obligations").is_some(),
        "new report must carry ledger: {d}"
    );
    for side in ["from", "to"] {
        let mut bad = d.clone();
        bad[side]["report"]["obligations"][0]["outcomes"][0]["status"] = json!("fulfilled");
        assert!(fixture_match::compare_diff_report(&json!({}), &bad).is_err());
    }
    let c = serde_json::to_value(relca_core::compare::compare(
        &src,
        &["A", "BPlan"],
        relca_core::compare::Analysis::Exhaustive,
    ))
    .unwrap();
    fixture_match::compare_compare_report(&json!({}), &c).unwrap();
    for plan in 0..2 {
        assert!(c["plans"][plan]["report"].get("obligations").is_some());
        let mut bad = c.clone();
        bad["plans"][plan]["report"]["obligations"][0]["outcomes"][0]["status"] =
            json!("fulfilled");
        assert!(fixture_match::compare_compare_report(&json!({}), &bad).is_err());
    }
    let src = std::fs::read_to_string(root().join("ctx-waive/model.relca")).unwrap();
    let m = serde_json::to_value(relca_core::mutation::mutate(&src)).unwrap();
    fixture_match::validate_mutation_wire(&m).unwrap();
    assert!(m["original"].get("obligations").is_some());
    let mut bad = m;
    bad["original"]["obligations"][0]["outcomes"][0]["status"] = json!("fulfilled");
    assert!(fixture_match::validate_mutation_wire(&bad).is_err());
}
