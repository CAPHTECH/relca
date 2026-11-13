//! Real CLI, closed wire and negative controls independent of new DTOs.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/requires_acceptance.rs"]
mod hand;
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn binary(src: &str, command: &str, args: &[&str]) -> (Value, i32) {
    let mut p = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg("/dev/stdin")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    p.stdin.take().unwrap().write_all(src.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (
        serde_json::from_slice(&o.stdout).unwrap(),
        o.status.code().unwrap(),
    )
}
#[test]
fn hand_expected_schema_and_old_language_preflight_cover_exactly32_inputs() {
    assert_eq!(hand::NAMES, fixture_match::REQUIRES_FIXTURES);
    for name in hand::NAMES {
        fixture_match::validate_expected(&hand::expected(name))
            .unwrap_or_else(|e| panic!("hand expected {name}: {e}"));
        let (a, _) = binary(&hand::old_skeleton(&hand::source(name)), "check", &[]);
        assert!(
            !["invalid", "unsupported"]
                .iter()
                .any(|v| a["verdict"] == *v),
            "old preflight {name}: {a}"
        );
    }
}
#[test]
fn all32_real_reports_and_expected_exit_codes() {
    for name in hand::NAMES {
        let h = hand::expected(name);
        let (a, exit) = binary(&hand::source(name), "check", &[]);
        hand::subset(&h, &a);
        fixture_match::compare_report(&h, &a).unwrap_or_else(|e| panic!("{name}: {e}"));
        let verdict: relca_core::Verdict = serde_json::from_value(h["verdict"].clone()).unwrap();
        assert_eq!(exit, verdict.exit_code(), "{name}");
    }
}
fn wire() -> Value {
    json!({"mode":"exhaustive","bounds":{"cases":2,"truncated":false},"capability_refusals":[{"case_id":1,"call":"r","capability":"A.run","result":"denied","at":{"min":0,"max":2},"coverage":"complete"}]})
}
#[test]
fn closed_refusal_wire_rejects_unknown_missing_duplicate_wrong_type_and_coverage() {
    let good = wire();
    fixture_match::validate_requires_wire(&good).unwrap();
    for (p, x) in [
        ("/capability_refusals", Value::Null),
        ("/capability_refusals/0/case_id", json!(0)),
        ("/capability_refusals/0/case_id", json!(3)),
        ("/capability_refusals/0/call", json!("")),
        ("/capability_refusals/0/capability", json!("run")),
        ("/capability_refusals/0/result", json!(false)),
        ("/capability_refusals/0/at/min", json!(3)),
        ("/capability_refusals/0/coverage", json!("sampled")),
        ("/capability_refusals/0/coverage", json!("truncated")),
        ("/bounds/cases", Value::Null),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(p).unwrap() = x;
        assert!(
            fixture_match::validate_requires_wire(&bad).is_err(),
            "fault {p}: {bad}"
        );
    }
    for p in ["/capability_refusals/0", "/capability_refusals/0/at"] {
        for key in good.pointer(p).unwrap().as_object().unwrap().keys() {
            let mut bad = good.clone();
            bad.pointer_mut(p)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                fixture_match::validate_requires_wire(&bad).is_err(),
                "missing {p}/{key}"
            );
        }
        let mut bad = good.clone();
        bad.pointer_mut(p)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("surplus".into(), json!(true));
        assert!(fixture_match::validate_requires_wire(&bad).is_err());
    }
    let mut bad = good.clone();
    bad["capability_refusals"][0]["executor"] = json!("p");
    assert!(fixture_match::validate_requires_wire(&bad).is_err());
    let mut bad = good.clone();
    let rows = bad["capability_refusals"].as_array_mut().unwrap();
    rows.push(rows[0].clone());
    assert!(fixture_match::validate_requires_wire(&bad).is_err());
    let mut bad = good.clone();
    bad["capability_refusals"][0]["case_id"] = json!(2);
    let mut r = good["capability_refusals"][0].clone();
    r["case_id"] = json!(1);
    bad["capability_refusals"].as_array_mut().unwrap().push(r);
    assert!(fixture_match::validate_requires_wire(&bad).is_err());
    for (cut, coverage) in [(json!(true), "truncated"), (Value::Null, "aborted")] {
        let mut v = good.clone();
        v["bounds"]["truncated"] = cut;
        v["capability_refusals"][0]["coverage"] = json!(coverage);
        fixture_match::validate_requires_wire(&v).unwrap();
    }
    let mut sampled = good.clone();
    sampled["mode"] = json!("simulation");
    sampled["simulation"] = json!({"completed_replications":4});
    sampled["capability_refusals"][0]["coverage"] = json!("sampled");
    sampled["capability_refusals"][0]["samples"] = json!(4);
    sampled["capability_refusals"][0]["observed_count"] = json!(4);
    fixture_match::validate_requires_wire(&sampled).unwrap();
    for (k, x) in [("observed_count", 0), ("observed_count", 5), ("samples", 3)] {
        let mut b = sampled.clone();
        b["capability_refusals"][0][k] = json!(x);
        assert!(fixture_match::validate_requires_wire(&b).is_err());
    }
}
#[test]
fn new_agency_schema_rejects_requires_but_mutation_wrapper_keeps_it() {
    let mut agency = json!({"rules":"agency-p1-requires-1","origin":[],"lineage":[],"lineage_groups":[],"refusal":[],"unchecked":["executors","via_model"]});
    fixture_match::validate_common_cause_agency(&agency).unwrap();
    agency["unchecked"]
        .as_array_mut()
        .unwrap()
        .push(json!("requires"));
    assert!(fixture_match::validate_common_cause_agency(&agency).is_err());
    let (a, _) = binary(&hand::source("req-start.broken-false"), "mutate", &[]);
    assert!(
        a["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("requires")),
        "wrapper remains about mutation generation"
    );
}
#[test]
fn refusal_reaches_every_nested_report_and_bad_rows_cannot_hide_in_wrappers() {
    let source=hand::source("req-start.broken-false").replace("  bounds","  cut C { include [A] }\n  configuration X { cut C; process Flow }\n  configuration Y { cut C; process Flow }\n  bounds");
    for (command, args, paths) in [
        ("check", vec!["--configuration", "X"], vec![""]),
        // Preserve the original no-selector explain coverage using the
        // single-process source; selected explain is covered separately.
        ("explain", vec![], vec![""]),
        (
            "diff",
            vec!["--from", "X", "--to", "Y"],
            vec!["/from/report", "/to/report"],
        ),
        (
            "compare",
            vec!["--plans", "X,Y", "--mode", "exhaustive"],
            vec!["/plans/0/report", "/plans/1/report"],
        ),
    ] {
        let single = hand::source("req-start.broken-false");
        let (a, exit) = binary(
            if command == "explain" {
                &single
            } else {
                &source
            },
            command,
            &args,
        );
        assert_eq!(exit, 1, "{command}: {a}");
        for p in paths {
            let r = a.pointer(p).unwrap();
            assert_eq!(r["capability_refusals"][0]["result"], "denied");
            fixture_match::validate_requires_wire(r).unwrap();
            let mut bad = a.clone();
            bad.pointer_mut(p).unwrap()["capability_refusals"][0]["case_id"] = json!(0);
            let comparison = match command {
                "diff" => fixture_match::compare_diff_report(&json!({}), &bad),
                "compare" => fixture_match::compare_compare_report(&json!({}), &bad),
                "explain" => fixture_match::validate_explain_wire(&bad),
                _ => fixture_match::compare_report(&json!({}), &bad),
            };
            assert!(comparison.is_err(), "nested bad row {command}{p}");
        }
    }
    // Satisfied original: unlike req-start.broken-false this does not stop at
    // mutation's original-verdict gate, so executed mutant reports are required.
    let (m, _) = binary(&hand::source("req-agency"), "mutate", &[]);
    assert_eq!(m["original"]["capability_refusals"][0]["result"], "denied");
    fixture_match::validate_mutation_wire(&m).unwrap();
    let mut executed = 0;
    for (i, kind) in m["mutations"].as_array().unwrap().iter().enumerate() {
        for (j, mutant) in kind["mutants"].as_array().unwrap().iter().enumerate() {
            if mutant["report"].is_object() {
                if !mutant["report"]["bounds"]["cases"].is_u64() {
                    assert!(
                        mutant["report"].get("capability_refusals").is_none(),
                        "static failure must omit new runtime table"
                    );
                    continue;
                }
                executed += 1;
                assert!(
                    mutant["report"].get("capability_refusals").is_some(),
                    "selected Post retains requires"
                );
                let mut bad = m.clone();
                // Use an explicit malformed row even when this mutant happens
                // not to refuse. No new empty-table exemption at nested depth.
                bad["mutations"][i]["mutants"][j]["report"]["capability_refusals"] = json!([{"case_id":0,"call":"p","capability":"Post.post","result":"denied","at":{"min":2,"max":2},"coverage":"complete"}]);
                assert!(fixture_match::validate_mutation_wire(&bad).is_err());
            }
        }
    }
    assert!(executed > 0, "must exercise executed nested mutant reports");
    let mut b = m;
    b["original"]["capability_refusals"][0]["coverage"] = json!("sampled");
    assert!(fixture_match::validate_mutation_wire(&b).is_err());
}

#[test]
fn new_metric_scalar_fields_have_closed_types_at_each_depth() {
    for (name, pointers) in [
        (
            "req-start.broken-false",
            vec!["/metrics/cases/0/tasks/0/refused_count"],
        ),
        (
            "req-wait-gap",
            vec![
                "/metrics/overall/intentional_wait",
                "/metrics/cases/0/intentional_wait",
            ],
        ),
    ] {
        let good = hand::expected(name);
        fixture_match::validate_expected(&good).unwrap();
        for p in pointers {
            let mut bad = good.clone();
            bad.pointer_mut(p)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("surplus".into(), json!(true));
            assert!(fixture_match::validate_expected(&bad).is_err());
            let mut bad = good.clone();
            bad.pointer_mut(p).unwrap()["min"] = json!({"value":0});
            assert!(fixture_match::validate_expected(&bad).is_err());
        }
    }
}

#[test]
fn contest_refused_reason_is_closed_and_cannot_be_a_normal_response() {
    // Independent wire control, not an expectation harvested from the runtime.
    let good = json!({"bounds":{"cases":1,"truncated":false},"properties":[{"id":"contract C #1 contest","verdict":"satisfied"},{"id":"contract C #1 contest_response","verdict":"violated"}],"contest_summary":{"business_ended_open":{"min":"0","max":"0","reason":null}},"contest_runtime":[{"id":"case1:contest1","static_id":"contract C #1 contest","case_id":1,"party":"Party","target":"returned","route":"Route.respond","respond_by":2,"basis":"モデル内での期限内応答","coverage":"complete","alternatives":[{"filed":true,"target_at":1,"filed_at":1,"deadline":3,"status":"violated","response_at":null,"closed_at":null,"executor":null,"result":null,"violation_reason":"refused","route_work":{"min":0,"max":0},"route_wait":{"min":0,"max":0},"route_cost":{"min":null,"max":null,"reason":"not_configured"}}]}]});
    fixture_match::validate_contest_runtime_wire(&good).unwrap();
    for (key, value) in [
        ("violation_reason", json!("guard_false")),
        ("response_at", json!(1)),
        ("result", json!("fail")),
        ("closed_at", json!(1)),
    ] {
        let mut bad = good.clone();
        bad["contest_runtime"][0]["alternatives"][0][key] = value;
        assert!(
            fixture_match::validate_contest_runtime_wire(&bad).is_err(),
            "refusal cannot masquerade as response {key}"
        );
    }
}

#[test]
fn real_explain_refuse_stage_rejects_executor_and_synthetic_finish() {
    let s=hand::source("req-start.broken-false").replace("  bounds", "  contract Trace { response case.received -> one_of(A.run.finish(denied)) by 5min after case.received otherwise unresolved }\n  bounds");
    let (good, exit) = binary(&s, "explain", &[]);
    assert_eq!(exit, 1, "{good}");
    fixture_match::validate_explain_wire(&good).unwrap();
    let cs = good["explanation"]["counterexamples"].as_array().unwrap();
    let (ci, c) = cs
        .iter()
        .enumerate()
        .find(|(_, c)| c["id"] == "contract Trace #1 response")
        .unwrap();
    let (si, _) = c["steps"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, s)| s["kind"] == "refuse")
        .unwrap();
    let mut bad = good.clone();
    bad["explanation"]["counterexamples"][ci]["steps"][si]["executor"] = json!("p");
    assert!(fixture_match::validate_explain_wire(&bad).is_err());
    let mut bad = good;
    let stage = &mut bad["explanation"]["counterexamples"][ci]["steps"][si];
    let event = json!({"kind":"finish","case_id":1,"binding":"r","actor":"A","capability":"A.run","executor":null,"result":"denied","location":stage["location"]});
    stage["events"].as_array_mut().unwrap().push(event);
    assert!(fixture_match::validate_explain_wire(&bad).is_err());
}

#[test]
fn simulation_refusals_are_selected_samples_not_candidate_visits() {
    let s = hand::source("req-kernel").replace(
        "  bounds",
        "  simulation replications 4 seed 0 confidence 0.5\n  bounds",
    );
    let (a, _) = binary(&s, "simulate", &[]);
    assert_eq!(a["simulation"]["completed_replications"], 4, "{a}");
    hand::subset(
        &json!([{"case_id":1,"call":"r","result":"denied","observed_count":4,"samples":4,"at":{"min":0,"max":0},"coverage":"sampled"}]),
        &a["capability_refusals"],
    );
    fixture_match::validate_simulation_wire(&a).unwrap();
    let (n, exit) = binary(
        &hand::source("req-same-time").replace(
            "  bounds",
            "  simulation replications 4 seed 0 confidence 0.5\n  bounds",
        ),
        "simulate",
        &[],
    );
    assert_eq!(exit, 4);
    assert_eq!(n["simulation"]["completed_replications"], 0);
    assert_eq!(n["capability_refusals"], json!([]));
}
