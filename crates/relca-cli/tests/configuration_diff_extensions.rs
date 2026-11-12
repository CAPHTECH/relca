//! New wire literals are checked without waiting for production Serde to exist.
#[path = "../../relca-core/tests/support/configuration_diff_inputs.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "support/diff_oracle.rs"]
mod oracle;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

const PRODUCER_LIMITATIONS: [&str; 2] = ["relation_state_producers", "relation_binding_identity"];

fn original_producer_limitations(v: &Value) -> Vec<Value> {
    let words = PRODUCER_LIMITATIONS.map(Value::from);
    let completed = ["relation", "annotations"].iter().all(|word| {
        v["checked"]
            .as_array()
            .is_some_and(|checked| checked.iter().any(|w| w == word))
    });
    if completed
        && v["unchecked"]
            .as_array()
            .is_some_and(|unchecked| unchecked.ends_with(&words))
    {
        words.to_vec()
    } else {
        vec![]
    }
}

fn upgraded(name: &str) -> Value {
    // This is a wire validator scaffold, NOT a source of semantic expected
    // values. Side reports are old existing wire; new cells are hand/old-IR.
    let src = acceptance::stripped(&acceptance::source(name));
    let original = serde_json::to_value(relca_core::diff::diff(&src, "From", "To")).unwrap();
    let limitations = original_producer_limitations(&original);
    let mut v = old_projection(original).unwrap();
    let (status, rows) = oracle::rows(&src, "From", "To");
    assert_eq!(status, "compared");
    v["differences"].as_array_mut().unwrap().extend(rows);
    let old = v["checked"].as_array().unwrap().clone();
    let mut checked = old;
    checked.splice(5..5, oracle::NEW_CHECKED.into_iter().map(|s| json!(s)));
    v["checked"] = json!(checked);
    v["unchecked"]
        .as_array_mut()
        .unwrap()
        .retain(|s| !oracle::REMOVE_UNCHECKED.iter().any(|w| s == w));
    if name == "diff2-preserve" {
        v["preservation"] = acceptance::expected(name)["preservation"].clone();
        v["differences"][0]["annotations"] = json!([{"kind":"preserve","selector":"Ctl","global":true,"properties":["contract Ctl #1 at_most_once"]}]);
    }
    if name == "diff2-override" {
        v["differences"][1]["annotations"] =
            acceptance::expected(name)["differences"][1]["annotations"].clone();
    }
    if name == "diff2-maintain" {
        v["differences"][0]["annotations"] =
            acceptance::expected(name)["differences"][0]["annotations"].clone();
        v["differences"][0]["reason"] = json!("保守");
    }
    // Restore only the ordered pair removed from an originally completed diff;
    // the legacy projection itself must still reject missing/partial markers.
    v["unchecked"].as_array_mut().unwrap().extend(limitations);
    v
}
/// Keep only the old wire of a production report (actor and relation rows
/// without annotations, old checked words, no preservation), so the scaffold
/// adds the independent oracle rows exactly once whether or not production
/// already emits the new cells.
fn old_projection(mut v: Value) -> Result<Value, String> {
    // ADR0051's two limitations are an indivisible, ordered suffix. Never
    // repair a partial/duplicated marker or accept an unknown word by erasure.
    let words = PRODUCER_LIMITATIONS;
    let unchecked = v["unchecked"].as_array().ok_or("unchecked array")?;
    let counts = words.map(|word| unchecked.iter().filter(|w| *w == word).count());
    match counts {
        [0, 0] => {}
        [1, 1] if unchecked.ends_with(&words.map(|w| json!(w))) => {}
        _ => return Err("incomplete or duplicated producer limitations".into()),
    }
    fixture_match::compare_diff_report(&json!({}), &v)?;
    if counts == [1, 1] {
        let unchecked = v["unchecked"].as_array_mut().unwrap();
        unchecked.truncate(unchecked.len() - 2);
    }
    let rows = v["differences"].as_array_mut().unwrap();
    rows.retain(|r| r["kind"] == "actor" || r["kind"] == "relation");
    for r in rows.iter_mut() {
        r.as_object_mut().unwrap().remove("annotations");
    }
    v["checked"]
        .as_array_mut()
        .unwrap()
        .retain(|s| !oracle::NEW_CHECKED.iter().any(|w| s == w));
    v.as_object_mut().unwrap().remove("preservation");
    Ok(v)
}
fn valid(v: &Value) -> Result<(), String> {
    fixture_match::compare_diff_report(&json!({}), v)
}

#[test]
fn old_scaffold_projection_requires_the_closed_producer_word_pair() {
    let src = acceptance::source("diff2-time");
    let good = serde_json::to_value(relca_core::diff::diff(&src, "From", "To")).unwrap();
    old_projection(good.clone()).unwrap();
    let mut partial = good.clone();
    partial["unchecked"].as_array_mut().unwrap().pop();
    assert!(old_projection(partial).is_err());
    let mut unknown = good.clone();
    unknown["unchecked"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!("unknown_word"));
    assert!(valid(&unknown).is_err());
    assert!(old_projection(unknown).is_err());
    let mut duplicate = good;
    duplicate["unchecked"]
        .as_array_mut()
        .unwrap()
        .push(json!("relation_binding_identity"));
    assert!(old_projection(duplicate).is_err());
}

#[test]
fn scaffold_restores_only_original_completed_limitations_and_rejects_fake_upgrade() {
    let src = acceptance::source("diff2-time");
    let completed = serde_json::to_value(relca_core::diff::diff(&src, "From", "To")).unwrap();
    let words = PRODUCER_LIMITATIONS.map(Value::from);
    assert_eq!(original_producer_limitations(&completed), words);
    let mut absent = completed.clone();
    absent["unchecked"] = json!([]);
    assert!(original_producer_limitations(&absent).is_empty());
    assert!(old_projection(absent).is_err());
    let restored = upgraded("diff2-time");
    assert!(restored["unchecked"].as_array().unwrap().ends_with(&words));
    valid(&restored).unwrap();

    let incomplete = serde_json::to_value(relca_core::diff::diff(&src, "From", "Missing")).unwrap();
    valid(&incomplete).unwrap();
    assert!(original_producer_limitations(&incomplete).is_empty());
    old_projection(incomplete.clone()).unwrap();
    let mut fake_upgraded = incomplete;
    fake_upgraded["unchecked"]
        .as_array_mut()
        .unwrap()
        .extend(words);
    assert!(valid(&fake_upgraded).is_err());
    assert!(original_producer_limitations(&fake_upgraded).is_empty());
    assert!(old_projection(fake_upgraded).is_err());
}

#[test]
fn closed_wire_literals_accept_six_kinds_and_three_annotations_independently() {
    for name in [
        "diff2-time",
        "diff2-resource",
        "diff2-meaning",
        "diff2-authority",
        "diff2-visibility",
        "diff2-contest",
        "diff2-preserve",
        "diff2-override",
        "diff2-maintain",
    ] {
        let v = upgraded(name);
        valid(&v).unwrap_or_else(|e| panic!("{name}: {e}\n{v}"));
    }
    // A partial expectation does not require actual-only fields. Wrong fields
    // and wrong types remain forbidden even when no actual output is supplied.
    fixture_match::validate_diff_expected(&json!({"differences":[{"kind":"time","target":"A.a"}]}))
        .unwrap();
    for expected in [
        json!({"differences":[{"kind":"relation","target":"A.a","from":null}]}),
        json!({"differences":[{"kind":"time","target":"A.a","from":null}]}),
        json!({"preservation":{"summary":{"made_up":1}}}),
    ] {
        assert!(fixture_match::validate_diff_expected(&expected).is_err());
    }
}

#[test]
fn malformed_closed_wire_matrix_never_relies_on_production_deserialization() {
    let bases = [
        "diff2-time",
        "diff2-resource",
        "diff2-meaning",
        "diff2-authority",
        "diff2-visibility",
        "diff2-contest",
        "diff2-preserve",
        "diff2-override",
        "diff2-maintain",
    ];
    for name in bases {
        valid(&upgraded(name)).unwrap_or_else(|e| panic!("positive {name}: {e}"));
    }
    for (name, path, replacement) in [
        ("diff2-time", "/differences/2/kind", json!("newkind")),
        ("diff2-time", "/differences/0/from", json!(null)),
        ("diff2-time", "/differences/2/from", json!(null)),
        ("diff2-time", "/differences/2/to", json!([])),
        ("diff2-time", "/differences/2/to", json!(["fork", "fork"])),
        ("diff2-time", "/differences/2/change", json!("added")),
        ("diff2-resource", "/differences/3/to/capacity", json!(-1)),
        (
            "diff2-resource",
            "/differences/3/to/executors",
            json!(["p", "p"]),
        ),
        (
            "diff2-resource",
            "/differences/2/to/permission",
            json!(["q", "p"]),
        ),
        ("diff2-meaning", "/differences/0/to", json!(false)),
        ("diff2-visibility", "/differences/2/to", json!(["A", "A"])),
        ("diff2-visibility", "/differences/2/to", json!("A")),
        ("diff2-authority", "/differences/1/to/unknown", json!(0)),
        (
            "diff2-authority",
            "/differences/1/to/authority_basis/0/id",
            json!("missing"),
        ),
        ("diff2-contest", "/differences/1/to/0/respond_by", json!(-1)),
        (
            "diff2-preserve",
            "/preservation/summary/satisfied",
            json!(1),
        ),
        (
            "diff2-preserve",
            "/preservation/properties/0/id",
            json!("missing"),
        ),
        (
            "diff2-preserve",
            "/differences/0/annotations/0/properties/0",
            json!("missing"),
        ),
        (
            "diff2-override",
            "/differences/1/annotations/0/kind",
            json!("magic"),
        ),
        (
            "diff2-override",
            "/differences/1/annotations/0/subject_kind",
            json!("capability"),
        ),
        (
            "diff2-override",
            "/differences/1/annotations/0/effects/0/bypasses",
            json!(1),
        ),
        (
            "diff2-override",
            "/differences/1/annotations/0/unchecked",
            json!([]),
        ),
        (
            "diff2-maintain",
            "/differences/0/annotations/0/links/0/to_duties/0/id",
            json!(999),
        ),
        (
            "diff2-maintain",
            "/differences/0/annotations/0/links/0/status",
            json!("conflict"),
        ),
        ("diff2-time", "/checked/5", json!("unregistered")),
    ] {
        let mut v = upgraded(name);
        // JSON pointer insertion is deliberately restricted to one known leaf.
        if let Some(slot) = v.pointer_mut(path) {
            *slot = replacement;
        } else {
            let (parent, key) = path.rsplit_once('/').unwrap();
            v.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), replacement);
        }
        assert!(valid(&v).is_err(), "accepted corrupt {name}{path}: {v}");
    }
    for name in ["diff2-time", "diff2-preserve"] {
        let mut v = upgraded(name);
        let row = v["differences"][0].clone();
        v["differences"].as_array_mut().unwrap().push(row);
        assert!(valid(&v).is_err(), "duplicate(kind,target)");
    }
    let mut missing = upgraded("diff2-time");
    missing["differences"][2]
        .as_object_mut()
        .unwrap()
        .remove("from");
    assert!(
        valid(&missing).is_err(),
        "actual complete requires new from"
    );
    let mut severity = upgraded("diff2-time");
    severity["diagnostics"]
        .as_array_mut()
        .unwrap()
        .push(json!({"code":"W-DIFF-3","severity":"error","span":null,"message":"wrong severity"}));
    assert!(valid(&severity).is_err());
}

#[test]
fn metric_recursion_keeps_new_rows_and_preservation_and_checks_them() {
    let src = acceptance::stripped(&acceptance::source("diff2-preserve")).replace(
        "  bounds",
        "  metrics { window 20min; currency \"JPY\" }\n  bounds",
    );
    let mut v = serde_json::to_value(relca_core::diff::diff(&src, "From", "To")).unwrap();
    v["preservation"] = acceptance::expected("diff2-preserve")["preservation"].clone();
    v["differences"][0]["annotations"] = json!([{"kind":"preserve","selector":"Ctl","global":true,"properties":["contract Ctl #1 at_most_once"]}]);
    valid(&v).unwrap_or_else(|e| panic!("metrics positive: {e}"));
    let mut bad = v.clone();
    bad["preservation"]["summary"]["satisfied"] = json!(1);
    assert!(valid(&bad).is_err());
    let src = acceptance::source("diff2-time").replace(
        "  bounds",
        "  metrics { window 20min; currency \"JPY\" }\n  bounds",
    );
    let mut v =
        old_projection(serde_json::to_value(relca_core::diff::diff(&src, "From", "To")).unwrap())
            .unwrap();
    let (_, new) = oracle::rows(&src, "From", "To");
    v["differences"].as_array_mut().unwrap().extend(new);
    valid(&v).unwrap_or_else(|e| panic!("metrics typed rows: {e}"));
    v["differences"][2].as_object_mut().unwrap().remove("to");
    assert!(valid(&v).is_err());
}

fn local_input(name: &str, src: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/diff2-cli");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.relca"));
    std::fs::write(&path, src).unwrap();
    path
}
#[test]
fn explain_uses_one_process_and_all_non_diff_entrances_stay_non_diff() {
    let src = acceptance::stripped(&acceptance::source("diff2-override"));
    // explain cannot choose configurations. Build a genuine single-process
    // model, never pass --configuration to explain.
    let start = src.find("  process Before").unwrap();
    let end = src.find("  process After").unwrap();
    let mut single = src[..start].to_owned();
    single.push_str(&src[end..]);
    single = single
        .lines()
        .filter(|l| !l.trim_start().starts_with("configuration "))
        .collect::<Vec<_>>()
        .join("\n");
    let path = local_input("explain-single", &single);
    for command in ["check", "explain", "simulate", "mutate"] {
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg(command)
            .arg(&path)
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(
            v.get("differences").is_none() && v.get("preservation").is_none(),
            "non-diff {command}"
        );
        assert!(!out.status.success() || v["verdict"] == "satisfied");
        if command == "explain" {
            assert_eq!(v["verdict"], "satisfied");
            assert!(v.get("explanation").is_some());
        }
    }
}

#[test]
fn all_38_partial_expectations_have_closed_types_before_running_new_implementation() {
    for name in fixture_match::CONFIGURATION_DIFF_EXTENSION_FIXTURES {
        fixture_match::validate_diff_expected(&acceptance::expected(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn actual_cli_warning_inventory_matches_all_38_hand_calculations() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut failures = vec![];
    for name in fixture_match::CONFIGURATION_DIFF_EXTENSION_FIXTURES {
        let path = root.join("fixtures").join(name).join("model.relca");
        let (from, to) = if name.starts_with("diff2-invoice") {
            ("AsIs", "ToBeA")
        } else {
            ("From", "To")
        };
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("diff")
            .arg(path)
            .args(["--from", from, "--to", to])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        let count = |code| {
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["code"] == code)
                .count()
        };
        let actual = (count("W-DIFF-1"), count("W-DIFF-3"));
        let hand = fixture_match::configuration_diff_hand_warnings(name);
        if actual != hand {
            failures.push(format!("{name}: W1/W3 {actual:?} != hand {hand:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
