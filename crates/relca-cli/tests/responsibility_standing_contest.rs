//! Real-process ADR 0018 acceptance. Share authored input inventory with core
//! so old-binary preflight cannot accidentally omit CLI-only literals.
#[path = "../../relca-core/tests/responsibility_standing_contest.rs"]
mod acceptance;

#[path = "../src/fixture_match.rs"]
mod fixture_match;

use serde_json::{Value, json};
use std::{fs, process::Command};

#[test]
fn r_cli_rejection_matrix_and_exit_status() {
    let dest = acceptance::root()
        .parent()
        .unwrap()
        .join(".tmp/cli-static-inputs");
    fs::create_dir_all(&dest).unwrap();
    let mut failures = Vec::new();
    for (label, s, code) in acceptance::rejection_inputs() {
        let path = dest.join(format!("{label}.relca"));
        fs::write(&path, &s).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("check")
            .arg(&path)
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&output.stdout).unwrap();
        let exit = if code.starts_with('U') { 4 } else { 3 };
        if output.status.code() != Some(exit) || acceptance::code_count(&v, code) != 1 {
            failures.push(format!(
                "{label}: exit {:?}, expected {exit}/{code}, {v}",
                output.status.code()
            ));
        }
        // The existing CLI mirrors diagnostics to stderr; stdout alone is JSON.
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn r_cli_19_fixtures_including_soft_warning_exit_zero() {
    for name in acceptance::FIXTURES {
        let dir = acceptance::root().join(name);
        let expected: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_relca"));
        if name.starts_with("resp-diff") {
            cmd.arg("diff")
                .arg(dir.join("model.relca"))
                .args(["--from", "A", "--to", "B"]);
        } else {
            cmd.arg("check").arg(dir.join("model.relca"));
        }
        let out = cmd.output().unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        let verdict: relca_core::Verdict =
            serde_json::from_value(expected["verdict"].clone()).unwrap();
        assert_eq!(out.status.code(), Some(verdict.exit_code()), "{name}: {v}");
        acceptance::partial(&expected, &v);
    }
}

#[test]
fn r_cli_structural_simulation_compare_and_configuration_use_closed_wire() {
    let dest = acceptance::root()
        .parent()
        .unwrap()
        .join(".tmp/cli-mode-inputs");
    fs::create_dir_all(&dest).unwrap();
    let (_, sim) = acceptance::extra_inputs()
        .into_iter()
        .find(|(l, _)| l == "simulation-probability")
        .unwrap();
    let configured = acceptance::simulation_source(
        &acceptance::source("resp-diff")
            .replace("result nondet { pass,fail }", "result nondet { fail }"),
    );
    for (label, s, spec, expected) in [
        (
            "simulate",
            sim.replace("result nondet { fail }", "result nondet { pass }"),
            json!({"command":"simulate"}),
            json!({"mode":"simulation","simulation":{"multiplicity":{"rows":1}},"contest_paths":[{"id":"contract Controls #1 contest","verdict":"satisfied"}]}),
        ),
        (
            "simulate-selected",
            configured.clone(),
            json!({"command":"simulate","configuration":"A"}),
            json!({"mode":"simulation","contest_paths":[{"id":"contract Controls #1 contest","verdict":"satisfied"}]}),
        ),
        (
            "compare-exhaustive",
            acceptance::source("resp-diff"),
            json!({"command":"compare","plans":["A","B"],"mode":"exhaustive"}),
            json!({"mode":"compare","mismatch":null}),
        ),
        (
            "compare-simulation",
            configured,
            json!({"command":"compare","plans":["A","B"],"mode":"simulation"}),
            json!({"mode":"compare","mismatch":null}),
        ),
    ] {
        let path = dest.join(format!("{label}.relca"));
        fs::write(&path, s).unwrap();
        let invocation = fixture_match::Invocation::parse(Some(&spec)).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(invocation.args(&path))
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        invocation
            .compare(&expected, &v)
            .unwrap_or_else(|e| panic!("{label}: {e}; {v}"));
        let verdict: relca_core::Verdict = serde_json::from_value(v["verdict"].clone()).unwrap();
        assert_eq!(out.status.code(), Some(verdict.exit_code()));
    }
}
