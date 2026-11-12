//! Engine output is index-based and independent of JSON field naming.

use crate::{Verdict, diag::Diagnostic, ir};
use serde::Serialize;

/// Replication-level audit, kept out of the user-facing aggregate report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct SimulationOutcome {
    #[serde(skip)]
    pub stochastic_arrivals: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<crate::quality::Quality>,
    #[serde(skip)]
    pub range_stops: Vec<(u64, Vec<StateRangeViolation>)>,
    #[serde(skip)]
    pub capability_refusals: Option<Vec<CapabilityRefusal>>,
    /// Configuration-only shape for an early refusal, never a sampled record.
    #[serde(skip)]
    pub metric_template: Option<crate::metrics::BusinessMetrics>,
    pub replications: Vec<ReplicationOutcome>,
    pub attempted_replications: u64,
    pub completed_replications: u64,
    pub truncated_replications: Option<u64>,
    pub diagnostics: Vec<Diagnostic>,
}

impl SimulationOutcome {
    pub(crate) fn add_refusals(&mut self, rows: impl IntoIterator<Item = CapabilityRefusal>) {
        let Some(table) = &mut self.capability_refusals else {
            return;
        };
        for mut row in rows {
            if let Some(old) = table
                .iter_mut()
                .find(|r| r.case_id == row.case_id && r.call == row.call)
            {
                old.at.min = old.at.min.min(row.at.min);
                old.at.max = old.at.max.max(row.at.max);
                old.observed_count = Some(old.observed_count.unwrap_or(0) + 1);
            } else {
                row.coverage = "sampled".into();
                row.observed_count = Some(1);
                row.samples = None;
                table.push(row);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplicationOutcome {
    /// Per-clause, per-case observations, before any deduplication or statistics.
    #[serde(skip)]
    pub contests: Vec<ContestObservation>,
    #[serde(skip)]
    pub lost_updates: Vec<ir::StateVarId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission: Option<AdmissionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario_observed_until: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<crate::metrics::BusinessMetrics>,
    pub replication: u64,
    pub draws: Vec<SimulationDraw>,
    pub cases: Vec<SimulationCase>,
    pub cut: Option<SimulationCut>,
    #[serde(rename = "witnesses")]
    pub overcommit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContestObservation {
    pub contest: usize,
    pub case_id: u64,
    pub target_known: bool,
    pub target_at: Option<ir::Tick>,
    pub filed: Option<bool>,
    pub started_at: Option<ir::Tick>,
    pub response_at: Option<ir::Tick>,
    pub closed_at: Option<ir::Tick>,
    pub failed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SimulationDraw {
    pub key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationCut {
    Horizon,
    Clock,
    LiveTasks,
    Numeric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryTruth {
    Hit,
    Miss,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SimulationCase {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cut_off: Option<bool>,
    pub case_id: u64,
    pub arrival: u64,
    pub terminal: Option<SimulationTerminal>,
    pub queries: Vec<QueryTruth>,
    #[serde(rename = "witnesses")]
    pub violations: Vec<bool>,
    pub deadlock: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SimulationTerminal {
    pub kind: SimulationTerminalKind,
    pub end: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationTerminalKind {
    ReceiptUnknown,
    End,
    Rejected,
    Overflow,
}

/// One outcome per exploration. Only completed uncut paths contribute metrics.
/// Diagnostics may contain W-DEADLOCK-0 or an unsupported engine failure; they
/// must not disguise an incomplete exploration as an empty successful result.
#[derive(Clone, PartialEq, Eq)]
pub struct Outcome {
    pub response_exercise: Option<Vec<CapabilityExercise>>,
    pub response_receipts: Option<Vec<ResponseReceipt>>,
    pub scenario_observed_until: Option<crate::metrics::Scalar>,
    pub quality: Option<crate::quality::Quality>,
    pub state_range_violations: Vec<StateRangeViolation>,
    pub capability_refusals: Option<Vec<CapabilityRefusal>>,
    pub unchecked: Vec<String>,
    pub postprocessing_diagnostics: Vec<Diagnostic>,
    pub contest_runtime: Option<Vec<crate::contest::Runtime>>,
    pub contest_summary: Option<crate::contest::Summary>,
    pub obligations: Option<Vec<crate::obligation::Obligation>>,
    pub metrics: Option<crate::metrics::BusinessMetrics>,
    pub case_outcomes: Option<Vec<CaseOutcome>>,
    pub admission: Option<AdmissionSummary>,
    pub probability_scheduler: bool,
    /// None preserves the legacy label. Some(true) includes unexamined suffixes.
    pub probability_policy_unresolved: Option<bool>,
    pub properties: Vec<PropertyOutcome>,
    pub terminals: Vec<TerminalOutcome>,
    pub truncated: bool,
    pub diagnostics: Vec<Diagnostic>,
}

/// Attempted values are exact decimal strings, not stored states or probabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRangeViolation {
    pub case_id: Option<u64>,
    pub time: u64,
    pub binding: String,
    pub actor: String,
    pub capability: String,
    pub phase: String,
    pub variable: String,
    pub operator: String,
    pub before: String,
    pub operand: String,
    pub attempted: String,
    pub lower: String,
    pub upper: String,
    pub write_span: crate::diag::Span,
    pub declaration_span: crate::diag::Span,
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("Outcome");
        if let Some(receipts) = &self.response_receipts {
            d.field("response_receipts", receipts);
        }
        if let Some(q) = &self.quality {
            d.field("quality", q);
        }
        if let Some(rows) = &self.capability_refusals {
            d.field("capability_refusals", rows);
        }
        if !self.unchecked.is_empty() {
            d.field("unchecked", &self.unchecked);
        }
        if !self.postprocessing_diagnostics.is_empty() {
            d.field(
                "postprocessing_diagnostics",
                &self.postprocessing_diagnostics,
            );
        }
        if let Some(unresolved) = self.probability_policy_unresolved {
            d.field("probability_policy_unresolved", &unresolved);
        }
        d.field("metrics", &self.metrics)
            .field("case_outcomes", &self.case_outcomes)
            .field("admission", &self.admission)
            .field("probability_scheduler", &self.probability_scheduler)
            .field("properties", &self.properties)
            .field("terminals", &self.terminals)
            .field("truncated", &self.truncated)
            .field("diagnostics", &self.diagnostics);
        if let Some(rows) = &self.obligations {
            d.field("obligations", rows);
        }
        if let Some(rows) = &self.contest_runtime {
            d.field("contest_runtime", rows);
        }
        if let Some(summary) = &self.contest_summary {
            d.field("contest_summary", summary);
        }
        d.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct CapabilityRefusal {
    pub case_id: u64,
    pub call: String,
    pub capability: String,
    pub result: String,
    pub at: RefusalRange,
    pub coverage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub samples: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct RefusalRange {
    pub min: u64,
    pub max: u64,
}

/// One per requested verify, in request order. ReachableEnd is existential:
/// a witness wins even with cuts; no witness plus a cut is inconclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyOutcome {
    pub cases: Option<Vec<CaseProperty>>,
    pub probability: Option<crate::probability::ProbabilityRange>,
    pub property: ir::Property,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseProperty {
    pub case_id: ir::CaseId,
    pub arrival: ir::Tick,
    pub verdict: Verdict,
    pub probability: Option<crate::probability::ProbabilityRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseOutcome {
    pub admitted_at: Option<Option<Range>>,
    pub case_id: ir::CaseId,
    pub arrival: ir::Tick,
    pub terminals: Vec<CaseTerminal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaseTerminalKind {
    ReceiptUnknown,
    End(ir::EndId),
    Rejected,
    Overflow(ir::EndId),
    CutOff,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CaseTerminal {
    pub observed_until: Option<Range>,
    pub observed_elapsed: Option<Range>,
    pub observed_work: Option<Range>,
    pub kind: CaseTerminalKind,
    pub completed_at: Option<Range>,
    pub elapsed: Option<Range>,
    pub work: Option<Range>,
    pub result_at: Option<Range>,
    pub probability: Option<crate::probability::ProbabilityRange>,
}

impl std::fmt::Debug for CaseTerminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("CaseTerminal");
        d.field("kind", &self.kind)
            .field("completed_at", &self.completed_at)
            .field("elapsed", &self.elapsed)
            .field("work", &self.work)
            .field("result_at", &self.result_at)
            .field("probability", &self.probability);
        if self.observed_until.is_some() {
            d.field("observed_until", &self.observed_until)
                .field("observed_elapsed", &self.observed_elapsed)
                .field("observed_work", &self.observed_work);
        }
        d.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdmissionSummary {
    pub rejected_count: Range,
    pub overflow_count: Option<Range>,
    pub external_wait_max: Option<Range>,
}

/// Inclusive exact extrema over observed complete uncut paths; min <= max.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Range {
    pub min: u64,
    pub max: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseReceipt {
    pub case_id: u64,
    pub command: String,
    pub attempt: u64,
    pub capability: String,
    pub receipt: crate::ast::Receipt,
    pub at: Range,
    #[serde(deserialize_with = "required_result")]
    pub observed_result: Option<String>,
    pub resolved_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityExercise {
    pub capability: ir::CapabilityId,
    pub min: usize,
    pub max: usize,
}

fn required_result<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    serde::Deserialize::deserialize(d)
}

/// At most one record per reachable EndId. Elapsed and result_at are absolute
/// ticks from case start; work sums resource-occupancy ticks, not task durations.
/// None means no any adoption on the measured paths; mixed/multiple adoption
/// semantics are unresolved (see plan) and must not be silently summarized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOutcome {
    pub probability: Option<crate::probability::ProbabilityRange>,
    pub end: ir::EndId,
    pub elapsed: Range,
    pub work: Range,
    pub result_at: Option<Range>,
}
