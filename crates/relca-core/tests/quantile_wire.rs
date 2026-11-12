//! Production deserialization, independent of the CLI comparator's schema.
#[path = "support/quantile_acceptance.rs"]
mod h;
use relca_core::quality::Quality;
use serde_json::{Value, json};

fn dec(v: &Value) -> Result<Quality, serde_json::Error> {
    serde_json::from_value(v.clone())
}

fn t07_hand_wire() -> Value {
    let mut q = h::hand_quality(true);
    // The fixture expected file is intentionally partial, not a full wire.
    // Embed its two hand quantile values in the complete legacy hand scaffold.
    q["quantiles"] =
        h::expected("qtl-sample-sim.broken-short-clock")["quality"]["quantiles"].clone();
    for row in q["quantiles"].as_array_mut().unwrap() {
        row["property"] = json!("Registered");
    }
    q
}

#[test]
fn r2_all_censored_cannot_justify_a_finite_conditional_point() {
    let q = h::hand_quality(true);
    dec(&q).expect("R2 unchanged N8 C0 point4 positive control");
    let t07 = t07_hand_wire();
    dec(&t07).expect("T07 point2 at p1/2 remains identified despite N4 C2");
    let mut bad = q.clone();
    bad["quantiles"][0]["value"]["censored"] = json!(8);
    assert_ne!(bad, q);
    assert!(
        dec(&bad).is_err(),
        "C=N=8 leaves H=0: given denominator may be zero"
    );
}

#[test]
fn r2_zero_denominator_cannot_be_established_with_remaining_censors() {
    let mut q = h::hand_quality(true);
    q["quantiles"][0]["value"] =
        json!({"point":null,"observations":8,"censored":0,"reason":"zero_denominator"});
    dec(&q).expect("known eight misses establish a zero denominator");
    let t07 = t07_hand_wire();
    dec(&t07).expect("T07 finite identified point with censor remains valid");
    let mut bad = q.clone();
    bad["quantiles"][0]["value"]["censored"] = json!(1);
    assert_ne!(bad, q);
    assert!(
        dec(&bad).is_err(),
        "one remaining censor might reach: zero denominator is not known"
    );
}

#[test]
fn quantile_missing_mass_reason_does_not_widen_the_legacy_cohort_reason_enum() {
    for name in ["qual-base", "qual-sim"] {
        let mut q = h::expected(name)["quality"].clone();
        dec(&q).expect("legacy hand wire remains a positive control");
        let value = &mut q["cohort"][0]["value"];
        if name == "qual-sim" {
            value["point"] = Value::Null;
        } else {
            for key in ["lower", "upper", "basis"] {
                value[key] = Value::Null;
            }
        }
        value["reason"] = json!("truncated");
        dec(&q).expect("coherent null cohort is valid with an old allowed reason");
        q["cohort"][0]["value"]["reason"] = json!("beyond_reached_mass");
        assert!(
            dec(&q).is_err(),
            "only the quantile-specific reason is invalid"
        );
    }
}

#[test]
fn old_quality_wire_stays_valid_but_quantile_rows_are_strict_and_mode_closed() {
    for sampled in [false, true] {
        let q = h::hand_quality(sampled);
        let decoded = dec(&q).expect("new hand quantile wire must deserialize");
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            q,
            "round-trip retains every quantile field"
        );
        let mut old = q.clone();
        old.as_object_mut().unwrap().remove("quantiles");
        old["checked"]
            .as_array_mut()
            .unwrap()
            .retain(|v| v != "quantiles");
        assert!(
            dec(&old).is_ok(),
            "legacy missing field is accepted without adding null/[]"
        );
        for path in ["/quantiles/0", "/quantiles/0/value"] {
            for key in q.pointer(path).unwrap().as_object().unwrap().keys() {
                let mut bad = q.clone();
                bad.pointer_mut(path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(dec(&bad).is_err(), "missing {path}/{key}");
            }
            let mut bad = q.clone();
            bad.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("extra".into(), json!(0));
            assert!(dec(&bad).is_err(), "unknown field");
        }
        for (p, value) in [
            ("/quantiles", json!([])),
            ("/quantiles", Value::Null),
            ("/quantiles/0/p", json!(0.5)),
            ("/quantiles/0/p", json!("0")),
            ("/quantiles/0/p", json!("3/2")),
            ("/quantiles/0/p", json!("2/4")),
            ("/quantiles/0/property", json!("Absent")),
            ("/quantiles/0/form", json!("all_completed")),
            ("/quantiles/0/name", json!("")),
            ("/quantiles/0/lower_bound", json!("0")),
        ] {
            let mut bad = q.clone();
            *bad.pointer_mut(p).unwrap() = value;
            assert!(dec(&bad).is_err(), "{p}: {bad}");
        }
        let raw = serde_json::to_string(&q).unwrap();
        let changed = raw.replacen("\"p\":\"19/20\"", "\"p\":\"19/20\",\"p\":\"19/20\"", 1);
        assert_ne!(raw, changed);
        assert!(
            serde_json::from_str::<Quality>(&changed).is_err(),
            "duplicate nested key cannot be erased by Value"
        );
        let changed = raw.replacen("\"reason\":null", "\"reason\":null,\"reason\":null", 1);
        assert_ne!(raw, changed);
        assert!(serde_json::from_str::<Quality>(&changed).is_err());
    }
}

#[test]
fn quantified_missing_values_have_reason_specific_lower_bounds_not_fake_numbers() {
    let scaffold = h::hand_quality(false);
    for (reason, lb) in [
        ("beyond_reached_mass", Some("4")),
        ("truncated", Some("0")),
        ("undetermined", Some("4")),
        ("zero_denominator", None),
        ("denominator_may_be_zero", None),
        ("aborted", None),
    ] {
        let mut q = scaffold.clone();
        q["quantiles"][0]["value"] =
            json!({"lower":null,"upper":null,"basis":null,"reason":reason});
        if reason == "beyond_reached_mass" {
            q["quantiles"][0]["form"] = json!("all_arrivals");
        }
        q["quantiles"][0]["lower_bound"] = json!(lb);
        if reason == "aborted" {
            q["checked"]
                .as_array_mut()
                .unwrap()
                .retain(|v| v != "quantiles");
            q["unchecked"]
                .as_array_mut()
                .unwrap()
                .push(json!("quantiles"));
        }
        dec(&q).unwrap_or_else(|e| panic!("legitimate reason-bearing null {reason}: {e}"));
        let mut bad = q.clone();
        bad["quantiles"][0]["value"]["lower"] = json!("4");
        assert!(dec(&bad).is_err(), "partial finite/null value");
        for time in [
            json!("-1"),
            json!("04"),
            json!("1/2"),
            json!("4.0"),
            json!(4),
            json!("NaN"),
            json!("Infinity"),
        ] {
            let mut bad = q.clone();
            bad["quantiles"][0]["lower_bound"] = time;
            assert!(dec(&bad).is_err(), "invalid lower-bound time");
        }
        if lb.is_none() {
            let mut bad = q.clone();
            bad["quantiles"][0]["lower_bound"] = json!("0");
            assert!(dec(&bad).is_err());
        }
    }
}

#[test]
fn f34c_missing_reason_is_a_closed_set_with_and_without_lower_bound() {
    for sampled in [false, true] {
        for lb in [None, Some("4")] {
            let mut q = h::hand_quality(sampled);
            q["quantiles"][0]["value"] = if sampled {
                json!({"point":null,"observations":8,"censored":2,"reason":"truncated"})
            } else {
                json!({"lower":null,"upper":null,"basis":null,"reason":"truncated"})
            };
            q["quantiles"][0]["lower_bound"] = json!(lb);
            dec(&q).expect("allowed reason positive control");
            q["quantiles"][0]["value"]["reason"] = json!("bogus");
            let err = dec(&q).expect_err("unknown reason must fail closed");
            assert!(
                err.to_string().contains("invalid missing quantile reason"),
                "{err}"
            );
        }
    }
}
