//! ADR 0029 CLI, wire, simulation and source-mapped counterexample acceptance.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/integer_acceptance.rs"]
mod hand;
use serde_json::{Value, json};
use std::process::Command;
fn run(command: &str, name: &str) -> (Value, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args([command, &format!("fixtures/{name}/model.relca")])
        .current_dir(hand::root())
        .output()
        .unwrap();
    (
        serde_json::from_slice(&output.stdout).unwrap(),
        output.status.code().unwrap(),
    )
}
fn code(v: &Value) -> i32 {
    serde_json::from_value::<relca_core::Verdict>(v["verdict"].clone())
        .unwrap()
        .exit_code()
}
fn run_source(command: &str, source: &str, args: &[&str]) -> (Value, i32) {
    static ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = hand::root().join("target").join(format!(
        "int-acceptance-{}-{}.relca",
        std::process::id(),
        ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    // Leave generated input in ignored target/ for reproducing failed assertions.
    std::fs::write(&path, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(path)
        .args(args)
        .current_dir(hand::root())
        .output()
        .unwrap();
    (
        serde_json::from_slice(&output.stdout).unwrap(),
        output.status.code().unwrap(),
    )
}
#[test]
fn all56_expectations_are_closed_before_the_binary_is_consulted() {
    for name in hand::names() {
        fixture_match::validate_expected(&hand::expected(&name))
            .unwrap_or_else(|e| panic!("{name}: authored schema: {e}"));
    }
}
#[test]
fn all56_cli_reports_and_exit_match_hand_expectations() {
    for name in hand::names() {
        let expected = hand::expected(&name);
        let (v, exit) = run("check", &name);
        fixture_match::compare_report(&expected, &v).unwrap_or_else(|e| panic!("{name}: {e}\n{v}"));
        assert_eq!(exit, code(&expected), "{name}");
    }
}
fn span_text<'a>(s: &'a str, span: &Value) -> &'a str {
    let a = span["start"].as_u64().unwrap() as usize;
    let b = span["end"].as_u64().unwrap() as usize;
    assert!(a < b && b <= s.len() && s.is_char_boundary(a) && s.is_char_boundary(b));
    &s[a..b]
}
/// Messages are fixed expectations (C1..C5), not the
/// binary's output; the old `undefined parameter type` must not come back.
#[test]
fn five_classification_migrations_have_exact_code_severity_message_span_and_exit() {
    for (name, token, message) in [
        (
            "int-class-int.broken-unbounded",
            "int",
            "unbounded int state type is unsupported",
        ),
        (
            "int-class-param.broken-caseid",
            "PersonId",
            "PersonId parameter type is unsupported",
        ),
        (
            "int-class-caseid.broken-reference",
            "case.other",
            "unsupported syntax: case.other argument",
        ),
        (
            "int-class-reads.broken-observed",
            "A.flag",
            "Actor state reads are unsupported",
        ),
        (
            "int-class-stateid.broken-caseid",
            "CaseId",
            "CaseId state type is unsupported",
        ),
    ] {
        // ADR0043 accepts the original observed read. Preserve this five-row
        // classification oracle on the still-unsupported qualified state read.
        let mut s = hand::source(name);
        let (v, exit) = if name == "int-class-reads.broken-observed" {
            let changed = s.replace("reads [flag]", "reads [A.flag]");
            assert_ne!(s, changed);
            s = changed;
            run_source("check", &s, &[])
        } else {
            run("check", name)
        };
        assert_eq!(exit, 4);
        assert_eq!(v["verdict"], "unsupported");
        let errors: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(errors.len(), 1, "{name}: no second fault");
        assert_eq!(errors[0]["code"], "U-SYN-0");
        assert_eq!(errors[0]["message"], message, "{name}");
        assert_eq!(span_text(&s, &errors[0]["span"]), token);
    }
}
#[test]
fn range_witness_exact_attempts_do_not_become_stored_values_or_static_errors() {
    for (name, before, op, operand, attempted, upper, case_id) in [
        (
            "int-upper.broken-overflow",
            "2",
            "+=",
            "1",
            "3",
            "2",
            Value::Null,
        ),
        (
            "int-underflow.broken-underflow",
            "0",
            "-=",
            "1",
            "-1",
            "2",
            Value::Null,
        ),
        (
            "int-set-stop.broken-outward",
            "0",
            "=",
            "3",
            "3",
            "2",
            Value::Null,
        ),
        (
            "int-atomic.broken-overflow",
            "1",
            "+=",
            "1",
            "2",
            "1",
            Value::Null,
        ),
        (
            "int-arrivals.broken-overflow",
            "1",
            "+=",
            "1",
            "2",
            "1",
            json!(1),
        ),
    ] {
        let (v, exit) = run("check", name);
        assert_eq!(exit, 1);
        assert_eq!(v["verdict"], "violated");
        fixture_match::validate_integer_wire(&v).unwrap();
        let ws = v["state_range_violations"].as_array().unwrap();
        assert_eq!(ws.len(), 1);
        let w = &ws[0];
        hand::subset(
            &json!({"case_id":case_id,"time":1,"binding":"a","actor":"A","capability":"inc","phase":"finish","variable":"n","operator":op,"before":before,"operand":operand,"attempted":attempted,"lower":"0","upper":upper}),
            w,
        );
        let s = hand::source(name);
        assert_eq!(span_text(&s, &w["write_span"]), format!("n {op} {operand}"));
        let decl = span_text(&s, &w["declaration_span"]);
        assert!(decl.contains("n:") && decl.contains("0.."), "{decl}");
        assert_eq!(v["bounds"]["truncated"], false);
        assert_eq!(v["bounds"]["range_violation_stops"], true);
        assert_eq!(v["terminals"], json!([]));
    }
}
#[test]
fn explain_range_is_shortest_prewrite_attempt_with_verified_independent_replay() {
    for (name, before, attempted) in [
        ("int-upper.broken-overflow", "2", "3"),
        ("int-underflow.broken-underflow", "0", "-1"),
        ("int-set-stop.broken-outward", "0", "3"),
        ("int-atomic.broken-overflow", "1", "2"),
    ] {
        let (v, exit) = run("explain", name);
        assert_eq!(exit, 1);
        fixture_match::validate_explain_wire(&v).unwrap();
        let cs = v["explanation"]["counterexamples"].as_array().unwrap();
        assert_eq!(cs.len(), 1);
        let c = &cs[0];
        assert_eq!(c["id"], "state_in_range");
        assert_eq!(c["status"], "verified");
        assert_eq!(c["replay"], json!({"status":"verified","reason":null}));
        assert!(c["property_location"].is_null());
        // Initial(received) -> queue -> start -> advance1 -> failed finish.
        assert_eq!(c["transition_count"], 4);
        let steps = c["steps"].as_array().unwrap();
        assert_eq!(
            steps
                .iter()
                .map(|s| s["kind"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["initial", "queue", "start", "advance", "range_violation"]
        );
        let last = steps.last().unwrap();
        assert_eq!(last["time"], 1);
        assert_eq!(last["events"], json!([]));
        assert_eq!(last["state_range_violation"]["before"], before);
        assert_eq!(last["state_range_violation"]["attempted"], attempted);
        assert_eq!(
            last["state_range_violation"],
            v["state_range_violations"][0]
        );
        let values = last["values"]["observed"].as_array().unwrap();
        assert_eq!(
            values.iter().find(|v| v["name"] == "n").unwrap()["value"],
            before
        );
        if name.starts_with("int-atomic") {
            assert_eq!(
                values.iter().find(|v| v["name"] == "flag").unwrap()["value"],
                "false"
            );
        }
        assert!(!last["capability_location"].is_null());
    }
}
fn sample(s: &str) -> String {
    s.replace(
        "bounds {",
        "simulation replications 4 seed 17 confidence 0.9; bounds {",
    )
}
#[test]
fn simulation_safe_is_sample_not_proof_and_range_is_not_cut_deadlock_or_terminal() {
    for (name, verdict, basis, count) in [
        ("int-upper", "inconclusive", "sample", 0),
        ("int-upper.broken-overflow", "violated", "witness", 4),
    ] {
        let s = sample(&hand::source(name));
        let v = serde_json::to_value(relca_core::report::simulate(&s)).unwrap();
        let (cli, exit) = run_source("simulate", &s, &[]);
        assert_eq!(cli, v);
        assert_eq!(exit, code(&v));
        fixture_match::validate_simulation_wire(&v).unwrap();
        let p = hand::property(&v, "state_in_range");
        assert_eq!(p["verdict"], verdict);
        assert_eq!(p["basis"], basis);
        assert_eq!(v["simulation"]["range_stopped_replications"], count);
        assert_eq!(v["simulation"]["truncated_replications"], 0);
        assert_eq!(v["bounds"]["truncated"], false);
        assert_eq!(v["case_outcomes"][0]["range_stopped_count"], count);
        assert_eq!(v["case_outcomes"][0]["deadlocked_count"], 0);
        assert_eq!(v["case_outcomes"][0]["unfinished_count"], 0);
        if count > 0 {
            assert_eq!(v["case_outcomes"][0]["terminals"], json!([]));
        }
        let m = relca_core::lower::lower(&relca_core::syntax::parse(&s).unwrap()).unwrap();
        let a = relca_core::engine::simulate(&m);
        let r = relca_core::reference::simulate(&m);
        assert_eq!(r.stop, relca_core::reference::StopReason::Complete);
        assert_eq!(a.replications.len(), 4);
        assert_eq!(r.outcome.replications.len(), 4);
        for o in [&a, &r.outcome] {
            for replication in &o.replications {
                assert!(replication.cut.is_none());
                assert!(replication.cases[0].deadlock.is_none());
                assert_eq!(replication.cases[0].terminal.is_none(), count > 0);
            }
        }
        assert_eq!(a.replications, r.outcome.replications);
    }
}
#[test]
fn simulation_rejects_nontrivial_nondet_initial_but_not_singleton() {
    let s = sample(&hand::source("int-nondet"));
    let v = serde_json::to_value(relca_core::report::simulate(&s)).unwrap();
    assert_eq!(v["verdict"], "unsupported");
    let singleton = s.replace("0..1", "0..0").replace("h == 1", "h == 0");
    let v = serde_json::to_value(relca_core::report::simulate(&singleton)).unwrap();
    assert_ne!(v["verdict"], "unsupported");
    assert_eq!(
        hand::property(&v, "state_in_range")["verdict"],
        "inconclusive"
    );
}
fn witness() -> Value {
    json!({"case_id":null,"time":1,"binding":"a","actor":"A","capability":"inc","phase":"finish","variable":"n","operator":"+=","before":"2","operand":"1","attempted":"3","lower":"0","upper":"2","write_span":{"start":20,"end":26},"declaration_span":{"start":4,"end":12}})
}
#[test]
fn independent_closed_witness_table_rejects_missing_extra_type_boundary_and_arithmetic_corruption()
{
    let good = witness();
    fixture_match::validate_state_range_violation(&good).unwrap();
    for key in good.as_object().unwrap().keys() {
        let mut bad = good.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(
            fixture_match::validate_state_range_violation(&bad).is_err(),
            "missing {key}"
        );
    }
    for (pointer, replacement) in [
        ("/attempted", json!(3)),
        ("/attempted", json!("2")),
        ("/attempted", json!("4")),
        ("/before", json!("-1")),
        ("/operand", json!("+1")),
        ("/upper", json!("18446744073709551616")),
        ("/operator", json!("*=")),
        ("/phase", json!("commit")),
        ("/case_id", json!(0)),
        ("/write_span/end", json!(0)),
        ("/declaration_span/end", json!(4)),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            fixture_match::validate_state_range_violation(&bad).is_err(),
            "accepted {pointer}: {bad}"
        );
    }
    let mut bad = good.clone();
    bad["extra"] = json!(false);
    assert!(fixture_match::validate_state_range_violation(&bad).is_err());
    let mut huge = good;
    huge["before"] = json!("18446744073709551615");
    huge["upper"] = huge["before"].clone();
    huge["attempted"] = json!("18446744073709551616");
    fixture_match::validate_state_range_violation(&huge).unwrap();
}

#[test]
fn all_command_wrappers_keep_range_fields_in_their_canonical_nested_reports() {
    let bare = hand::source("int-upper.broken-overflow");
    let s = bare.replace("bounds {", "cut C { include [A] }; configuration From { cut C; process P }; configuration To { cut C; process P }; bounds {");
    let (compare, exit) = run_source(
        "compare",
        &s,
        &["--plans", "From,To", "--mode", "exhaustive"],
    );
    assert_eq!(exit, 1);
    fixture_match::validate_compare_wire(&compare).unwrap();
    for p in compare["plans"].as_array().unwrap() {
        fixture_match::validate_integer_wire(&p["report"]).unwrap();
        assert_eq!(
            hand::property(&p["report"], "state_in_range")["verdict"],
            "violated"
        );
    }
    let (diff, exit) = run_source("diff", &s, &["--from", "From", "--to", "To"]);
    assert_eq!(exit, 1);
    fixture_match::compare_diff_report(&json!({}), &diff).unwrap();
    for side in ["from", "to"] {
        assert_eq!(
            hand::property(&diff[side]["report"], "state_in_range")["verdict"],
            "violated"
        );
    }
    let (mutation, _) = run_source("mutate", &bare, &[]);
    fixture_match::validate_mutation_wire(&mutation).unwrap();
    assert_eq!(
        hand::property(&mutation["original"], "state_in_range")["verdict"],
        "violated"
    );
    for row in mutation["mutations"].as_array().unwrap() {
        for mutant in row["mutants"].as_array().unwrap() {
            if mutant["report"].is_object() {
                fixture_match::validate_integer_wire(&mutant["report"]).unwrap();
            }
        }
    }
    let (sampled, _) = run_source(
        "compare",
        &sample(&s),
        &["--plans", "From,To", "--mode", "simulation"],
    );
    fixture_match::validate_compare_wire(&sampled).unwrap();
    for p in sampled["plans"].as_array().unwrap() {
        assert_eq!(
            hand::property(&p["report"], "state_in_range")["basis"],
            "witness"
        );
    }
}

#[test]
fn independent_range_table_rejects_flag_witness_property_and_sample_count_laundering() {
    let valid = json!({"mode":"exhaustive","bounds":{"cases":1,"clock":2,"truncated":false,"range_violation_stops":true},"properties":[{"id":"state_in_range","verdict":"violated"}],"state_range_violations":[witness()]});
    fixture_match::validate_integer_wire(&valid).unwrap();
    for (pointer, value) in [
        ("/bounds/range_violation_stops", json!(false)),
        ("/state_range_violations", json!([])),
        ("/properties/0/verdict", json!("satisfied")),
        ("/state_range_violations/0/time", json!(3)),
        ("/state_range_violations/0/case_id", json!(2)),
    ] {
        let mut bad = valid.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            fixture_match::validate_integer_wire(&bad).is_err(),
            "{pointer}"
        );
    }
    let mut bad = valid.clone();
    bad["properties"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"state_in_range","verdict":"violated"}));
    assert!(fixture_match::validate_integer_wire(&bad).is_err());
    let mut bad = valid.clone();
    bad["properties"][0]["probability"] = json!({"min":"0","max":"1"});
    assert!(fixture_match::validate_integer_wire(&bad).is_err());
    let mut old = valid;
    old["properties"] = json!([]);
    assert!(fixture_match::validate_integer_wire(&old).is_err());
    let sample = json!({"mode":"simulation","bounds":{"cases":1,"clock":2,"truncated":false},"properties":[{"id":"state_in_range","verdict":"inconclusive","basis":"sample"}],"simulation":{"status":"completed","completed_replications":4,"range_stopped_replications":0},"case_outcomes":[{"range_stopped_count":0}]});
    fixture_match::validate_integer_wire(&sample).unwrap();
    let mut bad = sample;
    bad["properties"][0]["verdict"] = json!("satisfied");
    assert!(fixture_match::validate_integer_wire(&bad).is_err());
}
