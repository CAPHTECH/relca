//! Real CLI acceptance: JSON shape, all ten rows, exact hand candidate population.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
        .join("model.relca")
}
fn invoke(args: &[&str]) -> (i32, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .output()
        .unwrap();
    let v = serde_json::from_slice(&out.stdout).expect("exactly one JSON object on stdout");
    (out.status.code().unwrap(), v)
}
#[test]
fn m_default_explicit_set_determinism_all_ten_rows_all_sites() {
    for name in fixture_match::MUTATION_FIXTURES {
        let p = path(name);
        let p = p.to_str().unwrap();
        let (exit, v) = invoke(&["mutate", p]);
        fixture_match::validate_mutation_wire(&v).unwrap();
        fixture_match::compare_mutation_fixture_sites(name, &v).unwrap();
        let source = std::fs::read_to_string(path(name)).unwrap();
        for row in v["mutations"].as_array().unwrap() {
            for m in row["mutants"].as_array().unwrap() {
                let site = &m["site"];
                let start = site["span"]["start"].as_u64().unwrap() as usize;
                let end = site["span"]["end"].as_u64().unwrap() as usize;
                assert!(
                    start < end
                        && end <= source.len()
                        && source.is_char_boundary(start)
                        && source.is_char_boundary(end)
                );
                let prefix = &source[..start];
                assert_eq!(
                    site["line"].as_u64().unwrap() as usize,
                    prefix.bytes().filter(|b| *b == b'\n').count() + 1
                );
                assert_eq!(
                    site["column"].as_u64().unwrap() as usize,
                    prefix.rsplit('\n').next().unwrap().chars().count() + 1
                );
            }
        }
        let expected: Value = serde_json::from_str(
            &std::fs::read_to_string(path(name).with_file_name("expected.json")).unwrap(),
        )
        .unwrap();
        let inv = fixture_match::Invocation::parse(Some(&json!({"command":"mutate"}))).unwrap();
        inv.compare(&expected, &v).unwrap();
        let verdict: relca_core::Verdict =
            serde_json::from_value(expected["verdict"].clone()).unwrap();
        assert_eq!(exit, verdict.exit_code());
        assert_eq!(
            (exit, v.clone()),
            invoke(&["mutate", p, "--set", "bpr-default"])
        );
        assert_eq!((exit, v), invoke(&["mutate", p]));
    }
}
#[test]
fn m_bad_arguments_and_input_failure_are_invalid_before_kind_fold() {
    let p = path("mut-empty");
    let p = p.to_str().unwrap();
    for args in [
        vec!["mutate"],
        vec!["mutate", p, "extra"],
        vec!["mutate", p, "--set"],
        vec!["mutate", p, "--set", ""],
        vec!["mutate", p, "--set", "other"],
        vec!["mutate", p, "--set", "bpr-default", "--set", "bpr-default"],
        vec!["mutate", p, "--set=bpr-default"],
        vec!["mutate", p, "--unknown"],
        vec!["mutate", p, "--mode", "exhaustive"],
        vec!["mutate", p, "--configuration", "Flow"],
        vec!["mutate", p, "--limit", "1"],
        vec!["mutate", "--set", "bpr-default"],
        vec!["mutate", "no-such-mutation-input.relca"],
        vec!["mutate", "no-such-mutation-input.relca", "--set", "other"],
    ] {
        let (exit, v) = invoke(&args);
        assert_eq!(exit, 3, "{args:?}");
        fixture_match::validate_mutation_wire(&v).unwrap();
        assert_eq!(v["verdict"], "invalid");
        assert!(v["original"].is_null());
        assert!(
            v["mutations"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["mutants"] == json!([]))
        );
        assert_eq!(
            v["run"]["reason"],
            if args == vec!["mutate", "no-such-mutation-input.relca"] {
                "input_failure"
            } else {
                "invalid_arguments"
            }
        );
        if v["run"]["reason"] == "invalid_arguments" {
            assert!(
                v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "E-SYN-0")
            );
        }
    }
}

#[test]
fn m_check_bytes_and_stderr_unchanged_around_mutate() {
    for name in fixture_match::MUTATION_FIXTURES {
        let p = path(name);
        let check = || {
            Command::new(env!("CARGO_BIN_EXE_relca"))
                .arg("check")
                .arg(&p)
                .output()
                .unwrap()
        };
        let before = check();
        let _ = invoke(&["mutate", p.to_str().unwrap()]);
        let after = check();
        assert_eq!(before.stdout, after.stdout);
        assert_eq!(before.stderr, after.stderr);
        assert_eq!(before.status, after.status);
        let v: Value = serde_json::from_slice(&after.stdout).unwrap();
        assert!(v.get("mutations").is_none());
        assert!(
            !v["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["field"].as_str().unwrap().starts_with("mutation."))
        );
    }
}
