//! Closed Q4 snapshot jobs: time hand arithmetic, not the older lexical walkers.
#[path = "../../../relca-core/tests/support/q4_oracle.rs"]
mod hand;
#[path = "ledger_shape.rs"]
mod ledger;
use serde_json::Value;

pub const IDS: [&str; 3] = [
    "q4-calendar-open",
    "q4-condition-true",
    "q4-finite-arrivals",
];
pub fn name(id: &str) -> Option<&'static str> {
    match id {
        "q4-calendar-open" => Some("open"),
        "q4-condition-true" => Some("condition-true"),
        "q4-finite-arrivals" => Some("arrivals"),
        _ => None,
    }
}
fn args(id: &str) -> [&'static str; 2] {
    match id {
        "q4-calendar-open" => ["check", "crates/relca-core/tests/support/q4/open.relca"],
        "q4-condition-true" => [
            "check",
            "crates/relca-core/tests/support/q4/condition-constant.relca",
        ],
        "q4-finite-arrivals" => [
            "simulate",
            "crates/relca-core/tests/support/q4/arrivals.relca",
        ],
        _ => panic!("unclassified Q4 job"),
    }
}
pub fn validate_set(jobs: &[Value]) {
    for id in IDS {
        let rows: Vec<_> = jobs.iter().filter(|j| j["id"] == id).collect();
        assert_eq!(rows.len(), 1, "Q4 closed snapshot inventory {id}");
        assert_eq!(rows[0]["args"], serde_json::json!(args(id)));
    }
    for row in jobs {
        let id = row["id"].as_str().unwrap();
        assert!(
            !id.starts_with("q4-") || IDS.contains(&id),
            "unclassified Q4 job {id}"
        );
    }
}
pub fn check(id: &str, actual_args: &[String], doc: &Value, exit: i32, stderr: &[u8]) {
    assert_eq!(actual_args, args(id));
    hand::validate(name(id).unwrap(), doc).unwrap();
    ledger::validate(&doc["ledger"]).unwrap();
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
