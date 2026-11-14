//! ADR 0034. Static reachability and exercise, not authorization, observed
//! execution, practical framing, or a measurement of anyone's skill.
//!
//! The optional recorder uses the legacy path rules but its own budgets. None
//! mode never records or charges this module's work to the legacy derivation.
use super::*;

const NOTICE: &str = "行使回数から不要とは判定しない";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "ViewpointsWire")]
pub struct Viewpoints {
    pub rules: String,
    pub scope: String,
    pub observation: Observation,
    pub framing: Framing,
    pub proposal: Proposal,
    pub retention: Retention,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<effects::Effects>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewpointsWire {
    rules: String,
    scope: String,
    observation: Observation,
    framing: Framing,
    proposal: Proposal,
    retention: Retention,
    effects: Option<effects::Effects>,
}
impl TryFrom<ViewpointsWire> for Viewpoints {
    type Error = &'static str;
    fn try_from(w: ViewpointsWire) -> std::result::Result<Self, Self::Error> {
        if !matches!(
            w.rules.as_str(),
            "agency-viewpoints-p1-1" | "agency-viewpoints-p1-2"
        ) || (w.rules == "agency-viewpoints-p1-2") != w.effects.is_some()
        {
            return Err("viewpoints rules/effects mismatch");
        }
        Ok(Self {
            rules: w.rules,
            scope: w.scope,
            observation: w.observation,
            framing: w.framing,
            proposal: w.proposal,
            retention: w.retention,
            effects: w.effects,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitReason {
    MergeControl,
    LoopControl,
    ResponseControl,
    DerivationLimit,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateReason {
    MergeControl,
    LoopControl,
    ResponseControl,
    StateGuards,
    /// A `uses` amount exceeds its resource's capacity: the call never starts.
    ResourceInfeasible,
    /// A lexical call has Actor capacity Some(0), or requires its completion.
    ActorCapacityInfeasible,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalReason {
    NoProposalContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Observation {
    Derived {
        sources: Vec<Source>,
        people: Vec<Person>,
        fallback: String,
    },
    NotDerived {
        reason: LimitReason,
        fallback: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub actor: String,
    pub normal: Reach,
    pub contest: ContestReach,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reach {
    pub actors: Vec<String>,
    pub people: Vec<String>,
    pub unattributed_actors: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContestReach {
    pub actors: Vec<String>,
    pub people: Vec<String>,
    pub unattributed_actors: Vec<String>,
    pub routes: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    pub person: String,
    pub normal: PersonNormal,
    pub contest: PersonContest,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonNormal {
    pub direct: Vec<String>,
    pub indirect: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonContest {
    pub direct: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Framing {
    Derived {
        ends: Vec<End>,
        actors: Vec<String>,
        complete: bool,
        unchecked: Vec<String>,
    },
    NotDerived {
        reason: LimitReason,
        complete: bool,
        unchecked: Vec<String>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum End {
    Derived {
        end: String,
        reachable: bool,
        selectors: Vec<Selector>,
        read_only: Vec<ReadOnly>,
    },
    NotDerived {
        end: String,
        reason: StateReason,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    pub call: String,
    pub capability: String,
    pub selects: Selects,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<bool>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Selects {
    AllContexts,
    SomeContexts,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadOnly {
    pub call: String,
    pub capability: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Proposal {
    NotDerived { reason: ProposalReason },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Retention {
    Derived {
        capabilities: Vec<Exercise>,
        notice: String,
        unchecked: Vec<String>,
    },
    NotDerived {
        reason: LimitReason,
        notice: String,
        unchecked: Vec<String>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Exercise {
    Derived {
        capability: String,
        normal: Count,
        contest_route: bool,
        basis: String,
        fallback: Fallback,
        join_any_cancellation: bool,
    },
    NotDerived {
        capability: String,
        reason: StateReason,
        contest_route: bool,
        basis: String,
        fallback: Fallback,
        join_any_cancellation: bool,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fallback {
    Unchecked,
    KeptByIntent,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Count {
    pub min: usize,
    pub max: usize,
}

fn framing_unchecked() -> Vec<String> {
    ["type_domain_authority", "practical_framing"]
        .map(String::from)
        .to_vec()
}
fn retention_unchecked() -> Vec<String> {
    ["fallback_dependency", "retention_basis", "actual_skill"]
        .map(String::from)
        .to_vec()
}
pub(super) fn budget(limits: Limits) -> Budget {
    Budget {
        limits,
        work: 0,
        cells: 0,
        contexts: 0,
        paths: 0,
        location: Location::default(),
    }
}
pub(super) fn depth(b: &mut Budget, n: usize) -> Result<()> {
    b.work()?;
    b.depth(n)
}
pub(super) fn condition_depth(c: &ir::Cond, b: &mut Budget, n: usize) -> Result<()> {
    depth(b, n)?;
    // Reserve the symbolic tree before symbolic() constructs it. This is only
    // called by the recorder, never by the legacy path mode.
    b.cells(1)?;
    if let ir::Cond::And(cs) | ir::Cond::Or(cs) = c {
        for c in cs {
            condition_depth(c, b, n.checked_add(1).ok_or_else(invalid)?)?;
        }
    }
    Ok(())
}
pub(super) fn symbolic_storage(c: &Symbolic, b: &mut Budget) -> Result<()> {
    if let Symbolic::And(cs) | Symbolic::Or(cs) = c {
        b.copy(cs.len())?;
        for c in cs {
            symbolic_storage(c, b)?;
        }
    }
    Ok(())
}
pub(super) fn guard_storage(p: &Path, b: &mut Budget) -> Result<()> {
    for g in &p.guards {
        b.work()?;
        match g {
            Guard::Domain(_, values) => b.copy(values.len())?,
            Guard::Symbolic(c, _) => symbolic_storage(c, b)?,
            _ => {}
        }
    }
    Ok(())
}
fn push_name(out: &mut Vec<String>, name: &str, b: &mut Budget) -> Result<()> {
    b.copy(1)?;
    out.push(name.into());
    Ok(())
}
pub(super) fn cap_name(m: &ir::Model, id: ir::CapabilityId) -> Result<String> {
    let c = m.capabilities.get(id.0).ok_or_else(invalid)?;
    Ok(format!(
        "{}.{}",
        m.actors.get(c.actor.0).ok_or_else(invalid)?.name,
        c.name
    ))
}
fn routes(m: &ir::Model, b: &mut Budget) -> Result<BTreeSet<ir::CapabilityId>> {
    let mut out = BTreeSet::new();
    for c in m.contests() {
        b.insert(&mut out, c.route)?;
    }
    Ok(out)
}

// Same rule as the engine's start condition (engine.rs resources_fit with
// nothing running, and the shortages explanation with occupied 0): a use
// fits only if amount <= capacity. Model-size scan, uncharged, so a model
// with every use within capacity keeps its exact budget accounting.
fn infeasible_capabilities(m: &ir::Model) -> Result<BTreeSet<ir::CapabilityId>> {
    let mut out = BTreeSet::new();
    for (i, cap) in m.capabilities.iter().enumerate() {
        for u in &cap.uses {
            if u.amount > m.resources.get(u.resource.0).ok_or_else(invalid)?.capacity {
                out.insert(ir::CapabilityId(i));
            }
        }
    }
    Ok(out)
}
pub(super) fn precise_resources(m: &ir::Model) -> bool {
    !m.start_policy
        .as_ref()
        .is_some_and(|p| p.call_order || p.priority.is_some())
}
pub(super) fn infeasible_call(m: &ir::Model, c: &ir::Call) -> Result<bool> {
    let cap = m.capabilities.get(c.capability.0).ok_or_else(invalid)?;
    for u in &cap.uses {
        if u.amount > m.resources.get(u.resource.0).ok_or_else(invalid)?.capacity {
            return Ok(true);
        }
    }
    Ok(false)
}
// The first (lexically, binding order) call on the path that can never start,
// including a requires call: the engine evaluates requires only once its
// resources fit, so neither the start nor the refusal happens.
fn first_infeasible(
    p: &Path,
    calls: &[&ir::Call],
    infeasible: &BTreeSet<ir::CapabilityId>,
    b: &mut Budget,
) -> Result<Option<ir::BindingId>> {
    if infeasible.is_empty() {
        return Ok(None);
    }
    b.steps(p.defined.len())?;
    for id in &p.defined {
        if infeasible.contains(&calls.get(id.0).ok_or_else(invalid)?.capability) {
            return Ok(Some(*id));
        }
    }
    Ok(None)
}

// Outside the limited CancelThenDrain recorder below, capacity-zero withdrawal
// keeps the existing paths/counts and withdraws satisfiable dependent rows.
// Stages follow control-flow order, with fork siblings sharing a stage. Path
// membership keeps the two arms of an If separate; no BindingId ordering is
// needed to decide completion dependence.
struct ActorCapacity {
    zero: BTreeSet<ir::BindingId>,
    stages: Vec<usize>,
}
impl ActorCapacity {
    fn new(m: &ir::Model, calls: &[&ir::Call], b: &mut Budget) -> Result<Self> {
        let mut zero = BTreeSet::new();
        // As for the resource scan, no extra charges on unaffected models.
        for c in calls {
            let cap = m.capabilities.get(c.capability.0).ok_or_else(invalid)?;
            if m.actors.get(cap.actor.0).ok_or_else(invalid)?.capacity == Some(0) {
                b.insert(&mut zero, c.binding)?;
            }
        }
        let mut stages = Vec::new();
        if !zero.is_empty() {
            b.copy(calls.len())?;
            stages.resize(calls.len(), 0);
            Self::index(&m.process.body, &mut stages, &mut 0, 0, b)?;
        }
        Ok(Self { zero, stages })
    }
    fn index(
        block: &ir::Block,
        stages: &mut [usize],
        next: &mut usize,
        level: usize,
        b: &mut Budget,
    ) -> Result<()> {
        depth(b, level)?;
        for stmt in block {
            b.work()?;
            *next = next.checked_add(1).ok_or_else(invalid)?;
            match &stmt.kind {
                ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => {
                    *stages.get_mut(c.binding.0).ok_or_else(invalid)? = *next;
                }
                ir::Stmt::Fork { branches, .. } => {
                    for c in branches {
                        b.work()?;
                        *stages.get_mut(c.binding.0).ok_or_else(invalid)? = *next;
                    }
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    Self::index(then_body, stages, next, level + 1, b)?;
                    Self::index(else_body, stages, next, level + 1, b)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn first_stage(&self, p: &Path, b: &mut Budget) -> Result<Option<usize>> {
        let mut first = None;
        for id in &self.zero {
            b.work()?;
            if p.defined.contains(id) {
                let stage = self.stages[id.0];
                first = Some(first.map_or(stage, |old: usize| old.min(stage)));
            }
        }
        Ok(first)
    }
}

fn unknown_guard(g: &Guard) -> bool {
    matches!(g, Guard::If(c, _) if has_state(c))
        || matches!(g, Guard::Symbolic(c, _) if c.unknown())
}

// Unknown state guards cannot establish impossibility, but a contradictory
// result guard must not turn a dead arm's capacity-zero call into a withdrawal.
fn possibly_satisfied(
    p: &Path,
    calls: &[&ir::Call],
    m: &ir::Model,
    b: &mut Budget,
) -> Result<bool> {
    b.copy(p.guards.len())?;
    guard_storage(p, b)?;
    let relaxed = Path {
        guards: p
            .guards
            .iter()
            .filter(|g| !unknown_guard(g))
            .cloned()
            .collect(),
        ..Path::default()
    };
    let local = path_support(&relaxed, m, b)?;
    b.copy(local.len())?;
    assignments(
        &local.into_iter().collect::<Vec<_>>(),
        &Values::new(),
        calls,
        m,
        b,
        |v, b| satisfied(&relaxed, v, b),
    )
}

#[cfg(test)]
pub(super) fn derive(m: &ir::Model, a: &Agency) -> (Viewpoints, Vec<diag::Diagnostic>) {
    with_limits_and_warnings(m, a, [DEFAULT; 3], DEFAULT)
}
#[cfg(test)]
fn with_limits(m: &ir::Model, a: &Agency, limits: [Limits; 3]) -> Viewpoints {
    with_limits_and_warnings(m, a, limits, DEFAULT).0
}
pub(super) fn with_limits_and_warnings(
    m: &ir::Model,
    a: &Agency,
    limits: [Limits; 3],
    effects_limits: Limits,
) -> (Viewpoints, Vec<diag::Diagnostic>) {
    with_limits_and_warnings_shared(m, a, limits, effects_limits, false)
}
pub(super) fn with_limits_and_warnings_shared(
    m: &ir::Model,
    a: &Agency,
    limits: [Limits; 3],
    effects_limits: Limits,
    shared: bool,
) -> (Viewpoints, Vec<diag::Diagnostic>) {
    let (effects, warnings) = if shared {
        effects::derive_shared(m, a, effects_limits, true)
    } else {
        effects::derive(m, a, effects_limits)
    };
    (
        Viewpoints {
            rules: "agency-viewpoints-p1-2".into(),
            scope: "selected_process_lexical_calls_and_declared_contest_routes".into(),
            observation: observation(m, a, &mut budget(limits[0])).unwrap_or_else(|_| {
                Observation::NotDerived {
                    reason: LimitReason::DerivationLimit,
                    fallback: "unchecked".into(),
                }
            }),
            framing: if m.has_merges() {
                Framing::NotDerived {
                    reason: LimitReason::MergeControl,
                    complete: false,
                    unchecked: framing_unchecked(),
                }
            } else {
                (if shared {
                    framing_shared(m, &mut budget(limits[1]), true)
                } else {
                    framing(m, &mut budget(limits[1]))
                })
                .unwrap_or_else(|_| Framing::NotDerived {
                    reason: LimitReason::DerivationLimit,
                    complete: false,
                    unchecked: framing_unchecked(),
                })
            },
            proposal: Proposal::NotDerived {
                reason: ProposalReason::NoProposalContract,
            },
            retention: if m.has_merges() {
                Retention::NotDerived {
                    reason: LimitReason::MergeControl,
                    notice: NOTICE.into(),
                    unchecked: retention_unchecked(),
                }
            } else {
                (if shared {
                    retention_shared(m, &mut budget(limits[2]), true)
                } else {
                    retention(m, &mut budget(limits[2]))
                })
                .unwrap_or_else(|_| Retention::NotDerived {
                    reason: LimitReason::DerivationLimit,
                    notice: NOTICE.into(),
                    unchecked: retention_unchecked(),
                })
            },
            effects: Some(effects),
        },
        warnings,
    )
}

fn observation(m: &ir::Model, a: &Agency, b: &mut Budget) -> Result<Observation> {
    depth(b, 3)?;
    let routes = routes(m, b)?;
    let mut sources = Vec::new();
    for (sid, source) in m.actors.iter().enumerate() {
        b.work()?;
        if source.source.is_none() {
            continue;
        }
        b.copy(3)?;
        let mut normal = BTreeSet::new();
        let mut contest = BTreeSet::new();
        let mut route_names = Vec::new();
        for (i, origin) in a.origin.iter().enumerate() {
            b.steps(origin.direct.len() + 1)?;
            if origin.direct.contains(&source.name) {
                b.insert(&mut normal, ir::ActorId(i))?;
            }
        }
        // Capability order, not contest order or name order.
        for (i, cap) in m.capabilities.iter().enumerate() {
            b.steps(cap.reads.len() + 1)?;
            if routes.contains(&ir::CapabilityId(i)) && cap.reads.contains(&ir::ActorId(sid)) {
                b.insert(&mut contest, cap.actor)?;
                b.copy(1)?;
                route_names.push(cap_name(m, ir::CapabilityId(i))?);
            }
        }
        let normal = reach(m, &normal, b)?;
        let r = reach(m, &contest, b)?;
        sources.push(Source {
            actor: source.name.clone(),
            normal,
            contest: ContestReach {
                actors: r.actors,
                people: r.people,
                unattributed_actors: r.unattributed_actors,
                routes: route_names,
            },
        });
    }
    let mut people = Vec::new();
    for (pid, person) in m.people.iter().enumerate() {
        b.copy(3)?;
        let mut direct = BTreeSet::new();
        let mut indirect = BTreeSet::new();
        let mut contest = BTreeSet::new();
        for (i, actor) in m.actors.iter().enumerate() {
            b.steps(actor.executors.as_ref().map_or(0, Vec::len) + 1)?;
            if !actor
                .executors
                .as_ref()
                .is_some_and(|e| e.contains(&ir::PersonId(pid)))
            {
                continue;
            }
            let origin = a.origin.get(i).ok_or_else(invalid)?;
            for s in &origin.direct {
                b.insert(&mut direct, s.as_str())?;
            }
            for s in &origin.indirect {
                b.insert(&mut indirect, s.as_str())?;
            }
            for id in &routes {
                let cap = m.capabilities.get(id.0).ok_or_else(invalid)?;
                b.work()?;
                if cap.actor.0 == i {
                    for sid in &cap.reads {
                        b.insert(
                            &mut contest,
                            m.actors.get(sid.0).ok_or_else(invalid)?.name.as_str(),
                        )?;
                    }
                }
            }
        }
        let ordered = |set: &BTreeSet<&str>, b: &mut Budget| -> Result<Vec<String>> {
            let mut out = Vec::new();
            for s in &m.actors {
                b.work()?;
                if set.contains(s.name.as_str()) {
                    push_name(&mut out, &s.name, b)?;
                }
            }
            Ok(out)
        };
        people.push(Person {
            person: person.name.clone(),
            normal: PersonNormal {
                direct: ordered(&direct, b)?,
                indirect: ordered(&indirect, b)?,
            },
            contest: PersonContest {
                direct: ordered(&contest, b)?,
            },
        });
    }
    Ok(Observation::Derived {
        sources,
        people,
        fallback: "unchecked".into(),
    })
}
fn reach(m: &ir::Model, ids: &BTreeSet<ir::ActorId>, b: &mut Budget) -> Result<Reach> {
    let mut out = Reach {
        actors: Vec::new(),
        people: Vec::new(),
        unattributed_actors: Vec::new(),
    };
    let mut people = BTreeSet::new();
    for id in ids {
        let actor = m.actors.get(id.0).ok_or_else(invalid)?;
        push_name(&mut out.actors, &actor.name, b)?;
        match &actor.executors {
            Some(ps) => {
                for p in ps {
                    b.insert(&mut people, *p)?;
                }
            }
            None => push_name(&mut out.unattributed_actors, &actor.name, b)?,
        }
    }
    for (i, p) in m.people.iter().enumerate() {
        b.work()?;
        if people.contains(&ir::PersonId(i)) {
            push_name(&mut out.people, &p.name, b)?;
        }
    }
    Ok(out)
}

#[derive(Clone, Default)]
pub(super) struct Trace {
    pub(super) blocked: bool,
    pub(super) started: BTreeSet<ir::BindingId>,
    pub(super) cancellation: BTreeSet<ir::BindingId>,
    pub(super) unknown_results: BTreeSet<ir::BindingId>,
    // Queued capacity-zero siblings have no result/definition. Keep their L
    // withdrawal separate from completion dependence of the fork's suffix.
    unstarted_zero: BTreeSet<ir::BindingId>,
}
impl Trace {
    pub(super) fn cells(&self) -> Result<usize> {
        self.started
            .len()
            .checked_add(self.cancellation.len())
            .and_then(|n| n.checked_add(self.unknown_results.len()))
            .and_then(|n| n.checked_add(self.unstarted_zero.len()))
            .ok_or_else(invalid)
    }
}
#[derive(Default)]
pub(super) struct Recorder<'a> {
    sinks: Vec<(Option<ir::EndId>, Path<'a>)>,
}
impl<'a> Recorder<'a> {
    pub(super) fn blocked(&mut self, mut p: Path<'a>, b: &mut Budget) -> Result<()> {
        p.trace.as_mut().ok_or_else(invalid)?.blocked = true;
        self.sink(None, p, b)
    }
    // V1 deliberately opts in only to the simple, resource-free domain. In
    // particular, no-match/zero-only matches retain U1's conservative paths.
    pub(super) fn precise_actor_fork(
        &self,
        incoming: &[Path<'a>],
        branches: &[ir::Call],
        join: &ir::Join,
        m: &ir::Model,
        b: &mut Budget,
    ) -> Result<BTreeSet<ir::BindingId>> {
        let ir::Join::Any {
            matches,
            remaining: ir::Remaining::CancelThenDrain,
            ..
        } = join
        else {
            return Ok(BTreeSet::new());
        };
        // No budget changes for models without a capacity-zero sibling.
        if !branches
            .iter()
            .any(|c| m.actors[m.capabilities[c.capability.0].actor.0].capacity == Some(0))
        {
            return Ok(BTreeSet::new());
        }
        if m.bounds.cases != 1
            || incoming.iter().any(uncertain)
            || m.capabilities.iter().any(|c| {
                c.requires.is_some()
                    || c.history_dependent_result()
                    || !c.uses.is_empty()
                    || c.duration == 0
                    || c.duration_distribution.is_some()
            })
        {
            return Ok(BTreeSet::new());
        }
        let mut zero = BTreeSet::new();
        let mut can_match = false;
        for c in branches {
            b.work()?;
            let cap = &m.capabilities[c.capability.0];
            if m.actors[cap.actor.0].capacity == Some(0) {
                b.insert(&mut zero, c.binding)?;
            } else {
                for x in matches {
                    b.steps(cap.results.len() + 1)?;
                    can_match |= x.binding == c.binding && cap.results.contains(&x.value);
                }
            }
        }
        // A cancelled result is absent, not a fresh nondeterministic value.
        // Leave models that read it outside this join to the old withdrawal.
        if !can_match || reads_zero(&m.process.body, &zero, m, b, 0)? {
            zero.clear();
        }
        Ok(zero)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn infeasible_fork(
        &mut self,
        mut incoming: Vec<Path<'a>>,
        branches: &[ir::Call],
        join: &'a ir::Join,
        m: &ir::Model,
        b: &mut Budget,
        actor_zero: &BTreeSet<ir::BindingId>,
    ) -> Result<Vec<Path<'a>>> {
        let mut feasible = BTreeSet::new();
        let mut effects = BTreeMap::new();
        for c in branches {
            if !actor_zero.contains(&c.binding) && !infeasible_call(m, c)? {
                b.insert(&mut feasible, c.binding)?;
            }
        }
        for p in &mut incoming {
            for id in actor_zero {
                b.insert(
                    &mut p.trace.as_mut().ok_or_else(invalid)?.unstarted_zero,
                    *id,
                )?;
            }
        }
        // Preserve the concurrent-writer uncertainty of the ordinary fork.
        let written: BTreeSet<_> = branches
            .iter()
            .filter(|c| feasible.contains(&c.binding))
            .flat_map(|c| {
                let cap = &m.capabilities[c.capability.0];
                cap.start_writes
                    .iter()
                    .chain(cap.finish_writes())
                    .map(|w| w.variable)
                    .chain(cap.kernel_writes.iter().copied())
            })
            .collect();
        for c in branches {
            if !feasible.contains(&c.binding) {
                continue;
            }
            for p in &mut incoming {
                b.copy(written.len())?;
                for v in &written {
                    p.store.insert(*v, Provenance::Unknown);
                }
            }
            incoming = call_paths(c, incoming, &mut effects, m, b)?;
        }
        for p in &mut incoming {
            b.copy(written.len())?;
            for v in &written {
                p.store.insert(*v, Provenance::Unknown);
            }
        }
        let mut out = Vec::new();
        if let ir::Join::Any {
            matches,
            remaining: ir::Remaining::CancelThenDrain,
            ..
        } = join
        {
            let mut terms = Vec::new();
            for x in matches {
                b.work()?;
                if feasible.contains(&x.binding) {
                    b.copy(1)?;
                    terms.push(if unknown_atomic_result(&ir::Cond::Eq(*x), m) {
                        Symbolic::Unknown
                    } else {
                        Symbolic::Eq(*x)
                    });
                }
            }
            let guard = Symbolic::Or(terms);
            for mut p in incoming {
                b.copy(
                    p.defined.len()
                        + p.guards.len()
                        + p.store.len()
                        + p.trace.as_ref().ok_or_else(invalid)?.cells()?
                        + 2,
                )?;
                guard_storage(&p, b)?;
                symbolic_storage(&guard, b)?;
                let mut stopped = p.clone();
                stopped.guards.push(Guard::Symbolic(guard.clone(), false));
                self.blocked(stopped, b)?;
                p.guards.push(Guard::Symbolic(guard.clone(), true));
                let trace = p.trace.as_mut().ok_or_else(invalid)?;
                for id in &feasible {
                    if trace.started.contains(id) {
                        b.insert(&mut trace.cancellation, *id)?;
                    }
                }
                for id in actor_zero {
                    b.insert(&mut trace.cancellation, *id)?;
                }
                out.push(p);
            }
        } else {
            for p in incoming {
                self.blocked(p, b)?;
            }
        }
        Ok(out)
    }
    pub(super) fn sink(
        &mut self,
        end: Option<ir::EndId>,
        p: Path<'a>,
        b: &mut Budget,
    ) -> Result<()> {
        b.path()?;
        b.cells(1)?;
        self.sinks.push((end, p));
        Ok(())
    }
    pub(super) fn join(
        &mut self,
        incoming: &mut [Path<'a>],
        branches: &[ir::Call],
        join: &'a ir::Join,
        m: &ir::Model,
        b: &mut Budget,
    ) -> Result<()> {
        if let ir::Join::Any {
            matches,
            remaining,
            if_none,
            ..
        } = join
        {
            for p in incoming {
                if *remaining == ir::Remaining::CancelThenDrain {
                    let trace = p.trace.as_mut().ok_or_else(invalid)?;
                    for c in branches {
                        b.work()?;
                        if trace.started.contains(&c.binding) {
                            b.insert(&mut trace.cancellation, c.binding)?;
                        }
                    }
                }
                b.copy(
                    p.defined.len()
                        + p.guards.len()
                        + p.store.len()
                        + p.trace.as_ref().ok_or_else(invalid)?.cells()?
                        + matches.len()
                        + 1,
                )?;
                guard_storage(p, b)?;
                let mut none = p.clone();
                // An atomic result is decided by the Actor state at commit
                // (ADR0035), so adoption is a state guard, exactly as an `if`
                // on that result: Unknown, never a free result choice.
                let atomic = matches
                    .iter()
                    .any(|x| unknown_atomic_result(&ir::Cond::Eq(*x), m));
                if atomic {
                    none.guards.push(Guard::Symbolic(Symbolic::Unknown, false));
                } else {
                    // NOT(OR matches), not NOT(AND matches). Legacy Any is unchanged.
                    for x in matches {
                        none.guards.push(Guard::Symbolic(Symbolic::Eq(*x), false));
                    }
                }
                self.sink(Some(*if_none), none, b)?;
                b.copy(1)?;
                p.guards.push(if atomic {
                    Guard::Symbolic(Symbolic::Unknown, true)
                } else {
                    Guard::Any(matches)
                });
            }
        }
        Ok(())
    }
}
fn reads_zero(
    block: &ir::Block,
    zero: &BTreeSet<ir::BindingId>,
    m: &ir::Model,
    b: &mut Budget,
    level: usize,
) -> Result<bool> {
    depth(b, level)?;
    for stmt in block {
        b.work()?;
        match &stmt.kind {
            ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => {
                b.steps(c.args.len())?;
                if c.args.iter().any(|id| zero.contains(id)) {
                    return Ok(true);
                }
            }
            ir::Stmt::Fork { branches, .. } => {
                for c in branches {
                    b.steps(c.args.len() + 1)?;
                    if c.args.iter().any(|id| zero.contains(id)) {
                        return Ok(true);
                    }
                }
            }
            ir::Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                let mut support = BTreeSet::new();
                condition_support(condition, &mut support, m, 0, b)?;
                if !zero.is_disjoint(&support)
                    || reads_zero(then_body, zero, m, b, level + 1)?
                    || reads_zero(else_body, zero, m, b, level + 1)?
                {
                    return Ok(true);
                }
            }
            _ => {}
        }
    }
    Ok(false)
}
fn recorded<'a>(m: &'a ir::Model, b: &mut Budget) -> Result<(Vec<&'a ir::Call>, Recorder<'a>)> {
    recorded_shared(m, b, false)
}
fn recorded_shared<'a>(
    m: &'a ir::Model,
    b: &mut Budget,
    shared: bool,
) -> Result<(Vec<&'a ir::Call>, Recorder<'a>)> {
    b.copy(m.process.bindings.len())?;
    let mut indexed = vec![None; m.process.bindings.len()];
    collect_shared(&m.process.body, &mut indexed, m, 0, b, shared)?;
    let calls = indexed
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)?;
    let mut r = Recorder::default();
    b.path()?;
    b.cells(1)?;
    let mut initial = Path {
        trace: Some(Trace::default()),
        ..Path::default()
    };
    for (i, v) in m.state_variables.iter().enumerate() {
        b.work()?;
        // An unset optional is not evidence of
        // an exercised/rejected operation. F/L conservatively leave it unknown;
        // symbolic() and the legacy U/refusal mode retain their existing rules.
        if v.optional && v.initial == ir::InitialValue::Fixed(ir::StateValue::None) {
            b.copy(1)?;
            initial.store.insert(ir::StateVarId(i), Provenance::Unknown);
        }
    }
    let tail = paths(
        &m.process.body,
        vec![initial],
        &mut BTreeMap::new(),
        m,
        b,
        Some(&mut r),
        0,
    )?;
    for p in tail {
        r.sink(None, p, b)?;
    }
    Ok((calls, r))
}
pub(super) fn uncertain(p: &Path) -> bool {
    p.guards.iter().any(|g| {
        matches!(g, Guard::If(c, _) if has_state(c))
            || matches!(g, Guard::Symbolic(c, _) if c.unknown())
    })
}
// Domain is evaluated but is not something the process reads (Q5).
fn read_support(p: &Path, m: &ir::Model, b: &mut Budget) -> Result<BTreeSet<ir::BindingId>> {
    let mut out = BTreeSet::new();
    for g in &p.guards {
        match g {
            Guard::Domain(..) => {}
            Guard::If(c, _) => condition_support(c, &mut out, m, 0, b)?,
            Guard::Symbolic(c, _) => c.support(&mut out, b)?,
            Guard::Any(ms) => {
                for x in *ms {
                    b.insert(&mut out, x.binding)?;
                }
            }
        }
    }
    Ok(out)
}
// Same mixed-radix/domain semantics as the legacy evaluator, with independent
// accounting for all new-mode buffers. Borrow the result domain instead of
// cloning it on every increment. Never allocate a Cartesian product.
fn assignments(
    vars: &[ir::BindingId],
    fixed: &Values,
    calls: &[&ir::Call],
    m: &ir::Model,
    b: &mut Budget,
    mut visit: impl FnMut(&Values, &mut Budget) -> Result<bool>,
) -> Result<bool> {
    b.copy(
        vars.len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(fixed.len()))
            .ok_or_else(invalid)?,
    )?;
    let vars: Vec<_> = vars
        .iter()
        .copied()
        .filter(|v| !fixed.contains_key(v))
        .collect();
    let mut indices = vec![0usize; vars.len()];
    let mut values = fixed.clone();
    let cap = |id: ir::BindingId| -> Result<&ir::Capability> {
        m.capabilities
            .get(calls.get(id.0).ok_or_else(invalid)?.capability.0)
            .ok_or_else(invalid)
    };
    for id in &vars {
        b.work()?;
        values.insert(*id, *cap(*id)?.results.first().ok_or_else(invalid)?);
    }
    loop {
        b.work()?;
        if visit(&values, b)? {
            return Ok(true);
        }
        let mut i = 0;
        loop {
            if i == vars.len() {
                return Ok(false);
            }
            b.work()?;
            let id = vars[i];
            let c = cap(id)?;
            let other = c
                .requires
                .as_ref()
                .map(|r| r.otherwise)
                .filter(|v| !c.results.contains(v));
            b.steps(c.results.len())?;
            indices[i] = indices[i].checked_add(1).ok_or_else(invalid)?;
            if let Some(v) = c
                .results
                .get(indices[i])
                .copied()
                .or_else(|| (indices[i] == c.results.len()).then_some(other).flatten())
            {
                values.insert(id, v);
                break;
            }
            indices[i] = 0;
            values.insert(id, *c.results.first().ok_or_else(invalid)?);
            i += 1;
        }
    }
}
fn framing(m: &ir::Model, b: &mut Budget) -> Result<Framing> {
    framing_shared(m, b, false)
}
fn framing_shared(m: &ir::Model, b: &mut Budget, shared: bool) -> Result<Framing> {
    let (calls, r) = if shared {
        recorded_shared(m, b, true)?
    } else {
        recorded(m, b)?
    };
    let capacity = ActorCapacity::new(m, &calls, b)?;
    let infeasible = if precise_resources(m) {
        BTreeSet::new()
    } else {
        infeasible_capabilities(m)?
    };
    let mut ends = Vec::new();
    let mut actors = BTreeSet::new();
    let mut complete = true;
    for (i, end) in m.ends.iter().enumerate() {
        if m.overflow_end == Some(ir::EndId(i)) {
            continue;
        }
        b.copy(1)?;
        let mut ps = Vec::new();
        for (id, p) in &r.sinks {
            b.work()?;
            if *id == Some(ir::EndId(i)) {
                b.copy(1)?;
                ps.push(p);
            }
        }
        let mut blocked = false;
        for p in &ps {
            if first_infeasible(p, &calls, &infeasible, b)?.is_some() {
                blocked = true;
                break;
            }
        }
        if blocked {
            // Fail closed: no new derivation of reachable:false (ruling).
            complete = false;
            ends.push(End::NotDerived {
                end: end.clone(),
                reason: StateReason::ResourceInfeasible,
            });
            continue;
        }
        let mut actor_blocked = false;
        for p in &ps {
            if capacity.first_stage(p, b)?.is_some() && possibly_satisfied(p, &calls, m, b)? {
                actor_blocked = true;
                break;
            }
        }
        if actor_blocked {
            complete = false;
            ends.push(End::NotDerived {
                end: end.clone(),
                reason: StateReason::ActorCapacityInfeasible,
            });
            continue;
        }
        if ps.iter().any(|p| uncertain(p)) {
            complete = false;
            ends.push(End::NotDerived {
                end: end.clone(),
                reason: StateReason::StateGuards,
            });
            continue;
        }
        let mut support = BTreeSet::new();
        let mut reachable = Vec::new();
        for p in ps {
            let local = path_support(p, m, b)?;
            b.copy(local.len())?;
            let vars: Vec<_> = local.iter().copied().collect();
            if assignments(&vars, &Values::new(), &calls, m, b, |v, b| {
                satisfied(p, v, b)
            })? {
                for id in read_support(p, m, b)? {
                    b.insert(&mut support, id)?;
                }
                b.copy(1)?;
                reachable.push((p, local));
            }
        }
        let mut selectors = Vec::new();
        let mut read_only = Vec::new();
        for j in &support {
            let mut contexts = BTreeSet::new();
            for (p, local) in &reachable {
                // Enumerate evaluation variables, then project *only reads*
                // defined on this path into a context (Q4/Q5).
                b.copy(
                    support
                        .len()
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(local.len()))
                        .ok_or_else(invalid)?,
                )?;
                let vars: Vec<_> = local
                    .union(&support.intersection(&p.defined).copied().collect())
                    .copied()
                    .collect();
                assignments(&vars, &Values::new(), &calls, m, b, |v, b| {
                    if satisfied(p, v, b)? {
                        b.copy(v.len())?;
                        let context: Values = v
                            .iter()
                            .filter(|(id, _)| {
                                **id != *j && support.contains(id) && p.defined.contains(id)
                            })
                            .map(|(id, v)| (*id, *v))
                            .collect();
                        if !contexts.contains(&context) {
                            b.context()?;
                            b.cells(1)?;
                            contexts.insert(context);
                        }
                    }
                    Ok(false)
                })?;
            }
            let mut selecting = false;
            let mut bypass = false;
            let mut has_blocked = false;
            for context in &contexts {
                let mut may = false;
                let mut cannot = false;
                for value in &m.capabilities[calls[j.0].capability.0].results {
                    b.work()?;
                    let mut reaches = false;
                    for (p, local) in &reachable {
                        b.copy(context.len() + local.len() + 1)?;
                        let mut fixed: Values = context
                            .iter()
                            .filter(|(id, _)| p.defined.contains(id))
                            .map(|(id, v)| (*id, *v))
                            .collect();
                        fixed.insert(*j, *value);
                        let vars: Vec<_> = local.iter().copied().collect();
                        if assignments(&vars, &fixed, &calls, m, b, |v, b| satisfied(p, v, b))? {
                            reaches = true;
                            break;
                        }
                    }
                    may |= reaches;
                    cannot |= !reaches;
                    if !reaches {
                        for (_, p) in &r.sinks {
                            if !p.trace.as_ref().is_some_and(|t| t.blocked)
                                || !p.defined.contains(j)
                                || uncertain(p)
                            {
                                continue;
                            }
                            let local = path_support(p, m, b)?;
                            b.copy(context.len() + local.len() + 1)?;
                            let mut fixed: Values = context
                                .iter()
                                .filter(|(id, _)| p.defined.contains(id))
                                .map(|(id, v)| (*id, *v))
                                .collect();
                            fixed.insert(*j, *value);
                            let vars: Vec<_> = local.into_iter().collect();
                            if assignments(&vars, &fixed, &calls, m, b, |v, b| satisfied(p, v, b))?
                            {
                                has_blocked = true;
                                break;
                            }
                        }
                    }
                }
                if may {
                    if cannot {
                        selecting = true;
                    } else {
                        bypass = true;
                    }
                }
            }
            b.copy(3)?;
            let call = calls[j.0];
            let name = m.process.bindings[j.0].name.clone();
            let capability = cap_name(m, call.capability)?;
            if selecting {
                b.insert(&mut actors, m.capabilities[call.capability.0].actor)?;
                selectors.push(Selector {
                    call: name,
                    capability,
                    selects: if bypass {
                        Selects::SomeContexts
                    } else {
                        Selects::AllContexts
                    },
                    blocked: has_blocked.then_some(true),
                });
            } else {
                read_only.push(ReadOnly {
                    call: name,
                    capability,
                });
            }
        }
        ends.push(End::Derived {
            end: end.clone(),
            reachable: !reachable.is_empty(),
            selectors,
            read_only,
        });
    }
    let mut actor_names = Vec::new();
    for id in actors {
        push_name(&mut actor_names, &m.actors[id.0].name, b)?;
    }
    Ok(Framing::Derived {
        ends,
        actors: actor_names,
        complete,
        unchecked: framing_unchecked(),
    })
}

fn retention(m: &ir::Model, b: &mut Budget) -> Result<Retention> {
    retention_shared(m, b, false)
}
fn retention_shared(m: &ir::Model, b: &mut Budget, shared: bool) -> Result<Retention> {
    let (calls, r) = if shared {
        recorded_shared(m, b, true)?
    } else {
        recorded(m, b)?
    };
    exercised(m, &calls, r, b)
}
fn exercised<'a>(
    m: &'a ir::Model,
    calls: &[&'a ir::Call],
    r: Recorder<'a>,
    b: &mut Budget,
) -> Result<Retention> {
    let routes = routes(m, b)?;
    let capacity = ActorCapacity::new(m, calls, b)?;
    let n = m.capabilities.len();
    b.copy(n.checked_mul(4).ok_or_else(invalid)?)?;
    let mut mins = vec![usize::MAX; n];
    let mut maxs = vec![0; n];
    let mut taint = vec![false; n];
    let infeasible = if precise_resources(m) {
        BTreeSet::new()
    } else {
        infeasible_capabilities(m)?
    };
    let mut blocked = vec![false; n];
    if !capacity.zero.is_empty() {
        b.copy(n)?;
    }
    let mut actor_blocked = if capacity.zero.is_empty() {
        Vec::new()
    } else {
        vec![false; n]
    };
    let mut cancellation = BTreeSet::new();
    let mut ps = Vec::new();
    let mut global = BTreeSet::new();
    let mut unknown_results = BTreeSet::new();
    let mut has_unknown = false;
    let mut reached = false;
    for (_, p) in r.sinks {
        // The never-started call and every later call on its path are counted
        // by the static path but never start; fork siblings are included
        // conservatively.
        if let Some(k) = first_infeasible(&p, calls, &infeasible, b)? {
            for id in p.defined.range(k..) {
                b.work()?;
                blocked[calls.get(id.0).ok_or_else(invalid)?.capability.0] = true;
            }
        }
        has_unknown |= uncertain(&p);
        b.copy(p.guards.len())?;
        let mut relaxed = p;
        relaxed.guards.retain(|g| !unknown_guard(g));
        let local = path_support(&relaxed, m, b)?;
        for id in &local {
            b.insert(&mut global, *id)?;
        }
        for id in &relaxed.trace.as_ref().ok_or_else(invalid)?.unknown_results {
            b.insert(&mut unknown_results, *id)?;
        }
        b.copy(1)?;
        ps.push((relaxed, local));
    }
    // A state-dependent requires result is not an independent fixed context.
    // Existentially evaluate it on each branch, including its refusal Domain.
    b.copy(global.len())?;
    let vars: Vec<_> = global.difference(&unknown_results).copied().collect();
    assignments(&vars, &Values::new(), calls, m, b, |values, b| {
        b.copy(n.checked_mul(2).ok_or_else(invalid)?)?;
        let mut lo = vec![usize::MAX; n];
        let mut hi = vec![0; n];
        let mut found = false;
        for (p, local) in &ps {
            b.copy(values.len() + local.len())?;
            let fixed: Values = values
                .iter()
                .filter(|(id, _)| p.defined.contains(id))
                .map(|(id, v)| (*id, *v))
                .collect();
            let local: Vec<_> = local.iter().copied().collect();
            if !assignments(&local, &fixed, calls, m, b, |v, b| satisfied(p, v, b))? {
                continue;
            }
            found = true;
            if let Some(stage) = capacity.first_stage(p, b)? {
                for id in &p.defined {
                    b.work()?;
                    // The zero call itself, but not a feasible fork sibling.
                    if capacity.zero.contains(id) || capacity.stages[id.0] > stage {
                        actor_blocked[calls[id.0].capability.0] = true;
                    }
                }
            }
            b.copy(n)?;
            let mut counts = vec![0usize; n];
            let trace = p.trace.as_ref().ok_or_else(invalid)?;
            for id in &trace.unstarted_zero {
                b.work()?;
                actor_blocked[calls[id.0].capability.0] = true;
            }
            for id in &trace.started {
                b.work()?;
                let cap = calls.get(id.0).ok_or_else(invalid)?.capability.0;
                counts[cap] = counts[cap].checked_add(1).ok_or_else(invalid)?;
            }
            for id in &trace.cancellation {
                b.insert(&mut cancellation, calls[id.0].capability)?;
            }
            for i in 0..n {
                b.work()?;
                lo[i] = lo[i].min(counts[i]);
                hi[i] = hi[i].max(counts[i]);
            }
        }
        if found {
            reached = true;
            for i in 0..n {
                b.work()?;
                mins[i] = mins[i].min(lo[i]);
                maxs[i] = maxs[i].max(hi[i]);
                taint[i] |= has_unknown && lo[i] != hi[i];
            }
        }
        Ok(false)
    })?;
    // No satisfiable sink: the minimum is underived, not zero. Lowered IR
    // always reaches one sink [R]; anything else is invalid static IR and the
    // whole block is not_derived (derivation_limit), as for other invalid IR.
    if !reached {
        return Err(invalid());
    }
    let mut capabilities = Vec::new();
    for (i, _) in m.capabilities.iter().enumerate() {
        b.copy(5)?;
        let capability = cap_name(m, ir::CapabilityId(i))?;
        let contest_route = routes.contains(&ir::CapabilityId(i));
        let join_any_cancellation = cancellation.contains(&ir::CapabilityId(i));
        let actor_blocked = actor_blocked.get(i).copied().unwrap_or(false);
        capabilities.push(if blocked[i] || actor_blocked || taint[i] {
            Exercise::NotDerived {
                capability,
                reason: if blocked[i] {
                    StateReason::ResourceInfeasible
                } else if actor_blocked {
                    StateReason::ActorCapacityInfeasible
                } else {
                    StateReason::StateGuards
                },
                contest_route,
                basis: "unknown".into(),
                fallback: if m.fallback_capabilities.contains(&ir::CapabilityId(i)) {
                    Fallback::KeptByIntent
                } else {
                    Fallback::Unchecked
                },
                join_any_cancellation,
            }
        } else {
            Exercise::Derived {
                capability,
                normal: Count {
                    min: mins[i],
                    max: maxs[i],
                },
                contest_route,
                basis: "unknown".into(),
                fallback: if m.fallback_capabilities.contains(&ir::CapabilityId(i)) {
                    Fallback::KeptByIntent
                } else {
                    Fallback::Unchecked
                },
                join_any_cancellation,
            }
        });
    }
    Ok(Retention::Derived {
        capabilities,
        notice: NOTICE.into(),
        unchecked: retention_unchecked(),
    })
}

// The old censuses stay closed. Unknown agy names fail rather than silently
// falling out through a prefix filter; public acceptance owns this population.
#[cfg(test)]
pub(crate) fn fixture(name: &str) -> bool {
    const NAMES: [&str; 46] = [
        "agy-o-people",
        "agy-o-people.broken-no-read",
        "agy-o-permit",
        "agy-o-permit.broken-one-executor",
        "agy-o-unattributed",
        "agy-o-unattributed.broken-declared-executor",
        "agy-o-contest",
        "agy-o-contest.broken-route-no-read",
        "agy-f-select",
        "agy-f-select.broken-merged-ends",
        "agy-f-or",
        "agy-f-or.broken-and",
        "agy-f-state",
        "agy-f-state.broken-binding-guard",
        "agy-f-singleton",
        "agy-f-singleton.broken-two-results",
        "agy-l-counts",
        "agy-l-counts.broken-unconditional",
        "agy-l-requires",
        "agy-l-requires.broken-no-requires",
        "agy-l-fork",
        "agy-l-fork.broken-missing-second-call",
        "agy-l-any",
        "agy-l-any.broken-sequential",
        "agy-p-name",
        "agy-p-name.broken-proposal-name",
        "agy-label",
        "agy-label.broken-actor-rename",
        "agy-l-state-local",
        "agy-l-state-local.broken-binding-guard",
        "agy-soft-budget",
        "agy-soft-budget.broken-one-unit-short",
        "agy-old-budget",
        "agy-old-budget.broken-legacy-limit",
        "agy-invoice",
        "agy-invoice.broken-ai-transfer",
        "agy-report-compat",
        "agy-report-compat.broken-old-key",
        "agy-f-new-binding",
        "agy-f-new-binding.broken-only-fail",
        "agy-mutate",
        "agy-mutate.broken-off-path-viewpoints",
        "agy-cut-source",
        "agy-cut-source.broken-excluded-caller",
        "agy-no-end-after-end",
        "agy-no-end-after-end.broken-enable-suffix",
    ];
    assert!(
        !name.starts_with("agy-") || NAMES.contains(&name),
        "unclassified {name}"
    );
    NAMES.contains(&name)
}

#[cfg(test)]
#[path = "../../tests/support/agency_viewpoints_budget.rs"]
mod budget_acceptance;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn lower(s: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap()
    }
    fn value(m: &ir::Model) -> Value {
        serde_json::to_value(derive(m, m.agency.as_derived().unwrap()).0).unwrap()
    }
    fn old_four_blocks(mut v: Value) -> Value {
        v.as_object_mut().unwrap().remove("effects");
        v["rules"] = json!("agency-viewpoints-p1-1");
        v
    }

    #[test]
    fn independent_soft_budget_driver_matches_hand_oracle_at_each_exact_boundary() {
        let src = include_str!("../../../../fixtures/agy-soft-budget/model.relca");
        let m = lower(src);
        let report = crate::report::check(src);
        budget_acceptance::verify(|block, dimension, n| {
            let mut limits = [DEFAULT; 3];
            let l = &mut limits[match block {
                "observation" => 0,
                "framing" => 1,
                _ => 2,
            }];
            match dimension {
                "depth" => l.depth = n,
                "paths" => l.paths = n,
                "contexts" => l.contexts = n,
                "cells" => l.cells = n,
                _ => l.work = n,
            }
            let mut r = report.clone();
            r.agency
                .as_mut()
                .unwrap()
                .as_derived_mut()
                .unwrap()
                .viewpoints = Some(with_limits(&m, m.agency.as_derived().unwrap(), limits));
            let mut r = serde_json::to_value(r).unwrap();
            r["agency"]["viewpoints"] = old_four_blocks(r["agency"]["viewpoints"].clone());
            r
        });
    }

    #[test]
    fn counters_overflow_and_failed_reservations_do_not_mutate_or_allocate() {
        for max in [0, usize::MAX] {
            let mut b = budget(Limits {
                work: max,
                cells: max,
                paths: max,
                contexts: max,
                ..DEFAULT
            });
            b.work = max;
            b.cells = max;
            b.paths = max;
            b.contexts = max;
            assert!(b.work().is_err());
            assert_eq!(b.work, max);
            assert!(b.cells(1).is_err());
            assert_eq!(b.cells, max);
            assert!(b.path().is_err());
            assert_eq!(b.paths, max);
            assert!(Budget::add(&mut b.contexts, 1, max, "contexts", &b.location).is_err());
            assert_eq!(b.contexts, max);
            let mut set = BTreeSet::new();
            b.work = 0;
            b.limits.work = usize::MAX;
            assert!(b.insert(&mut set, 1usize).is_err());
            assert!(set.is_empty(), "charge must precede inserting a cell");
            let mut r = Recorder::default();
            b.paths = 0;
            b.limits.paths = usize::MAX;
            assert!(r.sink(None, Path::default(), &mut b).is_err());
            assert!(r.sinks.is_empty(), "charge must precede storing a sink");
        }
        // A refused allocation must not call a visitor or reserve buffers. These
        // deliberately impossible lengths are rejected with checked arithmetic.
        assert!(usize::MAX.checked_mul(3).is_none());
        let m = lower(include_str!(
            "../../../../fixtures/agy-f-select/model.relca"
        ));
        let (calls, _) = recorded(&m, &mut budget(DEFAULT)).unwrap();
        let mut visited = false;
        let mut b = budget(Limits {
            cells: 0,
            ..DEFAULT
        });
        assert!(
            assignments(
                &[ir::BindingId(0)],
                &Values::new(),
                &calls,
                &m,
                &mut b,
                |_, _| {
                    visited = true;
                    Ok(false)
                }
            )
            .is_err()
        );
        assert!(!visited);
        assert_eq!(b.cells, 0);
    }

    #[test]
    fn recorder_none_is_exactly_the_legacy_accounting_mode() {
        let m = lower(include_str!("../../../../fixtures/agy-l-any/model.relca"));
        let mut legacy = budget(DEFAULT);
        let mut effects = BTreeMap::new();
        let tail = paths(
            &m.process.body,
            vec![Path::default()],
            &mut effects,
            &m,
            &mut legacy,
            None,
            0,
        )
        .unwrap();
        assert!(tail.is_empty());
        assert!(effects.is_empty());
        // Existing If/Any/End accounting only: initial if_none is *not* recorded.
        // Two binding insertions (2/2), one guard copy (1/1), three
        // statements (3/0): exactly work=6/cells=3, without any sink work.
        assert_eq!(
            (legacy.work, legacy.cells, legacy.paths, legacy.contexts),
            (6, 3, 0, 0)
        );
        let mut b = budget(DEFAULT);
        let (_, r) = recorded(&m, &mut b).unwrap();
        assert_eq!(r.sinks.len(), 2);
        assert!(b.paths > legacy.paths);
        assert!(b.cells > legacy.cells);
    }

    #[test]
    fn if_none_without_a_lexical_end_is_still_a_terminal_sink_in_public_ir() {
        let mut m = lower(include_str!("../../../../fixtures/agy-l-any/model.relca"));
        // DSL registered returned with an unreachable suffix End. Remove that
        // statement but keep its valid EndId, as a direct IR builder can do.
        assert!(matches!(
            m.process.body.last().unwrap().kind,
            ir::Stmt::End(ir::EndId(1))
        ));
        m.process.body.pop();
        let v = value(&m);
        assert_eq!(
            v["framing"]["ends"][1],
            json!({"end":"returned","status":"derived","reachable":true,
            "selectors":[{"call":"a","capability":"A.draw","selects":"all_contexts"},{"call":"b","capability":"B.draw","selects":"all_contexts"}],"read_only":[]})
        );
    }

    #[test]
    fn empty_state_guard_arms_taint_f_but_equal_exercise_functions_do_not_taint_l() {
        let src = r#"model EmptyArms version "0.2" {
            type V = enum { pass,fail }
            observed case { flag: optional<V> }
            actor Worker { capacity 1; capability work() -> V { duration fixed 1min; result nondet { pass,fail } } }
            process P { if flag == pass {} else {}; z = Worker.work(); end done }
            bounds { cases 1; live_tasks 1; clock 3min }
            verify safety no_resource_overcommit
        }"#;
        let m = lower(src);
        let v = value(&m);
        assert_eq!(
            v["framing"]["ends"],
            json!([{"end":"done","status":"not_derived","reason":"state_guards"}])
        );
        assert_eq!(
            v["retention"]["capabilities"][0]["normal"],
            json!({"min":1,"max":1})
        );
        let equal = src.replace(
            "if flag == pass {} else {}; z = Worker.work()",
            "if flag == pass { x = Worker.work() } else { y = Worker.work() }",
        );
        assert_ne!(equal, src, "the replacement must apply");
        let v = value(&lower(&equal));
        assert_eq!(
            v["retention"]["capabilities"][0]["normal"],
            json!({"min":1,"max":1})
        );
    }

    #[test]
    fn legacy_failure_remains_hard_and_empty_metadata_is_not_forged_evidence() {
        let m = lower(include_str!(
            "../../../../fixtures/agency-origin/model.relca"
        ));
        let error = derive_with_limits(&m, Limits { work: 0, ..DEFAULT }).unwrap_err();
        assert_eq!(error.code, diag::U_SEM_0);
        let mut raw = serde_json::to_value(&m.agency).unwrap();
        raw["viewpoints"]["framing"]["ends"][0]["unknown"] = json!(true);
        assert!(serde_json::from_value::<Agency>(raw).is_err());
        assert!(Agency::empty().viewpoints.is_none());
    }

    #[test]
    fn nonempty_observation_has_independent_exact_cell_work_and_depth_boundaries() {
        let src = include_str!("../../../../fixtures/agy-o-people/model.relca");
        let hand: Value = serde_json::from_str(include_str!(
            "../../../../fixtures/agy-o-people/expected.json"
        ))
        .unwrap();
        let m = lower(src);
        let full = value(&m);
        assert_eq!(old_four_blocks(full.clone()), hand["agency"]["viewpoints"]);
        for dimension in ["cells", "work", "depth"] {
            let run = |n| {
                let mut ls = [DEFAULT; 3];
                match dimension {
                    "cells" => ls[0].cells = n,
                    "work" => ls[0].work = n,
                    _ => ls[0].depth = n,
                }
                serde_json::to_value(with_limits(&m, m.agency.as_derived().unwrap(), ls)).unwrap()
            };
            let mut low = 0;
            let mut high = 1_000_000;
            while low < high {
                let mid = low + (high - low) / 2;
                if run(mid)["observation"]["status"] == "derived" {
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }
            assert!(low > 0);
            assert_eq!(run(low), full);
            let mut want = full.clone();
            want["observation"] =
                json!({"status":"not_derived","reason":"derivation_limit","fallback":"unchecked"});
            assert_eq!(run(low - 1), want);
            assert_eq!(run(0), want);
            eprintln!("ADR0034 nonempty observation/{dimension} k={low}");
        }
    }

    #[test]
    fn derived_tables_are_pure_and_forged_cached_viewpoints_are_not_an_input() {
        let mut m = lower(include_str!(
            "../../../../fixtures/agy-o-people/model.relca"
        ));
        let expected = value(&m);
        let before = m.clone();
        assert_eq!(value(&m), expected);
        assert_eq!(
            m, before,
            "new static derivation must not mutate execution IR/state"
        );
        m.agency.as_derived_mut().unwrap().viewpoints = None;
        assert_eq!(
            value(&m),
            expected,
            "a missing cache is not a missing contract"
        );
        m.agency.as_derived_mut().unwrap().viewpoints = Some(with_limits(
            &m,
            m.agency.as_derived().unwrap(),
            [Limits { work: 0, ..DEFAULT }; 3],
        ));
        assert_eq!(
            value(&m),
            expected,
            "a forged soft failure cannot change derivation"
        );
        assert!(cap_name(&m, ir::CapabilityId(usize::MAX)).is_err());
        m.capabilities[0].actor = ir::ActorId(usize::MAX);
        assert!(cap_name(&m, ir::CapabilityId(0)).is_err());
    }

    #[test]
    fn unknown_requires_propagates_through_a_later_symbolic_requires_domain() {
        let src = r#"model DependentRequires version "0.2" {
            type T = enum { pass,denied }
            observed case { a: optional<T>; b: optional<T> }
            actor First { capacity 1; capability work() -> T {
                duration fixed 1min; result nondet { pass }; writes b;
                requires a == pass else denied
            } }
            actor Second { capacity 1; capability work() -> T {
                duration fixed 1min; result nondet { pass };
                requires b == pass else denied
            } }
            process P { p = First.work(); q = Second.work(); end done }
            bounds { cases 1; live_tasks 2; clock 3min }
            verify safety no_resource_overcommit
        }"#;
        let m = lower(src);
        let v = value(&m);
        for row in v["retention"]["capabilities"].as_array().unwrap() {
            assert_eq!(row["status"], "not_derived");
            assert_eq!(row["reason"], "state_guards");
            assert!(row.get("normal").is_none());
        }
    }

    #[test]
    fn state_controlled_constant_writer_must_not_be_fixed_by_the_requires_domain() {
        let src = r#"model ConditionalWriter version "0.2" {
            type T = enum { pass,fail,posted,denied }
            observed case { flag: optional<T>; approval: optional<T> }
            actor Yes { capacity 1; capability write() -> T { duration fixed 1min; result nondet { pass }; writes approval; finish { approval = pass } } }
            actor No { capacity 1; capability write() -> T { duration fixed 1min; result nondet { fail }; writes approval; finish { approval = fail } } }
            actor Post { capacity 1; capability post() -> T { duration fixed 1min; result nondet { posted }; requires approval == pass else denied } }
            process P { if flag == pass { y = Yes.write() } else { n = No.write() }; p = Post.post(); end done }
            bounds { cases 1; live_tasks 2; clock 3min }
            verify safety no_resource_overcommit
        }"#;
        let v = value(&lower(src));
        assert_eq!(v["retention"]["capabilities"][2]["status"], "not_derived");
        assert_eq!(v["retention"]["capabilities"][2]["reason"], "state_guards");
        // If both unknown arms install the same writer value, Post's count
        // function is constant, so we can still derive 1..1 rather than taint all.
        let same = src.replace("approval = fail", "approval = pass");
        assert_ne!(same, src, "the replacement must apply");
        let v = value(&lower(&same));
        assert_eq!(
            v["retention"]["capabilities"][2]["normal"],
            json!({"min":1,"max":1})
        );
    }

    #[test]
    fn no_satisfiable_sink_is_not_derived_rather_than_zero_exercises() {
        let m = lower(include_str!(
            "../../../../fixtures/agy-l-counts/model.relca"
        ));
        let (calls, _) = recorded(&m, &mut budget(DEFAULT)).unwrap();
        let sink = |guards: Vec<Guard<'static>>| Recorder {
            sinks: vec![(
                None,
                Path {
                    guards,
                    trace: Some(Trace::default()),
                    ..Path::default()
                },
            )],
        };
        // Control: one satisfiable sink that starts nothing is a derived zero.
        let zero = exercised(&m, &calls, sink(vec![]), &mut budget(DEFAULT)).unwrap();
        let Retention::Derived { capabilities, .. } = zero else {
            panic!("satisfiable sink must derive");
        };
        assert!(!capabilities.is_empty());
        for row in &capabilities {
            assert!(
                matches!(
                    row,
                    Exercise::Derived {
                        normal: Count { min: 0, max: 0 },
                        ..
                    }
                ),
                "{row:?}"
            );
        }
        // An empty `join any` match set is never satisfied: there is no sink to
        // count from, so the minimum is underived, not 0.
        assert!(
            exercised(
                &m,
                &calls,
                sink(vec![Guard::Any(&[])]),
                &mut budget(DEFAULT)
            )
            .is_err(),
            "no satisfiable sink must not be reported as zero exercises"
        );
    }
}
