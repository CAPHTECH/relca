//! Independent hand acceptance for Q1 pre-merge review; intentional RED is not ignored.
#[path = "support/response_loss_review_oracle.rs"]
mod hand;
#[path = "support/response_loss_review_inputs.rs"]
mod input;
use relca_core::{Verdict, engine, ir, lower, reference, report, syntax};
use serde_json::{Value, json};

fn check(s: &str) -> Value {
    json!(report::check(s))
}

macro_rules! m1 {
    ($test:ident, $kind:literal, $source:expr) => {
        #[test]
        fn $test() {
            hand::m1(&check(&$source), $kind);
        }
    };
}
m1!(
    m1_common_cause_excludes_nonexecuting_alias,
    "common",
    input::common(true)
);
m1!(
    m1_uncertainty_judgement_excludes_nonexecuting_alias,
    "judgement",
    input::basis(Some("judgement"))
);
m1!(
    m1_uncertainty_target_excludes_nonexecuting_alias,
    "target",
    input::basis(Some("target"))
);

#[test]
fn m1_ordinary_call_controls_keep_real_names_and_basis() {
    let c = check(&input::common(false));
    assert_eq!(c["verdict"], "satisfied");
    let targets = hand::rows(&c["agency"]["common_causes"]["causes"][0]["targets"]);
    assert_eq!(targets.len(), 2);
    assert!(targets.iter().any(|t| t["call"] == "c1"));
    let b = check(&input::basis(None));
    assert_eq!(b["verdict"], "satisfied");
    assert!(
        hand::rows(&b["agency"]["lineage"])
            .iter()
            .any(|r| r["call"] == "bV")
    );
    let r = &b["agency"]["model_uncertainties"]["items"][0];
    assert_eq!(r["call"], "bV");
    assert_eq!(r["targets"], json!(["e"]));
    assert_eq!(r["kind"], "wlin2_basis");
}

macro_rules! suppressed {
    ($test:ident, $code:literal) => {
        #[test]
        fn $test() {
            let v = check(&input::warning($code, true));
            assert_eq!(v["verdict"], "satisfied", "{}", v["diagnostics"]);
            hand::suppressed(&v, $code);
        }
    };
}
suppressed!(m1_warning_agy0_has_reasoned_unchecked_mark, "W-AGY-0");
suppressed!(m1_warning_agy2_has_reasoned_unchecked_mark, "W-AGY-2");
suppressed!(m1_warning_lin2_has_reasoned_unchecked_mark, "W-LIN-2");
suppressed!(m1_warning_resp2_has_reasoned_unchecked_mark, "W-RESP-2");

#[test]
fn m1_warning_ordinary_controls_really_trigger_all_four_diagnostics() {
    for code in input::SUPPRESSED {
        let v = check(&input::warning(code, false));
        assert_eq!(v["verdict"], "satisfied");
        assert!(hand::warning(&v, code), "control did not exercise {code}");
        assert!(
            !hand::has_mark(&v, code),
            "old model was not response-unchecked"
        );
    }
}

#[test]
fn m1_agy1_retained_with_single_edit_nonbypass_control() {
    for single in [false, true] {
        let v = check(&input::bypass(single));
        assert_eq!(v["verdict"], "satisfied", "{}", v["diagnostics"]);
        assert_eq!(hand::warning(&v, "W-AGY-1"), !single);
    }
}

#[test]
fn m1_agy3_retained_with_single_edit_no_fallback_control() {
    for keep in [true, false] {
        let v = json!(report::check_configuration(&input::fallback(keep), "To"));
        assert_eq!(v["verdict"], "satisfied", "{}", v["diagnostics"]);
        assert_eq!(hand::warning(&v, "W-AGY-3"), keep);
    }
}

#[test]
fn m2_via_model_path_withdrawal_is_not_silent() {
    hand::via(&check(&input::via(true, true)));
}
#[test]
fn m2_no_via_declaration_does_not_hide_response_derivation_limit() {
    hand::via(&check(&input::via(true, false)));
}
#[test]
fn m2_ordinary_call_control_keeps_nonempty_via_paths() {
    let v = check(&input::via(false, true));
    assert_eq!(v["verdict"], "satisfied");
    assert!(!hand::rows(&v["via_model_paths"]["paths"]).is_empty());
    assert!(!hand::has_mark(&v, "via_model_paths"));
}

macro_rules! reachable {
    ($test:ident, $receipt:literal, $fail:literal, $violation:literal) => {
        #[test]
        fn $test() {
            hand::reachable(
                &check(&input::reachable($receipt, $fail, $violation)),
                $receipt,
                $fail,
                $violation,
            );
        }
    };
}
reachable!(
    m3_unknown_only_reachable_is_inconclusive,
    "unknown",
    false,
    false
);
reachable!(m3_delivered_control_reaches_end, "delivered", false, false);
reachable!(
    m3_known_rejected_control_still_violates_reachable,
    "delivered",
    true,
    false
);
reachable!(
    m3_independent_violation_is_not_erased_by_unknown_reachability,
    "unknown",
    false,
    true
);

fn model() -> ir::Model {
    lower::lower(&syntax::parse(&input::reachable("unknown", false, false)).unwrap()).unwrap()
}
fn raw_reachable(m: &ir::Model, o: &relca_core::outcome::Outcome) {
    assert!(!o.truncated);
    let p = o
        .properties
        .iter()
        .find(|p| matches!(p.property, ir::Property::ReachableEnd(_)))
        .unwrap();
    assert_eq!(
        p.verdict,
        Verdict::Inconclusive,
        "raw reachable must not classify ReceiptUnknown as failed: {:?}",
        m.ends
    );
}
#[test]
fn m3_engine_reachable_keeps_unresolved_case() {
    let m = model();
    raw_reachable(&m, &engine::explore(&m));
}
#[test]
fn m3_independent_reference_reachable_keeps_unresolved_case() {
    let m = model();
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    raw_reachable(&m, &r.outcome);
}

#[test]
fn single_edit_model_pairs_are_nonnoop_and_exact() {
    assert_eq!(
        input::edit(
            &input::common(false),
            "c1 = A.check()",
            "c1 = A.check() response_loss none"
        ),
        input::common(true)
    );
    for (kind, call, suffix) in [
        ("judgement", "bV = B.check(e)", input::RETRY),
        ("target", "e = P.make()", "response_loss none"),
    ] {
        assert_eq!(
            input::edit(&input::basis(None), call, &format!("{call} {suffix}")),
            input::basis(Some(kind))
        );
    }
    for code in input::SUPPRESSED {
        assert_eq!(
            input::edit(&input::warning(code, true), " response_loss none", ""),
            input::warning(code, false)
        );
    }
    assert_eq!(
        input::edit(
            &input::via(false, true),
            "e = Extract.make()",
            "e = Extract.make() response_loss none"
        ),
        input::via(true, true)
    );
    assert_eq!(
        input::edit(
            &input::reachable("unknown", false, false),
            "[unknown]",
            "[delivered]"
        ),
        input::reachable("delivered", false, false)
    );
    assert_eq!(
        input::edit(
            &input::reachable("delivered", false, false),
            "result nondet { pass }",
            "result nondet { fail }"
        ),
        input::reachable("delivered", true, false)
    );
}

#[test]
fn hand_alias_checker_rejects_one_leaked_judgement_target_or_lineage_row() {
    let good = json!({"agency": {
        "lineage":[{"call":"attempt1","subject_sources":["S"]}],
        "common_causes":{"causes":[{"targets":[{"call":"attempt1"}]}]},
        "model_uncertainties":{"items":[{"call":"judge1","targets":["attempt1"]}]}
    }});
    assert!(hand::alias_free(&good, "alias"));
    for pointer in [
        "/agency/lineage/0/call",
        "/agency/common_causes/causes/0/targets/0/call",
        "/agency/model_uncertainties/items/0/call",
        "/agency/model_uncertainties/items/0/targets/0",
    ] {
        let mut broken = good.clone();
        *broken.pointer_mut(pointer).unwrap() = json!("alias");
        assert!(!hand::alias_free(&broken, "alias"), "missed {pointer}");
    }
    let allowed = json!({"agency":{"lineage":[], "common_causes":{
        "causes":[{"targets":[{"call":"alias"}],"status":"not_derived","reason":"response_control"}]}}});
    assert!(hand::alias_free(&allowed, "alias"));
}

#[test]
fn hand_reason_checker_rejects_deleted_mark_or_reason_and_generic_notice() {
    for subject in input::SUPPRESSED.iter().copied().chain(["via_model_paths"]) {
        let good = json!({"assumptions":[{"field":subject,"value":"unchecked","affects":"response_control"}]});
        assert!(hand::has_mark(&good, subject));
        let mut missing = good.clone();
        missing["assumptions"] = json!([]);
        assert!(!hand::has_mark(&missing, subject));
        let mut bad = good.clone();
        bad["assumptions"][0]["affects"] = json!("other_limit");
        assert!(!hand::has_mark(&bad, subject));
        let mut unmarked = good.clone();
        unmarked["assumptions"][0]["value"] = json!("derived");
        assert!(!hand::has_mark(&unmarked, subject));
    }
    assert!(!hand::has_mark(
        &json!({"assumptions":[{"field":"agency.response_control","value":"unchecked","affects":"response_control"}]}),
        "W-AGY-0"
    ));
}

#[test]
fn hand_reachable_checker_rejects_violated_for_unresolved_receipt() {
    let good = json!({"verdict":"inconclusive", "bounds":{"truncated":false}, "diagnostics":[],
        "properties":[{"id":"reachable end.done","verdict":"inconclusive"},{"id":"receipt_resolution","verdict":"inconclusive"}],
        "terminals":[], "case_outcomes":[{"terminals":[{"kind":"receipt_unknown","end":null,"completed_at":null,"elapsed":null,"work":null,
            "observed_until":{"min":1,"max":1},"observed_elapsed":{"min":1,"max":1},"observed_work":{"min":1,"max":1}}]}],
        "response_receipts":[{"receipt":"unknown","observed_result":null}]});
    assert_eq!(hand::reachable(&good, "unknown", false, false), 2);
    let mut broken = good.clone();
    broken["properties"][0]["verdict"] = json!("violated");
    assert!(
        std::panic::catch_unwind(|| hand::reachable(&broken, "unknown", false, false)).is_err()
    );
}
