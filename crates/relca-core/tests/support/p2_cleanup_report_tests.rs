//! Connect in report.rs with cfg(test) path; no production API is widened.
use super::*;
use crate::p2_cleanup_shared::{contests, hand, input, q7};
use serde_json::json;
use std::cell::Cell;

fn finite(kind: &str) {
    let m = q7::model(kind);
    let admission = preflight::finite(&m);
    // Deliberately do not exhaustively match the enum in the test: adding a
    // variant must fail in production's match, not merely in this oracle.
    let selected = match kind {
        "f_ok" => admission.is_ok(),
        "f_actor" => matches!(admission, Err(preflight::FiniteFailure::Actor(_))),
        "f_arrivals" => matches!(admission, Err(preflight::FiniteFailure::StochasticArrivals)),
        "f_diagnostics" | "f_unsupported" => {
            matches!(admission, Err(preflight::FiniteFailure::Diagnostics))
        }
        "f_no_verify" => matches!(admission, Err(preflight::FiniteFailure::NoVerify)),
        _ => panic!("unknown finite case"),
    };
    assert!(selected, "wrong preflight arm for {kind}");
    let calls = Cell::new(0);
    // Raw helper boundary: outer driver/ledger wrappers are not this seam.
    let r = analyze_model(empty_report(input::SIMPLE), &m, |m| {
        calls.set(calls.get() + 1);
        crate::engine::explore(m)
    });
    let v = json!(r);
    let ok = kind == "f_ok";
    assert_eq!(calls.get(), usize::from(ok));
    if ok {
        assert_eq!(v["bounds"]["truncated"], false);
    }
    assert_eq!(
        v["ledger"]["execution"]["status"],
        if ok { "unavailable" } else { "not_run" }
    );
    assert_eq!(
        v["verdict"],
        if ok {
            "satisfied"
        } else if kind == "f_diagnostics" {
            "invalid"
        } else {
            "unsupported"
        }
    );
    if kind == "f_actor" {
        assert!(v["cut"]["name"].is_null());
        assert!(v["actors"].is_null());
    } else {
        assert_eq!(v["cut"]["name"], "C");
        assert!(v["actors"].is_array());
    }
    if !ok {
        let errors: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(
            errors[0]["code"],
            if kind == "f_diagnostics" {
                diag::E_TYPE_0
            } else {
                diag::U_SEM_0
            }
        );
        if kind == "f_diagnostics" || kind == "f_unsupported" {
            assert_eq!(errors[0]["span"], json!(m.process.span));
            assert_eq!(errors[0]["message"], m.diagnostics[0].message);
        }
        assert_eq!(
            v["quality"]["properties"][0]["expected_count"],
            json!({"lower":null,"upper":null,"basis":null,"reason":"aborted"})
        );
    }
    assert!(v["uncertainty"]["aleatory"].is_object());
}
macro_rules! f {
    ($name:ident,$case:literal) => {
        #[test]
        fn $name() {
            finite($case);
        }
    };
}
f!(p2_finite_actor, "f_actor");
f!(p2_finite_arrivals, "f_arrivals");
f!(p2_finite_diagnostics, "f_diagnostics");
f!(p2_finite_unsupported_diagnostics, "f_unsupported");
f!(p2_finite_no_verify, "f_no_verify");
f!(p2_finite_ok, "f_ok");
fn simulation(kind: &str) {
    let m = q7::model(kind);
    let scope = crate::sensitivity::statistics::SuiteMultiplicity::new(
        serde_json::from_value(json!("3/4")).unwrap(),
        &[1],
    )
    .unwrap();
    let scope = (kind == "s_scope").then_some(&scope);
    let admission = preflight::simulation(&m, scope);
    let selected = match kind {
        "s_ok" => admission.is_ok(),
        "s_missing" => matches!(admission, Err(preflight::SimulationFailure::MissingClause)),
        "s_diagnostics" | "s_unsupported" => {
            matches!(admission, Err(preflight::SimulationFailure::Diagnostics))
        }
        "s_arrivals" => matches!(
            admission,
            Err(preflight::SimulationFailure::StochasticArrivals)
        ),
        "s_range" => matches!(admission, Err(preflight::SimulationFailure::WireRange)),
        "s_scope" => matches!(admission, Err(preflight::SimulationFailure::Scope { .. })),
        _ => panic!("unknown simulation case"),
    };
    assert!(selected, "wrong preflight arm for {kind}");
    let calls = Cell::new(0);
    // Supplement 2 requires sealed refusal self-consistency as well as the
    // typed branch and zero-driver boundary.
    let r = simulation_driver(simulate_model_with_scope(
        empty_report(input::SIMPLE),
        &m,
        m.process.span,
        scope,
        |m| {
            calls.set(calls.get() + 1);
            crate::engine::simulate(m)
        },
    ));
    let v = json!(r);
    let ok = kind == "s_ok";
    assert_eq!(calls.get(), usize::from(ok));
    if ok {
        assert_eq!(v["bounds"]["truncated"], false);
    }
    assert_eq!(
        v["ledger"]["execution"]["status"],
        if ok { "ran" } else { "not_run" }
    );
    assert_eq!(
        v["verdict"],
        if ok {
            "satisfied"
        } else if matches!(kind, "s_missing" | "s_diagnostics" | "s_scope") {
            "invalid"
        } else {
            "unsupported"
        }
    );
    if !ok {
        assert_eq!(v["simulation"]["completed_replications"], 0);
        assert!(v["simulation"]["truncated_replications"].is_null());
        let errors: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(errors.len(), 1, "{errors:?}");
        let code = match kind {
            "s_missing" => diag::E_SIM_0,
            "s_diagnostics" => diag::E_TYPE_0,
            "s_scope" => diag::E_CMP_0,
            _ => diag::U_SEM_0,
        };
        assert_eq!(errors[0]["code"], code);
    }
    if !ok {
        assert_eq!(
            v["quality"]["properties"][0]["expected_count"],
            json!({"mean":null,"min":null,"max":null,"samples":0,"reason":"aborted"})
        );
    }
}
macro_rules! s {
    ($name:ident,$case:literal) => {
        #[test]
        fn $name() {
            simulation($case);
        }
    };
}
s!(p2_simulation_missing, "s_missing");
s!(p2_simulation_diagnostics, "s_diagnostics");
s!(p2_simulation_unsupported_diagnostics, "s_unsupported");
s!(p2_simulation_arrivals, "s_arrivals");
s!(p2_simulation_range, "s_range");
s!(p2_simulation_scope, "s_scope");
s!(p2_simulation_ok, "s_ok");

fn reducer(kind: &str) {
    let m = lower::lower(&syntax::parse(input::KNOWN_FAILURE).unwrap()).unwrap();
    contests::reducer(
        |m, o| {
            json!(
                crate::metrics::contests::summarize(m, o, false)
                    .unwrap()
                    .unwrap()
            )
        },
        &m,
        kind,
    );
}
#[test]
fn p2_reducer_response_cause() {
    reducer("unresolved");
}
#[test]
fn p2_reducer_filing_cause() {
    reducer("target");
}
#[test]
fn p2_reducer_known_row_preserved() {
    reducer("known");
}
#[test]
fn p2_missing_q6_record_reason() {
    let m = lower::lower_with_selection(&syntax::parse(input::WORK).unwrap(), Some("To"), None)
        .unwrap();
    let r = crate::report::check_with_selection(input::WORK, Some("To"), None);
    hand::missing_record(|a, z| {
        crate::diff::q6_differences(
            &m,
            if a { None } else { r.work_profiles.as_ref() },
            &m,
            if z { None } else { r.work_profiles.as_ref() },
        )
        .map(|r| json!(r))
    });
}
