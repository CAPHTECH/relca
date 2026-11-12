//! Closed sensitivity result types. No point is dropped after a refusal.
use super::{
    input::{Mode, Point, Target},
    inputs::EffectiveInputs,
    summary::{self, Summary},
};
use crate::{Verdict, diag::Diagnostic, ledger::Ledger, report, uncertainty::Uncertainty};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnalysisReport {
    Simulation(Box<report::SimulationReport>),
    Exhaustive(#[serde(deserialize_with = "strict_exhaustive")] Box<report::Report>),
}
fn strict_exhaustive<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Box<report::Report>, D::Error> {
    report::Report::deserialize_strict(d).map(Box::new)
}
impl AnalysisReport {
    pub(super) fn evaluated(&self) -> bool {
        let ledger = match self {
            Self::Exhaustive(r) => &r.ledger,
            Self::Simulation(r) => &r.ledger,
        };
        ledger.execution["status"] == "ran"
    }
    pub fn verdict(&self) -> Verdict {
        match self {
            Self::Exhaustive(r) => r.verdict,
            Self::Simulation(r) => r.verdict,
        }
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Exhaustive(r) => &r.diagnostics,
            Self::Simulation(r) => &r.diagnostics,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointStatus {
    Evaluated,
    Refused,
    NotRun,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    #[serde(deserialize_with = "nullable")]
    pub configuration: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub scenario: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveSelection {
    #[serde(deserialize_with = "nullable")]
    pub configuration: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub scenario: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub process: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointResult {
    pub id: String,
    pub model: String,
    #[serde(deserialize_with = "nullable")]
    pub source_sha256: Option<String>,
    pub requested: Selection,
    #[serde(deserialize_with = "nullable")]
    pub effective: Option<EffectiveSelection>,
    #[serde(deserialize_with = "nullable")]
    pub effective_inputs: Option<EffectiveInputs>,
    pub status: PointStatus,
    #[serde(deserialize_with = "nullable")]
    pub reason: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub report: Option<AnalysisReport>,
}
impl PointResult {
    pub(super) fn planned(point: &Point) -> Self {
        Self {
            id: point.id.clone(),
            model: point.model.clone(),
            source_sha256: None,
            requested: Selection {
                configuration: point.configuration.clone(),
                scenario: point.scenario.clone(),
            },
            effective: None,
            effective_inputs: None,
            status: PointStatus::NotRun,
            reason: Some("no_analysis".into()),
            report: None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub kind: String,
    pub status: String,
    #[serde(deserialize_with = "nullable")]
    pub reason: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub rationale: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub manifest_sha256: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub point_ids: Option<Vec<String>>,
    #[serde(deserialize_with = "nullable")]
    pub planned: Option<usize>,
    #[serde(deserialize_with = "nullable")]
    pub evaluated: Option<usize>,
    #[serde(deserialize_with = "nullable")]
    pub refused: Option<usize>,
    #[serde(deserialize_with = "nullable")]
    pub not_run: Option<usize>,
    #[serde(deserialize_with = "nullable")]
    pub all_points_evaluated: Option<bool>,
    pub continuous_region_checked: bool,
    pub robust_guarantee: bool,
}
impl Scope {
    pub(super) fn unavailable() -> Self {
        Self {
            kind: "enumerated_models".into(),
            status: "unavailable".into(),
            reason: Some("manifest_unavailable".into()),
            rationale: None,
            manifest_sha256: None,
            point_ids: None,
            planned: None,
            evaluated: None,
            refused: None,
            not_run: None,
            all_points_evaluated: None,
            continuous_region_checked: false,
            robust_guarantee: false,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultMode {
    SensitivityExhaustive,
    SensitivitySimulation,
}
impl ResultMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SensitivityExhaustive => "sensitivity_exhaustive",
            Self::SensitivitySimulation => "sensitivity_simulation",
        }
    }
    pub fn analysis(self) -> Mode {
        match self {
            Self::SensitivityExhaustive => Mode::Exhaustive,
            Self::SensitivitySimulation => Mode::Simulation,
        }
    }
}
impl From<Mode> for ResultMode {
    fn from(m: Mode) -> Self {
        match m {
            Mode::Exhaustive => Self::SensitivityExhaustive,
            Mode::Simulation => Self::SensitivitySimulation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "ReportWire")]
pub struct SensitivityReport {
    pub ledger: Ledger,
    pub schema: String,
    pub mode: ResultMode,
    pub scope: Scope,
    pub points: Vec<PointResult>,
    pub summaries: Vec<Summary>,
    pub uncertainty: Uncertainty,
    pub verdict: Verdict,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportWire {
    ledger: Ledger,
    schema: String,
    mode: ResultMode,
    scope: Scope,
    points: Vec<PointResult>,
    summaries: Vec<Summary>,
    uncertainty: Uncertainty,
    verdict: Verdict,
    diagnostics: Vec<Diagnostic>,
}
impl TryFrom<ReportWire> for SensitivityReport {
    type Error = String;
    fn try_from(r: ReportWire) -> Result<Self, String> {
        let result = Self {
            ledger: r.ledger,
            schema: r.schema,
            mode: r.mode,
            scope: r.scope,
            points: r.points,
            summaries: r.summaries,
            uncertainty: r.uncertainty,
            verdict: r.verdict,
            diagnostics: r.diagnostics,
        };
        result.validate()?;
        Ok(result)
    }
}
impl SensitivityReport {
    pub(super) fn new(mode: Mode) -> Self {
        let mode = ResultMode::from(mode);
        Self {
            ledger: Ledger::aggregate(mode.as_str()),
            schema: "relca-sensitivity-result/v1".into(),
            mode,
            scope: Scope::unavailable(),
            points: vec![],
            summaries: vec![],
            uncertainty: Uncertainty::not_lowered(),
            verdict: Verdict::Unsupported,
            diagnostics: vec![],
        }
    }
    pub(super) fn summary_inputs(&self) -> Vec<summary::Input> {
        self.points
            .iter()
            .map(|p| summary::Input {
                id: p.id.clone(),
                evaluated: p.status == PointStatus::Evaluated,
                report: p.report.as_ref().map(|r| json!(r)),
            })
            .collect()
    }
    pub(super) fn seal(mut self, targets: &[Target]) -> Self {
        if self.scope.status == "available" {
            self.scope.point_ids = Some(self.points.iter().map(|p| p.id.clone()).collect());
            self.scope.planned = Some(self.points.len());
            let count = |status| self.points.iter().filter(|p| p.status == status).count();
            let n = count(PointStatus::Evaluated);
            self.scope.evaluated = Some(n);
            self.scope.refused = Some(count(PointStatus::Refused));
            self.scope.not_run = Some(count(PointStatus::NotRun));
            self.scope.all_points_evaluated = Some(n == self.points.len());
        }
        self.summaries = summary::collect(self.mode.analysis(), targets, &self.summary_inputs());
        self.verdict = self.fold_verdict();
        let host = json!(self);
        self.uncertainty = Uncertainty::aggregate(&host, self.mode.as_str());
        let host = json!(self);
        crate::ledger::complete(
            &mut self.ledger,
            &host,
            &mut self.verdict,
            &mut self.diagnostics,
        );
        self
    }
    pub(super) fn fold_verdict(&self) -> Verdict {
        let errors = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == crate::diag::Severity::Error)
            .map(|d| {
                if d.code.starts_with("U-") {
                    Verdict::Unsupported
                } else {
                    Verdict::Invalid
                }
            });
        let children = self
            .points
            .iter()
            .filter_map(|p| p.report.as_ref().map(AnalysisReport::verdict));
        let incomplete =
            (self.scope.all_points_evaluated != Some(true)).then_some(Verdict::Inconclusive);
        children
            .chain(errors)
            .chain(incomplete)
            .max_by_key(|v| report::precedence(*v))
            .unwrap_or(Verdict::Unsupported)
    }
    pub fn validate(&self) -> Result<(), String> {
        super::validation::validate(self)
    }
    pub(super) fn host(&self) -> Value {
        json!(self)
    }
}
