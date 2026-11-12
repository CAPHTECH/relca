//! Internal linkage from lib.rs; no production API is widened.
use crate::{comparison_projection as projection, lower, report, syntax};
use serde_json::{Value, json};

// Test-owned linkage into the same crate-private production projection as the
// pre-existing tampered-report probes. No production source/API is changed.
#[path = "waits_review_projection.rs"]
mod waits_review_projection;

const SOURCE: &str = include_str!("../../../../fixtures/aset-compare-notes/model.relca");

#[test]
fn canonical_notes_are_required_and_forged_notes_fail_both_projection_modes() {
    let m = lower::lower_configuration(&syntax::parse(SOURCE).unwrap(), "Left")
        .unwrap()
        .model;
    let original = report::check_configuration(SOURCE, "Left");
    let sample = report::simulate_configuration(SOURCE, "Left");
    assert!(projection::exhaustive(&original, &m).is_ok());
    assert!(projection::simulation(&sample, &m).is_ok());
    let canonical = json!(original);
    let i = canonical["properties"]
        .as_array()
        .unwrap()
        .iter()
        .position(|p| p["id"] == "contract C #1 at_most_once")
        .unwrap();
    assert_eq!(
        canonical["properties"][i]["notes"],
        json!(["mechanism_unmodeled"])
    );
    let faults: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/aset-compare-notes.broken-replications/wire_faults.json"
    ))
    .unwrap();
    for fault in faults {
        let notes = fault["notes"].clone();
        let mut forged = canonical.clone();
        forged["properties"][i]["notes"] = notes.clone();
        match serde_json::from_value::<report::Report>(forged.clone()) {
            Ok(r) => {
                assert!(
                    projection::exhaustive(&r, &m).is_err(),
                    "forged exhaustive {notes}"
                );
            }
            Err(_) => { /* Rejecting malformed wire before projection is fail-closed. */ }
        }
        let mut r = sample.clone();
        let i = r
            .properties
            .iter()
            .position(|p| p["id"] == "contract C #1 at_most_once")
            .unwrap();
        r.properties[i]["notes"] = notes.clone();
        assert!(
            projection::simulation(&r, &m).is_err(),
            "forged sampled {notes}"
        );
    }
    // Notes on another property must not be accepted just because a valid
    // at_most_once property exists elsewhere in the same IR inventory.
    let mut wrong = sample.clone();
    let i = wrong
        .properties
        .iter()
        .position(|p| p["id"] == "reachable end.done")
        .unwrap();
    wrong.properties[i]["notes"] = json!(["mechanism_unmodeled"]);
    assert!(projection::simulation(&wrong, &m).is_err());
    let mut wrong = canonical;
    let i = wrong["properties"]
        .as_array()
        .unwrap()
        .iter()
        .position(|p| p["id"] == "reachable end.done")
        .unwrap();
    wrong["properties"][i]["notes"] = json!(["mechanism_unmodeled"]);
    let r: report::Report = serde_json::from_value(wrong).unwrap();
    assert!(projection::exhaustive(&r, &m).is_err());
}
