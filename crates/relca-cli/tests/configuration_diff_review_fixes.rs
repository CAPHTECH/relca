//! Boundary review of ADR 0031: the closed wire validator rejects override
//! records whose verdict contradicts their witnesses or the root fold, and the
//! authority record carries the Permission. Positive controls show each
//! rejection is caused by the one corrupted field.
#[path = "../../relca-core/tests/support/configuration_diff_inputs.rs"]
#[allow(dead_code)]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};

fn actual(name: &str) -> Value {
    serde_json::to_value(relca_core::diff::diff(
        &acceptance::source(name),
        "From",
        "To",
    ))
    .unwrap()
}
fn valid(v: &Value) -> Result<(), String> {
    fixture_match::compare_diff_report(&json!({}), v)
}
/// diff2-override: row 1 is relation P.post with one override annotation.
fn with_override(verdict: &str, effects: Value, root: &str) -> Value {
    let mut v = actual("diff2-override");
    assert_eq!(v["differences"][1]["target"], "P.post");
    let a = &mut v["differences"][1]["annotations"][0];
    assert_eq!(a["kind"], "override");
    a["verdict"] = json!(verdict);
    a["effects"] = effects;
    v["verdict"] = json!(root);
    v
}
fn all_paths() -> Value {
    json!([{"effect":"P.post","call":"x","judgements":["g"],"stops":"all_paths","bypasses":0}])
}

#[test]
fn production_override_output_is_valid() {
    let v = actual("diff2-override");
    assert_eq!(
        v["differences"][1]["annotations"][0]["effects"],
        all_paths()
    );
    valid(&v).unwrap();
    valid(&with_override("satisfied", all_paths(), "satisfied")).unwrap();
}

#[test]
fn vacuous_override_with_effects_is_rejected() {
    let bad = with_override("vacuous", all_paths(), "vacuous");
    assert!(valid(&bad).is_err(), "accepted vacuous with effects");
    valid(&with_override("vacuous", json!([]), "vacuous")).unwrap();
}

#[test]
fn violated_override_whose_effects_are_all_stopped_is_rejected() {
    let bad = with_override("violated", all_paths(), "violated");
    assert!(
        valid(&bad).is_err(),
        "accepted violated without failing effect"
    );
    let none =
        json!([{"effect":"P.post","call":"x","judgements":[],"stops":"none","bypasses":null}]);
    valid(&with_override("violated", none, "violated")).unwrap();
}

#[test]
fn override_verdict_not_folded_into_the_root_is_rejected() {
    let unknown =
        json!([{"effect":"P.post","call":"x","judgements":[],"stops":"unknown","bypasses":null}]);
    for (verdict, effects, weaker_root) in [
        ("unsupported", unknown.clone(), "satisfied"),
        ("unsupported", unknown.clone(), "vacuous"),
        ("vacuous", json!([]), "satisfied"),
    ] {
        let bad = with_override(verdict, effects, weaker_root);
        assert!(
            valid(&bad).is_err(),
            "accepted {verdict} under root {weaker_root}"
        );
    }
    valid(&with_override(
        "unsupported",
        unknown.clone(),
        "unsupported",
    ))
    .unwrap();
    valid(&with_override("unsupported", unknown, "violated")).unwrap();
}

// Permission in the authority record (wire-diff.md authority value + the
// permission field): null or a nonempty sorted unique person array. With
// permission null and three empty arrays the value itself is null.
#[test]
fn authority_record_requires_a_closed_permission_field() {
    let v = actual("diff2-resource");
    let i = v["differences"]
        .as_array()
        .unwrap()
        .iter()
        .position(|r| r["kind"] == "authority" && r["target"] == "A.a")
        .expect("authority A.a row (hand: A executors [q,p])");
    assert_eq!(
        v["differences"][i]["to"],
        json!({"permission":["p","q"],"grants":[],"authority_basis":[],"responsibilities":[]})
    );
    valid(&v).unwrap();
    let path = format!("/differences/{i}/to");
    for (mutation, value) in [
        ("missing", Value::Null),
        ("unsorted", json!(["q", "p"])),
        ("duplicate", json!(["p", "p"])),
        ("empty", json!([])),
        ("null-empty", Value::Null),
        ("type", json!("p")),
    ] {
        let mut bad = v.clone();
        let cell = bad.pointer_mut(&path).unwrap().as_object_mut().unwrap();
        if mutation == "missing" {
            cell.remove("permission");
        } else {
            cell.insert("permission".into(), value);
        }
        assert!(valid(&bad).is_err(), "accepted permission {mutation}");
    }
}
