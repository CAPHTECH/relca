//! CLI RED and mechanical, real-binary skeleton preflight, including inline inputs.
#[allow(dead_code)]
#[path = "../../relca-core/tests/business_metrics.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn binary(s: &str, command: &str, args: &[&str]) -> (Value, i32) {
    let mut p = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg("/dev/stdin")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    p.stdin.take().unwrap().write_all(s.as_bytes()).unwrap();
    let out = p.wait_with_output().unwrap();
    (
        serde_json::from_slice(&out.stdout).unwrap(),
        out.status.code().unwrap(),
    )
}

#[test]
fn m_preflight_every_fixture_and_inline_input_without_metrics_through_current_binary() {
    let mut rows = Vec::new();
    for name in acceptance::FIXTURES {
        let s = acceptance::strip_metrics(&acceptance::source(name));
        let configured = name.starts_with("met-compare") || name.starts_with("met-diff");
        let args: &[&str] = if configured {
            &["--configuration", "AsIs"]
        } else {
            &[]
        };
        for command in if name.starts_with("met-sample") || name.starts_with("met-exclusions") {
            vec!["check", "simulate"]
        } else {
            vec!["check"]
        } {
            let (v, exit) = binary(&s, command, args);
            let codes: Vec<_> = v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["severity"] == "error")
                .map(|d| d["code"].as_str().unwrap())
                .collect();
            rows.push(format!("{name}|{command}|{exit}|{codes:?}"));
            assert!(codes.is_empty(), "{name}: {v}");
        }
        if configured {
            let (v, _) = binary(&s, "check", &["--configuration", "ToBeP"]);
            assert!(
                !v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["severity"] == "error")
            );
        }
    }
    let static_inputs = acceptance::static_inputs();
    let static_names: std::collections::BTreeSet<_> =
        static_inputs.iter().map(|x| x.label.clone()).collect();
    for input in static_inputs
        .into_iter()
        .chain(acceptance::boundary_inputs())
        .chain(acceptance::comparison_inputs())
    {
        let skeleton = if static_names.contains(&input.label) {
            acceptance::add(
                &input
                    .skeleton
                    .replace("cut C {", "cut C { question \"日本語\";"),
                "simulation replications 4 seed 0 confidence 0.5",
            )
        } else {
            input.skeleton.clone()
        };
        let args: &[&str] = if input.label == "unselected-process-rate" {
            &["--configuration", "Selected"]
        } else if ["compare-simulation", "diff-new-route", "diff-union"]
            .contains(&input.label.as_str())
        {
            &["--configuration", "AsIs"]
        } else {
            &[]
        };
        let commands = if skeleton.contains("simulation replications") {
            vec!["check", "simulate"]
        } else {
            vec!["check"]
        };
        for command in commands {
            let (v, exit) = binary(&skeleton, command, args);
            let codes: Vec<_> = v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["severity"] == "error")
                .map(|d| d["code"].as_str().unwrap())
                .collect();
            let allowed = command == "simulate"
                && ["abort", "abort-late"].contains(&input.label.as_str())
                && codes == ["U-SEM-0"];
            rows.push(format!("inline:{}|{command}|{exit}|{codes:?}", input.label));
            assert!(codes.is_empty() || allowed, "{}: {v}", input.label);
        }
        if ["compare-simulation", "diff-new-route", "diff-union"].contains(&input.label.as_str()) {
            for command in if skeleton.contains("simulation replications") {
                vec!["check", "simulate"]
            } else {
                vec!["check"]
            } {
                let (v, exit) = binary(&skeleton, command, &["--configuration", "ToBeP"]);
                let codes: Vec<_> = v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|d| d["severity"] == "error")
                    .map(|d| d["code"].as_str().unwrap())
                    .collect();
                rows.push(format!(
                    "inline:{}/ToBeP|{command}|{exit}|{codes:?}",
                    input.label
                ));
                assert!(codes.is_empty(), "{}: {v}", input.label);
            }
        }
    }
    eprintln!("PREFLIGHT\n{}", rows.join("\n"));
}

#[test]
fn m_cli_metrics_opt_in_expected_schema_exit_and_static_diagnostics() {
    for name in fixture_match::METRICS_FIXTURES {
        let dir = acceptance::root().join(name);
        let inv = std::fs::read_to_string(dir.join("invocation.json"))
            .ok()
            .map(|s| serde_json::from_str(&s).unwrap());
        let inv = fixture_match::Invocation::parse(inv.as_ref()).unwrap();
        let expected: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).unwrap())
                .unwrap();
        inv.validate_expected(&expected).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(inv.args(&dir.join("model.relca")))
            .output()
            .unwrap();
        let actual: Value = serde_json::from_slice(&out.stdout).unwrap();
        inv.compare(&expected, &actual)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            out.status.code(),
            Some(if expected["verdict"] == "satisfied" {
                0
            } else {
                2
            })
        );
        if name == "met-legacy" {
            assert!(actual.get("metrics").is_none());
        }
    }
    for input in acceptance::static_inputs() {
        if let Some(code) = input.code {
            let (v, exit) = binary(&input.source, "check", &[]);
            assert_eq!(
                exit,
                if code.starts_with('E') { 3 } else { 4 },
                "{}: {v}",
                input.label
            );
            assert!(
                v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == code)
            );
            assert!(v.get("metrics").is_none());
            assert_eq!(v["properties"], json!([]));
        }
    }
}
