//! Real CLI exits, nested records and independently corrupted closed wire.
#[path = "../../relca-core/tests/authority_scope.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn forty_hand_expectations_are_valid_closed_wire() {
    assert_eq!(acceptance::NAMES, fixture_match::AUTHORITY_FIXTURES);
    for n in acceptance::NAMES {
        fixture_match::validate_expected(&acceptance::expected(n))
            .unwrap_or_else(|e| panic!("{n}: {e}"));
    }
}
#[test]
fn real_binary_forty_reports_and_hand_exit_codes() {
    let mut failures = Vec::new();
    for n in acceptance::NAMES {
        let e = acceptance::expected(n);
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("check")
            .arg(acceptance::root().join(n).join("model.relca"))
            .output()
            .unwrap();
        let a: Value = serde_json::from_slice(&out.stdout).unwrap();
        let verdict: relca_core::Verdict = serde_json::from_value(e["verdict"].clone()).unwrap();
        if let Err(error) = fixture_match::compare_report(&e, &a) {
            failures.push(format!("{n}: {error}"));
        }
        if out.status.code() != Some(verdict.exit_code()) {
            failures.push(format!(
                "{n}: exit {:?} != {}",
                out.status.code(),
                verdict.exit_code()
            ));
        }
    }
    assert!(failures.is_empty(), "feature RED:\n{}", failures.join("\n"));
}

fn record(v: &Value, tag: &str) {
    let words = v["unchecked"].as_array().expect("unchecked record");
    assert_eq!(
        words
            .iter()
            .filter(|w| **w == "dynamic_authority_revocation")
            .count(),
        1
    );
    assert_eq!(
        words.last().unwrap(),
        "dynamic_authority_revocation",
        "new word follows existing markers"
    );
    let rows: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["field"] == "authority.G.revocable_by")
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["value"], tag);
    let affects = rows[0]["affects"].as_str().unwrap();
    assert!(
        affects.contains("動的") && affects.contains("撤回") && affects.contains("検査していない"),
        "record must not claim dynamic checking: {affects}"
    );
}
#[test]
fn revocation_all_commands_nested_reports_and_absence_control() {
    let src = acceptance::source("auth-revocation");
    for v in [
        acceptance::check(&src),
        serde_json::to_value(relca_core::explain::explain(&src)).unwrap(),
    ] {
        record(&v, "person:p");
    }
    let absent = acceptance::check(&acceptance::source("auth-revocation.broken-missing-record"));
    assert!(absent.get("unchecked").is_none());
    assert!(
        !absent["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"] == "authority.G.revocable_by")
    );
    let actor = src
        .replace("actor A", "actor Board {}\nactor A")
        .replace("revocable_by p", "revocable_by Board");
    record(&acceptance::check(&actor), "actor:Board");
    let configured=src.replace("  bounds", "  cut C { include [A] }\n  configuration First { cut C; process Flow }\n  configuration Second { cut C; process Flow }\n  bounds");
    let d = serde_json::to_value(relca_core::diff::diff(&configured, "First", "Second")).unwrap();
    for side in ["from", "to"] {
        record(&d[side]["report"], "person:p");
    }
    fixture_match::compare_diff_report(&json!({}), &d).unwrap();
    let c = serde_json::to_value(relca_core::compare::compare(
        &configured,
        &["First", "Second"],
        relca_core::compare::Analysis::Exhaustive,
    ))
    .unwrap();
    for p in c["plans"].as_array().unwrap() {
        record(&p["report"], "person:p");
    }
    fixture_match::compare_compare_report(&json!({}), &c).unwrap();
    let m = serde_json::to_value(relca_core::mutation::mutate(&src)).unwrap();
    record(&m["original"], "person:p");
    for kind in m["mutations"].as_array().unwrap() {
        for mutation in kind["mutants"].as_array().unwrap() {
            if mutation["report"].is_object() && mutation["report"]["bounds"]["cases"].is_u64() {
                record(&mutation["report"], "person:p");
            }
        }
    }
    fixture_match::validate_mutation_wire(&m).unwrap();
    for wrapper in [&d, &c, &m] {
        assert!(
            !wrapper["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w == "dynamic_authority_revocation"),
            "record belongs to Report not wrapper"
        );
    }
    for (s, completed) in [
        (
            src.replace(
                "  bounds",
                "  simulation replications 2 seed 7 confidence 0.95\n  bounds",
            ),
            true,
        ),
        (src.clone(), false),
    ] {
        let sim = serde_json::to_value(relca_core::report::simulate(&s)).unwrap();
        record(&sim, "person:p");
        assert_eq!(
            sim["simulation"]["status"],
            // No simulation clause stays the existing E-SIM-0 (invalid); the
            // new record must still be attached to that report.
            if completed { "completed" } else { "invalid" }
        );
        fixture_match::validate_simulation_wire(&sim).unwrap();
    }
}

#[test]
fn executed_mutant_reports_keep_records_and_unnamed_authority_is_not_applicable() {
    let source =
        std::fs::read_to_string(acceptance::root().join("mut-evidence/model.relca")).unwrap();
    let source=source.replace("  bounds", "  people [p]\n  authority G { basis \"record\"; grants p to Reader.enter; revocable_by p }\n  bounds");
    let v = serde_json::to_value(relca_core::mutation::mutate(&source)).unwrap();
    record(&v["original"], "person:p");
    let kinds = v["mutations"].as_array().unwrap();
    let expiry = kinds
        .iter()
        .find(|k| k["kind"] == "authority_expired")
        .unwrap();
    assert_eq!(expiry["status"], "not_applicable");
    assert_eq!(expiry["reason"], "no_matching_site");
    assert_eq!(expiry["mutants"], json!([]));
    let mut executed = 0;
    for kind in kinds {
        for mutant in kind["mutants"].as_array().unwrap() {
            if mutant["report"]["bounds"]["cases"].is_u64() {
                record(&mutant["report"], "person:p");
                executed += 1;
            }
            if mutant["result"] == "not_run" {
                assert!(mutant["report"].is_null());
            }
        }
    }
    assert!(
        executed > 0,
        "must exercise executed nested reports, not just KindRows"
    );
    fixture_match::validate_mutation_wire(&v).unwrap();
}

#[test]
fn mutation_and_explain_nested_wire_reject_record_corruption() {
    let src = std::fs::read_to_string(acceptance::root().join("mut-evidence/model.relca")).unwrap();
    let mut m = serde_json::to_value(relca_core::mutation::mutate(&src)).unwrap();
    m["original"] = hand_record(m["original"].clone());
    let mut pointers = vec!["/original".to_owned()];
    for (i, kind) in m["mutations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        for (j, mutant) in kind["mutants"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            if mutant["report"]["bounds"]["cases"].is_u64() {
                mutant["report"] = hand_record(mutant["report"].clone());
                pointers.push(format!("/mutations/{i}/mutants/{j}/report"));
            }
        }
    }
    assert!(pointers.len() > 1);
    fixture_match::validate_mutation_wire(&m).unwrap();
    for pointer in pointers {
        for bad in bad_records(m.pointer(&pointer).unwrap()) {
            let mut v = m.clone();
            *v.pointer_mut(&pointer).unwrap() = bad;
            assert!(
                fixture_match::validate_mutation_wire(&v).is_err(),
                "nested mutation record {pointer}"
            );
        }
    }
    let old = acceptance::old_skeleton(&acceptance::source("auth-revocation"));
    let v = hand_record(serde_json::to_value(relca_core::explain::explain(&old)).unwrap());
    fixture_match::validate_explain_wire(&v).unwrap();
    for bad in bad_records(&v) {
        assert!(fixture_match::validate_explain_wire(&bad).is_err());
    }
}

fn hand_record(mut v: Value) -> Value {
    let word = json!("dynamic_authority_revocation");
    if let Some(a) = v.get_mut("unchecked") {
        a.as_array_mut().unwrap().push(word);
    } else {
        v["unchecked"] = json!([word]);
    }
    v["assumptions"].as_array_mut().unwrap().push(json!({"field":"authority.G.revocable_by","value":"person:p","affects":"動的な撤回は検査していない"}));
    v
}
fn bad_records(good: &Value) -> Vec<Value> {
    let mut rows = Vec::new();
    let last = good["assumptions"].as_array().unwrap().len() - 1;
    for value in [
        json!([]),
        Value::Null,
        json!(["unknown"]),
        json!([
            "dynamic_authority_revocation",
            "dynamic_authority_revocation"
        ]),
    ] {
        let mut v = good.clone();
        v["unchecked"] = value;
        rows.push(v);
    }
    let mut v = good.clone();
    v.as_object_mut().unwrap().remove("unchecked");
    rows.push(v);
    let mut v = good.clone();
    v["assumptions"].as_array_mut().unwrap().pop();
    rows.push(v);
    for tag in ["p", "person:", "actor:", "unknown:p", ""] {
        let mut v = good.clone();
        v["assumptions"][last]["value"] = json!(tag);
        rows.push(v);
    }
    let mut v = good.clone();
    v["assumptions"][last]["field"] = json!("authority..revocable_by");
    rows.push(v);
    let mut v = good.clone();
    let row = v["assumptions"][last].clone();
    v["assumptions"].as_array_mut().unwrap().push(row);
    rows.push(v);
    let mut v = good.clone();
    v["assumptions"][last]["surplus"] = json!(true);
    rows.push(v);
    rows
}
#[test]
fn closed_wire_requires_record_and_marker_pair_and_rejects_all_single_corruptions() {
    // Construct valid new wire from old semantics; never copy new solver output.
    let old = acceptance::old_skeleton(&acceptance::source("auth-revocation"));
    let good = hand_record(acceptance::check(&old));
    fixture_match::compare_report(&json!({}), &good)
        .expect("hand-authored record must be accepted");
    for bad in bad_records(&good) {
        assert!(
            fixture_match::compare_report(&json!({}), &bad).is_err(),
            "invalid record accepted: {bad}"
        );
    }
    let sim = old.replace(
        "  bounds",
        "  simulation replications 2 seed 7 confidence 0.95\n  bounds",
    );
    let good = hand_record(serde_json::to_value(relca_core::report::simulate(&sim)).unwrap());
    fixture_match::validate_simulation_wire(&good).unwrap();
    for bad in bad_records(&good) {
        assert!(fixture_match::validate_simulation_wire(&bad).is_err());
    }
}

#[test]
fn nested_wire_cannot_bypass_record_validation() {
    let src=acceptance::old_skeleton(&acceptance::source("auth-revocation")).replace("  bounds","  cut C { include [A] }\n  configuration First { cut C; process Flow }\n  configuration Second { cut C; process Flow }\n  bounds");
    let mut d = serde_json::to_value(relca_core::diff::diff(&src, "First", "Second")).unwrap();
    for side in ["from", "to"] {
        d[side]["report"] = hand_record(d[side]["report"].clone());
    }
    fixture_match::compare_diff_report(&json!({}), &d).unwrap();
    for side in ["from", "to"] {
        for bad in bad_records(&d[side]["report"]) {
            let mut v = d.clone();
            v[side]["report"] = bad;
            assert!(fixture_match::compare_diff_report(&json!({}), &v).is_err());
        }
    }
    let mut c = serde_json::to_value(relca_core::compare::compare(
        &src,
        &["First", "Second"],
        relca_core::compare::Analysis::Exhaustive,
    ))
    .unwrap();
    for p in c["plans"].as_array_mut().unwrap() {
        p["report"] = hand_record(p["report"].clone());
    }
    fixture_match::compare_compare_report(&json!({}), &c).unwrap();
    for i in 0..2 {
        for bad in bad_records(&c["plans"][i]["report"]) {
            let mut v = c.clone();
            v["plans"][i]["report"] = bad;
            assert!(fixture_match::compare_compare_report(&json!({}), &v).is_err());
        }
    }
}

#[test]
fn cut_abort_and_unused_revoker_keep_records_and_multiple_revokers_are_unique() {
    let src = acceptance::source("auth-revocation");
    record(
        &acceptance::check(&src.replace("clock 20min", "clock 1min")),
        "person:p",
    );
    let unused = src.replace("a = A.run(); ", "");
    record(&acceptance::check(&unused), "person:p");
    let two = src.replace(
        "process Flow",
        "authority H { basis \"second\"; grants q to A.run; revocable_by r }\nprocess Flow",
    );
    let v = acceptance::check(&two);
    record(&v, "person:p");
    let rows: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["field"].as_str().unwrap().starts_with("authority."))
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["field"], "authority.G.revocable_by");
    assert_eq!(rows[1]["field"], "authority.H.revocable_by");
    assert_eq!(rows[1]["value"], "person:r");
    let abort = src
        .replace(
            "  bounds",
            "  simulation replications 2 seed 7 confidence 0.95\n  bounds",
        )
        .replace("enum { pass }", "enum { pass, fail }")
        .replace("result nondet { pass }", "result nondet { pass, fail }");
    let v = serde_json::to_value(relca_core::report::simulate(&abort)).unwrap();
    record(&v, "person:p");
    assert_eq!(v["simulation"]["status"], "unsupported");
    fixture_match::validate_simulation_wire(&v).unwrap();
}
