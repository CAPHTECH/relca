//! Review regressions: ordinary check wire must remain entirely independent.
use relca_core::{explain, report};
use serde_json::Value;
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::path::PathBuf;

fn check_fixtures() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            let invocation = p.join("invocation.json");
            if !invocation.exists() {
                return true;
            }
            let v: Value =
                serde_json::from_str(&std::fs::read_to_string(invocation).unwrap()).unwrap();
            v["command"] == "check"
        })
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty());
    dirs
}

#[test]
fn a2_all_check_fixture_projections_are_exact() {
    for dir in check_fixtures() {
        let src = std::fs::read_to_string(dir.join("model.relca")).unwrap();
        let check = serde_json::to_value(report::check(&src)).unwrap();
        let mut explained = serde_json::to_value(explain::explain(&src)).unwrap();
        assert!(
            explained
                .as_object_mut()
                .unwrap()
                .remove("explanation")
                .is_some()
        );
        assert_eq!(
            ledger_shape::original_report(&explained),
            ledger_shape::original_report(&check),
            "{}",
            dir.display()
        );
    }
}

#[test]
fn a4_check_assumptions_never_contain_explanation_fields() {
    for dir in check_fixtures() {
        let src = std::fs::read_to_string(dir.join("model.relca")).unwrap();
        let _ = explain::explain(&src);
        let check = report::check(&src);
        assert!(
            check
                .assumptions
                .iter()
                .all(|a| !a.field.starts_with("explain.")),
            "{}",
            dir.display()
        );
    }
}
