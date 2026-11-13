use serde_json::Value;
use std::{path::PathBuf, process::Command};

fn invoke(command: &str, options: &[&str]) -> (i32, Value) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/support/configuration_operations.relca");
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(path)
        .args(options)
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        serde_json::from_slice(&out.stdout).unwrap(),
    )
}

#[test]
fn mutate_and_explain_use_the_same_selected_configuration_as_check() {
    for (name, process, elapsed) in [("First", "One", 1), ("Second", "Two", 2)] {
        let args = ["--configuration", name, "--scenario", "Late"];
        let (exit, check) = invoke("check", &args);
        assert_eq!(exit, 0);
        assert_eq!(
            check["case_outcomes"][0]["terminals"][0]["elapsed"]["min"],
            elapsed
        );
        let (exit, explain) = invoke("explain", &args);
        assert_eq!(exit, 0);
        assert_eq!(explain["terminals"], check["terminals"]);
        assert_eq!(explain["properties"], check["properties"]);
        assert_eq!(explain["explanation"]["status"], "complete");
        let (exit, mutate) = invoke(
            "mutate",
            &[
                "--configuration",
                name,
                "--scenario",
                "Late",
                "--kinds",
                "authority_expired",
            ],
        );
        assert_eq!(exit, 0);
        assert_eq!(mutate["original"], check);
        assert_eq!(mutate["mutations"][1]["mutants"][0]["result"], "detected");
        for report in [
            &explain,
            &mutate["original"],
            &mutate["mutations"][1]["mutants"][0]["report"],
        ] {
            assert_eq!(
                report["ledger"]["execution"]["selection"]["requested"]["configuration"],
                name
            );
            assert_eq!(
                report["ledger"]["execution"]["selection"]["effective"]["process"]["value"],
                process
            );
        }
    }
}

#[test]
fn missing_or_unknown_configuration_does_not_fall_back() {
    for selection in [
        vec!["--scenario", "Late"],
        vec!["--scenario", "Late", "--configuration", "Missing"],
    ] {
        let (_, check) = invoke("check", &selection);
        assert_eq!(check["verdict"], "invalid");
        for command in ["mutate", "explain"] {
            let (exit, v) = invoke(command, &selection);
            assert_eq!(exit, 3);
            let original = if command == "mutate" {
                &v["original"]
            } else {
                &v
            };
            assert_eq!(original["diagnostics"], check["diagnostics"]);
            assert_eq!(original["model"]["sha256"], check["model"]["sha256"]);
            if command == "mutate" {
                assert_eq!(v["run"]["completion"], "blocked");
            }
        }
    }
}
