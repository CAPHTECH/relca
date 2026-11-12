//! P3a R4: varying an assumption is not varying the business question.
#[path = "support/p3a_hand.rs"]
mod h;
#[path = "support/p3a_inputs.rs"]
mod i;
#[path = "support/response_loss_inputs.rs"]
mod q1;
use relca_core::{
    lower,
    sensitivity::{self, contract::Contract, input::Mode, inputs::EffectiveInputs},
    syntax,
};
use serde_json::{Value, json};
fn model(s: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(s).unwrap()).unwrap()
}
fn contract(s: &str) -> Contract {
    Contract::from_model(&model(s)).expect("Q1/responsibility must be projected, not refused")
}
fn run(points: &[(&str, String)]) -> Value {
    json!(sensitivity::run::run(&i::suite(points), Mode::Exhaustive))
}

#[test]
fn r4_loss_probability_and_basis_are_assumptions_not_retry_policy() {
    let base = contract(&i::point("0.1", 1));
    for p in ["0", "0.1", "0.2", "1"] {
        let s = i::point(p, 1);
        for s in [
            s.clone(),
            i::edit(&s, i::BASIS, "different assumed loss basis"),
        ] {
            let c = contract(&s);
            assert!(base.differences(&c).is_empty(), "p={p}");
            assert!(c.differences(&base).is_empty());
        }
    }
}
#[test]
fn r4_effective_none_and_nondeterminism_are_not_missing_uniform_probabilities() {
    for (s, loss) in [
        (q1::service("g", q1::NONE), json!({"kind":"none"})),
        (q1::source("g-nondet"), json!({"kind":"nondeterministic"})),
    ] {
        let v = json!(EffectiveInputs::from_model(&model(&s)).unwrap());
        assert_eq!(
            v["response_channels"],
            json!([{"command":"ack","capability":"Ledger.post","loss":loss}])
        );
        assert_eq!(
            json!(serde_json::from_value::<EffectiveInputs>(v.clone()).unwrap()),
            v
        );
    }
}
#[test]
fn r4_channel_kind_changes_are_not_probability_sensitivity() {
    let s = i::point("0.1", 1);
    let base = contract(&s);
    let nondet = i::edit(
        &s,
        &format!("response_loss probability 0.1 basis \"{}\"", i::BASIS),
        "response_loss nondet",
    );
    assert!(!base.differences(&contract(&nondet)).is_empty());
}
#[test]
fn r4_retry_policy_threshold_deadline_and_cut_are_contracts() {
    let s = i::point("0.1", 1);
    let base = contract(&s);
    for (a, z) in [
        ("max_attempts 2", "max_attempts 3"),
        ("backoff fixed 0min", "backoff fixed 3min"),
        ("when receipt unknown", "when result [rejected_duplicate]"),
        ("exhausted end failed", "exhausted end posted"),
        (">= 0.99", ">= 0.98"),
        ("within 2min", "within 3min"),
        (
            "cut C { include [Ledger] }",
            "cut C { question \"changed question\"; include [Ledger] }",
        ),
    ] {
        let changed = i::edit(&s, a, z);
        let other = contract(&changed);
        assert!(!base.differences(&other).is_empty(), "must reject {a}");
        let v = run(&[("base", s.clone()), ("changed", changed)]);
        h::verdict(&v, "invalid");
        assert_eq!(v["points"][1]["reason"], "sensitivity_contract_mismatch");
        assert!(
            h::rows(&v["points"])
                .iter()
                .all(|p| p["status"] != "evaluated")
        );
        assert_eq!(v["diagnostics"][0]["code"], "E-CMP-0");
    }
}
#[test]
fn r4_enumerated_loss_points_have_exact_unresolved_and_resolution_masses() {
    let v = run(&[
        ("zero", i::point("0", 1)),
        ("tenth", i::point("0.1", 1)),
        ("fifth", i::point("0.2", 1)),
    ]);
    h::verdict(&v, "violated");
    assert_eq!(h::rows(&v["points"]).len(), 3);
    for (p, known, unknown) in [
        (&v["points"][0], "1", "0"),
        (&v["points"][1], "99/100", "1/100"),
        (&v["points"][2], "24/25", "1/25"),
    ] {
        assert_eq!(p["status"], "evaluated");
        let r = &p["report"];
        h::probability(
            &h::row(&r["properties"], "id", "probability reached #1")["probability"],
            known,
        );
        let ts = &r["case_outcomes"][0]["terminals"];
        h::probability(&h::row(ts, "end", "done")["probability"], known);
        if unknown != "0" {
            let u = h::row(ts, "kind", "receipt_unknown");
            h::observed(u, 2, 2);
            h::probability(&u["probability"], unknown);
            assert_eq!(
                h::row(&r["properties"], "id", "receipt_resolution")["verdict"],
                "inconclusive"
            );
        } else {
            assert!(!h::rows(ts).iter().any(|t| t["kind"] == "receipt_unknown"));
        }
    }
    assert_eq!(v["scope"]["continuous_region_checked"], false);
    assert_eq!(v["scope"]["robust_guarantee"], false);
    assert_eq!(v["summaries"][0]["kind"], "finite_point_extrema");
    assert_eq!(v["summaries"][0]["value"], json!({"min":"24/25","max":"1"}));
}
#[test]
fn r4_effective_channels_keep_exact_p_basis_and_independence_with_closed_keys() {
    for (p, ratio) in [("0", "0"), ("0.1", "1/10"), ("0.2", "1/5")] {
        let v = json!(EffectiveInputs::from_model(&model(&i::point(p, 1))).unwrap());
        assert_eq!(
            v["response_channels"],
            json!([{"command":"ack","capability":"Ledger.post","loss":{"kind":"probability","probability":ratio,"basis":i::BASIS,"independence":"per_attempt_conditional_on_normal_finish"}}])
        );
        assert_eq!(
            json!(serde_json::from_value::<EffectiveInputs>(v.clone()).unwrap()),
            v
        );
        for field in ["probability", "basis", "independence"] {
            let mut bad = v.clone();
            bad["response_channels"][0]["loss"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(
                serde_json::from_value::<EffectiveInputs>(bad).is_err(),
                "missing {field}"
            );
        }
        let mut bad = v;
        bad["response_channels"][0]["loss"]["invented"] = json!(true);
        assert!(serde_json::from_value::<EffectiveInputs>(bad).is_err());
    }
}
#[test]
fn r4_duration_one_two_three_are_three_explicit_points_not_an_interval() {
    let v = run(&[
        ("one", i::DURATION.into()),
        ("two", i::edit(i::DURATION, "fixed 1min", "fixed 2min")),
        ("three", i::edit(i::DURATION, "fixed 1min", "fixed 3min")),
    ]);
    h::verdict(&v, "violated");
    for (point, prob) in h::rows(&v["points"]).iter().zip(["1", "1", "0"]) {
        assert_eq!(point["status"], "evaluated");
        h::probability(
            &h::row(
                &point["report"]["properties"],
                "id",
                "probability reached #1",
            )["probability"],
            prob,
        );
    }
    assert_eq!(v["scope"]["continuous_region_checked"], false);
    assert_eq!(v["scope"]["robust_guarantee"], false);
    assert_eq!(v["summaries"][0]["kind"], "finite_point_extrema");
    assert_eq!(v["summaries"][0]["value"], json!({"min":"0","max":"1"}));
    let bad = relca_core::report::check(&i::edit(
        i::DURATION,
        "duration fixed 1min",
        "duration interval 1min..3min",
    ));
    assert_eq!(bad.verdict, relca_core::Verdict::Unsupported);
    assert!(bad.diagnostics.iter().any(|d| d.code == "U-SYN-0"));
}
#[test]
fn r4_responsibility_and_static_authority_projection_keeps_duties_fixed() {
    let s = i::responsibility();
    let base = contract(&s);
    let duration = i::edit(&s, "fixed 1min", "fixed 3min");
    assert!(base.differences(&contract(&duration)).is_empty());
    for (a, z) in [
        ("responsible p for A.run", "responsible q for A.run"),
        ("grants p to A.run", "grants q to A.run"),
        ("assumed grant", "different grant"),
    ] {
        assert!(
            !base.differences(&contract(&i::edit(&s, a, z))).is_empty(),
            "{a}"
        );
    }
    let v = run(&[("one", s), ("three", duration)]);
    h::verdict(&v, "violated");
    assert!(
        h::rows(&v["points"])
            .iter()
            .all(|p| p["status"] == "evaluated")
    );
    assert!(
        h::rows(&v["points"][0]["report"]["responsibilities"])
            .iter()
            .any(|r| r["person"] == "p")
    );
}
