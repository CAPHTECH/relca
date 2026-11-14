//! ADR 0069: the raw new syntax reaches the actual public command boundary.
fn run(class: &str) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::process::Command::new("python3")
        .args([
            "-B",
            "crates/relca-core/tests/merge_if/cli.py",
            "--binary",
            env!("CARGO_BIN_EXE_relca"),
            class,
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn hand_values_m1_through_m5() {
    run("Behavior");
}

#[test]
fn connected_explain_simulate_quality_and_combinations() {
    run("Connected");
}

#[test]
fn static_broken_pairs() {
    run("Refusals");
}

#[test]
fn command_gates_have_merge_specific_reasons() {
    run("Gates");
}

#[test]
fn current_cli_golden_bytes_and_old_merge_inputs() {
    run("Preservation");
}

#[test]
fn independent_fault_controls() {
    run("Calibration");
}

#[test]
fn fraction_arithmetic_and_old_dsl_scaffolds() {
    run("Prerequisites");
}
