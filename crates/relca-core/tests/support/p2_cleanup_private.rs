//! Adapter-independent private obligations, compiled by integration acceptance.
//! The report/run test modules connect these to real production paths.
#![allow(dead_code)]
use serde_json::{Value, json};

pub fn runner(v: &Value, calls: usize) {
    assert_eq!(calls, 2, "no third driver after postbarrier contradiction");
    assert_eq!(v["scope"]["evaluated"], 1);
    assert_eq!(v["scope"]["refused"], 1);
    assert_eq!(v["scope"]["not_run"], 1);
    assert_ne!(v["verdict"], "satisfied");
    assert_eq!(v["points"][0]["status"], "evaluated");
    assert_eq!(
        v["points"][0]["report"]["ledger"]["execution"]["status"],
        "ran"
    );
    assert_eq!(v["points"][1]["status"], "refused");
    assert_eq!(v["points"][1]["reason"], "sensitivity_feature_combination");
    assert_eq!(
        v["points"][1]["report"]["ledger"]["execution"]["status"],
        "not_run"
    );
    assert_eq!(v["points"][2]["status"], "not_run");
    assert_eq!(v["points"][2]["reason"], "no_analysis");
    assert!(v["points"][2]["report"].is_null());
    assert_eq!(v["summaries"][0]["value"], Value::Null);
    assert_eq!(v["summaries"][0]["reason"], "incomplete_points");
    assert!(v["diagnostics"].as_array().unwrap().iter().any(|d| {
        d["message"]
            .as_str()
            .is_some_and(|s| s.contains("context_mismatch:") && s.contains("preexecution barrier"))
    }));
}
pub fn missing_record(mut differences: impl FnMut(bool, bool) -> Result<Value, String>) {
    assert_eq!(differences(false, false).unwrap(), json!([]));
    for (a, z) in [(true, false), (false, true), (true, true)] {
        let e = differences(a, z).expect_err("missing recorded profile is not an empty diff");
        assert!(e.starts_with("invalid_q6_record:"), "{e}");
        assert!(e.contains("missing recorded performance profile"), "{e}");
    }
}
