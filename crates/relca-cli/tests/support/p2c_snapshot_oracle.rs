//! Snapshot connector, not a new independent oracle.
//! All numeric expectations come from fixed p2_cleanup_calculations.md.
use serde_json::{Value, json};
pub const IDS: [&str; 3] = ["p2c-q9-range-stop", "p2c-prefix-diff", "p2c-withdrawn-diff"];

pub fn validate_set(jobs: &[Value]) {
    let mut actual: Vec<_> = jobs
        .iter()
        .filter_map(|j| j["id"].as_str())
        .filter(|id| id.starts_with("p2c-"))
        .collect();
    actual.sort();
    let mut expected = IDS;
    expected.sort();
    assert_eq!(actual, expected);
}
fn counts(n: u64) -> Value {
    json!([{"capability":"Probe.check","min":n,"max":n},
        {"capability":"Clerk.run","min":0,"max":0},
        {"capability":"Fast.run","min":0,"max":0}])
}
pub fn check(id: &str, args: &[String], doc: &Value, exit: i32, stderr: &[u8]) {
    assert!(IDS.contains(&id));
    let model = format!("crates/relca-cli/tests/snapshots/inputs/{id}.relca");
    let expected = if id == IDS[0] {
        vec!["simulate".to_owned(), model]
    } else {
        vec![
            "diff".to_owned(),
            model,
            "--from".into(),
            "From".into(),
            "--to".into(),
            "To".into(),
        ]
    };
    assert_eq!(args, expected);
    assert_eq!(exit, if id == IDS[0] { 1 } else { 0 });
    assert_eq!(
        doc["verdict"],
        if id == IDS[0] {
            "violated"
        } else {
            "satisfied"
        }
    );
    let mut lines = String::new();
    let owners = if id == IDS[0] {
        vec![("", &doc["diagnostics"])]
    } else {
        vec![
            ("from: ", &doc["from"]["report"]["diagnostics"]),
            ("to: ", &doc["to"]["report"]["diagnostics"]),
            ("diff: ", &doc["diagnostics"]),
        ]
    };
    for (prefix, diagnostics) in owners {
        for d in diagnostics.as_array().unwrap() {
            assert_ne!(d["code"], "U-LEDGER-0");
            lines.push_str(&format!(
                "{prefix}{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            ));
        }
    }
    assert_eq!(stderr, lines.as_bytes());
    assert_eq!(
        doc["ledger"]["derivation"]["bundle"],
        "relca-conclusions-p2-uncertainty-2"
    );
    if id == IDS[0] {
        assert_eq!(doc["simulation"]["range_stopped_replications"], 2);
        assert_eq!(doc["bounds"]["range_violation_stops"], true);
        let witness = doc["state_range_violations"].as_array().unwrap();
        assert_eq!(witness.len(), 1);
        assert_eq!(witness[0]["time"], 3);
        assert_eq!(witness[0]["attempted"], "2");
        assert_eq!(witness[0]["lower"], "0");
        assert_eq!(witness[0]["upper"], "1");
        let rows = doc["metrics"]["contests"]["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        for (r, route, failed, unresolved, reason) in [
            (&rows[0], "Reply.reject", 2, 0, "no_samples"),
            (&rows[1], "Reply.break_range", 0, 2, "unfinished"),
        ] {
            assert_eq!(r["route"], route);
            assert_eq!(r["trials"], 2);
            assert_eq!(r["target_observed"], 2);
            assert_eq!(r["filed"], 2);
            assert_eq!(r["route_failed"], failed);
            assert_eq!(r["unresolved"], unresolved);
            assert_eq!(
                r["filing_fraction"],
                json!({"value":"1","numerator":2,"denominator":2,"reason":null})
            );
            for field in ["service_time", "queue_wait", "response_elapsed"] {
                assert_eq!(
                    r[field],
                    json!({"mean":null,"min":null,"max":null,"samples":0,"reason":reason})
                );
            }
        }
        serde_json::from_value::<relca_core::report::SimulationReport>(doc.clone()).unwrap();
    } else {
        assert_eq!(doc["agency"]["differences"]["rules"], "agency-diff-p2-1");
        let rows: Vec<_> = doc["agency"]["differences"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["viewpoint"] == "degradation")
            .collect();
        if id == IDS[1] {
            assert_eq!(
                rows,
                vec![
                    &json!({"viewpoint":"degradation","item":"common_prefix","call":"Manual",
                "status":"changed","from":counts(1),"to":counts(2)})
                ]
            );
        } else {
            assert_eq!(rows.len(), 3);
            for item in ["normal", "degraded", "common_prefix"] {
                let r = rows.iter().find(|r| r["item"] == item).unwrap();
                assert_eq!(
                    **r,
                    json!({"viewpoint":"degradation","item":item,"status":"not_comparable",
                    "from":{"status":"not_derived","reason":"from.agency.degradation:multiple_case_interference"},
                    "to":{"status":"not_derived","reason":"to.agency.degradation:multiple_case_interference"}})
                );
            }
        }
        serde_json::from_value::<relca_core::diff::DiffReport>(doc.clone()).unwrap();
    }
}
