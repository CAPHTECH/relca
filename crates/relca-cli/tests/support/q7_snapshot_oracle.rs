//! Two closed, hand-calculated sensitivity jobs, not legacy state-IR Reports.
#[path = "../../../relca-core/tests/support/sensitivity_oracle.rs"]
mod hand;
use serde_json::{Value, json};
pub const IDS: [&str; 2] = ["q7-sensitivity-values", "q7-sensitivity-endpoints"];
fn args(id: &str) -> Option<Vec<String>> {
    let file = match id {
        "q7-sensitivity-values" => "values.json",
        "q7-sensitivity-endpoints" => "endpoints.json",
        _ => return None,
    };
    Some(vec![
        "sensitivity".into(),
        format!("crates/relca-core/tests/data/sensitivity/{file}"),
        "--mode".into(),
        "exhaustive".into(),
    ])
}
pub fn classified_args(actual: &[String]) -> bool {
    IDS.iter().any(|id| args(id).as_deref() == Some(actual))
}
pub fn check(id: &str, actual: &[String], doc: &Value, exit: i32, stderr: &[u8]) {
    assert_eq!(args(id).as_deref(), Some(actual));
    assert_eq!(exit, 0);
    assert!(stderr.is_empty());
    let (ids, value) = match id {
        "q7-sensitivity-values" => (vec!["base", "high", "miss"], ("1/250", "1/125")),
        "q7-sensitivity-endpoints" => (vec!["p0", "p1"], ("0", "0")),
        _ => panic!("unclassified sensitivity job"),
    };
    hand::suite(
        doc,
        &ids,
        [ids.len() as u64, 0, 0],
        "satisfied",
        Some(value),
    )
    .unwrap();
    let children = (0..ids.len())
        .map(|i| format!("/points/{i}/report"))
        .collect::<Vec<_>>();
    hand::references(doc, &children).unwrap();
    if id == IDS[0] {
        let expected = hand::hand();
        for p in doc["points"].as_array().unwrap() {
            let id = p["id"].as_str().unwrap();
            let probability = &expected["values"][id]["wrong"];
            assert_eq!(
                hand::property(&p["report"], hand::QUERY)["probability"],
                json!({"min":probability,"max":probability})
            );
            assert_eq!(p["source_sha256"], expected["source_sha256"][id]);
        }
    }
    let decoded: relca_core::sensitivity::wire::SensitivityReport =
        serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(json!(decoded), *doc);
}
