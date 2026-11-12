//! CLI の終了コードと JSON 結果の契約(design v0.2 §22.3)。
//! 未実装のサブコマンドは `unsupported` で exit 4、誤った呼び出しは `invalid` で exit 3。

use std::process::Command;
#[path = "support/ledger_shape.rs"]
mod ledger_shape;

fn run(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .output()
        .expect("relca を起動できない");
    let code = out.status.code().expect("シグナルで終了した");
    (
        code,
        String::from_utf8(out.stdout).expect("stdout が UTF-8 でない"),
    )
}

#[test]
fn unimplemented_commands_are_unsupported() {
    {
        let cmd = "verify";
        let (code, stdout) = run(&[cmd, "model.relca"]);
        assert_eq!(code, 4, "relca {cmd}");
        assert_eq!(
            stdout.trim(),
            ledger_shape::extend_raw(
                r#"{"verdict":"unsupported"}"#,
                &ledger_shape::Spec::minimal("verify")
            ),
            "relca {cmd}"
        );
    }
}

#[test]
fn arrivals_runtime_and_static_rejections_have_real_binary_exits() {
    for (name, exit, verdict, code) in [
        ("arrivals-simultaneous", 0, "satisfied", None),
        (
            "arrivals-initial-product.broken-fixed-initial",
            1,
            "violated",
            None,
        ),
        (
            "arrivals-relative-within.broken-tight-deadline",
            1,
            "violated",
            None,
        ),
        (
            "arrivals-count.broken-case-count",
            3,
            "invalid",
            Some("E-ARR-0"),
        ),
        (
            "arrivals-order.broken-descending",
            3,
            "invalid",
            Some("E-ARR-0"),
        ),
        (
            "arrivals-required.broken-missing-arrivals",
            3,
            "invalid",
            Some("E-ARR-0"),
        ),
        (
            "arrivals-shared-fixed.broken-unset-shared",
            4,
            "unsupported",
            Some("U-SEM-0"),
        ),
    ] {
        let v = json_run(&["check", &fixture_model(name)], exit);
        assert_eq!(v["verdict"], verdict, "{name}");
        if let Some(code) = code {
            assert_code(&v, code);
            assert!(v.get("case_outcomes").is_none());
            assert!(v.get("admission").is_none());
            assert_eq!(ledger_shape::original_field_count(&v), 13);
            assert_eq!(v["properties"], serde_json::json!([]));
            assert!(v["agency"].is_null());
            for key in ["cases", "live_tasks", "clock", "truncated"] {
                assert!(v["bounds"][key].is_null());
            }
        } else {
            assert_eq!(ledger_shape::original_field_count(&v), 15);
            assert_eq!(v["case_outcomes"].as_array().unwrap().len(), 2);
            assert!(v.get("admission").is_some());
        }
        assert_eq!(v["terminals"], serde_json::json!([]));
    }
}

#[test]
fn mutate_requires_a_readable_path() {
    for args in [
        &["mutate"][..],
        &["mutate", "no-such-mutation-input.relca"][..],
    ] {
        let (code, stdout) = run(args);
        assert_eq!(code, 3, "{args:?}: {stdout}");
        let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(value["verdict"], "invalid");
        assert_eq!(value["mode"], "mutation");
    }
    let (code, stdout) = run(&["mutation"]);
    assert_eq!(code, 3);
    assert_eq!(
        stdout.trim(),
        ledger_shape::extend_raw(
            r#"{"verdict":"invalid"}"#,
            &ledger_shape::Spec::minimal("cli_refusal")
        )
    );
}

#[test]
fn finite_contracts_use_all_six_real_exit_classes() {
    for (name, exit, verdict) in [
        ("contract-response", 0, "satisfied"),
        ("contract-response.broken-late-finish", 1, "violated"),
        ("contract-response.broken-short-clock", 2, "inconclusive"),
        ("serial-time.broken-missing-capacity", 3, "invalid"),
        ("contract-vacuous", 5, "vacuous"),
    ] {
        let value = json_run(&["check", &fixture_model(name)], exit);
        assert_eq!(value["verdict"], verdict);
        assert_eq!(
            ledger_shape::original_field_count(&value),
            if exit == 2 { 14 } else { 13 }
        );
        assert_eq!(
            value.get("unchecked"),
            (exit == 2)
                .then(|| serde_json::json!(["unreachable"]))
                .as_ref()
        );
    }
    assert_eq!(json_run(&["mutate"], 3)["verdict"], "invalid");
    for (name, exit, verdict) in [
        ("contract-diff", 5, "vacuous"),
        ("contract-diff.broken-zero-deadline", 1, "violated"),
    ] {
        let value = json_run(
            &[
                "diff",
                &fixture_model(name),
                "--from",
                "Before",
                "--to",
                "After",
            ],
            exit,
        );
        assert_eq!(value["verdict"], verdict);
    }
}

#[test]
fn executor_contracts_use_real_success_violation_vacuity_and_unsupported_exits() {
    for (name, exit, verdict) in [
        ("exec-separation", 0, "satisfied"),
        ("exec-separation.broken-shared-approver", 1, "violated"),
        ("exec-permitted", 0, "satisfied"),
        ("exec-permitted.broken-narrow-permit", 1, "violated"),
        ("exec-authority", 0, "satisfied"),
        ("exec-authority.broken-missing-grant", 1, "violated"),
        ("exec-cancel", 5, "vacuous"),
        ("exec-anonymous.broken-no-executor", 4, "unsupported"),
    ] {
        let value = json_run(&["check", &fixture_model(name)], exit);
        assert_eq!(value["verdict"], verdict, "{name}");
        assert_eq!(ledger_shape::original_field_count(&value), 13);
        if exit == 4 {
            assert_code(&value, "U-SEM-0");
            assert_eq!(value["properties"], serde_json::json!([]));
            assert_eq!(value["bounds"]["truncated"], serde_json::Value::Null);
        }
    }
    for (name, exit, verdict) in [
        ("exec-vacuous", 5, "vacuous"),
        ("exec-vacuous.broken-shared-approver", 1, "violated"),
    ] {
        let path = fixture_model(name);
        let value = json_run(&["diff", &path, "--from", "AsIs", "--to", "ToBeA"], exit);
        assert_eq!(value["verdict"], verdict);
        assert_eq!(value["properties"][1]["to"], "vacuous");
    }
}

#[test]
fn state_policy_is_invalid_error_exit3_not_unsupported_or_warning() {
    let value = json_run(
        &[
            "check",
            &fixture_model("state-policy.broken-latent-difficulty"),
        ],
        3,
    );
    assert_eq!(value["verdict"], "invalid");
    assert_eq!(ledger_shape::original_field_count(&value), 13);
    let diagnostics = value["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "E-POL-LATENT" && d["severity"] == "error")
    );
    assert_code(&value, "W-CUT-0");
    for key in ["cases", "live_tasks", "clock", "truncated"] {
        assert_eq!(value["bounds"][key], serde_json::Value::Null);
    }
    assert_eq!(value["agency"], serde_json::Value::Null);
    assert_eq!(value["properties"], serde_json::json!([]));
    assert_eq!(value["terminals"], serde_json::json!([]));
}

#[test]
fn conditional_probability_real_exits_and_e_prob_error() {
    for (name, exit, verdict) in [
        ("prob-sum", 0, "satisfied"),
        ("prob-mdp.broken-tight-upper", 1, "violated"),
        ("prob-cut.broken-short-clock", 2, "inconclusive"),
        ("prob-sum.broken-mass", 3, "invalid"),
        ("prob-rows.broken-missing-difficult", 3, "invalid"),
    ] {
        let v = json_run(&["check", &fixture_model(name)], exit);
        assert_eq!(v["verdict"], verdict);
        assert_eq!(
            ledger_shape::original_field_count(&v),
            if exit == 2 { 14 } else { 13 }
        );
        assert_eq!(
            v.get("unchecked"),
            (exit == 2)
                .then(|| serde_json::json!(["unreachable"]))
                .as_ref()
        );
        if exit == 3 {
            assert_code(&v, "E-PROB-0");
            assert!(
                v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "E-PROB-0" && d["severity"] == "error")
            );
            assert_eq!(v["properties"], serde_json::json!([]));
            assert_eq!(v["terminals"], serde_json::json!([]));
            assert!(v["agency"].is_null());
            for k in ["cases", "live_tasks", "clock", "truncated"] {
                assert!(v["bounds"][k].is_null());
            }
        } else {
            assert!(
                v["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p.get("probability").is_some())
            );
        }
    }
    let v = json_run(
        &[
            "diff",
            &fixture_model("prob-invoice.broken-ai-transfer"),
            "--from",
            "AsIs",
            "--to",
            "ToBeP",
        ],
        1,
    );
    assert_eq!(v["verdict"], "violated");
}

#[test]
fn duration_distributions_use_real_success_violation_static_error_and_clock_cut_exits() {
    for (name, exit, verdict) in [
        ("duration-serial", 0, "satisfied"),
        ("duration-no-lookahead.broken-late-choice", 1, "violated"),
        ("duration-clock", 2, "inconclusive"),
        ("duration-sum.broken-mass", 3, "invalid"),
        ("duration-duplicate.broken-duplicate-value", 3, "invalid"),
    ] {
        let v = json_run(&["check", &fixture_model(name)], exit);
        assert_eq!(v["verdict"], verdict);
        assert_eq!(
            ledger_shape::original_field_count(&v),
            if exit == 2 { 14 } else { 13 }
        );
        assert_eq!(
            v.get("unchecked"),
            (exit == 2)
                .then(|| serde_json::json!(["unreachable"]))
                .as_ref()
        );
        if exit == 3 {
            assert_code(&v, "E-PROB-0");
            assert_eq!(v["terminals"], serde_json::json!([]));
            assert_eq!(v["properties"], serde_json::json!([]));
            assert!(v["agency"].is_null());
            for key in ["cases", "live_tasks", "clock", "truncated"] {
                assert!(v["bounds"][key].is_null());
            }
        }
        if name == "duration-clock" {
            assert_eq!(v["bounds"]["truncated"], true);
            assert_eq!(v["properties"][0]["verdict"], "satisfied");
            assert_eq!(
                v["properties"][0]["probability"],
                serde_json::json!({"min":"1/2","max":"1/2"})
            );
        }
    }
}

#[test]
fn executor_each_clause_has_vacuous_and_anonymous_unsupported_real_exits() {
    let source = include_str!("../../../fixtures/exec-anonymous/model.relca");
    let prefix = source.split_once("  contract Controls {").unwrap().0;
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "p{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    for (kind, clause) in [
        ("separation", "separation A.a.finish, B.b.finish"),
        ("permitted", "permitted A.a"),
        ("authority_basis", "authority_basis A.a G"),
    ] {
        let base = format!("{prefix} contract Controls {{ {clause} }} }}").replace(
            "process Flow { x = A.a(); y = B.b(); end done }",
            "process Flow { end done }",
        );
        for (label, source, exit, verdict) in [
            ("vacuous", base.clone(), 5, "vacuous"),
            (
                "anonymous",
                base.replace("executors [p];", ""),
                4,
                "unsupported",
            ),
        ] {
            let path = root.join(format!("exec-{kind}-{label}.relca"));
            std::fs::write(&path, source).unwrap();
            let value = json_run(&["check", path.to_str().unwrap()], exit);
            assert_eq!(value["verdict"], verdict);
            if exit == 4 {
                assert_code(&value, "U-SEM-0");
                assert_eq!(value["properties"], serde_json::json!([]));
            } else {
                assert_eq!(value["properties"][1]["verdict"], "vacuous");
            }
        }
    }
}

#[test]
fn check_unreadable_file_is_invalid() {
    let missing =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/no-such-input-file.relca");
    assert!(!missing.exists());
    let (code, stdout) = run(&["check", missing.to_str().unwrap()]);
    assert_eq!(code, 3);
    let report: relca_core::report::Report = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report.verdict, relca_core::Verdict::Invalid);
    assert!(report.model.sha256.is_none());
    assert_eq!(report.bounds.truncated, None);
    assert!(report.properties.is_empty());
    assert!(report.terminals.is_empty());
    assert!(report.diagnostics.iter().any(|d| d.code == "W-CUT-0"));
}

#[test]
fn check_requires_exactly_one_path() {
    for args in [&["check"][..], &["check", "a", "b"][..]] {
        let (code, stdout) = run(args);
        assert_eq!(code, 3);
        let report: relca_core::report::Report = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report.verdict, relca_core::Verdict::Invalid);
        assert_eq!(report.bounds.truncated, None);
    }
}

#[test]
fn check_reads_source_and_emits_one_report_with_matching_exit() {
    // README is deliberately not DSL; the report still hashes the bytes read.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
    let src = std::fs::read_to_string(&path).unwrap();
    let expected = relca_core::report::check(&src);
    let (code, stdout) = run(&["check", path.to_str().unwrap()]);
    let actual: relca_core::report::Report = serde_json::from_str(&stdout).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(code, actual.verdict.exit_code());
    assert!(actual.model.sha256.is_some());
    assert_ne!(code, 0);
}

#[test]
fn bad_invocation_is_invalid() {
    for args in [&[][..], &["no-such-command"][..]] {
        let (code, stdout) = run(args);
        assert_eq!(code, 3, "relca {args:?}");
        assert_eq!(
            stdout.trim(),
            ledger_shape::extend_raw(
                r#"{"verdict":"invalid"}"#,
                &ledger_shape::Spec::minimal("cli_refusal")
            ),
            "relca {args:?}"
        );
    }
}

// F3 (2026-10-21): a reachable deadlock is a counterexample. Even when only
// safety is verified and it holds, the binary must not report satisfied.
#[test]
fn reachable_deadlock_is_violated_even_when_every_property_holds() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "p{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, src) in [
        (
            "dl_safety",
            r#"model M version "0.2" {
  type Verdict = enum { pass }
  actor Office {
    capacity 0
    capability short() -> Verdict {
      duration fixed 3min
      result nondet { pass }
    }
  }
  process Check {
    a = Office.short()
    end done
  }
  bounds { cases 1; live_tasks 2; clock 8min }
  verify safety no_resource_overcommit
}"#,
        ),
        (
            "noend_safety",
            r#"model M version "0.2" {
  type Verdict = enum { pass }
  actor Office {
    capacity 1
    capability short() -> Verdict {
      duration fixed 3min
      result nondet { pass }
    }
  }
  process Check {
    a = Office.short()
  }
  bounds { cases 1; live_tasks 2; clock 8min }
  verify safety no_resource_overcommit
}"#,
        ),
        (
            "partial_dl",
            r#"model M version "0.2" {
  type Verdict = enum { pass, fail }
  actor Office {
    capacity 1
    capability short() -> Verdict {
      duration fixed 3min
      result nondet { pass, fail }
    }
  }
  process Check {
    a = Office.short()
    if a == pass { end done } else { }
  }
  bounds { cases 1; live_tasks 2; clock 8min }
  verify reachable end.done
}"#,
        ),
    ] {
        let path = dir.join(format!("{name}.relca"));
        std::fs::write(&path, src).unwrap();
        let (code, stdout) = run(&["check", path.to_str().unwrap()]);
        let report: relca_core::report::Report = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report.verdict, relca_core::Verdict::Violated, "{name}");
        assert_eq!(code, 1, "{name}");
        assert!(
            report.diagnostics.iter().any(|d| d.code == "W-DEADLOCK-0"),
            "{name}"
        );
    }
}

// Review R2 (2026-10-21): a later engine error must not erase a deadlock that
// was already found. The overflow stays visible and truncated stays null.
#[test]
fn deadlock_found_before_an_engine_error_is_still_violated() {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!(
            "p{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
        .join("deadlock-then-overflow.relca");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"model DeadlockThenOverflow version "0.2" {
  type T = enum { pass, fail }
  actor A {
    capacity 1
    capability pick() -> T {
      duration fixed 1min
      result nondet { pass, fail }
    }
    capability long() -> T {
      duration fixed 18446744073709551615min
      result nondet { pass }
    }
  }
  process P {
    a = A.pick()
    if a == pass {
      b = A.long()
      end done
    } else { }
  }
  bounds { cases 1; live_tasks 1; clock 18446744073709551615min }
  verify safety no_resource_overcommit
}"#,
    )
    .unwrap();
    let (code, stdout) = run(&["check", path.to_str().unwrap()]);
    let report: relca_core::report::Report = serde_json::from_str(&stdout).unwrap();
    let codes: Vec<_> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"W-DEADLOCK-0"), "{codes:?}");
    assert!(codes.contains(&"U-SEM-0"), "{codes:?}");
    assert_eq!(report.verdict, relca_core::Verdict::Violated);
    assert_eq!(code, 1);
    assert_eq!(report.bounds.truncated, None);
    assert!(report.terminals.is_empty());
}

fn fixture_model(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
        .join("model.relca")
        .to_str()
        .unwrap()
        .to_owned()
}

fn json_run(args: &[&str], expected_exit: i32) -> serde_json::Value {
    let (exit, stdout) = run(args);
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("one JSON value required for {args:?}: {error}: {stdout}"));
    assert_eq!(exit, expected_exit, "{args:?}: {value}");
    value
}

fn assert_code(value: &serde_json::Value, code: &str) {
    assert!(
        value["diagnostics"]
            .as_array()
            .is_some_and(|diagnostics| diagnostics.iter().any(|d| d["code"] == code)),
        "{code}: {value}"
    );
}

#[test]
fn configuration_check_selects_serial_or_parallel_and_keeps_check_wire() {
    let path = fixture_model("config-diff-parallel");
    for (configuration, elapsed) in [("AsIs", 8), ("ToBeP", 5)] {
        let value = json_run(&["check", &path, "--configuration", configuration], 0);
        assert_eq!(value["verdict"], "satisfied");
        assert_eq!(ledger_shape::original_field_count(&value), 13);
        assert_eq!(
            value["terminals"][0]["elapsed"],
            serde_json::json!({"min":elapsed,"max":elapsed})
        );
        assert_eq!(
            value["terminals"][0]["work"],
            serde_json::json!({"min":8,"max":8})
        );
        assert!(value.get("configuration").is_none());
    }
}

#[test]
fn configuration_selector_is_required_for_configs_forbidden_for_legacy_and_resolved_by_name() {
    let path = fixture_model("config-diff-parallel");
    let value = json_run(&["check", &path], 3);
    assert_code(&value, "E-CONF-0");
    assert_eq!(value["bounds"]["truncated"], serde_json::Value::Null);
    let legacy = fixture_model("serial-time");
    assert_code(
        &json_run(&["check", &legacy, "--configuration", "AsIs"], 3),
        "E-CONF-0",
    );
    assert_code(
        &json_run(&["check", &path, "--configuration", "Unknown"], 3),
        "E-NAME-0",
    );
}

#[test]
fn diff_black_box_checks_manual_metrics_and_accepts_either_flag_order() {
    let path = fixture_model("config-diff-parallel");
    for flags in [
        ["--from", "AsIs", "--to", "ToBeP"],
        ["--to", "ToBeP", "--from", "AsIs"],
    ] {
        let value = json_run(&["diff", &path, flags[0], flags[1], flags[2], flags[3]], 0);
        assert_eq!(value["verdict"], "satisfied");
        assert_eq!(ledger_shape::original_field_count(&value), 11);
        assert_eq!(value["from"]["configuration"], "AsIs");
        assert_eq!(value["to"]["configuration"], "ToBeP");
        assert_eq!(
            value["differences"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["kind"] == "actor" || r["kind"] == "relation")
                .count(),
            0
        );
        assert_eq!(value["mismatch"], serde_json::Value::Null);
        assert_eq!(
            value["terminals"][0]["from"]["elapsed"],
            serde_json::json!({"min":8,"max":8})
        );
        assert_eq!(
            value["terminals"][0]["to"]["elapsed"],
            serde_json::json!({"min":5,"max":5})
        );
        for side in ["from", "to"] {
            assert_eq!(
                value["terminals"][0][side]["work"],
                serde_json::json!({"min":8,"max":8})
            );
            assert_eq!(
                value["terminals"][0][side]["result_at"],
                serde_json::Value::Null
            );
        }
    }
}

#[test]
fn diff_exits_follow_parent_verdict_not_only_both_side_successes() {
    for (name, to, exit, verdict, code) in [
        (
            "config-diff-parallel.broken-intent-only",
            "ToBeP",
            3,
            "invalid",
            Some("E-DIFF-0"),
        ),
        (
            "config-diff-add-actor.broken-other-cut",
            "ToBe",
            3,
            "invalid",
            Some("E-CMP-0"),
        ),
        (
            "config-diff-add-actor.broken-missing-intent",
            "ToBe",
            0,
            "satisfied",
            Some("W-DIFF-1"),
        ),
        (
            "config-diff-retire.broken-missing-loses",
            "ToBe",
            0,
            "satisfied",
            Some("W-DIFF-2"),
        ),
        ("config-diff-terminal-union", "ToBe", 1, "violated", None),
    ] {
        let path = fixture_model(name);
        let value = json_run(&["diff", &path, "--from", "AsIs", "--to", to], exit);
        assert_eq!(value["verdict"], verdict);
        if let Some(code) = code {
            assert_code(&value, code);
        }
        if exit == 3 {
            assert_eq!(value["from"]["report"]["verdict"], "satisfied");
            assert_eq!(value["to"]["report"]["verdict"], "satisfied");
        }
    }
}

#[test]
fn malformed_diff_arguments_return_only_minimal_invalid_json() {
    let path = fixture_model("config-diff-parallel");
    for args in [
        vec!["diff"],
        vec!["diff", &path],
        vec!["diff", &path, "--from", "AsIs"],
        vec!["diff", &path, "--to", "ToBeP"],
        vec!["diff", &path, "--from", "AsIs", "--to"],
        vec![
            "diff", &path, "--from", "AsIs", "--to", "ToBeP", "--from", "AsIs",
        ],
        vec![
            "diff", &path, "--from", "AsIs", "--to", "ToBeP", "--to", "ToBeP",
        ],
        vec![
            "diff",
            &path,
            "--from",
            "AsIs",
            "--to",
            "ToBeP",
            "--unknown",
            "x",
        ],
        vec![
            "diff",
            &path,
            "--from",
            "AsIs",
            "--to",
            "ToBeP",
            "extra.relca",
        ],
    ] {
        assert_eq!(
            json_run(&args, 3),
            ledger_shape::extend_value(
                serde_json::json!({"verdict":"invalid"}),
                &ledger_shape::Spec::minimal("cli_refusal")
            ),
            "{args:?}"
        );
    }
}

#[test]
fn diff_unreadable_source_and_unknown_configuration_keep_both_nested_reports() {
    let missing =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/no-such-diff-input.relca");
    assert!(!missing.exists());
    let value = json_run(
        &[
            "diff",
            missing.to_str().unwrap(),
            "--from",
            "A",
            "--to",
            "B",
        ],
        3,
    );
    assert_eq!(ledger_shape::original_field_count(&value), 11);
    for (side, name) in [("from", "A"), ("to", "B")] {
        assert_eq!(value[side]["configuration"], name);
        let report = &value[side]["report"];
        assert_eq!(report["verdict"], "invalid");
        assert_eq!(report["model"]["sha256"], serde_json::Value::Null);
        assert_eq!(report["bounds"]["truncated"], serde_json::Value::Null);
        assert_eq!(ledger_shape::original_field_count(report), 13);
    }
    for field in ["differences", "terminals", "properties", "checked"] {
        assert_eq!(value[field], serde_json::json!([]));
    }
    let path = fixture_model("config-diff-parallel");
    let value = json_run(&["diff", &path, "--from", "Unknown", "--to", "ToBeP"], 3);
    assert_code(&value["from"]["report"], "E-NAME-0");
    assert_eq!(value["to"]["report"]["verdict"], "satisfied");
    assert_eq!(value["differences"], serde_json::json!([]));
}

#[test]
fn diff_runtime_inconclusive_unsupported_and_malformed_source_fail_closed() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/configuration-cli-tests");
    std::fs::create_dir_all(&root).unwrap();
    let parallel = include_str!("../../../fixtures/config-diff-parallel/model.relca");
    for (name, source, exit, verdict) in [
        (
            "clock",
            parallel.replace("clock 10min", "clock 4min"),
            2,
            "inconclusive",
        ),
        (
            "unsupported",
            parallel.replace(
                "configuration ToBeP { cut C; process Parallel }",
                "configuration ToBeP { cut C; process Parallel; intent { preserve_fallback Office } }",
            ),
            4,
            "unsupported",
        ),
        ("malformed", "model".into(), 3, "invalid"),
    ] {
        let path = root.join(format!("{name}.relca"));
        std::fs::write(&path, source).unwrap();
        let value = json_run(
            &[
                "diff",
                path.to_str().unwrap(),
                "--from",
                "AsIs",
                "--to",
                "ToBeP",
            ],
            exit,
        );
        assert_eq!(value["verdict"], verdict, "{name}");
        if name == "malformed" {
            for side in ["from", "to"] {
                assert_eq!(value[side]["report"]["verdict"], "invalid");
            }
        }
    }
}

#[test]
fn legacy_process_and_cut_cardinality_remain_unsupported() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/configuration-cli-tests");
    std::fs::create_dir_all(&root).unwrap();
    let legacy = include_str!("../../../fixtures/serial-time/model.relca");
    for addition in [
        "process Second { end done }",
        "cut C { include [Office] }\ncut D { include [Office] }",
    ] {
        let src = legacy.replace("  bounds", &format!("  {addition}\n  bounds"));
        let path = root.join(if addition.starts_with("process") {
            "legacy-two-processes.relca"
        } else {
            "legacy-two-cuts.relca"
        });
        std::fs::write(&path, src).unwrap();
        assert_code(&json_run(&["check", path.to_str().unwrap()], 4), "U-SEM-0");
    }
}
