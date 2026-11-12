//! Retained independent lexer controls and hand migration oracles, no old build.
#[test]
fn independent_scanners_and_hand_migrations() {
    let output = std::process::Command::new("python3")
        .args([
            "-B",
            "scripts/source-scanner-controls.py",
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
