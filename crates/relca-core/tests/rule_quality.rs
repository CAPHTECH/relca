#[path = "support/rule_quality_inputs.rs"]
mod h;
use relca_core::{compare, engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn collect(src: &str, c: &str, sim: bool, independent: bool) -> Value {
    let m = lower::lower_with_selection(&syntax::parse(src).unwrap(), Some(c), None).unwrap();
    if independent {
        if sim {
            let x = reference::simulate(&m);
            assert_eq!(x.stop, reference::StopReason::Complete);
            json!(x.outcome.quality.unwrap())
        } else {
            let x = reference::explore(&m);
            assert_eq!(x.stop, reference::StopReason::Complete);
            json!(x.outcome.quality.unwrap())
        }
    } else if sim {
        json!(engine::simulate(&m).quality.unwrap())
    } else {
        json!(engine::explore(&m).quality.unwrap())
    }
}
fn run(sim: bool, independent: bool) {
    let src = h::source();
    for (c, enabled) in [("AsIs", true), ("ToBeB", false)] {
        let q = collect(&src, c, sim, independent);
        h::assert_quality(&q, enabled, sim, 3);
        h::assert_constant_quality(
            &q,
            &collect(&h::constant_control(&src, enabled), c, sim, independent),
        );
    }
}
#[test]
fn engine_exhaustive_rule_and_constant_pair() {
    run(false, false);
}
#[test]
fn revocable_rule_records_unchecked_but_uses_the_amended_fixed_value() {
    let s = h::revocable_source();
    let v = json!(report::check_configuration(&s, "ToBeB"));
    h::assert_revocable_report(&v);
    for independent in [false, true] {
        h::assert_quality(&collect(&s, "ToBeB", false, independent), false, false, 3);
    }
    let mut bad = v.clone();
    bad["quality"]["cohort"][0]["value"] = h::exact("1");
    assert!(std::panic::catch_unwind(|| h::assert_revocable_report(&bad)).is_err());
    let mut bad = v;
    bad["unchecked"]
        .as_array_mut()
        .unwrap()
        .retain(|x| x != "dynamic_authority_revocation");
    assert!(std::panic::catch_unwind(|| h::assert_revocable_report(&bad)).is_err());
}
#[test]
fn reference_exhaustive_rule_and_constant_pair() {
    run(false, true);
}
#[test]
fn engine_sample_rule_and_constant_pair() {
    run(true, false);
}
#[test]
fn reference_sample_rule_and_constant_pair() {
    run(true, true);
}
#[test]
fn compare_cp_hard_objectives_use_rule_values_not_point_decisions() {
    for sim in [false, true] {
        for tight in [false, true] {
            let s = if tight {
                h::old::replace(&h::source(), "<= 0.6", "<= 0.4")
            } else {
                h::source()
            };
            h::assert_compare(
                &json!(compare::compare(
                    &s,
                    &["AsIs", "ToBeB"],
                    if sim {
                        compare::Analysis::Simulation
                    } else {
                        compare::Analysis::Exhaustive
                    }
                )),
                sim,
                tight,
            );
        }
    }
}
#[test]
fn invoice_rule_and_no_rule_hand_risks_in_both_interpreters() {
    let s = h::invoice_rule();
    for (c, risk) in [
        ("AsIs", "93/125000"),
        ("ToBeP", "93/125000"),
        ("ToBeA", "231/125000"),
        ("ToBeB", "81/12500"),
    ] {
        for independent in [false, true] {
            let without_defer =
                h::without_defer(include_str!("../../../examples/invoice/rules.relca"));
            let input = if c == "ToBeB" { &without_defer } else { &s };
            let q = collect(input, c, false, independent);
            assert_eq!(q["cohort"][0]["value"], h::exact(risk));
        }
    }
}
#[test]
fn unsupported_rule_type_and_latent_policy_are_still_fail_closed() {
    assert_eq!(
        json!(report::check(&h::old::record_rule(false)))["verdict"],
        "satisfied"
    );
    let refused = json!(report::check(&h::old::record_rule(true)));
    assert_eq!(refused["verdict"], "unsupported");
    assert!(
        refused["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0" && d["message"] == "rule value type is unsupported")
    );
    let s = h::old::append(&h::source(), "latent case { truth: bool = true }");
    let s = h::old::replace(
        &s,
        "process P { x = A.run(); end done }",
        "process P { if truth == true { x = A.run(); end done } else { end other } }",
    );
    assert!(
        json!(report::check_configuration(&s, "AsIs"))["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-POL-LATENT")
    );
}
#[test]
fn independent_reference_does_not_call_engine_reducers() {
    for src in [
        include_str!("../src/reference/quality.rs"),
        include_str!("../src/reference/quality/quantile.rs"),
    ] {
        for forbidden in [
            "crate::quality::exhaustive(",
            "crate::quality::simulation(",
            "crate::quality::sample_quantile(",
            "crate::quality::inverse_quantile(",
        ] {
            assert!(
                !src.contains(forbidden),
                "independent reference may not call {forbidden}"
            );
        }
    }
}
#[test]
fn constant_only_positive_controls_validate_hand_cells_in_both_modes_and_interpreters() {
    for sim in [false, true] {
        for independent in [false, true] {
            for enabled in [false, true] {
                let s = h::constant_control(&h::source(), enabled);
                let q = collect(&s, "AsIs", sim, independent);
                h::assert_report(&json!({"quality":q,"assumptions":[]}), enabled, sim);
            }
        }
    }
}
#[test]
fn hand_oracle_rejects_one_wrong_ratio_without_other_changes() {
    let q = collect(
        &h::constant_control(&h::source(), true),
        "AsIs",
        false,
        false,
    );
    h::assert_quality(&q, true, false, 3);
    let mut bad = q;
    bad["cohort"][0]["value"]["upper"] = json!("0");
    assert!(std::panic::catch_unwind(|| h::assert_quality(&bad, true, false, 3)).is_err());
}

#[test]
fn constant_pair_comparator_rejects_condition_outside_the_one_alias() {
    let q = collect(&h::source(), "AsIs", false, false);
    let twin = collect(
        &h::constant_control(&h::source(), true),
        "AsIs",
        false,
        false,
    );
    h::assert_constant_quality(&q, &twin);
    let mut bad = twin;
    bad["properties"][0]["condition"] = json!("rule_enabled == false");
    assert!(std::panic::catch_unwind(|| h::assert_constant_quality(&q, &bad)).is_err());
}

#[test]
fn multiplicity_comparator_rejects_shifted_m_and_missing_schema_fields() {
    let v = json!({"evaluation":{"statistics":{"multiplicity":{"m":2,"alpha":"1/8"}}}});
    h::assert_multiplicity(&v);
    let mut bad = v.clone();
    bad["evaluation"]["statistics"]["multiplicity"]["m"] = json!(3);
    assert!(std::panic::catch_unwind(|| h::assert_multiplicity(&bad)).is_err());
    for key in ["m", "alpha"] {
        let mut missing = v.clone();
        missing["evaluation"]["statistics"]["multiplicity"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(h::multiplicity(&missing).is_err());
    }
}
#[test]
fn approval_pair_is_real_gate_not_a_rule_label_or_success_override() {
    for sim in [false, true] {
        for independent in [false, true] {
            for pass in [false, true] {
                for c in ["AsIs", "ToBeB"] {
                    let q = collect(&h::approval_source(pass), c, sim, independent);
                    h::assert_approval(&q, pass || c == "ToBeB", sim);
                }
            }
        }
    }
}
#[test]
fn hard_removed_or_loosened_does_not_turn_objectives_into_verdicts() {
    let s = h::old::replace(&h::source(), "; minimize quantile.All", "");
    let s = h::old::replace(&s, "<= 0.6", "<= 1");
    for src in [
        s.clone(),
        h::old::replace(&s, "hard { cohort.Rate <= 1 }", ""),
    ] {
        let v = json!(compare::compare(
            &src,
            &["AsIs", "ToBeB"],
            compare::Analysis::Exhaustive
        ));
        assert_eq!(v["verdict"], "satisfied");
        assert_eq!(v["evaluation"]["pareto"]["non_dominated"], json!(["ToBeB"]));
        assert_eq!(
            v["evaluation"]["pareto"]["dominated"][0]["configuration"],
            "AsIs"
        );
    }
}
