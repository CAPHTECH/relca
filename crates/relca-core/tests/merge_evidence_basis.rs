//! Independent hand expectations for the two newly integrated features.
use relca_core::{Verdict, engine, ir, lower, reference, syntax};

const INDEPENDENT: &str = include_str!("merge_review/evidence-independent.relca");
const DEPENDENT: &str = include_str!("merge_review/evidence-dependent.relca");

fn model(src: &str) -> ir::Model {
    lower::lower(&syntax::parse(src).unwrap()).unwrap()
}

#[test]
fn merged_evidence_target_is_closed_for_direct_transitive_and_snapshot_inputs() {
    let snapshot = DEPENDENT
        .replace(
            "actor Worker {",
            "observed case { snapshot:optional<V> }\n actor Worker {",
        )
        .replace(
            "capability echo(v:V) -> V {",
            "capability echo(v:V) -> V { writes snapshot; finish { snapshot=pass }",
        )
        .replace(
            "capability assess(v:V) -> V {",
            "capability assess(v:V) -> V { reads [snapshot];",
        )
        .replace("t = Judge.assess(other)", "t = Judge.assess(d)");
    for src in [
        DEPENDENT.to_owned(),
        DEPENDENT.replace("Judge.assess(other)", "Judge.assess(merged)"),
        DEPENDENT.replace("evidence_basis Judge", "evidence_basis direct Judge"),
        snapshot,
    ] {
        let errors = lower::lower(&syntax::parse(&src).unwrap()).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|d| d.code == "U-SEM-0" && d.message.contains("merge_control")),
            "{errors:?}"
        );
    }
}

#[test]
fn unrelated_merge_keeps_hand_verdicts_in_engine_and_independent_reference() {
    let control = INDEPENDENT.replace(
        "t = Judge.assess(d)",
        "if merged == pass { t = Judge.assess(d) } else { u = Judge.assess(d) }",
    );
    let overwritten = INDEPENDENT
        .replace(
            "actor Worker {",
            "observed case { snapshot:optional<V> }\n actor Worker {",
        )
        .replace(
            "capability echo(v:V) -> V {",
            "capability echo(v:V) -> V { writes snapshot; finish { snapshot=pass }",
        )
        .replace(
            "capability assess(v:V) -> V {",
            "capability assess(v:V) -> V { reads [snapshot];",
        )
        .replace(
            "t = Judge.assess(d)",
            "clean = Worker.echo(d); t = Judge.assess(d)",
        );
    for base in [INDEPENDENT, &control, &overwritten] {
        for (src, expected) in [
            (base.to_owned(), Verdict::Satisfied),
            (base.replace("reads [Invoice];", ""), Verdict::Violated),
        ] {
            let m = model(&src);
            let actual = engine::explore(&m);
            let independent = reference::explore(&m);
            assert_eq!(independent.stop, reference::StopReason::Complete);
            for result in [&actual, &independent.outcome] {
                assert!(!result.truncated);
                assert!(
                    result.diagnostics.iter().all(|d| !d.code.starts_with("U-")),
                    "{:?}",
                    result.diagnostics
                );
                assert_eq!(
                    result
                        .properties
                        .iter()
                        .find(|p| matches!(p.property, ir::Property::Contract(_)))
                        .unwrap()
                        .verdict,
                    expected
                );
            }
            assert_eq!(actual.properties, independent.outcome.properties);
        }
    }
}
