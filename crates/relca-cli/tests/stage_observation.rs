//! ADR 0074: handwritten stage-observation acceptance, without production edits.
fn run(class: &str) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::process::Command::new("python3")
        .args([
            "-B",
            "crates/relca-cli/tests/stage_observation/cli.py",
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
fn hand_values_s1_through_s5() {
    run("Behavior");
}
#[test]
fn responsibility_and_control_boundaries() {
    run("Boundaries");
}
#[test]
fn mutation_kind_nine_and_comparison_unchecked() {
    run("Commands");
}
#[test]
fn old_dsl_hand_times_and_static_refusal_substrates() {
    run("Substrate");
}
#[test]
fn old_inputs_preserve_hand_values_and_independence() {
    run("Preservation");
}
#[test]
fn review_page_stage_and_legacy_disclosure() {
    run("Page");
}
#[test]
fn independent_fault_controls_f1_through_f7() {
    run("Calibration");
}
