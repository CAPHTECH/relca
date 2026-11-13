//! Replay-checked examples of structural violations (ADR 0021).
use crate::{Verdict, diag, ir, report};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ExplainReport {
    #[serde(flatten)]
    pub report: report::Report,
    pub explanation: Explanation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Complete,
    Blocked,
    Unsupported,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayStatus {
    Verified,
    Unsupported,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayReason {
    ZeroDurationReferenceUnavailable,
    ReferenceLimit,
    IllegalTransition,
    EventMismatch,
    StateMismatch,
    ViolationMismatch,
    SourceMappingFailed,
    CaptureLimit,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Model,
    ExploredGraph,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    LoopControl,
    ReceiptQuery,
    ResolveEffect,
    Horizon,
    RangeViolation,
    Refuse,
    Wait,
    Wake,
    ContestFiled,
    ContestDeclined,
    ContestQueue,
    ContestResponse,
    ExecutionClose,
    Initial,
    Initialize,
    Admission,
    Admitted,
    Receive,
    Received,
    Queue,
    Start,
    Advance,
    DurationGate,
    Finish,
    Commit,
    Adopt,
    CancelRequest,
    CancelAck,
    Branch,
    Join,
    End,
    Deadlock,
    Deadline,
    Write,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Observed,
    Latent,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Omission {
    ProbabilisticProperty,
    NonPathProperty,
    StaticProperty,
}

#[derive(Debug, Clone, Serialize)]
pub struct Explanation {
    pub version: &'static str,
    pub status: Status,
    pub shortest_scope: Option<Scope>,
    pub counterexamples: Vec<Counterexample>,
    pub omitted: Vec<Omitted>,
    pub assumptions: Vec<ir::Assumption>,
    pub diagnostics: Vec<diag::Diagnostic>,
    pub checked: Vec<String>,
    pub unchecked: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Omitted {
    pub id: String,
    pub verdict: Verdict,
    pub reason: Omission,
}
#[derive(Debug, Clone, Serialize)]
pub struct Counterexample {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub degradation_routes: Vec<DegradationTrace>,
    pub id: String,
    pub case_id: Option<u64>,
    pub status: ReplayStatus,
    pub reason: Option<ReplayReason>,
    pub property_location: Option<Location>,
    pub context_location: Location,
    pub related_locations: Vec<Location>,
    pub transition_count: Option<usize>,
    pub steps: Vec<Stage>,
    pub replay: ReplayResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct DegradationTrace {
    pub route: String,
    pub location: Location,
    /// Actual Start steps inside this route span, not the lexical call list.
    pub started_capabilities: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReplayResult {
    pub status: ReplayStatus,
    pub reason: Option<ReplayReason>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Location {
    pub span: diag::Span,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}
impl Location {
    pub(crate) fn new(src: &str, span: diag::Span) -> Result<Self, ReplayReason> {
        Self::indexed(src, &Self::line_starts(src), span)
    }
    pub(crate) fn line_starts(src: &str) -> Vec<usize> {
        std::iter::once(0)
            .chain(
                src.bytes()
                    .enumerate()
                    .filter_map(|(n, b)| (b == b'\n').then_some(n + 1)),
            )
            .collect()
    }
    pub(crate) fn indexed(
        src: &str,
        starts: &[usize],
        span: diag::Span,
    ) -> Result<Self, ReplayReason> {
        if span.start > span.end
            || span.end > src.len()
            || !src.is_char_boundary(span.start)
            || !src.is_char_boundary(span.end)
        {
            return Err(ReplayReason::SourceMappingFailed);
        }
        let at = |byte| {
            let line = starts.partition_point(|start| *start <= byte);
            (line, src[starts[line - 1]..byte].chars().count() + 1)
        };
        let (line, column) = at(span.start);
        let (end_line, end_column) = at(span.end);
        Ok(Self {
            span,
            line,
            column,
            end_line,
            end_column,
        })
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Stage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_key: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_receipt: Option<crate::ast::Receipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_location: Option<Location>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shared_resources: Vec<SharedResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_range_violation: Option<crate::outcome::StateRangeViolation>,
    pub index: usize,
    pub time: u64,
    pub case_id: Option<u64>,
    pub binding: Option<String>,
    pub actor: Option<String>,
    pub capability: Option<String>,
    pub executor: Option<String>,
    pub result: Option<String>,
    pub kind: Kind,
    pub events: Vec<Event>,
    pub location: Location,
    pub values: Values,
    pub violation: Option<Violation>,
    pub resource: Option<Resource>,
    pub capability_location: Option<Location>,
    pub deadlock_detail: Option<DeadlockDetail>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Event {
    Business(BusinessEvent),
    Value(Value),
}
#[derive(Debug, Clone, Serialize)]
pub struct BusinessEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repetition: Option<ir::LoopSite>,
    pub kind: Kind,
    pub case_id: Option<u64>,
    pub binding: Option<String>,
    pub actor: Option<String>,
    pub capability: Option<String>,
    pub executor: Option<String>,
    pub result: Option<String>,
    pub location: Location,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack_at: Option<u64>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Values {
    pub observed: Vec<Value>,
    pub latent: Vec<Value>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Value {
    pub name: String,
    pub value: DisplayValue,
    pub visibility: Visibility,
    pub location: Location,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum DisplayValue {
    Scalar(String),
    CaseSet(Vec<u64>),
}
impl From<String> for DisplayValue {
    fn from(s: String) -> Self {
        Self::Scalar(s)
    }
}
impl From<&str> for DisplayValue {
    fn from(s: &str) -> Self {
        Self::Scalar(s.into())
    }
}
impl PartialEq<str> for DisplayValue {
    fn eq(&self, s: &str) -> bool {
        matches!(self, Self::Scalar(x) if x == s)
    }
}
impl PartialEq<&str> for DisplayValue {
    fn eq(&self, s: &&str) -> bool {
        self == *s
    }
}
impl PartialEq<DisplayValue> for String {
    fn eq(&self, s: &DisplayValue) -> bool {
        s == self.as_str()
    }
}
impl PartialEq<String> for DisplayValue {
    fn eq(&self, s: &String) -> bool {
        self == s.as_str()
    }
}
impl DisplayValue {
    /// Test-only scalar view for the ADR 0037 detail tests written before the
    /// ADR0035 case-set display; a case set is not a scalar.
    #[cfg(test)]
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Scalar(s) => s,
            Self::CaseSet(_) => panic!("case set display is not a scalar"),
        }
    }
    pub(crate) fn parse<T: std::str::FromStr>(&self) -> Result<T, ()> {
        match self {
            Self::Scalar(s) => s.parse().map_err(|_| ()),
            _ => Err(()),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Violation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_invariant: Option<StateInvariantDetail>,
    pub id: String,
    pub property_location: Option<Location>,
    pub at_step: usize,
    pub trigger_step: Option<usize>,
    pub deadline: Option<u64>,
    pub related_steps: Vec<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateInvariantDetail {
    pub case_id: u64,
    pub antecedent: bool,
    pub condition: bool,
    pub values: Vec<Value>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Resource {
    pub actor: String,
    pub capacity: u64,
    pub anonymous: bool,
    pub holders: Vec<Holder>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Holder {
    pub case_id: u64,
    pub binding: String,
    pub executor: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct DeadlockDetail {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shared_resources: Vec<SharedResource>,
    pub waiting_tasks: Vec<WaitingTask>,
    pub resources: Vec<Resource>,
    pub obligations: Vec<Obligation>,
    pub joins: Vec<Join>,
}
#[derive(Debug, Clone, Serialize)]
pub struct WaitingTask {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resource_shortages: Vec<ResourceShortage>,
    pub case_id: u64,
    pub binding: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SharedResource {
    pub resource: String,
    pub capacity: u64,
    pub occupied: u64,
    pub holders: Vec<SharedHolder>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SharedHolder {
    pub case_id: u64,
    pub binding: String,
    pub amount: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceShortage {
    pub resource: String,
    pub required: u64,
    pub available: u64,
    pub missing: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct Obligation {
    pub case_id: u64,
    pub id: String,
    pub trigger_step: usize,
    pub deadline: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct Join {
    pub case_id: u64,
    pub bindings: Vec<String>,
}

/// Private categorical selection provenance, never a probability/cause on wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KernelChoice {
    pub frozen_performance: Option<ir::FrozenPerformance>,
    pub case_id: u64,
    pub binding: String,
    pub row: usize,
    pub branch: usize,
}

impl Explanation {
    fn blocked() -> Self {
        Self {
            version: "invoice-counterexample-v1",
            status: Status::Blocked,
            shortest_scope: None,
            counterexamples: Vec::new(),
            omitted: Vec::new(),
            assumptions: Vec::new(),
            diagnostics: Vec::new(),
            checked: Vec::new(),
            unchecked: [
                "probabilistic_causes",
                "simulation_counterexamples",
                "real_world_validity",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }
}

pub fn explain(src: &str) -> ExplainReport {
    explain_with_limit(src, DEFAULT_CAPTURE_LIMIT)
}

const DEFAULT_CAPTURE_LIMIT: usize = 200_000;

pub fn explain_with_scenario(src: &str, scenario: Option<&str>) -> ExplainReport {
    explain_with_selection(src, None, scenario)
}

pub fn explain_with_selection(
    src: &str,
    configuration: Option<&str>,
    scenario: Option<&str>,
) -> ExplainReport {
    explain_selected(src, configuration, scenario, DEFAULT_CAPTURE_LIMIT)
}

fn explain_with_limit(src: &str, limit: usize) -> ExplainReport {
    explain_selected(src, None, None, limit)
}

fn explain_selected(
    src: &str,
    configuration: Option<&str>,
    scenario: Option<&str>,
    limit: usize,
) -> ExplainReport {
    let mut captured = None;
    let mut actual_capture_limit = limit;
    let report = report::check_selection_using(src, configuration, scenario, |model| {
        let (outcome, trace) = if limit == DEFAULT_CAPTURE_LIMIT {
            crate::engine::trace::explore(src, model)
        } else {
            let mut capture = crate::engine::trace::Capture::new(src, model);
            capture.capture_limit = limit;
            crate::engine::trace::explore_captured(
                model,
                crate::engine::DEFAULT_STATE_BUDGET,
                capture,
            )
        };
        actual_capture_limit = trace.capture_limit;
        captured = Some((model.clone(), trace));
        outcome
    });
    let mut explanation = Explanation::blocked();
    if let Some((model, trace)) = captured {
        if model.recovery().is_some() {
            explanation.version = "invoice-counterexample-p3a-recovery-1";
            explanation.assumptions.push(ir::Assumption{field:"explanation.recovery_events".into(),value:"atomic query finish with a distinct zero-time evidence event".into(),affects:"The shortest captured graph witness is expanded into observed events; query evidence and resolve are not extra Tasks, commits or business decisions.".into()});
        }
        explanation.status = Status::Complete;
        if trace.failure == Some(ReplayReason::CaptureLimit) {
            explanation.status = Status::Unsupported;
            explanation.diagnostics.push(diag::Diagnostic {
                code: diag::U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: "capture_limit: CaptureLimit".into(),
                span: None,
            });
            explanation
                .unchecked
                .extend(["capture", "shortest_path"].map(String::from));
        }
        let scope = if report.bounds.truncated == Some(false)
            && !trace
                .edges
                .iter()
                .flatten()
                .any(|e| e.stage.kind == Kind::Horizon)
        {
            Scope::Model
        } else {
            Scope::ExploredGraph
        };
        explanation.shortest_scope = Some(scope);
        explanation.checked = ["capture", "shortest_path", "source_locations"]
            .into_iter()
            .map(String::from)
            .collect();
        if trace.failure == Some(ReplayReason::CaptureLimit) {
            explanation
                .checked
                .retain(|s| s != "capture" && s != "shortest_path");
        }
        if scope == Scope::ExploredGraph {
            explanation
                .unchecked
                .push("unexplored_shortest_paths".into());
        }
        for (field, value, affects) in [
            (
                "explain.purpose",
                "example_not_cause",
                "一つの例であり、原因ではない。",
            ),
            (
                "explain.shortest",
                if scope == Scope::Model {
                    "model"
                } else {
                    "explored_graph"
                },
                "対象グラフの辺数最小、同順位は生成順。",
            ),
            (
                "explain.latent",
                "analyst_only",
                "潜在の値は分析者向けであり、方策から観測可能ではない。",
            ),
            (
                "explain.replay",
                "independent_reference_prefix",
                "経路の合法性と同じ違反を再生で確かめる。最短性・現実妥当性の証明ではない。",
            ),
        ] {
            explanation.assumptions.push(ir::Assumption {
                field: field.into(),
                value: value.into(),
                affects: affects.into(),
            });
        }
        for (property, verify) in report.properties.iter().zip(&model.verify) {
            let reason = match verify.property {
                ir::Property::ProbabilityReached(_) | ir::Property::AlmostSureEnd => {
                    Some(Omission::ProbabilisticProperty)
                }
                ir::Property::ReachableEnd(_)
                | ir::Property::RecoverableEnd
                | ir::Property::TerminatingEnd => Some(Omission::NonPathProperty),
                ir::Property::Contest(_) => Some(Omission::StaticProperty),
                _ => None,
            };
            if let Some(reason) = reason {
                explanation.omitted.push(Omitted {
                    id: property.id.clone(),
                    verdict: property.verdict,
                    reason,
                });
            } else if property.verdict == Verdict::Violated {
                let span = if let ir::Property::Contract(id) = verify.property {
                    model.contracts[id.0].span
                } else {
                    verify.span
                };
                add_counterexample(
                    &mut explanation,
                    src,
                    &model,
                    &trace,
                    &property.id,
                    (verify.property != ir::Property::StateInRange).then_some(span),
                );
            }
        }
        if report
            .diagnostics
            .iter()
            .any(|d| d.code == diag::W_DEADLOCK_0)
        {
            add_counterexample(
                &mut explanation,
                src,
                &model,
                &trace,
                "diagnostic:W-DEADLOCK-0",
                None,
            );
        }
        if explanation.status == Status::Complete {
            explanation.checked.push("reference_replay".into());
        } else {
            explanation.unchecked.push("reference_replay".into());
        }
    }
    seal(
        ExplainReport {
            report: report::finite_driver(report, crate::engine::DEFAULT_STATE_BUDGET),
            explanation,
        },
        actual_capture_limit,
    )
}

pub fn argument_failure(message: &str) -> ExplainReport {
    input_failure(message)
}

pub fn input_failure(message: &str) -> ExplainReport {
    input_failure_with_scenario(message, None)
}
pub fn input_failure_with_scenario(message: &str, scenario: Option<&str>) -> ExplainReport {
    input_failure_with_selection(message, None, scenario)
}
pub fn input_failure_with_selection(
    message: &str,
    configuration: Option<&str>,
    scenario: Option<&str>,
) -> ExplainReport {
    let mut report = report::check("");
    report.model.sha256 = None;
    report.cut = report::Cut::Status(report::CutStatus::Unresolved);
    report.ledger = crate::ledger::Ledger::input_failure("exhaustive");
    report.ledger.requested(configuration, scenario);
    report.diagnostics = vec![diag::Diagnostic {
        code: diag::E_SYN_0.into(),
        severity: diag::Severity::Error,
        message: message.into(),
        span: None,
    }];
    seal(
        ExplainReport {
            report,
            explanation: Explanation::blocked(),
        },
        DEFAULT_CAPTURE_LIMIT,
    )
}

fn seal(mut result: ExplainReport, limit: usize) -> ExplainReport {
    result.report.ledger.execution["limits"]["capture_limit"] = crate::ledger::known(limit);
    let host = serde_json::to_value(&result).expect("explanation uncertainty context");
    result.report.uncertainty.refresh(&host, false);
    let host = serde_json::to_value(&result).expect("explanation wire");
    crate::ledger::complete(
        &mut result.report.ledger,
        &host,
        &mut result.report.verdict,
        &mut result.report.diagnostics,
    );
    result
}

fn add_counterexample(
    out: &mut Explanation,
    src: &str,
    model: &ir::Model,
    trace: &crate::engine::trace::Capture,
    id: &str,
    span: Option<diag::Span>,
) {
    let context = trace
        .location(model.process.span)
        .expect("lowered process location");
    let mapped_property = span.map(|s| trace.location(s)).transpose();
    let mapping_failure = mapped_property.as_ref().err().copied();
    let property_location = mapped_property.ok().flatten();
    let (mut steps, target_case, mut choices) = trace.shortest(id).unwrap_or_default();
    expand_recovery_events(model, &mut steps, &mut choices);
    let case_id = if id == "diagnostic:W-DEADLOCK-0" || id == "safety no_resource_overcommit" {
        None
    } else {
        target_case
    };
    if let Some(last) = steps.last_mut() {
        last.violation = Some(Violation {
            state_invariant: last
                .violation
                .as_ref()
                .and_then(|v| v.state_invariant.clone()),
            id: id.into(),
            property_location: property_location.clone(),
            at_step: last.index,
            trigger_step: None,
            deadline: None,
            related_steps: Vec::new(),
        });
    }
    let obligations_failure = resolve_obligations(model, &mut steps).err();
    let final_choice = choices.last().and_then(Option::as_ref);
    let related_locations = enrich(src, model, trace, id, case_id, &mut steps, final_choice);
    let authority_mapping_failure =
        crate::reference::replay::validate_authority_locations(src, model, id, &related_locations)
            .err();
    let state_mapping_failure = crate::reference::replay::validate_state_invariant_locations(
        src,
        model,
        id,
        &steps,
        &related_locations,
        final_choice,
    )
    .err();
    let reason = trace
        .failure
        .or(mapping_failure)
        .or(obligations_failure)
        .or(authority_mapping_failure)
        .or(state_mapping_failure)
        .or_else(|| {
            if steps.is_empty() {
                Some(ReplayReason::IllegalTransition)
            } else {
                crate::reference::replay::replay_choices(model, &steps, id, case_id, &choices).err()
            }
        });
    let status = if reason.is_some() {
        ReplayStatus::Unsupported
    } else {
        ReplayStatus::Verified
    };
    let transition_count = if reason.is_none() {
        Some(steps.len() - 1)
    } else {
        steps.clear();
        None
    };
    if let Some(reason) = reason {
        out.status = Status::Unsupported;
        if reason == ReplayReason::SourceMappingFailed {
            out.checked.retain(|field| field != "source_locations");
            if !out
                .unchecked
                .iter()
                .any(|field| field == "source_locations")
            {
                out.unchecked.push("source_locations".into());
            }
        }
        out.diagnostics.push(diag::Diagnostic {
            code: diag::U_SEM_0.into(),
            severity: diag::Severity::Error,
            message: format!("{id}: {reason:?}"),
            span,
        });
    }
    let degradation_routes = model
        .process
        .degradations
        .iter()
        .filter_map(|r| {
            let branch = steps
                .iter()
                .find(|s| s.kind == Kind::Branch && s.location.span == r.span)?;
            let started_capabilities = steps
                .iter()
                .filter(|s| {
                    s.kind == Kind::Start
                        && s.location.span.start >= r.span.start
                        && s.location.span.end <= r.span.end
                })
                .filter_map(|s| {
                    s.actor
                        .as_ref()
                        .zip(s.capability.as_ref())
                        .map(|(a, c)| format!("{a}.{c}"))
                })
                .collect();
            Some(DegradationTrace {
                route: r.name.clone(),
                location: branch.location.clone(),
                started_capabilities,
            })
        })
        .collect();
    out.counterexamples.push(Counterexample {
        degradation_routes,
        id: id.into(),
        case_id,
        status,
        reason,
        property_location,
        context_location: context,
        related_locations,
        transition_count,
        steps,
        replay: ReplayResult { status, reason },
    });
}

pub(crate) fn recovery_evidence_id(case: u64, command: &str, binding: &str, at: u64) -> String {
    format!("case{case}:{command}:{binding}:{at}")
}
fn expand_recovery_events(
    m: &ir::Model,
    steps: &mut Vec<Stage>,
    choices: &mut Vec<Option<KernelChoice>>,
) {
    let Some((_, body)) = m.recovery() else {
        return;
    };
    if steps.len() != choices.len() {
        return;
    }
    fn queries(b: &ir::Block, rows: &mut Vec<(ir::BindingId, ir::BindingId)>) {
        for s in b {
            match &s.kind {
                ir::Stmt::Query(c, command) => rows.push((c.binding, *command)),
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    queries(then_body, rows);
                    queries(else_body, rows);
                }
                _ => {}
            }
        }
    }
    let mut rows = Vec::new();
    queries(body, &mut rows);
    let old = std::mem::take(steps);
    let old_choices = std::mem::take(choices);
    for (stage, choice) in old.into_iter().zip(old_choices) {
        let query = (stage.kind == Kind::Finish)
            .then(|| {
                rows.iter().find(|(binding, _)| {
                    stage.binding.as_deref() == Some(m.site_name(*binding).as_str())
                })
            })
            .flatten();
        let extra = query.map(|(binding, command)| {
            let mut q = stage.clone();
            q.kind = Kind::ReceiptQuery;
            q.command = Some(m.site_name(*command));
            q.saved_key = q.case_id;
            q.evidence_ref = (q.result.as_deref() != Some("still_unknown")).then(|| {
                recovery_evidence_id(
                    q.case_id.unwrap_or(0),
                    q.command.as_deref().unwrap_or(""),
                    &m.site_name(*binding),
                    q.time,
                )
            });
            q.events.clear();
            q.resource = None;
            q.shared_resources.clear();
            q.executor = None;
            q.actual_result = None;
            q.observed_receipt = None;
            q.attempt = None;
            q.retry_location = None;
            q.violation = None;
            q
        });
        steps.push(stage);
        choices.push(choice);
        if let Some(q) = extra {
            steps.push(q);
            choices.push(None);
        }
    }
    for (index, step) in steps.iter_mut().enumerate() {
        step.index = index;
    }
}

fn business(stage: &Stage, case_id: Option<u64>) -> impl Iterator<Item = &BusinessEvent> {
    stage.events.iter().filter_map(move |event| match event {
        Event::Business(event) if case_id.is_none() || event.case_id == case_id => Some(event),
        _ => None,
    })
}

fn pending_trigger(
    steps: &[Stage],
    model: &ir::Model,
    case_id: Option<u64>,
    trigger: ir::Trigger,
    targets: &[ir::Target],
    within: u64,
    expected_deadline: Option<u64>,
) -> Option<(usize, u64)> {
    for stage in steps {
        let triggered = business(stage, case_id).any(|event| match trigger {
            ir::Trigger::Received => matches!(event.kind, Kind::Received | Kind::Receive),
            ir::Trigger::Admitted => event.kind == Kind::Admitted,
            ir::Trigger::Event(pattern) => event_matches(event, pattern, model),
        });
        if !triggered {
            continue;
        }
        let deadline = stage.time.checked_add(within)?;
        if expected_deadline.is_some_and(|expected| expected != deadline) {
            continue;
        }
        let fulfilled = steps[stage.index..].iter().any(|later| {
            later.time <= deadline
                && targets.iter().any(|target| match target {
                    ir::Target::Event(pattern) => {
                        business(later, case_id).any(|event| event_matches(event, *pattern, model))
                    }
                    ir::Target::End(end) => {
                        later.case_id == case_id
                            && stage_end(later, &model.process.body) == Some(*end)
                    }
                })
        });
        if !fulfilled {
            return Some((stage.index, deadline));
        }
    }
    None
}
fn stage_end(stage: &Stage, block: &ir::Block) -> Option<ir::EndId> {
    if stage.kind != Kind::End {
        return None;
    }
    for statement in block {
        if let ir::Stmt::Response(_, r) = &statement.kind
            && r.retry_span == Some(stage.location.span)
        {
            return r.exhausted;
        }
        if statement.span == stage.location.span {
            match &statement.kind {
                ir::Stmt::End(end) => return Some(*end),
                ir::Stmt::Response(_, r) => return r.exhausted,
                ir::Stmt::Fork {
                    join: ir::Join::Any { if_none, .. },
                    ..
                } => return Some(*if_none),
                _ => {}
            }
        }
        if let ir::Stmt::If {
            then_body,
            else_body,
            ..
        } = &statement.kind
            && let Some(end) = stage_end(stage, then_body).or_else(|| stage_end(stage, else_body))
        {
            return Some(end);
        }
    }
    None
}
fn enrich(
    src: &str,
    model: &ir::Model,
    trace: &crate::engine::trace::Capture,
    id: &str,
    case_id: Option<u64>,
    steps: &mut [Stage],
    choice: Option<&KernelChoice>,
) -> Vec<Location> {
    let mut related = Vec::new();
    if let Some(variable) = model.verify.iter().find_map(|v| match v.property {
        ir::Property::NoLostUpdate(x) if model.loss_property_id(x) == id => Some(x),
        _ => None,
    }) {
        let cap_for = |s: &Stage| {
            model.capabilities.iter().find(|c| {
                s.actor.as_deref() == Some(model.actors[c.actor.0].name.as_str())
                    && s.capability.as_deref() == Some(c.name.as_str())
            })
        };
        if let Some(last) = steps.last() {
            let own = (last.case_id, last.binding.clone());
            let start = steps
                .iter()
                .rposition(|s| s.kind == Kind::Start && (s.case_id, s.binding.clone()) == own);
            if let Some(start) = start {
                let writer = steps
                    .iter()
                    .enumerate()
                    .skip(start + 1)
                    .take(steps.len().saturating_sub(start + 2))
                    .rev()
                    .find(|(_, s)| {
                        if (s.case_id, s.binding.clone()) == own {
                            return false;
                        }
                        cap_for(s).is_some_and(|c| {
                            if s.kind == Kind::Start {
                                c.start_writes.iter().any(|w| w.variable == variable)
                            } else if matches!(s.kind, Kind::Finish | Kind::ContestResponse) {
                                let result = s
                                    .result
                                    .as_ref()
                                    .and_then(|v| {
                                        model.types[c.return_type.0]
                                            .members
                                            .iter()
                                            .position(|x| x == v)
                                    })
                                    .map(ir::ValueId);
                                result.is_some_and(|r| {
                                    c.version_check.as_ref().is_none_or(|v| v.reject != r)
                                        && c.atomic.as_ref().is_none_or(|a| a.commit == r)
                                }) && c.finish_writes().any(|w| w.variable == variable)
                            } else {
                                false
                            }
                        })
                    })
                    .map(|(i, _)| i);
                let mut indices = vec![start];
                if let Some(writer) = writer {
                    indices.push(writer);
                }
                for n in writer.into_iter().chain(std::iter::once(steps.len() - 1)) {
                    if let Some(cap) = cap_for(&steps[n])
                        && let Ok(location) = trace.location(cap.span)
                        && !related.contains(&location)
                    {
                        related.push(location);
                    }
                }
                if let Some(v) = steps.last_mut().and_then(|s| s.violation.as_mut()) {
                    v.related_steps = indices;
                }
            }
        }
        return related;
    }
    if id == "state_in_range" {
        if let Some(w) = steps.last().and_then(|s| s.state_range_violation.as_ref()) {
            for span in [w.write_span, w.declaration_span] {
                if let Ok(l) = trace.location(span) {
                    related.push(l);
                }
            }
        }
        return related;
    }
    let Some((_, clause)) = model
        .contracts
        .iter()
        .enumerate()
        .find(|(n, c)| c.property_id(ir::ClauseId(*n)) == id)
    else {
        return related;
    };
    let mut indices = Vec::new();
    let mut pending = None;
    match &clause.kind {
        ir::ContractKind::StateInvariant { .. } => {
            if let Some(last) = steps.last() {
                indices.push(last.index);
            }
            if let Ok(locations) = state_invariant_locations(src, model, id, steps, choice) {
                related = locations;
            }
        }
        ir::ContractKind::ContestResponse { contest } => {
            let declaration = &model.contests()[*contest];
            pending = steps
                .iter()
                .find(|s| {
                    s.case_id == case_id
                        && match declaration.target {
                            crate::contest::ContestTarget::End(_) => s.kind == Kind::End,
                            crate::contest::ContestTarget::Commit(c) => {
                                business(s, case_id).any(|e| {
                                    event_matches(
                                        e,
                                        ir::Event {
                                            capability: c,
                                            kind: ir::EventKind::Commit,
                                        },
                                        model,
                                    )
                                })
                            }
                        }
                })
                .and_then(|s| {
                    s.time
                        .checked_add(declaration.respond_by)
                        .map(|deadline| (s.index, deadline))
                });
            indices = steps
                .iter()
                .filter(|s| {
                    s.case_id == case_id
                        && matches!(
                            s.kind,
                            Kind::End
                                | Kind::ContestFiled
                                | Kind::ContestQueue
                                | Kind::Start
                                | Kind::ContestResponse
                        )
                })
                .map(|s| s.index)
                .collect();
            if let Ok(location) = trace.location(model.capabilities[declaration.route.0].span) {
                related.push(location);
            }
        }
        ir::ContractKind::Excludes { before, start } => {
            for (index, stage) in steps.iter().enumerate() {
                if business(stage, case_id).any(|e| event_matches(e, *start, model))
                    && let Some(prior) = steps[..index]
                        .iter()
                        .find(|s| business(s, case_id).any(|e| event_matches(e, *before, model)))
                {
                    indices = vec![prior.index, stage.index];
                    break;
                }
            }
        }
        ir::ContractKind::Response {
            trigger,
            targets,
            within,
        }
        | ir::ContractKind::NamedResponse {
            trigger,
            targets,
            within,
            ..
        } => {
            pending = if model.tracks_obligations() {
                trace
                    .response_origin(id)
                    .and_then(|(case, site, at, deadline)| {
                        steps
                            .iter()
                            .find(|s| {
                                s.time == at
                                    && business(s, Some(case)).any(|e| {
                                        e.binding == site
                                            && match trigger {
                                                ir::Trigger::Received => {
                                                    matches!(e.kind, Kind::Received | Kind::Receive)
                                                }
                                                ir::Trigger::Admitted => e.kind == Kind::Admitted,
                                                ir::Trigger::Event(pattern) => {
                                                    event_matches(e, *pattern, model)
                                                }
                                            }
                                    })
                            })
                            .map(|s| (s.index, deadline))
                    })
            } else {
                pending_trigger(steps, model, case_id, *trigger, targets, *within, None)
            };
        }
        ir::ContractKind::AtMostOnce { event } => {
            indices = steps
                .iter()
                .filter(|s| business(s, case_id).any(|e| event_matches(e, *event, model)))
                .map(|s| s.index)
                .take(2)
                .collect();
        }
        ir::ContractKind::Separation { left, right } => {
            'outer: for a in steps.iter() {
                for l in business(a, case_id).filter(|e| event_matches(e, *left, model)) {
                    for b in steps.iter() {
                        if business(b, case_id)
                            .any(|r| event_matches(r, *right, model) && r.executor == l.executor)
                        {
                            indices = vec![a.index, b.index];
                            break 'outer;
                        }
                    }
                }
            }
        }
        ir::ContractKind::AuthorityBasis { authority, .. } => {
            let mut current = Some(*authority);
            while let Some(id) = current {
                let a = &model.authorities[id.0];
                for span in [
                    Some(a.span),
                    a.restrictions.scope_span,
                    a.restrictions.expires_span,
                    a.restrictions.delegated_span,
                ]
                .into_iter()
                .flatten()
                {
                    if let Ok(location) = trace.location(span)
                        && !related.contains(&location)
                    {
                        related.push(location);
                    }
                }
                current = a.restrictions.delegated_from;
            }
            if let Some(v) = steps.last_mut().and_then(|s| s.violation.as_mut()) {
                v.deadline = model.authorities[authority.0]
                    .restrictions
                    .effective_expires;
            }
        }
        ir::ContractKind::Permitted { capability } => {
            let cap = &model.capabilities[capability.0];
            if let Ok(ast) = crate::syntax::parse(src) {
                for item in ast.items {
                    if let crate::ast::Item::Permit(permit) = item
                        && permit.target.actor.value == model.actors[cap.actor.0].name
                        && permit.target.capability.value == cap.name
                        && let Ok(location) = trace.location(permit.span)
                    {
                        related.push(location);
                    }
                }
            }
        }
        _ => {}
    }
    if let Some(violation) = steps.last_mut().and_then(|s| s.violation.as_mut()) {
        violation.related_steps = indices;
        if let Some((trigger, deadline)) = pending {
            violation.trigger_step = Some(trigger);
            violation.deadline = Some(deadline);
        }
    }
    related
}

/// Recover exact source effects without adding source metadata to execution keys.
pub(crate) fn state_invariant_locations(
    src: &str,
    model: &ir::Model,
    id: &str,
    steps: &[Stage],
    choice: Option<&KernelChoice>,
) -> Result<Vec<Location>, ReplayReason> {
    let clause = model
        .contracts
        .iter()
        .enumerate()
        .find(|(n, c)| c.property_id(ir::ClauseId(*n)) == id)
        .map(|(_, c)| c)
        .ok_or(ReplayReason::SourceMappingFailed)?;
    if !matches!(clause.kind, ir::ContractKind::StateInvariant { .. }) {
        return Ok(Vec::new());
    }
    let last = steps.last().ok_or(ReplayReason::SourceMappingFailed)?;
    let detail = last
        .violation
        .as_ref()
        .and_then(|v| v.state_invariant.as_ref())
        .ok_or(ReplayReason::SourceMappingFailed)?;
    let mut spans = vec![clause.span];
    if matches!(last.kind, Kind::Initial | Kind::Initialize | Kind::Receive) {
        spans.extend(detail.values.iter().map(|v| v.location.span));
    } else if matches!(
        last.kind,
        Kind::Start | Kind::Finish | Kind::ContestResponse
    ) {
        let cap = model
            .capabilities
            .iter()
            .find(|c| {
                Some(c.name.as_str()) == last.capability.as_deref()
                    && Some(model.actors[c.actor.0].name.as_str()) == last.actor.as_deref()
            })
            .ok_or(ReplayReason::SourceMappingFailed)?;
        let ast = crate::syntax::parse(src).map_err(|_| ReplayReason::SourceMappingFailed)?;
        let declaration = ast
            .items
            .iter()
            .find_map(|i| match i {
                crate::ast::Item::Actor(a) if a.name.value == model.actors[cap.actor.0].name => {
                    a.items.iter().find_map(|i| match i {
                        crate::ast::ActorItem::Capability(c) if c.name.value == cap.name => Some(c),
                        _ => None,
                    })
                }
                _ => None,
            })
            .ok_or(ReplayReason::SourceMappingFailed)?;
        let writes = if last.kind == Kind::Start {
            &cap.start_writes
        } else {
            &cap.writes
        };
        let kernel_assignments = state_kernel_assignments(&ast, model, cap, last, choice)?;
        let mut variables: Vec<_> = writes.iter().map(|w| w.variable).collect();
        if matches!(last.kind, Kind::Finish | Kind::ContestResponse)
            && let (Some(kernel), Some(choice)) = (source_kernel(cap, choice)?, choice)
        {
            for (variable, _) in &kernel.rows[choice.row].branches[choice.branch].overrides {
                if !variables.contains(variable) {
                    variables.push(*variable);
                }
            }
        }
        for variable in variables {
            let var = &model.state_variables[variable.0];
            let name = match var.kind {
                ir::StateKind::ActorObserved(a)
                | ir::StateKind::ActorLatent(a)
                | ir::StateKind::Rule(a) => {
                    format!("{}.{}", model.actors[a.0].name, var.name)
                }
                _ => var.name.clone(),
            };
            if !detail
                .values
                .iter()
                .any(|v| v.location.span == var.span && v.name == name)
            {
                continue;
            }
            let matches = |target: &crate::ast::StateTarget| match target {
                crate::ast::StateTarget::Unqualified(n) => n.value == var.name,
                crate::ast::StateTarget::Qualified {
                    actor, variable, ..
                } => {
                    variable.value == var.name
                        && match var.kind {
                            ir::StateKind::ActorObserved(a)
                            | ir::StateKind::ActorLatent(a)
                            | ir::StateKind::Rule(a) => actor.value == model.actors[a.0].name,
                            _ => false,
                        }
                }
            };
            let assignment = kernel_assignments
                .iter()
                .find(|a| matches(&a.target))
                .or_else(|| {
                    declaration.items.iter().find_map(|item| match &item.value {
                        crate::ast::CapItem::Start(xs) if last.kind == Kind::Start => {
                            xs.iter().find(|a| matches(&a.target))
                        }
                        crate::ast::CapItem::Finish(xs)
                            if matches!(last.kind, Kind::Finish | Kind::ContestResponse) =>
                        {
                            xs.iter().find(|a| matches(&a.target))
                        }
                        _ => None,
                    })
                });
            let span = if let Some(a) = assignment {
                diag::Span {
                    start: a.target.span().start,
                    end: a.value.span.end,
                }
            } else {
                declaration
                    .items
                    .iter()
                    .find_map(|item| match &item.value {
                        crate::ast::CapItem::Writes(xs)
                            if matches!(last.kind, Kind::Finish | Kind::ContestResponse) =>
                        {
                            xs.iter().find(|t| matches(t)).map(|t| t.span())
                        }
                        _ => None,
                    })
                    .ok_or(ReplayReason::SourceMappingFailed)?
            };
            spans.push(span);
        }
    }
    let mut result = Vec::new();
    for span in spans {
        let l = Location::new(src, span)?;
        if !result.contains(&l) {
            result.push(l);
        }
    }
    Ok(result)
}

fn source_kernel<'a>(
    cap: &'a ir::Capability,
    choice: Option<&KernelChoice>,
) -> Result<Option<&'a ir::Kernel>, ReplayReason> {
    if let Some(source) = &cap.performance {
        let selector = choice
            .and_then(|c| c.frozen_performance.as_ref())
            .filter(|s| s.profile == source.profile)
            .ok_or(ReplayReason::SourceMappingFailed)?;
        Ok(Some(
            source
                .kernels
                .get(selector.row)
                .ok_or(ReplayReason::SourceMappingFailed)?,
        ))
    } else if choice.is_some_and(|c| c.frozen_performance.is_some()) {
        Err(ReplayReason::SourceMappingFailed)
    } else {
        Ok(cap.kernel.as_ref())
    }
}

fn state_kernel_assignments<'a>(
    ast: &'a crate::ast::Model,
    model: &ir::Model,
    cap: &ir::Capability,
    last: &Stage,
    choice: Option<&KernelChoice>,
) -> Result<&'a [crate::ast::Assignment], ReplayReason> {
    if !matches!(last.kind, Kind::Finish | Kind::ContestResponse) {
        return Ok(&[]);
    }
    let Some(kernel) = source_kernel(cap, choice)? else {
        return Ok(&[]);
    };
    let choice = choice
        .filter(|c| {
            Some(c.case_id) == last.case_id && Some(c.binding.as_str()) == last.binding.as_deref()
        })
        .ok_or(ReplayReason::SourceMappingFailed)?;
    let row = kernel
        .rows
        .get(choice.row)
        .ok_or(ReplayReason::SourceMappingFailed)?;
    let branch = row
        .branches
        .get(choice.branch)
        .filter(|b| !b.probability.is_zero())
        .ok_or(ReplayReason::SourceMappingFailed)?;
    let declaration = ast
        .items
        .iter()
        .find_map(|i| match i {
            crate::ast::Item::Kernel(k) if k.name.value == kernel.name => Some(k),
            _ => None,
        })
        .ok_or(ReplayReason::SourceMappingFailed)?;
    let values: Vec<_> = kernel
        .given
        .iter()
        .zip(&row.values)
        .map(|(id, v)| match v {
            ir::StateValue::None => Ok("none".into()),
            ir::StateValue::Integer(n) => Ok(n.to_string()),
            ir::StateValue::Value(v) => {
                Ok(model.types[model.state_variables[id.0].base.0].members[v.0].clone())
            }
            // Kernel givens never hold a case set (lowering refuses it, ADR0035).
            ir::StateValue::Set(_) => Err(ReplayReason::SourceMappingFailed),
        })
        .collect::<Result<_, _>>()?;
    let source_row = declaration
        .rows
        .iter()
        .find(|r| {
            r.pattern.as_ref().is_some_and(|p| {
                p.len() == values.len() && p.iter().zip(&values).all(|(p, v)| p.value == *v)
            })
        })
        .or_else(|| declaration.rows.iter().find(|r| r.pattern.is_none()))
        .ok_or(ReplayReason::SourceMappingFailed)?;
    let source_branch = source_row
        .branches
        .get(choice.branch)
        .ok_or(ReplayReason::SourceMappingFailed)?;
    if canonical_result(&source_branch.result, model, cap.return_type).as_deref()
        != Some(model.types[cap.return_type.0].members[branch.result.0].as_str())
    {
        return Err(ReplayReason::SourceMappingFailed);
    }
    Ok(&source_branch.assignments)
}

/// The written kernel result as the canonical member: a scalar name as is, an
/// ADR 0038 record literal in declaration order of the record's fields.
fn canonical_result(
    expr: &crate::ast::ValueExpr,
    model: &ir::Model,
    ty: ir::TypeId,
) -> Option<String> {
    let Some(fields) = &expr.fields else {
        return Some(expr.head.value.clone());
    };
    let t = model.types.get(ty.0)?;
    let shape = t.record.as_ref()?;
    if expr.head.value != t.name || fields.len() != shape.fields.len() {
        return None;
    }
    let parts: Option<Vec<_>> = shape
        .fields
        .iter()
        .map(|f| {
            fields
                .iter()
                .find(|(n, _)| n.value == f.name)
                .map(|(_, v)| format!("{} = {}", f.name, v.value))
        })
        .collect();
    Some(format!("{} {{ {} }}", t.name, parts?.join(", ")))
}

fn resolve_obligations(model: &ir::Model, steps: &mut [Stage]) -> Result<(), ReplayReason> {
    // Resolve for every target, including the diagnostic (which is not a clause).
    let details: Vec<_> = steps
        .iter()
        .flat_map(|s| {
            s.deadlock_detail
                .as_ref()
                .into_iter()
                .flat_map(|d| d.obligations.iter())
        })
        .map(|o| (o.case_id, o.id.clone(), o.deadline))
        .collect();
    let resolved: Vec<_> = details
        .iter()
        .map(|(case, id, deadline)| {
            model
                .contracts
                .iter()
                .enumerate()
                .find(|(n, c)| c.property_id(ir::ClauseId(*n)) == *id)
                .and_then(|(_, c)| match &c.kind {
                    ir::ContractKind::Response {
                        trigger,
                        targets,
                        within,
                    }
                    | ir::ContractKind::NamedResponse {
                        trigger,
                        targets,
                        within,
                        ..
                    } => pending_trigger(
                        steps,
                        model,
                        Some(*case),
                        *trigger,
                        targets,
                        *within,
                        Some(*deadline),
                    ),
                    _ => None,
                })
        })
        .collect();
    for stage in steps.iter_mut() {
        if let Some(detail) = &mut stage.deadlock_detail {
            for (obligation, resolved) in detail.obligations.iter_mut().zip(&resolved) {
                let (trigger, deadline) = resolved.ok_or(ReplayReason::ViolationMismatch)?;
                if deadline != obligation.deadline {
                    return Err(ReplayReason::ViolationMismatch);
                }
                obligation.trigger_step = trigger;
            }
        }
    }
    Ok(())
}

fn event_matches(event: &BusinessEvent, pattern: ir::Event, model: &ir::Model) -> bool {
    let cap = &model.capabilities[pattern.capability.0];
    event.actor.as_deref() == Some(model.actors[cap.actor.0].name.as_str())
        && event.capability.as_deref() == Some(cap.name.as_str())
        && match pattern.kind {
            ir::EventKind::CancelAck => event.kind == Kind::CancelAck,
            ir::EventKind::Start => event.kind == Kind::Start,
            ir::EventKind::Commit => event.kind == Kind::Commit,
            ir::EventKind::Finish(v) => {
                event.kind == Kind::Finish
                    && v.is_none_or(|v| {
                        event.result.as_deref()
                            == Some(model.types[cap.return_type.0].members[v.0].as_str())
                    })
            }
        }
}

#[cfg(test)]
pub(crate) fn synthetic_safety_report(
    src: &str,
    model: &ir::Model,
    trace: &crate::engine::trace::Capture,
) -> Counterexample {
    let verify = model
        .verify
        .iter()
        .find(|v| v.property == ir::Property::NoResourceOvercommit)
        .expect("safety requested");
    let mut out = Explanation::blocked();
    add_counterexample(
        &mut out,
        src,
        model,
        trace,
        "safety no_resource_overcommit",
        Some(verify.span),
    );
    out.counterexamples.remove(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_property_span_fails_closed() {
        let src = include_str!("../../../fixtures/explain-once.broken-repeat-result/model.relca");
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let (_, capture) = crate::engine::trace::explore(src, &m);
        let id = m.contracts[0].property_id(ir::ClauseId(0));
        let mut out = Explanation::blocked();
        add_counterexample(
            &mut out,
            src,
            &m,
            &capture,
            &id,
            Some(diag::Span {
                start: 0,
                end: src.len() + 1,
            }),
        );
        assert_eq!(
            out.counterexamples[0].reason,
            Some(ReplayReason::SourceMappingFailed)
        );
        assert!(out.counterexamples[0].steps.is_empty());
    }
    #[test]
    fn capture_limit_preserves_check_projection() {
        let src = include_str!("../../../fixtures/explain-once.broken-repeat-result/model.relca");
        let check = serde_json::to_value(report::check(src)).unwrap();
        let r = explain_with_limit(src, 2);
        assert_eq!(
            crate::ledger_test_shape::original_report(&serde_json::to_value(&r.report).unwrap()),
            crate::ledger_test_shape::original_report(&check)
        );
        assert_eq!(r.explanation.status, Status::Unsupported);
        assert!(
            r.explanation
                .diagnostics
                .iter()
                .any(|d| d.message.contains("CaptureLimit"))
        );
        assert!(
            r.explanation
                .counterexamples
                .iter()
                .all(|c| c.steps.is_empty())
        );
    }
    #[test]
    fn capture_limit_preserves_all_routes_even_without_a_violation() {
        for src in [
            include_str!("../../../fixtures/explain-response/model.relca"),
            include_str!("../../../fixtures/prob-zero/model.relca"),
            include_str!("../../../fixtures/arrivals-contracts/model.relca"),
        ] {
            let check = serde_json::to_value(report::check(src)).unwrap();
            for limit in [0, 1, 2, 5] {
                let r = explain_with_limit(src, limit);
                assert_eq!(
                    crate::ledger_test_shape::original_report(
                        &serde_json::to_value(&r.report).unwrap()
                    ),
                    crate::ledger_test_shape::original_report(&check)
                );
                assert_eq!(r.explanation.status, Status::Unsupported);
                assert!(
                    r.explanation
                        .diagnostics
                        .iter()
                        .any(|d| d.message.contains("capture_limit"))
                );
                assert!(
                    !r.explanation
                        .checked
                        .iter()
                        .any(|s| s == "capture" || s == "shortest_path")
                );
            }
        }
    }

    #[test]
    fn indexed_locations_match_original_scalar_coordinates_at_every_boundary() {
        let src = "α\tβ\r\n日本語\n\nend\r\n";
        let starts = Location::line_starts(src);
        for start in (0..=src.len()).filter(|n| src.is_char_boundary(*n)) {
            for end in (start..=src.len()).filter(|n| src.is_char_boundary(*n)) {
                let location = Location::indexed(src, &starts, diag::Span { start, end }).unwrap();
                let old = |byte| {
                    src[..byte].chars().fold((1, 1), |(line, col), c| {
                        if c == '\n' {
                            (line + 1, 1)
                        } else {
                            (line, col + 1)
                        }
                    })
                };
                assert_eq!((location.line, location.column), old(start));
                assert_eq!((location.end_line, location.end_column), old(end));
            }
        }
        assert_eq!(
            Location::indexed(src, &starts, diag::Span { start: 1, end: 2 }),
            Err(ReplayReason::SourceMappingFailed)
        );
    }
    #[test]
    fn deadlock_obligation_resolves_noninitial_trigger() {
        let src = "model M version \"0.2\" {type T=enum{ok} actor Gate{capacity 1;capability check()->T{duration fixed 1min;result nondet{ok}}} actor Worker{capacity 0;capability run()->T{duration fixed 1min;result nondet{ok}}} process P{x=Gate.check();y=Worker.run();end done} bounds{cases 1;live_tasks 2;clock 10min} contract C{response Gate.check.finish -> one_of(end.done) by 5min after Gate.check.finish otherwise unresolved}}";
        let r = explain(src);
        let c = r
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id == "diagnostic:W-DEADLOCK-0")
            .unwrap();
        assert_eq!(c.status, ReplayStatus::Verified);
        // initial 0, queue 1, start 2, advance 3, finish 4, join 5,
        // queue blocked Worker 6, deadlock 7.
        let detail = c.steps.last().unwrap().deadlock_detail.as_ref().unwrap();
        assert_eq!(detail.obligations.len(), 1);
        assert_eq!(detail.obligations[0].trigger_step, 4);
    }
    #[test]
    fn concurrent_same_capability_calls_keep_the_second_binding() {
        let src = "model M version \"0.2\" {type T=enum{ok} actor A{capacity 2;capability run()->T{commits;duration fixed 1min;result nondet{ok}}} process P{fork f{x=A.run();y=A.run()}join all f;end done} bounds{cases 1;live_tasks 2;clock 10min} contract C{at_most_once A.run.commit per case.id}}";
        let r = explain(src);
        let c = &r.explanation.counterexamples[0];
        assert_eq!(c.status, ReplayStatus::Verified);
        let finishes: Vec<_> = c
            .steps
            .iter()
            .filter(|s| s.kind == Kind::Finish)
            .map(|s| s.binding.as_deref().unwrap())
            .collect();
        assert_eq!(finishes, vec!["x", "y"]);
    }
    #[test]
    fn mapping_and_state_failures_are_scoped_to_explanation_not_root_fold() {
        let src=include_str!("../../../fixtures/explain-response/model.relca").replace("  process Flow {","  latent case {truth: bool=false}\n  process Flow {")
            .replace("response case.received -> one_of(end.done) by 5min after case.received otherwise unresolved","invariant Worker.run.finish -> truth == true");
        let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        let root = report::check(&src);
        let serialized = serde_json::to_value(&root).unwrap();
        let (_, trace) = crate::engine::trace::explore(&src, &m);
        let id = m.contracts[0].property_id(ir::ClauseId(0));
        for reason in [
            ReplayReason::SourceMappingFailed,
            ReplayReason::StateMismatch,
        ] {
            let mut broken = trace.clone();
            if reason == ReplayReason::SourceMappingFailed {
                broken.failure = Some(reason);
            } else {
                for edge in broken.edges.iter_mut().flatten() {
                    if edge.targets.contains(&id) {
                        edge.stage.values.latent[0].value = "true".into();
                    }
                }
            }
            let mut out = Explanation::blocked();
            add_counterexample(&mut out, &src, &m, &broken, &id, Some(m.contracts[0].span));
            assert_eq!(out.status, Status::Unsupported);
            assert_eq!(out.counterexamples[0].reason, Some(reason));
            assert!(out.counterexamples[0].steps.is_empty());
            assert_eq!(out.diagnostics[0].code, diag::U_SEM_0);
            assert_eq!(serde_json::to_value(&root).unwrap(), serialized);
            assert_eq!(root.verdict, Verdict::Violated);
        }
    }
}
