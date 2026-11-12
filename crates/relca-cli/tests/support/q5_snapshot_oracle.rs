//! Closed Q5 jobs, checked against the fixed pre-implementation hand oracle.
#[path = "../../../relca-core/tests/support/q5_oracle.rs"]
mod hand;
#[path = "../../../relca-core/tests/support/q5_inputs.rs"]
mod input;
use serde_json::Value;

pub const IDS: [&str; 3] = ["q5-start", "q5-external", "q5-conflict"];
pub fn name(id: &str) -> Option<&'static str> {
    match id {
        "q5-start" => Some("start"),
        "q5-external" => Some("external"),
        "q5-conflict" => Some("conflict"),
        _ => None,
    }
}
fn args(id: &str) -> [String; 2] {
    assert!(IDS.contains(&id));
    [
        "check".into(),
        format!("crates/relca-cli/tests/snapshots/inputs/{id}.relca"),
    ]
}
pub fn validate_set(jobs: &[Value]) {
    for id in IDS {
        let rows: Vec<_> = jobs.iter().filter(|j| j["id"] == id).collect();
        assert_eq!(rows.len(), 1, "Q5 closed snapshot inventory {id}");
        assert_eq!(rows[0]["args"], serde_json::json!(args(id)));
    }
    for row in jobs {
        let id = row["id"].as_str().unwrap();
        assert!(
            !id.starts_with("q5-") || IDS.contains(&id),
            "unclassified Q5 job {id}"
        );
    }
}
pub fn check(id: &str, actual_args: &[String], doc: &Value, exit: i32, stderr: &[u8]) {
    assert_eq!(actual_args, args(id));
    hand::validate(name(id).unwrap(), doc).unwrap();
    // Typed decoder independently checks closed local scopes and ledger links.
    let _: relca_core::report::Report = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(exit, 0);
    let mirror: String = doc["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(stderr, mirror.as_bytes());
}
