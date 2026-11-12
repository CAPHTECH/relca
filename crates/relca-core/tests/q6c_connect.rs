//! Test-first Q6c connection acceptance. Hand values precede implementation.
#[path = "support/q6a_inputs.rs"]
mod a_input;
#[path = "support/q6b_inputs.rs"]
mod b_input;
#[path = "support/q6b_review_inputs.rs"]
mod review;
// The review input module uses this conventional alias.
use b_input as input;
#[path = "support/q6b_oracle.rs"]
mod b_hand;
#[path = "support/q9_inputs.rs"]
mod contest_input;
#[path = "support/q6c_oracle.rs"]
mod h;
#[path = "support/q6c_inputs.rs"]
mod i;
#[path = "support/q6c_record.rs"]
mod private;
mod private_types {
    pub use relca_core::ir::{ActorId, Model, PersonId};
}
use relca_core::{
    agency,
    compare::{self, Analysis},
    diff, lower, report,
    sensitivity::{self, contract::Contract, input::Mode, inputs::EffectiveInputs},
    syntax,
};
use serde_json::json;
fn model(s: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(s).expect("hand source parses")).expect("hand source lowers")
}
#[test]
fn green_existing_work_profile_runtime_has_hand_probability_and_fixed_duration() {
    for (kind, p) in [
        ("base", "1/10"),
        ("probability", "1/4"),
        ("value", "0"),
        ("basis", "1/10"),
    ] {
        let s = i::point(kind);
        let m = model(&s);
        let e = relca_core::engine::explore(&m);
        let r = relca_core::reference::explore(&m);
        assert_eq!(r.stop, relca_core::reference::StopReason::Complete);
        for out in [&e, &r.outcome] {
            assert!(!out.truncated);
            let properties: Vec<_> = out
                .properties
                .iter()
                .filter(|p| p.cases.is_some())
                .collect();
            assert_eq!(properties.len(), 1);
            let cases = properties[0].cases.as_ref().unwrap();
            assert_eq!(cases.len(), 1);
            let actual = cases[0].probability.as_ref().unwrap();
            assert_eq!(
                (actual.min.to_string(), actual.max.to_string()),
                (p.into(), p.into())
            );
        }
        let v = serde_json::to_value(report::check(&s)).unwrap();
        h::probability(&v, p);
        h::two_minutes(&v);
        h::uncertainty(&v, kind);
    }
}
#[test]
fn green_existing_degradation_scope_keeps_whole_process_retention() {
    let v = serde_json::to_value(report::check(&b_input::base())).unwrap();
    h::scope(&v);
    assert_eq!(v["agency"]["degradation"], b_hand::expected("a7"));
}
fn source(kind: &str) -> String {
    match kind {
        "work" => i::work(),
        "bands" => i::bands(),
        "degrade" => i::degradation(),
        _ => panic!(),
    }
}
macro_rules! comparison {
    ($name:ident,$kind:literal,$compare:literal) => {
        #[test]
        fn $name() {
            let s = source($kind);
            for reverse in [false, true] {
                let plans = if reverse {
                    ["To", "From"]
                } else {
                    ["From", "To"]
                };
                if $compare {
                    h::compare(
                        &serde_json::to_value(compare::compare(&s, &plans, Analysis::Exhaustive))
                            .unwrap(),
                        $kind,
                        reverse,
                    );
                } else {
                    h::diff(
                        &serde_json::to_value(diff::diff(&s, plans[0], plans[1])).unwrap(),
                        $kind,
                        reverse,
                    );
                }
            }
        }
    };
}
comparison!(i17_diff_legacy_and_work_profiles, "work", false);
comparison!(i17_compare_legacy_and_work_profiles, "work", true);
comparison!(i17_diff_explicit_band_change, "bands", false);
comparison!(i17_compare_explicit_band_change, "bands", true);
comparison!(i17_diff_old_if_and_degrade_scope, "degrade", false);
comparison!(i17_compare_old_if_and_degrade_scope, "degrade", true);

#[test]
fn i17_not_derived_side_is_not_a_known_equal_dependency() {
    let a = report::check(&b_input::base());
    let z = report::check(&b_input::source("requires"));
    for reverse in [false, true] {
        let (from, to) = if reverse { (&z, &a) } else { (&a, &z) };
        fn side(r: &report::Report) -> agency::ComparisonSide<'_> {
            agency::ComparisonSide {
                agency: r
                    .agency
                    .as_ref()
                    .and_then(agency::AgencyDerivation::as_derived),
                responsibilities: r.responsibilities.as_deref(),
            }
        }
        let v = serde_json::to_value(agency::differences(side(from), side(to))).unwrap();
        let rows: Vec<_> = v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["viewpoint"] == "degradation" && r["item"] == "degraded")
            .collect();
        assert_eq!(rows.len(), 1, "not_derived is not an empty diff");
        let r = rows[0];
        assert_eq!(r["call"], "Manual");
        assert_eq!(r["status"], "not_comparable");
        let missing = if reverse { "from" } else { "to" };
        let known = if reverse { "to" } else { "from" };
        assert_eq!(
            r[missing],
            json!({"status":"not_derived","reason":format!("{missing}.agency.degradation.Manual.degraded:requires")})
        );
        assert_eq!(r[known], json!({"status":"derived","reason":null}));
    }
}
#[test]
fn i17_derived_degradation_difference_carries_counts_and_observation() {
    let a = report::check(&b_input::base());
    let z = report::check(&b_input::source("other"));
    let v = serde_json::to_value(agency::differences(
        agency::ComparisonSide {
            agency: a
                .agency
                .as_ref()
                .and_then(agency::AgencyDerivation::as_derived),
            responsibilities: None,
        },
        agency::ComparisonSide {
            agency: z
                .agency
                .as_ref()
                .and_then(agency::AgencyDerivation::as_derived),
            responsibilities: None,
        },
    ))
    .unwrap();
    let rows: Vec<_> = v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["viewpoint"] == "degradation")
        .collect();
    assert_eq!(rows.len(), 1);
    let r = rows[0];
    assert_eq!(r["item"], "degraded");
    assert_eq!(r["call"], "Manual");
    assert_eq!(r["status"], "changed");
    assert_eq!(r["from"], b_hand::expected("a7")["routes"][0]["degraded"]);
    assert_eq!(r["to"], b_hand::expected("other")["routes"][0]["degraded"]);
    serde_json::from_value::<agency::Differences>(v.clone()).unwrap();
    let mut corrupt = v;
    corrupt["rows"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["viewpoint"] == "degradation")
        .unwrap()["invented"] = json!(true);
    assert!(serde_json::from_value::<agency::Differences>(corrupt).is_err());
}
#[test]
fn i17_new_configuration_wire_is_closed() {
    let v = serde_json::to_value(diff::diff(&i::work(), "From", "To")).unwrap();
    h::diff(&v, "work", false);
    serde_json::from_value::<diff::DiffReport>(v.clone()).unwrap();
    for kind in ["qualification", "performance"] {
        let mut bad = v.clone();
        let r = bad["differences"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["kind"] == kind)
            .unwrap();
        r["invented"] = json!(true);
        assert!(
            serde_json::from_value::<diff::DiffReport>(bad).is_err(),
            "unknown {kind} key"
        );
    }
}

#[test]
fn i17_unconnected_engine_comparisons_are_q6_refusals() {
    for kind in ["s1", "retry"] {
        for one in [false, true] {
            let s = i::refused_comparison(kind, one);
            for compare_mode in [false, true] {
                let v = if compare_mode {
                    serde_json::to_value(compare::compare(
                        &s,
                        &["From", "To"],
                        Analysis::Exhaustive,
                    ))
                    .unwrap()
                } else {
                    serde_json::to_value(diff::diff(&s, "From", "To")).unwrap()
                };
                assert_eq!(v["verdict"], "unsupported", "{kind}/{one}: {v}");
                // Every refused selected report must name the Q6 boundary,
                // never a downstream retry-only gate or incidental parse error.
                // A clean non-I19 side need not itself be refused.
                let children: Vec<_> = if compare_mode {
                    v["plans"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| &p["report"])
                        .collect()
                } else {
                    vec![&v["from"]["report"], &v["to"]["report"]]
                };
                assert_eq!(children.len(), 2);
                let mut refused = 0;
                for child in children {
                    if child["verdict"] == "unsupported" {
                        h::refusal(child, false, false);
                        refused += 1;
                    } else {
                        h::no_errors(child);
                        assert_eq!(child["verdict"], "satisfied");
                    }
                }
                assert!(refused > 0, "a Q6/I19 side must be refused");
            }
        }
    }
}
#[test]
fn i18_gate_allows_only_kernel_probability_value_and_basis() {
    let base =
        Contract::from_model(&model(&i::point("base"))).expect("Q6 work profile gate is connected");
    for kind in ["probability", "value", "basis"] {
        let changed = Contract::from_model(&model(&i::point(kind))).unwrap();
        assert!(base.differences(&changed).is_empty(), "allowed {kind}");
    }
}
#[test]
fn i18_gate_rejects_qualification_band_load_process_and_question() {
    let base = Contract::from_model(&model(&i::point("base"))).unwrap();
    for kind in ["qualification", "band", "load", "process", "query"] {
        let changed = Contract::from_model(&model(&i::point(kind))).unwrap();
        assert!(!base.differences(&changed).is_empty(), "forbidden {kind}");
        assert!(!changed.differences(&base).is_empty());
    }
}
#[test]
fn i18_qualification_is_a_set_not_a_new_assignment_order() {
    let s = i::edit(
        &i::point("base"),
        "qualified_executors [q]",
        "qualified_executors [q,p]",
    );
    let s = i::edit(
        &s,
        "executor q { [0,1): kernel Low; [1,infinity): kernel Half }",
        "executor q { [0,1): kernel Low; [1,infinity): kernel Half }; executor p { [0,1): kernel Low; [1,infinity): kernel Half }",
    );
    let changed = i::edit(&s, "qualified_executors [q,p]", "qualified_executors [p,q]");
    let a = Contract::from_model(&model(&s)).unwrap();
    let b = Contract::from_model(&model(&changed)).unwrap();
    assert!(a.differences(&b).is_empty());
}
#[test]
fn i18_effective_inputs_include_every_selector_kernel() {
    for kind in [
        "base",
        "probability",
        "value",
        "basis",
        "qualification",
        "band",
    ] {
        let v = serde_json::to_value(EffectiveInputs::from_model(&model(&i::point(kind))).unwrap())
            .unwrap();
        h::effective(&v, kind);
        serde_json::from_value::<EffectiveInputs>(v.clone()).unwrap();
        let mut bad = v;
        bad["capabilities"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["capability"] == "Desk.run")
            .unwrap()["performance"]["invented"] = json!(true);
        assert!(serde_json::from_value::<EffectiveInputs>(bad).is_err());
    }
}
#[test]
fn i18_conditional_effective_kernel_keeps_all_conditions_writes_and_zero_mass() {
    let s = a_input::source("given-finish");
    let m = model(&s);
    let inputs = serde_json::to_value(EffectiveInputs::from_model(&m).unwrap()).unwrap();
    let op = inputs["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["capability"] == "Sampler.sample")
        .unwrap();
    let branch = |p: &str, value: &str, owner: &str| json!({"probability":p,"result":"ok","writes":[{"variable":format!("{owner}hit"),"value":value}]});
    let kernel = |owner: &str| {
        json!({"name":"K","basis":null,"given":[format!("{owner}flag")],"rows":[
            {"conditions":[{"variable":format!("{owner}flag"),"value":"false"}],"branches":[branch("0","true",owner),branch("1","false",owner)]},
            {"conditions":[{"variable":format!("{owner}flag"),"value":"true"}],"branches":[branch("1/2","true",owner),branch("1/2","false",owner)]}
        ]})
    };
    assert_eq!(
        op["performance"],
        json!({"profile":"P","basis":"fixture assumption; not measured","load":"none","snapshot":"start","duration_effect":"not_supported",
        "rows":[{"executor":"anonymous","lower":null,"upper":null,"kernel":kernel("case.observed.")}]})
    );
    let v = serde_json::to_value(report::check(&s)).unwrap();
    let row = v["uncertainty"]["aleatory"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "kernel" && r["capability"] == "Sampler.sample")
        .unwrap();
    let mut actual = row.clone();
    actual.as_object_mut().unwrap().remove("span").unwrap();
    let mut expected = kernel("");
    expected["kind"] = json!("kernel");
    expected["capability"] = json!("Sampler.sample");
    assert_eq!(actual, expected);
}
#[test]
fn i18_accepted_points_execute_with_exact_hand_values_and_evidence() {
    let kinds = ["base", "probability", "value", "basis"];
    let path = i::suite(&kinds.iter().map(|k| (*k, i::point(k))).collect::<Vec<_>>());
    h::suite(
        &serde_json::to_value(sensitivity::run::run(&path, Mode::Exhaustive)).unwrap(),
        &kinds,
    );
}
#[test]
fn i18_forbidden_points_refuse_whole_suite_before_any_execution() {
    for kind in ["qualification", "band", "load", "process", "query"] {
        let path = i::suite(&[("base", i::point("base")), (kind, i::point(kind))]);
        let v = serde_json::to_value(sensitivity::run::run(&path, Mode::Exhaustive)).unwrap();
        assert_eq!(v["verdict"], "invalid");
        assert_eq!(
            v["points"][1]["reason"], "sensitivity_contract_mismatch",
            "{kind}: {v}"
        );
        for p in v["points"].as_array().unwrap() {
            assert_ne!(p["status"], "evaluated");
        }
        assert_eq!(v["diagnostics"].as_array().unwrap().len(), 1);
    }
}
#[test]
fn i18_degrade_sensitivity_remains_specifically_unconnected() {
    let s = b_input::edit(
        &b_input::base(),
        "verify reachable end.done",
        "verify probability reached end.done >= 1",
    );
    let path = i::suite(&[("a", s.clone()), ("b", s)]);
    let v = serde_json::to_value(sensitivity::run::run(&path, Mode::Exhaustive)).unwrap();
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(v["diagnostics"].as_array().unwrap().len(), 2);
    for p in v["points"].as_array().unwrap() {
        assert_eq!(p["status"], "refused");
        assert_eq!(p["reason"], "sensitivity_feature_combination");
        h::sensitivity_refusal(p);
    }
}
#[test]
fn mc_arrival_nine_withdraws_normal_and_degraded_but_not_prefix() {
    for arrival in [0, 9] {
        let v = serde_json::to_value(report::check(&review::coverage(arrival, false))).unwrap();
        let t = &v["agency"]["degradation"];
        assert_eq!(t["status"], "derived");
        assert_eq!(
            t["routes"][0]["common_prefix"],
            b_hand::counts(&[("Probe.check", 1, 1)])
        );
        for side in ["normal", "degraded"] {
            let row = &t["routes"][0][side];
            if arrival == 9 {
                assert_eq!(
                    row,
                    &json!({"status":"not_derived","reason":"calendar","ends":null,"observation":null})
                );
            } else {
                assert_eq!(row, &b_hand::expected("suffix")["routes"][0][side]);
            }
        }
    }
}
