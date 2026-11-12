//! Closed Q6c jobs use the fixed hand oracle, not captured golden expectations.
//! q6c_snapshot_inputs separately checks every materialized model against its
//! independently authored acceptance source (including both manifest points).
#[path = "../../../relca-core/tests/support/q6c_oracle.rs"]
mod hand;
use serde_json::{Value, json};

pub const IDS: [&str; 3] = [
    "q6c-work-compare",
    "q6c-degrade-diff",
    "q6c-work-sensitivity",
];
fn args(id: &str) -> Value {
    let root = "crates/relca-cli/tests/snapshots/inputs";
    match id {
        "q6c-work-compare" => json!([
            "compare",
            format!("{root}/q6c-work.relca"),
            "--plans",
            "From,To",
            "--mode",
            "exhaustive"
        ]),
        "q6c-degrade-diff" => json!([
            "diff",
            format!("{root}/q6c-degrade.relca"),
            "--from",
            "From",
            "--to",
            "To"
        ]),
        "q6c-work-sensitivity" => json!([
            "sensitivity",
            format!("{root}/q6c-sensitivity.json"),
            "--mode",
            "exhaustive"
        ]),
        _ => panic!("unclassified Q6c job"),
    }
}
pub fn validate_set(jobs: &[Value]) {
    let found = jobs
        .iter()
        .filter(|j| j["id"].as_str().is_some_and(|id| id.starts_with("q6c-")))
        .collect::<Vec<_>>();
    assert_eq!(found.len(), IDS.len(), "closed Q6c job inventory");
    for id in IDS {
        let rows = found.iter().filter(|j| j["id"] == id).collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["args"], args(id));
        assert_eq!(rows[0]["expected_exit"], 0);
        assert_eq!(rows[0]["expected_verdict"], "satisfied");
    }
}
fn diagnostics(doc: &Value, prefix: &str) -> String {
    doc["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{prefix}{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect()
}
pub fn check(id: &str, actual: &[String], doc: &Value, exit: i32, stderr: &[u8]) {
    assert_eq!(json!(actual), args(id));
    assert_eq!(exit, 0);
    let mut mirror = String::new();
    match id {
        "q6c-work-compare" => {
            hand::compare(doc, "work", false);
            for p in doc["plans"].as_array().unwrap() {
                mirror.push_str(&diagnostics(
                    &p["report"],
                    &format!("configuration {}: ", p["configuration"].as_str().unwrap()),
                ));
                serde_json::from_value::<relca_core::report::Report>(p["report"].clone()).unwrap();
            }
            mirror.push_str(&diagnostics(doc, "compare: "));
            serde_json::from_value::<Vec<relca_core::compare::ConfigurationPair>>(
                doc["tables"]["configuration"].clone(),
            )
            .unwrap();
        }
        "q6c-degrade-diff" => {
            hand::diff(doc, "degrade", false);
            mirror.push_str(&diagnostics(&doc["from"]["report"], "from: "));
            mirror.push_str(&diagnostics(&doc["to"]["report"], "to: "));
            mirror.push_str(&diagnostics(doc, "diff: "));
            serde_json::from_value::<relca_core::diff::DiffReport>(doc.clone()).unwrap();
        }
        "q6c-work-sensitivity" => {
            hand::suite(doc, &["base", "probability"]);
            for p in doc["points"].as_array().unwrap() {
                mirror.push_str(&diagnostics(
                    &p["report"],
                    &format!("point {}: ", p["id"].as_str().unwrap()),
                ));
            }
            mirror.push_str(&diagnostics(doc, "sensitivity: "));
            serde_json::from_value::<relca_core::sensitivity::wire::SensitivityReport>(doc.clone())
                .unwrap();
        }
        _ => panic!("unclassified Q6c job"),
    }
    assert_eq!(stderr, mirror.as_bytes(), "exact public stderr prefixes");
    let ledger: relca_core::ledger::Ledger = serde_json::from_value(doc["ledger"].clone()).unwrap();
    ledger.validate(doc).unwrap();
}
