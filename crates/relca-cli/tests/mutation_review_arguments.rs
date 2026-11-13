use serde_json::Value;
use std::{path::PathBuf, process::Command};
fn invoke(command: &str, options: &[&str]) -> (i32, Value) {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/support/configuration_operations.relca");
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(p)
        .args(options)
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}
#[test]
fn minor4_configuration_values_follow_check_not_the_mutation_set_rule() {
    for name in ["", "--bad"] {
        let (_, check) = invoke("check", &["--scenario", "Late", "--configuration", name]);
        let (exit, mutate) = invoke("mutate", &["--scenario", "Late", "--configuration", name]);
        assert_eq!(exit, 3);
        let original = if mutate["original"].is_object() {
            &mutate["original"]
        } else {
            &mutate
        };
        assert_eq!(
            original["diagnostics"][0]["code"],
            check["diagnostics"][0]["code"]
        );
    }
}
#[test]
fn nit_unaccepted_mutation_limits_and_sites_are_not_recorded_as_128() {
    for args in [
        vec!["--mutant-limit", "9999"],
        vec!["--mutant-limit", "0"],
        vec![
            "--scenario",
            "Late",
            "--configuration",
            "First",
            "--sites",
            "unknown",
        ],
    ] {
        let (exit, v) = invoke("mutate", &args);
        assert_eq!(exit, 3);
        for value in [
            &v["run"]["mutant_limit"],
            &v["ledger"]["execution"]["limits"]["mutant_limit"],
        ] {
            assert_eq!(value["status"], "not_applicable");
            assert!(value["value"].is_null());
            assert_eq!(value["reason"], "no_analysis");
        }
    }
}

#[test]
fn scenario_values_are_checked_identically_before_loading_or_selection() {
    for value in ["", "-bad", "--bad", "Late", "Missing"] {
        let (_, check) = invoke("check", &["--configuration", "First", "--scenario", value]);
        let (_, mutation) = invoke("mutate", &["--configuration", "First", "--scenario", value]);
        if check["verdict"] == "satisfied" {
            assert_eq!(mutation["original"], check);
        } else {
            let report = if mutation["original"].is_object() {
                &mutation["original"]
            } else {
                &mutation
            };
            assert_eq!(
                report["diagnostics"][0]["code"], check["diagnostics"][0]["code"],
                "{value}"
            );
        }
    }
}
