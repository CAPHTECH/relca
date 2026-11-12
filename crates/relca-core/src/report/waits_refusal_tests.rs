//! Execute the frozen refusal oracle through real post-lowering report paths.
use super::*;
use serde_json::{Value, json};

#[path = "../../tests/support/waits_refusal_private.rs"]
mod oracle;

fn observe(probe: &oracle::Probe, reference: bool) -> Value {
    use oracle::Route;
    let ast = syntax::parse(&probe.source).unwrap();
    let mut model = lower::lower(&ast).unwrap();
    match probe.route {
        Route::SimulationLoweredError | Route::AnalyzeLoweredError => {
            model.diagnostics.push(diag::Diagnostic {
                code: diag::E_CAP_0.into(),
                severity: diag::Severity::Error,
                message: "a7fix injected lowered error".into(),
                span: None,
            });
        }
        Route::SimulationAggregateOverflow => {
            model.simulation.as_mut().unwrap().replications = u64::MAX;
            model.bounds.cases = 2;
        }
        Route::AnalyzeActorMetadataError => {
            model
                .actors
                .iter_mut()
                .find(|a| a.name == "A")
                .unwrap()
                .metadata
                .control = Some("not-a-control".into());
        }
        Route::AnalyzeNoVerify => model.verify.clear(),
        _ => {}
    }
    let mut base = empty_report(&probe.source);
    base.model.name = Some(ast.name.value.clone());
    base.cut = Cut::Status(CutStatus::Unresolved);
    let mut executions = 0;
    let report = if matches!(
        probe.route,
        Route::SimulationLoweredError
            | Route::SimulationAggregateOverflow
            | Route::SimulationOutcomeWithoutMetrics
    ) {
        serde_json::to_value(simulate_model(base, &model, ast.span, |m| {
            executions += 1;
            let mut outcome = if reference {
                crate::reference::simulate(m).outcome
            } else {
                engine::simulate(m)
            };
            if probe.route == Route::SimulationOutcomeWithoutMetrics {
                outcome.metric_template = None;
                for replication in &mut outcome.replications {
                    replication.metrics = None;
                }
            }
            outcome
        }))
        .unwrap()
    } else {
        serde_json::to_value(analyze_model(base, &model, |m| {
            executions += 1;
            let mut outcome = if reference {
                crate::reference::explore(m).outcome
            } else {
                engine::explore(m)
            };
            if probe.route == Route::AnalyzeOutcomeWithoutMetrics {
                outcome.metrics = None;
            }
            outcome
        }))
        .unwrap()
    };
    json!({"report": report, "executions": executions})
}

#[test]
fn engine_waits_refusal_private_112() {
    oracle::run(|probe| observe(probe, false));
}

#[test]
fn reference_waits_refusal_private_112() {
    oracle::run(|probe| observe(probe, true));
}
