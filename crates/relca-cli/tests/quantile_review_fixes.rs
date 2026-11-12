//! Actual commands exercise the driver boundary; inline, no population opt-outs.
#[path = "../../relca-core/tests/support/quantile_review_inputs.rs"]
mod input;
use serde_json::Value;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

fn run(source: &str, sampled: bool) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("target").join(format!(
        "c7-review-{}-{}.relca",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(if sampled { "simulate" } else { "check" })
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let verdict: relca_core::Verdict = serde_json::from_value(v["verdict"].clone()).unwrap();
    assert_eq!(out.status.code(), Some(verdict.exit_code()));
    v
}

#[test]
fn m1_real_check_command_keeps_closed_case_a_miss() {
    let v = run(input::DESK, false);
    assert_eq!(v["quality"]["quantiles"], input::rows(false, false, true));
    assert_eq!(v["quality"]["properties"][0]["cases"][0]["min"], "0");
    assert_eq!(v["quality"]["properties"][0]["cases"][0]["max"], "0");
}

#[test]
fn m1_real_simulate_command_keeps_closed_case_a_miss() {
    let v = run(input::DESK, true);
    assert_eq!(v["quality"]["quantiles"], input::rows(true, false, true));
    assert_eq!(v["quality"]["properties"][0]["cases"][0]["hits"], 0);
}

#[test]
fn m1_real_simulate_command_ignores_the_unselected_early_successor() {
    let fixed = input::edit(
        input::SELECTED,
        "duration {2min:0.5, 4min:0.5}",
        "duration fixed 4min",
    );
    for s in [fixed.as_str(), input::SELECTED] {
        assert_eq!(run(s, true)["quality"]["quantiles"], input::selected_rows());
    }
}
