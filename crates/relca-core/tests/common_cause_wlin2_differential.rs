//! All 24 positive-time inputs: each backend checked against hand oracle first.
#[path = "common_cause_wlin2.rs"]
mod acceptance;
use relca_core::{engine, lower, reference, syntax};
use serde_json::json;

#[test]
fn eight_boundary_models_have_hand_times_and_complete_reference_outcomes() {
    let cases = acceptance::boundary_cases();
    assert_eq!(cases.len(), 8);
    for c in cases {
        let m = lower::lower(&syntax::parse(&c.src).unwrap()).unwrap();
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{}", c.label);
        let e = engine::explore(&m);
        for o in [&r.outcome, &e] {
            assert!(!o.truncated, "{}", c.label);
            let done = o
                .terminals
                .iter()
                .find(|t| {
                    m.ends[t.end.0]
                        == if c.label == "reread" {
                            "posted"
                        } else {
                            "done"
                        }
                })
                .unwrap();
            assert_eq!(
                (done.elapsed.min, done.elapsed.max),
                (c.elapsed, c.elapsed),
                "{} hand elapsed",
                c.label
            );
            assert_eq!(
                (done.work.min, done.work.max),
                (c.work, c.work),
                "{} hand work",
                c.label
            );
            if c.label != "reread" {
                assert_eq!(done.probability.as_ref().unwrap().min.to_string(), "1");
                assert_eq!(done.probability.as_ref().unwrap().max.to_string(), "1");
            }
        }
        let mut ro = r.outcome;
        assert!(ro.diagnostics.starts_with(&m.diagnostics));
        ro.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, ro, "{} complete whole Outcome", c.label);
    }
}

#[test]
fn twenty_four_inputs_complete_zero_exclusions_and_whole_outcome_equality() {
    compare_population(false);
}
#[test]
fn old_syntax_preflight_all24_complete_in_both_backends() {
    compare_population(true);
}
fn compare_population(strip_metadata: bool) {
    let mut complete = 0;
    for name in acceptance::NAMES {
        let mut src = acceptance::source(name);
        if strip_metadata {
            src = src
                .replace(" basis \"sample X measured independent detection\"", "")
                .replace("tacit \"取引慣行の知識\"", "tacit");
        }
        let m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        let e = engine::explore(&m);
        let hand = acceptance::expected(name);
        for outcome in [&r.outcome, &e] {
            assert!(!outcome.truncated, "{name}");
            let ts:Vec<_>=outcome.terminals.iter().map(|t|json!({"end":m.ends[t.end.0],"probability":t.probability,"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|r|json!({"min":r.min,"max":r.max}))})).collect();
            assert_eq!(
                json!(ts),
                hand["terminals"],
                "hand times and probabilities {name}"
            );
            assert_eq!(
                outcome.properties.len(),
                hand["properties"].as_array().unwrap().len()
            );
            for (p, h) in outcome
                .properties
                .iter()
                .zip(hand["properties"].as_array().unwrap())
            {
                assert_eq!(
                    serde_json::to_value(p.verdict).unwrap(),
                    h["verdict"],
                    "hand verdict {name}"
                );
                assert_eq!(
                    json!(p.probability),
                    h["probability"],
                    "hand probability {name}"
                );
            }
        }
        let mut ro = r.outcome;
        assert!(ro.diagnostics.starts_with(&m.diagnostics));
        ro.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, ro, "whole runtime outcome {name}");
        complete += 1;
    }
    assert_eq!(complete, 24, "Complete24/excluded0; no denominator shrink");
}

#[test]
fn metadata_only_changes_leave_same_ir_runtime_and_reference_census_unchanged() {
    for name in ["lin-basis", "lin-tacit"] {
        let s = acceptance::source(name);
        let stripped = s
            .replace(" basis \"sample X measured independent detection\"", "")
            .replace("tacit \"取引慣行の知識\"", "tacit");
        let a = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
        let mut b = lower::lower(&syntax::parse(&stripped).unwrap()).unwrap();
        // Diagnostics are static input; compare runtime with the same diagnostics.
        b.diagnostics = a.diagnostics.clone();
        assert_eq!(
            engine::explore(&a),
            engine::explore(&b),
            "metadata must not affect runtime: {name}"
        );
        let ra = reference::explore(&a);
        let rb = reference::explore(&b);
        assert_eq!(ra.outcome, rb.outcome);
        let sa = ra.statistics;
        let sb = rb.statistics;
        assert_eq!(
            (
                sa.frames,
                sa.terminal_paths,
                sa.deadlock_paths,
                sa.adoptions,
                sa.normal_completions,
                sa.cancellation_completions,
                sa.simultaneous_normal_choices,
                sa.simultaneous_ack_choices
            ),
            (
                sb.frames,
                sb.terminal_paths,
                sb.deadlock_paths,
                sb.adoptions,
                sb.normal_completions,
                sb.cancellation_completions,
                sb.simultaneous_normal_choices,
                sb.simultaneous_ack_choices
            ),
            "reference census {name}"
        );
    }
}
