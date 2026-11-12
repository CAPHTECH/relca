//! CLI plumbing, not an oracle. Temporary files are confined to the worktree.
#![allow(dead_code)]
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
#[path = "ledger_expected.rs"]
pub mod oracle;
pub use oracle::{Spec, known};
static NEXT: AtomicUsize = AtomicUsize::new(0);
pub const SERIAL: &str = include_str!("../../../../fixtures/serial-time/model.relca");
pub const KEYS: &str = include_str!("../../../../fixtures/sim-keys/model.relca");
pub const PARALLEL: &str = include_str!("../../../../fixtures/config-diff-parallel/model.relca");
pub const VIA: &str = include_str!("../../../../fixtures/act-via/model.relca");
pub const RULE: &str = include_str!("../../../../fixtures/rule-diff/model.relca");
pub const MUTATE: &str = include_str!("../../../../fixtures/mut-evidence/model.relca");
pub const CONFORM_MODEL: &str = "crates/relca-cli/tests/snapshots/inputs/q3-conform-model.relca";
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
pub fn file(source: &str) -> PathBuf {
    let dir = root().join(".tmp").join(format!(
        "q3-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, source).unwrap();
    path
}
pub struct Output {
    pub value: Value,
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub exit: i32,
}
pub fn cli(args: &[&str]) -> Output {
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(root())
        .env("CARGO_NET_OFFLINE", "true")
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    let exit = out.status.code().expect("normal exit");
    assert!((0..=5).contains(&exit));
    assert!(out.stdout.ends_with(b"\n"));
    assert_eq!(out.stdout.iter().filter(|b| **b == b'\n').count(), 1);
    Output {
        value: serde_json::from_slice(&out.stdout).unwrap(),
        stdout: out.stdout,
        stderr: String::from_utf8(out.stderr).unwrap(),
        exit,
    }
}
pub fn invoke(command: &str, source: &str, flags: &[&str]) -> Output {
    let path = file(source);
    let mut args = vec![command, path.to_str().unwrap()];
    args.extend(flags);
    cli(&args)
}
pub fn ledger(v: &Value, s: &Spec) {
    oracle::validate(v, s).unwrap_or_else(|e| panic!("{e}"));
}
pub fn record(v: &Value, s: &Spec, errors: &mut Vec<String>) {
    if let Err(e) = oracle::validate(v, s) {
        errors.push(e);
    }
}
pub fn finish(errors: Vec<String>) {
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
pub fn serial_spec() -> Spec {
    Spec::finite("Check", &["Office"])
}
pub fn selected(process: &str, name: &str) -> Spec {
    let mut s = Spec::finite(process, &["Office"]);
    s.selection = oracle::selection(Some(name), None, process);
    s
}
pub fn mutation_spec() -> Spec {
    Spec::finite(
        "Flow",
        &["Invoice", "PurchaseOrder", "Reader", "Clerk", "Ledger"],
    )
}
pub fn rule_spec(name: &str, effective: &str) -> Spec {
    let mut s = Spec::finite("Flow", &["ApprovalRule", "Approver", "Ledger"]);
    s.selection = oracle::selection(Some(name), None, "Flow");
    s.actors["entries"][0]["rule"] = known(json!({"declared":"r1","effective":effective}));
    s
}
pub fn scalar(v: &Value, pointer: &str, want: Value) {
    assert_eq!(v.pointer(pointer), Some(&want), "{pointer}");
}
pub fn leaf_numbers(v: &Value, elapsed: u64, work: u64) {
    scalar(
        v,
        "/terminals/0/elapsed",
        json!({"min":elapsed,"max":elapsed}),
    );
    scalar(v, "/terminals/0/work", json!({"min":work,"max":work}));
    scalar(v, "/terminals/0/result_at", Value::Null);
}
pub fn no_new_diagnostics(v: &Value) {
    for d in v["diagnostics"].as_array().into_iter().flatten() {
        assert_ne!(
            d["code"], "U-LEDGER-0",
            "valid result must not gain a ledger failure"
        );
    }
}
