//! Hand-assembled positive and single-fault wires, no production suppression.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};

fn x(n: &str) -> Value {
    json!({"min":n,"max":n,"reason":null})
}
fn n() -> Value {
    json!({"min":null,"max":null,"reason":"not_configured"})
}
fn q() -> Value {
    json!({"lower":null,"upper":null,"basis":null,"reason":"not_configured"})
}
fn wire() -> Value {
    json!({"mode":"exhaustive","bounds":{"cases":1},
      "actors":[
        {"name":"A","capability":{"value":["x"]},"executors":{"value":{"ids":["p"]}}},
        {"name":"R","capability":{"value":["y"]},"executors":{"value":{"ids":["q"]}}}],
      "agency":{"lineage":[{"capability":"A.x"},{"capability":"R.y"}]},
      "cut":{"defer":[{"target":"R","actor":true,"reason":"later","loses":"load"}],"checked":["actor_membership","deferred_metrics"],"unchecked":[]},
      "metrics":{
        "rates":{"actors":[{"actor":"A","per_minute":10,"reason":null,"required":true},{"actor":"R","per_minute":20,"reason":null,"required":true}],
          "capabilities":[{"capability":"A.x","per_start":1,"reason":null,"required":true},{"capability":"R.y","per_start":2,"reason":null,"required":true}]},
        "actors":[{"actor":"A","work":n(),"peak_tasks":n(),"utilization":n(),"occupancy_cost":n()},
          {"actor":"R","work":n(),"peak_tasks":n(),"utilization":n(),"occupancy_cost":n()}],
        "people":[{"person":"p","work":n(),"peak_tasks":n()},{"person":"q","work":n(),"peak_tasks":n()}],
        "capabilities":[{"capability":"A.x","starts":x("1"),"start_cost":n()},{"capability":"R.y","starts":x("1"),"start_cost":n()}],
        "cases":[{"case_id":1,"arrival":0,"counts":{"completed":{"stats":x("1")}},"first_start_wait":n(),"work":n(),"observed_cost_total":n(),
          "tasks":[{"binding":"a","capability":"A.x","work":n(),"wait":n(),"starts":x("1")},{"binding":"r","capability":"R.y","work":n(),"wait":n(),"starts":x("1")}],
          "terminals":[{"kind":"end","end":"done","lead_time":n(),"cost_per_case":n(),"count":{"stats":x("1")}}]}],
        "terminals":[{"end":"done","cost_per_case":n()}],
        "overall":{"counts":{"offered":{"stats":x("1")}},"observed_until":n(),"work":n(),"named_people_work":n(),"peak_tasks":n(),"occupancy_cost":n(),"start_cost":n(),"observed_cost_total":n(),"observed_cost_per_arrival":n(),"business_value":{"value":null,"reason":"not_configured"}}},
      "quality":{"properties":[{"name":"Done","cases":[{"case_id":1,"arrival":0,"min":null,"max":null,"reason":"not_configured"}],"expected_count":q()}],
        "cohort":[{"name":"Rate","value":q()}],"quantiles":[{"name":"P95","value":q(),"lower_bound":null}],"outcomes_per_arrival":[{"kind":"end","end":"done","expected_count":q(),"rate":q()}]}
    })
}
#[test]
fn deferred_wire_positive_and_three_required_single_faults() {
    let good = wire();
    fixture_match::validate_deferred_metrics_wire(&good).unwrap();
    for (pointer, value) in [
        ("/metrics/actors/1/work/reason", json!("not_configureX")),
        ("/metrics/actors/0/work", x("3")),
        ("/metrics/actors/1/work", x("2")),
        ("/quality/cohort/0/value/reason", json!("not_configureX")),
        (
            "/quality/cohort/0/value",
            json!({"lower":"1","upper":"1","basis":"exact","reason":null}),
        ),
        ("/metrics/capabilities/0/starts", n()),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            fixture_match::validate_deferred_metrics_wire(&bad).is_err(),
            "must reject {pointer}"
        );
    }
}
#[test]
fn deferred_wire_missing_unit_price_stays_missing_not_free() {
    let mut v = wire();
    v["metrics"]["rates"]["actors"][0]["per_minute"] = Value::Null;
    v["metrics"]["rates"]["actors"][0]["reason"] = json!("not_configured");
    v["metrics"]["actors"][0]["occupancy_cost"] = n();
    fixture_match::validate_deferred_metrics_wire(&v).unwrap();
    v["metrics"]["actors"][0]["occupancy_cost"] = x("0");
    assert!(fixture_match::validate_deferred_metrics_wire(&v).is_err());
    v["metrics"]["rates"]["actors"][0]["per_minute"] = json!(0);
    v["metrics"]["rates"]["actors"][0]["reason"] = Value::Null;
    v["metrics"]["actors"][0]["occupancy_cost"] = n();
    fixture_match::validate_deferred_metrics_wire(&v).unwrap();
}

#[test]
fn deferred_wire_terminal_fees_separate_contest_only_and_normal_calls() {
    let good = json!(relca_core::report::check(include_str!(
        "../../../fixtures/ctx-contest-metrics.broken-missing-route-rate/model.relca"
    )));
    assert_eq!(good["cut"]["defer"], json!([]));
    assert_eq!(
        good["metrics"]["cases"][0]["terminals"][0]["cost_per_case"],
        x("1")
    );
    assert_eq!(good["metrics"]["terminals"][0]["cost_per_case"], x("1"));
    assert_eq!(good["metrics"]["overall"]["observed_cost_total"], n());
    fixture_match::validate_deferred_metrics_wire(&good).unwrap();

    // A missing fee on a normal call still withdraws the terminal cost.
    // This is a hand-edited wire, not a newly captured model or price estimate.
    let mut missing = good;
    let rate = missing["metrics"]["rates"]["actors"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["actor"] == "Judge")
        .unwrap();
    rate["per_minute"] = Value::Null;
    rate["reason"] = json!("not_configured");
    let actor = missing["metrics"]["actors"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["actor"] == "Judge")
        .unwrap();
    actor["occupancy_cost"] = n();
    missing["metrics"]["cases"][0]["terminals"][0]["cost_per_case"] = n();
    missing["metrics"]["terminals"][0]["cost_per_case"] = n();
    fixture_match::validate_deferred_metrics_wire(&missing).unwrap();
    for pointer in [
        "/metrics/cases/0/terminals/0/cost_per_case",
        "/metrics/terminals/0/cost_per_case",
    ] {
        let mut free = missing.clone();
        *free.pointer_mut(pointer).unwrap() = x("0");
        assert!(
            fixture_match::validate_deferred_metrics_wire(&free).is_err(),
            "{pointer}"
        );
    }
}
#[test]
fn deferred_wire_quality_only_and_no_indicator_checked_boundary() {
    let mut v = wire();
    v.as_object_mut().unwrap().remove("metrics");
    fixture_match::validate_deferred_metrics_wire(&v).unwrap();
    v.as_object_mut().unwrap().remove("quality");
    assert!(fixture_match::validate_deferred_metrics_wire(&v).is_err());
    v["cut"]["checked"] = json!(["actor_membership"]);
    v["cut"]["unchecked"] = json!(["deferred_metrics"]);
    fixture_match::validate_deferred_metrics_wire(&v).unwrap();
}
#[test]
fn deferred_wire_prior_aborted_and_no_terminal_reasons_are_not_overwritten() {
    let mut v = wire();
    v["metrics"]["actors"][1]["work"]["reason"] = json!("aborted");
    v["metrics"]["cases"][0]["terminals"][0]["lead_time"]["reason"] = json!("no_terminal");
    fixture_match::validate_deferred_metrics_wire(&v).unwrap();
    v["metrics"]["actors"][1]["work"]["min"] = json!("2");
    assert!(
        fixture_match::validate_deferred_metrics_wire(&v).is_err(),
        "prior refusal cannot carry numbers"
    );
}
#[test]
fn deferred_wire_sample_statistics_and_structural_trials_are_distinct() {
    fn samples(v: &mut Value) {
        if let Some(o) = v.as_object_mut() {
            if o.contains_key("min") && o.contains_key("max") && o.contains_key("reason") {
                let numeric = o["reason"].is_null();
                o.insert("mean".into(), o["min"].clone());
                o.insert("samples".into(), json!(if numeric { 3 } else { 0 }));
                return;
            }
            for x in o.values_mut() {
                samples(x);
            }
        } else if let Some(a) = v.as_array_mut() {
            for x in a {
                samples(x);
            }
        }
    }
    let mut v = wire();
    v["mode"] = json!("simulation");
    samples(&mut v["metrics"]);
    let missing = json!({"mean":null,"min":null,"max":null,"samples":0,"reason":"not_configured"});
    let zero = json!({"mean":"0","min":"0","max":"0","samples":3,"reason":null});
    v["quality"] = json!({"rules":"quality-cohort-p1-1","analysis":"simulation",
      "properties":[{"name":"Done","condition":"end.done","within":null,"cases":[{"case_id":1,"arrival":0,"hits":3,"unknown":0,"trials":3}],"expected_count":missing,"unknown_count":zero}],
      "cohort":[{"name":"Rate","form":"ratio_of_expected_counts","numerator":"Done","denominator":{"kind":"arrivals","count":1},"value":{"point":null,"numerator":missing,"denominator":missing,"samples":3,"reason":"not_configured"},"bounds_with_undetermined":null}],
      "quantiles":[{"name":"P95","p":"19/20","property":"Done","form":"all_arrivals","value":{"point":null,"observations":3,"censored":0,"reason":"not_configured"},"lower_bound":null}],
      "outcomes_per_arrival":[{"kind":"end","end":"done","expected_count":missing,"rate":{"point":null,"samples":3,"reason":"not_configured"}}],
      "checked":["case_reachability","expected_counts","ratio_of_expected_counts","denominator_disclosure","outcomes_per_arrival","quantiles"],"unchecked":[]});
    fixture_match::validate_deferred_metrics_wire(&v).unwrap();
    fixture_match::validate_quality_wire(&v["quality"], true).unwrap();
    let mut bad = v.clone();
    bad["quality"]["properties"][0]["expected_count"]["samples"] = json!(3);
    assert!(fixture_match::validate_deferred_metrics_wire(&bad).is_err());
    let mut bad = v;
    bad["quality"]["properties"][0]["cases"][0]["reason"] = json!("not_configured");
    assert!(
        fixture_match::validate_quality_wire(&bad["quality"], true).is_err(),
        "no new simulation case wire"
    );
}

#[test]
fn deferred_wire_prior_quality_reasons_never_exempt_endpoints_or_children() {
    for reason in [
        "truncated",
        "undetermined",
        "zero_denominator",
        "denominator_may_be_zero",
    ] {
        let mut good = wire();
        good["quality"]["cohort"][0]["value"] = json!({"point":null,"numerator":{"mean":null,"min":null,"max":null,"samples":0,"reason":"not_configured"},"denominator":{"mean":null,"min":null,"max":null,"samples":0,"reason":"not_configured"},"samples":3,"reason":reason});
        good["quality"]["cohort"][0]["bounds_with_undetermined"] = Value::Null;
        good["quality"]["properties"][0]["expected_count"]["reason"] = json!(reason);
        good["quality"]["properties"][0]["cases"][0]["reason"] = json!(reason);
        good["quality"]["quantiles"][0]["value"]["reason"] = json!("truncated");
        fixture_match::validate_deferred_metrics_wire(&good).unwrap();
        for (pointer, value) in [
            ("/quality/cohort/0/value/numerator/mean", json!("1")),
            ("/quality/cohort/0/value/numerator/min", json!("1")),
            ("/quality/cohort/0/value/numerator/max", json!("1")),
            ("/quality/cohort/0/value/denominator/mean", json!("0")),
            ("/quality/cohort/0/value/denominator/min", json!("0")),
            ("/quality/cohort/0/value/denominator/max", json!("0")),
            ("/quality/cohort/0/value/numerator/samples", json!(3)),
            ("/quality/cohort/0/value/denominator/samples", json!(3)),
            (
                "/quality/cohort/0/bounds_with_undetermined",
                json!({"lower":"0","upper":"1"}),
            ),
            ("/quality/properties/0/expected_count/lower", json!("0")),
            ("/quality/properties/0/expected_count/upper", json!("1")),
            ("/quality/properties/0/cases/0/min", json!("0")),
            ("/quality/properties/0/cases/0/max", json!("1")),
            ("/quality/quantiles/0/lower_bound", json!("1")),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(
                fixture_match::validate_deferred_metrics_wire(&bad).is_err(),
                "{reason}: {pointer}"
            );
        }
    }
}

#[test]
fn deferred_wire_prior_reason_and_retained_trials_cannot_restore_hard() {
    for reason in [
        "truncated",
        "undetermined",
        "zero_denominator",
        "denominator_may_be_zero",
    ] {
        let report = wire();
        let cohort = json!({"value":{"reason":reason}});
        let good = json!({"verdict":"inconclusive","interval":null,"basis":null,"point":null,"reason":reason});
        fixture_match::validate_deferred_hard_cell(&report, &cohort, &good).unwrap();
        for (field, value) in [
            ("interval", json!({"lower":"0","upper":"1"})),
            ("basis", json!("statistical")),
            ("point", json!("0")),
            ("verdict", json!("satisfied")),
        ] {
            let mut bad = good.clone();
            bad[field] = value;
            assert!(fixture_match::validate_deferred_hard_cell(&report, &cohort, &bad).is_err());
        }
    }
}
