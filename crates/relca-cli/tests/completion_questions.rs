//! ADR0025 binary, command nesting, and independent closed-wire controls.
#[path = "../../relca-core/tests/completion_questions.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn twenty_four_partial_expectations_are_closed_and_old_syntax_residuals_lower() {
    assert_eq!(acceptance::NAMES, fixture_match::COMPLETION_FIXTURES);
    for n in acceptance::NAMES {
        fixture_match::validate_expected(&acceptance::expected(n))
            .unwrap_or_else(|e| panic!("{n}: {e}"));
    }
}
#[test]
fn twenty_four_binary_reports_match_hand_arithmetic_and_exit() {
    for n in acceptance::NAMES {
        let e = acceptance::expected(n);
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(["check"])
            .arg(acceptance::root().join(n).join("model.relca"))
            .output()
            .unwrap();
        let a: Value = serde_json::from_slice(&out.stdout).unwrap();
        acceptance::assert_fixture_report(n, &a);
        acceptance::assert_hand_warnings(n, &a);
        fixture_match::compare_report(&e, &a).unwrap();
        let v: relca_core::Verdict = serde_json::from_value(e["verdict"].clone()).unwrap();
        assert_eq!(out.status.code(), Some(v.exit_code()), "{n}");
    }
}

#[test]
fn closed_completion_wire_controls_are_independent_of_production_dto() {
    let good = json!({"mode":"exhaustive","bounds":{"cases":1,"truncated":false},"diagnostics":[],"properties":[{"id":"recoverable end","verdict":"satisfied"},{"id":"terminating end","verdict":"satisfied"},{"id":"almost_sure end","verdict":"satisfied","probability":{"min":"1","max":"1"}}]});
    fixture_match::validate_completion_wire(&good).unwrap();
    for (pointer, value) in [
        ("/properties/0/id", json!("recoverable end.done")),
        ("/properties/0/verdict", json!("vacuous")),
        ("/properties/2/probability", Value::Null),
        ("/properties/2/probability/min", json!("2/2")),
        ("/properties/2/probability/min", json!("2")),
        ("/properties/2/probability/max", json!("0")),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            fixture_match::validate_completion_wire(&bad).is_err(),
            "{pointer}"
        );
    }
    let mut bad = good.clone();
    bad["properties"][2]
        .as_object_mut()
        .unwrap()
        .remove("probability");
    assert!(fixture_match::validate_completion_wire(&bad).is_err());
    for i in [0, 1] {
        let mut bad = good.clone();
        bad["properties"][i]["probability"] = json!({"min":"1","max":"1"});
        assert!(fixture_match::validate_completion_wire(&bad).is_err());
    }
    for words in [
        json!([]),
        json!(null),
        json!(["unreachable"]),
        json!(["completion_graph"]),
        json!(["agency"]),
        json!(["unreachable", "unreachable"]),
    ] {
        let mut bad = good.clone();
        bad["unchecked"] = words;
        assert!(fixture_match::validate_completion_wire(&bad).is_err());
    }
    let mut cut = good.clone();
    cut["bounds"]["truncated"] = json!(true);
    cut["unchecked"] = json!(["unreachable"]);
    fixture_match::validate_completion_wire(&cut).unwrap();
    let mut abort = cut.clone();
    abort["bounds"]["truncated"] = Value::Null;
    fixture_match::validate_completion_wire(&abort).unwrap();
    let warning = json!({"code":"W-UNREACH-0","severity":"warning","message":"Flow A.run never_started","span":{"start":2,"end":5}});
    let mut warned = good.clone();
    warned["diagnostics"] = json!([warning]);
    fixture_match::validate_completion_wire(&warned).unwrap();
    for (pointer, value) in [
        ("/diagnostics/0/span", Value::Null),
        ("/diagnostics/0/span/start", json!(5)),
        ("/diagnostics/0/severity", json!("error")),
        ("/diagnostics/0/code", json!("W-UNREACH-4")),
    ] {
        let mut bad = warned.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(fixture_match::validate_completion_wire(&bad).is_err());
    }
    cut["diagnostics"] = warned["diagnostics"].clone();
    assert!(fixture_match::validate_completion_wire(&cut).is_err());
    let mut sample = warned.clone();
    sample["mode"] = json!("simulation");
    sample["unchecked"] = json!(["unreachable"]);
    assert!(fixture_match::validate_completion_wire(&sample).is_err());
    let mut graph = good.clone();
    graph["unchecked"] = json!(["completion_graph"]);
    graph["properties"][0]["verdict"] = json!("unsupported");
    graph["diagnostics"] = json!([{"code":"U-SEM-0","severity":"error","message":"graph limit","span":{"start":1,"end":2}}]);
    fixture_match::validate_completion_wire(&graph).unwrap();

    let mut cases = good.clone();
    cases["bounds"]["cases"] = json!(2);
    cases["case_outcomes"] = json!([{"case_id":1,"arrival":0},{"case_id":2,"arrival":3}]);
    for pp in cases["properties"].as_array_mut().unwrap() {
        let prob = pp.as_object_mut().unwrap().remove("probability");
        pp["cases"] = json!([{"case_id":1,"arrival":0,"verdict":"satisfied"},{"case_id":2,"arrival":3,"verdict":"satisfied"}]);
        if let Some(prob) = prob {
            pp["cases"][0]["probability"] = prob.clone();
            pp["cases"][1]["probability"] = prob;
        }
    }
    fixture_match::validate_completion_wire(&cases).unwrap();
    for (pointer, value) in [
        ("/properties/2/cases/1/arrival", json!(0)),
        ("/properties/2/cases/1/case_id", json!(1)),
        ("/properties/2/cases/1/probability", Value::Null),
        (
            "/properties/0/cases/1/probability",
            json!({"min":"1","max":"1"}),
        ),
    ] {
        let mut bad = cases.clone();
        let field = bad.pointer_mut(pointer);
        if let Some(field) = field {
            *field = value;
        } else {
            bad["properties"][0]["cases"][1]["probability"] = value;
        }
        assert!(
            fixture_match::validate_completion_wire(&bad).is_err(),
            "cases {pointer}"
        );
    }

    let change = json!([{"check":"unchecked","key":"","field":"value","before":null,"after":["unreachable"],"direction":"changed"}]);
    fixture_match::validate_completion_changes(&change, false).unwrap();
    assert!(fixture_match::validate_completion_changes(&change, true).is_err());
    for (key, value) in [
        ("key", json!("/agency")),
        ("field", json!("words")),
        ("direction", json!("worsened")),
        ("after", json!([])),
        ("after", json!(["agency"])),
        ("after", json!(["unreachable", "unreachable"])),
    ] {
        let mut bad = change.clone();
        bad[0][key] = value;
        assert!(fixture_match::validate_completion_changes(&bad, false).is_err());
    }
}

#[test]
fn closed_single_and_arrivals_probability_presence_survives_every_nested_report() {
    let mut single = serde_json::to_value(relca_core::report::check(&acceptance::source(
        "done-call-kind",
    )))
    .unwrap();
    assert!(
        single["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "almost_sure end"),
        "completion parser/runtime required"
    );
    fixture_match::compare_report(&json!({}), &single).unwrap();
    let i = single["properties"]
        .as_array()
        .unwrap()
        .iter()
        .position(|p| p["id"] == "almost_sure end")
        .unwrap();
    single["properties"][i]
        .as_object_mut()
        .unwrap()
        .remove("probability");
    assert!(fixture_match::compare_report(&json!({}), &single).is_err());
    let mut arrivals = serde_json::to_value(relca_core::report::check(&acceptance::source(
        "done-arrivals",
    )))
    .unwrap();
    fixture_match::compare_report(&json!({}), &arrivals).unwrap();
    let i = arrivals["properties"]
        .as_array()
        .unwrap()
        .iter()
        .position(|p| p["id"] == "almost_sure end")
        .unwrap();
    arrivals["properties"][i]["cases"][1]
        .as_object_mut()
        .unwrap()
        .remove("probability");
    assert!(fixture_match::compare_report(&json!({}), &arrivals).is_err());
}

#[test]
fn check_explain_compare_diff_mutate_and_simulation_keep_completion_boundaries() {
    let src=acceptance::source("done-recovery.broken-zero-capacity").replace("  bounds","  cut C { include [Judge,Blocked] }\n  configuration A { cut C; process Flow }\n  configuration B { cut C; process Flow }\n  bounds");
    let file = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/completion-commands.relca");
    std::fs::write(&file, &src).unwrap();
    for (command, flags) in [
        ("check", vec!["--configuration", "A"]),
        // Preserve the original no-selector explain coverage using the
        // single-process source below; selected explain is tested separately.
        ("explain", vec![]),
        ("compare", vec!["--plans", "A,B", "--mode", "exhaustive"]),
        ("diff", vec!["--from", "A", "--to", "B"]),
    ] {
        let single = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/completion-commands-single.relca");
        std::fs::write(
            &single,
            acceptance::source("done-recovery.broken-zero-capacity"),
        )
        .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg(command)
            .arg(if command == "explain" { &single } else { &file })
            .args(flags)
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        let reports: Vec<&Value> = match command {
            "compare" => v["plans"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| &p["report"])
                .collect(),
            "diff" => vec![&v["from"]["report"], &v["to"]["report"]],
            _ => vec![&v],
        };
        for r in reports {
            for (id, verdict) in [
                ("recoverable end", "violated"),
                ("terminating end", "violated"),
                ("almost_sure end", "violated"),
            ] {
                let p = r["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["id"] == id)
                    .unwrap_or_else(|| panic!("{command} missing {id}"));
                assert_eq!(p["verdict"], verdict);
            }
            fixture_match::validate_completion_wire(r).unwrap();
        }
        if command == "explain" {
            for (id, reason) in [
                ("recoverable end", "non_path_property"),
                ("terminating end", "non_path_property"),
                ("almost_sure end", "probabilistic_property"),
            ] {
                assert!(
                    v["explanation"]["omitted"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|p| p["id"] == id && p["reason"] == reason),
                    "{v}"
                );
            }
        }
    }
    let m = serde_json::to_value(relca_core::mutation::mutate(&acceptance::source(
        "done-vacuity",
    )))
    .unwrap();
    fixture_match::validate_mutation_wire(&m).unwrap();
    assert!(
        m["original"]["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "almost_sure end")
    );
    for kind in m["mutations"].as_array().unwrap() {
        for mutant in kind["mutants"].as_array().unwrap() {
            if mutant["report"].is_object() {
                fixture_match::validate_completion_wire(&mutant["report"]).unwrap();
            }
        }
    }
}
