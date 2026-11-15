//! Real CLI acceptance. Input and hand calculations are shared, not semantics.
fn run(class: &str) {
    let output = std::process::Command::new("python3")
        .args([
            "-B",
            "crates/relca-core/tests/record_provenance/cli.py",
            "--binary",
            env!("CARGO_BIN_EXE_relca"),
            class,
        ])
        .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn hand_examples_and_broken_pairs() {
    run("Behavior");
}
#[test]
fn static_registration_and_combination_reasons() {
    run("Boundaries");
}
#[test]
fn observation_responsibility_and_common_error_boundaries() {
    run("Observation");
}
#[test]
fn command_entry_points_and_evidence_witnesses() {
    run("Commands");
}
#[test]
fn old_dsl_hand_values_and_command_scaffolds() {
    run("Substrate");
}
#[test]
fn old_contextual_names_and_source_record_semantics() {
    run("Preservation");
}
#[test]
fn no_registration_keeps_all_current_golden_channels() {
    run("Golden");
}
