//! Input census, independent IR arithmetic, and hand-fixed positive obligations.
use relca_core::{ast, lower, syntax};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[path = "support/relation_producer_read_oracle.rs"]
mod read_oracle;
use read_oracle::inventory;

fn census(case: &Value) -> Value {
    let src = case["source"].as_str().unwrap();
    let Ok(parsed) = syntax::parse(src) else {
        return json!({"status":"rejected","rows":[]});
    };
    let scenario = case["scenario"].as_str();
    let (Ok(a), Ok(b)) = (
        lower::lower_configuration_with_scenario(&parsed, case["from"].as_str().unwrap(), scenario),
        lower::lower_configuration_with_scenario(&parsed, case["to"].as_str().unwrap(), scenario),
    ) else {
        return json!({"status":"rejected","rows":[]});
    };
    let ac = a.model.cut.as_ref().unwrap();
    let bc = b.model.cut.as_ref().unwrap();
    if ac.name != bc.name || ac.horizon != bc.horizon {
        return json!({"status":"mismatch","rows":[]});
    }
    let before = inventory(&a.model);
    let after = inventory(&b.model);
    let mut rows = vec![];
    for (target, from) in before {
        let Some(to) = after.get(&target).filter(|to| **to != from) else {
            continue;
        };
        let mut covered = false;
        let mut annotated = false;
        for intent in &b.intent {
            match intent {
                ast::Intent::Claim {
                    target: t, loses, ..
                } if t.kind == ast::DiffKind::Relation && t.name() == target => {
                    covered = true;
                    annotated |= loses.is_some();
                }
                ast::Intent::Preserve { on, .. } => {
                    let scoped = on
                        .as_ref()
                        .is_some_and(|t| t.kind == ast::DiffKind::Relation && t.name() == target);
                    covered |= scoped;
                    annotated |= on.is_none() || scoped;
                }
                _ => {}
            }
        }
        // New binding warning messages cannot be conflated with old Actor rows.
        assert!(
            !(a.actors
                .symmetric_difference(&b.actors)
                .any(|actor| actor == &target)),
            "Actor/binding warning collision needs a separate hand baseline"
        );
        rows.push(json!({"target":target,"from":from,"to":to,"w1":usize::from(!covered),"w3":usize::from(!annotated)}));
    }
    json!({"status":"compared","rows":rows})
}

#[test]
fn all_inline_inputs_have_a_closed_independent_ir_count_and_hand_positive_rows() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data: Value = serde_json::from_slice(
        &std::fs::read(root.join("tests/support/relation_producer_inline_census.json")).unwrap(),
    )
    .unwrap();
    let cases = data["cases"].as_array().unwrap();
    assert_eq!(
        data["counts"],
        json!({"total":367,"positive":48,"zero":319,"rows":49})
    );
    let (mut positive, mut zero, mut rows) = (0, 0, 0);
    let mut ids = BTreeSet::new();
    for case in cases {
        let fingerprint = serde_json::to_vec(&json!([
            case["source"],
            case["from"],
            case["to"],
            case["scenario"]
        ]))
        .unwrap();
        let id: String = Sha256::digest(fingerprint)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(case["id"], id);
        assert!(ids.insert(id.clone()), "duplicate input {id}");
        let expected = case["expected"].as_array().unwrap();
        assert_eq!(
            census(case)["rows"],
            case["expected"],
            "independent IR {id}"
        );
        if expected.is_empty() {
            zero += 1;
        } else {
            positive += 1;
            rows += expected.len();
        }
        let actual = serde_json::to_value(relca_core::diff::diff_with_scenario(
            case["source"].as_str().unwrap(),
            case["from"].as_str().unwrap(),
            case["to"].as_str().unwrap(),
            case["scenario"].as_str(),
        ))
        .unwrap();
        let producers: Vec<_> = actual["differences"].as_array().unwrap().iter()
            .filter(|r| r["kind"] == "relation" && r["change"] == "modified")
            .map(|r| json!({"target":r["target"],"from":r["from"]["producer"],"to":r["to"]["producer"]})).collect();
        let hand: Vec<_> = expected
            .iter()
            .map(|r| json!({"target":r["target"],"from":r["from"],"to":r["to"]}))
            .collect();
        assert_eq!(producers, hand, "production rows {id}");
        for row in expected {
            for (field, code, message) in [
                (
                    "w1",
                    "W-DIFF-1",
                    format!(
                        "computed change for {} has no intent",
                        row["target"].as_str().unwrap()
                    ),
                ),
                (
                    "w3",
                    "W-DIFF-3",
                    format!(
                        "computed change for relation {} has no preserve, override, maintain or loses annotation",
                        row["target"].as_str().unwrap()
                    ),
                ),
            ] {
                let count = actual["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|d| d["code"] == code && d["message"] == message)
                    .count();
                assert_eq!(
                    count as u64,
                    row[field].as_u64().unwrap(),
                    "warning increment {id}/{field}"
                );
            }
        }
    }
    closed_counts(cases.len(), positive, zero, rows, &data["counts"]);
}

fn closed_counts(total: usize, positive: usize, zero: usize, rows: usize, expected: &Value) {
    assert_eq!(positive + zero, total, "closed input census");
    assert_eq!(
        json!({"total":total,"positive":positive,"zero":zero,"rows":rows}),
        *expected
    );
}

#[test]
fn census_count_controls_reject_a_dropped_input_or_a_changed_classification() {
    let expected = json!({"total":367,"positive":48,"zero":319,"rows":49});
    closed_counts(367, 48, 319, 49, &expected);
    assert!(std::panic::catch_unwind(|| closed_counts(366, 48, 318, 49, &expected)).is_err());
    assert!(std::panic::catch_unwind(|| closed_counts(367, 47, 319, 49, &expected)).is_err());
}

#[test]
fn independent_inline_367_census_recounts_without_production_diff() {
    let data: Value =
        serde_json::from_str(include_str!("support/relation_producer_inline_census.json")).unwrap();
    let cases = data["cases"].as_array().unwrap();
    let (mut positive, mut zero, mut rows) = (0, 0, 0);
    let (mut compared_zero, mut rejected, mut mismatch) = (0, 0, 0);
    let (mut w1, mut w3) = (0, 0);
    for case in cases {
        let independent = census(case);
        assert_eq!(independent["rows"], case["expected"], "{}", case["id"]);
        let n = independent["rows"].as_array().unwrap().len();
        for row in independent["rows"].as_array().unwrap() {
            w1 += row["w1"].as_u64().unwrap();
            w3 += row["w3"].as_u64().unwrap();
        }
        if n == 0 {
            zero += 1;
            match independent["status"].as_str().unwrap() {
                "compared" => compared_zero += 1,
                "rejected" => rejected += 1,
                "mismatch" => mismatch += 1,
                unknown => panic!("unclassified census status {unknown}"),
            }
        } else {
            positive += 1;
            rows += n;
        }
    }
    closed_counts(
        cases.len(),
        positive,
        zero,
        rows,
        &json!({"total":367,"positive":48,"zero":319,"rows":49}),
    );
    assert_eq!((compared_zero, rejected, mismatch), (232, 70, 17));
    assert_eq!((w1, w3), (47, 46));
}
