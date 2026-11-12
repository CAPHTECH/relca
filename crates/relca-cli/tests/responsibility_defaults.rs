//! Raw CLI acceptance: deliberately never use a historical-output projector.
#[path = "../../relca-core/tests/support/responsibility_defaults_inputs.rs"]
mod input;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Source(PathBuf);
impl Source {
    fn new(source: &str) -> Self {
        let path = test_tmp::root().join(format!(
            "relca-k4-{}-{}.relca",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, source).unwrap();
        Self(path)
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}
fn run(source: &str, command: &str, flags: &[&str]) -> (Value, String, i32) {
    let file = Source::new(source);
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(&file.0)
        .args(flags)
        .output()
        .unwrap();
    (
        serde_json::from_slice(&o.stdout).unwrap(),
        String::from_utf8(o.stderr).unwrap(),
        o.status.code().unwrap(),
    )
}
fn rows(v: &Value) -> Vec<&Value> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-RESP-0")
        .collect()
}
fn expected(source: &str, actor: &str) -> Value {
    let ast = relca_core::syntax::parse(source).unwrap();
    let span = ast
        .items
        .iter()
        .find_map(|item| match item {
            relca_core::ast::Item::Actor(a) if a.name.value == actor => Some(a.span),
            _ => None,
        })
        .unwrap();
    json!({"code":"W-RESP-0", "severity":"warning", "message":format!("{actor} has no declared responsibility"), "span":span})
}
fn two_plans(source: &str) -> String {
    // Simulation requires distinct process operation keys across plans.
    source.replace(
        "bounds {",
        "process Other { p = Poster.post(); end done }\nconfiguration A { cut Scope; process Flow }\nconfiguration B { cut Scope; process Other }\nbounds {",
    )
}
#[test]
fn raw_check_positive_and_broken_duty_pairs_have_exact_rows_and_stderr() {
    let mut failures = Vec::new();
    for (name, source, names) in input::cases() {
        if std::panic::catch_unwind(|| {
            let (v, err, code) = run(&source, "check", &[]);
            assert_eq!(code, 0, "{name}: {v}");
            let got = rows(&v);
            assert_eq!(got.len(), names.len(), "{name}: {v}");
            for (row, actor) in got.iter().zip(&names) {
                assert_eq!(**row, expected(&source, actor), "{name}");
            }
            let resp: Vec<_> = err.lines().filter(|s| s.contains("W-RESP-0")).collect();
            let expected: Vec<_> = names
                .iter()
                .map(|a| format!("W-RESP-0: {a} has no declared responsibility"))
                .collect();
            assert_eq!(resp, expected, "{name}");
        })
        .is_err()
        {
            failures.push(name);
        }
    }
    assert!(
        failures.is_empty(),
        "all failing raw CLI pairs: {failures:?}"
    );
}
#[test]
fn raw_cli_modes_keep_warning_transport_prefix_and_exit_separate() {
    let base = input::state();
    let selected = two_plans(&base);
    for (command, flags, source, pointers, prefixes, exit) in [
        ("check", vec![], &base, vec![""], vec![""], 0),
        ("explain", vec![], &base, vec![""], vec![""], 0),
        ("simulate", vec![], &base, vec![""], vec![""], 2),
        (
            "compare",
            vec!["--plans", "A,B", "--mode", "exhaustive"],
            &selected,
            vec!["/plans/0/report", "/plans/1/report"],
            vec!["configuration A: ", "configuration B: "],
            0,
        ),
        (
            "compare",
            vec!["--plans", "A,B", "--mode", "simulation"],
            &selected,
            vec!["/plans/0/report", "/plans/1/report"],
            vec!["configuration A: ", "configuration B: "],
            2,
        ),
        (
            "diff",
            vec!["--from", "A", "--to", "B"],
            &selected,
            vec!["/from/report", "/to/report"],
            vec!["from: ", "to: "],
            0,
        ),
        (
            "mutate",
            vec![],
            &base,
            vec!["/original"],
            vec!["original: "],
            4,
        ),
    ] {
        let (v, err, code) = run(source, command, &flags);
        assert_eq!(code, exit, "{command}: {v}");
        for pointer in pointers {
            let r = v.pointer(pointer).unwrap();
            assert_eq!(rows(r).len(), 1, "{command} {pointer}: {v}");
            assert_eq!(*rows(r)[0], expected(source, "Poster"));
        }
        let lines: Vec<_> = err.lines().filter(|s| s.contains("W-RESP-0")).collect();
        assert_eq!(
            lines,
            prefixes
                .iter()
                .map(|p| format!("{p}W-RESP-0: Poster has no declared responsibility"))
                .collect::<Vec<_>>(),
            "{command}: {err}"
        );
        if command == "simulate" {
            assert_eq!(v["verdict"], "inconclusive");
            assert_eq!(
                v["agency"]["viewpoints"]["effects"]["rows"][0]["process_judgements"]["reason"],
                "state_guards"
            );
        }
    }
}
#[test]
fn no_effect_raw_control_keeps_stderr_empty() {
    let (v, err, code) = run(&input::plain(), "check", &[]);
    assert_eq!(code, 0);
    assert_eq!(v["diagnostics"], json!([]));
    assert_eq!(err, "");
}
#[test]
fn legacy_substrate_accepts_every_transport_before_default_warning_implementation() {
    let source = input::state();
    let selected = two_plans(&source);
    for (command, flags, body, exit) in [
        ("check", vec![], &source, 0),
        ("explain", vec![], &source, 0),
        ("simulate", vec![], &source, 2),
        (
            "compare",
            vec!["--plans", "A,B", "--mode", "exhaustive"],
            &selected,
            0,
        ),
        (
            "compare",
            vec!["--plans", "A,B", "--mode", "simulation"],
            &selected,
            2,
        ),
        ("diff", vec!["--from", "A", "--to", "B"], &selected, 0),
        ("mutate", vec![], &source, 4),
    ] {
        let (value, _, code) = run(body, command, &flags);
        assert_eq!(code, exit, "{command} substrate: {value}");
    }
}
#[test]
fn latent_policy_refusal_is_not_hidden_by_default_warning() {
    let source = input::binding()
        .replace(
            "actor Input {",
            "latent case { hidden: bool = false }\nactor Input {",
        )
        .replace("if v == pass", "if hidden == true");
    let (v, err, code) = run(&source, "check", &[]);
    assert_eq!(code, 3);
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-POL-LATENT")
    );
    assert!(rows(&v).is_empty());
    assert!(err.contains("E-POL-LATENT"));
}

#[test]
fn shared_person_duty_and_people_only_raw_cli_keep_scope_disclosures() {
    for (source, actors, marker, present) in [
        (
            input::binding(),
            vec!["Judge", "Poster"],
            "responsibility_people_undeclared",
            true,
        ),
        (
            input::people_only(),
            vec!["Judge", "Poster"],
            "responsibility_people_undeclared",
            false,
        ),
        (
            input::shared_person(false),
            vec!["Judge", "Poster"],
            "responsibility_assignment_scope",
            false,
        ),
        (
            input::shared_person(true),
            vec![],
            "responsibility_assignment_scope",
            true,
        ),
    ] {
        let (v, _, code) = run(&source, "check", &[]);
        assert_eq!(code, 0);
        let want: Vec<_> = actors.iter().map(|a| expected(&source, a)).collect();
        assert_eq!(rows(&v), want.iter().collect::<Vec<_>>());
        assert_eq!(
            v["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!(marker)),
            present
        );
    }
}

fn mixed_matches(v: &Value, stderr: &str) -> bool {
    v["diagnostics"] == input::mixed_expected() && stderr == input::MIXED_STDERR
}

#[test]
fn full_mixed_raw_cli_diagnostics_and_all_stderr_lines_reject_order_faults() {
    let (v, stderr, code) = run(&input::mixed_diagnostics(), "check", &[]);
    assert_eq!(code, 0);
    assert_eq!(v["diagnostics"], input::mixed_expected());
    assert_eq!(stderr, input::MIXED_STDERR);
    assert!(mixed_matches(&v, &stderr));
    let mut swapped = v.clone();
    swapped["diagnostics"].as_array_mut().unwrap().swap(0, 1);
    assert!(!mixed_matches(&swapped, &stderr));
    let reversed = "W-RESP-0: Poster has no declared responsibility\nW-CUT-0: Cut is undeclared: all declared Actors are included; exclusions are unknown\n";
    assert!(!mixed_matches(&v, reversed));
}

#[test]
fn effects_not_derived_raw_cli_discloses_default_candidates_with_reason_and_process() {
    for (calls, derived) in [(1, true), (100, false)] {
        let (v, _, code) = run(&input::effects_limit(calls), "check", &[]);
        assert_eq!(
            code,
            if derived { 0 } else { 2 },
            "runtime completion/cutoff is separate from static derivation: {v}"
        );
        let effects = &v["agency"]["viewpoints"]["effects"];
        assert_eq!(
            rows(&v),
            vec![&expected(&input::effects_limit(calls), "Poster")]
        );
        assert_eq!(
            effects["status"],
            if derived { "derived" } else { "not_derived" }
        );
        if !derived {
            assert_eq!(effects["reason"], "derivation_limit");
        }
        assert_eq!(
            v["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s.as_str() == Some(input::UNDERIVED_NOTICE))
                .count(),
            usize::from(!derived)
        );
    }
}
