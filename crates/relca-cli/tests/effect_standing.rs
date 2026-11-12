//! Real CLI plus independent closed wire controls for ADR0040.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/effect_standing.rs"]
mod hand;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
#[test]
fn all_real_cli_jobs_collect_hand_verdict_exit_and_wire_failures() {
    let mut failures = vec![];
    for n in hand::names() {
        let path = format!("fixtures/{n}/model.relca");
        let inv = hand::invocation(&n);
        let invocation = fixture_match::Invocation::parse(inv.as_ref()).unwrap();
        let mut args = vec![
            inv.as_ref()
                .and_then(|v| v["command"].as_str())
                .unwrap_or("check")
                .to_owned(),
            path,
        ];
        if let Some(i) = inv.filter(|v| v["command"] == "diff") {
            args.extend([
                "--from".into(),
                i["from"].as_str().unwrap().into(),
                "--to".into(),
                i["to"].as_str().unwrap().into(),
            ]);
        }
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .current_dir(root())
            .output()
            .unwrap();
        let actual: Value = serde_json::from_slice(&out.stdout).unwrap();
        let expected = hand::expected(&n);
        let verdict: relca_core::Verdict =
            serde_json::from_value(expected["verdict"].clone()).unwrap();
        let comparison = invocation.compare(&expected, &actual);
        if actual["verdict"] != expected["verdict"]
            || out.status.code() != Some(verdict.exit_code())
            || comparison.is_err()
        {
            failures.push(format!(
                "{n}: expected {}/{}, got {}/{:?}, matcher={comparison:?}",
                expected["verdict"],
                verdict.exit_code(),
                actual["verdict"],
                out.status.code()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn all86_hand_expectations_are_well_typed_before_any_implementation_output() {
    let names = hand::names();
    assert_eq!(names.len(), 86);
    for n in names {
        let inv = hand::invocation(&n);
        fixture_match::Invocation::parse(inv.as_ref())
            .unwrap()
            .validate_expected(&hand::expected(&n))
            .unwrap_or_else(|e| panic!("{n}: {e}"));
    }
}

fn old_wire() -> Value {
    let mut v: Value =
        serde_json::from_str(include_str!("../../../fixtures/resp-contest/expected.json")).unwrap();
    v["agency"] = json!({});
    v["mode"] = json!("exhaustive");
    v["bounds"]["cases"] = json!(1);
    v["contest_runtime"] = json!([{"id":"case1:contest1","static_id":"contract Controls #1 contest","case_id":1,"party":"Vendor","target":"returned","route":"Clerk.recheck","respond_by":5,"basis":"モデル内での期限内応答","coverage":"complete","alternatives":[
        {"filed":false,"target_at":3,"filed_at":null,"deadline":null,"status":"not_filed","response_at":null,"closed_at":3,"executor":null,"result":null,"violation_reason":null,"route_work":null,"route_wait":null,"route_cost":{"min":null,"max":null,"reason":"never_created"}}
    ]}]);
    v
}
fn effect_wire() -> Value {
    let mut v = old_wire();
    v["contest_paths"][0]["end"] = Value::Null;
    v["contest_paths"][0]["effect"] = json!("Ledger.post.commit");
    v["contest_paths"][0]["rules"] = json!("contest-p1-2");
    v["contest_runtime"][0]["target"] = json!("Ledger.post.commit");
    v["contest_runtime"][0]["target_kind"] = json!("effect");
    v
}
fn valid(v: &Value) -> Result<(), String> {
    fixture_match::validate_responsibility_wire(v)?;
    fixture_match::validate_contest_runtime_wire(v)
}

#[test]
fn closed_wire_keeps_old_end_shape_and_rejects_all_malformed_effect_tags() {
    let old = old_wire();
    let good = effect_wire();
    valid(&old).unwrap();
    valid(&good).unwrap();
    for (pointer, value) in [
        ("/contest_paths/0/end", json!("returned")),
        ("/contest_paths/0/effect", Value::Null),
        ("/contest_paths/0/effect", json!("")),
        ("/contest_paths/0/effect", json!("Ledger.post.finish")),
        ("/contest_paths/0/rules", json!("contest-p1-1")),
        ("/contest_runtime/0/target_kind", json!("end")),
        ("/contest_runtime/0/target_kind", Value::Null),
        ("/contest_runtime/0/target", json!("Ledger.post.start")),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(valid(&bad).is_err(), "accepted malformed effect {pointer}");
    }
    for (table, key) in [
        ("contest_paths", "effect"),
        ("contest_runtime", "target_kind"),
    ] {
        let mut bad = good.clone();
        bad[table][0].as_object_mut().unwrap().remove(key);
        assert!(valid(&bad).is_err(), "missing {key}");
    }
    let mut bad = old.clone();
    bad["contest_paths"][0]["effect"] = Value::Null;
    assert!(valid(&bad).is_err(), "old End must omit effect, not null");
    let mut bad = old;
    bad["contest_runtime"][0]["target_kind"] = json!("effect");
    assert!(valid(&bad).is_err(), "End must omit target_kind");
    // This table validator is also a public test seam used without the static
    // table by the older controls. Effect tagging remains mandatory there too.
    let mut runtime_only = good;
    runtime_only
        .as_object_mut()
        .unwrap()
        .remove("contest_paths");
    fixture_match::validate_contest_runtime_wire(&runtime_only).unwrap();
    runtime_only["contest_runtime"][0]
        .as_object_mut()
        .unwrap()
        .remove("target_kind");
    assert!(fixture_match::validate_contest_runtime_wire(&runtime_only).is_err());
}

#[test]
fn cli_explain_and_compare_transport_effect_tables_without_configuration_on_explain() {
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args([
            "explain",
            "fixtures/sov-concurrent.broken-short/model.relca",
        ])
        .current_dir(root())
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(1));
    fixture_match::Invocation::Explain
        .compare(&hand::expected("sov-concurrent.broken-short"), &v)
        .unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args([
            "compare",
            "fixtures/sov-diff/model.relca",
            "--plans",
            "Skipping,Posting",
            "--mode",
            "exhaustive",
        ])
        .current_dir(root())
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(1));
    fixture_match::Invocation::parse(Some(
        &json!({"command":"compare","plans":["Skipping","Posting"],"mode":"exhaustive"}),
    ))
    .unwrap()
    .compare(&json!({"verdict":"violated"}), &v)
    .unwrap();
    let plans = v["plans"].as_array().unwrap();
    assert_eq!(plans.len(), 2);
    assert_eq!(
        plans[0]["report"]["contest_runtime"][0]["alternatives"],
        json!([])
    );
    hand::subset(
        &hand::expected("sov-basic")["contest_runtime"],
        &plans[1]["report"]["contest_runtime"],
        "compare effect row",
    );
}
