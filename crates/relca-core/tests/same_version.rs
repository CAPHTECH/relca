//! ADR 0076: engine and independent reference must each meet the hand verdict.
use relca_core::{Verdict, engine, ir, lower, reference, syntax};
use serde_json::Value;

fn cases() -> Value {
    let out = std::process::Command::new("python3")
        .args(["-B", "crates/relca-core/tests/same_version/inputs.py"])
        .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).unwrap()
}

fn verify(source: &str, expected: &str) {
    let ast = syntax::parse(source).unwrap_or_else(|e| panic!("parse: {e:?}"));
    let m = lower::lower(&ast).unwrap_or_else(|e| panic!("lower: {e:?}"));
    let want: Verdict = serde_json::from_value(Value::String(expected.into())).unwrap();
    let actual = engine::explore(&m);
    let independent = reference::explore(&m);
    assert_eq!(independent.stop, reference::StopReason::Complete);
    for (name, out) in [("engine", &actual), ("reference", &independent.outcome)] {
        let rows: Vec<_> = out
            .properties
            .iter()
            .filter(|p| matches!(p.property, ir::Property::Contract(_)))
            .collect();
        assert_eq!(rows.len(), 1, "{name}: exactly one contract");
        assert_eq!(rows[0].verdict, want, "{name}: independent hand verdict");
        assert!(!out.truncated, "{name}: no computational cut");
        assert!(out.diagnostics.iter().all(|d| !d.code.starts_with("U-")));
    }
    assert_eq!(actual.properties, independent.outcome.properties);
}

fn all_cases(field: &str) {
    let mut failed = Vec::new();
    for (name, row) in cases().as_object().unwrap() {
        eprintln!("CASE {field} {name}");
        let result = std::panic::catch_unwind(|| {
            let mut src = row[field].as_str().unwrap().to_owned();
            if std::env::var_os("RELCA_VERSION_SUBSTRATE_FAULT").is_some() {
                src = src.replace("type Doc =", "type Doc ??? =");
            }
            verify(&src, row["expected"].as_str().unwrap());
        });
        if result.is_err() {
            failed.push(name.clone());
        }
    }
    assert!(failed.is_empty(), "ADR 0076 {field} failures: {failed:?}");
}

#[test]
fn old_dsl_witnesses_match_hand_verdicts_in_both_interpreters() {
    all_cases("witness");
}

#[test]
fn stage_one_same_version_matches_hand_verdicts_in_both_interpreters() {
    all_cases("source");
}

#[test]
fn sampled_version_witnesses_match_the_independent_interpreter() {
    let cases = cases();
    for (name, broken) in [("a2_reuse", false), ("a2_reused", true)] {
        let src = cases[name]["source"]
            .as_str()
            .unwrap()
            .replace("result nondet { pass, fail }", "result nondet { pass }");
        let close = src.rfind('}').unwrap();
        let src = format!(
            "{} simulation replications 4 seed 7 confidence 0.5 }}",
            &src[..close]
        );
        let m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
        let actual = engine::simulate(&m);
        let independent = reference::simulate(&m);
        assert_eq!(independent.stop, reference::StopReason::Complete);
        assert_eq!(actual.replications.len(), 4);
        assert_eq!(actual.replications, independent.outcome.replications);
        assert!(actual.replications.iter().all(|replication| {
            replication
                .cases
                .iter()
                .all(|case| case.violations.contains(&true) == broken)
        }));
    }
}
