//! Actual CLI surfaces plus independent closed-table negative controls.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/resource_acceptance.rs"]
mod hand;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};
fn run(cmd: &str, source: &str, extra: &[&str]) -> (i32, Value) {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/resource-cli-acceptance");
    std::fs::create_dir_all(&dir).unwrap();
    let name = format!(
        "{}-{:?}.relca",
        std::process::id(),
        std::thread::current().id()
    );
    let path = dir.join(name);
    std::fs::write(&path, source).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(cmd)
        .arg(path)
        .args(extra)
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}
#[test]
fn all_resource_expectations_are_valid_closed_wire_and_names_are_exact() {
    let names: std::collections::BTreeSet<_> = hand::names().into_iter().collect();
    assert_eq!(
        names,
        fixture_match::RESOURCE_FIXTURES
            .iter()
            .map(|n| n.to_string())
            .collect()
    );
    for n in hand::names() {
        fixture_match::Invocation::LegacyCheck
            .validate_expected(&hand::expected(&n))
            .unwrap_or_else(|e| panic!("{n}: {e}"));
    }
}
#[test]
fn check_and_explain_keep_actor_resource_and_report_keys_separate() {
    let s = hand::source("rsrc-over-demand.broken-too-much");
    let (exit, v) = run("explain", &s, &[]);
    assert_eq!(exit, 1, "{v}");
    fixture_match::validate_explain_wire(&v).unwrap();
    assert!(v.get("resources").is_none());
    let c = v["explanation"]["counterexamples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "diagnostic:W-DEADLOCK-0")
        .unwrap();
    assert_eq!(c["replay"]["status"], "verified");
    let d = &c["steps"].as_array().unwrap().last().unwrap()["deadlock_detail"];
    assert_eq!(d["resources"][0]["actor"], "A");
    assert_eq!(
        d["shared_resources"],
        json!([{"resource":"R","capacity":1,"occupied":0,"holders":[]}])
    );
    assert_eq!(
        d["waiting_tasks"][0]["resource_shortages"],
        json!([{"resource":"R","required":2,"available":1,"missing":1}])
    );
}
#[test]
fn cli_simulate_samples_resource_totals_and_refuses_unresolved_competition() {
    let s = hand::edit(
        &hand::source("rsrc-global"),
        "verify safety no_resource_overcommit",
        "",
    );
    let s = hand::edit(
        &s,
        "  bounds",
        "  simulation replications 2 seed 1 confidence 0.95\n  bounds",
    );
    let (exit, v) = run("simulate", &s, &[]);
    assert_eq!(exit, 0, "{v}");
    fixture_match::validate_simulation_wire(&v).unwrap();
    assert_eq!(
        v["metrics"]["resources"][0]["occupancy_time"],
        json!({"mean":"4","min":"4","max":"4","samples":2,"reason":null})
    );
    let s = hand::edit(&s, "policy fifo tie_by case_id", "");
    let (exit, v) = run("simulate", &s, &[]);
    assert_eq!(exit, 4);
    assert_eq!(v["verdict"], "unsupported");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0")
    );
    let s = hand::edit(
        &hand::source("rsrc-competition"),
        "  bounds",
        "  simulation replications 2 seed 7 confidence 0.95\n  bounds",
    );
    let (exit, v) = run("simulate", &s, &[]);
    assert_eq!(
        exit, 4,
        "Actor-local FIFO leaves Actor-crossing resource nondeterminism: {v}"
    );
}
#[test]
fn cli_compare_retains_resource_rows_and_diff_keeps_uncalculated_resource_unchecked() {
    let s = hand::edit(
        &hand::source("rsrc-global"),
        "  bounds",
        "  configuration Before { cut K; process P }\n  configuration After { cut K; process P }\n  bounds",
    );
    let (exit, v) = run(
        "compare",
        &s,
        &["--plans", "Before,After", "--mode", "exhaustive"],
    );
    assert_eq!(exit, 0, "{v}");
    fixture_match::validate_compare_wire(&v).unwrap();
    for p in v["plans"].as_array().unwrap() {
        hand::subset(
            &hand::expected("rsrc-global")["metrics"],
            &p["report"]["metrics"],
        );
    }
    let rows = v["tables"]["metrics"].as_array().unwrap();
    for metric in ["occupancy_time", "peak_units", "utilization"] {
        assert!(
            rows.iter().any(|r| r["key"]["scope"] == "resource"
                && r["key"]["subject"] == "R"
                && r["key"]["metric"] == metric),
            "missing resource {metric}: {rows:?}"
        );
    }
    let (exit, v) = run("diff", &s, &["--from", "Before", "--to", "After"]);
    assert_eq!(exit, 0, "{v}");
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "resource"),
        "U7 guard: {v}"
    );
    // ADR 0031 lists `resource` in the root checked words (Actor capacity and
    // executors of active Capabilities). Declared resources are not computed by
    // that type, so the U7 guard (plan K35) must not also claim resource checked.
    assert!(
        !v["checked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "resource"),
        "U7 guard: uncalculated resource claimed checked: {v}"
    );
}
#[test]
fn closed_resource_metric_comparator_rejects_unknown_cost_missing_fields_empty_and_wrong_modes() {
    // Calibrate the wire table without depending on the unimplemented syntax.
    let old = hand::old_skeleton(&hand::source("rsrc-weighted"));
    let mut v = serde_json::to_value(relca_core::report::check(&old)).unwrap();
    let old_checked = v["metrics"]["checked"].as_array().unwrap().clone();
    v["metrics"]["resources"] = hand::expected("rsrc-weighted")["metrics"]["resources"].clone();
    v["metrics"]["rules"] = json!("business-metrics-p1-resource-1");
    v["metrics"]["checked"].as_array_mut().unwrap().extend([
        json!("resource_occupancy_time"),
        json!("resource_peak_units"),
        json!("resource_capacity_window_utilization"),
    ]);
    fixture_match::validate_business_metrics_wire(&v).unwrap();
    for key in [
        "resource",
        "capacity",
        "occupancy_time",
        "peak_units",
        "utilization",
    ] {
        let mut b = v.clone();
        b["metrics"]["resources"][0]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            fixture_match::validate_business_metrics_wire(&b).is_err(),
            "missing {key}"
        );
    }
    let mut b = v.clone();
    b["metrics"]["resources"][0]["cost"] = json!(0);
    assert!(fixture_match::validate_business_metrics_wire(&b).is_err());
    let mut b = v.clone();
    b["metrics"]["resources"] = json!([]);
    assert!(fixture_match::validate_business_metrics_wire(&b).is_err());
    let mut b = v.clone();
    let r = b["metrics"]["resources"][0].clone();
    b["metrics"]["resources"].as_array_mut().unwrap().push(r);
    assert!(fixture_match::validate_business_metrics_wire(&b).is_err());
    let mut b = v.clone();
    b["metrics"]["resources"][0]["occupancy_time"]["samples"] = json!(2);
    assert!(fixture_match::validate_business_metrics_wire(&b).is_err());
    let mut b = v.clone();
    b["metrics"]["rules"] = json!("business-metrics-p1-1");
    assert!(fixture_match::validate_business_metrics_wire(&b).is_err());
    let mut b = v.clone();
    b["metrics"]["checked"] = json!(old_checked);
    assert!(fixture_match::validate_business_metrics_wire(&b).is_err());
}

#[test]
fn closed_shared_resource_display_table_rejects_unknown_missing_and_inconsistent_holders() {
    let rows = json!([{"resource":"R","capacity":2,"occupied":2,"holders":[{"case_id":1,"binding":"x","amount":1},{"case_id":2,"binding":"x","amount":1}]}]);
    fixture_match::validate_shared_resource_snapshots(&rows).unwrap();
    for key in ["resource", "capacity", "occupied", "holders"] {
        let mut b = rows.clone();
        b[0].as_object_mut().unwrap().remove(key);
        assert!(fixture_match::validate_shared_resource_snapshots(&b).is_err());
    }
    for key in ["case_id", "binding", "amount"] {
        let mut b = rows.clone();
        b[0]["holders"][0].as_object_mut().unwrap().remove(key);
        assert!(fixture_match::validate_shared_resource_snapshots(&b).is_err());
    }
    for (pointer, value) in [
        ("/0/cost", json!(0)),
        ("/0/occupied", json!(1)),
        ("/0/holders/0/amount", json!(0)),
        ("/0/holders/1/case_id", json!(1)),
    ] {
        let mut b = rows.clone();
        if pointer == "/0/cost" {
            b[0]["cost"] = value;
        } else {
            *b.pointer_mut(pointer).unwrap() = value;
        }
        assert!(
            fixture_match::validate_shared_resource_snapshots(&b).is_err(),
            "{pointer}"
        );
    }
    assert!(fixture_match::validate_shared_resource_snapshots(&json!([])).is_err());
}
