//! Internal run seam, connected through two cfg(test) hooks.
//! Inject AFTER common admission, and replace the actual second child only.
use super::*;
use crate::p2_cleanup_shared::{hand, input, q7};
use serde_json::json;
use std::cell::{Cell, RefCell};
thread_local! {static CASE:RefCell<Option<&'static str>>=const {RefCell::new(None)};static CALLS:Cell<usize>=const {Cell::new(0)};}
pub(super) fn model_at_barrier(i: usize, m: &mut crate::ir::Model) {
    CASE.with(|c| {
        if let Some(kind) = *c.borrow()
            && kind != "post"
            && i == 1
        {
            *m = q7::model(kind);
        }
    });
}
pub(super) fn child_or_else(produce: impl FnOnce() -> AnalysisReport) -> AnalysisReport {
    if CASE.with(|c| c.borrow().is_none()) {
        return produce();
    }
    let n = CALLS.with(|c| {
        let n = c.get() + 1;
        c.set(n);
        n
    });
    if CASE.with(|c| *c.borrow() == Some("post")) && n == 2 {
        return AnalysisReport::Exhaustive(Box::new(report::sensitivity_preflight_failure(
            input::SIMPLE,
            None,
            None,
            None,
            vec![Diagnostic {
                code: diag::U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: "injected postbarrier refusal".into(),
                span: None,
            }],
        )));
    }
    produce()
}
fn run_case(kind: &'static str, mode: Mode) -> (SensitivityReport, usize) {
    let p = input::suite(3);
    if mode == Mode::Simulation {
        let mut v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        v["confidence"] = json!("1/2");
        std::fs::write(&p, v.to_string()).unwrap();
    }
    CASE.with(|c| *c.borrow_mut() = Some(kind));
    CALLS.with(|c| c.set(0));
    let (r, drivers) = crate::evaluation_spy::observe(|| run(&p, mode));
    let calls = CALLS.with(Cell::get);
    CASE.with(|c| *c.borrow_mut() = None);
    if kind == "post" {
        assert_eq!(drivers[1], 1, "only the first exhaustive backend runs");
    } else {
        assert_eq!(
            drivers[1] + drivers[4],
            0,
            "all preflight failures precede any backend"
        );
    }
    (r, calls)
}
fn barrier(kind: &'static str, reason: &str, code: &str, mode: Mode) {
    let (r, calls) = run_case(kind, mode);
    let v = json!(r);
    assert_eq!(calls, 0);
    assert_eq!(v["scope"]["evaluated"], 0);
    assert_eq!(v["scope"]["refused"], 1);
    assert_eq!(v["scope"]["not_run"], 2);
    assert_eq!(v["points"][1]["reason"], reason);
    let ds = v["points"][1]["report"]["diagnostics"].as_array().unwrap();
    assert!(ds.iter().any(|d| d["code"] == code));
    assert_eq!(
        v["points"][1]["report"]["ledger"]["execution"]["status"],
        "not_run"
    );
    assert_eq!(v["summaries"][0]["reason"], "incomplete_points");
    serde_json::from_value::<SensitivityReport>(v).unwrap();
}
#[test]
fn p2_barrier_wire_range_capacity() {
    barrier(
        "s_range",
        "sensitivity_capacity_limit",
        diag::U_SEM_0,
        Mode::Simulation,
    );
}
#[test]
fn p2_barrier_typed_scope() {
    barrier(
        "s_scope",
        "sensitivity_contract_mismatch",
        diag::E_CMP_0,
        Mode::Simulation,
    );
}
#[test]
fn p2_barrier_typed_actor() {
    barrier(
        "f_actor",
        "sensitivity_feature_combination",
        diag::U_SEM_0,
        Mode::Exhaustive,
    );
}
#[test]
fn p2_barrier_typed_missing() {
    barrier(
        "s_missing",
        "sensitivity_model_invalid",
        diag::E_SIM_0,
        Mode::Simulation,
    );
}
#[test]
fn p2_barrier_typed_diagnostics() {
    for mode in [Mode::Exhaustive, Mode::Simulation] {
        barrier(
            if mode == Mode::Exhaustive {
                "f_diagnostics"
            } else {
                "s_diagnostics"
            },
            "sensitivity_model_invalid",
            diag::E_TYPE_0,
            mode,
        );
    }
    for (kind, mode) in [
        ("f_diagnostics_renamed", Mode::Exhaustive),
        ("s_diagnostics_renamed", Mode::Simulation),
    ] {
        barrier(kind, "sensitivity_model_invalid", diag::E_TYPE_0, mode);
    }
}
#[test]
fn p2_barrier_typed_unsupported_diagnostics() {
    for mode in [Mode::Exhaustive, Mode::Simulation] {
        barrier(
            if mode == Mode::Exhaustive {
                "f_unsupported"
            } else {
                "s_unsupported"
            },
            "sensitivity_feature_combination",
            diag::U_SEM_0,
            mode,
        );
    }
}
#[test]
fn p2_barrier_typed_arrivals() {
    for mode in [Mode::Exhaustive, Mode::Simulation] {
        barrier(
            if mode == Mode::Exhaustive {
                "f_arrivals"
            } else {
                "s_arrivals"
            },
            "sensitivity_feature_combination",
            diag::U_SEM_0,
            mode,
        );
    }
}
#[test]
fn p2_barrier_typed_no_verify() {
    barrier(
        "f_no_verify",
        "sensitivity_feature_combination",
        diag::U_SEM_0,
        Mode::Exhaustive,
    );
}
#[test]
fn p2_postbarrier_not_run_stops_following_driver() {
    let (r, calls) = run_case("post", Mode::Exhaustive);
    let v = json!(r);
    hand::runner(&v, calls);
    serde_json::from_value::<SensitivityReport>(v).unwrap();
}
#[test]
fn p2_semantic_abort_is_ran_point_result_not_barrier_refusal() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/sensitivity/abort-pair.json");
    let r = run(&p, Mode::Simulation);
    let v = json!(r);
    for p in v["points"].as_array().unwrap() {
        assert_eq!(p["status"], "evaluated");
        assert_eq!(p["reason"], "point_result");
        assert_eq!(p["report"]["ledger"]["execution"]["status"], "ran");
    }
    serde_json::from_value::<SensitivityReport>(v).unwrap();
}
