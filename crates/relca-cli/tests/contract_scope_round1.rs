//! Independent scope-route and disclosure controls, without changing fixed acceptance.
use std::{path::Path, process::Command};

#[test]
fn scope_routes_and_intent_round_one() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("python3")
        .args([
            "-B",
            "crates/relca-cli/tests/contract_scope/round1.py",
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
