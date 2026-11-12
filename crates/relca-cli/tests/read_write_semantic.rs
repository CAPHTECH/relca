#[test]
fn current_raw_k5_rows_match_the_hand_source_inventory() {
    let result = std::process::Command::new("python3")
        .args([
            "-B",
            "crates/relca-cli/tests/read_write_semantic.py",
            "--self-test",
            "--binary",
            env!("CARGO_BIN_EXE_relca"),
        ])
        .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
