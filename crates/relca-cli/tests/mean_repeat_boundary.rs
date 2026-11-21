//! The mean computed from the expanded process must reach each public consumer.
#[path = "../src/fixture_match.rs"]
mod wire;
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

#[test]
fn repeat_mean_matches_unrolled_in_compare_diff_and_sensitivity() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let work = root
        .join(".tmp")
        .join(format!("mean-repeat-{}", std::process::id()));
    fs::create_dir_all(&work).unwrap();
    let model = work.join("model.relca");
    let source = include_str!("../../../fixtures/mean-repeat/model.relca");
    fs::write(&model, source).unwrap();
    let model = model.to_str().unwrap();
    let run = |argv: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_relca"))
            .current_dir(&root)
            .args(argv)
            .output()
            .unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{argv:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(doc["verdict"], "satisfied", "{argv:?}");
        doc
    };
    let expected: Value =
        serde_json::from_str(include_str!("../../../fixtures/mean-repeat/expected.json")).unwrap();
    let compare_report = |report: &Value| {
        wire::Invocation::parse(None)
            .unwrap()
            .compare(&expected, report)
            .unwrap();
    };
    let exhaustive = run(&[
        "compare",
        model,
        "--plans",
        "Loop,Hand",
        "--mode",
        "exhaustive",
    ]);
    wire::validate_compare_wire(&exhaustive).unwrap();
    for (i, plan) in ["Loop", "Hand"].into_iter().enumerate() {
        compare_report(&exhaustive["plans"][i]["report"]);
        assert_eq!(
            exhaustive["evaluation"]["hard"][0]["values"][i],
            json!({"configuration":plan,"verdict":"satisfied","basis":"exact","interval":{"lower":"9/2","upper":"9/2"},"reason":null,"source_reason":null})
        );
        assert_eq!(
            exhaustive["evaluation"]["objectives"][0]["values"][i],
            json!({"configuration":plan,"lower":"15/2","upper":"15/2","basis":"exact","reason":null})
        );
    }
    let diff = run(&["diff", model, "--from", "Loop", "--to", "Hand"]);
    for side in ["from", "to"] {
        compare_report(&diff[side]["report"]);
    }
    for configuration in ["Loop", "Hand"] {
        // Separate suites: rewriting control flow is not a parameter variation.
        let manifest = work.join(format!("{configuration}.json"));
        fs::write(&manifest, json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"one fixed process for mean validation"},"points":[{"id":"point","model":"model.relca","configuration":configuration,"scenario":null}],"targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":null}).to_string()).unwrap();
        let sensitivity = run(&[
            "sensitivity",
            manifest.to_str().unwrap(),
            "--mode",
            "exhaustive",
        ]);
        assert_eq!(sensitivity["scope"]["evaluated"], 1);
        compare_report(&sensitivity["points"][0]["report"]);
    }
}

#[test]
fn repeat_mean_mutant_matches_the_same_edit_in_every_written_round() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = "fixtures/mean-repeat/model.relca";
    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap();
        (
            out.status.code(),
            serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        )
    };
    let mutate = |plan| {
        let (exit, doc) = run(&[
            "mutate",
            fixture,
            "--configuration",
            plan,
            "--kinds",
            "hold_delayed",
        ]);
        assert_eq!(doc["original"]["verdict"], "satisfied", "{plan}: {doc}");
        assert!(
            doc["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x == "metric_constraints")
        );
        assert!(doc["original"]["metric_checks"].is_null());
        (exit, doc)
    };
    let _hand = mutate("Hand");
    let (exit, looped) = mutate("Loop");
    assert_eq!((exit, &looped["verdict"]), (Some(1), &json!("violated")));
    let kind = looped["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "hold_delayed")
        .unwrap();
    assert_eq!(kind["candidates"], 1);
    let [edit] = kind["mutants"].as_array().unwrap().as_slice() else {
        panic!("{kind}")
    };
    assert_eq!(edit["site"]["statement_path"], "body/0/repeat/1");
    assert_eq!(edit["site"]["retained_branch"], "else");
    assert_eq!(edit["result"], "undetected");
    assert_eq!(edit["reason"], "no_assigned_check");
    assert_eq!(edit["assigned_changes"], json!([]));
    let mutant = &edit["report"];
    assert!(mutant["metric_checks"].is_null());

    // Hand's automatic mutant edits only one guard. Independently replace
    // both guards to match the single edit to the repeat body. The trailing
    // End keeps the lexical name used by verify without making it reachable.
    let source = include_str!("../../../fixtures/mean-repeat/model.relca");
    let (head, rest) = source.split_once("  process Unrolled {").unwrap();
    let (_, tail) = rest.split_once("  cut C").unwrap();
    let written = format!(
        "{head}  process Unrolled {{ r1 = A.run(); wait 3min; r2 = A.run(); end escalated; end done }}\n  cut C{tail}"
    );
    let work = root
        .join(".tmp")
        .join(format!("mean-repeat-mutant-{}", std::process::id()));
    fs::create_dir_all(&work).unwrap();
    let file = work.join("all-keep-else.relca");
    fs::write(&file, written).unwrap();
    let (exit, hand) = run(&["check", file.to_str().unwrap(), "--configuration", "Hand"]);
    assert_eq!(exit, Some(1));
    assert_eq!(mutant["verdict"], hand["verdict"]);
    assert_eq!(mutant["properties"], hand["properties"]);
    let probability = hand["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "probability reached #1")
        .unwrap();
    assert_eq!(probability["probability"], json!({"min":"0","max":"0"}));

    // Both calls always run: elapsed = 2+3+2, work = 2+2,
    // cost = 2*(2*2+1). No case reaches done.
    assert_eq!(mutant["quality"]["means"], hand["quality"]["means"]);
    let means = mutant["quality"]["means"].as_array().unwrap();
    assert_eq!(means.len(), 4);
    for (name, value) in [("Lead", "7"), ("Work", "4"), ("Cost", "10")] {
        let row = means.iter().find(|r| r["name"] == name).unwrap();
        assert_eq!(
            row["value"],
            json!({"lower":value,"upper":value,"basis":"exact","reason":null})
        );
    }
    let reached = means.iter().find(|r| r["name"] == "DoneLead").unwrap();
    assert_eq!(
        reached["value"],
        json!({"lower":null,"upper":null,"basis":null,"reason":"zero_denominator"})
    );
}

#[test]
fn nonrepeat_metric_verify_stays_unchecked_during_mutation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Use the same input and invocation as main's metric-check-mutate golden.
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .current_dir(root)
        .args(["mutate", "fixtures/mean-metric-check/model.relca"])
        .output()
        .unwrap();
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(4), "{doc}");
    assert!(
        doc["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "metric_constraints")
    );
    assert!(doc["original"]["metric_checks"].is_null());
    assert!(doc["original"]["repetition"].is_null());
    assert!(
        doc["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["status"] == "generated")
    );
}
