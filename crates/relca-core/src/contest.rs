//! Static contest structure, not an occurrence, response or remedy monitor.
use crate::{Verdict, diag, ir, responsibility as resp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "../tests/support/effect_standing_population.rs"]
pub(crate) mod population;

#[cfg(test)]
#[path = "../tests/support/q9_inputs.rs"]
pub(crate) mod q9_input;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContestTarget {
    End(ir::EndId),
    Commit(ir::CapabilityId),
}
impl ContestTarget {
    pub fn end(self) -> Option<ir::EndId> {
        if let Self::End(e) = self {
            Some(e)
        } else {
            None
        }
    }
    pub fn active(self, m: &ir::Model) -> bool {
        match self {
            Self::End(e) => control_dependencies(m).terminals.contains_key(&e),
            Self::Commit(c) => resp::calls(m).values().any(|call| call.capability == c),
        }
    }
    pub fn valid(self, m: &ir::Model) -> bool {
        match self {
            Self::End(e) => e.0 < m.ends.len(),
            Self::Commit(c) => m.capabilities.get(c.0).is_some_and(|c| c.commits),
        }
    }
    pub fn name(self, m: &ir::Model) -> String {
        match self {
            Self::End(e) => m.ends[e.0].clone(),
            Self::Commit(c) => format!("{}.commit", resp::cap_name(m, c)),
        }
    }
    pub fn effect(self) -> bool {
        matches!(self, Self::Commit(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contest {
    pub name: Option<String>,
    pub ordinal: usize,
    pub party: ir::ActorId,
    pub target: ContestTarget,
    pub route: ir::CapabilityId,
    pub reversible_by: Vec<ir::PersonId>,
    pub respond_by: ir::Tick,
    pub rates: Option<Rates>,
    pub span: diag::Span,
}

/// Fixed conditional probability and evidence declarations (ADR 0063).
/// Evidence is recorded verbatim, never opened or certified as measured data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rates {
    pub probability: crate::probability::Rational,
    pub occurrence_basis: String,
    pub service_time_basis: String,
    pub independence: String,
}

pub(crate) fn rates_declarations_valid(model: &ir::Model) -> bool {
    model.contests().iter().all(|c| {
        c.rates.as_ref().is_none_or(|r| {
            r.probability.valid()
                && !r.probability.is_zero()
                && !r.occurrence_basis.trim().is_empty()
                && !r.service_time_basis.trim().is_empty()
                && r.independence == "per_case_per_clause"
                && matches!(c.target, ContestTarget::End(e) if e.0 < model.ends.len())
                && model.capabilities.get(c.route.0).is_some()
                && model
                    .process
                    .bindings
                    .len()
                    .checked_add(model.contests().len())
                    .is_some()
        })
    })
}

impl Contest {
    pub fn property_id(&self) -> String {
        format!(
            "contract{} #{} contest",
            self.name
                .as_ref()
                .map(|n| format!(" {n}"))
                .unwrap_or_default(),
            self.ordinal
        )
    }
}

/// Possible leaf outcomes, never frequencies or an occurrence probability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub id: String,
    pub static_id: String,
    pub case_id: u64,
    pub party: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_kind: Option<String>,
    pub route: String,
    pub respond_by: u64,
    pub basis: String,
    pub coverage: crate::obligation::Coverage,
    pub alternatives: Vec<Alternative>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alternative {
    pub filed: bool,
    pub target_at: u64,
    pub filed_at: Option<u64>,
    pub deadline: Option<u64>,
    pub status: RuntimeStatus,
    pub response_at: Option<u64>,
    pub closed_at: Option<u64>,
    pub executor: Option<String>,
    pub result: Option<String>,
    pub violation_reason: Option<String>,
    pub route_work: Option<IntegerRange>,
    pub route_wait: Option<IntegerRange>,
    pub route_cost: crate::metrics::Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStatus {
    NotFiled,
    Responded,
    Violated,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegerRange {
    pub min: u64,
    pub max: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub business_ended_open: crate::metrics::Range,
}

pub(crate) fn canonical_alternatives(
    mut alternatives: Vec<Alternative>,
    model: &ir::Model,
    id: usize,
) -> Vec<Alternative> {
    let cap = &model.capabilities[model.contests()[id].route.0];
    alternatives.sort_by_cached_key(|a| {
        (
            a.filed,
            a.target_at,
            a.response_at,
            a.executor
                .as_ref()
                .and_then(|name| model.people.iter().position(|p| &p.name == name)),
            a.result.as_ref().and_then(|name| {
                cap.results
                    .iter()
                    .position(|v| &model.types[cap.return_type.0].members[v.0] == name)
            }),
            serde_json::to_string(a).expect("contest DTO serializes"),
        )
    });
    alternatives.dedup();
    alternatives
}

pub(crate) fn runtime_declarations_valid(model: &ir::Model) -> bool {
    rates_declarations_valid(model)
        && model.contests().iter().all(|d| {
            d.party.0 < model.actors.len()
                && d.target.valid(model)
                && model.capabilities.get(d.route.0).is_some_and(|c| {
                    c.actor.0 < model.actors.len() && c.return_type.0 < model.types.len()
                })
        })
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContestPath {
    pub id: String,
    pub rules: String,
    pub party: String,
    pub end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub effect: Option<String>,
    pub route: String,
    pub reversible_by: Vec<String>,
    pub respond_by: ir::Tick,
    pub checks: Checks,
    pub verdict: Verdict,
    pub basis: String,
    pub missing: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checks {
    pub route_exists: Check,
    pub reversal_authority: Reversal,
    pub deadline_defined: Check,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub satisfied: bool,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reversal {
    pub satisfied: bool,
    pub quantifier: String,
    pub people: Vec<Person>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    pub person: String,
    pub satisfied: bool,
    pub decisions: Vec<Decision>,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub capability: String,
    pub executor: bool,
    pub included: bool,
    pub permission: bool,
    pub authorities: Vec<String>,
}

// Lexical control dependence includes guarded continuations after an early End.
// Surviving paths join by union: this is a candidate envelope, not reachability.
#[derive(Default)]
pub(crate) struct ControlDependencies {
    pub terminals: BTreeMap<ir::EndId, BTreeSet<ir::CapabilityId>>,
    pub calls: BTreeMap<ir::BindingId, BTreeSet<ir::CapabilityId>>,
    /// Unmodeled condition releases, kept apart from certified If guards.
    pub condition_calls: BTreeMap<ir::BindingId, BTreeSet<ir::CapabilityId>>,
}
// Conservative reservation for call/terminal sets and nested working copies.
// The normal process has already passed Agency's depth/work validation.
pub(crate) fn control_cells(m: &ir::Model) -> Option<usize> {
    m.process
        .bindings
        .len()
        .checked_add(m.ends.len())?
        .checked_add(129)?
        .checked_mul(m.process.bindings.len().checked_add(1)?)
}

// The additional walk must not consume the existing If-control allowance.
// Reserve it independently, with an explicit finite implementation bound.
const CONDITION_CONTROL_LIMIT: usize = 1_000_000;
pub(crate) fn condition_control_cells(m: &ir::Model) -> Option<usize> {
    if m.has_condition_wait() {
        control_cells(m)
    } else {
        Some(0)
    }
}
pub(crate) fn control_within_limits(m: &ir::Model, cell_limit: usize) -> bool {
    control_cells(m).is_some_and(|n| n <= cell_limit)
        && condition_control_cells(m).is_some_and(|n| n <= CONDITION_CONTROL_LIMIT)
}
pub(crate) fn control_dependencies(m: &ir::Model) -> ControlDependencies {
    fn may_end(b: &ir::Block) -> bool {
        b.iter().any(|s| match &s.kind {
            ir::Stmt::Response(_, r) => {
                r.exhausted.is_some() || r.recovery.as_ref().is_some_and(may_end)
            }
            ir::Stmt::End(_)
            | ir::Stmt::Fork {
                join: ir::Join::Any { .. },
                ..
            } => true,
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => may_end(then_body) || may_end(else_body),
            _ => false,
        })
    }
    fn walk(
        m: &ir::Model,
        cs: &BTreeMap<ir::BindingId, &ir::Call>,
        b: &ir::Block,
        mut guards: BTreeSet<ir::CapabilityId>,
        out: &mut ControlDependencies,
    ) -> Option<BTreeSet<ir::CapabilityId>> {
        let mut alive = true;
        for s in b {
            match &s.kind {
                ir::Stmt::LoopControl(_) => {}
                ir::Stmt::Wait(_) | ir::Stmt::WaitUntil(_) => {} // Condition releases are tracked separately below.
                ir::Stmt::ResolveEffect { evidence, .. } => {
                    if let Some(c) = cs.get(evidence) {
                        guards.insert(c.capability);
                    }
                }
                ir::Stmt::StopReceiptUnknown => alive = false,
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    let mut condition_caps = BTreeSet::new();
                    resp::decision_support(condition, m, cs, &mut condition_caps);
                    let mut next = guards.clone();
                    next.extend(condition_caps.iter().copied());
                    let yes = walk(m, cs, then_body, next.clone(), out);
                    let no = walk(m, cs, else_body, next, out);
                    match (yes, no) {
                        // A branch that may end early makes the condition decide the continuation.
                        (Some(y), Some(n)) if may_end(then_body) || may_end(else_body) => {
                            guards.extend(y.union(&n).copied());
                        }
                        (Some(y), Some(n)) => {
                            guards.extend(
                                y.intersection(&n)
                                    .filter(|c| !condition_caps.contains(c))
                                    .copied(),
                            );
                        }
                        (Some(y), None) | (None, Some(y)) => {
                            guards = y;
                        }
                        (None, None) => {
                            alive = false;
                        }
                    }
                }
                ir::Stmt::Fork {
                    branches,
                    join: ir::Join::Any { if_none, .. },
                    ..
                } => {
                    for c in branches {
                        out.calls.insert(c.binding, guards.clone());
                    }
                    let mut next = guards.clone();
                    next.extend(branches.iter().map(|c| c.capability));
                    out.terminals.entry(*if_none).or_default().extend(next);
                    guards.extend(branches.iter().map(|c| c.capability));
                }
                ir::Stmt::Fork { branches, .. } => {
                    for c in branches {
                        out.calls.insert(c.binding, guards.clone());
                    }
                }
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => {
                    out.calls.insert(c.binding, guards.clone());
                }
                ir::Stmt::Response(c, r) => {
                    out.calls.insert(c.binding, guards.clone());
                    if let Some(body) = &r.recovery {
                        let mut recovery_guards = guards.clone();
                        recovery_guards.insert(c.capability);
                        walk(m, cs, body, recovery_guards, out);
                    }
                }
                ir::Stmt::End(e) => {
                    out.terminals
                        .entry(*e)
                        .or_default()
                        .extend(guards.iter().copied());
                    alive = false; /* suffix remains lexical */
                }
            }
        }
        alive.then_some(guards)
    }
    #[derive(Clone, Default)]
    struct ConditionGuards {
        waiting: bool,
        capabilities: BTreeSet<ir::CapabilityId>,
    }
    fn condition_walk(
        m: &ir::Model,
        cs: &BTreeMap<ir::BindingId, &ir::Call>,
        body: &ir::Block,
        mut guards: ConditionGuards,
        out: &mut ControlDependencies,
    ) -> Option<ConditionGuards> {
        for statement in body {
            match &statement.kind {
                ir::Stmt::LoopControl(_) => {}
                ir::Stmt::WaitUntil(condition) => {
                    guards.waiting = true;
                    resp::decision_support(condition, m, cs, &mut guards.capabilities);
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    let yes = condition_walk(m, cs, then_body, guards.clone(), out);
                    let no = condition_walk(m, cs, else_body, guards, out);
                    guards = match (yes, no) {
                        (Some(mut y), Some(n)) => {
                            y.waiting |= n.waiting;
                            y.capabilities.extend(n.capabilities);
                            y
                        }
                        (Some(g), None) | (None, Some(g)) => g,
                        (None, None) => return None,
                    };
                }
                ir::Stmt::Call(call) | ir::Stmt::Query(call, _) => {
                    if guards.waiting {
                        out.condition_calls
                            .entry(call.binding)
                            .or_default()
                            .extend(&guards.capabilities);
                    }
                }
                ir::Stmt::Response(call, r) => {
                    if guards.waiting {
                        out.condition_calls
                            .entry(call.binding)
                            .or_default()
                            .extend(&guards.capabilities);
                    }
                    if let Some(body) = &r.recovery {
                        condition_walk(m, cs, body, guards.clone(), out);
                    }
                }
                ir::Stmt::Fork { branches, .. } => {
                    if guards.waiting {
                        for call in branches {
                            out.condition_calls
                                .entry(call.binding)
                                .or_default()
                                .extend(&guards.capabilities);
                        }
                    }
                }
                ir::Stmt::End(_) | ir::Stmt::StopReceiptUnknown => return None,
                ir::Stmt::Wait(_) | ir::Stmt::ResolveEffect { .. } => (),
            }
        }
        Some(guards)
    }
    let mut out = ControlDependencies::default();
    let cs = resp::calls(m);
    walk(m, &cs, &m.process.body, BTreeSet::new(), &mut out);
    if m.has_condition_wait() {
        condition_walk(
            m,
            &cs,
            &m.process.body,
            ConditionGuards::default(),
            &mut out,
        );
    }
    out
}
#[cfg(test)]
#[test]
fn round1_handler_end_has_query_and_decision_meaning() {
    let m = crate::lower::lower(&crate::syntax::parse(&crate::r2_inputs::source("R01")).unwrap())
        .unwrap();
    let deps = control_dependencies(&m);
    let end = ir::EndId(m.ends.iter().position(|s| s == "shipped").unwrap());
    let names: BTreeSet<_> = deps.terminals[&end]
        .iter()
        .map(|id| m.capabilities[id.0].name.as_str())
        .collect();
    assert!(names.contains("query_status"), "{names:?}");
    assert!(names.contains("decide"), "{names:?}");
}
pub(crate) fn derive(m: &ir::Model, c: &Contest) -> ContestPath {
    let all = control_dependencies(m);
    let candidates = candidates(m, c.target, &all);
    let mut people = Vec::new();
    for p in &c.reversible_by {
        let mut decisions = Vec::new();
        for id in &candidates {
            let cap = &m.capabilities[id.0];
            let a = &m.actors[cap.actor.0];
            let executor = a.executors.as_ref().is_some_and(|ps| ps.contains(p));
            let permission = cap
                .permission
                .as_ref()
                .or(a.executors.as_ref())
                .is_some_and(|ps| ps.contains(p));
            let authorities = m
                .authorities
                .iter()
                .filter(|a| {
                    a.grants
                        .iter()
                        .any(|g| g.capability == *id && g.persons.contains(p))
                })
                .map(|a| a.name.clone())
                .collect();
            decisions.push(Decision {
                capability: resp::cap_name(m, *id),
                executor,
                included: resp::included(m, cap.actor),
                permission,
                authorities,
            });
        }
        let satisfied = decisions
            .iter()
            .any(|d| d.executor && d.included && d.permission && !d.authorities.is_empty());
        let reason = if satisfied {
            None
        } else {
            Some(
                if decisions.is_empty() {
                    "no_decision"
                } else if !decisions.iter().any(|d| d.executor) {
                    "executor_not_connected"
                } else if !decisions.iter().any(|d| d.executor && d.included) {
                    "decision_excluded"
                } else if !decisions
                    .iter()
                    .any(|d| d.executor && d.included && d.permission)
                {
                    "permission_missing"
                } else {
                    "Permission あり、Authority 未接続"
                }
                .into(),
            )
        };
        people.push(Person {
            person: m.people[p.0].name.clone(),
            satisfied,
            decisions,
            reason,
        });
    }
    let route = resp::included(m, m.capabilities[c.route.0].actor);
    let reversal = people.iter().all(|p| p.satisfied);
    let deadline = c.respond_by > 0;
    let missing = [
        ("route_exists", route),
        ("reversal_authority", reversal),
        ("deadline_defined", deadline),
    ]
    .into_iter()
    .filter(|(_, b)| !*b)
    .map(|(s, _)| s.into())
    .collect::<Vec<_>>();
    ContestPath {
        id: c.property_id(),
        rules: if c.target.effect() {
            "contest-p1-2"
        } else {
            "contest-p1-1"
        }
        .into(),
        party: m.actors[c.party.0].name.clone(),
        end: (!c.target.effect()).then(|| c.target.name(m)),
        effect: c.target.effect().then(|| c.target.name(m)),
        route: resp::cap_name(m, c.route),
        reversible_by: c
            .reversible_by
            .iter()
            .map(|p| m.people[p.0].name.clone())
            .collect(),
        respond_by: c.respond_by,
        checks: Checks {
            route_exists: Check {
                satisfied: route,
                reason: (!route).then(|| "route_excluded".into()),
            },
            reversal_authority: Reversal {
                satisfied: reversal,
                quantifier: "exists_decision_per_person".into(),
                people,
            },
            deadline_defined: Check {
                satisfied: deadline,
                reason: (!deadline).then(|| "nonpositive_deadline".into()),
            },
        },
        verdict: if missing.is_empty() {
            Verdict::Satisfied
        } else {
            Verdict::Violated
        },
        basis: if missing.is_empty() {
            "構造上存在"
        } else {
            "構造の不足"
        }
        .into(),
        missing,
    }
}

pub(crate) fn candidates(
    m: &ir::Model,
    target: ContestTarget,
    all: &ControlDependencies,
) -> BTreeSet<ir::CapabilityId> {
    match target {
        ContestTarget::End(e) => all.terminals.get(&e).cloned().unwrap_or_default(),
        ContestTarget::Commit(cap) => {
            let mut result = BTreeSet::new();
            let calls = resp::calls(m);
            let mut found = false;
            for call in calls.values().filter(|c| c.capability == cap) {
                found = true;
                result.extend(all.calls.get(&call.binding).into_iter().flatten().copied());
            }
            // results is the lowered, positive-support result set, not the type domain.
            // A Capability whose result is decided by the shared state history
            // (atomic, ADR 0035, or version_check, ADR 0041 M1) decides whether
            // it commits (ADR 0040 U2), even with one declared result value.
            if found
                && (m.capabilities[cap.0].history_dependent_result()
                    || m.capabilities[cap.0]
                        .results
                        .iter()
                        .collect::<BTreeSet<_>>()
                        .len()
                        >= 2)
            {
                result.insert(cap);
            }
            result
        }
    }
}

pub(crate) fn effect_sites_valid(m: &ir::Model) -> bool {
    let Some(meta) = &m.responsibility else {
        return true;
    };
    let calls = resp::calls(m);
    meta.standings.iter().all(|s| match s.target {
        ContestTarget::End(_) => true,
        ContestTarget::Commit(cap) => {
            s.target.valid(m)
                && calls.values().filter(|c| c.capability == cap).count() <= 1
                && !meta.contests.iter().any(|c| c.route == cap)
        }
    })
}

pub(crate) fn has_effects(m: &ir::Model) -> bool {
    m.contests().iter().any(|c| c.target.effect())
}

pub(crate) const CONDITION_WAIT_UNSUPPORTED: &str = "condition_wait_contest_unsupported";
/// No rule certifies a condition release as an If decision. Refuse the
/// combination before either static candidates or restricted grants are read.
pub(crate) fn condition_wait_refusal(m: &ir::Model) -> Option<diag::Diagnostic> {
    let contest = m.contests().first()?;
    m.has_condition_wait().then(|| diag::Diagnostic {
        code: diag::U_SEM_0.into(),
        severity: diag::Severity::Error,
        message: CONDITION_WAIT_UNSUPPORTED.into(),
        span: Some(contest.span),
    })
}

/// Same decision candidates and grant enumeration as derive, before qualification
/// by execution/Permission/Cut. An unrestricted alternative cannot mask a record
/// whose scope/expiry meaning on this path has not been specified.
pub(crate) fn restricted_authority_reference(m: &ir::Model) -> Option<(crate::diag::Span, String)> {
    if m.contests().is_empty() {
        return None;
    }
    let control = control_dependencies(m);
    for c in m.contests() {
        for id in &candidates(m, c.target, &control) {
            for p in &c.reversible_by {
                for a in &m.authorities {
                    if a.restrictions.restricted()
                        && a.grants
                            .iter()
                            .any(|g| g.capability == *id && g.persons.contains(p))
                    {
                        return Some((c.span, a.name.clone()));
                    }
                }
            }
        }
    }
    None
}
pub(crate) fn missing_warnings(m: &ir::Model, meta: &resp::Metadata) -> Vec<diag::Diagnostic> {
    fn ends(b: &ir::Block, out: &mut BTreeSet<ir::EndId>) {
        for s in b {
            match &s.kind {
                ir::Stmt::Response(_, r) => {
                    if let Some(end) = r.exhausted {
                        out.insert(end);
                    }
                }
                ir::Stmt::End(e) => {
                    out.insert(*e);
                }
                ir::Stmt::Fork {
                    join: ir::Join::Any { if_none, .. },
                    ..
                } => {
                    out.insert(*if_none);
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    ends(then_body, out);
                    ends(else_body, out);
                }
                _ => {}
            }
        }
    }
    let mut terminals = BTreeSet::new();
    ends(&m.process.body, &mut terminals);
    let mut warnings = Vec::new();
    for s in &meta.standings {
        if !match s.target {
            ContestTarget::End(e) => terminals.contains(&e),
            ContestTarget::Commit(c) => resp::calls(m).values().any(|call| call.capability == c),
        } || meta
            .contests
            .iter()
            .any(|c| c.party == s.actor && c.target == s.target)
        {
            continue;
        }
        let party = &m.actors[s.actor.0].name;
        let all = format!("{party}.contest");
        let exact = format!("{all}.{}", s.target.name(m));
        let explicit = m.cut.as_ref().is_some_and(|c| {
            // Only exclude names the whole party; defer counts for the contest labels alone.
            c.exclude
                .iter()
                .any(|e| matches!(&e.target, ir::CutTarget::Actor(a) if *a == s.actor))
                || c.exclude.iter().chain(&c.defer).any(
                    |e| matches!(&e.target, ir::CutTarget::Label(v) if v == &all || v == &exact),
                )
        });
        if !explicit {
            warnings.push(resp::warning(
                diag::W_CONTEST_0,
                format!(
                    "{party}/{} has no contest route or explicit Cut",
                    s.target.name(m)
                ),
                s.span,
            ));
        }
    }
    warnings
}
pub(crate) fn property_id(m: &ir::Model, id: usize) -> String {
    m.responsibility.as_ref().unwrap().contests[id].property_id()
}
pub(crate) fn verdict(m: &ir::Model, id: usize) -> Verdict {
    // An abort after failed validation must not report a forged static verdict.
    if crate::responsibility::validate(m).is_err() {
        return Verdict::Unsupported;
    }
    if restricted_authority_reference(m).is_some() {
        return Verdict::Unsupported;
    }
    m.responsibility
        .as_ref()
        .and_then(|m| m.contest_paths.get(id))
        .map_or(Verdict::Unsupported, |r| r.verdict)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q9_model(name: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(&q9_input::source(name)).unwrap()).unwrap()
    }

    #[test]
    fn q9_lower_preserves_evidence_without_certifying_it() {
        for (name, probability) in [("c", "1/4"), ("tiny", "1/1000000"), ("fixed", "1")] {
            let m = q9_model(name);
            let rates = m.contests()[0].rates.as_ref().unwrap();
            assert_eq!(rates.probability.to_string(), probability);
            assert_eq!(rates.occurrence_basis, q9_input::OCCURRENCE);
            assert_eq!(rates.service_time_basis, q9_input::SERVICE);
            assert_eq!(rates.independence, "per_case_per_clause");
            for (suffix, value) in [
                ("occurrence_basis", q9_input::OCCURRENCE),
                ("service_time_basis", q9_input::SERVICE),
                ("independence", "per_case_per_clause"),
                ("validation", "analyst-declared-not-validated"),
                ("probability", probability),
                ("target", "end.returned"),
                ("service_time", "Reviewer.reply.duration"),
            ] {
                let field = format!("contest.1.{suffix}");
                let rows: Vec<_> = m.assumptions.iter().filter(|a| a.field == field).collect();
                assert_eq!(rows.len(), 1, "{field}");
                assert_eq!(rows[0].value, value, "{field}");
                assert!(!rows[0].affects.is_empty());
            }
        }
        let legacy = q9_model("legacy");
        assert!(legacy.contests()[0].rates.is_none());
        assert!(
            !legacy
                .assumptions
                .iter()
                .any(|a| a.field.starts_with("contest.1."))
        );
    }

    #[test]
    fn q9_public_ir_rates_are_validated_independently_of_the_stage_gate() {
        let base = q9_model("c");
        assert!(rates_declarations_valid(&base));
        let mut rederived = base.clone();
        crate::responsibility::derive(&mut rederived).expect("valid declaration positive control");
        for defect in 0..9 {
            let mut m = base.clone();
            let c = &mut m.responsibility.as_mut().unwrap().contests[0];
            let rates = c.rates.as_mut().unwrap();
            match defect {
                0 => rates.probability = crate::probability::Rational::zero(),
                1 => rates.probability = crate::probability::Rational::decimal("1.01").unwrap(),
                2 => rates.probability = crate::probability::Rational::decimal("-0.1").unwrap(),
                3 => rates.occurrence_basis = " \n ".into(),
                4 => rates.service_time_basis.clear(),
                5 => rates.independence = "actor_name".into(),
                6 => c.target = ContestTarget::End(ir::EndId(usize::MAX)),
                7 => c.target = ContestTarget::Commit(ir::CapabilityId(0)),
                8 => c.route = ir::CapabilityId(usize::MAX),
                _ => unreachable!(),
            }
            assert!(!rates_declarations_valid(&m), "defect {defect}");
            assert!(
                crate::responsibility::derive(&mut m).is_err(),
                "defect {defect}"
            );
            let reference = crate::reference::explore(&m);
            assert_eq!(reference.stop, crate::reference::StopReason::SemanticAbort);
            for out in [reference.outcome, crate::engine::explore(&m)] {
                assert!(out.terminals.is_empty());
                assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
            }
        }
    }

    #[test]
    fn q9_i10_public_modes_preserve_source_evidence_and_reject_forged_uncertainty() {
        use crate::uncertainty::{Uncertainty, wire::Evidence};
        let source = q9_input::source("fixed");
        let checked = crate::report::check(&source);
        let simulated = crate::report::simulate(&source);
        assert_eq!(checked.verdict, crate::Verdict::Satisfied);
        assert_eq!(simulated.verdict, crate::Verdict::Inconclusive);
        let table = simulated
            .metrics
            .as_ref()
            .unwrap()
            .contests
            .as_ref()
            .unwrap();
        assert_eq!(
            table.rows[0]
                .filing_fraction
                .value
                .as_ref()
                .unwrap()
                .to_string(),
            "1"
        );
        assert_eq!(
            table.rows[0]
                .response_elapsed
                .mean
                .as_ref()
                .unwrap()
                .0
                .to_string(),
            "2"
        );
        assert!(
            serde_json::to_string(&simulated.uncertainty.model)
                .unwrap()
                .contains("analyst-declared-not-validated")
        );
        assert!(simulated.uncertainty.aleatory.items.iter().any(|x| matches!(x,
            crate::uncertainty::wire::Item::Evidence(Evidence::ContestOccurrence { probability, .. })
                if probability == &crate::probability::Rational::one())));
        let mut forged = serde_json::to_value(simulated).unwrap();
        forged["uncertainty"]["model"]["items"][0]["kind"] = serde_json::json!("invented_kind");
        assert!(serde_json::from_value::<Uncertainty>(forged["uncertainty"].clone()).is_err());
        let without_metrics = crate::report::simulate(&q9_input::source("no-metrics"));
        assert!(without_metrics.metrics.is_none());
        assert_eq!(without_metrics.verdict, crate::Verdict::Inconclusive);
        let legacy = crate::report::simulate(&q9_input::source("legacy-fixed"));
        assert_eq!(legacy.verdict, crate::Verdict::Unsupported);
        assert!(legacy.metrics.as_ref().unwrap().contests.is_none());
    }

    #[test]
    fn q4_condition_walk_has_its_own_explicit_cell_limit() {
        let base = crate::lower::lower(&crate::syntax::parse(contest_source()).unwrap()).unwrap();
        for (bindings, within) in [(936, true), (937, false)] {
            let mut m = base.clone();
            m.process
                .bindings
                .resize(bindings, m.process.bindings[0].clone());
            assert_eq!(condition_control_cells(&m), Some(0));
            assert!(control_within_limits(&m, usize::MAX));
            m.process.body.push(ir::Statement {
                kind: ir::Stmt::WaitUntil(ir::Cond::Eq(ir::ResultMatch {
                    binding: ir::BindingId(1),
                    value: ir::ValueId(0),
                })),
                span: m.process.body[1].span,
            });
            assert_eq!(control_cells(&m), condition_control_cells(&m));
            assert_eq!(control_within_limits(&m, usize::MAX), within);
        }
    }

    #[test]
    fn d2_review_canonicalization_does_not_decide_cut_semantics() {
        let m = crate::lower::lower(&crate::syntax::parse(contest_source()).unwrap()).unwrap();
        let a = Alternative {
            filed: true,
            target_at: 1,
            filed_at: Some(1),
            deadline: Some(3),
            status: RuntimeStatus::Unresolved,
            response_at: None,
            closed_at: None,
            executor: None,
            result: None,
            violation_reason: None,
            route_work: Some(IntegerRange { min: 1, max: 1 }),
            route_wait: Some(IntegerRange { min: 0, max: 0 }),
            route_cost: crate::metrics::Range {
                min: Some(2.into()),
                max: Some(2.into()),
                reason: None,
            },
        };
        assert_eq!(
            canonical_alternatives(vec![a.clone(), a.clone()], &m, 0),
            vec![a]
        );
    }
    fn contest_source() -> &'static str {
        include_str!("../../../fixtures/resp-contest/model.relca")
    }
    fn on_posted(flow: &str) -> String {
        contest_source()
            .replace("standing over [end.returned]", "standing over [end.posted]")
            .replace(
                "contest Vendor on end.returned",
                "contest Vendor on end.posted",
            )
            .replace(
                "    if approval == fail { end returned } else { end posted }",
                flow,
            )
    }
    #[test]
    fn r_nested_early_end_keeps_the_deciding_capability() {
        // posted is reached only when approval != fail, in both spellings.
        for flow in [
            "    if approval == fail { end returned } else { }; end posted",
            "    if approval == fail { if approval == fail { end returned } else { } } else { }; end posted",
        ] {
            let r = crate::report::check(&on_posted(flow));
            assert_eq!(r.verdict, Verdict::Satisfied, "{flow}: {:?}", r.diagnostics);
            let row = &r.contest_paths.as_ref().unwrap()[0];
            assert_eq!(
                row.checks.reversal_authority.people[0].decisions[0].capability,
                "Approver.approve"
            );
        }
    }
    #[test]
    fn r_defer_of_the_whole_party_does_not_count_as_explicit() {
        let src = include_str!("../../../fixtures/resp-cut.broken-no-explicit-cut/model.relca");
        let warned = |s: &str| {
            crate::report::check(s)
                .diagnostics
                .iter()
                .any(|d| d.code == diag::W_CONTEST_0)
        };
        assert!(warned(src));
        let cut = "    include [Invoice,Reader,Clerk,Approver,Vendor]\n";
        assert!(warned(&src.replacen(
            cut,
            &format!("{cut}    defer Vendor reason \"later\" loses \"contest\"\n"),
            1
        )));
        assert!(!warned(&src.replacen(
            cut,
            &format!("{cut}    defer Vendor.contest reason \"later\" loses \"contest\"\n"),
            1
        )));
    }
    #[test]
    fn r_runtime_abort_keeps_the_static_contest_row() {
        let src = contest_source().replace(
            "    contest Vendor on",
            "    response Approver.approve.finish -> one_of(end.returned) by 18446744073709551615min after Approver.approve.finish otherwise unresolved\n    contest Vendor on",
        );
        let r = crate::report::check(&src);
        assert_eq!(r.verdict, Verdict::Unsupported);
        let row = r
            .properties
            .iter()
            .find(|p| p.id.ends_with("contest"))
            .unwrap();
        assert_eq!(row.verdict, Verdict::Satisfied);
    }
    #[test]
    fn r_forged_static_verdict_is_not_reported() {
        let ast = crate::syntax::parse(include_str!(
            "../../../fixtures/resp-contest.broken-no-authority/model.relca"
        ))
        .unwrap();
        let mut m = crate::lower::lower(&ast).unwrap();
        assert_eq!(verdict(&m, 0), Verdict::Violated);
        m.responsibility.as_mut().unwrap().contest_paths[0].verdict = Verdict::Satisfied;
        assert_eq!(verdict(&m, 0), Verdict::Unsupported);
        let out = crate::engine::explore(&m);
        assert!(
            out.properties
                .iter()
                .all(|p| p.verdict != Verdict::Satisfied
                    || !matches!(p.property, ir::Property::Contest(_)))
        );
    }
    #[test]
    fn r_selected_decisions_do_not_borrow_another_process_grant() {
        let ast = crate::syntax::parse(include_str!(
            "../../../fixtures/resp-diff.broken-no-authorized-decision/model.relca"
        ))
        .unwrap();
        let a = crate::lower::lower_configuration(&ast, "A").unwrap().model;
        let b = crate::lower::lower_configuration(&ast, "B").unwrap().model;
        assert_eq!(verdict(&a, 0), Verdict::Satisfied);
        assert_eq!(verdict(&b, 0), Verdict::Violated);
        let row = &b.responsibility.as_ref().unwrap().contest_paths[0];
        assert_eq!(row.checks.reversal_authority.people[0].decisions.len(), 1);
        assert_eq!(
            row.checks.reversal_authority.people[0].decisions[0].capability,
            "Approver.second"
        );
        assert!(
            row.checks.reversal_authority.people[0].decisions[0]
                .authorities
                .is_empty()
        );
    }
}
