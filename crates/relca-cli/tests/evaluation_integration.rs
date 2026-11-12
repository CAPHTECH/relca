#[path = "../../relca-core/tests/support/evaluation_integration_acceptance.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use acceptance::Op;
use serde_json::Value;
use std::{
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn run(src: &str, op: Op<'_>, day: bool) -> Value {
    static ID: AtomicUsize = AtomicUsize::new(0);
    let dir = test_tmp::root().join(format!(
        "relca-b7int-{}-{}",
        std::process::id(),
        ID.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, src).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_relca"));
    match op {
        Op::Compare(plans, sampled) => {
            c.arg("compare").arg(&path).args([
                "--plans",
                &plans.join(","),
                "--mode",
                if sampled { "simulation" } else { "exhaustive" },
            ]);
        }
        Op::Single(config, sampled) => {
            c.arg(if sampled { "simulate" } else { "check" })
                .arg(&path)
                .args(["--configuration", config]);
        }
    }
    if day {
        c.args(["--scenario", "Day"]);
    }
    let out = c.output().unwrap();
    let v: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stderr)));
    let verdict: relca_core::Verdict = serde_json::from_value(v["verdict"].clone()).unwrap();
    assert_eq!(
        out.status.code(),
        Some(verdict.exit_code()),
        "real CLI exit"
    );
    if matches!(op, Op::Compare(..)) {
        fixture_match::validate_compare_wire(&v)
            .unwrap_or_else(|e| panic!("real compare wire: {e}"));
    }
    v
}

fn record_wire() -> Value {
    let s = acceptance::i::append(
        acceptance::q::RECORD,
        "configuration Left { cut C; process P }\nconfiguration Right { cut C; process P }\ncompare [Left,Right] hard { cohort.WrongRate <= 0.02 } objectives { minimize cohort.Conditional }",
    );
    let v = run(&s, Op::Compare(&["Left", "Right"], false), false);
    assert_eq!(v["tables"]["quality"], acceptance::quality_table::record());
    v
}

#[test]
fn quality_table_positive_wire() {
    fixture_match::validate_compare_wire(&record_wire()).unwrap();
}

fn quality_fault(fault: &str) {
    let mut v = record_wire();
    match fault {
        "absent" => {
            v["tables"].as_object_mut().unwrap().remove("quality");
        }
        "empty" => v["tables"]["quality"] = serde_json::json!([]),
        "missing" => {
            v["tables"]["quality"].as_array_mut().unwrap().remove(0);
        }
        "duplicate" => {
            let r = v["tables"]["quality"][0].clone();
            v["tables"]["quality"].as_array_mut().unwrap().insert(0, r);
        }
        "columns" => v["tables"]["quality"][0]["values"]
            .as_array_mut()
            .unwrap()
            .reverse(),
        "rows" => v["tables"]["quality"].as_array_mut().unwrap().swap(0, 1),
        "denominator" => v["tables"]["quality"][3]["denominator"]["count"] = serde_json::json!(2),
        "value" => {
            v["tables"]["quality"][0]["values"][0]["value"]["lower"] = serde_json::json!("0")
        }
        _ => panic!("unknown quality fault"),
    }
    assert!(
        fixture_match::validate_compare_wire(&v).is_err(),
        "quality fault accepted: {fault}"
    );
}
macro_rules! quality_fault_test {
    ($name:ident, $fault:literal) => {
        #[test]
        fn $name() {
            quality_fault($fault);
        }
    };
}
quality_fault_test!(quality_table_rejects_empty, "empty");
quality_fault_test!(quality_table_rejects_missing, "missing");
quality_fault_test!(quality_table_rejects_duplicate, "duplicate");
quality_fault_test!(quality_table_rejects_columns, "columns");
quality_fault_test!(quality_table_rejects_value, "value");
quality_fault_test!(quality_table_rejects_rows, "rows");
quality_fault_test!(quality_table_rejects_denominator, "denominator");
quality_fault_test!(quality_table_rejects_absent, "absent");

#[test]
fn quality_table_remains_empty_when_cut_mismatch_prevents_building_tables() {
    let s = acceptance::i::append(
        acceptance::q::RECORD,
        "cut Other { include [A,Ledger] }\nconfiguration Left { cut C; process P }\nconfiguration Right { cut Other; process P }\ncompare [Left,Right] hard { cohort.WrongRate <= 0.02 }",
    );
    let mut v = run(&s, Op::Compare(&["Left", "Right"], false), false);
    assert_eq!(v["verdict"], "invalid");
    assert_eq!(v["tables"]["quality"], serde_json::json!([]));
    assert!(
        v["plans"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["report"]["quality"].is_object())
    );
    fixture_match::validate_compare_wire(&v).unwrap();
    v["tables"]["quality"] = acceptance::quality_table::record();
    assert!(
        fixture_match::validate_compare_wire(&v).is_err(),
        "unperformed comparison cannot publish a quality table"
    );
}

fn end_inventory_wire(sampled: bool) -> Value {
    let s = r#"model EndInventory version "0.2" {
        type T = enum { ok }
        actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }
        process P { x = A.run(); if x == ok { end done } else { end never } }
        process Q { x = A.run(); if x == ok { end done } else { end never } }
        process Unselected { x = A.run(); end unselected }
        cut C { include [A] }
        configuration Left { cut C; process P }
        configuration Right { cut C; process Q }
        bounds { cases 1; live_tasks 1; clock 10min }
        simulation replications 2 seed 7 confidence 0.5
        metrics { window 10min; currency "JPY" }
        property case.Done = reached(end.done)
        metric cohort.Rate = expected(count(case.Done)) / arrivals
        verify reachable end.done
        compare [Left,Right] objectives { minimize metrics.overall.work }
    }"#;
    run(s, Op::Compare(&["Left", "Right"], sampled), false)
}

#[test]
fn m4_selected_end_inventory_positive_excludes_unselected_process() {
    for sampled in [false, true] {
        let v = end_inventory_wire(sampled);
        // Quality and Agency framing are model-wide, not the selected process
        // inventory. This namesake must not manufacture a comparison row.
        assert!(
            v["plans"][0]["report"]["quality"]["outcomes_per_arrival"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["end"] == "unselected")
        );
        assert!(
            !v["tables"]["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["end"] == "unselected")
        );
        fixture_match::validate_compare_wire(&v).unwrap();
    }
}

fn missing_selected_end(end: &str) {
    for sampled in [false, true] {
        let mut v = end_inventory_wire(sampled);
        let rows = v["tables"]["terminals"].as_array_mut().unwrap();
        let before = rows.len();
        rows.retain(|r| !(r["kind"] == "end" && r["end"] == end));
        assert_eq!(rows.len() + 1, before, "effective whole-End deletion");
        assert!(
            fixture_match::validate_compare_wire(&v).is_err(),
            "entire End inventory lost: {end}, sampled={sampled}"
        );
    }
}
#[test]
fn m4_rejects_all_done_rows_missing() {
    missing_selected_end("done");
}
#[test]
fn m4_rejects_all_unobserved_end_rows_missing() {
    missing_selected_end("never");
}

#[test]
fn hand_cutoff_wire_is_rectangular_and_rejects_one_missing_cutoff_row() {
    use acceptance::i;
    for sampled in [false, true] {
        let s = i::simulation(1, false);
        let mut v = run(&s, Op::Compare(&["Good", "Bad"], sampled), true);
        // Independent expected row, not copied from the report or table.
        let value = if sampled {
            serde_json::json!({"count":3})
        } else {
            serde_json::json!({"probability":{"min":"1","max":"1"},"completed_at":null,"elapsed":null,"work":{"min":1,"max":1},"result_at":null})
        };
        v["tables"]["terminals"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["kind"] != "cut_off");
        v["tables"]["terminals"].as_array_mut().unwrap().push(serde_json::json!({"kind":"cut_off","end":null,"case_id":1,"arrival":0,
            "values":[{"configuration":"Good","status":"reported","value":value},{"configuration":"Bad","status":"reported","value":value}]}));
        fixture_match::validate_compare_wire(&v)
            .unwrap_or_else(|e| panic!("hand-cutoff positive: {e}"));
        let mut missing = v.clone();
        missing["tables"]["terminals"].as_array_mut().unwrap().pop();
        assert!(
            fixture_match::validate_compare_wire(&missing).is_err(),
            "a horizon declaration requires a cutoff row even before any cutoff is observed"
        );
        let mut duplicate = v.clone();
        let row = v["tables"]["terminals"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .clone();
        duplicate["tables"]["terminals"]
            .as_array_mut()
            .unwrap()
            .push(row);
        assert!(fixture_match::validate_compare_wire(&duplicate).is_err());
    }
}
