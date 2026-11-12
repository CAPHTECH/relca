//! Hand-wire calibration and production decode obligations, ADR 0058.
#[path = "support/ledger_expected.rs"]
mod oracle;
use oracle::{Spec, known};
use serde_json::{Value, json};

fn host() -> Value {
    json!({"uncertainty":{"rules":"uncertainty-p2-1",
        "aleatory":{"status":"unavailable","reason":"input_unavailable","items":[]},
        "estimation":{"status":"not_applicable","reason":"exhaustive_analysis","items":[]},
        "model":{"status":"unavailable","reason":"input_unavailable","items":[]}},
        "actors":null,"cut_context":{"notices":[],"reentry":[],"horizon":[]},
        "agency":null,"verdict":"invalid","mode":"exhaustive","unit":"min",
        "model":{"name":null,"sha256":null},"cut":"undeclared","assumptions":[],
        "diagnostics":[{"code":"E-SYN-0","severity":"error","message":"unreadable","span":null}],
        "bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},
        "properties":[],"terminals":[]})
}
fn control() -> (Value, Spec) {
    let mut v = host();
    let s = Spec::prelower("exhaustive", true);
    v["ledger"] = oracle::expected(&v, &s).unwrap();
    (v, s)
}
fn object_paths(v: &Value, p: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(fields) => {
            out.push(p.into());
            for (k, v) in fields {
                object_paths(v, &format!("{p}/{k}"), out);
            }
        }
        Value::Array(rows) => {
            for (n, v) in rows.iter().enumerate() {
                object_paths(v, &format!("{p}/{n}"), out);
            }
        }
        _ => {}
    }
}
#[test]
fn green_hand_control_has_closed_keys_at_every_depth() {
    let mut v = host();
    v["agency"] = json!({"rules":"agency-p0-1"});
    let mut s = Spec::finite("P", &["A"]);
    s.actors["entries"][0]["rule"] = known(json!({"declared":"r1","effective":"r2"}));
    v["ledger"] = oracle::expected(&v, &s).unwrap();
    oracle::validate(&v, &s).unwrap();
    let mut paths = Vec::new();
    object_paths(&v["ledger"], "/ledger", &mut paths);
    assert!(paths.len() >= 20);
    for p in paths {
        let keys = v
            .pointer(&p)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            let mut bad = v.clone();
            bad.pointer_mut(&p)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(&key);
            assert!(oracle::validate(&bad, &s).is_err(), "missing {p}/{key}");
        }
        let mut bad = v.clone();
        bad.pointer_mut(&p)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("surplus".into(), json!(0));
        assert!(oracle::validate(&bad, &s).is_err(), "extra {p}/surplus");
    }
}
#[test]
fn green_missing_unknown_empty_and_fake_defaults_are_rejected() {
    let (v, s) = control();
    for (p, replacement) in [
        ("/ledger/schema", json!("relca-conclusion-ledger/v2")),
        ("/ledger/producer/version", json!("")),
        ("/ledger/producer/version", json!("0.2")),
        ("/ledger/producer/build_identity", known("dirty-2026")),
        ("/ledger/derivation/bundle", json!("agency-p0-1")),
        ("/ledger/backends/external/version", json!("0.0.0")),
        ("/ledger/backends/external/status", json!("connected")),
        ("/ledger/execution/bounds", known("/bounds")),
        ("/ledger/execution/limits/state_budget", known(1_000_000)),
        ("/ledger/actor_versions/status", json!("known")),
        ("/ledger/context/model", known("/missing")),
        ("/ledger/subjects/0/source_ledgers", json!(["/ledger"])),
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(p).unwrap() = replacement;
        assert!(oracle::validate(&bad, &s).is_err(), "escaped {p}");
    }
}
#[test]
fn green_seed_zero_budget_zero_and_unavailable_are_distinct() {
    let mut v = host();
    v["simulation"] = json!({"replications":4,"completed_replications":2,"seed":0});
    let mut s = Spec::simulation("P", &["A"], 0, 4, "3/4");
    s.limits["state_budget"] = known(0);
    v["ledger"] = oracle::expected(&v, &s).unwrap();
    oracle::validate(&v, &s).unwrap();
    for (p, value) in [
        ("/ledger/execution/seed", oracle::unavailable("not_lowered")),
        ("/ledger/execution/replications", known(2)),
        ("/ledger/execution/limits/state_budget", known(1_000_000)),
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(p).unwrap() = value;
        assert!(
            oracle::validate(&bad, &s).is_err(),
            "default masked real setting {p}"
        );
    }
}
#[test]
fn green_local_versions_are_closed_and_must_match_the_original_field() {
    let mut v = host();
    v["agency"] = json!({"rules":"agency-p0-1"});
    let s = Spec::finite("P", &[]);
    v["ledger"] = oracle::expected(&v, &s).unwrap();
    oracle::validate(&v, &s).unwrap();
    let mut bad = v.clone();
    bad["ledger"]["derivation"]["components"][0]["version"] = json!("agency-p1-state-1");
    assert!(oracle::validate(&bad, &s).is_err());
    let mut bad = v.clone();
    bad["agency"]["rules"] = json!("forged");
    bad["ledger"]["derivation"]["components"][0]["version"] = json!("forged");
    assert!(
        oracle::validate(&bad, &s)
            .unwrap_err()
            .starts_with("UNSUPPORTED_LOCAL_VERSION")
    );
    let mut bad = v.clone();
    bad["ledger"]["derivation"]["components"] = json!([]);
    assert!(oracle::validate(&bad, &s).is_err());
}
#[test]
fn green_actor_versions_not_declared_and_rule_direction_are_not_fabricated() {
    let mut v = host();
    let mut s = Spec::finite("P", &["Document", "AI", "Rule"]);
    s.actors["entries"][1]["via_model"] = known("doc@v3");
    s.actors["entries"][2]["rule"] = known(json!({"declared":"r1","effective":"r2"}));
    v["ledger"] = oracle::expected(&v, &s).unwrap();
    oracle::validate(&v, &s).unwrap();
    for (p, value) in [
        (
            "/ledger/actor_versions/entries/0/via_model",
            known("default-v1"),
        ),
        ("/ledger/actor_versions/entries/1/via_model", known("v3")),
        (
            "/ledger/actor_versions/entries/2/rule/value/effective",
            json!("r1"),
        ),
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(p).unwrap() = value;
        assert!(oracle::validate(&bad, &s).is_err());
    }
}
#[test]
fn green_aggregate_ownership_is_by_configuration_not_array_guessing() {
    let mut v = json!({"verdict":"satisfied","mode":"compare","plans":[
        {"configuration":"B","report":host()},{"configuration":"A","report":host()}],
        "tables":{"properties":[{"values":[{"configuration":"A","verdict":"satisfied"},
            {"configuration":"B","verdict":"satisfied"}]}]}});
    let s = Spec::aggregate("compare");
    v["ledger"] = oracle::expected(&v, &s).unwrap();
    oracle::validate(&v, &s).unwrap();
    assert_eq!(
        v["ledger"]["subjects"][0]["source_ledgers"],
        json!(["/plans/1/report/ledger"])
    );
    let mut bad = v.clone();
    bad["ledger"]["subjects"][0]["source_ledgers"] = json!(["/plans/0/report/ledger"]);
    assert!(oracle::validate(&bad, &s).is_err());
    v["plans"][1]["configuration"] = json!("B");
    assert!(oracle::expected(&v, &s).is_err());
}
#[test]
fn green_mutant_context_uses_original_and_only_its_own_report() {
    let mut v = json!({"verdict":"unsupported","original":host(),"run":{},"mutations":[
        {"status":"generated","mutants":[{"report":host(),"site":{"actor":"A"},"operator":"drop"},
            {"report":null,"site":{"actor":"B"},"operator":"drop"}]}]});
    let s = Spec::aggregate("mutation");
    v["ledger"] = oracle::expected(&v, &s).unwrap();
    oracle::validate(&v, &s).unwrap();
    let targets = v["ledger"]["subjects"].as_array().unwrap();
    let a = targets
        .iter()
        .find(|x| x["target"] == "/mutations/0/mutants/0")
        .unwrap();
    let b = targets
        .iter()
        .find(|x| x["target"] == "/mutations/0/mutants/1")
        .unwrap();
    assert_eq!(
        a["source_ledgers"],
        json!(["/original/ledger", "/mutations/0/mutants/0/report/ledger"])
    );
    assert_eq!(b["source_ledgers"], json!(["/original/ledger"]));
}
#[test]
fn production_decoder_requires_ledger_instead_of_defaulting() {
    let (v, _) = control();
    serde_json::from_value::<relca_core::report::Report>(v.clone())
        .expect("hand positive decoder control");
    let mut bad = v;
    bad.as_object_mut().unwrap().remove("ledger");
    assert!(
        serde_json::from_value::<relca_core::report::Report>(bad).is_err(),
        "LEDGER_MISSING accepted by production decoder"
    );
}
#[test]
fn production_decoder_rejects_unknown_version_empty_version_and_context_mismatch() {
    let (v, _) = control();
    serde_json::from_value::<relca_core::report::Report>(v.clone())
        .expect("hand positive decoder control");
    for (p, value) in [
        ("/ledger/schema", json!("relca-conclusion-ledger/v999")),
        ("/ledger/producer/version", json!("")),
        ("/ledger/context/model", known("/nowhere")),
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(p).unwrap() = value;
        assert!(
            serde_json::from_value::<relca_core::report::Report>(bad).is_err(),
            "INVALID_LEDGER accepted at {p}"
        );
    }
}
#[test]
fn production_ledger_diagnostic_is_registered_as_error_and_folds_closed() {
    use relca_core::{Verdict, diag, report};
    assert_eq!(
        diag::severity_of("U-LEDGER-0"),
        Some(diag::Severity::Error),
        "LEDGER_DIAGNOSTIC_MISSING"
    );
    let d = diag::Diagnostic {
        code: "U-LEDGER-0".into(),
        severity: diag::Severity::Error,
        message: "version_unavailable: missing producer version".into(),
        span: None,
    };
    assert_eq!(report::fold_verdict(&[], &[d]), Verdict::Unsupported);
}
