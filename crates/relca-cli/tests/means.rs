#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::path::PathBuf;

#[test]
fn means_closed_wire_and_hand_expectations() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for name in fixture_match::MEAN_FIXTURES.iter().filter(|n| {
        ["mean-chance", "mean-work", "mean-people", "mean-contention"]
            .iter()
            .any(|p| n.starts_with(p))
    }) {
        let path = root.join(name);
        let expected: Value =
            serde_json::from_str(&std::fs::read_to_string(path.join("expected.json")).unwrap())
                .unwrap();
        let invocation = fixture_match::Invocation::parse(None).unwrap();
        invocation.validate_expected(&expected).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(invocation.args(&path.join("model.relca")))
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&output.stdout).unwrap();
        invocation
            .compare(&expected, &v)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(output.status.code(), Some(0));
        let q = &v["quality"];
        let _: relca_core::quality::Quality = serde_json::from_value(q.clone()).unwrap();
        fixture_match::validate_quality_wire(q, false).unwrap();
        for (p, x) in [
            ("/means/0/measure", json!("unknown")),
            ("/means/0/name", json!("")),
            ("/means/0/property", json!("Absent")),
            ("/means/0/form", json!("unknown")),
            ("/means/0/value/lower", json!("-1")),
            ("/means/0/value/reason", json!("unavailable")),
            ("/means/0/lower_bound", json!("-1")),
        ] {
            let mut bad = q.clone();
            *bad.pointer_mut(p).unwrap() = x;
            assert!(
                fixture_match::validate_quality_wire(&bad, false).is_err(),
                "{name}: {p}"
            );
            assert!(
                serde_json::from_value::<relca_core::quality::Quality>(bad).is_err(),
                "{name}: {p}"
            );
        }
        for p in ["/means/0", "/means/0/value"] {
            for k in q.pointer(p).unwrap().as_object().unwrap().keys() {
                let mut bad = q.clone();
                bad.pointer_mut(p)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(k);
                assert!(
                    fixture_match::validate_quality_wire(&bad, false).is_err(),
                    "{name}: missing {p}/{k}"
                );
            }
        }
    }
}

#[test]
fn mean_population_is_closed_and_wire_order_is_declared() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut names: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_str().unwrap().to_owned())
        .filter(|n| n.starts_with("mean-"))
        .collect();
    names.sort();
    assert_eq!(names, fixture_match::MEAN_FIXTURES);
    let s = std::fs::read_to_string(root.join("mean-chance/model.relca"))
        .unwrap()
        .replace(
            "  metric mean.Lead",
            "  metric quantile.Median = quantile(0.5, case.Any)\n  metric mean.Lead",
        );
    let report = relca_core::report::check(&s);
    let raw = serde_json::to_string(&report).unwrap();
    let q = &raw[raw.find("\"quality\":{").unwrap()..];
    let keys = [
        "properties",
        "cohort",
        "quantiles",
        "means",
        "outcomes_per_arrival",
        "checked",
        "unchecked",
    ];
    let positions: Vec<_> = keys
        .iter()
        .map(|k| q.find(&format!("\"{k}\":")).unwrap())
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]));
    let q: Value = serde_json::to_value(report.quality.unwrap()).unwrap();
    fixture_match::validate_quality_wire(&q, false).unwrap();
}

#[test]
fn busy_sampled_mean_cli_wire_is_closed() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/mean-simulate/model.relca");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("simulate")
        .arg(path)
        .args(["--scenario", "Busy"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    let q = &doc["quality"];
    assert_eq!(
        q["means"][0]["value"],
        json!({"point":"5","observations":6,"censored":0,"reason":null})
    );
    fixture_match::validate_quality_wire(q, true).unwrap();
    for (field, bad) in [
        ("point", json!(null)),
        ("observations", json!(0)),
        ("censored", json!(1)),
        ("reason", json!("truncated")),
    ] {
        let mut bad_q = q.clone();
        bad_q["means"][0]["value"][field] = bad;
        assert!(
            fixture_match::validate_quality_wire(&bad_q, true).is_err(),
            "{field}"
        );
        assert!(
            serde_json::from_value::<relca_core::quality::Quality>(bad_q).is_err(),
            "{field}"
        );
    }
}

#[test]
fn actual_metric_consumer_wire_rejects_a_wrong_family_or_hard_transport() {
    let binary = env!("CARGO_BIN_EXE_relca");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let run = |args: &[&str]| {
        let output = std::process::Command::new(binary)
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap();
        assert!(output.stderr.is_empty());
        (
            output.status.code().unwrap(),
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        )
    };
    let (code, mut simulation) = run(&["simulate", "fixtures/mean-mixed-family/model.relca"]);
    assert_eq!(code, 2);
    fixture_match::validate_simulation_wire(&simulation).unwrap();
    simulation["metric_checks"][0]["alpha"] = json!("1/40");
    assert!(
        fixture_match::validate_simulation_wire(&simulation)
            .unwrap_err()
            .contains("metric check alpha differs from family")
    );
    let (code, mut compare) = run(&[
        "compare",
        "crates/relca-cli/tests/snapshots/inputs/metric-compare-families.relca",
        "--plans",
        "Fast,Slow",
        "--mode",
        "simulation",
    ]);
    assert_eq!(code, 1);
    fixture_match::validate_compare_wire(&compare).unwrap();
    compare["evaluation"]["hard"][2]["values"][0]["verdict"] = json!("satisfied");
    assert!(
        fixture_match::validate_compare_wire(&compare)
            .unwrap_err()
            .contains("mean hard transport or verdict")
    );
    let (code, original) = run(&["check", "fixtures/mean-metric-check/model.relca"]);
    assert_eq!(code, 0);
    let expected: Value = serde_json::from_str(include_str!(
        "../../../fixtures/mean-metric-check/expected.json"
    ))
    .unwrap();
    let invocation = fixture_match::Invocation::parse(None).unwrap();
    invocation.compare(&expected, &original).unwrap();
    let _: relca_core::report::Report = serde_json::from_value(original.clone()).unwrap();
    for key in ["basis", "reason"] {
        let mut bad = original.clone();
        bad["metric_checks"][0][key] = json!("unknown");
        let err = invocation
            .compare(&json!({"verdict":"satisfied"}), &bad)
            .unwrap_err();
        assert!(err.contains(&format!("metric_checks[0].{key}")), "{err}");
        assert!(
            serde_json::from_value::<relca_core::report::Report>(bad)
                .unwrap_err()
                .to_string()
                .contains("unknown metric check word")
        );
    }
    let (code, mut ratio) = run(&[
        "simulate",
        "crates/relca-core/tests/support/cohort-ratio-rows.relca",
    ]);
    assert_eq!(code, 2);
    assert_eq!(ratio["simulation"]["multiplicity"]["rows"], 8);
    assert_eq!(ratio["simulation"]["multiplicity"]["alpha_after"], "1/32");
    fixture_match::validate_simulation_wire(&ratio).unwrap();
    ratio["simulation"]["multiplicity"]["rows"] = json!(4);
    assert!(
        fixture_match::validate_simulation_wire(&ratio)
            .unwrap_err()
            .contains("Bonferroni rows")
    );
}
