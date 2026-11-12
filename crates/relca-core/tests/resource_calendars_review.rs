//! Review checkpoint: regressions are intentionally RED on 9b8c12aa.
#[path = "support/q4_oracle.rs"]
mod hand;
#[path = "support/q4_inputs.rs"]
mod input;
#[path = "support/q4_review.rs"]
mod review;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

#[test]
fn m1_properties_use_samples_not_first_support_integer() {
    let mut failures = Vec::new();
    for name in ["arrivals", "arrivals-burst", "arrivals-calendar"] {
        let value = json!(report::simulate(&input::source(name)));
        let expected = hand::expected(name);
        if let Err(error) = hand::subset(&expected["properties"], &value["properties"], name) {
            failures.push(error);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn m2_condition_wait_withdraws_responsibility_decision_support() {
    let mut failures = Vec::new();
    for dataflow in [false, true] {
        let value = json!(report::check(&review::condition_source(dataflow)));
        assert_eq!(value["verdict"], "violated");
        assert!(
            value["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "W-DEADLOCK-0")
        );
        if let Err(error) = review::condition_withdrawal(&value) {
            failures.push(format!("dataflow={dataflow}: {error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn green_m2_if_control_keeps_existing_decision_support() {
    for dataflow in [false, true] {
        let value = json!(report::check(&review::branch_control(dataflow)));
        assert_eq!(value["verdict"], "satisfied");
        let rows = value["responsibilities"].as_array().unwrap();
        let approval = rows
            .iter()
            .find(|r| r["target"]["value"] == "Approver.approve")
            .unwrap();
        assert_eq!(approval["refusal"]["status"], "present");
        assert_eq!(approval["refusal"]["reason"], Value::Null);
        hand::subset(
            &json!([{"capability":"Ledger.post","call":"posted","status":"all_paths",
        "judgements":[{"capability":"Approver.approve","call":"approval","stops":"all_paths"}]}]),
            &approval["refusal"]["effects"],
            "if control",
        )
        .unwrap();
    }
}

#[test]
fn m3_scenario_drain_uses_full_support_not_first_row() {
    let mut failures = Vec::new();
    for (last, reverse) in [(4, false), (4, true), (9, false)] {
        let value = json!(report::simulate_with_selection(
            &review::scenario_arrivals(last, reverse),
            None,
            Some("Day")
        ));
        assert_eq!(value["verdict"], "satisfied");
        if let Err(error) = review::drain(&value, last) {
            failures.push(format!("last={last}, reverse={reverse}: {error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn green_review_oracles_reject_single_fault_witnesses() {
    review::control_pairs();
    for (name, description, bad) in hand::mutation_controls() {
        hand::validate(name, &hand::expected(name)).unwrap();
        assert!(hand::validate(name, &bad).is_err(), "{description}");
    }
}

#[test]
fn green_review_sources_have_single_edit_witnesses() {
    for dataflow in [false, true] {
        assert_eq!(
            input::edit(
                &review::condition_source(dataflow),
                "wait until approval == pass",
                "if approval == pass { } else { end returned }"
            ),
            review::branch_control(dataflow)
        );
    }
    for ordered in [false, true] {
        assert_eq!(
            input::edit(
                &review::queued_source(ordered, true),
                "remaining cancel_then_drain",
                "remaining drain"
            ),
            review::queued_source(ordered, false)
        );
    }
    assert_eq!(
        input::edit(
            &review::receipt_source(false),
            "response_loss sequence [unknown]",
            "response_loss sequence [delivered]"
        ),
        review::receipt_source(true)
    );
    assert_eq!(
        input::edit(
            &review::scenario_arrivals(4, false),
            "0.5: fixed [0min, 4min]",
            "0.5: fixed [0min, 9min]"
        ),
        review::scenario_arrivals(9, false)
    );
}

#[test]
fn green_m2_both_interpreters_observe_bound_result_wait_stopping_the_effect() {
    for dataflow in [false, true] {
        let source = review::condition_source(dataflow);
        let model = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
        let engine = engine::explore(&model);
        let mut reference = reference::explore(&model);
        assert_eq!(reference.stop, reference::StopReason::Complete);
        for outcome in [&engine, &reference.outcome] {
            assert!(!outcome.truncated);
            assert!(outcome.diagnostics.iter().any(|d| d.code == "W-DEADLOCK-0"));
            let cases = outcome.case_outcomes.as_ref().unwrap();
            assert_eq!(cases.len(), 1);
            assert_eq!(cases[0].terminals.len(), 1);
            let end = &cases[0].terminals[0];
            assert!(matches!(
                end.kind,
                relca_core::outcome::CaseTerminalKind::End(_)
            ));
            assert_eq!(end.completed_at.unwrap().min, 3);
            assert_eq!(end.completed_at.unwrap().max, 3);
            assert_eq!(end.work.unwrap().min, 3);
            assert_eq!(end.work.unwrap().max, 3);
        }
        assert!(
            reference
                .outcome
                .diagnostics
                .starts_with(&model.diagnostics)
        );
        reference
            .outcome
            .diagnostics
            .drain(..model.diagnostics.len());
        assert_eq!(engine, reference.outcome);
    }
}

#[test]
fn green_m1_stopped_queue_boundary_has_matching_interpreters_and_cancel_drain_pair() {
    for ordered in [false, true] {
        for cancel in [true, false] {
            let source = review::queued_source(ordered, cancel);
            let model = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
            let engine = engine::explore(&model);
            let mut reference = reference::explore(&model);
            assert_eq!(reference.stop, reference::StopReason::Complete);
            for (which, outcome) in [("engine", &engine), ("reference", &reference.outcome)] {
                assert!(
                    !outcome.truncated,
                    "{which}, ordered={ordered}, cancel={cancel}"
                );
                let expected = if cancel {
                    [(1, 1), (3, 1)]
                } else {
                    [(7, 3), (10, 3)]
                };
                let cases = outcome.case_outcomes.as_ref().unwrap();
                assert_eq!(cases.len(), 2);
                for (case, (end, work)) in cases.iter().zip(expected) {
                    assert_eq!(case.terminals.len(), 1);
                    hand::subset(&json!({"completed_at":{"min":end,"max":end}, "work":{"min":work,"max":work}}),
                        &json!({"completed_at":case.terminals[0].completed_at, "work":case.terminals[0].work}), which).unwrap();
                }
                let metrics = json!(outcome.metrics);
                for case in metrics["cases"].as_array().unwrap() {
                    hand::subset(
                        &json!([{"binding":"b","starts":hand::scalar(if cancel {0} else {1}),
                        "work":hand::scalar(if cancel {0} else {2})}]),
                        &case["tasks"],
                        which,
                    )
                    .unwrap();
                }
            }
            assert!(
                reference
                    .outcome
                    .diagnostics
                    .starts_with(&model.diagnostics)
            );
            reference
                .outcome
                .diagnostics
                .drain(..model.diagnostics.len());
            assert_eq!(
                engine, reference.outcome,
                "ordered={ordered}, cancel={cancel}"
            );
        }
    }
}

#[test]
fn green_m1_receipt_stop_does_not_leave_queue_and_block_later_calendar_start() {
    for delivered in [false, true] {
        let source = review::receipt_source(delivered);
        let model = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
        let engine = engine::explore(&model);
        let mut reference = reference::explore(&model);
        assert_eq!(reference.stop, reference::StopReason::Complete);
        for outcome in [&engine, &reference.outcome] {
            assert!(!outcome.truncated);
            let cases = outcome.case_outcomes.as_ref().unwrap();
            assert_eq!(cases.len(), 2);
            for (case, at) in cases.iter().zip([1, 5]) {
                assert_eq!(case.terminals.len(), 1);
                if delivered {
                    assert_eq!(case.terminals[0].work.unwrap().min, 1);
                    assert_eq!(case.terminals[0].work.unwrap().max, 1);
                    assert!(matches!(
                        case.terminals[0].kind,
                        relca_core::outcome::CaseTerminalKind::End(_)
                    ));
                    assert_eq!(case.terminals[0].completed_at.unwrap().min, at);
                    assert_eq!(case.terminals[0].completed_at.unwrap().max, at);
                } else {
                    assert_eq!(
                        case.terminals[0].kind,
                        relca_core::outcome::CaseTerminalKind::ReceiptUnknown
                    );
                }
            }
            let receipts = json!(outcome.response_receipts);
            hand::subset(&json!([
                {"case_id":1,"attempt":1,"at":hand::point(1),"receipt":if delivered {"delivered"} else {"unknown"}},
                {"case_id":2,"attempt":1,"at":hand::point(5),"receipt":if delivered {"delivered"} else {"unknown"}}
            ]), &receipts, "receipt stop pair").unwrap();
        }
        assert!(
            reference
                .outcome
                .diagnostics
                .starts_with(&model.diagnostics)
        );
        reference
            .outcome
            .diagnostics
            .drain(..model.diagnostics.len());
        assert_eq!(engine, reference.outcome, "delivered={delivered}");
    }
}
