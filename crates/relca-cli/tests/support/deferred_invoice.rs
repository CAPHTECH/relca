//! Explicit no-defer arithmetic controls; never restore or normalize output.
#![allow(dead_code)]
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

pub fn without_defer(src: &str) -> String {
    assert!(
        src.lines()
            .any(|line| line.trim_start().starts_with("defer "))
    );
    src.lines()
        .filter(|line| !line.trim_start().starts_with("defer "))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

pub fn numeric_control(src: &str, command: &str, extra: &[&str]) -> Output {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "invoice-no-defer-{}-{}.relca",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, without_defer(src)).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(&path)
        .args(extra)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    out
}

pub fn assert_deferred(report: &Value) {
    super::fixture_match::validate_deferred_metrics_wire(report).unwrap();
    assert_eq!(
        report["metrics"]["overall"]["work"],
        json!({"min":null,"max":null,"reason":"not_configured"})
    );
    assert!(
        report["cut"]["checked"]
            .as_array()
            .unwrap()
            .contains(&json!("deferred_metrics"))
    );
    if report["quality"].is_object() {
        assert_eq!(
            report["quality"]["cohort"][0]["value"],
            json!({"lower":null,"upper":null,"basis":null,"reason":"not_configured"})
        );
    }
}
