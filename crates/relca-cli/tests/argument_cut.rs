use serde_json::Value;
use std::{
    path::PathBuf,
    process::{Command, Output},
};
fn run(command: &str, options: &[&str]) -> Output {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/support/configuration_operations.relca");
    Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(path)
        .args(options)
        .output()
        .unwrap()
}
#[test]
fn unparsed_input_never_claims_an_undeclared_cut_or_warns_about_it() {
    for command in [
        "check",
        "simulate",
        "explain",
        "mutate",
        "compare",
        "diff",
        "conform",
        "sensitivity",
    ] {
        let out = run(command, &["--unknown", "value"]);
        assert_eq!(out.status.code(), Some(3), "{command}");
        assert!(
            !String::from_utf8_lossy(&out.stderr).contains("W-CUT-0"),
            "{command}"
        );
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        if let Some(cut) = v.get("cut") {
            assert_eq!(cut, "unresolved", "{command}");
        }
        assert!(
            !v["diagnostics"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|d| d["code"] == "W-CUT-0")
        );
    }
}
#[test]
fn missing_configuration_explains_the_available_selector() {
    let out = run("explain", &["--scenario", "Late"]);
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("E-CONF-0"));
    assert!(stderr.contains("--configuration"), "{stderr}");
}
#[test]
fn a_read_model_without_a_cut_still_warns() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/serial-time/model.relca");
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("check")
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stderr).contains("W-CUT-0"));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["cut"], "undeclared");
}
