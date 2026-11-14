//! The fixed scope suite's Old process does not call the authority target.
//! This complementary control keeps that lexical call, isolating scope filtering.
use std::{path::Path, process::Command};

#[test]
fn excluded_called_authority_is_not_a_mutation_candidate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("python3")
        .args([
            "-B",
            "crates/relca-cli/tests/contract_scope/candidates.py",
            "--binary",
            env!("CARGO_BIN_EXE_relca"),
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
fn scoped_static_refusals_do_not_require_simulation_settings() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("python3")
        .args([
            "-B",
            "crates/relca-cli/tests/contract_scope/entrypoints.py",
            "--binary",
            env!("CARGO_BIN_EXE_relca"),
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
