//! Closed combinations for the additional ADR 0056 wire, without weakening
//! the older no-receipt reports or treating a missing nullable key as null.
use super::*;

const CHECKED: [&str; 3] = ["retry_bounded", "receipt_channel", "receipt_not_success"];
const UNCHECKED: [&str; 5] = [
    "asynchronous_external_task",
    "general_status_query",
    "correlated_response_loss",
    "unbounded_retry",
    "fairness",
];
const SCOPE: &str = "declared sequential channels and observed transitions under selected bounds; exploration coverage is not business completion";

/// Response coverage is local: sampling must not widen the closed obligation
/// coverage vocabulary used by exhaustive-only tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseCoverage {
    Complete,
    Truncated,
    Aborted,
    RangeStopped,
    Sampled,
}
impl From<crate::obligation::Coverage> for ResponseCoverage {
    fn from(c: crate::obligation::Coverage) -> Self {
        use crate::obligation::Coverage as C;
        match c {
            C::Complete => Self::Complete,
            C::Truncated => Self::Truncated,
            C::Aborted => Self::Aborted,
            C::RangeStopped => Self::RangeStopped,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseAnalysis {
    pub rules: String,
    pub coverage: ResponseCoverage,
    pub business_horizon: bool,
    pub scope: String,
    pub checked: Vec<String>,
    pub unchecked: Vec<String>,
}

pub(super) fn analysis(m: &ir::Model, coverage: crate::obligation::Coverage) -> ResponseAnalysis {
    let recovery = m.recovery().is_some();
    ResponseAnalysis {
        rules: if recovery {
            "response-p3a-recovery-1"
        } else {
            "response-p2-1"
        }
        .into(),
        coverage: coverage.into(),
        business_horizon: m.horizon.is_some(),
        scope: SCOPE.into(),
        checked: CHECKED
            .into_iter()
            .chain(
                recovery
                    .then_some([
                        "exact_g_query",
                        "recovery_evidence_matching",
                        "recovery_incomplete_case",
                    ])
                    .into_iter()
                    .flatten(),
            )
            .map(String::from)
            .collect(),
        unchecked: UNCHECKED
            .into_iter()
            .chain(recovery.then_some("non_idempotent_recovery"))
            .map(String::from)
            .collect(),
    }
}

pub(super) fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}

pub(super) fn unavailable_transport(m: &ir::Model) -> ResponseAnalysis {
    ResponseAnalysis {
        rules: "response-p2-1".into(),
        coverage: ResponseCoverage::Aborted,
        business_horizon: m.horizon.is_some(),
        scope: SCOPE.into(),
        checked: CHECKED.map(String::from).to_vec(),
        unchecked: UNCHECKED.map(String::from).to_vec(),
    }
}

pub(super) fn sampled_analysis(m: &ir::Model) -> ResponseAnalysis {
    let mut a = analysis(m, crate::obligation::Coverage::Complete);
    a.coverage = ResponseCoverage::Sampled;
    a.checked = a
        .checked
        .into_iter()
        .map(|s| format!("sampled_{}", s.strip_prefix("exact_").unwrap_or(&s)))
        .collect();
    a.unchecked.push("unobserved_paths".into());
    a
}
pub(super) enum Host<'a> {
    Check(&'a Report),
    Simulation(&'a SimulationReport),
}
impl<'a> From<&'a Report> for Host<'a> {
    fn from(r: &'a Report) -> Self {
        Self::Check(r)
    }
}
impl<'a> From<&'a SimulationReport> for Host<'a> {
    fn from(r: &'a SimulationReport) -> Self {
        Self::Simulation(r)
    }
}
pub(super) fn validate<'a>(host: impl Into<Host<'a>>) -> Result<(), &'static str> {
    match host.into() {
        Host::Check(report) => validate_check(report),
        Host::Simulation(report) => {
            let recovery = report.response_recovery.is_some();
            if recovery != report.response_analysis.is_some() {
                return Err("sampled recovery analysis missing or unpaired");
            }
            if let Some(a) = &report.response_analysis {
                let checked: Vec<_> = CHECKED
                    .into_iter()
                    .chain([
                        "exact_g_query",
                        "recovery_evidence_matching",
                        "recovery_incomplete_case",
                    ])
                    .map(|s| format!("sampled_{}", s.strip_prefix("exact_").unwrap_or(s)))
                    .collect();
                let unchecked: Vec<_> = UNCHECKED
                    .into_iter()
                    .chain(["non_idempotent_recovery", "unobserved_paths"])
                    .map(String::from)
                    .collect();
                if a.rules != "response-p3a-recovery-1"
                    || a.coverage != ResponseCoverage::Sampled
                    || a.scope != SCOPE
                    || a.checked != checked
                    || a.unchecked != unchecked
                {
                    return Err("invalid sampled response scope or checked vocabulary");
                }
                if !a.business_horizon
                    && report
                        .case_outcomes
                        .iter()
                        .any(|c| c["cut_off_count"].as_u64().unwrap_or(0) > 0)
                {
                    return Err("sampled response cutoff without a horizon");
                }
            }
            Ok(())
        }
    }
}
fn validate_check(report: &Report) -> Result<(), &'static str> {
    if report.response_recovery.is_some()
        && (report.response_receipts.is_none() || report.response_analysis.is_none())
    {
        return Err("recovery requires receipts and response analysis");
    }
    let Some(receipts) = &report.response_receipts else {
        if report.response_analysis.is_some() {
            return Err("response analysis without a channel");
        }
        if report.case_outcomes.as_ref().is_some_and(|cs| {
            cs.iter().any(|c| {
                c.terminals
                    .iter()
                    .any(|t| matches!(t.kind, CaseTerminalKind::ReceiptUnknown))
            })
        }) {
            return Err("receipt_unknown without response channel");
        }
        return Ok(());
    };
    let analysis = report
        .response_analysis
        .as_ref()
        .ok_or("response analysis missing")?;
    let recovery = report.response_recovery.is_some();
    let checked: Vec<_> = CHECKED
        .into_iter()
        .chain(
            recovery
                .then_some([
                    "exact_g_query",
                    "recovery_evidence_matching",
                    "recovery_incomplete_case",
                ])
                .into_iter()
                .flatten(),
        )
        .map(String::from)
        .collect();
    let unchecked: Vec<_> = UNCHECKED
        .into_iter()
        .chain(recovery.then_some("non_idempotent_recovery"))
        .map(String::from)
        .collect();
    if analysis.rules
        != if recovery {
            "response-p3a-recovery-1"
        } else {
            "response-p2-1"
        }
        || analysis.scope != SCOPE
        || analysis.checked != checked
        || analysis.unchecked != unchecked
    {
        return Err("invalid response scope or checked vocabulary");
    }
    if analysis.coverage
        != crate::obligation::Coverage::of(
            report
                .diagnostics
                .iter()
                .any(|d| d.severity == diag::Severity::Error),
            report.bounds.truncated == Some(true),
            !report.state_range_violations.is_empty(),
        )
        .into()
    {
        return Err("response exploration coverage mismatch");
    }
    let cases = report
        .case_outcomes
        .as_ref()
        .ok_or("response channel requires case outcomes")?;
    if !analysis.business_horizon
        && cases.iter().any(|c| {
            c.terminals
                .iter()
                .any(|t| matches!(t.kind, CaseTerminalKind::CutOff))
        })
    {
        return Err("response business cutoff without a declared horizon");
    }
    let mut identities = std::collections::BTreeSet::new();
    for r in receipts {
        if r.case_id == 0
            || !(1..=32).contains(&r.attempt)
            || r.command.is_empty()
            || r.capability.is_empty()
            || r.at.min > r.at.max
            || !cases.iter().any(|c| c.case_id == r.case_id)
            || (r.receipt == crate::ast::Receipt::Unknown) != r.observed_result.is_none()
            || r.receipt == crate::ast::Receipt::Unknown && r.resolved_effect
        {
            return Err("invalid response receipt");
        }
        if !identities.insert((
            r.case_id,
            &r.command,
            r.attempt,
            r.receipt == crate::ast::Receipt::Unknown,
            &r.observed_result,
            r.resolved_effect,
        )) {
            return Err("duplicate response observation");
        }
    }
    for c in cases {
        for t in &c.terminals {
            if matches!(t.kind, CaseTerminalKind::ReceiptUnknown) {
                if t.end.is_some()
                    || t.completed_at.is_some()
                    || t.elapsed.is_some()
                    || t.work.is_some()
                    || t.result_at.is_some()
                    || [t.observed_until, t.observed_elapsed, t.observed_work]
                        .iter()
                        .any(|r| r.is_none_or(|r| r.min > r.max))
                {
                    return Err("invalid receipt_unknown outcome");
                }
            } else if t.observed_until.is_some()
                || t.observed_elapsed.is_some()
                || t.observed_work.is_some()
            {
                return Err("receipt observation on a business terminal");
            }
        }
    }
    let automatic: Vec<_> = report
        .properties
        .iter()
        .filter(|p| p.id == "receipt_resolution")
        .collect();
    if automatic.len() != 1 {
        return Err("missing or duplicate receipt_resolution");
    }
    let unresolved = cases.iter().any(|c| {
        c.terminals.iter().any(|t| {
            matches!(
                t.kind,
                CaseTerminalKind::ReceiptUnknown | CaseTerminalKind::CutOff
            )
        })
    }) || analysis.coverage != ResponseCoverage::Complete;
    if unresolved && automatic[0].verdict != Verdict::Inconclusive {
        return Err("unresolved receipt promoted to success");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round2_sampled_coverage_keeps_cutoff_in_the_verdict() {
        for id in ["T04", "T05"] {
            let source = crate::r2_inputs::source(id);
            let r = crate::report::simulate_with_selection(
                &source,
                None,
                (id == "T05").then_some("Day"),
            );
            assert_eq!(
                r.verdict,
                Verdict::Inconclusive,
                "{id}: {:?}",
                r.diagnostics
            );
            assert_eq!(
                r.response_analysis.as_ref().unwrap().coverage,
                ResponseCoverage::Sampled
            );
            assert_eq!(r.bounds.truncated, Some(id == "T04"));
        }
    }
    #[test]
    fn round2_recovery_coexistence_is_a_local_check_not_a_ledger_accident() {
        let mut r = crate::report::check(&crate::r2_inputs::source("R01"));
        r.response_receipts = None;
        r.response_analysis = None;
        assert_eq!(
            validate(&r),
            Err("recovery requires receipts and response analysis")
        );
    }
    #[test]
    fn r2_unconnected_report_transport_fails_closed() {
        let checked = crate::report::check_using(&crate::r2_inputs::source("R04"), |m| {
            let mut o = crate::engine::explore(m);
            o.response_recovery = None;
            o
        });
        assert_eq!(checked.verdict, Verdict::Unsupported);
        assert!(checked.diagnostics.iter().any(|d| d.message
            == "recovery_feature_combination: recovery report transport is not connected"));
        serde_json::from_value::<crate::report::Report>(serde_json::to_value(checked).unwrap())
            .expect("closed refusal report");
        let sampled = super::super::simulate_using(&crate::r2_inputs::source("R01"), None, |m| {
            let mut o = crate::engine::simulate(m);
            o.response_recovery = None;
            o
        });
        assert_eq!(sampled.verdict, Verdict::Unsupported);
    }
    #[test]
    fn typed_receipt_wire_rejects_missing_null_and_promoted_completion() {
        let s = r#"model M version "0.2" { type V=enum {ok}
            actor A { capacity 1; capability c()->V { duration fixed 1min; result nondet {ok} } }
            process P {r=A.c() response_loss sequence [unknown]; end done}
            bounds { cases 1; live_tasks 1; clock 3min } verify safety no_resource_overcommit }"#;
        let v = serde_json::to_value(check(s)).unwrap();
        assert_eq!(
            v["response_analysis"]["checked"],
            serde_json::json!(CHECKED)
        );
        assert_eq!(v["response_analysis"]["coverage"], "complete");
        let _: Report = serde_json::from_value(v.clone()).unwrap();
        for field in [
            "end",
            "completed_at",
            "elapsed",
            "work",
            "result_at",
            "observed_until",
            "observed_elapsed",
            "observed_work",
        ] {
            let mut bad = v.clone();
            bad["case_outcomes"][0]["terminals"][0]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(serde_json::from_value::<Report>(bad).is_err(), "{field}");
        }
        for field in [
            "receipt",
            "observed_result",
            "at",
            "resolved_effect",
            "case_id",
            "command",
            "attempt",
            "capability",
        ] {
            let mut bad = v.clone();
            bad["response_receipts"][0]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(serde_json::from_value::<Report>(bad).is_err(), "{field}");
        }
        let mut bad = v;
        bad["case_outcomes"][0]["terminals"][0]["completed_at"] =
            serde_json::json!({"min":1,"max":1});
        assert!(serde_json::from_value::<Report>(bad).is_err());
    }

    #[test]
    fn business_cutoff_does_not_certify_pending_response_resolution() {
        let s = r#"model M version "0.2" {type V=enum {ok}
            actor A {capacity 1;capability run()->V {duration fixed 2min;result nondet {ok}}}
            process P {r=A.run() response_loss none;end done}
            scenario Day {arrivals fixed [0min];horizon 1min;bounds {cases 1;live_tasks 1;clock 5min}}
            verify safety no_resource_overcommit }"#;
        let report = super::super::check_with_selection(s, None, Some("Day"));
        assert_eq!(
            report.verdict,
            Verdict::Inconclusive,
            "{:?}",
            report.diagnostics
        );
        assert_eq!(report.bounds.truncated, Some(false));
        assert_eq!(
            report.case_outcomes.as_ref().unwrap()[0].terminals[0].kind,
            CaseTerminalKind::CutOff
        );
        assert_eq!(
            report
                .properties
                .iter()
                .find(|p| p.id == "receipt_resolution")
                .unwrap()
                .verdict,
            Verdict::Inconclusive
        );
        let control = super::super::check_with_selection(
            &s.replace("response_loss none", ""),
            None,
            Some("Day"),
        );
        let safety = |r: &Report| {
            r.properties
                .iter()
                .find(|p| p.id == "safety no_resource_overcommit")
                .unwrap()
                .verdict
        };
        assert_eq!(
            safety(&control),
            Verdict::Inconclusive,
            "a horizon leaves the future safety suffix unchecked"
        );
        assert_eq!(
            safety(&report),
            safety(&control),
            "receipt bookkeeping must preserve the existing Core horizon verdict"
        );
        assert!(report.response_analysis.as_ref().unwrap().business_horizon);
        let _: Report = serde_json::from_value(serde_json::to_value(report).unwrap()).unwrap();
    }
}
