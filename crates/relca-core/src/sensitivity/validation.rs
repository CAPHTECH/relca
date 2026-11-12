//! Self-consistency checks, not authentication of an analyst's source model.
use super::{input::Target, summary, wire::*};
use serde_json::{Value, json};
use std::collections::BTreeSet;
fn fail(detail: &str) -> String {
    format!("context_mismatch: sensitivity {detail}")
}
fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn validate(r: &SensitivityReport) -> Result<(), String> {
    if r.schema != "relca-sensitivity-result/v1"
        || r.scope.kind != "enumerated_models"
        || r.scope.continuous_region_checked
        || r.scope.robust_guarantee
    {
        return Err(fail("schema/scope/guarantee"));
    }
    let s = &r.scope;
    if s.manifest_sha256.as_deref().is_some_and(|s| !hash(s)) {
        return Err(fail("manifest hash"));
    }
    if s.status == "available" {
        let ids = r.points.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
        let count = |status| r.points.iter().filter(|p| p.status == status).count();
        let n = count(PointStatus::Evaluated);
        if s.reason.is_some()
            || s.rationale.as_ref().is_none()
            || s.manifest_sha256.is_none()
            || s.point_ids.as_ref() != Some(&ids)
            || s.planned != Some(ids.len())
            || s.evaluated != Some(n)
            || s.refused != Some(count(PointStatus::Refused))
            || s.not_run != Some(count(PointStatus::NotRun))
            || s.all_points_evaluated != Some(n == ids.len())
        {
            return Err(fail("planned counts/identities"));
        }
    } else if s.status != "unavailable"
        || s.reason.as_deref() != Some("manifest_unavailable")
        || s.planned.is_some()
        || s.point_ids.is_some()
        || s.evaluated.is_some()
        || s.refused.is_some()
        || s.not_run.is_some()
        || s.all_points_evaluated.is_some()
        || !r.points.is_empty()
        || !r.summaries.is_empty()
    {
        return Err(fail("unavailable counts"));
    }
    let mut ids = BTreeSet::new();
    for p in &r.points {
        // Invalid manifests can preserve duplicate/blank IDs, but may not run.
        if (!ids.insert(&p.id) || p.id.trim().is_empty()) && p.status == PointStatus::Evaluated {
            return Err(fail("point identity"));
        }
        if p.source_sha256.as_deref().is_some_and(|s| !hash(s)) {
            return Err(fail("source hash"));
        }
        match p.status {
            PointStatus::Evaluated
                if p.report.is_none()
                    || p.effective.is_none()
                    || p.effective_inputs.is_none()
                    || p.source_sha256.is_none() =>
            {
                return Err(fail("evaluated evidence"));
            }
            PointStatus::Refused if p.reason.is_none() => return Err(fail("refusal reason")),
            PointStatus::NotRun if p.reason.is_none() || p.report.is_some() => {
                return Err(fail("not_run evidence"));
            }
            _ => {}
        }
        if p.reason.as_deref() == Some("point_result") && p.status != PointStatus::Evaluated {
            return Err(fail("point_result requires an evaluated child"));
        }
        if let Some(e) = &p.effective_inputs {
            if e.rules != "sensitivity-inputs-p2-1" {
                return Err(fail("effective input version"));
            }
            for i in &e.initials {
                if i.entries.iter().any(|e| !e.probability.valid()) {
                    return Err(fail("initial mass"));
                }
            }
        }
        if let Some(child) = &p.report {
            if p.status == PointStatus::Evaluated
                && p.reason.as_deref()
                    != (child.verdict() != crate::Verdict::Satisfied).then_some("point_result")
            {
                return Err(fail("evaluated point reason"));
            }
            if p.status == PointStatus::Evaluated
                && (!child.evaluated()
                    || !matches!(
                        (r.mode, child),
                        (
                            ResultMode::SensitivityExhaustive,
                            AnalysisReport::Exhaustive(_)
                        ) | (
                            ResultMode::SensitivitySimulation,
                            AnalysisReport::Simulation(_)
                        )
                    ))
            {
                return Err(fail("evaluated child mode/execution"));
            }
            let h = json!(child);
            if json!(p.source_sha256) != h["model"]["sha256"] {
                return Err(fail("child source ownership"));
            }
            if json!(p.requested) != h["ledger"]["execution"]["selection"]["requested"] {
                return Err(fail("requested selection"));
            }
            if p.status != PointStatus::Evaluated
                && h["ledger"]["backends"]["builtin"]["status"] != "not_run"
            {
                return Err(fail("executed prefix"));
            }
            if let Some(e) = &p.effective {
                for key in ["configuration", "scenario", "process"] {
                    let v = &h["ledger"]["execution"]["selection"]["effective"][key];
                    let expected = &json!(e)[key];
                    if v["status"] == "known" && v["value"] != *expected {
                        return Err(fail("effective selection"));
                    }
                }
            }
            if h["ledger"]["actor_versions"]["status"] == "known"
                && let Some(e) = &p.effective_inputs
                && json!(e.bounds)
                    != json!({"cases":h["bounds"]["cases"],"clock":h["bounds"]["clock"],"live_tasks":h["bounds"]["live_tasks"]})
            {
                return Err(fail("effective bounds"));
            }
        }
    }
    let targets = r
        .summaries
        .iter()
        .map(|s| Target {
            kind: "probability_query".into(),
            id: s.id.clone(),
        })
        .collect::<Vec<_>>();
    let mut ids = BTreeSet::new();
    if targets
        .iter()
        .any(|t| t.id.trim().is_empty() || !ids.insert(&t.id))
        || r.summaries != summary::collect(r.mode.analysis(), &targets, &r.summary_inputs())
    {
        return Err(fail("summary ownership/value"));
    }
    if r.verdict != r.fold_verdict() {
        return Err(fail("root verdict"));
    }
    let host = r.host();
    r.ledger.validate(&host)?;
    // Validate every actual child in its own source context, never the first
    // point's hash/selection/seed as a proxy for the rest of the suite.
    for p in &r.points {
        if let Some(child) = &p.report {
            let h: Value = json!(child);
            match child {
                AnalysisReport::Exhaustive(c) => c.ledger.validate(&h)?,
                AnalysisReport::Simulation(c) => c.ledger.validate(&h)?,
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn planned_but_unexecuted_point_uses_the_existing_no_analysis_reason() {
        let point = PointResult::planned(&super::super::input::Point {
            id: "p".into(),
            model: "p.relca".into(),
            configuration: None,
            scenario: None,
        });
        assert_eq!(point.status, PointStatus::NotRun);
        assert_eq!(point.reason.as_deref(), Some("no_analysis"));
        assert!(point.report.is_none());
    }
    #[test]
    fn simulation_child_roundtrip_requires_its_own_ledger_and_three_classes() {
        let v = json!(crate::report::simulate(include_str!(
            "../../tests/data/sensitivity/zero.relca"
        )));
        let decoded: AnalysisReport = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(json!(decoded), v);
        let mut bad = v.clone();
        bad["uncertainty"].as_object_mut().unwrap().remove("model");
        assert!(serde_json::from_value::<AnalysisReport>(bad).is_err());
        let mut bad = v.clone();
        bad["unexpected"] = json!(0);
        assert!(serde_json::from_value::<AnalysisReport>(bad).is_err());
        let mut bad = v;
        bad["ledger"]["execution"]["seed"]["value"] = json!(1);
        assert!(serde_json::from_value::<AnalysisReport>(bad).is_err());
    }
    #[test]
    fn unavailable_scope_is_not_a_zero_point_success_and_wire_is_closed() {
        let mut r = SensitivityReport::new(super::super::input::Mode::Exhaustive);
        r.diagnostics.push(
            super::super::input::InputError::new(
                super::super::input::FailureKind::ManifestInvalid,
                "bad manifest",
            )
            .diagnostic(),
        );
        let r = r.seal(&[]);
        r.validate().unwrap();
        let v = json!(r);
        assert_eq!(v["scope"]["planned"], Value::Null);
        assert_eq!(v["verdict"], "invalid");
        let decoded: SensitivityReport = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(json!(decoded), v);
        let mut bad = v.clone();
        bad["scope"]["planned"] = json!(0);
        assert!(serde_json::from_value::<SensitivityReport>(bad).is_err());
        for key in ["uncertainty", "summaries", "scope"] {
            let mut bad = v.clone();
            bad.as_object_mut().unwrap().remove(key);
            assert!(serde_json::from_value::<SensitivityReport>(bad).is_err());
        }
        let mut bad = v;
        bad["scope"]["extra"] = json!(true);
        assert!(serde_json::from_value::<SensitivityReport>(bad).is_err());
    }
    #[test]
    fn all_child_sources_are_linked_and_cross_point_ownership_is_rejected() {
        let mut r = SensitivityReport::new(super::super::input::Mode::Exhaustive);
        r.scope.status = "available".into();
        r.scope.reason = None;
        r.scope.rationale = Some("two hand inputs".into());
        r.scope.manifest_sha256 = Some("a".repeat(64));
        for (id, src) in [
            (
                "base",
                include_str!("../../tests/data/sensitivity/base.relca"),
            ),
            (
                "high",
                include_str!("../../tests/data/sensitivity/high.relca"),
            ),
        ] {
            let child = crate::report::check(src);
            let model = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
            let mut p = PointResult::planned(&super::super::input::Point {
                id: id.into(),
                model: format!("{id}.relca"),
                configuration: None,
                scenario: None,
            });
            p.status = PointStatus::Evaluated;
            p.reason = None;
            p.source_sha256 = child.model.sha256.clone();
            p.effective = Some(EffectiveSelection {
                configuration: None,
                scenario: None,
                process: Some(model.process.name.clone()),
            });
            p.effective_inputs =
                Some(super::super::inputs::EffectiveInputs::from_model(&model).unwrap());
            p.report = Some(AnalysisReport::Exhaustive(Box::new(child)));
            r.points.push(p);
        }
        let r = r.seal(&[Target {
            kind: "probability_query".into(),
            id: "probability reached #1".into(),
        }]);
        r.validate().unwrap();
        let mut wrong_mode = r.clone();
        wrong_mode.mode = ResultMode::SensitivitySimulation;
        wrong_mode.ledger = crate::ledger::Ledger::aggregate(wrong_mode.mode.as_str());
        let wrong_mode = wrong_mode.seal(&[Target {
            kind: "probability_query".into(),
            id: "probability reached #1".into(),
        }]);
        assert!(
            wrong_mode.validate().is_err(),
            "exhaustive children cannot become sampling evidence by relabelling the root"
        );
        let v = json!(r);
        let verdict = v["ledger"]["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["target"] == "/verdict")
            .unwrap();
        assert_eq!(
            verdict["source_ledgers"],
            json!(["/points/0/report/ledger", "/points/1/report/ledger"])
        );
        assert!(serde_json::from_value::<SensitivityReport>(v.clone()).is_ok());
        let mut bad = v.clone();
        bad["points"][0]["report"]["unknown_child_field"] = json!(0);
        assert!(
            serde_json::from_value::<SensitivityReport>(bad).is_err(),
            "sensitivity children have closed report fields"
        );
        let mut bad = v.clone();
        bad["points"][1]["source_sha256"] = v["points"][0]["source_sha256"].clone();
        assert!(serde_json::from_value::<SensitivityReport>(bad).is_err());
        let mut bad = v.clone();
        bad["summaries"][0]["value"]["max"] = json!("1");
        assert!(serde_json::from_value::<SensitivityReport>(bad).is_err());
        let mut bad = v;
        bad["uncertainty"]["model"]["items"] = json!(["/points/0/report/uncertainty/model"]);
        assert!(serde_json::from_value::<SensitivityReport>(bad).is_err());
    }
}
