//! I11/I14: capture all inputs, lower and gate every point, freeze the sampling
//! family, then execute the declared list exactly once in declaration order.
use super::{
    contract::Contract,
    input::{FailureKind, InputError, Mode},
    inputs::EffectiveInputs,
    source::{ManifestSnapshot, SourceSnapshot},
    statistics::SuiteMultiplicity,
    wire::*,
};
use crate::{
    diag::{self, Diagnostic},
    lower, report, syntax,
};
use std::path::Path;

#[cfg(test)]
#[path = "../../tests/support/p2_cleanup_run_tests.rs"]
mod p2_cleanup_run_tests;

pub fn run(path: &Path, mode: Mode) -> SensitivityReport {
    let mut r = SensitivityReport::new(mode);
    let manifest = match ManifestSnapshot::read(path, mode) {
        Ok(m) => m,
        Err(e) => {
            r.diagnostics.push(e.diagnostic());
            return r.seal(&[]);
        }
    };
    let m = manifest.manifest();
    r.scope.status = "available".into();
    r.scope.reason = None;
    r.scope.rationale = Some(m.scope.rationale.clone());
    r.scope.manifest_sha256 = Some(manifest.sha256().into());
    r.points = m.points.iter().map(PointResult::planned).collect();
    if let Err(e) = manifest.validate() {
        refuse_input(&mut r, &e);
        return r.seal(&[]);
    }
    let sources = match manifest.load_sources() {
        Ok(s) => s,
        Err(e) => {
            refuse_input(&mut r, &e);
            return r.seal(&m.targets);
        }
    };
    let mut models = Vec::with_capacity(sources.len());
    let mut baseline = None;
    for (i, s) in sources.iter().enumerate() {
        r.points[i].source_sha256 = Some(s.sha256().into());
        let model = match syntax::parse(s.source()).and_then(|a| {
            lower::lower_with_selection(
                &a,
                s.point().configuration.as_deref(),
                s.point().scenario.as_deref(),
            )
        }) {
            Ok(m) => m,
            Err(ds) => {
                let reason = if ds.iter().any(|d| d.code == diag::E_PROB_0) {
                    "conditional_distribution_invalid"
                } else {
                    "sensitivity_model_invalid"
                };
                refuse_model(&mut r, i, s, reason, None, ds);
                models.push(None);
                continue;
            }
        };
        r.points[i].effective = Some(EffectiveSelection {
            configuration: s.point().configuration.clone(),
            scenario: model.scenario.as_ref().map(|s| s.name.clone()),
            process: Some(model.process.name.clone()),
        });
        let checked = (|| {
            r.points[i].effective_inputs = Some(EffectiveInputs::from_model(&model)?);
            let contract = Contract::from_model(&model)?;
            contract.validate_targets(&m.targets)?;
            if mode == Mode::Exhaustive && model.arrival_distribution.is_some() {
                return Err(InputError::new(
                    FailureKind::FeatureUnsupported,
                    "finite arrival distributions are simulation only",
                ));
            }
            if mode == Mode::Simulation && model.completion_requested() {
                return Err(InputError::new(
                    FailureKind::FeatureUnsupported,
                    "structural completion is not available to simulation",
                ));
            }
            if mode == Mode::Simulation
                && model
                    .simulation
                    .as_ref()
                    .is_none_or(|s| Some(&s.confidence) != m.confidence.as_ref())
            {
                return Err(InputError::new(
                    FailureKind::ContractMismatch,
                    "simulation clause/confidence must match suite request",
                ));
            }
            if let Some(base) = &baseline {
                let diff: Vec<String> = Contract::differences(base, &contract);
                if !diff.is_empty() {
                    return Err(InputError::new(
                        FailureKind::ContractMismatch,
                        format!("changed contract locations: {}", diff.join(", ")),
                    ));
                }
            } else {
                baseline = Some(contract);
            }
            Ok(())
        })();
        if let Err(e) = checked {
            let e = e.at_point(&s.point().id);
            refuse_model(&mut r, i, s, e.reason(), Some(&model), vec![e.diagnostic()]);
        }
        models.push(Some(model));
    }
    // Even a valid first point cannot run before the last point's gate.
    if r.points.iter().any(|p| p.status == PointStatus::Refused) {
        return r.seal(&m.targets);
    }
    let scope = if mode == Mode::Simulation {
        let plan = models
            .iter()
            .map(|m| {
                let model = m.as_ref().expect("all lowered after gate");
                u64::try_from(model.probability_queries.len())
                    .ok()
                    .and_then(|n| n.checked_mul(model.bounds.cases))
                    .ok_or_else(|| {
                        InputError::new(
                            FailureKind::CapacityLimit,
                            "planned probability row count overflow",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .and_then(|rows| {
                SuiteMultiplicity::new(m.confidence.clone().expect("validated manifest"), &rows)
            });
        match plan {
            Ok(s) => Some(s),
            Err(e) => {
                refuse_input(&mut r, &e);
                return r.seal(&m.targets);
            }
        }
    } else {
        None
    };
    #[cfg(test)]
    for (i, model) in models.iter_mut().enumerate() {
        let model = model.as_mut().unwrap();
        p2_cleanup_run_tests::model_at_barrier(i, model);
        // The private hook replaces the whole lowered model. Record the actual
        // injected inputs with the ordinary producer, not the captured model's
        // stale bounds; production has no such mutation between these stages.
        r.points[i].effective_inputs = Some(EffectiveInputs::from_model(model).unwrap());
    }
    // Backend-specific static checks use the very same predicates as the child
    // producers. This barrier covers every model before the first driver call.
    for (i, (s, model)) in sources.iter().zip(&models).enumerate() {
        let model = model.as_ref().expect("all models passed the common gate");
        if let Err(failure) = report::sensitivity_preexecution(model, mode, scope.as_ref()) {
            refuse_model(
                &mut r,
                i,
                s,
                failure.reason,
                Some(model),
                failure.diagnostics,
            );
        }
    }
    if r.points.iter().any(|p| p.status == PointStatus::Refused) {
        return r.seal(&m.targets);
    }
    for (i, s) in sources.iter().enumerate() {
        let point = s.point();
        let produce = || match &scope {
            None => AnalysisReport::Exhaustive(Box::new(report::check_with_selection(
                s.source(),
                point.configuration.as_deref(),
                point.scenario.as_deref(),
            ))),
            Some(scope) => {
                AnalysisReport::Simulation(Box::new(report::simulate_with_multiplicity(
                    s.source(),
                    point.configuration.as_deref(),
                    point.scenario.as_deref(),
                    scope,
                )))
            }
        };
        #[cfg(test)]
        let child = p2_cleanup_run_tests::child_or_else(produce);
        #[cfg(not(test))]
        let child = produce();
        if !child.evaluated() {
            // The captured sources and every backend predicate already passed.
            // A not-run child is an admission contradiction, not point_result.
            // Preserve earlier real executions and never invoke later drivers.
            let reason = "sensitivity_feature_combination";
            r.points[i].status = PointStatus::Refused;
            r.points[i].reason = Some(reason.into());
            r.diagnostics
                .extend(child.diagnostics().iter().cloned().map(|mut d| {
                    d.message = format!("{reason}: point {:?}: {}", point.id, d.message);
                    d
                }));
            r.diagnostics.push(Diagnostic {
                code: diag::U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: format!(
                    "{reason}: point {:?}: context_mismatch: preexecution barrier admitted a not-run child",
                    point.id
                ),
                span: None,
            });
            r.points[i].report = Some(child);
            break;
        }
        r.points[i].status = PointStatus::Evaluated;
        r.points[i].reason = if child.verdict() == crate::Verdict::Satisfied {
            None
        } else {
            Some("point_result".into())
        };
        r.points[i].report = Some(child);
    }
    r.seal(&m.targets)
}

fn refuse_input(r: &mut SensitivityReport, e: &InputError) {
    if let Some(id) = &e.point
        && let Some(point) = r.points.iter_mut().find(|p| p.id == *id)
    {
        point.status = PointStatus::Refused;
        point.reason = Some(e.reason().into());
    }
    r.diagnostics.push(e.diagnostic());
}
fn refuse_model(
    r: &mut SensitivityReport,
    i: usize,
    s: &SourceSnapshot,
    reason: &str,
    model: Option<&crate::ir::Model>,
    ds: Vec<Diagnostic>,
) {
    let p = &mut r.points[i];
    p.status = PointStatus::Refused;
    p.reason = Some(reason.into());
    let child = report::sensitivity_preflight_failure(
        s.source(),
        s.point().configuration.as_deref(),
        s.point().scenario.as_deref(),
        model,
        ds.clone(),
    );
    p.report = Some(match r.mode {
        ResultMode::SensitivityExhaustive => AnalysisReport::Exhaustive(Box::new(child)),
        ResultMode::SensitivitySimulation => AnalysisReport::Simulation(Box::new(
            report::sensitivity_simulation_preflight_failure(child, model),
        )),
    });
    r.diagnostics.extend(ds.into_iter().map(|mut d| {
        d.message = format!("{reason}: point {:?}: {}", s.point().id, d.message);
        d
    }));
}

pub fn argument_failure(mode: Mode, detail: &str) -> SensitivityReport {
    let mut r = SensitivityReport::new(mode);
    r.diagnostics
        .push(InputError::new(FailureKind::ManifestInvalid, detail).diagnostic());
    r.seal(&[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn simulation_only_traffic_cannot_run_an_exhaustive_prefix() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp")
            .join(format!("q7-traffic-gate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let distribution = include_str!("../../tests/support/q4/arrivals.relca");
        let fixed=distribution.replace("arrivals distribution basis \"fixture assumption; not measured\" {\n    0.5: fixed [0min, 0min]\n    0.5: fixed [0min, 4min]\n  }","arrivals fixed [0min, 0min]");
        assert_ne!(distribution, fixed);
        let model = |s: &str| lower::lower(&syntax::parse(s).unwrap()).unwrap();
        assert!(
            Contract::from_model(&model(distribution))
                .unwrap()
                .differences(&Contract::from_model(&model(&fixed)).unwrap())
                .is_empty()
        );
        std::fs::write(dir.join("fixed.relca"), fixed).unwrap();
        std::fs::write(dir.join("distribution.relca"), distribution).unwrap();
        let manifest = json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"unsupported exhaustive traffic"},
            "points":[{"id":"fixed","model":"fixed.relca","configuration":null,"scenario":null},{"id":"stochastic","model":"distribution.relca","configuration":null,"scenario":null}],
            "targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":null});
        let path = dir.join("manifest.json");
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let (r, calls) = crate::evaluation_spy::observe(|| run(&path, Mode::Exhaustive));
        assert_eq!(calls[1], 0);
        assert_eq!(r.scope.evaluated, Some(0));
        assert_eq!(r.scope.refused, Some(1));
        assert_eq!(r.scope.not_run, Some(1));
        assert_eq!(r.verdict, crate::Verdict::Unsupported);
        r.validate().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn static_sampling_refusals_are_not_evaluated_points_or_a_successful_prefix() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp")
            .join(format!("q7-static-sample-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = include_str!("../../tests/data/sensitivity/zero.relca").replace(
            "verify probability",
            "verify terminating end\nverify probability",
        );
        std::fs::write(dir.join("model.relca"), source).unwrap();
        let manifest = json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"static completion refusal"},
            "points":[{"id":"p0","model":"model.relca","configuration":null,"scenario":null},{"id":"p1","model":"model.relca","configuration":null,"scenario":null}],
            "targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":"7/8"});
        let path = dir.join("manifest.json");
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let (r, calls) = crate::evaluation_spy::observe(|| run(&path, Mode::Simulation));
        assert_eq!(calls[4], 0);
        assert_eq!(r.scope.evaluated, Some(0));
        assert_eq!(r.scope.refused, Some(2));
        assert_eq!(r.verdict, crate::Verdict::Unsupported);
        for p in &r.points {
            let child = json!(p.report);
            assert_eq!(child["mode"], "simulation");
            assert!(
                child["cut"].is_object(),
                "a known lowered Cut must not become unresolved"
            );
            assert_eq!(child["uncertainty"]["aleatory"]["status"], "available");
            assert_eq!(child["uncertainty"]["model"]["status"], "available");
            assert_eq!(child["uncertainty"]["estimation"]["status"], "unavailable");
            assert_eq!(child["uncertainty"]["estimation"]["reason"], "no_analysis");
            assert_eq!(child["ledger"]["execution"]["status"], "not_run");
        }
        r.validate().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn real_exhaustive_and_sampling_suites_roundtrip_in_each_child_context() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sensitivity");
        for (file, mode) in [
            ("values.json", Mode::Exhaustive),
            ("endpoints.json", Mode::Exhaustive),
            ("zero-pair.json", Mode::Simulation),
            ("abort-pair.json", Mode::Simulation),
        ] {
            let r = run(&dir.join(file), mode);
            r.validate().unwrap_or_else(|e| panic!("{file}: {e}"));
            let v = json!(r);
            let decoded: SensitivityReport = serde_json::from_value(v.clone()).unwrap();
            assert_eq!(json!(decoded), v);
            assert_eq!(v["scope"]["planned"], v["scope"]["evaluated"]);
        }
    }
}
