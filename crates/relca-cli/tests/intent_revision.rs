//! Round 1 acceptance: expectations are from the adjacent fixture calculations,
//! not from captured stdout. The frozen first-round acceptance stays untouched.
use serde_json::{Value, json};
use std::{path::Path, process::Command};
#[path = "support/intent_snapshot_oracle.rs"]
mod hand;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn model(name: &str) -> String {
    std::fs::read_to_string(root().join("fixtures").join(name).join("model.relca")).unwrap()
}

fn diff(source: &str, from: &str, to: &str) -> Value {
    serde_json::to_value(relca_core::diff::diff(source, from, to)).unwrap()
}

fn warnings(v: &Value, expected: [usize; 3]) {
    let ds = v["diagnostics"].as_array().unwrap();
    assert_eq!(
        [1, 2, 3].map(|n| ds
            .iter()
            .filter(|d| d["code"] == format!("W-DIFF-{n}"))
            .count()),
        expected
    );
}

fn no_positions(v: &mut Value) {
    for side in ["from", "to"] {
        v[side]["report"]["model"]["sha256"] = Value::Null;
    }
    for declaration in v["replacements"].as_array_mut().unwrap() {
        declaration["span"] = Value::Null;
    }
}

#[test]
fn jointly_sufficient_redundant_replacements_cover_the_gap_in_any_order() {
    let source = model("intent-gap");
    let value = diff(&source, "A", "B");
    assert_eq!(value["verdict"], "satisfied");
    assert_eq!(value["differences"].as_array().unwrap().len(), 23);
    let doc = value["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "visibility" && r["target"] == "Doc")
        .unwrap();
    assert_eq!(doc["from"], json!([]));
    assert_eq!(doc["to"], json!(["X", "Y"]));
    assert_eq!(
        doc["covered_by"],
        json!([
            {"intent":"replace","from":"O.o1","to":"X.a","via":"derived"},
            {"intent":"replace","from":"O.o2","to":"X.b","via":"derived"},
            {"intent":"replace","from":"O.o3","to":"Y.a","via":"derived"},
            {"intent":"replace","from":"O.o4","to":"Y.b","via":"derived"}
        ])
    );
    for declaration in value["replacements"].as_array().unwrap() {
        assert_eq!(declaration["covers"].as_array().unwrap().len(), 9);
    }
    warnings(&value, [0, 0, 0]);
    let lines: Vec<_> = source
        .lines()
        .filter(|l| l.trim_start().starts_with("replace "))
        .collect();
    let mut reversed = lines.iter().rev();
    let changed = source
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("replace ") {
                *reversed.next().unwrap()
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut a = value;
    let mut b = diff(&changed, "A", "B");
    no_positions(&mut a);
    no_positions(&mut b);
    assert_eq!(a, b);
}

#[test]
fn partner_guidance_is_only_for_unmatched_intent_claims() {
    let source = model("intent-legacy");
    let v = diff(&source, "C", "B");
    let errors = |v: &Value| {
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "E-DIFF-0")
            .cloned()
            .collect::<Vec<_>>()
    };
    let ds = errors(&v);
    assert_eq!(ds.len(), 1);
    assert!(
        ds[0]["message"]
            .as_str()
            .unwrap()
            .contains("intent from C to B")
    );
    let preserve = source.replace("retire Clerk reason \"x\" loses \"y\"", "preserve Controls");
    let v = diff(&preserve, "C", "B");
    let ds = errors(&v);
    assert_eq!(ds.len(), 1);
    assert!(
        ds[0]["message"]
            .as_str()
            .unwrap()
            .contains("outside destination configuration scope")
    );
    assert!(!ds[0]["message"].as_str().unwrap().contains("intent from"));
    let mixed = source.replace(
        "retire Clerk reason \"x\" loses \"y\"",
        "retire Clerk reason \"x\" loses \"y\"; preserve Controls",
    );
    let ds = errors(&diff(&mixed, "C", "B"));
    assert_eq!(ds.len(), 2);
    assert_eq!(
        ds.iter()
            .filter(|d| d["message"]
                .as_str()
                .unwrap()
                .contains("intent from C to B"))
            .count(),
        1
    );
    let two = source
        .replace("  configuration A { cut K; process PA }\n", "")
        .replace("applies_to [A]", "applies_to [C]");
    let ds = errors(&diff(&two, "C", "B"));
    assert_eq!(ds.len(), 1);
    assert!(!ds[0]["message"].as_str().unwrap().contains("intent from"));
    let replacement = source.replace(
        "retire Clerk reason \"x\" loses \"y\"",
        "replace Clerk.check with Bot.check reason \"x\" loses \"y\"",
    );
    let ds = errors(&diff(&replacement, "C", "B"));
    assert_eq!(ds.len(), 1);
    assert!(
        ds[0]["message"]
            .as_str()
            .unwrap()
            .contains("intent from C to B")
    );
    let (body, _) = replacement.rsplit_once('}').unwrap();
    let paired = format!(
        "{body} intent from C to B {{ replace Clerk.check with Bot.check reason \"x\" loses \"y\" }} }}"
    );
    let ds = errors(&diff(&paired, "C", "B"));
    assert_eq!(ds.len(), 1);
    assert!(!ds[0]["message"].as_str().unwrap().contains("intent from"));
}

#[test]
fn split_call_sites_leave_unexplained_readers_and_producers_uncovered() {
    let value = diff(&model("intent-split"), "A", "B");
    assert_eq!(value["verdict"], "satisfied");
    assert_eq!(value["differences"].as_array().unwrap().len(), 15);
    let uncovered: Vec<_> = value["differences"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r.get("covered_by").is_none())
        .map(|r| (r["kind"].as_str().unwrap(), r["target"].as_str().unwrap()))
        .collect();
    assert_eq!(
        uncovered,
        [
            ("actor", "Audit"),
            ("relation", "Audit.log"),
            ("authority", "Audit.log"),
            ("resource", "Audit"),
            ("visibility", "Doc"),
            ("relation", "w")
        ]
    );
    assert_eq!(
        value["replacements"][0]["covers"].as_array().unwrap().len(),
        9
    );
    warnings(&value, [6, 0, 6]);
}

#[test]
fn review_gap_reaches_the_real_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args([
            "diff",
            "fixtures/intent-gap/model.relca",
            "--from",
            "A",
            "--to",
            "B",
        ])
        .current_dir(root())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    warnings(&value, [0, 0, 0]);
}

#[test]
fn closed_fixture_population_matches_hand_values_and_rejects_corrupt_rows() {
    let mut found: Vec<_> = std::fs::read_dir(root().join("fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("intent-"))
        .collect();
    found.sort();
    let mut expected = hand::NAMES.map(str::to_owned).to_vec();
    expected.sort();
    assert_eq!(found, expected);
    assert!(
        std::panic::catch_unwind(|| relca_test_support::fixture_match::intent_fixture(
            "intent-unknown"
        ))
        .is_err()
    );
    let jobs: Vec<_> = hand::NAMES
        .iter()
        .map(|n| json!({"id":format!("fixture-{n}")}))
        .collect();
    hand::validate_set(&jobs).unwrap();
    for name in hand::NAMES {
        let directory = root().join("fixtures").join(name);
        let invocation: Value =
            serde_json::from_slice(&std::fs::read(directory.join("invocation.json")).unwrap())
                .unwrap();
        let args = vec![
            "diff".to_owned(),
            format!("fixtures/{name}/model.relca"),
            "--from".into(),
            invocation["from"].as_str().unwrap().into(),
            "--to".into(),
            "B".into(),
        ];
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .current_dir(root())
            .output()
            .unwrap();
        let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
        hand::check(
            &format!("fixture-{name}"),
            &args,
            &doc,
            out.status.code().unwrap(),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected: Value =
            serde_json::from_slice(&std::fs::read(directory.join("expected.json")).unwrap())
                .unwrap();
        relca_test_support::fixture_match::compare_diff_report(&expected, &doc)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut corrupt = doc;
        corrupt["differences"][0]["target"] = json!("Invented");
        assert!(
            hand::check(
                &format!("fixture-{name}"),
                &args,
                &corrupt,
                out.status.code().unwrap()
            )
            .is_err(),
            "{name}: lost hand inventory check"
        );
    }
}
