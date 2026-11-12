#[path = "../../relca-core/tests/support/evaluation_waits_acceptance.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use acceptance::Op;
use serde_json::{Value, json};
use std::{
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn run(source: &str, op: Op<'_>, day: bool) -> Value {
    static ID: AtomicUsize = AtomicUsize::new(0);
    let dir = test_tmp::root().join(format!(
        "relca-b7ontoa7-{}-{}",
        std::process::id(),
        ID.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, source).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_relca"));
    match op {
        Op::Compare(plans, sample) => {
            command.arg("compare").arg(&path).args([
                "--plans",
                &plans.join(","),
                "--mode",
                if sample { "simulation" } else { "exhaustive" },
            ]);
        }
        Op::Single(config, sample) => {
            command
                .arg(if sample { "simulate" } else { "check" })
                .arg(&path)
                .args(["--configuration", config]);
        }
    }
    if day {
        command.args(["--scenario", "Day"]);
    }
    let output = command.output().unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let verdict: relca_core::Verdict = serde_json::from_value(value["verdict"].clone()).unwrap();
    assert_eq!(output.status.code(), Some(verdict.exit_code()));
    // Existing CLI warnings can accompany satisfied; current stderr bytes are
    // protected by the golden snapshots, not assumed empty.
    value
}

#[test]
fn combined_waiting_evaluation_wire_accepts_both_modes_and_rejects_missing_duplicate_moved_word() {
    for sample in [false, true] {
        let v = acceptance::compared(acceptance::W, sample);
        fixture_match::validate_compare_wire(&v).unwrap();
        for fault in ["missing", "duplicate", "moved"] {
            let mut bad = v.clone();
            let words = bad["unchecked"].as_array_mut().unwrap();
            match fault {
                "missing" => {
                    let word = words.pop().unwrap();
                    assert_eq!(word, "waiting_tables");
                }
                "duplicate" => words.push(json!("waiting_tables")),
                "moved" => {
                    let word = words.pop().unwrap();
                    words.insert(0, word);
                }
                _ => unreachable!(),
            }
            assert!(
                fixture_match::validate_compare_wire(&bad).is_err(),
                "accepted {fault}"
            );
        }
        let control = acceptance::compared(&acceptance::masked(acceptance::W), sample);
        fixture_match::validate_compare_wire(&control).unwrap();
        let mut bad = control;
        bad["unchecked"]
            .as_array_mut()
            .unwrap()
            .push(json!("waiting_tables"));
        assert!(
            fixture_match::validate_compare_wire(&bad).is_err(),
            "undeclared waiting_tables"
        );
    }
}

#[test]
fn intentional_wait_existing_field_requires_the_exact_closed_rows_and_values() {
    for sample in [false, true] {
        let v = acceptance::compared(acceptance::W, sample);
        fixture_match::validate_compare_wire(&v).unwrap();
        let positions: Vec<_> = v["tables"]["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter_map(|(ix, r)| {
                (r["key"]["scope"] == "case" && r["key"]["metric"] == "intentional_wait")
                    .then_some(ix)
            })
            .collect();
        assert_eq!(
            positions.len(),
            2,
            "one existing intentional_wait row per declared case"
        );
        for fault in [
            "missing_row",
            "wrong_value",
            "extra_field",
            "missing_source_field",
            "duplicate_row",
        ] {
            let mut bad = v.clone();
            match fault {
                "missing_row" => {
                    bad["tables"]["metrics"]
                        .as_array_mut()
                        .unwrap()
                        .remove(positions[0]);
                }
                "wrong_value" => {
                    bad["tables"]["metrics"][positions[0]]["values"][0]["value"]["min"] = json!("2")
                }
                "extra_field" => {
                    bad["plans"][0]["report"]["metrics"]["cases"][0]["intentional_wait_typo"] =
                        acceptance::scalar("1", sample)
                }
                "missing_source_field" => {
                    bad["plans"][0]["report"]["metrics"]["cases"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("intentional_wait");
                }
                "duplicate_row" => {
                    let row = bad["tables"]["metrics"][positions[0]].clone();
                    bad["tables"]["metrics"]
                        .as_array_mut()
                        .unwrap()
                        .insert(positions[0], row);
                }
                _ => unreachable!(),
            }
            assert!(
                fixture_match::validate_compare_wire(&bad).is_err(),
                "accepted intentional_wait {fault}"
            );
        }
    }
}
