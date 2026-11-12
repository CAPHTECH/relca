//! Cross-command projection is not claimed as independent replay verification.
//! Replay is exercised through the public explain entry point's verified output.
use relca_core::{explain, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::path::Path;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}

#[test]
fn explain_projection_is_exact_and_does_not_mutate_check_before_or_after() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("explain-")
        })
        .collect();
    dirs.sort();
    assert_eq!(dirs.len(), 12);
    for d in dirs {
        let src = std::fs::read_to_string(d.join("model.relca")).unwrap();
        let before = serde_json::to_value(report::check(&src)).unwrap();
        let mut explained = serde_json::to_value(explain::explain(&src)).unwrap();
        let ex = explained
            .as_object_mut()
            .unwrap()
            .remove("explanation")
            .expect("not stub");
        let after = serde_json::to_value(report::check(&src)).unwrap();
        assert_eq!(before, after);
        assert_eq!(
            ledger_shape::original_report(&before),
            ledger_shape::original_report(&explained)
        );
        for c in ex["counterexamples"].as_array().unwrap() {
            assert_eq!(c["replay"]["status"], "verified");
        }
    }
}

#[test]
fn probability_reachable_and_static_contest_never_become_single_cause_paths() {
    for name in [
        "prob-mdp.broken-tight-upper",
        "resp-contest.broken-no-authority",
        "explain-deadlock.broken-zero-capacity",
    ] {
        let src = fixture(name);
        let v = serde_json::to_value(explain::explain(&src)).unwrap();
        let ex = &v["explanation"];
        assert!(
            ex["counterexamples"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["id"] == "diagnostic:W-DEADLOCK-0")
        );
        let reason = if name.starts_with("prob-") {
            "probabilistic_property"
        } else if name.starts_with("resp-") {
            "static_property"
        } else {
            "non_path_property"
        };
        assert!(
            ex["omitted"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["verdict"] == "violated" && o["reason"] == reason)
        );
        assert_eq!(v["verdict"], "violated");
    }
}

#[test]
fn satisfied_vacuous_inconclusive_and_unsupported_never_gain_candidates() {
    for name in [
        "explain-response",
        "contract-vacuous",
        "contract-response.broken-short-clock",
        "exec-anonymous.broken-no-executor",
    ] {
        let src = fixture(name);
        let v: Value = serde_json::to_value(explain::explain(&src)).unwrap();
        assert_eq!(v["explanation"]["counterexamples"], json!([]));
        assert!(
            v["explanation"]["omitted"]
                .as_array()
                .unwrap()
                .iter()
                .all(|o| !o["id"].as_str().unwrap().starts_with("contract "))
        );
    }
}
