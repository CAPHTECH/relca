//! Fixed denominator: 28 inputs = 24 runtime Complete + 4 static rejections.
#[path = "actor_table_cut_debtor.rs"]
mod acceptance;
use relca_core::{engine, lower, reference, syntax};
use serde_json::json;

#[test]
fn runtime24_complete24_excluded0_each_backend_first_matches_hand_arithmetic() {
    let mut complete = 0;
    let mut rejected = 0;
    for name in acceptance::NAMES {
        let hand = acceptance::expected(name);
        let src = acceptance::source(name);
        if hand["verdict"] == "invalid" {
            rejected += 1;
            assert!(syntax::parse(&src).and_then(|a| lower::lower(&a)).is_err());
            continue;
        }
        let model = lower::lower(&syntax::parse(&src).expect(name)).expect(name);
        // Arrivals at 60 and durations of 1440min walk deeper than the
        // reference defaults; widen the walk limits only, on a larger stack.
        let deep = model.clone();
        let r = std::thread::Builder::new()
            .stack_size(512 << 20)
            .spawn(move || {
                reference::explore_with_limits(
                    &deep,
                    reference::Limits {
                        steps_per_path: 100_000,
                        control_depth: 4096,
                        branch_depth: 4096,
                        ..reference::Limits::default()
                    },
                )
            })
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        let e = engine::explore(&model);
        for outcome in [&e, &r.outcome] {
            assert!(!outcome.truncated, "hand complete {name}");
            assert_eq!(
                outcome.properties.len(),
                hand["properties"].as_array().unwrap().len()
            );
            for p in &outcome.properties {
                assert_eq!(
                    serde_json::to_value(p.verdict).unwrap(),
                    "satisfied",
                    "{name}"
                );
            }
            let terminals:Vec<_>=outcome.terminals.iter().map(|t| json!({"end":model.ends[t.end.0],"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|r|json!({"min":r.min,"max":r.max}))})).collect();
            acceptance::subset(
                &json!({"terminals":hand["terminals"]}),
                &json!({"terminals":terminals}),
            );
            if name == "act-arrival-units" {
                let cases = outcome.case_outcomes.as_ref().unwrap();
                assert_eq!(cases.len(), 2);
                for (case, at) in cases.iter().zip([31, 61]) {
                    let t = &case.terminals[0];
                    let end = t.completed_at.unwrap();
                    assert_eq!((end.min, end.max), (at, at));
                    let w = t.work.unwrap();
                    assert_eq!((w.min, w.max), (1, 1));
                }
            }
            if hand.get("obligations").is_some() {
                acceptance::subset(
                    &hand["obligations"],
                    &serde_json::to_value(outcome.obligations.as_ref().unwrap()).unwrap(),
                );
            }
        }
        let mut reference = r.outcome;
        assert!(reference.diagnostics.starts_with(&model.diagnostics));
        reference.diagnostics.drain(..model.diagnostics.len());
        assert_eq!(e, reference, "whole runtime Outcome {name}");
        complete += 1;
    }
    assert_eq!((complete, rejected), (24, 4));
}

#[test]
fn metadata_pairs_preserve_reference_counters_and_every_small_engine_budget_boundary() {
    for (good, bad) in [
        ("act-grain", "act-grain.broken-swapped"),
        ("act-control", "act-control.broken-explicit-environment"),
        ("act-horizon", "act-horizon.broken-short-horizon"),
        ("act-debtor-value", "act-debtor-value.broken-other-person"),
    ] {
        let a = lower::lower(&syntax::parse(&acceptance::source(good)).unwrap()).unwrap();
        let b = lower::lower(&syntax::parse(&acceptance::source(bad)).unwrap()).unwrap();
        let ra = reference::explore(&a);
        let rb = reference::explore(&b);
        let counters = |s: &reference::Statistics| {
            (
                s.frames,
                s.terminal_paths,
                s.deadlock_paths,
                s.adoptions.clone(),
                s.normal_completions,
                s.cancellation_completions,
                s.simultaneous_normal_choices,
                s.simultaneous_ack_choices,
            )
        };
        assert_eq!(
            counters(&ra.statistics),
            counters(&rb.statistics),
            "reference census {good}"
        );
        for budget in 0..=128 {
            let mut ea = engine::explore_with_budget(&a, budget);
            let mut eb = engine::explore_with_budget(&b, budget);
            // F14 already asserts every hand ledger field and its only allowed
            // value difference. Do not pretend debtor names are runtime state.
            if good == "act-debtor-value" {
                ea.obligations = None;
                eb.obligations = None;
            }
            assert_eq!(ea, eb, "engine budget={budget} {good}");
        }
    }
}
