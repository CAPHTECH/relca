#[path = "../../relca-core/tests/support/evaluation_mutation_acceptance.rs"]
mod h;

fn run(case: &str) {
    let parent = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/evaluation-mutation-inputs");
    std::fs::create_dir_all(&parent).unwrap();
    let path = parent.join(format!("{}-{case}.relca", std::process::id()));
    std::fs::write(&path, h::source(case)).unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_relca"));
    command
        .arg("compare")
        .arg(path)
        .arg("--plans")
        .arg(h::plans(case).join(","))
        .arg("--mode")
        .arg(match h::analysis(case) {
            relca_core::compare::Analysis::Exhaustive => "exhaustive",
            relca_core::compare::Analysis::Simulation => "simulation",
        });
    if let Some(scenario) = h::scenario(case) {
        command.arg("--scenario").arg(scenario);
    }
    let out = command.output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(h::exit(case)),
        "{case}: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    // The existing CLI also renders warnings/errors to stderr; stdout is JSON.
    h::hand(case, &serde_json::from_slice(&out.stdout).unwrap());
}
h::mutation_acceptance_tests!();
