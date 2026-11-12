//! Resolved engine input. Only lower constructs accepted models; public fields
//! also permit direct engine test builders, which must respect these invariants.

use crate::diag::{Diagnostic, Span};
use serde::{Deserialize, Serialize};

/// Integer minutes. Arithmetic in engine is checked, never wrapping/saturating.
pub type Tick = u64;

/// Builtin type and argument tags, disjoint from all declaration-table indices.
pub const CASE_ID_TYPE: TypeId = TypeId(usize::MAX - 1);
pub const CURRENT_CASE: BindingId = BindingId(usize::MAX);

macro_rules! index {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub usize);
    };
}

index!(
    ActorId,
    "Index into Model.actors, stable in source declaration order."
);
index!(
    CapabilityId,
    "Index into Model.capabilities (a global, not per-Actor table)."
);
index!(TypeId, "Index into Model.types.");
index!(StateVarId, "Index into Model.state_variables.");
index!(CaseId, "Zero-based position in the fixed arrival list.");
index!(PersonId, "Index into Model.people.");
index!(
    ResourceId,
    "Index into Model.resources, in declaration order."
);
index!(AuthorityId, "Index into Model.authorities.");
index!(
    ValueId,
    "Index into EnumType.members, qualified by the accompanying TypeId."
);
index!(
    BindingId,
    "Index into Process.bindings; a call site defines one slot."
);
index!(
    ForkId,
    "Index into Process.forks; unique for each lexical fork site."
);
index!(
    EndId,
    "Index into Model.ends; equal terminal names share an EndId."
);
index!(
    ClauseId,
    "Index into Model.contracts, in model-wide clause order."
);

/// Exactly one process and explicit finite bounds in the supported subset.
/// All indices are valid; result comparisons read definitely available results,
/// while state comparisons read the current case/Actor state.
/// Diagnostics here are warnings only; an error makes lower return Err instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub performances: Vec<Performance>,
    pub case_priority: Option<CasePriority>,
    pub arrival_distribution: Option<ArrivalDistribution>,
    /// None retains the legacy common analysis window (ADR 0059).
    pub resource_time: Option<ResourceTime>,
    pub concurrency_resolutions: Vec<ResolvedPair>,
    /// Configuration intent only; never consulted by either interpreter.
    pub fallback_capabilities: Vec<CapabilityId>,
    pub quality: Option<QualitySpec>,
    pub comparison: Option<ComparisonSpec>,
    pub resources: Vec<Resource>,
    pub metrics: Option<MetricsSpec>,
    /// Resolved responsibility and contest declarations. Static structure and
    /// runtime contest response are separate verification obligations.
    pub responsibility: Option<crate::responsibility::Metadata>,
    pub simulation: Option<SimulationSpec>,
    pub arrivals: Option<Vec<Tick>>,
    pub arrival_every: Option<FixedEveryRecord>,
    pub scenario: Option<ScenarioRecord>,
    pub horizon: Option<Tick>,
    pub start_policy: Option<StartPolicy>,
    pub admission_queue: Option<AdmissionQueue>,
    pub concurrent_cases: Option<AdmissionLimit>,
    pub overflow_end: Option<EndId>,
    pub kernels: Vec<Kernel>,
    pub initial_distributions: Vec<Vec<(StateValue, crate::probability::Rational)>>,
    pub probability_queries: Vec<ProbabilityQuery>,
    pub state_variables: Vec<StateVariable>,
    pub people: Vec<Person>,
    pub authorities: Vec<Authority>,
    /// ADR 0010 clauses, independent of which process is selected. Each has a
    /// matching Contract request in verify, including inactive clauses.
    pub contracts: Vec<ContractClause>,
    /// Static ADR 0009 summary; execution does not consult it.
    pub agency: crate::agency::AgencyDerivation,
    pub name: String,
    pub version: String,
    pub types: Vec<EnumType>,
    pub actors: Vec<Actor>,
    pub capabilities: Vec<Capability>,
    pub process: Process,
    pub ends: Vec<String>,
    pub bounds: Bounds,
    pub verify: Vec<Verify>,
    pub assumptions: Vec<Assumption>,
    pub diagnostics: Vec<Diagnostic>,
    /// None when the model declares no Cut (W-CUT-0 then stays in diagnostics).
    pub cut: Option<Cut>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedEveryRecord {
    pub every: Tick,
    pub from: Tick,
    pub until: Tick,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrivalDistribution {
    pub basis: String,
    pub entries: Vec<(Vec<Tick>, crate::probability::Rational)>,
    pub span: Span,
}

impl ArrivalDistribution {
    /// Declaration envelope, not the arrivals realized by any one replication.
    pub(crate) fn declared_bounds(&self) -> Option<(Tick, Tick)> {
        let from = self
            .entries
            .iter()
            .filter_map(|(times, _)| times.first().copied())
            .min()?;
        let until = self
            .entries
            .iter()
            .filter_map(|(times, _)| times.last().copied())
            .max()?;
        Some((from, until))
    }

    pub(crate) fn validate(&self, m: &Model) -> Result<(), &'static str> {
        if self.basis.trim().is_empty()
            || self.entries.is_empty()
            || self.entries.len() > 256
            || m.bounds.cases > 10000
        {
            return Err("invalid stochastic arrival distribution");
        }
        let mut total = crate::probability::Rational::zero();
        let mut seen = std::collections::BTreeSet::new();
        for (times, p) in &self.entries {
            if !p.valid()
                || times.is_empty()
                || times.len() as u64 != m.bounds.cases
                || times.windows(2).any(|w| w[0] > w[1])
                || m.horizon.is_some_and(|h| times.iter().any(|t| *t > h))
                || !seen.insert(times)
            {
                return Err("invalid stochastic arrival support");
            }
            total.0 += &p.0;
        }
        if total != crate::probability::Rational::one()
            || m.arrivals.as_ref() != self.entries.first().map(|e| &e.0)
        {
            return Err("invalid stochastic arrival mass or shape record");
        }
        Ok(())
    }
}

/// Absolute, fully covered start-slot tables, not physical safety capacities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceTime {
    pub overtime_rates: Vec<Option<u64>>,
    pub calendars: Vec<Calendar>,
    pub actors: Vec<Option<usize>>,
    pub resources: Vec<Option<usize>>,
    pub people: Vec<Option<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Calendar {
    pub name: String,
    pub through: Tick,
    pub intervals: Vec<CalendarInterval>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CalendarInterval {
    pub from: Tick,
    pub until: Tick,
    pub capacity: u64,
}

impl Calendar {
    /// No implicit capacity outside the explicitly declared partition.
    pub fn capacity_at(&self, time: Tick) -> Option<u64> {
        self.intervals
            .iter()
            .find(|i| i.from <= time && time < i.until)
            .map(|i| i.capacity)
    }
}

impl ResourceTime {
    pub(crate) fn validate(&self, m: &Model) -> Result<(), &'static str> {
        let bad = "invalid_calendar_ir";
        if self.actors.len() != m.actors.len()
            || self.overtime_rates.len() != m.actors.len()
            || self.resources.len() != m.resources.len()
            || self.people.len() != m.people.len()
        {
            return Err(bad);
        }
        let mut names = std::collections::BTreeSet::new();
        for c in &self.calendars {
            if c.name.is_empty() || !names.insert(&c.name) || c.intervals.is_empty() {
                return Err(bad);
            }
            let mut end = 0;
            for i in &c.intervals {
                if i.from != end || i.until <= i.from {
                    return Err(bad);
                }
                end = i.until;
            }
            if end != c.through {
                return Err(bad);
            }
        }
        let check = |id: Option<usize>, max: u64| -> Result<(), &'static str> {
            if let Some(id) = id {
                let c = self.calendars.get(id).ok_or(bad)?;
                if c.through <= m.bounds.clock || c.intervals.iter().any(|i| i.capacity > max) {
                    return Err(bad);
                }
            }
            Ok(())
        };
        for (i, a) in m.actors.iter().enumerate() {
            check(self.actors[i], a.capacity.unwrap_or(0))?;
        }
        for (i, r) in m.resources.iter().enumerate() {
            check(self.resources[i], r.capacity)?;
        }
        for id in &self.people {
            check(*id, 1)?;
        }
        for c in &m.capabilities {
            if c.duration == 0
                && c.duration_distribution
                    .as_ref()
                    .is_none_or(|d| d.entries.iter().all(|(t, _)| *t == 0))
            {
                continue;
            }
            let a = m.actors.get(c.actor.0).ok_or(bad)?;
            if self.actors[c.actor.0].is_none()
                || a.executors
                    .iter()
                    .flatten()
                    .any(|p| self.people.get(p.0).is_none_or(Option::is_none))
                || c.uses
                    .iter()
                    .any(|u| self.resources.get(u.resource.0).is_none_or(Option::is_none))
            {
                return Err(bad);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonSpec {
    pub plans: Vec<String>,
    pub under: Option<Vec<String>>,
    pub cut: Option<String>,
    pub hard: Vec<HardConstraint>,
    pub objectives: Vec<Objective>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardConstraint {
    pub cohort: usize,
    pub op: HardComparison,
    pub bound: crate::probability::Rational,
    pub span: Span,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardComparison {
    Le,
    Lt,
    Ge,
    Gt,
}
impl HardComparison {
    pub fn text(self) -> &'static str {
        match self {
            Self::Le => "<=",
            Self::Lt => "<",
            Self::Ge => ">=",
            Self::Gt => ">",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Minimize,
    Maximize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Objective {
    pub direction: Direction,
    pub target: ObjectiveRef,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectiveRef {
    Cohort(usize),
    Quantile(usize),
    Overall(OverallObjective),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverallObjective {
    Work,
    NamedPeopleWork,
    PeakTasks,
    ObservedCostTotal,
    ObservedCostPerArrival,
}
impl OverallObjective {
    pub fn text(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::NamedPeopleWork => "named_people_work",
            Self::PeakTasks => "peak_tasks",
            Self::ObservedCostTotal => "observed_cost_total",
            Self::ObservedCostPerArrival => "observed_cost_per_arrival",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioRecord {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualitySpec {
    pub properties: Vec<QualityProperty>,
    pub metrics: Vec<CohortMetric>,
    pub quantiles: Vec<QuantileMetric>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantileMetric {
    pub name: String,
    pub p: crate::probability::Rational,
    pub property: usize,
    pub given_reached: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityProperty {
    pub name: String,
    pub target: QualityTarget,
    pub within: Option<Tick>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualityTarget {
    State(Cond),
    Ends(Vec<EndId>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortMetric {
    pub name: String,
    pub numerator: usize,
    pub denominator: Denominator,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denominator {
    Arrivals,
    Property(usize),
}

/// Resolved ADR 0019 fees, indexed by the declaration tables. Required sets
/// describe lexical calls of the selected process, including unreachable calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricsSpec {
    pub breakdown_waits: bool,
    pub window: Tick,
    pub currency: String,
    pub actor_rates: Vec<Option<u64>>,
    pub start_rates: Vec<Option<u64>>,
    pub required_actors: Vec<ActorId>,
    pub required_capabilities: Vec<CapabilityId>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationSpec {
    pub replications: u64,
    pub seed: u64,
    pub confidence: crate::probability::Rational,
    pub span: Span,
    pub replications_span: Span,
    pub seed_span: Span,
    pub confidence_span: Span,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartPolicy {
    /// None is the legacy FIFO discipline, not an implicit priority tie.
    pub priority: Option<QueueTie>,
    pub observable: bool,
    pub call_order: bool,
    pub assign_first_idle: bool,
    /// Execution classification, independent of display-only ActorMetadata.
    pub classes: Option<PolicyClasses>,
    /// Source executors order, indexed by Actor, only for extended policies.
    /// Actor.executors retains its historical canonical set order for static
    /// reports/Agency/diff. Each named row is a permutation of that exact set.
    pub executor_order: Vec<Option<Vec<PersonId>>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyClasses {
    pub policy: Vec<ActorId>,
    pub environment: Vec<ActorId>,
    pub party: Vec<ActorId>,
}

/// ADR 0062: ties are nondeterministic unless explicitly ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueTie {
    Nondet,
    FifoCaseId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionTie {
    Nondet,
    CaseId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasePriority {
    pub ranks: Vec<u64>,
    pub classes: PolicyClasses,
    pub span: Span,
}

impl StartPolicy {
    pub fn extended(&self) -> bool {
        self.priority.is_some() || self.call_order || self.assign_first_idle
    }
    pub fn controls(&self, actor: ActorId) -> bool {
        self.classes
            .as_ref()
            .is_some_and(|c| c.policy.contains(&actor))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionQueue {
    pub capacity: u64,
    pub overflow: Overflow,
}

pub type AdmissionLimit = AdmissionQueue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    Reject,
    WaitExternalFifo,
    WaitExternalPriority(QueueTie),
    End(EndId),
}

impl Model {
    pub(crate) fn unsupported_stochastic_arrival_combination(&self) -> bool {
        self.arrival_distribution.is_some()
            && (self.quality.is_some()
                || !self.response_attempts().is_empty()
                || !self.contests().is_empty()
                || self.has_condition_wait()
                || self.cut.as_ref().is_some_and(|c| !c.defer.is_empty()))
    }
    pub fn explicit_limits(&self) -> bool {
        self.concurrent_cases.is_some()
            || self
                .admission_queue
                .is_some_and(|q| q.overflow != Overflow::Reject)
    }
    pub fn external_wait(&self) -> bool {
        self.admission_queue
            .iter()
            .chain(self.concurrent_cases.iter())
            .any(|q| {
                matches!(
                    q.overflow,
                    Overflow::WaitExternalFifo | Overflow::WaitExternalPriority(_)
                )
            })
    }
    pub fn external_priority(&self) -> Option<QueueTie> {
        self.admission_queue
            .iter()
            .chain(self.concurrent_cases.iter())
            .find_map(|q| {
                if let Overflow::WaitExternalPriority(tie) = q.overflow {
                    Some(tie)
                } else {
                    None
                }
            })
    }
    pub fn loss_property_id(&self, id: StateVarId) -> String {
        let v = &self.state_variables[id.0];
        format!(
            "no_lost_update {}.{}",
            self.actors[v.kind.actor().expect("shared observed state").0].name,
            v.name
        )
    }
    pub(crate) fn completion_requested(&self) -> bool {
        self.verify.iter().any(|v| v.property.completion())
    }
    /// Effective symmetric overlap relation. Actor rows separately retain which
    /// endpoint actually declared each edge (including an explicit empty list).
    pub fn overlap_pairs(&self) -> std::collections::BTreeSet<(ActorId, ActorId)> {
        self.actors
            .iter()
            .enumerate()
            .flat_map(|(i, a)| {
                a.metadata.overlaps.iter().flatten().map(move |id| {
                    let a = ActorId(i);
                    if a < *id { (a, *id) } else { (*id, a) }
                })
            })
            .collect()
    }
    pub fn tracks_obligations(&self) -> bool {
        !self.contests().is_empty()
            || self.contracts.iter().any(|c| {
                matches!(
                    c.kind,
                    ContractKind::NamedResponse { .. }
                        | ContractKind::Excludes { .. }
                        | ContractKind::Waive { .. }
                )
            })
    }
    pub fn contests(&self) -> &[crate::contest::Contest] {
        self.responsibility
            .as_ref()
            .map_or(&[], |m| m.contests.as_slice())
    }
    /// Synthetic invocation sites occupy a disjoint suffix; these are not
    /// process result slots and must never index Process::bindings/results.
    pub(crate) fn contest_site(&self, contest: usize) -> BindingId {
        BindingId(self.process.bindings.len() + contest)
    }
    pub(crate) fn site_contest(&self, site: BindingId) -> Option<usize> {
        site.0
            .checked_sub(self.process.bindings.len())
            .filter(|i| *i < self.contests().len())
    }
    pub(crate) fn site_name(&self, site: BindingId) -> String {
        match self.site_contest(site) {
            // ':' cannot occur in a source binding identifier. The wire site
            // namespace therefore cannot alias a legal binding named contest1.
            Some(id) => format!("contest:{}", id + 1),
            None => self.process.bindings[site.0].name.clone(),
        }
    }
    /// Cross-configuration identity, not the physical slot/replay/RNG name.
    /// ':' cannot occur in a source binding, so commands and attempts cannot
    /// alias ordinary calls or the separate contest: namespace.
    pub(crate) fn comparison_site_name(&self, site: BindingId) -> String {
        if let Some(r) = self.response_attempt(site) {
            format!(
                "response:{}:attempt{}",
                self.site_name(r.command),
                r.attempt
            )
        } else {
            self.site_name(site)
        }
    }
    pub fn legacy_route(&self) -> bool {
        self.bounds.cases == 1
            && !self.uses_work_profiles()
            && self.process.degradations.is_empty()
            && !self.uses_priority()
            && self.resource_time.is_none()
            && !self.has_condition_wait()
            && self.response_attempts().is_empty()
            && self.contests().iter().all(|c| c.rates.is_none())
            && self.scenario.is_none()
            && self.arrivals.is_none()
            && self.start_policy.is_none()
            && self.admission_queue.is_none()
            && self.concurrent_cases.is_none()
    }

    pub fn uses_priority(&self) -> bool {
        self.start_policy
            .as_ref()
            .is_some_and(|p| p.priority.is_some())
            || self.external_priority().is_some()
            || self.capabilities.iter().any(|c| {
                c.version_check
                    .as_ref()
                    .is_some_and(|v| v.resolve.is_some())
            })
    }

    pub fn uses_work_profiles(&self) -> bool {
        self.capabilities
            .iter()
            .any(|c| c.qualified_executors.is_some() || c.performance.is_some())
    }

    /// Cut indices are public IR too. Check them before either interpreter can
    /// collect observations; an invalid declaration is not an opaque label.
    pub(crate) fn validate_cut_targets(&self) -> Result<(), &'static str> {
        if let Some(cut) = &self.cut {
            for id in
                cut.include
                    .iter()
                    .copied()
                    .chain(
                        cut.exclude
                            .iter()
                            .chain(&cut.defer)
                            .filter_map(|e| match e.target {
                                CutTarget::Actor(id) => Some(id),
                                CutTarget::Label(_) => None,
                            }),
                    )
            {
                if id.0 >= self.actors.len() {
                    return Err("invalid Cut Actor index");
                }
            }
        }
        Ok(())
    }

    /// ADR 0053: declaration-only projection, never part of a runtime state or
    /// scheduler. Exact Actor.Capability labels retain their historical wire.
    pub(crate) fn deferred_metrics(&self) -> Option<DeferredMetrics> {
        let cut = self.cut.as_ref()?;
        if cut.defer.is_empty() || self.metrics.is_none() && self.quality.is_none() {
            return None;
        }
        let mut layers = DeferredMetrics::default();
        for entry in &cut.defer {
            match &entry.target {
                CutTarget::Actor(actor) => layers.capabilities.extend(
                    self.capabilities
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.actor == *actor)
                        .map(|(i, _)| CapabilityId(i)),
                ),
                CutTarget::Label(label) => {
                    let cap = self.capabilities.iter().enumerate().find(|(_, c)| {
                        self.actors
                            .get(c.actor.0)
                            .is_some_and(|a| label == &format!("{}.{}", a.name, c.name))
                    });
                    if let Some((i, _)) = cap {
                        layers.capabilities.insert(CapabilityId(i));
                    } else {
                        layers.opaque = true;
                    }
                }
            }
        }
        fn intersects(b: &Block, d: &DeferredMetrics) -> bool {
            b.iter().any(|s| match &s.kind {
                Stmt::Call(c) | Stmt::Query(c, _) => d.capability(c.capability),
                Stmt::Response(c, r) => {
                    d.capability(c.capability)
                        || r.recovery.as_ref().is_some_and(|b| intersects(b, d))
                }
                Stmt::Fork { branches, .. } => branches.iter().any(|c| d.capability(c.capability)),
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => intersects(then_body, d) || intersects(else_body, d),
                Stmt::Wait(_)
                | Stmt::WaitUntil(_)
                | Stmt::End(_)
                | Stmt::ResolveEffect { .. }
                | Stmt::StopReceiptUnknown => false,
            })
        }
        layers.process = layers.opaque
            || intersects(&self.process.body, &layers)
            || self
                .contests()
                .iter()
                .any(|c| c.target.active(self) && layers.capability(c.route));
        Some(layers)
    }
}

#[derive(Default)]
pub(crate) struct DeferredMetrics {
    opaque: bool,
    capabilities: std::collections::BTreeSet<CapabilityId>,
    pub process: bool,
}
impl DeferredMetrics {
    pub fn capability(&self, id: CapabilityId) -> bool {
        self.opaque || self.capabilities.contains(&id)
    }
    pub fn actor(&self, m: &Model, id: ActorId) -> bool {
        self.opaque
            || self
                .capabilities
                .iter()
                .any(|c| m.capabilities[c.0].actor == id)
    }
    pub fn person(&self, m: &Model, id: PersonId) -> bool {
        self.opaque
            || m.actors.iter().enumerate().any(|(i, a)| {
                self.actor(m, ActorId(i)) && a.executors.as_ref().is_some_and(|ps| ps.contains(&id))
            })
    }
    pub fn resource(&self, m: &Model, id: ResourceId) -> bool {
        self.opaque
            || self
                .capabilities
                .iter()
                .any(|c| m.capabilities[c.0].uses.iter().any(|u| u.resource == id))
    }
}

/// ADR 0007 Cut after resolution. It limits conclusions and analyst metrics
/// (ADR 0053), never the explored behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cut {
    pub horizon: Vec<Horizon>,
    pub name: String,
    pub question: Option<String>,
    pub include: Vec<ActorId>,
    pub exclude: Vec<CutEntry>,
    pub defer: Vec<CutEntry>,
    pub externality: Vec<String>,
    pub reentry: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Horizon {
    pub from: Option<String>,
    pub to: String,
    pub within: Tick,
    pub structural_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutEntry {
    pub target: CutTarget,
    pub reason: String,
    pub loses: Option<String>,
}

/// A one-word target naming a declared Actor resolves; anything else is a
/// label for something outside the model and is recorded verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CutTarget {
    Actor(ActorId),
    Label(String),
}

/// Nonempty finite unique member names. Values are never probabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumType {
    pub record: Option<RecordShape>,
    pub name: String,
    pub members: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordShape {
    pub fields: Vec<RecordField>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordField {
    pub name: String,
    pub ty: TypeId,
}

/// None preserves omitted capacity; it is never infinity or an invented 1.
/// Positive-duration capabilities require Some(capacity), including if unused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub metadata: ActorMetadata,
    pub executors: Option<Vec<PersonId>>,
    pub source: Option<SourceKind>,
    pub name: String,
    pub capacity: Option<u64>,
    pub span: Span,
}

/// Declaration provenance only; execution state never contains this metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActorMetadata {
    pub rule: Option<RuleInfo>,
    pub instance: Option<ActorInstance>,
    pub source_description: Option<String>,
    pub boundary: Option<String>,
    pub grain: Option<String>,
    pub contains: Option<Vec<ActorId>>,
    pub overlaps: Option<Vec<ActorId>>,
    pub control: Option<String>,
    pub via_model: Option<String>,
    pub capacity_declared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleInfo {
    pub declared_version: String,
    pub effective_version: String,
    pub values: Vec<RuleValue>,
    pub amendment: Option<RuleAmendment>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleValue {
    pub variable: StateVarId,
    pub declared: StateValue,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleAmendment {
    pub from: String,
    pub to: String,
    pub by: PersonId,
    pub under: AuthorityId,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleGrant {
    pub rule: ActorId,
    pub persons: Vec<PersonId>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleBasis {
    pub rule: ActorId,
    pub version: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorInstance {
    PerCase,
}

impl Actor {
    pub fn is_per_case(&self) -> bool {
        self.metadata.instance == Some(ActorInstance::PerCase)
    }
}

#[cfg(test)]
pub(crate) fn per_case_fixture(name: &str) -> bool {
    const NAMES: [&str; 34] = [
        "pcase-capability",
        "pcase-capability.broken-capability",
        "pcase-capacity",
        "pcase-capacity.broken-capacity",
        "pcase-case-product",
        "pcase-case-product.broken-fixed-case",
        "pcase-common",
        "pcase-common.broken-one-reader",
        "pcase-executors",
        "pcase-executors.broken-executors",
        "pcase-explain",
        "pcase-explain.broken-invariant",
        "pcase-joint",
        "pcase-joint.broken-half",
        "pcase-keys",
        "pcase-keys.broken-unmarked-single",
        "pcase-late",
        "pcase-late.broken-clock",
        "pcase-latent-policy",
        "pcase-latent-policy.broken-policy",
        "pcase-marginal",
        "pcase-marginal.broken-error-row",
        "pcase-nondet",
        "pcase-nondet.broken-fixed",
        "pcase-optin",
        "pcase-optin.broken-unmarked",
        "pcase-reject",
        "pcase-reject.broken-zero-queue",
        "pcase-same-document",
        "pcase-same-document.broken-second-kernel",
        "pcase-shared-fixed",
        "pcase-shared-fixed.broken-marker",
        "pcase-write-ir",
        "pcase-write-ir.broken-override",
    ];
    assert!(
        !name.starts_with("pcase-") || NAMES.contains(&name),
        "unclassified per-case fixture {name}"
    );
    NAMES.contains(&name)
}

/// Closed ADR 0033 population, owned by resources/resource_crossings acceptance.
/// Unknown additions must not silently disappear from the older census.
#[cfg(test)]
pub(crate) fn resource_fixture(name: &str) -> bool {
    const NAMES: [&str; 60] = [
        "rsrc-exact",
        "rsrc-exact.broken-one-unit",
        "rsrc-shared",
        "rsrc-shared.broken-missing-use",
        "rsrc-weighted",
        "rsrc-weighted.broken-less-capacity",
        "rsrc-atomic",
        "rsrc-atomic.broken-excess-demand",
        "rsrc-reuse",
        "rsrc-reuse.broken-excess-next",
        "rsrc-cooperative",
        "rsrc-cooperative.broken-drain",
        "rsrc-immediate",
        "rsrc-immediate.broken-cooperative",
        "rsrc-request-held",
        "rsrc-request-held.broken-longer-ack",
        "rsrc-refusal",
        "rsrc-refusal.broken-enabled",
        "rsrc-gate-after-wait",
        "rsrc-gate-after-wait.broken-no-use",
        "rsrc-competition",
        "rsrc-competition.broken-same-actor",
        "rsrc-global",
        "rsrc-global.broken-two-units",
        "rsrc-people",
        "rsrc-people.broken-two-people",
        "rsrc-over-demand",
        "rsrc-over-demand.broken-too-much",
        "rsrc-unused-zero",
        "rsrc-unused-zero.broken-used",
        "rsrc-members",
        "rsrc-members.broken-members",
        "rsrc-actor-use",
        "rsrc-actor-use.broken-actor-use",
        "rsrc-collision",
        "rsrc-collision.broken-same-name",
        "rsrc-zero-amount",
        "rsrc-zero-amount.broken-zero",
        "rsrc-unknown",
        "rsrc-unknown.broken-unknown",
        "rsrc-duplicate",
        "rsrc-duplicate.broken-duplicate",
        "rsrc-second-use",
        "rsrc-second-use.broken-second-use",
        "rsrc-actor-capacity",
        "rsrc-actor-capacity.broken-no-capacity",
        "rsrc-all-start",
        "rsrc-all-start.broken-no-proof",
        "rsrc-clock",
        "rsrc-clock.broken-short-clock",
        "rsrc-release-fork",
        "rsrc-release-fork.broken-parallel",
        "rsrc-queued-cancel",
        "rsrc-queued-cancel.broken-startable",
        "rsrc-huge",
        "rsrc-huge.broken-fits",
        "rsrc-zero-time",
        "rsrc-zero-time.broken-positive",
        "rsrc-half-open",
        "rsrc-half-open.broken-longer",
    ];
    assert!(
        !name.starts_with("rsrc-") || NAMES.contains(&name),
        "unclassified resource fixture {name}"
    );
    NAMES.contains(&name)
}

impl Model {
    /// Ownership only: execution and reference interpreters keep separate state
    /// and initialization algorithms. Invalid IDs are rejected by their guards.
    pub fn is_shared_latent(&self, kind: StateKind) -> bool {
        matches!(kind, StateKind::ActorLatent(a) if self.actors.get(a.0).is_some_and(|a| !a.is_per_case()))
    }
    pub fn is_shared_observed(&self, kind: StateKind) -> bool {
        matches!(kind, StateKind::ActorObserved(a) if self.actors.get(a.0).is_some_and(|a| !a.is_per_case()))
    }
    pub fn is_shared_state(&self, kind: StateKind) -> bool {
        self.is_shared_latent(kind) || self.is_shared_observed(kind)
    }

    pub fn reads_shared_observed(&self, c: &Cond) -> bool {
        match c {
            Cond::StateEq(x) => self
                .state_variables
                .get(x.variable.0)
                .is_some_and(|v| self.is_shared_observed(v.kind)),
            Cond::IntCompare { variable, .. } | Cond::Contains { set: variable, .. } => self
                .state_variables
                .get(variable.0)
                .is_some_and(|v| self.is_shared_observed(v.kind)),
            Cond::And(cs) | Cond::Or(cs) => cs.iter().any(|c| self.reads_shared_observed(c)),
            Cond::Eq(_) => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Debtor {
    Person(PersonId),
    Actor(ActorId),
}

/// Results are nonempty, distinct indices into return_type; duration is fixed.
/// No result is chosen or made observable until normal completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    pub receipt_query: Option<ReceiptQuery>,
    pub performance: Option<PerformanceSource>,
    /// Modelled eligibility, independent of permission and Actor declaration order.
    pub qualified_executors: Option<Vec<PersonId>>,
    pub version_check: Option<VersionCheck>,
    /// Observed case snapshots used only by static lineage derivation.
    pub read_variables: Vec<StateVarId>,
    pub atomic: Option<Atomic>,
    pub uses: Vec<ResourceUse>,
    pub requires: Option<Requires>,
    pub start_writes: Vec<StateWrite>,
    pub kernel: Option<Kernel>,
    pub kernel_reads_latent: Vec<StateVarId>,
    pub kernel_writes: Vec<StateVarId>,
    pub writes: Vec<StateWrite>,
    /// None uses the Actor's executors; Some is the explicit permit union.
    pub permission: Option<Vec<PersonId>>,
    pub params: Vec<Parameter>,
    pub reads: Vec<ActorId>,
    pub commits: bool,
    pub name: String,
    pub actor: ActorId,
    pub return_type: TypeId,
    pub duration: Tick,
    /// Sorted unique ticks, including zero-weight declaration entries. When
    /// present, duration is the canonical placeholder 0, never a sampled time.
    pub duration_distribution: Option<DurationDistribution>,
    pub results: Vec<ValueId>,
    pub cancel: Cancel,
    pub span: Span,
}

/// Declared selector table. Kernel IDs index Model.kernels; results are typed at
/// each Capability binding, exactly like an ordinary kernel use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Performance {
    pub name: String,
    pub basis: String,
    pub queued: bool,
    pub rows: Vec<PerformanceRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceRow {
    pub executor: Option<PersonId>,
    pub from: u64,
    pub until: Option<u64>,
    pub kernel: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceSource {
    pub profile: usize,
    /// Same row order as the declared profile, with result/write types resolved.
    pub kernels: Vec<Kernel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FrozenPerformance {
    pub profile: usize,
    pub row: usize,
    pub executor: Option<PersonId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub name: String,
    pub capacity: u64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceUse {
    pub resource: ResourceId,
    pub amount: u64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurationDistribution {
    pub entries: Vec<(Tick, crate::probability::Rational)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requires {
    pub condition: Cond,
    pub otherwise: ValueId,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atomic {
    pub condition: Cond,
    pub commit_when: bool,
    pub commit: ValueId,
    pub reject: ValueId,
    pub writes: Vec<StateWrite>,
    pub span: Span,
}

impl Capability {
    /// Declared probabilistic sources, not a runtime selector or fallback.
    pub fn result_kernels(&self) -> impl Iterator<Item = &Kernel> {
        self.kernel
            .iter()
            .chain(self.performance.iter().flat_map(|p| &p.kernels))
    }

    pub fn structural_kernel(&self) -> Option<&Kernel> {
        self.result_kernels().next()
    }

    pub fn history_dependent_result(&self) -> bool {
        self.atomic.is_some() || self.version_check.is_some()
    }
    pub fn possible_results(&self) -> Vec<ValueId> {
        let mut values = self.results.clone();
        for v in self
            .version_check
            .iter()
            .map(|v| v.reject)
            .chain(self.requires.iter().map(|v| v.otherwise))
        {
            if !values.contains(&v) {
                values.push(v);
            }
        }
        values
    }
    pub fn finish_writes(&self) -> impl Iterator<Item = &StateWrite> {
        self.writes
            .iter()
            .chain(self.atomic.iter().flat_map(|a| &a.writes))
    }
    /// Static normal-finish domain, excluding zero-work requires refusals.
    pub fn finish_results(&self) -> Vec<ValueId> {
        let mut values = self.results.clone();
        if let Some(v) = &self.version_check
            && !values.contains(&v.reject)
        {
            values.push(v.reject);
        }
        values
    }
    pub fn commits_result(&self, result: ValueId) -> bool {
        self.commits
            && self
                .version_check
                .as_ref()
                .is_none_or(|v| v.reject != result)
            && self.atomic.as_ref().is_none_or(|a| a.commit == result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionCheck {
    pub resolve: Option<ResolutionTie>,
    pub variables: Vec<StateVarId>,
    pub reject: ValueId,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommutativeKind {
    InsertCase,
    Add,
    Subtract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConcurrencyResolution {
    Serialized,
    Lock(ResourceId),
    Commutative(CommutativeKind),
    Versioned,
    LastWriteWins,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPair {
    pub variable: StateVarId,
    pub left: CapabilityId,
    pub right: CapabilityId,
    pub resolution: ConcurrencyResolution,
}

impl Model {
    pub fn has_requires(&self) -> bool {
        fn block(b: &Block, m: &Model) -> bool {
            b.iter().any(|s| match &s.kind {
                Stmt::Call(c) | Stmt::Query(c, _) => m
                    .capabilities
                    .get(c.capability.0)
                    .is_some_and(|c| c.requires.is_some()),
                Stmt::Response(c, r) => {
                    m.capabilities
                        .get(c.capability.0)
                        .is_some_and(|c| c.requires.is_some())
                        || r.recovery.as_ref().is_some_and(|b| block(b, m))
                }
                Stmt::Fork { branches, .. } => branches.iter().any(|c| {
                    m.capabilities
                        .get(c.capability.0)
                        .is_some_and(|c| c.requires.is_some())
                }),
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => block(then_body, m) || block(else_body, m),
                _ => false,
            })
        }
        fn has_end(b: &Block, e: EndId) -> bool {
            b.iter().any(|s| match &s.kind {
                Stmt::Response(_, r) => r.exhausted == Some(e),
                Stmt::End(x) => *x == e,
                Stmt::Fork {
                    join: Join::Any { if_none, .. },
                    ..
                } => *if_none == e,
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => has_end(then_body, e) || has_end(else_body, e),
                _ => false,
            })
        }
        block(&self.process.body, self)
            || self.contests().iter().any(|c| {
                (match c.target {
                    crate::contest::ContestTarget::End(e) => has_end(&self.process.body, e),
                    _ => c.target.active(self),
                }) && self
                    .capabilities
                    .get(c.route.0)
                    .is_some_and(|c| c.requires.is_some())
            })
    }
    pub fn has_wait(&self) -> bool {
        fn block(b: &Block) -> bool {
            b.iter().any(|s| match &s.kind {
                Stmt::Wait(_) => true,
                Stmt::Response(_, r) => {
                    (r.retry_span.is_some() && r.max_attempts > 1)
                        || r.recovery.as_ref().is_some_and(block)
                }
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => block(then_body) || block(else_body),
                _ => false,
            })
        }
        block(&self.process.body)
    }
    pub fn has_condition_wait(&self) -> bool {
        fn contains(b: &Block) -> bool {
            b.iter().any(|s| match &s.kind {
                Stmt::WaitUntil(_) => true,
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => contains(then_body) || contains(else_body),
                _ => false,
            })
        }
        contains(&self.process.body)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authority {
    pub rule_grants: Vec<RuleGrant>,
    pub under: Option<RuleBasis>,
    pub name: String,
    pub basis: String,
    pub grants: Vec<Grant>,
    pub issued_by: Option<PersonId>,
    pub not_by: Vec<PersonId>,
    pub restrictions: AuthorityRestrictions,
    pub span: Span,
}

/// Static metadata only: never copied into exploration state or scheduler keys.
/// Conditions stay at their declaration so delegation does not copy condition trees.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuthorityRestrictions {
    pub scope: Option<Cond>,
    pub expires: Option<Tick>,
    pub effective_expires: Option<Tick>,
    pub revocable_by: Option<AuthorityRevoker>,
    pub delegated_from: Option<AuthorityId>,
    pub scope_span: Option<Span>,
    pub expires_span: Option<Span>,
    pub revocable_span: Option<Span>,
    pub delegated_span: Option<Span>,
    pub issuer_span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityRevoker {
    Person(PersonId),
    Actor(ActorId),
}

impl AuthorityRestrictions {
    pub fn restricted(&self) -> bool {
        self.scope.is_some() || self.expires.is_some() || self.delegated_from.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub capability: CapabilityId,
    pub persons: Vec<PersonId>,
}

/// Unspecified is legal only where no cancellation may be requested.
/// Cooperative ack_within is >= 1, with each delay in 1..=d explored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cancel {
    Immediate,
    Unspecified,
    Cooperative { ack_within: Tick },
    ForbiddenAfterStart,
}

/// cases is always 1; live_tasks counts created but not terminal tasks.
/// The boundary is strictly greater than the bound, not greater-or-equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub cases: u64,
    pub live_tasks: u64,
    pub clock: Tick,
}

/// All result/fork slots are lexical; no dynamic name lookup by engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub name: String,
    pub degradations: Vec<Degradation>,
    pub bindings: Vec<Binding>,
    pub forks: Vec<String>,
    pub body: Block,
    pub span: Span,
}

/// Independently validated correspondence to an executable If at the original span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Degradation {
    pub name: String,
    pub span: Span,
    pub condition: ResultMatch,
    pub producer: CapabilityId,
    pub degraded_calls: Vec<Call>,
    pub normal_calls: Vec<Call>,
}

/// A call result is absent until finish. Cancelled tasks have no enum value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub name: String,
    pub ty: TypeId,
    pub span: Span,
}

/// Control-flow tree in execution order; no loops or detached tasks.
pub type Block = Vec<Statement>;

/// Source provenance remains available for engine diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub kind: Stmt,
    pub span: Span,
}

/// Fork creates its entire branch set before dispatch and waits at its join.
/// End terminates control immediately; suffix statements are not executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Query(Call, BindingId),
    ResolveEffect {
        command: BindingId,
        evidence: BindingId,
    },
    StopReceiptUnknown,
    WaitUntil(Cond),
    Wait(Tick),
    Call(Call),
    Response(Call, Box<ResponseAttempt>),
    Fork {
        id: ForkId,
        branches: Vec<Call>,
        join: Join,
    },
    If {
        condition: Cond,
        then_body: Block,
        else_body: Block,
    },
    End(EndId),
}

/// Capability determines Actor and result type; binding is unique per call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// Static provenance only: results remain input-independent nondeterminism.
    pub args: Vec<BindingId>,
    pub binding: BindingId,
    pub capability: CapabilityId,
    pub span: Span,
}

/// A statically expanded attempt. Original and internal result slots are
/// distinct; no unknown receipt is represented as an enum result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseAttempt {
    pub recovery: Option<Block>,
    /// Explicit declaration, distinct from the independently checked mechanism.
    pub idempotency: Option<String>,
    pub effect_modeled: bool,
    /// Non-executable public result alias; it is not an additional Task.
    pub command_call: Call,
    pub command: BindingId,
    pub attempt: u64,
    pub max_attempts: u64,
    pub loss: ResponseLoss,
    pub retry_unknown: bool,
    pub retry_results: Vec<ValueId>,
    pub backoff: Tick,
    pub exhausted: Option<EndId>,
    pub idempotent: bool,
    pub retry_span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseLoss {
    None,
    Nondet,
    Sequence(Vec<crate::ast::Receipt>),
    Probability {
        probability: crate::probability::Rational,
        basis: String,
    },
}

/// An exact, read-only finish-time projection of one G backing set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptQuery {
    pub target: CapabilityId,
    pub key_parameter: usize,
    pub availability: crate::ast::QueryAvailability,
}

impl Model {
    pub fn recovery(&self) -> Option<(BindingId, &Block)> {
        self.response_attempts().into_iter().find_map(|(_, r)| {
            (r.attempt == 1)
                .then_some(r)
                .and_then(|r| r.recovery.as_ref().map(|b| (r.command, b)))
        })
    }
}

impl Model {
    pub fn response_attempts(&self) -> Vec<(&Call, &ResponseAttempt)> {
        let mut rows = Vec::new();
        let mut blocks = vec![self.process.body.iter()];
        while let Some(block) = blocks.last_mut() {
            let Some(s) = block.next() else {
                blocks.pop();
                continue;
            };
            match &s.kind {
                Stmt::Response(c, r) => rows.push((c, r.as_ref())),
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    blocks.push(else_body.iter());
                    blocks.push(then_body.iter());
                }
                _ => {}
            }
        }
        rows
    }

    pub fn response_attempt(&self, binding: BindingId) -> Option<&ResponseAttempt> {
        // Reserved compiler-only provenance namespace, checked by both public
        // IR validators. Legacy calls need no control-tree walk or allocation.
        if self
            .process
            .bindings
            .get(binding.0)
            .is_none_or(|b| !b.name.starts_with("@response:"))
        {
            return None;
        }
        self.response_attempts()
            .into_iter()
            .find(|(c, _)| c.binding == binding)
            .map(|(_, r)| r)
    }
}

/// The containing fork is the resolved target (no second textual fork name).
/// Any.matches covers every branch exactly once with a value of its return type.
/// if_none jumps to the terminal only after all branches end without a match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Join {
    All,
    Any {
        matches: Vec<ResultMatch>,
        tie: Tie,
        remaining: Remaining,
        if_none: EndId,
    },
}

/// ValueId is qualified by Process.bindings[binding].ty, not a global enum index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultMatch {
    pub binding: BindingId,
    pub value: ValueId,
}

/// By must be a full permutation of the fork's BindingIds; never source order
/// unless that order was explicitly written. Nondet explores all tied winners.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tie {
    By(Vec<BindingId>),
    Nondet,
}

/// CancelThenDrain requests cancellation, but request alone releases nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remaining {
    CancelThenDrain,
    Drain,
}

/// Typed result/state comparisons. And/Or have >= 2 operands; grouping is already
/// represented by the tree. Only analytical contracts may read latent state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cond {
    Contains {
        set: StateVarId,
        element: Element,
    },
    IntCompare {
        variable: StateVarId,
        operator: IntComparison,
        value: i128,
    },
    StateEq(StateMatch),
    Eq(ResultMatch),
    And(Vec<Cond>),
    Or(Vec<Cond>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    CurrentCase,
    Parameter(usize),
}

pub use crate::ast::SourceKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub ty: TypeId,
    pub span: Span,
}

/// Stable machine properties; report renders the exact shared property ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Property {
    NoLostUpdate(StateVarId),
    StateInRange,
    RecoverableEnd,
    TerminatingEnd,
    AlmostSureEnd,
    Contest(usize),
    ProbabilityReached(usize),
    NoResourceOvercommit,
    ReachableEnd(EndId),
    Contract(ClauseId),
}

/// Capability-wide event pattern, matching every lexical invocation. A normal
/// completion publishes its value and finish/commit together; cancellation does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub capability: CapabilityId,
    pub kind: EventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    CancelAck,
    Start,
    Finish(Option<ValueId>),
    Commit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Received,
    Admitted,
    Event(Event),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    End(EndId),
    Event(Event),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractClause {
    /// Source ordinal includes static clauses; None preserves direct legacy builders.
    pub ordinal: Option<usize>,
    pub name: Option<String>,
    pub kind: ContractKind,
    pub span: Span,
}

impl ContractClause {
    pub fn response(&self) -> Option<(Trigger, &[Target], Tick)> {
        match &self.kind {
            ContractKind::Response {
                trigger,
                targets,
                within,
            }
            | ContractKind::NamedResponse {
                trigger,
                targets,
                within,
                ..
            } => Some((*trigger, targets, *within)),
            _ => None,
        }
    }
    pub fn property_id(&self, id: ClauseId) -> String {
        let kind = match self.kind {
            ContractKind::Invariant { .. } | ContractKind::StateInvariant { .. } => "invariant",
            ContractKind::AtMostOnce { .. } => "at_most_once",
            ContractKind::Response { .. } => "response",
            ContractKind::NamedResponse { .. } => "response",
            ContractKind::Excludes { .. } => "excludes",
            ContractKind::Waive { .. } => "waive",
            ContractKind::ContestResponse { .. } => "contest_response",
            ContractKind::RequiresEvent { .. } => "requires_event",
            ContractKind::Separation { .. } => "separation",
            ContractKind::Permitted { .. } => "permitted",
            ContractKind::AuthorityBasis { .. } => "authority_basis",
        };
        let name = self
            .name
            .as_ref()
            .map(|n| format!(" {n}"))
            .unwrap_or_default();
        format!(
            "contract{name} #{} {kind}",
            self.ordinal.unwrap_or(id.0 + 1)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractKind {
    /// Index in Model::contests. Shares the source ordinal of the static row.
    ContestResponse {
        contest: usize,
    },
    Excludes {
        before: Event,
        start: Event,
    },
    Waive {
        response: ClauseId,
        event: Event,
        reason: String,
        authority: AuthorityId,
    },
    NamedResponse {
        debtor: Option<Debtor>,
        label: Option<String>,
        non_waivable: bool,
        trigger: Trigger,
        targets: Vec<Target>,
        within: Tick,
    },
    Separation {
        left: Event,
        right: Event,
    },
    Permitted {
        capability: CapabilityId,
    },
    AuthorityBasis {
        capability: CapabilityId,
        authority: AuthorityId,
    },
    /// None only for an inactive Capability whose condition needs another process's binding.
    Invariant {
        event: Event,
        condition: Option<Cond>,
    },
    StateInvariant {
        antecedent: Option<Cond>,
        condition: Cond,
    },
    AtMostOnce {
        event: Event,
    },
    Response {
        trigger: Trigger,
        targets: Vec<Target>,
        within: Tick,
    },
    RequiresEvent {
        before: Event,
        start: Event,
    },
}

/// Verification source order is also outcome property order (one per request).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verify {
    pub property: Property,
    pub span: Span,
}

/// Explicitly recorded default/model assumption, mapped verbatim to ADR 0002.
/// Values are descriptions, not executable defaults or numeric measurements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assumption {
    pub field: String,
    pub value: String,
    pub affects: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateValue {
    Value(ValueId),
    None,
    Integer(u64),
    Set(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    Rule(ActorId),
    ObservedCase,
    LatentCase,
    ActorLatent(ActorId),
    ActorObserved(ActorId),
}

impl StateKind {
    pub fn is_observed(self) -> bool {
        matches!(
            self,
            Self::ObservedCase | Self::ActorObserved(_) | Self::Rule(_)
        )
    }
    pub fn is_latent(self) -> bool {
        !self.is_observed()
    }
    pub fn actor(self) -> Option<ActorId> {
        match self {
            Self::ActorLatent(a) | Self::ActorObserved(a) | Self::Rule(a) => Some(a),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialValue {
    Distribution(usize),
    Fixed(StateValue),
    Nondet,
}

#[cfg(test)]
pub(crate) fn rule_fixture(name: &str) -> bool {
    static NAMES: std::sync::OnceLock<std::collections::BTreeSet<String>> =
        std::sync::OnceLock::new();
    NAMES
        .get_or_init(|| {
            serde_json::from_str(include_str!("../tests/support/rule_names.json"))
                .expect("closed Rule fixture names")
        })
        .contains(name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateVariable {
    pub last_write_wins: bool,
    pub case_set_size: Option<u8>,
    /// None is the legacy enum domain; integers never allocate an EnumType.
    pub integer_max: Option<u64>,
    pub name: String,
    pub kind: StateKind,
    pub base: TypeId,
    pub optional: bool,
    pub initial: InitialValue,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteSource {
    InsertCase {
        parameter: usize,
    },
    Executor,
    Result,
    Constant(StateValue),
    Integer {
        operator: AssignmentOp,
        operand: i128,
        span: Span,
    },
}

pub use crate::ast::{AssignmentOp, IntComparison};

impl StateVariable {
    pub fn cardinality(&self, types: &[EnumType]) -> u128 {
        if let Some(n) = self.case_set_size {
            return 1u128.checked_shl(u32::from(n)).unwrap_or(0);
        }
        self.integer_max.map_or_else(
            || {
                types
                    .get(self.base.0)
                    .map_or(0, |t| t.members.len() as u128)
            },
            |n| u128::from(n) + 1,
        )
    }
    pub fn domain<'a>(
        &'a self,
        types: &'a [EnumType],
    ) -> Box<dyn Iterator<Item = StateValue> + 'a> {
        if self.case_set_size.is_some() {
            return Box::new(std::iter::empty());
        }
        match self.integer_max {
            Some(n) => Box::new((0..=n).map(StateValue::Integer)),
            None => Box::new(
                (0..types.get(self.base.0).map_or(0, |t| t.members.len()))
                    .map(|i| StateValue::Value(ValueId(i))),
            ),
        }
    }
    pub fn contains(&self, types: &[EnumType], value: StateValue) -> bool {
        if let Some(n) = self.case_set_size {
            return matches!(value, StateValue::Set(bits) if n > 0 && n <= 64 && (n == 64 || bits >> n == 0));
        }
        match (self.integer_max, value) {
            (Some(n), StateValue::Integer(v)) => v <= n,
            (None, StateValue::Value(v)) => types
                .get(self.base.0)
                .is_some_and(|t| v.0 < t.members.len()),
            (None, StateValue::None) => self.optional,
            _ => false,
        }
    }
}

impl Model {
    pub fn has_integers(&self) -> bool {
        self.state_variables.iter().any(|v| v.integer_max.is_some())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateWrite {
    pub variable: StateVarId,
    pub source: WriteSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateMatch {
    pub variable: StateVarId,
    pub value: StateValue,
}

pub use crate::ast::Comparison;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbabilityQuery {
    pub condition: ProbabilityCondition,
    pub within: Option<Tick>,
    pub comparison: Comparison,
    pub bound: crate::probability::Rational,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbabilityCondition {
    State(Cond),
    End(EndId),
}
impl From<Cond> for ProbabilityCondition {
    fn from(c: Cond) -> Self {
        Self::State(c)
    }
}
impl Property {
    pub fn completion(self) -> bool {
        matches!(
            self,
            Self::RecoverableEnd | Self::TerminatingEnd | Self::AlmostSureEnd
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kernel {
    pub name: String,
    pub basis: Option<String>,
    pub given: Vec<StateVarId>,
    pub rows: Vec<KernelRow>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelRow {
    pub values: Vec<StateValue>,
    pub branches: Vec<KernelBranch>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelBranch {
    pub probability: crate::probability::Rational,
    pub result: ValueId,
    pub overrides: Vec<(StateVarId, StateValue)>,
}
impl Model {
    pub fn uses_probability(&self) -> bool {
        !self.kernels.is_empty()
            || self.contests().iter().any(|c| c.rates.is_some())
            || self
                .response_attempts()
                .iter()
                .any(|(_, r)| matches!(r.loss, ResponseLoss::Probability { .. }))
            || !self.initial_distributions.is_empty()
            || !self.probability_queries.is_empty()
            || self
                .capabilities
                .iter()
                .any(|c| c.duration_distribution.is_some())
    }
}
