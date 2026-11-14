//! ADR 0070: a hand verdict is checked against both independent interpreters.
//! The Python helper emits input text and literal expectations, never executes Relca.
use relca_core::{Verdict, engine, ir, lower, reference, syntax};
use serde_json::Value;
use std::process::Command;

fn cases() -> Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("python3")
        .args(["-B", "crates/relca-core/tests/evidence_basis/inputs.py"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn model(source: &str) -> ir::Model {
    let ast = syntax::parse(source).unwrap_or_else(|e| panic!("parse: {e:?}"));
    lower::lower(&ast).unwrap_or_else(|e| panic!("lower: {e:?}"))
}

fn verify(source: &str, expected: &str) {
    let m = model(source);
    let want: Verdict = serde_json::from_value(Value::String(expected.into())).unwrap();
    let engine = engine::explore(&m);
    let reference = reference::explore(&m);
    assert_eq!(reference.stop, reference::StopReason::Complete);
    for (name, outcome) in [("engine", &engine), ("reference", &reference.outcome)] {
        let rows: Vec<_> = outcome
            .properties
            .iter()
            .filter(|p| matches!(p.property, ir::Property::Contract(_)))
            .collect();
        assert_eq!(rows.len(), 1, "{name}: exactly one contract");
        assert_eq!(rows[0].verdict, want, "{name}: independent hand verdict");
        assert!(!outcome.truncated, "{name}: no computational cut");
        assert!(
            !outcome.diagnostics.iter().any(|d| d.code.starts_with("U-")),
            "{name}: no semantic abort disguised as a verdict"
        );
    }
    // This compares observations after both met the independently fixed verdict.
    assert_eq!(engine.properties, reference.outcome.properties);
}

#[test]
fn old_dsl_witnesses_parse_lower_and_match_both_interpreters() {
    for (name, row) in cases().as_object().unwrap() {
        eprintln!("OLD {name}");
        let mut source = row["witness"].as_str().unwrap().to_owned();
        // Explicitly opt-in temporary harness fault, for pre-implementation
        // GREEN calibration. No src file or persistent fixture is altered.
        if std::env::var_os("RELCA_EVIDENCE_SUBSTRATE_FAULT").is_some() {
            source = source.replace("type V =", "type V ??? =");
        }
        verify(&source, row["expected"].as_str().unwrap());
    }
}

#[test]
fn stage_one_each_hand_case_meets_engine_and_independent_reference() {
    // Collect each failure rather than allowing the first unsupported syntax
    // to hide the remaining cases. Catching does not convert RED to success.
    let mut failed = Vec::new();
    for (name, row) in cases().as_object().unwrap() {
        eprintln!("NEW {name}");
        let result = std::panic::catch_unwind(|| {
            verify(
                row["source"].as_str().unwrap(),
                row["expected"].as_str().unwrap(),
            );
        });
        if result.is_err() {
            failed.push(name.clone());
        }
    }
    assert!(failed.is_empty(), "ADR 0070 RED cases: {failed:?}");
}
