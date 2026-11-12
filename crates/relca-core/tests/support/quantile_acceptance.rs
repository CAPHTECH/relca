//! Hand-only quantile oracle transport. No production arithmetic in helpers.
#![allow(dead_code)]
use relca_core::report;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
#[path = "../../../relca-cli/tests/support/ledger_shape.rs"]
mod e9b_wire;

pub const QUANTILE_AFFECTS: &str = "到着から性質への初到達時間の左の最小整数値で補間しない。全流入は不達を含み、given reachedは到達した質量だけを分母にする。網羅は案件ごとのxごとのmin/maxからの区間で同時の分布ではない。simulationは反復と案件をまとめた点推定であり、案件を独立標本とせず、区間推定でも全スケジュールの検証でもない。打切りで識別できない値は理由つきnullと下界に分ける。";

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn names() -> Vec<String> {
    serde_json::from_str(include_str!("quantile_names.json")).unwrap()
}
pub fn inventory() -> Value {
    serde_json::from_str(include_str!("quantile_inventory.json")).unwrap()
}
pub fn source(name: &str) -> String {
    fs::read_to_string(root().join("fixtures").join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_slice(
        &fs::read(root().join("fixtures").join(name).join("expected.json")).unwrap(),
    )
    .unwrap()
}
pub fn replace(s: &str, old: &str, new: &str) -> String {
    assert!(s.contains(old), "missing edit witness {old}");
    let out = s.replacen(old, new, 1);
    assert_ne!(out, s, "no-op control");
    out
}
pub fn no_quantiles(s: &str) -> String {
    let out: String = s
        .split_inclusive('\n')
        .map(|line| {
            if line.trim_start().starts_with("metric quantile.") {
                line.bytes()
                    .map(|b| if b == b'\n' { '\n' } else { ' ' })
                    .collect::<String>()
            } else {
                line.to_owned()
            }
        })
        .collect();
    assert_ne!(out, s, "must actually mask a quantile declaration");
    assert_eq!(out.len(), s.len(), "keep old diagnostic byte spans");
    out
}
pub fn run(name: &str, s: &str) -> Value {
    let inv = &inventory()[name]["invocation"];
    if inv["command"] == "simulate" {
        serde_json::to_value(report::simulate_with_selection(
            s,
            inv["configuration"].as_str(),
            inv["scenario"].as_str(),
        ))
        .unwrap()
    } else {
        serde_json::to_value(report::check_with_selection(
            s,
            inv["configuration"].as_str(),
            inv["scenario"].as_str(),
        ))
        .unwrap()
    }
}
pub fn runtime(name: &str) -> bool {
    expected(name).get("quality").is_some()
}
pub fn hand(name: &str, actual: &Value) {
    let expected = expected(name);
    assert_eq!(
        actual["verdict"], expected["verdict"],
        "{name}: quantile must not set verdict"
    );
    if let Some(q) = expected.get("quality") {
        assert_eq!(
            actual["quality"]["quantiles"], q["quantiles"],
            "{name}: hand CDF inverse, complete ordered quantile rows"
        );
        let rows = actual["quality"]["quantiles"]
            .as_array()
            .expect("declared nonempty quantiles");
        assert!(!rows.is_empty());
        let aborted = rows.iter().all(|r| r["value"]["reason"] == "aborted");
        let inventory = if aborted { "unchecked" } else { "checked" };
        assert!(
            actual["quality"][inventory]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "quantiles")
        );
        let other = if aborted { "checked" } else { "unchecked" };
        assert!(
            !actual["quality"][other]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "quantiles")
        );
        let plain = run(name, &no_quantiles(&source(name)));
        assert_eq!(
            actual["diagnostics"], plain["diagnostics"],
            "{name}: old code,message,byte span and diagnostic order"
        );
        assumptions(actual);
    } else {
        assert!(
            actual.get("quality").is_none(),
            "{name}: static failure has no partial quality"
        );
        let errors: Vec<_> = actual["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(errors.len(), 1, "{name}: exactly one static fault");
        let inv = inventory();
        let row = &inv[name];
        assert_eq!(errors[0]["code"], row["code"], "{name}: error class");
        if row["message"].is_string() {
            assert_eq!(
                errors[0]["message"], row["message"],
                "{name}: exact message"
            );
        }
    }
}
pub fn assumptions(v: &Value) {
    let rows = v["assumptions"].as_array().unwrap();
    let pos: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r["field"] == "quality.quantile")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(pos.len(), 1, "one explicit quantile assumption");
    let i = pos[0];
    assert_eq!(rows[i]["value"], "left-inverse-first-hit-no-interpolation");
    assert_eq!(
        rows[i]["affects"], QUANTILE_AFFECTS,
        "fixed guarantee/scope disclosure,not just a nonempty label"
    );
    for (j, row) in rows.iter().enumerate() {
        match row["field"].as_str().unwrap() {
            "probability.scheduler" => assert!(j > i),
            _ => assert!(
                j <= i,
                "quantile follows every old assumption, including quality.scheduler/estimation"
            ),
        }
    }
}
/// Only explicit new output positions are removed. Old counters, diagnostics,
/// metrics, quality properties/cohort/outcomes and draw audits are untouched.
pub fn without_uncertainty(mut v: Value) -> Value {
    // Validate the live E9b block before projecting metadata that did not exist
    // in this older feature's numeric contract. Never copy it into hand values.
    e9b_wire::uncertainty_wire::validate_host(&v).unwrap();
    v.as_object_mut().unwrap().remove("uncertainty");
    v
}
pub fn old_projection(v: Value) -> Value {
    let mut v = without_uncertainty(v);
    v["model"].as_object_mut().unwrap().remove("sha256");
    if let Some(q) = v.get_mut("quality") {
        q.as_object_mut().unwrap().remove("quantiles");
        for k in ["checked", "unchecked"] {
            q[k].as_array_mut().unwrap().retain(|x| x != "quantiles");
        }
    }
    v["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["field"] != "quality.quantile");
    v
}

pub fn hand_quality(sampled: bool) -> Value {
    // Reuse a pre-existing independent HAND scaffold, never runtime output.
    let n = if sampled { "qual-sim" } else { "qual-base" };
    let mut q = expected(n)["quality"].clone();
    q["checked"]
        .as_array_mut()
        .unwrap()
        .push(json!("quantiles"));
    q["quantiles"] = json!([{"name":"Lead","p":"19/20","property":"Registered","form":"reached_only",
        "value":if sampled {json!({"point":"4","observations":8,"censored":0,"reason":null})}
                else {json!({"lower":"4","upper":"4","basis":"exact","reason":null})},
        "lower_bound":null}]);
    q
}
