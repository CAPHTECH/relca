//! E9b publication/refusal coverage, not per_run/duration-given execution.
#[path = "support/sensitivity_harness.rs"]
mod h;
#[path = "../../relca-core/tests/support/sensitivity_oracle.rs"]
mod o;
use serde_json::json;
#[test]
fn red_verify_has_three_classes_without_reading_any_source() {
    let a = h::cli(&["verify", "missing-q7t-model.relca"]);
    let b = h::cli(&["verify", h::data().join("base.relca").to_str().unwrap()]);
    assert_eq!(a.exit, 4);
    assert_eq!(a.stdout, b.stdout);
    assert_eq!(a.stderr, b.stderr);
    o::uncertainty(
        &a.value,
        [
            ("unavailable", Some("input_unavailable")),
            ("not_applicable", Some("no_analysis")),
            ("unavailable", Some("input_unavailable")),
        ],
    )
    .unwrap();
}
#[test]
fn red_conform_marks_trace_only_and_does_not_infer_future_probability() {
    let out = h::cli(&[
        "conform",
        "crates/relca-cli/tests/snapshots/inputs/q3-conform-model.relca",
        "--log",
        "crates/relca-cli/tests/snapshots/inputs/q3-conform-ok.jsonl",
        "--scope",
        "event-contracts",
    ]);
    assert_eq!(out.exit, 0);
    o::uncertainty(
        &out.value,
        [
            ("not_applicable", Some("observable_trace_only")),
            ("not_applicable", Some("observable_trace_only")),
            ("available", None),
        ],
    )
    .unwrap();
}
#[test]
fn red_common_refusal_keeps_unavailable_classes_and_nonzero_exit() {
    let out = h::cli(&["unknown-q7t-command"]);
    assert_eq!(out.exit, 3);
    o::uncertainty(
        &out.value,
        [
            ("unavailable", Some("input_unavailable")),
            ("not_applicable", Some("no_analysis")),
            ("unavailable", Some("input_unavailable")),
        ],
    )
    .unwrap();
}
#[test]
fn red_ledger_links_uncertainty_to_the_same_source_context() {
    let out = h::cli(&["check", h::data().join("base.relca").to_str().unwrap()]);
    o::uncertainty(
        &out.value,
        [
            ("available", None),
            ("not_applicable", Some("exhaustive_analysis")),
            ("available", None),
        ],
    )
    .unwrap();
    let subjects = out.value["ledger"]["subjects"].as_array().unwrap();
    assert!(
        subjects
            .iter()
            .any(|s| s["target"] == "/uncertainty" && s["source_ledgers"] == json!([]))
    );
    assert!(
        out.value["ledger"]["derivation"]["components"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "/uncertainty/rules" && c["version"] == "uncertainty-p2-1")
    );
}
