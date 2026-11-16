//! Closed scope shape and source-bound resource attribution, independent of lower.
#[allow(dead_code)]
#[path = "support/defer_effects_inputs.rs"]
mod hand;
use relca_test_support::fixture_match;
use serde_json::{Value, json};

#[test]
fn typed_scope_is_required_and_nullable_only_for_process_or_none() {
    let scoped = hand::expected(4).unwrap()["entries"][0].clone();
    let process = hand::expected(1).unwrap()["entries"][0].clone();
    let none = json!({"target":"Empty","resolution":"actor","capabilities":[],"withdraws":"none","because":[],"scope":null});
    for good in [&scoped, &process, &none] {
        let typed: relca_core::report::DeferEffect = serde_json::from_value(good.clone()).unwrap();
        assert_eq!(json!(typed), *good);
        let mut missing = good.clone();
        missing.as_object_mut().unwrap().remove("scope");
        assert!(serde_json::from_value::<relca_core::report::DeferEffect>(missing).is_err());
    }
    let mut bad = scoped.clone();
    bad["scope"] = Value::Null;
    assert!(serde_json::from_value::<relca_core::report::DeferEffect>(bad).is_err());
    for mut bad in [process, none] {
        bad["scope"] = scoped["scope"].clone();
        assert!(serde_json::from_value::<relca_core::report::DeferEffect>(bad).is_err());
    }
}

fn source() -> String {
    hand::source(4).replace(
        "resource Desk capacity 1",
        "resource Desk capacity 1\nresource Other capacity 1",
    )
}

#[test]
fn live_scope_is_required_by_dto_and_shared_wire() {
    let good = json!(relca_core::report::check(&source()));
    fixture_match::validate_deferred_metrics_wire(&good).unwrap();
    let _: relca_core::report::Report = serde_json::from_value(good.clone()).unwrap();
    let mut bad = good;
    bad["cut"]["defer_effects"]["entries"][0]
        .as_object_mut()
        .unwrap()
        .remove("scope");
    assert!(fixture_match::validate_deferred_metrics_wire(&bad).is_err());
    assert!(serde_json::from_value::<relca_core::report::Report>(bad).is_err());
}

#[test]
fn wire_scope_key_is_required_even_when_its_value_would_be_null() {
    let empty = hand::source(0)
        .replace("actor A {", "actor Empty {}\nactor A {")
        .replace(
            "include [A,R];",
            "include [A,R,Empty]; defer Empty reason \"later\" loses \"load\";",
        );
    for (source, withdraws) in [
        (hand::source(1), "process"),
        (hand::source(4), "scoped"),
        (empty, "none"),
    ] {
        let good = json!(relca_core::report::check(&source));
        assert_eq!(
            good["cut"]["defer_effects"]["entries"][0]["withdraws"],
            withdraws
        );
        fixture_match::validate_deferred_metrics_wire(&good).unwrap();
        let mut missing = good;
        missing["cut"]["defer_effects"]["entries"][0]
            .as_object_mut()
            .unwrap()
            .remove("scope");
        assert!(
            fixture_match::validate_deferred_metrics_wire(&missing).is_err(),
            "missing scope key for {withdraws}"
        );
    }
}

#[test]
fn wire_scope_resources_require_known_sorted_unique_names() {
    let good = json!(relca_core::report::check(&source()));
    fixture_match::validate_deferred_metrics_wire(&good).unwrap();
    for resources in [
        json!(["Unknown"]),
        json!(["Desk", "Desk"]),
        json!(["Other", "Desk"]),
    ] {
        let mut bad = good.clone();
        bad["cut"]["defer_effects"]["entries"][0]["scope"]["resources"] = resources;
        assert!(fixture_match::validate_deferred_metrics_wire(&bad).is_err());
    }
}

#[test]
fn metrics_without_resource_inventory_is_an_empty_inventory() {
    let good = json!(relca_core::report::check(&source()));
    assert_eq!(
        good["cut"]["defer_effects"]["entries"][0]["scope"]["resources"],
        json!(["Desk"])
    );
    let mut bad = good;
    // metrics omits resources only when the model has none; a named scoped
    // resource must then be rejected rather than left to source evidence.
    bad["metrics"].as_object_mut().unwrap().remove("resources");
    assert!(fixture_match::validate_deferred_metrics_wire(&bad).is_err());
}

#[test]
fn source_scope_resources_reject_a_different_known_resource() {
    let source = source();
    let good = json!(relca_core::report::check(&source));
    fixture_match::validate_deferred_metrics_with_source(&good, &source, None).unwrap();
    let mut bad = good;
    bad["cut"]["defer_effects"]["entries"][0]["scope"]["resources"] = json!(["Other"]);
    // Wire-only cannot know which operation uses Other; the source path must.
    fixture_match::validate_deferred_metrics_wire(&bad).unwrap();
    let error =
        fixture_match::validate_deferred_metrics_with_source(&bad, &source, None).unwrap_err();
    assert!(error.contains("source resource inventory"), "{error}");
}

#[test]
fn quality_only_scoped_resources_need_source_for_name_attribution() {
    let source = hand::source(4)
        .lines()
        .filter(|line| {
            !line.trim_start().starts_with("metrics {") && !line.trim_start().starts_with("metric ")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let report = relca_core::report::check(&source);
    assert_eq!(report.verdict, relca_core::Verdict::Satisfied);
    let good = json!(report);
    assert!(good.get("metrics").is_none());
    assert!(good["quality"].is_object());
    assert_eq!(good["cut"]["defer_effects"], hand::expected(4).unwrap());
    fixture_match::validate_deferred_metrics_wire(&good).unwrap();
    fixture_match::validate_deferred_metrics_with_source(&good, &source, None).unwrap();

    let mut wrong = good.clone();
    wrong["cut"]["defer_effects"]["entries"][0]["scope"]["resources"] = json!(["Other"]);
    // No inventory is not an empty inventory: names need source evidence here.
    fixture_match::validate_deferred_metrics_wire(&wrong).unwrap();
    let error =
        fixture_match::validate_deferred_metrics_with_source(&wrong, &source, None).unwrap_err();
    assert!(error.contains("source resource inventory"), "{error}");
    for resources in [json!(["Desk", "Desk"]), json!(["Other", "Desk"])] {
        let mut bad_order = good.clone();
        bad_order["cut"]["defer_effects"]["entries"][0]["scope"]["resources"] = resources;
        assert!(fixture_match::validate_deferred_metrics_wire(&bad_order).is_err());
    }
}

#[test]
fn compare_projection_validates_resources_before_discarding_the_inventory() {
    let source = hand::comparison(false).replace(
        "resource Desk capacity 1",
        "resource Desk capacity 1\nresource Other capacity 1",
    );
    for mode in [
        relca_core::compare::Analysis::Exhaustive,
        relca_core::compare::Analysis::Simulation,
    ] {
        let good = json!(relca_core::compare::compare(
            &source,
            &["Included", "Unrelated"],
            mode
        ));
        fixture_match::validate_compare_wire(&good).unwrap();
        let mut bad = good;
        bad["plans"][1]["report"]["cut"]["defer_effects"]["entries"][0]["scope"]["resources"] =
            json!(["Unknown"]);
        let error = fixture_match::validate_compare_wire(&bad).unwrap_err();
        assert!(error.contains("absent from report inventory"), "{error}");
    }
}
