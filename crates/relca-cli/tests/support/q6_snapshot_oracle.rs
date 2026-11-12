//! Closed positive Q6a jobs, connected to the separately authored hand oracle.
#[path = "../../../relca-core/tests/support/q6a_oracle.rs"]
mod hand;
#[path = "../../../relca-core/tests/support/q6a_inputs.rs"]
mod input;
use serde_json::{Value, json};

pub const IDS: [&str; 2] = ["q6a-qualified-check", "q6a-burst-simulate"];
fn case(id: &str) -> (&'static str, &'static str, &'static str) {
    match id {
        "q6a-qualified-check" => (
            "qualified",
            "check",
            "crates/relca-cli/tests/snapshots/inputs/q6a-qualified.relca",
        ),
        "q6a-burst-simulate" => (
            "burst",
            "simulate",
            "crates/relca-cli/tests/snapshots/inputs/q6a-burst.relca",
        ),
        _ => panic!("unclassified Q6a job"),
    }
}
pub fn validate_set(jobs: &[Value]) {
    let found: Vec<_> = jobs
        .iter()
        .filter(|j| j["id"].as_str().is_some_and(|id| id.starts_with("q6a-")))
        .collect();
    assert_eq!(found.len(), IDS.len(), "closed Q6a snapshot inventory");
    for id in IDS {
        let rows: Vec<_> = found.iter().filter(|j| j["id"] == id).collect();
        assert_eq!(rows.len(), 1, "one job per closed ID");
        let (_, mode, path) = case(id);
        assert_eq!(rows[0]["args"], json!([mode, path]));
        assert_eq!(rows[0]["expected_exit"], 0);
        assert_eq!(rows[0]["expected_verdict"], "satisfied");
    }
}
pub fn check(id: &str, args: &[String], doc: &Value, exit: i32, stderr: &[u8], source: &str) {
    let (name, mode, path) = case(id);
    assert_eq!(args, &[mode.to_owned(), path.to_owned()]);
    assert_eq!(
        source.split_whitespace().collect::<Vec<_>>(),
        input::source(name).split_whitespace().collect::<Vec<_>>(),
        "source is the fixed independent input, not an output-derived example"
    );
    assert_eq!(exit, 0);
    let diagnostic_mirror: String = doc["diagnostics"]
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
    assert_eq!(stderr, diagnostic_mirror.as_bytes());
    match mode {
        "check" => {
            hand::validate(name, doc).unwrap();
            serde_json::from_value::<relca_core::report::Report>(doc.clone()).unwrap();
        }
        "simulate" => {
            hand::validate_sim(name, doc).unwrap();
            serde_json::from_value::<relca_core::report::SimulationReport>(doc.clone()).unwrap();
        }
        _ => panic!(),
    }
}
