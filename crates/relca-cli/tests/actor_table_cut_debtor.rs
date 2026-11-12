//! Real binary, fixed hand oracles and independent closed-wire calibration.
#[path = "../../relca-core/tests/actor_table_cut_debtor.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn all_28_hand_expectations_validate_without_using_new_production_output() {
    assert_eq!(acceptance::NAMES, fixture_match::ACTOR_TABLE_FIXTURES);
    for name in acceptance::NAMES {
        fixture_match::validate_expected(&acceptance::expected(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}
#[test]
fn all_28_real_binary_reports_have_hand_values_not_self_selected_exit_codes() {
    for name in acceptance::NAMES {
        let hand = acceptance::expected(name);
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(["check"])
            .arg(acceptance::root().join(name).join("model.relca"))
            .output()
            .unwrap();
        let actual: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            actual["verdict"], hand["verdict"],
            "hand verdict {name}: {actual}"
        );
        acceptance::subset(&hand, &actual);
        acceptance::assert_context(&actual);
        fixture_match::compare_report(&hand, &actual).unwrap_or_else(|e| panic!("{name}: {e}"));
        let verdict: relca_core::Verdict = serde_json::from_value(hand["verdict"].clone()).unwrap();
        assert_eq!(out.status.code(), Some(verdict.exit_code()), "{name}");
    }
}

fn field(value: Value, status: &str) -> Value {
    json!({"value":value,"status":status})
}
fn hand_actor() -> Value {
    json!({"name":"A",
        "boundary":field(json!("one actor"),"declared"),
        "grain":field(json!("model"),"declared"),
        "contains":field(json!([]),"declared"),
        "overlaps":field(json!([]),"declared"),
        "capability":field(json!(["run"]),"declared"),
        "capacity":field(json!(1),"default"),
        "executors":field(json!({"kind":"named","ids":["p"]}),"declared"),
        "control":field(json!("policy"),"declared"),
        "source":field(json!("document"),"declared"),
        "via_model":field(json!("doc@v1"),"declared"),
        "state":field(Value::Null,"default"),
        "latent":field(json!(["A.quality"]),"declared")})
}
fn hand_wire() -> Value {
    json!({"actors":[hand_actor()],"cut":{"reentry":["review cut quarterly"]},
        "cut_context":{"notices":acceptance::NOTICES,"reentry":["review cut quarterly"],"horizon":[{"from":"start","to":"finish","within":60,"structural_only":false}]},
        "via_model_paths":{"rules":"via-model-read-paths-1","basis":"追跡であって、AI を介した変化の認定ではない","scope":"selected_process_lexical_reads","truncated":false,"path_count_lower_bound":2,
            "paths":[{"actor":"A","model":"doc@v1","start_call":"a","calls":[{"call":"a","capability":"A.run"}],"target":{"kind":"U","call":"a","effect_call":"posted","stops":"all_paths"}},{"actor":"A","model":"doc@v1","start_call":"a","calls":[{"call":"a","capability":"A.run"},{"call":"posted","capability":"Ledger.post"}],"target":{"kind":"E","call":"posted"}}],
            "unchecked":["transformation_recognition","state_guard_control_paths","contest_paths","fallback_paths"]}})
}

#[test]
fn k18_closed_metadata_single_missing_extra_enum_null_identity_and_order_faults() {
    let good = hand_wire();
    fixture_match::validate_actor_context_wire(&good).unwrap();
    for pointer in [
        "",
        "/actors/0",
        "/actors/0/boundary",
        "/actors/0/grain",
        "/actors/0/contains",
        "/actors/0/overlaps",
        "/actors/0/capability",
        "/actors/0/capacity",
        "/actors/0/executors",
        "/actors/0/executors/value",
        "/actors/0/control",
        "/actors/0/source",
        "/actors/0/via_model",
        "/actors/0/state",
        "/actors/0/latent",
        "/cut_context",
        "/cut_context/horizon/0",
        "/via_model_paths",
        "/via_model_paths/paths/0",
        "/via_model_paths/paths/0/calls/0",
        "/via_model_paths/paths/0/target",
        "/via_model_paths/paths/1/target",
    ] {
        let keys: Vec<_> = good
            .pointer(pointer)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        for key in keys {
            if pointer.is_empty() && key == "cut" {
                continue;
            }
            let mut bad = good.clone();
            bad.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(&key);
            assert!(
                fixture_match::validate_actor_context_wire(&bad).is_err(),
                "missing {pointer}/{key}"
            );
        }
        // Root has other report fields, but all objects OWNED by the new wire
        // are closed. The ordinary Report comparator closes the root.
        if !pointer.is_empty() {
            let mut bad = good.clone();
            bad.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("surplus".into(), json!(true));
            assert!(
                fixture_match::validate_actor_context_wire(&bad).is_err(),
                "extra {pointer}"
            );
        }
    }
    for (pointer, value) in [
        ("/actors", json!({})),
        ("/actors/0/name", json!("")),
        ("/actors/0/grain/status", json!("inferred")),
        ("/actors/0/grain/value", Value::Null),
        ("/actors/0/contains/status", json!("default")),
        ("/actors/0/capacity/status", json!("declared")),
        ("/actors/0/control/value", json!("human")),
        ("/actors/0/executors/value/ids", json!(["p", "p"])),
        ("/actors/0/via_model/value", json!("bad")),
        ("/actors/0/state/value", json!("pretend")),
        (
            "/cut_context/notices",
            json!([acceptance::NOTICES[1], acceptance::NOTICES[0]]),
        ),
        ("/cut_context/reentry", json!([])),
        ("/cut_context/horizon/0/structural_only", json!("true")),
        ("/via_model_paths/rules", json!("new-rules")),
        ("/via_model_paths/path_count_lower_bound", json!(1)),
        ("/via_model_paths/truncated", json!(true)),
        ("/via_model_paths/unchecked", json!(["claimed_checked"])),
        ("/via_model_paths/paths/0/start_call", json!("different")),
        ("/via_model_paths/paths/0/actor", json!("AI")),
        ("/via_model_paths/paths/0/target/stops", json!("none")),
        ("/via_model_paths/paths/1/target/kind", json!("J")),
    ] {
        // Declared capacity=1 IS a legitimate alternate wire. Change it to
        // null as the single fault instead; default-vs-declared needs input
        // evidence (F8), not a wire-only guess about declaration provenance.
        let (pointer, value) = if pointer == "/actors/0/capacity/status" {
            ("/actors/0/capacity/value", Value::Null)
        } else {
            (pointer, value)
        };
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            fixture_match::validate_actor_context_wire(&bad).is_err(),
            "fault {pointer}"
        );
    }
    for pointer in ["/actors", "/cut_context/horizon", "/via_model_paths/paths"] {
        let mut bad = good.clone();
        let rows = bad.pointer_mut(pointer).unwrap().as_array_mut().unwrap();
        rows.push(rows[0].clone());
        assert!(
            fixture_match::validate_actor_context_wire(&bad).is_err(),
            "duplicate {pointer}"
        );
    }
    let mut truncated = good.clone();
    truncated["via_model_paths"]["truncated"] = json!(true);
    truncated["via_model_paths"]["path_count_lower_bound"] = json!(4097);
    truncated["via_model_paths"]["unchecked"]
        .as_array_mut()
        .unwrap()
        .push(json!("path_enumeration_limit"));
    fixture_match::validate_actor_context_wire(&truncated).unwrap();
    let mut empty = good;
    empty["actors"] = Value::Null;
    empty["cut"] = json!("unresolved");
    empty["cut_context"]["reentry"] = json!([]);
    empty["cut_context"]["horizon"] = json!([]);
    empty.as_object_mut().unwrap().remove("via_model_paths");
    fixture_match::validate_actor_context_wire(&empty).unwrap();
}

#[test]
fn every_report_containing_command_including_errors_has_mandatory_metadata() {
    let src = acceptance::source("act-cut-context");
    let report = acceptance::check(&src);
    fixture_match::compare_report(&json!({}), &report).unwrap();
    let explain = serde_json::to_value(relca_core::explain::explain(&src)).unwrap();
    acceptance::assert_context(&explain);
    let sim = serde_json::to_value(relca_core::report::simulate(&src)).unwrap();
    acceptance::assert_context(&sim);
    fixture_match::validate_simulation_wire(&sim).unwrap();
    let configs=src.replace("  bounds","  configuration X { cut C; process Flow }\n  configuration Y { cut C; process Flow }\n  bounds");
    let diff = serde_json::to_value(relca_core::diff::diff(&configs, "X", "Y")).unwrap();
    fixture_match::compare_diff_report(&json!({}), &diff).unwrap();
    for side in ["from", "to"] {
        let mut bad = diff.clone();
        bad[side]["report"]
            .as_object_mut()
            .unwrap()
            .remove("actors");
        assert!(fixture_match::compare_diff_report(&json!({}), &bad).is_err());
    }
    let compare = serde_json::to_value(relca_core::compare::compare(
        &configs,
        &["Y", "X"],
        relca_core::compare::Analysis::Exhaustive,
    ))
    .unwrap();
    fixture_match::compare_compare_report(&json!({}), &compare).unwrap();
    for i in 0..2 {
        let mut bad = compare.clone();
        bad["plans"][i]["report"]
            .as_object_mut()
            .unwrap()
            .remove("cut_context");
        assert!(fixture_match::compare_compare_report(&json!({}), &bad).is_err());
    }
    let mutation = serde_json::to_value(relca_core::mutation::mutate(&src)).unwrap();
    fixture_match::validate_mutation_wire(&mutation).unwrap();
    let mut bad = mutation;
    bad["original"]
        .as_object_mut()
        .unwrap()
        .remove("cut_context");
    assert!(fixture_match::validate_mutation_wire(&bad).is_err());
    for v in [
        acceptance::check("invalid"),
        serde_json::to_value(relca_core::report::simulation_input_failure("unreadable")).unwrap(),
        serde_json::to_value(relca_core::explain::input_failure("unreadable")).unwrap(),
    ] {
        acceptance::assert_context(&v);
        assert_eq!(v["actors"], Value::Null);
    }
}

#[test]
fn real_cli_read_usage_and_simulation_refusal_paths_never_omit_context() {
    let root = acceptance::root().parent().unwrap().to_path_buf();
    let absent = ".tmp/actor-acceptance-file-that-does-not-exist.relca";
    assert!(!root.join(absent).exists());
    for args in [
        vec!["check", absent],
        vec!["check"],
        vec!["simulate", absent],
        vec!["simulate"],
        vec!["explain", absent],
        vec!["explain"],
        vec!["simulate", "fixtures/ctx-contest/model.relca"],
        vec!["diff", absent, "--from", "X", "--to", "Y"],
        vec!["compare", absent, "--plans", "X,Y", "--mode", "exhaustive"],
        vec!["compare", absent, "--plans", "X,Y", "--mode", "simulation"],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .current_dir(&root)
            .output()
            .unwrap();
        assert_ne!(
            out.status.code(),
            Some(0),
            "error/refusal cannot succeed {args:?}"
        );
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        match args[0] {
            "diff" => {
                for side in ["from", "to"] {
                    acceptance::assert_context(&v[side]["report"]);
                    assert!(v[side]["report"].get("actors").is_some());
                }
            }
            "compare" => {
                for plan in v["plans"].as_array().unwrap() {
                    acceptance::assert_context(&plan["report"]);
                    assert!(plan["report"].get("actors").is_some());
                }
            }
            _ => {
                acceptance::assert_context(&v);
                assert!(v.get("actors").is_some());
            }
        }
    }
}
