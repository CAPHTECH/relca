//! ADR 0071 test-first acceptance. Python fixes the wire by hand; this test also
//! opens the existing parser/lower boundary for every old-language substrate.
use serde_json::Value;
use std::{path::Path, process::Command};

#[test]
fn old_scope_substrates_parse_and_lower_without_executing_new_syntax() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("python3")
        .args(["-B", "crates/relca-cli/tests/contract_scope/inputs.py"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let values: Value = serde_json::from_slice(&out.stdout).unwrap();
    for (name, value) in values.as_object().unwrap() {
        let source = value["source"].as_str().unwrap();
        let ast = relca_core::syntax::parse(source)
            .unwrap_or_else(|d| panic!("{name}: old DSL must parse: {d:?}"));
        relca_core::lower::lower_configuration(&ast, value["configuration"].as_str().unwrap())
            .unwrap_or_else(|d| panic!("{name}: old DSL must lower: {d:?}"));
    }
}

#[test]
fn stage_one_contract_scope_real_cli() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("python3")
        .args([
            "-B",
            "crates/relca-cli/tests/contract_scope/cli.py",
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
