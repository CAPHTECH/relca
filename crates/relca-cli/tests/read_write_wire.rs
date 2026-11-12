#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/quantile_review_inputs.rs"]
mod inline;
use serde_json::{Value, json};

fn source_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn witness() -> Value {
    let source =
        std::fs::read_to_string(source_root().join("fixtures/aobs-capacity/model.relca")).unwrap();
    let mut v = json!(relca_core::report::check(&source));
    let pins: Value =
        serde_json::from_str(include_str!("read_write_wire_expectations.json")).unwrap();
    let loc = &pins[0];
    assert_eq!(loc["job"], "fx-aobs-capacity");
    // Independent test-IR table fixed before implementation; do not get rows from
    // production, and do not silently duplicate them after implementation.
    v["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["field"] != "concurrency.read_write");
    assert_eq!(
        v["assumptions"].as_array().unwrap().len() as u64,
        7 // independently frozen historical prefix, including arrivals
    );
    v["assumptions"].as_array_mut().unwrap().splice(
        loc["index"].as_u64().unwrap() as usize..loc["index"].as_u64().unwrap() as usize,
        loc["rows"].as_array().unwrap().clone(),
    );
    v
}

#[test]
fn k5_wire_accepts_exact_hand_row_without_erasing_it() {
    let v = witness();
    fixture_match::Invocation::LegacyCheck
        .compare(&json!({"verdict":"satisfied"}), &v)
        .unwrap();
    assert_eq!(
        v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["field"] == "concurrency.read_write")
            .unwrap()["value"],
        "Ledger.posted: Ledger.post -> Ledger.post"
    );
}

#[test]
fn k5_wire_rejects_one_character_candidate_value_fault() {
    let mut v = witness();
    v["assumptions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["field"] == "concurrency.read_write")
        .unwrap()["value"] = json!("Ledger.posted: Ledger.post -> Ledger.past");
    let error = fixture_match::Invocation::LegacyCheck
        .compare(&json!({"verdict":"satisfied"}), &v)
        .unwrap_err();
    assert!(
        error.contains("wire read_write:"),
        "reject the exact hand payload, not an unrelated wire failure: {error}"
    );
}

#[test]
fn k5_wire_rejects_another_unknown_assumption_field() {
    let mut v = witness();
    v["assumptions"]
        .as_array_mut()
        .unwrap()
        .push(json!({"field":"concurrency.unknown","value":"x","affects":"x"}));
    assert!(
        fixture_match::Invocation::LegacyCheck
            .compare(&json!({"verdict":"satisfied"}), &v)
            .is_err()
    );
}

#[test]
fn k5_wire_table_is_exactly_the_frozen_ir_and_inline_hand_tables() {
    let rows: Value =
        serde_json::from_str(include_str!("read_write_wire_expectations.json")).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 70);
    let canonical: std::collections::BTreeSet<_> = rows[..30]
        .iter()
        .map(|row| row["job"].as_str().unwrap())
        .collect();
    assert_eq!(canonical.len(), 30);
    assert!(canonical.iter().all(|job| job.starts_with("fx-")));
    let extra: Vec<Value> =
        serde_json::from_str(include_str!("read_write_inline_wire_expectations.json")).unwrap();
    assert_eq!(extra.len(), 40);
    assert_eq!(&rows[30..], extra.as_slice());
    let out = std::process::Command::new("sha256sum")
        .arg(source_root().join("crates/relca-cli/tests/read_write_inline_wire_expectations.json"))
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout)
            .unwrap()
            .split_whitespace()
            .next(),
        Some("0276a3f534b14955aad1578ef43c42a6812a9c622d27283169e9a81cdf69c10c")
    );
}

fn inline_wire(v: &Value) -> Result<(), String> {
    fixture_match::Invocation::LegacyCheck.compare(&json!({"verdict":v["verdict"]}), v)
}

#[test]
fn k5_selection_pins_close_positive_and_zero_hand_inventory() {
    let pins: Vec<Value> =
        serde_json::from_str(include_str!("read_write_selection_expectations.json")).unwrap();
    let positive: Vec<Value> =
        serde_json::from_str(include_str!("read_write_inline_wire_expectations.json")).unwrap();
    assert_eq!(pins.len(), 82);
    assert_eq!(pins.iter().filter(|p| p["rows"] == json!([])).count(), 42);
    for old in positive {
        let pin = pins.iter().find(|p| p["job"] == old["job"]).unwrap();
        for key in ["source_sha256", "index", "rows"] {
            assert_eq!(pin[key], old[key]);
        }
        if pin["job"].as_str().unwrap().contains("d06-") {
            assert_eq!(
                pin["process"],
                if pin["configuration"] == "Left" {
                    "P"
                } else {
                    "Q"
                }
            );
        }
    }
    let hash = std::process::Command::new("sha256sum")
        .arg(source_root().join("crates/relca-cli/tests/read_write_selection_expectations.json"))
        .output()
        .unwrap();
    assert!(hash.status.success());
    assert_eq!(
        String::from_utf8(hash.stdout)
            .unwrap()
            .split_whitespace()
            .next(),
        Some("a5541b6646bdbc09d4649fb8348cf7228c8702c984f6235dd41203e835d6b18c")
    );
}

#[test]
fn k5_inline_wire_rejects_all_candidates_deleted() {
    let mut v = json!(relca_core::report::check(inline::DESK));
    v["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["field"] != "concurrency.read_write");
    assert!(inline_wire(&v).unwrap_err().contains("wire read_write:"));
}

#[test]
fn k5_inline_wire_accepts_fixed_zero_candidates() {
    let v = json!(relca_core::report::check(inline::CLOSED));
    assert!(
        !v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "concurrency.read_write")
    );
    inline_wire(&v).unwrap();
    let pins: Vec<Value> =
        serde_json::from_str(include_str!("read_write_selection_expectations.json")).unwrap();
    assert!(
        pins.iter()
            .any(|p| p["source_sha256"] == v["model"]["sha256"]
                && p["rows"] == json!([])
                && p["configuration"].is_null())
    );
}

fn selected_desk() -> String {
    let mut s = inline::DESK.replace("replications 1", "replications 4");
    s = s
        .lines()
        .filter(|l| !l.trim_start().starts_with("metric quantile."))
        .collect::<Vec<_>>()
        .join("\n");
    let body = &s[s.find("  process P").unwrap()..s.find("  cut C").unwrap()];
    let second = body.replace("process P", "process Q");
    let end = s.rfind('}').unwrap();
    s.insert_str(end,&format!("\n{second}\nconfiguration Left {{ cut C; process P }}\nconfiguration Right {{ cut C; process Q }}\nmetric cohort.Rate = expected(count(case.Flag)) / arrivals\ncompare [Left,Right] cut C hard {{ cohort.Rate <= 0.75 }} objectives {{ minimize cohort.Rate }}\n"));
    s
}

#[test]
fn k5_inline_selected_check_and_simulate_require_all_candidates() {
    let s = selected_desk();
    for configuration in ["Left", "Right"] {
        for simulation in [false, true] {
            let mut v = if simulation {
                json!(relca_core::report::simulate_configuration(
                    &s,
                    configuration
                ))
            } else {
                json!(relca_core::report::check_configuration(&s, configuration))
            };
            let inv = if simulation {
                fixture_match::Invocation::Simulate {
                    configuration: Some(configuration.into()),
                }
            } else {
                fixture_match::Invocation::Check {
                    configuration: configuration.into(),
                }
            };
            inv.compare(&json!({"verdict":v["verdict"]}), &v).unwrap();
            v["assumptions"]
                .as_array_mut()
                .unwrap()
                .retain(|r| r["field"] != "concurrency.read_write");
            assert!(
                inv.compare(&json!({"verdict":v["verdict"]}), &v)
                    .unwrap_err()
                    .contains("wire read_write:")
            );
        }
    }
}

#[test]
fn k5_inline_wire_binds_candidate_to_selected_configuration_and_process() {
    let s = selected_desk();
    let left = json!(relca_core::report::check_configuration(&s, "Left"));
    let right = json!(relca_core::report::check_configuration(&s, "Right"));
    let inv = fixture_match::Invocation::Check {
        configuration: "Left".into(),
    };
    inv.compare(&json!({"verdict":left["verdict"]}), &left)
        .unwrap();
    fixture_match::Invocation::Check {
        configuration: "Right".into(),
    }
    .compare(&json!({"verdict":right["verdict"]}), &right)
    .unwrap();
    let right_row = right["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["field"] == "concurrency.read_write")
        .unwrap();
    let mut wrong = left.clone();
    *wrong["assumptions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["field"] == "concurrency.read_write")
        .unwrap() = right_row.clone();
    assert!(
        inv.compare(&json!({"verdict":left["verdict"]}), &wrong)
            .unwrap_err()
            .contains("wire read_write:")
    );
    for plans in [
        vec!["Left".to_string(), "Right".to_string()],
        vec!["Right".to_string(), "Left".to_string()],
    ] {
        let refs = plans.iter().map(String::as_str).collect::<Vec<_>>();
        let mut v = json!(relca_core::compare::compare(
            &s,
            &refs,
            relca_core::compare::Analysis::Exhaustive
        ));
        let invocation = fixture_match::Invocation::Compare {
            plans: plans.clone(),
            mode: fixture_match::Analysis::Exhaustive,
        };
        invocation
            .compare(&json!({"verdict":v["verdict"]}), &v)
            .unwrap();
        let p = v["plans"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["configuration"] == "Left")
            .unwrap();
        *p["report"]["assumptions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["field"] == "concurrency.read_write")
            .unwrap() = right_row.clone();
        assert!(
            invocation
                .compare(&json!({"verdict":v["verdict"]}), &v)
                .unwrap_err()
                .contains("wire read_write:")
        );
        assert!(
            fixture_match::validate_compare_wire(&v)
                .unwrap_err()
                .contains("wire read_write:")
        );
    }
    let mut diff = json!(relca_core::diff::diff(&s, "Left", "Right"));
    fixture_match::compare_diff_report(&json!({"verdict":diff["verdict"]}), &diff).unwrap();
    *diff["from"]["report"]["assumptions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["field"] == "concurrency.read_write")
        .unwrap() = right_row.clone();
    assert!(
        fixture_match::compare_diff_report(&json!({"verdict":diff["verdict"]}), &diff)
            .unwrap_err()
            .contains("wire read_write:")
    );
}

#[test]
fn k5_inline_wire_requires_the_hand_pair_before_the_quality_tail() {
    let v = json!(relca_core::report::check(inline::DESK));
    inline_wire(&v).unwrap();
    let rows = v["assumptions"].as_array().unwrap();
    let candidate = rows
        .iter()
        .position(|r| r["field"] == "concurrency.read_write")
        .unwrap();
    let quality = rows
        .iter()
        .position(|r| r["field"] == "quality.definition")
        .unwrap();
    assert_eq!(candidate, 8, "the independently captured lower base slot");
    assert!(candidate < quality);
    assert_eq!(
        rows[candidate]["value"],
        "Desk.seen: process.P@if:462 -> Desk.first"
    );
}

#[test]
fn k5_inline_wire_rejects_one_character_pair_fault() {
    let mut v = json!(relca_core::report::check(inline::DESK));
    let row = v["assumptions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["field"] == "concurrency.read_write")
        .unwrap();
    row["value"] = json!(
        row["value"]
            .as_str()
            .unwrap()
            .replace("Desk.first", "Desk.fIrst")
    );
    assert!(inline_wire(&v).unwrap_err().contains("wire read_write:"));
}

#[test]
fn k5_inline_wire_rejects_a_row_on_an_unregistered_empty_inline_input() {
    let mut v = json!(relca_core::report::check(inline::CLOSED));
    let extra: Vec<Value> =
        serde_json::from_str(include_str!("read_write_inline_wire_expectations.json")).unwrap();
    assert!(
        !extra
            .iter()
            .any(|pin| pin["source_sha256"] == v["model"]["sha256"])
    );
    let rows = v["assumptions"].as_array_mut().unwrap();
    assert!(!rows.iter().any(|r| r["field"] == "concurrency.read_write"));
    let at = rows
        .iter()
        .position(|r| r["field"] == "quality.definition")
        .unwrap();
    rows.insert(at, extra[0]["rows"][0].clone());
    assert!(inline_wire(&v).unwrap_err().contains("wire read_write:"));
}

#[test]
fn k5_inline_wire_still_rejects_reordered_quantile_disclosure() {
    let mut v = json!(relca_core::report::check(inline::DESK));
    let rows = v["assumptions"].as_array_mut().unwrap();
    let at = rows
        .iter()
        .position(|r| r["field"] == "quality.quantile")
        .unwrap();
    let quantile = rows.remove(at);
    let before = rows
        .iter()
        .position(|r| r["field"] == "quality.definition")
        .unwrap();
    rows.insert(before, quantile);
    assert!(
        inline_wire(&v)
            .unwrap_err()
            .contains("quantile assumption fields/order")
    );
}
