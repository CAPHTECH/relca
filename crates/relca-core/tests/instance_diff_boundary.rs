#[path = "support/instance_diff_boundary.rs"]
mod hand;
use relca_core::{ast, diff, ir, lower, syntax};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn instance_map(m: &ir::Model) -> BTreeMap<String, Option<ir::ActorInstance>> {
    m.actors
        .iter()
        .map(|a| (a.name.clone(), a.metadata.instance))
        .collect()
}
#[test]
fn all_configuration_instance_maps_equal_the_hand_map_with_and_without_per_case() {
    for per_case in [false, true] {
        for amended in [false, true] {
            let source = hand::source(per_case, amended);
            let a = syntax::parse(&source).unwrap();
            let configurations: Vec<_> = a
                .items
                .iter()
                .filter_map(|i| {
                    if let ast::Item::Configuration(c) = i {
                        Some(c.name.value.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(configurations, hand::CONFIGURATIONS);
            let mut wanted = BTreeMap::from([(
                "Invoice".into(),
                per_case.then_some(ir::ActorInstance::PerCase),
            )]);
            if amended {
                wanted.insert("Rule".into(), None);
            }
            for configuration in configurations {
                let m = lower::lower_configuration(&a, configuration).unwrap().model;
                assert_eq!(
                    instance_map(&m),
                    wanted,
                    "{per_case}/{amended}/{configuration}"
                );
                if amended {
                    let rule = m
                        .actors
                        .iter()
                        .find(|a| a.name == "Rule")
                        .unwrap()
                        .metadata
                        .rule
                        .as_ref()
                        .unwrap();
                    let changed = configuration == "Right";
                    assert_eq!(rule.effective_version, if changed { "v2" } else { "v1" });
                    assert_eq!(
                        rule.amendment.is_some(),
                        changed,
                        "amendment is genuinely applied"
                    );
                    let ir::InitialValue::Fixed(ir::StateValue::Value(value)) =
                        m.state_variables[rule.values[0].variable.0].initial
                    else {
                        panic!("fixed bool");
                    };
                    let ty = m.state_variables[rule.values[0].variable.0].base;
                    assert_eq!(
                        m.types[ty.0].members[value.0],
                        if changed { "false" } else { "true" }
                    );
                }
            }
        }
    }
}
fn run(source: &str) -> (Value, i32) {
    let r = diff::diff(source, "Left", "Right");
    let exit = r.verdict.exit_code();
    (serde_json::to_value(r).unwrap(), exit)
}
#[test]
fn diff_positive_hand_values_are_not_a_cross_source_instance_comparison() {
    for per_case in [false, true] {
        for amended in [false, true] {
            let (r, exit) = run(&hand::source(per_case, amended));
            hand::positive(&r, per_case, amended, exit).unwrap();
        }
    }
    assert_eq!(
        hand::source(false, false),
        hand::source(true, false).replace("instance per_case; ", "")
    );
}
#[test]
fn forbidden_overrides_have_exact_syntax_or_lower_diagnostics() {
    for case in hand::REJECTIONS {
        let source = case.source();
        // Direct entry distinguishes syntax failure from semantic lower failure.
        match syntax::parse(&source) {
            Ok(a) => {
                for configuration in hand::CONFIGURATIONS {
                    let diagnostics = lower::lower_configuration(&a, configuration)
                        .err()
                        .expect("override must be rejected");
                    assert_eq!(
                        serde_json::to_value(diagnostics).unwrap(),
                        case.diagnostics(),
                        "{} {configuration}",
                        case.name()
                    );
                }
            }
            Err(diagnostics) => {
                let expected = case.diagnostics();
                let errors: Vec<_> = expected
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|d| d["severity"] == "error")
                    .cloned()
                    .collect();
                assert_eq!(serde_json::to_value(diagnostics).unwrap(), json!(errors));
            }
        }
        let (r, exit) = run(&source);
        hand::rejected(&r, case, exit).unwrap_or_else(|e| panic!("{}: {e}", case.name()));
    }
}
#[test]
fn instance_map_comparator_rejects_one_side_value_loss_and_invented_changes() {
    let (r, exit) = run(&hand::source(true, false));
    hand::positive(&r, true, false, exit).unwrap();
    for side in ["from", "to"] {
        let index = r[side]["report"]["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .position(|a| a["field"] == "Invoice.instance")
            .unwrap();
        let mut wrong = r.clone();
        wrong[side]["report"]["assumptions"][index]["value"] = json!("shared(default)");
        assert!(hand::positive(&wrong, true, false, exit).is_err());
        let mut missing = r.clone();
        missing[side]["report"]["assumptions"]
            .as_array_mut()
            .unwrap()
            .remove(index);
        assert!(hand::positive(&missing, true, false, exit).is_err());
    }
    let mut extra = r.clone();
    extra["differences"] = json!([{"kind":"actor","target":"Invoice","change":"modified"}]);
    assert!(hand::positive(&extra, true, false, exit).is_err());
    assert!(hand::positive(&r, true, false, 4).is_err());
}
