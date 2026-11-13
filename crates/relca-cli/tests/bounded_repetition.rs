//! Run only the new stage-1 behavioral/refusal acceptance against the real binary.
#[test]
fn stage_one_public_cli_acceptance() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::process::Command::new("python3")
        .args([
            "-B",
            "crates/relca-core/tests/bounded_repetition/cli.py",
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
