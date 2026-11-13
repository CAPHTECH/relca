use relca_core::{lower, reference, syntax};

fn run(source: &str, expected_frames: usize) {
    let model = lower::lower(&syntax::parse(source).unwrap()).unwrap();
    let result = reference::explore_with_limits(&model, reference::Limits::default());
    eprintln!("stop={:?} frames={}", result.stop, result.statistics.frames);
    // Frozen against f77c7cde on the CLI main stack, not against this runner.
    assert_eq!(result.stop, reference::StopReason::EnumerationLimit);
    assert_eq!(result.statistics.frames, expected_frames);
    assert_eq!(result.outcome.case_outcomes.as_ref().unwrap().len(), 2);
}

#[test]
fn two_case_l2_n32_on_standard_test_stack() {
    let source =
        include_str!("bounded_repetition/models/L2-32.relca").replace("cases 1", "cases 2");
    let source = source.trim_end().strip_suffix('}').unwrap().to_owned()
        + "\narrivals fixed [0min,0min]\npolicy fifo tie_by case_id\n}";
    run(&source, 107);
}

#[test]
fn two_case_nondeterministic_n32_on_standard_test_stack() {
    run(
        r#"model Two version "0.2" {
        type V=enum {pass,fail}
        actor A {capacity 1;capability c()->V {duration fixed 1min;result nondet {pass,fail}}}
        process P {repeat work at_most 32 {r=A.c();if r==pass {leave}else{again}}on_limit{end limit}end done}
        cut C {include [A]}
        arrivals fixed [0min,0min]
        policy fifo tie_by case_id
        bounds {cases 2;live_tasks 1;clock 500min}
        verify safety no_resource_overcommit
    }"#,
        129,
    );
}
