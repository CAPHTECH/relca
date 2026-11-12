//! Review-close hand acceptance; no production collector is an oracle.
#[path = "support/sensitivity_review_inputs.rs"]
mod i;
use relca_core::{engine, lower, reference, report, sensitivity, syntax};
use serde_json::{Value, json};

fn model(source: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(source).expect("hand source parses")).expect("hand source lowers")
}

#[test]
fn a2_hand_joint_and_marginals_in_both_interpreters() {
    for (kind, wrong, joint) in [
        ("independent", "1/5000", "1/100"),
        ("common", "1/500", "1/10"),
        ("ordered", "1/500", "1/10"),
    ] {
        let source = i::a2(kind);
        let m = model(&source);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for out in [&e, &r.outcome] {
            assert!(!out.truncated);
            let values: Vec<_> = out
                .properties
                .iter()
                .map(|p| {
                    let p = p.probability.as_ref().unwrap();
                    (p.min.to_string(), p.max.to_string())
                })
                .collect();
            let hand: Vec<_> = [wrong, joint, "1/10", "1/10"]
                .into_iter()
                .map(|p| (p.to_owned(), p.to_owned()))
                .collect();
            assert_eq!(values, hand, "{kind}");
        }
    }
}

#[test]
fn a2_gate_allows_given_latent_reads_and_latent_reader_actor_changes() {
    let contracts: Vec<_> = ["independent", "common", "ordered"]
        .iter()
        .map(|k| sensitivity::contract::Contract::from_model(&model(&i::a2(k))).unwrap())
        .collect();
    for c in &contracts[1..] {
        assert!(contracts[0].differences(c).is_empty());
    }
    // A separate observation-reader change is NOT required here: reads [Record]
    // stays fixed. The varied Actor latent read is kernel_reads [Cause.bad].
}

#[test]
fn a2_actor_labels_do_not_change_probabilities_or_evidence_sources() {
    let source = i::a2("common");
    let renamed = source
        .replace("Cause", "Weather")
        .replace("grain document", "grain model");
    let a = serde_json::to_value(report::check(&source)).unwrap();
    let b = serde_json::to_value(report::check(&renamed)).unwrap();
    assert_eq!(a["properties"], b["properties"]);
    for v in [&a, &b] {
        for field in ["av", "bv"] {
            let row = v["agency"]["lineage"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["call"] == field)
                .unwrap();
            assert_eq!(row["subject_sources"], json!(["Record"]));
        }
    }
}

#[test]
fn a3_expected_counts_and_denominators_are_not_rates() {
    for (busy, degraded, count, cases, rate) in [
        (false, false, "1/100", 1, "1/100"),
        (true, false, "1/50", 2, "1/100"),
        (false, true, "1/25", 1, "1/25"),
        (true, true, "2/25", 2, "1/25"),
    ] {
        let v = serde_json::to_value(report::check(&i::a3(busy, degraded))).unwrap();
        i::assert_a3(&v, count, cases, rate);
    }
}

fn without_span(mut v: Value, source: &str, fragment: &str) -> Value {
    let span = &v["span"];
    let start = span["start"].as_u64().unwrap() as usize;
    let end = span["end"].as_u64().unwrap() as usize;
    assert!(start < end && end <= source.len());
    assert!(source[start..end].contains(fragment));
    v.as_object_mut().unwrap().remove("span");
    v
}

#[test]
fn m2_aleatory_initial_values_and_kernel_conditions_match_hand_inputs() {
    let source = i::a2("common");
    let v = serde_json::to_value(report::check(&source)).unwrap();
    assert_eq!(v["uncertainty"]["aleatory"]["status"], "available");
    let items = v["uncertainty"]["aleatory"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 4);
    for (variable, yes, no) in [("wrong", "1/50", "49/50"), ("bad", "1/10", "9/10")] {
        let row = items
            .iter()
            .find(|r| r["kind"] == "initial_distribution" && r["variable"] == variable)
            .unwrap();
        assert_eq!(
            without_span(row.clone(), &source, "bool"),
            json!({
                "kind":"initial_distribution","variable":variable,"instance":"per_case",
                "entries":[{"value":"true","probability":yes},{"value":"false","probability":no}]
            })
        );
    }
    for (capability, name) in [("A.check", "KA"), ("B.check", "KB")] {
        let row = items
            .iter()
            .find(|r| r["kind"] == "kernel" && r["capability"] == capability)
            .unwrap();
        assert_eq!(
            without_span(row.clone(), &source, "capability check"),
            json!({
                "kind":"kernel","capability":capability,"name":name,"basis":null,"given":["bad"],
                "rows":[
                    {"conditions":[{"variable":"bad","value":"true"}],"branches":[{"probability":"1","result":"miss","writes":[]}]},
                    {"conditions":[{"variable":"bad","value":"false"}],"branches":[{"probability":"1","result":"detect","writes":[]}]}
                ]
            })
        );
    }
}

#[test]
fn m8_distribution_value_change_is_allowed_but_nondet_is_not() {
    let source = i::a2("independent");
    let changed = source.replace("true: 0.02, false: 0.98", "true: 0.04, false: 0.96");
    let nondet = source.replace("wrong: bool ~ { true: 0.02, false: 0.98 }", "wrong: bool");
    let contract = |s: &str| sensitivity::contract::Contract::from_model(&model(s)).unwrap();
    let a = contract(&source);
    assert!(a.differences(&contract(&changed)).is_empty());
    let v = serde_json::to_value(report::check(&changed)).unwrap();
    assert_eq!(
        i::property(&v, "probability reached #1")["probability"],
        json!({"min":"1/2500","max":"1/2500"})
    );
    let b = contract(&nondet);
    assert!(
        !a.differences(&b).is_empty(),
        "latent distribution to nondet changed the question"
    );
    assert!(!b.differences(&a).is_empty());
}

#[test]
fn m1_distribution_traffic_retains_every_exact_support_row_without_fake_zero() {
    let m = model(&i::traffic());
    let v = serde_json::to_value(sensitivity::inputs::EffectiveInputs::from_model(&m).unwrap())
        .unwrap();
    assert_eq!(
        v["traffic"]["distribution"]["entries"],
        json!([
            {"times":[2,2],"probability":"1/4"}, {"times":[2,5],"probability":"3/4"}
        ])
    );
    assert_eq!(
        v["traffic"]["distribution"]["basis"],
        "hand assumption; not measured"
    );
    assert!(
        v["traffic"].get("arrivals").unwrap().is_null(),
        "distributed arrivals must not claim a fixed schedule"
    );
}

#[test]
fn m1_default_zero_and_fixed_every_keep_their_actual_arrivals() {
    let default = i::a2("independent");
    let every = i::traffic().replace(
        i::ARRIVAL_DECL,
        "arrivals fixed_every 2min during [2min, 6min)",
    );
    for (source, times) in [(&default, json!([0])), (&every, json!([2, 4]))] {
        let m = model(source);
        let v = serde_json::to_value(sensitivity::inputs::EffectiveInputs::from_model(&m).unwrap())
            .unwrap();
        assert_eq!(v["traffic"]["arrivals"], times);
        let report = serde_json::to_value(report::check(source)).unwrap();
        assert_eq!(report["verdict"], "satisfied", "{}", report["diagnostics"]);
        if let Some(outcomes) = report["case_outcomes"].as_array() {
            let actual: Vec<_> = outcomes.iter().map(|c| c["arrival"].clone()).collect();
            assert_eq!(json!(actual), times);
        } else {
            assert_eq!(times, json!([0]));
            assert!(!report["terminals"].as_array().unwrap().is_empty());
        }
    }
}
