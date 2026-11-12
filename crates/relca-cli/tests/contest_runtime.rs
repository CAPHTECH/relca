//! Real binary acceptance: expectations are not derived from its verdict.
#[path = "../../relca-core/tests/contest_runtime.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use std::process::Command;

#[test]
fn all_twelve_hand_expectations_and_existing_syntax_are_legal_before_runtime_implementation() {
    assert_eq!(acceptance::NAMES, fixture_match::CONTEST_RUNTIME_FIXTURES);
    for name in acceptance::NAMES {
        fixture_match::validate_expected(&acceptance::expected(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        relca_core::lower::lower(&relca_core::syntax::parse(&acceptance::source(name)).unwrap())
            .unwrap();
    }
}
#[test]
fn twelve_real_binary_reports_and_exit_codes() {
    for name in acceptance::NAMES {
        let expected = acceptance::expected(name);
        let output = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("check")
            .arg(acceptance::root().join(name).join("model.relca"))
            .output()
            .unwrap();
        let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        acceptance::subset(&expected, &actual);
        fixture_match::compare_report(&expected, &actual).unwrap();
        let verdict: relca_core::Verdict =
            serde_json::from_value(expected["verdict"].clone()).unwrap();
        assert_eq!(output.status.code(), Some(verdict.exit_code()), "{name}");
    }
}

fn hand_wire() -> serde_json::Value {
    serde_json::json!({
        "bounds":{"cases":1,"truncated":false},
        "contest_summary":{"business_ended_open":{"min":"0","max":"0","reason":null}},
        "properties":[{"id":"contract Controls #1 contest","verdict":"satisfied"},{"id":"contract Controls #1 contest_response","verdict":"satisfied"}],
        "contest_runtime":[{"id":"case1:contest1","static_id":"contract Controls #1 contest","case_id":1,"party":"Party","target":"returned","route":"Route.respond","respond_by":2,"basis":"モデル内での期限内応答","coverage":"complete","alternatives":[
            {"filed":false,"target_at":1,"filed_at":null,"deadline":null,"status":"not_filed","response_at":null,"closed_at":1,"executor":null,"result":null,"violation_reason":null,"route_work":null,"route_wait":null,"route_cost":{"min":null,"max":null,"reason":"never_created"}},
            {"filed":true,"target_at":1,"filed_at":1,"deadline":3,"status":"responded","response_at":3,"closed_at":3,"executor":"p","result":"pass","violation_reason":null,"route_work":{"min":2,"max":2},"route_wait":{"min":0,"max":0},"route_cost":{"min":null,"max":null,"reason":"not_configured"}}
        ]}]
    })
}

#[test]
fn a11_closed_runtime_schema_positive_and_single_fault_controls() {
    use serde_json::{Value, json};
    let good = hand_wire();
    fixture_match::validate_contest_runtime_wire(&good).unwrap();
    for key in ["contest_runtime", "contest_summary"] {
        let mut bad = good.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(
            fixture_match::validate_contest_runtime_wire(&bad).is_err(),
            "missing {key}"
        );
    }
    let mut late = good.clone();
    late["contest_runtime"][0]["alternatives"][1]["status"] = json!("violated");
    late["contest_runtime"][0]["alternatives"][1]["violation_reason"] = json!("late_response");
    late["contest_runtime"][0]["alternatives"][1]["response_at"] = json!(4);
    late["contest_runtime"][0]["alternatives"][1]["closed_at"] = json!(4);
    fixture_match::validate_contest_runtime_wire(&late).unwrap();
    late["contest_runtime"][0]["alternatives"][1]["response_at"] = json!(3);
    late["contest_runtime"][0]["alternatives"][1]["closed_at"] = json!(3);
    assert!(
        fixture_match::validate_contest_runtime_wire(&late).is_err(),
        "equality is not a late response"
    );
    let mut deadlock = good.clone();
    let filed = &mut deadlock["contest_runtime"][0]["alternatives"][1];
    filed["status"] = json!("violated");
    filed["violation_reason"] = json!("deadlock");
    for key in [
        "response_at",
        "closed_at",
        "executor",
        "result",
        "route_work",
        "route_wait",
    ] {
        filed[key] = Value::Null;
    }
    deadlock["contest_summary"]["business_ended_open"]["max"] = json!("1");
    fixture_match::validate_contest_runtime_wire(&deadlock).unwrap();
    let mut cut = good.clone();
    cut["bounds"]["truncated"] = json!(true);
    cut["contest_runtime"][0]["coverage"] = json!("truncated");
    cut["contest_summary"]["business_ended_open"] =
        json!({"min":null,"max":null,"reason":"truncated"});
    let filed = &mut cut["contest_runtime"][0]["alternatives"][1];
    filed["status"] = json!("unresolved");
    for key in [
        "response_at",
        "closed_at",
        "result",
        "route_work",
        "route_wait",
    ] {
        filed[key] = Value::Null;
    }
    filed["route_cost"] = json!({"min":null,"max":null,"reason":"truncated"});
    fixture_match::validate_contest_runtime_wire(&cut).unwrap();
    for pointer in [
        "/contest_runtime/0",
        "/contest_runtime/0/alternatives/1",
        "/contest_runtime/0/alternatives/1/route_work",
        "/contest_runtime/0/alternatives/1/route_wait",
        "/contest_runtime/0/alternatives/1/route_cost",
        "/contest_summary",
        "/contest_summary/business_ended_open",
    ] {
        let mut bad = good.clone();
        bad.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("surplus".into(), json!(true));
        assert!(
            fixture_match::validate_contest_runtime_wire(&bad).is_err(),
            "extra {pointer}"
        );
        for key in good.pointer(pointer).unwrap().as_object().unwrap().keys() {
            let mut bad = good.clone();
            bad.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                fixture_match::validate_contest_runtime_wire(&bad).is_err(),
                "missing {pointer}/{key}"
            );
        }
    }
    for (pointer, value) in [
        ("/contest_runtime", Value::Null),
        ("/contest_summary", Value::Null),
        ("/contest_summary/business_ended_open/min", json!("2")),
        ("/contest_summary/business_ended_open/max", json!("2")),
        ("/contest_summary/business_ended_open/min", json!("1/2")),
        ("/contest_runtime/0/id", json!("case2:contest1")),
        ("/contest_runtime/0/case_id", json!(0)),
        (
            "/contest_runtime/0/static_id",
            json!("contract Controls #2 contest"),
        ),
        ("/contest_runtime/0/coverage", json!("sampled")),
        (
            "/contest_runtime/0/alternatives/1/status",
            json!("accepted"),
        ),
        (
            "/contest_runtime/0/alternatives/1/status",
            json!("not_filed"),
        ),
        ("/contest_runtime/0/alternatives/1/filed_at", json!(2)),
        ("/contest_runtime/0/alternatives/1/deadline", json!(4)),
        ("/contest_runtime/0/alternatives/1/response_at", json!(4)),
        ("/contest_runtime/0/alternatives/1/result", Value::Null),
        ("/contest_runtime/0/alternatives/1/closed_at", Value::Null),
        ("/contest_runtime/0/alternatives/0/response_at", json!(3)),
        ("/contest_runtime/0/alternatives/1/route_work/min", json!(3)),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            fixture_match::validate_contest_runtime_wire(&bad).is_err(),
            "fault {pointer}"
        );
    }
    for pointer in ["/contest_runtime", "/contest_runtime/0/alternatives"] {
        let mut bad = good.clone();
        let rows = bad.pointer_mut(pointer).unwrap().as_array_mut().unwrap();
        rows.push(rows[0].clone());
        assert!(
            fixture_match::validate_contest_runtime_wire(&bad).is_err(),
            "duplicate {pointer}"
        );
    }
}

#[test]
fn a11_diff_compare_mutate_nested_reports_cannot_bypass_runtime_validation() {
    use serde_json::json;
    let src=acceptance::source("ctx-contest").replace("  bounds", "  configuration A { cut C; process Flow }\n  configuration B { cut C; process Flow }\n  bounds");
    let d = serde_json::to_value(relca_core::diff::diff(&src, "A", "B")).unwrap();
    assert!(
        d["from"]["report"].get("contest_runtime").is_some(),
        "D2 nested runtime required"
    );
    fixture_match::compare_diff_report(&json!({}), &d).unwrap();
    for side in ["from", "to"] {
        let mut bad = d.clone();
        bad[side]["report"]["contest_runtime"][0]["alternatives"][0]["status"] = json!("accepted");
        assert!(fixture_match::compare_diff_report(&json!({}), &bad).is_err());
    }
    let c = serde_json::to_value(relca_core::compare::compare(
        &src,
        &["A", "B"],
        relca_core::compare::Analysis::Exhaustive,
    ))
    .unwrap();
    fixture_match::compare_compare_report(&json!({}), &c).unwrap();
    for plan in 0..2 {
        let mut bad = c.clone();
        bad["plans"][plan]["report"]["contest_runtime"][0]["case_id"] = json!(0);
        assert!(fixture_match::compare_compare_report(&json!({}), &bad).is_err());
    }
    let m = serde_json::to_value(relca_core::mutation::mutate(&acceptance::source(
        "ctx-contest",
    )))
    .unwrap();
    assert!(m["original"].get("contest_runtime").is_some());
    fixture_match::validate_mutation_wire(&m).unwrap();
    let mut bad = m;
    bad["original"]["contest_runtime"][0]["coverage"] = json!("sampled");
    assert!(fixture_match::validate_mutation_wire(&bad).is_err());
}
