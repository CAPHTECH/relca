//! Real binary, U8 front half. New tests must fail for missing meaning, not build errors.
#[path = "support/sensitivity_harness.rs"]
mod h;
#[path = "../../relca-core/tests/support/sensitivity_oracle.rs"]
mod o;
use serde_json::json;
use std::fs;

#[test]
fn green_old_cli_values_stderr_and_exits_are_hand_fixed() {
    for (id, src) in [("base", o::BASE), ("high", o::HIGH), ("miss", o::MISS)] {
        let d = h::dir();
        let p = h::write(&d, "model.relca", src);
        let out = h::cli(&["check", p.to_str().unwrap()]);
        assert_eq!(out.exit, 0);
        assert_eq!(out.stderr, "");
        let hand = o::hand();
        let p = &hand["values"][id]["wrong"];
        assert_eq!(
            o::property(&out.value, o::QUERY)["probability"],
            json!({"min":p,"max":p})
        );
        let core = serde_json::to_value(relca_core::report::check(src)).unwrap();
        assert_eq!(out.value, core);
    }
    let d = h::dir();
    let p = h::write(
        &d,
        "bad.relca",
        o::BASE.replace("0.20: pass; 0.80: fail", "0.20: pass"),
    );
    let bad = h::cli(&["check", p.to_str().unwrap()]);
    assert_eq!(bad.exit, 3);
    assert!(bad.stderr.contains("E-PROB-0"));
    let p = h::write(&d, "cut.relca", o::BASE.replace("clock 4min", "clock 0min"));
    let cut = h::cli(&["check", p.to_str().unwrap()]);
    assert_eq!(cut.exit, 2);
    assert_eq!(cut.value["bounds"]["truncated"], true);
    let p = h::write(&d, "zero.relca", o::ZERO);
    let sim = h::cli(&["simulate", p.to_str().unwrap()]);
    assert_eq!(sim.exit, 0);
    o::zero_statistics(&sim.value, 1, "1/16", "1/2", "satisfied").unwrap();
}
#[test]
fn red_manifest_three_points_do_not_invent_cartesian_product() {
    let out = h::fixed("values.json", "exhaustive");
    o::suite(
        &out.value,
        &["base", "high", "miss"],
        [3, 0, 0],
        "satisfied",
        Some(("1/250", "1/125")),
    )
    .unwrap();
    assert_eq!(out.exit, 0);
    assert_eq!(out.stderr, "");
    assert_eq!(
        out.value["scope"]["manifest_sha256"],
        o::hand()["values_manifest_sha256"]
    );
    for p in out.value["points"].as_array().unwrap() {
        let id = p["id"].as_str().unwrap();
        let hand = o::hand();
        let want = &hand["values"][id]["wrong"];
        assert_eq!(
            o::property(&p["report"], o::QUERY)["probability"],
            json!({"min":want,"max":want})
        );
        assert_eq!(p["source_sha256"], p["report"]["model"]["sha256"]);
        assert_eq!(p["source_sha256"], hand["source_sha256"][id]);
        assert!(p["source_sha256"].as_str().is_some_and(|s| s.len() == 64));
    }
    let pointers = (0..3)
        .map(|n| format!("/points/{n}/report"))
        .collect::<Vec<_>>();
    o::references(&out.value, &pointers).unwrap();
    let subjects = out.value["ledger"]["subjects"].as_array().unwrap();
    let verdict = subjects.iter().find(|s| s["target"] == "/verdict").unwrap();
    assert_eq!(
        verdict["source_ledgers"],
        json!([
            "/points/0/report/ledger",
            "/points/1/report/ledger",
            "/points/2/report/ledger"
        ])
    );
}
#[test]
fn red_endpoints_are_only_finite_observations_not_worst_or_robust() {
    let out = h::fixed("endpoints.json", "exhaustive");
    o::suite(
        &out.value,
        &["p0", "p1"],
        [2, 0, 0],
        "satisfied",
        Some(("0", "0")),
    )
    .unwrap();
    let d = h::dir();
    for name in ["p0", "phalf", "p1"] {
        h::write(
            &d,
            &format!("{name}.relca"),
            fs::read(h::data().join(format!("{name}.relca"))).unwrap(),
        );
    }
    let m = h::manifest(
        ["p0", "phalf", "p1"]
            .iter()
            .map(|n| h::point(n, &format!("{n}.relca")))
            .collect(),
        false,
    );
    let out = h::run_custom(&d, &m, "exhaustive");
    o::suite(
        &out.value,
        &["p0", "phalf", "p1"],
        [3, 0, 0],
        "satisfied",
        Some(("0", "1/4")),
    )
    .unwrap();
}
#[test]
fn red_suite_bonferroni_changes_child_interval_and_statistical_verdict() {
    let out = h::fixed("zero-pair.json", "simulation");
    h::runner(&out.value);
    assert_eq!(out.exit, 2);
    o::suite(&out.value, &["z0", "z1"], [2, 0, 0], "inconclusive", None).unwrap();
    for p in out.value["points"].as_array().unwrap() {
        o::zero_statistics(&p["report"], 2, "1/32", "607705/1048576", "inconclusive").unwrap();
        assert_eq!(p["report"]["simulation"]["seed"], 0);
        assert_eq!(p["report"]["simulation"]["confidence"], "7/8");
        assert_eq!(p["report"]["simulation"]["multiplicity"]["scope"], "suite");
    }
}
#[test]
fn red_runtime_abort_preserves_planned_rows_not_completed_prefix() {
    let out = h::fixed("abort-pair.json", "simulation");
    h::runner(&out.value);
    assert_eq!(out.exit, 4);
    o::suite(&out.value, &["a0", "a1"], [2, 0, 0], "unsupported", None).unwrap();
    for p in out.value["points"].as_array().unwrap() {
        let r = &p["report"];
        assert_eq!(r["simulation"]["replications"], 4);
        assert_eq!(r["simulation"]["attempted_replications"], 3);
        assert_eq!(r["simulation"]["completed_replications"], 2);
        assert_eq!(r["simulation"]["multiplicity"]["rows"], 4);
        assert_eq!(r["simulation"]["multiplicity"]["alpha_after"], "1/64");
        for q in r["properties"].as_array().unwrap() {
            for c in q["cases"].as_array().unwrap() {
                assert!(c.get("interval").is_none());
            }
        }
    }
}
#[test]
fn red_two_cases_use_four_replications_not_eight_independent_trials() {
    let source = include_str!("../../relca-core/tests/data/sensitivity/zero-two-cases.relca");
    let (d, m) = h::custom(&[("two0", source), ("two1", source)], true);
    let out = h::run_custom(&d, &m, "simulation");
    h::runner(&out.value);
    assert_eq!(out.exit, 2);
    o::suite(
        &out.value,
        &["two0", "two1"],
        [2, 0, 0],
        "inconclusive",
        None,
    )
    .unwrap();
    for p in out.value["points"].as_array().unwrap() {
        let r = &p["report"];
        assert_eq!(r["simulation"]["multiplicity"]["rows"], 4);
        assert_eq!(r["simulation"]["multiplicity"]["alpha_after"], "1/64");
        let cases = o::property(r, o::QUERY)["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 2);
        for case in cases {
            assert_eq!(case["hits"], 0);
            assert_eq!(case["trials"], 4);
            assert_eq!(
                case["interval"],
                json!({"lower":"0","upper":"677849/1048576"})
            );
            assert_eq!(case["verdict"], "inconclusive");
        }
    }
}
#[test]
fn red_clock_cut_keeps_point_and_withdraws_all_point_extrema() {
    let cut = o::BASE.replace("clock 4min", "clock 0min");
    let (d, m) = h::custom(&[("base", o::BASE), ("cut", &cut)], false);
    let out = h::run_custom(&d, &m, "exhaustive");
    o::suite(
        &out.value,
        &["base", "cut"],
        [2, 0, 0],
        "inconclusive",
        None,
    )
    .unwrap();
    assert_eq!(out.exit, 2);
    assert_eq!(
        out.value["points"][1]["report"]["bounds"]["truncated"],
        true
    );
    assert_eq!(out.value["summaries"][0]["reason"], "incomplete_points");
}
#[test]
fn red_missing_complement_refuses_all_points_without_evaluating_prefix() {
    let bad = o::BASE.replace("0.20: pass; 0.80: fail", "0.20: pass");
    let (d, m) = h::custom(
        &[("base", o::BASE), ("bad", &bad), ("miss", o::MISS)],
        false,
    );
    let out = h::run_custom(&d, &m, "exhaustive");
    h::runner(&out.value);
    o::suite(
        &out.value,
        &["base", "bad", "miss"],
        [0, 1, 2],
        "invalid",
        None,
    )
    .unwrap();
    assert_eq!(out.exit, 3);
    assert_eq!(out.value["verdict"], "invalid");
    assert!(out.stderr.contains("E-PROB-0"));
    assert_eq!(out.value["scope"]["planned"], 3);
    assert_eq!(out.value["scope"]["evaluated"], 0);
    assert_eq!(out.value["points"].as_array().unwrap().len(), 3);
    h::no_executed_prefix(&out.value);
    let bad = &out.value["points"][1];
    assert_eq!(bad["status"], "refused");
    assert_eq!(bad["report"]["verdict"], "invalid");
    assert!(bad["report"]["model"]["sha256"].is_string());
}
fn contract_difference(from: &str, to: &str) {
    assert_eq!(o::BASE.matches(from).count(), 1);
    let bad = o::BASE.replacen(from, to, 1);
    let (d, m) = h::custom(&[("base", o::BASE), ("changed", &bad)], false);
    let out = h::run_custom(&d, &m, "exhaustive");
    h::reject(&out, 3, "E-CMP-0", "sensitivity_contract_mismatch");
    h::no_executed_prefix(&out.value);
}
#[test]
fn red_same_question_gate_rejects_threshold_change() {
    contract_difference("<= 0.008", "<= 0.01");
}
#[test]
fn red_same_question_gate_rejects_cut_change() {
    contract_difference(
        "cut C { include [Check] }",
        "cut C { question \"different\"; include [Check] }",
    );
}
#[test]
fn red_same_question_gate_rejects_process_change() {
    contract_difference("if v == pass", "if v == fail");
}

#[test]
fn red_duplicate_unknown_missing_and_empty_manifest_fields_fail_closed() {
    let (d, m) = h::custom(&[("p", o::BASE)], false);
    for mutation in 0..5 {
        let mut bad = m.clone();
        match mutation {
            0 => {
                bad.as_object_mut().unwrap().remove("confidence");
            }
            1 => bad["extra"] = json!(true),
            2 => bad["points"] = json!([]),
            3 => bad["points"][0]["id"] = json!(""),
            _ => bad["points"]
                .as_array_mut()
                .unwrap()
                .push(h::point("p", "p0.relca")),
        }
        let out = h::run_custom(&d, &bad, "exhaustive");
        h::runner(&out.value);
        assert_eq!(out.exit, 3);
        h::no_executed_prefix(&out.value);
    }
    let raw = serde_json::to_string(&m).unwrap().replacen(
        "{",
        "{\"schema\":\"relca-sensitivity/v1\",",
        1,
    );
    let path = h::write(&d, "duplicate.json", raw);
    let out = h::run(&path, "exhaustive");
    h::runner(&out.value);
    assert_eq!(out.exit, 3);
    h::no_executed_prefix(&out.value);
}
#[test]
fn red_future_schema_and_implicit_cartesian_request_are_unsupported() {
    let (d, m) = h::custom(&[("p", o::BASE)], false);
    let mut bad = m.clone();
    bad["schema"] = json!("relca-sensitivity/v2");
    let out = h::run_custom(&d, &bad, "exhaustive");
    h::reject(&out, 4, "U-SYN-0", "sensitivity_schema_unsupported");
    let mut bad = m;
    bad["scope"]["kind"] = json!("cartesian_product");
    let out = h::run_custom(&d, &bad, "exhaustive");
    h::reject(&out, 4, "U-SYN-0", "sensitivity_schema_unsupported");
}
#[test]
fn red_unresolved_target_and_mode_confidence_fail_closed() {
    let (d, m) = h::custom(&[("p", o::BASE)], false);
    let mut bad = m.clone();
    bad["targets"][0]["id"] = json!("probability reached #99");
    let out = h::run_custom(&d, &bad, "exhaustive");
    h::reject(&out, 3, "E-NAME-0", "sensitivity_target_invalid");
    let mut bad = m;
    bad["confidence"] = json!("7/8");
    let out = h::run_custom(&d, &bad, "exhaustive");
    h::runner(&out.value);
    assert_eq!(out.exit, 3);
}
#[test]
fn red_paths_are_local_and_missing_input_is_not_a_zero_success_point() {
    let (d, m) = h::custom(&[("p", o::BASE)], false);
    for file in [
        "../escape.relca",
        "https://example.invalid/model.relca",
        "/absolute.relca",
        "missing.relca",
    ] {
        let mut bad = m.clone();
        bad["points"][0]["model"] = json!(file);
        let out = h::run_custom(&d, &bad, "exhaustive");
        h::reject(&out, 3, "E-CON-0", "sensitivity_input_unavailable");
        h::no_executed_prefix(&out.value);
    }
    #[cfg(unix)]
    {
        let outer = h::dir();
        let target = h::write(&outer, "other.relca", o::BASE);
        std::os::unix::fs::symlink(target, d.join("link.relca")).unwrap();
        let mut bad = m;
        bad["points"][0]["model"] = json!("link.relca");
        let out = h::run_custom(&d, &bad, "exhaustive");
        h::reject(&out, 3, "E-CON-0", "sensitivity_input_unavailable");
    }
}
#[test]
fn red_point_limit_257_is_refused_not_a_256_point_prefix() {
    let d = h::dir();
    h::write(&d, "model.relca", o::BASE);
    let m = h::manifest(
        (0..257)
            .map(|i| h::point(&format!("p{i}"), "model.relca"))
            .collect(),
        false,
    );
    let out = h::run_custom(&d, &m, "exhaustive");
    h::reject(&out, 4, "U-SEM-0", "sensitivity_capacity_limit");
    h::no_executed_prefix(&out.value);
}
#[test]
fn red_point_limit_256_is_an_allowed_declared_scope() {
    let d = h::dir();
    h::write(&d, "model.relca", o::BASE);
    let m = h::manifest(
        (0..256)
            .map(|i| h::point(&format!("p{i}"), "model.relca"))
            .collect(),
        false,
    );
    let out = h::run_custom(&d, &m, "exhaustive");
    h::runner(&out.value);
    assert_eq!(out.exit, 0);
    assert_eq!(out.value["scope"]["planned"], 256);
    assert_eq!(out.value["scope"]["evaluated"], 256);
    assert_eq!(out.value["points"].as_array().unwrap().len(), 256);
}
#[test]
fn red_manifest_byte_limit_inclusive_and_one_byte_over() {
    let (d, m) = h::custom(&[("p", o::BASE)], false);
    let mut bytes = serde_json::to_vec(&m).unwrap();
    bytes.resize(1 << 20, b' ');
    let path = h::write(&d, "manifest.json", &bytes);
    let ok = h::run(&path, "exhaustive");
    h::runner(&ok.value);
    assert_eq!(ok.exit, 0);
    bytes.push(b' ');
    let path = h::write(&d, "manifest.json", &bytes);
    let bad = h::run(&path, "exhaustive");
    h::reject(&bad, 4, "U-SEM-0", "sensitivity_capacity_limit");
    h::no_executed_prefix(&bad.value);
}
#[test]
fn red_source_byte_limit_inclusive_and_one_byte_over() {
    let d = h::dir();
    let mut bytes = o::BASE.as_bytes().to_vec();
    bytes.resize(4 << 20, b' ');
    let m = h::manifest(vec![h::point("p", "model.relca")], false);
    h::write(&d, "model.relca", &bytes);
    let ok = h::run_custom(&d, &m, "exhaustive");
    h::runner(&ok.value);
    assert_eq!(ok.exit, 0);
    bytes.push(b' ');
    h::write(&d, "model.relca", &bytes);
    let bad = h::run_custom(&d, &m, "exhaustive");
    h::reject(&bad, 4, "U-SEM-0", "sensitivity_capacity_limit");
    h::no_executed_prefix(&bad.value);
}
#[test]
fn red_total_source_limit_rejects_valid_individual_sources_without_prefix() {
    let d = h::dir();
    let mut bytes = o::BASE.as_bytes().to_vec();
    bytes.resize(4 << 20, b' ');
    let mut points = Vec::new();
    for n in 0..17 {
        let name = format!("p{n}.relca");
        h::write(&d, &name, &bytes);
        points.push(h::point(&format!("p{n}"), &name));
    }
    let m = h::manifest(points, false);
    let out = h::run_custom(&d, &m, "exhaustive");
    h::reject(&out, 4, "U-SEM-0", "sensitivity_capacity_limit");
    h::no_executed_prefix(&out.value);
}
