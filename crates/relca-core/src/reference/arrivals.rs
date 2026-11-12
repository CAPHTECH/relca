//! Independent, unmerged tree semantics for ADR 0015. Minute ticks, past-task
//! intervals, queue observations, and trace suffixes; no engine code is used.
use super::*;

#[cfg(test)]
pub(crate) mod q6_private;
#[path = "response_arrivals.rs"]
mod response_arrivals;
#[path = "simulation.rs"]
pub(super) mod simulation;
use crate::outcome::CaseTerminalKind as Kind;
// Case index, historical Task index, and conditional finish probability.
type FinishCoin = (usize, usize, Rational);
#[cfg(test)]
#[path = "arrivals_policy_tests.rs"]
mod fixed_policy_tests;

#[derive(Clone, Copy)]
enum Boundary {
    Coins,
    Admit,
    Draw,
    Choose,
    Receive,
    Activate,
    Run,
}
#[derive(Clone)]
struct QueueEntry {
    task: usize,
    at: Tick,
}
#[derive(Clone)]
struct CaseFrame<'a> {
    frame: Frame<'a>,
    queue: Vec<QueueEntry>,
    receipt: bool,
    end: Option<EndId>,
}
impl CaseFrame<'_> {
    fn stopped(&self, m: &Model) -> bool {
        if self.frame.trace.iter().any(|o| matches!(o.event, Occurrence::ResponseStopped(id) if id.0 < m.process.bindings.len())) { return true; }
        let Some(_end) = self.end else {
            return false;
        };
        self.frame.tasks.iter().all(Task::terminal)
            && m.contests()
                .iter()
                .enumerate()
                .filter(|(id, _)| self.frame.contest_target(m, *id).is_some())
                .all(|(id, _)| {
                    self.frame
                        .trace
                        .iter()
                        .find_map(|o| match o.event {
                            Occurrence::ContestChoice(n, filed) if n == id => Some(filed),
                            _ => None,
                        })
                        .is_some_and(|filed| {
                            !filed
                                || self
                                    .frame
                                    .trace
                                    .iter()
                                    .any(|o| matches!(o.event,Occurrence::ContestQueue(n) if n==id))
                        })
                })
    }
}
#[derive(Clone)]
// A world is already one charged frame; inline case payloads avoid allocating
// an extra box per case at every branch of the unmerged reference tree.
#[allow(clippy::large_enum_variant)]
enum CaseSlot<'a> {
    NotHere,
    Declined,
    Here(CaseFrame<'a>),
    Held(CaseFrame<'a>),
    Overflow(CaseFrame<'a>),
}
impl<'a> CaseSlot<'a> {
    fn payload(&self) -> Option<&CaseFrame<'a>> {
        match self {
            Self::Here(c) | Self::Held(c) | Self::Overflow(c) => Some(c),
            _ => None,
        }
    }
    fn payload_mut(&mut self) -> Option<&mut CaseFrame<'a>> {
        match self {
            Self::Here(c) | Self::Held(c) | Self::Overflow(c) => Some(c),
            _ => None,
        }
    }
}
#[derive(Clone)]
struct World<'a> {
    external_peak: u64,
    now: Tick,
    next: usize,
    boundary: Boundary,
    shared: Vec<StateValue>,
    cases: Vec<CaseSlot<'a>>,
    steps: usize,
}

fn metric_observation(
    m: &Model,
    w: &World<'_>,
    times: &[Tick],
    cut: bool,
    horizon: bool,
) -> crate::metrics::BusinessMetrics {
    let _timer = crate::perf::span("reference.metric_snapshot");
    let records: Vec<_> = w
        .cases
        .iter()
        .enumerate()
        .map(|(i, slot)| {
            let (tasks, terminal, rejected, deadlock, future) = match slot {
                CaseSlot::NotHere => ([].as_slice(), None, false, false, true),
                CaseSlot::Declined => ([].as_slice(), None, true, false, false),
                CaseSlot::Here(c) | CaseSlot::Held(c) | CaseSlot::Overflow(c) => (
                    c.frame.tasks.as_slice(),
                    c.frame.business.or_else(|| c.end.map(|e| (e, c.frame.now))),
                    false,
                    !cut && c.end.is_none() && !c.stopped(m),
                    false,
                ),
            };
            metrics::Observation {
                condition_intervals: slot
                    .payload()
                    .map_or_else(Vec::new, |c| c.frame.condition_waits.clone()),
                admitted_at: match slot {
                    CaseSlot::Here(c) => {
                        if m.explicit_limits() {
                            c.frame.trace.iter().find_map(|o| {
                                matches!(o.event, Occurrence::Admission).then_some(o.at)
                            })
                        } else {
                            Some(times[i])
                        }
                    }
                    _ => None,
                },
                wait_intervals: slot.payload().map_or_else(Vec::new, |c| {
                    c.frame
                        .trace
                        .iter()
                        .filter_map(|o| match o.event {
                            Occurrence::Wait(end) => Some((o.at, end)),
                            _ => None,
                        })
                        .collect()
                }),
                intentional_wait: match slot {
                    CaseSlot::Here(c) => c.frame.intentional_wait(w.now),
                    _ => 0,
                },
                tasks,
                arrival: times[i],
                terminal,
                rejected,
                deadlock,
                future,
            }
        })
        .collect();
    let mut point = metrics::project_with_horizon(m, &records, w.now, horizon);
    if !m.response_attempts().is_empty() {
        let unknown = w
            .cases
            .iter()
            .filter(|s| {
                s.payload().is_some_and(|c| {
                    c.frame
                        .trace
                        .iter()
                        .any(|o| matches!(o.event, Occurrence::ResponseStopped(_)))
                })
            })
            .count() as u64;
        point.overall.counts.receipt_unknown_count = Some(metrics::count(unknown));
    }
    if m.explicit_limits() {
        let mut admitted = 0;
        let mut overflowed = 0;
        for (k, (row, slot)) in point.cases.iter_mut().zip(&w.cases).enumerate() {
            let at = slot.payload().and_then(|c| {
                c.frame
                    .trace
                    .iter()
                    .find_map(|o| matches!(o.event, Occurrence::Admission).then_some(o.at))
            });
            let overflow = matches!(slot, CaseSlot::Overflow(_));
            admitted += u64::from(at.is_some());
            overflowed += u64::from(overflow);
            row.counts.admitted = Some(metrics::count(u64::from(at.is_some())));
            row.counts.overflowed = m.overflow_end.map(|_| metrics::count(u64::from(overflow)));
            row.external_wait = m.external_wait().then(|| {
                at.map_or_else(
                    || {
                        metrics::absent(if !cut && matches!(slot, CaseSlot::Held(_)) {
                            crate::metrics::Reason::Deadlock
                        } else {
                            crate::metrics::Reason::Unfinished
                        })
                    },
                    |at| metrics::amount(at - times[k]),
                )
            });
        }
        point.overall.counts.admitted = Some(metrics::count(admitted));
        point.overall.counts.overflowed = m.overflow_end.map(|_| metrics::count(overflowed));
    }
    point
}
#[derive(Clone)]
struct Bound {
    endpoints: [Rational; 4],
    numeric: bool,
    computational: bool,
}
impl Bound {
    fn exact(x: bool) -> Self {
        Self {
            endpoints: std::array::from_fn(|_| if x { Rational::one() } else { Rational::zero() }),
            numeric: false,
            computational: false,
        }
    }
    fn unknown(numeric: bool) -> Self {
        Self {
            endpoints: [
                Rational::zero(),
                Rational::one(),
                Rational::zero(),
                Rational::one(),
            ],
            numeric,
            computational: true,
        }
    }
}
struct Tree<'a> {
    response_exercise: BTreeMap<CapabilityId, (usize, usize)>,
    response_receipts: Vec<crate::outcome::ResponseReceipt>,
    first_hits: Vec<Vec<Option<u128>>>,
    first_closed: Vec<Vec<bool>>,
    first_trees: Vec<super::quality::FirstTree>,
    #[cfg(test)]
    resource_worlds: Option<Vec<World<'a>>>,
    depth: usize,
    m: &'a Model,
    limits: Limits,
    times: Vec<Tick>,
    cases: Vec<Interpreter<'a>>,
    terminals: Vec<BTreeMap<Kind, CaseTerminal>>,
    unresolved: Vec<bool>,
    stats: Statistics,
    truncated: bool,
    numeric: bool,
    deadlock: bool,
    overcommit: bool,
    chance: bool,
    scheduler: bool,
    policy_unresolved: bool,
    rejects: Option<Range>,
    overflows: Option<Range>,
    external_peaks: Option<Range>,
    admissions: Vec<Option<Range>>,
    unadmitted: Vec<bool>,
    #[cfg(test)]
    initial_leaves: Vec<(Tick, Vec<Vec<StateValue>>, Rational)>,
    #[cfg(test)]
    receipts: Vec<(usize, Tick, Vec<StateValue>)>,
    #[cfg(test)]
    duration_leaves: Vec<(Rational, Vec<Option<Tick>>)>,
}
fn interpreter(m: &Model, limits: Limits) -> Interpreter<'_> {
    Interpreter {
        case_index: 0,
        probability_stack: Vec::new(),
        probability_result: None,
        probability_scheduler: false,
        probability_chance: false,
        denominator_bits: 4096,
        duration_numeric_cut: false,
        model: m,
        limits,
        evidence: Evidence::default(),
        terminals: BTreeMap::new(),
        statistics: Statistics::default(),
        contract_facts: vec![(false, false); m.contracts.len()],
    }
}
// Reference ownership classification does not use engine state or helpers.
pub(super) fn shared_latent(m: &Model, kind: StateKind) -> bool {
    match kind {
        StateKind::ActorLatent(a) => m
            .actors
            .get(a.0)
            .is_some_and(|a| a.metadata.instance.is_none()),
        _ => false,
    }
}

pub(super) fn shared_state(m: &Model, kind: StateKind) -> bool {
    match kind {
        StateKind::ActorLatent(a) | StateKind::ActorObserved(a) => m
            .actors
            .get(a.0)
            .is_some_and(|a| a.metadata.instance.is_none()),
        _ => false,
    }
}

pub(super) fn shared_guard(m: &Model, c: &Cond) -> bool {
    match c {
        Cond::StateEq(x) => {
            matches!(m.state_variables[x.variable.0].kind, StateKind::ActorObserved(a) if m.actors[a.0].metadata.instance.is_none())
        }
        Cond::IntCompare { variable, .. } | Cond::Contains { set: variable, .. } => {
            matches!(m.state_variables[variable.0].kind, StateKind::ActorObserved(a) if m.actors[a.0].metadata.instance.is_none())
        }
        Cond::And(cs) | Cond::Or(cs) => cs.iter().any(|c| shared_guard(m, c)),
        Cond::Eq(_) => false,
    }
}

#[cfg(test)]
mod rule_tests {
    use super::*;
    #[test]
    fn fixed_rule_guard_adds_zero_reference_shared_read_candidates() {
        let src = r#"model M version "0.2" { actor R { version "r1"; decides enabled: bool = true } process P { if R.enabled == true { end yes } else { end no } } cut C { include [R] } bounds { cases 2; live_tasks 2; clock 2min } arrivals fixed [0min,0min] verify safety no_resource_overcommit }"#;
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        assert!(!shared_state(&m, m.state_variables[0].kind));
        let Stmt::If { condition, .. } = &m.process.body[0].kind else {
            panic!()
        };
        assert!(!shared_guard(&m, condition));
        let frames = [
            super::super::replay::initial(&m),
            super::super::replay::initial(&m),
        ];
        let candidates = frames.iter().filter(|f| f.control.iter().rev().find_map(|b| b.first()).is_some_and(|s| matches!(&s.kind, Stmt::If { condition, .. } if shared_guard(&m, condition)))).count();
        assert_eq!(candidates, 0);
        assert_eq!(super::super::explore(&m).stop, StopReason::Complete);
    }
}

pub(super) fn validate_observed_writers(m: &Model) -> Execution {
    for c in &m.capabilities {
        if let Some(check) = &c.version_check {
            require(!check.variables.is_empty() && c.kernel.is_none())?;
            require(
                m.types
                    .get(c.return_type.0)
                    .is_some_and(|t| check.reject.0 < t.members.len()),
            )?;
            require(
                !c.results.contains(&check.reject)
                    && c.requires
                        .as_ref()
                        .is_none_or(|r| r.otherwise != check.reject),
            )?;
            let mut names = BTreeSet::new();
            for id in &check.variables {
                require(names.insert(*id))?;
                let v = m
                    .state_variables
                    .get(id.0)
                    .ok_or(StopReason::SemanticAbort)?;
                require(v.kind == StateKind::ActorObserved(c.actor) && shared_state(m, v.kind))?;
                require(
                    c.finish_writes().any(|w| w.variable == *id)
                        && !c.start_writes.iter().any(|w| w.variable == *id),
                )?;
            }
            require(check.variables.windows(2).all(|v| v[0] < v[1]))?;
        }
    }
    let mut table = Vec::new();
    for (i, v) in m.state_variables.iter().enumerate() {
        let id = StateVarId(i);
        require(
            m.verify
                .iter()
                .filter(|p| p.property == Property::NoLostUpdate(id))
                .count()
                == usize::from(v.last_write_wins),
        )?;
        if v.last_write_wins {
            require(matches!(v.kind, StateKind::ActorObserved(_)) && shared_state(m, v.kind))?;
        }
        if m.bounds.cases < 2
            || !matches!(v.kind, StateKind::ActorObserved(_))
            || !shared_state(m, v.kind)
        {
            continue;
        }
        let writes: Vec<_> = m
            .capabilities
            .iter()
            .map(|c| {
                c.start_writes
                    .iter()
                    .chain(c.finish_writes())
                    .filter(|w| w.variable == id)
                    .collect::<Vec<_>>()
            })
            .collect();
        for left in 0..m.capabilities.len() {
            if writes[left].is_empty() {
                continue;
            }
            for right in left..m.capabilities.len() {
                if writes[right].is_empty() {
                    continue;
                }
                let a = &m.capabilities[left];
                let b = &m.capabilities[right];
                let reason = if m.actors[a.actor.0].capacity == Some(1) {
                    Some(ConcurrencyResolution::Serialized)
                } else if let Some(resource) = m.resources.iter().enumerate().find_map(|(r, v)| {
                    let x = a.uses.iter().find(|u| u.resource.0 == r)?;
                    let y = b.uses.iter().find(|u| u.resource.0 == r)?;
                    (u128::from(x.amount) + u128::from(y.amount) > u128::from(v.capacity))
                        .then_some(ResourceId(r))
                }) {
                    Some(ConcurrencyResolution::Lock(resource))
                } else {
                    let kinds: Vec<_> = writes[left]
                        .iter()
                        .chain(&writes[right])
                        .map(|w| match w.source {
                            WriteSource::InsertCase { .. } => Some(CommutativeKind::InsertCase),
                            WriteSource::Integer {
                                operator: AssignmentOp::Add,
                                ..
                            } => Some(CommutativeKind::Add),
                            WriteSource::Integer {
                                operator: AssignmentOp::Subtract,
                                ..
                            } => Some(CommutativeKind::Subtract),
                            _ => None,
                        })
                        .collect();
                    if kinds[0].is_some() && kinds.iter().all(|k| *k == kinds[0]) {
                        Some(ConcurrencyResolution::Commutative(kinds[0].unwrap()))
                    } else if a
                        .version_check
                        .as_ref()
                        .is_some_and(|v| v.variables.contains(&id))
                        && b.version_check
                            .as_ref()
                            .is_some_and(|v| v.variables.contains(&id))
                    {
                        Some(ConcurrencyResolution::Versioned)
                    } else if v.last_write_wins {
                        Some(ConcurrencyResolution::LastWriteWins)
                    } else {
                        None
                    }
                };
                table.push(ResolvedPair {
                    variable: id,
                    left: CapabilityId(left),
                    right: CapabilityId(right),
                    resolution: reason.ok_or(StopReason::SemanticAbort)?,
                });
            }
        }
    }
    require(table == m.concurrency_resolutions)?;
    Ok(())
}

pub(super) fn validate_scenario(m: &Model, times: &[Tick]) -> Execution {
    require(m.arrival_distribution.is_none())?;
    if let Some(t) = &m.resource_time {
        require(t.validate(m).is_ok())?;
    }
    require(m.scenario.as_ref().is_none_or(|s| !s.name.is_empty()))?;
    if let Some(stop) = m.horizon {
        require(stop > 0 && m.scenario.is_some() && times.iter().all(|at| *at <= stop))?;
    }
    if let Some(record) = &m.arrival_every {
        require(record.every > 0 && record.from < record.until && record.count <= 10000)?;
        let expected = (record.until - record.from - 1) / record.every + 1;
        require(
            m.arrivals.is_some()
                && expected == record.count
                && expected == times.len() as u64
                && expected == m.bounds.cases,
        )?;
        for (index, time) in times.iter().enumerate() {
            require(
                record
                    .every
                    .checked_mul(index as u64)
                    .and_then(|offset| record.from.checked_add(offset))
                    == Some(*time),
            )?;
        }
    }
    Ok(())
}

fn domain_support(
    m: &Model,
    v: &StateVariable,
    remaining: usize,
) -> Result<Vec<(StateValue, Rational)>, StopReason> {
    if v.integer_max.is_some() && reference_cardinality(m, v) > remaining as u128 {
        return Err(StopReason::EnumerationLimit);
    }
    Ok(reference_domain(m, v)
        .map(|x| (x, Rational::one()))
        .collect())
}
impl<'a> Tree<'a> {
    fn calendar_pending(&self, w: &World<'_>) -> Result<bool, StopReason> {
        if self.m.resource_time.is_none() {
            return Ok(false);
        }
        let frames: Vec<_> = w
            .cases
            .iter()
            .map(|s| match s {
                CaseSlot::Here(c) => Some(&c.frame),
                _ => None,
            })
            .collect();
        let queue: Vec<_> = w
            .cases
            .iter()
            .map(|s| match s {
                CaseSlot::Here(c) => c
                    .queue
                    .iter()
                    .map(|q| (c.frame.tasks[q.task].call.binding, q.at))
                    .collect(),
                _ => BTreeMap::new(),
            })
            .collect();
        let active: Vec<_> = w
            .cases
            .iter()
            .map(|s| matches!(s, CaseSlot::Here(c) if !c.stopped(self.m)))
            .collect();
        policy::calendar_pending(self.m, &frames, &queue, &active)
    }
    fn notify(&self, w: &mut World<'a>, own_case: usize, own_task: usize, writes: &[StateWrite]) {
        for (k, slot) in w.cases.iter_mut().enumerate() {
            if let CaseSlot::Here(c) = slot {
                for (j, task) in c.frame.tasks.iter_mut().enumerate() {
                    if (k == own_case && j == own_task) || task.phase != Phase::Running {
                        continue;
                    }
                    for write in writes {
                        if (k == own_case
                            || shared_state(self.m, self.m.state_variables[write.variable.0].kind))
                            && task.checked_write(write.variable, self.m)
                        {
                            task.stale.insert(write.variable);
                        }
                    }
                }
            }
        }
    }
    fn state_snapshots(&mut self, w: &mut World<'a>) -> Execution {
        if !self
            .m
            .contracts
            .iter()
            .any(|c| matches!(c.kind, ContractKind::StateInvariant { .. }))
        {
            return Ok(());
        }
        let mut error = None;
        for k in 0..w.cases.len() {
            // ADR 0039 m8: received Held and Overflow cases are monitored too.
            if !w.cases[k].payload().is_some_and(|c| {
                c.frame
                    .trace
                    .iter()
                    .any(|o| matches!(o.event, Occurrence::Receipt))
            }) {
                continue;
            }
            let values = self.values(w, k);
            let Some(c) = w.cases[k].payload_mut() else {
                unreachable!()
            };
            if c.frame
                .state_snapshots
                .last()
                .is_none_or(|(_, old)| *old != values)
            {
                c.frame.state_snapshots.push((w.now, values));
            }
            if let Err(e) =
                self.cases[k].inspect_trace(&c.frame, w.now, false, c.stopped(self.m), false)
            {
                error.get_or_insert(e);
            }
        }
        error.map_or(Ok(()), Err)
    }
    fn new_case(&self, at: Tick) -> CaseFrame<'a> {
        CaseFrame {
            frame: Frame {
                condition_waits: Vec::new(),
                lost_updates: BTreeSet::new(),
                state_snapshots: Vec::new(),
                business: None,
                state_values: self
                    .m
                    .state_variables
                    .iter()
                    .map(|v| match v.initial {
                        _ if shared_state(self.m, v.kind) => StateValue::None,
                        InitialValue::Fixed(x) => x,
                        _ => StateValue::None,
                    })
                    .collect(),
                trace: Vec::new(),
                now: at,
                steps: 0,
                control: vec![&self.m.process.body],
                waiting: None,
                tasks: Vec::new(),
                adoption: None,
            },
            queue: Vec::new(),
            receipt: false,
            end: None,
        }
    }
    fn reserve(&mut self, n: usize) -> Execution {
        if n > self.limits.frames.saturating_sub(self.stats.frames) {
            return Err(StopReason::EnumerationLimit);
        }
        self.stats.frames += n;
        Ok(())
    }
    fn copy(&mut self, w: &World<'a>) -> Result<World<'a>, StopReason> {
        self.reserve(1)?;
        Ok(w.clone())
    }
    fn values(&self, w: &World<'a>, k: usize) -> Vec<StateValue> {
        let Some(c) = w.cases[k].payload() else {
            unreachable!()
        };
        self.m
            .state_variables
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if shared_state(self.m, v.kind) {
                    w.shared[i]
                } else {
                    c.frame.state_values[i]
                }
            })
            .collect()
    }
    fn record(
        &mut self,
        w: &mut World<'a>,
        k: usize,
        event: Occurrence,
        person: Option<PersonId>,
        closed: bool,
    ) -> Execution {
        self.record_context(w, k, event, person, (closed, None))
    }
    fn record_context(
        &mut self,
        w: &mut World<'a>,
        k: usize,
        event: Occurrence,
        person: Option<PersonId>,
        context: (bool, Option<BindingId>),
    ) -> Execution {
        let (closed, binding) = context;
        match event {
            Occurrence::Dispatch(cap) => {
                self.cases[k].statistics.observed.capabilities.insert(cap);
                if let Some(p) = person {
                    self.cases[k].statistics.observed.people.insert(p);
                }
            }
            Occurrence::Terminal(e) => {
                self.cases[k].statistics.observed.ends.insert(e);
            }
            _ => {}
        }
        let values = self.values(w, k);
        self.response_observe(w, k, &event)?;
        #[cfg(test)]
        if matches!(event, Occurrence::Receipt) {
            self.receipts.push((k, w.now, values.clone()));
        }
        let Some(c) = w.cases[k].payload_mut() else {
            return Err(StopReason::SemanticAbort);
        };
        c.frame.state_values = values;
        // Unlike legacy trace elision, arrivals traces also retain Dispatch for
        // historical admission accounting even without requested properties.
        let result_values = response::visible_values(&c.frame, self.m, Some(&event));
        c.frame.trace.push(Observation {
            binding,
            state_values: c.frame.state_values.clone(),
            person,
            at: w.now,
            event,
            values: result_values,
        });
        let result = self.cases[k].inspect_trace(&c.frame, w.now, false, closed, false);
        if result.is_err() {
            let _ = self.cases[k].collect_obligations(&c.frame, "aborted");
        }
        for (i, v) in self.m.state_variables.iter().enumerate() {
            if shared_state(self.m, v.kind) {
                w.shared[i] = c.frame.state_values[i];
                c.frame.state_values[i] = StateValue::None;
            }
        }
        let state_result = self.state_snapshots(w);
        result.and(state_result)
    }
    fn condition(&self, w: &World<'a>, k: usize, c: &Cond) -> Result<bool, StopReason> {
        let CaseSlot::Here(case) = &w.cases[k] else {
            return Err(StopReason::SemanticAbort);
        };
        let mut frame = case.frame.clone();
        frame.state_values = self.values(w, k);
        self.cases[k].condition(&frame, c, 0)
    }
    fn ending(&mut self, w: &mut World<'a>, k: usize, end: EndId) -> Execution {
        self.record(w, k, Occurrence::Terminal(end), None, true)?;
        let overflow = matches!(w.cases[k], CaseSlot::Overflow(_));
        let Some(c) = w.cases[k].payload_mut() else {
            unreachable!()
        };
        c.end = Some(end);
        if !self.m.contests().is_empty() {
            c.frame.business = Some((end, w.now));
            c.frame.control.clear();
            c.frame.waiting = None;
        }
        let mut work = 0u64;
        for t in &c.frame.tasks {
            if crate::contest::has_effects(self.m) && self.m.site_contest(t.call.binding).is_some()
            {
                continue;
            }
            if let Some(start) = t.start {
                work = work
                    .checked_add(
                        t.stop
                            .ok_or(StopReason::SemanticAbort)?
                            .checked_sub(start)
                            .ok_or(StopReason::SemanticAbort)?,
                    )
                    .ok_or(StopReason::SemanticAbort)?;
            }
        }
        self.terminal_value(
            k,
            CaseTerminal {
                observed_until: None,
                observed_elapsed: None,
                observed_work: None,
                kind: if overflow {
                    Kind::Overflow(end)
                } else {
                    Kind::End(end)
                },
                completed_at: Some(singleton(w.now)),
                elapsed: Some(singleton(w.now - self.times[k])),
                work: Some(singleton(work)),
                result_at: c.frame.adoption.map(|t| singleton(t - self.times[k])),
                probability: None,
            },
        )?;
        self.stats.terminal_paths += 1;
        Ok(())
    }
    fn terminal_value(&mut self, k: usize, t: CaseTerminal) -> Execution {
        if let Some(old) = self.terminals[k].get_mut(&t.kind) {
            require(old.result_at.is_some() == t.result_at.is_some())?;
            for (a, b) in [
                (&mut old.completed_at, t.completed_at),
                (&mut old.elapsed, t.elapsed),
                (&mut old.work, t.work),
                (&mut old.result_at, t.result_at),
            ] {
                if let (Some(a), Some(b)) = (a, b) {
                    extend(a, b.min);
                    extend(a, b.max);
                }
            }
        } else {
            self.terminals[k].insert(t.kind, t);
        }
        Ok(())
    }
    fn observe_first(&mut self, w: &World<'a>) -> Execution {
        if self.first_hits.is_empty() {
            return Ok(());
        }
        let spec = self.m.quality.as_ref().unwrap();
        for (i, p) in spec.properties.iter().enumerate() {
            let mut views = vec![(false, false, None); self.times.len()];
            for (k, view) in views.iter_mut().enumerate() {
                if self.first_closed[i][k] {
                    continue;
                }
                // The closing view is still observable. Only subsequent views
                // are excluded; business end alone does not close open routes.
                self.first_closed[i][k] = matches!(w.cases[k], CaseSlot::Declined)
                    || w.cases[k].payload().is_some_and(|c| {
                        c.end.is_some()
                            && (matches!(p.target, QualityTarget::Ends(_)) || c.stopped(self.m))
                    })
                    || p.within
                        .is_some_and(|d| w.now >= self.times[k] && w.now - self.times[k] > d);
                let Some(c) = w.cases[k].payload().filter(|c| c.receipt) else {
                    continue;
                };
                let terminal = c.frame.business.or_else(|| {
                    c.frame.trace.iter().find_map(|o| match o.event {
                        Occurrence::Terminal(e) => Some((e, o.at)),
                        _ => None,
                    })
                });
                let at = match p.target {
                    QualityTarget::Ends(_) => terminal.map_or(w.now, |(_, at)| at),
                    _ => w.now,
                };
                if at < self.times[k] || p.within.is_some_and(|d| at - self.times[k] > d) {
                    continue;
                }
                let hit = match &p.target {
                    QualityTarget::Ends(es) => terminal.is_some_and(|(e, _)| es.contains(&e)),
                    QualityTarget::State(cond) => {
                        snapshot_condition_at(cond, &[], &self.values(w, k), k)?
                    }
                };
                *view = (true, hit, Some(at));
            }
            super::quality::observe_first_world(
                &mut self.first_hits[i],
                &self.times,
                w.now,
                true,
                &views,
            );
        }
        Ok(())
    }
    fn first_observations(
        &self,
        w: &World<'a>,
        cut: ReferenceCut,
    ) -> Vec<Vec<crate::quality::Arrival>> {
        use crate::quality::Arrival;
        if self.first_hits.is_empty() {
            return Vec::new();
        }
        self.m
            .quality
            .as_ref()
            .unwrap()
            .properties
            .iter()
            .enumerate()
            .map(|(i, p)| {
                w.cases
                    .iter()
                    .enumerate()
                    .map(|(k, slot)| {
                        if let Some(t) = self.first_hits[i][k] {
                            return Arrival::Hit(t);
                        }
                        let closed = matches!(slot, CaseSlot::Declined)
                            || slot.payload().is_some_and(|c| {
                                c.end.is_some()
                                    && (matches!(p.target, QualityTarget::Ends(_))
                                        || c.stopped(self.m))
                            });
                        if self.first_closed[i][k] || closed || cut == ReferenceCut::None {
                            return Arrival::Never;
                        }
                        let at = match cut {
                            ReferenceCut::Clock => self.m.bounds.clock as u128 + 1,
                            ReferenceCut::Horizon => w.now as u128 + 1,
                            _ => w.now as u128,
                        };
                        let earliest = at.saturating_sub(self.times[k] as u128);
                        if p.within.is_some_and(|d| earliest > d as u128) {
                            Arrival::Never
                        } else {
                            Arrival::Censored {
                                earliest,
                                latest: p.within.map(u128::from),
                                computational: cut != ReferenceCut::Horizon,
                            }
                        }
                    })
                    .collect()
            })
            .collect()
    }
    fn observe(&mut self, w: &World<'a>) -> Execution {
        self.observe_first(w)?;
        #[cfg(test)]
        if let Some(worlds) = &mut self.resource_worlds {
            worlds.push(w.clone());
        }
        let quantities = reference_resource_units(
            w.cases
                .iter()
                .filter_map(|slot| match slot {
                    CaseSlot::Here(c) => Some(c.frame.tasks.iter()),
                    _ => None,
                })
                .flatten(),
            self.m,
        )?;
        for (r, quantity) in self.m.resources.iter().zip(quantities) {
            self.overcommit |= quantity > r.capacity;
        }
        let mut people = BTreeSet::new();
        let mut counts = vec![0u64; self.m.actors.len()];
        for (k, slot) in w.cases.iter().enumerate() {
            if let Some(c) = slot.payload() {
                self.cases[k]
                    .evidence
                    .lost_updates
                    .extend(c.frame.lost_updates.iter().copied());
                self.cases[k].inspect_trace(&c.frame, w.now, false, c.stopped(self.m), false)?;
                for t in &c.frame.tasks {
                    if t.phase == Phase::Running {
                        counts[self.m.capabilities[t.call.capability.0].actor.0] += 1;
                        if let Some(p) = t.person {
                            self.overcommit |= !people.insert(p);
                        }
                    }
                }
            }
        }
        for (a, n) in self.m.actors.iter().zip(counts) {
            if n > 0 {
                self.overcommit |= n > a.capacity.ok_or(StopReason::SemanticAbort)?;
            }
        }
        Ok(())
    }
    fn terminal_kind(w: &World<'a>, k: usize) -> Option<Kind> {
        match &w.cases[k] {
            CaseSlot::Declined => Some(Kind::Rejected),
            CaseSlot::Here(c) => c.end.map(Kind::End).or_else(|| {
                c.frame
                    .trace
                    .iter()
                    .any(|o| matches!(o.event, Occurrence::ResponseStopped(_)))
                    .then_some(Kind::ReceiptUnknown)
            }),
            CaseSlot::Overflow(c) => c.end.map(Kind::Overflow),
            _ => None,
        }
    }
    fn leaf(&mut self, w: &World<'a>, cut: ReferenceCut) -> Result<Vec<Bound>, StopReason> {
        if cut == ReferenceCut::None && !self.m.response_attempts().is_empty() {
            for id in 0..self.m.capabilities.len() {
                for slot in &w.cases {
                    let count = slot.payload().map_or(0, |c| {
                        c.frame
                            .tasks
                            .iter()
                            .filter(|t| t.call.capability.0 == id && t.start.is_some())
                            .count()
                    });
                    self.response_exercise
                        .entry(CapabilityId(id))
                        .and_modify(|(lo, hi)| {
                            *lo = (*lo).min(count);
                            *hi = (*hi).max(count);
                        })
                        .or_insert((count, count));
                }
            }
        }
        if self
            .m
            .quality
            .as_ref()
            .is_some_and(|q| !q.quantiles.is_empty())
        {
            self.observe_first(w)?;
            let views = self
                .first_observations(w, cut)
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|o| match o {
                            crate::quality::Arrival::Hit(t) => crate::quality::FirstView {
                                hit: Some(t),
                                ..Default::default()
                            },
                            crate::quality::Arrival::Never => crate::quality::FirstView {
                                closed: true,
                                ..Default::default()
                            },
                            c => crate::quality::FirstView {
                                censor: Some(c),
                                ..Default::default()
                            },
                        })
                        .collect()
                })
                .collect();
            self.first_trees.push(super::quality::FirstTree {
                views,
                ..Default::default()
            });
        }
        if self.m.scenario.is_some() {
            if let Some(r) = &mut self.stats.scenario_observed_until {
                extend(r, w.now);
            } else {
                self.stats.scenario_observed_until = Some(singleton(w.now));
            }
        }
        self.stats.horizon |= cut == ReferenceCut::Horizon;
        #[cfg(test)]
        if let Some(worlds) = &mut self.resource_worlds {
            worlds.push(w.clone());
        }
        if cut == ReferenceCut::None && !self.m.contests().is_empty() {
            let open = if crate::contest::has_effects(self.m) {
                w.cases
                    .iter()
                    .filter(|slot| matches!(slot, CaseSlot::Here(c) if c.frame.ended_open(self.m)))
                    .count() as u64
            } else {
                w.cases.iter().filter(|slot| matches!(slot,CaseSlot::Here(c) if c.end.is_some()
                && self.m.contests().iter().enumerate().filter(|(_,d)|d.target.end()==c.end).any(|(id,_)| {
                    c.frame.trace.iter().find_map(|o|match o.event { Occurrence::ContestChoice(n,filed) if n==id=>Some(filed),_=>None })
                        .is_none_or(|filed| filed && !c.frame.tasks.iter().any(|t|self.m.site_contest(t.call.binding)==Some(id) && matches!(t.phase,Phase::Finished(_))))
                }))).count() as u64
            };
            if let Some(range) = &mut self.stats.contest_open {
                extend(range, open);
            } else {
                self.stats.contest_open = Some(singleton(open));
            }
        }
        for (k, slot) in w.cases.iter().enumerate() {
            if let Some(c) = slot.payload() {
                self.cases[k].collect_obligations(
                    &c.frame,
                    if c.stopped(self.m) {
                        "terminal_unresolved"
                    } else {
                        match cut {
                            ReferenceCut::Range => "state_range_violation",
                            ReferenceCut::Clock => "clock",
                            ReferenceCut::Horizon => "horizon",
                            ReferenceCut::LiveTasks => "live_tasks",
                            ReferenceCut::Numeric => "state_budget",
                            ReferenceCut::None => "deadlock_unresolved",
                        }
                    },
                )?;
                self.cases[k].collect_contests(&c.frame, cut, c.stopped(self.m))?;
            }
        }
        let is_cut = cut != ReferenceCut::None;
        if self.m.metrics.is_some() && self.stats.projection == Projection::Full {
            let point =
                metric_observation(self.m, w, &self.times, is_cut, cut == ReferenceCut::Horizon);
            self.stats
                .metric_envelope
                .get_or_insert_with(metrics::Envelope::default)
                .add(point);
        }
        self.truncated |= is_cut && !matches!(cut, ReferenceCut::Range | ReferenceCut::Horizon);
        self.numeric |= cut == ReferenceCut::Numeric;
        let r = w
            .cases
            .iter()
            .filter(|s| matches!(s, CaseSlot::Declined))
            .count() as u64;
        let upper = r + if is_cut {
            (self.times.len() - w.next) as u64
        } else {
            0
        };
        if let Some(range) = &mut self.rejects {
            range.min = range.min.min(r);
            range.max = range.max.max(upper)
        } else {
            self.rejects = Some(Range { min: r, max: upper });
        }
        let mut out = Vec::new();
        let missing = if is_cut {
            (self.times.len() - w.next) as u64
        } else {
            0
        };
        let overflows = w
            .cases
            .iter()
            .filter(|s| matches!(s, CaseSlot::Overflow(_)))
            .count() as u64;
        let held = w
            .cases
            .iter()
            .filter(|s| matches!(s, CaseSlot::Held(_)))
            .count() as u64;
        for (dst, low, high) in [
            (&mut self.overflows, overflows, overflows + missing),
            (
                &mut self.external_peaks,
                w.external_peak,
                w.external_peak.max(held + missing),
            ),
        ] {
            if let Some(r) = dst {
                r.min = r.min.min(low);
                r.max = r.max.max(high);
            } else {
                *dst = Some(Range {
                    min: low,
                    max: high,
                });
            }
        }
        for (k, slot) in w.cases.iter().enumerate() {
            let at = slot.payload().and_then(|c| {
                c.frame
                    .trace
                    .iter()
                    .find_map(|o| matches!(o.event, Occurrence::Admission).then_some(o.at))
            });
            if let Some(at) = at {
                if let Some(r) = &mut self.admissions[k] {
                    extend(r, at);
                } else {
                    self.admissions[k] = Some(singleton(at));
                }
            } else {
                self.unadmitted[k] = true;
            }
        }
        for (k, slot) in w.cases.iter().enumerate() {
            let kind = Self::terminal_kind(w, k);
            self.cases[k].statistics.completion_bad |=
                kind == Some(Kind::Rejected) || (kind.is_none() && !is_cut);
            self.cases[k].statistics.completion_unknown |= kind.is_none() && is_cut;
            self.unresolved[k] |= is_cut
                && !matches!(slot, CaseSlot::Declined)
                && !slot.payload().is_some_and(|c| c.stopped(self.m));
            if let Some(c) = slot.payload() {
                self.cases[k].inspect_trace(
                    &c.frame,
                    if cut == ReferenceCut::Clock {
                        self.m.bounds.clock
                    } else {
                        w.now
                    },
                    matches!(cut, ReferenceCut::Clock | ReferenceCut::Horizon),
                    !is_cut || c.stopped(self.m),
                    // A complete leaf with this case not stopped is a deadlock.
                    !is_cut && !c.stopped(self.m),
                )?;
            }
            for q in &self.m.probability_queries {
                // An evaluation error aborts like the legacy reference; it is
                // never read as a miss.
                let mut hit = false;
                if let Some(c) = slot.payload()
                    && c.receipt
                {
                    for o in &c.frame.trace {
                        hit |= observation_hit(q, o, self.times[k])?;
                    }
                }
                let expired = q.within.is_some_and(|d| {
                    (w.now >= self.times[k] && w.now - self.times[k] > d)
                        || (matches!(cut, ReferenceCut::Clock | ReferenceCut::Horizon)
                            && self.times[k]
                                <= if cut == ReferenceCut::Horizon {
                                    w.now
                                } else {
                                    self.m.bounds.clock
                                }
                            && d <= (if cut == ReferenceCut::Horizon {
                                w.now
                            } else {
                                self.m.bounds.clock
                            }) - self.times[k])
                });
                out.push(if hit {
                    Bound::exact(true)
                } else if kind.is_some() {
                    Bound::exact(false)
                } else if cut == ReferenceCut::Numeric {
                    Bound::unknown(true)
                } else if !is_cut || expired {
                    Bound::exact(false)
                } else {
                    Bound::unknown(false)
                });
            }
            for end in 0..self.m.ends.len() {
                out.push(
                    if kind == Some(Kind::End(EndId(end)))
                        || kind == Some(Kind::Overflow(EndId(end)))
                    {
                        Bound::exact(true)
                    } else if kind.is_some() || !is_cut || cut == ReferenceCut::Horizon {
                        Bound::exact(false)
                    } else {
                        Bound::unknown(cut == ReferenceCut::Numeric)
                    },
                );
            }
            out.push(if kind == Some(Kind::Rejected) {
                Bound::exact(true)
            } else if !matches!(slot, CaseSlot::NotHere) || !is_cut {
                Bound::exact(false)
            } else {
                Bound::unknown(cut == ReferenceCut::Numeric)
            });
            if self.m.completion_requested() {
                out.push(if matches!(kind, Some(Kind::End(_) | Kind::Overflow(_))) {
                    Bound::exact(true)
                } else if kind.is_some() || !is_cut {
                    Bound::exact(false)
                } else {
                    Bound::unknown(cut == ReferenceCut::Numeric)
                });
            }
            if self.m.horizon.is_some() {
                out.push(if cut == ReferenceCut::Horizon {
                    Bound::exact(kind.is_none())
                } else if kind.is_some() || !is_cut {
                    Bound::exact(false)
                } else {
                    Bound::unknown(cut == ReferenceCut::Numeric)
                });
                if cut == ReferenceCut::Horizon && kind.is_none() {
                    let work = if let CaseSlot::Here(c) = slot {
                        c.frame
                            .tasks
                            .iter()
                            .filter_map(|t| {
                                t.start
                                    .map(|start| t.stop.unwrap_or(w.now).min(w.now) - start)
                            })
                            .sum()
                    } else {
                        0
                    };
                    self.terminals[k]
                        .entry(Kind::CutOff)
                        .and_modify(|t| extend(t.work.as_mut().expect("cutoff work"), work))
                        .or_insert(CaseTerminal {
                            observed_until: None,
                            observed_elapsed: None,
                            observed_work: None,
                            kind: Kind::CutOff,
                            completed_at: None,
                            elapsed: None,
                            work: Some(singleton(work)),
                            result_at: None,
                            probability: None,
                        });
                }
            }
            if !self.m.response_attempts().is_empty() {
                out.push(if kind == Some(Kind::ReceiptUnknown) {
                    Bound::exact(true)
                } else if kind.is_some() || !is_cut {
                    Bound::exact(false)
                } else {
                    Bound::unknown(cut == ReferenceCut::Numeric)
                });
            }
        }
        if let Some(q) = &self.m.quality {
            for p in &q.properties {
                for (k, slot) in w.cases.iter().enumerate() {
                    out.push(quality_truth(
                        self.m,
                        p,
                        slot,
                        k,
                        self.times[k],
                        w.now,
                        cut,
                    )?);
                }
            }
            for (target, end) in crate::quality::outcome_keys(self.m) {
                for (k, slot) in w.cases.iter().enumerate() {
                    let kind = Self::terminal_kind(w, k);
                    let b = if let Some(kind) = kind {
                        Bound::exact(match kind {
                            Kind::End(e) => {
                                target == "end" && end.as_deref() == Some(&self.m.ends[e.0])
                            }
                            Kind::Overflow(e) => {
                                target == "overflow" && end.as_deref() == Some(&self.m.ends[e.0])
                            }
                            Kind::Rejected => target == "rejected",
                            Kind::CutOff => target == "cut_off",
                            Kind::ReceiptUnknown => {
                                target == "receipt_unknown" || target == "unfinished"
                            }
                        })
                    } else if target == "rejected" && !matches!(slot, CaseSlot::NotHere) {
                        Bound::exact(false)
                    } else if cut == ReferenceCut::Horizon {
                        Bound::exact(target == "cut_off")
                    } else if is_cut {
                        Bound::unknown(false)
                    } else {
                        Bound::exact(target == "deadlock")
                    };
                    out.push(b);
                }
            }
        }
        Ok(out)
    }
    fn reduce(&mut self, children: Vec<(Rational, Vec<Bound>)>, random: bool) -> Vec<Bound> {
        if children.is_empty() {
            return Vec::new();
        }
        if self
            .m
            .quality
            .as_ref()
            .is_some_and(|q| !q.quantiles.is_empty())
        {
            let subtrees = self
                .first_trees
                .split_off(self.first_trees.len() - children.len());
            self.first_trees.push(super::quality::FirstTree {
                random,
                children: children
                    .iter()
                    .map(|(p, _)| p.clone())
                    .zip(subtrees)
                    .collect(),
                ..Default::default()
            });
        }
        if random {
            self.chance = true
        } else {
            self.scheduler |= children.len() > 1;
        }
        let mut out = Vec::new();
        for i in 0..children[0].1.len() {
            let mut b = Bound::exact(false);
            for (_, xs) in &children {
                b.numeric |= xs[i].numeric;
                b.computational |= xs[i].computational;
            }
            if random {
                for (p, xs) in &children {
                    for j in 0..4 {
                        let term = Rational(p.0.clone() * &xs[i].endpoints[j].0);
                        b.numeric |= !term.within_bits(4096);
                        b.endpoints[j].0 += term.0;
                        b.numeric |= !b.endpoints[j].within_bits(4096);
                    }
                }
            } else {
                for j in 0..4 {
                    b.endpoints[j] = if j < 2 {
                        children
                            .iter()
                            .map(|(_, xs)| xs[i].endpoints[j].clone())
                            .min()
                            .unwrap()
                    } else {
                        children
                            .iter()
                            .map(|(_, xs)| xs[i].endpoints[j].clone())
                            .max()
                            .unwrap()
                    };
                }
            }
            if b.numeric {
                b = Bound::unknown(true);
                let base = self.times.len()
                    * (self.m.probability_queries.len()
                        + self.m.ends.len()
                        + 1
                        + usize::from(self.m.completion_requested()));
                if i < base {
                    self.numeric = true;
                    self.truncated = true;
                }
            }
            out.push(b);
        }
        out
    }
    fn queued_time(c: &CaseFrame<'a>, j: usize) -> Option<Tick> {
        c.queue.iter().find(|e| e.task == j).map(|e| e.at)
    }
    // Keep the new admission branch's owned World and iterator temporaries out
    // of walk_inner's recursive frame. Even an untaken branch reserves stack
    // space in an unoptimized build; legacy paths must not pay that per level.
    fn priority_readmission_children(
        &mut self,
        w: &World<'a>,
        admissions: &mut Vec<(Rational, Vec<Bound>)>,
    ) -> Execution {
        let candidates = self.priority_admissions(w);
        self.policy_unresolved |= candidates.len() > 1;
        for k in candidates {
            let mut next = w.clone();
            let CaseSlot::Held(c) = std::mem::replace(&mut next.cases[k], CaseSlot::NotHere) else {
                unreachable!()
            };
            next.cases[k] = CaseSlot::Here(c);
            self.record(&mut next, k, Occurrence::Admission, None, false)?;
            admissions.push((Rational::one(), self.walk(next)?));
        }
        Ok(())
    }

    // The legacy batch also moves a large inline CaseSlot. Keeping that
    // temporary out of the recursive frame preserves the pre-Q5 stack budget.
    fn legacy_readmit(&mut self, w: &mut World<'a>) -> Result<bool, StopReason> {
        if self.m.external_priority().is_some() {
            return Ok(false);
        }
        let mut resumed = false;
        for k in 0..w.cases.len() {
            if !matches!(w.cases[k], CaseSlot::Held(_)) {
                continue;
            }
            let waiting = w.cases.iter().filter(|s| matches!(s, CaseSlot::Here(c) if c.end.is_none() && !c.frame.tasks.iter().any(|t| t.start.is_some()))).count() as u64;
            let concurrent = w
                .cases
                .iter()
                .filter(|s| matches!(s, CaseSlot::Here(c) if c.end.is_none()))
                .count() as u64;
            if self
                .m
                .admission_queue
                .is_some_and(|q| waiting >= q.capacity)
                || self
                    .m
                    .concurrent_cases
                    .is_some_and(|q| concurrent >= q.capacity)
            {
                break;
            }
            let CaseSlot::Held(c) = std::mem::replace(&mut w.cases[k], CaseSlot::NotHere) else {
                unreachable!()
            };
            w.cases[k] = CaseSlot::Here(c);
            self.record(w, k, Occurrence::Admission, None, false)?;
            resumed = true;
        }
        Ok(resumed)
    }

    fn priority_finish(&mut self, w: &World<'_>, case: usize, task: usize) -> bool {
        self.priority_finish_order(w, case, task).0
    }

    fn priority_finish_order(&mut self, w: &World<'_>, case: usize, task: usize) -> (bool, bool) {
        let frames: Vec<_> = w
            .cases
            .iter()
            .map(|s| match s {
                CaseSlot::Here(c) if !c.stopped(self.m) => Some(&c.frame),
                _ => None,
            })
            .collect();
        let (allowed, unresolved) = policy::finish_order(self.m, &frames, case, task, w.now);
        self.policy_unresolved |= unresolved;
        (allowed, unresolved)
    }

    fn priority_admissions(&self, w: &World<'_>) -> Vec<usize> {
        if self.m.external_priority().is_none() {
            return Vec::new();
        }
        let waiting = w.cases.iter().filter(|s| matches!(s, CaseSlot::Here(c) if c.end.is_none() && !c.frame.tasks.iter().any(|t| t.start.is_some()))).count() as u64;
        let active = w
            .cases
            .iter()
            .filter(|s| matches!(s, CaseSlot::Here(c) if c.end.is_none()))
            .count() as u64;
        if self
            .m
            .admission_queue
            .is_some_and(|q| waiting >= q.capacity)
            || self
                .m
                .concurrent_cases
                .is_some_and(|q| active >= q.capacity)
        {
            return Vec::new();
        }
        let held: Vec<_> = w
            .cases
            .iter()
            .map(|s| matches!(s, CaseSlot::Held(_)))
            .collect();
        policy::held(self.m, &held, &self.times)
    }

    fn eligible(
        &self,
        w: &World<'a>,
        k: usize,
        j: usize,
    ) -> Result<Vec<Option<PersonId>>, StopReason> {
        if self
            .m
            .start_policy
            .as_ref()
            .is_some_and(StartPolicy::extended)
        {
            let frames: Vec<_> = w
                .cases
                .iter()
                .map(|slot| match slot {
                    CaseSlot::Here(c) => Some(&c.frame),
                    _ => None,
                })
                .collect();
            let queued: Vec<_> = w
                .cases
                .iter()
                .map(|slot| match slot {
                    CaseSlot::Here(c) => c
                        .queue
                        .iter()
                        .map(|entry| (c.frame.tasks[entry.task].call.binding, entry.at))
                        .collect(),
                    _ => BTreeMap::new(),
                })
                .collect();
            return Ok(policy::starts(self.m, &frames, &queued)?
                .remove(&(k, j))
                .unwrap_or_default());
        }
        let CaseSlot::Here(c) = &w.cases[k] else {
            return Ok(Vec::new());
        };
        let task = &c.frame.tasks[j];
        if task.phase != Phase::Queued {
            return Ok(Vec::new());
        }
        let actor = self.m.capabilities[task.call.capability.0].actor;
        let quantities = reference_resource_units(
            w.cases
                .iter()
                .filter_map(|slot| match slot {
                    CaseSlot::Here(c) => Some(c.frame.tasks.iter()),
                    _ => None,
                })
                .flatten(),
            self.m,
        )?;
        for claim in &self.m.capabilities[task.call.capability.0].uses {
            let limit = self.m.resources[claim.resource.0].capacity;
            let used = quantities[claim.resource.0];
            if used > limit || claim.amount > limit - used {
                return Ok(Vec::new());
            }
        }
        let own = Self::queued_time(c, j).ok_or(StopReason::SemanticAbort)?;
        let mut used = 0u64;
        let mut busy = BTreeSet::new();
        for (other, slot) in w.cases.iter().enumerate() {
            if let CaseSlot::Here(c) = slot {
                for (index, t) in c.frame.tasks.iter().enumerate() {
                    let a = self.m.capabilities[t.call.capability.0].actor;
                    if t.phase == Phase::Running {
                        if a == actor {
                            used += 1
                        }
                        if let Some(p) = t.person {
                            busy.insert(p);
                        }
                    }
                    if self.m.start_policy.is_some() && t.phase == Phase::Queued && a == actor {
                        let at = Self::queued_time(c, index).ok_or(StopReason::SemanticAbort)?;
                        if at < own || (at == own && other < k) {
                            return Ok(Vec::new());
                        }
                    }
                }
            }
        }
        let a = &self.m.actors[actor.0];
        if used >= a.capacity.ok_or(StopReason::SemanticAbort)? {
            return Ok(Vec::new());
        }
        if !policy::calendar_slots(
            self.m,
            &self.m.capabilities[task.call.capability.0],
            w.now,
            used,
            &quantities,
        ) {
            return Ok(Vec::new());
        }
        Ok(match &a.executors {
            Some(ps) => ps
                .iter()
                .filter(|p| {
                    self.m.capabilities[task.call.capability.0]
                        .qualified_executors
                        .as_ref()
                        .is_none_or(|eligible| eligible.contains(p))
                        && !busy.contains(p)
                        && policy::working(self.m, **p, w.now)
                })
                .map(|p| Some(*p))
                .collect(),
            None => vec![None],
        })
    }

    fn backlog(&self, world: &World<'_>, case: usize, task: usize) -> u64 {
        let CaseSlot::Here(current) = &world.cases[case] else {
            return 0;
        };
        let actor = self.m.capabilities[current.frame.tasks[task].call.capability.0].actor;
        let mut queued = 0;
        for (k, slot) in world.cases.iter().enumerate() {
            if let CaseSlot::Here(c) = slot {
                for (j, t) in c.frame.tasks.iter().enumerate() {
                    if (k, j) != (case, task)
                        && t.phase == Phase::Queued
                        && self.m.capabilities[t.call.capability.0].actor == actor
                    {
                        queued += 1;
                    }
                }
            }
        }
        queued
    }
    fn coins(&self, w: &World<'a>, bits: u64) -> Result<Option<Vec<FinishCoin>>, StopReason> {
        let mut coins = Vec::new();
        for (k, slot) in w.cases.iter().enumerate() {
            if let CaseSlot::Here(c) = slot {
                for (j, t) in c.frame.tasks.iter().enumerate() {
                    if t.phase != Phase::Running || t.finish.is_some() {
                        continue;
                    }
                    let Some(d) = &self.m.capabilities[t.call.capability.0].duration_distribution
                    else {
                        continue;
                    };
                    let age = w
                        .now
                        .checked_sub(t.start.ok_or(StopReason::SemanticAbort)?)
                        .ok_or(StopReason::SemanticAbort)?;
                    if t.checked_through.is_some_and(|checked| checked >= age) {
                        continue;
                    }
                    let Some((_, p)) = d.entries.iter().find(|(at, p)| *at == age && !p.is_zero())
                    else {
                        continue;
                    };
                    let mut alive = Rational::zero();
                    for (at, q) in &d.entries {
                        if *at >= age {
                            alive.0 += &q.0;
                            if !alive.within_bits(bits) {
                                return Ok(None);
                            }
                        }
                    }
                    let Some(q) = reference_duration_divide(p, &alive, bits) else {
                        return Ok(None);
                    };
                    coins.push((k, j, q));
                }
            }
        }
        Ok(Some(coins))
    }
    fn coin_tree(
        &mut self,
        w: &World<'a>,
        coins: &[(usize, usize, Rational)],
    ) -> Result<Vec<Bound>, StopReason> {
        let size = coins
            .iter()
            .try_fold(1usize, |n, (_, _, p)| {
                n.checked_mul(if *p == Rational::one() { 1 } else { 2 })
            })
            .ok_or(StopReason::EnumerationLimit)?;
        self.reserve(size)?;
        let mut children = Vec::new();
        for choice in 0..size {
            let mut digit = choice;
            let mut weight = Rational::one();
            let mut decisions = Vec::new();
            for (k, j, p) in coins {
                let finishes = *p == Rational::one() || digit % 2 == 0;
                if *p != Rational::one() {
                    digit /= 2;
                }
                weight.0 *= if finishes {
                    p.0.clone()
                } else {
                    Rational::one().0 - &p.0
                };
                if !weight.within_bits(4096) {
                    return self.leaf(w, ReferenceCut::Numeric);
                }
                decisions.push((*k, *j, finishes));
            }
            if !weight.within_bits(4096) {
                return self.leaf(w, ReferenceCut::Numeric);
            }
            let mut next = w.clone();
            next.boundary = Boundary::Admit;
            for (k, j, finishes) in decisions {
                if let CaseSlot::Here(c) = &mut next.cases[k] {
                    let t = &mut c.frame.tasks[j];
                    t.checked_through = Some(w.now - t.start.unwrap());
                    if finishes {
                        t.finish = Some(w.now)
                    }
                }
            }
            #[cfg(test)]
            self.duration_leaves.push((
                weight.clone(),
                next.cases
                    .iter()
                    .flat_map(|s| match s {
                        CaseSlot::Here(c) => {
                            c.frame.tasks.iter().map(|t| t.finish).collect::<Vec<_>>()
                        }
                        _ => Vec::new(),
                    })
                    .collect(),
            ));
            children.push((weight, self.walk(next)?));
        }
        Ok(self.reduce(children, true))
    }
    fn initials(&mut self, w: &World<'a>, random: bool) -> Result<Vec<Bound>, StopReason> {
        let mut variables = Vec::new();
        for (k, slot) in w.cases.iter().enumerate() {
            if slot.payload().is_some_and(|c| !c.receipt) {
                for (i, v) in self.m.state_variables.iter().enumerate() {
                    if shared_state(self.m, v.kind) && self.m.bounds.cases > 1 {
                        continue;
                    }
                    let values = match v.initial {
                        InitialValue::Distribution(d) if random => self.m.initial_distributions[d]
                            .iter()
                            .filter(|(_, p)| !p.is_zero())
                            .cloned()
                            .collect::<Vec<_>>(),
                        InitialValue::Nondet if !random => domain_support(
                            self.m,
                            v,
                            self.limits.frames.saturating_sub(self.stats.frames),
                        )?,
                        _ => continue,
                    };
                    variables.push((k, i, values));
                }
            }
        }
        let size = variables
            .iter()
            .try_fold(1usize, |n, (_, _, xs)| n.checked_mul(xs.len()))
            .ok_or(StopReason::EnumerationLimit)?;
        self.reserve(size)?;
        // Preserve an already observed prefix if a later extended-policy path
        // aborts before the recursive reduction returns. Legacy aggregation is
        // intentionally unchanged.
        if self
            .m
            .start_policy
            .as_ref()
            .is_some_and(StartPolicy::extended)
        {
            if random && !variables.is_empty() {
                self.chance = true;
            } else {
                self.scheduler |= size > 1;
            }
        }
        let mut children = Vec::new();
        for tuple in 0..size {
            let mut value = tuple;
            let mut weight = Rational::one();
            let mut assignments = Vec::new();
            for (k, i, options) in &variables {
                let (x, p) = &options[value % options.len()];
                value /= options.len();
                weight.0 *= &p.0;
                if !weight.within_bits(4096) {
                    return self.leaf(w, ReferenceCut::Numeric);
                }
                assignments.push((*k, *i, *x));
            }
            if !weight.within_bits(4096) {
                return self.leaf(w, ReferenceCut::Numeric);
            }
            let mut next = w.clone();
            next.boundary = if random {
                Boundary::Choose
            } else {
                Boundary::Receive
            };
            for (k, i, x) in assignments {
                if shared_state(self.m, self.m.state_variables[i].kind) {
                    next.shared[i] = x
                } else if let Some(c) = next.cases[k].payload_mut() {
                    c.frame.state_values[i] = x;
                }
            }
            #[cfg(test)]
            if random || self.m.has_integers() {
                self.initial_leaves.push((
                    w.now,
                    next.cases
                        .iter()
                        .filter_map(|s| {
                            s.payload()
                                .filter(|c| !c.receipt)
                                .map(|c| c.frame.state_values.clone())
                        })
                        .collect(),
                    weight.clone(),
                ));
            }
            children.push((weight, self.walk(next)?));
        }
        Ok(self.reduce(children, random && !variables.is_empty()))
    }
    fn cancel_choices(
        &mut self,
        w: &World<'a>,
        k: usize,
        winner: BindingId,
    ) -> Result<Vec<Bound>, StopReason> {
        let CaseSlot::Here(c) = &w.cases[k] else {
            unreachable!()
        };
        let wait = c.frame.waiting.as_ref().unwrap();
        let mut domains = Vec::new();
        if matches!(
            wait.join,
            Some(Join::Any {
                remaining: Remaining::CancelThenDrain,
                ..
            })
        ) {
            for i in wait.tasks.clone() {
                let t = &c.frame.tasks[i];
                if t.call.binding == winner || t.terminal() {
                    continue;
                }
                match (self.m.capabilities[t.call.capability.0].cancel, t.phase) {
                    (Cancel::Unspecified, _) => return Err(StopReason::SemanticAbort),
                    (_, Phase::Queued) => domains.push((i, None)),
                    (Cancel::Immediate, Phase::Running) => {
                        domains.push((i, Some(0)));
                    }
                    (Cancel::Cooperative { ack_within }, Phase::Running) => {
                        let cap = &self.m.capabilities[t.call.capability.0];
                        let mut last = ack_within;
                        w.now.checked_add(last).ok_or(StopReason::SemanticAbort)?;
                        if let Some(d) = &cap.duration_distribution {
                            let longest = d
                                .entries
                                .iter()
                                .filter(|(_, p)| !p.is_zero())
                                .map(|(t, _)| *t)
                                .max()
                                .ok_or(StopReason::SemanticAbort)?;
                            let end = t
                                .start
                                .unwrap()
                                .checked_add(longest)
                                .ok_or(StopReason::SemanticAbort)?;
                            last = last.min(
                                end.saturating_add(1)
                                    .min(self.m.bounds.clock.saturating_add(1))
                                    - w.now,
                            );
                        }
                        domains.push((i, Some(last)));
                    }
                    _ => {}
                }
            }
        }
        let count = domains
            .iter()
            .try_fold(1usize, |n, (_, last)| {
                n.checked_mul(last.map_or(1, |t| usize::try_from(t.max(1)).unwrap_or(usize::MAX)))
            })
            .ok_or(StopReason::EnumerationLimit)?;
        self.reserve(count)?;
        let mut children = Vec::new();
        for choice in 0..count {
            let mut digit = choice;
            let mut next = w.clone();
            let CaseSlot::Here(c) = &mut next.cases[k] else {
                unreachable!()
            };
            require(c.frame.adoption.replace(w.now).is_none())?;
            c.frame.waiting.as_mut().unwrap().winner = Some(winner);
            for (i, last) in &domains {
                if let Some(last) = last {
                    if *last == 0 {
                        c.frame.tasks[*i].ack = Some(w.now);
                        continue;
                    }
                    let delay = digit % *last as usize + 1;
                    digit /= *last as usize;
                    c.frame.tasks[*i].ack = Some(
                        w.now
                            .checked_add(delay as u64)
                            .ok_or(StopReason::SemanticAbort)?,
                    );
                } else {
                    c.frame.tasks[*i].phase = Phase::Cancelled;
                    c.frame.tasks[*i].stop = Some(w.now);
                }
            }
            children.push((Rational::one(), self.walk(next)?));
        }
        Ok(self.reduce(children, false))
    }
    fn contest_management(
        &mut self,
        w: &World<'a>,
        k: usize,
    ) -> Result<Vec<World<'a>>, StopReason> {
        let CaseSlot::Here(c) = &w.cases[k] else {
            return Ok(Vec::new());
        };
        if c.stopped(self.m) {
            return Ok(Vec::new());
        }
        if let Some(wake) = c.frame.sleep()
            && !crate::contest::has_effects(self.m)
        {
            if wake > w.now {
                return Ok(Vec::new());
            }
            let mut next = self.copy(w)?;
            self.record(&mut next, k, Occurrence::Wake, None, false)?;
            return Ok(vec![next]);
        }
        if c.end.is_some() || crate::contest::has_effects(self.m) {
            if let Some(id) = c.frame.contest_pending(self.m) {
                let mut out = Vec::new();
                for filed in [false, true] {
                    if !filed
                        && self.m.contests()[id]
                            .rates
                            .as_ref()
                            .is_some_and(|r| r.probability == Rational::one())
                    {
                        continue;
                    }
                    let mut next = self.copy(w)?;
                    self.record(
                        &mut next,
                        k,
                        Occurrence::ContestChoice(id, filed),
                        None,
                        false,
                    )?;
                    out.push(next);
                }
                return Ok(out);
            }
            if let Some(id) = self.m.contests().iter().enumerate().find_map(|(id, _)| {
                (c.frame
                    .trace
                    .iter()
                    .any(|o| matches!(o.event,Occurrence::ContestChoice(n,true) if n==id))
                    && !c
                        .frame
                        .trace
                        .iter()
                        .any(|o| matches!(o.event,Occurrence::ContestQueue(n) if n==id)))
                .then_some(id)
            }) {
                let d = &self.m.contests()[id];
                require(self.m.capabilities[d.route.0].params.is_empty())?;
                c.frame
                    .contest_target(self.m, id)
                    .unwrap()
                    .at
                    .checked_add(d.respond_by)
                    .ok_or(StopReason::SemanticAbort)?;
                let mut next = self.copy(w)?;
                self.record(&mut next, k, Occurrence::ContestQueue(id), None, false)?;
                if crate::responsibility::included(self.m, self.m.capabilities[d.route.0].actor) {
                    let CaseSlot::Here(c) = &mut next.cases[k] else {
                        unreachable!()
                    };
                    let start = c
                        .frame
                        .tasks
                        .iter()
                        .position(|t| self.m.site_contest(t.call.binding).is_some())
                        .unwrap_or(c.frame.tasks.len());
                    let task = c.frame.tasks.len();
                    let waiting = c.frame.waiting.clone();
                    self.cases[k].create(
                        &mut c.frame,
                        &[Call {
                            binding: self.m.contest_site(id),
                            capability: d.route,
                            args: Vec::new(),
                            span: d.span,
                        }],
                        None,
                    )?;
                    if crate::contest::has_effects(self.m) {
                        c.frame.waiting = waiting;
                    } else {
                        c.frame.waiting.as_mut().unwrap().tasks.start = start;
                    }
                    c.queue.push(QueueEntry { task, at: w.now });
                }
                return Ok(vec![next]);
            }
            if c.end.is_some() {
                return Ok(Vec::new());
            }
        }
        if let Some(wake) = c.frame.sleep() {
            if wake > w.now {
                return Ok(Vec::new());
            }
            let mut next = self.copy(w)?;
            self.record(&mut next, k, Occurrence::Wake, None, false)?;
            return Ok(vec![next]);
        }
        if let Some(wait) = &c.frame.waiting {
            if !wait.tasks.clone().all(|j| c.frame.tasks[j].terminal())
                || !self.cases[k].winners(&c.frame, wait).is_empty()
            {
                return Ok(Vec::new());
            }
            let mut next = self.copy(w)?;
            if wait.join.is_none()
                && self
                    .m
                    .response_attempt(c.frame.tasks[wait.tasks.start].call.binding)
                    .is_some()
            {
                self.response_progress(&mut next, k)?;
                return Ok(vec![next]);
            }
            if let Some(Join::Any { if_none, .. }) = wait.join
                && wait.winner.is_none()
            {
                self.ending(&mut next, k, *if_none)?;
            } else if let CaseSlot::Here(c) = &mut next.cases[k] {
                c.frame.waiting = None;
            }
            return Ok(vec![next]);
        }
        if c.frame.control.iter().all(|b| b.is_empty()) {
            return Ok(Vec::new());
        }
        if c.frame.control.iter().rev().find_map(|b| b.first()).is_some_and(|s| matches!(&s.kind, Stmt::If { condition, .. } if shared_guard(self.m, condition))) { return Ok(Vec::new()); }
        let mut next = self.copy(w)?;
        let CaseSlot::Here(c) = &mut next.cases[k] else {
            unreachable!()
        };
        while c.frame.control.last().is_some_and(|b| b.is_empty()) {
            c.frame.control.pop();
        }
        let block = c
            .frame
            .control
            .last_mut()
            .ok_or(StopReason::SemanticAbort)?;
        let statement = &block[0];
        *block = &block[1..];
        match &statement.kind {
            Stmt::WaitUntil(_) => return Err(StopReason::SemanticAbort),
            Stmt::Wait(duration) => {
                let wake = w
                    .now
                    .checked_add(*duration)
                    .ok_or(StopReason::SemanticAbort)?;
                self.record(&mut next, k, Occurrence::Wait(wake), None, false)?;
            }
            Stmt::End(end) => self.ending(&mut next, k, *end)?,
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                let yes = self.condition(&next, k, condition)?;
                require(
                    self.cases[k]
                        .statistics
                        .observed
                        .branch_statement(self.m, statement, yes),
                )?;
                let branch = if yes { then_body } else { else_body };
                let CaseSlot::Here(c) = &mut next.cases[k] else {
                    unreachable!()
                };
                if c.frame.control.len() >= self.limits.control_depth {
                    return Err(StopReason::EnumerationLimit);
                }
                c.frame.control.push(branch);
            }
            Stmt::Response(call, r) => {
                if let Some(event) = response::input(&c.frame, self.m, call, r, k)? {
                    self.record(&mut next, k, event, None, false)?;
                }
                let CaseSlot::Here(c) = &mut next.cases[k] else {
                    unreachable!()
                };
                self.cases[k].create(&mut c.frame, std::slice::from_ref(call), None)?;
                c.queue.extend(
                    c.frame
                        .waiting
                        .as_ref()
                        .unwrap()
                        .tasks
                        .clone()
                        .map(|task| QueueEntry { task, at: w.now }),
                );
            }
            Stmt::Call(call) => {
                self.cases[k].create(&mut c.frame, std::slice::from_ref(call), None)?;
                c.queue.extend(
                    c.frame
                        .waiting
                        .as_ref()
                        .unwrap()
                        .tasks
                        .clone()
                        .map(|task| QueueEntry { task, at: w.now }),
                );
            }
            Stmt::Fork { branches, join, .. } => {
                self.cases[k].create(&mut c.frame, branches, Some(join))?;
                c.queue.extend(
                    c.frame
                        .waiting
                        .as_ref()
                        .unwrap()
                        .tasks
                        .clone()
                        .map(|task| QueueEntry { task, at: w.now }),
                );
            }
        }
        Ok(vec![next])
    }

    fn walk(&mut self, w: World<'a>) -> Result<Vec<Bound>, StopReason> {
        if self.depth >= self.limits.branch_depth {
            return Err(StopReason::EnumerationLimit);
        }
        self.depth += 1;
        let saved = self.first_hits.clone();
        let saved_closed = self.first_closed.clone();
        let result = self.walk_inner(w);
        self.first_hits = saved;
        self.first_closed = saved_closed;
        self.depth -= 1;
        result
    }

    // Keep management's probability layer out of the recursive walk's frame.
    // Legacy choices remain nondeterministic; only a pending rates clause gets
    // a weighted reduction. No scheduler choices are assigned probabilities.
    fn contest_branches(&mut self, w: &World<'a>) -> Result<Option<Vec<Bound>>, StopReason> {
        for k in 0..w.cases.len() {
            let probability = match &w.cases[k] {
                CaseSlot::Here(c) => c
                    .frame
                    .contest_pending(self.m)
                    .and_then(|id| self.m.contests()[id].rates.as_ref())
                    .map(|r| r.probability.clone()),
                _ => None,
            };
            let states = self.contest_management(w, k)?;
            if states.is_empty() {
                continue;
            }
            let weights = match &probability {
                Some(p) => vec![Rational(Rational::one().0 - &p.0), p.clone()]
                    .into_iter()
                    .filter(|p| !p.is_zero())
                    .collect::<Vec<_>>(),
                None => vec![Rational::one(); states.len()],
            };
            require(weights.len() == states.len())?;
            let mut children = Vec::new();
            for (weight, next) in weights.into_iter().zip(states) {
                children.push((weight, self.walk(next)?));
            }
            return Ok(Some(self.reduce(children, probability.is_some())));
        }
        Ok(None)
    }

    fn walk_inner(&mut self, mut w: World<'a>) -> Result<Vec<Bound>, StopReason> {
        loop {
            self.observe_first(&w)?;
            if w.steps >= self.limits.steps_per_path {
                return Err(StopReason::EnumerationLimit);
            }
            w.steps += 1;
            if w.cases.iter().any(|s|matches!(s,CaseSlot::Here(c)if c.frame.tasks.iter().filter(|t|!t.terminal()).count()as u64>self.m.bounds.live_tasks)) {return self.leaf(&w,ReferenceCut::LiveTasks)}
            if w.now > self.m.bounds.clock {
                return self.leaf(&w, ReferenceCut::Clock);
            }
            match w.boundary {
                Boundary::Coins => {
                    let Some(coins) = self.coins(&w, 4096)? else {
                        return self.leaf(&w, ReferenceCut::Numeric);
                    };
                    if !coins.is_empty() {
                        return self.coin_tree(&w, &coins);
                    }
                    w.boundary = Boundary::Admit;
                }
                Boundary::Admit => {
                    while w.next < self.times.len() && self.times[w.next] == w.now {
                        let k = w.next;
                        w.next += 1;
                        let waiting=w.cases.iter().filter(|s|matches!(s,CaseSlot::Here(c)if c.end.is_none() && !c.frame.tasks.iter().any(|t|t.start.is_some()))).count()as u64;
                        if self
                            .m
                            .admission_queue
                            .is_some_and(|q| waiting >= q.capacity)
                            || self.m.concurrent_cases.is_some_and(|q| {
                                w.cases
                                    .iter()
                                    .filter(|s| matches!(s, CaseSlot::Here(c) if c.end.is_none()))
                                    .count() as u64
                                    >= q.capacity
                            })
                        {
                            let overflow = self
                                .m
                                .admission_queue
                                .or(self.m.concurrent_cases)
                                .unwrap()
                                .overflow;
                            if overflow != Overflow::Reject {
                                let frame = self.new_case(w.now);
                                w.cases[k] = match overflow {
                                    Overflow::WaitExternalFifo
                                    | Overflow::WaitExternalPriority(_) => CaseSlot::Held(frame),
                                    Overflow::End(_) => CaseSlot::Overflow(frame),
                                    Overflow::Reject => unreachable!(),
                                };
                                continue;
                            }
                            w.cases[k] = CaseSlot::Declined;
                            self.terminal_value(
                                k,
                                CaseTerminal {
                                    observed_until: None,
                                    observed_elapsed: None,
                                    observed_work: None,
                                    kind: Kind::Rejected,
                                    completed_at: None,
                                    elapsed: None,
                                    work: None,
                                    result_at: None,
                                    probability: None,
                                },
                            )?;
                        } else {
                            let frame = Frame {
                                condition_waits: Vec::new(),
                                lost_updates: BTreeSet::new(),
                                state_snapshots: Vec::new(),
                                business: None,
                                state_values: self
                                    .m
                                    .state_variables
                                    .iter()
                                    .map(|v| match (v.kind, v.initial) {
                                        (_, _) if shared_state(self.m, v.kind) => StateValue::None,
                                        (_, InitialValue::Fixed(x)) => x,
                                        _ => StateValue::None,
                                    })
                                    .collect(),
                                trace: Vec::new(),
                                now: w.now,
                                steps: 0,
                                control: vec![&self.m.process.body],
                                waiting: None,
                                tasks: Vec::new(),
                                adoption: None,
                            };
                            w.cases[k] = CaseSlot::Here(CaseFrame {
                                frame,
                                queue: Vec::new(),
                                receipt: false,
                                end: None,
                            });
                        }
                    }
                    w.boundary = Boundary::Draw;
                    w.external_peak = w.external_peak.max(
                        w.cases
                            .iter()
                            .filter(|c| matches!(c, CaseSlot::Held(_)))
                            .count() as u64,
                    );
                }
                // Nothing to draw/choose: initials would yield one weight-1 child at a walk level and frame per arrival (aset-limit's 64 stop at the limits).
                Boundary::Draw
                    if self
                        .m
                        .state_variables
                        .iter()
                        .any(|v| v.case_set_size.is_some())
                        && self.m.initial_distributions.is_empty() =>
                {
                    w.boundary = Boundary::Choose
                }
                Boundary::Choose
                    if self
                        .m
                        .state_variables
                        .iter()
                        .any(|v| v.case_set_size.is_some())
                        && !self
                            .m
                            .state_variables
                            .iter()
                            .any(|v| v.initial == InitialValue::Nondet) =>
                {
                    w.boundary = Boundary::Receive
                }
                Boundary::Draw => return self.initials(&w, true),
                Boundary::Choose => return self.initials(&w, false),
                Boundary::Receive => {
                    for k in 0..w.cases.len() {
                        if w.cases[k].payload().is_some_and(|c| !c.receipt) {
                            self.record(&mut w, k, Occurrence::Receipt, None, false)?;
                            if let Some(c) = w.cases[k].payload_mut() {
                                c.receipt = true;
                            }
                        }
                    }
                    w.boundary = if self.m.explicit_limits() {
                        Boundary::Activate
                    } else {
                        Boundary::Run
                    };
                }
                Boundary::Activate => {
                    for k in 0..w.cases.len() {
                        if matches!(&w.cases[k], CaseSlot::Here(c) if !c.frame.trace.iter().any(|o| matches!(o.event, Occurrence::Admission)))
                        {
                            self.record(&mut w, k, Occurrence::Admission, None, false)?;
                        } else if matches!(&w.cases[k], CaseSlot::Overflow(c) if c.end.is_none()) {
                            self.ending(
                                &mut w,
                                k,
                                self.m.overflow_end.ok_or(StopReason::SemanticAbort)?,
                            )?;
                        }
                    }
                    w.boundary = Boundary::Run;
                }
                Boundary::Run => break,
            }
        }
        self.observe(&w)?;
        // Resolve every case's management stack before dispatch. Past Tasks and
        // queue records remain intact; receipt waiting is read from their history.
        let mut children = Vec::new();
        if !self.m.contests().is_empty()
            && let Some(result) = self.contest_branches(&w)?
        {
            return Ok(result);
        }
        for k in 0..w.cases.len() {
            if !self.m.contests().is_empty() {
                break;
            }
            while let CaseSlot::Here(c) = &w.cases[k] {
                if c.end.is_some()
                    || c.frame
                        .trace
                        .iter()
                        .any(|o| matches!(o.event, Occurrence::ResponseStopped(_)))
                {
                    break;
                }
                if let Some(wake) = c.frame.sleep() {
                    if wake > w.now {
                        break;
                    }
                    self.record(&mut w, k, Occurrence::Wake, None, false)?;
                    continue;
                }
                if let Some(wait) = &c.frame.waiting {
                    if !wait.tasks.clone().all(|j| c.frame.tasks[j].terminal()) {
                        break;
                    }
                    let winners = self.cases[k].winners(&c.frame, wait);
                    if !winners.is_empty() {
                        break;
                    }
                    if wait.join.is_none()
                        && self
                            .m
                            .response_attempt(c.frame.tasks[wait.tasks.start].call.binding)
                            .is_some()
                    {
                        self.response_progress(&mut w, k)?;
                        continue;
                    }
                    if let Some(Join::Any { if_none, .. }) = wait.join
                        && wait.winner.is_none()
                    {
                        let end = *if_none;
                        self.ending(&mut w, k, end)?;
                        break;
                    }
                    if let CaseSlot::Here(c) = &mut w.cases[k] {
                        c.frame.waiting = None;
                    }
                    continue;
                }
                let CaseSlot::Here(c) = &mut w.cases[k] else {
                    unreachable!()
                };
                while c.frame.control.last().is_some_and(|b| b.is_empty()) {
                    c.frame.control.pop();
                }
                let Some(block) = c.frame.control.last_mut() else {
                    break;
                };
                let statement = &block[0];
                if let Stmt::WaitUntil(condition) = &statement.kind {
                    if shared_guard(self.m, condition) {
                        c.frame.condition_enter();
                        break;
                    }
                    if self.cases[k].condition(&c.frame, condition, 0)? {
                        c.frame.condition_pass()?;
                        continue;
                    }
                    c.frame.condition_enter();
                    break;
                }
                if matches!(&statement.kind, Stmt::If { condition, .. } if shared_guard(self.m, condition))
                {
                    break;
                }
                *block = &block[1..];
                match &statement.kind {
                    Stmt::WaitUntil(_) => return Err(StopReason::SemanticAbort),
                    Stmt::Wait(duration) => {
                        let wake = w
                            .now
                            .checked_add(*duration)
                            .ok_or(StopReason::SemanticAbort)?;
                        self.record(&mut w, k, Occurrence::Wait(wake), None, false)?;
                    }
                    Stmt::End(e) => {
                        self.ending(&mut w, k, *e)?;
                        break;
                    }
                    Stmt::If {
                        condition,
                        then_body,
                        else_body,
                    } => {
                        let yes = self.condition(&w, k, condition)?;
                        require(
                            self.cases[k]
                                .statistics
                                .observed
                                .branch_statement(self.m, statement, yes),
                        )?;
                        let branch = if yes { then_body } else { else_body };
                        if let CaseSlot::Here(c) = &mut w.cases[k] {
                            if c.frame.control.len() >= self.limits.control_depth {
                                return Err(StopReason::EnumerationLimit);
                            }
                            c.frame.control.push(branch);
                        }
                    }
                    Stmt::Response(call, r) => {
                        if let Some(event) = response::input(&c.frame, self.m, call, r, k)? {
                            self.record(&mut w, k, event, None, false)?;
                        }
                        let CaseSlot::Here(c) = &mut w.cases[k] else {
                            unreachable!()
                        };
                        self.cases[k].create(&mut c.frame, std::slice::from_ref(call), None)?;
                        c.queue.extend(
                            c.frame
                                .waiting
                                .as_ref()
                                .unwrap()
                                .tasks
                                .clone()
                                .map(|task| QueueEntry { task, at: w.now }),
                        );
                    }
                    Stmt::Call(call) => {
                        self.cases[k].create(&mut c.frame, std::slice::from_ref(call), None)?;
                        let range = c.frame.waiting.as_ref().unwrap().tasks.clone();
                        c.queue
                            .extend(range.map(|task| QueueEntry { task, at: w.now }));
                    }
                    Stmt::Fork { branches, join, .. } => {
                        self.cases[k].create(&mut c.frame, branches, Some(join))?;
                        let range = c.frame.waiting.as_ref().unwrap().tasks.clone();
                        c.queue
                            .extend(range.map(|task| QueueEntry { task, at: w.now }));
                    }
                }
                if w.cases.iter().any(|s|matches!(s,CaseSlot::Here(c)if c.frame.tasks.iter().filter(|t|!t.terminal()).count()as u64>self.m.bounds.live_tasks)){return self.leaf(&w,ReferenceCut::LiveTasks)}
            }
        }
        let mut policy_choices = 0usize;
        for k in 0..w.cases.len() {
            let CaseSlot::Here(c) = &w.cases[k] else {
                continue;
            };
            if c.stopped(self.m) {
                continue;
            }
            if c.end.is_none()
                && c.frame.waiting.is_none()
                && c.frame.sleep().is_none()
                && let Some(statement) = c.frame.control.iter().rev().find_map(|b| b.first())
                && let Stmt::WaitUntil(condition) = &statement.kind
                && shared_guard(self.m, condition)
                && self.condition(&w, k, condition)?
            {
                let mut next = self.copy(&w)?;
                if let CaseSlot::Here(c) = &mut next.cases[k] {
                    c.frame.condition_pass()?;
                }
                children.push((Rational::one(), self.walk(next)?));
            }
            if c.end.is_none()
                && c.frame.waiting.is_none()
                && c.frame.sleep().is_none()
                && let Some(statement) = c.frame.control.iter().rev().find_map(|b| b.first())
                && let Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } = &statement.kind
                && shared_guard(self.m, condition)
            {
                let yes = self.condition(&w, k, condition)?;
                require(
                    self.cases[k]
                        .statistics
                        .observed
                        .branch_statement(self.m, statement, yes),
                )?;
                let mut next = self.copy(&w)?;
                if let CaseSlot::Here(c) = &mut next.cases[k] {
                    while c.frame.control.last().is_some_and(|b| b.is_empty()) {
                        c.frame.control.pop();
                    }
                    let block = c
                        .frame
                        .control
                        .last_mut()
                        .ok_or(StopReason::SemanticAbort)?;
                    *block = &block[1..];
                    if c.frame.control.len() >= self.limits.control_depth {
                        return Err(StopReason::EnumerationLimit);
                    }
                    c.frame
                        .control
                        .push(if yes { then_body } else { else_body });
                }
                children.push((Rational::one(), self.walk(next)?));
            }
            let due: Vec<_> = c
                .frame
                .tasks
                .iter()
                .enumerate()
                .filter_map(|(j, t)| {
                    (t.phase == Phase::Running && (t.finish == Some(w.now) || t.ack == Some(w.now)))
                        .then_some(j)
                })
                .collect();
            for &j in &due {
                let CaseSlot::Here(c) = &w.cases[k] else {
                    unreachable!()
                };
                let t = &c.frame.tasks[j];
                let cap_id = t.call.capability;
                let cap = &self.m.capabilities[cap_id.0];
                let person = t.person;
                let kernel = work_profiles::kernel(self.m, t)?;
                if t.finish == Some(w.now) && self.priority_finish(&w, k, j) {
                    let mut frame = c.frame.clone();
                    frame.state_values = self.values(&w, k);
                    if self.cases[k].range_attempt(&frame, j, false, Some(k as u64 + 1))? {
                        children.push((Rational::one(), self.leaf(&w, ReferenceCut::Range)?));
                    } else {
                        let options = if reference_stale(t, cap) {
                            vec![(
                                Rational::one(),
                                cap.version_check.as_ref().unwrap().reject,
                                Vec::new(),
                            )]
                        } else if let Some(kernel) = kernel {
                            let vals = self.values(&w, k);
                            let row = kernel
                                .rows
                                .iter()
                                .find(|r| {
                                    r.values
                                        .iter()
                                        .zip(&kernel.given)
                                        .all(|(x, id)| *x == vals[id.0])
                                })
                                .ok_or(StopReason::SemanticAbort)?;
                            row.branches
                                .iter()
                                .filter(|b| !b.probability.is_zero())
                                .map(|b| (b.probability.clone(), b.result, b.overrides.clone()))
                                .collect::<Vec<_>>()
                        } else if let Some(a) = &cap.atomic {
                            let x = if self.condition(&w, k, &a.condition)? == a.commit_when {
                                a.commit
                            } else {
                                a.reject
                            };
                            vec![(Rational::one(), x, Vec::new())]
                        } else {
                            cap.results
                                .iter()
                                .map(|x| (Rational::one(), *x, Vec::new()))
                                .collect()
                        };
                        self.reserve(options.len())?;
                        let mut finishes = Vec::new();
                        for (p, x, overrides) in options {
                            let mut next = w.clone();
                            let values = self.values(&next, k);
                            let CaseSlot::Here(c) = &mut next.cases[k] else {
                                unreachable!()
                            };
                            c.frame.state_values = values;
                            c.frame.tasks[j].phase = Phase::Finished(x);
                            c.frame.tasks[j].stop = Some(w.now);
                            self.cases[k].finish_state(&mut c.frame, j, x);
                            for (i, value) in overrides {
                                c.frame.state_values[i.0] = value;
                            }
                            for (i, v) in self.m.state_variables.iter().enumerate() {
                                if shared_state(self.m, v.kind) {
                                    next.shared[i] = c.frame.state_values[i];
                                    c.frame.state_values[i] = StateValue::None;
                                }
                            }
                            let writes = if reference_stale(t, cap) {
                                &[][..]
                            } else {
                                cap.atomic.as_ref().map_or(cap.writes.as_slice(), |a| {
                                    if x == a.commit { &a.writes } else { &[] }
                                })
                            };
                            self.notify(&mut next, k, j, writes);
                            self.record_context(
                                &mut next,
                                k,
                                Occurrence::Completion(cap_id, x),
                                person,
                                (false, Some(t.call.binding)),
                            )?;
                            self.stats.normal_completions += 1;
                            finishes.push((p, self.response_finish(next, k, t.call.binding, x)?));
                        }
                        if kernel.is_some() {
                            let x = self.reduce(finishes, true);
                            children.push((Rational::one(), x));
                        } else {
                            children.extend(finishes);
                        }
                    }
                }
                if t.ack == Some(w.now) {
                    let mut next = self.copy(&w)?;
                    let CaseSlot::Here(c) = &mut next.cases[k] else {
                        unreachable!()
                    };
                    c.frame.tasks[j].phase = Phase::Cancelled;
                    c.frame.tasks[j].stop = Some(w.now);
                    if self.m.tracks_obligations() {
                        self.record(
                            &mut next,
                            k,
                            Occurrence::CancelAck(t.call.capability),
                            t.person,
                            false,
                        )?;
                    }
                    self.stats.cancellation_completions += 1;
                    children.push((Rational::one(), self.walk(next)?));
                }
            }
            let CaseSlot::Here(c) = &w.cases[k] else {
                unreachable!()
            };
            if c.end.is_none()
                && due.is_empty()
                && let Some(wait) = &c.frame.waiting
            {
                let winners = self.cases[k].winners(&c.frame, wait);
                self.policy_unresolved |= winners.len() > 1;
                for winner in winners {
                    let child = self.cancel_choices(&w, k, winner)?;
                    children.push((Rational::one(), child));
                }
            }
            let task_count = c.frame.tasks.len();
            for j in 0..task_count {
                let eligible = self.eligible(&w, k, j)?;
                if !eligible.is_empty()
                    && self.m.start_policy.as_ref().is_some_and(|p| {
                        p.controls(self.m.capabilities[c.frame.tasks[j].call.capability.0].actor)
                    })
                {
                    let cap = &self.m.capabilities[c.frame.tasks[j].call.capability.0];
                    let count = if let Some(req) = &cap.requires
                        && !self.condition(&w, k, &req.condition)?
                    {
                        1
                    } else {
                        eligible.len()
                    };
                    policy_choices += count;
                    self.policy_unresolved |= policy_choices > 1;
                }
                for person in eligible {
                    let CaseSlot::Here(old) = &w.cases[k] else {
                        unreachable!()
                    };
                    let task = &old.frame.tasks[j];
                    let id = task.call.capability;
                    let cap = &self.m.capabilities[id.0];
                    if let Some(req) = &cap.requires
                        && !self.condition(&w, k, &req.condition)?
                    {
                        let binding = task.call.binding;
                        let mut refused = self.copy(&w)?;
                        if let CaseSlot::Here(c) = &mut refused.cases[k] {
                            c.frame.tasks[j].phase = Phase::Refused(req.otherwise);
                            c.frame.tasks[j].stop = Some(w.now);
                        }
                        self.record_context(
                            &mut refused,
                            k,
                            Occurrence::Refusal(id, req.otherwise),
                            None,
                            (false, Some(binding)),
                        )?;
                        children.push((Rational::one(), self.walk(refused)?));
                        break;
                    }
                    let mut frame = old.frame.clone();
                    frame.state_values = self.values(&w, k);
                    if self.cases[k].range_attempt(&frame, j, true, Some(k as u64 + 1))? {
                        children.push((Rational::one(), self.leaf(&w, ReferenceCut::Range)?));
                        continue;
                    }
                    let before = frame.state_values;
                    let frozen = work_profiles::select(self.m, id, person, self.backlog(&w, k, j))?;
                    let mut next = self.copy(&w)?;
                    let CaseSlot::Here(c) = &mut next.cases[k] else {
                        unreachable!()
                    };
                    let task = &mut c.frame.tasks[j];
                    let id = task.call.capability;
                    let cap = &self.m.capabilities[id.0];
                    task.frozen_performance = frozen;
                    task.phase = Phase::Running;
                    task.stale.clear();
                    task.person = person;
                    task.start = Some(w.now);
                    if let Some(d) = &cap.duration_distribution {
                        for (t, p) in &d.entries {
                            if !p.is_zero() {
                                w.now.checked_add(*t).ok_or(StopReason::SemanticAbort)?;
                            }
                        }
                    } else {
                        task.finish = Some(
                            w.now
                                .checked_add(cap.duration)
                                .ok_or(StopReason::SemanticAbort)?,
                        );
                    }
                    let binding = task.call.binding;
                    c.frame.state_values = before.clone();
                    for w in &cap.start_writes {
                        c.frame.state_values[w.variable.0] =
                            reference_effect_at(w, &before, None, k, person)?;
                    }
                    for (i, v) in self.m.state_variables.iter().enumerate() {
                        if shared_state(self.m, v.kind) {
                            next.shared[i] = c.frame.state_values[i];
                            c.frame.state_values[i] = StateValue::None;
                        }
                    }
                    self.notify(&mut next, k, j, &cap.start_writes);
                    self.record_context(
                        &mut next,
                        k,
                        Occurrence::Dispatch(id),
                        person,
                        (false, Some(binding)),
                    )?;
                    #[cfg(test)]
                    q6_private::started(self.m, &w, &next, k, j);
                    children.push((Rational::one(), self.walk(next)?));
                }
            }
        }
        if children.is_empty() && self.m.external_priority().is_some() {
            self.priority_readmission_children(&w, &mut children)?;
        }
        if !children.is_empty() {
            return Ok(self.reduce(children, false));
        }
        if self.legacy_readmit(&mut w)? {
            return self.walk(w);
        }
        let ongoing=w.cases.iter().any(|s|matches!(s,CaseSlot::Here(c)if c.frame.tasks.iter().any(|t|t.phase==Phase::Running)));
        let active = w
            .cases
            .iter()
            .any(|s| s.payload().is_some_and(|c| !c.stopped(self.m)));
        let sleeping = w
            .cases
            .iter()
            .any(|s| matches!(s, CaseSlot::Here(c) if c.frame.sleep().is_some()));
        let calendar_pending = self.calendar_pending(&w)?;
        if !ongoing && !sleeping && !calendar_pending && w.next == self.times.len() {
            if active {
                self.deadlock = true;
                self.stats.deadlock_paths += 1;
            }
            return self.leaf(&w, ReferenceCut::None);
        }
        if self.m.horizon == Some(w.now) && w.now <= self.m.bounds.clock {
            return self.leaf(&w, ReferenceCut::Horizon);
        }
        if w.now == self.m.bounds.clock {
            return self.leaf(&w, ReferenceCut::Clock);
        }
        w.now = w.now.checked_add(1).ok_or(StopReason::SemanticAbort)?;
        w.boundary = Boundary::Coins;
        for slot in &mut w.cases {
            if let Some(c) = slot.payload_mut()
                && !c.stopped(self.m)
            {
                c.frame.now = w.now;
            }
        }
        self.walk(w)
    }
}
fn quality_truth(
    m: &Model,
    p: &QualityProperty,
    slot: &CaseSlot<'_>,
    k: usize,
    arrival: Tick,
    now: Tick,
    cut: ReferenceCut,
) -> Result<Bound, StopReason> {
    let mut hit = false;
    if let Some(c) = slot.payload()
        && c.receipt
    {
        for o in &c.frame.trace {
            if o.at < arrival || p.within.is_some_and(|d| o.at - arrival > d) {
                continue;
            }
            hit |= match &p.target {
                QualityTarget::Ends(es) => {
                    matches!(o.event,Occurrence::Terminal(e) if es.contains(&e))
                }
                QualityTarget::State(c) => snapshot_condition_at(c, &o.values, &o.state_values, k)?,
            };
        }
    }
    if hit {
        return Ok(Bound::exact(true));
    }
    let closed = matches!(slot, CaseSlot::Declined)
        || slot.payload().is_some_and(|c| {
            c.end.is_some() && (matches!(p.target, QualityTarget::Ends(_)) || c.stopped(m))
        });
    let expired = p.within.is_some_and(|d| {
        (now >= arrival && now - arrival > d)
            || (matches!(cut, ReferenceCut::Clock | ReferenceCut::Horizon)
                && arrival
                    <= if cut == ReferenceCut::Horizon {
                        now
                    } else {
                        m.bounds.clock
                    }
                && d <= (if cut == ReferenceCut::Horizon {
                    now
                } else {
                    m.bounds.clock
                }) - arrival)
    });
    Ok(if closed || cut == ReferenceCut::None || expired {
        Bound::exact(false)
    } else {
        let mut b = Bound::unknown(cut == ReferenceCut::Numeric);
        b.computational = cut != ReferenceCut::Horizon;
        b
    })
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
pub(super) fn explore(m: &Model, limits: Limits) -> ReferenceRun {
    explore_projected(m, limits, Projection::Full)
}

pub(super) fn explore_projected(m: &Model, limits: Limits, projection: Projection) -> ReferenceRun {
    #[cfg(test)]
    if m.quality.as_ref().is_some_and(|q| !q.quantiles.is_empty()) {
        super::quality::GRAPH_BUILDS.with(|n| n.set(n.get() + 1));
    }
    let times = m.arrivals.clone().unwrap_or_else(|| {
        if m.bounds.cases == 1 {
            vec![0]
        } else {
            Vec::new()
        }
    });
    let n = times.len();
    let mut tree = Tree {
        response_exercise: BTreeMap::new(),
        response_receipts: Vec::new(),
        first_closed: vec![
            vec![false; n];
            m.quality.as_ref().map_or(0, |q| if q.quantiles.is_empty() {
                0
            } else {
                q.properties.len()
            })
        ],
        first_hits: vec![
            vec![None; n];
            m.quality.as_ref().map_or(0, |q| if q.quantiles.is_empty() {
                0
            } else {
                q.properties.len()
            })
        ],
        first_trees: Vec::new(),
        #[cfg(test)]
        resource_worlds: None,
        depth: 0,
        m,
        limits,
        times,
        cases: (0..n)
            .map(|k| {
                let mut i = interpreter(m, limits);
                i.case_index = k;
                i
            })
            .collect(),
        terminals: vec![BTreeMap::new(); n],
        unresolved: vec![false; n],
        stats: Statistics {
            projection,
            ..Statistics::default()
        },
        truncated: false,
        numeric: false,
        deadlock: false,
        overcommit: false,
        chance: false,
        scheduler: false,
        policy_unresolved: false,
        rejects: None,
        overflows: None,
        external_peaks: None,
        admissions: vec![None; n],
        unadmitted: vec![false; n],
        #[cfg(test)]
        initial_leaves: Vec::new(),
        #[cfg(test)]
        receipts: Vec::new(),
        #[cfg(test)]
        duration_leaves: Vec::new(),
    };
    let result = (|| {
        let mut template = m.clone();
        template.bounds.cases = m.bounds.cases;
        validate(&template, limits)?;
        validate_observed_writers(m)?;
        require(
            n > 0 && n as u64 == m.bounds.cases && tree.times.windows(2).all(|p| p[0] <= p[1]),
        )?;
        if n > 1 {
            require(m.state_variables.iter().all(|v| {
                !shared_latent(m, v.kind) || matches!(v.initial, InitialValue::Fixed(_))
            }))?;
            require(m.capabilities.iter().all(|c| {
                c.writes
                    .iter()
                    .map(|w| w.variable)
                    .chain(c.kernel_writes.iter().copied())
                    .all(|i| !shared_latent(m, m.state_variables[i.0].kind))
            }))?;
        }
        tree.reserve(1)?;
        let world = World {
            external_peak: 0,
            now: 0,
            next: 0,
            boundary: Boundary::Coins,
            steps: 0,
            cases: vec![CaseSlot::NotHere; n],
            shared: m
                .state_variables
                .iter()
                .map(|v| match (v.kind, v.initial) {
                    (_, InitialValue::Fixed(x)) if shared_state(m, v.kind) => x,
                    _ => StateValue::None,
                })
                .collect(),
        };
        tree.walk(world)
    })();
    let stop = result
        .as_ref()
        .err()
        .copied()
        .unwrap_or(StopReason::Complete);
    let result = result.ok();
    let stride = m.probability_queries.len()
        + m.ends.len()
        + 1
        + usize::from(m.completion_requested())
        + usize::from(m.horizon.is_some())
        + usize::from(!m.response_attempts().is_empty());
    let mut properties = Vec::new();
    for v in &m.verify {
        if let Property::Contest(id) = v.property {
            properties.push(PropertyOutcome {
                property: v.property,
                verdict: crate::contest::verdict(m, id),
                probability: None,
                cases: None,
            });
            continue;
        }
        let mut cases = Vec::new();
        if !matches!(
            v.property,
            Property::NoResourceOvercommit | Property::NoLostUpdate(_)
        ) {
            for k in 0..n {
                let mut probability = None;
                let verdict = match v.property {
                    p if p.completion() => {
                        if p == Property::AlmostSureEnd {
                            probability = Some(ProbabilityRange {
                                min: Rational::zero(),
                                max: Rational::one(),
                            });
                        }
                        let stats = &tree.cases[k].statistics;
                        if p == Property::AlmostSureEnd && stop == StopReason::Complete {
                            let b = &result.as_ref().unwrap()
                                [k * stride + m.probability_queries.len() + m.ends.len() + 1];
                            probability = Some(ProbabilityRange {
                                min: b.endpoints[0].clone(),
                                max: b.endpoints[3].clone(),
                            });
                            if b.numeric {
                                Verdict::Inconclusive
                            } else if b.endpoints[0] == Rational::one() {
                                Verdict::Satisfied
                            } else if b.endpoints[1] < Rational::one() {
                                Verdict::Violated
                            } else {
                                Verdict::Inconclusive
                            }
                        } else if stats.completion_bad {
                            Verdict::Violated
                        } else if stop != StopReason::Complete {
                            Verdict::Unsupported
                        } else if stats.completion_unknown {
                            Verdict::Inconclusive
                        } else {
                            Verdict::Satisfied
                        }
                    }
                    Property::ProbabilityReached(id) if stop == StopReason::Complete => {
                        let b = &result.as_ref().unwrap()[k * stride + id];
                        let q = &m.probability_queries[id];
                        probability = Some(ProbabilityRange {
                            min: b.endpoints[0].clone(),
                            max: b.endpoints[3].clone(),
                        });
                        if b.numeric {
                            Verdict::Inconclusive
                        } else {
                            match q.comparison {
                                Comparison::Le if b.endpoints[3] <= q.bound => Verdict::Satisfied,
                                Comparison::Le if b.endpoints[2] > q.bound => Verdict::Violated,
                                Comparison::Ge if b.endpoints[0] >= q.bound => Verdict::Satisfied,
                                Comparison::Ge if b.endpoints[1] < q.bound => Verdict::Violated,
                                _ => Verdict::Inconclusive,
                            }
                        }
                    }
                    Property::ReachableEnd(e)
                        if tree.terminals[k].contains_key(&Kind::End(e))
                            || tree.terminals[k].contains_key(&Kind::Overflow(e)) =>
                    {
                        Verdict::Satisfied
                    }
                    Property::Contract(id) if tree.cases[k].contract_facts[id.0].1 => {
                        Verdict::Violated
                    }
                    Property::StateInRange if !tree.cases[k].statistics.ranges.is_empty() => {
                        Verdict::Violated
                    }
                    _ if stop != StopReason::Complete => Verdict::Unsupported,
                    _ if tree.unresolved[k] => Verdict::Inconclusive,
                    Property::StateInRange => Verdict::Satisfied,
                    // Complete enumeration is not completion of a caller
                    // waiting on an unknown receipt. Keep this case open.
                    Property::ReachableEnd(_)
                        if tree.terminals[k].contains_key(&Kind::ReceiptUnknown) =>
                    {
                        Verdict::Inconclusive
                    }
                    Property::ReachableEnd(_) => Verdict::Violated,
                    Property::Contract(id) => {
                        if tree.cases[k].contract_facts[id.0].0 {
                            Verdict::Satisfied
                        } else {
                            Verdict::Vacuous
                        }
                    }
                    _ => Verdict::Unsupported,
                };
                cases.push(CaseProperty {
                    case_id: CaseId(k),
                    arrival: tree.times[k],
                    verdict,
                    probability,
                });
            }
        }
        let verdict = if matches!(
            v.property,
            Property::NoResourceOvercommit | Property::NoLostUpdate(_)
        ) {
            if match v.property {
                Property::NoLostUpdate(id) => tree
                    .cases
                    .iter()
                    .any(|i| i.evidence.lost_updates.contains(&id)),
                _ => tree.overcommit,
            } {
                Verdict::Violated
            } else if stop != StopReason::Complete {
                Verdict::Unsupported
            } else if tree.truncated
                || tree.stats.horizon
                || tree.cases.iter().any(|i| !i.statistics.ranges.is_empty())
            {
                Verdict::Inconclusive
            } else {
                Verdict::Satisfied
            }
        } else {
            fold(cases.iter().map(|c| c.verdict))
        };
        properties.push(PropertyOutcome {
            property: v.property,
            verdict,
            probability: None,
            cases: (!matches!(
                v.property,
                Property::NoResourceOvercommit | Property::NoLostUpdate(_)
            ))
            .then_some(cases),
        });
    }
    let mut case_outcomes = Vec::new();
    for k in 0..n {
        let mut terminals = if stop == StopReason::Complete {
            std::mem::take(&mut tree.terminals[k])
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
                let offset = m.probability_queries.len()
                    + match t.kind {
                        Kind::End(e) | Kind::Overflow(e) => e.0,
                        Kind::Rejected => m.ends.len(),
                        Kind::CutOff => m.ends.len() + 1 + usize::from(m.completion_requested()),
                        Kind::ReceiptUnknown => {
                            m.ends.len()
                                + 1
                                + usize::from(m.completion_requested())
                                + usize::from(m.horizon.is_some())
                        }
                    };
                let b = &result.as_ref().unwrap()[k * stride + offset];
                t.probability = Some(ProbabilityRange {
                    min: b.endpoints[0].clone(),
                    max: b.endpoints[3].clone(),
                });
            }
        }
        case_outcomes.push(CaseOutcome {
            admitted_at: m.external_wait().then(|| {
                if stop == StopReason::Complete && !tree.unadmitted[k] {
                    tree.admissions[k]
                } else {
                    None
                }
            }),
            case_id: CaseId(k),
            arrival: tree.times[k],
            terminals,
        });
    }
    let mut diagnostics = m.diagnostics.clone();
    if tree.deadlock {
        diagnostics.push(diagnostic(
            diag::W_DEADLOCK_0,
            if m.contests().is_empty()
                && !m.completion_requested()
                && m.resources.is_empty()
                && !m.explicit_limits()
                && m.legacy_route()
            {
                "reachable stopped nonterminal path"
            } else {
                "a reachable nonterminal state has no enabled transition"
            },
        ));
    }
    if tree.numeric {
        diagnostics.push(diagnostic(
            diag::W_BUDGET_0,
            "probability denominator budget exceeded",
        ));
    }
    if stop != StopReason::Complete {
        diagnostics.push(diagnostic(
            diag::U_SEM_0,
            if stop == StopReason::EnumerationLimit {
                "reference enumeration limit exceeded"
            } else {
                "reference arithmetic or unresolved semantics"
            },
        ));
    }
    if stop == StopReason::Complete
        && !tree.truncated
        && !tree.stats.horizon
        && tree.cases.iter().all(|i| i.statistics.ranges.is_empty())
    {
        let mut o = crate::completion::Observed::default();
        for c in &tree.cases {
            o.merge(&c.statistics.observed);
        }
        diagnostics.extend(o.warnings(m));
    }
    // ADR 0029 U2: a range stop on a joint branch leaves every case's
    // continuation there unknown, as the global computational cut does.
    let range_stopped = tree.cases.iter().any(|i| !i.statistics.ranges.is_empty());
    let mut single_terminals = Vec::new();
    if n == 1 && !m.response_attempts().is_empty() {
        for t in &case_outcomes[0].terminals {
            if let Kind::End(end) | Kind::Overflow(end) = t.kind {
                single_terminals.push(TerminalOutcome {
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
    ReferenceRun {
        outcome: Outcome {
            response_exercise: if !m.response_attempts().is_empty()
                && stop == StopReason::Complete
                && !tree.truncated
                && !tree.stats.horizon
            {
                Some(
                    tree.response_exercise
                        .iter()
                        .map(
                            |(capability, (min, max))| crate::outcome::CapabilityExercise {
                                capability: *capability,
                                min: *min,
                                max: *max,
                            },
                        )
                        .collect(),
                )
            } else {
                None
            },
            response_receipts: (!m.response_attempts().is_empty())
                .then_some(tree.response_receipts.clone()),
            scenario_observed_until: m.scenario.as_ref().map(|_| {
                let reason = super::reference_reason(
                    stop != StopReason::Complete,
                    tree.truncated,
                    range_stopped,
                );
                let r = tree.stats.scenario_observed_until.unwrap_or(singleton(0));
                crate::metrics::Scalar::Range(crate::metrics::Range {
                    min: reason.is_none().then(|| r.min.into()),
                    max: reason.is_none().then(|| r.max.into()),
                    reason,
                })
            }),
            quality: m
                .quality
                .as_ref()
                .filter(|_| validate(m, limits).is_ok())
                .map(|q| {
                    let base = n * stride;
                    let unknown = Bound::unknown(false);
                    let get = |i: usize| {
                        let b = result
                            .as_ref()
                            .and_then(|r| r.get(base + i))
                            .unwrap_or(&unknown);
                        (b.endpoints.clone(), b.computational)
                    };
                    let ps = (0..q.properties.len())
                        .map(|i| (0..n).map(|k| get(i * n + k)).collect())
                        .collect::<Vec<_>>();
                    let os = (0..crate::quality::outcome_keys(m).len())
                        .map(|i| {
                            (0..n)
                                .map(|k| get(q.properties.len() * n + i * n + k))
                                .collect()
                        })
                        .collect::<Vec<_>>();
                    let abort = stop != StopReason::Complete;
                    let mut out = super::quality::exhaustive(m, &ps, &os, abort);
                    super::quality::add_tree_quantiles(&mut out, m, tree.first_trees.last(), abort);
                    out
                }),
            state_range_violations: tree
                .cases
                .iter()
                .flat_map(|i| i.statistics.ranges.clone())
                .collect(),
            capability_refusals: m.has_requires().then(|| {
                tree.cases
                    .iter()
                    .enumerate()
                    .flat_map(|(k, i)| {
                        i.statistics
                            .refusals
                            .values()
                            .cloned()
                            .map(|mut r| {
                                r.case_id = k as u64 + 1;
                                r.coverage = super::reference_word(
                                    stop != StopReason::Complete,
                                    tree.truncated,
                                    range_stopped,
                                )
                                .into();
                                r
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect()
            }),
            unchecked: if stop != StopReason::Complete
                || tree.truncated
                || tree.stats.horizon
                || range_stopped
            {
                vec!["unreachable".into()]
            } else {
                Vec::new()
            },
            postprocessing_diagnostics: Vec::new(),
            contest_runtime: (!m.contests().is_empty()
                && crate::contest::runtime_declarations_valid(m))
            .then(|| {
                tree.cases
                    .iter()
                    .enumerate()
                    .flat_map(|(k, i)| {
                        m.contests().iter().enumerate().map(move |(id, d)| {
                            crate::contest::Runtime {
                                id: format!("case{}:contest{}", k + 1, id + 1),
                                static_id: d.property_id(),
                                case_id: k as u64 + 1,
                                party: m.actors[d.party.0].name.clone(),
                                target: d.target.name(m),
                                target_kind: d.target.effect().then(|| "effect".into()),
                                route: crate::responsibility::cap_name(m, d.route),
                                respond_by: d.respond_by,
                                basis: "モデル内での期限内応答".into(),
                                coverage: super::reference_coverage(
                                    stop != StopReason::Complete,
                                    tree.truncated,
                                    range_stopped,
                                ),
                                alternatives: contest_table_alternatives(
                                    i.statistics
                                        .contest_alternatives
                                        .get(&id)
                                        .cloned()
                                        .unwrap_or_default(),
                                    m,
                                    id,
                                    tree.truncated,
                                    range_stopped,
                                    stop != StopReason::Complete,
                                ),
                            }
                        })
                    })
                    .collect()
            }),
            contest_summary: (!m.contests().is_empty()).then(|| {
                let reason = super::reference_reason(
                    stop != StopReason::Complete,
                    tree.truncated,
                    range_stopped,
                );
                let range = tree.stats.contest_open.unwrap_or(singleton(0));
                crate::contest::Summary {
                    business_ended_open: crate::metrics::Range {
                        min: reason.is_none().then(|| range.min.into()),
                        max: reason.is_none().then(|| range.max.into()),
                        reason,
                    },
                }
            }),
            obligations: m.tracks_obligations().then(|| {
                tree.cases
                    .iter()
                    .enumerate()
                    .flat_map(|(k, i)| {
                        i.statistics
                            .obligations
                            .iter()
                            .cloned()
                            .map(move |mut row| {
                                row.case_id = k as u64 + 1;
                                row.id = row.id.replacen("case1:", &format!("case{}:", k + 1), 1);
                                row.coverage = super::reference_coverage(
                                    stop != StopReason::Complete,
                                    tree.truncated,
                                    range_stopped,
                                );
                                row
                            })
                    })
                    .collect()
            }),
            metrics: tree.stats.metric_envelope.take().and_then(|e| {
                e.finish(super::reference_word(
                    stop != StopReason::Complete,
                    tree.truncated,
                    range_stopped,
                ))
            }),
            case_outcomes: Some(case_outcomes),
            admission: Some(AdmissionSummary {
                overflow_count: m.overflow_end.map(|_| {
                    tree.overflows.unwrap_or(Range {
                        min: 0,
                        max: n as u64,
                    })
                }),
                external_wait_max: m.external_wait().then(|| {
                    tree.external_peaks.unwrap_or(Range {
                        min: 0,
                        max: n as u64,
                    })
                }),
                rejected_count: tree.rejects.unwrap_or(Range {
                    min: 0,
                    max: n as u64,
                }),
            }),
            properties,
            terminals: single_terminals,
            truncated: tree.truncated,
            diagnostics,
            probability_scheduler: tree.chance && tree.scheduler,
            probability_policy_unresolved: m.start_policy.as_ref().filter(|p| p.extended()).map(
                |_| {
                    tree.policy_unresolved
                        || tree.stats.horizon
                        || tree.truncated
                        || tree.numeric
                        || stop != StopReason::Complete
                        || tree.cases.iter().any(|i| !i.statistics.ranges.is_empty())
                },
            ),
        },
        stop,
        statistics: tree.stats,
    }
}

#[cfg(test)]
mod arrivals_tests {
    use super::*;
    #[test]
    fn effect_reference_queue_history_and_normal_wait_remain_separate() {
        let src = include_str!("../../../../fixtures/sov-concurrent/model.relca").replace(
            "n = Clerk.notify();",
            "n = Clerk.notify(); n2 = Clerk.notify();",
        );
        let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        let mut t = tree(&m, Limits::default(), vec![0]);
        t.resource_worlds = Some(Vec::new());
        t.walk(World {
            external_peak: 0,
            now: 0,
            next: 0,
            boundary: Boundary::Coins,
            shared: Vec::new(),
            steps: 0,
            cases: vec![CaseSlot::NotHere],
        })
        .unwrap();
        let mut saw_route_and_second_wait = false;
        for w in t.resource_worlds.unwrap() {
            if let CaseSlot::Here(c) = &w.cases[0] {
                for q in &c.queue {
                    let task = &c.frame.tasks[q.task];
                    assert_eq!(task.metric_ready, Some(q.at));
                    if m.site_contest(task.call.binding).is_some() {
                        assert_eq!(q.at, 3);
                    }
                }
                if let Some(wait) = &c.frame.waiting {
                    assert!(
                        wait.tasks
                            .clone()
                            .all(|j| m.site_contest(c.frame.tasks[j].call.binding).is_none())
                    );
                    saw_route_and_second_wait |= c
                        .queue
                        .iter()
                        .any(|q| m.site_contest(c.frame.tasks[q.task].call.binding).is_some())
                        && wait
                            .tasks
                            .clone()
                            .any(|j| m.site_name(c.frame.tasks[j].call.binding) == "n2");
                }
            }
        }
        assert!(saw_route_and_second_wait);
    }
    #[test]
    fn resource_same_tick_reference_world_retains_both_finish_and_ack_orders() {
        let mut m = super::super::resource_tests::model("rsrc-cooperative");
        m.capabilities[1].duration = 3;
        m.arrivals = Some(vec![0]);
        let mut t = tree(&m, Limits::default(), vec![0]);
        t.resource_worlds = Some(Vec::new());
        t.walk(World {
            external_peak: 0,
            now: 0,
            next: 0,
            boundary: Boundary::Coins,
            shared: Vec::new(),
            steps: 0,
            cases: vec![CaseSlot::NotHere],
        })
        .unwrap();
        let mut endings = BTreeSet::new();
        for w in t.resource_worlds.unwrap() {
            if let CaseSlot::Here(c) = &w.cases[0]
                && w.now == 3
                && c.frame.tasks.get(1).is_some_and(Task::terminal)
            {
                endings.insert(matches!(c.frame.tasks[1].phase, Phase::Finished(_)));
                if c.frame.tasks.iter().all(|t| t.phase != Phase::Running) {
                    assert_eq!(
                        reference_resource_units(c.frame.tasks.iter(), &m).unwrap(),
                        [0]
                    );
                }
            }
        }
        assert_eq!(endings, BTreeSet::from([false, true]));
    }
    fn resource_world(m: &Model) -> World<'_> {
        World {
            external_peak: 0,
            now: 0,
            next: 2,
            boundary: Boundary::Run,
            shared: Vec::new(),
            steps: 0,
            cases: (0..2)
                .map(|_| {
                    let mut f = replay::initial(m);
                    f.tasks = super::super::resource_tests::tasks(m);
                    f.tasks.truncate(1);
                    CaseSlot::Here(CaseFrame {
                        frame: f,
                        queue: vec![QueueEntry { task: 0, at: 0 }],
                        receipt: true,
                        end: None,
                    })
                })
                .collect(),
        }
    }
    #[test]
    fn resource_h01_h08_reference_world_observer_resource_only_and_overflow() {
        for capacity in [2, 1] {
            let mut m = super::super::resource_tests::model("rsrc-global.broken-two-units");
            m.resources[0].capacity = capacity;
            let mut w = resource_world(&m);
            for slot in &mut w.cases {
                if let CaseSlot::Here(c) = slot {
                    c.frame.tasks[0].phase = Phase::Running;
                    c.frame.tasks[0].start = Some(0);
                    c.frame.tasks[0].finish = Some(2);
                }
            }
            if let CaseSlot::Here(c) = &mut w.cases[0] {
                c.end = Some(EndId(0));
            }
            for cut in [false, true] {
                let mut t = tree(&m, Limits::default(), vec![0, 0]);
                t.truncated = cut;
                t.numeric = cut;
                if cut {
                    t.stats
                        .ranges
                        .push(super::super::resource_tests::range_witness());
                }
                t.observe(&w).unwrap();
                assert_eq!(t.overcommit, capacity == 1);
            }
        }
        let mut m = super::super::resource_tests::model("rsrc-global");
        m.resources[0].capacity = u64::MAX;
        m.capabilities[0].uses[0].amount = u64::MAX;
        let mut w = resource_world(&m);
        for slot in &mut w.cases {
            if let CaseSlot::Here(c) = slot {
                c.frame.tasks[0].phase = Phase::Running;
            }
        }
        assert_eq!(
            tree(&m, Limits::default(), vec![0, 0]).observe(&w),
            Err(StopReason::SemanticAbort)
        );
    }
    #[test]
    fn resource_h10_reference_world_fifo_unstartable_head_and_other_actor_control() {
        let mut m = super::super::resource_tests::model("rsrc-global");
        m.capabilities.push(m.capabilities[0].clone());
        m.capabilities[0].uses[0].amount = 2;
        for other_actor in [false, true] {
            if other_actor {
                let mut a = m.actors[0].clone();
                a.name = "B".into();
                m.actors.push(a);
                m.capabilities[1].actor = ActorId(1);
            }
            let mut w = resource_world(&m);
            if let CaseSlot::Here(c) = &mut w.cases[1] {
                c.frame.tasks[0].call.capability = CapabilityId(1);
            }
            let t = tree(&m, Limits::default(), vec![0, 0]);
            assert!(t.eligible(&w, 0, 0).unwrap().is_empty());
            assert_eq!(!t.eligible(&w, 1, 0).unwrap().is_empty(), other_actor);
        }
    }
    #[test]
    fn resource_h03_h04_h06_reference_world_actual_cancel_seams_and_h05_atomic_candidates() {
        for name in ["rsrc-cooperative", "rsrc-immediate", "rsrc-queued-cancel"] {
            let mut m = super::super::resource_tests::model(name);
            m.arrivals = Some(vec![0]);
            let mut t = tree(&m, Limits::default(), vec![0]);
            t.resource_worlds = Some(Vec::new());
            t.walk(World {
                external_peak: 0,
                now: 0,
                next: 0,
                boundary: Boundary::Coins,
                shared: Vec::new(),
                steps: 0,
                cases: vec![CaseSlot::NotHere],
            })
            .unwrap();
            let (mut request, mut ack, mut cancelled) = (false, false, false);
            for w in t.resource_worlds.as_ref().unwrap() {
                let CaseSlot::Here(c) = &w.cases[0] else {
                    continue;
                };
                let units = reference_resource_units(c.frame.tasks.iter(), &m).unwrap();
                if name == "rsrc-queued-cancel" {
                    assert_eq!(units, [0]);
                    if c.frame
                        .tasks
                        .get(1)
                        .is_some_and(|b| b.phase == Phase::Cancelled)
                    {
                        cancelled = true;
                    }
                } else {
                    if w.now == 2
                        && c.frame
                            .tasks
                            .get(1)
                            .is_some_and(|b| b.phase == Phase::Running && b.ack.is_some())
                    {
                        assert_eq!(units, [1]);
                        request = true;
                    }
                    let at = if name == "rsrc-immediate" { 2 } else { 3 };
                    if w.now == at
                        && c.frame
                            .tasks
                            .get(1)
                            .is_some_and(|b| b.phase == Phase::Cancelled)
                        && c.frame.tasks.iter().all(|b| b.phase != Phase::Running)
                    {
                        assert_eq!(units, [0]);
                        ack = true;
                        let mut probe = w.clone();
                        let CaseSlot::Here(c) = &mut probe.cases[0] else {
                            unreachable!()
                        };
                        let mut p = c.frame.tasks[0].clone();
                        p.phase = Phase::Queued;
                        p.call.capability = CapabilityId(2);
                        p.call.binding = BindingId(2);
                        p.start = None;
                        p.stop = None;
                        let j = c.frame.tasks.len();
                        c.frame.tasks.push(p);
                        c.queue.push(QueueEntry { task: j, at });
                        assert!(!t.eligible(&probe, 0, j).unwrap().is_empty());
                    }
                }
            }
            if name == "rsrc-queued-cancel" {
                assert!(cancelled);
            } else {
                assert!(request && ack);
            }
        }
        let mut m = super::super::resource_tests::model("rsrc-atomic");
        m.actors[0].capacity = Some(3);
        m.capabilities.push(m.capabilities[0].clone());
        m.capabilities[2].uses.retain(|u| u.resource.0 == 0);
        m.capabilities[1].uses.retain(|u| u.resource.0 == 1);
        for reverse in [false, true] {
            if reverse {
                m.capabilities[0].uses.reverse();
            }
            let mut f = replay::initial(&m);
            f.tasks = super::super::resource_tests::tasks(&m);
            let mut p = f.tasks[0].clone();
            p.call.capability = CapabilityId(2);
            p.call.binding = BindingId(2);
            f.tasks.push(p);
            f.tasks[1].phase = Phase::Running;
            let w = World {
                external_peak: 0,
                now: 0,
                next: 1,
                boundary: Boundary::Run,
                shared: Vec::new(),
                steps: 0,
                cases: vec![CaseSlot::Here(CaseFrame {
                    frame: f,
                    queue: (0..3).map(|task| QueueEntry { task, at: 0 }).collect(),
                    receipt: true,
                    end: None,
                })],
            };
            let t = tree(&m, Limits::default(), vec![0]);
            assert!(t.eligible(&w, 0, 0).unwrap().is_empty());
            assert!(!t.eligible(&w, 0, 2).unwrap().is_empty());
            let CaseSlot::Here(c) = &w.cases[0] else {
                unreachable!()
            };
            assert_eq!(
                reference_resource_units(c.frame.tasks.iter(), &m).unwrap(),
                [0, 1]
            );
        }
    }
    #[test]
    fn resource_p11_full_ordered_terminal_interval_paths_are_independent_not_just_extrema() {
        for same_actor in [false, true] {
            let name = if same_actor {
                "rsrc-competition.broken-same-actor"
            } else {
                "rsrc-competition"
            };
            let m = super::super::resource_tests::model(name);
            let mut t = tree(&m, Limits::default(), vec![0, 0]);
            t.resource_worlds = Some(Vec::new());
            t.walk(World {
                external_peak: 0,
                now: 0,
                next: 0,
                boundary: Boundary::Coins,
                shared: Vec::new(),
                steps: 0,
                cases: vec![CaseSlot::NotHere; 2],
            })
            .unwrap();
            let mut paths = BTreeSet::new();
            for w in t.resource_worlds.as_ref().unwrap() {
                if w.cases
                    .iter()
                    .all(|c| matches!(c,CaseSlot::Here(c) if c.end.is_some()))
                {
                    let mut intervals = Vec::new();
                    for (k, c) in w.cases.iter().enumerate() {
                        let CaseSlot::Here(c) = c else { unreachable!() };
                        for task in &c.frame.tasks {
                            intervals.push((
                                task.start.unwrap(),
                                task.stop.unwrap(),
                                k + 1,
                                m.site_name(task.call.binding),
                            ));
                        }
                    }
                    intervals.sort();
                    paths.insert(intervals);
                }
            }
            let first = vec![
                (0, 2, 1, "x".into()),
                (2, 5, 1, "y".into()),
                (5, 7, 2, "x".into()),
                (7, 10, 2, "y".into()),
            ];
            let second = vec![
                (0, 2, 1, "x".into()),
                (2, 4, 2, "x".into()),
                (4, 7, 1, "y".into()),
                (7, 10, 2, "y".into()),
            ];
            assert_eq!(
                paths,
                if same_actor {
                    BTreeSet::from([second])
                } else {
                    BTreeSet::from([first, second])
                }
            );
        }
    }
    #[test]
    fn integer_initial_case_product_has_four_tuples_and_budgeted_support() {
        let mut m = crate::integer_tests::model(&crate::integer_tests::latent(1));
        m.bounds.cases = 2;
        m.arrivals = Some(vec![0, 0]);
        m.start_policy = Some(StartPolicy::default());
        let mut t = tree(&m, Limits::default(), vec![0, 0]);
        t.walk(World {
            external_peak: 0,
            now: 0,
            next: 0,
            boundary: Boundary::Coins,
            steps: 0,
            cases: vec![CaseSlot::NotHere; 2],
            shared: vec![StateValue::None],
        })
        .unwrap();
        let tuples: std::collections::HashSet<_> = t
            .initial_leaves
            .iter()
            .filter(|(_, vs, _)| {
                vs.len() == 2 && vs.iter().all(|v| matches!(v[0], StateValue::Integer(_)))
            })
            .map(|(_, vs, _)| vs.iter().map(|v| v[0]).collect::<Vec<_>>())
            .collect();
        assert_eq!(tuples.len(), 4);
        for remaining in [0, 1, 3, 4, 5] {
            let support = domain_support(&m, &m.state_variables[0], remaining);
            assert_eq!(support.is_ok(), remaining >= 2);
        }
        for frames in [0, 1, 3, 4, 5] {
            let r = explore(
                &m,
                Limits {
                    frames,
                    ..Limits::default()
                },
            );
            assert!(r.statistics.frames <= frames);
        }
    }
    #[test]
    fn d1_review_numeric_cut_reason_arrivals() {
        let m = model(include_str!(
            "../../../../fixtures/ctx-obligation/model.relca"
        ));
        let mut tree = tree(&m, Limits::default(), vec![0]);
        let mut frame = replay::initial(&m);
        tree.cases[0]
            .record(&mut frame, Occurrence::Receipt, None)
            .unwrap();
        let world = World {
            external_peak: 0,
            now: 0,
            next: 1,
            boundary: Boundary::Run,
            shared: Vec::new(),
            steps: 0,
            cases: vec![CaseSlot::Here(CaseFrame {
                frame,
                queue: Vec::new(),
                receipt: true,
                end: None,
            })],
        };
        tree.leaf(&world, ReferenceCut::Numeric).unwrap();
        let result = &tree.cases[0].statistics.obligations[0].outcomes[0];
        assert_eq!(result.status, crate::obligation::Status::Unresolved);
        assert_eq!(result.reason.as_deref(), Some("state_budget"));
        assert_eq!(result.deadline, 4);
    }
    fn model(s: &str) -> Model {
        crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap()
    }
    fn query(out: &Outcome, id: usize, case: usize) -> &ProbabilityRange {
        out.properties
            .iter()
            .find(|p| p.property == Property::ProbabilityReached(id))
            .unwrap()
            .cases
            .as_ref()
            .unwrap()[case]
            .probability
            .as_ref()
            .unwrap()
    }
    #[test]
    fn mutation_f1_fifo_case_probabilities() {
        let r = explore(
            &model(include_str!(
                "../../../../fixtures/arrivals-simultaneous/model.relca"
            )),
            Limits::default(),
        );
        assert_eq!(r.stop, StopReason::Complete);
        assert_eq!(query(&r.outcome, 0, 0).min.to_string(), "1/2");
        assert_eq!(query(&r.outcome, 0, 1).max.to_string(), "0");
    }
    #[test]
    fn mutation_f4_same_tick_admission() {
        let r = explore(
            &model(include_str!(
                "../../../../fixtures/arrivals-same-time-slot/model.relca"
            )),
            Limits::default(),
        );
        assert_eq!(r.stop, StopReason::Complete);
        assert_eq!(r.outcome.admission.unwrap().rejected_count, singleton(1));
        assert_eq!(
            r.outcome.case_outcomes.unwrap()[2].terminals[0].kind,
            Kind::Rejected
        );
    }
    #[test]
    fn mutation_f3_rejection_is_unconditional_mass() {
        let r = explore(
            &model(include_str!(
                "../../../../fixtures/arrivals-reject-mass/model.relca"
            )),
            Limits::default(),
        );
        assert_eq!(r.stop, StopReason::Complete);
        assert_eq!(query(&r.outcome, 0, 2).min.to_string(), "1/2");
        assert_eq!(query(&r.outcome, 1, 2).min.to_string(), "3/8");
    }
    #[test]
    fn mutation_f5_independent_initial_values() {
        let r = explore(
            &model(include_str!(
                "../../../../fixtures/arrivals-initial-product/model.relca"
            )),
            Limits::default(),
        );
        assert_eq!(r.stop, StopReason::Complete);
        assert_eq!(query(&r.outcome, 0, 1).min.to_string(), "1/4");
    }
    fn tree(m: &Model, limits: Limits, times: Vec<Tick>) -> Tree<'_> {
        let n = times.len();
        Tree {
            response_exercise: BTreeMap::new(),
            response_receipts: Vec::new(),
            first_hits: Vec::new(),
            first_closed: Vec::new(),
            first_trees: Vec::new(),
            overflows: None,
            external_peaks: None,
            admissions: vec![None; n],
            unadmitted: vec![false; n],
            resource_worlds: None,
            depth: 0,
            m,
            limits,
            times,
            cases: (0..n).map(|_| interpreter(m, limits)).collect(),
            terminals: vec![BTreeMap::new(); n],
            unresolved: vec![false; n],
            stats: Statistics::default(),
            truncated: false,
            numeric: false,
            deadlock: false,
            overcommit: false,
            chance: false,
            scheduler: false,
            policy_unresolved: false,
            rejects: None,
            initial_leaves: Vec::new(),
            receipts: Vec::new(),
            duration_leaves: Vec::new(),
        }
    }
    fn frame(m: &Model, receipt: bool) -> CaseFrame<'_> {
        CaseFrame {
            frame: Frame {
                condition_waits: Vec::new(),
                lost_updates: BTreeSet::new(),
                state_snapshots: Vec::new(),
                business: None,
                state_values: m
                    .state_variables
                    .iter()
                    .map(|v| match v.initial {
                        InitialValue::Fixed(x) => x,
                        _ => StateValue::None,
                    })
                    .collect(),
                trace: Vec::new(),
                now: 0,
                steps: 0,
                control: vec![&m.process.body],
                waiting: None,
                tasks: Vec::new(),
                adoption: None,
            },
            queue: Vec::new(),
            receipt,
            end: None,
        }
    }
    fn world(m: &Model, n: usize) -> World<'_> {
        World {
            external_peak: 0,
            now: 0,
            next: n,
            boundary: Boundary::Run,
            shared: vec![StateValue::None; m.state_variables.len()],
            cases: (0..n).map(|_| CaseSlot::Here(frame(m, true))).collect(),
            steps: 0,
        }
    }
    #[test]
    fn state_invariant_independent_snapshots_retain_passive_witness_after_writer_clause_abort() {
        let mut m = model(include_str!(
            "../../../../fixtures/sinv-ended-clean.broken-increment/model.relca"
        ));
        let cap = CapabilityId(
            m.capabilities
                .iter()
                .position(|c| c.name == "post")
                .unwrap(),
        );
        let event = Event {
            capability: cap,
            kind: EventKind::Finish(None),
        };
        m.contracts.push(ContractClause {
            name: Some("Overflow".into()),
            ordinal: None,
            span: m.process.span,
            kind: ContractKind::Response {
                trigger: Trigger::Event(event),
                targets: vec![Target::End(EndId(0))],
                within: u64::MAX,
            },
        });
        m.verify.push(Verify {
            property: Property::Contract(ClauseId(1)),
            span: m.process.span,
        });
        let approval = m
            .state_variables
            .iter()
            .position(|v| v.name == "approval")
            .unwrap();
        let posted = m
            .state_variables
            .iter()
            .position(|v| v.name == "posted")
            .unwrap();
        let ty = &m.types[m.state_variables[approval].base.0];
        let pass = ValueId(ty.members.iter().position(|v| v == "pass").unwrap());
        let fail = ValueId(ty.members.iter().position(|v| v == "fail").unwrap());
        let mut w = world(&m, 2);
        w.now = 2;
        w.shared[posted] = StateValue::Integer(0);
        for (k, slot) in w.cases.iter_mut().enumerate() {
            let CaseSlot::Here(c) = slot else {
                unreachable!()
            };
            c.frame.now = 2;
            c.frame.state_values[approval] = StateValue::Value(if k == 0 { pass } else { fail });
            c.frame.trace.push(Observation {
                binding: None,
                person: None,
                at: 0,
                event: Occurrence::Receipt,
                values: vec![None; m.process.bindings.len()],
                state_values: c.frame.state_values.clone(),
            });
            if k == 1 {
                c.end = Some(EndId(m.ends.iter().position(|e| e == "returned").unwrap()));
            }
        }
        let mut t = tree(&m, Limits::default(), vec![0, 0]);
        t.state_snapshots(&mut w).unwrap();
        w.now = 3;
        w.shared[posted] = StateValue::Integer(1);
        if let CaseSlot::Here(c) = &mut w.cases[0] {
            c.frame.now = 3;
        }
        assert_eq!(
            t.record_context(
                &mut w,
                0,
                Occurrence::Completion(cap, pass),
                None,
                (false, None)
            ),
            Err(StopReason::SemanticAbort)
        );
        assert!(t.cases[1].contract_facts[0].1);
        let CaseSlot::Here(reader) = &w.cases[1] else {
            unreachable!()
        };
        assert_eq!(reader.frame.now, 2);
        assert_eq!(
            reader.frame.state_snapshots[0].1[posted],
            StateValue::Integer(0)
        );
        assert_eq!(
            reader.frame.state_snapshots.last().unwrap().1[posted],
            StateValue::Integer(1)
        );
    }
    fn set_task<'a>(w: &mut World<'a>, k: usize, m: &'a Model, queued: Tick, running: bool) {
        let CaseSlot::Here(c) = &mut w.cases[k] else {
            unreachable!()
        };
        let call = match &m.process.body[0].kind {
            Stmt::Call(c) => c.clone(),
            _ => panic!(),
        };
        c.frame.control = vec![&m.process.body[1..]];
        c.frame.tasks = vec![Task {
            frozen_performance: None,
            stale: BTreeSet::new(),
            metric_ready: None,
            checked_through: None,
            person: None,
            call,
            phase: if running {
                Phase::Running
            } else {
                Phase::Queued
            },
            start: running.then_some(0),
            stop: None,
            finish: running.then_some(5),
            ack: None,
        }];
        c.frame.waiting = Some(Waiting {
            tasks: 0..1,
            join: None,
            winner: None,
        });
        c.queue = vec![QueueEntry {
            task: 0,
            at: queued,
        }];
    }
    #[test]
    fn a_enter_time_uses_queue_trace_not_arrival_and_finishes_6_7() {
        let mut m = model(include_str!(
            "../../../../fixtures/arrivals-count/model.relca"
        ));
        m.capabilities[0].duration = 1;
        let mut w = world(&m, 3);
        w.now = 4;
        set_task(&mut w, 0, &m, 4, false);
        set_task(&mut w, 1, &m, 3, false);
        set_task(&mut w, 2, &m, 0, true);
        for slot in &mut w.cases {
            if let CaseSlot::Here(c) = slot {
                c.frame.now = 4;
            }
        }
        let mut t = tree(&m, Limits::default(), vec![0, 1, 2]);
        assert!(t.eligible(&w, 0, 0).unwrap().is_empty());
        t.walk(w).unwrap();
        assert_eq!(
            t.terminals[0][&Kind::End(EndId(0))].completed_at,
            Some(singleton(7))
        );
        assert_eq!(
            t.terminals[1][&Kind::End(EndId(0))].completed_at,
            Some(singleton(6))
        );
    }
    #[test]
    fn a_room_init_full_product_then_choice_receipt_and_no_future_draw() {
        let m = model(
            r#"model M version "0.2" {latent case {x:bool ~ {true:0.5,false:0.5};y:bool}
            process P{end done} arrivals fixed[0min,0min] bounds{cases 2;live_tasks 1;clock 5min} verify probability reached x == true <= 1}"#,
        );
        let mut w = world(&m, 2);
        for slot in &mut w.cases {
            if let CaseSlot::Here(c) = slot {
                c.receipt = false;
            }
        }
        let mut denied = tree(
            &m,
            Limits {
                frames: 3,
                ..Limits::default()
            },
            vec![0, 0],
        );
        assert!(matches!(
            denied.initials(&w, true),
            Err(StopReason::EnumerationLimit)
        ));
        assert!(denied.initial_leaves.is_empty());
        assert!(denied.receipts.is_empty());
        assert_eq!(denied.stats.frames, 0);
        let mut t = tree(&m, Limits::default(), vec![0, 0]);
        let _ = t.initials(&w, true).unwrap();
        assert_eq!(t.initial_leaves.len(), 4);
        assert!(
            t.initial_leaves
                .iter()
                .all(|(_, _, p)| p.to_string() == "1/4")
        );
        assert_eq!(t.receipts.len(), 32);
        for (_, at, values) in &t.receipts {
            assert_eq!(*at, 0);
            assert!(values.iter().all(|x| !matches!(x, StateValue::None)));
        }
        let tuples: BTreeSet<_> = t
            .initial_leaves
            .iter()
            .map(|(_, values, _)| format!("{values:?}"))
            .collect();
        assert_eq!(tuples.len(), 4);
        let mut t = tree(&m, Limits::default(), vec![0, 8]);
        let mut w = world(&m, 2);
        w.cases = vec![CaseSlot::Declined, CaseSlot::NotHere];
        w.next = 1;
        let _ = t.initials(&w, true).unwrap();
        assert!(t.receipts.is_empty());
        assert!(t.initial_leaves.iter().all(|(_, vs, _)| vs.is_empty()));
    }
    #[test]
    fn percase_reference_joint_support_room_and_case_product() {
        for (src, weights) in [
            (
                include_str!("../../../../fixtures/pcase-joint/model.relca"),
                ["16/25", "4/25", "4/25", "1/25"],
            ),
            (
                include_str!("../../../../fixtures/pcase-joint.broken-half/model.relca"),
                ["1/4"; 4],
            ),
        ] {
            let m = model(src);
            let mut w = world(&m, 2);
            for slot in &mut w.cases {
                if let CaseSlot::Here(c) = slot {
                    c.receipt = false;
                }
            }
            let mut denied = tree(
                &m,
                Limits {
                    frames: 3,
                    ..Limits::default()
                },
                vec![0, 0],
            );
            assert!(matches!(
                denied.initials(&w, true),
                Err(StopReason::EnumerationLimit)
            ));
            assert_eq!(denied.stats.frames, 0);
            assert!(denied.initial_leaves.is_empty());
            let mut t = tree(&m, Limits::default(), vec![0, 0]);
            t.initials(&w, true).unwrap();
            assert_eq!(t.initial_leaves.len(), 4);
            let mut support = BTreeMap::new();
            let mut sum = Rational::zero();
            for (at, vs, p) in &t.initial_leaves {
                assert_eq!(*at, 0);
                support.insert(format!("{:?},{:?}", vs[0][0], vs[1][0]), p.to_string());
                sum.0 += &p.0;
            }
            assert_eq!(sum, Rational::one());
            for (a, b, p) in [
                (0, 0, weights[0]),
                (1, 0, weights[1]),
                (0, 1, weights[2]),
                (1, 1, weights[3]),
            ] {
                assert_eq!(
                    support[&format!("Value(ValueId({a})),Value(ValueId({b}))")],
                    p
                );
            }
        }
        let m = model(
            &include_str!("../../../../fixtures/pcase-case-product/model.relca")
                .replace("x: bool ~", "y: bool; x: bool ~"),
        );
        let y = m
            .state_variables
            .iter()
            .position(|v| v.name == "y")
            .unwrap();
        let mut w = world(&m, 2);
        for slot in &mut w.cases {
            if let CaseSlot::Here(c) = slot {
                c.receipt = false;
            }
        }
        let mut t = tree(&m, Limits::default(), vec![0, 0]);
        t.initials(&w, true).unwrap();
        assert_eq!(t.initial_leaves.len(), 16);
        assert_eq!(
            t.initial_leaves
                .iter()
                .map(|(_, v, _)| format!("{v:?}"))
                .collect::<BTreeSet<_>>()
                .len(),
            16
        );
        let mut sum = Rational::zero();
        for (_, vs, p) in &t.initial_leaves {
            assert!(
                vs.iter().all(|v| v[y] == StateValue::None),
                "no nondeterminism inside chance layer"
            );
            sum.0 += &p.0;
        }
        assert_eq!(sum, Rational::one());
        assert_eq!(t.receipts.len(), 16 * 4 * 2);
        assert!(
            t.receipts
                .iter()
                .all(|(_, _, vs)| vs.iter().all(|v| *v != StateValue::None))
        );
        let m = model(include_str!(
            "../../../../fixtures/pcase-nondet/model.relca"
        ));
        let mut w = world(&m, 2);
        for slot in &mut w.cases {
            if let CaseSlot::Here(c) = slot {
                c.receipt = false;
            }
        }
        let mut t = tree(&m, Limits::default(), vec![0, 0]);
        t.initials(&w, false).unwrap();
        assert_eq!(t.receipts.len(), 8);
        let tuples: BTreeSet<_> = t
            .receipts
            .as_chunks::<2>()
            .0
            .iter()
            .map(|vs| format!("{:?},{:?}", vs[0].2, vs[1].2))
            .collect();
        assert_eq!(tuples.len(), 4);
    }
    #[test]
    fn percase_reference_zero_mass_and_later_receipt_preserve_payload() {
        let m = model(
            &include_str!("../../../../fixtures/pcase-joint/model.relca")
                .replace("routine:0.8, difficult:0.2", "routine:0, difficult:1"),
        );
        let mut w = world(&m, 2);
        for s in &mut w.cases {
            if let CaseSlot::Here(c) = s {
                c.receipt = false;
            }
        }
        let mut t = tree(&m, Limits::default(), vec![0, 0]);
        t.initials(&w, true).unwrap();
        assert_eq!(t.initial_leaves.len(), 1);
        assert_eq!(t.initial_leaves[0].2, Rational::one());
        let m = model(include_str!("../../../../fixtures/pcase-late/model.relca"));
        let mut w = world(&m, 2);
        w.now = 3;
        for s in &mut w.cases {
            if let CaseSlot::Here(c) = s {
                c.frame.now = 3;
            }
        }
        if let CaseSlot::Here(c) = &mut w.cases[0] {
            c.frame.state_values[0] = StateValue::Value(ValueId(1));
        }
        if let CaseSlot::Here(c) = &mut w.cases[1] {
            c.receipt = false;
        }
        let mut t = tree(&m, Limits::default(), vec![0, 3]);
        t.initials(&w, true).unwrap();
        assert_eq!(t.initial_leaves.len(), 2);
        assert!(
            t.initial_leaves
                .iter()
                .all(|(at, values, _)| *at == 3 && values.len() == 1)
        );
        assert!(t.receipts.iter().all(|(k, at, _)| *k == 1 && *at == 3));
        assert!(t.values(&w, 0)[0] == StateValue::Value(ValueId(1)));
    }
    #[test]
    fn a_duration_cross_case_joint_before_normal_and_ack_no_redraw() {
        let m = model(
            r#"model M version "0.2" {type T=enum{ok} observed case{hit:bool=false}
        kernel K{* -> {0.5:ok with hit=true;0.5:ok with hit=false}}
        actor A{capacity 2;capability run()->T{duration{1min:0.5,3min:0.5};writes hit;outcomes kernel K}}
        process P{r=A.run();end done} arrivals fixed[0min,0min] bounds{cases 2;live_tasks 1;clock 10min} verify probability reached hit == true <= 1}"#,
        );
        let mut w = world(&m, 2);
        w.now = 1;
        w.boundary = Boundary::Coins;
        for k in 0..2 {
            set_task(&mut w, k, &m, 0, true);
            if let CaseSlot::Here(c) = &mut w.cases[k] {
                c.frame.now = 1;
                c.frame.tasks[0].finish = None;
                c.frame.tasks[0].ack = Some(1);
            }
        }
        let mut t = tree(&m, Limits::default(), vec![0, 0]);
        let coins = t.coins(&w, 4096).unwrap().unwrap();
        assert_eq!(coins.len(), 2);
        assert!(
            t.coins(&w, 1).unwrap().is_none(),
            "numeric failure is not an enumeration abort"
        );
        let mut denied = tree(
            &m,
            Limits {
                frames: 3,
                ..Limits::default()
            },
            vec![0, 0],
        );
        assert!(matches!(
            denied.coin_tree(&w, &coins),
            Err(StopReason::EnumerationLimit)
        ));
        assert!(denied.duration_leaves.is_empty());
        t.coin_tree(&w, &coins).unwrap();
        assert_eq!(t.duration_leaves.len(), 4);
        assert!(
            t.duration_leaves
                .iter()
                .all(|(p, _)| p.to_string() == "1/4")
        );
        assert!(t.stats.normal_completions > 0);
        assert!(t.stats.cancellation_completions > 0);
        for slot in &mut w.cases {
            if let CaseSlot::Here(c) = slot {
                c.frame.tasks[0].checked_through = Some(1);
            }
        }
        assert!(t.coins(&w, 4096).unwrap().unwrap().is_empty());
        assert!(
            w.cases
                .iter()
                .all(|s| matches!(s,CaseSlot::Here(c)if c.frame.tasks[0].ack==Some(1)))
        );
    }
    fn contract_model() -> Model {
        model(
            r#"model M version "0.2" {type T=enum {ok} people[p,q]
        actor A{executors[p,q];capability run()->T{duration fixed 1min;result nondet{ok}}}
        actor B{executors[p,q];capability run()->T{duration fixed 1min;result nondet{ok}}}
        process P{a=A.run();b=B.run();end done} arrivals fixed[0min,0min] bounds{cases 2;live_tasks 1;clock 5min}
        contract C{separation A.run.finish,B.run.finish;requires_event B.run.start after A.run.finish}}"#,
        )
    }
    fn observation(event: Occurrence, person: usize) -> Observation {
        Observation {
            binding: None,
            state_values: Vec::new(),
            person: Some(PersonId(person)),
            at: 0,
            event,
            values: vec![None, None],
        }
    }
    #[test]
    fn a_separation_and_requires_are_local_trace_suffixes() {
        let m = contract_model();
        let mut t = tree(&m, Limits::default(), vec![0, 0]);
        for (k, p, q) in [(0, 0, 1), (1, 1, 0)] {
            let mut f = frame(&m, true).frame;
            f.trace = vec![
                observation(Occurrence::Completion(CapabilityId(0), ValueId(0)), p),
                observation(Occurrence::Completion(CapabilityId(1), ValueId(0)), q),
            ];
            t.cases[k].inspect_trace(&f, 0, false, true, false).unwrap();
            assert_eq!(t.cases[k].contract_facts[0], (true, false));
            if k == 1 {
                f.trace[1].person = Some(PersonId(1));
                t.cases[k].inspect_trace(&f, 0, false, true, false).unwrap();
                assert_eq!(t.cases[k].contract_facts[0], (true, true));
            }
        }
        assert!(!t.cases[0].contract_facts[0].1);
        let mut t = tree(&m, Limits::default(), vec![0, 0]);
        let mut f = frame(&m, true).frame;
        f.trace = vec![observation(
            Occurrence::Completion(CapabilityId(0), ValueId(0)),
            0,
        )];
        t.cases[0].inspect_trace(&f, 0, false, true, false).unwrap();
        f.trace = vec![observation(Occurrence::Dispatch(CapabilityId(1)), 1)];
        t.cases[1].inspect_trace(&f, 0, false, true, false).unwrap();
        assert_eq!(t.cases[0].contract_facts[1], (false, false));
        assert_eq!(t.cases[1].contract_facts[1], (true, true));
    }
    #[test]
    fn a_cut_reasons_absorbing_facts_and_abort_withdrawal() {
        let mut m = model(include_str!(
            "../../../../fixtures/arrivals-relative-within/model.relca"
        ));
        m.probability_queries[0].within = Some(0);
        m.bounds.clock = 5;
        let mut w = world(&m, 2);
        w.now = 5;
        let mut t = tree(&m, Limits::default(), vec![5, 6]);
        let live = t.leaf(&w, ReferenceCut::LiveTasks).unwrap();
        assert_eq!(live[0].endpoints[3], Rational::one());
        let clock = t.leaf(&w, ReferenceCut::Clock).unwrap();
        assert_eq!(clock[0].endpoints[3], Rational::zero());
        let numeric = t.leaf(&w, ReferenceCut::Numeric).unwrap();
        assert!(numeric[0].numeric);
        assert_eq!(numeric[0].endpoints[3], Rational::one());
        let m = model(
            r#"model M version "0.2" {type T=enum {ok} actor A{capacity 1;capability run()->T{duration fixed 1min;result nondet{ok}}}
            process P{a=A.run();end done} arrivals fixed[0min,2min] bounds{cases 2;live_tasks 1;clock 5min}
            contract C{response case.received -> one_of(end.done) by 18446744073709551615min after case.received otherwise unresolved} verify reachable end.done}"#,
        );
        let r = explore(&m, Limits::default());
        assert_eq!(r.stop, StopReason::SemanticAbort);
        assert!(
            r.outcome
                .case_outcomes
                .unwrap()
                .iter()
                .all(|c| c.terminals.is_empty())
        );
        assert_eq!(
            r.outcome.properties.last().unwrap().cases.as_ref().unwrap()[0].verdict,
            Verdict::Satisfied
        );
    }

    #[test]
    fn a_numeric_limit_zero_support_and_enumeration_defenses() {
        // The 20 nested flips recurse deeper than the 2MiB test-thread stack
        // in a debug build once ADR0035/0037 and ADR0039 share walk_inner
        // (it passed on each branch alone); run on a larger stack like the
        // other deep reference walks (actor_table_cut_debtor*.rs).
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(numeric_limit_zero_support_and_enumeration_defenses)
            .unwrap()
            .join()
            .unwrap();
    }

    fn numeric_limit_zero_support_and_enumeration_defenses() {
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
        let r = explore(&model(&source), Limits::default());
        assert_eq!(r.stop, StopReason::Complete);
        assert!(r.outcome.truncated);
        assert!(
            r.outcome
                .properties
                .iter()
                .all(|p| p.verdict == Verdict::Inconclusive)
        );
        let mut m = model(include_str!("../../../../fixtures/prob-zero/model.relca"));
        m.arrivals = Some(vec![0]);
        let r = explore(&m, Limits::default());
        assert_eq!(r.stop, StopReason::Complete);
        assert_eq!(r.statistics.normal_completions, 1);
        let r = explore(
            &m,
            Limits {
                branch_depth: 0,
                ..Limits::default()
            },
        );
        assert_eq!(r.stop, StopReason::EnumerationLimit);
        let r = explore(
            &m,
            Limits {
                steps_per_path: 0,
                ..Limits::default()
            },
        );
        assert_eq!(r.stop, StopReason::EnumerationLimit);
    }
}
