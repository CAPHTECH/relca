//! Every selected old-syntax skeleton has a hand timing oracle before comparison.
#[path = "configuration_diff_extensions.rs"]
mod acceptance;
use relca_core::{engine, lower, reference, syntax};
use serde_json::json;

fn hand(name: &str, to: bool, m: &relca_core::ir::Model, o: &relca_core::outcome::Outcome) {
    assert!(!o.truncated, "bounded complete {name}");
    assert!(!o.terminals.is_empty(), "business terminal {name}");
    for t in &o.terminals {
        let (lo, hi, wl, wh) = if name.starts_with("diff2-invoice") {
            match (to, m.ends[t.end.0].as_str()) {
                (false, "posted") => (17, 17, 17, 17),
                (true, "posted") => (9, 9, 12, 12),
                (false, "returned") => (14, 16, 14, 16),
                (true, "returned") => (6, 8, 9, 11),
                _ => panic!("unreachable invoice terminal"),
            }
        } else if name.starts_with("diff2-capacity") || name.starts_with("diff2-mismatch") {
            let e = if !to || name.ends_with("one-slot") {
                8
            } else {
                5
            };
            (e, e, 8, 8)
        } else if name.starts_with("diff2-time") {
            let e = if !to || name.ends_with("sequential") {
                2
            } else {
                1
            };
            (e, e, 2, 2)
        } else if name.starts_with("diff2-resource") {
            let e = u64::from(to && !name.ends_with("no-call"));
            (e, e, e, e)
        } else if name.starts_with("diff2-maintain") {
            let e = u64::from(to);
            (e, e, e, e)
        } else if name.starts_with("diff2-authority") {
            let e = if to && !name.ends_with("no-call") {
                2
            } else {
                1
            };
            (e, e, e, e)
        } else if name.starts_with("diff2-contest") || name.starts_with("diff2-preserve-union") {
            (2, 2, 2, 2)
        } else if name.starts_with("diff2-preserve") {
            let e = if to && !name.ends_with("restored-call") {
                1
            } else {
                2
            };
            (e, e, e, e)
        } else if name.starts_with("diff2-override") {
            if !to {
                (2, 2, 2, 2)
            } else if name.ends_with("bypass") {
                (3, 3, 3, 3)
            } else {
                (2, 3, 2, 3)
            }
        } else {
            (1, 1, 1, 1)
        };
        assert_eq!(
            (t.elapsed.min, t.elapsed.max, t.work.min, t.work.max),
            (lo, hi, wl, wh),
            "hand timing {name}/to={to}/{}",
            m.ends[t.end.0]
        );
        assert!(t.result_at.is_none());
    }
}

#[test]
fn all_76_selected_skeletons_reference_complete_zero_exclusions_and_hand_times() {
    let mut n = 0;
    for name in acceptance::population::NAMES {
        let src = acceptance::stripped(&acceptance::source(name));
        let ast = syntax::parse(&src).unwrap();
        for (i, selection) in if name.starts_with("diff2-invoice") {
            ["AsIs", "ToBeA"]
        } else {
            ["From", "To"]
        }
        .iter()
        .enumerate()
        {
            let m = lower::lower_configuration(&ast, selection).unwrap().model;
            let run = reference::explore(&m);
            assert_eq!(
                run.stop,
                reference::StopReason::Complete,
                "{name}/{selection}"
            );
            let engine = engine::explore(&m);
            hand(name, i == 1, &m, &engine);
            hand(name, i == 1, &m, &run.outcome);
            let mut independent = run.outcome;
            assert!(independent.diagnostics.starts_with(&m.diagnostics));
            independent.diagnostics.drain(..m.diagnostics.len());
            assert_eq!(
                engine, independent,
                "whole runtime Outcome {name}/{selection}"
            );
            n += 1;
        }
    }
    assert_eq!(
        n, 76,
        "38 dirs, 76 selected runtime skeletons, Complete76, exclusions0"
    );
}

#[test]
fn adding_intent_never_changes_selected_executable_ir_or_outcome_census() {
    let mut n = 0;
    for name in acceptance::population::NAMES {
        let src = acceptance::source(name);
        if !src.contains("intent {") {
            continue;
        }
        let ast = syntax::parse(&src).expect("new intent grammar");
        let skeleton = syntax::parse(&acceptance::stripped(&src)).unwrap();
        for selection in if name.starts_with("diff2-invoice") {
            ["AsIs", "ToBeA"]
        } else {
            ["From", "To"]
        } {
            let m = lower::lower_configuration(&ast, selection)
                .expect("new intent names")
                .model;
            let old = lower::lower_configuration(&skeleton, selection)
                .unwrap()
                .model;
            assert_eq!(
                m, old,
                "intent metadata stays outside executable model {name}/{selection}"
            );
            assert_eq!(
                engine::explore(&m),
                engine::explore(&old),
                "Outcome/census {name}/{selection}"
            );
            n += 1;
        }
    }
    assert_eq!(
        json!(n),
        json!(34),
        "17 intent-bearing dirs, two selections each"
    );
}
