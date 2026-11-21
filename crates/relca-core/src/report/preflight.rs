//! Pure backend admission checks, shared by ordinary producers and the suite's
//! all-point barrier. No interpreter, RNG, source I/O or outcome construction.
use crate::{
    diag, ir,
    sensitivity::{input::InputError, statistics::SuiteMultiplicity},
};

pub(super) enum FiniteFailure {
    Actor(String),
    StochasticArrivals,
    Diagnostics,
    NoVerify,
}
pub(super) fn finite(m: &ir::Model) -> Result<(), FiniteFailure> {
    if let Err(e) = crate::actor_table::validate(m) {
        return Err(FiniteFailure::Actor(e.message.into()));
    }
    if m.arrival_distribution.is_some() {
        return Err(FiniteFailure::StochasticArrivals);
    }
    if m.diagnostics
        .iter()
        .any(|d| d.severity == diag::Severity::Error)
    {
        return Err(FiniteFailure::Diagnostics);
    }
    if m.verify.is_empty() {
        return Err(FiniteFailure::NoVerify);
    }
    Ok(())
}
impl FiniteFailure {
    pub(super) fn reason(&self, m: &ir::Model) -> &'static str {
        match self {
            Self::Actor(_) | Self::StochasticArrivals | Self::NoVerify => {
                "sensitivity_feature_combination"
            }
            Self::Diagnostics => diagnostics_reason(m),
        }
    }
    pub(super) fn diagnostics(self, m: &ir::Model) -> Vec<diag::Diagnostic> {
        match self {
            Self::Actor(message) => vec![error(diag::U_SEM_0, &message, None)],
            Self::StochasticArrivals => vec![
                error(
                    diag::U_SEM_0,
                    "stochastic_arrivals_mode_unsupported",
                    Some(m.process.span),
                )
                .unconnected(
                    "stochastic_arrivals_mode_unsupported",
                    "exhaustive_stochastic_arrivals",
                    arrival_blockers(m),
                ),
            ],
            Self::Diagnostics => m.diagnostics.clone(),
            Self::NoVerify => vec![error(
                diag::U_SEM_0,
                "No verification properties were requested",
                None,
            )],
        }
    }
}
fn error(code: &str, message: &str, span: Option<diag::Span>) -> diag::Diagnostic {
    diag::Diagnostic {
        presentation: Default::default(),
        code: code.into(),
        severity: diag::Severity::Error,
        message: message.into(),
        span,
    }
}

#[derive(Clone, Copy)]
pub(super) struct SimulationCounts {
    pub effective_rows: u64,
}
pub(super) enum SimulationFailure {
    MissingClause,
    Diagnostics,
    StochasticArrivals,
    WireRange,
    Scope { error: InputError },
}
pub(super) fn simulation(
    m: &ir::Model,
    scope: Option<&SuiteMultiplicity>,
) -> Result<SimulationCounts, SimulationFailure> {
    let Some(spec) = &m.simulation else {
        return Err(SimulationFailure::MissingClause);
    };
    if m.diagnostics
        .iter()
        .any(|d| d.severity == diag::Severity::Error)
    {
        return Err(SimulationFailure::Diagnostics);
    }
    if m.unsupported_stochastic_arrival_combination() {
        return Err(SimulationFailure::StochasticArrivals);
    }
    let local_rows = crate::evaluation::simulation_rows(m);
    if local_rows.is_none() || spec.replications.checked_mul(m.bounds.cases).is_none() {
        return Err(SimulationFailure::WireRange);
    }
    let local_rows = local_rows.unwrap();
    if let Some(scope) = scope {
        scope
            .validate_child(local_rows, &spec.confidence)
            .map_err(|error| SimulationFailure::Scope { error })?;
    }
    Ok(SimulationCounts {
        effective_rows: scope.map_or(local_rows, SuiteMultiplicity::rows),
    })
}
impl SimulationFailure {
    pub(super) fn reason(&self, m: &ir::Model) -> &'static str {
        match self {
            Self::MissingClause => "sensitivity_model_invalid",
            Self::Diagnostics => diagnostics_reason(m),
            Self::StochasticArrivals => "sensitivity_feature_combination",
            Self::WireRange => "sensitivity_capacity_limit",
            Self::Scope { error } => error.reason(),
        }
    }
    pub(super) fn diagnostics(
        self,
        m: &ir::Model,
        source_span: diag::Span,
    ) -> Vec<diag::Diagnostic> {
        match self {
            Self::MissingClause => vec![error(
                diag::E_SIM_0,
                "simulation requires an explicit simulation clause",
                Some(source_span),
            )],
            Self::Diagnostics => m.diagnostics.clone(),
            Self::StochasticArrivals => vec![stochastic_combination(m)],
            Self::WireRange => vec![error(
                diag::U_SEM_0,
                "simulation aggregate counts exceed the integer wire range",
                m.simulation.as_ref().map(|s| s.span),
            )],
            Self::Scope { error, .. } => vec![error.diagnostic()],
        }
    }
}

fn arrival_blockers(m: &ir::Model) -> Vec<diag::Blocker> {
    m.arrival_distribution
        .iter()
        .map(|a| diag::Blocker::at("feature", None, Some(a.span)))
        .collect()
}

fn stochastic_combination(m: &ir::Model) -> diag::Diagnostic {
    let reason = "stochastic_arrivals_feature_combination";
    let mut blockers = arrival_blockers(m);
    let mut cause = None;
    if let Some(quality) = &m.quality {
        cause = Some("quality");
        // Quality IR retains names but has no declaration spans.
        blockers.extend(
            quality
                .properties
                .iter()
                .map(|p| diag::Blocker::at("feature", Some(p.name.clone()), None)),
        );
        blockers.extend(
            quality
                .metrics
                .iter()
                .map(|p| diag::Blocker::at("feature", Some(p.name.clone()), None)),
        );
        blockers.extend(
            quality
                .means
                .iter()
                .map(|p| diag::Blocker::at("feature", Some(p.name.clone()), None)),
        );
        blockers.extend(
            quality
                .quantiles
                .iter()
                .map(|p| diag::Blocker::at("feature", Some(p.name.clone()), None)),
        );
    }
    for (call, attempt) in m.response_attempts() {
        cause.get_or_insert("response_or_retry");
        blockers.push(diag::Blocker::for_call("call", m, call, attempt.command));
    }
    for contest in m.contests() {
        cause.get_or_insert("contest");
        blockers.push(diag::Blocker::at("contest", None, Some(contest.span)));
    }
    let waits = m.condition_wait_blockers();
    if !waits.is_empty() {
        cause.get_or_insert("condition_wait");
        blockers.extend(waits);
    }
    if let Some(cut) = &m.cut {
        for entry in &cut.defer {
            cause.get_or_insert("cut_defer");
            let name = match &entry.target {
                ir::CutTarget::Actor(actor) => m.actors.get(actor.0).map(|a| a.name.clone()),
                ir::CutTarget::Label(label) => Some(label.clone()),
            };
            blockers.push(diag::Blocker::at("feature", name, None));
        }
    }
    let d = error(diag::U_SEM_0, reason, Some(m.process.span));
    match cause {
        Some(cause) => d.unconnected(reason, cause, blockers),
        None => d,
    }
}

// Only Diagnostics inherits the already classified IR diagnostics. Typed
// admission failures above never infer their reason from a diagnostic code.
fn diagnostics_reason(m: &ir::Model) -> &'static str {
    if m.diagnostics
        .iter()
        .any(|d| d.severity == diag::Severity::Error && !d.code.starts_with("U-"))
    {
        "sensitivity_model_invalid"
    } else {
        "sensitivity_feature_combination"
    }
}
