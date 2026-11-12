//! Implementation-side adapters call these fixed assertions with their own
//! independent reducer. This file never implements quality arithmetic itself.
#![allow(dead_code)]
use serde_json::Value;

pub fn endpoint_contract(mut reduce: impl FnMut(&Value) -> Value) {
    let controls: Value =
        serde_json::from_str(include_str!("quality_private_oracles.json")).unwrap();
    for row in controls["endpoint_controls"].as_array().unwrap() {
        let actual = reduce(&row["endpoints"]);
        assert_eq!(
            actual["unknown"], row["unknown"],
            "both endpoint pairs must be inspected: {row}"
        );
        if let Some(basis) = row.get("basis") {
            assert_eq!(&actual["basis"], basis, "exact only if b0=b3");
        }
        if let Some(reason) = row.get("reason") {
            assert_eq!(&actual["reason"], reason, "computational unknown");
        }
    }
}

pub fn replication_contract(mut aggregate: impl FnMut(&Value) -> Value) {
    let controls: Value =
        serde_json::from_str(include_str!("quality_private_oracles.json")).unwrap();
    for row in controls["replication_controls"].as_array().unwrap() {
        let actual = aggregate(row);
        assert_eq!(
            actual["point"], row["point"],
            "ratio of sums,not average of ratios: {row}"
        );
        for key in ["numerator_sample", "denominator_sample", "reason", "bounds"] {
            if let Some(want) = row.get(key) {
                assert_eq!(&actual[key], want, "{key}: {row}");
            }
        }
    }
}
