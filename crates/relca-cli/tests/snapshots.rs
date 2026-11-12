//! ADR 0055: current raw bytes; hand-computed tests remain the correctness oracle.
#[test]
fn current_cli_golden_bytes() {
    let output = std::process::Command::new("python3")
        .args([
            "-B",
            "scripts/snapshots.py",
            "--binary",
            env!("CARGO_BIN_EXE_relca"),
        ])
        .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    eprint!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
