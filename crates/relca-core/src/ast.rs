//! Lossless-in-structure ADR 0001 syntax tree: declaration order, duplicates,
//! omission, and explicit condition grouping survive parsing for lower to check.
//! Comments/separators need not survive; all spans refer to original source bytes.

use crate::diag::Span;

/// Source-bearing syntax value. No normalization or name resolution by syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

/// An unresolved identifier, including enum members and call result bindings.
pub type Ident = Spanned<String>;

/// INT is nonnegative. Values not representable in u64 must be rejected, not
/// wrapped. Syntax retains units so lower can emit U-SEM-0 for non-min units.
pub type Integer = Spanned<u64>;

/// Exactly one model wrapper; version is the decoded STRING with its full span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub name: Ident,
    pub version: Spanned<String>,
    pub items: Vec<Item>,
    pub span: Span,
}

impl Model {
    pub fn has_merges(&self) -> bool {
        fn block(b: &Block) -> bool {
            b.iter().any(|s| match &s.value {
                Stmt::Merge { .. } => true,
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => block(then_body) || block(else_body),
                Stmt::Repeat { body, on_limit, .. } => block(body) || block(on_limit),
                Stmt::Call(c) => c.recovery.as_deref().is_some_and(block),
                _ => false,
            })
        }
        self.items
            .iter()
            .any(|i| matches!(i, Item::Process(p) if block(&p.body)))
    }

    pub fn has_evidence_basis(&self) -> bool {
        self.items.iter().any(|item| {
            matches!(item, Item::Contract(c)
            if c.clauses.iter().any(|c| matches!(c.value, ContractClause::EvidenceBasis { .. })))
        })
    }

    pub fn uses_priority(&self) -> bool {
        let limit = |q: &Spanned<AdmissionQueue>| {
            matches!(q.value.overflow, Overflow::WaitExternalPriority(_))
        };
        self.items.iter().any(|item| match item {
            Item::Policy(p) => p.value.priority.is_some(),
            Item::Queue(q) | Item::Concurrent(q) => limit(q),
            Item::Scenario(s) => s.items.iter().any(|i| match i {
                ScenarioItem::Policy(p) => p.value.priority.is_some(),
                ScenarioItem::Queue(q) | ScenarioItem::Concurrent(q) => limit(q),
                _ => false,
            }),
            Item::Actor(a) => a.items.iter().any(|i| matches!(i,
                ActorItem::Capability(c) if c.items.iter().any(|i| matches!(i.value, CapItem::VersionCheck { resolve: Some(_), .. })))),
            _ => false,
        })
    }

    pub fn has_condition_wait(&self) -> bool {
        fn contains(b: &Block) -> bool {
            b.iter().any(|s| match &s.value {
                Stmt::WaitUntil(_) => true,
                Stmt::Merge {
                    then_body,
                    else_body,
                    ..
                }
                | Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => contains(then_body) || contains(else_body),
                _ => false,
            })
        }
        self.items
            .iter()
            .any(|i| matches!(i, Item::Process(p) if contains(&p.body)))
    }
}

/// Phase 0 items (ADR 0001, 0007, 0008). Repetitions remain visible to lower.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Performance(Performance),
    CasePriority(Spanned<Vec<Integer>>),
    ArrivalsDistribution(ArrivalDistribution),
    Calendar(CalendarDecl),
    PersonCalendar { person: Ident, calendar: Ident },
    Compare(CompareDecl),
    QualityProperty(QualityProperty),
    CohortMetric(CohortMetric),
    QuantileMetric(QuantileMetric),
    Resource(ResourceDecl),
    Metrics(MetricsDecl),
    Responsible(Responsible),
    Simulation(SimulationDecl),
    ArrivalsFixed(Spanned<Vec<Duration>>),
    ArrivalsEvery(Spanned<FixedEveryDecl>),
    Scenario(ScenarioDecl),
    Policy(Spanned<StartPolicy>),
    Queue(Spanned<AdmissionQueue>),
    Concurrent(Spanned<AdmissionQueue>),
    Kernel(KernelDecl),
    ObservedCase(Vec<VarDecl>),
    LatentCase(Vec<VarDecl>),
    People(Spanned<Vec<Ident>>),
    Permit(PermitDecl),
    Authority(AuthorityDecl),
    Type(TypeDecl),
    Actor(Actor),
    Process(Process),
    Bounds(Bounds),
    Verify(Verify),
    Cut(CutDecl),
    Configuration(Configuration),
    Contract(ContractDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareDecl {
    pub plans: Vec<Ident>,
    pub under: Option<Vec<Ident>>,
    pub cut: Option<Ident>,
    pub hard: Vec<HardRow>,
    pub objectives: Vec<Objective>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardRow {
    pub cohort: Ident,
    pub op: crate::ir::HardComparison,
    pub bound: Ident,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Objective {
    pub direction: crate::ir::Direction,
    pub target: ObjectiveRef,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectiveRef {
    Cohort(Ident),
    Quantile(Ident),
    Overall(crate::ir::OverallObjective),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedEveryDecl {
    pub every: Duration,
    pub from: Duration,
    pub until: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioDecl {
    pub name: Ident,
    pub items: Vec<ScenarioItem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityProperty {
    pub name: Ident,
    pub target: QualityTarget,
    pub within: Option<Duration>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualityTarget {
    State(Spanned<Cond>),
    Ends(Vec<Ident>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortMetric {
    pub name: Ident,
    pub numerator: Ident,
    pub denominator: Option<Ident>,
    pub denominator_span: Span,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantileMetric {
    pub name: Ident,
    pub p: Ident,
    pub property: Ident,
    pub given_reached: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioItem {
    ArrivalsDistribution(ArrivalDistribution),
    ArrivalsFixed(Spanned<Vec<Duration>>),
    ArrivalsEvery(Spanned<FixedEveryDecl>),
    Policy(Spanned<StartPolicy>),
    Queue(Spanned<AdmissionQueue>),
    Concurrent(Spanned<AdmissionQueue>),
    Bounds(Bounds),
    Simulation(SimulationDecl),
    Horizon(Duration),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDecl {
    pub name: Ident,
    pub capacity: Integer,
    pub calendar: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrivalDistribution {
    pub basis: Option<Spanned<String>>,
    pub entries: Vec<ArrivalSchedule>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrivalSchedule {
    pub probability: Ident,
    pub times: Vec<Duration>,
    pub span: Span,
}

/// ADR 0059: preserve all intervals, references and omitted close behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarDecl {
    pub name: Ident,
    pub through: Duration,
    pub on_close: Option<Ident>,
    pub intervals: Vec<CalendarInterval>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarInterval {
    pub from: Duration,
    pub until: Duration,
    pub capacity: Integer,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceUse {
    pub resource: Ident,
    pub amount: Integer,
    pub span: Span,
}

/// ADR 0019 declaration; retain repeated entries for semantic rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricsDecl {
    pub items: Vec<Spanned<MetricsItem>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetricsItem {
    OvertimeRate {
        actor: Ident,
        price: Integer,
    },
    BreakdownWaits,
    Window(Duration),
    Currency(Spanned<String>),
    ActorRate {
        actor: Ident,
        price: Integer,
    },
    StartRate {
        target: CapabilityPath,
        price: Integer,
    },
}

/// ADR 0016 configuration; retain numeric lexemes until semantic validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationDecl {
    pub replications: Ident,
    pub seed: Ident,
    pub confidence: Ident,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartPolicy {
    pub priority: Option<crate::ir::QueueTie>,
    pub observable: bool,
    pub extensions: Vec<Spanned<PolicyExtension>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyExtension {
    CallOrder,
    FirstIdle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionQueue {
    pub capacity: Integer,
    pub overflow: Overflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overflow {
    Reject,
    WaitExternalFifo,
    WaitExternalPriority(crate::ir::QueueTie),
    End(Ident),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityPath {
    pub actor: Ident,
    pub capability: Ident,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermitDecl {
    pub persons: Vec<Ident>,
    pub target: CapabilityPath,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityDecl {
    pub name: Ident,
    pub items: Vec<Spanned<AuthorityItem>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityItem {
    ChangeGrants {
        persons: Vec<Ident>,
        rule: Ident,
    },
    Under {
        rule: Ident,
        version: Spanned<String>,
    },
    Basis(Spanned<String>),
    Grants {
        persons: Vec<Ident>,
        target: CapabilityPath,
    },
    IssuedBy(Ident),
    NotBy(Vec<Ident>),
    Scope(Spanned<Cond>),
    Expires(Duration),
    RevocableBy(Ident),
    DelegatedFrom(Ident),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractDecl {
    pub name: Option<Ident>,
    pub applies_to: Option<(Vec<Ident>, Spanned<String>)>,
    pub clauses: Vec<Spanned<ContractClause>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub actor: Ident,
    pub capability: Ident,
    pub kind: EventKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    CancelAck,
    Start,
    Finish(Option<Ident>),
    Commit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    Received,
    /// `case.admitted`, with the source span of the word for context refusals.
    Admitted(Span),
    Event(Event),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    End(Ident),
    Event(Event),
}

/// Analyst declarations, not measured or validated party behavior. Probability
/// range and nonblank evidence are checked when lowering, not while parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContestRates {
    pub probability: Ident,
    pub occurrence_basis: Spanned<String>,
    pub service_time_basis: Spanned<String>,
    pub independence: Ident,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractClause {
    Excludes {
        before: Event,
        start: Event,
    },
    Waive {
        response: Vec<Ident>,
        event: Event,
        reason: Spanned<String>,
        authority: Ident,
    },
    Contest {
        party: Ident,
        target: StandingRef,
        route: CapabilityPath,
        reversible_by: Vec<Ident>,
        respond_by: Duration,
        rates: Option<ContestRates>,
    },
    Separation {
        left: Event,
        right: Event,
    },
    Permitted {
        target: CapabilityPath,
    },
    AuthorityBasis {
        target: CapabilityPath,
        authority: Ident,
    },
    EvidenceBasis {
        direct: bool,
        target: CapabilityPath,
        sources: Vec<Path>,
    },
    Invariant {
        event: Event,
        condition: Spanned<Cond>,
    },
    StateInvariant {
        antecedent: Option<Spanned<InvariantStateCond>>,
        condition: Spanned<InvariantStateCond>,
    },
    AtMostOnce {
        event: Event,
    },
    Response {
        label: Option<Ident>,
        debtor: Option<Ident>,
        non_waivable: bool,
        trigger: Trigger,
        targets: Vec<Target>,
        duration: Duration,
        after: Trigger,
    },
    RequiresEvent {
        event: Event,
        after: Event,
    },
}

/// Invariant-local syntax: bare booleans do not extend decision expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantStateCond {
    Compare(Spanned<Cond>),
    Bool(Ident),
    And(Vec<Spanned<InvariantStateCond>>),
    Or(Vec<Spanned<InvariantStateCond>>),
    Group(Box<Spanned<InvariantStateCond>>),
}

/// ADR 0008 selection metadata; processes and Cuts remain model declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration {
    pub name: Ident,
    pub cut: Ident,
    pub process: Ident,
    pub amendments: Vec<Amendment>,
    pub intent: Vec<Intent>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Amendment {
    pub rule: Ident,
    pub from: Spanned<String>,
    pub to: Spanned<String>,
    pub by: Ident,
    pub under: Ident,
    pub values: Vec<(Ident, Ident)>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentAction {
    Add,
    Retire,
    Change,
}

/// Configuration-only metadata. These declarations never enter execution state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Claim {
        action: IntentAction,
        target: DiffTarget,
        reason: Option<Spanned<String>>,
        keep_fallback: bool,
        loses: Option<Spanned<String>>,
        span: Span,
    },
    Preserve {
        selector: PropertySelector,
        on: Option<DiffTarget>,
        span: Span,
    },
    Override {
        target: DiffTarget,
        subject: Ident,
        reason: Option<Spanned<String>>,
        span: Span,
    },
    Maintain {
        target: DiffTarget,
        person: Ident,
        reason: Option<Spanned<String>>,
        span: Span,
    },
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    ContractScope,
    Actor,
    Relation,
    Meaning,
    Authority,
    Resource,
    Time,
    Visibility,
    Contest,
    Rule,
    Qualification,
    Performance,
    Degradation,
}

impl DiffKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::ContractScope => "contract_scope",
            Self::Actor => "actor",
            Self::Relation => "relation",
            Self::Meaning => "meaning",
            Self::Authority => "authority",
            Self::Resource => "resource",
            Self::Time => "time",
            Self::Visibility => "visibility",
            Self::Contest => "contest",
            Self::Rule => "rule",
            Self::Qualification => "qualification",
            Self::Performance => "performance",
            Self::Degradation => "degradation",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::ContractScope,
            Self::Actor,
            Self::Relation,
            Self::Meaning,
            Self::Authority,
            Self::Resource,
            Self::Time,
            Self::Visibility,
            Self::Contest,
            Self::Rule,
        ]
        .into_iter()
        .find(|k| k.name() == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffTarget {
    pub kind: DiffKind,
    pub path: Path,
}
impl DiffTarget {
    pub fn name(&self) -> String {
        self.path
            .value
            .iter()
            .map(|p| p.value.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertySelector {
    Contract(Ident),
    Exact(Spanned<String>),
}
impl PropertySelector {
    pub fn value(&self) -> &str {
        match self {
            Self::Contract(s) | Self::Exact(s) => &s.value,
        }
    }
    pub fn span(&self) -> Span {
        match self {
            Self::Contract(s) | Self::Exact(s) => s.span,
        }
    }
}

/// ADR 0007 Cut. Items keep source order and duplicates for lower to check;
/// targets are unresolved paths (an Actor name or a label for outside things).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutDecl {
    pub name: Ident,
    pub items: Vec<Spanned<CutItem>>,
    pub span: Span,
}

/// Dotted path as written: `Payment`, `Vendor.contest.rates`.
pub type Path = Spanned<Vec<Ident>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CutItem {
    Horizon(Vec<Horizon>),
    Question(Spanned<String>),
    Include(Vec<Ident>),
    Exclude(Exclusion),
    Defer(Exclusion),
    Externality(Vec<Path>),
    /// Recorded verbatim; Phase 0 does not interpret re-entry routes.
    Reentry(Vec<Spanned<String>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Horizon {
    pub from: Option<Ident>,
    pub to: Ident,
    pub within: Duration,
    pub structural_only: bool,
}

/// reason is required by syntax; a missing loses is a W-CUT-1 for lower.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exclusion {
    pub target: Path,
    pub reason: Spanned<String>,
    pub loses: Option<Spanned<String>>,
}

/// Enum syntax requires at least one member; member names remain unresolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDecl {
    pub name: Ident,
    pub members: Vec<Ident>,
    pub record: Option<Vec<RecordFieldDecl>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFieldDecl {
    pub names: Vec<Ident>,
    pub ty: Ident,
    pub inline: Option<Vec<Ident>>,
}

/// A scalar or a structurally retained named record literal. The head is never
/// an encoded literal: fields, their order, duplicates and spans remain intact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueExpr {
    pub head: Ident,
    pub fields: Option<Vec<(Ident, Ident)>>,
    pub span: Span,
}
impl From<Ident> for ValueExpr {
    fn from(head: Ident) -> Self {
        Self {
            span: head.span,
            head,
            fields: None,
        }
    }
}
impl std::ops::Deref for ValueExpr {
    type Target = Ident;
    fn deref(&self) -> &Ident {
        &self.head
    }
}

/// Capacity and capability items retain order and duplicates; no defaults here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub name: Ident,
    pub items: Vec<ActorItem>,
    pub span: Span,
}

/// Actor contracts admitted by ADR 0001 and ADR 0009.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorItem {
    Calendar(Ident),
    Version(Spanned<String>),
    Decides(VarDecl),
    Instance(Spanned<ActorInstance>),
    Boundary(Spanned<String>),
    Grain(Ident),
    Contains(Spanned<Vec<Ident>>),
    Overlaps(Spanned<Vec<Ident>>),
    Control(Ident),
    ViaModel(Spanned<String>),
    ControlParty(Span),
    Standing {
        targets: Vec<StandingRef>,
        basis: Spanned<String>,
        span: Span,
    },
    Latent(VarDecl),
    State(VarDecl),
    Executors(Spanned<Vec<Ident>>),
    Capacity(Spanned<u64>),
    Source(Spanned<SourceKind>, Option<Spanned<String>>),
    Capability(Capability),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandingRef {
    End(Ident),
    Commit(CapabilityPath),
}
impl StandingRef {
    pub fn span(&self) -> Span {
        match self {
            Self::End(n) => n.span,
            Self::Commit(p) => p.span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorInstance {
    PerCase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Document,
    Record,
    Tacit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Responsible {
    pub person: Ident,
    pub target: ResponsibilityTarget,
    pub within: Option<Duration>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponsibilityTarget {
    Capability(CapabilityPath),
    Condition(Spanned<Cond>),
    Formula(Spanned<ResponsibilityCond>),
}

/// Only responsibility targets accept implication and bare observed booleans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponsibilityCond {
    Compare(Spanned<Cond>),
    Bool(Ident),
    And(Vec<Spanned<ResponsibilityCond>>),
    Or(Vec<Spanned<ResponsibilityCond>>),
    Implies(
        Box<Spanned<ResponsibilityCond>>,
        Box<Spanned<ResponsibilityCond>>,
    ),
    Group(Box<Spanned<ResponsibilityCond>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: Ident,
    pub ty: Ident,
}

/// Positionally matched parameters; return type is a name, not a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    pub name: Ident,
    pub params: Vec<Param>,
    pub return_type: Ident,
    pub inline_result: Option<Vec<Ident>>,
    pub items: Vec<Spanned<CapItem>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Performance {
    pub name: Ident,
    pub basis: Spanned<String>,
    pub queued: bool,
    pub rows: Vec<PerformanceExecutor>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceExecutor {
    pub executor: Ident,
    pub bands: Vec<PerformanceBand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceBand {
    pub from: u64,
    pub until: Option<u64>,
    pub kernel: Ident,
}

/// Empty/repeated cap_item lists are representable, never silently defaulted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapItem {
    ReceiptQuery {
        target: Ident,
        key: Ident,
        availability: QueryAvailability,
    },
    OutcomesPerformance(Ident),
    QualifiedExecutors(Vec<Ident>),
    Idempotency(Ident),
    EffectScopeModeled,
    VersionCheck {
        variables: Vec<Ident>,
        reject: Ident,
        resolve: Option<crate::ir::ResolutionTie>,
    },
    Atomic {
        condition: Spanned<Cond>,
        then_branch: AtomicBranch,
        else_branch: AtomicBranch,
    },
    Uses(Vec<ResourceUse>),
    Requires {
        condition: Spanned<Cond>,
        otherwise: ValueExpr,
    },
    Start(Vec<Assignment>),
    OutcomesKernel(Ident),
    KernelReadsLatent(Vec<Path>),
    Writes(Vec<StateTarget>),
    Finish(Vec<Assignment>),
    Reads(Vec<Ident>),
    Commits,
    DurationFixed(Duration),
    DurationDistribution(Vec<DurationEntry>),
    ResultNondet(Vec<ValueExpr>),
    Cancel(Cancel),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicBranch {
    pub assignments: Vec<Assignment>,
    pub result: Ident,
}

/// Unit spelling is retained, including a recognizable out-of-scope unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duration {
    pub amount: Integer,
    pub unit: Ident,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurationEntry {
    pub duration: Duration,
    pub probability: Ident,
    pub span: Span,
}

/// A missing cancel item differs from an explicit forbidden_after_start item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cancel {
    Immediate,
    Cooperative { ack_within: Duration },
    ForbiddenAfterStart,
}

/// Blocks may be empty in the grammar. End is a statement, not a block suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub name: Ident,
    /// Named branch annotations; the executable tree retains ordinary If semantics.
    pub degradations: Vec<Degradation>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Degradation {
    pub name: Ident,
    pub span: Span,
}

/// Ordered statement tree, including statements syntactically following end.
pub type Block = Vec<Spanned<Stmt>>;

impl Model {
    pub fn has_repetition(&self) -> bool {
        fn block(b: &Block) -> bool {
            b.iter().any(|s| match &s.value {
                Stmt::Repeat { .. } => true,
                Stmt::Merge {
                    then_body,
                    else_body,
                    ..
                }
                | Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => block(then_body) || block(else_body),
                Stmt::Call(c) => c.recovery.as_deref().is_some_and(block),
                _ => false,
            })
        }
        self.items
            .iter()
            .any(|i| matches!(i, Item::Process(p) if block(&p.body)))
    }
}

/// Fork owns its mandatory immediately following join; branches are only calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Merge {
        binding: Ident,
        condition: Spanned<Cond>,
        then_body: Block,
        else_body: Block,
    },
    Yield(Ident),
    Repeat {
        name: Ident,
        at_most: Integer,
        gap: Option<Duration>,
        body: Block,
        on_limit: Block,
    },
    Again,
    Leave,
    ResolveEffect {
        command: Ident,
        evidence: Ident,
    },
    StopReceiptUnknown,
    WaitUntil(Spanned<Cond>),
    Wait(Duration),
    Call(Call),
    Fork {
        name: Ident,
        branches: Vec<Call>,
        join: Spanned<Join>,
    },
    If {
        condition: Spanned<Cond>,
        then_body: Block,
        else_body: Block,
    },
    End(Ident),
}

/// Binding is the result/branch name; actor and capability are unresolved names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub recovery: Option<Box<Block>>,
    pub query_command: Option<Box<Ident>>,
    pub binding: Ident,
    pub actor: Ident,
    pub capability: Ident,
    pub args: Vec<Ident>,
    pub response_loss: Option<Spanned<ResponseLoss>>,
    pub retry: Option<Retry>,
    pub span: Span,
}

/// Receipt is a channel observation, never a member of the result enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Receipt {
    Delivered,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryAvailability {
    Available,
    Unavailable,
    Nondet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseLoss {
    None,
    Nondet,
    Sequence(Vec<Spanned<Receipt>>),
    Probability {
        probability: Ident,
        basis: Option<Ident>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retry {
    pub max_attempts: Integer,
    pub receipt_unknown: bool,
    pub results: Vec<Ident>,
    pub backoff: Duration,
    pub exhausted: Ident,
    pub span: Span,
}

/// Any carries every mandatory clause; all has no implicit success condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Join {
    All {
        fork: Ident,
    },
    Any {
        fork: Ident,
        result: Ident,
        tie: Spanned<Tie>,
        remaining: Spanned<Remaining>,
        if_none: Ident,
    },
}

/// By retains the written order; Nondet must never become a random choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tie {
    By(Vec<Ident>),
    Nondet,
}

/// Both alternatives drain every branch before subsequent statements execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remaining {
    CancelThenDrain,
    Drain,
}

/// Each unparenthesized chain has only one operator (no implicit precedence).
/// A chain has >= 2 operands; Group preserves explicit parentheses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cond {
    Contains {
        element: Path,
        set: Path,
    },
    Compare {
        binding: Ident,
        operator: Spanned<IntComparison>,
        value: Ident,
    },
    LocalEq {
        actor: Ident,
        variable: Ident,
        operator: Spanned<IntComparison>,
        value: Ident,
    },
    Eq {
        binding: Ident,
        value: Ident,
    },
    And(Vec<Spanned<Cond>>),
    Or(Vec<Spanned<Cond>>),
    Group(Box<Spanned<Cond>>),
}

/// At least one bound syntactically; duplicates and missing fields survive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bounds {
    pub items: Vec<Spanned<Bound>>,
    pub span: Span,
}

/// No implicit clock, task, or case bounds are invented by the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bound {
    Cases(Integer),
    LiveTasks(Integer),
    Clock(Duration),
}

/// Only the two property forms admitted by the shared vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Property {
    RecoverableEnd,
    TerminatingEnd,
    AlmostSureEnd,
    ProbabilityReached {
        condition: ProbabilityCondition,
        within: Option<Box<Duration>>,
        comparison: Comparison,
        bound: Ident,
    },
    NoResourceOvercommit,
    ReachableEnd(Ident),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbabilityCondition {
    State(Spanned<Cond>),
    End(Ident),
}

/// Verification requests preserve source order and their complete source span.
pub type Verify = Spanned<Property>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarDecl {
    pub last_write_wins: Vec<Span>,
    pub case_set: bool,
    pub integer_max: Option<u64>,
    pub distribution: Option<Vec<(Ident, Ident)>>,
    pub names: Vec<Ident>,
    pub ty: Ident,
    pub optional: bool,
    pub default: Option<ValueExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub operator: AssignmentOp,
    pub target: StateTarget,
    pub value: ValueExpr,
}

/// Preserve ownership syntax until name resolution; never encode a path in a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateTarget {
    Unqualified(Ident),
    Qualified {
        actor: Ident,
        variable: Ident,
        span: Span,
    },
}

impl StateTarget {
    pub fn span(&self) -> Span {
        match self {
            Self::Unqualified(n) => n.span,
            Self::Qualified { span, .. } => *span,
        }
    }
}

impl std::ops::Deref for StateTarget {
    type Target = Ident;
    fn deref(&self) -> &Ident {
        match self {
            Self::Unqualified(n) => n,
            Self::Qualified { variable, .. } => variable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOp {
    Assign,
    Add,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntComparison {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    Le,
    Ge,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelDecl {
    pub name: Ident,
    pub given: Vec<Path>,
    pub basis: Vec<Spanned<String>>,
    pub rows: Vec<KernelRow>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelRow {
    pub pattern: Option<Vec<Ident>>,
    pub branches: Vec<KernelBranch>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelBranch {
    pub probability: Ident,
    pub result: ValueExpr,
    pub assignments: Vec<Assignment>,
}
