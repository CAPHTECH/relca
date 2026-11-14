//! ADR 0070: only the new real-CLI acceptance, with hand-authored expectations.
#[test]
fn stage_one_evidence_basis_cli() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::process::Command::new("python3")
        .args([
            "-B",
            "crates/relca-core/tests/evidence_basis/cli.py",
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
