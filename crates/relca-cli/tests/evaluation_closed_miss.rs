#[path = "../../relca-core/tests/support/evaluation_closed_miss_acceptance.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::Value;
use std::{
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn run(source: &str, plans: &[&str], sampled: bool) -> Value {
    static ID: AtomicUsize = AtomicUsize::new(0);
    let dir = test_tmp::root().join(format!(
        "relca-c7d06-{}-{}",
        std::process::id(),
        ID.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("compare")
        .arg(&path)
        .args([
            "--plans",
            &plans.join(","),
            "--mode",
            if sampled { "simulation" } else { "exhaustive" },
        ])
        .output()
        .unwrap();
    assert!(out.stderr.is_empty(), "real CLI stderr: {:?}", out.stderr);
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let verdict: relca_core::Verdict = serde_json::from_value(v["verdict"].clone()).unwrap();
    assert_eq!(
        out.status.code(),
        Some(verdict.exit_code()),
        "real CLI exit"
    );
    fixture_match::validate_compare_wire(&v).unwrap();
    v
}
