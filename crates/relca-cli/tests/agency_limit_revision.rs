#[path = "../../relca-core/tests/support/agency_limit_revision_inputs.rs"]
mod input;
use serde_json::Value;
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

fn run(source: &str, scale: usize) -> Output {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join(".tmp/agency-limit-revision");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "{}-{}.relca",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("check")
        .arg(path)
        .args(["--agency-limit-scale", &scale.to_string()])
        .current_dir(root)
        .output()
        .unwrap()
}
fn wire(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap()
}
fn warnings(v: &Value) -> Vec<&Value> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-AGY-5")
        .collect()
}

#[test]
fn merge_projection_uses_the_recorded_scale() {
    let source = input::merge_or(12);
    let low = run(&source, 1);
    let low = wire(&low);
    assert_eq!(low["agency"]["reason"], "derivation_limit");
    assert_eq!(low["agency"]["limit"]["limit"], 1_000_000);
    let high = run(&source, 1000);
    assert_eq!(high.status.code(), Some(0));
    let high = wire(&high);
    assert_eq!(
        high["ledger"]["execution"]["limits"]["agency_limit_scale"]["value"],
        1000
    );
    if high["agency"]["status"] == "not_derived" {
        // Increasing a budget does not promise a complete derivation.
        assert_eq!(high["agency"]["limit"]["counter"], "work");
        assert_eq!(high["agency"]["limit"]["limit"], 1_000_000_000);
    } else {
        assert!(high["agency"]["viewpoints"].is_object());
    }
}

#[test]
fn merge_projection_framing_limit_keeps_its_computational_reason() {
    let source = input::merge_or(12).replace(" || ", " && ");
    let low_out = run(&source, 1);
    assert_eq!(low_out.status.code(), Some(0));
    let low = wire(&low_out);
    let f = &low["agency"]["viewpoints"]["framing"];
    assert_eq!(f["status"], "not_derived");
    assert_eq!(f["reason"], "derivation_limit");
    assert_eq!(f["limit"]["counter"], "work");
    assert_eq!(f["limit"]["limit"], 1_000_000);
    assert!(f["limit"]["requested"].as_u64().unwrap() > 1_000_000);
    assert!(f.get("ends").is_none());
    assert_eq!(warnings(&low).len(), 1);
    assert!(
        warnings(&low)[0]["message"]
            .as_str()
            .unwrap()
            .contains("viewpoint framing not derived:")
    );
    assert_eq!(
        String::from_utf8_lossy(&low_out.stderr)
            .lines()
            .filter(|s| s.starts_with("W-AGY-5:"))
            .count(),
        1
    );
    let high_out = run(&source, 1000);
    assert_eq!(high_out.status.code(), Some(0));
    let high = wire(&high_out);
    let v = &high["agency"]["viewpoints"];
    assert_eq!(v["framing"]["status"], "derived");
    assert_eq!(v["framing"]["complete"], true);
    assert!(warnings(&high).is_empty());
    for (end, classification) in [("done", "all_contexts"), ("rejected", "some_contexts")] {
        let row = v["framing"]["ends"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["end"] == end)
            .unwrap();
        let got: std::collections::BTreeMap<_, _> = row["selectors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                (
                    s["call"].as_str().unwrap().to_owned(),
                    s["selects"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(
            got,
            (1..=12)
                .map(|i| (format!("j{i}"), classification.to_owned()))
                .collect()
        );
        assert_eq!(row["read_only"].as_array().unwrap().len(), 1);
        assert_eq!(row["read_only"][0]["call"], "r");
    }
    let judge = v["retention"]["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["capability"] == "Judge.check")
        .unwrap();
    assert_eq!(judge["normal"], serde_json::json!({"min":12,"max":12}));
}
#[test]
fn repeat_warnings_describe_only_final_blocks() {
    let out = run(input::REPEAT, 1);
    let v = wire(&out);
    for block in ["framing", "retention"] {
        assert_eq!(v["agency"]["viewpoints"][block]["reason"], "loop_control");
    }
    assert!(warnings(&v).is_empty(), "{v}");
    assert!(!String::from_utf8_lossy(&out.stderr).contains("W-AGY-5"));
}
#[test]
fn response_warnings_describe_only_final_blocks() {
    let out = run(&input::votes(12, " && ", true), 1);
    let v = wire(&out);
    assert_eq!(
        v["agency"]["viewpoints"]["framing"]["reason"],
        "response_control"
    );
    assert!(warnings(&v).is_empty(), "{v}");
    assert!(!String::from_utf8_lossy(&out.stderr).contains("W-AGY-5"));
}
#[test]
fn a_final_viewpoint_budget_failure_still_warns() {
    let out = run(&input::votes(12, " && ", false), 1);
    let v = wire(&out);
    assert_eq!(
        v["agency"]["viewpoints"]["framing"]["reason"],
        "derivation_limit"
    );
    assert_eq!(
        v["agency"]["viewpoints"]["framing"]["limit"]["limit"],
        1_000_000
    );
    assert_eq!(warnings(&v).len(), 1);
    assert!(
        warnings(&v)[0]["message"]
            .as_str()
            .unwrap()
            .contains("framing")
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("W-AGY-5"));
}
