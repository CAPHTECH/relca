//! Real binary acceptance and closed-wire fault calibration for ADR 0021.
#[path = "support/current_expectations.rs"]
mod current_expectations;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .output()
        .unwrap()
}
fn model(name: &str) -> String {
    root()
        .join("fixtures")
        .join(name)
        .join("model.relca")
        .to_str()
        .unwrap()
        .to_owned()
}

#[test]
fn x_cli_all_twelve_full_ordered_paths_and_projection() {
    for name in fixture_match::EXPLAIN_FIXTURES {
        let path = model(name);
        let mut expected: Value = serde_json::from_str(
            &std::fs::read_to_string(root().join("fixtures").join(name).join("expected.json"))
                .unwrap(),
        )
        .unwrap();
        let before = run(&["check", &path]);
        let output = run(&["explain", &path]);
        let after = run(&["check", &path]);
        assert_eq!(before.stdout, after.stdout);
        assert_eq!(before.stderr, after.stderr);
        assert_eq!(before.status.code(), after.status.code());
        let mut actual: Value = serde_json::from_slice(&output.stdout).unwrap();
        let source = std::fs::read_to_string(&path).unwrap();
        current_expectations::migrate(name, &source, &mut expected);
        current_expectations::assert_k4(name, &source, &actual);
        fixture_match::Invocation::Explain
            .compare(&expected, &actual)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(output.status.code(), before.status.code());
        assert!(output.stdout.ends_with(b"\n"));
        actual.as_object_mut().unwrap().remove("explanation");
        assert_eq!(
            fixture_match::original_report_fields(&actual),
            fixture_match::original_report_fields(
                &serde_json::from_slice::<Value>(&before.stdout).unwrap()
            )
        );
        assert_eq!(
            output.stderr, before.stderr,
            "no extra replay diagnostics in successful fixtures"
        );
    }
}

#[test]
fn x_cli_arguments_and_input_failures_have_complete_blocked_explain_reports() {
    let path = model("explain-response");
    for args in [
        vec!["explain"],
        vec!["explain", ""],
        vec!["explain", "--bad"],
        vec!["explain", &path, "extra"],
        vec!["explain", &path, "--mode", "simulation"],
        vec!["explain", &path, "--configuration", "Flow"],
        vec!["explain", &path, "--limit", "1"],
        vec!["explain", "/no/such/relca-explain-input"],
        vec!["explain", &path, "--seed", "0"],
        vec!["explain", &path, "--budget", "1"],
    ] {
        let out = run(&args);
        assert_eq!(out.status.code(), Some(3), "{args:?}");
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        fixture_match::validate_explain_wire(&v).unwrap();
        assert_eq!(v["verdict"], "invalid");
        assert_eq!(v["explanation"]["status"], "blocked");
        assert_eq!(v["explanation"]["counterexamples"], json!([]));
        assert!(v["model"]["sha256"].is_null());
    }
    let conf = model("config-diff-parallel.broken-no-configuration");
    let out = run(&["explain", &conf]);
    assert_eq!(out.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    fixture_match::validate_explain_wire(&v).unwrap();
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CONF-0")
    );
}

#[test]
fn x_cli_invocation_is_closed_no_selector_or_mode() {
    let inv = fixture_match::Invocation::parse(Some(&json!({"command":"explain"}))).unwrap();
    assert_eq!(inv, fixture_match::Invocation::Explain);
    assert_eq!(
        inv.args(Path::new("model.relca")),
        ["explain", "model.relca"]
    );
    for key in ["configuration", "mode", "limit", "seed", "budget"] {
        let mut v = json!({"command":"explain"});
        v[key] = json!("x");
        assert!(fixture_match::Invocation::parse(Some(&v)).is_err());
    }
}

// This is a hand-authored complete report around an ordinary check projection,
// never obtained from explain. It calibrates the validator before implementation.
fn wire() -> Value {
    let mut v=serde_json::to_value(relca_core::report::check("model M version \"0.2\" { process Flow { end done } bounds { cases 1; live_tasks 1; clock 1min } verify safety no_resource_overcommit }")).unwrap();
    let l = json!({"span":{"start":1,"end":2},"line":1,"column":2,"end_line":1,"end_column":3});
    let violation = json!({"id":"contract Controls #1 invariant","property_location":l,"at_step":1,"trigger_step":null,"deadline":null,"related_steps":[]});
    let s = json!({"index":0,"time":0,"case_id":1,"binding":null,"actor":null,"capability":null,"executor":null,"result":null,"kind":"initial","events":[{"kind":"received","case_id":1,"binding":null,"actor":null,"capability":null,"executor":null,"result":null,"location":l}],"location":l,"values":{"observed":[],"latent":[{"name":"truth","value":"false","visibility":"latent","location":l}]},"violation":null,"resource":null,"capability_location":null,"deadlock_detail":null});
    let mut last = s.clone();
    last["index"] = json!(1);
    last["kind"] = json!("finish");
    last["violation"] = violation;
    let resource = json!({"actor":"A","capacity":1,"anonymous":false,"holders":[{"case_id":1,"binding":"x","executor":"p"}]});
    last["resource"] = resource.clone();
    last["capability_location"] = l.clone();
    last["deadlock_detail"] = json!({"waiting_tasks":[{"case_id":1,"binding":"x","actor":"A"}],"resources":[resource],"obligations":[{"case_id":1,"id":"contract Controls #1 invariant","trigger_step":0,"deadline":5}],"joins":[{"case_id":1,"bindings":["x"]}]});
    v["verdict"] = json!("violated");
    v["properties"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"contract Controls #1 invariant","verdict":"violated"}));
    v["explanation"] = json!({"version":"invoice-counterexample-v1","status":"complete","shortest_scope":"model","counterexamples":[{"id":"contract Controls #1 invariant","case_id":1,"status":"verified","reason":null,"property_location":l,"context_location":l,"related_locations":[l],"transition_count":1,"steps":[s,last],"replay":{"status":"verified","reason":null}}],"omitted":[],"assumptions":[{"field":"explain.purpose","value":"example_not_cause","affects":"example"},{"field":"explain.shortest","value":"model","affects":"edges"},{"field":"explain.latent","value":"analyst_only","affects":"not policy"},{"field":"explain.replay","value":"independent_reference_prefix","affects":"legality"}],"diagnostics":[],"checked":[],"unchecked":["probabilistic_causes","simulation_counterexamples","real_world_validity"]});
    v
}

#[test]
fn x_wire_rejects_unknown_missing_and_corrupt_nested_fields_before_production_exists() {
    let v = wire();
    fixture_match::validate_explain_wire(&v).unwrap();
    fn objects(v: &Value, p: String, out: &mut Vec<String>) {
        match v {
            Value::Object(o) => {
                out.push(p.clone());
                for (k, x) in o {
                    objects(x, format!("{p}/{k}"), out)
                }
            }
            Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    objects(x, format!("{p}/{i}"), out)
                }
            }
            _ => {}
        }
    }
    let mut ps = vec![];
    objects(&v, String::new(), &mut ps);
    for p in ps {
        let mut bad = v.clone();
        bad.pointer_mut(&p)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("trace_probability".into(), json!(1));
        assert!(
            fixture_match::validate_explain_wire(&bad).is_err(),
            "unknown {p}"
        );
        for key in v.pointer(&p).unwrap().as_object().unwrap().keys() {
            // Ordinary report optional keys are checked by its existing tests.
            if !p.starts_with("/explanation") {
                continue;
            }
            let mut bad = v.clone();
            bad.pointer_mut(&p)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                fixture_match::validate_explain_wire(&bad).is_err(),
                "missing {p}/{key}"
            );
        }
    }
    for (p, x) in [
        (
            "/explanation/counterexamples/0/steps/1/kind",
            json!("made_up"),
        ),
        (
            "/explanation/counterexamples/0/reason",
            json!("recording_limit"),
        ),
        ("/explanation/counterexamples/0/steps/1/index", json!(-1)),
        ("/explanation/counterexamples/0/steps/1/index", json!(2)),
        ("/explanation/counterexamples/0/steps/1/time", json!(-1)),
        ("/explanation/counterexamples/0/transition_count", json!(2)),
        (
            "/explanation/counterexamples/0/replay/status",
            json!("unsupported"),
        ),
        (
            "/explanation/counterexamples/0/steps/1/location/span/end",
            json!(0),
        ),
        (
            "/explanation/counterexamples/0/steps/0/values/latent/0/visibility",
            json!("observed"),
        ),
        (
            "/explanation/counterexamples/0/steps/1/violation/at_step",
            json!(0),
        ),
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(p).unwrap() = x;
        assert!(fixture_match::validate_explain_wire(&bad).is_err(), "{p}");
    }
    let mut bad = v.clone();
    let c = bad["explanation"]["counterexamples"][0].clone();
    bad["explanation"]["counterexamples"]
        .as_array_mut()
        .unwrap()
        .push(c);
    assert!(fixture_match::validate_explain_wire(&bad).is_err());
    let mut bad = v.clone();
    bad["explanation"]["counterexamples"][0]["status"] = json!("unsupported");
    assert!(fixture_match::validate_explain_wire(&bad).is_err());
    let mut bad = v.clone();
    bad.as_object_mut().unwrap().remove("model");
    assert!(fixture_match::validate_explain_wire(&bad).is_err());
}
