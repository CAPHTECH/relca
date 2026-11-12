//! Implementation-owned real-CLI tests from orch's MJ2 specification.
#[path = "../../relca-core/tests/support/q6b_inputs.rs"]
mod input;
use relca_core::report::SimulationReport;
use serde_json::Value;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

fn invoke(clock: u64) -> (Value, i32) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join(".tmp").join(format!(
        "mj2-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let src = input::edit(
        &input::source("latent-ok"),
        "false -> { 1:ok }; true -> { 1:fail }",
        "false -> { 0.5:ok; 0.5:fail }; true -> { 0.5:ok; 0.5:fail }",
    );
    let src = input::edit(
        &src,
        "simulation replications 4 seed 1",
        "simulation replications 64 seed 1",
    );
    let src = if clock == 10 {
        src
    } else {
        input::edit(&src, "clock 10min", &format!("clock {clock}min"))
    };
    let path = dir.join("model.relca");
    std::fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("simulate")
        .arg(path)
        .current_dir(&root)
        .env("CARGO_NET_OFFLINE", "true")
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert_eq!(out.stdout.iter().filter(|b| **b == b'\n').count(), 1);
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let expected: String = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(String::from_utf8(out.stderr).unwrap(), expected);
    (v, out.status.code().unwrap())
}
#[test]
fn cut_kernel_degradation_is_inconclusive_exit_two_and_decodes() {
    let (v, exit) = invoke(3);
    assert_eq!(v["bounds"]["truncated"], true);
    assert!(
        v["case_outcomes"].to_string().contains("done"),
        "sampled witness must exist"
    );
    assert_eq!((v["verdict"].as_str(), exit), (Some("inconclusive"), 2));
    serde_json::from_value::<SimulationReport>(v).expect("CLI's own wire decodes");
}
#[test]
fn generous_clock_kernel_degradation_stays_satisfied_exit_zero() {
    let (v, exit) = invoke(10);
    assert_eq!(v["bounds"]["truncated"], false);
    assert_eq!((v["verdict"].as_str(), exit), (Some("satisfied"), 0));
    serde_json::from_value::<SimulationReport>(v).unwrap();
}
