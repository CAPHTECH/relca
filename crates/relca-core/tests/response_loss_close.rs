#[path = "support/response_loss_close_inputs.rs"]
mod input;
#[path = "support/response_loss_review_inputs.rs"]
mod review;
use relca_core::{Verdict, engine, ir, lower, reference, report, syntax};
use serde_json::{Value, json};

#[test]
fn mixed_known_other_end_and_unknown_is_inconclusive_in_both_interpreters() {
    let m = lower::lower(&syntax::parse(input::MIXED).unwrap()).unwrap();
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for o in [engine::explore(&m), r.outcome] {
        assert!(!o.truncated);
        let p = o
            .properties
            .iter()
            .find(|p| matches!(p.property, ir::Property::ReachableEnd(_)))
            .unwrap();
        assert_eq!(p.verdict, Verdict::Inconclusive);
    }
    let v = json!(report::check(input::MIXED));
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["bounds"]["truncated"], false);
    let ts = v["case_outcomes"][0]["terminals"].as_array().unwrap();
    assert_eq!(ts.len(), 2);
    assert!(ts.iter().any(|t| t["kind"] == "end"
        && t["end"] == "rejected"
        && t["elapsed"] == json!({"min":1,"max":1})));
    assert!(ts.iter().any(|t| t["kind"] == "receipt_unknown"
        && t["end"].is_null()
        && t["completed_at"].is_null()
        && t["observed_work"] == json!({"min":1,"max":1})));
    assert!(
        v["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["end"] == "rejected")
    );
}

#[test]
fn alias_only_common_cause_becomes_none_not_empty_table() {
    let r = report::check(input::SINGLE_COMMON);
    assert_eq!(r.verdict, Verdict::Satisfied, "{:?}", r.diagnostics);
    assert!(
        r.agency
            .as_ref()
            .unwrap()
            .as_derived()
            .unwrap()
            .common_causes
            .is_none(),
        "{:?}",
        r.agency
            .as_ref()
            .unwrap()
            .as_derived()
            .unwrap()
            .common_causes
    );
    assert!(json!(r)["agency"].get("common_causes").is_none());
    // No actual pair exists even without a response alias.
    let ordinary = report::check(&input::SINGLE_COMMON.replace(" response_loss none", ""));
    assert!(
        ordinary
            .agency
            .unwrap()
            .as_derived()
            .unwrap()
            .common_causes
            .is_none()
    );
}

#[test]
fn wlin_diagnostics_do_not_leak_response_alias_names() {
    let ordinary = json!(report::check(&review::warning("W-LIN-2", false)));
    assert!(
        ordinary["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "W-LIN-2" && d["message"].as_str().unwrap().contains("bV"))
    );
    let response = json!(report::check(&review::warning("W-LIN-2", true)));
    assert_eq!(response["verdict"], "satisfied");
    let lin: Vec<&Value> = response["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"].as_str().unwrap().starts_with("W-LIN-"))
        .collect();
    assert!(lin.is_empty(), "{lin:?}");
    assert!(
        response["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"] == "agency.diagnostics.W-LIN-2"
                && a["value"] == "unchecked(response_control)")
    );
}
