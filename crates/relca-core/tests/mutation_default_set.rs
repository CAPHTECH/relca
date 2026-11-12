//! ADR0046 public acceptance. Private AST/comparison obligations are separate.
#[path = "support/mutation_default_acceptance.rs"]
mod hand;
use relca_core::{mutation, report};
use serde_json::Value;

#[test]
fn all_hand_pairs_originals_reach_intended_gate_not_an_accidental_parser_error() {
    for (name, e) in hand::oracle().as_object().unwrap() {
        let v = serde_json::to_value(report::check(&hand::source(name))).unwrap();
        assert_eq!(
            v["verdict"], e["expected"]["original_verdict"],
            "{name}: {}",
            v["diagnostics"]
        );
    }
}

#[test]
fn all_hand_pairs_generate_exact_new_sites_results_held_and_boundaries() {
    for name in hand::oracle().as_object().unwrap().keys() {
        let s = hand::source(name);
        let v: Value = serde_json::to_value(mutation::mutate(&s)).unwrap();
        hand::check(name, &v);
        assert_eq!(
            v["original"],
            serde_json::to_value(report::check(&s)).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn every_hand_pair_is_reported_even_when_other_pairs_are_red() {
    let mut failures = vec![];
    for name in hand::oracle().as_object().unwrap().keys() {
        if std::panic::catch_unwind(|| {
            hand::check(
                name,
                &serde_json::to_value(mutation::mutate(&hand::source(name))).unwrap(),
            )
        })
        .is_err()
        {
            failures.push(name.clone());
        }
    }
    assert!(failures.is_empty(), "RED fixtures: {}", failures.join(", "));
}

#[test]
fn argument_and_input_failures_block_six_rows_but_keep_four_reasons() {
    for report in [
        mutation::argument_failure("bad arguments"),
        mutation::input_failure("unreadable input"),
        mutation::mutate_set("not a model", "not-default"),
    ] {
        let v = serde_json::to_value(report).unwrap();
        assert_eq!(v["verdict"], "invalid");
        assert!(v["original"].is_null());
        for i in [0, 1, 2, 4, 8, 9] {
            assert_eq!(v["mutations"][i]["status"], "blocked");
            assert_eq!(v["mutations"][i]["reason"], v["run"]["reason"]);
            assert_eq!(v["mutations"][i]["mutants"], serde_json::json!([]));
        }
        assert!(
            v["unchecked"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("authority_expiry"))
        );
        assert!(
            !v["checked"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("authority_expiry"))
        );
    }
}
