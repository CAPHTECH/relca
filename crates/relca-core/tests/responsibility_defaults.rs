//! ADR0054 hand-oracle acceptance. No production undeclared-responsibility helper.
#[path = "support/responsibility_defaults_inputs.rs"]
mod input;
#[path = "support/responsibility_defaults_oracle.rs"]
mod oracle;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn warnings(v: &Value) -> Vec<&Value> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-RESP-0")
        .collect()
}
fn expected(source: &str, actors: &[&str]) -> Vec<Value> {
    let m = lower::lower(&syntax::parse(source).unwrap()).unwrap();
    actors.iter().map(|name| {
        let a = m.actors.iter().find(|a| a.name == *name).unwrap();
        json!({"code":"W-RESP-0","severity":"warning","message":format!("{name} has no declared responsibility"),"span":a.span})
    }).collect()
}
#[test]
fn hand_names_order_complete_rows_and_single_fault_pairs() {
    let mut failures = Vec::new();
    for (name, source, actors) in input::cases() {
        if std::panic::catch_unwind(|| {
            let v = json!(report::check(&source));
            assert_eq!(v["verdict"], "satisfied", "{name}: {v}");
            assert_eq!(
                warnings(&v),
                expected(&source, &actors).iter().collect::<Vec<_>>(),
                "{name}: {v}"
            );
        })
        .is_err()
        {
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "all failing hand pairs: {failures:?}");
}
#[test]
fn no_effect_remains_warning_free() {
    let v = json!(report::check(&input::plain()));
    assert_eq!(v["verdict"], "satisfied");
    assert!(warnings(&v).is_empty());
}
#[test]
fn independent_ir_candidate_walker_agrees_with_every_hand_pair() {
    for (name, source, names) in input::cases() {
        let model = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
        assert_eq!(
            json!(oracle::expected(&model)),
            json!(expected(&source, &names)),
            "{name}"
        );
    }
}
#[test]
fn state_guard_u_is_still_unknown_but_e_is_known() {
    let v = json!(report::check(&input::state()));
    let row = &v["agency"]["viewpoints"]["effects"]["rows"][0];
    assert_eq!(row["e"]["actors"], json!(["Poster"]));
    assert_eq!(row["process_judgements"]["reason"], "state_guards");
    assert_eq!(row["state_refusers"], json!([]));
    assert_eq!(warnings(&v).len(), 1);
    assert!(
        v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("responsibility_people_undeclared"))
    );
}
#[test]
fn names_and_grain_are_not_meaning() {
    let source = input::state();
    let changed = source
        .replace("Poster", "Machine")
        .replace("capacity 1;", "grain document; capacity 1;");
    let v = json!(report::check(&changed));
    assert_eq!(
        warnings(&v),
        expected(&changed, &["Machine"]).iter().collect::<Vec<_>>()
    );
}
#[test]
fn duties_do_not_change_hand_calculated_runtime_or_reference_agreement() {
    for source in [
        input::binding(),
        input::duty(),
        input::state(),
        input::plain(),
    ] {
        let m = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(e.terminals, r.outcome.terminals);
        assert_eq!(e.properties, r.outcome.properties);
        let v = json!(report::check(&source));
        for t in v["terminals"].as_array().unwrap() {
            let minutes = if !source.contains("Input.seed()") {
                1
            } else if t["end"] == "returned" {
                2
            } else {
                3
            };
            assert_eq!(t["elapsed"], json!({"min":minutes,"max":minutes}));
            assert_eq!(t["work"], json!({"min":minutes,"max":minutes}));
        }
    }
}
#[test]
fn anonymous_atomic_and_version_check_keep_one_warning_per_actor() {
    for (source, names) in [
        (
            include_str!("../../../fixtures/agy2-atomic-u/model.relca").to_owned(),
            vec!["Approver", "Ledger"],
        ),
        (
            include_str!("../../../fixtures/agy2-version-check/model.relca").to_owned(),
            vec!["Ledger"],
        ),
    ] {
        let ast = syntax::parse(&source).unwrap();
        let m = lower::lower(&ast).unwrap();
        let original = json!(report::check(&source));
        assert_eq!(
            warnings(&original),
            expected(&source, &names).iter().collect::<Vec<_>>()
        );
        // Remove executors and people as one attribution transformation, not a label inference.
        let mut anonymous = ast;
        anonymous
            .items
            .retain(|i| !matches!(i, relca_core::ast::Item::People(_)));
        for item in &mut anonymous.items {
            if let relca_core::ast::Item::Actor(a) = item {
                let capacity = a.items.iter().find_map(|i| match i {
                    relca_core::ast::ActorItem::Executors(p) => Some(p.value.len() as u64),
                    _ => None,
                });
                if let Some(value) = capacity
                    && !a
                        .items
                        .iter()
                        .any(|i| matches!(i, relca_core::ast::ActorItem::Capacity(_)))
                {
                    a.items.push(relca_core::ast::ActorItem::Capacity(
                        relca_core::ast::Spanned {
                            value,
                            span: a.span,
                        },
                    ));
                }
                a.items
                    .retain(|i| !matches!(i, relca_core::ast::ActorItem::Executors(_)));
            }
        }
        let result = lower::lower(&anonymous).unwrap();
        let got: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == "W-RESP-0")
            .map(|d| d.message.clone())
            .collect();
        assert_eq!(
            got,
            names
                .iter()
                .map(|n| format!("{n} has no declared responsibility"))
                .collect::<Vec<_>>()
        );
        assert_eq!(m.process.bindings.len(), result.process.bindings.len());
    }
}

#[test]
fn people_only_pair_preserves_anonymous_capacity_and_rejects_missing_judge() {
    let anonymous = lower::lower(&syntax::parse(&input::binding()).unwrap()).unwrap();
    let source = input::people_only();
    let people = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
    for (a, b) in anonymous.actors.iter().zip(&people.actors) {
        assert_eq!(a.capacity, b.capacity);
        assert_eq!(a.executors, b.executors);
        assert!(b.executors.is_none());
    }
    let want = expected(&source, &["Judge", "Poster"]);
    let actual = json!(report::check(&source));
    assert_eq!(warnings(&actual), want.iter().collect::<Vec<_>>());
    let mut missing_judge = actual.clone();
    missing_judge["diagnostics"]
        .as_array_mut()
        .unwrap()
        .retain(|d| d["message"] != "Judge has no declared responsibility");
    assert_ne!(warnings(&missing_judge), want.iter().collect::<Vec<_>>());
    for (source, absent_people) in [(input::binding(), true), (source, false)] {
        let v = json!(report::check(&source));
        assert_eq!(
            v["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("responsibility_people_undeclared")),
            absent_people
        );
    }
}

#[test]
fn shared_person_one_duty_suppresses_both_actors_and_discloses_assignment_scope() {
    let source = input::shared_person(true);
    let actual = json!(report::check(&source));
    assert_eq!(actual["verdict"], "satisfied");
    assert!(warnings(&actual).is_empty());
    assert_eq!(actual["responsibilities"].as_array().unwrap().len(), 1);
    assert_eq!(actual["responsibilities"][0]["person"], "p");
    assert!(
        actual["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("responsibility_assignment_scope"))
    );
    let no_duty = input::shared_person(false);
    let control = json!(report::check(&no_duty));
    assert_eq!(
        warnings(&control),
        expected(&no_duty, &["Judge", "Poster"])
            .iter()
            .collect::<Vec<_>>()
    );
    assert!(
        !control["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("responsibility_assignment_scope"))
    );
}

#[test]
fn full_mixed_diagnostics_keep_hand_order_and_reject_swapping() {
    let actual = json!(report::check(&input::mixed_diagnostics()));
    let want = input::mixed_expected();
    assert_eq!(actual["verdict"], "satisfied");
    assert_eq!(actual["diagnostics"], want);
    let mut swapped = actual["diagnostics"].clone();
    swapped.as_array_mut().unwrap().swap(0, 1);
    assert_ne!(
        swapped, want,
        "same full-vector equality rejects relative-order fault"
    );
}

#[test]
fn effects_not_derived_discloses_default_candidates_with_reason_and_process() {
    // Fixed input sizes, not a threshold learned from production output.
    for (calls, derived) in [(1, true), (100, false)] {
        let source = input::effects_limit(calls);
        let m = lower::lower(&syntax::parse(&source).unwrap())
            .unwrap_or_else(|e| panic!("calls={calls}: {e:?}"));
        assert_eq!(
            m.diagnostics
                .iter()
                .filter(|d| d.code == "W-RESP-0")
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>(),
            ["Poster has no declared responsibility"],
            "known legacy E is not retracted when effects hits its own limit"
        );
        let agency = json!(m.agency);
        assert_eq!(
            agency["viewpoints"]["effects"]["status"],
            if derived { "derived" } else { "not_derived" }
        );
        if !derived {
            assert_eq!(
                agency["viewpoints"]["effects"]["reason"],
                "derivation_limit"
            );
        }
        assert_eq!(
            agency["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s.as_str() == Some(input::UNDERIVED_NOTICE))
                .count(),
            usize::from(!derived),
            "reasoned W-RESP-0 candidate disclosure, not a claim of no U/E"
        );
    }
}
