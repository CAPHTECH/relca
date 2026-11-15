use serde_json::{Value, json};
use std::{path::Path, process::Command};

#[test]
fn staggered_cases_do_not_reuse_another_cases_last_examination() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args([
            "check",
            "crates/relca-core/tests/same_version_round1/mixed-cases.relca",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(value["verdict"], "violated");
    assert_eq!(value["bounds"]["truncated"], false);
    let row = value["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "contract V #1 same_version")
        .unwrap();
    assert_eq!(
        row,
        &json!({"id":"contract V #1 same_version", "verdict":"violated", "cases":[
        {"case_id":1,"arrival":0,"verdict":"satisfied"},
        {"case_id":2,"arrival":10,"verdict":"violated"}]})
    );
}

#[test]
fn rejected_examinations_have_no_examined_version_in_real_explanations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["atomic-rejected-check", "version-check-rejected-check"] {
        let model = format!("crates/relca-core/tests/same_version_round1/{name}.relca");
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(["explain", &model])
            .current_dir(&root)
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            out.status.code(),
            Some(1),
            "{name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(value["verdict"], "violated", "{name}");
        let cx = value["explanation"]["counterexamples"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "contract V #1 same_version")
            .unwrap();
        assert_eq!(cx["status"], "verified", "{name}: {cx}");
        assert_eq!(cx["replay"]["status"], "verified");
        assert_eq!(
            cx["version"],
            json!({"binding":"p", "capability":"Approver.approve", "parameter":"d", "version":"d", "examined":null})
        );
    }
}
