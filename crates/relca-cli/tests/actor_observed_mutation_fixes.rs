//! ADR 0032 mutation survivors (bbc444c) on the `relca explain` wire.
//!
//! Hand expectation for each fixture: the model is `violated` (exit 1) and
//! the engine witness replays in the independent reference, so the
//! explanation is `complete` and every counterexample is `verified`.
//! - aobs-per-case.broken-false (A21, A16): per_case observed values do not
//!   exist before each admission, so the Initial stage carries none of them
//!   and replay must not expect them; the invariant `Doc.flag == true` fails
//!   at the first `Probe.check.finish`.
//! - aobs-late.broken-no-requires (A26): case 1 posts at 0..1 (posted 1);
//!   case 2 is admitted at 3 and must see the shared posted 1, so its post at
//!   3..4 attempts 2 > 1. A replay that admits case 2 with the stale initial
//!   value 0 would not reach that range violation.
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn explain(name: &str) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["explain", &format!("fixtures/{name}/model.relca")])
        .current_dir(root)
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(1), "{name}: {v}");
    assert_eq!(v["verdict"], "violated", "{name}");
    v
}

fn complete_and_verified(name: &str) -> Value {
    let v = explain(name);
    assert_eq!(v["explanation"]["status"], "complete", "{name}: {v}");
    let cs = v["explanation"]["counterexamples"].as_array().unwrap();
    assert!(!cs.is_empty(), "{name}: {v}");
    for c in cs {
        assert_eq!(c["replay"]["status"], "verified", "{name}: {c}");
    }
    v
}

/// A21 + A16: per_case observations are absent from the Initial stage in the
/// engine witness and in the reference replay.
#[test]
fn per_case_broken_false_explain_replays_without_initial_per_case_values() {
    let v = complete_and_verified("aobs-per-case.broken-false");
    for c in v["explanation"]["counterexamples"].as_array().unwrap() {
        let first = &c["steps"].as_array().unwrap()[0];
        let observed = first["values"]["observed"].as_array().unwrap();
        assert!(
            !observed
                .iter()
                .any(|o| o["name"] == "Doc.flag" || o["name"] == "Doc.answer"),
            "per_case values before admission: {first}"
        );
    }
}

/// A26: the late admission sees the shared value written by the earlier case.
#[test]
fn late_broken_no_requires_explain_replays_the_shared_write_into_the_late_case() {
    let v = complete_and_verified("aobs-late.broken-no-requires");
    let ranges = v["state_range_violations"].as_array().unwrap();
    assert_eq!(ranges.len(), 1, "{v}");
    let r = &ranges[0];
    assert_eq!(
        (
            &r["case_id"],
            &r["time"],
            &r["phase"],
            &r["before"],
            &r["attempted"]
        ),
        (
            &Value::from(2),
            &Value::from(4),
            &Value::from("finish"),
            &Value::from("1"),
            &Value::from("2")
        ),
        "{r}"
    );
}
