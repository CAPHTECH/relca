use relca_core::{lower, reference, syntax};

#[test]
fn completed_subtrees_do_not_retain_all_path_leaves() {
    let calls = (0..8).map(|i| format!("r{i}=A.c();")).collect::<String>();
    let source = format!(
        r#"model Branches version "0.2" {{
        type V=enum {{ok,bad}}
        kernel K {{*->{{0.5:ok;0.5:bad}}}}
        actor A {{capacity 1;capability c()->V {{duration fixed 1min;outcomes kernel K}}}}
        process P {{{calls} end done}}
        cut C {{include [A]}}
        bounds {{cases 1;live_tasks 1;clock 100min}}
        verify probability reached end.done >= 1
    }}"#
    );
    let m = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(r.statistics.terminal_paths, 256);
    assert!(
        r.statistics.path_choice_peak < 100,
        "retained {} choice slots",
        r.statistics.path_choice_peak
    );
}
