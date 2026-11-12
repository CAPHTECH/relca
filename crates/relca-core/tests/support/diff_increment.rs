//! ADR 0031: the only increments an old-input diff may gain, enumerated here
//! from the actual rows and wire-diff wording, never from production helpers.
//! Old inputs cannot spell preserve / override / maintain, so every row
//! without `loses` gains exactly one W-DIFF-3.
#![allow(dead_code)]
use serde_json::{Value, json};

pub const NEW_KINDS: [&str; 7] = [
    "meaning",
    "authority",
    "resource",
    "time",
    "visibility",
    "contest",
    "relation", // ADR0051 producer rows follow every preexisting kind.
];
pub const NEW_CHECKED: [&str; 10] = [
    "meaning",
    "authority",
    "resource",
    "time",
    "visibility",
    "contest",
    "preserve",
    "override",
    "maintain",
    "annotations",
];
pub const REMOVE_UNCHECKED: [&str; 10] = [
    "time_intent",
    "resource",
    "meaning",
    "authority",
    "visibility",
    "contest",
    "preserve",
    "override",
    "maintain",
    "responsibility",
];

/// Old actor / relation rows keep their prefix; new rows follow ALL of them in
/// the kind order of NEW_KINDS. Returns the new rows.
pub fn new_rows(v: &Value, expected_producers: &[Value]) -> Vec<Value> {
    let rows = v["differences"].as_array().unwrap();
    let producers: Vec<_> = rows
        .iter()
        .filter(|r| r["kind"] == "relation" && r["change"] == "modified")
        .cloned()
        .collect();
    assert_eq!(producers, expected_producers, "caller hand producer oracle");
    let old = rows
        .iter()
        .take_while(|r| {
            r["kind"] == "actor" || (r["kind"] == "relation" && r["change"] != "modified")
        })
        .count();
    let new = rows[old..].to_vec();
    let rank: Vec<_> = new
        .iter()
        .map(|r| {
            if r["kind"] == "relation" {
                assert_eq!(
                    r["change"], "modified",
                    "only the closed producer form is new"
                );
            }
            NEW_KINDS
                .iter()
                .position(|k| r["kind"] == *k)
                .unwrap_or_else(|| panic!("new row after old rows has an old/unknown kind: {r}"))
        })
        .collect();
    assert!(rank.is_sorted(), "new rows in NEW_KINDS order: {v}");
    new
}

/// Asserts diagnostics == old ++ W-DIFF-1(new rows) ++ W-DIFF-3(rows without
/// loses, row order) and returns the old part, to be compared with the
/// pre-ADR-0031 expectation unchanged.
pub fn old_diagnostics(v: &Value, expected_producers: &[Value]) -> Vec<Value> {
    let mut increments: Vec<_> = new_rows(v, expected_producers)
        .iter()
        .map(|r| {
            let target = r["target"].as_str().unwrap();
            let message = if r["kind"] == "relation" {
                format!("computed change for {target} has no intent")
            } else {
                format!(
                    "computed change for {} {target} has no intent",
                    r["kind"].as_str().unwrap()
                )
            };
            json!({"code":"W-DIFF-1","severity":"warning","span":null,"message":message})
        })
        .collect();
    for r in v["differences"].as_array().unwrap() {
        if r["loses"].is_null() {
            increments.push(json!({"code":"W-DIFF-3","severity":"warning","span":null,
            "message":format!("computed change for {} {} has no preserve, override, maintain or loses annotation",
                r["kind"].as_str().unwrap(), r["target"].as_str().unwrap())}));
        }
    }
    let all = v["diagnostics"].as_array().unwrap();
    assert!(all.len() >= increments.len(), "missing increments: {v}");
    let (old, tail) = all.split_at(all.len() - increments.len());
    assert_eq!(
        tail,
        increments.as_slice(),
        "exact permitted increments: {v}"
    );
    old.to_vec()
}

/// unchecked of a completed comparison: the old list minus REMOVE_UNCHECKED.
pub fn migrated_unchecked(old: &[&str]) -> Value {
    json!(
        old.iter()
            .copied()
            .filter(|w| !REMOVE_UNCHECKED.contains(w))
            .chain(["relation_state_producers", "relation_binding_identity"])
            .collect::<Vec<_>>()
    )
}

#[test]
fn k1_historical_helper_refuses_unknown_kind_and_unchecked_word() {
    let bad = json!({"differences":[{"kind":"unknown_kind","target":"entry","change":"modified"}],"diagnostics":[]});
    assert!(std::panic::catch_unwind(|| old_diagnostics(&bad, &[])).is_err());
    let bad = json!(["relation_state_producers", "unknown_word"]);
    assert!(std::panic::catch_unwind(|| assert_eq!(bad, migrated_unchecked(&[]))).is_err());
    // Positive control and a producer payload fault: knowing the kind is not a bypass.
    let producer = json!({"kind":"relation","target":"entry","change":"modified","reason":null,"loses":null,
        "from":{"producer":"Clerk.enter"},"to":{"producer":"Extractor.extract"}});
    let good = json!({"differences":[producer.clone()]});
    assert_eq!(
        new_rows(&good, std::slice::from_ref(&producer)),
        vec![producer.clone()]
    );
    let mut bad = good;
    bad["differences"][0]["to"]["producer"] = json!("Other.make");
    assert!(std::panic::catch_unwind(|| new_rows(&bad, &[producer])).is_err());
}

/// checked of a completed comparison: NEW_CHECKED after the static five words.
pub fn migrated_checked(old: &[&str]) -> Value {
    let mut words = old.to_vec();
    words.splice(5..5, NEW_CHECKED);
    json!(words)
}
