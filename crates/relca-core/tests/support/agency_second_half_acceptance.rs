#![allow(dead_code)]
use relca_core::{diff, report};
use serde_json::{Value, json};
use std::path::PathBuf;
#[path = "responsibility_defaults_oracle.rs"]
mod responsibility_oracle;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn population() -> Vec<Value> {
    serde_json::from_str(include_str!("agency_second_half_population.json")).unwrap()
}
pub fn fixture(name: &str) -> bool {
    let names: Vec<String> =
        serde_json::from_str(include_str!("agency_second_half_names.json")).unwrap();
    names.iter().any(|n| n == name)
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join("fixtures").join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(root().join("fixtures").join(name).join("expected.json")).unwrap(),
    )
    .unwrap()
}
pub fn replace(s: &str, from: &str, to: &str) -> String {
    let out = s.replace(from, to);
    assert_ne!(out, s, "counterfactual must change source: {from}");
    out
}
pub fn all_writers_source() -> String {
    let s = source("agy2-reads-observed");
    let s = replace(
        &s,
        "  actor Clerk",
        "  actor Extra { source record }\n  actor Earlier { capacity 1; capability write() -> Entry { reads [Extra]; writes entry; duration fixed 1min; result nondet { entry } } }\n  actor Clerk",
    );
    let s = replace(
        &s,
        "e = Clerk.enter()",
        "old = Earlier.write(); e = Clerk.enter()",
    );
    replace(
        &s,
        "include [Doc, PO, Clerk, Approver, Ledger]",
        "include [Doc, PO, Extra, Earlier, Clerk, Approver, Ledger]",
    )
}
pub fn check(row: &Value) -> Value {
    let s = source(row["name"].as_str().unwrap());
    let i = &row["invocation"];
    match i["command"].as_str() {
        Some("diff") => serde_json::to_value(diff::diff(
            &s,
            i["from"].as_str().unwrap(),
            i["to"].as_str().unwrap(),
        ))
        .unwrap(),
        Some("check") => serde_json::to_value(report::check_configuration(
            &s,
            i["configuration"].as_str().unwrap(),
        ))
        .unwrap(),
        _ => serde_json::to_value(report::check(&s)).unwrap(),
    }
}
/// Expected objects are selected independent assertions; every array is exact,
/// including order and absence of extra effects/judgements/differences.
pub fn subset(want: &Value, actual: &Value, at: &str) {
    if let Some(o) = want.as_object() {
        assert!(actual.is_object(), "{at}: expected object, got {actual}");
        for (k, v) in o {
            subset(
                v,
                actual
                    .get(k)
                    .unwrap_or_else(|| panic!("{at}/{k}: missing (ADR0043 not implemented)")),
                &format!("{at}/{k}"),
            );
        }
    } else if let Some(a) = want.as_array() {
        let b = actual
            .as_array()
            .unwrap_or_else(|| panic!("{at}: expected array, got {actual}"));
        assert_eq!(a.len(), b.len(), "{at}: exact ordered row inventory");
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            subset(a, b, &format!("{at}/{i}"));
        }
    } else {
        assert_eq!(actual, want, "{at}");
    }
}
pub fn assert_report(row: &Value, actual: &Value) {
    let name = row["name"].as_str().unwrap();
    let want = expected(name);
    subset(&want, actual, name);
    if let Some(codes) = row["codes"].as_array() {
        let mut expected: Vec<_> = codes.iter().map(|x| x.as_str().unwrap()).collect();
        if !actual["agency"].is_null() {
            // ADR 0054 "範囲" / "意味" replaces the old people/state-only
            // responsibility_targets table, not the other diagnostic order.
            let ast = relca_core::syntax::parse(&source(name)).unwrap();
            let model = relca_core::lower::lower_with_selection(
                &ast,
                row["invocation"]["configuration"].as_str(),
                None,
            )
            .unwrap();
            responsibility_oracle::assert_report(&model, actual);
            let responsibilities = responsibility_oracle::expected(&model);
            expected.retain(|code| *code != "W-RESP-0");
            let at = expected
                .iter()
                .position(|code| code.starts_with("W-UNREACH-"))
                .unwrap_or(expected.len());
            expected.splice(
                at..at,
                std::iter::repeat_n("W-RESP-0", responsibilities.len()),
            );
        }
        let got: Vec<_> = actual["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["code"].as_str().unwrap())
            .collect();
        assert_eq!(
            got, expected,
            "exact diagnostic order and multiplicities {name}"
        );
    }
    for (pointer, want) in row["probes"].as_object().unwrap() {
        assert_eq!(actual.pointer(pointer), Some(want), "{name}{pointer}");
    }
    if let Some(es) = actual
        .pointer("/agency/viewpoints/effects/rows")
        .and_then(Value::as_array)
    {
        for e in es {
            if e["process_judgements"].is_object() {
                assert!(
                    e.get("j").is_none() && e.get("u").is_none(),
                    "unknown is not empty {name}"
                );
            }
        }
        assert!(
            !actual["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("executors"))
        );
    }
}
