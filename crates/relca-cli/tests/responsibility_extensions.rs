//! ADR 0028 real binary and an independently closed wire oracle.
#[path = "../../relca-core/tests/responsibility_extensions.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn all_thirty_fixture_expectations_have_legal_closed_schema() {
    assert_eq!(
        acceptance::NAMES,
        fixture_match::RESPONSIBILITY_EXTENSION_FIXTURES
    );
    for name in acceptance::NAMES {
        fixture_match::validate_expected(&acceptance::expected(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    // Exercise the explain comparator before new syntax can mask a wrong
    // wrapper choice. Plain compare_report intentionally rejects explanation.
    let old = acceptance::stripped(&acceptance::source("resp2-within"));
    let explain = serde_json::to_value(relca_core::explain::explain(&old)).unwrap();
    fixture_match::Invocation::Explain
        .compare(&json!({}), &explain)
        .unwrap();
}

#[test]
fn thirty_real_binary_reports_and_exit_codes() {
    for name in acceptance::NAMES {
        let expected = acceptance::expected(name);
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("check")
            .arg(acceptance::root().join(name).join("model.relca"))
            .output()
            .unwrap();
        let actual: Value = serde_json::from_slice(&out.stdout).unwrap();
        eprintln!("CLI fixture {name}");
        acceptance::subset(&expected, &actual);
        fixture_match::compare_report(&expected, &actual).unwrap_or_else(|e| panic!("{name}: {e}"));
        let verdict: relca_core::Verdict =
            serde_json::from_value(expected["verdict"].clone()).unwrap();
        assert_eq!(out.status.code(), Some(verdict.exit_code()), "{name}");
    }
}

#[test]
fn cli_rejection_matrix_exit_and_single_process_explain() {
    // These boundaries are covered by the public API matrix too; on-disk cases
    // below make the intended exit contract observable independently.
    for (name, exit) in [
        ("resp2-bool.broken-enum", 3),
        ("resp2-classification.broken-operator", 4),
        ("resp2-collision.broken-capability", 3),
        ("resp2-design.broken-unknown", 3),
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("check")
            .arg(acceptance::root().join(name).join("model.relca"))
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(exit), "{name}");
    }
    for name in ["resp2-within", "resp2-within.broken-short"] {
        let path = acceptance::root().join(name).join("model.relca");
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("explain")
            .arg(&path)
            .output()
            .unwrap();
        let actual: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(0));
        fixture_match::Invocation::Explain
            .compare(&json!({}), &actual)
            .unwrap();
        assert_eq!(
            actual["responsibilities"],
            acceptance::check(&acceptance::source(name))["responsibilities"]
        );
        assert_eq!(
            actual["responsibilities"][0]["time_frame"],
            acceptance::expected(name)["responsibilities"][0]["time_frame"]
        );
        assert_eq!(actual["explanation"]["counterexamples"], json!([]));
    }
}

fn hand_wire() -> Value {
    json!({"agency":{},"properties":[],"contest_paths":[],"responsibilities":[{
        "id":1,"rules":"responsibility-p1-2","person":"p","target":{"kind":"maintenance","value":"Effect.maintenance"},"actors":["Judge"],
        "observation":{"status":"present","required_sources":["Record"],"direct":["Record"],"indirect":["Record"],"missing_sources":[],"reason":null},
        "refusal":{"status":"present","effects":[{"capability":"Effect.put","call":"out","status":"all_paths","judgements":[{"actor":"Judge","capability":"Judge.check","call":"j","stops":"all_paths","bypasses":0}]}],"reason":null},
        "time_frame":{"status":"satisfied","within":5,"required_duration":5,"basis":"static_capability_path_upper","scope":"static_responsibility_reachability","reason":null,"paths":[{"kind":"refusal","source":null,"judgement_call":"j","effect_call":"out","calls":["j","out"],"duration_upper":5}]},"warnings":[]
    }]})
}

#[test]
fn new_time_wire_is_closed_and_classifications_are_arithmetically_consistent() {
    let good = hand_wire();
    fixture_match::validate_responsibility_wire(&good).unwrap();
    let mut late = good.clone();
    late["responsibilities"][0]["time_frame"]["status"] = json!("violated");
    late["responsibilities"][0]["time_frame"]["within"] = json!(4);
    fixture_match::validate_responsibility_wire(&late).unwrap();
    let mut unknown = good.clone();
    unknown["responsibilities"][0]["time_frame"] = json!({"status":"unchecked","within":5,"required_duration":null,"basis":"static_capability_path_upper","scope":"static_responsibility_reachability","reason":"duration_unknown","paths":[]});
    fixture_match::validate_responsibility_wire(&unknown).unwrap();
    for pointer in [
        "/responsibilities/0/time_frame",
        "/responsibilities/0/time_frame/paths/0",
    ] {
        for key in good.pointer(pointer).unwrap().as_object().unwrap().keys() {
            let mut bad = good.clone();
            bad.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                fixture_match::validate_responsibility_wire(&bad).is_err(),
                "missing {pointer}/{key}"
            );
        }
        let mut bad = good.clone();
        bad.pointer_mut(pointer).unwrap()["unknown"] = json!(true);
        assert!(
            fixture_match::validate_responsibility_wire(&bad).is_err(),
            "extra key {pointer}"
        );
    }
    for (pointer, value) in [
        ("/responsibilities/0/rules", json!("responsibility-p1-3")),
        ("/responsibilities/0/rules", json!("responsibility-p1-1")),
        ("/responsibilities/0/target/kind", json!("actor")),
        ("/responsibilities/0/time_frame/status", json!("present")),
        ("/responsibilities/0/time_frame/status", json!("violated")),
        (
            "/responsibilities/0/time_frame/required_duration",
            Value::Null,
        ),
        ("/responsibilities/0/time_frame/within", json!(-1)),
        ("/responsibilities/0/time_frame/within", json!(5.5)),
        ("/responsibilities/0/time_frame/required_duration", json!(4)),
        ("/responsibilities/0/time_frame/reason", json!("unlimited")),
        ("/responsibilities/0/time_frame/basis", json!("simulation")),
        (
            "/responsibilities/0/time_frame/scope",
            json!("execution_sla"),
        ),
        ("/responsibilities/0/time_frame/paths", json!([])),
        ("/responsibilities/0/time_frame/paths/0/calls", json!([])),
        (
            "/responsibilities/0/time_frame/paths/0/calls",
            json!(["out", "j"]),
        ),
        (
            "/responsibilities/0/time_frame/paths/0/calls",
            json!(["j", "j", "out"]),
        ),
        (
            "/responsibilities/0/time_frame/paths/0/duration_upper",
            json!(4),
        ),
        (
            "/responsibilities/0/time_frame/paths/0/source",
            json!("Record"),
        ),
        (
            "/responsibilities/0/time_frame/paths/0/kind",
            json!("schedule"),
        ),
        (
            "/responsibilities/0/time_frame/paths/0/effect_call",
            Value::Null,
        ),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            fixture_match::validate_responsibility_wire(&bad).is_err(),
            "fault {pointer}"
        );
    }
    let mut duplicate = good.clone();
    let path = duplicate["responsibilities"][0]["time_frame"]["paths"][0].clone();
    duplicate["responsibilities"][0]["time_frame"]["paths"]
        .as_array_mut()
        .unwrap()
        .push(path);
    assert!(fixture_match::validate_responsibility_wire(&duplicate).is_err());
    for reason in [Value::Null, json!("arbitrary")] {
        let mut bad = unknown.clone();
        bad["responsibilities"][0]["time_frame"]["reason"] = reason;
        assert!(fixture_match::validate_responsibility_wire(&bad).is_err());
    }
    let mut old = good.clone();
    old["responsibilities"][0]["target"] = json!({"kind":"capability","value":"Judge.check"});
    old["responsibilities"][0]["rules"] = json!("responsibility-p1-1");
    old["responsibilities"][0]["time_frame"] = json!({"status":"unchecked"});
    fixture_match::validate_responsibility_wire(&old).unwrap();
    old["responsibilities"][0]["time_frame"]["within"] = Value::Null;
    assert!(
        fixture_match::validate_responsibility_wire(&old).is_err(),
        "no nullable extra keys on old rows"
    );
}

#[test]
fn every_wrapper_validates_nested_time_evidence() {
    let src = acceptance::source("resp2-within");
    let configured=src.replace("  bounds","  configuration A { cut Scope; process Flow }\n  configuration B { cut Scope; process Flow }\n  bounds");
    let d = serde_json::to_value(relca_core::diff::diff(&configured, "A", "B")).unwrap();
    fixture_match::compare_diff_report(&json!({}), &d).unwrap();
    for side in ["from", "to"] {
        assert_eq!(
            d[side]["report"]["responsibilities"][0]["time_frame"]["status"],
            "satisfied"
        );
        let mut bad = d.clone();
        bad[side]["report"]["responsibilities"][0]["time_frame"]["required_duration"] = Value::Null;
        assert!(fixture_match::compare_diff_report(&json!({}), &bad).is_err());
    }
    let c = serde_json::to_value(relca_core::compare::compare(
        &configured,
        &["A", "B"],
        relca_core::compare::Analysis::Exhaustive,
    ))
    .unwrap();
    fixture_match::compare_compare_report(&json!({}), &c).unwrap();
    for i in 0..2 {
        let mut bad = c.clone();
        bad["plans"][i]["report"]["responsibilities"][0]["time_frame"]["paths"] = json!([]);
        assert!(fixture_match::compare_compare_report(&json!({}), &bad).is_err());
    }
    let m = serde_json::to_value(relca_core::mutation::mutate(&src)).unwrap();
    fixture_match::validate_mutation_wire(&m).unwrap();
    let mut locations = vec!["/original".to_owned()];
    // MutationReport nests executed reports at mutations[i].mutants[j].report.
    for (i, kind) in m["mutations"].as_array().unwrap().iter().enumerate() {
        for (j, mutant) in kind["mutants"].as_array().unwrap().iter().enumerate() {
            if mutant["report"]["responsibilities"].is_array() {
                locations.push(format!("/mutations/{i}/mutants/{j}/report"));
            }
        }
    }
    assert!(
        locations.len() > 1,
        "mutant transport must actually be exercised"
    );
    for pointer in locations {
        let mut bad = m.clone();
        bad.pointer_mut(&pointer).unwrap()["responsibilities"][0]["time_frame"]["status"] =
            json!("accepted");
        assert!(
            fixture_match::validate_mutation_wire(&bad).is_err(),
            "{pointer}"
        );
    }
}
