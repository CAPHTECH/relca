//! Real binary coverage and negative controls for the A7-only closed wire table.
#[allow(dead_code)]
#[path = "../../relca-core/tests/waits.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn every_wait_invocation_meets_hand_oracle_exit_and_closed_wire() {
    let rows = acceptance::rows();
    let mut failures = Vec::new();
    for (name, row) in rows.as_object().unwrap() {
        let path = acceptance::root()
            .join("fixtures")
            .join(name)
            .join("model.relca");
        let invocation = fixture_match::Invocation::parse(Some(&row["invocation"])).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(invocation.args(&path))
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        let code = match row["verdict"].as_str().unwrap() {
            "satisfied" => 0,
            "violated" => 1,
            "inconclusive" => 2,
            "invalid" => 3,
            "unsupported" => 4,
            "vacuous" => 5,
            _ => unreachable!(),
        };
        if v["verdict"] != row["verdict"] || out.status.code() != Some(code) {
            failures.push(format!(
                "{name}: exit {:?}, {} wanted {}",
                out.status.code(),
                v["verdict"],
                row["verdict"]
            ));
            continue;
        }
        let expected: Value = serde_json::from_str(
            &std::fs::read_to_string(path.with_file_name("expected.json")).unwrap(),
        )
        .unwrap();
        if let Err(e) = invocation.compare(&expected, &v) {
            failures.push(format!("{name}: {e}"));
            continue;
        }
        if !row["hand"].is_null() {
            let reports: Vec<_> = if row["invocation"]["command"] == "compare" {
                v["plans"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| &p["report"])
                    .collect()
            } else {
                vec![&v]
            };
            for report in reports {
                acceptance::hand(name, &report["metrics"], &row["hand"]);
                acceptance::assumptions(report);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "all acceptance failures at once:\n{}",
        failures.join("\n")
    );
}

fn wire_control() -> Value {
    let s = acceptance::source("wait-fifo");
    let mut v = serde_json::to_value(relca_core::report::check(&acceptance::masked(&s))).unwrap();
    assert_eq!(v["verdict"], "satisfied", "old grammar independent control");
    let rows = acceptance::rows();
    let h = &rows["wait-fifo"]["hand"];
    for (actual, want) in v["metrics"]["cases"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(h["cases"].as_array().unwrap())
    {
        actual["joins"] = want["joins"].clone();
        for t in actual["terminals"].as_array_mut().unwrap() {
            let bt = want["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .find(|w| w["end"] == t["end"]);
            if let Some(w) = bt {
                t["breakdown"] = w["breakdown"].clone();
            } else {
                let reason = t["lead_time"].clone();
                t["breakdown"] = json!({"external":reason,"in_service":reason,"contest_route":reason,
                    "intentional":reason,"unattributed":reason,"queue":{"actor_capacity":reason,"resource":reason,"executor":reason,"combined":reason}});
            }
        }
        for (t, w) in actual["tasks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(want["tasks"].as_array().unwrap())
        {
            t["wait_causes"] = w["wait_causes"].clone();
            t["predecessor_wait"] = w["predecessor_wait"].clone();
        }
    }
    for (a, w) in v["metrics"]["actors"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(h["actors"].as_array().unwrap())
    {
        a["queue"] = w["queue"].clone();
    }
    v["metrics"]["overall"]["queue_wait_observed"] = h["overall"]["queue_wait_observed"].clone();
    v["metrics"]["checked"].as_array_mut().unwrap().extend([
        json!("waiting_decomposition"),
        json!("queue_length_max_and_average"),
    ]);
    let u = v["metrics"]["unchecked"].as_array_mut().unwrap();
    u.retain(|x| x != "waiting_decomposition");
    for x in u {
        if x == "queue_length_statistics" {
            *x = json!("queue_length_quantiles");
        }
    }
    v["assumptions"].as_array_mut().unwrap().push(json!({"field":"metrics.waiting","value":"exclusive_interval_partition_v1","affects":acceptance::WAITING_AFFECTS}));
    v
}

#[test]
fn closed_wait_table_accepts_hand_authored_wire_but_rejects_unknown_missing_mode_and_bound_slots() {
    let v = wire_control();
    fixture_match::validate_waiting_wire(&v).unwrap();
    fixture_match::compare_report(&json!({}), &v).unwrap();
    let mut bad = Vec::new();
    for path in [
        "/metrics/cases/0/terminals/0/breakdown",
        "/metrics/cases/0/tasks/0/wait_causes",
        "/metrics/actors/0/queue",
    ] {
        let mut x = v.clone();
        x.pointer_mut(path).unwrap()["extra"] = json!(0);
        bad.push(x);
    }
    let mut x = v.clone();
    x["metrics"]["actors"][0]["queue"]["length_max"]["bound"] = json!("lower");
    bad.push(x);
    let mut x = v.clone();
    x["metrics"]["cases"][0]["terminals"][0]["breakdown"]
        .as_object_mut()
        .unwrap()
        .remove("unattributed");
    bad.push(x);
    let mut x = v.clone();
    x["metrics"]["cases"][0]["tasks"][0]["wait_causes"]["executor"] =
        json!({"mean":"0","min":"0","max":"0","samples":4,"reason":null});
    bad.push(x);
    let mut x = v.clone();
    x["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|a| a["field"] != "metrics.waiting");
    bad.push(x);
    let mut x = v.clone();
    x["metrics"]["cases"][0]
        .as_object_mut()
        .unwrap()
        .remove("joins");
    bad.push(x);
    let mut x = v.clone();
    x["metrics"]["actors"][0]["queue"]["wait_total"]["bound"] = json!("upper");
    bad.push(x);
    let mut x = v.clone();
    x["metrics"]["cases"][0]["tasks"][0]["queue"] = json!({});
    bad.push(x);
    for (i, b) in bad.iter().enumerate() {
        assert_ne!(&v, b);
        assert!(
            fixture_match::compare_report(&json!({}), b).is_err(),
            "closed negative {i} escaped"
        );
    }
}

#[test]
fn independent_join_inventory_oracle_rejects_wrong_mode_name_branch_order_and_join_any_reason() {
    let h = acceptance::rows()["wait-person"]["hand"].clone();
    acceptance::hand("join-oracle-positive", &h, &h);
    let mut mutations = Vec::new();
    for (key, value) in [
        ("mode", json!("any")),
        ("fork", json!("invented")),
        ("extra", json!(0)),
    ] {
        let mut x = h.clone();
        x["cases"][0]["joins"][0][key] = value;
        mutations.push(x);
    }
    let mut x = h.clone();
    x["cases"][0]["joins"][0]["branches"]
        .as_array_mut()
        .unwrap()
        .reverse();
    mutations.push(x);
    for x in mutations {
        assert_ne!(h["cases"][0]["joins"], x["cases"][0]["joins"]);
        assert!(
            std::panic::catch_unwind(|| acceptance::hand("corrupted-join", &x, &h)).is_err(),
            "join metadata oracle must reject the actual corruption"
        );
    }
    let any = acceptance::rows()["wait-any"]["hand"].clone();
    assert_eq!(
        any["cases"][0]["joins"][0]["branches"][0]["join_wait"]["reason"],
        "join_any"
    );
    let mut wire = wire_control();
    for c in wire["metrics"]["cases"].as_array_mut().unwrap() {
        c["joins"] = any["cases"][0]["joins"].clone();
    }
    fixture_match::compare_report(&json!({}), &wire).unwrap();
    let mut bad = wire.clone();
    bad["metrics"]["cases"][0]["joins"][0]["mode"] = json!("all");
    assert!(fixture_match::compare_report(&json!({}), &bad).is_err());
    let mut bad = wire.clone();
    bad["metrics"]["cases"][0]["joins"][0]["branches"][0]["join_wait"]["reason"] =
        json!("not_applicable");
    assert!(fixture_match::compare_report(&json!({}), &bad).is_err());
}

fn review_binary_unchecked(command: &str) {
    let run = |name: &str| {
        let path = acceptance::root()
            .join("fixtures")
            .join(name)
            .join("model.relca");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_relca"));
        cmd.arg(command).arg(path);
        if command == "compare" {
            cmd.args(["--plans", "Left,Right", "--mode", "exhaustive"]);
        } else {
            cmd.args(["--from", "Left", "--to", "Right"]);
        }
        let out = cmd.output().unwrap();
        assert_eq!(out.status.code(), Some(0));
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["verdict"], "satisfied");
        v
    };
    let old = run("wait-compare-diff.broken-no-breakdown");
    let new = run("wait-compare-diff");
    let count = |v: &Value| {
        v["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| *x == "waiting_tables")
            .count()
    };
    assert_eq!(count(&old), 0);
    assert_eq!(
        count(&new),
        1,
        "binary must name waiting table exclusion exactly once"
    );
    assert_eq!(new["metrics"], old["metrics"]);
}
#[test]
fn review_binary_compare_names_waiting_tables_once() {
    review_binary_unchecked("compare");
}
#[test]
fn review_binary_diff_names_waiting_tables_once() {
    review_binary_unchecked("diff");
}
