#[path = "../../relca-core/tests/support/read_write_oracle.rs"]
mod h;
#[path = "support/ledger_shape.rs"]
mod ledger_shape;
use serde_json::Value;
use std::{path::PathBuf, process::Command};

fn root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::create_dir_all(root.join(".tmp")).unwrap();
    root
}

fn accept(name: &str) {
    let (_, src, expected) = h::cases().into_iter().find(|(n, _, _)| *n == name).unwrap();
    let path = root().join(format!(".tmp/k5-cli-{}-{name}.relca", std::process::id()));
    std::fs::write(&path, &src).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["check", path.to_str().unwrap()])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let expected: Vec<_> = expected.iter().map(String::as_str).collect();
    h::assert_rows(&src, &v, &expected);
    assert_eq!(
        o.status.code(),
        Some(relca_core::report::check(&src).verdict.exit_code())
    );
}

macro_rules! cases { ($($test:ident => $name:literal),+ $(,)?) => { $(
    #[test] fn $test() { accept($name); }
)+ }; }
cases! {
    requires => "requires", broken_unwritten => "broken-unwritten",
    repeated_read => "repeated-read", atomic => "atomic",
    atomic_broken_unwritten => "atomic-broken-unwritten",
    both_capability_conditions => "both-capability-conditions",
    process_if => "process-if", process_if_broken_unwritten => "process-if-broken-unwritten",
    utf8_byte_position => "utf8-byte-position", unused_reader => "unused-reader",
    self_pair => "self-pair", start_writer => "start-writer",
    variable_order => "variable-order", single_case => "single-case",
    case_local => "case-local", per_case => "per-case", evaluation_only => "evaluation-only",
    integer_comparison => "integer-comparison", serialized_actor => "serialized-actor",
    serialized_resource => "serialized-resource",
    two_if_sites => "two-if-sites", writer_declaration_order => "writer-declaration-order",
}

#[test]
fn real_cli_explain_simulate_diff_compare_carry_the_same_candidates() {
    let src = h::surface_source();
    let config = h::configuration_source();
    let path = root().join(format!(".tmp/k5-surfaces-{}.relca", std::process::id()));
    let wanted = h::rows(&h::model(&src));
    for args in [
        vec!["explain"],
        vec!["simulate"],
        vec!["diff", "--from", "Left", "--to", "Right"],
        vec!["compare", "--plans", "Right,Left", "--mode", "exhaustive"],
    ] {
        std::fs::write(&path, if args.len() == 1 { &src } else { &config }).unwrap();
        let o = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg(args[0])
            .arg(&path)
            .args(&args[1..])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        let pointers: &[&str] = match args[0] {
            "diff" => &["/from/report", "/to/report"],
            "compare" => &["/plans/0/report", "/plans/1/report"],
            _ => &[""],
        };
        for p in pointers {
            h::assert_candidate_tail(v.pointer(p).unwrap(), &wanted);
        }
    }
}

#[test]
fn verify_reports_phase_and_backend_reason_without_fallback() {
    let exists = "fixtures/serial-time/model.relca";
    let missing = ".tmp/k5-missing.relca";
    for args in [
        vec!["verify"],
        vec!["verify", exists],
        vec!["verify", missing],
    ] {
        let o = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .current_dir(root())
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(4), "{args:?}");
        assert_eq!(
            o.stdout,
            ledger_shape::extend_raw(
                "{\"verdict\":\"unsupported\"}\n",
                &ledger_shape::Spec::minimal("verify")
            )
            .as_bytes()
        );
        assert_eq!(o.stderr, h::VERIFY_REASON.as_bytes(), "{args:?}");
    }
}

#[test]
fn unwritten_control_and_check_io_failure_keep_old_classification() {
    let src = h::BASE.replacen("requires x", "requires y", 1);
    let path = root().join(format!(".tmp/k5-cli-negative-{}.relca", std::process::id()));
    std::fs::write(&path, &src).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["check", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    h::assert_rows(&src, &v, &[]);
    let missing = ".tmp/k5-missing.relca";
    let error = std::fs::read_to_string(root().join(missing)).unwrap_err();
    let expected_message = format!("Cannot read {missing}: {error}");
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["check", missing])
        .current_dir(root())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["verdict"], "invalid");
    assert_eq!(v["diagnostics"][0]["code"], "E-SYN-0");
    assert_eq!(v["diagnostics"][0]["message"], expected_message);
    assert_eq!(
        String::from_utf8(o.stderr).unwrap().lines().next(),
        Some(format!("E-SYN-0: {expected_message}").as_str())
    );
}
