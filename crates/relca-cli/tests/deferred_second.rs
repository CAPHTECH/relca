#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/deferred_second_inputs.rs"]
mod h;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::Value;
static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
fn run(src: &str, name: &str) -> Value {
    run_command(src, name, "check")
}
fn run_command(src: &str, name: &str, command: &str) -> Value {
    let dir = test_tmp::root().join(format!("k2fix2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "{name}-{}.relca",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&path, src).unwrap();
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_relca"));
    c.arg(command).arg(&path);
    if let Some(s) = h::scenario(name) {
        c.args(["--scenario", s]);
    }
    if name == "if_none" {
        c.args(["--configuration", "Left"]);
    }
    let o = c.output().unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(
        o.status.code(),
        Some(if h::scenario(name).is_some() { 2 } else { 0 }),
        "{v}"
    );
    v
}
macro_rules! case {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            let control = run(&h::source($name, false), $name);
            h::assert_control(&control["metrics"], $name);
            let src = h::source($name, true);
            let v = run(&src, $name);
            h::assert_preserved_structure_and_counts(&control["metrics"], &v["metrics"]);
            h::assert_all_quantities(&v["metrics"]);
            fixture_match::validate_deferred_metrics_wire(&v).unwrap();
            fixture_match::validate_deferred_metrics_with_source(&v, &src, None).unwrap();
        }
    };
}
case!(real_non_d_immediate_cancel_work_and_cost, "immediate");
case!(real_non_d_horizon_work_and_cost, "horizon");
case!(
    real_non_d_unfinished_route_work_and_cost,
    "unfinished_route"
);
#[test]
fn real_if_none_end_activates_contest_in_both_matchers() {
    let src = h::source("if_none", true);
    let v = run(&src, "if_none");
    let wire = fixture_match::validate_deferred_metrics_wire(&v);
    let source = fixture_match::validate_deferred_metrics_with_source(&v, &src, Some("Left"));
    assert_eq!(
        wire.is_ok(),
        source.is_ok(),
        "wire={wire:?}, source={source:?}"
    );
    wire.unwrap();
    source.unwrap();
}

#[test]
fn hand_if_none_end_wire_is_accepted_at_both_independent_seams() {
    let src = h::source("if_none", true);
    let mut v = run(&src, "if_none");
    // Hand corrections required by root B, not a copy of production projection.
    let null = serde_json::json!({"min":null,"max":null,"reason":"not_configured"});
    v["metrics"]["actors"][0]["work"] = null.clone();
    v["metrics"]["actors"][0]["occupancy_cost"] = null.clone();
    v["metrics"]["capabilities"][0]["start_cost"] = null.clone();
    v["metrics"]["cases"][0]["tasks"][0]["work"] = null;
    let wire = fixture_match::validate_deferred_metrics_wire(&v);
    let source = fixture_match::validate_deferred_metrics_with_source(&v, &src, Some("Left"));
    assert_eq!(
        wire.is_ok(),
        source.is_ok(),
        "wire={wire:?}, source={source:?}"
    );
    wire.unwrap();
    source.unwrap();
    let disconnected = src.replace("if none -> returned", "if none -> posted");
    assert!(
        fixture_match::validate_deferred_metrics_with_source(&v, &disconnected, Some("Left"))
            .is_err(),
        "an unselected end declaration alone must not activate its route"
    );
}

#[test]
fn report_wire_rejects_prior_reason_null_case_without_defer_and_accepts_declared() {
    #[path = "../../relca-core/tests/support/deferred_review_inputs.rs"]
    mod quality;
    for reason in ["truncated", "undetermined"] {
        for deferred in [false, true] {
            let src = quality::source(reason, deferred);
            let mut v = serde_json::json!(relca_core::report::check_with_selection(
                &src,
                Some("Left"),
                quality::scenario(reason)
            ));
            // One changed subtree: keep the old prior reason and all other
            // report fields. Only the two case endpoints become null.
            v["quality"]["properties"][0]["cases"][0]["min"] = Value::Null;
            v["quality"]["properties"][0]["cases"][0]["max"] = Value::Null;
            assert_eq!(
                fixture_match::validate_deferred_metrics_wire(&v).is_ok(),
                deferred
            );
            assert_eq!(
                fixture_match::validate_deferred_metrics_with_source(&v, &src, Some("Left"))
                    .is_ok(),
                deferred
            );
        }
    }
}

#[test]
fn raw_second_inputs_are_accepted_by_both_matchers() {
    // The preceding 93 jobs run in deferred_review; this adds the 16 new jobs.
    let mut executed = 0;
    for name in h::NAMES.into_iter().chain(["if_none"]) {
        for deferred in [false, true] {
            let src = h::source(name, deferred);
            let selection = (name == "if_none").then_some("Left");
            for command in ["check", "explain"] {
                let v = run_command(&src, name, command);
                fixture_match::validate_deferred_metrics_wire(&v).unwrap();
                fixture_match::validate_deferred_metrics_with_source(&v, &src, selection).unwrap();
                if name == "if_none" && command == "explain" {
                    // ADR 0068: this is now an accepted selected report, not
                    // an argument refusal. Both source-aware matchers above
                    // still check it, and its metrics must agree with check.
                    let check = serde_json::to_value(relca_core::report::check_with_selection(
                        &src,
                        Some("Left"),
                        None,
                    ))
                    .unwrap();
                    assert_eq!(v["verdict"], "satisfied");
                    assert_eq!(v["metrics"], check["metrics"]);
                    assert_eq!(
                        v["ledger"]["execution"]["selection"]["requested"]["configuration"],
                        "Left"
                    );
                }
                if deferred {
                    h::assert_all_quantities(&v["metrics"]);
                } else if name != "if_none" {
                    h::assert_control(&v["metrics"], name);
                }
                executed += 1;
            }
        }
    }
    assert_eq!(executed, 16);
}
