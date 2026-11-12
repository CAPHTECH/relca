#[path = "../../relca-core/tests/support/instance_diff_boundary.rs"]
mod hand;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn run(source: &str) -> (Value, i32) {
    static ID: AtomicUsize = AtomicUsize::new(0);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = root.join("target/u12-instance-inputs");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!(
        "{}-{}.relca",
        std::process::id(),
        ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("diff")
        .arg(path)
        .args(["--from", "Left", "--to", "Right"])
        .current_dir(root)
        .output()
        .unwrap();
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    // main.rs emits both side reports, then diff diagnostics. Keep the prefixes
    // and duplicates belonging to different sides.
    let mut stderr = String::new();
    for (side, ds) in [
        ("from", &r["from"]["report"]["diagnostics"]),
        ("to", &r["to"]["report"]["diagnostics"]),
        ("diff", &r["diagnostics"]),
    ] {
        for d in ds.as_array().unwrap() {
            stderr.push_str(&format!(
                "{side}: {}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            ));
        }
    }
    assert_eq!(out.stderr, stderr.as_bytes());
    (r, out.status.code().unwrap())
}
#[test]
fn real_diff_hand_values_cover_shared_per_case_and_genuine_rule_amendment() {
    for per_case in [false, true] {
        for amended in [false, true] {
            let (r, exit) = run(&hand::source(per_case, amended));
            hand::positive(&r, per_case, amended, exit).unwrap();
        }
    }
}
#[test]
fn real_diff_refuses_every_override_with_exact_diagnostics_and_exit() {
    for case in hand::REJECTIONS {
        let (r, exit) = run(&case.source());
        hand::rejected(&r, case, exit).unwrap_or_else(|e| panic!("{}: {e}", case.name()));
    }
}
#[test]
fn raw_cli_instance_and_diagnostic_comparators_reject_single_faults() {
    let (r, exit) = run(&hand::source(true, false));
    hand::positive(&r, true, false, exit).unwrap();
    for side in ["from", "to"] {
        let mut broken = r.clone();
        let a = broken[side]["report"]["assumptions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|a| a["field"] == "Invoice.instance")
            .unwrap();
        a["value"] = json!("shared(default)");
        assert!(hand::positive(&broken, true, false, exit).is_err());
    }
    for case in hand::REJECTIONS {
        let (r, exit) = run(&case.source());
        hand::rejected(&r, case, exit).unwrap();
        for side in ["from", "to"] {
            for field in ["code", "message", "severity", "span"] {
                let mut broken = r.clone();
                broken[side]["report"]["diagnostics"][0][field] = if field == "span" {
                    json!({"start":999999,"end":1000000})
                } else {
                    json!("corrupted")
                };
                assert!(hand::rejected(&broken, case, exit).is_err());
            }
        }
        assert!(hand::rejected(&r, case, 0).is_err());
    }
}
