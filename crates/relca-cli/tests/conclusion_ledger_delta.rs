//! One-transition delta audit; the ordinary raw snapshot comparator is unchanged.
use std::{path::PathBuf, process::Command};
fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/ledger_delta.py")
}
#[test]
fn green_raw_delta_calibration_rejects_ledger_and_three_channel_faults() {
    let out = Command::new("python3")
        .arg("-B")
        .arg(script())
        .arg("--calibrate")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "delta calibration: positive=2 rejected_faults=7\n"
    );
}
#[test]
fn all_existing_snapshot_jobs_allow_only_ledger_insertions_or_current_exact_bytes() {
    let out = Command::new("python3")
        .arg("-B")
        .arg(script())
        .args(["--binary", env!("CARGO_BIN_EXE_relca")])
        .env("CARGO_NET_OFFLINE", "true")
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
