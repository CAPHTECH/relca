use relca_core::{Verdict, lower, reference, syntax};

#[test]
fn arrivals_reserve_the_next_finish_only_after_the_previous_subtree() {
    // Observed on main 8c3765f4, including the completed prefix path count.
    for (source, frames, paths) in [
        (
            include_str!("../../../fixtures/arrivals-initial-product/model.relca"),
            20,
            2,
        ),
        (
            include_str!("../../../fixtures/pol-label-cofinish/model.relca"),
            15,
            4,
        ),
    ] {
        let model = lower::lower(&syntax::parse(source).unwrap()).unwrap();
        let r = reference::explore_with_limits(
            &model,
            reference::Limits {
                frames,
                ..reference::Limits::default()
            },
        );
        assert_eq!(r.stop, reference::StopReason::EnumerationLimit);
        assert_eq!(r.statistics.frames, frames);
        assert_eq!(r.statistics.terminal_paths, paths);
    }
}
#[test]
fn small_frame_budgets_retain_the_baseline_stop_and_witness() {
    // Captured from main 8c3765f4, not from the revised evaluator.
    for (source, expected) in [
        (
            include_str!("../../../fixtures/nondet-race/model.relca"),
            vec![
                Verdict::Unsupported,
                Verdict::Satisfied,
                Verdict::Unsupported,
                Verdict::Unsupported,
            ],
        ),
        (
            include_str!("../../../fixtures/coop-cancel/model.relca"),
            vec![Verdict::Satisfied],
        ),
        (
            include_str!("../../../fixtures/prob-cancel/model.relca"),
            vec![
                Verdict::Unsupported,
                Verdict::Unsupported,
                Verdict::Unsupported,
            ],
        ),
    ] {
        let m = lower::lower(&syntax::parse(source).unwrap()).unwrap();
        let r = reference::explore_with_limits(
            &m,
            reference::Limits {
                frames: 8,
                ..reference::Limits::default()
            },
        );
        assert_eq!(r.stop, reference::StopReason::EnumerationLimit);
        assert_eq!(r.statistics.frames, 8);
        assert_eq!(
            r.outcome
                .properties
                .iter()
                .map(|p| p.verdict)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(r.outcome.terminals.is_empty());
    }
}
