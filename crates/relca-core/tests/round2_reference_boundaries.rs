use relca_core::{Verdict, lower, reference, syntax};
mod round2_models;

#[test]
fn zero_loop_wait_does_not_change_the_deadlock_explanation() {
    let source = r#"model M version "0.2" {type V=enum {ok} actor A {capacity 0;capability c()->V {duration fixed 1min;result nondet {ok}}}process P {repeat work at_most 1 {wait 0min;r=A.c();leave}on_limit{end limit}end done}bounds {cases 1;live_tasks 1;clock 10min}verify reachable end.done}"#;
    let run = |s: &str| {
        let m = lower::lower(&syntax::parse(s).unwrap()).unwrap();
        reference::explore(&m)
            .outcome
            .diagnostics
            .into_iter()
            .find(|d| d.code == "W-DEADLOCK-0")
            .unwrap()
            .message
    };
    assert_eq!(run(source), run(&source.replace("wait 0min;", "")));
}

#[test]
fn start_range_leaf_stays_on_its_source_choice() {
    let model = lower::lower(&syntax::parse(round2_models::START).unwrap()).unwrap();
    let r = reference::explore(&model);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(
        r.outcome.properties[0].verdict,
        Verdict::Inconclusive,
        "{:?}",
        r.outcome
    );
}

#[test]
fn first_range_witness_follows_the_completed_sibling_not_the_producer() {
    let mut model = lower::lower(&syntax::parse(round2_models::FINISH).unwrap()).unwrap();
    // The independently validated public IR permits this concurrent writer
    // state even though the DSL's fork lowerer rejects shared writer syntax.
    model.capabilities[1].writes[0].variable = model.capabilities[0].writes[0].variable;
    let r = reference::explore(&model);
    let row = r
        .outcome
        .state_range_violations
        .iter()
        .find(|v| v.binding == "b")
        .unwrap();
    assert_eq!((row.time, row.before.as_str()), (1, "2"));
}

#[test]
fn an_unvisited_producer_range_is_not_reported_after_a_budget_stop() {
    let mut model = lower::lower(&syntax::parse(round2_models::FINISH).unwrap()).unwrap();
    model.capabilities[1].writes[0].variable = model.capabilities[0].writes[0].variable;
    let r = reference::explore_with_limits(
        &model,
        reference::Limits {
            frames: 3,
            ..reference::Limits::default()
        },
    );
    assert_eq!(r.stop, reference::StopReason::EnumerationLimit);
    assert!(
        r.outcome.state_range_violations.is_empty(),
        "{:?}",
        r.outcome.state_range_violations
    );
}

#[test]
fn nested_over_has_the_stage_reason_in_each_header_position() {
    for header in [
        "repeat inner over items at_most 2",
        "repeat inner at_most 2 over items",
        "repeat inner at_most 2 gap fixed 1min over items",
    ] {
        let source = format!(
            "model M version \"0.2\" {{process P {{repeat outer at_most 2 {{{header} {{wait 1min;again}}on_limit{{leave}};again}}on_limit{{end done}}}} bounds {{cases 1;live_tasks 1;clock 10min}}verify reachable end.done}}"
        );
        let errors = syntax::parse(&source).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|d| d.code == "U-SEM-0" && d.message == "loop_stage_unsupported"),
            "{errors:?}"
        );
    }
}

#[test]
fn nested_non_over_shapes_keep_the_original_combination_diagnostic() {
    for inner in [
        "repeat over at_most 2 {wait 1min;again}on_limit{leave}",
        "repeat inner {wait 1min;again}on_limit{leave}",
    ] {
        let source = format!(
            "model M version \"0.2\" {{process P {{repeat outer at_most 2 {{{inner};again}}on_limit{{end done}}}}bounds {{cases 1;live_tasks 1;clock 10min}}verify reachable end.done}}"
        );
        let errors = syntax::parse(&source).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|d| d.code == "U-SEM-0" && d.message.contains("loop_feature_combination")),
            "{errors:?}"
        );
    }
}
