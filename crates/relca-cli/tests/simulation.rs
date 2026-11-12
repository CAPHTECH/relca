//! Real-binary contracts independent of the not-yet-implemented core API.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .output()
        .unwrap()
}
fn output(args: &[&str], exit: i32) -> Value {
    let out = run(args);
    assert_eq!(
        out.status.code(),
        Some(exit),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: Value = serde_json::from_slice(&out.stdout).expect("exactly one JSON on stdout");
    fixture_match::validate_simulation_wire(&value).unwrap();
    value
}

#[test]
fn s_cli_simulation_real_binary_exit0_through4_never5() {
    assert_eq!(fixture_match::SIM_FIXTURES.len(), 24);
    assert_eq!(fixture_match::CONTRACT_FIXTURES.len(), 19);
    assert_eq!(fixture_match::EXECUTOR_FIXTURES.len(), 14);
    for (name, exit, verdict) in [
        ("sim-keys", 0, "satisfied"),
        ("sim-cut", 1, "violated"),
        ("sim-reachable.broken-unseen-end", 2, "inconclusive"),
        ("sim-required.broken-missing-simulation", 3, "invalid"),
        ("sim-range.broken-zero-replications", 3, "invalid"),
        ("sim-fifo.broken-no-policy", 4, "unsupported"),
    ] {
        let path = root().join(name).join("model.relca");
        let v = output(&["simulate", path.to_str().unwrap()], exit);
        assert_eq!(v["mode"], "simulation");
        assert_eq!(v["verdict"], verdict);
    }
}

#[test]
fn s_cli_usage_and_read_failure_do_not_use_unsupported_stub() {
    let path = root().join("sim-keys/model.relca");
    let p = path.to_str().unwrap();
    for args in [
        vec!["simulate"],
        vec!["simulate", p, "extra"],
        vec!["simulate", p, "--seed", "0"],
        vec!["simulate", p, "--confidence", "0.95"],
        vec!["simulate", p, "--replications", "4"],
        vec!["simulate", "--configuration", "C", p],
        vec!["simulate", p, "--configuration"],
        vec!["simulate", p, "--configuration", ""],
        vec!["simulate", p, "--configuration", "--oops"],
        vec![
            "simulate",
            p,
            "--configuration",
            "C",
            "--configuration",
            "C",
        ],
        vec!["simulate", p, "--unknown", "C"],
    ] {
        let out = run(&args);
        assert_eq!(out.status.code(), Some(3), "{args:?}");
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        fixture_match::validate_simulation_wire(&v).unwrap();
        assert_eq!(v["mode"], "simulation");
        assert_eq!(v["verdict"], "invalid");
        assert!(
            String::from_utf8_lossy(&out.stderr)
                .contains("usage: relca simulate <path> [--configuration NAME]")
        );
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-SYN-0")
        );
    }
    let missing = root().join("does-not-exist-simulation.relca");
    let v = output(&["simulate", missing.to_str().unwrap()], 3);
    assert_eq!(v["verdict"], "invalid");
    assert!(v["model"]["sha256"].is_null());
}

#[test]
fn s_cli_max_seed_is_not_rounded_or_a_string() {
    let p = root().join("sim-range/model.relca");
    let v = output(&["simulate", p.to_str().unwrap()], 0);
    assert_eq!(v["simulation"]["seed"].as_u64(), Some(u64::MAX));
}

struct Input(PathBuf);
impl Input {
    fn new(source: &str) -> Self {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let parent = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
        assert!(parent.is_dir());
        let p = parent.join(format!(
            "simulation-{}-{}.relca",
            std::process::id(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::write(&p, source).unwrap();
        Self(p)
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}
fn added(source: &str) -> String {
    let i = source.rfind('}').unwrap();
    format!(
        "{}\n simulation replications 4 seed 0 confidence 0.5\n{}",
        &source[..i],
        &source[i..]
    )
}

#[test]
fn s_cli_configuration_selection_and_newroute_case_results() {
    let s = fs::read_to_string(root().join("config-diff-add-actor/model.relca")).unwrap();
    let f = Input::new(&added(&s));
    let p = f.0.to_str().unwrap();
    let v = output(&["simulate", p, "--configuration", "AsIs"], 2);
    assert_eq!(v["verdict"], "inconclusive");
    // ADR 0008: an unknown configuration name is E-NAME-0, as in check.
    let v = output(&["simulate", p, "--configuration", "Unknown"], 3);
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-NAME-0")
    );
    let v = output(&["simulate", p], 3);
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CONF-0")
    );
    let f = Input::new(&added(&s).replace("  bounds", "  arrivals fixed [0min]\n  bounds"));
    let v = output(
        &["simulate", f.0.to_str().unwrap(), "--configuration", "AsIs"],
        2,
    );
    assert_eq!(v["case_outcomes"][0]["terminals"][0]["count"], 4);
    let v = output(
        &[
            "simulate",
            f.0.to_str().unwrap(),
            "--configuration",
            "Unknown",
        ],
        3,
    );
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-NAME-0")
    );
}

#[test]
fn s_cli_check_diff_without_simulation_keep_closed_keys_and_assumptions() {
    // Full JSON equality, not round-trip through a DTO that might discard new keys.
    for (name, extra) in [
        ("serial-time", vec![]),
        ("config-diff-add-actor", vec!["--configuration", "AsIs"]),
    ] {
        let s = fs::read_to_string(root().join(name).join("model.relca")).unwrap();
        let original = Input::new(&s);
        let appended = Input::new(&added(&s));
        let mut args = vec!["check", original.0.to_str().unwrap()];
        args.extend(extra.clone());
        let a = run(&args);
        let mut args = vec!["check", appended.0.to_str().unwrap()];
        args.extend(extra);
        let b = run(&args);
        assert_eq!(a.status.code(), b.status.code());
        assert_eq!(a.stderr, b.stderr);
        let (mut a, mut b): (Value, Value) = (
            serde_json::from_slice(&a.stdout).unwrap(),
            serde_json::from_slice(&b.stdout).unwrap(),
        );
        a["model"]["sha256"] = json!(null);
        b["model"]["sha256"] = json!(null);
        assert_eq!(a, b);
        assert_eq!(a["mode"], "exhaustive");
        assert!(a.get("simulation").is_none());
    }
    let s = fs::read_to_string(root().join("config-diff-add-actor/model.relca")).unwrap();
    let a = Input::new(&s);
    let b = Input::new(&added(&s));
    let a = run(&[
        "diff",
        a.0.to_str().unwrap(),
        "--from",
        "AsIs",
        "--to",
        "ToBe",
    ]);
    let b = run(&[
        "diff",
        b.0.to_str().unwrap(),
        "--from",
        "AsIs",
        "--to",
        "ToBe",
    ]);
    assert_eq!(a.status.code(), b.status.code());
    assert_eq!(a.stderr, b.stderr);
    let (mut a, mut b): (Value, Value) = (
        serde_json::from_slice(&a.stdout).unwrap(),
        serde_json::from_slice(&b.stdout).unwrap(),
    );
    for side in ["from", "to"] {
        a[side]["report"]["model"]["sha256"] = json!(null);
        b[side]["report"]["model"]["sha256"] = json!(null);
    }
    assert_eq!(a, b);
}

#[test]
fn s_cli_other_stub_commands_remain_unchanged() {
    // mutate and explain have dedicated acceptance targets now.
    {
        let cmd = "verify";
        let out = run(&[cmd, "model.relca"]);
        assert_eq!(out.status.code(), Some(4));
        assert_eq!(
            String::from_utf8(out.stdout).unwrap().trim(),
            ledger_shape::extend_raw(
                "{\"verdict\":\"unsupported\"}",
                &ledger_shape::Spec::minimal("verify")
            )
        );
    }
}

#[path = "support/ledger_shape.rs"]
mod ledger_shape;
