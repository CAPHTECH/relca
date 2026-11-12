//! Real CLI checks for inline survivor inputs; the closed fixture census stays unchanged.
#[allow(dead_code)]
#[path = "../../relca-core/tests/waits_mutations.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use std::{fs, process::Command};

#[test]
fn survivor_inline_inputs_meet_real_binary_exit_and_hand_arithmetic() {
    let dir = test_tmp::root().join(format!("relca-waits-survivors-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    for id in acceptance::IDS {
        let (source, scenario, verdict, exit) = acceptance::input(id);
        let path = dir.join(format!("{id}.relca"));
        fs::write(&path, source).unwrap();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_relca"));
        cmd.arg("check").arg(&path);
        if let Some(scenario) = scenario {
            cmd.args(["--scenario", scenario]);
        }
        let out = cmd.output().unwrap();
        let stderr = std::str::from_utf8(&out.stderr).unwrap();
        let mut stderr_codes: Vec<_> = stderr
            .lines()
            .map(|line| {
                line.split_once(": ")
                    .expect("stderr must contain diagnostic lines only")
                    .0
            })
            .collect();
        stderr_codes.sort_unstable();
        assert_eq!(
            stderr_codes,
            acceptance::diagnostic_codes(id),
            "{id}: exact stderr code multiset"
        );
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        acceptance::diagnostics(id, &v);
        assert_eq!(out.status.code(), Some(exit), "{id}: {}", v["diagnostics"]);
        assert_eq!(v["verdict"], verdict, "{id}");
        fixture_match::compare_report(&serde_json::json!({}), &v).unwrap();
        acceptance::hand(id, &v["metrics"]);
    }
}
