//! Calibration of the narrow projection used by pre-existing acceptance tests.
use relca_test_support::stage_projection::{bytes, legacy_dto, observation, project};
use serde_json::json;

#[test]
fn raw_projection_keeps_old_bytes_and_cannot_hide_existing_value_or_spacing_changes() {
    let old = br#"{"agency":{"viewpoints":{"observation":{"status":"derived","sources":[],"people":[],"fallback":"unchecked"}}},"n":1}
"#;
    let new = br#"{"agency":{"viewpoints":{"observation":{"status":"derived","sources":[],"people":[],"fallback":"unchecked","stages":[]}}},"n":1}
"#;
    assert_eq!(bytes(new), old);
    for changed in [
        String::from_utf8(new.to_vec())
            .unwrap()
            .replace("\"n\":1", "\"n\":2"),
        String::from_utf8(new.to_vec())
            .unwrap()
            .replace("\"n\":1", "\"n\": 1"),
        String::from_utf8(new.to_vec())
            .unwrap()
            .replace("\"people\":[]", "\"people\":[\"p\"]"),
    ] {
        assert_ne!(bytes(changed.as_bytes()), old);
    }
}

#[test]
fn null_middle_fields_and_exact_unchecked_words_do_not_remove_unrelated_names() {
    let old = br#"{"responsibilities":[{"observation":{"status":"not_derived","required_sources":[],"reason":"merge_control"}}],"unchecked":["legacy"],"other":{"stages":[]}}
"#;
    let new = br#"{"responsibilities":[{"observation":{"stages_status":"not_derived","status":"not_derived","stages":null,"required_sources":[],"reason":"merge_control","stages_reason":"merge_control"}}],"unchecked":["legacy","stage_observation(stage_correspondence_not_defined)"],"other":{"stages":[]}}
"#;
    assert_eq!(bytes(new), old);
    let value: serde_json::Value = serde_json::from_slice(new).unwrap();
    assert_eq!(
        project(&value),
        serde_json::from_slice::<serde_json::Value>(old).unwrap()
    );
    let unrelated = json!({"other":{"stages":[]},"unchecked":["stage_observation(not_approved)"]});
    assert_eq!(project(&unrelated), unrelated);
}

#[test]
fn strict_observation_projection_rejects_missing_new_fields_instead_of_empty_removal() {
    for missing in [
        json!({"status":"derived","sources":[],"people":[],"fallback":"unchecked"}),
        json!({"status":"present","required_sources":["Doc"],"stages":[]}),
        json!({"status":"not_derived","required_sources":[],"stages":null,"stages_status":"not_derived"}),
    ] {
        assert!(std::panic::catch_unwind(|| observation(&missing)).is_err());
    }
}

#[test]
fn legacy_dto_removes_only_new_defaults_and_current_roundtrip_keeps_stage_payload() {
    let old =
        json!({"observation":{"status":"present","required_sources":["Doc"],"direct":["Doc"]}});
    let mut decoded = old.clone();
    decoded["observation"]["stages"] = serde_json::Value::Null;
    decoded["observation"]["stages_status"] = json!("");
    assert_eq!(legacy_dto(&decoded, &old), old);
    decoded["observation"]["direct"] = json!([]);
    assert_ne!(legacy_dto(&decoded, &old), old);
    let current = json!({"observation":{"status":"derived","sources":[],"stages":[{"call":"v","direct":["Doc"]}]}});
    let mut wrong = current.clone();
    wrong["observation"]["stages"][0]["direct"] = json!([]);
    assert_ne!(legacy_dto(&wrong, &current), current);
}

#[test]
fn malformed_new_marker_is_left_for_old_closed_validator_not_panicked_or_hidden() {
    for value in [
        json!({"status":"not_derived","sources":[],"stages":[]}),
        json!({"status":"present","required_sources":[],"stages":[],"stages_status":"unknown"}),
    ] {
        assert!(relca_test_support::stage_projection::marked_observation(&value).is_none());
        assert_eq!(project(&value), value);
    }
}
