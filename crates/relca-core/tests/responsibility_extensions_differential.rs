//! Static metadata must not create runtime states or consume exploration budget.
#[path = "responsibility_extensions.rs"]
mod acceptance;
use relca_core::{engine, lower, reference, syntax};
use serde_json::json;

#[test]
fn twenty_six_runtime_inputs_complete_without_static_responsibility_changing_outcome() {
    let mut count = 0;
    for name in acceptance::NAMES {
        let expected = acceptance::expected(name);
        if expected["verdict"] != "satisfied" {
            continue;
        }
        let src = acceptance::source(name);
        let m = lower::lower(&syntax::parse(&src).expect("new syntax")).expect("new lower");
        let old = lower::lower(&syntax::parse(&acceptance::stripped(&src)).unwrap()).unwrap();
        let reference = reference::explore(&m);
        let reference_old = reference::explore(&old);
        assert_eq!(reference.stop, reference::StopReason::Complete, "{name}");
        assert_eq!(
            reference_old.stop,
            reference::StopReason::Complete,
            "{name} skeleton"
        );
        // engine::explore returns runtime diagnostics only; the reference
        // outcome is prefixed by the model's lowering diagnostics
        // (differential.rs keeps the same seam).
        for (mut actual, mut control, prefixed) in [
            (engine::explore(&m), engine::explore(&old), false),
            (reference.outcome.clone(), reference_old.outcome, true),
        ] {
            assert!(
                actual
                    .properties
                    .iter()
                    .all(|p| p.verdict == relca_core::Verdict::Satisfied),
                "{name}"
            );
            assert!(
                !actual
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == relca_core::diag::Severity::Error),
                "{name}"
            );
            if let Some(hand) = expected.get("terminals") {
                let ts:Vec<_>=actual.terminals.iter().map(|t|json!({"end":m.ends[t.end.0],"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|r|json!({"min":r.min,"max":r.max}))})).collect();
                acceptance::subset(hand, &json!(ts));
            }
            // Static warnings and source spans naturally differ. Drop only each
            // model's own lowering diagnostics, not arbitrary runtime warnings.
            if prefixed {
                assert!(actual.diagnostics.starts_with(&m.diagnostics));
                assert!(control.diagnostics.starts_with(&old.diagnostics));
                actual.diagnostics.drain(..m.diagnostics.len());
                control.diagnostics.drain(..old.diagnostics.len());
            }
            assert_eq!(
                actual, control,
                "whole runtime Outcome, including census {name}"
            );
        }
        count += 1;
    }
    assert_eq!(
        count, 26,
        "30 inputs, four static rejections, zero runtime exclusions"
    );
}
