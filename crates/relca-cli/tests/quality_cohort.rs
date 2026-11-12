//! Real CLI acceptance uses the same hand JSON, never the core report as oracle.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/quality_acceptance.rs"]
mod h;
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn all_quality_commands_match_hand_values_and_exit_collect_all_red() {
    let mut failures = vec![];
    for n in h::names() {
        let directory = h::root().join("fixtures").join(&n);
        let ip = directory.join("invocation.json");
        let inv: Option<Value> = ip
            .exists()
            .then(|| serde_json::from_slice(&std::fs::read(ip).unwrap()).unwrap());
        let invocation = fixture_match::Invocation::parse(inv.as_ref()).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(invocation.args(&directory.join("model.relca")))
            .output()
            .unwrap();
        let result = std::panic::catch_unwind(|| {
            let v: Value = serde_json::from_slice(&output.stdout).unwrap();
            h::hand(&n, &v);
            let expected: relca_core::Verdict =
                serde_json::from_value(h::expected(&n)["verdict"].clone()).unwrap();
            assert_eq!(
                output.status.code(),
                Some(expected.exit_code()),
                "{n}: actual process exit"
            );
            invocation.compare(&h::expected(&n), &v).unwrap();
        });
        if result.is_err() {
            failures.push(n);
        }
    }
    assert!(
        failures.is_empty(),
        "all CLI quality assertion failures: {failures:?}"
    );
}

#[test]
fn closed_quality_wire_accepts_hand_json_and_rejects_unknown_missing_and_fake_nulls() {
    let inventory = h::inventory();
    for n in h::names() {
        let e = h::expected(&n);
        let inv = &inventory[&n]["invocation"];
        let parsed = fixture_match::Invocation::parse((!inv.is_null()).then_some(inv)).unwrap();
        parsed
            .validate_expected(&e)
            .unwrap_or_else(|err| panic!("{n}: malformed hand expectation: {err}"));
        let Some(q) = e.get("quality") else {
            continue;
        };
        if !q.is_object() {
            continue;
        }
        let sampled = q["analysis"] == "simulation";
        fixture_match::validate_quality_wire(q, sampled)
            .unwrap_or_else(|e| panic!("{n}: independent hand JSON invalid: {e}"));
        for path in [
            "",
            "/properties/0",
            "/properties/0/cases/0",
            "/properties/0/expected_count",
            "/cohort/0",
            "/cohort/0/value",
            "/outcomes_per_arrival/0",
            "/outcomes_per_arrival/0/rate",
        ] {
            let Some(obj) = q.pointer(path).and_then(Value::as_object) else {
                continue;
            };
            let mut bad = q.clone();
            bad.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("rank".into(), json!(1));
            assert!(
                fixture_match::validate_quality_wire(&bad, sampled).is_err(),
                "{n}: unknown {path}"
            );
            for key in obj.keys() {
                let mut bad = q.clone();
                bad.pointer_mut(path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(
                    fixture_match::validate_quality_wire(&bad, sampled).is_err(),
                    "{n}: missing {path}/{key}"
                );
            }
        }
        if !sampled && !q["cohort"].as_array().unwrap().is_empty() {
            let mut bad = q.clone();
            bad["cohort"][0]["value"]["lower"] = Value::Null;
            bad["cohort"][0]["value"]["reason"] = Value::Null;
            assert!(fixture_match::validate_quality_wire(&bad, false).is_err());
        }
    }
}
