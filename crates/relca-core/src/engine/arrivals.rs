//! ADR 0015 global graph. Legacy State/Task and the old exploration are unchanged.
//! State is used only as a case-local payload/view for the existing bundle rules;
//! Actor values live in the global bank, never in the stored case payloads.
use super::*;
#[cfg(test)]
pub(crate) mod q6_private;
mod work_profiles;

#[cfg(test)]
#[path = "contest_rates_tests.rs"]
mod contest_rates_tests;

#[cfg(test)]
#[path = "response_arrivals_tests.rs"]
mod response_tests;

#[cfg(test)]
#[path = "../../tests/support/actor_observed_private_arrivals.rs"]
mod actor_observed_private;
#[cfg(test)]
#[path = "../../tests/support/caseset_private_arrivals.rs"]
mod caseset_private;
#[cfg(test)]
#[path = "arrivals_limits_tests.rs"]
mod explicit_limits_tests;
#[cfg(test)]
#[path = "arrivals_policy_tests.rs"]
mod fixed_policy_tests;
#[cfg(test)]
#[path = "arrivals_rule_tests.rs"]
mod rule_tests;
#[cfg(test)]
#[path = "state_invariant_arrivals_tests.rs"]
mod state_invariant_tests;

#[cfg(test)]
mod actor_observed_edges {
    use super::*;

    fn model(src: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap()
    }

    #[test]
    fn shared_read_write_simulation_refuses_both_orders_without_sampling_them() {
        let src = include_str!("../../../../fixtures/aobs-read-write-order/model.relca");
        for later in [false, true] {
            let mut src = src.replacen(
                "type R",
                "simulation replications 1 seed 0 confidence 0.5; type R",
                1,
            );
            if later {
                src = src.replace("[0min,2min]", "[0min,3min]");
            }
            let m = model(&src);
            let engine = super::super::simulate(&m);
            let reference = crate::reference::simulate(&m).outcome;
            assert_eq!(engine.completed_replications, usize::from(later) as u64);
            assert_eq!(
                reference.completed_replications,
                engine.completed_replications
            );
            assert_eq!(reference.replications, engine.replications);
            assert_eq!(
                engine.diagnostics.iter().any(|d| d.code == diag::U_SEM_0),
                !later
            );
            assert_eq!(
                reference
                    .diagnostics
                    .iter()
                    .any(|d| d.code == diag::U_SEM_0),
                !later
            );
        }
    }

    #[test]
    fn sampled_start_effect_is_published_before_the_other_case_branch() {
        let src = include_str!("../../../../fixtures/aobs-start-visible/model.relca")
            .replacen(
                "type R",
                "simulation replications 1 seed 0 confidence 0.5; type R",
                1,
            )
            // Alt finishes at4, not simultaneously with Desk at5: this tests
            // start visibility without introducing a second scheduler choice.
            .replace("duration fixed 3min", "duration fixed 2min");
        let m = model(&src);
        let engine = super::super::simulate(&m);
        let reference = crate::reference::simulate(&m).outcome;
        assert_eq!(engine.completed_replications, 1);
        assert_eq!(reference.completed_replications, 1);
        assert_eq!(engine.replications, reference.replications);
    }

    #[test]
    fn contest_management_preserves_the_shared_read_write_race() {
        let src = include_str!("../../../../fixtures/aobs-read-write-order/model.relca")
            .replacen("type R", "people [p]; actor Route {executors [p]; capability respond()->R {duration fixed 1min; result nondet {ok}}} actor Party {control party; standing over [end.done] basis \"claim\"} contract X {contest Party on end.done route Route.respond reversible_by [p] respond_by 2min} type R", 1)
            .replace("include [Desk,Alt]", "include [Desk,Alt,Route,Party]");
        let m = model(&src);
        let engine = super::super::explore(&m);
        let reference = crate::reference::explore(&m);
        assert_eq!(reference.stop, crate::reference::StopReason::Complete);
        assert_eq!(engine.case_outcomes, reference.outcome.case_outcomes);
        let cases = engine.case_outcomes.unwrap();
        let completed = cases[1].terminals[0].completed_at.unwrap();
        assert_eq!((completed.min, completed.max), (3, 4));
    }
}
use outcome::{
    AdmissionSummary, CaseOutcome, CaseProperty, CaseTerminal, CaseTerminalKind as Kind, Range,
};
use std::{collections::HashMap, rc::Rc};
type StartCandidates = BTreeMap<(usize, usize), Vec<Option<ir::PersonId>>>;

#[cfg(test)]
mod metrics_tests {
    use super::*;
    #[test]
    fn completed_snapshot_and_global_peak_survive_other_case_store() {
        let m = crate::lower::lower(
            &crate::syntax::parse(include_str!("../../../../fixtures/met-fifo/model.relca"))
                .unwrap(),
        )
        .unwrap();
        let times = arrivals(&m).unwrap();
        let mut facts = Facts::new(times.len());
        let nodes = graph(&m, &times, 10000, &mut facts).unwrap();
        let finished = nodes
            .iter()
            .find(|n| {
                n.state
                    .cases
                    .iter()
                    .all(|slot| matches!(slot,Slot::Admitted(c) if c.terminal.is_some()))
            })
            .unwrap();
        let mut global = (*finished.state).clone();
        let first = global.view(0, &m).unwrap().metric_history.unwrap();
        let peak = global.metric_peak.clone().unwrap();
        assert_eq!(peak.tasks, 2);
        let second = global.view(1, &m).unwrap();
        global.store(1, second, &m);
        assert_eq!(global.view(0, &m).unwrap().metric_history, Some(first));
        assert_eq!(global.metric_peak, Some(peak));
        // Re-observation does not count an already completed case again.
        facts.observe(&global, &m, &times).unwrap();
        facts.observe(&global, &m, &times).unwrap();
        let point =
            serde_json::to_value(metric_snapshot(&global, &m, &times, global.time, true)).unwrap();
        assert_eq!(point["overall"]["counts"]["completed"]["stats"]["min"], "3");
        assert_eq!(point["overall"]["work"]["min"], "24");
    }

    #[test]
    fn global_hash_without_metrics_retains_the_prior_field_stream() {
        use std::hash::{Hash, Hasher};
        let source = include_str!("../../../../fixtures/arrivals-staggered/model.relca");
        let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        let s = initial(&m, 3);
        assert!(s.metric_peak.is_none());
        let mut old = std::collections::hash_map::DefaultHasher::new();
        s.time.hash(&mut old);
        s.cursor.hash(&mut old);
        s.phase.hash(&mut old);
        s.shared.hash(&mut old);
        s.cases.hash(&mut old);
        let mut new = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut new);
        assert_eq!(old.finish(), new.finish());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Phase {
    Duration,
    Admission,
    Chance,
    Nondet,
    Receipt,
    Activate,
    Mixed,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Case {
    local: State,
    queued_at: Vec<u64>,
    ever_started: bool,
    received: bool,
    admitted_at: Option<u64>,
    terminal: Option<ir::EndId>,
}
impl Case {
    fn stopped(&self) -> bool {
        if self.local.response.as_ref().is_some_and(|h| h.stopped) {
            return true;
        }
        if self.terminal.is_none() {
            return false;
        }
        let Some(c) = &self.local.contest else {
            return true;
        };
        c.routes
            .iter()
            .all(|r| r.target_at.is_none() || r.filed.is_some())
            && self
                .local
                .tasks
                .iter()
                .all(|t| t.lifecycle == Lifecycle::Done)
            && c.routes.iter().all(|r| r.filed != Some(true) || r.queued)
    }
}
/// Share unchanged case histories, copying only the case written by a successor.
/// Equality, hashing and Debug remain value-based, not allocation-based. Logical
/// state/edge counts and Room reservations are unchanged by this storage choice.
#[derive(Clone, PartialEq, Eq, Hash)]
struct SharedCase(Rc<Case>);
impl From<Case> for SharedCase {
    fn from(value: Case) -> Self {
        Self(Rc::new(value))
    }
}
impl std::ops::Deref for SharedCase {
    type Target = Case;
    fn deref(&self) -> &Case {
        &self.0
    }
}
impl std::ops::DerefMut for SharedCase {
    fn deref_mut(&mut self) -> &mut Case {
        Rc::make_mut(&mut self.0)
    }
}
impl std::fmt::Debug for SharedCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Slot {
    Future,
    Rejected,
    Admitted(SharedCase),
    Held(SharedCase),
    Overflow(SharedCase),
}
impl Slot {
    fn payload(&self) -> Option<&Case> {
        match self {
            Self::Admitted(c) | Self::Held(c) | Self::Overflow(c) => Some(&**c),
            _ => None,
        }
    }
    fn payload_mut(&mut self) -> Option<&mut Case> {
        match self {
            Self::Admitted(c) | Self::Held(c) | Self::Overflow(c) => Some(&mut **c),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Global {
    external_peak: Option<u64>,
    metric_peak: Option<metrics::Peak>,
    time: u64,
    cursor: usize,
    phase: Phase,
    shared: Vec<ir::StateValue>,
    cases: Vec<Slot>,
}
impl std::hash::Hash for Global {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.time.hash(h);
        self.cursor.hash(h);
        self.phase.hash(h);
        self.shared.hash(h);
        self.cases.hash(h);
        if let Some(peak) = &self.metric_peak {
            peak.hash(h);
        }
        if let Some(peak) = self.external_peak {
            peak.hash(h);
        }
    }
}
impl Global {
    fn calendar_pending(&self, m: &ir::Model) -> EngineResult<bool> {
        let ranks = m
            .start_policy
            .as_ref()
            .filter(|p| p.call_order)
            .map(|_| policy::ranks(m))
            .transpose()?;
        for (k, slot) in self.cases.iter().enumerate() {
            let Slot::Admitted(c) = slot else { continue };
            if c.stopped() {
                continue;
            }
            for (j, task) in c.local.tasks.iter().enumerate() {
                let cap = &m.capabilities[task.capability.0];
                if task.lifecycle != Lifecycle::Queued
                    || cap.qualified_executors.as_ref().is_some_and(Vec::is_empty)
                    || !self.fifo_head(k, j, m)
                    || !m.actors[cap.actor.0].capacity.is_some_and(|n| n > 0)
                    || cap
                        .uses
                        .iter()
                        .any(|u| u.amount > m.resources[u.resource.0].capacity)
                {
                    continue;
                }
                if let Some(p) = &m.start_policy
                    && p.call_order
                    && p.priority.is_none()
                    && p.controls(cap.actor)
                {
                    let ranks = ranks.as_ref().ok_or("missing calendar policy ranks")?;
                    let own = (c.queued_at[j], k, ranks[task.binding.0]);
                    let blocked = self.cases.iter().enumerate().any(|(other, slot)| {
                        let Slot::Admitted(c) = slot else {
                            return false;
                        };
                        c.local.tasks.iter().enumerate().any(|(i, t)| {
                            let other_cap = &m.capabilities[t.capability.0];
                            t.lifecycle == Lifecycle::Queued
                                && p.controls(other_cap.actor)
                                && (c.queued_at[i], other, ranks[t.binding.0]) < own
                                && (other_cap.actor == cap.actor
                                    || cap.uses.iter().any(|u| {
                                        other_cap.uses.iter().any(|v| u.resource == v.resource)
                                    }))
                        })
                    });
                    if blocked {
                        continue;
                    }
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn calendar_fit(
        &self,
        cap: &ir::Capability,
        actors: &[u64],
        units: &[u64],
        m: &ir::Model,
    ) -> bool {
        let Some(tables) = &m.resource_time else {
            return true;
        };
        let fits = |id: Option<usize>, used: u64, need: u64| {
            id.is_none_or(|id| {
                tables
                    .calendars
                    .get(id)
                    .and_then(|c| c.capacity_at(self.time))
                    .is_some_and(|capacity| used.checked_add(need).is_some_and(|n| n <= capacity))
            })
        };
        fits(tables.actors[cap.actor.0], actors[cap.actor.0], 1)
            && cap.uses.iter().all(|u| {
                fits(
                    tables.resources[u.resource.0],
                    units[u.resource.0],
                    u.amount,
                )
            })
    }

    fn on_shift(&self, person: ir::PersonId, m: &ir::Model) -> bool {
        m.resource_time.as_ref().is_none_or(|t| {
            t.people[person.0].is_none_or(|id| t.calendars[id].capacity_at(self.time) == Some(1))
        })
    }
    fn normalize_state_invariants(&mut self, m: &ir::Model) -> EngineResult<()> {
        if !m
            .contracts
            .iter()
            .any(|c| matches!(c.kind, ir::ContractKind::StateInvariant { .. }))
            || self.live_cut(m)
        {
            return Ok(());
        }
        for k in 0..self.cases.len() {
            // ADR 0039 m8: Held and Overflow cases were received at arrival
            // and are monitored like admitted ones; rejected/Future are not.
            if !self.cases[k].payload().is_some_and(|c| c.received) {
                continue;
            }
            let mut view = self.view(k, m)?;
            update_state_invariants(&mut view, m, k)?;
            if let Some(c) = self.cases[k].payload_mut() {
                c.local.monitors = view.monitors;
            }
        }
        Ok(())
    }
    fn resource_occupancy(&self, m: &ir::Model) -> EngineResult<Vec<u64>> {
        resource_occupancy(
            self.cases
                .iter()
                .filter_map(|s| match s {
                    Slot::Admitted(c) => Some(c.local.tasks.iter()),
                    _ => None,
                })
                .flatten(),
            m,
        )
    }
    fn view(&self, k: usize, m: &ir::Model) -> EngineResult<State> {
        let Some(c) = self.cases[k].payload() else {
            return Err("inactive case view".into());
        };
        let mut s = c.local.clone();
        s.time = self.time;
        for (i, v) in m.state_variables.iter().enumerate() {
            if m.is_shared_state(v.kind) {
                s.state_values[i] = self.shared[i];
            }
        }
        Ok(s)
    }
    fn store(&mut self, k: usize, mut s: State, m: &ir::Model) {
        if let Some(h) = &mut s.response {
            let old = self.cases[k]
                .payload()
                .and_then(|c| c.local.response.as_ref())
                .map_or(0, |h| h.rows.len());
            for row in &mut h.rows[old..] {
                row.effect_retained &= row.expected_state == s.state_values;
            }
        }
        let writes = match &self.cases[k] {
            Slot::Admitted(c) => transition_writes(&c.local, &s, m),
            _ => Vec::new(),
        };
        metrics::observe(&mut s, m);
        for (i, v) in m.state_variables.iter().enumerate() {
            if m.is_shared_state(v.kind) {
                self.shared[i] = s.state_values[i];
                s.state_values[i] = ir::StateValue::None;
            }
        }
        let Some(c) = self.cases[k].payload_mut() else {
            unreachable!()
        };
        c.local = s;
        for (j, slot) in self.cases.iter_mut().enumerate() {
            if j != k
                && let Slot::Admitted(c) = slot
            {
                notify_writes(&mut c.local, None, &writes, m);
            }
        }
        if let Some(peak) = &mut self.metric_peak {
            peak.observe(
                self.cases.iter().filter_map(|slot| match slot {
                    Slot::Admitted(c) => c
                        .local
                        .metric_history
                        .as_ref()
                        .map(|h| (&c.local, &h.tasks[..])),
                    _ => None,
                }),
                m,
            );
        }
    }
    fn waiting(&self) -> usize {
        self.cases
            .iter()
            .filter(|s| matches!(s, Slot::Admitted(c) if c.terminal.is_none() && !c.ever_started))
            .count()
    }
    fn room(&self, m: &ir::Model) -> bool {
        m.admission_queue
            .is_none_or(|q| (self.waiting() as u64) < q.capacity)
            && m.concurrent_cases.is_none_or(|q| {
                (self
                    .cases
                    .iter()
                    .filter(|s| matches!(s, Slot::Admitted(c) if c.terminal.is_none()))
                    .count() as u64)
                    < q.capacity
            })
    }
    fn live_cut(&self, m: &ir::Model) -> bool {
        self.cases.iter().any(
            |s| matches!(s, Slot::Admitted(c) if c.local.exceeds_live_bound(m.bounds.live_tasks)),
        )
    }
    fn fifo_head(&self, k: usize, j: usize, m: &ir::Model) -> bool {
        // Include all queued predecessors, even in stopped cases. Candidate
        // liveness is checked separately, as in the reference dispatch rules.
        if m.start_policy.is_none() {
            return true;
        }
        let Slot::Admitted(c) = &self.cases[k] else {
            return false;
        };
        if let Some(p) = &m.start_policy
            && p.priority.is_some()
        {
            let cap = &m.capabilities[c.local.tasks[j].capability.0];
            if !p.controls(cap.actor) {
                return true;
            }
            let ranks = policy::ranks(m).expect("validated lexical sites");
            let own = (c.queued_at[j], k, ranks[c.local.tasks[j].binding.0]);
            return !self.cases.iter().enumerate().any(|(other, slot)| {
                let Slot::Admitted(c) = slot else {
                    return false;
                };
                c.local.tasks.iter().enumerate().any(|(i, task)| {
                    let before = &m.capabilities[task.capability.0];
                    task.lifecycle == Lifecycle::Queued
                        && p.controls(before.actor)
                        && policy::priority_before(
                            m,
                            (c.queued_at[i], other, ranks[task.binding.0]),
                            own,
                        )
                        && (before.actor == cap.actor
                            || before
                                .uses
                                .iter()
                                .any(|u| cap.uses.iter().any(|v| u.resource == v.resource)))
                })
            });
        }
        let actor = m.capabilities[c.local.tasks[j].capability.0].actor;
        let key = (c.queued_at[j], k);
        !self.cases.iter().enumerate().any(|(other,s)| {
            matches!(s, Slot::Admitted(c) if c.local.tasks.iter().enumerate().any(|(i,t)|
                t.lifecycle == Lifecycle::Queued && m.capabilities[t.capability.0].actor == actor && (c.queued_at[i],other) < key))
        })
    }

    /// Intersecting checked receipts constrain only currently due finishes.
    /// Staleness and results are deliberately not read by this relation.
    fn version_finish_order(&self, k: usize, j: usize, m: &ir::Model) -> (bool, bool) {
        let Slot::Admitted(own) = &self.cases[k] else {
            return (false, false);
        };
        let cap = &m.capabilities[own.local.tasks[j].capability.0];
        let Some(check) = &cap.version_check else {
            return (true, false);
        };
        let Some(tie) = check.resolve else {
            return (true, false);
        };
        let ranks = &m
            .case_priority
            .as_ref()
            .expect("validated resolution priority")
            .ranks;
        let mut allowed = true;
        let mut unresolved = false;
        for (other, slot) in self.cases.iter().enumerate() {
            let Slot::Admitted(c) = slot else { continue };
            if c.stopped() {
                continue;
            }
            for (site, task) in c.local.tasks.iter().enumerate() {
                let due = matches!(task.lifecycle, Lifecycle::Running { finish, .. } if finish == self.time)
                    || matches!(
                        task.lifecycle,
                        Lifecycle::DistributedRunning { due: true, .. }
                    );
                if !due {
                    continue;
                }
                let before = &m.capabilities[task.capability.0];
                let Some(peer) = &before.version_check else {
                    continue;
                };
                if before.actor != cap.actor
                    || peer.resolve != Some(tie)
                    || !peer.variables.iter().any(|id| check.variables.contains(id))
                {
                    continue;
                }
                allowed &= !(ranks[other] > ranks[k]
                    || (ranks[other] == ranks[k] && tie == ir::ResolutionTie::CaseId && other < k));
                unresolved |= (other != k || site != j)
                    && ranks[other] == ranks[k]
                    && (tie == ir::ResolutionTie::Nondet || other == k);
            }
        }
        (allowed, unresolved)
    }

    /// Lightweight preselection: no successor allocation, monitor or Room use.
    fn fixed_starts(&self, m: &ir::Model) -> EngineResult<StartCandidates> {
        let p = m.start_policy.as_ref().ok_or("missing fixed policy")?;
        let ranks = policy::ranks(m)?;
        let units = self.resource_occupancy(m)?;
        let mut counts = vec![0; m.actors.len()];
        let mut busy = BTreeSet::new();
        let mut queue = Vec::new();
        for (k, slot) in self.cases.iter().enumerate() {
            if let Slot::Admitted(c) = slot {
                for (j, t) in c.local.tasks.iter().enumerate() {
                    let actor = m.capabilities[t.capability.0].actor;
                    if t.lifecycle.running() {
                        counts[actor.0] += 1;
                        if let Some(person) = t.executor {
                            busy.insert(person);
                        }
                    } else if t.lifecycle == Lifecycle::Queued {
                        queue.push((k, j, (c.queued_at[j], k, ranks[t.binding.0]), t.capability));
                    }
                }
            }
        }
        let mut candidates = BTreeMap::new();
        let mut minimum = None;
        for &(k, j, key, cap_id) in &queue {
            if p.priority.is_some() && matches!(&self.cases[k], Slot::Admitted(c) if c.stopped()) {
                continue;
            }
            let cap = &m.capabilities[cap_id.0];
            let actor = &m.actors[cap.actor.0];
            let ordered = p.priority.is_none() && p.call_order && p.controls(cap.actor);
            if !self.fifo_head(k, j, m)
                || counts[cap.actor.0] >= actor.capacity.ok_or("capacity-less execution")?
                || !resources_fit(cap, &units, m)
                || !self.calendar_fit(cap, &counts, &units, m)
            {
                continue;
            }
            if ordered
                && queue.iter().any(|&(_, _, earlier, other)| {
                    let other = &m.capabilities[other.0];
                    earlier < key
                        && p.controls(other.actor)
                        && (other.actor == cap.actor
                            || cap
                                .uses
                                .iter()
                                .any(|u| other.uses.iter().any(|v| v.resource == u.resource)))
                })
            {
                continue;
            }
            let order = if p.assign_first_idle && p.controls(cap.actor) {
                &p.executor_order[cap.actor.0]
            } else {
                &actor.executors
            };
            let mut people: Vec<_> = order.as_ref().map_or_else(
                || vec![None],
                |ps| {
                    ps.iter()
                        .filter(|p| {
                            cap.qualified_executors
                                .as_ref()
                                .is_none_or(|qs| qs.contains(p))
                                && !busy.contains(p)
                                && self.on_shift(**p, m)
                        })
                        .map(|p| Some(*p))
                        .collect()
                },
            );
            if p.assign_first_idle && p.controls(cap.actor) {
                people.truncate(1);
            }
            if people.is_empty() {
                continue;
            }
            if ordered {
                minimum = Some(minimum.map_or(key, |old: (ir::Tick, usize, usize)| old.min(key)));
            }
            candidates.insert((k, j), people);
        }
        if p.priority.is_some() {
            let available: Vec<_> = candidates.keys().copied().collect();
            candidates.retain(|&(k, j), _| {
                let Slot::Admitted(c) = &self.cases[k] else {
                    return false;
                };
                let task = &c.local.tasks[j];
                if !p.controls(m.capabilities[task.capability.0].actor) {
                    return true;
                }
                let key = (c.queued_at[j], k, ranks[task.binding.0]);
                !available.iter().any(|&(a, b)| {
                    let Slot::Admitted(other) = &self.cases[a] else {
                        return false;
                    };
                    let before = &other.local.tasks[b];
                    p.controls(m.capabilities[before.capability.0].actor)
                        && policy::priority_before(
                            m,
                            (other.queued_at[b], a, ranks[before.binding.0]),
                            key,
                        )
                })
            });
        } else if p.call_order {
            candidates.retain(|&(k, j), _| {
                let Slot::Admitted(c) = &self.cases[k] else {
                    return false;
                };
                let t = &c.local.tasks[j];
                !p.controls(m.capabilities[t.capability.0].actor)
                    || Some((c.queued_at[j], k, ranks[t.binding.0])) == minimum
            });
        }
        Ok(candidates)
    }
}
#[derive(Clone)]
struct Node {
    state: Rc<Global>,
    actions: Vec<Vec<(Rational, usize)>>,
    unknown: bool,
    cut: ProbabilityCut,
    numeric: bool,
}

// The global driver supplies labels without changing Global, Case or local keys.
impl trace::Capture {
    fn global_context(&mut self, s: &Global, m: &ir::Model) {
        if self.failure == Some(crate::explain::ReplayReason::CaptureLimit) {
            return;
        }
        let mut resources: Vec<_> = m
            .actors
            .iter()
            .map(|a| crate::explain::Resource {
                actor: a.name.clone(),
                capacity: a.capacity.unwrap_or(0),
                anonymous: a.executors.is_none(),
                holders: Vec::new(),
            })
            .collect();
        for (k, slot) in s.cases.iter().enumerate() {
            if let Slot::Admitted(c) = slot {
                for task in &c.local.tasks {
                    if task.lifecycle.running() {
                        resources[m.capabilities[task.capability.0].actor.0]
                            .holders
                            .push(crate::explain::Holder {
                                case_id: k as u64 + 1,
                                binding: m.site_name(task.binding),
                                executor: task.executor.map(|p| m.people[p.0].name.clone()),
                            });
                    }
                }
            }
        }
        self.shared_resources = Some(resources);
        let mut rows: Vec<_> = m
            .resources
            .iter()
            .map(|r| crate::explain::SharedResource {
                resource: r.name.clone(),
                capacity: r.capacity,
                occupied: 0,
                holders: Vec::new(),
            })
            .collect();
        for (k, slot) in s.cases.iter().enumerate() {
            if let Slot::Admitted(c) = slot {
                for t in c.local.tasks.iter().filter(|t| t.lifecycle.running()) {
                    for u in &m.capabilities[t.capability.0].uses {
                        rows[u.resource.0]
                            .holders
                            .push(crate::explain::SharedHolder {
                                case_id: k as u64 + 1,
                                binding: m.site_name(t.binding),
                                amount: u.amount,
                            });
                    }
                }
            }
        }
        for row in &mut rows {
            let Some(sum) = row
                .holders
                .iter()
                .try_fold(0u64, |n, h| n.checked_add(h.amount))
            else {
                self.failure = Some(crate::explain::ReplayReason::IllegalTransition);
                return;
            };
            row.occupied = sum;
            row.holders.sort_by_key(|h| (h.case_id, h.binding.clone()));
        }
        self.shared_model_resources = Some(rows);
    }
    fn global_leaf(&mut self, s: &Global, m: &ir::Model, kind: crate::explain::Kind) {
        if self.failure == Some(crate::explain::ReplayReason::CaptureLimit) {
            return;
        }
        use crate::explain::{
            DeadlockDetail, Join as TraceJoin, Kind as K, Obligation, WaitingTask,
        };
        let limit = if kind == K::Horizon {
            s.time
        } else {
            m.bounds.clock
        };
        let mut cases = BTreeMap::new();
        let mut detail = DeadlockDetail {
            shared_resources: Vec::new(),
            waiting_tasks: Vec::new(),
            resources: Vec::new(),
            obligations: Vec::new(),
            joins: Vec::new(),
        };
        let mut related_actors = BTreeSet::new();
        let mut origins = Vec::new();
        for (k, slot) in s.cases.iter().enumerate() {
            if let Some(c) = slot.payload() {
                if c.stopped() {
                    continue;
                }
                if c.local.obligations.is_some() {
                    let mut local = c.local.clone();
                    if kind == K::Deadlock {
                        contest_deadlock(&mut local, m);
                        close_responses(&mut local);
                    } else {
                        refresh_contests(&mut local, m, limit, true);
                        expire_debts(&mut local, limit, true);
                    }
                    origins.extend(Self::origins(&local, m, k as u64 + 1));
                    for id in Self::targets(&local, m) {
                        cases.entry(id).or_insert(k as u64 + 1);
                    }
                    if kind == K::Deadlock {
                        detail
                            .obligations
                            .extend(Self::debt_details(&local, m, k as u64 + 1));
                    }
                }
                for (n, monitor) in c.local.monitors.iter().enumerate() {
                    if monitor.violated
                        || monitor
                            .pending
                            .iter()
                            .any(|d| kind == K::Deadlock || *d <= limit)
                    {
                        cases
                            .entry(m.contracts[n].property_id(ir::ClauseId(n)))
                            .or_insert(k as u64 + 1);
                    }
                    for deadline in &monitor.pending {
                        detail.obligations.push(Obligation {
                            case_id: k as u64 + 1,
                            id: m.contracts[n].property_id(ir::ClauseId(n)),
                            trigger_step: 0,
                            deadline: *deadline,
                        });
                    }
                }
                for task in &c.local.tasks {
                    let actor = m.capabilities[task.capability.0].actor;
                    related_actors.insert(m.actors[actor.0].name.clone());
                    if task.lifecycle == Lifecycle::Queued {
                        let resource_shortages = match Self::shortages(
                            &m.capabilities[task.capability.0],
                            self.shared_model_resources.as_deref().unwrap_or(&[]),
                        ) {
                            Ok(rows) => rows,
                            Err(reason) => {
                                self.failure.get_or_insert(reason);
                                return;
                            }
                        };
                        detail.waiting_tasks.push(WaitingTask {
                            resource_shortages,
                            case_id: k as u64 + 1,
                            binding: m.site_name(task.binding),
                            actor: m.actors[actor.0].name.clone(),
                        });
                    }
                }
                if c.local.waiting {
                    detail.joins.push(TraceJoin {
                        case_id: k as u64 + 1,
                        bindings: c
                            .local
                            .tasks
                            .iter()
                            .map(|t| m.site_name(t.binding))
                            .collect(),
                    });
                }
            }
        }
        if kind == K::Deadlock {
            cases.insert("diagnostic:W-DEADLOCK-0".into(), 1);
        }
        detail.resources = self
            .shared_resources
            .as_ref()
            .into_iter()
            .flatten()
            .filter(|r| related_actors.contains(&r.actor))
            .cloned()
            .collect();
        detail.shared_resources = self.shared_model_resources.as_ref().into_iter().flatten().enumerate().filter(|(i,_)| s.cases.iter().any(|slot| matches!(slot, Slot::Admitted(c) if c.local.tasks.iter().filter(|t| t.lifecycle == Lifecycle::Queued).any(|t| m.capabilities[t.capability.0].uses.iter().any(|u| u.resource.0 == *i))))).map(|(_,r)| r.clone()).collect();
        let mut base = State::initial(m, 0);
        base.time = if kind == K::Deadline {
            m.bounds.clock
        } else {
            s.time
        };
        let stage = self.stage(&base, m, kind).map(|mut stage| {
            stage.case_id = None;
            stage.values = crate::explain::Values::default();
            if kind == K::Deadlock {
                stage.deadlock_detail = Some(detail);
            }
            stage
        });
        let node = self.add(stage, cases.keys().cloned().collect());
        self.record_origins(node, origins);
        if let Some(edge) = self.edges[self.current].last_mut().filter(|e| e.to == node) {
            edge.cases = cases;
        }
    }
    fn global_initial(&mut self, s: &Global, m: &ir::Model) {
        let mut state = State::initial(m, 0);
        state.time = s.time;
        match self.stage(&state, m, crate::explain::Kind::Initial) {
            Ok(mut stage) => {
                stage.case_id = None;
                stage.values.observed.retain(|value| {
                    m.state_variables.iter().any(|var| matches!(var.kind, ir::StateKind::ActorObserved(a) if !m.actors[a.0].is_per_case() && value.name == format!("{}.{}", m.actors[a.0].name, var.name) && value.location.span == var.span))
                });
                stage.values.latent.retain(|v| {
                    m.state_variables.iter().any(|var| {
                        var.name == v.name
                            && m.is_shared_state(var.kind)
                            && (!m.actors.iter().any(|a| a.is_per_case())
                                || var.span == v.location.span)
                    })
                });
                self.initial = Some(stage);
            }
            Err(reason) => self.failure = Some(reason),
        }
    }
    fn global_label(
        &mut self,
        a: &Global,
        b: &Global,
        m: &ir::Model,
    ) -> Result<crate::explain::Stage, crate::explain::ReplayReason> {
        use crate::explain::{Event as E, Kind as K, Values};
        let mut base = State::initial(m, 0);
        base.time = b.time;
        let mut stage = self.stage(&base, m, K::Initialize)?;
        stage.case_id = None;
        stage.values = Values::default();
        if a.time != b.time {
            stage.kind = K::Advance;
            return Ok(stage);
        }
        match a.phase {
            Phase::Admission => {
                stage.kind = K::Admission;
                stage.location = self.location(self.arrival_span.unwrap_or(m.process.span))?;
                for k in a.cursor..b.cursor {
                    let mut event = Self::event(&stage, K::Admission);
                    event.case_id = Some(k as u64 + 1);
                    stage.events.push(E::Business(event));
                }
            }
            Phase::Receipt => {
                stage.kind = K::Receive;
                stage.location = self.location(self.arrival_span.unwrap_or(m.process.span))?;
                for (k, (old, new)) in a.cases.iter().zip(&b.cases).enumerate() {
                    if old
                        .payload()
                        .zip(new.payload())
                        .is_some_and(|(old, new)| !old.received && new.received)
                    {
                        let mut event = Self::event(&stage, K::Received);
                        event.case_id = Some(k as u64 + 1);
                        stage.events.push(E::Business(event));
                    }
                }
            }
            Phase::Duration => {
                stage.kind = K::DurationGate;
                for k in 0..a.cases.len() {
                    if let (Slot::Admitted(_), Slot::Admitted(_)) = (&a.cases[k], &b.cases[k]) {
                        self.case_id = k as u64 + 1;
                        let before = a
                            .view(k, m)
                            .map_err(|_| crate::explain::ReplayReason::StateMismatch)?;
                        let after = b
                            .view(k, m)
                            .map_err(|_| crate::explain::ReplayReason::StateMismatch)?;
                        if before.tasks != after.tasks {
                            stage
                                .events
                                .extend(self.classify(&before, &after, m)?.events);
                        }
                    }
                }
            }
            Phase::Chance | Phase::Nondet => {
                stage.kind = K::Initialize;
                for k in 0..a.cases.len() {
                    if let (Some(old), Some(_)) = (a.cases[k].payload(), b.cases[k].payload())
                        && !old.received
                    {
                        self.case_id = k as u64 + 1;
                        let after = b
                            .view(k, m)
                            .map_err(|_| crate::explain::ReplayReason::StateMismatch)?;
                        let initialized = self.stage(&after, m, K::Initialize)?;
                        stage
                            .events
                            .push(E::Business(Self::event(&initialized, K::Initialize)));
                        stage.events.extend(
                            initialized
                                .values
                                .observed
                                .into_iter()
                                .chain(initialized.values.latent)
                                .map(E::Value),
                        );
                    }
                }
            }
            Phase::Activate => {
                stage.kind = K::Admitted;
                for k in 0..a.cases.len() {
                    if a.cases[k]
                        .payload()
                        .zip(b.cases[k].payload())
                        .is_some_and(|(old, new)| old.admitted_at != new.admitted_at)
                    {
                        let mut event = Self::event(&stage, K::Admitted);
                        event.case_id = Some(k as u64 + 1);
                        stage.events.push(E::Business(event));
                    } else if matches!((&a.cases[k], &b.cases[k]), (Slot::Overflow(old), Slot::Overflow(new)) if old.terminal.is_none() && new.terminal.is_some())
                    {
                        let mut event = Self::event(&stage, K::End);
                        event.case_id = Some(k as u64 + 1);
                        stage.events.push(E::Business(event));
                    }
                }
            }
            Phase::Mixed => {
                if a.cases
                    .iter()
                    .zip(&b.cases)
                    .any(|(a, b)| matches!((a, b), (Slot::Held(_), Slot::Admitted(_))))
                {
                    stage.kind = K::Admitted;
                    for (k, (old, new)) in a.cases.iter().zip(&b.cases).enumerate() {
                        if matches!((old, new), (Slot::Held(_), Slot::Admitted(_))) {
                            let mut event = Self::event(&stage, K::Admitted);
                            event.case_id = Some(k as u64 + 1);
                            stage.events.push(E::Business(event));
                        }
                    }
                    return Ok(stage);
                }
                for k in 0..a.cases.len() {
                    if let (Slot::Admitted(old), Slot::Admitted(new)) = (&a.cases[k], &b.cases[k])
                        && old != new
                    {
                        if m.capabilities.iter().any(|c| c.version_check.is_some())
                            || m.state_variables.iter().any(|v| v.last_write_wins)
                        {
                            let mut active = old.clone();
                            active.local.monitors.clone_from(&new.local.monitors);
                            for task in &mut active.local.tasks {
                                if let Some(other) =
                                    new.local.tasks.iter().find(|t| t.binding == task.binding)
                                {
                                    task.stale.clone_from(&other.stale);
                                }
                            }
                            if active == *new {
                                continue;
                            }
                        }
                        if m.contracts
                            .iter()
                            .any(|c| matches!(c.kind, ir::ContractKind::StateInvariant { .. }))
                        {
                            let mut without_monitor_change = old.clone();
                            without_monitor_change
                                .local
                                .monitors
                                .clone_from(&new.local.monitors);
                            if without_monitor_change == *new {
                                continue;
                            }
                        }
                        self.case_id = k as u64 + 1;
                        let before = a
                            .view(k, m)
                            .map_err(|_| crate::explain::ReplayReason::StateMismatch)?;
                        let after = b
                            .view(k, m)
                            .map_err(|_| crate::explain::ReplayReason::StateMismatch)?;
                        stage = if old.terminal != new.terminal {
                            let mut stage = self.stage(&after, m, K::End)?;
                            stage.location = self.location(self.spans[before.pc])?;
                            stage
                        } else {
                            self.classify(&before, &after, m)?
                        };
                        // Resource snapshots include every case sharing this Actor.
                        if let Some(resource) = &mut stage.resource {
                            resource.holders.clear();
                            for (other, slot) in b.cases.iter().enumerate() {
                                if let Slot::Admitted(c) = slot {
                                    for task in &c.local.tasks {
                                        if task.lifecycle.running()
                                            && m.actors[m.capabilities[task.capability.0].actor.0]
                                                .name
                                                == resource.actor
                                        {
                                            resource.holders.push(crate::explain::Holder {
                                                case_id: other as u64 + 1,
                                                binding: m.site_name(task.binding),
                                                executor: task
                                                    .executor
                                                    .map(|p| m.people[p.0].name.clone()),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
        Ok(stage)
    }
    fn global_transition(&mut self, a: &Global, b: &Global, m: &ir::Model) -> usize {
        if self.failure == Some(crate::explain::ReplayReason::CaptureLimit) {
            return 0;
        }
        let mut cases = BTreeMap::new();
        for (k, slot) in b.cases.iter().enumerate() {
            if let Some(c) = slot.payload() {
                for id in Self::targets(&c.local, m) {
                    cases.entry(id).or_insert(k as u64 + 1);
                }
            }
        }
        let label = self.global_label(a, b, m);
        let node = self.add_transition(label, cases.keys().cloned().collect());
        for k in 0..b.cases.len() {
            if m.contracts
                .iter()
                .any(|c| matches!(c.kind, ir::ContractKind::StateInvariant { .. }))
                && b.cases[k].payload().is_some_and(|c| c.received)
            {
                match b.view(k, m) {
                    Ok(view) => self.capture_state_details(node, &view, m, k as u64 + 1),
                    Err(_) => {
                        self.failure
                            .get_or_insert(crate::explain::ReplayReason::StateMismatch);
                    }
                }
            }
        }
        self.record_origins(
            node,
            b.cases
                .iter()
                .enumerate()
                .flat_map(|(k, slot)| match slot {
                    Slot::Admitted(c) | Slot::Held(c) | Slot::Overflow(c) => {
                        Self::origins(&c.local, m, k as u64 + 1)
                    }
                    _ => Vec::new(),
                })
                .collect(),
        );
        if let Some(edge) = self.edges[self.current].last_mut().filter(|e| e.to == node) {
            edge.cases = cases;
        }
        node
    }
}

struct Facts {
    response_receipts: Vec<outcome::ResponseReceipt>,
    horizon: bool,
    observed_until: Option<Range>,
    aborted_completion: Vec<crate::completion::Graph>,
    trace: Option<trace::Capture>,
    // Already allocated successors awaiting trace-only observation on abort.
    trace_partial_actions: Vec<Global>,
    metrics: Option<crate::metrics::BusinessMetrics>,
    per: Vec<Exploration>,
    cut: Vec<bool>,
    terminals: Vec<BTreeMap<Kind, CaseTerminal>>,
    truncated: bool,
    budget: bool,
    numeric: bool,
    deadlock: bool,
    overcommit: bool,
    scheduler: bool,
    policy_unresolved: bool,
    chance: bool,
    #[cfg(test)]
    clones: usize,
    #[cfg(test)]
    census: (usize, usize, usize),
}
impl Facts {
    fn state_invariant_generated_prefix(
        &mut self,
        parent: &Global,
        writer: usize,
        generated: &State,
        m: &ir::Model,
    ) -> EngineResult<()> {
        if !generated.initialized
            || generated.time > m.bounds.clock
            || generated.exceeds_live_bound(m.bounds.live_tasks)
        {
            return Ok(());
        }
        let node = if let Some(trace) = &mut self.trace {
            let before = parent.view(writer, m)?;
            trace.case_id = writer as u64 + 1;
            let stage = trace.classify(&before, generated, m);
            Some(trace.add_transition(stage, Vec::new()))
        } else {
            None
        };
        let mut cases = BTreeMap::new();
        for k in 0..parent.cases.len() {
            if !parent.cases[k]
                .payload()
                .is_some_and(|c| c.received || k == writer && parent.phase == Phase::Receipt)
            {
                continue;
            }
            let mut view = if k == writer {
                generated.clone()
            } else {
                parent.view(k, m)?
            };
            for (i, v) in m.state_variables.iter().enumerate() {
                if m.is_shared_state(v.kind) {
                    view.state_values[i] = generated.state_values[i];
                }
            }
            update_state_invariants(&mut view, m, k)?;
            self.per[k].contracts(&view);
            for id in trace::Capture::targets(&view, m) {
                cases.entry(id).or_insert(k as u64 + 1);
            }
            if let (Some(trace), Some(node)) = (&mut self.trace, node) {
                trace.capture_state_details(node, &view, m, k as u64 + 1);
            }
        }
        if let (Some(trace), Some(node)) = (&mut self.trace, node)
            && let Some(edge) = trace.edges[trace.current]
                .last_mut()
                .filter(|e| e.to == node)
        {
            edge.targets = cases.keys().cloned().collect();
            edge.cases = cases;
        }
        Ok(())
    }
    fn collect_prefix(&mut self, s: &Global, m: &ir::Model, reason: &str) -> EngineResult<()> {
        if !m.tracks_obligations() {
            return Ok(());
        }
        for (k, slot) in s.cases.iter().enumerate() {
            // Held and overflowed cases were received at arrival and carry
            // receipt obligations like admitted ones (the reference leaf).
            if let Some(c) = slot.payload() {
                if c.stopped() {
                    self.per[k].collect_debts(&c.local, m, "terminal_unresolved");
                } else {
                    let local = s.view(k, m)?;
                    self.per[k].collect_debts(&local, m, reason);
                }
            }
        }
        Ok(())
    }
    fn local_using<T>(
        &mut self,
        k: usize,
        action: impl FnOnce(&mut Exploration) -> EngineResult<T>,
    ) -> EngineResult<T> {
        if let Some(trace) = &mut self.trace {
            trace.case_id = k as u64 + 1;
        }
        self.per[k].trace = self.trace.take();
        self.per[k].range_case = Some(k as u64 + 1);
        let result = action(&mut self.per[k]);
        self.trace = self.per[k].trace.take();
        result
    }
    fn new(n: usize) -> Self {
        Self {
            response_receipts: Vec::new(),
            horizon: false,
            observed_until: None,
            aborted_completion: Vec::new(),
            trace: None,
            trace_partial_actions: Vec::new(),
            metrics: None,
            per: (0..n).map(|_| Exploration::default()).collect(),
            cut: vec![false; n],
            terminals: vec![BTreeMap::new(); n],
            truncated: false,
            budget: false,
            numeric: false,
            deadlock: false,
            overcommit: false,
            scheduler: false,
            policy_unresolved: false,
            chance: false,
            #[cfg(test)]
            clones: 0,
            #[cfg(test)]
            census: (0, 0, 1),
        }
    }
    fn mark_cut(&mut self, s: &Global, budget: bool) {
        self.truncated = true;
        self.budget |= budget;
        for (k, c) in s.cases.iter().enumerate() {
            self.cut[k] |= !matches!(c, Slot::Rejected) && !c.payload().is_some_and(Case::stopped);
        }
    }
    fn observe(&mut self, s: &Global, m: &ir::Model, arrivals: &[u64]) -> EngineResult<()> {
        for (resource, used) in m.resources.iter().zip(s.resource_occupancy(m)?) {
            self.overcommit |= used > resource.capacity;
        }
        let mut counts = vec![0u64; m.actors.len()];
        let mut people = BTreeSet::new();
        for (k, slot) in s.cases.iter().enumerate() {
            let terminal = match slot {
                Slot::Rejected => Some((Kind::Rejected, None)),
                Slot::Admitted(c) | Slot::Held(c) | Slot::Overflow(c) => {
                    if let Some(history) = &c.local.response {
                        for observation in &history.rows {
                            let cap = &m.capabilities[m
                                .response_attempts()
                                .into_iter()
                                .find(|(call, _)| call.binding == observation.binding)
                                .ok_or("unknown receipt site")?
                                .0
                                .capability
                                .0];
                            let row = outcome::ResponseReceipt {
                                case_id: k as u64 + 1,
                                command: m.site_name(observation.command),
                                attempt: observation.attempt,
                                capability: format!("{}.{}", m.actors[cap.actor.0].name, cap.name),
                                receipt: observation.receipt,
                                at: Range {
                                    min: observation.at,
                                    max: observation.at,
                                },
                                observed_result: (observation.receipt
                                    == crate::ast::Receipt::Delivered)
                                    .then(|| {
                                        m.types[cap.return_type.0].members[observation.actual.0]
                                            .clone()
                                    }),
                                resolved_effect: observation.resolved_effect,
                            };
                            if let Some(old) = self.response_receipts.iter_mut().find(|old| {
                                old.case_id == row.case_id
                                    && old.command == row.command
                                    && old.attempt == row.attempt
                                    && old.receipt == row.receipt
                                    && old.observed_result == row.observed_result
                                    && old.resolved_effect == row.resolved_effect
                            }) {
                                extend(&mut old.at, row.at.min);
                            } else {
                                self.response_receipts.push(row);
                            }
                        }
                    }
                    self.per[k]
                        .lost_updates
                        .extend(c.local.lost_updates.iter().copied());
                    self.per[k].contracts(&c.local);
                    for t in &c.local.tasks {
                        if t.lifecycle.running() {
                            counts[m.capabilities[t.capability.0].actor.0] += 1;
                            if let Some(p) = t.executor {
                                self.overcommit |= !people.insert(p);
                            }
                        }
                    }
                    c.terminal
                        .map(|e| {
                            (
                                if matches!(slot, Slot::Overflow(_)) {
                                    Kind::Overflow(e)
                                } else {
                                    Kind::End(e)
                                },
                                Some(c),
                            )
                        })
                        .or_else(|| {
                            c.local
                                .response
                                .as_ref()
                                .is_some_and(|h| h.stopped)
                                .then_some((Kind::ReceiptUnknown, Some(c)))
                        })
                }
                Slot::Future => None,
            };
            if let Some((kind, c)) = terminal {
                let point = |x| Range { min: x, max: x };
                let mut value = CaseTerminal {
                    observed_until: None,
                    observed_elapsed: None,
                    observed_work: None,
                    kind,
                    completed_at: c.map(|c| {
                        point(
                            c.local
                                .contest
                                .as_ref()
                                .and_then(|x| x.business.as_ref())
                                .map_or(c.local.time, |b| b.at),
                        )
                    }),
                    elapsed: c.map(|c| {
                        point(
                            c.local
                                .contest
                                .as_ref()
                                .and_then(|x| x.business.as_ref())
                                .map_or(c.local.time, |b| b.at)
                                - arrivals[k],
                        )
                    }),
                    work: c.map(|c| {
                        point(
                            c.local
                                .contest
                                .as_ref()
                                .and_then(|x| x.business.as_ref())
                                .map_or(c.local.work, |b| b.work),
                        )
                    }),
                    result_at: c.and_then(|c| c.local.result_at.map(|x| point(x - arrivals[k]))),
                    probability: None,
                };
                if kind == Kind::ReceiptUnknown {
                    value.observed_until = value.completed_at.take();
                    value.observed_elapsed = value.elapsed.take();
                    value.observed_work = value.work.take();
                    value.result_at = None;
                }
                if let Some(t) = self.terminals[k].get_mut(&kind) {
                    if t.result_at.is_some() != value.result_at.is_some() {
                        return Err("mixed adopted/unadopted paths to one case terminal".into());
                    }
                    for (dst, src) in [
                        (&mut t.completed_at, value.completed_at),
                        (&mut t.elapsed, value.elapsed),
                        (&mut t.work, value.work),
                        (&mut t.result_at, value.result_at),
                        (&mut t.observed_until, value.observed_until),
                        (&mut t.observed_elapsed, value.observed_elapsed),
                        (&mut t.observed_work, value.observed_work),
                    ] {
                        if let (Some(dst), Some(src)) = (dst, src) {
                            extend(dst, src.min);
                            extend(dst, src.max);
                        }
                    }
                } else {
                    self.terminals[k].insert(kind, value);
                }
            }
        }
        for (a, n) in m.actors.iter().zip(counts) {
            if n > 0 {
                self.overcommit |= n > a.capacity.ok_or("capacity-less execution")?;
            }
        }
        Ok(())
    }
}

fn arrivals(m: &ir::Model) -> EngineResult<Vec<u64>> {
    validate(m)?;
    if m.bounds.cases == 0 || m.verify.is_empty() {
        return Err("invalid multi-case bounds".into());
    }
    let a = match &m.arrivals {
        Some(a) => a.clone(),
        None if m.bounds.cases == 1 => vec![0],
        _ => return Err("multiple cases require arrivals".into()),
    };
    if a.is_empty() || a.len() as u64 != m.bounds.cases || a.windows(2).any(|p| p[0] > p[1]) {
        return Err("invalid arrival list".into());
    }
    if m.bounds.cases >= 2
        && (m.state_variables.iter().any(|v| {
            m.is_shared_latent(v.kind) && !matches!(v.initial, ir::InitialValue::Fixed(_))
        }) || m.capabilities.iter().any(|c| {
            c.writes
                .iter()
                .map(|w| w.variable)
                .chain(c.kernel_writes.iter().copied())
                .any(|i| m.is_shared_latent(m.state_variables[i.0].kind))
        }))
    {
        return Err("multi-case Actor latent state must be fixed and read-only".into());
    }
    Ok(a)
}

/// A conjunction over joint path states, not a correlation inferred from
/// separately aggregated terminal, receipt, work or effect extrema.
pub(super) fn receipt_unknown_held(m: &ir::Model, command: &str) -> Option<(bool, bool)> {
    let times = arrivals(m).ok()?;
    let (call, rule) = m
        .response_attempts()
        .into_iter()
        .find(|(_, r)| m.site_name(r.command) == command && r.max_attempts == 1)?;
    if rule.loss != ir::ResponseLoss::Sequence(vec![crate::ast::Receipt::Unknown]) {
        return None;
    }
    let mut facts = Facts::new(times.len());
    let nodes = graph(m, &times, DEFAULT_STATE_BUDGET, &mut facts).ok()?;
    if facts.truncated || facts.horizon || facts.per.iter().any(|e| !e.ranges.is_empty()) {
        return None;
    }
    let (mut reached, mut held) = (false, true);
    for node in nodes.iter().filter(|n| n.actions.is_empty() && !n.unknown) {
        for slot in &node.state.cases {
            if let Some(c) = slot.payload()
                && let Some(h) = &c.local.response
            {
                for row in h
                    .rows
                    .iter()
                    .filter(|row| row.command == rule.command && row.binding == call.binding)
                {
                    reached = true;
                    held &= row.receipt == crate::ast::Receipt::Unknown
                        && row.effect_retained
                        && h.stopped
                        && c.terminal.is_none()
                        && c.local.results[rule.command.0].is_none()
                        && c.local.results[call.binding.0].is_none();
                }
            }
        }
    }
    Some((reached, held))
}

pub(super) fn validate_scenario(m: &ir::Model) -> EngineResult<()> {
    if m.arrival_distribution.is_some() {
        return Err("stochastic_arrivals_mode_unsupported".into());
    }
    if let Some(t) = &m.resource_time {
        t.validate(m)?;
    }
    let a = m.arrivals.as_deref().unwrap_or(&[0]);
    if m.scenario.as_ref().is_some_and(|s| s.name.is_empty())
        || m.horizon
            .is_some_and(|d| d == 0 || m.scenario.is_none() || a.iter().any(|t| *t > d))
    {
        return Err("invalid scenario horizon".into());
    }
    if let Some(e) = &m.arrival_every
        && (m.arrivals.is_none()
            || e.every == 0
            || e.from >= e.until
            || e.count > 10000
            || e.count != 1 + (e.until - e.from - 1) / e.every
            || e.count != m.bounds.cases
            || e.count != a.len() as u64
            || a.iter().enumerate().any(|(i, t)| {
                e.every
                    .checked_mul(i as u64)
                    .and_then(|v| e.from.checked_add(v))
                    != Some(*t)
            }))
    {
        return Err("inconsistent fixed_every record".into());
    }
    Ok(())
}
fn initial(m: &ir::Model, n: usize) -> Global {
    Global {
        external_peak: m.external_wait().then_some(0),
        metric_peak: m.metrics.as_ref().map(|_| metrics::Peak::new(m)),
        time: 0,
        cursor: 0,
        phase: Phase::Duration,
        cases: vec![Slot::Future; n],
        shared: m
            .state_variables
            .iter()
            .map(|v| match (v.kind, v.initial) {
                (_, ir::InitialValue::Fixed(x)) if m.is_shared_state(v.kind) => x,
                _ => ir::StateValue::None,
            })
            .collect(),
    }
}
type Action = Vec<(Rational, Global)>;
fn one(s: Global) -> Vec<Action> {
    vec![vec![(Rational::one(), s)]]
}
fn next_phase(s: &Global, p: Phase, r: &mut dyn Generation) -> EngineResult<Vec<Action>> {
    r.take(1)?;
    let mut s = s.clone();
    s.phase = p;
    Ok(one(s))
}

// One product, reserved in its entirety before any successor clone. Distribution
// weights are not scheduler weights; every nondet tuple is a separate action.
fn domain_support(
    m: &ir::Model,
    v: &ir::StateVariable,
    r: &mut dyn Generation,
    k: usize,
    i: usize,
) -> EngineResult<Vec<(ir::StateValue, Rational)>> {
    if v.integer_max.is_some() {
        let count = v.cardinality(&m.types);
        if let Some(sample) = r.sampling()
            && count > 1
        {
            sample.note_initial(m, i, k as u64 + 1);
            return sample.reject();
        }
        if count > r.remaining() as u128 {
            return Err(Halt::Budget);
        }
    }
    Ok(v.domain(&m.types).map(|x| (x, Rational::one())).collect())
}
fn initialize(
    s: &Global,
    m: &ir::Model,
    r: &mut dyn Generation,
    f: &mut Facts,
    chance: bool,
) -> EngineResult<Vec<Action>> {
    let mut vars = Vec::new();
    for (k, slot) in s.cases.iter().enumerate() {
        if slot.payload().is_some_and(|c| !c.received) {
            for (i, v) in m.state_variables.iter().enumerate() {
                if m.is_shared_state(v.kind) && m.bounds.cases != 1 {
                    continue;
                }
                let support: Vec<(ir::StateValue, Rational)> = match v.initial {
                    ir::InitialValue::Distribution(d) if chance => m.initial_distributions[d]
                        .iter()
                        .filter(|(_, p)| !p.is_zero())
                        .cloned()
                        .collect(),
                    ir::InitialValue::Nondet if !chance => domain_support(m, v, r, k, i)?,
                    _ => continue,
                };
                vars.push((k, i, support));
            }
        }
    }
    if let Some(sample) = r.sampling() {
        let mut n = s.clone();
        n.phase = if chance {
            Phase::Nondet
        } else {
            Phase::Receipt
        };
        if !chance {
            for (k, i, xs) in &vars {
                if xs.iter().any(|(x, _)| *x != xs[0].0) {
                    sample.note_initial(m, *i, *k as u64 + 1);
                }
            }
            if !sample.provenance.is_empty() {
                return sample.reject();
            }
        }
        for (k, i, xs) in &vars {
            let choice = if chance {
                let (case, site) = simulation::Sample::initial_site(m, *i, *k as u64 + 1);
                sample.choose(
                    case,
                    &site,
                    1,
                    &xs.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(),
                )?
            } else {
                if xs.iter().any(|(x, _)| *x != xs[0].0) {
                    sample.note_initial(m, *i, *k as u64 + 1);
                    return sample.reject();
                }
                0
            };
            let x = xs.get(choice).ok_or("empty initial support")?.0;
            if m.is_shared_state(m.state_variables[*i].kind) {
                n.shared[*i] = x;
            } else if let Some(c) = n.cases[*k].payload_mut() {
                c.local.state_values[*i] = x;
            }
        }
        return Ok(one(n));
    }
    let count = vars
        .iter()
        .try_fold(
            1usize,
            |n, (_, _, xs): &(usize, usize, Vec<(ir::StateValue, Rational)>)| {
                n.checked_mul(xs.len())
            },
        )
        .ok_or(Halt::Budget)?;
    r.take(count)?;
    let mut choices = Vec::new();
    for code in 0..count {
        let mut digits = code;
        let mut p = Rational::one();
        let mut assignments = Vec::new();
        for (k, i, xs) in &vars {
            let (x, q) = &xs[digits % xs.len()];
            digits /= xs.len();
            p.0 *= &q.0;
            if !p.within_bits(4096) {
                f.numeric = true;
                if f.trace.is_some() {
                    f.trace_partial_actions
                        .extend(choices.into_iter().map(|(_, state)| state));
                }
                return Err(Halt::Budget);
            }
            assignments.push((*k, *i, *x));
        }
        if !p.within_bits(4096) {
            f.numeric = true;
            if f.trace.is_some() {
                f.trace_partial_actions
                    .extend(choices.into_iter().map(|(_, state)| state));
            }
            return Err(Halt::Budget);
        }
        #[cfg(test)]
        {
            f.clones += 1;
        }
        let mut n = s.clone();
        n.phase = if chance {
            Phase::Nondet
        } else {
            Phase::Receipt
        };
        for (k, i, x) in assignments {
            if m.is_shared_state(m.state_variables[i].kind) {
                n.shared[i] = x
            } else if let Some(c) = n.cases[k].payload_mut() {
                c.local.state_values[i] = x
            }
        }
        choices.push((p, n));
    }
    if chance {
        f.chance |= !vars.is_empty();
        Ok(vec![choices])
    } else {
        Ok(choices.into_iter().map(|x| vec![x]).collect())
    }
}
fn duration(
    s: &Global,
    m: &ir::Model,
    r: &mut dyn Generation,
    f: &mut Facts,
) -> EngineResult<Vec<Action>> {
    let mut coins = Vec::new();
    for (k, slot) in s.cases.iter().enumerate() {
        if let Slot::Admitted(c) = slot {
            for (j, t) in c.local.tasks.iter().enumerate() {
                if let Lifecycle::DistributedRunning {
                    started,
                    next,
                    due: false,
                    ..
                } = t.lifecycle
                {
                    let support: Vec<_> = m.capabilities[t.capability.0]
                        .duration_distribution
                        .as_ref()
                        .ok_or("missing duration")?
                        .entries
                        .iter()
                        .filter(|(_, p)| !p.is_zero())
                        .collect();
                    let (at, p) = support.get(next).ok_or("invalid duration cursor")?;
                    if started.checked_add(*at).ok_or("finish time overflow")? != s.time {
                        continue;
                    }
                    let mut residual = Rational::zero();
                    for (_, q) in &support[next..] {
                        residual.0 += &q.0;
                        if !residual.within_bits(4096) {
                            f.numeric = true;
                            return Err(Halt::Budget);
                        }
                    }
                    let q = duration_hazard(p, &residual, 4096).ok_or_else(|| {
                        f.numeric = true;
                        Halt::Budget
                    })?;
                    coins.push((k, j, q));
                }
            }
        }
    }
    if coins.is_empty() {
        return next_phase(s, Phase::Admission, r);
    }
    if let Some(sample) = r.sampling() {
        let mut n = s.clone();
        n.phase = Phase::Admission;
        for (k, j, q) in &coins {
            let Slot::Admitted(c) = &s.cases[*k] else {
                unreachable!()
            };
            let t = &c.local.tasks[*j];
            let Lifecycle::DistributedRunning { next, .. } = t.lifecycle else {
                unreachable!()
            };
            let site = simulation::Sample::task_site(m, t.binding, "duration");
            let finish = sample.choose(
                *k as u64 + 1,
                &site,
                next + 1,
                &[q.clone(), Rational(Rational::one().0 - &q.0)],
            )? == 0;
            if let Slot::Admitted(c) = &mut n.cases[*k]
                && let Lifecycle::DistributedRunning { next, due, .. } =
                    &mut c.local.tasks[*j].lifecycle
            {
                *due = finish;
                if !finish {
                    *next += 1;
                }
            }
        }
        return Ok(one(n));
    }
    let count = coins
        .iter()
        .try_fold(1usize, |n, (_, _, q)| {
            n.checked_mul(if *q == Rational::one() { 1 } else { 2 })
        })
        .ok_or(Halt::Budget)?;
    r.take(count)?;
    let mut action = Vec::new();
    for code in 0..count {
        let mut code = code;
        let mut p = Rational::one();
        let mut decisions = Vec::new();
        for (k, j, q) in &coins {
            let hit = *q == Rational::one() || code % 2 == 0;
            if *q != Rational::one() {
                code /= 2;
            }
            p.0 *= if hit {
                q.0.clone()
            } else {
                Rational::one().0 - &q.0
            };
            if !p.within_bits(4096) {
                f.numeric = true;
                if f.trace.is_some() {
                    f.trace_partial_actions
                        .extend(action.into_iter().map(|(_, state)| state));
                }
                return Err(Halt::Budget);
            }
            decisions.push((*k, *j, hit));
        }
        if !p.within_bits(4096) {
            f.numeric = true;
            if f.trace.is_some() {
                f.trace_partial_actions
                    .extend(action.into_iter().map(|(_, state)| state));
            }
            return Err(Halt::Budget);
        }
        #[cfg(test)]
        {
            f.clones += 1;
        }
        let mut n = s.clone();
        n.phase = Phase::Admission;
        for (k, j, hit) in decisions {
            if let Slot::Admitted(c) = &mut n.cases[k]
                && let Lifecycle::DistributedRunning { due, next, .. } =
                    &mut c.local.tasks[j].lifecycle
            {
                *due = hit;
                if !hit {
                    *next += 1;
                }
            }
        }
        action.push((p, n));
    }
    f.chance = true;
    Ok(vec![action])
}

fn transition(
    s: &Global,
    m: &ir::Model,
    a: &[u64],
    program: &[Instruction],
    entry: usize,
    r: &mut dyn Generation,
    f: &mut Facts,
) -> EngineResult<Option<Vec<Action>>> {
    let result = transition_inner(s, m, a, program, entry, r, f);
    if result.is_err()
        && m.contracts
            .iter()
            .any(|c| matches!(c.kind, ir::ContractKind::StateInvariant { .. }))
    {
        for k in 0..f.per.len() {
            if let Some(generated) = f.per[k].state_invariant_aborted_bundle.take() {
                f.state_invariant_generated_prefix(s, k, &generated, m)?;
            }
            let partial = std::mem::take(&mut f.per[k].partial_successors);
            for generated in &partial {
                f.state_invariant_generated_prefix(s, k, generated, m)?;
            }
            f.per[k].partial_successors = partial;
        }
    }
    let mut actions = result?;
    for (_, next) in actions.iter_mut().flatten().flatten() {
        next.normalize_state_invariants(m)?;
    }
    Ok(actions)
}

fn transition_inner(
    s: &Global,
    m: &ir::Model,
    a: &[u64],
    program: &[Instruction],
    entry: usize,
    r: &mut dyn Generation,
    f: &mut Facts,
) -> EngineResult<Option<Vec<Action>>> {
    let ranges_before: usize = f.per.iter().map(|e| e.range_attempts).sum();
    if let Some(sample) = r.sampling() {
        sample.priority_tie = false;
    }
    match s.phase {
        Phase::Duration => return duration(s, m, r, f).map(Some),
        Phase::Admission => {
            r.take(1)?;
            let mut n = s.clone();
            while n.cursor < a.len() && a[n.cursor] == s.time {
                let k = n.cursor;
                n.cursor += 1;
                let room = n.room(m);
                let overflow = m
                    .admission_queue
                    .or(m.concurrent_cases)
                    .map_or(ir::Overflow::Reject, |q| q.overflow);
                if !room && overflow == ir::Overflow::Reject {
                    n.cases[k] = Slot::Rejected;
                    continue;
                }
                let mut local = State::initial(m, entry);
                local.time = s.time;
                local.initialized = true;
                for (i, v) in m.state_variables.iter().enumerate() {
                    if m.is_shared_state(v.kind) {
                        local.state_values[i] = ir::StateValue::None;
                    }
                }
                let c = Case {
                    local,
                    queued_at: Vec::new(),
                    ever_started: false,
                    received: false,
                    admitted_at: None,
                    terminal: None,
                }
                .into();
                n.cases[k] = if room {
                    Slot::Admitted(c)
                } else {
                    match overflow {
                        ir::Overflow::WaitExternalFifo | ir::Overflow::WaitExternalPriority(_) => {
                            Slot::Held(c)
                        }
                        ir::Overflow::End(_) => Slot::Overflow(c),
                        ir::Overflow::Reject => unreachable!(),
                    }
                };
            }
            if let Some(peak) = &mut n.external_peak {
                *peak = (*peak).max(
                    n.cases
                        .iter()
                        .filter(|c| matches!(c, Slot::Held(_)))
                        .count() as u64,
                );
            }
            n.phase = Phase::Chance;
            return Ok(Some(one(n)));
        }
        Phase::Chance => return initialize(s, m, r, f, true).map(Some),
        Phase::Nondet => return initialize(s, m, r, f, false).map(Some),
        Phase::Receipt => {
            r.take(1)?;
            let mut n = s.clone();
            for k in 0..n.cases.len() {
                if n.cases[k].payload().is_some_and(|c| !c.received) {
                    let mut v = n.view(k, m)?;
                    f.local_using(k, |e| emit_in(&mut v, Bundle::Received, None, m, e, k))?;
                    n.store(k, v, m);
                    if let Some(c) = n.cases[k].payload_mut() {
                        c.received = true;
                    }
                }
            }
            n.phase = if m.explicit_limits() {
                Phase::Activate
            } else {
                Phase::Mixed
            };
            return Ok(Some(one(n)));
        }
        Phase::Activate => {
            r.take(1)?;
            let mut n = s.clone();
            for k in 0..n.cases.len() {
                if matches!(&n.cases[k], Slot::Admitted(c) if c.admitted_at.is_none()) {
                    let mut v = n.view(k, m)?;
                    f.local_using(k, |e| emit_in(&mut v, Bundle::Admitted, None, m, e, k))?;
                    n.store(k, v, m);
                    n.cases[k].payload_mut().unwrap().admitted_at = Some(s.time);
                } else if matches!(&n.cases[k], Slot::Overflow(c) if c.terminal.is_none()) {
                    let end = m.overflow_end.ok_or("missing overflow terminal")?;
                    let mut v = n.view(k, m)?;
                    f.local_using(k, |e| emit_in(&mut v, Bundle::End(end), None, m, e, k))?;
                    if let Some(contest) = &mut v.contest {
                        contest.business = Some(BusinessEnd {
                            end,
                            at: s.time,
                            work: 0,
                            result_at: None,
                        });
                        for route in &mut contest.routes {
                            route.filed = Some(false);
                        }
                    }
                    close_responses(&mut v);
                    f.per[k].contracts(&v);
                    f.per[k].collect_debts(&v, m, "terminal_unresolved");
                    n.store(k, v, m);
                    n.cases[k].payload_mut().unwrap().terminal = Some(end);
                }
            }
            n.phase = Phase::Mixed;
            return Ok(Some(one(n)));
        }
        Phase::Mixed => {}
    }
    // Stabilize case-local management before global dispatch. No completion,
    // adoption or Start bundle is hidden in this closure.
    let mut actions = Vec::new();
    for k in 0..s.cases.len() {
        let Slot::Admitted(c) = &s.cases[k] else {
            continue;
        };
        if c.local.response.as_ref().is_some_and(|h| h.stopped) {
            continue;
        }
        if c.terminal.is_some() || crate::contest::has_effects(m) {
            if !c.stopped() {
                let v = s.view(k, m)?;
                if let Some(sample) = r.sampling() {
                    sample.case = k as u64 + 1;
                }
                let control = f.local_using(k, |e| contest_control(&v, m, r, e))?;
                if let Some(Step::Next(xs)) = control {
                    for action in probability_actions(xs, &mut f.per[k])? {
                        let mut transported = Vec::new();
                        for (p, v) in action {
                            let mut n = s.clone();
                            n.store(k, v, m);
                            if let Slot::Admitted(c) = &mut n.cases[k] {
                                let tasks = c.local.tasks.len();
                                c.queued_at.resize(tasks, s.time);
                            }
                            transported.push((p, n));
                        }
                        actions.push(transported);
                    }
                    // Preserve a configured probability action, and legacy
                    // nondeterministic alternatives. Settle before dispatch.
                    return Ok(Some(actions));
                }
            }
            if c.terminal.is_some() {
                continue;
            }
        }
        let mut v = s.view(k, m)?;
        let mut terminal = None;
        match &program[v.pc] {
            Instruction::Receipt {
                binding,
                rule,
                retry,
                failure,
                recovery,
                next,
            } => {
                response::advance(&mut v, *binding, rule, *retry, *failure, *next)?;
                recovery::enter(&mut v, rule.command, *recovery)?;
            }
            Instruction::ResolveEffect {
                command,
                evidence,
                site,
                next,
            } => {
                recovery::resolve(&mut v, *command, *evidence, *site, m, k)?;
                v.pc = *next;
            }
            Instruction::ReceiptUnknown => recovery::stop(&mut v)?,
            Instruction::Await { condition, next } => {
                let true_now = condition_at(condition, &v.results, &v.state_values, k)?;
                if true_now {
                    if m.reads_shared_observed(condition) {
                        continue;
                    }
                    if let Some(w) = v
                        .metric_history
                        .as_mut()
                        .and_then(|h| h.conditions.as_mut())
                        .and_then(|ws| ws.last_mut())
                        && w.1.is_none()
                    {
                        w.1 = Some(s.time);
                    }
                    v.pc = *next;
                } else {
                    let Some(waits) = v
                        .metric_history
                        .as_mut()
                        .and_then(|h| h.conditions.as_mut())
                    else {
                        continue;
                    };
                    if waits.last().is_some_and(|w| w.1.is_none()) {
                        continue;
                    }
                    waits.push((s.time, None));
                }
            }
            Instruction::Sleep { duration, next, .. } => match v.sleep_until {
                None => {
                    let wake = s.time.checked_add(*duration).ok_or("wait time overflow")?;
                    v.sleep_until = Some(wake);
                    if let Some(waits) = v.metric_history.as_mut().and_then(|h| h.waits.as_mut()) {
                        waits.push((s.time, wake));
                    }
                }
                Some(wake) if wake <= s.time => {
                    v.sleep_until = None;
                    v.pc = *next;
                }
                Some(_) => continue,
            },
            Instruction::Branch {
                condition: c,
                yes,
                no,
                span,
                site,
            } => {
                if m.reads_shared_observed(c) {
                    continue;
                }
                let selected = condition_at(c, &v.results, &v.state_values, k)?;
                f.per[k].observed.branch(*span, site, selected);
                v.pc = if selected { *yes } else { *no }
            }
            Instruction::End(e) => {
                if m.recovery().is_some()
                    && v.response
                        .as_ref()
                        .is_some_and(|h| !h.unresolved.is_empty())
                {
                    return Err("invalid_recovery_ir".into());
                }
                terminal = Some(*e);
            }
            Instruction::Wait { calls, .. } if !v.waiting => {
                for call in calls {
                    response::capture(&mut v, call, m, k)?;
                    recovery::queued(&mut v, call);
                }
                v.tasks.extend(calls.iter().map(|c| Task {
                    frozen_performance: None,
                    stale: Vec::new(),
                    executor: None,
                    binding: c.binding,
                    capability: c.capability,
                    lifecycle: Lifecycle::Queued,
                }));
                v.waiting = true;
            }
            Instruction::Wait { join, next, .. }
                if v.tasks
                    .iter()
                    .filter(|t| m.site_contest(t.binding).is_none())
                    .all(|t| t.lifecycle == Lifecycle::Done) =>
            {
                if let ir::Join::Any {
                    matches, if_none, ..
                } = join
                    && v.adopted.is_none()
                {
                    if matches
                        .iter()
                        .any(|t| v.results[t.binding.0] == Some(t.value))
                    {
                        continue;
                    }
                    terminal = Some(*if_none);
                }
                if terminal.is_none() {
                    v.tasks.retain(|t| m.site_contest(t.binding).is_some());
                    v.waiting = false;
                    v.adopted = None;
                    v.pc = *next;
                }
            }
            _ => continue,
        }
        r.take(1)?;
        let mut n = s.clone();
        if let Some(e) = terminal {
            f.local_using(k, |facts| {
                emit_in(&mut v, Bundle::End(e), None, m, facts, k)
            })?;
            close_responses(&mut v);
            f.per[k].contracts(&v);
            f.per[k].collect_debts(&v, m, "terminal_unresolved");
            if let Some(contest) = &mut v.contest {
                contest.business = Some(BusinessEnd {
                    end: e,
                    at: s.time,
                    work: v.work,
                    result_at: v.result_at,
                });
                for (id, d) in m.contests().iter().enumerate() {
                    if let crate::contest::ContestTarget::End(target) = d.target {
                        if target == e {
                            contest.routes[id].target_at = Some(s.time);
                        } else {
                            contest.routes[id].filed = Some(false);
                        }
                    }
                }
                for (i, clause) in m.contracts.iter().enumerate() {
                    if let ir::ContractKind::ContestResponse { contest: id } = clause.kind {
                        v.monitors[i].activated |= m.contests()[id].target.end() == Some(e);
                    }
                }
                v.tasks.retain(|t| m.site_contest(t.binding).is_some());
                v.adopted = None;
                v.waiting = true;
            }
            capture_end_open(&mut v, m);
        }
        n.store(k, v, m);
        if let Slot::Admitted(c) = &mut n.cases[k] {
            c.terminal = terminal;
            c.queued_at = c
                .local
                .tasks
                .iter()
                .map(|t| {
                    s.cases
                        .get(k)
                        .and_then(|slot| {
                            if let Slot::Admitted(old) = slot {
                                old.local
                                    .tasks
                                    .iter()
                                    .position(|x| x.binding == t.binding)
                                    .map(|j| old.queued_at[j])
                            } else {
                                None
                            }
                        })
                        .unwrap_or(s.time)
                })
                .collect();
        }
        return Ok(Some(one(n)));
    }
    let mut counts = vec![0u64; m.actors.len()];
    let mut busy = BTreeSet::new();
    for slot in &s.cases {
        if let Slot::Admitted(c) = slot {
            for t in &c.local.tasks {
                if t.lifecycle.running() {
                    counts[m.capabilities[t.capability.0].actor.0] += 1;
                    if let Some(p) = t.executor {
                        busy.insert(p);
                    }
                }
            }
        }
    }
    let fixed = if m
        .start_policy
        .as_ref()
        .is_some_and(ir::StartPolicy::extended)
    {
        Some(s.fixed_starts(m)?)
    } else {
        None
    };
    let mut policy_choices = 0usize;
    let mut priority_task_choices = 0usize;
    let generated = (|| -> EngineResult<()> {
        for k in 0..s.cases.len() {
            let Slot::Admitted(c) = &s.cases[k] else {
                continue;
            };
            if c.stopped() {
                continue;
            }
            let v = s.view(k, m)?;
            if c.terminal.is_none()
                && let Instruction::Await { condition, next } = &program[v.pc]
                && m.reads_shared_observed(condition)
                && condition_at(condition, &v.results, &v.state_values, k)?
            {
                r.take(1)?;
                let mut local = v.clone();
                if let Some(w) = local
                    .metric_history
                    .as_mut()
                    .and_then(|h| h.conditions.as_mut())
                    .and_then(|ws| ws.last_mut())
                    && w.1.is_none()
                {
                    w.1 = Some(s.time);
                }
                local.pc = *next;
                let mut n = s.clone();
                n.store(k, local, m);
                actions.push(vec![(Rational::one(), n)]);
                if let Some(sample) = r.sampling() {
                    sample.unique(&actions)?;
                }
            }
            if c.terminal.is_none()
                && let Instruction::Branch {
                    condition: guard,
                    yes,
                    no,
                    span,
                    site,
                } = &program[v.pc]
                && m.reads_shared_observed(guard)
            {
                r.take(1)?;
                let mut v = v.clone();
                let selected = condition_at(guard, &v.results, &v.state_values, k)?;
                f.per[k].observed.branch(*span, site, selected);
                v.pc = if selected { *yes } else { *no };
                let mut n = s.clone();
                n.store(k, v, m);
                actions.push(vec![(Rational::one(), n)]);
                if let Some(sample) = r.sampling() {
                    sample.unique(&actions)?;
                }
            }
            if let Some(sample) = r.sampling() {
                sample.case = k as u64 + 1;
            }
            let mut due = Vec::new();
            for (j, t) in v.tasks.iter().enumerate() {
                match t.lifecycle {
                    Lifecycle::Running { finish, ack }
                        if finish == s.time || ack == Some(s.time) =>
                    {
                        due.push(Due { i: j, finish, ack })
                    }
                    Lifecycle::DistributedRunning { due: true, ack, .. } => due.push(Due {
                        i: j,
                        finish: s.time,
                        ack,
                    }),
                    Lifecycle::DistributedRunning { ack: Some(at), .. } if at == s.time => due
                        .push(Due {
                            i: j,
                            finish: s.time.wrapping_add(1),
                            ack: Some(at),
                        }),
                    _ => {}
                }
            }
            if c.terminal.is_none()
                && due.is_empty()
                && v.adopted.is_none()
                && let Instruction::Wait {
                    join:
                        ir::Join::Any {
                            matches,
                            tie,
                            remaining,
                            ..
                        },
                    ..
                } = &program[v.pc]
            {
                let candidates: Vec<_> = matches
                    .iter()
                    .filter(|t| v.results[t.binding.0] == Some(t.value))
                    .map(|t| t.binding)
                    .collect();
                if !candidates.is_empty() {
                    let winners = match tie {
                        ir::Tie::Nondet => candidates,
                        ir::Tie::By(order) => vec![
                            *order
                                .iter()
                                .find(|id| candidates.contains(id))
                                .ok_or("invalid tie order")?,
                        ],
                    };
                    f.policy_unresolved |= matches!(tie, ir::Tie::Nondet) && winners.len() > 1;
                    for v in adopt(&v, &winners, *remaining, m, r)? {
                        let mut n = s.clone();
                        n.store(k, v, m);
                        actions.push(vec![(Rational::one(), n)]);
                        if let Some(sample) = r.sampling() {
                            sample.unique(&actions)?;
                        }
                    }
                }
            }
            for mut d in due {
                // Only the normal finish is ordered. An independently due ack
                // remains a successor even when that same Task's finish waits.
                if d.finish == s.time {
                    let (allowed, unresolved) = s.version_finish_order(k, d.i, m);
                    f.policy_unresolved |= unresolved;
                    if let Some(sample) = r.sampling() {
                        sample.priority_tie |= unresolved;
                    }
                    if !allowed {
                        if d.ack != Some(s.time) {
                            continue;
                        }
                        d.finish = s.time ^ 1;
                    }
                }
                let task = &v.tasks[d.i];
                if d.finish == s.time && f.local_using(k, |e| range_in(&v, task, false, m, e, k))? {
                    if d.ack == Some(s.time) {
                        for local in f.local_using(k, |e| {
                            complete_in(
                                &v,
                                Due {
                                    finish: s.time ^ 1,
                                    ..d
                                },
                                m,
                                r,
                                e,
                                k,
                            )
                        })? {
                            let mut n = s.clone();
                            n.store(k, local, m);
                            actions.push(vec![(Rational::one(), n)]);
                        }
                    }
                    continue;
                }
                if let Some(sample) = r.sampling() {
                    sample.note_task(m, task.binding, false);
                }
                if let Some(kernel) = super::work_profiles::kernel(m, task)? {
                    if d.finish == s.time {
                        let action = if let Some(sample) = r.sampling() {
                            sample.kernels.push((actions.len(), d.i));
                            sample.sites.push((
                                actions.len(),
                                simulation::Sample::task_site(m, task.binding, "kernel"),
                                k as u64 + 1,
                            ));
                            vec![(Rational::one(), s.clone())]
                        } else {
                            let local_actions = f.local_using(k, |e| {
                                response::kernel_actions(&v, d.i, kernel, m, r, e, k)
                            })?;
                            f.chance = true;
                            let mut converted = Vec::new();
                            for xs in local_actions {
                                let mut action = Vec::new();
                                for (p, v) in xs {
                                    let mut n = s.clone();
                                    n.store(k, v, m);
                                    action.push((p, n));
                                }
                                converted.push(action);
                            }
                            if converted.len() > 1 {
                                actions.extend(converted);
                                continue;
                            }
                            converted.pop().ok_or("missing kernel response action")?
                        };
                        actions.push(action);
                        if let Some(sample) = r.sampling() {
                            sample.unique(&actions)?;
                        }
                    }
                    if d.ack == Some(s.time) {
                        r.take(1)?;
                        let mut v = v.clone();
                        v.tasks[d.i].lifecycle = Lifecycle::Done;
                        if m.tracks_obligations() {
                            f.local_using(k, |e| {
                                emit_in(
                                    &mut v,
                                    Bundle::CancelAck(task.capability, task.executor),
                                    Some(task.binding),
                                    m,
                                    e,
                                    k,
                                )
                            })?;
                        }
                        let mut n = s.clone();
                        n.store(k, v, m);
                        actions.push(vec![(Rational::one(), n)]);
                        if let Some(sample) = r.sampling() {
                            sample.unique(&actions)?;
                        }
                    }
                } else {
                    f.per[k].chance_groups.clear();
                    let xs = f.local_using(k, |e| complete_in(&v, d, m, r, e, k))?;
                    for action in probability_actions(xs, &mut f.per[k])? {
                        let mut global_action = Vec::new();
                        for (p, local) in action {
                            let mut n = s.clone();
                            n.store(k, local, m);
                            global_action.push((p, n));
                        }
                        actions.push(global_action);
                        if let Some(sample) = r.sampling() {
                            sample.unique(&actions)?;
                        }
                    }
                }
            }
            for (j, t) in v.tasks.iter().enumerate() {
                if fixed.as_ref().is_some_and(|cs| !cs.contains_key(&(k, j))) {
                    continue;
                }
                if t.lifecycle != Lifecycle::Queued || !s.fifo_head(k, j, m) {
                    continue;
                }
                let cap = &m.capabilities[t.capability.0];
                let actor = &m.actors[cap.actor.0];
                if counts[cap.actor.0] >= actor.capacity.ok_or("capacity-less execution")? {
                    continue;
                }
                if !resources_fit(cap, &s.resource_occupancy(m)?, m) {
                    continue;
                }
                if !s.calendar_fit(cap, &counts, &s.resource_occupancy(m)?, m) {
                    continue;
                }
                let people: Vec<_> = if let Some(cs) = &fixed {
                    cs[&(k, j)].clone()
                } else {
                    match &actor.executors {
                        Some(ps) => ps
                            .iter()
                            .filter(|p| {
                                cap.qualified_executors
                                    .as_ref()
                                    .is_none_or(|qs| qs.contains(p))
                                    && !busy.contains(p)
                                    && s.on_shift(**p, m)
                            })
                            .map(|p| Some(*p))
                            .collect(),
                        None => vec![None],
                    }
                };
                if m.start_policy
                    .as_ref()
                    .is_some_and(|p| p.controls(cap.actor))
                {
                    if m.start_policy
                        .as_ref()
                        .is_some_and(|p| p.priority.is_some())
                        && !people.is_empty()
                    {
                        priority_task_choices += 1;
                        if let Some(sample) = r.sampling() {
                            sample.priority_tie |= priority_task_choices > 1;
                        }
                    }
                    let refused = cap
                        .requires
                        .as_ref()
                        .map(|req| condition_at(&req.condition, &v.results, &v.state_values, k))
                        .transpose()?
                        .is_some_and(|ok| !ok);
                    policy_choices += if refused {
                        usize::from(!people.is_empty())
                    } else {
                        people.len()
                    };
                    f.policy_unresolved |= policy_choices > 1;
                }
                for executor in people {
                    if let Some(req) = &cap.requires
                        && !condition_at(&req.condition, &v.results, &v.state_values, k)?
                    {
                        r.take(1)?;
                        let mut local = v.clone();
                        local.tasks[j].lifecycle = Lifecycle::Done;
                        if !crate::contest::has_effects(m) || m.site_contest(t.binding).is_none() {
                            local
                                .results
                                .resize(local.results.len().max(t.binding.0 + 1), None);
                            local.results[t.binding.0] = Some(req.otherwise);
                        }
                        f.local_using(k, |e| {
                            record_refusal(
                                &mut local,
                                t.binding,
                                t.capability,
                                req.otherwise,
                                m,
                                e,
                            );
                            Ok(())
                        })?;
                        let mut n = s.clone();
                        n.store(k, local, m);
                        actions.push(vec![(Rational::one(), n)]);
                        if let Some(sample) = r.sampling() {
                            sample.unique(&actions)?;
                        }
                        break;
                    }
                    if let Some(sample) = r.sampling() {
                        sample.note_task(m, t.binding, false);
                    }
                    let lifecycle = if let Some(d) = &cap.duration_distribution {
                        for (t, p) in &d.entries {
                            if !p.is_zero() {
                                s.time.checked_add(*t).ok_or("finish time overflow")?;
                            }
                        }
                        Lifecycle::DistributedRunning {
                            started: s.time,
                            next: 0,
                            due: false,
                            ack: None,
                        }
                    } else {
                        Lifecycle::Running {
                            finish: s
                                .time
                                .checked_add(cap.duration)
                                .ok_or("finish time overflow")?,
                            ack: None,
                        }
                    };
                    if f.local_using(k, |e| range_in(&v, t, true, m, e, k))? {
                        continue;
                    }
                    r.take(1)?;
                    let mut n = s.clone();
                    let before = v.state_values.clone();
                    let mut v = v.clone();
                    v.tasks[j].frozen_performance = s.freeze_performance(k, j, executor, m)?;
                    v.tasks[j].executor = executor;
                    v.tasks[j].lifecycle = lifecycle;
                    v.tasks[j].stale.clear();
                    for w in &cap.start_writes {
                        v.state_values[w.variable.0] = write_value(w, &before, None, k, executor)?;
                    }
                    notify_writes(&mut v, Some(t.binding), &cap.start_writes, m);
                    f.local_using(k, |e| {
                        emit_in(
                            &mut v,
                            Bundle::Start(t.capability, executor),
                            Some(t.binding),
                            m,
                            e,
                            k,
                        )
                    })?;
                    n.store(k, v, m);
                    if let Slot::Admitted(c) = &mut n.cases[k]
                        && c.terminal.is_none()
                    {
                        c.ever_started = true;
                    }
                    actions.push(vec![(Rational::one(), n)]);
                    if let Some(sample) = r.sampling() {
                        sample.unique(&actions)?;
                    }
                }
            }
        }
        Ok(())
    })();
    if let Err(halt) = generated {
        if f.trace.is_some() {
            f.trace_partial_actions
                .extend(actions.into_iter().flatten().map(|(_, state)| state));
        }
        return Err(halt);
    }
    if !actions.is_empty() || f.per.iter().map(|e| e.range_attempts).sum::<usize>() != ranges_before
    {
        return Ok(Some(actions));
    }
    let mut next = a.get(s.cursor).copied();
    // Only after all immediate management, finishes and starts have settled.
    if let Some(tie) = m.external_priority()
        && s.room(m)
    {
        let ranks = &m.case_priority.as_ref().ok_or("invalid_priority_ir")?.ranks;
        let held: Vec<_> = (0..s.cases.len())
            .filter(|&k| matches!(s.cases[k], Slot::Held(_)))
            .collect();
        let eligible: Vec<_> = held
            .iter()
            .copied()
            .filter(|&k| {
                !held.iter().any(|&other| {
                    ranks[other] > ranks[k]
                        || (ranks[other] == ranks[k]
                            && tie == ir::QueueTie::FifoCaseId
                            && (a[other], other) < (a[k], k))
                })
            })
            .collect();
        f.policy_unresolved |= eligible.len() > 1;
        if let Some(sample) = r.sampling() {
            sample.priority_tie |= eligible.len() > 1;
        }
        if !eligible.is_empty() {
            let mut choices = Vec::new();
            for k in eligible {
                r.take(1)?;
                let mut n = s.clone();
                let Slot::Held(mut c) = std::mem::replace(&mut n.cases[k], Slot::Future) else {
                    unreachable!()
                };
                c.admitted_at = Some(s.time);
                n.cases[k] = Slot::Admitted(c);
                let mut v = n.view(k, m)?;
                f.local_using(k, |e| emit_in(&mut v, Bundle::Admitted, None, m, e, k))?;
                n.store(k, v, m);
                choices.push(vec![(Rational::one(), n)]);
                if let Some(sample) = r.sampling() {
                    sample.unique(&choices)?;
                }
            }
            return Ok(Some(choices));
        }
    }
    let mut readmitted = None;
    // The old atomic FIFO batch is intentionally preserved for old inputs.
    for k in 0..s.cases.len() {
        if m.external_priority().is_some() {
            break;
        }
        if !matches!(s.cases[k], Slot::Held(_)) {
            continue;
        }
        if !readmitted.as_ref().unwrap_or(s).room(m) {
            break;
        }
        if readmitted.is_none() {
            // Charge before cloning or publishing any admission evidence.
            r.take(1)?;
            readmitted = Some(s.clone());
        }
        let n = readmitted.as_mut().unwrap();
        let Slot::Held(mut c) = std::mem::replace(&mut n.cases[k], Slot::Future) else {
            unreachable!()
        };
        c.admitted_at = Some(s.time);
        n.cases[k] = Slot::Admitted(c);
        let mut v = n.view(k, m)?;
        f.local_using(k, |e| emit_in(&mut v, Bundle::Admitted, None, m, e, k))?;
        n.store(k, v, m);
    }
    if let Some(n) = readmitted {
        return Ok(Some(one(n)));
    }
    for slot in &s.cases {
        if let Slot::Admitted(c) = slot {
            if let Some(wake) = c.local.sleep_until {
                next = Some(next.map_or(wake, |x| x.min(wake)));
            }
            for t in &c.local.tasks {
                let at = match t.lifecycle {
                    Lifecycle::Running { finish, ack } => ack.map_or(finish, |x| x.min(finish)),
                    Lifecycle::DistributedRunning {
                        started,
                        next,
                        due: false,
                        ack,
                    } => {
                        let d = m.capabilities[t.capability.0]
                            .duration_distribution
                            .as_ref()
                            .ok_or("missing duration")?;
                        let t = d
                            .entries
                            .iter()
                            .filter(|(_, p)| !p.is_zero())
                            .nth(next)
                            .ok_or("invalid duration cursor")?
                            .0;
                        let end = started.checked_add(t).ok_or("finish time overflow")?;
                        ack.map_or(end, |x| x.min(end))
                    }
                    _ => continue,
                };
                next = Some(next.map_or(at, |x| x.min(at)));
            }
        }
    }
    if let Some(tables) = &m.resource_time {
        let calendar_pending = s.calendar_pending(m)?;
        if calendar_pending {
            let boundary = tables
                .calendars
                .iter()
                .flat_map(|c| &c.intervals)
                .map(|i| i.from)
                .filter(|t| *t > s.time)
                .min();
            // An explicit finite table cannot establish permanent deadlock
            // beyond the computational window. Retain a censor event.
            let at = boundary
                .or_else(|| m.bounds.clock.checked_add(1))
                .ok_or("calendar clock overflow")?;
            next = Some(next.map_or(at, |n| n.min(at)));
        }
    }
    let Some(at) = next else {
        if !m.contests().is_empty() {
            for (k, slot) in s.cases.iter().enumerate() {
                if let Slot::Admitted(c) = slot
                    && c.stopped()
                {
                    f.per[k].collect_debts(&c.local, m, "terminal_unresolved");
                }
            }
        }
        for (k, slot) in s.cases.iter().enumerate() {
            if let Some(c) = slot.payload()
                && !c.stopped()
            {
                f.deadlock = true;
                let mut v = c.local.clone();
                f.per[k].collect_debts(&v, m, "deadlock_unresolved");
                close_responses(&mut v);
                f.per[k].contracts(&v);
            }
        }
        if let Some(trace) = &mut f.trace
            && s.cases
                .iter()
                .any(|slot| slot.payload().is_some_and(|c| !c.stopped()))
        {
            trace.global_leaf(s, m, crate::explain::Kind::Deadlock);
        }
        return Ok(None);
    };
    let next_event = at;
    let horizon = m.horizon.filter(|d| *d <= m.bounds.clock);
    let at = horizon.map_or(at, |d| at.min(d));
    if at > s.time && at <= m.bounds.clock {
        r.take(1)?;
        let mut n = s.clone();
        n.time = at;
        n.phase = Phase::Duration;
        for (k, slot) in n.cases.iter_mut().enumerate() {
            if let Some(c) = slot.payload_mut()
                && !c.stopped()
            {
                let count = c
                    .local
                    .tasks
                    .iter()
                    .filter(|t| {
                        t.lifecycle.running()
                            && (!crate::contest::has_effects(m)
                                || m.site_contest(t.binding).is_none())
                    })
                    .count() as u64;
                c.local.work = c
                    .local
                    .work
                    .checked_add((at - s.time).checked_mul(count).ok_or("work overflow")?)
                    .ok_or("work overflow")?;
                c.local.time = at;
                expire_debts(&mut c.local, at, false);
                refresh_contests(&mut c.local, m, at, false);
                for monitor in &mut c.local.monitors {
                    monitor.violated |= monitor.pending.iter().any(|d| *d < at);
                }
                f.per[k].contracts(&c.local);
            }
        }
        return Ok(Some(one(n)));
    }
    // An event still due at D is not processed by a horizon stop (review m1).
    let horizon_stop = horizon == Some(s.time) && next_event > s.time;
    if at <= s.time && !horizon_stop {
        return Err("non-progressing multi-case clock".into());
    }
    for (k, slot) in s.cases.iter().enumerate() {
        if let Some(c) = slot.payload()
            && !c.stopped()
        {
            let mut v = c.local.clone();
            let limit = if horizon_stop { s.time } else { m.bounds.clock };
            expire_debts(&mut v, limit, true);
            refresh_contests(&mut v, m, limit, true);
            f.per[k].collect_debts(&v, m, if horizon_stop { "horizon" } else { "clock" });
            for monitor in &mut v.monitors {
                monitor.violated |= monitor.pending.iter().any(|d| *d <= limit);
            }
            f.per[k].contracts(&v);
        }
    }
    if let Some(trace) = &mut f.trace {
        trace.global_leaf(
            s,
            m,
            if horizon_stop {
                crate::explain::Kind::Horizon
            } else {
                crate::explain::Kind::Deadline
            },
        );
    }
    Ok(Some(Vec::new())) // Clock cut, distinct from deadlock and budget.
}

fn metric_snapshot(
    s: &Global,
    m: &ir::Model,
    a: &[u64],
    until: u64,
    stopped: bool,
) -> crate::metrics::BusinessMetrics {
    metric_snapshot_with_horizon(s, m, a, until, stopped, false)
}

fn metric_snapshot_with_horizon(
    s: &Global,
    m: &ir::Model,
    a: &[u64],
    until: u64,
    stopped: bool,
    horizon: bool,
) -> crate::metrics::BusinessMetrics {
    let _timer = crate::perf::span("engine.metric_snapshot");
    let cases: Vec<_> = s
        .cases
        .iter()
        .enumerate()
        .map(|(i, slot)| {
            let (state, end, rejected, deadlocked, not_arrived) = match slot {
                Slot::Future => (None, None, false, false, true),
                Slot::Rejected => (None, None, true, false, false),
                Slot::Admitted(c) | Slot::Held(c) | Slot::Overflow(c) => (
                    Some(&c.local),
                    c.terminal,
                    false,
                    stopped && c.terminal.is_none() && !c.stopped(),
                    false,
                ),
            };
            metrics::CaseObservation {
                admitted_at: match slot {
                    Slot::Admitted(c) => {
                        if m.explicit_limits() {
                            c.admitted_at
                        } else {
                            Some(a[i])
                        }
                    }
                    _ => None,
                },
                state,
                arrival: a[i],
                end,
                rejected,
                deadlocked,
                not_arrived,
            }
        })
        .collect();
    let mut result = metrics::snapshot_with_horizon(
        m,
        &cases,
        s.metric_peak.as_ref().expect("enabled peak"),
        until,
        horizon,
    );
    if m.explicit_limits() {
        use crate::metrics as w;
        let number = |x: u64| {
            w::Scalar::Range(w::Range {
                min: Some(x.into()),
                max: Some(x.into()),
                reason: None,
            })
        };
        let count = |x| w::Count {
            stats: number(x),
            observed_total: None,
        };
        let mut admitted = 0;
        let mut overflowed = 0;
        for (i, (row, slot)) in result.cases.iter_mut().zip(&s.cases).enumerate() {
            let at = slot.payload().and_then(|c| c.admitted_at);
            let is_overflow = matches!(slot, Slot::Overflow(_));
            admitted += u64::from(at.is_some());
            overflowed += u64::from(is_overflow);
            row.counts.admitted = Some(count(u64::from(at.is_some())));
            row.counts.overflowed = m.overflow_end.map(|_| count(u64::from(is_overflow)));
            row.external_wait = m.external_wait().then(|| {
                at.map_or_else(
                    || {
                        w::Scalar::Range(w::Range {
                            min: None,
                            max: None,
                            reason: Some(if stopped && matches!(slot, Slot::Held(_)) {
                                w::Reason::Deadlock
                            } else {
                                w::Reason::Unfinished
                            }),
                        })
                    },
                    |at| number(at - a[i]),
                )
            });
        }
        result.overall.counts.admitted = Some(count(admitted));
        result.overall.counts.overflowed = m.overflow_end.map(|_| count(overflowed));
    }
    result
}

fn graph(m: &ir::Model, a: &[u64], budget: usize, f: &mut Facts) -> EngineResult<Vec<Node>> {
    let _timer = crate::perf::span("engine.graph");
    #[cfg(test)]
    if m.quality.as_ref().is_some_and(|q| !q.quantiles.is_empty()) {
        crate::quality::GRAPH_BUILDS.with(|n| n.set(n.get() + 1));
    }
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    let seed = Rc::new(initial(m, a.len()));
    if let Some(trace) = &mut f.trace {
        trace.global_initial(&seed, m);
    }
    let mut trace_ids = f.trace.as_ref().map(|_| vec![0usize]);
    let mut nodes = vec![Node {
        state: seed.clone(),
        actions: Vec::new(),
        unknown: true,
        cut: ProbabilityCut::Pending,
        numeric: false,
    }];
    let mut ids = HashMap::from([(seed, 0)]);
    let mut pending = vec![0];
    if budget == 0 {
        f.mark_cut(&nodes[0].state, true);
        return Ok(nodes);
    }
    let result = (|| -> EngineResult<()> {
        while let Some(id) = pending.pop() {
            if let (Some(trace), Some(ids)) = (&mut f.trace, &trace_ids) {
                trace.begin(ids[id]);
            }
            let s = nodes[id].state.clone();
            if let Some(trace) = &mut f.trace {
                trace.global_context(&s, m);
            }
            if s.live_cut(m) {
                f.collect_prefix(&s, m, "live_tasks")?;
                nodes[id].cut = ProbabilityCut::LiveTasks;
                f.mark_cut(&s, false);
                continue;
            }
            f.observe(&s, m, a)?;
            for e in &mut f.per {
                e.bundle_aborted = false;
            }
            let mut room = Room(budget.saturating_sub(nodes.len()));
            let range_before: usize = f.per.iter().map(|e| e.range_attempts).sum();
            let result = transition(&s, m, a, &program, entry, &mut room, f);
            let range_stop = range_before != f.per.iter().map(|e| e.range_attempts).sum::<usize>();
            if range_stop {
                nodes[id].cut = ProbabilityCut::Range;
            }
            if range_stop {
                // Same payload rule as mark_cut and the reference: a stopped case
                // (including an overflowed one) keeps its finished rows.
                for (k, slot) in s.cases.iter().enumerate() {
                    f.cut[k] |= !matches!(slot, Slot::Rejected)
                        && !slot.payload().is_some_and(Case::stopped);
                }
            }
            #[cfg(test)]
            {
                f.census.2 = f.census.2.max(budget - room.0);
            }
            if result.is_err()
                && let Some(trace) = &mut f.trace
            {
                let mut record = |next: Global| {
                    let node = trace.global_transition(&s, &next, m);
                    if let Some(&existing) = ids.get(&Rc::new(next)) {
                        trace.merge(node, trace_ids.as_ref().expect("trace node IDs")[existing]);
                    }
                };
                for next in std::mem::take(&mut f.trace_partial_actions) {
                    record(next);
                }
                for (k, local) in f.per.iter_mut().enumerate() {
                    for state in std::mem::take(&mut local.partial_successors) {
                        let mut next = (*s).clone();
                        next.store(k, state, m);
                        record(next);
                    }
                }
            }
            let actions = match result {
                Ok(None) => {
                    if m.scenario.is_some() {
                        if let Some(r) = &mut f.observed_until {
                            extend(r, s.time);
                        } else {
                            f.observed_until = Some(Range {
                                min: s.time,
                                max: s.time,
                            });
                        }
                    }
                    if m.metrics.is_some() {
                        metrics::merge(&mut f.metrics, metric_snapshot(&s, m, a, s.time, true));
                    }
                    nodes[id].unknown = false;
                    continue;
                }
                Ok(Some(xs)) if xs.is_empty() => {
                    if range_stop {
                        continue;
                    }
                    if m.horizon == Some(s.time) && s.time <= m.bounds.clock {
                        nodes[id].cut = ProbabilityCut::Horizon;
                        f.horizon = true;
                        if let Some(r) = &mut f.observed_until {
                            extend(r, s.time);
                        } else {
                            f.observed_until = Some(Range {
                                min: s.time,
                                max: s.time,
                            });
                        }
                        // Held cases (ADR 0039 external wait) are received
                        // and unfinished too: cut off like admitted ones.
                        for (k, slot) in s.cases.iter().enumerate() {
                            if let Some(c) = slot.payload() {
                                f.cut[k] |= !c.stopped();
                                if c.terminal.is_none() {
                                    let work = Range {
                                        min: c.local.work,
                                        max: c.local.work,
                                    };
                                    if let Some(t) = f.terminals[k].get_mut(&Kind::CutOff) {
                                        extend(t.work.as_mut().expect("cutoff work"), c.local.work);
                                    } else {
                                        f.terminals[k].insert(
                                            Kind::CutOff,
                                            CaseTerminal {
                                                observed_until: None,
                                                observed_elapsed: None,
                                                observed_work: None,
                                                kind: Kind::CutOff,
                                                completed_at: None,
                                                elapsed: None,
                                                work: Some(work),
                                                result_at: None,
                                                probability: None,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                        if m.metrics.is_some() {
                            metrics::merge(
                                &mut f.metrics,
                                metric_snapshot_with_horizon(&s, m, a, s.time, false, true),
                            );
                        }
                    } else {
                        nodes[id].cut = ProbabilityCut::Clock;
                        f.mark_cut(&s, false);
                    }
                    continue;
                }
                Ok(Some(xs)) => xs,
                Err(Halt::Budget) => {
                    nodes[id].cut = ProbabilityCut::StateBudget;
                    nodes[id].numeric = f.numeric;
                    f.mark_cut(&s, !f.numeric);
                    break;
                }
                Err(e) => {
                    for (k, slot) in s.cases.iter().enumerate() {
                        if slot.payload().is_some() && !f.per[k].bundle_aborted {
                            f.per[k].collect_debts(&s.view(k, m)?, m, "aborted");
                        }
                    }
                    for id in &pending {
                        f.collect_prefix(&nodes[*id].state, m, "aborted")?;
                    }
                    return Err(e);
                }
            };
            nodes[id].unknown = range_stop;
            f.scheduler |= actions.len() > 1;
            for action in actions {
                #[cfg(test)]
                {
                    f.census.1 += action.len();
                }
                let mut edges = BTreeMap::new();
                for (p, s) in action {
                    if p.is_zero() {
                        continue;
                    }
                    let trace_node = f
                        .trace
                        .as_mut()
                        .map(|trace| trace.global_transition(&nodes[id].state, &s, m));
                    let s = Rc::new(s);
                    let j = if let Some(j) = ids.get(&s) {
                        if let (Some(trace), Some(ids), Some(node)) =
                            (&mut f.trace, &trace_ids, trace_node)
                        {
                            trace.merge(node, ids[*j]);
                        }
                        *j
                    } else {
                        let j = nodes.len();
                        if let (Some(ids), Some(node)) = (&mut trace_ids, trace_node) {
                            ids.push(node);
                        }
                        ids.insert(s.clone(), j);
                        pending.push(j);
                        nodes.push(Node {
                            state: s,
                            actions: Vec::new(),
                            unknown: true,
                            cut: ProbabilityCut::Pending,
                            numeric: false,
                        });
                        j
                    };
                    edges.entry(j).or_insert_with(Rational::zero).0 += p.0;
                }
                nodes[id]
                    .actions
                    .push(edges.into_iter().map(|(j, p)| (p, j)).collect());
            }
        }
        // Budget leaves already generated before another branch refused still count
        // as observed evidence, but no unprocessed business/query events are invented.
        for n in &nodes {
            f.observe(&n.state, m, a)?;
            if n.unknown {
                f.collect_prefix(
                    &n.state,
                    m,
                    match n.cut {
                        ProbabilityCut::Range => "state_range_violation",
                        ProbabilityCut::Clock => "clock",
                        ProbabilityCut::Horizon => "horizon",
                        ProbabilityCut::LiveTasks => "live_tasks",
                        ProbabilityCut::Pending | ProbabilityCut::StateBudget => "state_budget",
                    },
                )?;
                if n.cut != ProbabilityCut::Range && n.cut != ProbabilityCut::Horizon {
                    f.mark_cut(&n.state, n.cut == ProbabilityCut::Pending);
                }
            }
        }
        #[cfg(test)]
        {
            f.census.0 = nodes.len();
        }
        Ok(())
    })();
    if result.is_err() && m.completion_requested() {
        f.aborted_completion = (0..a.len()).map(|k| completion_graph(&nodes, k)).collect();
    }
    if crate::perf::enabled() {
        crate::perf::count("engine.graph", "states", nodes.len());
        crate::perf::count(
            "engine.graph",
            "edges",
            nodes.iter().flat_map(|n| &n.actions).map(Vec::len).sum(),
        );
    }
    result?;
    Ok(nodes)
}
fn completion_graph(nodes: &[Node], k: usize) -> crate::completion::Graph {
    let mut g = crate::completion::Graph::default();
    for n in nodes {
        g.nodes.push(crate::completion::Node {
            target: matches!(kind(&n.state, k), Some(Kind::End(_) | Kind::Overflow(_))),
            unknown: n.unknown || n.numeric,
            actions: Vec::new(),
        });
    }
    for (i, n) in nodes.iter().enumerate() {
        if kind(&n.state, k) == Some(Kind::Rejected) {
            g.nodes[i].unknown = false;
            g.known_failure = true;
            continue;
        }
        for a in &n.actions {
            g.action(i, a.clone());
        }
        g.known_failure |= n.actions.is_empty() && !n.unknown && kind(&n.state, k).is_none();
    }
    g
}
fn topology(nodes: &[Node]) -> EngineResult<Vec<usize>> {
    let mut indegree = vec![0; nodes.len()];
    for n in nodes {
        for a in &n.actions {
            for (_, j) in a {
                indegree[*j] += 1;
            }
        }
    }
    let mut ready: Vec<_> = indegree
        .iter()
        .enumerate()
        .filter_map(|(i, n)| (*n == 0).then_some(i))
        .collect();
    let mut order = Vec::new();
    while let Some(i) = ready.pop() {
        order.push(i);
        for a in &nodes[i].actions {
            for (_, j) in a {
                indegree[*j] -= 1;
                if indegree[*j] == 0 {
                    ready.push(*j);
                }
            }
        }
    }
    if order.len() != nodes.len() {
        return Err("multi-case graph contains a cycle".into());
    }
    Ok(order)
}
fn kind(s: &Global, k: usize) -> Option<Kind> {
    match &s.cases[k] {
        Slot::Rejected => Some(Kind::Rejected),
        Slot::Admitted(c) => c.terminal.map(Kind::End).or_else(|| {
            c.local
                .response
                .as_ref()
                .is_some_and(|h| h.stopped)
                .then_some(Kind::ReceiptUnknown)
        }),
        Slot::Overflow(c) => c.terminal.map(Kind::Overflow),
        Slot::Future | Slot::Held(_) => None,
    }
}
fn ended(s: &Global, k: usize) -> Option<(ir::EndId, u64)> {
    match &s.cases[k] {
        Slot::Admitted(c) | Slot::Overflow(c) => c.terminal.map(|e| {
            (
                e,
                c.local
                    .contest
                    .as_ref()
                    .and_then(|c| c.business.as_ref())
                    .map_or(c.local.time, |b| b.at),
            )
        }),
        _ => None,
    }
}
fn solve(
    nodes: &[Node],
    order: &[usize],
    k: usize,
    target: Option<Kind>,
    query: Option<&ir::ProbabilityQuery>,
    m: &ir::Model,
    a: &[u64],
) -> EngineResult<([Rational; 4], bool)> {
    let _timer = crate::perf::span("engine.solve");
    let unknown = || {
        [
            Rational::zero(),
            Rational::one(),
            Rational::zero(),
            Rational::one(),
        ]
    };
    let mut table = vec![std::array::from_fn(|_| Rational::zero()); nodes.len()];
    let mut numeric = vec![false; nodes.len()];
    for &i in order.iter().rev() {
        let n = &nodes[i];
        let s = &n.state;
        let hit = if let Some(t) = target {
            if t == Kind::CutOff {
                n.cut == ProbabilityCut::Horizon && kind(s, k).is_none()
            } else {
                kind(s, k) == Some(t)
            }
        } else if let Some(q) = query {
            if s.cases[k].payload().is_some_and(|c| c.received)
                && s.time >= a[k]
                && q.within.is_none_or(|d| s.time - a[k] <= d)
            {
                let v = s.view(k, m)?;
                query_hit(q, &v, ended(s, k), a[k])?
            } else {
                false
            }
        } else {
            false
        };
        if hit {
            table[i] = std::array::from_fn(|_| Rational::one());
            continue;
        }
        if kind(s, k).is_some()
            && (target.is_some()
                || query.is_some_and(|q| matches!(q.condition, ir::ProbabilityCondition::End(_)))
                || !matches!(&s.cases[k],Slot::Admitted(c) if !c.stopped()))
        {
            continue;
        } // Business terminal misses absorb; state queries can observe route writes.
        // An admitted case never becomes rejected later, whatever cut follows.
        if target == Some(Kind::Rejected) && s.cases[k].payload().is_some() {
            continue;
        }
        if n.numeric {
            numeric[i] = true;
            table[i] = unknown();
            continue;
        }
        if target.is_some() && n.cut == ProbabilityCut::Horizon {
            continue;
        }
        if query.and_then(|q| q.within).is_some_and(|d| {
            (s.time >= a[k] && s.time - a[k] > d)
                || (matches!(n.cut, ProbabilityCut::Clock | ProbabilityCut::Horizon)
                    && a[k]
                        <= if n.cut == ProbabilityCut::Horizon {
                            s.time
                        } else {
                            m.bounds.clock
                        }
                    && d <= (if n.cut == ProbabilityCut::Horizon {
                        s.time
                    } else {
                        m.bounds.clock
                    }) - a[k])
        }) {
            continue;
        }
        let mut actions = Vec::new();
        if n.unknown {
            actions.push(unknown());
        }
        for action in &n.actions {
            let mut v = std::array::from_fn::<_, 4, _>(|_| Rational::zero());
            for (p, j) in action {
                numeric[i] |= numeric[*j] || !p.within_bits(4096);
                for (z, x) in v.iter_mut().enumerate() {
                    let product = Rational(&p.0 * &table[*j][z].0);
                    numeric[i] |= !product.within_bits(4096);
                    x.0 += product.0;
                    numeric[i] |= !x.within_bits(4096);
                }
            }
            actions.push(v);
        }
        if numeric[i] {
            table[i] = unknown()
        } else if !actions.is_empty() {
            for z in 0..4 {
                table[i][z] = if z < 2 {
                    actions.iter().map(|v| v[z].clone()).min().unwrap()
                } else {
                    actions.iter().map(|v| v[z].clone()).max().unwrap()
                };
            }
        }
    }
    Ok((table[0].clone(), numeric[0]))
}
fn quality_result(
    m: &ir::Model,
    a: &[u64],
    nodes: &[Node],
    order: &[usize],
) -> crate::quality::Quality {
    use crate::quality::reduce_stopped;
    let values: EngineResult<Vec<Vec<_>>> = m
        .quality
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .map(|p| {
            (0..a.len())
                .map(|k| {
                    let hits = nodes
                        .iter()
                        .map(|n| {
                            let s = &n.state;
                            if s.cases[k].payload().is_some_and(|c| c.received) {
                                quality_hit(p, &s.view(k, m)?, ended(s, k), a[k])
                            } else {
                                Ok(false)
                            }
                        })
                        .collect::<EngineResult<Vec<_>>>()?;
                    Ok(reduce_stopped(
                        order,
                        |i| nodes[i].actions.clone(),
                        |i| {
                            let n = &nodes[i];
                            let s = &n.state;
                            if hits[i] {
                                return (Some(true), false, false);
                            }
                            if kind(s, k).is_some()
                                && (matches!(p.target, ir::QualityTarget::Ends(_))
                                    || !matches!(&s.cases[k],Slot::Admitted(c) if !c.stopped()))
                            {
                                return (Some(false), false, false);
                            }
                            if p.within.is_some_and(|d| {
                                (s.time >= a[k] && s.time - a[k] > d)
                                    || (n.unknown
                                        && matches!(
                                            n.cut,
                                            ProbabilityCut::Clock | ProbabilityCut::Horizon
                                        )
                                        && a[k]
                                            <= if n.cut == ProbabilityCut::Horizon {
                                                s.time
                                            } else {
                                                m.bounds.clock
                                            }
                                        && d <= (if n.cut == ProbabilityCut::Horizon {
                                            s.time
                                        } else {
                                            m.bounds.clock
                                        }) - a[k])
                            }) {
                                return (Some(false), false, false);
                            }
                            (
                                None,
                                n.unknown || n.numeric,
                                n.numeric || n.cut != ProbabilityCut::Horizon,
                            )
                        },
                        4096,
                    ))
                })
                .collect()
        })
        .collect();
    let values = match values {
        Ok(v) => v,
        Err(_) => return quality_aborted(m),
    };
    let outcomes = crate::quality::outcome_keys(m)
        .iter()
        .map(|(target, end)| {
            (0..a.len())
                .map(|k| {
                    reduce_stopped(
                        order,
                        |i| nodes[i].actions.clone(),
                        |i| {
                            let n = &nodes[i];
                            let s = &n.state;
                            if let Some(t) = kind(s, k) {
                                let hit = match t {
                                    Kind::End(e) => {
                                        *target == "end" && end.as_deref() == Some(&m.ends[e.0])
                                    }
                                    Kind::Overflow(e) => {
                                        *target == "overflow"
                                            && end.as_deref() == Some(&m.ends[e.0])
                                    }
                                    Kind::Rejected => *target == "rejected",
                                    Kind::CutOff => *target == "cut_off",
                                    Kind::ReceiptUnknown => {
                                        matches!(*target, "receipt_unknown" | "unfinished")
                                    }
                                };
                                return (Some(hit), false, false);
                            }
                            if *target == "rejected" && s.cases[k].payload().is_some() {
                                return (Some(false), false, false);
                            }
                            if n.cut == ProbabilityCut::Horizon && !n.numeric {
                                return (Some(*target == "cut_off"), false, false);
                            }
                            if n.unknown || n.numeric {
                                return (None, true, true);
                            }
                            if n.actions.is_empty() {
                                return (Some(*target == "deadlock"), false, false);
                            }
                            (None, false, false)
                        },
                        4096,
                    )
                })
                .collect()
        })
        .collect::<Vec<Vec<_>>>();
    let mut q = crate::quality::exhaustive(m, &values, &outcomes, false);
    let spec = m.quality.as_ref().unwrap();
    if !spec.quantiles.is_empty() {
        let rows: EngineResult<Vec<_>> = spec
            .quantiles
            .iter()
            .map(|c| {
                let p = &spec.properties[c.property];
                let views = (0..a.len())
                    .map(|k| {
                        nodes
                            .iter()
                            .map(|n| {
                                let s = &n.state;
                                let hit = if s.cases[k].payload().is_some_and(|c| c.received)
                                    && quality_hit(p, &s.view(k, m)?, ended(s, k), a[k])?
                                {
                                    Some(
                                        match p.target {
                                            ir::QualityTarget::Ends(_) => {
                                                ended(s, k).unwrap().1 as u128
                                            }
                                            _ => s.time as u128,
                                        } - a[k] as u128,
                                    )
                                } else {
                                    None
                                };
                                let closed = kind(s, k).is_some()
                                    && (matches!(p.target, ir::QualityTarget::Ends(_))
                                        || !matches!(&s.cases[k],Slot::Admitted(c) if !c.stopped()))
                                    || p.within
                                        .is_some_and(|d| s.time >= a[k] && s.time - a[k] > d);
                                let at = if n.cut == ProbabilityCut::Clock {
                                    m.bounds.clock as u128 + 1
                                } else if n.cut == ProbabilityCut::Horizon {
                                    s.time as u128 + 1
                                } else {
                                    s.time as u128
                                };
                                Ok(crate::quality::FirstView {
                                    hit,
                                    closed,
                                    censor: (n.unknown || n.numeric).then(|| {
                                        crate::quality::Arrival::censor(
                                            at.saturating_sub(a[k] as u128),
                                            p.within.map(u128::from),
                                            n.cut != ProbabilityCut::Horizon || n.numeric,
                                        )
                                    }),
                                })
                            })
                            .collect::<EngineResult<Vec<_>>>()
                    })
                    .collect::<EngineResult<Vec<_>>>()?;
                let f =
                    crate::quality::first_curves(order, |i| nodes[i].actions.clone(), &views, 4096);
                let (v, lb) = crate::quality::inverse_quantile(c, &f);
                Ok(crate::quality::quantile_row(m, c, v, lb))
            })
            .collect();
        match rows {
            Ok(rows) => q.quantiles = Some(rows),
            Err(_) => return quality_aborted(m),
        }
    }
    q
}

#[cfg(test)]
mod quality_graph_contract {
    use super::*;
    #[test]
    fn b6int_declared_horizon_does_not_reclassify_live_computational_cuts() {
        let controls: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/support/quality_deferred.json"))
                .unwrap();
        let src = controls["qual-horizon"]["source"]
            .as_str()
            .unwrap()
            .replace("horizon 14min", "horizon 30min")
            .replace("clock 30min", "clock 14min");
        let m = crate::lower::lower_with_selection(
            &crate::syntax::parse(&src).unwrap(),
            None,
            Some("Day"),
        )
        .unwrap();
        // Direct graph seam over valid selected IR. A later business bound
        // does not explain a leaf already stopped by a computational cut.
        let a = arrivals(&m).unwrap();
        let mut facts = Facts::new(a.len());
        let original = graph(&m, &a, DEFAULT_STATE_BUDGET, &mut facts).unwrap();
        let order = topology(&original).unwrap();
        for cut in [
            ProbabilityCut::Clock,
            ProbabilityCut::LiveTasks,
            ProbabilityCut::StateBudget,
            ProbabilityCut::Pending,
            ProbabilityCut::Range,
        ] {
            let mut nodes = original.clone();
            let mut count = 0;
            for node in &mut nodes {
                if node.unknown {
                    node.cut = cut;
                    count += 1;
                }
            }
            assert!(count > 0);
            let q = quality_result(&m, &a, &nodes, &order);
            assert_eq!(q.cohort[0]["value"]["reason"], "truncated");
        }
        // Numeric exhaustion is computational even on a horizon-marked leaf.
        let mut nodes = original;
        for node in &mut nodes {
            if node.unknown {
                node.cut = ProbabilityCut::Horizon;
                node.numeric = true;
            }
        }
        assert_eq!(
            quality_result(&m, &a, &nodes, &order).cohort[0]["value"]["reason"],
            "truncated"
        );
    }
    #[test]
    fn qp02_only_live_unknown_nodes_determine_computational_uncertainty() {
        let src = include_str!("../../../../fixtures/qual-clock.broken-short-clock/model.relca");
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let a = arrivals(&m).unwrap();
        let mut f = Facts::new(a.len());
        let mut nodes = graph(&m, &a, DEFAULT_STATE_BUDGET, &mut f).unwrap();
        let order = topology(&nodes).unwrap();
        for cut in [
            ProbabilityCut::Clock,
            ProbabilityCut::LiveTasks,
            ProbabilityCut::StateBudget,
            ProbabilityCut::Pending,
            ProbabilityCut::Range,
        ] {
            let mut unknown = 0;
            for n in &mut nodes {
                if n.unknown {
                    n.cut = cut;
                    unknown += 1;
                }
            }
            assert!(unknown > 0);
            let q = quality_result(&m, &a, &nodes, &order);
            assert_eq!(q.cohort[0]["value"]["reason"], "truncated");
        }
        // An expanded node's default Pending marker is not a live cut.
        let src = include_str!("../../../../fixtures/qual-base/model.relca");
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let a = arrivals(&m).unwrap();
        let mut f = Facts::new(a.len());
        let mut nodes = graph(&m, &a, DEFAULT_STATE_BUDGET, &mut f).unwrap();
        let order = topology(&nodes).unwrap();
        for n in &mut nodes {
            assert!(!n.unknown);
            n.cut = ProbabilityCut::Pending;
        }
        assert!(quality_result(&m, &a, &nodes, &order).cohort[0]["value"]["reason"].is_null());
    }
}
fn fold(v: impl Iterator<Item = Verdict>) -> Verdict {
    v.max_by_key(|v| match v {
        Verdict::Satisfied => 0,
        Verdict::Vacuous => 1,
        Verdict::Inconclusive => 2,
        Verdict::Unsupported => 3,
        Verdict::Violated => 4,
        Verdict::Invalid => 5,
    })
    .unwrap_or(Verdict::Unsupported)
}

fn sampled_contests(
    s: &Global,
    m: &ir::Model,
    cut: Option<outcome::SimulationCut>,
    range_stopped: bool,
) -> Vec<outcome::ContestObservation> {
    let mut rows = Vec::new();
    for (k, slot) in s.cases.iter().enumerate() {
        for (id, d) in m
            .contests()
            .iter()
            .enumerate()
            .filter(|(_, d)| d.rates.is_some())
        {
            let c = slot.payload();
            let r = c
                .and_then(|c| c.local.contest.as_ref())
                .map(|c| &c.routes[id]);
            let target_at = r.and_then(|r| r.target_at);
            let filed = r.and_then(|r| r.filed).filter(|_| target_at.is_some());
            let response_at = r.and_then(|r| r.response_at);
            let excluded = !crate::responsibility::included(m, m.capabilities[d.route.0].actor);
            let route_done = c.is_some_and(|c| {
                c.local
                    .tasks
                    .iter()
                    .any(|t| t.binding == m.contest_site(id) && t.lifecycle == Lifecycle::Done)
            });
            let failed = filed == Some(true)
                && response_at.is_none()
                && (excluded || route_done || (cut.is_none() && !range_stopped));
            // A later stop is not the time of an already observed refusal or
            // cancellation. Without a recorded timestamp, leave closure unknown.
            let failure_at = if excluded {
                target_at
            } else if route_done {
                r.and_then(|r| r.failure_at)
            } else {
                (cut.is_none() && !range_stopped).then_some(s.time)
            };
            rows.push(outcome::ContestObservation {
                contest: id,
                case_id: k as u64 + 1,
                target_known: target_at.is_some()
                    || (cut.is_none() && !range_stopped)
                    || c.is_some_and(|c| c.terminal.is_some())
                    || matches!(slot, Slot::Rejected),
                target_at,
                filed,
                started_at: r.and_then(|r| r.started_at),
                response_at,
                closed_at: response_at
                    .or_else(|| (filed == Some(false)).then_some(target_at).flatten())
                    .or_else(|| failed.then_some(failure_at).flatten()),
                failed,
            });
        }
    }
    rows
}

pub(super) fn simulate(m: &ir::Model) -> outcome::SimulationOutcome {
    use outcome::{
        ReplicationOutcome, SimulationCase, SimulationCut, SimulationOutcome, SimulationTerminal,
        SimulationTerminalKind,
    };
    let mut out = SimulationOutcome::default();
    if m.unsupported_stochastic_arrival_combination() {
        out.diagnostics.push(simulation::runtime_diagnostic(
            m,
            1,
            0,
            false,
            "stochastic_arrivals_feature_combination",
            &[],
        ));
        return out;
    }
    out.stochastic_arrivals = m.arrival_distribution.is_some();
    if let Some(d) = &m.arrival_distribution
        && d.validate(m).is_err()
    {
        out.diagnostics.push(simulation::runtime_diagnostic(
            m,
            1,
            0,
            false,
            "invalid stochastic arrival distribution",
            &[],
        ));
        return out;
    }
    let mut shape = m.clone();
    shape.arrival_distribution = None;
    let a = match arrivals(&shape) {
        Ok(a) => a,
        Err(_) => {
            out.diagnostics.push(simulation::runtime_diagnostic(
                m,
                1,
                0,
                false,
                "invalid arrivals model",
                &[],
            ));
            return out;
        }
    };
    let Some(spec) = &m.simulation else {
        return out;
    };
    out.metric_template = metrics::finish(None, m, crate::obligation::Coverage::Aborted);
    out.capability_refusals = m.has_requires().then(Vec::new);
    let mut quality_observations = Vec::new();
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    for rep in 1..=spec.replications {
        out.attempted_replications = rep;
        let mut sample = simulation::Sample::new(spec.seed, rep);
        let mut realized = shape.clone();
        if let Some(d) = &m.arrival_distribution {
            let weights: Vec<_> = d.entries.iter().map(|e| e.1.clone()).collect();
            let selected = match sample.choose(0, "arrival.schedule", 1, &weights) {
                Ok(i) => i,
                Err(_) => {
                    out.diagnostics.push(simulation::runtime_diagnostic(
                        m,
                        rep,
                        0,
                        false,
                        "arrival schedule sampling failed",
                        &[],
                    ));
                    return out;
                }
            };
            realized.arrivals = Some(d.entries[selected].0.clone());
        }
        let m = &realized;
        let a = m.arrivals.clone().unwrap_or_else(|| a.clone());
        let mut s = initial(m, a.len());
        let mut facts = Facts::new(a.len());
        let mut hit = vec![vec![false; m.probability_queries.len()]; a.len()];
        let mut quality_hits =
            vec![vec![false; a.len()]; m.quality.as_ref().map_or(0, |q| q.properties.len())];
        // Observer lifetime is path-local, not part of execution-state identity.
        let mut quality_closed = vec![vec![false; a.len()]; quality_hits.len()];
        let mut first_hits = vec![
            vec![None; a.len()];
            m.quality.as_ref().map_or(0, |q| if q.quantiles.is_empty() {
                0
            } else {
                q.properties.len()
            })
        ];
        let mut cut = None;
        let mut deadlocks = vec![None; a.len()];
        let path = (|| -> EngineResult<()> {
            loop {
                if s.live_cut(m) {
                    cut = Some(SimulationCut::LiveTasks);
                    break;
                }
                facts.observe(&s, m, &a)?;
                if let Some(q) = &m.quality {
                    for (i, p) in q.properties.iter().enumerate() {
                        let mut first_views = first_hits
                            .get(i)
                            .map(|_| vec![(false, false, None); a.len()]);
                        for (k, hits) in quality_hits[i].iter_mut().enumerate() {
                            if quality_closed[i][k] {
                                continue;
                            }
                            // Include the closing transition itself, then freeze both
                            // B6 reachability and first arrival before later shared writes.
                            quality_closed[i][k] = kind(&s, k).is_some()
                                && (matches!(p.target, ir::QualityTarget::Ends(_))
                                    || s.cases[k].payload().is_none_or(|c| c.stopped()))
                                || p.within
                                    .is_some_and(|d| s.time >= a[k] && s.time - a[k] > d);
                            if s.cases[k].payload().is_some_and(|c| c.received) {
                                let arrived = quality_hit(p, &s.view(k, m)?, ended(&s, k), a[k])?;
                                *hits |= arrived;
                                if let Some(views) = &mut first_views {
                                    let at = match p.target {
                                        ir::QualityTarget::Ends(_) => {
                                            ended(&s, k).map_or(s.time, |(_, t)| t)
                                        }
                                        _ => s.time,
                                    };
                                    views[k] = (true, arrived, Some(at));
                                }
                            }
                        }
                        if let Some(views) = first_views {
                            crate::quality::observe_first_world(
                                &mut first_hits[i],
                                &a,
                                s.time,
                                true,
                                &views,
                            );
                        }
                    }
                }
                for (k, hits) in hit.iter_mut().enumerate() {
                    if s.cases[k].payload().is_some_and(|c| c.received) {
                        let view = s.view(k, m)?;
                        for (i, q) in m.probability_queries.iter().enumerate() {
                            hits[i] |= query_hit(q, &view, ended(&s, k), a[k])?;
                        }
                    }
                }
                sample.sites.clear();
                sample.kernels.clear();
                sample.provenance.clear();
                let mut scratch = Facts::new(a.len());
                match transition(&s, m, &a, &program, entry, &mut sample, &mut scratch) {
                    Ok(None) => {
                        for (k, slot) in s.cases.iter().enumerate() {
                            if slot
                                .payload()
                                .is_some_and(|c| c.terminal.is_none() && !c.stopped())
                            {
                                deadlocks[k] = Some(s.time);
                            }
                            for (i, violated) in scratch.per[k].violation.iter().enumerate() {
                                facts.per[k].violation.resize(m.contracts.len(), false);
                                facts.per[k].violation[i] |= *violated;
                            }
                        }
                        break;
                    }
                    Ok(Some(actions)) if actions.is_empty() => {
                        if scratch.per.iter().any(|e| !e.ranges.is_empty()) {
                            for (dst, src) in facts.per.iter_mut().zip(&scratch.per) {
                                dst.ranges = src.ranges.clone();
                            }
                            break;
                        }
                        cut = Some(if m.horizon == Some(s.time) && s.time <= m.bounds.clock {
                            SimulationCut::Horizon
                        } else {
                            SimulationCut::Clock
                        });
                        for k in 0..a.len() {
                            facts.per[k].violation.resize(m.contracts.len(), false);
                            for (i, v) in scratch.per[k].violation.iter().enumerate() {
                                facts.per[k].violation[i] |= *v;
                            }
                        }
                        break;
                    }
                    Ok(Some(mut actions)) => {
                        if scratch.per.iter().any(|e| !e.ranges.is_empty()) {
                            return sample.reject();
                        }
                        sample.unique(&actions)?;
                        let action = actions.remove(0);
                        if let Some((_, task)) =
                            sample.kernels.iter().find(|(i, _)| *i == 0).copied()
                        {
                            let (_, case) = sample.tag(0).ok_or("missing kernel site")?;
                            sample.case = case;
                            let k = case as usize - 1;
                            let local = sample_kernel_in(
                                &s.view(k, m)?,
                                task,
                                m,
                                &mut sample,
                                &mut scratch.per[k],
                                k,
                            )?;
                            s.store(k, local, m);
                            s.normalize_state_invariants(m)?;
                            continue;
                        }
                        let selected = if let Some((site, case)) = sample.tag(0) {
                            sample.choose(
                                case,
                                &site,
                                1,
                                &action.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
                            )?
                        } else {
                            0
                        };
                        let next = action
                            .into_iter()
                            .nth(selected)
                            .ok_or("empty sampled action")?
                            .1;
                        for k in 0..a.len() {
                            if !m.has_requires() {
                                break;
                            }
                            if let (Slot::Admitted(_), Slot::Admitted(_)) =
                                (&s.cases[k], &next.cases[k])
                            {
                                let old = s.view(k, m)?;
                                let mut local = next.view(k, m)?;
                                selected_refusals(&old, &mut local, m, &mut facts.per[k]);
                            }
                        }
                        s = next;
                    }
                    Err(Halt::Budget) if scratch.numeric => {
                        cut = Some(SimulationCut::Numeric);
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        })();
        if let Err(h) = path {
            let message = match h {
                Halt::Unsupported(s) => s,
                Halt::Budget => "unexpected simulation limit",
            };
            out.diagnostics.clear();
            out.diagnostics.push(simulation::runtime_diagnostic(
                m,
                rep,
                s.time,
                sample.refused,
                message,
                &sample.provenance,
            ));
            out.truncated_replications = None;
            return out;
        }
        if let Some(q) = &m.quality {
            let truths: Vec<Vec<crate::outcome::QueryTruth>> = q
                .properties
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    (0..a.len())
                        .map(|k| {
                            let closed = kind(&s, k).is_some()
                                && (matches!(p.target, ir::QualityTarget::Ends(_))
                                    || s.cases[k].payload().is_none_or(|c| c.stopped()));
                            simulation::QueryObservation {
                                hit: quality_hits[i][k],
                                closed: closed || deadlocks[k].is_some(),
                                time: s.time,
                                arrival: a[k],
                                within: p.within,
                                cut,
                                clock: if cut == Some(SimulationCut::Horizon) {
                                    s.time
                                } else {
                                    m.bounds.clock
                                },
                            }
                            .quality_truth(facts.per.iter().any(|e| !e.ranges.is_empty()))
                        })
                        .collect()
                })
                .collect();
            let keys = crate::quality::outcome_keys(m);
            let outcomes = (0..a.len())
                .map(|k| {
                    let key = match kind(&s, k) {
                        Some(Kind::End(e)) => ("end", Some(m.ends[e.0].clone())),
                        Some(Kind::Overflow(e)) => ("overflow", Some(m.ends[e.0].clone())),
                        Some(Kind::Rejected) => ("rejected", None),
                        Some(Kind::CutOff) => ("cut_off", None),
                        Some(Kind::ReceiptUnknown) => ("receipt_unknown", None),
                        None if deadlocks[k].is_some() => ("deadlock", None),
                        None if cut == Some(SimulationCut::Horizon) => ("cut_off", None),
                        _ => ("unfinished", None),
                    };
                    #[cfg(test)]
                    let key = crate::quality_impl_tests::outcome_lookup_key("engine_arrivals", key);
                    keys.iter().position(|x| *x == key)
                })
                .collect::<Option<Vec<_>>>();
            let Some(outcomes) = outcomes else {
                return simulation::invalid_outcome("quality outcome inventory mismatch", m);
            };
            let arrivals = q
                .properties
                .iter()
                .take(first_hits.len())
                .enumerate()
                .map(|(i, p)| {
                    (0..a.len())
                        .map(|k| {
                            if let Some(t) = first_hits[i][k] {
                                crate::quality::Arrival::Hit(t)
                            } else if truths[i][k] == crate::outcome::QueryTruth::Unknown {
                                let at = if cut == Some(SimulationCut::Clock) {
                                    m.bounds.clock as u128 + 1
                                } else if cut == Some(SimulationCut::Horizon) {
                                    s.time as u128 + 1
                                } else {
                                    s.time as u128
                                };
                                crate::quality::Arrival::censor(
                                    at.saturating_sub(a[k] as u128),
                                    p.within.map(u128::from),
                                    cut != Some(SimulationCut::Horizon),
                                )
                            } else {
                                crate::quality::Arrival::Never
                            }
                        })
                        .collect()
                })
                .collect();
            quality_observations.push(crate::quality::Observation {
                truths,
                outcomes,
                arrivals,
                computational: cut != Some(SimulationCut::Horizon),
            });
        }
        let cases = (0..a.len())
            .map(|k| {
                let terminal = match kind(&s, k) {
                    Some(Kind::End(e)) => Some(SimulationTerminal {
                        kind: SimulationTerminalKind::End,
                        end: Some(m.ends[e.0].clone()),
                    }),
                    Some(Kind::Overflow(e)) => Some(SimulationTerminal {
                        kind: SimulationTerminalKind::Overflow,
                        end: Some(m.ends[e.0].clone()),
                    }),
                    Some(Kind::Rejected) => Some(SimulationTerminal {
                        kind: SimulationTerminalKind::Rejected,
                        end: None,
                    }),
                    Some(Kind::CutOff) => unreachable!("cutoff is not a business terminal"),
                    Some(Kind::ReceiptUnknown) => Some(SimulationTerminal {
                        kind: SimulationTerminalKind::ReceiptUnknown,
                        end: None,
                    }),
                    None => None,
                };
                let queries = m
                    .probability_queries
                    .iter()
                    .enumerate()
                    .map(|(i, q)| {
                        if facts.per.iter().any(|e| !e.ranges.is_empty()) && !hit[k][i] {
                            return outcome::QueryTruth::Unknown;
                        }
                        simulation::QueryObservation {
                            hit: hit[k][i],
                            closed: terminal.is_some() || deadlocks[k].is_some(),
                            time: s.time,
                            arrival: a[k],
                            within: q.within,
                            cut,
                            clock: if cut == Some(SimulationCut::Horizon) {
                                s.time
                            } else {
                                m.bounds.clock
                            },
                        }
                        .truth()
                    })
                    .collect();
                SimulationCase {
                    cut_off: m
                        .horizon
                        .map(|_| cut == Some(SimulationCut::Horizon) && terminal.is_none()),
                    case_id: k as u64 + 1,
                    arrival: a[k],
                    terminal,
                    queries,
                    violations: facts.per[k].violation.clone(),
                    deadlock: deadlocks[k],
                }
            })
            .collect();
        simulation::path_warning(
            &mut out,
            m,
            cut == Some(SimulationCut::Numeric),
            deadlocks.iter().any(Option::is_some),
        );
        let ranges: Vec<_> = facts.per.iter().flat_map(|e| e.ranges.clone()).collect();
        let range_stopped = !ranges.is_empty();
        if !ranges.is_empty() {
            out.range_stops.push((rep, ranges));
        }
        let response_recovery = m
            .recovery()
            .map(|_| {
                use crate::recovery::Stop;
                let rows: Vec<_> = s
                    .cases
                    .iter()
                    .enumerate()
                    .filter_map(|(k, slot)| {
                        slot.payload().map(|c| {
                            (
                                k,
                                &c.local,
                                if c.terminal.is_some() {
                                    Stop::End
                                } else if c.local.response.as_ref().is_some_and(|h| h.stopped) {
                                    Stop::ReceiptUnknown
                                } else if cut == Some(SimulationCut::Horizon) {
                                    Stop::CutOff
                                } else if range_stopped {
                                    Stop::StateRange
                                } else if cut.is_some() {
                                    Stop::Truncated
                                } else {
                                    Stop::Deadlock
                                },
                            )
                        })
                    })
                    .collect();
                recovery::path(m, &rows)
            })
            .transpose();
        let response_recovery = match response_recovery {
            Ok(r) => r,
            Err(h) => {
                let message = match h {
                    Halt::Unsupported(s) => s,
                    Halt::Budget => "unexpected recovery observation limit",
                };
                out.diagnostics.clear();
                out.diagnostics.push(simulation::runtime_diagnostic(
                    m,
                    rep,
                    s.time,
                    false,
                    message,
                    &sample.provenance,
                ));
                out.truncated_replications = None;
                return out;
            }
        };
        out.replications.push(ReplicationOutcome {
            response_recovery,
            contests: sampled_contests(&s, m, cut, range_stopped),
            admission: m.explicit_limits().then(|| {
                let rejected = s
                    .cases
                    .iter()
                    .filter(|s| matches!(s, Slot::Rejected))
                    .count() as u64;
                let overflow = s
                    .cases
                    .iter()
                    .filter(|s| matches!(s, Slot::Overflow(_)))
                    .count() as u64;
                let missing = if cut.is_some() {
                    (a.len() - s.cursor) as u64
                } else {
                    0
                };
                AdmissionSummary {
                    rejected_count: Range {
                        min: rejected,
                        max: rejected + missing,
                    },
                    overflow_count: m.overflow_end.map(|_| Range {
                        min: overflow,
                        max: overflow + missing,
                    }),
                    external_wait_max: m.external_wait().then(|| Range {
                        min: s.external_peak.unwrap(),
                        max: s.external_peak.unwrap().max(
                            s.cases
                                .iter()
                                .filter(|s| matches!(s, Slot::Held(_)))
                                .count() as u64
                                + missing,
                        ),
                    }),
                }
            }),
            lost_updates: facts
                .per
                .iter()
                .flat_map(|e| e.lost_updates.iter().copied())
                .collect(),
            scenario_observed_until: m.scenario.as_ref().map(|_| {
                if cut == Some(SimulationCut::Clock) {
                    m.bounds.clock
                } else {
                    s.time
                }
            }),
            metrics: m.metrics.as_ref().map(|_| {
                metric_snapshot_with_horizon(
                    &s,
                    m,
                    &a,
                    if cut == Some(SimulationCut::Clock) {
                        m.bounds.clock
                    } else {
                        s.time
                    },
                    deadlocks.iter().any(Option::is_some),
                    cut == Some(SimulationCut::Horizon),
                )
            }),
            replication: rep,
            draws: sample.draws,
            cases,
            cut,
            overcommit: facts.overcommit,
        });
        out.add_refusals(facts.per.iter().enumerate().flat_map(|(k, e)| {
            e.refusals.values().cloned().map(move |mut r| {
                r.case_id = k as u64 + 1;
                r
            })
        }));
        out.completed_replications = rep;
        *out.truncated_replications.get_or_insert(0) +=
            u64::from(cut.is_some() && cut != Some(SimulationCut::Horizon));
    }
    if m.recovery().is_some() && !out.replications.is_empty() {
        out.response_recovery = Some(crate::recovery::Recovery::new(
            out.replications
                .iter()
                .filter_map(|r| r.response_recovery.clone())
                .collect(),
            crate::recovery::Coverage::Sampled,
        ));
    }
    out.quality = m
        .quality
        .as_ref()
        .map(|_| crate::quality::simulation(m, &quality_observations, false));
    out
}

fn recovery_paths(
    m: &ir::Model,
    nodes: &[Node],
) -> EngineResult<Option<Vec<crate::recovery::Path>>> {
    use crate::recovery::Stop;
    m.recovery()
        .map(|_| {
            nodes
                .iter()
                .filter(|n| n.actions.is_empty() || n.unknown)
                .map(|n| {
                    let rows: Vec<_> = n
                        .state
                        .cases
                        .iter()
                        .enumerate()
                        .filter_map(|(k, slot)| {
                            slot.payload().map(|c| {
                                let stop = if c.terminal.is_some() {
                                    Stop::End
                                } else if c.local.response.as_ref().is_some_and(|h| h.stopped) {
                                    Stop::ReceiptUnknown
                                } else {
                                    match n.cut {
                                        ProbabilityCut::Range => Stop::StateRange,
                                        ProbabilityCut::Horizon => Stop::CutOff,
                                        ProbabilityCut::Clock
                                        | ProbabilityCut::LiveTasks
                                        | ProbabilityCut::StateBudget => Stop::Truncated,
                                        ProbabilityCut::Pending => {
                                            if n.unknown {
                                                Stop::Truncated
                                            } else {
                                                Stop::Deadlock
                                            }
                                        }
                                    }
                                };
                                (k, &c.local, stop)
                            })
                        })
                        .collect();
                    #[cfg(test)]
                    if crate::r2_test_adapter::active() {
                        for (k, _, stop) in &rows {
                            recovery::capture(
                                m,
                                &n.state.view(*k, m)?,
                                *k,
                                if n.cut == ProbabilityCut::Clock {
                                    m.bounds.clock
                                } else {
                                    n.state.time
                                },
                                *stop,
                            );
                        }
                    }
                    recovery::path(m, &rows)
                })
                .collect()
        })
        .transpose()
}

pub(super) fn explore(m: &ir::Model, budget: usize) -> outcome::Outcome {
    explore_traced(m, budget, None).0
}

pub(super) fn explore_traced(
    m: &ir::Model,
    budget: usize,
    trace: Option<trace::Capture>,
) -> (outcome::Outcome, Option<trace::Capture>) {
    let _timer = crate::perf::span("engine.total");
    let a = match arrivals(m) {
        Ok(a) => a,
        Err(Halt::Unsupported(msg)) => return (Exploration::default().abort(msg, m), trace),
        Err(Halt::Budget) => unreachable!(),
    };
    let mut f = Facts::new(a.len());
    f.trace = trace;
    let result = graph(m, &a, budget, &mut f)
        .and_then(|nodes| topology(&nodes).map(|order| (nodes, order)))
        .and_then(|(nodes, order)| {
            let paths = recovery_paths(m, &nodes)?;
            Ok((nodes, order, paths))
        });
    let (nodes, order, abort, response_paths) = match result {
        Ok((n, o, p)) => (n, o, None, p),
        Err(Halt::Unsupported(msg)) => (
            Vec::new(),
            Vec::new(),
            Some(msg),
            m.recovery().map(|_| Vec::new()),
        ),
        Err(Halt::Budget) => unreachable!(),
    };
    let mut properties = Vec::new();
    let mut completion_exceeded = false;
    let completion: Vec<_> = if m.completion_requested() {
        (0..a.len())
            .map(|k| {
                let g = if abort.is_some() && k < f.aborted_completion.len() {
                    std::mem::take(&mut f.aborted_completion[k])
                } else {
                    completion_graph(&nodes, k)
                };
                completion_exceeded |= g.exceeded;
                g.analyze(abort.is_some())
            })
            .collect()
    } else {
        Vec::new()
    };
    for verify in &m.verify {
        if let ir::Property::Contest(id) = verify.property {
            properties.push(outcome::PropertyOutcome {
                property: verify.property,
                verdict: crate::contest::verdict(m, id),
                probability: None,
                cases: None,
            });
            continue;
        }
        let mut rows = Vec::new();
        if !matches!(
            verify.property,
            ir::Property::NoResourceOvercommit | ir::Property::NoLostUpdate(_)
        ) {
            for k in 0..a.len() {
                let mut probability = None;
                let verdict = match verify.property {
                    p if p.completion() => {
                        let (v, b) = completion[k].property(p);
                        probability = b;
                        v
                    }
                    ir::Property::ProbabilityReached(id) if abort.is_none() => {
                        let q = &m.probability_queries[id];
                        let (b, numeric) = solve(&nodes, &order, k, None, Some(q), m, &a)
                            .expect("validated query");
                        f.numeric |= numeric;
                        f.truncated |= numeric;
                        probability = Some(ProbabilityRange {
                            min: b[0].clone(),
                            max: b[3].clone(),
                        });
                        if numeric {
                            Verdict::Inconclusive
                        } else {
                            match q.comparison {
                                ir::Comparison::Le if b[3] <= q.bound => Verdict::Satisfied,
                                ir::Comparison::Le if b[2] > q.bound => Verdict::Violated,
                                ir::Comparison::Ge if b[0] >= q.bound => Verdict::Satisfied,
                                ir::Comparison::Ge if b[1] < q.bound => Verdict::Violated,
                                _ => Verdict::Inconclusive,
                            }
                        }
                    }
                    ir::Property::ReachableEnd(e)
                        if f.terminals[k].contains_key(&Kind::End(e))
                            || f.terminals[k].contains_key(&Kind::Overflow(e)) =>
                    {
                        Verdict::Satisfied
                    }
                    ir::Property::Contract(id) if f.per[k].violation.get(id.0) == Some(&true) => {
                        Verdict::Violated
                    }
                    ir::Property::StateInRange if !f.per[k].ranges.is_empty() => Verdict::Violated,
                    _ if abort.is_some() => Verdict::Unsupported,
                    ir::Property::StateInRange => {
                        if f.cut[k] {
                            Verdict::Inconclusive
                        } else {
                            Verdict::Satisfied
                        }
                    }
                    ir::Property::Contract(_) | ir::Property::ReachableEnd(_) if f.cut[k] => {
                        Verdict::Inconclusive
                    }
                    // A stopped caller has no business End yet. Do not turn
                    // an unresolved receipt into a proof of unreachability.
                    ir::Property::ReachableEnd(_)
                        if f.terminals[k].contains_key(&Kind::ReceiptUnknown) =>
                    {
                        Verdict::Inconclusive
                    }
                    ir::Property::ReachableEnd(_) => Verdict::Violated,
                    ir::Property::Contract(id) => {
                        if f.per[k].activation.get(id.0) == Some(&true) {
                            Verdict::Satisfied
                        } else {
                            Verdict::Vacuous
                        }
                    }
                    _ => Verdict::Unsupported,
                };
                rows.push(CaseProperty {
                    case_id: ir::CaseId(k),
                    arrival: a[k],
                    verdict,
                    probability,
                });
            }
        }
        let verdict = if matches!(
            verify.property,
            ir::Property::NoResourceOvercommit | ir::Property::NoLostUpdate(_)
        ) {
            if match verify.property {
                ir::Property::NoLostUpdate(id) => {
                    f.per.iter().any(|e| e.lost_updates.contains(&id))
                }
                _ => f.overcommit,
            } {
                Verdict::Violated
            } else if abort.is_some() {
                Verdict::Unsupported
            } else if f.truncated || f.horizon || f.per.iter().any(|e| !e.ranges.is_empty()) {
                Verdict::Inconclusive
            } else {
                Verdict::Satisfied
            }
        } else {
            fold(rows.iter().map(|r| r.verdict))
        };
        properties.push(outcome::PropertyOutcome {
            property: verify.property,
            verdict,
            probability: None,
            cases: (!matches!(
                verify.property,
                ir::Property::NoResourceOvercommit | ir::Property::NoLostUpdate(_)
            ))
            .then_some(rows),
        });
    }
    let mut rejected = Range {
        min: u64::MAX,
        max: 0,
    };
    for n in &nodes {
        if n.actions.is_empty() || n.unknown {
            let r = n
                .state
                .cases
                .iter()
                .filter(|s| matches!(s, Slot::Rejected))
                .count() as u64;
            rejected.min = rejected.min.min(r);
            rejected.max = rejected.max.max(
                r + if n.unknown {
                    (a.len() - n.state.cursor) as u64
                } else {
                    0
                },
            );
        }
    }
    if rejected.min == u64::MAX {
        rejected = Range {
            min: 0,
            max: a.len() as u64,
        };
    }
    let mut case_outcomes = Vec::new();
    for k in 0..a.len() {
        let mut terminals = if abort.is_none() {
            std::mem::take(&mut f.terminals[k])
                .into_values()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        {
            for t in &mut terminals {
                if !m.uses_probability() && t.kind != Kind::CutOff {
                    continue;
                }
                let (b, numeric) = solve(&nodes, &order, k, Some(t.kind), None, m, &a)
                    .expect("validated terminal");
                f.numeric |= numeric;
                f.truncated |= numeric;
                t.probability = Some(ProbabilityRange {
                    min: b[0].clone(),
                    max: b[3].clone(),
                });
            }
        }
        case_outcomes.push(CaseOutcome {
            admitted_at: m.external_wait().then(|| {
                let mut range = None;
                let mut certain = abort.is_none();
                for n in &nodes {
                    if !n.actions.is_empty() && !n.unknown {
                        continue;
                    }
                    if let Some(t) = n.state.cases[k].payload().and_then(|c| c.admitted_at) {
                        if let Some(r) = &mut range {
                            extend(r, t);
                        } else {
                            range = Some(Range { min: t, max: t });
                        }
                    } else {
                        certain = false;
                    }
                }
                if certain { range } else { None }
            }),
            case_id: ir::CaseId(k),
            arrival: a[k],
            terminals,
        });
    }
    let mut diagnostics = Vec::new();
    let mut single_terminals = Vec::new();
    if a.len() == 1 && !m.response_attempts().is_empty() {
        for t in &case_outcomes[0].terminals {
            if let Kind::End(end) | Kind::Overflow(end) = t.kind {
                single_terminals.push(outcome::TerminalOutcome {
                    end,
                    elapsed: t.elapsed.unwrap(),
                    work: t.work.unwrap(),
                    result_at: t.result_at,
                    probability: t.probability.clone(),
                });
            }
        }
        for p in &mut properties {
            if let Some(c) = p.cases.as_ref().and_then(|cs| cs.first()) {
                p.probability = c.probability.clone();
            }
        }
    }
    if !m.response_attempts().is_empty() {
        for p in &mut properties {
            if let Some(cases) = &p.cases
                && let Some(first) = cases.first()
                && cases.iter().all(|c| c.probability == first.probability)
            {
                p.probability = first.probability.clone();
            }
        }
    }
    // A numerical limit can be discovered while reducing a query or terminal,
    // after the safety row was first constructed. Global safety is still cut.
    if f.truncated && abort.is_none() {
        for p in &mut properties {
            if p.property == ir::Property::NoResourceOvercommit && p.verdict == Verdict::Satisfied {
                p.verdict = Verdict::Inconclusive;
            }
        }
    }
    for (yes, code, msg) in [
        (
            f.deadlock,
            diag::W_DEADLOCK_0,
            "a reachable nonterminal state has no enabled transition",
        ),
        (
            f.budget || f.numeric,
            diag::W_BUDGET_0,
            if f.numeric {
                "probability denominator exceeded the exact arithmetic limit"
            } else {
                "exploration stopped at the engine state budget"
            },
        ),
    ] {
        if yes {
            diagnostics.push(diag::Diagnostic {
                code: code.into(),
                severity: diag::Severity::Warning,
                message: msg.into(),
                span: None,
            });
        }
    }
    if let Some(msg) = abort {
        diagnostics.push(diag::Diagnostic {
            code: diag::U_SEM_0.into(),
            severity: diag::Severity::Error,
            message: msg.into(),
            span: None,
        });
    }
    let trace = f.trace.take();
    if !f.truncated && !f.horizon && abort.is_none() && f.per.iter().all(|e| e.ranges.is_empty()) {
        let mut observed = crate::completion::Observed::default();
        for e in &f.per {
            observed.merge(&e.observed);
        }
        diagnostics.extend(observed.warnings(m));
    }
    // ADR 0029 U2: a range stop on any joint branch leaves every case's
    // continuation unknown there, like the global computational cut.
    let coverage = crate::obligation::Coverage::of(
        abort.is_some(),
        f.truncated,
        f.per.iter().any(|e| !e.ranges.is_empty()),
    );
    let contest_runtime = (!m.contests().is_empty()).then(|| {
        f.per
            .iter()
            .enumerate()
            .flat_map(|(k, e)| {
                e.contest_tables(m, abort.is_some())
                    .0
                    .unwrap_or_default()
                    .into_iter()
                    .enumerate()
                    .map(move |(id, mut row)| {
                        row.case_id = k as u64 + 1;
                        row.id = row.id.replacen("case1:", &format!("case{}:", k + 1), 1);
                        row.coverage = coverage;
                        row.alternatives =
                            contest_table_alternatives(row.alternatives, row.coverage, m, id);
                        row
                    })
            })
            .collect()
    });
    let contest_summary = (!m.contests().is_empty()).then(|| {
        let mut low = u64::MAX;
        let mut high = 0;
        for node in &nodes {
            if !node.actions.is_empty() && !node.unknown { continue; }
            let open = node.state.cases.iter().filter(|slot| matches!(slot,Slot::Admitted(c)
                if c.terminal.is_some() && c.local.contest.as_ref().is_some_and(|x|
                    if crate::contest::has_effects(m) { x.end_open.unwrap_or(false) } else { x.routes.iter().any(|r| r.filed.is_none() || (r.filed==Some(true) && r.response_at.is_none())) })) ).count() as u64;
            low = low.min(open); high = high.max(open);
        }
        let reason = coverage.reason();
        crate::contest::Summary { business_ended_open:crate::metrics::Range {
            min:reason.is_none().then(|| low.min(high).into()),max:reason.is_none().then(||high.into()),reason,
        }}
    });
    (
        outcome::Outcome {
            response_recovery: response_paths.map(|paths| {
                use crate::recovery::{Coverage, Recovery};
                Recovery::new(
                    paths,
                    if coverage == crate::obligation::Coverage::Complete {
                        Coverage::Complete
                    } else {
                        Coverage::Partial
                    },
                )
            }),
            response_exercise: if !m.response_attempts().is_empty()
                && abort.is_none()
                && !f.truncated
                && !f.horizon
            {
                Some(
                    m.capabilities
                        .iter()
                        .enumerate()
                        .map(|(id, _)| {
                            let mut min = usize::MAX;
                            let mut max = 0;
                            for node in nodes.iter().filter(|n| n.actions.is_empty() && !n.unknown)
                            {
                                for slot in &node.state.cases {
                                    let count = slot
                                        .payload()
                                        .and_then(|c| c.local.response.as_ref())
                                        .map_or(0, |h| {
                                            h.started.values().filter(|c| c.0 == id).count()
                                        });
                                    min = min.min(count);
                                    max = max.max(count);
                                }
                            }
                            outcome::CapabilityExercise {
                                capability: ir::CapabilityId(id),
                                min: if min == usize::MAX { 0 } else { min },
                                max,
                            }
                        })
                        .collect(),
                )
            } else {
                None
            },
            response_receipts: (!m.response_attempts().is_empty()).then_some(f.response_receipts),
            scenario_observed_until: m.scenario.as_ref().map(|_| {
                let reason = if abort.is_some() {
                    Some(crate::metrics::Reason::Aborted)
                } else if f.truncated {
                    Some(crate::metrics::Reason::Truncated)
                } else if f.per.iter().any(|e| !e.ranges.is_empty()) {
                    Some(crate::metrics::Reason::RangeStopped)
                } else {
                    None
                };
                crate::metrics::Scalar::Range(crate::metrics::Range {
                    min: reason
                        .is_none()
                        .then(|| f.observed_until.map_or(0, |r| r.min).into()),
                    max: reason
                        .is_none()
                        .then(|| f.observed_until.map_or(0, |r| r.max).into()),
                    reason,
                })
            }),
            quality: m.quality.as_ref().filter(|_| validate(m).is_ok()).map(|_| {
                if abort.is_some() {
                    quality_aborted(m)
                } else {
                    quality_result(m, &a, &nodes, &order)
                }
            }),
            state_range_violations: f.per.iter().flat_map(|e| e.ranges.clone()).collect(),
            capability_refusals: m.has_requires().then(|| {
                f.per
                    .iter()
                    .enumerate()
                    .flat_map(|(k, e)| {
                        e.refusals
                            .values()
                            .cloned()
                            .map(|mut r| {
                                r.case_id = k as u64 + 1;
                                r.coverage = coverage.word().into();
                                r
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect()
            }),
            unchecked: {
                let mut u = Vec::new();
                // Every horizon leaf stops a pending execution whose suffix is
                // not inspected, with or without a business cutoff row (M2).
                if f.truncated
                    || f.horizon
                    || abort.is_some()
                    || f.per.iter().any(|e| !e.ranges.is_empty())
                {
                    u.push("unreachable".into());
                }
                if completion_exceeded {
                    u.push("completion_graph".into());
                }
                u
            },
            postprocessing_diagnostics: if completion_exceeded {
                m.verify
                    .iter()
                    .filter(|v| v.property.completion())
                    .map(|v| diag::Diagnostic {
                        code: diag::U_SEM_0.into(),
                        severity: diag::Severity::Error,
                        message:
                            "completion graph edge limit exceeded; completion_graph is unchecked"
                                .into(),
                        span: Some(v.span),
                    })
                    .collect()
            } else {
                Vec::new()
            },
            contest_runtime,
            contest_summary,
            obligations: m.tracks_obligations().then(|| {
                f.per
                    .iter()
                    .enumerate()
                    .flat_map(|(k, e)| {
                        e.obligations.iter().cloned().map(move |mut row| {
                            row.case_id = k as u64 + 1;
                            row.id = row.id.replacen("case1:", &format!("case{}:", k + 1), 1);
                            row.coverage = coverage;
                            row
                        })
                    })
                    .collect()
            }),
            metrics: metrics::finish(f.metrics, m, coverage),
            case_outcomes: Some(case_outcomes),
            admission: Some(AdmissionSummary {
                rejected_count: rejected,
                overflow_count: m
                    .overflow_end
                    .map(|_| admission_range(&nodes, a.len(), false)),
                external_wait_max: m
                    .external_wait()
                    .then(|| admission_range(&nodes, a.len(), true)),
            }),
            properties,
            terminals: single_terminals,
            probability_scheduler: f.scheduler && f.chance,
            probability_policy_unresolved: m.start_policy.as_ref().filter(|p| p.extended()).map(
                |_| {
                    f.policy_unresolved
                        || f.horizon
                        || f.truncated
                        || f.numeric
                        || abort.is_some()
                        || f.per.iter().any(|e| !e.ranges.is_empty())
                },
            ),
            truncated: f.truncated,
            diagnostics,
        },
        trace,
    )
}

fn admission_range(nodes: &[Node], total: usize, peak: bool) -> Range {
    let mut result = Range {
        min: u64::MAX,
        max: 0,
    };
    for n in nodes {
        if !n.actions.is_empty() && !n.unknown {
            continue;
        }
        let s = &n.state;
        let count = if peak {
            s.external_peak.unwrap_or(0)
        } else {
            s.cases
                .iter()
                .filter(|c| matches!(c, Slot::Overflow(_)))
                .count() as u64
        };
        let unseen = if n.unknown {
            (total - s.cursor) as u64
        } else {
            0
        };
        let high = if peak {
            count.max(
                s.cases
                    .iter()
                    .filter(|c| matches!(c, Slot::Held(_)))
                    .count() as u64
                    + unseen,
            )
        } else {
            count + unseen
        };
        result.min = result.min.min(count);
        result.max = result.max.max(high);
    }
    if result.min == u64::MAX {
        Range {
            min: 0,
            max: total as u64,
        }
    } else {
        result
    }
}

#[cfg(test)]
pub(super) fn compatibility_counts(m: &ir::Model) -> (usize, usize, usize) {
    let a = arrivals(m).unwrap();
    let mut f = Facts::new(a.len());
    let nodes = graph(m, &a, DEFAULT_STATE_BUDGET, &mut f).unwrap();
    (nodes.len(), f.census.1, f.census.2)
}

#[cfg(test)]
#[path = "arrivals_scenario_tests.rs"]
mod scenario_impl_tests;

#[cfg(test)]
pub(super) fn trace_compatibility_at_budget(src: &str, m: &ir::Model, budget: usize) {
    let a = arrivals(m).unwrap();
    let mut plain = Facts::new(a.len());
    let mut traced = Facts::new(a.len());
    traced.trace = Some(trace::Capture::new(src, m));
    let left = graph(m, &a, budget, &mut plain);
    let right = graph(m, &a, budget, &mut traced);
    assert_eq!(left.as_ref().map(Vec::len), right.as_ref().map(Vec::len));
    assert_eq!(plain.census, traced.census, "budget{budget}");
}

#[cfg(test)]
mod simulation_legacy_arrivals_census {
    use super::*;

    // Project only absent B4 fields from the historical Debug fingerprint.
    // A present value is never erased; it instead fails this old-population
    // assertion. Production Debug and the frozen 26 hashes are unchanged.
    fn legacy_debug(out: &outcome::Outcome) -> String {
        assert!(
            out.case_outcomes
                .iter()
                .flatten()
                .all(|c| c.admitted_at.is_none())
        );
        if let Some(a) = &out.admission {
            assert!(a.overflow_count.is_none() && a.external_wait_max.is_none());
        }
        format!("{out:?}")
            .replace("metrics: None, ", "")
            .replace("admitted_at: None, ", "")
            .replace(", overflow_count: None", "")
            .replace(", external_wait_max: None", "")
    }

    #[test]
    fn simulation_legacy_arrivals_census() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut inputs = Vec::new();
        for name in ARRIVALS_FIXTURES {
            let source = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
            if let Ok(m) = crate::syntax::parse(&source).and_then(|a| crate::lower::lower(&a)) {
                inputs.push((name.to_owned(), m));
            }
        }
        assert_eq!(inputs.len(), 20);
        // Read the six exact source literals, not copies with an accidentally
        // different queue/cancel/clock. The review file itself remains frozen.
        let review = include_str!("../../tests/arrivals_fifo_queue_review.rs");
        for (i, source) in review.split("r#\"").skip(1).enumerate() {
            let source = source.split("\"#").next().unwrap();
            let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
            inputs.push((format!("review-{i}"), m));
        }
        assert_eq!(inputs.len(), 26);
        let mut rows = Vec::new();
        for (name, m) in inputs {
            assert!(!m.explicit_limits(), "new syntax in old census: {name}");
            assert!(m.metrics.is_none(), "old Debug projection excludes metrics");
            let a = arrivals(&m).unwrap();
            let mut f = Facts::new(a.len());
            let nodes = graph(&m, &a, DEFAULT_STATE_BUDGET, &mut f).unwrap();
            let (_, generated, held) = f.census;
            let budgets: Vec<_> = (0..=held + 1)
                .map(|b| format!("{b}: {}", legacy_debug(&explore(&m, b))))
                .collect();
            let mut configured = m.clone();
            configured.simulation = Some(ir::SimulationSpec {
                replications: 4,
                seed: 0,
                confidence: Rational::decimal("0.5").unwrap(),
                span: m.process.span,
                replications_span: m.process.span,
                seed_span: m.process.span,
                confidence_span: m.process.span,
            });
            assert_eq!(
                compatibility_counts(&configured),
                (nodes.len(), generated, held),
                "{name}"
            );
            for (b, expected) in budgets.iter().enumerate() {
                assert_eq!(
                    *expected,
                    format!("{b}: {}", legacy_debug(&explore(&configured, b))),
                    "{name}/budget{b}: simulation config is exhaustive-neutral"
                );
            }
            let fingerprint = budgets
                .join("\n")
                .bytes()
                .fold(0xcbf29ce484222325u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
                });
            rows.push(format!(
                "{name} {} {generated} {held} {fingerprint:016x}",
                nodes.len()
            ));
            println!("SIMULATION-ARRIVALS-BASELINE {}", rows.last().unwrap());
        }
        let actual = rows.join("\n");
        println!("SIMULATION-ARRIVALS-CENSUS\n{actual}");
        // Measured on 38108d0 with cfg(test) observers only, before ADR 0016
        // production semantics. These are compatibility data, not hand oracles.
        const BASELINE: &str = "arrivals-simultaneous 80 79 80 888b5117d1f03009
arrivals-simultaneous.broken-no-policy 152 151 152 544df8ff7921cace
arrivals-staggered 51 50 51 5cf0504169667b26
arrivals-staggered.broken-extra-capacity 51 50 51 1f8c4dc6399a33f1
arrivals-reject-mass 137 136 137 f36446c2344203ec
arrivals-reject-mass.broken-larger-queue 186 185 186 cca3f4c3083af54d
arrivals-same-time-slot 34 33 34 968be9a931ab07ff
arrivals-same-time-slot.broken-later-arrival 51 50 51 9cf13c0360cdfe11
arrivals-initial-product 195 200 196 304abc59fa4c8ba0
arrivals-initial-product.broken-fixed-initial 51 52 52 fdff604b8a2251c9
arrivals-relative-within 40 39 40 45228a6a9b614825
arrivals-relative-within.broken-tight-deadline 40 39 40 51e233d5ae7d058f
arrivals-count 34 33 34 53d0c6f8ac2b373d
arrivals-order 34 33 34 53d0c6f8ac2b373d
arrivals-required 34 33 34 53d0c6f8ac2b373d
arrivals-shared-fixed 34 33 34 528c4b72322b9551
arrivals-contracts 28 27 28 05b09e78738079ca
arrivals-contracts.broken-short-response 28 27 28 5741994c7ea03fa6
arrivals-zero-queue 12 11 12 d96698b5d5f9b9d6
arrivals-zero-queue.broken-positive-queue 34 33 34 7b2f1319f7742eff
review-0 73 74 74 b7bd1fee485ed2a5
review-1 72 71 72 0926d2f0fb6fc53f
review-2 14 13 14 b118175ccc604be9
review-3 88 93 89 68c705c2a32ecdba
review-4 302 315 303 efe08cfc69caa465
review-5 208 213 209 abfeaaaad4e81849";
        assert_eq!(actual, BASELINE);
    }
}

#[cfg(test)]
mod simulation_tests {
    use super::*;
    #[test]
    fn s_dedup_global_phase_cursor_preserved() {
        let m = crate::syntax::parse(include_str!("../../../../fixtures/sim-fifo/model.relca"))
            .and_then(|a| crate::lower::lower(&a))
            .unwrap();
        let s = initial(&m, 2);
        let mut sample = simulation::Sample::new(0, 1);
        sample
            .unique(&[
                vec![(Rational::one(), s.clone())],
                vec![(Rational::one(), s.clone())],
            ])
            .unwrap();
        for mutate in [
            |s: &mut Global| s.phase = Phase::Admission,
            |s: &mut Global| s.cursor = 1,
        ] {
            let mut t = s.clone();
            mutate(&mut t);
            assert!(
                simulation::Sample::new(0, 1)
                    .unique(&[
                        vec![(Rational::one(), s.clone())],
                        vec![(Rational::one(), t)]
                    ])
                    .is_err()
            );
        }
    }
}

#[cfg(test)]
mod arrivals_tests {
    use super::*;
    #[test]
    fn cf_global_and_engine_sample_keys_keep_stale_and_removed_task_loss() {
        let m = crate::syntax::parse(include_str!("../../../../fixtures/cf-version/model.relca"))
            .and_then(|a| crate::lower::lower(&a))
            .unwrap();
        let id = m.capabilities[0].version_check.as_ref().unwrap().variables[0];
        let (mut a, _, _) = seed(&m, 2);
        task(
            &mut a,
            0,
            0,
            Lifecycle::Running {
                finish: 1,
                ack: None,
            },
        );
        let mut b = a.clone();
        if let Slot::Admitted(c) = &mut b.cases[0] {
            c.local.tasks[0].stale.push(id);
        }
        assert_ne!(a, b);
        assert_eq!(HashSet::from([a.clone(), b.clone()]).len(), 2);
        assert!(
            simulation::Sample::new(1, 1)
                .unique(&[
                    vec![(Rational::one(), a.clone())],
                    vec![(Rational::one(), b)]
                ])
                .is_err()
        );
        if let Slot::Admitted(c) = &mut a.cases[0] {
            c.local.tasks.clear();
        }
        let mut b = a.clone();
        if let Slot::Admitted(c) = &mut b.cases[0] {
            c.local.lost_updates.push(id);
        }
        assert_ne!(a, b);
        assert_eq!(HashSet::from([a, b]).len(), 2);
    }
    #[test]
    fn resource_h05_global_atomic_candidate_set_never_reserves_partial_units() {
        let mut m = super::super::resource_tests::fixture("rsrc-atomic");
        m.metrics = None;
        m.actors[0].capacity = Some(3);
        m.capabilities.push(m.capabilities[0].clone());
        m.capabilities[2].uses.retain(|u| u.resource.0 == 0);
        m.capabilities[1].uses.retain(|u| u.resource.0 == 1);
        for reverse in [false, true] {
            if reverse {
                m.capabilities[0].uses.reverse();
            }
            let (mut local, p) = super::super::resource_tests::queued(&m);
            local.tasks.clear();
            local.results.resize(3, None);
            for (i, capability) in [1, 0, 2].into_iter().enumerate() {
                local.tasks.push(Task {
                    frozen_performance: None,
                    stale: Vec::new(),
                    binding: ir::BindingId(i),
                    capability: ir::CapabilityId(capability),
                    executor: None,
                    lifecycle: if i == 0 {
                        Lifecycle::Running {
                            finish: 3,
                            ack: None,
                        }
                    } else {
                        Lifecycle::Queued
                    },
                });
            }
            let (mut s, _, entry) = seed(&m, 1);
            if let Slot::Admitted(c) = &mut s.cases[0] {
                c.local = local;
                c.queued_at = vec![0; 3];
            }
            assert_eq!(s.resource_occupancy(&m).unwrap(), [0, 1]);
            let xs = transition(&s, &m, &[0], &p, entry, &mut Room(1000), &mut Facts::new(1))
                .unwrap()
                .unwrap();
            assert_eq!(xs.len(), 1);
            let Slot::Admitted(c) = &xs[0][0].1.cases[0] else {
                unreachable!()
            };
            assert_eq!(c.local.tasks[1].lifecycle, Lifecycle::Queued);
            assert!(c.local.tasks[2].lifecycle.running());
            assert_eq!(s.resource_occupancy(&m).unwrap(), [0, 1]);
        }
    }
    #[test]
    fn resource_h01_h08_global_observer_counts_all_cases_checked_and_includes_terminal_work() {
        for capacity in [2, 1] {
            let mut m = super::super::resource_tests::fixture("rsrc-global.broken-two-units");
            m.resources[0].capacity = capacity;
            let (mut s, _, _) = seed(&m, 2);
            for k in 0..2 {
                task(
                    &mut s,
                    k,
                    0,
                    Lifecycle::Running {
                        finish: 2,
                        ack: None,
                    },
                );
            }
            if let Slot::Admitted(c) = &mut s.cases[0] {
                c.terminal = Some(ir::EndId(0));
            }
            assert_eq!(s.resource_occupancy(&m).unwrap(), [2]);
            for cut in [false, true] {
                let mut facts = Facts::new(2);
                facts.truncated = cut;
                if cut {
                    facts.per[0]
                        .ranges
                        .push(super::super::resource_tests::range_witness());
                }
                facts.observe(&s, &m, &[0, 0]).unwrap();
                assert_eq!(facts.overcommit, capacity == 1);
            }
        }
        let mut m = super::super::resource_tests::fixture("rsrc-global");
        m.resources[0].capacity = u64::MAX;
        m.capabilities[0].uses[0].amount = u64::MAX;
        let (mut s, _, _) = seed(&m, 2);
        for k in 0..2 {
            task(
                &mut s,
                k,
                0,
                Lifecycle::Running {
                    finish: 2,
                    ack: None,
                },
            );
        }
        assert!(s.resource_occupancy(&m).is_err());
        assert!(Facts::new(2).observe(&s, &m, &[0, 0]).is_err());
    }

    #[test]
    fn resource_h10_fifo_head_blocks_unstartable_demand_but_not_other_actor() {
        let mut m = super::super::resource_tests::fixture("rsrc-global");
        m.metrics = None;
        m.capabilities.push(m.capabilities[0].clone());
        m.capabilities[0].uses[0].amount = 2;
        for other_actor in [false, true] {
            if other_actor {
                let mut a = m.actors[0].clone();
                a.name = "B".into();
                m.actors.push(a);
                m.capabilities[1].actor = ir::ActorId(1);
            }
            let (mut s, p, entry) = seed(&m, 2);
            for k in 0..2 {
                task(&mut s, k, 0, Lifecycle::Queued);
            }
            if let Slot::Admitted(c) = &mut s.cases[1] {
                c.local.tasks[0].capability = ir::CapabilityId(1);
            }
            assert_eq!(s.fifo_head(1, 0, &m), other_actor);
            let mut f = Facts::new(2);
            let xs = transition(&s, &m, &[0, 0], &p, entry, &mut Room(1000), &mut f).unwrap();
            let starts = xs.iter().flatten().flatten().any(|(_,s)| matches!(&s.cases[1],Slot::Admitted(c) if c.local.tasks[0].lifecycle.running()));
            assert_eq!(starts, other_actor);
            if !other_actor {
                assert!(xs.is_none());
            }
        }
    }

    #[test]
    fn resource_h03_h04_h06_global_actual_request_ack_and_queued_cancel_snapshots() {
        for name in ["rsrc-cooperative", "rsrc-immediate", "rsrc-queued-cancel"] {
            let mut m = super::super::resource_tests::fixture(name);
            m.arrivals = Some(vec![0]);
            let (s, p, entry) = seed(&m, 1);
            let mut stack = vec![s];
            let mut seen = HashSet::new();
            let (mut request, mut ack, mut queued_cancel) = (false, false, false);
            while let Some(s) = stack.pop() {
                if !seen.insert(s.clone()) {
                    continue;
                }
                let used = s.resource_occupancy(&m).unwrap()[0];
                let Slot::Admitted(c) = &s.cases[0] else {
                    unreachable!()
                };
                if name == "rsrc-queued-cancel" {
                    assert_eq!(used, 0);
                    if c.local
                        .tasks
                        .get(1)
                        .is_some_and(|t| t.lifecycle == Lifecycle::Done)
                    {
                        queued_cancel = true;
                    }
                } else {
                    if s.time == 2
                        && c.local.tasks.get(1).is_some_and(|t| {
                            matches!(t.lifecycle, Lifecycle::Running { ack: Some(_), .. })
                        })
                    {
                        assert_eq!(used, 1);
                        request = true;
                    }
                    let at = if name == "rsrc-immediate" { 2 } else { 3 };
                    if s.time == at
                        && c.local
                            .tasks
                            .get(1)
                            .is_some_and(|t| t.lifecycle == Lifecycle::Done)
                        && c.local.results[1].is_none()
                    {
                        assert_eq!(used, 0);
                        assert!(resources_fit(
                            &m.capabilities[2],
                            &s.resource_occupancy(&m).unwrap(),
                            &m
                        ));
                        ack = true;
                    }
                }
                if let Some(xs) = transition(
                    &s,
                    &m,
                    &[0],
                    &p,
                    entry,
                    &mut Room(10000),
                    &mut Facts::new(1),
                )
                .unwrap()
                {
                    stack.extend(xs.into_iter().flatten().map(|(_, s)| s));
                }
            }
            if name == "rsrc-queued-cancel" {
                assert!(queued_cancel);
            } else {
                assert!(request && ack);
            }
        }
    }
    /// Mutation N26: the budget refuses only a domain larger than the room
    /// left. A 0..1 domain has 2 values; with exactly 2 left it lists both
    /// (the check does not take from the room), with 1 left it is a budget halt.
    #[test]
    fn integer_case_domain_fits_a_room_equal_to_its_count() {
        let m = crate::integer_tests::model(&crate::integer_tests::latent(1));
        let xs = domain_support(&m, &m.state_variables[0], &mut Room(2), 0, 0);
        let values: Vec<_> = xs
            .expect("room equal to the count")
            .into_iter()
            .map(|(v, _)| v)
            .collect();
        assert_eq!(
            values,
            [ir::StateValue::Integer(0), ir::StateValue::Integer(1)]
        );
        assert!(matches!(
            domain_support(&m, &m.state_variables[0], &mut Room(1), 0, 0),
            Err(Halt::Budget)
        ));
    }
    #[test]
    fn integer_case_domain_product_is_reserved_before_clone_and_receipt() {
        let mut m = crate::integer_tests::model(&crate::integer_tests::latent(1));
        m.bounds.cases = 2;
        m.arrivals = Some(vec![0, 0]);
        let (mut s, _, _) = seed(&m, 2);
        s.phase = Phase::Nondet;
        for slot in &mut s.cases {
            if let Slot::Admitted(c) = slot {
                c.received = false;
            }
        }
        for budget in [0, 1, 3, 4, 5] {
            let mut facts = Facts::new(2);
            let mut room = Room(budget);
            let xs = initialize(&s, &m, &mut room, &mut facts, false);
            if budget < 4 {
                assert!(matches!(xs, Err(Halt::Budget)));
                assert_eq!(facts.clones, 0);
            } else {
                let xs = xs.unwrap();
                let tuples: HashSet<Vec<_>> = xs
                    .iter()
                    .map(|a| {
                        a[0].1
                            .cases
                            .iter()
                            .map(|slot| {
                                let Slot::Admitted(c) = slot else { panic!() };
                                assert!(!c.received);
                                c.local.state_values[0]
                            })
                            .collect()
                    })
                    .collect();
                assert_eq!(tuples.len(), 4);
                assert_eq!(room.0, budget - 4);
                assert_eq!(facts.clones, 4);
            }
        }
        let huge = crate::integer_tests::model(&crate::integer_tests::latent(u64::MAX));
        assert!(matches!(
            domain_support(&huge, &huge.state_variables[0], &mut Room(1), 0, 0),
            Err(Halt::Budget)
        ));
    }
    fn model(s: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap()
    }
    fn simple() -> ir::Model {
        model(include_str!(
            "../../../../fixtures/arrivals-count/model.relca"
        ))
    }
    fn seed(m: &ir::Model, n: usize) -> (Global, Vec<Instruction>, usize) {
        let mut program = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut program);
        let mut s = initial(m, n);
        s.cursor = n;
        s.phase = Phase::Mixed;
        for slot in &mut s.cases {
            let mut local = State::initial(m, entry);
            local.initialized = true;
            *slot = Slot::Admitted(
                Case {
                    admitted_at: None,
                    local,
                    queued_at: Vec::new(),
                    ever_started: false,
                    received: true,
                    terminal: None,
                }
                .into(),
            );
        }
        (s, program, entry)
    }
    fn task(s: &mut Global, k: usize, queued: u64, lifecycle: Lifecycle) {
        let Slot::Admitted(c) = &mut s.cases[k] else {
            unreachable!()
        };
        c.local.waiting = true;
        c.local.tasks = vec![Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: ir::BindingId(0),
            capability: ir::CapabilityId(0),
            lifecycle,
        }];
        c.queued_at = vec![queued];
        c.ever_started = lifecycle.running();
    }
    #[test]
    fn effect_end_open_identity_is_case_local() {
        let m = model(include_str!("../../../../fixtures/sov-cases/model.relca"));
        let (s, _, _) = seed(&m, 2);
        let mut identities = std::collections::HashSet::from([s.clone()]);
        for k in 0..2 {
            for open in [false, true] {
                let mut n = s.clone();
                let Slot::Admitted(c) = &mut n.cases[k] else {
                    unreachable!()
                };
                c.local.contest.as_mut().unwrap().end_open = Some(open);
                identities.insert(n);
            }
        }
        assert_eq!(identities.len(), 5);
    }
    #[test]
    fn effect_route_queue_identity_survives_normal_compaction_and_next_wait() {
        let src = include_str!("../../../../fixtures/sov-concurrent/model.relca").replace(
            "n = Clerk.notify();",
            "n = Clerk.notify(); n2 = Clerk.notify();",
        );
        let m = model(&src);
        let (mut s, p, entry) = seed(&m, 1);
        s.time = 5;
        let n = ir::BindingId(
            m.process
                .bindings
                .iter()
                .position(|b| b.name == "n")
                .unwrap(),
        );
        let pc = p.iter().position(|i| matches!(i, Instruction::Wait { calls, .. } if calls.iter().any(|c| c.binding == n))).unwrap();
        let normal = crate::responsibility::calls(&m)[&n].capability;
        let Slot::Admitted(c) = &mut s.cases[0] else {
            unreachable!()
        };
        c.local.pc = pc;
        c.local.time = 5;
        c.local.waiting = true;
        c.local.tasks = vec![
            Task {
                frozen_performance: None,
                stale: Vec::new(),
                binding: n,
                capability: normal,
                executor: Some(ir::PersonId(0)),
                lifecycle: Lifecycle::Done,
            },
            Task {
                frozen_performance: None,
                stale: Vec::new(),
                binding: m.contest_site(0),
                capability: m.contests()[0].route,
                executor: None,
                lifecycle: Lifecycle::Queued,
            },
        ];
        c.queued_at = vec![2, 3];
        let r = &mut c.local.contest.as_mut().unwrap().routes[0];
        r.target_at = Some(3);
        r.filed = Some(true);
        r.queued = true;
        let mut facts = Facts::new(1);
        for expected_len in [1, 2] {
            let actions = transition(&s, &m, &[0], &p, entry, &mut Room(100), &mut facts)
                .unwrap()
                .unwrap();
            assert_eq!(actions.len(), 1);
            s = actions[0][0].1.clone();
            let Slot::Admitted(c) = &s.cases[0] else {
                unreachable!()
            };
            assert_eq!(c.local.tasks.len(), expected_len);
            assert_eq!(c.local.tasks[0].binding, m.contest_site(0));
            assert_eq!(c.queued_at[0], 3);
            if expected_len == 2 {
                assert_eq!(c.queued_at[1], 5);
            }
        }
    }
    #[test]
    fn a_enter_time_fifo_not_arrival_and_direct_finish_times() {
        let mut m = simple();
        m.capabilities[0].duration = 1;
        m.bounds.clock = 10;
        let (mut s, p, entry) = seed(&m, 3);
        s.time = 4;
        task(&mut s, 0, 4, Lifecycle::Queued);
        task(&mut s, 1, 3, Lifecycle::Queued);
        task(
            &mut s,
            2,
            0,
            Lifecycle::Running {
                finish: 5,
                ack: None,
            },
        );
        assert!(!s.fifo_head(0, 0, &m));
        assert!(s.fifo_head(1, 0, &m));
        let a = [0, 1, 2];
        let mut f = Facts::new(3);
        for _ in 0..100 {
            let Some(actions) = transition(&s, &m, &a, &p, entry, &mut Room(1000), &mut f).unwrap()
            else {
                break;
            };
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].len(), 1);
            s = actions[0][0].1.clone();
        }
        for (k, t) in [(0, 7), (1, 6)] {
            let Slot::Admitted(c) = &s.cases[k] else {
                unreachable!()
            };
            assert!(c.terminal.is_some());
            assert_eq!(c.local.time, t);
        }
    }
    #[test]
    fn a_hash_all_semantic_fields_and_no_path_mass() {
        let m = simple();
        let (mut s, _, _) = seed(&m, 2);
        task(&mut s, 0, 0, Lifecycle::Queued);
        task(&mut s, 1, 0, Lifecycle::Queued);
        s.shared = vec![ir::StateValue::None];
        let mut variants = vec![s.clone()];
        let mut n = s.clone();
        n.phase = Phase::Admission;
        variants.push(n);
        let mut n = s.clone();
        n.cursor -= 1;
        variants.push(n);
        let mut n = s.clone();
        n.shared[0] = ir::StateValue::Value(ir::ValueId(0));
        variants.push(n);
        for k in 0..2 {
            let mut n = s.clone();
            if let Slot::Admitted(c) = &mut n.cases[k] {
                c.queued_at[0] = 1;
            }
            variants.push(n);
            let mut n = s.clone();
            if let Slot::Admitted(c) = &mut n.cases[k] {
                c.ever_started = true;
            }
            variants.push(n);
            let mut n = s.clone();
            if let Slot::Admitted(c) = &mut n.cases[k] {
                c.local.monitors.push(Monitor {
                    activated: true,
                    ..Monitor::default()
                });
            }
            variants.push(n);
            let mut n = s.clone();
            if let Slot::Admitted(c) = &mut n.cases[k] {
                c.local
                    .state_values
                    .push(ir::StateValue::Value(ir::ValueId(1)));
            }
            variants.push(n);
        }
        let count = variants.len();
        let keys: HashSet<_> = variants.into_iter().collect();
        assert_eq!(keys.len(), count);
        let weighted = [
            (Rational::one(), s.clone()),
            (Rational::decimal("0.5").unwrap(), s.clone()),
        ];
        assert_eq!(weighted[0].1, weighted[1].1);
    }
    fn product() -> ir::Model {
        model(
            r#"model M version "0.2" {
        latent case { x:bool ~ { true:0.5,false:0.5 };y:bool }
        process P { end done } arrivals fixed [0min,0min]
        bounds { cases 2;live_tasks 1;clock 5min } verify probability reached x == true <= 1 }"#,
        )
    }

    fn percase_admitted(m: &ir::Model) -> Global {
        let mut s = initial(m, 2);
        s.phase = Phase::Admission;
        let mut facts = Facts::new(2);
        transition(&s, m, &[0, 0], &[], 0, &mut Room(1), &mut facts)
            .unwrap()
            .unwrap()
            .remove(0)
            .remove(0)
            .1
    }
    fn local_values(s: &Global) -> Vec<Vec<ir::StateValue>> {
        s.cases
            .iter()
            .map(|c| match c {
                Slot::Admitted(c) => c.local.state_values.clone(),
                _ => panic!("expected admission"),
            })
            .collect()
    }
    #[test]
    fn percase_joint_support_room_and_zero_mass() {
        for (source, expected) in [
            (
                include_str!("../../../../fixtures/pcase-joint/model.relca"),
                ["16/25", "4/25", "4/25", "1/25"],
            ),
            (
                include_str!("../../../../fixtures/pcase-joint.broken-half/model.relca"),
                ["1/4"; 4],
            ),
        ] {
            let m = model(source);
            let s = percase_admitted(&m);
            let mut f = Facts::new(2);
            assert!(matches!(
                initialize(&s, &m, &mut Room(3), &mut f, true),
                Err(Halt::Budget)
            ));
            assert_eq!(f.clones, 0);
            let xs = initialize(&s, &m, &mut Room(4), &mut f, true).unwrap();
            assert_eq!((xs.len(), xs[0].len(), f.clones), (1, 4, 4));
            let mut support = BTreeMap::new();
            let mut sum = Rational::zero();
            for (p, n) in &xs[0] {
                assert_eq!(n.phase, Phase::Nondet);
                assert!(n.shared.iter().all(|v| *v == ir::StateValue::None));
                let values = local_values(n);
                support.insert(
                    format!("{:?},{:?}", values[0][0], values[1][0]),
                    p.to_string(),
                );
                sum.0 += &p.0;
            }
            assert_eq!(sum, Rational::one());
            for (a, b, p) in [
                (0, 0, expected[0]),
                (1, 0, expected[1]),
                (0, 1, expected[2]),
                (1, 1, expected[3]),
            ] {
                assert_eq!(
                    support[&format!("Value(ValueId({a})),Value(ValueId({b}))")],
                    p
                );
            }
            assert_ne!(
                xs[0][1].1, xs[0][2].1,
                "ordered case payload enters state identity"
            );
        }
        let m = model(
            &include_str!("../../../../fixtures/pcase-joint/model.relca")
                .replace("routine:0.8, difficult:0.2", "routine:0, difficult:1"),
        );
        let xs = initialize(
            &percase_admitted(&m),
            &m,
            &mut Room(1),
            &mut Facts::new(2),
            true,
        )
        .unwrap();
        assert_eq!((xs.len(), xs[0].len()), (1, 1));
        assert_eq!(xs[0][0].0, Rational::one());
    }
    #[test]
    fn percase_case_actor_one_layer_then_nondet() {
        let m = model(
            &include_str!("../../../../fixtures/pcase-case-product/model.relca")
                .replace("x: bool ~", "y: bool; x: bool ~"),
        );
        let s = percase_admitted(&m);
        let y = m
            .state_variables
            .iter()
            .position(|v| v.name == "y")
            .unwrap();
        let mut f = Facts::new(2);
        let xs = initialize(&s, &m, &mut Room(16), &mut f, true).unwrap();
        assert_eq!((xs.len(), xs[0].len()), (1, 16));
        let mut support = HashSet::new();
        let mut sum = Rational::zero();
        for (p, n) in &xs[0] {
            let values = local_values(n);
            assert!(
                values
                    .iter()
                    .enumerate()
                    .all(|(k, v)| v[y] == local_values(&s)[k][y]),
                "nondet placeholder is unchanged until its own layer"
            );
            assert_eq!(n.phase, Phase::Nondet);
            support.insert(values);
            sum.0 += &p.0;
            let choices = initialize(n, &m, &mut Room(4), &mut f, false).unwrap();
            assert_eq!(choices.len(), 4);
            assert_eq!(
                choices
                    .iter()
                    .map(|a| local_values(&a[0].1))
                    .collect::<HashSet<_>>()
                    .len(),
                4
            );
            for a in choices {
                assert_eq!(a.len(), 1);
                assert_eq!(a[0].0, Rational::one());
                assert_eq!(a[0].1.phase, Phase::Receipt);
            }
        }
        assert_eq!(support.len(), 16);
        assert_eq!(sum, Rational::one());
        let m = model(include_str!(
            "../../../../fixtures/pcase-nondet/model.relca"
        ));
        let xs = initialize(
            &percase_admitted(&m),
            &m,
            &mut Room(4),
            &mut Facts::new(2),
            false,
        )
        .unwrap();
        assert_eq!(xs.len(), 4);
        assert!(xs.iter().all(|a| a.len() == 1 && a[0].0 == Rational::one()));
        assert_eq!(
            xs.iter()
                .map(|a| local_values(&a[0].1))
                .collect::<HashSet<_>>()
                .len(),
            4
        );
    }
    #[test]
    fn percase_fixed_bank_view_store_and_actual_write_intermediate() {
        let m = model(include_str!(
            "../../../../fixtures/pcase-shared-fixed/model.relca"
        ));
        let mut s = percase_admitted(&m);
        let doc = m
            .state_variables
            .iter()
            .position(|v| v.name == "difficulty")
            .unwrap();
        let shared = m
            .state_variables
            .iter()
            .position(|v| v.name == "truth")
            .unwrap();
        assert_eq!(s.shared[doc], ir::StateValue::None);
        assert_ne!(s.shared[shared], ir::StateValue::None);
        assert_ne!(local_values(&s)[0][doc], ir::StateValue::None);
        assert_eq!(local_values(&s)[0][shared], ir::StateValue::None);
        let mut view = s.view(0, &m).unwrap();
        view.state_values[doc] = ir::StateValue::Value(ir::ValueId(1));
        s.store(0, view, &m);
        assert_eq!(s.shared[doc], ir::StateValue::None);
        assert_eq!(
            s.view(0, &m).unwrap().state_values[doc],
            ir::StateValue::Value(ir::ValueId(1))
        );
        assert_eq!(
            s.view(1, &m).unwrap().state_values[doc],
            ir::StateValue::Value(ir::ValueId(0))
        );

        let mut m = model(include_str!(
            "../../../../fixtures/pcase-write-ir/model.relca"
        ));
        let doc = ir::StateVarId(
            m.state_variables
                .iter()
                .position(|v| v.name == "difficulty")
                .unwrap(),
        );
        m.capabilities[0].kernel_writes.push(doc);
        for row in &mut m.capabilities[0].kernel.as_mut().unwrap().rows {
            row.branches[0]
                .overrides
                .push((doc, ir::StateValue::Value(ir::ValueId(1))));
        }
        arrivals(&m).unwrap();
        let mut p = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut p);
        let mut s = initial(&m, 2);
        let mut f = Facts::new(2);
        let mut intermediates = Vec::new();
        for _ in 0..100 {
            let Some(mut xs) =
                transition(&s, &m, &[0, 0], &p, entry, &mut Room(100), &mut f).unwrap()
            else {
                break;
            };
            assert_eq!((xs.len(), xs[0].len()), (1, 1));
            s = xs.remove(0).remove(0).1;
            if s.cases.iter().all(|s| matches!(s, Slot::Admitted(_))) {
                intermediates.push(
                    local_values(&s)
                        .iter()
                        .map(|v| v[doc.0])
                        .collect::<Vec<_>>(),
                );
            }
        }
        let r = ir::StateValue::Value(ir::ValueId(0));
        let d = ir::StateValue::Value(ir::ValueId(1));
        assert!(intermediates.contains(&vec![d, r]));
        assert_eq!(intermediates.last(), Some(&vec![d, d]));
        let (mut s, p, entry) = seed(&m, 2);
        s.cases[1] = Slot::Rejected;
        s.time = 1;
        task(
            &mut s,
            0,
            0,
            Lifecycle::Running {
                finish: 5,
                ack: Some(1),
            },
        );
        let xs = transition(
            &s,
            &m,
            &[0, 0],
            &p,
            entry,
            &mut Room(10),
            &mut Facts::new(2),
        )
        .unwrap()
        .unwrap();
        assert_eq!((xs.len(), xs[0].len()), (1, 1));
        assert_eq!(
            xs[0][0].1.view(0, &m).unwrap().state_values[doc.0],
            r,
            "cancel acknowledgement does not apply kernel override"
        );
    }
    #[test]
    fn percase_later_admission_never_reinitializes_received_payload() {
        let m = model(include_str!("../../../../fixtures/pcase-late/model.relca"));
        let (mut s, _, _) = seed(&m, 2);
        s.phase = Phase::Chance;
        s.time = 3;
        let d = ir::StateValue::Value(ir::ValueId(1));
        if let Slot::Admitted(c) = &mut s.cases[0] {
            c.local.state_values[0] = d;
            c.received = true;
        }
        if let Slot::Admitted(c) = &mut s.cases[1] {
            c.received = false;
        }
        let xs = initialize(&s, &m, &mut Room(2), &mut Facts::new(2), true).unwrap();
        assert_eq!(xs[0].len(), 2);
        for (_, n) in &xs[0] {
            assert_eq!(local_values(n)[0][0], d);
        }
        assert_eq!(
            xs[0].iter().map(|(p, _)| p.to_string()).collect::<Vec<_>>(),
            ["4/5", "1/5"]
        );
    }
    #[test]
    fn percase_static_assumptions_preserve_old_order_and_lineage() {
        let src = include_str!("../../../../fixtures/lin-single.broken-second-call/model.relca");
        let old = model(src);
        let marked = model(&src.replace("actor S {", "actor S { instance per_case;"));
        assert!(marked.actors[0].is_per_case());
        let mut assumptions = marked.assumptions.clone();
        assumptions
            .retain(|a| a.field != "S.instance" && a.field != "initial.actor_case_independence");
        assert_eq!(assumptions, old.assumptions);
        let mut agency = serde_json::to_value(
            crate::report::check(&src.replace("actor S {", "actor S { instance per_case;")).agency,
        )
        .unwrap();
        if let Some(rows) = agency
            .get_mut("common_causes")
            .and_then(|c| c.get_mut("causes"))
            .and_then(|c| c.as_array_mut())
        {
            for row in rows {
                row.as_object_mut().unwrap().remove("instance");
            }
        }
        assert_eq!(
            agency,
            serde_json::to_value(crate::report::check(src).agency).unwrap()
        );
    }
    #[test]
    fn a_room_init_joint_then_nondet_receipts_rejection_and_future() {
        let m = product();
        let (mut s, p, entry) = seed(&m, 2);
        s.phase = Phase::Chance;
        for slot in &mut s.cases {
            if let Slot::Admitted(c) = slot {
                c.received = false;
            }
        }
        let mut f = Facts::new(2);
        assert!(matches!(
            initialize(&s, &m, &mut Room(3), &mut f, true),
            Err(Halt::Budget)
        ));
        assert_eq!(f.clones, 0);
        let xs = initialize(&s, &m, &mut Room(4), &mut f, true).unwrap();
        assert_eq!(xs.len(), 1);
        assert_eq!(xs[0].len(), 4);
        assert_eq!(f.clones, 4);
        let mut tuples = HashSet::new();
        for (weight, s) in &xs[0] {
            assert_eq!(weight.to_string(), "1/4");
            assert_eq!(s.phase, Phase::Nondet);
            tuples.insert(
                s.cases
                    .iter()
                    .map(|slot| match slot {
                        Slot::Admitted(c) => {
                            assert!(!c.received);
                            c.local.state_values[0]
                        }
                        _ => unreachable!(),
                    })
                    .collect::<Vec<_>>(),
            );
            let choices = initialize(s, &m, &mut Room(4), &mut f, false).unwrap();
            assert_eq!(choices.len(), 4);
            for choice in choices {
                assert_eq!(choice.len(), 1);
                assert_eq!(choice[0].0, Rational::one());
                let r = transition(&choice[0].1, &m, &[0, 0], &p, entry, &mut Room(1), &mut f)
                    .unwrap()
                    .unwrap();
                assert!(
                    r[0][0]
                        .1
                        .cases
                        .iter()
                        .all(|slot| matches!(slot,Slot::Admitted(c)if c.received))
                );
            }
        }
        assert_eq!(tuples.len(), 4);
        assert_eq!(f.per[0].state_events.len(), 16);
        assert_eq!(f.per[1].state_events.len(), 16);
        s.cases = vec![Slot::Rejected, Slot::Future];
        let before = f.clones;
        let choices = initialize(&s, &m, &mut Room(1), &mut f, true).unwrap();
        assert_eq!(choices[0].len(), 1);
        assert_eq!(f.clones - before, 1);
        assert!(matches!(choices[0][0].1.cases[1], Slot::Future));
        let mut denied = m.clone();
        denied.admission_queue = Some(ir::AdmissionQueue {
            capacity: 0,
            overflow: ir::Overflow::Reject,
        });
        let out = explore(&denied, 1000);
        assert!(
            out.case_outcomes
                .unwrap()
                .iter()
                .all(|c| c.terminals[0].kind == Kind::Rejected)
        );
    }
    #[test]
    fn a_duration_cross_case_joint_survivor_ack_and_kernel_not_predrawn() {
        let m = model(
            r#"model M version "0.2" {type T=enum{ok} observed case{hit:bool=false}
        kernel K{* -> {0.5:ok with hit=true;0.5:ok with hit=false}}
        actor A{capacity 2;capability run()->T{duration{1min:0.5,3min:0.5};writes hit;outcomes kernel K}}
        process P{r=A.run();end done} arrivals fixed[0min,0min] bounds{cases 2;live_tasks 1;clock 10min} verify probability reached hit == true <= 1}"#,
        );
        let (mut s, p, entry) = seed(&m, 2);
        s.time = 1;
        s.phase = Phase::Duration;
        for k in 0..2 {
            task(
                &mut s,
                k,
                0,
                Lifecycle::DistributedRunning {
                    started: 0,
                    next: 0,
                    due: false,
                    ack: Some(1),
                },
            );
        }
        let mut f = Facts::new(2);
        assert!(matches!(
            duration(&s, &m, &mut Room(3), &mut f),
            Err(Halt::Budget)
        ));
        assert_eq!(f.clones, 0);
        let xs = duration(&s, &m, &mut Room(4), &mut f).unwrap();
        assert_eq!(xs[0].len(), 4);
        for (p, s) in &xs[0] {
            assert_eq!(p.to_string(), "1/4");
            assert_eq!(s.phase, Phase::Admission);
            for slot in &s.cases {
                if let Slot::Admitted(c) = slot {
                    assert!(matches!(
                        c.local.tasks[0].lifecycle,
                        Lifecycle::DistributedRunning { ack: Some(1), .. }
                    ));
                }
            }
        }
        let mut survivor = xs[0].last().unwrap().1.clone();
        survivor.phase = Phase::Duration;
        let again = duration(&survivor, &m, &mut Room(1), &mut f).unwrap();
        assert_eq!(again[0].len(), 1);
        let mut both = xs[0][0].1.clone();
        both.phase = Phase::Mixed;
        assert_eq!(f.per.iter().map(|e| e.kernel_draws).sum::<usize>(), 0);
        let actions = transition(&both, &m, &[0, 0], &p, entry, &mut Room(10), &mut f)
            .unwrap()
            .unwrap();
        assert!(
            actions.iter().any(
                |a| matches!(&a[0].1.cases[0],Slot::Admitted(c)if c.local.results[0].is_some())
            )
        );
        assert!(actions.iter().any(|a|matches!(&a[0].1.cases[0],Slot::Admitted(c)if c.local.tasks[0].lifecycle==Lifecycle::Done && c.local.results[0].is_none())));
        assert_eq!(f.per.iter().map(|e| e.kernel_draws).sum::<usize>(), 2);
        assert_eq!(actions.iter().filter(|a| a.len() == 2).count(), 2);
    }
    fn contracts() -> ir::Model {
        model(
            r#"model M version "0.2" { type T=enum { ok } people [p,q]
        actor A { executors [p,q]; capability run()->T { duration fixed 1min;result nondet {ok} } }
        actor B { executors [p,q]; capability run()->T { duration fixed 1min;result nondet {ok} } }
        process P { a=A.run();b=B.run();end done } arrivals fixed [0min,0min]
        bounds {cases 2;live_tasks 1;clock 10min}
        contract C { separation A.run.finish,B.run.finish;requires_event B.run.start after A.run.finish } }"#,
        )
    }
    #[test]
    fn a_separation_case_and_requires_case_direct_bundles() {
        let m = contracts();
        let (mut s, _, _) = seed(&m, 2);
        let mut f = Facts::new(2);
        for (k, p, q) in [
            (0, ir::PersonId(0), ir::PersonId(1)),
            (1, ir::PersonId(1), ir::PersonId(0)),
        ] {
            let mut v = s.view(k, &m).unwrap();
            emit(
                &mut v,
                Bundle::Finish(ir::CapabilityId(0), ir::ValueId(0), Some(p)),
                &m,
                &mut f.per[k],
            )
            .unwrap();
            emit(
                &mut v,
                Bundle::Finish(ir::CapabilityId(1), ir::ValueId(0), Some(q)),
                &m,
                &mut f.per[k],
            )
            .unwrap();
            s.store(k, v, &m);
        }
        assert!(!f.per[0].violation[0]);
        assert!(!f.per[1].violation[0]);
        let mut v = s.view(1, &m).unwrap();
        emit(
            &mut v,
            Bundle::Finish(ir::CapabilityId(1), ir::ValueId(0), Some(ir::PersonId(1))),
            &m,
            &mut f.per[1],
        )
        .unwrap();
        assert!(f.per[1].violation[0]);
        assert!(!f.per[0].violation[0]);
        let (s, _, _) = seed(&m, 2);
        let mut f = Facts::new(2);
        emit(
            &mut s.view(0, &m).unwrap(),
            Bundle::Finish(ir::CapabilityId(0), ir::ValueId(0), Some(ir::PersonId(0))),
            &m,
            &mut f.per[0],
        )
        .unwrap();
        emit(
            &mut s.view(1, &m).unwrap(),
            Bundle::Start(ir::CapabilityId(1), Some(ir::PersonId(1))),
            &m,
            &mut f.per[1],
        )
        .unwrap();
        assert!(!f.per[0].activation[1]);
        assert!(f.per[1].violation[1]);
    }
    #[test]
    fn d1_a4_direct_case_events_do_not_share_waivers_or_exclusions() {
        let src = include_str!("../../../../fixtures/ctx-waive/model.relca")
            .replace("cases 1", "cases 2")
            .replace("  bounds", "  arrivals fixed [0min,10min]\n  bounds")
            .replace(
                "    waive",
                "    excludes Officer.release.finish, B.run.start\n    waive",
            );
        let m = model(&src);
        let (mut s, _, _) = seed(&m, 2);
        let mut f = Facts::new(2);
        for k in 0..2 {
            let mut v = s.view(k, &m).unwrap();
            v.time = k as u64 * 10;
            emit(&mut v, Bundle::Received, &m, &mut f.per[k]).unwrap();
            s.store(k, v, &m);
        }
        s.time = 1;
        let mut first = s.view(0, &m).unwrap();
        emit_at(
            &mut first,
            Bundle::Finish(ir::CapabilityId(0), ir::ValueId(0), Some(ir::PersonId(0))),
            Some(ir::BindingId(0)),
            &m,
            &mut f.per[0],
        )
        .unwrap();
        s.store(0, first, &m);
        s.time = 11;
        let mut second = s.view(1, &m).unwrap();
        assert!(second.obligations.as_ref().unwrap()[0].resolution.is_none());
        emit_at(
            &mut second,
            Bundle::Start(ir::CapabilityId(1), None),
            Some(ir::BindingId(1)),
            &m,
            &mut f.per[1],
        )
        .unwrap();
        assert!(!second.monitors[1].violated);
        assert!(!second.monitors[1].before_seen);
        let first = s.view(0, &m).unwrap();
        assert_eq!(
            first.obligations.as_ref().unwrap()[0]
                .resolution
                .as_ref()
                .unwrap()
                .status,
            crate::obligation::Status::Waived
        );
        assert!(first.monitors[1].before_seen);
        s.time = 11;
        second.time = 11;
        emit_at(
            &mut second,
            Bundle::Finish(ir::CapabilityId(0), ir::ValueId(0), Some(ir::PersonId(0))),
            Some(ir::BindingId(0)),
            &m,
            &mut f.per[1],
        )
        .unwrap();
        assert_eq!(
            first.obligations.as_ref().unwrap()[0]
                .resolution
                .as_ref()
                .unwrap()
                .resolved_at,
            Some(1)
        );
        assert_eq!(
            second.obligations.as_ref().unwrap()[0]
                .resolution
                .as_ref()
                .unwrap()
                .resolved_at,
            Some(11)
        );
    }
    #[test]
    fn a_cut_reasons_numeric_cycle_and_abort_metrics() {
        let m = model(include_str!(
            "../../../../fixtures/arrivals-relative-within/model.relca"
        ));
        let (mut s, _, _) = seed(&m, 2);
        s.time = 5;
        let a = [5, 6];
        let mut node = Node {
            state: Rc::new(s),
            actions: Vec::new(),
            unknown: true,
            cut: ProbabilityCut::StateBudget,
            numeric: false,
        };
        let mut q = m.probability_queries[0].clone();
        q.within = Some(0);
        let (b, _) = solve(&[node.clone()], &[0], 0, None, Some(&q), &m, &a).unwrap();
        assert_eq!(b[3], Rational::one());
        let mut clock = m.clone();
        clock.bounds.clock = 5;
        node.cut = ProbabilityCut::Clock;
        let (b, _) = solve(&[node.clone()], &[0], 0, None, Some(&q), &clock, &a).unwrap();
        assert_eq!(b[3], Rational::zero());
        node.numeric = true;
        let (b, numeric) = solve(&[node.clone()], &[0], 0, None, Some(&q), &clock, &a).unwrap();
        assert!(numeric);
        assert_eq!(b[3], Rational::one());
        node.actions = vec![vec![(Rational::one(), 0)]];
        assert!(matches!(topology(&[node]), Err(Halt::Unsupported(_))));
        let s = r#"model M version "0.2" {type T=enum {ok} actor A{capacity 1;capability run()->T{duration fixed 1min;result nondet{ok}}}
        process P{a=A.run();end done} arrivals fixed[0min,2min] bounds{cases 2;live_tasks 1;clock 5min}
        contract C{response case.received -> one_of(end.done) by 18446744073709551615min after case.received otherwise unresolved} verify reachable end.done}"#;
        let out = explore(&model(s), 1000);
        assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
        assert!(
            out.case_outcomes
                .unwrap()
                .iter()
                .all(|c| c.terminals.is_empty())
        );
        assert_eq!(
            out.properties.last().unwrap().cases.as_ref().unwrap()[0].verdict,
            Verdict::Satisfied
        );
    }

    #[test]
    fn a_numeric_denominator_limit_and_zero_mass_not_materialized() {
        let tiny = format!("0.{}1", "0".repeat(62));
        let rest = format!("0.{}", "9".repeat(63));
        let mut body = String::new();
        for i in 0..20 {
            body.push_str(&format!("x{i}=A.flip();if x{i} == pass {{ "));
        }
        body.push_str("m=A.mark();end yes");
        body.push_str(&" } else { end no }".repeat(20));
        let source = format!(
            r#"model M version "0.2" {{type T=enum{{pass,fail}} observed case{{hit:bool=false}}
        kernel K{{* -> {{{tiny}:pass;{rest}:fail}}}}
        actor A{{capacity 1;capability flip()->T{{duration fixed 1min;outcomes kernel K}} capability mark()->T{{duration fixed 1min;result nondet{{pass}};writes hit;finish{{hit=true}}}}}}
        process P{{{body}}} arrivals fixed[0min] bounds{{cases 1;live_tasks 1;clock 25min}}
        verify safety no_resource_overcommit verify probability reached hit == true <= 1 }}"#
        );
        let out = explore(&model(&source), 10000);
        assert!(out.truncated);
        assert!(out.diagnostics.iter().any(|d| d.code == diag::W_BUDGET_0));
        assert!(
            out.properties
                .iter()
                .all(|p| p.verdict == Verdict::Inconclusive)
        );
        let mut m = model(include_str!("../../../../fixtures/prob-zero/model.relca"));
        m.arrivals = Some(vec![0]);
        let mut f = Facts::new(1);
        let nodes = graph(&m, &[0], 1000, &mut f).unwrap();
        assert!(!nodes.is_empty());
        assert_eq!(f.per[0].kernel_clones, 1);
        assert_eq!(
            explore(&m, 1000)
                .properties
                .iter()
                .find_map(|p| p.cases.as_ref().and_then(|c| c[0].probability.as_ref()))
                .unwrap()
                .max,
            Rational::zero()
        );
    }
    #[test]
    fn resource_review_global_leaf_without_resource_table_fails_closed() {
        let src = include_str!("../../../../fixtures/rsrc-global/model.relca");
        let m = super::super::resource_tests::fixture("rsrc-global");
        assert!(!m.capabilities[0].uses.is_empty());
        let (mut s, _, _) = seed(&m, 1);
        task(&mut s, 0, 0, Lifecycle::Queued);
        let mut control = trace::Capture::new(src, &m);
        control.global_context(&s, &m);
        assert!(control.shared_model_resources.is_some());
        control.global_leaf(&s, &m, crate::explain::Kind::Deadlock);
        assert_eq!(control.failure, None);
        let mut c = trace::Capture::new(src, &m);
        assert!(c.shared_model_resources.is_none());
        c.global_leaf(&s, &m, crate::explain::Kind::Deadlock);
        assert_eq!(c.failure, Some(crate::explain::ReplayReason::StateMismatch));
    }
}

#[cfg(test)]
mod d2_joint_tests {
    use super::*;
    #[test]
    fn d2_f10_contains_all_four_joint_case_choices() {
        let m = crate::lower::lower(
            &crate::syntax::parse(include_str!(
                "../../../../fixtures/ctx-contest-resource/model.relca"
            ))
            .unwrap(),
        )
        .unwrap();
        let times = arrivals(&m).unwrap();
        let mut facts = Facts::new(2);
        let nodes = graph(&m, &times, 10000, &mut facts).unwrap();
        let mut masks = BTreeSet::new();
        for n in &nodes {
            let choices: Option<Vec<_>> = n
                .state
                .cases
                .iter()
                .map(|slot| match slot {
                    Slot::Admitted(c) => c.local.contest.as_ref().and_then(|c| c.routes[0].filed),
                    _ => None,
                })
                .collect();
            if let Some(c) = choices {
                masks.insert((c[0], c[1]));
            }
        }
        assert_eq!(
            masks,
            BTreeSet::from([(false, false), (false, true), (true, false), (true, true)])
        );
        for budget in [0, 1, 2, 3, 10, 30, 100] {
            let mut facts = Facts::new(2);
            let _ = graph(&m, &times, budget, &mut facts);
            assert!(
                facts.census.2 <= budget.max(1),
                "budget {budget}: {:?}",
                facts.census
            );
        }
    }
}
