//! Real CLI, fixed hand CDF values and closed JSON, not report-as-oracle.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/quantile_acceptance.rs"]
mod h;
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn all_quantile_fixtures_exit_and_hand_values_collect_every_red() {
    let mut failures = vec![];
    for name in h::names() {
        let inv = h::inventory()[&name]["invocation"].clone();
        let invocation =
            fixture_match::Invocation::parse((!inv.is_null()).then_some(&inv)).unwrap();
        let path = h::root().join("fixtures").join(&name).join("model.relca");
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(invocation.args(&path))
            .output()
            .unwrap();
        let result = std::panic::catch_unwind(|| {
            let v: Value = serde_json::from_slice(&out.stdout).unwrap();
            h::hand(&name, &v);
            let e = h::expected(&name);
            let verdict: relca_core::Verdict =
                serde_json::from_value(e["verdict"].clone()).unwrap();
            assert_eq!(
                out.status.code(),
                Some(verdict.exit_code()),
                "{name}: actual exit"
            );
            invocation.compare(&e, &v).unwrap();
        });
        if result.is_err() {
            failures.push(name);
        }
    }
    assert!(
        failures.is_empty(),
        "all real CLI quantile failures: {failures:?}"
    );
}

#[test]
fn closed_quantile_table_accepts_hand_wire_and_rejects_all_malformed_positions() {
    for sampled in [false, true] {
        let q = h::hand_quality(sampled);
        fixture_match::validate_quality_wire(&q, sampled)
            .expect("valid independent HAND quality with quantile");
        for path in ["/quantiles/0", "/quantiles/0/value"] {
            let mut bad = q.clone();
            bad.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("rank".into(), json!(1));
            assert!(
                fixture_match::validate_quality_wire(&bad, sampled).is_err(),
                "unknown {path}"
            );
            for key in q.pointer(path).unwrap().as_object().unwrap().keys() {
                let mut bad = q.clone();
                bad.pointer_mut(path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(
                    fixture_match::validate_quality_wire(&bad, sampled).is_err(),
                    "missing {path}/{key}"
                );
            }
        }
        for (path, value) in [
            ("/quantiles", json!([])),
            ("/quantiles/0/p", json!(0.95)),
            ("/quantiles/0/p", json!("0")),
            ("/quantiles/0/p", json!("3/2")),
            ("/quantiles/0/p", json!("2/4")),
            ("/quantiles/0/property", json!("Absent")),
            ("/quantiles/0/name", json!("")),
            ("/quantiles/0/form", json!("successful")),
            ("/quantiles/0/lower_bound", json!("2")),
            (
                "/quantiles/0/value/reason",
                json!("nondeterministic_denominator"),
            ),
            ("/quantiles/0/value/reason", json!("truncated")),
        ] {
            let mut bad = q.clone();
            *bad.pointer_mut(path).unwrap() = value;
            assert!(
                fixture_match::validate_quality_wire(&bad, sampled).is_err(),
                "bad {path}: {bad}"
            );
        }
        let mut bad = q.clone();
        let row = bad["quantiles"][0].clone();
        bad["quantiles"].as_array_mut().unwrap().push(row);
        assert!(
            fixture_match::validate_quality_wire(&bad, sampled).is_err(),
            "duplicate name"
        );
        let mut bad = q.clone();
        bad["quantiles"][0]["name"] = bad["cohort"][0]["name"].clone();
        assert!(
            fixture_match::validate_quality_wire(&bad, sampled).is_err(),
            "cross-cohort duplicate"
        );
        let mut bad = q.clone();
        // Isolate the forbidden reason: the null payload is otherwise valid.
        // Keeping finite numbers here would also violate the null/reason rule
        // and let an accidentally widened cohort reason enum escape this test.
        if sampled {
            bad["cohort"][0]["value"]["point"] = Value::Null;
        } else {
            for key in ["lower", "upper", "basis"] {
                bad["cohort"][0]["value"][key] = Value::Null;
            }
        }
        bad["cohort"][0]["value"]["reason"] = json!("beyond_reached_mass");
        assert!(
            fixture_match::validate_quality_wire(&bad, sampled).is_err(),
            "new quantile reason must not leak into cohort"
        );
        for time in [
            json!(2),
            json!("-1"),
            json!("2/1"),
            json!("1/2"),
            json!("02"),
            json!("2.0"),
            json!("Infinity"),
            json!("NaN"),
        ] {
            let mut bad = q.clone();
            bad["quantiles"][0]["value"][if sampled { "point" } else { "lower" }] = time;
            assert!(
                fixture_match::validate_quality_wire(&bad, sampled).is_err(),
                "noninteger/noncanonical time"
            );
        }
        let mut bad = q.clone();
        bad["quantiles"][0]["value"][if sampled { "lower" } else { "point" }] = json!("4");
        assert!(
            fixture_match::validate_quality_wire(&bad, sampled).is_err(),
            "mode-exclusive shape"
        );
        if sampled {
            let mut bad = q.clone();
            bad["quantiles"][0]["value"]["censored"] = json!(9);
            assert!(
                fixture_match::validate_quality_wire(&bad, true).is_err(),
                "C>N"
            );
        } else {
            for (lo, hi, basis) in [("5", "4", "per_case_relaxation"), ("3", "4", "exact")] {
                let mut bad = q.clone();
                bad["quantiles"][0]["value"] =
                    json!({"lower":lo,"upper":hi,"basis":basis,"reason":null});
                assert!(
                    fixture_match::validate_quality_wire(&bad, false).is_err(),
                    "range ordering/exact"
                );
            }
        }
    }
}

#[test]
fn every_hand_partial_expectation_is_valid_before_running_the_cli() {
    for name in h::names() {
        let inv = h::inventory()[&name]["invocation"].clone();
        let i = fixture_match::Invocation::parse((!inv.is_null()).then_some(&inv)).unwrap();
        i.validate_expected(&h::expected(&name))
            .unwrap_or_else(|e| panic!("{name}: authored fixture defect: {e}"));
    }
}

#[test]
fn raw_quality_key_order_is_closed_and_declaration_order_not_lexicographic() {
    let path = h::root().join("fixtures/qtl-label.broken-renamed/model.relca");
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["check", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let raw = String::from_utf8(out.stdout).unwrap();
    let start = raw.find("\"quality\":").expect("quality output");
    let raw = &raw[start..];
    let positions: Vec<_> = [
        "rules",
        "analysis",
        "properties",
        "cohort",
        "quantiles",
        "outcomes_per_arrival",
        "checked",
        "unchecked",
    ]
    .iter()
    .map(|k| raw.find(&format!("\"{k}\"")).unwrap())
    .collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "top-level quality order"
    );
    let q = &raw[positions[4]..positions[5]];
    assert!(
        q.find("\"Z1\"").unwrap() < q.find("\"A0\"").unwrap(),
        "declaration,not dictionary order"
    );
    let positions: Vec<_> = ["name", "p", "property", "form", "value", "lower_bound"]
        .iter()
        .map(|k| q.find(&format!("\"{k}\"")).unwrap())
        .collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "quantile row order precedes cohort form dispatch"
    );
}

#[test]
fn sample_quantile_value_keeps_point_observations_censored_reason_key_order() {
    let path = h::root().join("fixtures/qtl-sample-sim/model.relca");
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["simulate", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let raw = String::from_utf8(out.stdout).unwrap();
    let quality = &raw[raw.find("\"quality\":").expect("declared quality object")..];
    let start = quality
        .find("\"quantiles\"")
        .expect("quantile rows,not metrics unchecked word");
    let rest = &quality[start..];
    let end = rest.find("\"lower_bound\"").unwrap();
    let row = &rest[..end];
    let positions: Vec<_> = ["name", "p", "property", "form", "value"]
        .iter()
        .map(|k| row.find(&format!("\"{k}\"")).unwrap())
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]));
    let value = &row[*positions.last().unwrap()..];
    let positions: Vec<_> = ["point", "observations", "censored", "reason"]
        .iter()
        .map(|k| value.find(&format!("\"{k}\"")).unwrap())
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn closed_quantile_names_exactly_match_disk_and_current_runner() {
    let mut actual: Vec<_> = std::fs::read_dir(h::root().join("fixtures"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("model.relca").is_file())
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_owned())
        .filter(|n| n.starts_with("qtl-"))
        .collect();
    actual.sort();
    assert_eq!(actual, h::names());
    assert_eq!(
        fixture_match::QUANTILE_FIXTURES,
        actual.iter().map(String::as_str).collect::<Vec<_>>()
    );
    assert_eq!(actual.len(), 67);
    assert!(!fixture_match::QUANTILE_FIXTURES.contains(&"qtl-unregistered"));
    let runner = include_str!("fixtures.rs");
    assert!(
        runner.contains("fixture_match::QUANTILE_FIXTURES.contains(&name.as_str())")
            && runner.contains("fixture_match::QUANTILE_FIXTURES.contains(&name.as_ref())"),
        "the current raw runner classifies and executes the complete quantile table"
    );
}
