//! Real binary acceptance: argument grammar, diagnostics, order and closed wire.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
fn path(name: &str) -> String {
    root()
        .join(name)
        .join("model.relca")
        .to_str()
        .unwrap()
        .to_owned()
}
fn run(args: &[&str], exit: i32) -> (Value, String) {
    let p = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&p.stdout).expect("stdout exactly one JSON");
    assert_eq!(p.status.code(), Some(exit), "{args:?}: {v}");
    (v, String::from_utf8(p.stderr).unwrap())
}
fn diagnostic(v: &Value, code: &str) {
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == code),
        "{v}"
    );
}
fn empty(v: &Value) {
    assert_eq!(v["tables"], json!({"properties":[],"terminals":[]}));
}
struct Input(PathBuf);
impl Input {
    fn new(s: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
        let p = target.join(format!(
            "compare input {} {}.relca",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::write(&p, s).unwrap();
        Self(p)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

#[test]
fn c_cli_both_flag_orders_preserve_requested_order_with_three_plans_and_space_path() {
    let src = fs::read_to_string(path("compare-terminal"))
        .unwrap()
        .replace(
            "  bounds",
            "  configuration Cplan { cut C; process P }\n  bounds",
        );
    let input = Input::new(&src);
    for args in [
        vec![
            "compare",
            input.path(),
            "--plans",
            "Bplan,Aplan,Cplan",
            "--mode",
            "exhaustive",
        ],
        vec![
            "compare",
            input.path(),
            "--mode",
            "exhaustive",
            "--plans",
            "Bplan,Aplan,Cplan",
        ],
    ] {
        let (v, _) = run(&args, 0);
        fixture_match::validate_compare_wire(&v).unwrap();
        assert_eq!(
            v["plans"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p["configuration"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["Bplan", "Aplan", "Cplan"]
        );
        assert!(v.get("common_random").is_none());
    }
}

#[test]
fn c_cli_argument_matrix_invalid_is_not_unsupported_or_default_analysis() {
    let p = path("compare-terminal");
    let malformed = vec![
        vec![],
        vec![""],
        vec!["--plans", "Aplan,Bplan", "--mode", "exhaustive"],
        vec![&p],
        vec![&p, "--plans"],
        vec![&p, "--mode"],
        vec![&p, "--plans", "Aplan,Bplan"],
        vec![&p, "--mode", "exhaustive"],
        vec![&p, &p, "--plans", "Aplan,Bplan", "--mode", "exhaustive"],
        vec![&p, "--plans=Aplan,Bplan", "--mode", "exhaustive"],
        vec![&p, "--plans", "Aplan,Bplan", "--mode=exhaustive"],
        vec![
            &p,
            "--plans",
            "Aplan,Bplan",
            "--plans",
            "Aplan,Bplan",
            "--mode",
            "exhaustive",
        ],
        vec![
            &p,
            "--plans",
            "Aplan,Bplan",
            "--mode",
            "exhaustive",
            "--mode",
            "simulation",
        ],
    ];
    for args in malformed {
        let mut argv = vec!["compare"];
        argv.extend(args);
        let (v, stderr) = run(&argv, 3);
        diagnostic(&v, "E-SYN-0");
        empty(&v);
        assert_eq!(v["mode"], "compare");
        assert!(v["analysis"].is_null());
        assert_eq!(v["plans"], json!([]));
        assert!(stderr.contains(
            "usage: relca compare <path> --plans A,B[,C...] --mode exhaustive|simulation"
        ));
        fixture_match::validate_compare_wire(&v).unwrap();
    }
    for plans in [
        "",
        ",",
        ",Aplan",
        "Aplan,",
        "Aplan,,Bplan",
        "Aplan",
        "Aplan,Aplan",
        "Aplan, Bplan",
        "Aplan,1Bplan",
        "Aplan,B-plan",
    ] {
        let (v, stderr) = run(
            &["compare", &p, "--plans", plans, "--mode", "exhaustive"],
            3,
        );
        diagnostic(&v, "E-SYN-0");
        empty(&v);
        assert!(v["analysis"].is_null());
        assert!(stderr.contains("E-SYN-0"));
    }
    for mode in ["", "unknown", "Exhaustive", "SIMULATION"] {
        let (v, _) = run(
            &["compare", &p, "--plans", "Aplan,Bplan", "--mode", mode],
            3,
        );
        diagnostic(&v, "E-SYN-0");
        empty(&v);
        assert!(v["analysis"].is_null());
    }
    for flag in [
        "--unknown",
        "--seed",
        "--replications",
        "--confidence",
        "--scenarios",
        "--rank",
        "--pareto",
        "--from",
        "--configuration",
    ] {
        let (v, _) = run(
            &[
                "compare",
                &p,
                "--plans",
                "Aplan,Bplan",
                "--mode",
                "exhaustive",
                flag,
                "0",
            ],
            3,
        );
        diagnostic(&v, "E-SYN-0");
        empty(&v);
    }
}

#[test]
fn c_cli_name_resolution_read_failure_missing_simulation_and_source_syntax() {
    let p = path("compare-terminal");
    for unknown in ["Unknown", "aplan"] {
        let (v, stderr) = run(
            &[
                "compare",
                &p,
                "--plans",
                &format!("Aplan,{unknown}"),
                "--mode",
                "exhaustive",
            ],
            3,
        );
        diagnostic(&v["plans"][1]["report"], "E-NAME-0");
        empty(&v);
        assert_eq!(v["plans"][0]["report"]["verdict"], "satisfied");
        assert!(stderr.contains(&format!("configuration {unknown}: E-NAME-0:")));
    }
    let no = path("arrivals-count");
    let (v, _) = run(
        &[
            "compare",
            &no,
            "--plans",
            "Aplan,Bplan",
            "--mode",
            "exhaustive",
        ],
        3,
    );
    for p in v["plans"].as_array().unwrap() {
        diagnostic(&p["report"], "E-CONF-0");
    }
    let (v, _) = run(
        &[
            "compare",
            &p,
            "--plans",
            "Aplan,Bplan",
            "--mode",
            "simulation",
        ],
        3,
    );
    for p in v["plans"].as_array().unwrap() {
        diagnostic(&p["report"], "E-SIM-0");
    }
    assert!(v["mismatch"].is_null());
    empty(&v);
    let missing = root().join("nonexistent-compare-input.relca");
    assert!(!missing.exists());
    let (v, _) = run(
        &[
            "compare",
            missing.to_str().unwrap(),
            "--plans",
            "Bplan,Aplan",
            "--mode",
            "simulation",
        ],
        3,
    );
    assert_eq!(v["analysis"], "simulation");
    empty(&v);
    for p in v["plans"].as_array().unwrap() {
        diagnostic(&p["report"], "E-SYN-0");
        assert!(p["report"]["model"]["sha256"].is_null());
    }
    let source = fs::read_to_string(&p).unwrap();
    let changed = source.replace("  bounds", "  calendar BusyDay {}\n  bounds");
    assert_ne!(source, changed);
    let input = Input::new(&changed);
    let (v, _) = run(
        &[
            "compare",
            input.path(),
            "--plans",
            "Aplan,Bplan",
            "--mode",
            "exhaustive",
        ],
        3,
    );
    for p in v["plans"].as_array().unwrap() {
        diagnostic(&p["report"], "E-SYN-0");
    }
    empty(&v);
}

#[test]
fn c_cli_simulation_maximum_seed_is_an_integer_not_a_string_or_float() {
    let src = fs::read_to_string(path("compare-random"))
        .unwrap()
        .replace("seed 0", "seed 18446744073709551615")
        .replace(">= 0.5", ">= 0");
    let input = Input::new(&src);
    let (v, _) = run(
        &[
            "compare",
            input.path(),
            "--plans",
            "Bplan,Aplan",
            "--mode",
            "simulation",
        ],
        0,
    );
    fixture_match::validate_compare_wire(&v).unwrap();
    for p in v["plans"].as_array().unwrap() {
        assert_eq!(p["report"]["simulation"]["seed"].as_u64(), Some(u64::MAX));
    }
}

#[test]
fn c_cli_all_16_new_invocations_match_independent_expectations() {
    assert_eq!(fixture_match::COMPARE_FIXTURES.len(), 16);
    for name in fixture_match::COMPARE_FIXTURES {
        let dir = root().join(name);
        let spec: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("invocation.json")).unwrap())
                .unwrap();
        let expected: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
        let invocation = fixture_match::Invocation::parse(Some(&spec)).unwrap();
        invocation.validate_expected(&expected).unwrap();
        let args = invocation.args(&dir.join("model.relca"));
        let exit = serde_json::from_value::<relca_core::Verdict>(expected["verdict"].clone())
            .unwrap()
            .exit_code();
        let (actual, _) = run(&args.iter().map(String::as_str).collect::<Vec<_>>(), exit);
        invocation.compare(&expected, &actual).unwrap();
    }
}
