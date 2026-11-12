//! Q4/Q7 seam: existing hand-inputs, not captured output as a new oracle.
#[path = "support/q4_close.rs"]
mod close;
#[path = "support/q4_review.rs"]
mod review;
use relca_core::{diff, report, uncertainty::Uncertainty};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn checked(value: &Value) {
    let uncertainty: Uncertainty = serde_json::from_value(value["uncertainty"].clone()).unwrap();
    assert_eq!(json!(uncertainty), value["uncertainty"]);
    assert!(
        !value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-LEDGER-0")
    );
    for category in ["aleatory", "estimation", "model"] {
        for item in value["uncertainty"][category]["items"].as_array().unwrap() {
            if let Some(at) = item.as_str().or_else(|| item["at"].as_str()) {
                assert!(value.pointer(at).is_some(), "missing {at}");
            }
        }
    }
    assert!(
        value["ledger"]["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["target"] == "/uncertainty")
    );
    assert!(
        value["ledger"]["derivation"]["components"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "/uncertainty/rules" && c["version"] == "uncertainty-p2-1")
    );
}

fn quantities(value: &Value, at: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if at.is_empty() && matches!(key.as_str(), "ledger" | "uncertainty") {
                    continue;
                }
                let path = format!("{at}/{key}");
                if ["calendars", "calendar", "wait_causes", "arrival_time"].contains(&key.as_str())
                {
                    out.insert(path.clone());
                }
                quantities(value, &path, out);
            }
        }
        Value::Array(rows) => {
            for (i, row) in rows.iter().enumerate() {
                quantities(row, &format!("{at}/{i}"), out);
            }
        }
        _ => {}
    }
}

#[test]
fn calendar_and_wait_causes_are_model_context_not_randomness_or_ci() {
    let value = json!(report::check(include_str!("support/q4/open.relca")));
    checked(&value);
    assert_eq!(value["verdict"], "satisfied");
    assert_eq!(value["case_outcomes"][0]["case_id"], 1);
    let terminal = &value["case_outcomes"][0]["terminals"][0];
    assert_eq!(terminal["end"], "done");
    assert_eq!(terminal["elapsed"], json!({"min":7,"max":7}));
    assert_eq!(terminal["work"], json!({"min":2,"max":2}));
    assert_eq!(
        value["uncertainty"]["aleatory"]["reason"],
        "no_probabilistic_model"
    );
    assert_eq!(
        value["uncertainty"]["estimation"]["reason"],
        "exhaustive_analysis"
    );
    let mut expected = BTreeSet::new();
    quantities(&value, "", &mut expected);
    assert!(expected.iter().any(|p| p.ends_with("/calendars")));
    assert!(expected.iter().any(|p| p.ends_with("/wait_causes")));
    let actual: BTreeSet<_> = value["uncertainty"]["model"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["at"].as_str().map(str::to_owned))
        .collect();
    assert!(expected.is_subset(&actual));
    assert!(serde_json::from_value::<report::Report>(value).is_ok());
}

#[test]
fn finite_arrival_input_mass_and_sampled_arrival_time_are_separate() {
    let value = json!(report::simulate(include_str!("support/q4/arrivals.relca")));
    checked(&value);
    let input = value["uncertainty"]["aleatory"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "arrival_distribution")
        .unwrap();
    assert_eq!(
        input["entries"],
        json!([
            {"times":[0,0],"probability":"1/2"}, {"times":[0,4],"probability":"1/2"}
        ])
    );
    let mut expected = BTreeSet::new();
    quantities(&value, "", &mut expected);
    let arrivals: BTreeSet<_> = expected
        .into_iter()
        .filter(|p| p.ends_with("/arrival_time"))
        .collect();
    assert!(!arrivals.is_empty());
    for at in arrivals {
        assert!(
            value["uncertainty"]["estimation"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["at"] == at && i["reason"] == "interval_not_configured")
        );
    }
    assert_eq!(value["simulation"]["replications"], 4);
}

#[test]
fn new_wait_until_refusals_keep_three_classes_and_real_diff_children() {
    for (from, to) in [(true, false), (false, true), (true, true)] {
        let value = json!(diff::diff(&close::diff(from, to), "From", "To"));
        assert_eq!(value["verdict"], "violated");
        assert!(value["diagnostics"].as_array().unwrap().iter().any(|d| {
            d["code"] == "U-SEM-0"
                && d["message"]
                    .as_str()
                    .unwrap()
                    .contains("condition_wait_diff_unsupported")
        }));
        checked(&value);
        for category in ["aleatory", "estimation", "model"] {
            assert_eq!(
                value["uncertainty"][category]["items"],
                json!([
                    format!("/from/report/uncertainty/{category}"),
                    format!("/to/report/uncertainty/{category}")
                ])
            );
        }
        assert!(serde_json::from_value::<diff::DiffReport>(value.clone()).is_ok());
        let mut bad = value;
        bad["uncertainty"]["model"]["items"][0] = json!("/to/report/uncertainty/model");
        assert!(serde_json::from_value::<diff::DiffReport>(bad).is_err());
    }
    for effect in [false, true] {
        for restricted in [false, true] {
            let value = json!(report::check(&close::contest(true, restricted, 1, effect)));
            close::refused(&value, "condition_wait_contest_unsupported");
            checked(&value);
            assert!(serde_json::from_value::<report::Report>(value).is_ok());
        }
    }
}

#[test]
fn closed_input_payloads_and_spans_reject_single_faults() {
    let value = json!(report::check(include_str!("data/sensitivity/base.relca")));
    checked(&value);
    let input = &value["uncertainty"];
    for (at, bad_value) in [
        ("/aleatory/items/0/span/extra", json!(0)),
        ("/aleatory/items/0/extra", json!(0)),
        ("/aleatory/items/0/entries/0/probability", json!("3/2")),
    ] {
        let mut bad = input.clone();
        // The first entry is the handwritten 1/50 initial distribution.
        let (parent, key) = at.rsplit_once('/').unwrap();
        bad.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), bad_value);
        assert!(
            serde_json::from_value::<Uncertainty>(bad).is_err(),
            "accepted {at}"
        );
    }
}
