#[path = "support/deferred_second_inputs.rs"]
mod h;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};
macro_rules! case {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            let mut failures = Vec::new();
            let mut controls = std::collections::BTreeMap::new();
            for deferred in [false, true] {
                let src = h::source($name, deferred);
                let m = lower::lower_with_selection(
                    &syntax::parse(&src).unwrap(),
                    None,
                    h::scenario($name),
                )
                .unwrap();
                let e = engine::explore(&m);
                let r = reference::explore(&m);
                assert_eq!(r.stop, reference::StopReason::Complete);
                for (side, metrics) in [
                    ("engine", json!(e.metrics)),
                    ("reference", json!(r.outcome.metrics)),
                ] {
                    if deferred {
                        h::assert_preserved_structure_and_counts(&controls[side], &metrics);
                        if std::panic::catch_unwind(|| h::assert_all_quantities(&metrics)).is_err()
                        {
                            failures.push(side);
                        }
                    } else {
                        h::assert_control(&metrics, $name);
                        controls.insert(side, metrics);
                    }
                }
            }
            assert!(failures.is_empty(), "non-D leaks {failures:?}");
        }
    };
}
case!(non_d_immediate_cancel_work_and_cost, "immediate");
case!(non_d_horizon_work_and_cost, "horizon");
case!(non_d_unfinished_route_work_and_cost, "unfinished_route");
fn reject_undeclared(reason: &str) {
    let src = super_source(reason);
    let mut v = json!(report::check_with_selection(
        &src,
        Some("Left"),
        if reason == "undetermined" {
            Some("Day")
        } else {
            None
        }
    ));
    assert!(serde_json::from_value::<report::Report>(v.clone()).is_ok());
    v["quality"]["properties"][0]["cases"][0]["min"] = Value::Null;
    v["quality"]["properties"][0]["cases"][0]["max"] = Value::Null;
    assert!(
        serde_json::from_value::<report::Report>(v.clone()).is_err(),
        "undeclared {reason} null accepted"
    );
}
#[test]
fn report_truncated_case_null_requires_declared_defer() {
    reject_undeclared("truncated");
}
#[test]
fn report_undetermined_case_null_requires_declared_defer() {
    reject_undeclared("undetermined");
}
#[path = "support/deferred_review_inputs.rs"]
mod previous;
fn super_source(reason: &str) -> String {
    previous::source(reason, false)
}
#[test]
fn report_case_null_prior_reason_accepts_checked_defer() {
    for reason in ["truncated", "undetermined"] {
        let src = previous::source(reason, true);
        let v = json!(report::check_with_selection(
            &src,
            Some("Left"),
            previous::scenario(reason)
        ));
        assert!(v["quality"]["properties"][0]["cases"][0]["min"].is_null());
        assert!(serde_json::from_value::<report::Report>(v).is_ok());
    }
}

#[test]
fn report_prior_reason_null_requires_both_declaration_and_checked_marker() {
    for has_defer in [false, true] {
        let mut v = json!(report::check_with_selection(
            &previous::source("truncated", has_defer),
            Some("Left"),
            None
        ));
        v["quality"]["properties"][0]["cases"][0]["min"] = Value::Null;
        v["quality"]["properties"][0]["cases"][0]["max"] = Value::Null;
        let checked = v["cut"]["checked"].as_array_mut().unwrap();
        if has_defer {
            checked.retain(|x| x != "deferred_metrics");
        } else {
            checked.push(json!("deferred_metrics"));
        }
        assert!(
            serde_json::from_value::<report::Report>(v).is_err(),
            "only one prerequisite present: declaration={has_defer}"
        );
    }
}

#[test]
fn second_review_constructors_are_closed_and_lower() {
    let mut cases = Vec::new();
    for name in h::NAMES.into_iter().chain(["if_none"]) {
        for deferred in [false, true] {
            let src = h::source(name, deferred);
            assert!(
                lower::lower_with_selection(
                    &syntax::parse(&src).unwrap(),
                    if name == "if_none" {
                        Some("Left")
                    } else {
                        None
                    },
                    h::scenario(name)
                )
                .is_ok()
            );
            cases.push(json!({"name":format!("{name}_{deferred}"),"source":src,"scenario":h::scenario(name),"selection":if name=="if_none" {Some("Left")} else {None}}));
        }
    }
    assert_eq!(cases.len(), 8);
    assert_eq!(
        cases
            .iter()
            .map(|c| c["source"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        8
    );
}
