//! Real CLI checks. Each invocation gets a process + atomic nonce directory.
#[path = "../../relca-core/tests/support/p2_cleanup_inputs.rs"]
mod input;
use serde_json::{Value, json};
fn run(s: &str, args: &[&str], exit: i32) -> Value {
    let d = input::dir();
    let p = d.join("model.relca");
    std::fs::write(&p, s).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(args[0])
        .arg(&p)
        .args(&args[1..])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn c1_cli_prefix_only_diff() {
    let v = run(
        &input::prefix_diff(),
        &["diff", "--from", "From", "--to", "To"],
        0,
    );
    let rows: Vec<_> = v["agency"]["differences"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["viewpoint"] == "degradation")
        .collect();
    assert_eq!(rows.len(), 1);
    let r = rows[0];
    assert_eq!(r["item"], "common_prefix");
    assert_eq!(r["call"], "Manual");
    assert_eq!(r["status"], "changed");
    assert_eq!(
        r["from"],
        json!([{"capability":"Probe.check","min":1,"max":1},{"capability":"Clerk.run","min":0,"max":0},{"capability":"Fast.run","min":0,"max":0}])
    );
    assert_eq!(
        r["to"],
        json!([{"capability":"Probe.check","min":2,"max":2},{"capability":"Clerk.run","min":0,"max":0},{"capability":"Fast.run","min":0,"max":0}])
    );
}
#[test]
fn q7_cli_stochastic_check_has_no_spurious_ledger_error() {
    let v = run(&input::stochastic(), &["check"], 4);
    let errors: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0]["code"], "U-SEM-0");
    assert_eq!(errors[0]["message"], "stochastic_arrivals_mode_unsupported");
    serde_json::from_value::<relca_core::report::Report>(v).unwrap();
}
#[test]
fn c4_cli_whole_table_withdrawal_is_not_an_empty_equality() {
    let v = run(
        &input::withdrawn_diff(),
        &["diff", "--from", "From", "--to", "To"],
        0,
    );
    let rows: Vec<_> = v["agency"]["differences"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["viewpoint"] == "degradation")
        .collect();
    assert_eq!(rows.len(), 3);
    for item in ["normal", "degraded", "common_prefix"] {
        let r = rows.iter().find(|r| r["item"] == item).unwrap();
        assert!(r.get("call").is_none());
        assert_eq!(r["status"], "not_comparable");
        for owner in ["from", "to"] {
            assert_eq!(
                r[owner],
                json!({"status":"not_derived","reason":format!("{owner}.agency.degradation:multiple_case_interference")})
            );
        }
    }
}
#[test]
fn c2_cli_all_three_owners_have_new_bundle_and_local_rules() {
    let v = run(input::WORK, &["diff", "--from", "From", "--to", "To"], 0);
    assert_eq!(v["agency"]["differences"]["rules"], "agency-diff-p2-1");
    for p in [
        "/ledger/derivation/bundle",
        "/from/report/ledger/derivation/bundle",
        "/to/report/ledger/derivation/bundle",
    ] {
        assert_eq!(v.pointer(p).unwrap(), "relca-conclusions-p2-uncertainty-2");
    }
    serde_json::from_value::<relca_core::diff::DiffReport>(v).unwrap();
}
#[test]
fn q9_cli_known_failure_is_not_lost_to_later_range_stop() {
    let v = run(input::KNOWN_FAILURE, &["simulate"], 1);
    assert_eq!(v["verdict"], "violated");
    assert_eq!(v["simulation"]["range_stopped_replications"], 2);
    let r = &v["metrics"]["contests"]["rows"][0];
    assert_eq!(r["filed"], 2);
    assert_eq!(r["route_failed"], 2);
    assert_eq!(r["unresolved"], 0);
    assert_eq!(r["service_time"]["reason"], "no_samples");
}
