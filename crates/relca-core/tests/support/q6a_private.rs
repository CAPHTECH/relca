//! Fixed private acceptance. Integration compiles the body; connection is pending.
//! Production cfg(test) adapters must observe real interpreter state, never return
//! pre-built expected objects or reuse the engine for the reference observation.
#![allow(dead_code)]
use super::{hand, input};
use serde_json::{Value, json};

/// Connect in report.rs using simulate_using and reference::simulate, as in Q9.
pub fn reference_aggregate(mut run: impl FnMut(&str) -> Value) {
    for name in [
        "people",
        "reverse",
        "burst",
        "lighter",
        "held",
        "later",
        "simultaneous",
        "refused",
        "admitted",
    ] {
        hand::validate_sim(name, &run(&input::source(name))).unwrap();
    }
}

/// Adapter must lower the valid source, apply exactly the named IR mutation,
/// then call BOTH public execution entrances (explore and simulate). No DSL
/// rewriting is a substitute. Each returned row is the actual abort projection.
pub fn public_ir(mut run: impl FnMut(&str, &str, &str) -> Value) {
    for (name, fault) in [
        ("people", "qualified_add_outside_actor"),
        ("people", "profile_missing_executor"),
        ("burst", "band_gap"),
        ("burst", "kernel_id_out_of_range"),
        ("people", "ordinary_and_performance_sources"),
    ] {
        for mode in ["explore", "simulate"] {
            let actual = run(&input::source(name), fault, mode);
            hand::subset(
                &json!({"engine":{"code":"U-SEM-0","reason":"invalid_q6_ir","terminals":0},
                "reference":{"stop":"SemanticAbort","terminals":0}}),
                &actual,
                fault,
            )
            .unwrap();
        }
    }
}

/// Freeze two otherwise identical Running states with the same clock/person,
/// but Low versus Half selectors, made by actual Start transitions. Derive
/// comparison/hash/store/replay observations from those objects. Compare after
/// cloning and after both view/store round trips; one global hash comparison is
/// not sufficient. Hash inputs must differ (hash digest collision is not the
/// property). Both normal finishes must yield their frozen probabilities.
pub fn selector_identity(mut observe: impl FnMut(&str) -> Value) {
    for side in ["engine", "reference"] {
        let v = observe(side);
        hand::subset(
            &json!({"equal":false,"hash_inputs_equal":false,
            "clone_equal":false,"view_store_equal":false,"replay_equal":false,
            "low_bad":"1/10","high_bad":"1/2"}),
            &v,
            side,
        )
        .unwrap();
    }
}

/// Inject each frozen-selector corruption into an actual Running task before
/// finish, then exercise tree, sample and replay: no default kernel is allowed.
pub fn selector_corruption(mut observe: impl FnMut(&str, &str, &str) -> Value) {
    for side in ["engine", "reference"] {
        for mode in ["tree", "sample", "replay"] {
            for fault in [
                "missing",
                "profile_out_of_range",
                "executor_out_of_range",
                "band_out_of_range",
            ] {
                let v = observe(side, mode, fault);
                hand::subset(
                    &json!({"aborted":true,"normal_finishes":0,"kernel_draws":0}),
                    &v,
                    fault,
                )
                .unwrap();
            }
        }
    }
}

/// A real sampler/event observer, not a reconstructed draw count from metrics.
/// Start at0, duration4 and no normal Finish before cancellation at2. Cancel
/// request at1, ack at2. requires false has neither Start nor Finish. The
/// ordinary-kernel control uses the SAME kernel site and observation hooks.
pub fn draw_lifecycle(mut run: impl FnMut(&str, &str) -> Value) {
    for side in ["engine", "reference"] {
        for (case, starts, finishes, acks, draws, work) in [
            ("performance_cancel", 1, 0, 1, 0, 2),
            ("ordinary_cancel", 1, 0, 1, 0, 2),
            ("performance_refused", 0, 0, 0, 0, 0),
            ("performance_finished", 1, 1, 0, 1, 4),
        ] {
            let v = run(side, case);
            hand::subset(
                &json!({"starts":starts,"normal_finishes":finishes,"cancel_acks":acks,
                "kernel_draws":draws,"draws_at_start":0,"work":work}),
                &v,
                case,
            )
            .unwrap();
        }
    }
}

/// Count actual pre-Start Queued tasks, not metric queue lengths or busy slots.
/// The adapter builds one real state per named distinction and takes a Start
/// transition; the result must be its selected kernel and pre-Start count.
pub fn backlog_projection(mut run: impl FnMut(&str, &str) -> Value) {
    for side in ["engine", "reference"] {
        for (case, q, kernel) in [
            ("self_only", 0, "Low"),
            ("running_same_actor", 0, "Low"),
            ("held_same_actor", 0, "Low"),
            ("queued_other_actor", 0, "Low"),
            ("not_arrived", 0, "Low"),
            ("queued_other_capability_same_actor", 1, "Half"),
            ("queued_in_stopped_case", 1, "Half"),
            ("same_tick_first_start", 1, "Half"),
            ("same_tick_second_start", 0, "Low"),
        ] {
            hand::subset(&json!({"q":q,"kernel":kernel}), &run(side, case), case).unwrap();
        }
    }
}
