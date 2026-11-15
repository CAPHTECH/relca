//! ADR 0076: real CLI acceptance with independently hand-authored expectations.
#[test]
fn stage_one_same_version_cli() {
    let out = std::process::Command::new("python3")
        .args([
            "-B",
            "crates/relca-core/tests/same_version/cli.py",
            "--binary",
            env!("CARGO_BIN_EXE_relca"),
        ])
        .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
