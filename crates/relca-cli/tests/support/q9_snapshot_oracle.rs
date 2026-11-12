//! Closed ADR 0063 hand-valued snapshot jobs, outside the pre-Q9 state-IR census.
#[path = "../../../relca-core/tests/support/q9_oracle.rs"]
mod hand;
use serde_json::{Value, json};

pub const IDS: [&str; 2] = ["q9-rates-check", "q9-rates-simulate"];
const SOURCE: &str = "crates/relca-cli/tests/snapshots/inputs/q9-rates.relca";
fn args(id: &str) -> Option<Vec<String>> {
    let command = match id {
        "q9-rates-check" => "check",
        "q9-rates-simulate" => "simulate",
        _ => return None,
    };
    Some(vec![command.into(), SOURCE.into()])
}
pub fn validate_set(jobs: &[Value]) {
    let selected: Vec<_> = jobs
        .iter()
        .filter(|j| {
            j["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("q9-rates-"))
        })
        .collect();
    assert_eq!(selected.len(), IDS.len(), "closed Q9 snapshot inventory");
    for id in IDS {
        let matches: Vec<_> = selected.iter().filter(|j| j["id"] == id).collect();
        assert_eq!(
            matches.len(),
            1,
            "Q9 snapshot job must occur exactly once: {id}"
        );
        assert_eq!(matches[0]["args"], json!(args(id).unwrap()));
    }
}
pub fn check(id: &str, argv: &[String], doc: &Value, exit: i32, stderr: &[u8], source: &str) {
    assert_eq!(args(id).as_deref(), Some(argv));
    let warnings = std::str::from_utf8(stderr).unwrap();
    assert!(warnings.contains("W-CONTEST-0: Party/posted has no contest route or explicit Cut"));
    assert_eq!(
        warnings.contains("W-UNREACH-2: process Flow: end.posted is never reached"),
        id == "q9-rates-check"
    );
    assert!(!warnings.contains("U-SEM-0:") && !warnings.contains("E-"));
    let ast = relca_core::syntax::parse(source).unwrap();
    let m = relca_core::lower::lower(&ast).unwrap();
    assert_eq!(m.contests().len(), 1);
    let c = &m.contests()[0];
    assert_eq!(c.property_id(), "contract Controls #1 contest");
    assert_eq!(c.rates.as_ref().unwrap().probability.to_string(), "1/4");
    assert_eq!(m.simulation.as_ref().unwrap().replications, 16);
    assert_eq!(m.metrics.as_ref().unwrap().currency, "JPY");
    match id {
        "q9-rates-check" => {
            assert_eq!((exit, &doc["verdict"]), (1, &json!("violated")));
            assert_eq!(
                hand::property(doc, " #1 contest_response")["verdict"],
                "violated"
            );
            assert_eq!(
                doc["case_outcomes"][0]["terminals"][0]["completed_at"],
                json!({"min":1,"max":1})
            );
            assert!(doc["metrics"].get("contests").is_none());
            hand::assumptions(doc, 1).unwrap();
        }
        "q9-rates-simulate" => {
            assert_eq!(exit, 2);
            hand::validate_sim("c", doc).unwrap();
            assert_eq!(doc["metrics"]["contests"]["rows"][0]["filed"], 4);
            assert_eq!(
                doc["metrics"]["contests"]["rows"][0]["filing_fraction"]["value"],
                "1/4"
            );
        }
        _ => panic!("unclassified Q9 snapshot job"),
    }
}
