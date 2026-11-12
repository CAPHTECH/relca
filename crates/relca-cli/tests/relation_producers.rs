#[path = "../../relca-core/tests/support/relation_producers.rs"]
mod acceptance;
#[test]
fn join_any_reads_branch_results() {
    acceptance::join_positive(run)
}
#[test]
fn join_any_same_producer_and_join_all_controls() {
    acceptance::join_controls(run)
}
#[test]
fn join_any_directed_intent() {
    acceptance::join_intent(run)
}
#[test]
fn producer_wire_refuses_non_preserve_annotations() {
    acceptance::wire_annotations()
}
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn run(src: &str, a: &str, b: &str) -> serde_json::Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join(".tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "k1-{}-{}.relca",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("diff")
        .arg(&path)
        .args(["--from", a, "--to", b])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let expected = match v["verdict"].as_str().unwrap() {
        "satisfied" => 0,
        "violated" => 1,
        "inconclusive" => 2,
        "invalid" => 3,
        "unsupported" => 4,
        "vacuous" => 5,
        _ => panic!("verdict"),
    };
    assert_eq!(out.status.code(), Some(expected), "{v}");
    v
}
#[test]
fn minimal_positive_and_reverse() {
    acceptance::positive(run)
}
#[test]
fn same_producer_and_other_zero_controls() {
    acceptance::controls(run)
}
#[test]
fn lexical_reads_forks_conditions_and_suffix() {
    acceptance::lexical(run)
}
#[test]
fn directed_intent_and_explicit_refusals() {
    acceptance::intent(run)
}
#[test]
fn failed_sides_and_mismatch_keep_relation_unchecked() {
    acceptance::failures(run)
}
#[test]
fn invoice_hand_nucleus() {
    for src in [
        include_str!("../../../fixtures/invoice-phase0-ai/model.relca"),
        include_str!("../../../examples/invoice/phase1.relca"),
    ] {
        acceptance::invoice(run, src)
    }
}
#[test]
fn closed_wire_valid_forms() {
    acceptance::wire_controls()
}
#[test]
fn closed_wire_single_faults() {
    acceptance::wire_faults()
}
