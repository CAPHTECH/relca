//! Three closed snapshot inputs from independently fixed Q8 hand examples.
#[path = "approval_inputs.rs"]
#[allow(dead_code)]
mod input;
#[path = "approval_wire.rs"]
#[allow(dead_code)]
mod wire;
use serde_json::{Value, json};

pub const IDS: [&str; 3] = [
    "q8-approval-baseline",
    "q8-approval-ineligible",
    "q8-approval-person-union",
];
fn name(id: &str) -> &'static str {
    match id {
        "q8-approval-baseline" => "a01_baseline",
        "q8-approval-ineligible" => "a02_ineligible",
        "q8-approval-person-union" => "a08_same_person_union",
        _ => panic!("unclassified approval snapshot job"),
    }
}
pub fn arguments(id: &str) -> Vec<String> {
    assert!(IDS.contains(&id));
    vec![
        "conform".into(),
        "crates/relca-cli/tests/snapshots/inputs/q8-approval.relca".into(),
        "--log".into(),
        format!("crates/relca-cli/tests/snapshots/inputs/{id}.jsonl"),
        "--scope".into(),
        "approval-burden".into(),
        "--approval-spec".into(),
        format!("crates/relca-cli/tests/snapshots/inputs/{id}.json"),
    ]
}
pub fn check(id: &str, doc: &Value, exit: i32) -> Result<(), String> {
    wire::report(doc, exit)?;
    let c = input::case(name(id));
    let n = c.expected.numbers.unwrap();
    let b = &doc["approval_burden"];
    if exit != 0
        || doc["log"]["events"] != c.records.len() - 1
        || json!([
            b["rate"]["numerator"],
            b["rate"]["denominator"],
            b["rate"]["value"],
            b["rate"]["known_ineligible"],
            b["rate"]["unknown_episodes"],
            b["rate"]["no_observed_approval_cases"]
        ]) != json!([n.n, n.d, n.rate, n.ineligible, n.unknown, n.no_approval])
        || json!([
            b["reinvestigation"]["total"],
            b["reinvestigation"]["mean"],
            b["reinvestigation"]["known_episodes"],
            b["reinvestigation"]["unknown_episodes"]
        ]) != json!([n.total, n.mean, n.known_time, n.unknown_time])
    {
        return Err(format!(
            "approval snapshot independent hand values differ: {id}"
        ));
    }
    Ok(())
}
pub fn check_inputs(id: &str, root: &std::path::Path) {
    let c = input::case(name(id));
    let args = arguments(id);
    assert_eq!(
        std::fs::read_to_string(root.join(&args[1]))
            .unwrap()
            .trim_end(),
        c.model
    );
    assert_eq!(
        std::fs::read_to_string(root.join(&args[3])).unwrap(),
        c.log()
    );
    let mut saved: Value =
        serde_json::from_slice(&std::fs::read(root.join(&args[7])).unwrap()).unwrap();
    saved["model_sha256"] = c.spec["model_sha256"].clone();
    saved["log_sha256"] = c.spec["log_sha256"].clone();
    assert_eq!(
        saved, c.spec,
        "snapshot declarations must remain the fixed acceptance inputs"
    );
}
