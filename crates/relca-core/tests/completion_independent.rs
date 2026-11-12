//! Independent tiny exploration supplements, not replaces, the population oracle.
#[path = "support/completion_explorer.rs"]
pub mod completion_explorer;
#[path = "support/completion_explorer_adapter.rs"]
pub mod completion_explorer_adapter;
#[path = "support/completion_explorer_population.rs"]
pub mod completion_explorer_population;
#[path = "support/completion_explorer_product.rs"]
pub mod completion_explorer_product;
use completion_explorer::*;
use completion_explorer_adapter::*;
use completion_explorer_population::*;
use relca_core::{engine, lower, report, syntax};
use std::collections::BTreeSet;

pub fn model(e: &Example) -> relca_core::ir::Model {
    let m = lower::lower(&syntax::parse(&e.source).unwrap()).unwrap();
    // Full data adapter equality includes handwritten control tree, calls,
    // durations/results/resources/people and independently anchored byte spans.
    // Check type and end tables too, rather than treating lower as an oracle.
    assert_eq!(m.types.len(), 1);
    assert_eq!(m.types[0].name, "V");
    assert_eq!(m.types[0].members, ["pass", "fail"]);
    assert!(m.process.bindings.iter().all(|b| b.ty.0 == 0));
    assert!(m.capabilities.iter().all(|c| c.return_type.0 == 0));
    let expected_ends =
        if e.name.starts_with("arm-") || e.name == "mixed-deadlock" || e.own.bindings.len() == 1 {
            vec!["good", "bad"]
        } else {
            vec!["done"]
        };
    assert_eq!(m.ends, expected_ends);
    assert_eq!(adapt(&m).unwrap(), e.own, "{} lower transport", e.name);
    m
}
pub fn expected(e: &Example) -> Observation {
    let own = explore(&e.own);
    let hand = e
        .hand_terminals
        .iter()
        .map(|(n, t, w)| ((*n).to_owned(), *t, *w))
        .collect::<BTreeSet<_>>();
    assert_eq!(own.terminals, hand, "{} hand arithmetic", e.name);
    assert_eq!(own.deadlock, e.deadlock, "{} hand deadlock", e.name);
    assert_eq!(own.cut, e.cut, "{} hand cut", e.name);
    own
}
#[test]
fn independent_population_matches_public_outcome_and_four_observation_sets() {
    let examples = population();
    assert_eq!(examples.len(), 19);
    let mut names = BTreeSet::new();
    for e in examples {
        assert!(names.insert(e.name));
        let m = model(&e);
        let wanted = expected(&e);
        let actual = engine::explore(&m);
        compare(
            &wanted,
            &completion_explorer_product::observations(&m, &actual),
        )
        .unwrap_or_else(|error| panic!("{} {error}", e.name));
        completion_explorer_product::check_properties(&m, &actual, &wanted);
        completion_explorer_product::check_report(
            &serde_json::to_value(report::check(&e.source)).unwrap(),
            &e.own,
            &wanted,
        )
        .unwrap_or_else(|error| panic!("{} {error}", e.name));
    }
}
#[test]
fn independent_comparator_rejects_missing_successor_extra_terminal_and_tick_shift() {
    let e = population().into_iter().find(|e| e.name == "both").unwrap();
    let wanted = expected(&e);
    // Also pin all four observation sets by hand: the explorer cannot agree
    // with a broken product merely by returning empty or self-consistent sets.
    assert_eq!(wanted.ends, BTreeSet::from(["good".into(), "bad".into()]));
    assert_eq!(wanted.started, BTreeSet::from(["A.x".into()]));
    assert_eq!(wanted.assigned, BTreeSet::from(["p".into()]));
    let Op::If(_, _, _, loc) = e.own.body[1] else {
        panic!()
    };
    assert_eq!(wanted.arms, BTreeSet::from([(loc, true), (loc, false)]));
    let mut missing = wanted.clone();
    missing.terminals.remove(&("bad".into(), 1, 1));
    missing.ends.remove("bad");
    missing.arms.remove(&(loc, false));
    assert!(compare(&wanted, &missing).is_err());
    let mut extra = wanted.clone();
    extra.terminals.insert(("invented".into(), 1, 1));
    extra.ends.insert("invented".into());
    assert!(compare(&wanted, &extra).is_err());
    let mut shifted = wanted.clone();
    shifted.terminals.remove(&("good".into(), 1, 1));
    shifted.terminals.insert(("good".into(), 2, 1));
    assert!(compare(&wanted, &shifted).is_err());
}
#[test]
fn hand_two_step_and_nondeterministic_successor_models_calibrate_explorer_itself() {
    let examples = population();
    for name in [
        "serial",
        "parallel-two",
        "parallel-one",
        "shared-person",
        "distinct-people",
        "serial-wait",
    ] {
        expected(examples.iter().find(|e| e.name == name).unwrap());
    }
    let e = examples.iter().find(|e| e.name == "both").unwrap();
    let mut broken = e.own.clone();
    broken.capabilities[0].results = vec!["pass".into()];
    assert!(
        compare(&expected(e), &explore(&broken)).is_err(),
        "missing nondeterministic successor"
    );
    broken = e.own.clone();
    broken.capabilities[0].duration = 2;
    assert!(
        compare(&expected(e), &explore(&broken)).is_err(),
        "finish shifted one tick"
    );
    let mut extra = explore(&e.own);
    extra.terminals.insert(("bad".into(), 0, 0));
    assert!(
        compare(&expected(e), &extra).is_err(),
        "extra completed state"
    );
}
#[test]
fn hand_warning_inventory_and_single_edit_dsl_pairs_are_fixed_before_product_execution() {
    let examples = population();
    let find = |name| examples.iter().find(|e| e.name == name).unwrap();
    for (from, to, old, new) in [
        (
            "both",
            "pass-only",
            "result nondet { pass, fail }",
            "result nondet { pass }",
        ),
        (
            "anonymous-one",
            "capacity-zero",
            "capacity 1; capability x",
            "capacity 0; capability x",
        ),
        (
            "resource-one",
            "resource-zero",
            "resource R capacity 1",
            "resource R capacity 0",
        ),
        ("clock-boundary", "clock-zero", "clock 1min", "clock 0min"),
        ("executor-pool", "both", "executors [p,q]", "executors [p]"),
        (
            "arm-call",
            "arm-call-removed",
            "y = B.y(); end bad",
            "end bad",
        ),
        (
            "distinct-people",
            "shared-person",
            "actor B { capacity 1; executors [q]",
            "actor B { capacity 1; executors [p]",
        ),
    ] {
        assert_eq!(find(from).source.matches(old).count(), 1);
        assert_eq!(
            find(from).source.replacen(old, new, 1),
            find(to).source,
            "{from}/{to}"
        );
    }
    let inventory = |name| {
        let e = find(name);
        warnings(&e.own, &expected(e))
            .into_iter()
            .map(|w| (w.code, w.target, w.reason))
            .collect::<BTreeSet<_>>()
    };
    let mut hand = BTreeSet::from([
        ("W-UNREACH-0".into(), "A.unused".into(), "not_called".into()),
        ("W-UNREACH-3".into(), "q".into(), "never_assigned".into()),
    ]);
    assert_eq!(inventory("both"), hand);
    hand.extend([
        ("W-UNREACH-1".into(), "else".into(), "never_taken".into()),
        ("W-UNREACH-2".into(), "bad".into(), "never_reached".into()),
    ]);
    assert_eq!(inventory("pass-only"), hand);
    assert!(inventory("clock-zero").is_empty());
    assert_eq!(inventory("capacity-zero").len(), 7);
    assert_eq!(
        inventory("executor-pool"),
        BTreeSet::from([("W-UNREACH-0".into(), "A.unused".into(), "not_called".into())])
    );
}
#[test]
fn warnings_and_raw_comparator_reject_each_code_target_reason_span_and_completion_edit() {
    let examples = population();
    let codes = examples
        .iter()
        .flat_map(|e| warnings(&e.own, &expected(e)))
        .map(|w| w.code)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        codes,
        BTreeSet::from([
            "W-UNREACH-0".into(),
            "W-UNREACH-1".into(),
            "W-UNREACH-2".into(),
            "W-UNREACH-3".into()
        ])
    );
    let e = examples.iter().find(|e| e.name == "pass-only").unwrap();
    let wanted = expected(e);
    let actual = serde_json::to_value(report::check(&e.source)).unwrap();
    completion_explorer_product::check_report(&actual, &e.own, &wanted).unwrap();
    for code in ["W-UNREACH-0", "W-UNREACH-1", "W-UNREACH-2", "W-UNREACH-3"] {
        let index = actual["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .position(|d| d["code"] == code)
            .unwrap();
        for field in ["code", "message", "span"] {
            let mut broken = actual.clone();
            broken["diagnostics"][index][field] = serde_json::json!("corrupt");
            assert!(
                completion_explorer_product::check_report(&broken, &e.own, &wanted).is_err(),
                "{code} {field}"
            );
        }
    }
    let mut broken = actual.clone();
    broken["properties"][0]["verdict"] = serde_json::json!("violated");
    assert!(completion_explorer_product::check_report(&broken, &e.own, &wanted).is_err());
    for extra in [false, true] {
        let mut broken = actual.clone();
        let ps = broken["properties"].as_array_mut().unwrap();
        if extra {
            ps.push(ps[0].clone());
        } else {
            ps.pop();
        }
        assert!(completion_explorer_product::check_report(&broken, &e.own, &wanted).is_err());
    }
}
#[test]
fn unsupported_shapes_are_not_covered_and_never_fall_back_to_engine() {
    let e = population().remove(0);
    let m = model(&e);
    for fault in 0..8 {
        let mut outside = m.clone();
        match fault {
            0 => outside.bounds.cases = 2,
            1 => outside.start_policy = Some(Default::default()),
            2 => outside.capabilities[0].duration = 0,
            3 => {
                outside.capabilities[0].cancel =
                    relca_core::ir::Cancel::Cooperative { ack_within: 1 }
            }
            4 => {
                outside.capabilities[0].atomic = Some(relca_core::ir::Atomic {
                    condition: relca_core::ir::Cond::Eq(relca_core::ir::ResultMatch {
                        binding: relca_core::ir::BindingId(0),
                        value: relca_core::ir::ValueId(0),
                    }),
                    commit_when: true,
                    commit: relca_core::ir::ValueId(0),
                    reject: relca_core::ir::ValueId(1),
                    writes: vec![],
                    span: m.process.span,
                })
            }
            5 => outside.horizon = Some(1),
            6 => outside.bounds.clock = 21,
            7 => {
                outside.capabilities[0].requires = Some(relca_core::ir::Requires {
                    condition: relca_core::ir::Cond::Eq(relca_core::ir::ResultMatch {
                        binding: relca_core::ir::BindingId(0),
                        value: relca_core::ir::ValueId(0),
                    }),
                    otherwise: relca_core::ir::ValueId(1),
                    span: m.process.span,
                })
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(adapt(&outside), Err(NotCovered(_))),
            "fault {fault}"
        );
    }
    let outside = lower::lower(
        &syntax::parse(include_str!("../../../fixtures/any-drain/model.relca")).unwrap(),
    )
    .unwrap();
    assert!(matches!(adapt(&outside), Err(NotCovered(_))));
}
