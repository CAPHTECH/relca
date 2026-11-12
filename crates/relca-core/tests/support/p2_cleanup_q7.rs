//! Private IR cases. Only the selected field changes after successful lowering.
#![allow(dead_code)]
use super::core::{diag, ir, lower, probability::Rational, syntax};
use super::input;
pub fn model(kind: &str) -> ir::Model {
    let source = if kind == "s_arrivals" {
        input::KNOWN_FAILURE.to_owned()
    } else {
        input::edit(
            input::SIMPLE,
            "  type R",
            "  observed case { good: bool = true }\n  type R",
        )
    };
    let condition = if kind == "s_arrivals" {
        "ready == false"
    } else {
        "good == true"
    };
    let at = source.rfind('}').unwrap();
    let source = format!(
        "{}\nproperty case.Good = reached({condition})\n}}",
        &source[..at]
    );
    let mut m = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
    match kind {
        "f_actor" => m.actors[0].metadata.control = Some("not_a_control".into()),
        "f_arrivals" | "s_arrivals" => {
            m.arrival_distribution = Some(ir::ArrivalDistribution {
                basis: "assumed".into(),
                entries: vec![(vec![0], Rational::one())],
                span: m.process.span,
            })
        }
        "f_diagnostics" | "s_diagnostics" | "f_diagnostics_renamed" | "s_diagnostics_renamed" => {
            m.diagnostics = vec![diag::Diagnostic {
                code: diag::E_TYPE_0.into(),
                severity: diag::Severity::Error,
                message: if kind.ends_with("renamed") {
                    "U-prefixed-looking text changed without changing the typed failure"
                } else {
                    "arbitrary renamed detail"
                }
                .into(),
                span: Some(m.process.span),
            }]
        }
        "f_unsupported" | "s_unsupported" => {
            m.diagnostics = vec![diag::Diagnostic {
                code: diag::U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: "not the reason vocabulary".into(),
                span: Some(m.process.span),
            }]
        }
        "f_no_verify" => m.verify.clear(),
        "s_missing" => m.simulation = None,
        "s_range" => {
            m.bounds.cases = 2;
            m.simulation.as_mut().unwrap().replications = u64::MAX;
        }
        "s_scope" => m.bounds.cases = 4,
        "f_ok" | "s_ok" => (),
        _ => panic!("unknown Q7 case {kind}"),
    }
    m
}
