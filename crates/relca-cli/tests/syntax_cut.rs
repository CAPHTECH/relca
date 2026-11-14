//! Cut presence is known only after a successful parse, not from source text or
//! diagnostic codes. These expectations are handwritten, not snapshot-derived.
#[path = "support/sensitivity_harness.rs"]
mod h;

use serde_json::Value;

const MODEL: &str = r#"model SyntaxCut version "0.2" {
  type T = enum { ok }
  actor A { capacity 1; capability run() -> T {
    duration fixed 1min; result nondet { ok }
  } }
  process P { x = A.run(); end done }
  scenario Late { arrivals fixed [3min] }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify safety no_resource_overcommit
}"#;

fn reports<'a>(v: &'a Value, found: &mut Vec<&'a Value>) {
    match v {
        Value::Object(fields) => {
            if fields.contains_key("cut")
                && fields.contains_key("model")
                && fields.contains_key("diagnostics")
            {
                found.push(v);
            }
            for child in fields.values() {
                reports(child, found);
            }
        }
        Value::Array(items) => {
            for child in items {
                reports(child, found);
            }
        }
        _ => {}
    }
}

fn acceptance(source: &str, parsed: bool, declared_cut: bool, exit: i32, code: &str) {
    assert_eq!(relca_core::syntax::parse(source).is_ok(), parsed);
    if parsed {
        let diagnostics = relca_core::lower::lower(&relca_core::syntax::parse(source).unwrap())
            .expect_err("the handwritten undefined Actor must stop lowering");
        assert!(diagnostics.iter().any(|d| d.code == code));
    }
    let dir = h::dir();
    let path = h::write(&dir, "model.relca", source);
    let log = h::write(&dir, "events.jsonl", "");
    let approval = h::write(&dir, "approval.json", "{}");
    let manifest = h::manifest(vec![h::point("bad", "model.relca")], false);
    let manifest_path = h::write(
        &dir,
        "manifest.json",
        serde_json::to_vec(&manifest).unwrap(),
    );
    let simulation_manifest = h::manifest(vec![h::point("bad", "model.relca")], true);
    let simulation_manifest_path = h::write(
        &dir,
        "simulation.json",
        serde_json::to_vec(&simulation_manifest).unwrap(),
    );
    let path = path.to_str().unwrap();
    let log = log.to_str().unwrap();
    let approval = approval.to_str().unwrap();
    for (command, options, children) in [
        ("check", vec![], 1),
        (
            "check",
            vec!["--configuration", "First", "--scenario", "Late"],
            1,
        ),
        ("explain", vec![], 1),
        ("explain", vec!["--configuration", "First"], 1),
        ("simulate", vec![], 1),
        (
            "simulate",
            vec!["--configuration", "First", "--scenario", "Late"],
            1,
        ),
        ("mutate", vec![], 1),
        ("mutate", vec!["--configuration", "First"], 1),
        (
            "compare",
            vec!["--plans", "First,Second", "--mode", "exhaustive"],
            2,
        ),
        (
            "compare",
            vec!["--plans", "First,Second", "--mode", "simulation"],
            2,
        ),
        ("diff", vec!["--from", "First", "--to", "Second"], 2),
        (
            "conform",
            vec!["--log", log, "--scope", "event-contracts"],
            1,
        ),
        (
            "conform",
            vec![
                "--log",
                log,
                "--scope",
                "approval-burden",
                "--approval-spec",
                approval,
            ],
            1,
        ),
        ("sensitivity", vec!["--mode", "exhaustive"], 1),
        ("sensitivity", vec!["--mode", "simulation"], 1),
    ] {
        let input = if command == "sensitivity" {
            if options.contains(&"simulation") {
                simulation_manifest_path.to_str().unwrap()
            } else {
                manifest_path.to_str().unwrap()
            }
        } else {
            path
        };
        let mut args = vec![command, input];
        args.extend(options);
        let out = h::cli(&args);
        assert_eq!(out.exit, exit, "{args:?}: {}", out.stderr);
        assert_eq!(
            out.stderr.contains("W-CUT-0"),
            parsed && !declared_cut,
            "{args:?}: {}",
            out.stderr
        );
        let mut found = Vec::new();
        reports(&out.value, &mut found);
        assert_eq!(found.len(), children, "{args:?}: {}", out.value);
        for report in found {
            // Conformance already leaves Cut unresolved on a lowering refusal.
            let cut = if parsed && !declared_cut && command != "conform" {
                "undeclared"
            } else {
                "unresolved"
            };
            assert_eq!(report["cut"], cut, "{args:?}");
            assert!(report["model"]["sha256"].is_string(), "{args:?}");
            for key in ["properties", "terminals"] {
                if let Some(rows) = report.get(key) {
                    assert_eq!(rows, &serde_json::json!([]), "{args:?}: {key}");
                }
            }
            let diagnostics = report["diagnostics"].as_array().unwrap();
            assert_eq!(
                diagnostics
                    .iter()
                    .filter(|d| d["code"] == "W-CUT-0")
                    .count(),
                usize::from(parsed && !declared_cut),
                "{args:?}"
            );
            if !parsed {
                assert!(diagnostics.iter().any(|d| d["code"] == code), "{args:?}");
            }
        }
    }
}

#[test]
fn syntax_error_with_cut_is_unresolved_in_every_command() {
    let source = MODEL.replace("  type T", "  cut Scope { include [A] }\n  type T");
    acceptance(
        &source.replace("end done", "end }"),
        false,
        true,
        3,
        "E-SYN-0",
    );
}

#[test]
fn syntax_error_without_cut_is_unresolved_in_every_command() {
    acceptance(
        &MODEL.replace("end done", "end }"),
        false,
        false,
        3,
        "E-SYN-0",
    );
}

#[test]
fn unsupported_syntax_and_lexer_failure_do_not_claim_cut_presence() {
    acceptance(
        &MODEL.replace("end done", "while true {}"),
        false,
        false,
        4,
        "U-SYN-0",
    );
    acceptance(&format!("{MODEL} @"), false, false, 3, "E-SYN-0");
}

#[test]
fn parsed_lower_failure_without_cut_still_warns_in_every_command() {
    acceptance(
        &MODEL.replace("A.run()", "Missing.run()"),
        true,
        false,
        3,
        "E-NAME-0",
    );
}

#[test]
fn parsed_lower_failure_with_cut_stays_unresolved_without_warning_in_every_command() {
    let source = MODEL.replace("  type T", "  cut Scope { include [A] }\n  type T");
    acceptance(
        &source.replace("A.run()", "Missing.run()"),
        true,
        true,
        3,
        "E-NAME-0",
    );
}
