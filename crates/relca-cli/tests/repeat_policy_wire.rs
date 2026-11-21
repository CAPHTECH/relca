use relca_test_support::fixture_match::Invocation;
use serde_json::{Value, json};

#[test]
fn repeat_and_response_tail_is_closed_in_the_arrivals_wire() {
    let base = include_str!("../../../fixtures/mean-repeat-policy/model.relca");
    let marker = "  verify probability";
    assert_eq!(base.matches(marker).count(), 1);
    // The two cases finish seven and eight minutes from receipt, before the
    // twelve-minute deadline. The response ledger and repeat both contribute
    // a different unchecked Agency item.
    let src = base.replacen(marker, "  contract Due { response delivery: case.received -> one_of(end.done) by 12min after case.received otherwise unresolved }\n  verify probability", 1);
    let r = relca_core::report::check_with_selection(&src, Some("Loop"), None);
    let raw = serde_json::to_value(r).unwrap();
    let invocation = Invocation::parse(None).unwrap();
    let expected = json!({"verdict":"satisfied"});
    invocation.compare(&expected, &raw).unwrap();
    assert!(!raw["obligations"].as_array().unwrap().is_empty());
    let tail = raw["agency"]["unchecked"].as_array().unwrap();
    let i = tail.len() - 2;
    assert_eq!(
        tail[i..],
        [json!("obligation_debtor"), json!("loop_control")]
    );

    for change in ["swap", "missing"] {
        let mut bad = raw.clone();
        let tail = bad["agency"]["unchecked"].as_array_mut().unwrap();
        if change == "swap" {
            tail.swap(i, i + 1);
        } else {
            tail.remove(i);
        }
        assert!(
            invocation
                .compare(&expected, &bad)
                .unwrap_err()
                .contains("Agency unchecked marker"),
            "{change}"
        );
    }
    let mut bad = raw.clone();
    let rows = bad["assumptions"].as_array_mut().unwrap();
    let row = rows
        .iter_mut()
        .find(|r| r["field"] == "via_model_paths")
        .unwrap();
    row["affects"] = Value::String("fabricated description".into());
    assert!(
        invocation
            .compare(&expected, &bad)
            .unwrap_err()
            .contains("assumption")
    );
}
