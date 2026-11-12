#[path = "../../relca-core/tests/support/evaluation_c6_acceptance.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "support/test_tmp.rs"]
mod test_tmp;

fn run(src: &str, mutate: bool, day: bool) -> serde_json::Value {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static ID: AtomicUsize = AtomicUsize::new(0);
    let dir = test_tmp::root().join(format!(
        "relca-b7onto-{}-{}",
        std::process::id(),
        ID.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("model.relca");
    std::fs::write(&p, src).unwrap();
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_relca"));
    cmd.arg(if mutate { "mutate" } else { "check" }).arg(&p);
    if day {
        cmd.args(["--scenario", "Day"]);
    }
    let out = cmd.output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let verdict: relca_core::Verdict = serde_json::from_value(v["verdict"].clone()).unwrap();
    assert_eq!(out.status.code(), Some(verdict.exit_code()));
    if mutate {
        fixture_match::validate_mutation_wire(&v).unwrap();
    }
    v
}

#[test]
fn wire_null_report_exception_does_not_leak_to_six_executable_kinds() {
    let src = acceptance::day();
    let v = run(&src, true, true);
    for i in [2, 8] {
        let mut bad = v.clone();
        bad["mutations"][i]["mutants"][0]["report"] = serde_json::Value::Null;
        assert!(fixture_match::validate_mutation_wire(&bad).is_err());
    }
    let mut bad = v.clone();
    bad["mutations"][7]["mutants"][0]["operator"] = serde_json::json!("unknown");
    assert!(fixture_match::validate_mutation_wire(&bad).is_err());
}

#[test]
fn wire_keeps_expiry_summary_and_kind7_operator_boundaries() {
    let v = run(&acceptance::mixed(), true, true);
    for (i, field, value) in [
        (1, "operator", serde_json::json!("remove_denominator_end")),
        (4, "operator", serde_json::json!("expire_authority")),
        (7, "report", v["original"].clone()),
    ] {
        let mut bad = v.clone();
        bad["mutations"][i]["mutants"][0][field] = value;
        assert!(fixture_match::validate_mutation_wire(&bad).is_err());
    }
    for i in [1, 4] {
        let mut bad = v.clone();
        bad["mutations"][i]["mutants"][0]["report"] = serde_json::Value::Null;
        assert!(fixture_match::validate_mutation_wire(&bad).is_err());
    }
    let mut bad = v.clone();
    bad["mutations"][1]["mutants"][0]["assigned_changes"][0]["direction"] =
        serde_json::json!("held");
    assert!(fixture_match::validate_mutation_wire(&bad).is_err());
}
