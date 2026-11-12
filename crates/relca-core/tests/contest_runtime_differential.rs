//! D2: zero exclusions; compare whole Outcome, including new runtime tables.
#[path = "contest_runtime.rs"]
mod acceptance;
use relca_core::{engine, lower, reference, syntax};
use serde_json::json;

#[test]
fn twelve_inputs_complete_in_both_independent_interpreters() {
    let mut complete = 0;
    for name in acceptance::NAMES {
        let model = lower::lower(&syntax::parse(&acceptance::source(name)).unwrap()).unwrap();
        let r = reference::explore(&model);
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        let e = engine::explore(&model);
        let expected = acceptance::expected(name);
        assert_eq!(
            serde_json::to_value(r.verdict()).unwrap(),
            expected["verdict"],
            "hand verdict {name}"
        );
        for outcome in [&r.outcome, &e] {
            assert_eq!(
                outcome.truncated,
                expected["bounds"]["truncated"].as_bool().unwrap(),
                "hand cut {name}"
            );
            for (p, hand) in outcome
                .properties
                .iter()
                .zip(expected["properties"].as_array().unwrap())
            {
                assert_eq!(
                    serde_json::to_value(p.verdict).unwrap(),
                    hand["verdict"],
                    "hand property {name}"
                );
            }
            assert_eq!(
                outcome.properties.len(),
                expected["properties"].as_array().unwrap().len(),
                "hand inventory {name}"
            );
            let terminals: Vec<_> = outcome
                .terminals
                .iter()
                .map(|t| {
                    json!({
                        "end":model.ends[t.end.0],
                        "elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},
                        "work":{"min":t.work.min,"max":t.work.max},
                        "result_at":t.result_at.map(|r|json!({"min":r.min,"max":r.max}))
                    })
                })
                .collect();
            assert_eq!(
                json!(terminals),
                expected["terminals"],
                "hand business snapshot {name}"
            );
            if name.contains("origin") || name.contains("resource") {
                let cases = outcome.case_outcomes.as_ref().expect("arrivals case rows");
                assert_eq!(cases.len(), if name.contains("resource") { 2 } else { 1 });
                for case in cases {
                    assert_eq!(case.terminals.len(), 1);
                    let t = &case.terminals[0];
                    let completed = t.completed_at.unwrap();
                    assert_eq!(
                        (completed.min, completed.max),
                        if name.contains("origin") {
                            (14, 14)
                        } else {
                            (4, 4)
                        }
                    );
                    let elapsed = t.elapsed.unwrap();
                    let work = t.work.unwrap();
                    assert_eq!((elapsed.min, elapsed.max, work.min, work.max), (4, 4, 4, 4));
                }
            }
            assert_eq!(
                outcome
                    .obligations
                    .as_ref()
                    .expect("contest opts into ledger")
                    .len(),
                0
            );
            if expected.get("metrics").is_some() {
                acceptance::subset(
                    &expected["metrics"],
                    &serde_json::to_value(outcome.metrics.as_ref().unwrap()).unwrap(),
                );
            }
        }
        let mut reference = r.outcome;
        assert!(reference.diagnostics.starts_with(&model.diagnostics));
        reference.diagnostics.drain(..model.diagnostics.len());
        assert_eq!(e, reference, "all runtime fields {name}");
        complete += 1;
    }
    assert_eq!(complete, 12);
}
