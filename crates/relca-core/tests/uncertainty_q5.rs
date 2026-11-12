//! E9b transport across the accepted Q5 inputs, not another priority scheduler.
#[path = "support/q5_oracle.rs"]
mod hand;
#[path = "support/q5_inputs.rs"]
mod input;
use relca_core::{report, uncertainty::Uncertainty};
use serde_json::{Value, json};

#[test]
fn real_q5_start_external_and_finish_keep_hand_values_and_model_context() {
    for (name, source) in [
        (
            "start",
            include_str!("../../relca-cli/tests/snapshots/inputs/q5-start.relca"),
        ),
        (
            "external",
            include_str!("../../relca-cli/tests/snapshots/inputs/q5-external.relca"),
        ),
        (
            "conflict",
            include_str!("../../relca-cli/tests/snapshots/inputs/q5-conflict.relca"),
        ),
    ] {
        let value = json!(report::check(source));
        hand::validate(name, &value).unwrap();
        assert!(
            !value["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-LEDGER-0")
        );
        let u = &value["uncertainty"];
        let typed: Uncertainty = serde_json::from_value(u.clone()).unwrap();
        assert_eq!(json!(typed), *u);
        assert_eq!(
            u["aleatory"],
            json!({"status":"not_configured","reason":"no_probabilistic_model","items":[]})
        );
        assert_eq!(
            u["estimation"],
            json!({"status":"not_applicable","reason":"exhaustive_analysis","items":[]})
        );
        assert_eq!(u["model"]["status"], "available");
        assert!(
            u["model"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| *i == json!({"kind":"model_context","at":"/priority_policy"}))
        );
        for item in u["model"]["items"].as_array().unwrap() {
            if let Some(at) = item["at"].as_str() {
                assert!(value.pointer(at).is_some(), "{name}: {at}");
            }
        }
        let components = value["ledger"]["derivation"]["components"]
            .as_array()
            .unwrap();
        for (path, version) in [
            ("/priority_policy/rules", "priority-queues-v1"),
            ("/uncertainty/rules", "uncertainty-p2-1"),
        ] {
            assert!(
                components
                    .iter()
                    .any(|c| c["at"] == path && c["version"] == version)
            );
        }
        let report: report::Report = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(json!(report), value);
        let mut stale = value;
        stale["priority_policy"] = Value::Null;
        assert!(
            serde_json::from_value::<report::Report>(stale).is_err(),
            "stale priority reference: {name}"
        );
    }
}
