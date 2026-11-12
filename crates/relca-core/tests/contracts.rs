//! Shared compile/wire contracts, not tests of the future execution semantics.

use relca_core::{Verdict, ast, diag, engine, ir, lower, outcome, report, syntax};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;

#[test]
fn component_signatures_compose_without_implementation_dependencies() {
    let _: fn(&str) -> Result<ast::Model, Vec<diag::Diagnostic>> = syntax::parse;
    let _: fn(&ast::Model) -> Result<ir::Model, Vec<diag::Diagnostic>> = lower::lower;
    let _: fn(&ir::Model) -> outcome::Outcome = engine::explore;
    let _: fn(&str) -> report::Report = report::check;
    let _: fn(&[report::Property], &[diag::Diagnostic]) -> Verdict = report::fold_verdict;
    let _: fn(&str, &[&str], relca_core::compare::Analysis) -> relca_core::compare::CompareReport =
        relca_core::compare::compare;
}

#[test]
fn adr0002_example_round_trips_without_added_or_missing_keys() {
    let value = ledger_shape::extend_value(
        serde_json::json!({
            "verdict": "satisfied",
            "agency": null,
            "actors": [],
            "mode": "exhaustive",
            "unit": "min",
            "model": { "name": "Serial", "sha256": "…" },
            "cut": "undeclared",
            "cut_context": {
                "notices": [
                    "Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない",
                    "この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"
                ],
                "reentry": [], "horizon": []
            },
            "assumptions": [{ "field": "…", "value": "…", "affects": "…" }],
            "diagnostics": [{
                "code": "W-CUT-0", "severity": "warning", "message": "…", "span": null
            }],
            "bounds": { "cases": 1, "live_tasks": 2, "clock": 10, "truncated": false },
            "properties": [{ "id": "reachable end.done", "verdict": "satisfied" }],
            "terminals": [{
                "end": "done", "elapsed": { "min": 8, "max": 8 },
                "work": { "min": 8, "max": 8 }, "result_at": null
            }]
        }),
        &ledger_shape::Spec::prelower("exhaustive", false),
    );
    let report: report::Report = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(report).unwrap(), value);
}

#[test]
fn verdict_wire_values_and_exit_codes_remain_distinct() {
    for verdict in [
        Verdict::Satisfied,
        Verdict::Violated,
        Verdict::Unsupported,
        Verdict::Invalid,
        Verdict::Inconclusive,
        Verdict::Vacuous,
    ] {
        let json = serde_json::to_value(verdict).unwrap();
        assert_eq!(json.as_str(), Some(verdict.as_str()));
        assert_eq!(serde_json::from_value::<Verdict>(json).unwrap(), verdict);
        assert_eq!(verdict.exit_code() == 0, verdict == Verdict::Satisfied);
    }
    assert_eq!(Verdict::Vacuous.exit_code(), 5);
}

#[test]
fn non_model_source_is_never_reported_as_satisfied() {
    let report = report::check("not a Relca model");
    assert_ne!(report.verdict.exit_code(), 0);
    assert!(report.terminals.is_empty());
    assert!(report.properties.is_empty());
    assert!(report.diagnostics.iter().any(|d| d.code == diag::W_CUT_0));
    assert!(report.diagnostics.iter().any(|d| {
        d.severity == diag::Severity::Error
            && [diag::E_SYN_0, diag::U_SYN_0].contains(&d.code.as_str())
    }));
    let value = serde_json::to_value(report).unwrap();
    assert_eq!(value["cut"], "undeclared");
    assert!(value["model"]["name"].is_null());
    assert!(value["bounds"]["clock"].is_null());
    assert!(value["bounds"]["truncated"].is_null());
}

#[test]
fn unanalysed_bounds_round_trip_with_explicit_null_not_false() {
    let value = serde_json::json!({
        "cases": null, "live_tasks": null, "clock": null, "truncated": null
    });
    let bounds: report::Bounds = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(bounds.truncated, None);
    assert_eq!(serde_json::to_value(bounds).unwrap(), value);
}

#[test]
fn diagnostic_vocabulary_and_byte_span_wire_shape_are_fixed() {
    assert_eq!(
        [
            diag::E_SYN_0,
            diag::U_SYN_0,
            diag::U_SEM_0,
            diag::E_NAME_0,
            diag::E_TYPE_0,
            diag::E_CAP_0,
            diag::E_CANCEL_0,
            diag::W_CUT_0,
            diag::W_DEADLOCK_0,
        ],
        [
            "E-SYN-0",
            "U-SYN-0",
            "U-SEM-0",
            "E-NAME-0",
            "E-TYPE-0",
            "E-CAP-0",
            "E-CANCEL-0",
            "W-CUT-0",
            "W-DEADLOCK-0"
        ]
    );
    let diagnostic = diag::Diagnostic {
        code: diag::E_SYN_0.into(),
        severity: diag::Severity::Error,
        message: "invalid token".into(),
        span: Some(diag::Span { start: 3, end: 6 }),
    };
    let value = serde_json::to_value(&diagnostic).unwrap();
    assert_eq!(value["span"], serde_json::json!({"start": 3, "end": 6}));
    assert_eq!(
        serde_json::from_value::<diag::Diagnostic>(value).unwrap(),
        diagnostic
    );
}

#[test]
fn arrivals_diagnostic_registration_severity_and_byte_span() {
    assert_eq!(diag::CODES.iter().filter(|c| **c == "E-ARR-0").count(), 1);
    assert_eq!(diag::severity_of("E-ARR-0"), Some(diag::Severity::Error));
    for near_miss in ["E-ARR-1", "E-ARR", "E-ARR-00", "W-ARR-0"] {
        assert_eq!(diag::severity_of(near_miss), None);
    }
    let src = include_str!("../../../fixtures/arrivals-order.broken-descending/model.relca");
    let r = report::check(src);
    let d = r
        .diagnostics
        .iter()
        .find(|d| d.code == "E-ARR-0")
        .expect("registered arrivals error");
    let span = d.span.as_ref().unwrap();
    assert_eq!(&src[span.start..span.end], "0min");
    assert_eq!(d.severity, diag::Severity::Error);
    assert_eq!(r.verdict.exit_code(), 3);
}

#[test]
fn simulation_diagnostic_registration_severity_and_byte_span() {
    assert_eq!(diag::CODES.iter().filter(|c| **c == "E-SIM-0").count(), 1);
    assert_eq!(diag::severity_of("E-SIM-0"), Some(diag::Severity::Error));
    for near_miss in ["E-SIM-1", "E-SIM", "E-SIM-00", "W-SIM-0"] {
        assert_eq!(diag::severity_of(near_miss), None);
    }
    let src = include_str!("../../../fixtures/sim-range.broken-zero-replications/model.relca");
    let r = report::check(src);
    let d = r
        .diagnostics
        .iter()
        .find(|d| d.code == "E-SIM-0")
        .expect("simulation invalid");
    let span = d.span.as_ref().unwrap();
    assert_eq!(&src[span.start..span.end], "0");
    assert_eq!(
        span.start,
        src.find("replications 0").unwrap() + "replications ".len()
    );
    assert_eq!(r.verdict.exit_code(), 3);
}
