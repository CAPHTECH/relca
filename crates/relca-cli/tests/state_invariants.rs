//! Real process boundary, exit, exact diagnostics, and closed new explain detail.
#[path = "../../relca-core/tests/support/state_invariant_acceptance.rs"]
mod h;
#[path = "../src/fixture_match.rs"]
mod wire;
use serde_json::{Value, json};
use std::process::Command;
fn run(command: &str, name: &str) -> (Value, i32) {
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args([command, &format!("fixtures/{name}/model.relca")])
        .current_dir(h::root())
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let stderr: String = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(
        o.stderr,
        stderr.as_bytes(),
        "only canonical diagnostic stderr"
    );
    (v, o.status.code().unwrap())
}
#[test]
fn new_classification_positions_have_precise_code_message_and_byte_span() {
    for (name, code, message, fragment) in [
        (
            "sinv-static-binding-bare.broken-binding-bare",
            "U-SEM-0",
            "state invariant reads a process binding",
            "j",
        ),
        (
            "sinv-static-binding-compare.broken-binding-compare",
            "U-SEM-0",
            "state invariant reads a process binding",
            "j",
        ),
        (
            "sinv-static-unknown.broken-unknown",
            "E-NAME-0",
            "undefined state invariant variable",
            "absent_name",
        ),
        (
            "sinv-bool-type.broken-optional",
            "E-TYPE-0",
            "bare invariant atom requires nonoptional builtin bool",
            "ok",
        ),
        (
            "sinv-static-terminal.broken-terminal",
            "U-SYN-0",
            "terminal or receipt antecedent",
            "end.posted",
        ),
        (
            "sinv-static-receipt.broken-receipt",
            "U-SYN-0",
            "terminal or receipt antecedent",
            "case.received",
        ),
        (
            "sinv-static-event-bare.broken-event-bare",
            "U-SYN-0",
            "bare event invariant atom",
            "ok",
        ),
        (
            "sinv-static-qualified-bare.broken-qualified-bare",
            "U-SYN-0",
            "qualified bare invariant atom",
            "Ledger.flag",
        ),
    ] {
        let (v, exit) = run("check", name);
        h::subset(&h::expected(name), &v);
        assert_eq!(exit, if code.starts_with('U') { 4 } else { 3 });
        let ds: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(ds.len(), 1);
        let d = &ds[0];
        assert_eq!(d["code"], code);
        assert!(
            d["message"].as_str().unwrap().ends_with(message),
            "exact invariant-local diagnostic {d}"
        );
        let s = h::source(name);
        let a = d["span"]["start"].as_u64().unwrap() as usize;
        let b = d["span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&s[a..b], fragment);
        assert!(a >= s.rfind("invariant").unwrap());
    }
}
#[test]
fn check_and_explain_use_same_root_classification_and_only_explain_adds_detail() {
    for (name, exit) in [
        ("sinv-approval", 0),
        ("sinv-approval.broken-bypass", 1),
        ("sinv-initial.broken-initial", 1),
        ("sinv-vacuous", 5),
        ("sinv-clock.broken-truncated", 2),
        ("sinv-ended-clean.broken-increment", 1),
    ] {
        let (a, ac) = run("check", name);
        let (b, bc) = run("explain", name);
        assert_eq!((ac, bc), (exit, exit));
        assert_eq!(a["properties"], b["properties"]);
        assert!(a.get("explanation").is_none());
        wire::validate_explain_wire(&b).unwrap();
        if exit == 1 {
            let c = &b["explanation"]["counterexamples"][0];
            let last = c["steps"].as_array().unwrap().last().unwrap();
            assert_eq!(last["violation"]["state_invariant"]["antecedent"], true);
            assert_eq!(last["violation"]["state_invariant"]["condition"], false);
        }
    }
}
#[test]
fn new_detail_wire_is_closed_and_rejects_unknown_keys_and_wrong_types() {
    let (v, exit) = run("explain", "sinv-initial.broken-initial");
    assert_eq!(exit, 1);
    wire::validate_explain_wire(&v).unwrap();
    let at = "/explanation/counterexamples/0/steps/0/violation/state_invariant";
    for (field, bad) in [
        ("antecedent", json!("true")),
        ("condition", json!(0)),
        ("case_id", json!("1")),
        ("values", Value::Null),
        ("unknown", json!(true)),
    ] {
        let mut t = v.clone();
        t.pointer_mut(at).unwrap()[field] = bad;
        assert_ne!(v, t);
        assert!(
            wire::validate_explain_wire(&t).is_err(),
            "closed type {field}"
        );
    }
    let mut t = v.clone();
    t.pointer_mut(&format!("{at}/values/0")).unwrap()["value"] = json!(false);
    assert_ne!(v, t);
    assert!(wire::validate_explain_wire(&t).is_err());
    let mut t = v.clone();
    t["state_invariant"] = json!({});
    assert!(
        wire::validate_explain_wire(&t).is_err(),
        "not a global report key"
    );
    let mut t = v.clone();
    t.pointer_mut("/explanation/counterexamples/0/steps/0/violation")
        .unwrap()["unknown"] = json!(0);
    assert!(wire::validate_explain_wire(&t).is_err());
}

#[test]
fn closed_new_detail_branch_is_calibrated_without_waiting_for_new_runtime() {
    // An honest legacy carrier plus synthetic new detail tests the independent
    // wire table only. It is not a semantic state-invariant or replay claim.
    let (mut v, exit) = run("explain", "explain-invariant.broken-cut-guard");
    assert_eq!(exit, 1);
    let steps = v
        .pointer_mut("/explanation/counterexamples/0/steps")
        .unwrap()
        .as_array_mut()
        .unwrap();
    let index = steps.len() - 1;
    let last = &mut steps[index];
    let loc = last["location"].clone();
    last["violation"]["state_invariant"] = json!({"case_id":1,"antecedent":true,"condition":false,
        "values":[{"name":"ok","value":"false","visibility":"observed","location":loc}]});
    wire::validate_explain_wire(&v).unwrap();
    let path = format!("/explanation/counterexamples/0/steps/{index}/violation/state_invariant");
    for (key, value) in [
        ("case_id", json!("1")),
        ("antecedent", json!("true")),
        ("condition", json!(0)),
        ("values", Value::Null),
        ("unknown", json!(true)),
    ] {
        let mut t = v.clone();
        t.pointer_mut(&path).unwrap()[key] = value;
        assert_ne!(v, t);
        assert!(wire::validate_explain_wire(&t).is_err(), "closed {key}");
    }
    let mut t = v.clone();
    t.pointer_mut(&format!("{path}/values/0")).unwrap()["value"] = json!(false);
    assert!(wire::validate_explain_wire(&t).is_err());
    let mut t = v.clone();
    t["state_invariant"] = json!({});
    assert!(wire::validate_explain_wire(&t).is_err());
}
#[test]
fn simulation_safe_has_exit2_and_initial_witness_has_exit1() {
    for (name, exit) in [
        ("sinv-simulation", 2),
        ("sinv-simulation.broken-initial", 1),
    ] {
        let (v, c) = run("simulate", name);
        assert_eq!(c, exit);
        h::subset(&h::expected(name), &v);
        let p = h::property(&v, "contract Controls #1 invariant");
        assert_eq!(
            p["cases"][0]["basis"],
            if exit == 1 { "witness" } else { "sample" }
        );
    }
}
