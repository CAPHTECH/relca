//! ADR 0002 wire contract and report component entry points. Null is retained
//! for unknown pre-lowering metadata; it must never be replaced with fake zeros.

use crate::{Verdict, ast, diag, engine, ir, ir::Assumption, lower, outcome, syntax};
use serde::{Deserialize, Serialize};
#[cfg(test)]
mod completion_impl_tests;
#[cfg(test)]
// Frozen review helpers privately load their own originals. Aliasing the
// existing private engine oracle cannot provide the review-only range oracle.
#[allow(clippy::duplicate_mod)]
#[path = "report/q9_review_tests.rs"]
mod q9_review_tests;
#[cfg(test)]
mod waits_refusal_tests;
use sha2::{Digest, Sha256};
use std::fmt::Write;

#[cfg(test)]
mod metrics_tests {
    #[test]
    fn late_refusal_withdraws_all_metric_samples() {
        let source=include_str!("../../../fixtures/sim-keys/model.relca")
            .replace("process P { a = A.c(); if a == first { end yes } else { end no } }", "process P { a = A.c(); if a == first { b = B.c(); end yes } else { end no } }")
            .replace("  bounds", "  actor B { capacity 1; capability c() -> T { duration fixed 1min; result nondet {first,second} } }\n  metrics {window 20min;currency \"JPY\";rate A per_minute 1;rate B per_minute 1;rate A.c per_start 1;rate B.c per_start 1}\n  bounds");
        let report = super::simulate(&source);
        assert_eq!(report.simulation["attempted_replications"], 3);
        assert_eq!(report.simulation["completed_replications"], 2);
        let metrics = serde_json::to_value(report.metrics.unwrap()).unwrap();
        assert_eq!(metrics["status"], "aborted");
        fn check(v: &serde_json::Value) {
            match v {
                serde_json::Value::Object(fields) if fields.contains_key("samples") => {
                    assert_eq!(fields["samples"], 0);
                    assert!(
                        fields["mean"].is_null()
                            && fields["min"].is_null()
                            && fields["max"].is_null()
                    );
                    assert_eq!(fields["reason"], "aborted");
                }
                serde_json::Value::Object(fields) => {
                    for v in fields.values() {
                        check(v);
                    }
                }
                serde_json::Value::Array(values) => {
                    for v in values {
                        check(v);
                    }
                }
                _ => {}
            }
        }
        check(&metrics);
    }
}

/// ADR 0002 result fields plus ADR 0058's mandatory conclusion ledger.
/// Successful lowering fills all metadata/bounds; properties/terminals are not
/// evaluated on static errors. Only verdict Satisfied permits CLI exit zero.
macro_rules! declare_report {
    ($( $(#[$attr:meta])* pub $field:ident: $ty:ty, )*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct Report { $( $(#[$attr])* pub $field: $ty, )* }

        #[derive(Deserialize)]
        struct ReportWire { $( $(#[$attr])* $field: $ty, )* }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct StrictReportWire { $( $(#[$attr])* $field: $ty, )* }

        impl<'de> Deserialize<'de> for Report {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let wire = ReportWire::deserialize(d)?;
                let report = Self { $( $field: wire.$field, )* };
                report.validate_decoded_wire()
            }
        }
        impl Report {
            /// New sensitivity children are closed, without changing the legacy
            /// flattened Report decoder used by explain's existing consumers.
            pub(crate) fn deserialize_strict<'de,D:serde::Deserializer<'de>>(d:D)->Result<Self,D::Error> {
                let wire=StrictReportWire::deserialize(d)?;
                Self { $( $field:wire.$field, )* }.validate_decoded_wire()
            }
            fn validate_decoded_wire<E:serde::de::Error>(self)->Result<Self,E> {
                let report=self;
                report.ledger.validate(&serde_json::to_value(&report).map_err(E::custom)?)
                    .map_err(E::custom)?;
                response::validate(&report).map_err(E::custom)?;
                if let Some(recovery)=&report.response_recovery { recovery.validate_host(&serde_json::to_value(&report).map_err(E::custom)?).map_err(E::custom)?; }
                priority::validate_report(&report).map_err(E::custom)?;
                if let Some(table) = &report.work_profiles {
                    table.validate_host(&serde_json::to_value(&report).map_err(E::custom)?).map_err(E::custom)?;
                }
                crate::agency::degradation::validate_host(report.agency.as_ref(),report.actors.as_deref(),&report.assumptions,report.work_profiles.as_ref()).map_err(E::custom)?;
                crate::agency::degradation::validate_cut(report.agency.as_ref(),report.bounds.truncated,report.verdict).map_err(E::custom)?;
                if let Some(quality) = &report.quality {
                    let deferred = matches!(&report.cut, Cut::Declared(cut)
                        if !cut.defer.is_empty() && cut.checked.iter().any(|v| v == "deferred_metrics"));
                    quality.validate_report_defer(deferred).map_err(E::custom)?;
                }
                Ok(report)
            }
        }
    };
}
declare_report! {
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub response_recovery: Option<crate::recovery::Recovery>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_profiles: Option<work_profiles::WorkProfiles>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority_policy: Option<priority::PriorityPolicy>,
    pub ledger: crate::ledger::Ledger,
    pub uncertainty: crate::uncertainty::Uncertainty,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_time: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_receipts: Option<Vec<outcome::ResponseReceipt>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_analysis: Option<Box<ResponseAnalysis>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<crate::quality::Quality>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_range_violations: Vec<outcome::StateRangeViolation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_refusals: Option<Vec<outcome::CapabilityRefusal>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unchecked: Vec<String>,
    pub actors: Option<Vec<crate::actor_table::Actor>>,
    pub cut_context: CutContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_model_paths: Option<crate::agency::ViaModelPaths>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contest_runtime: Option<Vec<crate::contest::Runtime>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contest_summary: Option<crate::contest::Summary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obligations: Option<Vec<crate::obligation::Obligation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<crate::metrics::BusinessMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responsibilities: Option<Vec<crate::responsibility::Responsibility>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contest_paths: Option<Vec<crate::contest::ContestPath>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_outcomes: Option<Vec<CaseOutcome>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admission: Option<AdmissionSummary>,
    pub agency: Option<crate::agency::Agency>,
    pub verdict: Verdict,
    pub mode: Mode,
    pub unit: Unit,
    pub model: ModelMetadata,
    pub cut: Cut,
    pub assumptions: Vec<Assumption>,
    pub diagnostics: Vec<diag::Diagnostic>,
    pub bounds: Bounds,
    pub properties: Vec<Property>,
    pub terminals: Vec<Terminal>,
}

mod preflight;
pub mod priority;
#[cfg(test)]
#[path = "report/q4_review_tests.rs"]
mod q4_review_tests;
mod response;
pub mod work_profiles;
pub use response::{ResponseAnalysis, ResponseCoverage};

/// Exhaustive denotes the requested analysis mode, not proof that a stub ran it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Exhaustive,
}

/// Phase 0 has precisely one time/work unit, integer minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    #[serde(rename = "min")]
    Min,
}

/// ADR 0007: "undeclared" (with W-CUT-0, also when parsing failed),
/// "unresolved" (a cut was written but static errors stopped resolution), or
/// the resolved declaration. Conclusions hold only under the stated Cut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Cut {
    Status(CutStatus),
    Declared(Box<DeclaredCut>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutStatus {
    Undeclared,
    Unresolved,
}

impl Cut {
    pub const UNDECLARED: Cut = Cut::Status(CutStatus::Undeclared);
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CutContext {
    pub notices: Vec<String>,
    pub reentry: Vec<String>,
    pub horizon: Vec<ir::Horizon>,
}
impl Default for CutContext {
    fn default() -> Self {
        Self {
        notices: vec![
            "Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない".into(),
            "この結果を運用の範囲決定に使う場合は採用経路を別に追うこと".into(),
        ], reentry: Vec::new(), horizon: Vec::new(),
    }
    }
}
impl CutContext {
    pub(crate) fn resolved(model: &ir::Model) -> Self {
        let mut context = Self::default();
        if let Some(cut) = &model.cut {
            context.reentry = cut.reentry.clone();
            context.horizon = cut.horizon.clone();
        }
        context
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredCut {
    pub name: String,
    pub question: Option<String>,
    pub include: Vec<String>,
    pub exclude: Vec<CutEntry>,
    pub defer: Vec<CutEntry>,
    pub externality: Vec<String>,
    pub reentry: Vec<String>,
    /// The v0.2 §6.2 detections applied, and those Phase 0 cannot apply.
    pub checked: Vec<String>,
    pub unchecked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CutEntry {
    pub target: String,
    pub actor: bool,
    pub reason: String,
    pub loses: Option<String>,
}

const CHECKED: [&str; 3] = ["actor_membership", "excluded_call", "missing_loses"];
const UNCHECKED: [&str; 4] = [
    "party_contest_path",
    "retired_capability",
    "comparison_mismatch",
    "deferred_metrics",
];

pub(crate) fn declared_cut(model: &ir::Model, cut: &ir::Cut) -> DeclaredCut {
    let actor = |id: ir::ActorId| model.actors[id.0].name.clone();
    let entries = |entries: &[ir::CutEntry]| {
        entries
            .iter()
            .map(|e| CutEntry {
                target: match &e.target {
                    ir::CutTarget::Actor(id) => actor(*id),
                    ir::CutTarget::Label(label) => label.clone(),
                },
                actor: matches!(e.target, ir::CutTarget::Actor(_)),
                reason: e.reason.clone(),
                loses: e.loses.clone(),
            })
            .collect()
    };
    DeclaredCut {
        name: cut.name.clone(),
        question: cut.question.clone(),
        include: cut.include.iter().map(|id| actor(*id)).collect(),
        exclude: entries(&cut.exclude),
        defer: entries(&cut.defer),
        externality: cut.externality.clone(),
        reentry: cut.reentry.clone(),
        checked: CHECKED
            .into_iter()
            .chain(
                model
                    .responsibility
                    .is_some()
                    .then_some("party_contest_path"),
            )
            .chain(model.deferred_metrics().map(|_| "deferred_metrics"))
            .map(String::from)
            .collect(),
        unchecked: UNCHECKED
            .into_iter()
            .filter(|v| model.responsibility.is_none() || *v != "party_contest_path")
            .filter(|v| *v != "deferred_metrics" || model.deferred_metrics().is_none())
            .map(String::from)
            .collect(),
    }
}

/// Full-source SHA-256 is lowercase hex, not a hash of normalized AST/IR.
/// Null is only for unavailable failure/stub metadata; see plan unresolved #7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub name: Option<String>,
    pub sha256: Option<String>,
}

/// Known accepted bounds are Some; unknown bounds on failure are null.
/// truncated is null until the engine completes an exploration, then reflects
/// actual computational cuts. A static/input failure or an aborted exploration
/// is not evidence of no truncation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bounds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_violation_stops: Option<bool>,
    pub cases: Option<u64>,
    pub live_tasks: Option<u64>,
    pub clock: Option<u64>,
    pub truncated: Option<bool>,
}

/// id is exactly safety no_resource_overcommit or reachable end.<name>.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Property {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_notes"
    )]
    pub notes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cases: Option<Vec<CaseProperty>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<crate::probability::ProbabilityRange>,
    pub id: String,
    pub verdict: Verdict,
}

pub(crate) fn deserialize_notes<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Vec<String>>, D::Error> {
    let notes = Vec::<String>::deserialize(d)?;
    if notes != ["mechanism_unmodeled"] {
        return Err(serde::de::Error::custom("invalid property notes"));
    }
    Ok(Some(notes))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseProperty {
    pub case_id: u64,
    pub arrival: u64,
    pub verdict: Verdict,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<crate::probability::ProbabilityRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseOutcome {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_nullable"
    )]
    pub admitted_at: Option<Option<Range>>,
    pub case_id: u64,
    pub arrival: u64,
    pub terminals: Vec<CaseTerminal>,
}

// Preserve the distinction between a legacy absent key and a declared, unknown
// admission time when reports pass through a typed JSON round trip.
fn present_nullable<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(d)?))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseTerminalKind {
    ReceiptUnknown,
    End,
    Rejected,
    Overflow,
    CutOff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseTerminal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_until: Option<Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_elapsed: Option<Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_work: Option<Range>,
    pub kind: CaseTerminalKind,
    #[serde(deserialize_with = "response::nullable")]
    pub end: Option<String>,
    #[serde(deserialize_with = "response::nullable")]
    pub completed_at: Option<Range>,
    #[serde(deserialize_with = "response::nullable")]
    pub elapsed: Option<Range>,
    #[serde(deserialize_with = "response::nullable")]
    pub work: Option<Range>,
    #[serde(deserialize_with = "response::nullable")]
    pub result_at: Option<Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<crate::probability::ProbabilityRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionSummary {
    pub rejected_count: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflow_count: Option<Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_wait_max: Option<Range>,
}

/// Inclusive integer extrema over uncut completed paths; min <= max.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub min: u64,
    pub max: u64,
}

/// Unique by end; omit unreachable ends from the list, not from properties.
/// result_at must serialize as null when no any result was adopted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terminal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<crate::probability::ProbabilityRange>,
    pub end: String,
    pub elapsed: Range,
    pub work: Range,
    pub result_at: Option<Range>,
}

#[cfg(test)]
#[path = "../tests/support/q6c_record_tests.rs"]
mod q6c_record_tests;

#[cfg(test)]
#[path = "../tests/support/p2_cleanup_report_tests.rs"]
mod p2_cleanup_report_tests;

/// Whole-source pipeline. A failed front end never runs the following stages.
pub fn check(src: &str) -> Report {
    finite_driver(
        check_with(src, syntax::parse, lower::lower, engine::explore),
        engine::DEFAULT_STATE_BUDGET,
    )
}

pub fn check_with_selection(
    src: &str,
    configuration: Option<&str>,
    scenario: Option<&str>,
) -> Report {
    finite_driver(
        check_selection_using(src, configuration, scenario, engine::explore),
        engine::DEFAULT_STATE_BUDGET,
    )
}

/// Sensitivity's all-point gate has already parsed/lowered the captured bytes.
/// Preserve a static refusal without calling any analysis driver on a prefix.
pub(crate) fn sensitivity_preflight_failure(
    src: &str,
    configuration: Option<&str>,
    scenario: Option<&str>,
    model: Option<&ir::Model>,
    diagnostics: Vec<diag::Diagnostic>,
) -> Report {
    let mut r = empty_report(src);
    r.ledger.requested(configuration, scenario);
    if let Ok(ast) = syntax::parse(src) {
        r.model.name = Some(ast.name.value.clone());
        if ast.items.iter().any(|i| matches!(i, ast::Item::Cut(_))) {
            r.cut = Cut::Status(CutStatus::Unresolved);
        }
    }
    if let Some(m) = model
        && crate::actor_table::validate(m).is_ok()
    {
        r.model.name = Some(m.name.clone());
        r.ledger.lowered(m);
        r.ledger.execution["limits"]["state_budget"] = crate::ledger::na("no_analysis");
        r.actors = Some(crate::actor_table::project(m));
        r.uncertainty = crate::uncertainty::Uncertainty::from_model(m);
        r.resource_time = resource_time_record(m);
        r.priority_policy = priority::record(m);
        if let Err(diagnostic) = work_profiles::attach(&mut r, m, src) {
            r.diagnostics.push(diagnostic);
        }
        r.cut_context = CutContext::resolved(m);
        r.bounds = Bounds {
            cases: Some(m.bounds.cases),
            clock: Some(m.bounds.clock),
            live_tasks: Some(m.bounds.live_tasks),
            truncated: None,
            range_violation_stops: None,
        };
        r.assumptions = m.assumptions.clone();
        r.diagnostics.extend(m.diagnostics.clone());
        if let Some(cut) = &m.cut {
            r.cut = Cut::Declared(Box::new(declared_cut(m, cut)));
        }
    }
    finish(r, diagnostics)
}

pub(crate) fn sensitivity_simulation_preflight_failure(
    base: Report,
    model: Option<&ir::Model>,
) -> SimulationReport {
    let mut r = simulation_empty(base);
    if let Some(m) = model {
        r.ledger.simulation_settings(m);
        r.ledger.execution["limits"]["state_budget"] = crate::ledger::na("no_analysis");
    }
    seal_simulation(r)
}

/// The suite checks every backend's full static admission before any driver is
/// called. Ordinary producers use these same pure predicates below.
pub(crate) struct PreexecutionFailure {
    pub reason: &'static str,
    pub diagnostics: Vec<diag::Diagnostic>,
}
pub(crate) fn sensitivity_preexecution(
    model: &ir::Model,
    mode: crate::sensitivity::input::Mode,
    scope: Option<&crate::sensitivity::statistics::SuiteMultiplicity>,
) -> Result<(), PreexecutionFailure> {
    match mode {
        crate::sensitivity::input::Mode::Exhaustive => {
            preflight::finite(model).map_err(|e| PreexecutionFailure {
                reason: e.reason(model),
                diagnostics: e.diagnostics(model),
            })
        }
        crate::sensitivity::input::Mode::Simulation => preflight::simulation(model, scope)
            .map(|_| ())
            .map_err(|e| PreexecutionFailure {
                reason: e.reason(model),
                diagnostics: e.diagnostics(model, model.process.span),
            }),
    }
}

pub(crate) fn check_selection_using(
    src: &str,
    configuration: Option<&str>,
    scenario: Option<&str>,
    execute: impl FnOnce(&ir::Model) -> outcome::Outcome,
) -> Report {
    if configuration.is_none() && scenario.is_none() {
        return check_using(src, execute);
    }
    let mut report = check_with(
        src,
        syntax::parse,
        |a| lower::lower_with_selection(a, configuration, scenario),
        execute,
    );
    report.ledger.requested(configuration, scenario);
    if report.ledger.actor_versions["status"] == "known" {
        report.ledger.execution["selection"]["effective"]["configuration"] =
            configuration.map_or_else(crate::ledger::undeclared, crate::ledger::known);
    }
    seal(report)
}

pub(crate) fn scenario_record(
    m: &ir::Model,
    observed: serde_json::Value,
) -> Option<serde_json::Value> {
    use serde_json::json;
    let s = m.scenario.as_ref()?;
    let arrivals = m.arrivals.as_deref().unwrap_or(&[0]);
    let distribution_bounds = m
        .arrival_distribution
        .as_ref()
        .and_then(|d| d.declared_bounds());
    let shape = if m.arrival_distribution.is_some() {
        json!({"form":"distribution","every":null,"from":distribution_bounds.map(|b| b.0),"until":distribution_bounds.map(|b| b.1),"count":m.bounds.cases})
    } else if let Some(e) = &m.arrival_every {
        json!({"form":"fixed_every","every":e.every,"from":e.from,"until":e.until,"count":e.count})
    } else {
        json!({"form":"fixed","every":null,"from":arrivals.first(),"until":arrivals.last(),"count":arrivals.len()})
    };
    let drain_from = if m.arrival_distribution.is_some() {
        distribution_bounds.map(|b| b.1)
    } else {
        arrivals.last().copied()
    };
    Some(json!({"name":s.name,"arrivals":shape,"horizon":m.horizon,
        "drain":{"from":drain_from,"until":m.horizon.map_or(json!("all_terminal_or_clock"), |d| json!(d))},
        "aggregation":{"from":0,"until":if m.horizon.is_some() {"horizon_or_all_terminal_or_clock"} else {"all_terminal_or_clock"},"observed_until":observed},"warmup":0}))
}

fn resource_time_record(model: &ir::Model) -> Option<serde_json::Value> {
    use serde_json::json;
    let arrivals = model.arrival_distribution.as_ref().map(|d| json!({"basis":d.basis,"mode":"simulation_only",
        "site":"arrival.schedule","entries":d.entries.iter().map(|(times,p)| json!({"times":times,"probability":p.to_string()})).collect::<Vec<_>>()}));
    if model.resource_time.is_none() && (model.has_condition_wait() || arrivals.is_some()) {
        let mut checked = Vec::new();
        if model.has_condition_wait() {
            checked.extend([
                "observed_condition_reassessment",
                "condition_wait_partition",
            ]);
        }
        if arrivals.is_some() {
            checked.extend([
                "finite_arrival_sampling",
                "realized_arrival_relative_deadlines",
                "sampled_arrival_wire",
            ]);
        }
        return Some(
            json!({"rules":"resource-time-v1", "condition_waits":model.has_condition_wait(), "finite_arrivals":arrivals,
            "checked":checked,
            "unchecked":["condition_timeout","external_observation_updates"]}),
        );
    }
    let t = model.resource_time.as_ref()?;
    let calendar_name =
        |id: Option<usize>| id.and_then(|i| t.calendars.get(i)).map(|c| c.name.clone());
    Some(
        serde_json::json!({"rules":"resource-time-v1", "on_close":"continue", "finite_arrivals":arrivals,
        "calendars":t.calendars,
        "actors":model.actors.iter().enumerate().map(|(i,a)| json!({"actor":a.name,
            "calendar":calendar_name(t.actors.get(i).copied().flatten()),
            "overtime_per_minute":t.overtime_rates.get(i).copied().flatten()})).collect::<Vec<_>>(),
        "resources":model.resources.iter().enumerate().map(|(i,r)| json!({"resource":r.name,
            "calendar":calendar_name(t.resources.get(i).copied().flatten())})).collect::<Vec<_>>(),
        "people":model.people.iter().enumerate().map(|(i,p)| json!({"person":p.name,
            "calendar":calendar_name(t.people.get(i).copied().flatten())})).collect::<Vec<_>>(),
        "checked":["calendar_start_gates","calendar_wait_partition","calendar_capacity_integrals","task_local_tariffs"],
        "unchecked":["calendar_preemption","executor_transfer","person_overtime_pricing"]}),
    )
}

/// Ordinary analysis with an alternate exhaustive driver, retaining every
/// front-end and report projection rule of check.
pub(crate) fn check_using(
    src: &str,
    execute: impl FnOnce(&ir::Model) -> outcome::Outcome,
) -> Report {
    check_with(src, syntax::parse, lower::lower, execute)
}

/// ADR 0016 has a separate wire contract; exhaustive Report never acquires
/// simulation keys, null placeholders, or a different scheduler assumption.
macro_rules! declare_simulation_report {
    ($( $(#[$attr:meta])* pub $field:ident: $ty:ty, )*) => {
        #[derive(Debug, Clone, Serialize)]
        pub struct SimulationReport { $( $(#[$attr])* pub $field: $ty, )* }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct SimulationWire { $( $(#[$attr])* $field: $ty, )* }
        impl<'de> Deserialize<'de> for SimulationReport {
            fn deserialize<D: serde::Deserializer<'de>>(d:D)->Result<Self,D::Error> {
                let wire=SimulationWire::deserialize(d)?;
                let r=Self { $( $field:wire.$field, )* };
                r.ledger.validate(&serde_json::to_value(&r).map_err(serde::de::Error::custom)?)
                    .map_err(serde::de::Error::custom)?;
                response::validate(&r).map_err(serde::de::Error::custom)?;
                if let Some(recovery)=&r.response_recovery { recovery.validate_host(&serde_json::to_value(&r).map_err(serde::de::Error::custom)?).map_err(serde::de::Error::custom)?; }
                if let Some(table) = &r.work_profiles {
                    table.validate_host(&serde_json::to_value(&r).map_err(serde::de::Error::custom)?)
                        .map_err(serde::de::Error::custom)?;
                }
                crate::agency::degradation::validate_host(r.agency.as_ref(),r.actors.as_deref(),&r.assumptions,r.work_profiles.as_ref()).map_err(serde::de::Error::custom)?;
                crate::agency::degradation::validate_cut(r.agency.as_ref(),r.bounds.truncated,r.verdict).map_err(serde::de::Error::custom)?;
                Ok(r)
            }
        }
    }
}
fn simulation_mode<'de, D: serde::Deserializer<'de>>(d: D) -> Result<&'static str, D::Error> {
    if String::deserialize(d)? == "simulation" {
        Ok("simulation")
    } else {
        Err(serde::de::Error::custom("unknown simulation mode"))
    }
}
// The decoder interns only this closed constant; it never borrows input bytes.
type SimulationModeName = &'static str;
declare_simulation_report! {
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub response_recovery: Option<crate::recovery::Recovery>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub response_analysis: Option<Box<ResponseAnalysis>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_profiles: Option<work_profiles::WorkProfiles>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority_policy: Option<priority::PriorityPolicy>,
    pub ledger: crate::ledger::Ledger,
    pub uncertainty: crate::uncertainty::Uncertainty,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_time: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<crate::quality::Quality>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_range_violations: Vec<outcome::StateRangeViolation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_refusals: Option<Vec<outcome::CapabilityRefusal>>,
    pub actors: Option<Vec<crate::actor_table::Actor>>,
    pub cut_context: CutContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_model_paths: Option<crate::agency::ViaModelPaths>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unchecked: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<crate::metrics::BusinessMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responsibilities: Option<Vec<crate::responsibility::Responsibility>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contest_paths: Option<Vec<crate::contest::ContestPath>>,
    pub agency: Option<crate::agency::Agency>,
    pub verdict: Verdict,
    #[serde(deserialize_with="simulation_mode")]
    pub mode: SimulationModeName,
    pub unit: Unit,
    pub model: ModelMetadata,
    pub cut: Cut,
    pub assumptions: Vec<Assumption>,
    pub diagnostics: Vec<diag::Diagnostic>,
    pub bounds: Bounds,
    pub simulation: serde_json::Value,
    pub properties: Vec<serde_json::Value>,
    pub terminals: Vec<serde_json::Value>,
    pub case_outcomes: Vec<serde_json::Value>,
    pub admission: Option<serde_json::Value>,
}

pub fn simulate(src: &str) -> SimulationReport {
    simulate_selected(src, None)
}
pub fn simulate_configuration(src: &str, configuration: &str) -> SimulationReport {
    simulate_selected(src, Some(configuration))
}

pub fn simulate_with_selection(
    src: &str,
    configuration: Option<&str>,
    scenario: Option<&str>,
) -> SimulationReport {
    simulation_driver(simulate_pipeline(
        src,
        configuration,
        scenario,
        engine::simulate,
        &mut None,
    ))
}

/// The ordinary simulation pipeline with a frozen, pre-execution family. The
/// model's seed, n and confidence are not rewritten; its confidence must match.
pub fn simulate_with_multiplicity(
    src: &str,
    configuration: Option<&str>,
    scenario: Option<&str>,
    scope: &crate::sensitivity::statistics::SuiteMultiplicity,
) -> SimulationReport {
    let report = simulate_pipeline_raw(
        src,
        configuration,
        scenario,
        engine::simulate,
        &mut None,
        Some(scope),
    );
    simulation_driver(simulation_selection(report, configuration, scenario))
}

fn sampled_arrival(
    sampled: &outcome::SimulationOutcome,
    case: usize,
    aborted: bool,
) -> serde_json::Value {
    use serde_json::json;
    let times: Vec<_> = sampled
        .replications
        .iter()
        .filter_map(|r| r.cases.get(case).map(|c| c.arrival))
        .collect();
    if aborted || times.is_empty() {
        return json!({"mean":null,"min":null,"max":null,"samples":0,"reason":if aborted {"aborted"} else {"no_samples"}});
    }
    let total: num_bigint::BigInt = times.iter().copied().map(num_bigint::BigInt::from).sum();
    let mean = num_rational::BigRational::new(total, times.len().into()).to_string();
    json!({"mean":mean,"min":times.iter().min().unwrap().to_string(),"max":times.iter().max().unwrap().to_string(),"samples":times.len(),"reason":null})
}

pub(crate) fn summarize_metrics(
    sampled: &outcome::SimulationOutcome,
    aborted: bool,
) -> Option<crate::metrics::BusinessMetrics> {
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use num_traits::Zero;
    use serde_json::{Value, json};
    let mut records: Vec<_> = sampled
        .replications
        .iter()
        .filter_map(|r| r.metrics.as_ref())
        .map(|m| {
            m.assert_wait_populations();
            serde_json::to_value(m).expect("metric record")
        })
        .collect();
    let mut value = records.first().cloned().or_else(|| {
        sampled.metric_template.as_ref().map(|m| {
            m.assert_wait_populations();
            serde_json::to_value(m).expect("metric template")
        })
    })?;
    // Only A7's conditional dwell and lower marks use union presence. Numeric
    // samples still come from actual replication records, never from this template.
    if value["overall"].get("queue_wait_observed").is_some() {
        let mut template: crate::metrics::BusinessMetrics =
            serde_json::from_value(value).expect("metric template");
        for record in &records {
            template.union_wait_presence(
                &serde_json::from_value(record.clone()).expect("metric record"),
            );
        }
        if aborted {
            template.withdraw_wait_bounds();
        }
        value = serde_json::to_value(template).expect("metric template");
    }
    if sampled.stochastic_arrivals {
        for record in std::iter::once(&mut value).chain(records.iter_mut()) {
            for case in record["cases"].as_array_mut().expect("metric cases") {
                let arrival = case["arrival"].as_u64();
                case["arrival_time"] = arrival.map_or_else(
                    || json!({"min":null,"max":null,"reason":"aborted"}),
                    |n| json!({"min":n.to_string(),"max":n.to_string(),"reason":null}),
                );
                case["arrival"] = Value::Null;
            }
        }
    }
    fn summarize(value: &mut Value, records: &[&Value], aborted: bool) {
        match value {
            Value::Object(fields) if fields.contains_key("min") && fields.contains_key("max") => {
                let numbers: Vec<_> = records
                    .iter()
                    .filter_map(|v| {
                        serde_json::from_value::<crate::metrics::MetricNumber>(v["min"].clone())
                            .ok()
                    })
                    .collect();
                let reason = if aborted {
                    Some("aborted".to_owned())
                } else if numbers.is_empty() {
                    Some(
                        match fields["reason"].as_str() {
                            Some("no_terminal") | Some("no_samples") => "no_samples",
                            Some(s) => s,
                            None => "no_samples",
                        }
                        .to_owned(),
                    )
                } else {
                    None
                };
                *value = if reason.is_some() {
                    json!({"mean":null,"min":null,"max":null,"samples":0,"reason":reason})
                } else {
                    let sum = numbers
                        .iter()
                        .fold(BigRational::zero(), |sum, x| sum + &x.0);
                    let mean = sum / BigRational::from_integer(BigInt::from(numbers.len()));
                    json!({"mean":mean.to_string(),"min":numbers.iter().min().unwrap(),"max":numbers.iter().max().unwrap(),
                        "samples":numbers.len(),"reason":null})
                };
            }
            Value::Object(fields)
                if fields.contains_key("stats") && fields.contains_key("observed_total") =>
            {
                // A record without an integer count makes the total unknown,
                // never zero.
                let total = if aborted {
                    None
                } else {
                    records
                        .iter()
                        .map(|v| v["stats"]["min"].as_str()?.parse::<u64>().ok())
                        .sum::<Option<u64>>()
                };
                let stats_records: Vec<_> = records.iter().map(|v| &v["stats"]).collect();
                summarize(fields.get_mut("stats").unwrap(), &stats_records, aborted);
                fields.insert("observed_total".into(), json!(total));
            }
            Value::Object(fields) => {
                for (key, v) in fields.iter_mut() {
                    let children: Vec<_> = records.iter().map(|r| &r[key]).collect();
                    summarize(v, &children, aborted);
                }
                if fields.contains_key("completed_cases_total") {
                    let total = fields["count"]["observed_total"].clone();
                    fields.insert("completed_cases_total".into(), total);
                }
            }
            Value::Array(values) => {
                for (i, v) in values.iter_mut().enumerate() {
                    let children: Vec<_> = records.iter().map(|r| &r[i]).collect();
                    summarize(v, &children, aborted);
                }
            }
            _ => {}
        }
    }
    summarize(&mut value, &records.iter().collect::<Vec<_>>(), aborted);
    value["status"] = if aborted {
        "aborted"
    } else if sampled.truncated_replications.is_some_and(|n| n > 0) {
        "truncated"
    } else {
        "complete"
    }
    .into();
    value["statistical_unit"] = "replication".into();
    value["estimate_note"] = "点推定であり、全スケジュールの検証でも区間推定でもない".into();
    let mut result: crate::metrics::BusinessMetrics =
        serde_json::from_value(value).expect("summarized metrics");
    result.align_wait_reasons();
    Some(result)
}

fn simulation_empty(mut base: Report) -> SimulationReport {
    place_output_assumptions(&mut base.assumptions, base.quality.is_some(), false);
    simulation_base(base)
}

// This conversion also initializes reports which will receive producer output;
// only the final exits may decide whether the waiting disclosure is applicable.
fn simulation_base(mut base: Report) -> SimulationReport {
    if base.quality.is_some() {
        place_quality_assumptions(&mut base.assumptions);
    }
    use serde_json::json;
    let status = if base.verdict == Verdict::Invalid {
        "invalid"
    } else {
        "unsupported"
    };
    base.ledger.simulation_mode(base.model.sha256.is_none());
    SimulationReport {
        response_recovery: None,
        response_analysis: None,
        work_profiles: base.work_profiles,
        priority_policy: base.priority_policy,
        resource_time: base.resource_time,
        ledger: base.ledger,
        uncertainty: base.uncertainty,
        scenario: base.scenario,
        quality: base.quality,
        state_range_violations: Vec::new(),
        capability_refusals: None,
        actors: base.actors,
        cut_context: base.cut_context,
        via_model_paths: base.via_model_paths,
        unchecked: base.unchecked,
        metrics: None,
        responsibilities: base.responsibilities,
        contest_paths: base.contest_paths,
        agency: base.agency,
        verdict: base.verdict,
        mode: "simulation",
        unit: base.unit,
        model: base.model,
        cut: base.cut,
        assumptions: base.assumptions,
        diagnostics: base.diagnostics,
        bounds: base.bounds,
        properties: Vec::new(),
        terminals: Vec::new(),
        case_outcomes: Vec::new(),
        admission: None,
        simulation: json!({"status":status,"replications":null,"seed":null,"confidence":null,
            "attempted_replications":0,"completed_replications":0,"truncated_replications":null,
            "interval_method":"clopper-pearson","multiplicity":null,
            "random_method":"sha256-counter-v1","nondeterminism":"reject-on-encounter","deadlock_witnesses":[]}),
    }
}

/// Usage/read failures know no source hash or bounds and do not invoke parsing.
pub fn simulation_input_failure(message: &str) -> SimulationReport {
    let mut base = empty_report("");
    base.model.sha256 = None;
    seal_simulation(simulation_empty(finish(
        base,
        vec![diag::Diagnostic {
            code: diag::E_SYN_0.into(),
            severity: diag::Severity::Error,
            message: message.into(),
            span: None,
        }],
    )))
}

pub(crate) fn comparison_input_failure(message: &str) -> Report {
    let mut report = empty_report("");
    report.model.sha256 = None;
    report.ledger = crate::ledger::Ledger::input_failure("exhaustive");
    finish(
        report,
        vec![diag::Diagnostic {
            code: diag::E_SYN_0.into(),
            severity: diag::Severity::Error,
            message: message.into(),
            span: None,
        }],
    )
}

fn simulate_selected(src: &str, selection: Option<&str>) -> SimulationReport {
    simulation_driver(simulate_using(src, selection, engine::simulate))
}

#[cfg(test)]
mod q6_private {
    #[test]
    fn reference_aggregate() {
        crate::q6_acceptance::private::reference_aggregate(|source| {
            serde_json::json!(super::simulation_driver(super::simulate_using(
                source,
                None,
                |m| {
                    let r = crate::reference::simulate(m);
                    assert_eq!(r.stop, crate::reference::StopReason::Complete);
                    r.outcome
                }
            )))
        });
    }
}

fn simulate_using(
    src: &str,
    selection: Option<&str>,
    execute: impl FnOnce(&ir::Model) -> outcome::SimulationOutcome,
) -> SimulationReport {
    simulate_pipeline(src, selection, None, execute, &mut None)
}

pub(crate) fn simulate_selected_metadata(
    src: &str,
    configuration: &str,
) -> (SimulationReport, Option<lower::SelectedConfiguration>) {
    let (report, selected) = simulate_selected_using(src, configuration, engine::simulate);
    (simulation_driver(report), selected)
}

pub(crate) fn simulate_selected_using(
    src: &str,
    configuration: &str,
    execute: impl FnOnce(&ir::Model) -> outcome::SimulationOutcome,
) -> (SimulationReport, Option<lower::SelectedConfiguration>) {
    let mut selected = None;
    let report = simulate_pipeline(src, Some(configuration), None, execute, &mut selected);
    (report, selected)
}

pub(crate) fn simulate_selected_scenario(
    src: &str,
    configuration: &str,
    scenario: Option<&str>,
) -> (SimulationReport, Option<lower::SelectedConfiguration>) {
    let mut selected = None;
    let report = simulate_pipeline(
        src,
        Some(configuration),
        scenario,
        engine::simulate,
        &mut selected,
    );
    (simulation_driver(report), selected)
}

fn simulate_pipeline(
    src: &str,
    selection: Option<&str>,
    scenario: Option<&str>,
    execute: impl FnOnce(&ir::Model) -> outcome::SimulationOutcome,
    selected: &mut Option<lower::SelectedConfiguration>,
) -> SimulationReport {
    let report = simulate_pipeline_raw(src, selection, scenario, execute, selected, None);
    simulation_selection(report, selection, scenario)
}

fn simulation_selection(
    mut report: SimulationReport,
    selection: Option<&str>,
    scenario: Option<&str>,
) -> SimulationReport {
    report.ledger.requested(selection, scenario);
    if report.ledger.actor_versions["status"] == "known" {
        report.ledger.execution["selection"]["effective"]["configuration"] =
            selection.map_or_else(crate::ledger::undeclared, crate::ledger::known);
    }
    seal_simulation(report)
}

fn simulate_pipeline_raw(
    src: &str,
    selection: Option<&str>,
    scenario: Option<&str>,
    execute: impl FnOnce(&ir::Model) -> outcome::SimulationOutcome,
    selected: &mut Option<lower::SelectedConfiguration>,
    scope: Option<&crate::sensitivity::statistics::SuiteMultiplicity>,
) -> SimulationReport {
    let mut base = empty_report(src);
    let ast = match syntax::parse(src) {
        Ok(a) => a,
        Err(ds) => return simulation_empty(finish(base, ds)),
    };
    base.model.name = Some(ast.name.value.clone());
    if ast.items.iter().any(|i| matches!(i, ast::Item::Cut(_))) {
        base.cut = Cut::Status(CutStatus::Unresolved);
    }
    if scenario.is_none()
        && !ast
            .items
            .iter()
            .any(|i| matches!(i, ast::Item::Scenario(_)))
        && !ast
            .items
            .iter()
            .any(|i| matches!(i, ast::Item::Simulation(_)))
    {
        let mut quality_model = None;
        if ast.items.iter().any(|i| {
            matches!(
                i,
                ast::Item::QualityProperty(_)
                    | ast::Item::CohortMetric(_)
                    | ast::Item::QuantileMetric(_)
            )
        }) {
            let model = match selection {
                Some(c) => lower::lower_configuration(&ast, c).map(|s| s.model),
                None => lower::lower(&ast),
            };
            if let Ok(m) = model {
                base.quality = Some(crate::quality::simulation(&m, &[], true));
                quality_model = Some(m);
            }
        }
        // Preserve legacy missing-configuration behavior byte-for-byte. For a
        // new revocation record only, resolve its names before publishing it.
        if ast.items.iter().any(|i| matches!(i, ast::Item::Authority(a) if a.items.iter().any(|f| matches!(f.value, ast::AuthorityItem::RevocableBy(_))))) {
            let lowered = match selection {
                Some(c) => lower::lower_configuration(&ast, c).map(|s| s.model),
                None => lower::lower(&ast),
            };
            if let Ok(m) = lowered {
                base.assumptions = m.assumptions.clone();
                authority_record(&mut base, &m);
            }
        }
        if let Some(m) = &quality_model {
            quality_refusal_rows(&mut base.assumptions, m);
        }
        return simulation_empty(finish(
            base,
            vec![diag::Diagnostic {
                code: diag::E_SIM_0.into(),
                severity: diag::Severity::Error,
                message: "simulation requires an explicit simulation clause".into(),
                span: Some(ast.span),
            }],
        ));
    }
    let lowered = match selection {
        Some(c) => lower::lower_configuration_with_scenario(&ast, c, scenario).map(|c| {
            let model = c.model.clone();
            *selected = Some(c);
            model
        }),
        None => lower::lower_with_selection(&ast, None, scenario),
    };
    let model = match lowered {
        Ok(m) => m,
        Err(ds) => return simulation_empty(finish(base, ds)),
    };
    if let Err(diagnostic) = work_profiles::attach(&mut base, &model, src) {
        return simulation_empty(finish(base, vec![diagnostic]));
    }
    match scope {
        None => simulate_model(base, &model, ast.span, execute),
        Some(scope) => simulate_model_with_scope(base, &model, ast.span, Some(scope), execute),
    }
}

// Keep the post-lowering guards and the producer in one real pipeline. The
// private acceptance can supply a damaged lowered model without copying it.
fn simulate_model(
    base: Report,
    model: &ir::Model,
    source_span: diag::Span,
    execute: impl FnOnce(&ir::Model) -> outcome::SimulationOutcome,
) -> SimulationReport {
    simulate_model_with_scope(base, model, source_span, None, execute)
}

fn simulate_model_with_scope(
    base: Report,
    model: &ir::Model,
    source_span: diag::Span,
    scope: Option<&crate::sensitivity::statistics::SuiteMultiplicity>,
    execute: impl FnOnce(&ir::Model) -> outcome::SimulationOutcome,
) -> SimulationReport {
    let called = std::cell::Cell::new(false);
    let mut report = simulate_model_raw(base, model, source_span, scope, |m| {
        called.set(true);
        execute(m)
    });
    if model.simulation.is_some() {
        report.ledger.simulation_settings(model);
        // Settings record lowering even on raw preexecution refusals. Mirror
        // those declared bounds in the host before sealing, without inventing
        // an observed truncation or range-stop count.
        if report.ledger.actor_versions["status"] == "known" {
            report.bounds.cases = Some(model.bounds.cases);
            report.bounds.clock = Some(model.bounds.clock);
            report.bounds.live_tasks = Some(model.bounds.live_tasks);
            report.actors = Some(crate::actor_table::project(model));
        }
        if called.get() {
            report.ledger.unknown_driver_called();
        }
    }
    seal_simulation(report)
}

fn simulate_model_raw(
    mut base: Report,
    model: &ir::Model,
    source_span: diag::Span,
    scope: Option<&crate::sensitivity::statistics::SuiteMultiplicity>,
    execute: impl FnOnce(&ir::Model) -> outcome::SimulationOutcome,
) -> SimulationReport {
    base.uncertainty = crate::uncertainty::Uncertainty::from_model(model);
    let admission = preflight::simulation(model, scope);
    use crate::{
        outcome::{QueryTruth, SimulationTerminalKind},
        probability::Rational,
    };
    use num_rational::BigRational;
    use serde_json::json;
    if model.quality.is_some() {
        base.quality = Some(crate::quality::simulation(model, &[], true));
    }
    let m = model;
    // These fields historically start at the wire-range guard, not at the
    // earlier diagnostics/arrival refusals. Keep that projection boundary.
    let prepare = |base: &mut Report| {
        base.resource_time = resource_time_record(m);
        base.priority_policy = priority::record(m);
        if m.authorities
            .iter()
            .any(|a| a.restrictions.revocable_by.is_some())
        {
            base.assumptions = m.assumptions.clone();
            authority_record(base, m);
        }
        base.actors = Some(crate::actor_table::project(m));
        base.via_model_paths = crate::agency::via_model_paths(m);
    };
    let effective_rows = match admission {
        Err(failure @ preflight::SimulationFailure::MissingClause) => {
            if model.quality.is_some() {
                quality_refusal_rows(&mut base.assumptions, model);
            }
            return simulation_empty(finish(base, failure.diagnostics(m, source_span)));
        }
        Err(failure @ preflight::SimulationFailure::Diagnostics)
        | Err(failure @ preflight::SimulationFailure::StochasticArrivals) => {
            return simulation_refusal(base, m, failure.diagnostics(m, source_span));
        }
        Err(failure @ preflight::SimulationFailure::WireRange) => {
            prepare(&mut base);
            return simulation_refusal(base, m, failure.diagnostics(m, source_span));
        }
        Err(preflight::SimulationFailure::Scope { error, .. }) => {
            prepare(&mut base);
            // This new refusal is after successful lowering but before the
            // producer. Keep declared bounds/selection known, not fake
            // truncation or a pre-lowering ledger with an Actor table.
            base.ledger.lowered(m);
            base.bounds = Bounds {
                cases: Some(m.bounds.cases),
                live_tasks: Some(m.bounds.live_tasks),
                clock: Some(m.bounds.clock),
                truncated: None,
                range_violation_stops: None,
            };
            base.cut_context = CutContext::resolved(m);
            if let Some(cut) = &m.cut {
                base.cut = Cut::Declared(Box::new(declared_cut(m, cut)));
            }
            if base.assumptions.is_empty() {
                base.assumptions = m.assumptions.clone();
            }
            base.diagnostics.extend(m.diagnostics.clone());
            return simulation_refusal(base, m, vec![error.diagnostic()]);
        }
        Ok(counts) => {
            prepare(&mut base);
            counts.effective_rows
        }
    };
    let spec = m.simulation.as_ref().expect("admitted simulation clause");
    let n = spec.replications;
    let times = m.arrivals.clone().unwrap_or_else(|| vec![0]);
    let rows = num_bigint::BigInt::from(effective_rows);
    let alpha_before = (!m.probability_queries.is_empty()).then(|| {
        Rational((Rational::one().0 - &spec.confidence.0) / BigRational::from_integer(2.into()))
    });
    let alpha_after = alpha_before
        .as_ref()
        .map(|a| Rational(&a.0 / BigRational::from_integer(rows.clone())));
    base.agency = Some(m.agency.clone());
    base.cut_context = CutContext::resolved(m);
    base.responsibilities = m
        .responsibility
        .as_ref()
        .map(|m| m.responsibilities.clone());
    base.contest_paths = m.responsibility.as_ref().map(|m| m.contest_paths.clone());
    base.assumptions = m.assumptions.clone();
    if let Some(c) = &m.cut {
        base.cut = Cut::Declared(Box::new(declared_cut(m, c)));
    }
    base.diagnostics = m.diagnostics.clone();
    let mut out = simulation_base(base);
    if m.tracks_obligations() {
        out.unchecked.push("obligation_outcomes".into());
    }
    out.unchecked.push("unreachable".into());
    if let Some(i) = out
        .unchecked
        .iter()
        .position(|s| s == "dynamic_authority_revocation")
    {
        let word = out.unchecked.remove(i);
        out.unchecked.push(word);
    }
    out.bounds = Bounds {
        range_violation_stops: None,
        cases: Some(m.bounds.cases),
        live_tasks: Some(m.bounds.live_tasks),
        clock: Some(m.bounds.clock),
        truncated: None,
    };
    let sampled = execute(m);
    out.response_recovery = sampled.response_recovery.clone();
    if out.response_recovery.is_some() {
        out.response_analysis = Some(Box::new(response::sampled_analysis(m)));
    }
    for (_, ranges) in &sampled.range_stops {
        for r in ranges {
            if !out.state_range_violations.contains(r) {
                out.state_range_violations.push(r.clone());
            }
        }
    }
    out.bounds.range_violation_stops = (!out.state_range_violations.is_empty()).then_some(true);
    let aborted = sampled
        .diagnostics
        .iter()
        .any(|d| d.severity == diag::Severity::Error);
    out.metrics = summarize_metrics(&sampled, aborted);
    if let Some(metrics) = &mut out.metrics {
        match crate::metrics::contests::summarize(m, &sampled, aborted) {
            Ok(table) => {
                metrics.contests = table.map(Box::new);
                if m.deferred_metrics().is_some_and(|d| d.process)
                    && let Some(table) = &mut metrics.contests
                {
                    table.apply_deferred();
                }
            }
            Err(message) => {
                out.diagnostics.push(diag::Diagnostic {
                    code: diag::U_SEM_0.into(),
                    severity: diag::Severity::Error,
                    message,
                    span: Some(m.process.span),
                });
                out.metrics = None;
                out.verdict = Verdict::Unsupported;
                return out;
            }
        }
    }
    if m.scenario.is_some() {
        use crate::metrics::{MetricNumber, Sample};
        let values: Vec<u64> = sampled
            .replications
            .iter()
            .filter_map(|r| r.scenario_observed_until)
            .collect();
        let reason = if aborted {
            Some(crate::metrics::Reason::Aborted)
        } else if values.is_empty() {
            Some(crate::metrics::Reason::NoSamples)
        } else {
            None
        };
        let sample = Sample {
            mean: reason.is_none().then(|| {
                MetricNumber(BigRational::new(
                    values.iter().map(|v| num_bigint::BigInt::from(*v)).sum(),
                    values.len().into(),
                ))
            }),
            min: reason
                .is_none()
                .then(|| (*values.iter().min().unwrap()).into()),
            max: reason
                .is_none()
                .then(|| (*values.iter().max().unwrap()).into()),
            samples: if reason.is_none() {
                values.len() as u64
            } else {
                0
            },
            reason,
        };
        out.scenario = scenario_record(m, json!(sample));
    }
    out.quality = sampled.quality.clone();
    if out.quality.is_some() {
        out.assumptions.push(quality_estimation());
    }
    out.capability_refusals = m.has_requires().then(|| {
        if aborted {
            Vec::new()
        } else {
            sampled
                .capability_refusals
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|mut r| {
                    r.samples = Some(sampled.completed_replications);
                    r
                })
                .collect()
        }
    });
    out.diagnostics.extend(sampled.diagnostics.clone());
    let mut deadlocks = Vec::new();
    if !aborted {
        out.bounds.truncated = Some(sampled.truncated_replications.is_some_and(|k| k > 0));
        for k in 0..times.len() {
            if let Some(r) = sampled
                .replications
                .iter()
                .find(|r| r.cases[k].deadlock.is_some())
            {
                deadlocks.push(
                    json!({"case_id":k+1,"replication":r.replication,"time":r.cases[k].deadlock}),
                );
            }
        }
        if !deadlocks.is_empty() && !out.diagnostics.iter().any(|d| d.code == diag::W_DEADLOCK_0) {
            out.diagnostics.push(diag::Diagnostic {
                code: diag::W_DEADLOCK_0.into(),
                severity: diag::Severity::Warning,
                message: "a sampled nonterminal state has no enabled transition".into(),
                span: Some(m.process.span),
            });
        }
        if !out.diagnostics.iter().any(|d| d.code == diag::W_BUDGET_0)
            && sampled
                .replications
                .iter()
                .any(|r| r.cut == Some(outcome::SimulationCut::Numeric))
        {
            out.diagnostics.push(diag::Diagnostic {
                code: diag::W_BUDGET_0.into(),
                severity: diag::Severity::Warning,
                message: "probability denominator exceeded the exact arithmetic limit".into(),
                span: Some(m.process.span),
            });
        }
    }
    out.simulation = json!({"status":if aborted {"unsupported"} else {"completed"},
        "replications":n,"seed":spec.seed,"confidence":spec.confidence,
        "attempted_replications":sampled.attempted_replications,"completed_replications":sampled.completed_replications,
        "truncated_replications":sampled.truncated_replications,"interval_method":"clopper-pearson",
        "multiplicity":{"method":"bonferroni","rows":effective_rows,"alpha_before":alpha_before,"alpha_after":alpha_after},
        "random_method":"sha256-counter-v1","nondeterminism":"reject-on-encounter","deadlock_witnesses":deadlocks});
    if scope.is_some() {
        out.simulation["multiplicity"]["scope"] = json!("suite");
    }
    if m.horizon.is_some() {
        out.simulation["cut_off_replications"] = if aborted {
            serde_json::Value::Null
        } else {
            json!(
                sampled
                    .replications
                    .iter()
                    .filter(|r| r.cases.iter().any(|c| c.cut_off == Some(true)))
                    .count()
            )
        };
    }
    if m.has_integers() {
        out.simulation["range_stopped_replications"] = json!(sampled.range_stops.len());
    }
    // A structural question was refused before sampling. Do not manufacture
    // sampled property rows (or CP intervals) for questions outside this mode.
    if m.completion_requested() {
        out.verdict = Verdict::Unsupported;
        if out.quality.is_some() {
            place_quality_assumptions(&mut out.assumptions);
        }
        place_output_assumptions(
            &mut out.assumptions,
            out.quality.is_some(),
            out.metrics.is_some(),
        );
        place_read_write_assumptions(&mut out.assumptions, m);
        return out;
    }
    let a = alpha_before
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "none".into());
    let b = alpha_after
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "none".into());
    for (field, value, affects) in [
        (
            "simulation.random",
            "sha256-counter-v1; category-bias<=1/18446744073709551616".into(),
            "draws use keys formed from seed, replication, case, site and occurrence; each categorical probability has absolute discretization bias at most 2^-64; SHA-256 is not used for cryptographic purposes",
        ),
        (
            "simulation.replications",
            "independent; unit=replication".into(),
            "replications are independently initialized; cases sharing one replication are not independent statistical trials",
        ),
        (
            "simulation.multiplicity",
            if scope.is_some() {
                format!("scope=suite; bonferroni; rows={rows}; alpha_before={a}; alpha_after={b}")
            } else {
                format!("bonferroni; rows={rows}; alpha_before={a}; alpha_after={b}")
            },
            "two one-sided Clopper-Pearson errors are allocated to each probability-query case row; the reported confidence is simultaneous across these rows",
        ),
        (
            "simulation.nondeterminism",
            "reject-on-encounter".into(),
            "unresolved nondeterministic successor actions are not assigned probabilities or resolved by a fixed ordering",
        ),
        (
            "simulation.guarantee",
            "estimation-not-proof".into(),
            "statistical verdicts concern this model and load under the declared confidence; absence of sampled violations is not a safety proof",
        ),
    ] {
        out.assumptions.push(Assumption {
            field: field.into(),
            value,
            affects: affects.into(),
        });
    }
    if m.scenario.is_some()
        && let Some(a) = out
            .assumptions
            .iter_mut()
            .find(|a| a.field == "simulation.random")
    {
        a.affects.push_str("; scenario の名前は鍵に入らない");
    }
    if m.metrics.is_some() {
        out.assumptions.push(Assumption {
            field: "metrics.estimation".into(),
            value: "point-estimate-only; unit=replication".into(),
            affects: "点推定であり、全スケジュールの検証でも区間推定でもない。案件を独立な標本と数えず、反復ごとに観測して標本平均・最小・最大・標本数を出す。".into(),
        });
    }
    place_output_assumptions(
        &mut out.assumptions,
        out.quality.is_some(),
        out.metrics.is_some(),
    );
    let mut verdicts = Vec::new();
    let mut intervals = std::collections::BTreeMap::new();
    for verify in &m.verify {
        let id = match verify.property {
            ir::Property::NoLostUpdate(id) => m.loss_property_id(id),
            ir::Property::StateInRange => "state_in_range".into(),
            ir::Property::RecoverableEnd => "recoverable end".into(),
            ir::Property::TerminatingEnd => "terminating end".into(),
            ir::Property::AlmostSureEnd => "almost_sure end".into(),
            ir::Property::Contest(c) => crate::contest::property_id(m, c),
            ir::Property::ProbabilityReached(q) => format!("probability reached #{}", q + 1),
            ir::Property::ReachableEnd(e) => format!("reachable end.{}", m.ends[e.0]),
            ir::Property::Contract(c) => m.contracts[c.0].property_id(c),
            ir::Property::NoResourceOvercommit => "safety no_resource_overcommit".into(),
        };
        if let ir::Property::Contest(c) = verify.property {
            let verdict = crate::contest::verdict(m, c);
            out.properties
                .push(json!({"id":id,"verdict":verdict,"basis":"structural"}));
            verdicts.push(verdict);
            continue;
        }
        if verify.property == ir::Property::StateInRange {
            let witness = (!aborted).then(|| sampled.range_stops.first()).flatten();
            let verdict = if aborted {
                Verdict::Unsupported
            } else if witness.is_some() {
                Verdict::Violated
            } else {
                Verdict::Inconclusive
            };
            out.properties.push(json!({"id":id,"verdict":verdict,"basis":if witness.is_some(){"witness"}else{"sample"},"witness":witness.map(|(rep,_)|json!({"replication":rep}))}));
            verdicts.push(verdict);
            continue;
        }
        if matches!(
            verify.property,
            ir::Property::NoResourceOvercommit | ir::Property::NoLostUpdate(_)
        ) {
            let witness = (!aborted)
                .then(|| {
                    sampled.replications.iter().find(|r| match verify.property {
                        ir::Property::NoLostUpdate(v) => r.lost_updates.contains(&v),
                        _ => r.overcommit,
                    })
                })
                .flatten();
            let verdict = if aborted {
                Verdict::Unsupported
            } else if witness.is_some() {
                Verdict::Violated
            } else {
                Verdict::Inconclusive
            };
            out.properties.push(json!({"id":id,"verdict":verdict,"basis":if witness.is_some(){"witness"}else{"sample"},"witness":witness.map(|r|json!({"replication":r.replication}))}));
            verdicts.push(verdict);
            continue;
        }
        let mut cases = Vec::new();
        let mut cv = Vec::new();
        for (k, arrival) in times.iter().enumerate() {
            if aborted {
                cases.push(json!({"case_id":k+1,"arrival":arrival,"verdict":"unsupported","basis":"sample","witness":null}));
                cv.push(Verdict::Unsupported);
                continue;
            }
            if let ir::Property::ProbabilityReached(q) = verify.property {
                let hits = sampled
                    .replications
                    .iter()
                    .filter(|r| r.cases[k].queries[q] == QueryTruth::Hit)
                    .count() as u32;
                let unknown = sampled
                    .replications
                    .iter()
                    .filter(|r| r.cases[k].queries[q] == QueryTruth::Unknown)
                    .count() as u32;
                let alpha = alpha_after.as_ref().unwrap();
                let interval = intervals
                    .entry((hits, unknown))
                    .or_insert_with(|| crate::simulation::interval(n as u32, hits, unknown, alpha));
                let verdict = crate::simulation::statistical_verdict(
                    n as u32,
                    hits,
                    unknown,
                    &m.probability_queries[q],
                    alpha,
                );
                cases.push(json!({"case_id":k+1,"arrival":arrival,"verdict":verdict,"basis":"statistical","hits":hits,"unknown":unknown,"trials":n,"interval":interval}));
                cv.push(verdict);
            } else {
                let witness = sampled.replications.iter().find(|r| match verify.property {
                    ir::Property::ReachableEnd(e) => {
                        r.cases[k].terminal.as_ref().is_some_and(|t| {
                            matches!(
                                t.kind,
                                SimulationTerminalKind::End | SimulationTerminalKind::Overflow
                            ) && t.end.as_deref() == Some(m.ends[e.0].as_str())
                        })
                    }
                    ir::Property::Contract(c) => r.cases[k].violations.get(c.0) == Some(&true),
                    _ => false,
                });
                let verdict = match (witness.is_some(), verify.property) {
                    (true, ir::Property::ReachableEnd(_)) => Verdict::Satisfied,
                    (true, _) => Verdict::Violated,
                    _ => Verdict::Inconclusive,
                };
                cases.push(json!({"case_id":k+1,"arrival":arrival,"verdict":verdict,"basis":if witness.is_some(){"witness"}else{"sample"},"witness":witness.map(|r|json!({"replication":r.replication}))}));
                cv.push(verdict);
            }
        }
        if m.arrival_distribution.is_some() {
            for (k, case) in cases.iter_mut().enumerate() {
                case["arrival"] = json!(null);
                case["arrival_time"] = sampled_arrival(&sampled, k, aborted);
            }
        }
        let verdict = cv
            .into_iter()
            .max_by_key(|v| precedence(*v))
            .unwrap_or(Verdict::Unsupported);
        out.properties
            .push(json!({"id":id,"verdict":verdict,"cases":cases}));
        if let Some(notes) = property_notes(m, verify.property) {
            out.properties.last_mut().unwrap()["notes"] = json!(notes);
        }
        verdicts.push(verdict);
    }
    let mut rejected = 0u64;
    for (k, arrival) in times.iter().enumerate() {
        if aborted {
            out.case_outcomes.push(json!({"case_id":k+1,"arrival":arrival,"terminals":[],"unfinished_count":null,"deadlocked_count":null}));
            if m.arrival_distribution.is_some() {
                let row = out.case_outcomes.last_mut().unwrap();
                row["arrival"] = json!(null);
                row["arrival_time"] = sampled_arrival(&sampled, k, true);
            }
            if m.horizon.is_some() {
                out.case_outcomes.last_mut().unwrap()["cut_off_count"] = serde_json::Value::Null;
            }
            continue;
        }
        let mut terminals = Vec::new();
        for end in &m.ends {
            let count = sampled
                .replications
                .iter()
                .filter(|r| {
                    r.cases[k].terminal.as_ref().is_some_and(|t| {
                        matches!(
                            t.kind,
                            SimulationTerminalKind::End | SimulationTerminalKind::Overflow
                        ) && t.end.as_ref() == Some(end)
                    })
                })
                .count();
            if count > 0 {
                terminals.push(json!({"kind":if m.overflow_end.is_some_and(|e| &m.ends[e.0] == end) { "overflow" } else { "end" },"end":end,"count":count}));
            }
        }
        let count = sampled
            .replications
            .iter()
            .filter(|r| {
                r.cases[k]
                    .terminal
                    .as_ref()
                    .is_some_and(|t| t.kind == SimulationTerminalKind::Rejected)
            })
            .count();
        rejected += count as u64;
        if count > 0 {
            terminals.push(json!({"kind":"rejected","end":null,"count":count}));
        }
        let receipt_unknown = sampled
            .replications
            .iter()
            .filter(|r| {
                r.cases[k]
                    .terminal
                    .as_ref()
                    .is_some_and(|t| t.kind == SimulationTerminalKind::ReceiptUnknown)
            })
            .count();
        if receipt_unknown > 0 {
            terminals.push(json!({"kind":"receipt_unknown","end":null,"count":receipt_unknown}));
        }
        let deadlocked = sampled
            .replications
            .iter()
            .filter(|r| r.cases[k].terminal.is_none() && r.cases[k].deadlock.is_some())
            .count();
        let range_stopped = sampled
            .range_stops
            .iter()
            .filter(|(_, rs)| rs.iter().any(|r| r.case_id.unwrap_or(1) == k as u64 + 1))
            .count();
        let unfinished = receipt_unknown
            + sampled
                .replications
                .iter()
                .filter(|r| {
                    r.cases[k].terminal.is_none()
                        && r.cases[k].cut_off != Some(true)
                        && r.cases[k].deadlock.is_none()
                        && !sampled.range_stops.iter().any(|(rep, rs)| {
                            *rep == r.replication
                                && rs.iter().any(|w| w.case_id.unwrap_or(1) == k as u64 + 1)
                        })
                })
                .count();
        out.case_outcomes.push(json!({"case_id":k+1,"arrival":arrival,"terminals":terminals,"unfinished_count":unfinished,"deadlocked_count":deadlocked}));
        if m.arrival_distribution.is_some() {
            let row = out.case_outcomes.last_mut().unwrap();
            row["arrival"] = json!(null);
            row["arrival_time"] = sampled_arrival(&sampled, k, false);
        }
        if m.horizon.is_some() {
            out.case_outcomes.last_mut().unwrap()["cut_off_count"] = json!(
                sampled
                    .replications
                    .iter()
                    .filter(|r| r.cases[k].cut_off == Some(true))
                    .count()
            );
        }
        if m.has_integers() {
            out.case_outcomes.last_mut().unwrap()["range_stopped_count"] = json!(range_stopped);
        }
    }
    if !m.response_attempts().is_empty() && !aborted {
        out.properties
            .push(json!({"id":"receipt_resolution","verdict":"inconclusive"}));
        verdicts.push(Verdict::Inconclusive);
    }
    if !aborted {
        out.admission = Some(json!({"rejected_count":rejected}));
        for (key, enabled) in [
            ("overflow_count", m.overflow_end.is_some()),
            ("external_wait_max", m.external_wait()),
        ] {
            if !enabled {
                continue;
            }
            let ranges: Vec<_> = sampled
                .replications
                .iter()
                .filter_map(|r| r.admission.as_ref())
                .filter_map(|a| {
                    if key == "overflow_count" {
                        a.overflow_count
                    } else {
                        a.external_wait_max
                    }
                })
                .collect();
            if let (Some(min), Some(max)) = (
                ranges.iter().map(|r| r.min).min(),
                ranges.iter().map(|r| r.max).max(),
            ) {
                out.admission.as_mut().unwrap()[key] = json!({"min":min,"max":max});
            }
        }
    }
    if m.recovery().is_some() && out.response_recovery.is_none() {
        out.diagnostics.push(recovery_transport_pending(m));
        verdicts.push(Verdict::Unsupported);
    }
    out.verdict = if aborted && m.responsibility.is_none() {
        Verdict::Unsupported
    } else {
        verdicts
            .into_iter()
            .chain(aborted.then_some(Verdict::Unsupported))
            .chain((!deadlocks.is_empty()).then_some(Verdict::Violated))
            .max_by_key(|v| precedence(*v))
            .unwrap_or(Verdict::Unsupported)
    };
    place_read_write_assumptions(&mut out.assumptions, m);
    out
}

#[cfg(test)]
mod simulation_tests {
    use super::*;
    #[test]
    fn s_overcommit_snapshot_is_a_violation_with_replication_witness() {
        let source = include_str!("../../../fixtures/sim-deadlock/model.relca");
        let out = simulate_using(source, None, |m| {
            let mut out = engine::simulate(m);
            out.replications[0].overcommit = true;
            out
        });
        assert_eq!(out.verdict, Verdict::Violated);
        let safety = out
            .properties
            .iter()
            .find(|p| p["id"] == "safety no_resource_overcommit")
            .unwrap();
        assert_eq!(safety["basis"], "witness");
        assert_eq!(safety["witness"]["replication"], 1);
        assert_eq!(safety["verdict"], "violated");
    }
}

/// ADR 0008 selection; the source hash and ADR 0002 wire stay unchanged.
pub fn check_configuration(src: &str, configuration: &str) -> Report {
    check_selected(src, configuration).0
}

pub(crate) fn check_selected(
    src: &str,
    configuration: &str,
) -> (Report, Option<lower::SelectedConfiguration>) {
    let (report, selected) = check_selected_using(src, configuration, engine::explore);
    (
        finite_driver(report, engine::DEFAULT_STATE_BUDGET),
        selected,
    )
}

pub(crate) fn check_selected_using(
    src: &str,
    configuration: &str,
    execute: impl FnOnce(&ir::Model) -> outcome::Outcome,
) -> (Report, Option<lower::SelectedConfiguration>) {
    check_selected_scenario_using(src, configuration, None, execute)
}

pub(crate) fn check_selected_scenario_using(
    src: &str,
    configuration: &str,
    scenario: Option<&str>,
    execute: impl FnOnce(&ir::Model) -> outcome::Outcome,
) -> (Report, Option<lower::SelectedConfiguration>) {
    let mut report = empty_report(src);
    report.ledger.requested(Some(configuration), scenario);
    let ast = match syntax::parse(src) {
        Ok(ast) => ast,
        Err(diagnostics) => return (finish(report, diagnostics), None),
    };
    report.model.name = Some(ast.name.value.clone());
    if ast.has_condition_wait()
        || ast.items.iter().any(|i| {
            matches!(
                i,
                ast::Item::Calendar(_) | ast::Item::ArrivalsDistribution(_)
            )
        })
    {
        // ADR 0059's refusal inventory is explicitly empty, not an observed
        // cohort. Legacy syntax retains its historical omitted field.
        report.case_outcomes = Some(Vec::new());
    }
    if ast.items.iter().any(|i| matches!(i, ast::Item::Cut(_))) {
        report.cut = Cut::Status(CutStatus::Unresolved);
    }
    let selected = match lower::lower_configuration_with_scenario(&ast, configuration, scenario) {
        Ok(selected) => selected,
        Err(diagnostics) => return (finish(report, diagnostics), None),
    };
    if let Err(diagnostic) = work_profiles::attach(&mut report, &selected.model, src) {
        return (finish(report, vec![diagnostic]), Some(selected));
    }
    let report = analyze_model(report, &selected.model, execute);
    (report, Some(selected))
}

/// The ordinary check of an already parsed (mutated) AST: only parsing is
/// replaced; lowering and analysis are the same as `check` (ADR 0020).
pub(crate) fn check_ast_using(
    src: &str,
    model: &ast::Model,
    execute: impl FnOnce(&ir::Model) -> outcome::Outcome,
) -> Report {
    check_with(src, |_| Ok(model.clone()), lower::lower, execute)
}

pub(crate) fn check_ast_with_scenario_using(
    src: &str,
    model: &ast::Model,
    scenario: Option<&str>,
    execute: impl FnOnce(&ir::Model) -> outcome::Outcome,
) -> Report {
    if scenario.is_none() {
        return check_ast_using(src, model, execute);
    }
    let mut report = check_with(
        src,
        |_| Ok(model.clone()),
        |a| lower::lower_with_selection(a, None, scenario),
        execute,
    );
    report.ledger.requested(None, scenario);
    seal(report)
}

// Private injection seam keeps the pipeline tests independent of other components.
fn check_with(
    src: &str,
    parse: impl FnOnce(&str) -> Result<ast::Model, Vec<diag::Diagnostic>>,
    lower: impl FnOnce(&ast::Model) -> Result<ir::Model, Vec<diag::Diagnostic>>,
    explore: impl FnOnce(&ir::Model) -> outcome::Outcome,
) -> Report {
    let mut report = empty_report(src);
    let ast = match parse(src) {
        Ok(ast) => ast,
        Err(diagnostics) => return finish(report, diagnostics),
    };
    report.model.name = Some(ast.name.value.clone());
    if ast.items.iter().any(|i| matches!(i, ast::Item::Cut(_))) {
        report.cut = Cut::Status(CutStatus::Unresolved);
    }
    if ast.has_condition_wait()
        || ast.items.iter().any(|i| {
            matches!(
                i,
                ast::Item::Calendar(_) | ast::Item::ArrivalsDistribution(_)
            )
        })
    {
        report.case_outcomes = Some(Vec::new());
    }
    let model = match lower(&ast) {
        Ok(model) => model,
        Err(diagnostics) => return finish(report, diagnostics),
    };
    if let Err(diagnostic) = work_profiles::attach(&mut report, &model, src) {
        return finish(report, vec![diagnostic]);
    }
    analyze_model(report, &model, explore)
}

fn analyze_model(
    mut report: Report,
    model: &ir::Model,
    explore: impl FnOnce(&ir::Model) -> outcome::Outcome,
) -> Report {
    let admission = preflight::finite(model);
    report.ledger.lowered(model);
    report.resource_time = resource_time_record(model);
    report.priority_policy = priority::record(model);
    report.model.name = Some(model.name.clone());
    let prepare = |report: &mut Report| {
        report.actors = Some(crate::actor_table::project(model));
        report.uncertainty = crate::uncertainty::Uncertainty::from_model(model);
        report.cut_context = CutContext::resolved(model);
        report.via_model_paths = crate::agency::via_model_paths(model);
        if let Some(cut) = &model.cut {
            report.cut = Cut::Declared(Box::new(declared_cut(model, cut)));
        }
        report.assumptions = model.assumptions.clone();
        if model.contests().iter().any(|c| c.rates.is_some()) {
            report.assumptions.push(Assumption {
            field: "metrics.contests".into(),
            value: "not_provided".into(),
            affects: "checkは異議の標本集約を提供しない。根拠付き起票の正支持上で契約を検査し、標本の率と時間はsimulateでのみ集約する。".into(),
        });
        }
        authority_record(report, model);
        report.bounds = Bounds {
            range_violation_stops: None,
            cases: Some(model.bounds.cases),
            live_tasks: Some(model.bounds.live_tasks),
            clock: Some(model.bounds.clock),
            truncated: None,
        };
    };
    let prepare_analysis = |report: &mut Report| {
        report.diagnostics.extend(model.diagnostics.clone());
        report.agency = Some(model.agency.clone());
        report.responsibilities = model
            .responsibility
            .as_ref()
            .map(|m| m.responsibilities.clone());
        report.contest_paths = model
            .responsibility
            .as_ref()
            .map(|m| m.contest_paths.clone());
    };
    match admission {
        Err(failure @ preflight::FiniteFailure::Actor(_)) => {
            return analysis_refusal(report, model, failure.diagnostics(model));
        }
        Err(failure @ preflight::FiniteFailure::StochasticArrivals) => {
            prepare(&mut report);
            report.case_outcomes = Some(Vec::new());
            return analysis_refusal(report, model, failure.diagnostics(model));
        }
        Err(failure @ preflight::FiniteFailure::Diagnostics) => {
            prepare(&mut report);
            return analysis_refusal(report, model, failure.diagnostics(model));
        }
        Err(failure @ preflight::FiniteFailure::NoVerify) => {
            prepare(&mut report);
            prepare_analysis(&mut report);
            return analysis_refusal(report, model, failure.diagnostics(model));
        }
        Ok(()) => {
            prepare(&mut report);
            prepare_analysis(&mut report);
        }
    }
    report.ledger.unknown_driver_called();
    let outcome = explore(model);
    report.scenario = scenario_record(
        model,
        serde_json::to_value(&outcome.scenario_observed_until).expect("observation wire"),
    );
    apply_outcome(report, model, outcome)
}

fn empty_report(src: &str) -> Report {
    let mut sha256 = String::with_capacity(64);
    for byte in Sha256::digest(src.as_bytes()) {
        write!(sha256, "{byte:02x}").expect("writing into a String cannot fail");
    }
    Report {
        priority_policy: None,
        work_profiles: None,
        resource_time: None,
        ledger: crate::ledger::Ledger::prelower("exhaustive", false),
        uncertainty: crate::uncertainty::Uncertainty::not_lowered(),
        response_receipts: None,
        response_recovery: None,
        response_analysis: None,
        scenario: None,
        quality: None,
        state_range_violations: Vec::new(),
        capability_refusals: None,
        unchecked: Vec::new(),
        actors: None,
        cut_context: CutContext::default(),
        via_model_paths: None,
        contest_runtime: None,
        contest_summary: None,
        obligations: None,
        metrics: None,
        responsibilities: None,
        contest_paths: None,
        case_outcomes: syntax::has_priority_declaration(src).then(Vec::new),
        admission: None,
        agency: None,
        verdict: Verdict::Unsupported,
        mode: Mode::Exhaustive,
        unit: Unit::Min,
        model: ModelMetadata {
            name: None,
            sha256: Some(sha256),
        },
        cut: Cut::UNDECLARED,
        assumptions: Vec::new(),
        diagnostics: Vec::new(),
        bounds: Bounds {
            range_violation_stops: None,
            cases: None,
            live_tasks: None,
            clock: None,
            truncated: None,
        },
        properties: Vec::new(),
        terminals: Vec::new(),
    }
}

#[cfg(test)]
mod actor_metadata_tests {
    use super::*;
    #[test]
    fn ledger_unknown_and_empty_versions_still_fail_closed() {
        let source = include_str!("../../../fixtures/serial-time/model.relca");
        let control = serde_json::to_value(check(source)).unwrap();
        for version in ["unknown", ""] {
            let mut bad = control.clone();
            bad["ledger"]["producer"]["version"] = version.into();
            assert!(serde_json::from_value::<Report>(bad).is_err());
            eprintln!("rejected producer version {version:?}");
            let mut bad = control.clone();
            bad["agency"]["rules"] = version.into();
            let components = bad["ledger"]["derivation"]["components"]
                .as_array_mut()
                .unwrap();
            let component = components
                .iter_mut()
                .find(|c| c["at"] == "/agency/rules")
                .unwrap();
            component["version"] = version.into();
            assert!(serde_json::from_value::<Report>(bad).is_err());
            eprintln!("rejected registered local version {version:?}");
        }
    }
    #[test]
    fn ledger_does_not_treat_a_declared_process_name_as_an_unknown_version() {
        let source = include_str!("../../../fixtures/serial-time/model.relca");
        let renamed = source.replace("process Check", "process unknown");
        assert_ne!(source, renamed);
        let r = check(&renamed);
        assert_eq!(r.verdict, Verdict::Satisfied);
        assert_eq!(r.terminals[0].elapsed, Range { min: 8, max: 8 });
        assert_eq!(
            r.ledger.execution["selection"]["effective"]["process"],
            crate::ledger::known("unknown")
        );
    }
    #[test]
    fn ledger_decoder_rejects_known_selection_and_bound_type_faults() {
        let src = include_str!("../../../fixtures/serial-time/model.relca");
        let control = serde_json::to_value(check(src)).unwrap();
        for (path, value) in [
            (
                "/ledger/execution/selection/effective/process/value",
                serde_json::json!(9),
            ),
            ("/ledger/execution/bounds/value", serde_json::json!(false)),
        ] {
            let mut bad = control.clone();
            *bad.pointer_mut(path).unwrap() = value;
            assert!(
                serde_json::from_value::<Report>(bad).is_err(),
                "accepted {path}"
            );
        }
    }
    #[test]
    fn ledger_decode_missing_version_must_not_take_package_default() {
        let source = include_str!("../../../fixtures/serial-time/model.relca");
        let mut v = serde_json::to_value(check(source)).unwrap();
        v["ledger"]["producer"]
            .as_object_mut()
            .unwrap()
            .remove("version");
        assert!(serde_json::from_value::<Report>(v).is_err());
    }
    #[test]
    fn ledger_emission_fault_is_nonzero_and_keeps_independent_failures() {
        let src = include_str!("../../../fixtures/serial-time/model.relca");
        for verdict in [Verdict::Satisfied, Verdict::Invalid, Verdict::Violated] {
            let mut r = check(src);
            r.verdict = verdict;
            r.ledger.producer["version"] = "".into();
            let r = seal(r);
            assert_eq!(
                r.verdict,
                if verdict == Verdict::Satisfied {
                    Verdict::Unsupported
                } else {
                    verdict
                }
            );
            assert_ne!(r.verdict.exit_code(), 0);
            let d = r
                .diagnostics
                .iter()
                .find(|d| d.code == diag::U_LEDGER_0)
                .unwrap();
            assert_eq!(d.severity, diag::Severity::Error);
            assert_eq!(d.span, None);
            assert!(d.message.starts_with("version_unavailable"));
        }
    }
    #[test]
    fn ledger_budget_zero_and_unknown_injected_driver_are_not_defaults() {
        let src = include_str!("../../../fixtures/serial-time/model.relca");
        let calls = std::cell::Cell::new(0);
        let report = check_using(src, |m| {
            calls.set(calls.get() + 1);
            engine::explore_with_budget(m, 0)
        });
        assert_eq!(calls.get(), 1);
        assert_eq!(report.bounds.truncated, Some(true));
        assert_eq!(report.ledger.execution["status"], "unavailable");
        assert_eq!(
            report.ledger.execution["limits"]["state_budget"],
            crate::ledger::unavailable("execution_settings_unavailable")
        );
        let report = finite_driver(report, 0);
        assert_eq!(report.verdict, Verdict::Inconclusive);
        assert_eq!(
            report.ledger.execution["limits"]["state_budget"],
            crate::ledger::known(0)
        );
        assert_eq!(report.ledger.execution["status"], "ran");
        assert_eq!(calls.get(), 1);
    }
    #[test]
    fn actor_table_and_resolved_cut_survive_zero_and_one_state_budgets() {
        for source in [
            include_str!("../../../fixtures/act-via/model.relca"),
            include_str!("../../../fixtures/act-horizon/model.relca"),
        ] {
            let full = check(source);
            for budget in [0, 1] {
                let small = check_using(source, |m| engine::explore_with_budget(m, budget));
                assert_eq!(small.actors, full.actors);
                assert_eq!(small.cut, full.cut);
                assert_eq!(small.cut_context, full.cut_context);
                assert_eq!(small.via_model_paths, full.via_model_paths);
                assert_eq!(small.bounds.truncated, Some(true));
                assert_eq!(small.verdict, Verdict::Inconclusive);
            }
        }
    }
}

fn authority_record(report: &mut Report, model: &ir::Model) {
    if model
        .authorities
        .iter()
        .any(|a| a.restrictions.revocable_by.is_some())
        && !report
            .unchecked
            .iter()
            .any(|s| s == "dynamic_authority_revocation")
    {
        report.unchecked.push("dynamic_authority_revocation".into());
    }
}

fn quality_estimation() -> Assumption {
    Assumption {field:"quality.estimation".into(),value:"ratio_of_sums_per_replication".into(),affects:"ΣN/ΣDの点推定。標本は反復単位であり案件を独立標本としない。expected_countは既知hit件数、unknown_countは未確定件数の別標本。観測結末は観測停止時点の分類であり将来の到達確率ではない。区間推定でも全スケジュールの検証でもない。".into()}
}

/// A simulate refusal after quality was lowered (ADR 0045 boundary review M1):
/// the rows the quality-free model would publish here, plus the main path's
/// four quality rows (the lowered three and quality.estimation) as one block.
/// Existing non-quality rows are preserved, including the legacy revocation
/// branch's Authority and metrics siblings.
fn quality_refusal_rows(rows: &mut Vec<Assumption>, m: &ir::Model) {
    rows.retain(|row| !row.field.starts_with("quality."));
    rows.extend(
        m.assumptions
            .iter()
            .filter(|row| row.field.starts_with("quality."))
            .cloned(),
    );
    rows.push(quality_estimation());
    place_quality_assumptions(rows);
}

fn simulation_refusal(
    mut base: Report,
    model: &ir::Model,
    diagnostics: Vec<diag::Diagnostic>,
) -> SimulationReport {
    if model.quality.is_some() {
        base.quality = Some(crate::quality::simulation(model, &[], true));
        quality_refusal_rows(&mut base.assumptions, model);
    }
    simulation_empty(finish(base, diagnostics))
}

fn quality_scheduler() -> Assumption {
    Assumption {field:"quality.scheduler".into(),value:"per_case_relaxation".into(),affects:"案件別のmin/maxの和は同時の極値ではない。未確定では保守的外端点を示すが確定した期待件数とは呼ばない。計算切断はtruncated、業務打切りだけundeterminedとし、性質ごとに確定判定する。観測結末と性質の将来到達の未知を分ける。".into()}
}

fn analysis_refusal(
    mut report: Report,
    model: &ir::Model,
    diagnostics: Vec<diag::Diagnostic>,
) -> Report {
    if let Some(spec) = &model.quality {
        use crate::{probability::Rational, quality::Solution};
        let unknown = Solution::new(
            [
                Rational::zero(),
                Rational::one(),
                Rational::zero(),
                Rational::one(),
            ],
            true,
        );
        report.quality = Some(crate::quality::exhaustive(
            model,
            &vec![vec![unknown.clone(); model.bounds.cases as usize]; spec.properties.len()],
            &vec![
                vec![unknown; model.bounds.cases as usize];
                crate::quality::outcome_keys(model).len()
            ],
            true,
        ));
        report
            .assumptions
            .retain(|row| !row.field.starts_with("quality."));
        report.assumptions.extend(
            model
                .assumptions
                .iter()
                .filter(|row| row.field.starts_with("quality."))
                .cloned(),
        );
        report.assumptions.push(quality_scheduler());
    }
    place_output_assumptions(&mut report.assumptions, report.quality.is_some(), false);
    finish(report, diagnostics)
}

// A disclosure explains published output, not just an IR opt-in. All exits
// with known quality definitions share this ordering, including refusals.
fn place_output_assumptions(rows: &mut Vec<Assumption>, quality: bool, metrics: bool) {
    if quality {
        place_quality_assumptions(rows);
    }
    if metrics {
        place_waiting_assumption(rows);
    } else {
        rows.retain(|row| row.field != "metrics.waiting");
    }
    // Append the disclosure after derived rows so existing assumption indices
    // and their uncertainty references retain their meanings.
    if let Some(at) = rows.iter().position(|row| row.field == "metrics.contests") {
        let disclosure = rows.remove(at);
        rows.push(disclosure);
    }
}

fn place_read_write_assumptions(rows: &mut Vec<Assumption>, model: &ir::Model) {
    let at = read_write_anchor(rows, model);
    rows.splice(at..at, lower::read_write_assumptions(model));
}

fn read_write_anchor(rows: &[Assumption], model: &ir::Model) -> usize {
    if let Some(last) = rows.iter().rposition(|r| r.field.ends_with(".concurrency")) {
        return last + 1;
    }
    // lower::assumptions appends concurrency after its base-model declarations,
    // before finish adds obligations, arrivals and output-specific blocks. With
    // no resolution row, the same slot is the end of that base prefix: schedule,
    // initial values, revocation records, kernels, durations and instances.
    let mut base = std::collections::BTreeSet::from([
        "schedule".to_owned(),
        "initial.actor_case_independence".to_owned(),
    ]);
    for variable in &model.state_variables {
        if matches!(
            variable.initial,
            ir::InitialValue::Nondet | ir::InitialValue::Distribution(_)
        ) {
            let prefix = match variable.kind {
                ir::StateKind::ActorLatent(actor) => model.actors[actor.0].name.as_str(),
                _ => "latent",
            };
            base.insert(format!("{prefix}.{}", variable.name));
        }
    }
    for authority in &model.authorities {
        if authority.restrictions.revocable_by.is_some() {
            base.insert(format!("authority.{}.revocable_by", authority.name));
        }
    }
    for kernel in &model.kernels {
        base.insert(format!("kernel.{}", kernel.name));
    }
    for capability in &model.capabilities {
        if capability.duration_distribution.is_some() {
            base.insert(format!(
                "duration.{}.{}",
                model.actors[capability.actor.0].name, capability.name
            ));
        }
    }
    for actor in &model.actors {
        base.insert(format!("{}.instance", actor.name));
    }
    rows.iter()
        .rposition(|r| base.contains(&r.field))
        .map_or(0, |i| i + 1)
}

fn place_quality_assumptions(rows: &mut Vec<Assumption>) {
    let mut quality = Vec::new();
    let mut quantile = Vec::new();
    rows.retain(|row| {
        if row.field == "quality.quantile" {
            quantile.push(row.clone());
            false
        } else if row.field.starts_with("quality.") {
            quality.push(row.clone());
            false
        } else {
            true
        }
    });
    let position = rows
        .iter()
        .position(|row| row.field == "probability.scheduler")
        .unwrap_or(rows.len());
    // Retain the pre-C7 position of the old quality block, including after
    // simulation's rows. Waiting is placed after quality in a separate pass.
    rows.splice(position..position, quality);
    let position = rows
        .iter()
        .position(|row| row.field == "probability.scheduler")
        .unwrap_or(rows.len());
    rows.splice(position..position, quantile);
}

fn place_waiting_assumption(rows: &mut Vec<Assumption>) {
    if let Some(i) = rows.iter().position(|a| a.field == "metrics.waiting") {
        let row = rows.remove(i);
        let at = rows
            .iter()
            .position(|a| {
                matches!(
                    a.field.as_str(),
                    "quality.quantile" | "probability.scheduler"
                )
            })
            .unwrap_or(rows.len());
        rows.insert(at, row);
    }
}

fn apply_outcome(mut mut_report: Report, model: &ir::Model, outcome: outcome::Outcome) -> Report {
    let response_exercise = outcome.response_exercise.clone();
    mut_report.response_receipts = outcome.response_receipts.clone();
    mut_report.response_recovery = outcome.response_recovery.clone();
    let response_incomplete = outcome.truncated
        || !outcome.state_range_violations.is_empty()
        || outcome
            .diagnostics
            .iter()
            .chain(&outcome.postprocessing_diagnostics)
            .any(|d| d.severity == diag::Severity::Error);
    if outcome.response_receipts.is_some() {
        mut_report.response_analysis = Some(Box::new(response::analysis(
            model,
            crate::obligation::Coverage::of(
                outcome
                    .diagnostics
                    .iter()
                    .chain(&outcome.postprocessing_diagnostics)
                    .any(|d| d.severity == diag::Severity::Error),
                outcome.truncated,
                !outcome.state_range_violations.is_empty(),
            ),
        )));
    }
    mut_report.quality = outcome.quality.clone();
    if mut_report.quality.is_some() {
        mut_report.assumptions.push(quality_scheduler());
    }
    mut_report.unchecked = outcome.unchecked.clone();
    authority_record(&mut mut_report, model);
    mut_report.contest_runtime = outcome.contest_runtime.clone();
    mut_report.contest_summary = outcome.contest_summary.clone();
    mut_report.obligations = outcome.obligations.clone();
    if mut_report
        .obligations
        .as_ref()
        .is_some_and(|rows| !rows.is_empty())
        && let Some(agency) = &mut mut_report.agency
    {
        // ADR 0042 integration ruling (kept by ADR 0043's integration (c)):
        // obligation_debtor is appended once, after every lower marker,
        // including C5's responsibility_people_undeclared.
        agency.unchecked.push("obligation_debtor".into());
    }
    mut_report.metrics = outcome.metrics.clone();
    // Alternate exhaustive drivers (notably explain's trace collector) also
    // pass this boundary. Projection is idempotent and never alters the trace.
    if let Some(metrics) = &mut mut_report.metrics {
        metrics.apply_deferred(model);
    }
    if let Some(quality) = &mut mut_report.quality {
        quality.apply_deferred(model);
    }
    mut_report.capability_refusals = outcome.capability_refusals.clone();
    mut_report.state_range_violations = outcome.state_range_violations.clone();
    mut_report.bounds.range_violation_stops =
        (!outcome.state_range_violations.is_empty()).then_some(true);
    if outcome.probability_scheduler {
        mut_report.assumptions.push(Assumption {
            field: "probability.scheduler".into(),
            value: if outcome.probability_policy_unresolved == Some(false) { "fixed-policy" } else { "full-information" }.into(),
            affects: if outcome.probability_policy_unresolved == Some(false) { "方策の区分の選択は宣言した固定方策で決まる。min/maxは残りの環境の選択の全ての解き方での幅であり、環境は全状態を見られる。方策の最適値ではない。" } else { "min/max は完全情報の緩和境界で、実現できる方策の値ではない" }.into(),
        });
    }
    // As in simulation, finish the quality block before placing waiting:
    // existing rows (including quality) -> waiting -> quantile / scheduler.
    place_output_assumptions(
        &mut mut_report.assumptions,
        mut_report.quality.is_some(),
        mut_report.metrics.is_some(),
    );
    place_read_write_assumptions(&mut mut_report.assumptions, model);
    // An engine error aborted exploration: whether it would have been cut is
    // unknown, so it is null like an unanalyzed model, never false.
    let aborted = outcome
        .diagnostics
        .iter()
        .any(|d| d.severity == diag::Severity::Error);
    mut_report.bounds.truncated = (!aborted).then_some(outcome.truncated);
    mut_report.properties = outcome
        .properties
        .into_iter()
        .map(|property| Property {
            notes: property_notes(model, property.property),
            cases: property.cases.map(|cs| {
                cs.into_iter()
                    .map(|c| CaseProperty {
                        case_id: u64::try_from(c.case_id.0).expect("arrival index fits u64") + 1,
                        arrival: c.arrival,
                        verdict: c.verdict,
                        probability: c.probability,
                    })
                    .collect()
            }),
            probability: property.probability,
            id: match property.property {
                ir::Property::NoLostUpdate(id) => model.loss_property_id(id),
                ir::Property::StateInRange => "state_in_range".into(),
                ir::Property::RecoverableEnd => "recoverable end".into(),
                ir::Property::TerminatingEnd => "terminating end".into(),
                ir::Property::AlmostSureEnd => "almost_sure end".into(),
                ir::Property::Contest(id) => crate::contest::property_id(model, id),
                ir::Property::ProbabilityReached(id) => format!("probability reached #{}", id + 1),
                ir::Property::NoResourceOvercommit => "safety no_resource_overcommit".into(),
                ir::Property::ReachableEnd(end) => format!("reachable end.{}", model.ends[end.0]),
                ir::Property::Contract(id) => model.contracts[id.0].property_id(id),
            },
            verdict: property.verdict,
        })
        .collect();
    mut_report.terminals = outcome
        .terminals
        .into_iter()
        .map(|terminal| Terminal {
            probability: terminal.probability,
            end: model.ends[terminal.end.0].clone(),
            elapsed: range(terminal.elapsed),
            work: range(terminal.work),
            result_at: terminal.result_at.map(range),
        })
        .collect();
    mut_report.case_outcomes = outcome.case_outcomes.map(|cs| {
        cs.into_iter()
            .map(|c| CaseOutcome {
                admitted_at: c.admitted_at.map(|at| at.map(range)),
                case_id: u64::try_from(c.case_id.0).expect("arrival index fits u64") + 1,
                arrival: c.arrival,
                terminals: c
                    .terminals
                    .into_iter()
                    .map(|t| {
                        let (kind, end) = match t.kind {
                            outcome::CaseTerminalKind::ReceiptUnknown => {
                                (CaseTerminalKind::ReceiptUnknown, None)
                            }
                            outcome::CaseTerminalKind::CutOff => (CaseTerminalKind::CutOff, None),
                            outcome::CaseTerminalKind::End(id) => {
                                (CaseTerminalKind::End, Some(model.ends[id.0].clone()))
                            }
                            outcome::CaseTerminalKind::Overflow(id) => {
                                (CaseTerminalKind::Overflow, Some(model.ends[id.0].clone()))
                            }
                            outcome::CaseTerminalKind::Rejected => {
                                (CaseTerminalKind::Rejected, None)
                            }
                        };
                        CaseTerminal {
                            observed_until: t.observed_until.map(range),
                            observed_elapsed: t.observed_elapsed.map(range),
                            observed_work: t.observed_work.map(range),
                            kind,
                            end,
                            completed_at: t.completed_at.map(range),
                            elapsed: t.elapsed.map(range),
                            work: t.work.map(range),
                            result_at: t.result_at.map(range),
                            probability: t.probability,
                        }
                    })
                    .collect(),
            })
            .collect()
    });
    mut_report.admission = outcome.admission.map(|a| AdmissionSummary {
        rejected_count: range(a.rejected_count),
        overflow_count: a.overflow_count.map(range),
        external_wait_max: a.external_wait_max.map(range),
    });
    if mut_report.response_receipts.is_some() {
        let cases: Vec<_> = mut_report
            .case_outcomes
            .as_ref()
            .into_iter()
            .flatten()
            .map(|c| CaseProperty {
                case_id: c.case_id,
                arrival: c.arrival,
                probability: None,
                verdict: if response_incomplete
                    || mut_report.response_recovery.as_ref().is_some_and(|r| {
                        r.paths.iter().flat_map(|p| &p.commands).any(|r| {
                            r.case_id == c.case_id
                                && (r.unresolved || r.stop_reason == crate::recovery::Stop::CutOff)
                        })
                    })
                    || c.terminals.iter().any(|t| {
                        matches!(
                            t.kind,
                            CaseTerminalKind::ReceiptUnknown | CaseTerminalKind::CutOff
                        )
                    }) {
                    Verdict::Inconclusive
                } else {
                    Verdict::Satisfied
                },
            })
            .collect();
        let unresolved = cases.iter().any(|c| c.verdict == Verdict::Inconclusive)
            || response_incomplete
            || mut_report
                .response_recovery
                .as_ref()
                .is_some_and(crate::recovery::Recovery::unsettled);
        mut_report.properties.push(Property {
            id: "receipt_resolution".into(),
            verdict: if unresolved {
                Verdict::Inconclusive
            } else {
                Verdict::Satisfied
            },
            cases: Some(cases),
            probability: None,
            notes: None,
        });
        for (_, r) in model
            .response_attempts()
            .into_iter()
            .filter(|(_, r)| r.attempt == 1)
        {
            mut_report.assumptions.push(Assumption { field:format!("response.{}", model.site_name(r.command)), value:format!("max_attempts={}; backoff={}; loss={:?}",r.max_attempts,r.backoff,r.loss), affects:if model.recovery().is_some(){"retry_bounded; exact modeled G finish-time query and evidence matching only; no general status query, asynchronous task, correlated channel or non-idempotent recovery inference"}else{"retry_bounded; receipt_channel; receipt_not_success; atomic server-finish channel only; no asynchronous task, status query or correlated channel inference"}.into() });
        }
    }
    let mut final_report = finish(
        mut_report,
        outcome
            .diagnostics
            .into_iter()
            .chain(outcome.postprocessing_diagnostics)
            .collect(),
    );
    crate::agency::response::apply(&mut final_report, model, response_exercise.as_deref());
    if model.recovery().is_some() && final_report.response_recovery.is_none() {
        final_report.response_analysis = Some(Box::new(response::unavailable_transport(model)));
        for p in final_report
            .properties
            .iter_mut()
            .filter(|p| p.id == "receipt_resolution")
        {
            p.verdict = Verdict::Inconclusive;
            for case in p.cases.as_mut().into_iter().flatten() {
                case.verdict = Verdict::Inconclusive;
            }
        }
        final_report = finish(final_report, vec![recovery_transport_pending(model)]);
    }
    seal(final_report)
}

fn recovery_transport_pending(model: &ir::Model) -> diag::Diagnostic {
    diag::Diagnostic {
        code: diag::U_SEM_0.into(),
        severity: diag::Severity::Error,
        message: "recovery_feature_combination: recovery report transport is not connected".into(),
        span: Some(model.process.span),
    }
}

pub(crate) fn property_notes(m: &ir::Model, p: ir::Property) -> Option<Vec<String>> {
    let ir::Property::Contract(id) = p else {
        return None;
    };
    let ir::ContractKind::AtMostOnce { event } = m.contracts.get(id.0)?.kind else {
        return None;
    };
    let modeled = m.capabilities.get(event.capability.0).is_some_and(|c| {
        let Some(a) = &c.atomic else {
            return false;
        };
        let ir::Cond::Contains {
            set,
            element: ir::Element::Parameter(i),
        } = a.condition
        else {
            return false;
        };
        event.kind == ir::EventKind::Commit
            && c.commits
            && !a.commit_when
            && c.params.get(i).is_some_and(|p| p.ty == ir::CASE_ID_TYPE)
            && m.state_variables.get(set.0).is_some_and(|v| {
                v.case_set_size.is_some() && v.kind == ir::StateKind::ActorObserved(c.actor)
            })
            && a.writes.iter().any(|w| {
                w.variable == set && w.source == ir::WriteSource::InsertCase { parameter: i }
            })
    });
    (!modeled).then(|| vec!["mechanism_unmodeled".into()])
}

pub(crate) fn notes_for_id(m: &ir::Model, id: &str) -> Option<Vec<String>> {
    m.contracts
        .iter()
        .enumerate()
        .find(|(i, c)| c.property_id(ir::ClauseId(*i)) == id)
        .and_then(|(i, _)| property_notes(m, ir::Property::Contract(ir::ClauseId(i))))
}

fn range(value: outcome::Range) -> Range {
    Range {
        min: value.min,
        max: value.max,
    }
}

fn degradation_cut_verdict(
    verdict: Verdict,
    truncated: Option<bool>,
    agency: Option<&crate::agency::Agency>,
) -> Verdict {
    if truncated == Some(true)
        && agency.is_some_and(|a| a.degradation.is_some())
        && matches!(verdict, Verdict::Satisfied | Verdict::Vacuous)
    {
        Verdict::Inconclusive
    } else {
        verdict
    }
}

fn finish(mut report: Report, diagnostics: Vec<diag::Diagnostic>) -> Report {
    report.diagnostics.extend(diagnostics);
    // W-CUT-0 exactly once when undeclared, never when a cut was written.
    let undeclared = report.cut == Cut::UNDECLARED;
    let mut seen_cut = false;
    report.diagnostics.retain(|diagnostic| {
        if diagnostic.code != diag::W_CUT_0 {
            return true;
        }
        let keep = undeclared && !seen_cut;
        seen_cut = true;
        keep
    });
    if undeclared && !seen_cut {
        report.diagnostics.push(diag::Diagnostic {
            code: diag::W_CUT_0.into(),
            severity: diag::Severity::Warning,
            message: "Cut is undeclared; exclusions are unknown".into(),
            span: None,
        });
    }
    report.verdict = fold_verdict(&report.properties, &report.diagnostics);
    // A witnessed reachable End does not close an unfinished degradation path.
    // Keep old input verdicts byte-stable; this is the ADR 0064 whole-run gate.
    report.verdict = degradation_cut_verdict(
        report.verdict,
        report.bounds.truncated,
        report.agency.as_ref(),
    );
    seal(report)
}

pub(crate) fn finite_driver(mut report: Report, budget: usize) -> Report {
    report.ledger.finite_driver(budget);
    seal(report)
}

fn simulation_driver(mut report: SimulationReport) -> SimulationReport {
    report.ledger.simulation_driver();
    seal_simulation(report)
}

pub fn seal_simulation(mut report: SimulationReport) -> SimulationReport {
    // Both the driver and direct/early simulation paths seal here. Apply the
    // same whole-run gate before serializing evidence for the ledger/decoder.
    report.verdict = degradation_cut_verdict(
        report.verdict,
        report.bounds.truncated,
        report.agency.as_ref(),
    );
    let host = serde_json::to_value(&report).expect("simulation uncertainty context");
    report.uncertainty.refresh(&host, true);
    let host = serde_json::to_value(&report).expect("simulation wire");
    crate::ledger::complete(
        &mut report.ledger,
        &host,
        &mut report.verdict,
        &mut report.diagnostics,
    );
    report
}

/// Finish provenance without touching the model, analysis, or existing evidence.
pub fn seal(mut report: Report) -> Report {
    let timer = crate::perf::span("seal");
    let host = serde_json::to_value(&report).expect("report uncertainty context");
    report.uncertainty.refresh(&host, false);
    let host = serde_json::to_value(&report).expect("report wire");
    crate::ledger::complete(
        &mut report.ledger,
        &host,
        &mut report.verdict,
        &mut report.diagnostics,
    );
    drop(timer);
    crate::perf::flush();
    report
}

/// ADR 0002 precedence; warnings are neutral and an empty property set is unsupported.
pub fn fold_verdict(properties: &[Property], diagnostics: &[diag::Diagnostic]) -> Verdict {
    let errors = diagnostics
        .iter()
        .filter(|d| d.severity == diag::Severity::Error);
    let diagnostic_verdicts = errors.map(|d| {
        if d.code.starts_with("U-") {
            Verdict::Unsupported
        } else {
            Verdict::Invalid
        }
    });
    // A reachable deadlock is a counterexample (the v0.1 §15 diagnostics that
    // v0.2 inherits), even under a cut and even if every property held.
    let deadlock = diagnostics
        .iter()
        .filter(|d| !properties.is_empty() && d.code == diag::W_DEADLOCK_0)
        .map(|_| Verdict::Violated);
    properties
        .iter()
        .map(|p| p.verdict)
        .chain(diagnostic_verdicts)
        .chain(deadlock)
        .max_by_key(|v| precedence(*v))
        .unwrap_or(Verdict::Unsupported)
}

pub(crate) fn precedence(verdict: Verdict) -> u8 {
    match verdict {
        Verdict::Satisfied => 0,
        Verdict::Vacuous => 1,
        Verdict::Inconclusive => 2,
        Verdict::Unsupported => 3,
        Verdict::Violated => 4,
        Verdict::Invalid => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_write_rows_precede_every_output_tail_without_reordering_old_rows() {
        let src = r#"model Tail version "0.2" {
          type R = enum { ok, no }
          observed case { flag: bool = false }
          actor Policy { version "r1"; decides required: bool = true }
          actor A { capacity 2; state x: 0..2 = 0
            capability r() -> R { requires x < 2 && flag == false else no
              duration fixed 1min; result nondet { ok } }
            capability w() -> R { duration fixed 1min; result nondet { ok }
              writes [x, flag]; finish { x += 1; flag = true } }
          }
          process P { r = A.r(); w = A.w(); end done }
          cut C { include [A, Policy] }
          bounds { cases 2; live_tasks 2; clock 5min }
          arrivals fixed [0min, 0min]
          verify safety no_resource_overcommit
        }"#;
        let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
        let first_resolution = m
            .assumptions
            .iter()
            .position(|r| r.field.ends_with(".concurrency"))
            .unwrap();
        let without_resolution: Vec<_> = m
            .assumptions
            .iter()
            .filter(|r| !r.field.ends_with(".concurrency"))
            .cloned()
            .collect();
        assert_eq!(
            read_write_anchor(&without_resolution, &m),
            first_resolution,
            "without a resolution, use the same end-of-base slot before finish adds arrivals"
        );
        // Independent boundary inventory: model declarations, exhaustive and
        // sampled output, and their refusal variants can publish these alone.
        let mut blocks: Vec<Vec<&str>> = [
            "metrics.window",
            "metrics.currency",
            "metrics.rates",
            "metrics.lead_time",
            "metrics.observation",
            "metrics.waiting",
            "metrics.estimation",
            "quality.definition",
            "quality.denominator",
            "quality.form",
            "quality.scheduler",
            "quality.estimation",
            "quality.rule",
            "quality.quantile",
            "probability.scheduler",
            "Policy.rule",
            "simulation.random",
            "simulation.replications",
            "simulation.multiplicity",
            "simulation.nondeterminism",
            "simulation.guarantee",
        ]
        .into_iter()
        .map(|field| vec![field])
        .collect();
        blocks.push(vec!["arrivals", "policy.queue", "simulation.random"]);
        for block in blocks {
            let row = |field: &str| Assumption {
                field: field.into(),
                value: "old value".into(),
                affects: "old explanation".into(),
            };
            let mut rows = vec![row("A.x.concurrency")];
            rows.extend(block.iter().map(|field| row(field)));
            let old = rows.clone();
            place_read_write_assumptions(&mut rows, &m);
            assert_eq!(rows[0], old[0]);
            assert_eq!(rows[1].field, "concurrency.read_write");
            assert_eq!(rows[2].field, "concurrency.read_write_excluded");
            assert_eq!(&rows[3..], &old[1..], "unchanged tail {block:?}");
            let mut zero = old.clone();
            place_read_write_assumptions(&mut zero, &model());
            assert_eq!(zero, old, "no candidates or excluded pairs: identity");
        }
    }

    #[test]
    fn aborted_exploration_leaves_truncated_null() {
        let src = "model M version \"0.2\" { type T = enum { pass }
          actor A { capacity 1
            capability c() -> T { duration fixed 18446744073709551615min; result nondet { pass } } }
          process P { a = A.c()
            b = A.c()
            end done }
          bounds { cases 1; live_tasks 1; clock 18446744073709551615min }
          verify reachable end.done }";
        let report = check(src);
        assert!(report.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
        assert_eq!(report.verdict, Verdict::Unsupported);
        assert_eq!(report.bounds.truncated, None);
        let completed = check(&src.replace("18446744073709551615min;", "1min;"));
        assert_eq!(completed.verdict, Verdict::Satisfied);
        assert_eq!(completed.bounds.truncated, Some(false));
    }

    fn diagnostic(code: &str, severity: diag::Severity) -> diag::Diagnostic {
        diag::Diagnostic {
            code: code.into(),
            severity,
            message: code.into(),
            span: None,
        }
    }

    fn ast_model() -> ast::Model {
        let span = diag::Span { start: 0, end: 0 };
        ast::Model {
            name: ast::Spanned {
                value: "Example".into(),
                span,
            },
            version: ast::Spanned {
                value: "0.2".into(),
                span,
            },
            items: Vec::new(),
            span,
        }
    }

    fn model() -> ir::Model {
        let span = diag::Span { start: 0, end: 0 };
        ir::Model {
            case_priority: None,
            performances: Vec::new(),
            comparison: None,
            concurrency_resolutions: Vec::new(),
            resource_time: None,
            arrival_distribution: None,
            arrival_every: None,
            horizon: None,
            scenario: None,
            fallback_capabilities: Vec::new(),
            quality: None,
            resources: Vec::new(),
            responsibility: None,
            simulation: None,
            metrics: None,
            arrivals: None,
            start_policy: None,
            admission_queue: None,
            concurrent_cases: None,
            overflow_end: None,
            kernels: Vec::new(),
            initial_distributions: Vec::new(),
            probability_queries: Vec::new(),
            state_variables: Vec::new(),
            people: Vec::new(),
            authorities: Vec::new(),
            contracts: Vec::new(),
            agency: crate::agency::Agency::empty(),
            name: "Example".into(),
            version: "0.2".into(),
            types: Vec::new(),
            actors: Vec::new(),
            capabilities: Vec::new(),
            process: ir::Process {
                degradations: Vec::new(),
                name: "Flow".into(),
                bindings: Vec::new(),
                forks: Vec::new(),
                body: Vec::new(),
                span,
            },
            ends: vec!["unreached".into(), "done".into()],
            bounds: ir::Bounds {
                cases: 1,
                live_tasks: 2,
                clock: 10,
            },
            verify: vec![ir::Verify {
                property: ir::Property::ReachableEnd(ir::EndId(1)),
                span,
            }],
            assumptions: vec![Assumption {
                field: "schedule".into(),
                value: "work-conserving".into(),
                affects: "time".into(),
            }],
            diagnostics: vec![diagnostic(diag::W_CUT_0, diag::Severity::Warning)],
            cut: None,
        }
    }

    #[test]
    fn fold_all_verdict_pairs_in_both_orders() {
        let ordered = [
            Verdict::Satisfied,
            Verdict::Vacuous,
            Verdict::Inconclusive,
            Verdict::Unsupported,
            Verdict::Violated,
            Verdict::Invalid,
        ];
        for (i, left) in ordered.iter().enumerate() {
            for (j, right) in ordered.iter().enumerate() {
                let properties = [
                    Property {
                        notes: None,
                        cases: None,
                        probability: None,
                        id: "a".into(),
                        verdict: *left,
                    },
                    Property {
                        notes: None,
                        cases: None,
                        probability: None,
                        id: "b".into(),
                        verdict: *right,
                    },
                ];
                assert_eq!(
                    fold_verdict(&properties, &[]),
                    ordered[i.max(j)],
                    "{left:?}/{right:?}"
                );
            }
        }
    }

    #[test]
    fn fold_classifies_diagnostics_without_promoting_warnings() {
        let properties = [Property {
            notes: None,
            cases: None,
            probability: None,
            id: "p".into(),
            verdict: Verdict::Satisfied,
        }];
        for code in [
            diag::E_SYN_0,
            diag::E_NAME_0,
            diag::E_TYPE_0,
            diag::E_CAP_0,
            diag::E_CANCEL_0,
        ] {
            assert_eq!(
                fold_verdict(&properties, &[diagnostic(code, diag::Severity::Error)]),
                Verdict::Invalid
            );
        }
        for code in [diag::U_SYN_0, diag::U_SEM_0] {
            let diagnostics = [diagnostic(code, diag::Severity::Error)];
            assert_eq!(
                fold_verdict(&properties, &diagnostics),
                Verdict::Unsupported
            );
            assert_eq!(
                fold_verdict(
                    &[Property {
                        notes: None,
                        cases: None,
                        probability: None,
                        id: "p".into(),
                        verdict: Verdict::Violated
                    }],
                    &diagnostics
                ),
                Verdict::Violated
            );
        }
        for code in [diag::W_CUT_0, diag::W_BUDGET_0] {
            assert_eq!(
                fold_verdict(&properties, &[diagnostic(code, diag::Severity::Warning)]),
                Verdict::Satisfied
            );
        }
        let deadlock = [diagnostic(diag::W_DEADLOCK_0, diag::Severity::Warning)];
        for verdict in [
            Verdict::Satisfied,
            Verdict::Vacuous,
            Verdict::Inconclusive,
            Verdict::Unsupported,
        ] {
            let held = [Property {
                notes: None,
                cases: None,
                probability: None,
                id: "safety no_resource_overcommit".into(),
                verdict,
            }];
            assert_eq!(fold_verdict(&held, &deadlock), Verdict::Violated);
        }
        let invalid = [
            deadlock[0].clone(),
            diagnostic(diag::E_CAP_0, diag::Severity::Error),
        ];
        assert_eq!(fold_verdict(&properties, &invalid), Verdict::Invalid);
        assert_eq!(fold_verdict(&[], &deadlock), Verdict::Unsupported);
        assert_eq!(fold_verdict(&[], &[]), Verdict::Unsupported);
        assert_eq!(
            fold_verdict(&properties, &[diagnostic("unknown", diag::Severity::Error)]),
            Verdict::Invalid
        );
    }

    #[test]
    fn syntax_failure_stops_lower_and_engine_and_keeps_hash() {
        for (code, verdict) in [
            (diag::E_SYN_0, Verdict::Invalid),
            (diag::U_SYN_0, Verdict::Unsupported),
        ] {
            let report = check_with(
                "abc",
                |_| Err(vec![diagnostic(code, diag::Severity::Error)]),
                |_| panic!("lower must not run"),
                |_| panic!("engine must not run"),
            );
            assert_eq!(report.verdict, verdict);
            assert!(report.model.name.is_none());
            assert_eq!(
                report.model.sha256.as_deref(),
                Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
            );
            assert!(report.properties.is_empty());
            assert!(report.terminals.is_empty());
            assert_eq!(report.bounds.truncated, None);
            assert_eq!(report.cut, Cut::UNDECLARED);
            assert_eq!(
                report
                    .diagnostics
                    .iter()
                    .filter(|d| d.code == diag::W_CUT_0)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn lower_failure_stops_engine_and_static_error_wins_over_unsupported() {
        for diagnostics in [
            vec![diagnostic(diag::U_SEM_0, diag::Severity::Error)],
            vec![
                diagnostic(diag::U_SEM_0, diag::Severity::Error),
                diagnostic(diag::E_TYPE_0, diag::Severity::Error),
            ],
        ] {
            let expected = if diagnostics.len() == 1 {
                Verdict::Unsupported
            } else {
                Verdict::Invalid
            };
            let report = check_with(
                "source",
                |_| Ok(ast_model()),
                |_| Err(diagnostics),
                |_| panic!("engine must not run"),
            );
            assert_eq!(report.verdict, expected);
            assert_eq!(report.model.name.as_deref(), Some("Example"));
            assert_eq!(report.bounds.clock, None);
            assert_eq!(report.bounds.truncated, None);
            assert!(report.terminals.is_empty());
            assert!(report.properties.is_empty());
        }
    }

    #[test]
    fn outcome_mapping_preserves_names_ranges_assumptions_and_diagnostics() {
        let report = check_with(
            "source",
            |_| Ok(ast_model()),
            |_| Ok(model()),
            |model| {
                assert_eq!(model.name, "Example");
                outcome::Outcome {
                    response_exercise: None,
                    response_receipts: None,
                    response_recovery: None,
                    scenario_observed_until: None,
                    quality: None,
                    state_range_violations: Vec::new(),
                    capability_refusals: None,
                    unchecked: Vec::new(),
                    postprocessing_diagnostics: Vec::new(),
                    contest_runtime: None,
                    contest_summary: None,
                    obligations: None,
                    metrics: None,
                    case_outcomes: None,
                    admission: None,
                    probability_scheduler: false,
                    probability_policy_unresolved: None,
                    properties: vec![
                        outcome::PropertyOutcome {
                            cases: None,
                            probability: None,
                            property: ir::Property::NoResourceOvercommit,
                            verdict: Verdict::Inconclusive,
                        },
                        outcome::PropertyOutcome {
                            cases: None,
                            probability: None,
                            property: ir::Property::ReachableEnd(ir::EndId(1)),
                            verdict: Verdict::Satisfied,
                        },
                    ],
                    terminals: vec![outcome::TerminalOutcome {
                        probability: None,
                        end: ir::EndId(1),
                        elapsed: outcome::Range { min: 4, max: 5 },
                        work: outcome::Range { min: 7, max: 8 },
                        result_at: Some(outcome::Range { min: 3, max: 3 }),
                    }],
                    truncated: true,
                    diagnostics: vec![
                        diagnostic(diag::W_DEADLOCK_0, diag::Severity::Warning),
                        diagnostic(diag::W_CUT_0, diag::Severity::Warning),
                    ],
                }
            },
        );
        // The reachable deadlock is a counterexample despite the cut (F3).
        assert_eq!(report.verdict, Verdict::Violated);
        assert_eq!(report.assumptions, model().assumptions);
        assert_eq!(
            report
                .diagnostics
                .iter()
                .filter(|d| d.code == diag::W_CUT_0)
                .count(),
            1
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == diag::W_DEADLOCK_0)
        );
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(crate::ledger_test_shape::original_field_count(&json), 13);
        assert_eq!(json["mode"], "exhaustive");
        assert_eq!(json["unit"], "min");
        assert_eq!(json["cut"], "undeclared");
        assert_eq!(
            json["bounds"],
            serde_json::json!({"cases":1,"live_tasks":2,"clock":10,"truncated":true})
        );
        assert_eq!(
            json["properties"],
            serde_json::json!([
                {"id":"safety no_resource_overcommit","verdict":"inconclusive"},
                {"id":"reachable end.done","verdict":"satisfied"}
            ])
        );
        assert_eq!(
            json["terminals"],
            serde_json::json!([{
                "end":"done","elapsed":{"min":4,"max":5},"work":{"min":7,"max":8},"result_at":{"min":3,"max":3}
            }])
        );
    }

    #[test]
    fn null_result_and_no_terminal_are_not_fake_zero_ranges() {
        for terminals in [
            Vec::new(),
            vec![outcome::TerminalOutcome {
                probability: None,
                end: ir::EndId(1),
                elapsed: outcome::Range { min: 8, max: 8 },
                work: outcome::Range { min: 8, max: 8 },
                result_at: None,
            }],
        ] {
            let expected_len = terminals.len();
            let report = apply_outcome(
                empty_report("source"),
                &model(),
                outcome::Outcome {
                    response_exercise: None,
                    response_receipts: None,
                    response_recovery: None,
                    scenario_observed_until: None,
                    quality: None,
                    state_range_violations: Vec::new(),
                    capability_refusals: None,
                    unchecked: Vec::new(),
                    postprocessing_diagnostics: Vec::new(),
                    contest_runtime: None,
                    contest_summary: None,
                    obligations: None,
                    metrics: None,
                    case_outcomes: None,
                    admission: None,
                    probability_scheduler: false,
                    probability_policy_unresolved: None,
                    properties: vec![outcome::PropertyOutcome {
                        cases: None,
                        probability: None,
                        property: ir::Property::ReachableEnd(ir::EndId(1)),
                        verdict: Verdict::Violated,
                    }],
                    terminals,
                    truncated: false,
                    diagnostics: Vec::new(),
                },
            );
            let json = serde_json::to_value(report).unwrap();
            assert_eq!(json["bounds"]["truncated"], false);
            assert_eq!(json["terminals"].as_array().unwrap().len(), expected_len);
            if expected_len > 0 {
                assert!(json["terminals"][0]["result_at"].is_null());
            }
        }
    }

    #[test]
    fn zero_properties_are_explicitly_unsupported_without_exploring() {
        let mut model = model();
        model.verify.clear();
        let report = check_with(
            "source",
            |_| Ok(ast_model()),
            |_| Ok(model),
            |_| panic!("empty verification must not run engine"),
        );
        assert_eq!(report.verdict, Verdict::Unsupported);
        assert_eq!(report.bounds.truncated, None);
        assert_eq!(report.bounds.clock, Some(10)); // declaration, not an analysis
        assert!(report.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
        assert!(report.diagnostics.iter().any(|d| d.code == diag::W_CUT_0));
    }

    #[test]
    fn lowered_error_does_not_claim_an_unrun_analysis_completed() {
        let mut model = model();
        model
            .diagnostics
            .push(diagnostic(diag::E_CAP_0, diag::Severity::Error));
        let report = check_with(
            "source",
            |_| Ok(ast_model()),
            |_| Ok(model),
            |_| panic!("lowered errors must not run engine"),
        );
        assert_eq!(report.verdict, Verdict::Invalid);
        assert_eq!(report.bounds.truncated, None);
        assert_eq!(report.bounds.clock, Some(10));
        assert!(report.properties.is_empty());
        assert!(report.terminals.is_empty());
        assert!(serde_json::to_value(report).unwrap()["bounds"]["truncated"].is_null());
    }

    #[test]
    fn sha256_is_full_source_lowercase_hex_with_no_normalization() {
        assert_eq!(
            empty_report("").model.sha256.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
        assert_ne!(
            empty_report("abc").model.sha256,
            empty_report("abc\n").model.sha256
        );
        assert_eq!(
            empty_report("日本語").model.sha256,
            Some("77710aedc74ecfa33685e33a6c7df5cc83004da1bdcef7fb280f5c2b2e97e0a5".into())
        );
    }
}
