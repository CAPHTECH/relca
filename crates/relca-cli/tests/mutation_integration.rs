//! C6 intake through the real binary and recursively closed wire comparator.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/mutation_integration_inputs.rs"]
mod i;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{io::Write, path::PathBuf, process::Command};

fn invoke(s: &str, args: &[&str], exit: i32) -> Value {
    static ID: AtomicUsize = AtomicUsize::new(0);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tmp = test_tmp::root();
    std::fs::create_dir_all(&tmp).unwrap();
    let path = tmp.join(format!(
        "c6int-cli-{}-{}.relca",
        std::process::id(),
        ID.fetch_add(1, Ordering::Relaxed)
    ));
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    f.write_all(s.as_bytes()).unwrap();
    // The CLI runs from root, not this test's cwd; TMPDIR can be relative.
    let path = path.canonicalize().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("mutate")
        .arg(&path)
        .args(args)
        .current_dir(&root)
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: Value = serde_json::from_slice(&output.stdout).expect("exactly one JSON, never a panic");
    fixture_match::validate_mutation_wire(&v).unwrap_or_else(|e| panic!("{e}\n{v}"));
    v
}
#[test]
fn selected_scenario_wire_keeps_original_and_mutant_metadata() {
    let v = invoke(&i::scenario(i::X), &["--scenario", "Day"], 4);
    assert_eq!(v["mutations"][1]["mutants"][0]["result"], "detected");
    assert_eq!(
        v["mutations"][1]["mutants"][0]["report"]["scenario"],
        v["original"]["scenario"]
    );
}
#[test]
fn missing_scenario_wire_keeps_six_blocked_and_four_unsupported() {
    let v = invoke(&i::scenario(i::X), &[], 3);
    for k in [0, 1, 2, 4, 8, 9] {
        assert_eq!(v["mutations"][k]["status"], "blocked");
    }
    for k in [5, 7] {
        assert_eq!(v["mutations"][k]["status"], "unsupported");
    }
    for k in [3, 6] {
        assert_eq!(v["mutations"][k]["status"], "blocked");
        assert_eq!(v["mutations"][k]["reason"], "original_not_decidable");
        assert_eq!(v["mutations"][k]["mutants"], serde_json::json!([]));
    }
}
#[test]
fn observed_record_lineage_has_three_held_rows_on_the_wire() {
    let v = invoke(&i::observed(true, true, true), &[], 4);
    let cs = v["mutations"][4]["mutants"][0]["assigned_changes"]
        .as_array()
        .unwrap();
    assert_eq!(cs.len(), 3);
    assert!(cs.iter().all(|c| c["direction"] == "held"));
    assert_eq!(cs[0]["before"], json!(["Archive", "Invoice"]));
}
#[test]
fn quality_is_carried_and_not_claimed_as_other_changes() {
    let v = invoke(&i::quality(), &[], 4);
    let m = &v["mutations"][4]["mutants"][0];
    assert_eq!(v["original"]["quality"]["cohort"][0]["value"]["lower"], "1");
    assert_eq!(m["report"]["quality"]["cohort"][0]["value"]["lower"], "0");
    assert!(
        m["other_changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["check"] != "quality")
    );
    assert_eq!(v["mutations"][7]["status"], "generated");
    assert!(v["mutations"][7]["reason"].is_null());
    let definitions = v["mutations"][7]["mutants"].as_array().unwrap();
    assert_eq!(
        definitions
            .iter()
            .map(|m| m["site"]["removed_end"].clone())
            .collect::<Vec<_>>(),
        vec![json!("rejected"), json!("done")]
    );
    assert!(definitions.iter().all(|m| m["result"] == "detected"
        && m["report"].is_null()
        && m["other_changes"] == json!([])));
}
#[test]
fn rule_judgements_and_withdrawn_quality_are_valid_mutation_wire() {
    let v = invoke(&i::rule(false, true), &[], 4);
    let original = &v["original"];
    assert_eq!(
        original["quality"]["cohort"][0]["value"]["reason"],
        Value::Null
    );
    assert_eq!(
        original["quality"]["cohort"][0]["value"],
        json!({"lower":"0","upper":"1","basis":"per_case_relaxation","reason":null})
    );
    assert!(
        original["agency"]["refusal"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["judgements"]
                .as_array()
                .unwrap()
                .iter()
                .any(|j| j["kind"] == "rule"))
    );
    assert_eq!(v["mutations"][1]["status"], "not_applicable");
}
#[test]
fn observed_read_via_model_rule_token_is_closed_not_an_unknown_word_exemption() {
    let v = invoke(&i::observed(true, false, false), &[], 4);
    assert_eq!(
        v["original"]["via_model_paths"]["rules"],
        "via-model-read-paths-reads-1"
    );
    for bad in [
        "via-model-read-paths-reads-2",
        "via-model-read-paths-arbitrary-1",
        "",
    ] {
        let mut fault = v.clone();
        fault["original"]["via_model_paths"]["rules"] = json!(bad);
        assert!(fixture_match::validate_mutation_wire(&fault).is_err());
    }
}

#[test]
fn field_reads_refusal_keeps_one_error_message_full_span_and_exit_four() {
    let source = i::replace(
        &i::observed(true, false, true),
        "reads [snap]",
        "reads [snap.confidence]",
    );
    let v = invoke(&source, &[], 4);
    assert_eq!(v["original_verdict"], "unsupported");
    let errors: Vec<_> = v["original"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["code"], "U-SYN-0");
    assert_eq!(errors[0]["message"], "Actor state reads are unsupported");
    let start = source.find("snap.confidence").unwrap();
    assert_eq!(
        errors[0]["span"],
        json!({"start":start,"end":start+"snap.confidence".len()})
    );
}

#[test]
fn horizon_cutoff_mutant_is_inconclusive_and_does_not_relax_detected_wire() {
    let v = invoke(&i::quality_horizon(4), &["--scenario", "Day"], 4);
    let m = &v["mutations"][4]["mutants"][0];
    assert_eq!(m["result"], "inconclusive");
    assert_eq!(m["reason"], "assigned_check_unavailable");
    assert_eq!(m["report"]["bounds"]["truncated"], false);
    assert_eq!(
        m["report"]["case_outcomes"][0]["terminals"][0]["kind"],
        "cut_off"
    );
    assert_eq!(v["original"]["quality"]["cohort"][0]["value"]["lower"], "1");
    assert_eq!(m["report"]["quality"]["cohort"][0]["value"]["lower"], "0");
}

#[test]
fn legacy_guard_horizon_rows_are_inconclusive_and_obey_the_closed_wire() {
    for horizon in [5, 3] {
        let v = invoke(&i::guard_horizon(horizon), &["--scenario", "Day"], 4);
        for k in [2, 8] {
            let m = v["mutations"][k]["mutants"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["id"].as_str().unwrap().ends_with("body/2:keep_then"))
                .unwrap();
            assert_eq!(
                m["result"],
                if horizon == 3 {
                    "inconclusive"
                } else {
                    "undetected"
                }
            );
            assert_eq!(
                m["reason"],
                if horizon == 3 {
                    "assigned_check_unavailable"
                } else {
                    "no_assigned_check"
                }
            );
            assert_eq!(m["report"]["bounds"]["truncated"], false);
            assert_eq!(
                m["report"]["verdict"],
                if horizon == 3 {
                    "inconclusive"
                } else {
                    "satisfied"
                }
            );
            let terminal = &m["report"]["case_outcomes"][0]["terminals"][0];
            assert_eq!(
                terminal["kind"],
                if horizon == 3 { "cut_off" } else { "end" }
            );
            assert_eq!(
                terminal["end"],
                if horizon == 3 {
                    Value::Null
                } else {
                    json!("done")
                }
            );
        }
        assert_eq!(
            v["run"]["completion"],
            if horizon == 3 {
                "inconclusive"
            } else {
                "complete"
            }
        );
        assert_eq!(
            v["run"]["reason"],
            if horizon == 3 {
                json!("mutant_incomplete")
            } else {
                Value::Null
            }
        );
    }
}
