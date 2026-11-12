#[path = "../../relca-core/tests/support/evaluation_review_acceptance.rs"]
mod h;
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

fn run(id: &str, source: &str, args: &[&str]) -> Output {
    let parent =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/evaluation-review-inputs");
    fs::create_dir_all(&parent).unwrap();
    let path = parent.join(format!("{}-{id}.relca", std::process::id()));
    fs::write(&path, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(args[0])
        .arg(&path)
        .args(&args[1..])
        .output()
        .unwrap()
}

fn legacy(id: &str, source: &str, message: Option<&str>) {
    let out = run(id, source, &["check"]);
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    if let Some(message) = message {
        assert_eq!(v["verdict"], "unsupported");
        assert_eq!(v["diagnostics"][0]["code"], "U-SYN-0");
        assert_eq!(v["diagnostics"][0]["message"], message);
        let start = source.find("compare").unwrap();
        assert_eq!(
            v["diagnostics"][0]["span"],
            serde_json::json!({"start":start,"end":start+7})
        );
    } else {
        assert_eq!(v["verdict"], "satisfied", "compare remains a legal binding");
    }
    assert_eq!(
        out.status.code(),
        Some(if message.is_some() { 4 } else { 0 })
    );
}

#[test]
fn old_scenario_compare_keeps_every_baseline_byte_and_span() {
    legacy(
        "scenario",
        include_str!("support/evaluation_old_scenario.relca"),
        Some("unsupported syntax: scenario item"),
    );
}
#[test]
fn old_model_compare_keeps_every_baseline_byte_and_span() {
    legacy(
        "model",
        include_str!("support/evaluation_old_model.relca"),
        Some("unsupported syntax: compare"),
    );
}
#[test]
fn old_process_compare_binding_keeps_every_baseline_byte() {
    legacy(
        "binding",
        include_str!("support/evaluation_old_binding.relca"),
        None,
    );
}
#[test]
fn only_bracketed_scenario_compare_gets_the_new_scope_rejection() {
    let source = include_str!("support/evaluation_old_scenario.relca")
        .replace("compare A B", "compare [A, B]");
    let out = run("bracketed-scenario", &source, &["check"]);
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(v["diagnostics"][0]["code"], "U-SYN-0");
    assert_eq!(
        v["diagnostics"][0]["message"],
        "unsupported syntax: compare"
    );
    let start = source.find("compare").unwrap();
    assert_eq!(
        v["diagnostics"][0]["span"],
        serde_json::json!({"start":start,"end":start+7})
    );
}

fn cut(case: &str) {
    let out = run(
        case,
        &h::cut_source(case),
        &["compare", "--plans", "AsIs,ToBeP", "--mode", "exhaustive"],
    );
    assert_eq!(out.status.code(), Some(3));
    h::cut_hand(case, &serde_json::from_slice(&out.stdout).unwrap());
}
#[test]
fn declared_and_interplan_cut_mismatches_both_survive() {
    cut("cross");
}
#[test]
fn interplan_cut_mismatch_without_declared_cut_keeps_the_old_pair() {
    cut("no-declared-cut");
}
#[test]
fn declared_cut_mismatch_does_not_fabricate_an_interplan_pair() {
    cut("declared-only");
}

fn weighted(block: &str, word: &str) {
    let out = run(
        &format!("{block}-{word}"),
        &h::weighted_source(block, word),
        &["compare", "--plans", "Check,Fast", "--mode", "exhaustive"],
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        out.status.code(),
        Some(4),
        "{block}/{word}: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(v["verdict"], "unsupported");
    assert!(v.get("evaluation").is_none());
    for p in v["plans"].as_array().unwrap() {
        assert!(
            p["report"]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SYN-0")
        );
        assert!(
            !p["report"]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-SYN-0")
        );
    }
}
#[test]
fn hard_row_weight_is_unsupported_not_malformed() {
    weighted("hard", "weight");
}
#[test]
fn hard_row_score_is_unsupported_not_malformed() {
    weighted("hard", "score");
}
#[test]
fn objective_row_weight_is_unsupported_not_malformed() {
    weighted("objectives", "weight");
}
#[test]
fn objective_row_score_is_unsupported_not_malformed() {
    weighted("objectives", "score");
}
