#[path = "../../relca-core/tests/support/actor_start_boundary.rs"]
mod hand;
use hand::{CASES, Case};
use serde_json::Value;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn check(case: Case) -> (Value, i32) {
    check_source(case.name(), &case.source())
}

fn check_source(name: &str, source: &str) -> (Value, i32) {
    static ID: AtomicUsize = AtomicUsize::new(0);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = root.join("target/u12-start-inputs");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!(
        "{}-{}-{}.relca",
        std::process::id(),
        name,
        ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("check")
        .arg(path)
        .current_dir(root)
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let stderr: String = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(out.stderr, stderr.as_bytes());
    (report, out.status.code().unwrap())
}

#[test]
fn red_v1_legacy_join_any_precisely_derives_after_zero_sibling() {
    for capacity in [0, 1] {
        let (r, exit) = check_source("join-any", &hand::any_source(capacity));
        hand::assert_any(capacity, &r, exit);
    }
}

#[test]
fn green_u1_cli_reason_priority_is_resource_then_actor_then_state() {
    for (actor, resource) in [(0, 0), (0, 1), (1, 1)] {
        let (r, exit) = check_source("reason-priority", &hand::priority_source(actor, resource));
        hand::assert_priority(actor, resource, &r, exit);
    }
}
#[test]
fn green_u1_cli_preserves_o_p_effects_exit_and_terminals() {
    for case in CASES {
        let (r, exit) = check(case);
        hand::unchanged(case, &r, exit);
    }
}
#[test]
fn green_u1_cli_unaffected_rows_and_resource_priority() {
    for case in CASES {
        hand::unaffected(case, &check(case).0);
    }
}
macro_rules! framing_tests {($($name:ident: $case:ident;)+) => {$(
    #[test] fn $name() { hand::actor_framing(Case::$case, &check(Case::$case).0); }
)+};}
framing_tests! {
    red_u1_f_zero: Zero;
    red_u1_f_sequential: Sequential;
    red_u1_f_fork_all: ForkAll;
    red_u1_f_requires_zero: RequiresZero;
    red_u1_f_branch_shared: BranchShared;
}
macro_rules! retention_tests {($($name:ident: $case:ident, $cap:literal;)+) => {$(
    #[test] fn $name() { hand::actor_retention(Case::$case, $cap, &check(Case::$case).0); }
)+};}
retention_tests! {
    red_u1_l_zero: Zero, "A.x";
    red_u1_l_sequential_zero: Sequential, "A.x";
    red_u1_l_sequential_after: Sequential, "B.after";
    red_u1_l_fork_zero: ForkAll, "A.x";
    red_u1_l_fork_after: ForkAll, "B.after";
    red_u1_l_requires_zero: RequiresZero, "A.x";
    red_u1_l_branch_zero: BranchShared, "A.x";
    red_u1_l_branch_shared_capability: BranchShared, "B.y";
}

macro_rules! precision_tests {
    ($($name:ident: $kind:ident, $ordered:literal, $reverse:literal, $assert:ident;)+) => {$(
        #[test]
        fn $name() {
            let p = hand::fork_probe(hand::ForkKind::$kind,0,$ordered,$reverse);
            let (r,exit) = check_source(&p.name,&p.source);
            p.assert_core(&r,exit);
            p.$assert(&r);
        }
    )+};
}
precision_tests! {
    red_v1_f_match: Match,false,false,assert_framing;
    red_v1_l_after_match: Match,false,false,assert_retention;
    red_v1_f_variable_match: Variable,false,false,assert_framing;
    red_v1_l_after_variable_match: Variable,false,false,assert_retention;
    red_v3_f_ordered_any: Match,true,false,assert_framing;
    red_v3_l_after_ordered_any: Match,true,false,assert_retention;
    red_v3_f_ordered_reverse_any: Match,true,true,assert_framing;
    red_v3_l_after_ordered_reverse_any: Match,true,true,assert_retention;
}
#[test]
fn green_v1_core_and_nonprecision_outputs_are_hand_values() {
    let mut probes = hand::conservative_forks();
    probes.extend(
        [hand::ForkKind::Match, hand::ForkKind::Variable]
            .into_iter()
            .map(|k| hand::fork_probe(k, 0, false, false)),
    );
    probes.extend([
        hand::fork_probe(hand::ForkKind::Match, 1, false, false),
        hand::fork_probe(hand::ForkKind::Match, 0, true, false),
        hand::fork_probe(hand::ForkKind::Match, 0, true, true),
    ]);
    for p in probes {
        let (r, exit) = check_source(&p.name, &p.source);
        p.assert_core(&r, exit);
    }
}
#[test]
fn green_v1_conservative_drain_no_match_zero_only_all_state_requires_resource() {
    for p in hand::conservative_forks() {
        let (r, exit) = check_source(&p.name, &p.source);
        p.assert_all(&r, exit);
    }
}
#[test]
fn green_v1_positive_capacity_pair_and_comparator_faults() {
    let zero = hand::fork_probe(hand::ForkKind::Match, 0, false, false);
    let one = hand::fork_probe(hand::ForkKind::Match, 1, false, false);
    assert_eq!(
        zero.source,
        one.source
            .replacen("actor A { capacity 1", "actor A { capacity 0", 1)
    );
    let (r, exit) = check_source(&one.name, &one.source);
    one.assert_all(&r, exit);
    hand::comparator_controls();
}
#[test]
fn green_v2_state_priority_dead_mixed_nested_and_independent_arm() {
    for p in hand::state_probes() {
        let (r, exit) = check_source(&p.name, &p.source);
        p.assert_all(&r, exit);
    }
}
#[test]
fn green_v3_resource_free_ordered_prefix_sibling_and_positive_any() {
    for p in [
        hand::ordered_sequential(0),
        hand::ordered_sequential(1),
        hand::ordered_same_actor(),
        hand::fork_probe(hand::ForkKind::All, 0, true, false),
        hand::fork_probe(hand::ForkKind::All, 1, true, false),
        hand::fork_probe(hand::ForkKind::Match, 1, true, false),
    ] {
        assert!(!p.source.contains("resource "));
        assert!(!p.source.contains("uses "));
        let (r, exit) = check_source(&p.name, &p.source);
        p.assert_all(&r, exit);
    }
}
#[test]
fn green_v4_nonempty_jue_atomic_guard_deletion_and_dead_effect() {
    for p in hand::jue_probes() {
        let (r, exit) = check_source(&p.name, &p.source);
        p.assert_all(&r, exit);
    }
}

#[test]
fn green_v_boundary_pairs_are_literal_non_noop_edits() {
    hand::edit_witnesses();
}

#[test]
fn green_v1_followup_sequential_zero_after_refined_fork_and_positive_pair() {
    for capacity in [0, 1] {
        let p = hand::sequential_after_refinement(capacity);
        let (r, exit) = check_source(&p.name, &p.source);
        p.assert_all(&r, exit);
    }
}
#[test]
fn green_v1_followup_three_siblings_with_two_zero_capacities() {
    for capacity in [0, 1] {
        let p = hand::three_siblings(capacity);
        let (r, exit) = check_source(&p.name, &p.source);
        p.assert_all(&r, exit);
    }
}
#[test]
fn green_v1_followup_fork_in_if_does_not_taint_other_arm() {
    for drain in [false, true] {
        let p = hand::fork_in_if(drain);
        let (r, exit) = check_source(&p.name, &p.source);
        p.assert_all(&r, exit);
    }
}
#[test]
fn green_v1_followup_zero_duration_and_unused_requires_keep_conservative_aci() {
    for enabled in [false, true] {
        for p in [
            hand::fallback_zero_duration(enabled),
            hand::fallback_unused_requires(enabled),
        ] {
            let (r, exit) = check_source(&p.name, &p.source);
            p.assert_all(&r, exit);
        }
    }
}
#[test]
fn green_v1_followup_literal_pairs_and_comparator_faults() {
    hand::followup_controls();
}
