//! Hand values for the five extra jobs that exercise metric-check consumers.
use relca_test_support::fixture_match as wire;
use serde_json::{Value, json};

pub const IDS: [&str; 6] = [
    "metric-check-explain",
    "metric-check-diff",
    "metric-check-mutate",
    "metric-check-suite",
    "metric-check-compare-sampled",
    "metric-check-simulation-refusal",
];

pub fn check(id: &str, args: &[String], doc: &Value, exit: i32, stderr: &[u8]) {
    let marker = |v: &Value| {
        assert!(
            v["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("metric_constraints"))
        )
    };
    let family = |r: &Value, rows, alpha| {
        assert_eq!(r["simulation"]["multiplicity"]["rows"], rows);
        assert_eq!(r["simulation"]["multiplicity"]["alpha_after"], alpha);
        let c = &r["metric_checks"][0];
        assert_eq!(c["constraint"], "cohort.OnTime >= 9/10");
        assert_eq!(c["alpha"], alpha);
        assert_eq!(c["verdict"], "inconclusive");
        assert_eq!(r["metric_checks"][1]["reason"], "no_statistical_bound");
    };
    match id {
        "metric-check-explain" => {
            assert_eq!((args[0].as_str(), exit), ("explain", 1));
            assert!(stderr.is_empty());
            wire::validate_explain_wire(doc).unwrap();
            assert_eq!(
                doc["metric_checks"][0]["interval"],
                json!({"lower":"93/2","upper":"93/2"})
            );
            assert_eq!(doc["metric_checks"][0]["verdict"], "violated");
            assert_eq!(doc["verdict"], "violated");
        }
        "metric-check-diff" => {
            assert_eq!((args[0].as_str(), exit), ("diff", 1));
            wire::compare_diff_report(&json!({"verdict":"violated"}), doc).unwrap();
            marker(doc);
            assert_eq!(
                doc["from"]["report"]["metric_checks"][1]["verdict"],
                "satisfied"
            );
            assert_eq!(
                doc["to"]["report"]["metric_checks"][1]["verdict"],
                "violated"
            );
        }
        "metric-check-mutate" => {
            assert_eq!((args[0].as_str(), exit), ("mutate", 4));
            wire::validate_mutation_wire(doc).unwrap();
            marker(doc);
            assert!(doc["original"]["metric_checks"].is_null());
        }
        "metric-check-suite" => {
            assert_eq!((args[0].as_str(), exit), ("sensitivity", 4));
            assert!(stderr.is_empty());
            marker(doc);
            assert_eq!(doc["scope"]["evaluated"], 2);
            for p in doc["points"].as_array().unwrap() {
                family(&p["report"], 4, "1/160");
                wire::validate_suite_simulation_wire(&p["report"], 4).unwrap();
            }
            let _: relca_core::sensitivity::wire::SensitivityReport =
                serde_json::from_value(doc.clone()).unwrap();
            let mut bad = doc.clone();
            bad["unchecked"] = json!(["unknown"]);
            assert!(
                serde_json::from_value::<relca_core::sensitivity::wire::SensitivityReport>(bad)
                    .is_err()
            );
        }
        "metric-check-compare-sampled" => {
            assert_eq!((args[0].as_str(), exit), ("compare", 1));
            assert!(stderr.is_empty());
            wire::validate_compare_wire(doc).unwrap();
            assert_eq!(doc["evaluation"]["statistics"]["multiplicity"]["m"], 4);
            assert_eq!(
                doc["evaluation"]["statistics"]["multiplicity"]["alpha"],
                "1/160"
            );
            for (p, verdict) in doc["plans"]
                .as_array()
                .unwrap()
                .iter()
                .zip(["inconclusive", "violated"])
            {
                let r = &p["report"];
                assert_eq!(r["simulation"]["multiplicity"]["rows"], 2);
                assert_eq!(r["simulation"]["multiplicity"]["alpha_after"], "1/80");
                assert_eq!(r["metric_checks"][0]["alpha"], "1/80");
                assert_eq!(r["metric_checks"][0]["verdict"], verdict);
                assert_eq!(r["properties"][0]["verdict"], "satisfied");
            }
            assert_eq!(doc["evaluation"]["plans"][1]["status"], "excluded");
            assert_eq!(
                doc["evaluation"]["hard"][2]["values"][0]["reason"],
                "no_statistical_bound"
            );
        }
        "metric-check-simulation-refusal" => {
            assert_eq!((args[0].as_str(), exit), ("simulate", 3));
            wire::validate_simulation_wire(doc).unwrap();
            assert_eq!(doc["diagnostics"][0]["code"], "E-SIM-0");
            assert_eq!(doc["quality"]["means"][0]["value"]["reason"], "aborted");
            assert!(
                doc["assumptions"].as_array().unwrap().iter().any(
                    |a| a["field"] == "quality.mean" && a["value"] == "pooled-case-sample-mean"
                )
            );
        }
        _ => unreachable!(),
    }
}
