//! Closed S4 job, connected to the fixed independent A7 hand table and input.
#[path = "../../../relca-core/tests/support/q6b_oracle.rs"]
mod hand;
#[path = "../../../relca-core/tests/support/q6b_inputs.rs"]
mod input;
use serde_json::{Value, json};

pub const IDS: [&str; 1] = ["q6b-a7-check"];
const SOURCE: &str = "crates/relca-cli/tests/snapshots/inputs/q6b-a7.relca";
pub fn validate_set(jobs: &[Value]) {
    let selected: Vec<_> = jobs
        .iter()
        .filter(|j| j["id"].as_str().is_some_and(|id| id.starts_with("q6b-")))
        .collect();
    assert_eq!(selected.len(), IDS.len(), "closed Q6b snapshot inventory");
    for id in IDS {
        let matches: Vec<_> = selected.iter().filter(|j| j["id"] == id).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["args"], json!(["check", SOURCE]));
    }
}
pub fn check(id: &str, argv: &[String], doc: &Value, exit: i32, stderr: &[u8], source: &str) {
    assert_eq!(id, IDS[0]);
    assert_eq!(json!(argv), json!(["check", SOURCE]));
    assert_eq!(source.trim_end(), input::source("a7"));
    assert_eq!((exit, &doc["verdict"]), (0, &json!("satisfied")));
    hand::validate_report("a7", doc).unwrap();
    hand::validate_times("a7", doc).unwrap();
    let stderr = std::str::from_utf8(stderr).unwrap();
    assert!(!stderr.contains("E-") && !stderr.contains("U-SEM-0"));
    let m = relca_core::lower::lower(&relca_core::syntax::parse(source).unwrap()).unwrap();
    assert_eq!(m.process.degradations.len(), 1);
    assert!(!m.legacy_route());
    assert_eq!(m.process.degradations[0].name, "Manual");
}
