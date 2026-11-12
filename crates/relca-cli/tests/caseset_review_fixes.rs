//! ADR0035 boundary-review regressions. Old-syntax inputs must keep the
//! pre-ADR0035 binary's stdout, stderr and exit byte for byte (duties whose
//! condition reads a result binding never disappear, unknown executor members
//! stay unsupported). The new atomic context keeps the old classification.
use serde_json::Value;
use std::{fs, process::Command};
#[path = "support/ledger_shape.rs"]
mod ledger_shape;

fn rows() -> Vec<Value> {
    let v: Value = serde_json::from_str(include_str!("caseset_review_fixes.json")).unwrap();
    v["rows"].as_array().unwrap().clone()
}

fn run(row: &Value) -> (Vec<u8>, Vec<u8>, i32) {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!(
            "p{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
        .join("caseset-review-fixes");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}.relca", row["name"].as_str().unwrap()));
    fs::write(&path, row["source"].as_str().unwrap()).unwrap();
    let mut args: Vec<String> = row["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_owned())
        .collect();
    args.insert(1, path.to_str().unwrap().to_owned());
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(&args)
        .output()
        .unwrap();
    (out.stdout, out.stderr, out.status.code().unwrap())
}

#[test]
fn closed_row_set() {
    let names: Vec<_> = rows()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        names,
        [
            "duty-selected",
            "duty-unselected",
            "formula-selected",
            "formula-unselected",
            "executor-start",
            "executor-finish",
            "atomic-unknown"
        ]
    );
}

#[test]
fn old_syntax_outputs_match_the_pre_adr0035_binary_byte_for_byte() {
    for row in rows().iter().filter(|r| r.get("stdout").is_some()) {
        let name = row["name"].as_str().unwrap();
        let (stdout, stderr, exit) = run(row);
        let mut spec = ledger_shape::Spec::prelower("exhaustive", false);
        if row["args"].as_array().unwrap().len() == 3 {
            spec.selection["requested"]["configuration"] = row["args"][2].clone();
        }
        assert_eq!(exit, row["exit"].as_i64().unwrap() as i32, "{name}");
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            ledger_shape::extend_raw(row["stdout"].as_str().unwrap(), &spec),
            "{name} stdout"
        );
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            row["stderr"].as_str().unwrap(),
            "{name} stderr"
        );
    }
}

#[test]
fn unselected_binding_duty_is_refused_not_silently_satisfied() {
    for name in ["duty-unselected", "formula-unselected"] {
        let row = rows().into_iter().find(|r| r["name"] == name).unwrap();
        let (stdout, _, exit) = run(&row);
        let v: Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(exit, 4, "{name}");
        assert_eq!(v["verdict"], "unsupported", "{name}");
        assert_eq!(v["diagnostics"][0]["code"], "U-SEM-0", "{name}");
        assert_eq!(
            v["diagnostics"][0]["message"], "probability conditions require state",
            "{name}"
        );
    }
}

#[test]
fn unknown_word_inside_atomic_is_unsupported_not_malformed() {
    let row = rows()
        .into_iter()
        .find(|r| r["name"] == "atomic-unknown")
        .unwrap();
    let (stdout, _, exit) = run(&row);
    let v: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(exit, row["exit"].as_i64().unwrap() as i32);
    assert_eq!(v["verdict"], row["verdict"]);
    let codes: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].clone())
        .collect();
    assert_eq!(codes, row["codes"].as_array().unwrap().clone());
    let source = row["source"].as_str().unwrap();
    let start = v["diagnostics"][0]["span"]["start"].as_u64().unwrap() as usize;
    assert!(source[start..].starts_with("foo"), "{v}");
}
