#![allow(dead_code)]
use serde_json::Value;
use std::path::PathBuf;
#[path = "responsibility_defaults_oracle.rs"]
pub(crate) mod responsibility_oracle;
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn population() -> Value {
    serde_json::from_str(include_str!("agency_viewpoints_population.json")).unwrap()
}
pub fn fixture(name: &str) -> bool {
    let c5: Vec<String> =
        serde_json::from_str(include_str!("agency_second_half_names.json")).unwrap();
    if c5.iter().any(|n| n == name) {
        return true;
    }
    if !name.starts_with("agy-") {
        return false;
    }
    assert!(
        population()["names"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n == name),
        "unknown ADR0034 fixture {name}"
    );
    true
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
pub fn check(name: &str) -> Value {
    let s = source(name);
    serde_json::to_value(if name.starts_with("agy-invoice") {
        relca_core::report::check_configuration(&s, "AsIs")
    } else {
        relca_core::report::check(&s)
    })
    .unwrap()
}
pub fn replace(s: &str, from: &str, to: &str) -> String {
    let changed = s.replace(from, to);
    assert_ne!(changed, s, "counterfactual replacement must occur: {from}");
    changed
}
/// Preserve the frozen O/F/P/L oracle; C5 checks effects/version separately.
pub fn legacy_viewpoints(v: &Value) -> Value {
    let mut v = v.clone();
    if v["rules"] == "agency-viewpoints-p1-2" {
        v["rules"] = Value::String("agency-viewpoints-p1-1".into());
        v.as_object_mut().unwrap().remove("effects");
    }
    v
}
pub fn assert_report(name: &str, actual: &Value) {
    let e = expected(name);
    assert_eq!(
        actual["verdict"], e["verdict"],
        "{name} must lower before viewpoints assertions: {actual}"
    );
    let mut got: Vec<_> = actual["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    let mut want: Vec<_> = e["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    got.sort();
    want.sort();
    // The old numeric/lineage oracle is unchanged. ADR 0054 "範囲" / "意味"
    // requires the complete source/selection-bound responsibility rows before
    // comparing the old non-responsibility code inventory. No count-only cap.
    if actual.pointer("/agency/viewpoints/rules")
        == Some(&serde_json::json!("agency-viewpoints-p1-2"))
    {
        got.retain(|c| !["W-AGY-2", "W-AGY-3"].contains(c));
        let ast = relca_core::syntax::parse(&source(name)).unwrap();
        let model = relca_core::lower::lower_with_selection(
            &ast,
            name.starts_with("agy-invoice").then_some("AsIs"),
            None,
        )
        .unwrap();
        responsibility_oracle::assert_report(&model, actual);
        got.retain(|code| *code != "W-RESP-0");
        want.retain(|code| *code != "W-RESP-0");
    }
    assert_eq!(got, want, "diagnostics {name}");
    if e["agency"].is_null() {
        assert!(actual["agency"].is_null());
    } else {
        assert!(actual["agency"].is_object());
        assert!(
            actual["agency"].get("viewpoints").is_some(),
            "ADR0034 viewpoints is not implemented: {name}"
        );
        assert_eq!(
            legacy_viewpoints(&actual["agency"]["viewpoints"]),
            e["agency"]["viewpoints"],
            "complete hand-oracle payload {name}"
        );
    }
}
