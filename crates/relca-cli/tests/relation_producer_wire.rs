//! Independent fixture comparator, deliberately not production Difference Serde.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/relation_producers.rs"]
mod hand;
use serde_json::{Value, json};
fn valid() -> Value {
    // The old supported substrate supplies unrelated report fields; the new
    // relation form itself is hand assembled before serializer implementation.
    let src = hand::source(hand::BEFORE, hand::BEFORE, "");
    let mut v = serde_json::to_value(relca_core::diff::diff(&src, "From", "To")).unwrap();
    v["differences"] = json!([hand::row("x", "A.one", "A.two")]);
    v["unchecked"] = json!(["relation_state_producers", "relation_binding_identity"]);
    v
}
fn accepts(v: &Value) -> bool {
    fixture_match::compare_diff_report(&json!({}), v).is_ok()
}
#[test]
fn completed_diff_requires_both_limitation_words_even_without_producer_rows() {
    let src = hand::source(hand::BEFORE, hand::BEFORE, "");
    let valid = serde_json::to_value(relca_core::diff::diff(&src, "From", "To")).unwrap();
    assert!(accepts(&valid));
    for words in [
        json!([]),
        json!(["relation_state_producers"]),
        json!(["relation_binding_identity"]),
    ] {
        let mut broken = valid.clone();
        broken["unchecked"] = words;
        assert!(
            !accepts(&broken),
            "accepted completed diff without both words {broken}"
        );
    }
    // The metrics entry recurses through a stripped comparison. It must not
    // provide a second way to lose both producer limitation words.
    let src = include_str!("../../../fixtures/met-diff/model.relca");
    let mut metric = serde_json::to_value(relca_core::diff::diff(src, "AsIs", "ToBeP")).unwrap();
    assert!(metric.get("metrics").is_some());
    assert!(accepts(&metric));
    metric["unchecked"]
        .as_array_mut()
        .unwrap()
        .retain(|s| s != "relation_state_producers" && s != "relation_binding_identity");
    assert!(
        !accepts(&metric),
        "metrics recursion accepted both words missing"
    );
}
#[test]
fn refused_and_cut_mismatched_diffs_do_not_require_limitation_words() {
    let invalid =
        hand::source(hand::BEFORE, hand::AFTER, "").replace("x = A.two()", "x = Unknown.two()");
    let mismatch = hand::source(hand::BEFORE, hand::AFTER, "").replace(
        "configuration To { cut C;",
        "cut D { include [A,R] } configuration To { cut D;",
    );
    for src in [invalid, mismatch] {
        let v = serde_json::to_value(relca_core::diff::diff(&src, "From", "To")).unwrap();
        assert!(
            v["checked"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s != "relation")
        );
        assert!(
            v["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s != "relation_state_producers" && s != "relation_binding_identity")
        );
        assert!(
            accepts(&v),
            "{:?}",
            fixture_match::compare_diff_report(&json!({}), &v)
        );
    }
}
#[test]
fn producer_annotations_have_only_preserve() {
    for annotation in hand::forbidden_producer_annotations() {
        let mut b = valid();
        b["differences"][0]["annotations"] = json!([annotation]);
        assert!(!accepts(&b), "accepted {b}");
    }
}
#[test]
fn producer_wire_independent_positive_and_legacy_control() {
    let v = valid();
    assert!(
        accepts(&v),
        "{:?}",
        fixture_match::compare_diff_report(&json!({}), &v)
    );
    let mut legacy = v.clone();
    legacy["differences"] =
        json!([{"kind":"relation","target":"A.one","change":"added","reason":null,"loses":null}]);
    assert!(accepts(&legacy));
    // A producer suffix follows the older high-rank time row, not rank1.
    let mut suffix = v;
    suffix["differences"].as_array_mut().unwrap().insert(0,json!({"kind":"time","target":"A.one","change":"modified","reason":null,"loses":null,"from":["sequential"],"to":["fork"]}));
    assert!(accepts(&suffix));
}
#[test]
fn producer_wire_independent_single_faults() {
    let v = valid();
    let mut faults = vec![];
    for key in ["from", "to"] {
        let mut b = v.clone();
        b["differences"][0].as_object_mut().unwrap().remove(key);
        faults.push(b);
        let mut b = v.clone();
        b["differences"][0][key] = Value::Null;
        faults.push(b);
    }
    for producer in ["", "A", "A.one.more", "A..one", "A.1", "A. one"] {
        let mut b = v.clone();
        b["differences"][0]["from"]["producer"] = json!(producer);
        faults.push(b);
    }
    for change in ["added", "removed"] {
        let mut b = v.clone();
        b["differences"][0]["change"] = json!(change);
        faults.push(b);
    }
    let mut b = v.clone();
    b["differences"][0]["from"] = b["differences"][0]["to"].clone();
    faults.push(b);
    let mut b = v.clone();
    b["differences"][0]["target"] = json!("A.one");
    faults.push(b);
    let mut b = v.clone();
    b["differences"][0]["extra"] = json!(1);
    faults.push(b);
    let mut b = v.clone();
    b["differences"][0]["to"]["extra"] = json!(1);
    faults.push(b);
    let mut b = v.clone();
    b["differences"]
        .as_array_mut()
        .unwrap()
        .push(hand::row("x", "A.one", "A.two"));
    faults.push(b);
    let mut b = v.clone();
    b["differences"].as_array_mut().unwrap().push(json!({"kind":"time","target":"A.one","change":"modified","reason":null,"loses":null,"from":["sequential"],"to":["fork"]}));
    faults.push(b);
    let mut b = v;
    b["unchecked"] = json!(["relation_state_producers"]);
    faults.push(b);
    for b in faults {
        assert!(!accepts(&b), "accepted single fault {b}");
    }
}
