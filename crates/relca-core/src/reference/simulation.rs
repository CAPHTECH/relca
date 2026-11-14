//! Independent sampled trace interpreter. No engine module, state, draw,
//! normalizer, query, monitor or statistical helper is used here.
use super::*;
#[cfg(test)]
#[path = "simulation_limits_tests.rs"]
mod explicit_limits_tests;
use crate::outcome::{
    QueryTruth, ReplicationOutcome, SimulationCase, SimulationCut, SimulationDraw,
    SimulationOutcome, SimulationTerminal, SimulationTerminalKind,
};
use num_bigint::BigInt;
use sha2::{Digest, Sha256};

struct Random {
    priority_tie: bool,
    seed: u64,
    replication: u64,
    audit: Vec<SimulationDraw>,
    origins: Vec<(usize, String, diag::Span)>,
}
impl Random {
    fn origin(&mut self, m: &Model, case: usize, binding: BindingId, join: bool) {
        let mut pending = vec![m.process.body.as_slice()];
        let mut span = m.process.span;
        while let Some(block) = pending.pop() {
            for statement in block {
                match &statement.kind {
                    Stmt::Call(call) | Stmt::Response(call, _) if call.binding == binding => {
                        span = call.span
                    }
                    Stmt::Fork { branches, .. } => {
                        if let Some(call) = branches.iter().find(|c| c.binding == binding) {
                            span = if join { statement.span } else { call.span };
                        }
                    }
                    Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        pending.push(then_body);
                        pending.push(else_body);
                    }
                    _ => {}
                }
            }
        }
        self.origins.push((
            case,
            format!("{}.{}", m.process.name, m.site_name(binding)),
            m.site_contest(binding)
                .map_or(span, |id| m.contests()[id].span),
        ));
    }
    fn pick(
        &mut self,
        case: usize,
        site: String,
        occurrence: usize,
        probabilities: &[Rational],
    ) -> Result<usize, StopReason> {
        let key = [
            "relca-sim-v1".to_owned(),
            format!("seed={}", self.seed),
            format!("rep={}", self.replication),
            format!("case={case}"),
            format!("site={site}"),
            format!("n={occurrence}"),
        ]
        .join("|");
        let bytes = Sha256::digest(key.as_bytes());
        let integer = bytes
            .iter()
            .take(8)
            .fold(BigInt::from(0), |n, b| n * 256 + *b);
        let chosen = select_integer(&integer, probabilities)?;
        self.audit.push(SimulationDraw { key });
        Ok(chosen)
    }
}

fn select_integer(integer: &BigInt, probabilities: &[Rational]) -> Result<usize, StopReason> {
    let mut total = Rational::zero();
    let mut chosen = None;
    for (i, p) in probabilities.iter().enumerate() {
        require(p.valid())?;
        total.0 += &p.0;
        if chosen.is_none() && integer * total.0.denom() < total.0.numer() * (BigInt::from(1) << 64)
        {
            chosen = Some(i);
        }
    }
    require(total == Rational::one())?;
    chosen.ok_or(StopReason::SemanticAbort)
}

// Trace-derived future-relevant contract history, not a copy of the engine's
// incremental bundle monitor. Each clause is reduced from its own observations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct History {
    active: bool,
    broken: bool,
    once: u8,
    prerequisite: bool,
    left: BTreeSet<PersonId>,
    right: BTreeSet<PersonId>,
    obligations: Vec<Tick>,
}
fn histories(m: &Model, f: &Frame<'_>, case_index: usize) -> Result<Vec<History>, StopReason> {
    m.contracts.iter().enumerate().map(|(clause_index, clause)| {
        let mut h = History::default();
        for o in &f.trace {
            match &clause.kind {
                ContractKind::Excludes { before, start } => {
                    if o.matches_event(*start, m) { h.broken |= h.prerequisite; }
                    if o.matches_event(*before, m) { h.active = true; h.prerequisite = true; }
                }
                ContractKind::Waive { event, .. } => { h.active |= o.matches_event(*event, m); }
                ContractKind::Invariant { event, condition } if o.matches_event(*event, m) => {
                    h.active = true;
                    h.broken |= !snapshot_condition_at(condition.as_ref().ok_or(StopReason::SemanticAbort)?, &o.values, &o.state_values, case_index)?;
                }
                ContractKind::AtMostOnce { event } if o.matches_event(*event, m) => {
                    h.active = true; h.once = (h.once + 1).min(2); h.broken |= h.once > 1;
                }
                ContractKind::RequiresEvent { before, start } => {
                    if o.matches_event(*start, m) { h.active = true; h.broken |= !h.prerequisite; }
                    h.prerequisite |= o.matches_event(*before, m);
                }
                ContractKind::Response { trigger, targets, within } | ContractKind::NamedResponse { trigger, targets, within, .. } => {
                    h.broken |= h.obligations.iter().any(|d| *d < o.at);
                    if o.matches_trigger(*trigger, m) {
                        h.active = true;
                        h.obligations.push(o.at.checked_add(*within).ok_or(StopReason::SemanticAbort)?);
                    }
                    if targets.iter().any(|t| o.matches_target(*t, m)) { h.obligations.retain(|d| *d < o.at); }
                    if m.contracts.iter().any(|w| matches!(&w.kind, ContractKind::Waive { response, event, .. } if response.0 == clause_index && o.matches_event(*event, m))) { h.obligations.retain(|d| *d < o.at); }
                    h.obligations.sort(); h.obligations.dedup();
                }
                ContractKind::Separation { left, right } => {
                    if o.matches_event(*left, m) { h.left.insert(o.person.ok_or(StopReason::SemanticAbort)?); }
                    if o.matches_event(*right, m) { h.right.insert(o.person.ok_or(StopReason::SemanticAbort)?); }
                    h.active = !h.left.is_empty() && !h.right.is_empty();
                    h.broken |= !h.left.is_disjoint(&h.right);
                }
                ContractKind::Permitted { capability } if matches!(o.event, Occurrence::Dispatch(c) if c == *capability) => {
                    let cap = &m.capabilities[capability.0];
                    let allowed = cap.permission.as_ref().or(m.actors[cap.actor.0].executors.as_ref()).ok_or(StopReason::SemanticAbort)?;
                    h.active = true; h.broken |= !allowed.contains(&o.person.ok_or(StopReason::SemanticAbort)?);
                }
                ContractKind::AuthorityBasis { capability, authority } if matches!(o.event, Occurrence::Completion(c, _) if c == *capability) => {
                    let person = o.person.ok_or(StopReason::SemanticAbort)?;
                    h.active = true;
                    h.broken |= !authority::event_holds(m, *authority, *capability, person, o, case_index)?;
                }
                _ => {}
            }
        }
        if let ContractKind::EvidenceBasis { capability, direct, sources, .. } = &clause.kind {
            let (active, broken, _) = evidence::facts(m, &f.trace, *capability, *direct, sources)?;
            h.active = active;
            h.broken = broken;
        }
        if let ContractKind::StateInvariant { antecedent, condition } = &clause.kind {
            for (_, values) in &f.state_snapshots {
                if match antecedent { Some(c) => snapshot_condition_at(c, &[], values, case_index)?, None => true } {
                    h.active = true;
                    h.broken |= !snapshot_condition_at(condition, &[], values, case_index)?;
                }
            }
        }
        h.broken |= h.obligations.iter().any(|d| *d < f.now);
        Ok(h)
    }).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskSignature {
    frozen_performance: Option<FrozenPerformance>,
    stale: BTreeSet<StateVarId>,
    binding: BindingId,
    capability: CapabilityId,
    phase: u8,
    value: Option<ValueId>,
    person: Option<PersonId>,
    start: Option<Tick>,
    finish: Option<Tick>,
    ack: Option<Tick>,
    checked: Option<Tick>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct CaseSignature {
    condition_waits: Vec<(Tick, Option<Tick>)>,
    channel_history: Vec<(BindingId, u64, ValueId, crate::ast::Receipt, bool)>,
    slot: u8,
    admitted_at: Option<Tick>,
    lost_updates: BTreeSet<StateVarId>,
    sleep: Option<Tick>,
    metric_history: Option<Vec<MetricTaskSignature>>,
    receipt: bool,
    end: Option<EndId>,
    values: Vec<StateValue>,
    control: Vec<Vec<usize>>,
    waiting: Option<(Option<Join>, Option<BindingId>)>,
    tasks: Vec<TaskSignature>,
    queued: Vec<Tick>,
    ever_started: bool,
    results: Vec<Option<ValueId>>,
    adoption: Option<Tick>,
    work: Tick,
    histories: Vec<History>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct MetricTaskSignature {
    binding: BindingId,
    ready: Option<Tick>,
    start: Option<Tick>,
    stop: Option<Tick>,
    person: Option<PersonId>,
    status: u8,
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum SlotSignature {
    Future,
    Rejected,
    Case(Box<CaseSignature>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Signature {
    external_peak: u64,
    time: Tick,
    cursor: usize,
    boundary: u8,
    shared: Vec<StateValue>,
    cases: Vec<SlotSignature>,
}
fn signature(m: &Model, w: &World<'_>) -> Result<Signature, StopReason> {
    let mut cases = Vec::new();
    for (case_index, slot) in w.cases.iter().enumerate() {
        cases.push(match slot {
            CaseSlot::NotHere => SlotSignature::Future,
            CaseSlot::Declined => SlotSignature::Rejected,
            CaseSlot::Here(c) | CaseSlot::Held(c) | CaseSlot::Overflow(c) => {
                let f = &c.frame;
                let results = response::visible_values(f, m, None);
                let mut work = 0u64;
                for t in &f.tasks {
                    if let Some(start) = t.start {
                        work = work
                            .checked_add(
                                t.stop
                                    .unwrap_or(w.now)
                                    .checked_sub(start)
                                    .ok_or(StopReason::SemanticAbort)?,
                            )
                            .ok_or(StopReason::SemanticAbort)?;
                    }
                }
                let range = f
                    .waiting
                    .as_ref()
                    .map(|wait| wait.tasks.clone())
                    .unwrap_or(0..0);
                let tasks = range
                    .clone()
                    .map(|j| {
                        let t = &f.tasks[j];
                        TaskSignature {
                            frozen_performance: t.frozen_performance.clone(),
                            stale: t.stale.clone(),
                            binding: t.call.binding,
                            capability: t.call.capability,
                            phase: match t.phase {
                                Phase::Queued => 0,
                                Phase::Running => 1,
                                Phase::Finished(_) => 2,
                                Phase::Cancelled => 3,
                                Phase::Refused(_) => 4,
                            },
                            value: if let Phase::Finished(x) | Phase::Refused(x) = t.phase {
                                Some(x)
                            } else {
                                None
                            },
                            person: t.person,
                            start: (t.phase == Phase::Running
                                && m.capabilities[t.call.capability.0]
                                    .duration_distribution
                                    .is_some())
                            .then_some(t.start)
                            .flatten(),
                            finish: (t.phase == Phase::Running).then_some(t.finish).flatten(),
                            ack: (t.phase == Phase::Running).then_some(t.ack).flatten(),
                            checked: (t.phase == Phase::Running)
                                .then_some(t.checked_through)
                                .flatten(),
                        }
                    })
                    .collect();
                SlotSignature::Case(Box::new(CaseSignature {
                    channel_history: f
                        .trace
                        .iter()
                        .filter_map(|o| match &o.event {
                            Occurrence::Channel(r) => {
                                Some((r.command, r.attempt, r.actual, r.observation, r.resolved))
                            }
                            _ => None,
                        })
                        .collect(),
                    slot: match slot {
                        CaseSlot::Held(_) => 1,
                        CaseSlot::Overflow(_) => 2,
                        _ => 0,
                    },
                    admitted_at: f
                        .trace
                        .iter()
                        .find_map(|o| matches!(o.event, Occurrence::Admission).then_some(o.at)),
                    lost_updates: f.lost_updates.clone(),
                    condition_waits: f.condition_waits.clone(),
                    sleep: f.sleep(),
                    metric_history: m.metrics.as_ref().map(|_| {
                        f.tasks
                            .iter()
                            .map(|t| MetricTaskSignature {
                                binding: t.call.binding,
                                ready: t.metric_ready,
                                start: t.start,
                                stop: t.stop,
                                person: t.person,
                                status: match t.phase {
                                    Phase::Queued => 0,
                                    Phase::Running => 1,
                                    Phase::Finished(_) => 2,
                                    Phase::Cancelled => 3,
                                    Phase::Refused(_) => 4,
                                },
                            })
                            .collect()
                    }),
                    receipt: c.receipt,
                    end: c.end,
                    values: f.state_values.clone(),
                    control: f
                        .control
                        .iter()
                        .map(|b| b.iter().map(|s| s.span.start).collect())
                        .collect(),
                    waiting: f
                        .waiting
                        .as_ref()
                        .map(|wait| (wait.join.cloned(), wait.winner)),
                    tasks,
                    queued: range
                        .map(|j| c.queue.iter().find(|q| q.task == j).map_or(w.now, |q| q.at))
                        .collect(),
                    ever_started: f.tasks.iter().any(|t| t.start.is_some()),
                    results,
                    adoption: f.adoption,
                    work,
                    histories: histories(m, f, case_index)?,
                }))
            }
        });
    }
    Ok(Signature {
        external_peak: w.external_peak,
        time: w.now,
        cursor: w.next,
        boundary: match w.boundary {
            Boundary::Coins => 0,
            Boundary::Admit => 1,
            Boundary::Draw => 2,
            Boundary::Choose => 3,
            Boundary::Receive => 4,
            Boundary::Run => 5,
            Boundary::Activate => 6,
        },
        shared: w.shared.clone(),
        cases,
    })
}

type Action<'a> = (Option<(usize, String)>, Vec<(Rational, World<'a>)>);
#[cfg(test)]
pub(super) fn observed_signature(m: &Model, w: &World<'_>) -> String {
    format!("{:?}", signature(m, w).unwrap())
}
#[cfg(test)]
pub(super) fn observe_corrupt_sample<'a>(tree: &mut Tree<'a>, w: World<'a>) -> (bool, usize) {
    let mut random = Random {
        priority_tie: false,
        seed: 1,
        replication: 1,
        audit: vec![],
        origins: vec![],
    };
    let result = run_rep(tree, w, &mut random, &mut false, &mut 4);
    (result.is_err(), random.audit.len())
}
fn same_action(m: &Model, a: &Action<'_>, b: &Action<'_>) -> bool {
    if a.0 != b.0 {
        return false;
    }
    let normalize = |xs: &Vec<(Rational, World<'_>)>| {
        let mut out: Vec<(Rational, Result<Signature, StopReason>)> = Vec::new();
        for (p, w) in xs.iter().filter(|(p, _)| !p.is_zero()) {
            let state = signature(m, w);
            if let Some((q, last)) = out.last_mut()
                && *last == state
            {
                q.0 += &p.0;
            } else {
                out.push((p.clone(), state));
            }
        }
        out
    };
    normalize(&a.1) == normalize(&b.1)
}
enum Enabled<'a> {
    Response {
        case: usize,
        task: usize,
        value: ValueId,
    },
    Concrete(Action<'a>),
    Kernel {
        case: usize,
        task: usize,
        site: String,
    },
}
impl<'a> From<Action<'a>> for Enabled<'a> {
    fn from(action: Action<'a>) -> Self {
        Self::Concrete(action)
    }
}
fn keep<'a>(
    m: &Model,
    first: &mut Option<Enabled<'a>>,
    action: impl Into<Enabled<'a>>,
    nondet: &mut bool,
) -> Execution {
    let action = action.into();
    if let Some(old) = first {
        // All candidates are offers from the same immutable World. A kernel
        // factor is identified by its unique lexical call and case. Equal
        // factors therefore have the same given values, IR row, ordered
        // intervals and effects without materializing their children.
        let same = match (&*old, &action) {
            (
                Enabled::Response {
                    case: a,
                    task: i,
                    value: x,
                },
                Enabled::Response {
                    case: b,
                    task: j,
                    value: y,
                },
            ) => a == b && i == j && x == y,
            (Enabled::Concrete(a), Enabled::Concrete(b)) => same_action(m, a, b),
            (
                Enabled::Kernel {
                    case: a,
                    task: i,
                    site: s,
                },
                Enabled::Kernel {
                    case: b,
                    task: j,
                    site: t,
                },
            ) => a == b && i == j && s == t,
            _ => false,
        };
        if !same {
            *nondet = true;
            return Err(StopReason::SemanticAbort);
        }
    } else {
        *first = Some(action);
    }
    Ok(())
}

fn append<'a>(
    tree: &Tree<'a>,
    w: &mut World<'a>,
    k: usize,
    event: Occurrence,
    person: Option<PersonId>,
) {
    #[cfg(test)]
    crate::reference::work_profiles::observe(w.now, tree.m, &event);
    let values = tree.values(w, k);
    let Some(c) = w.cases[k].payload_mut() else {
        unreachable!()
    };
    let result = response::visible_values(&c.frame, tree.m, Some(&event));
    c.frame.trace.push(Observation {
        binding: None,
        state_values: values,
        person,
        at: w.now,
        event,
        values: result,
    });
    if tree
        .m
        .contracts
        .iter()
        .any(|c| matches!(c.kind, ContractKind::StateInvariant { .. }))
    {
        for j in 0..w.cases.len() {
            // ADR 0039 m8: received Held and Overflow cases are monitored too.
            if !w.cases[j].payload().is_some_and(|c| {
                c.frame
                    .trace
                    .iter()
                    .any(|o| matches!(o.event, Occurrence::Receipt))
            }) {
                continue;
            }
            let values = tree.values(w, j);
            if let Some(c) = w.cases[j].payload_mut()
                && c.frame
                    .state_snapshots
                    .last()
                    .is_none_or(|(_, old)| *old != values)
            {
                c.frame.state_snapshots.push((w.now, values));
            }
        }
    }
}

fn finished<'a>(
    tree: &Tree<'a>,
    w: &World<'a>,
    k: usize,
    j: usize,
    value: ValueId,
    overrides: &[(StateVarId, StateValue)],
) -> World<'a> {
    let mut next = w.clone();
    let values = tree.values(w, k);
    let CaseSlot::Here(c) = &mut next.cases[k] else {
        unreachable!()
    };
    let cap = c.frame.tasks[j].call.capability;
    let binding = c.frame.tasks[j].call.binding;
    let capability = &tree.m.capabilities[cap.0];
    let stale = reference_stale(&c.frame.tasks[j], capability);
    let person = c.frame.tasks[j].person;
    c.frame.state_values = values;
    c.frame.tasks[j].phase = Phase::Finished(value);
    c.frame.tasks[j].stop = Some(w.now);
    tree.cases[k].finish_state(&mut c.frame, j, value);
    for (i, x) in overrides {
        c.frame.state_values[i.0] = *x;
    }
    for (i, v) in tree.m.state_variables.iter().enumerate() {
        if shared_state(tree.m, v.kind) {
            next.shared[i] = c.frame.state_values[i];
            c.frame.state_values[i] = StateValue::None;
        }
    }
    let writes = if stale {
        &[][..]
    } else {
        capability
            .atomic
            .as_ref()
            .map_or(capability.writes.as_slice(), |a| {
                if value == a.commit { &a.writes } else { &[] }
            })
    };
    tree.notify(&mut next, k, j, writes);
    append(
        tree,
        &mut next,
        k,
        Occurrence::Completion(cap, value),
        person,
    );
    if tree.m.site_contest(binding).is_some() || evidence::present(tree.m) {
        next.cases[k]
            .payload_mut()
            .unwrap()
            .frame
            .trace
            .last_mut()
            .unwrap()
            .binding = Some(binding);
    }
    next
}

fn finish_kernel<'a>(
    tree: &Tree<'a>,
    w: &World<'a>,
    k: usize,
    j: usize,
    site: String,
    random: &mut Random,
) -> Result<World<'a>, StopReason> {
    let CaseSlot::Here(c) = &w.cases[k] else {
        return Err(StopReason::SemanticAbort);
    };
    let values = tree.values(w, k);
    let kernel =
        work_profiles::kernel(tree.m, &c.frame.tasks[j])?.ok_or(StopReason::SemanticAbort)?;
    let row = kernel
        .rows
        .iter()
        .find(|row| {
            row.values
                .iter()
                .zip(&kernel.given)
                .all(|(x, id)| *x == values[id.0])
        })
        .ok_or(StopReason::SemanticAbort)?;
    #[cfg(test)]
    crate::q6_acceptance::draw(w.now, site.clone());
    let choice = random.pick(
        k + 1,
        site,
        1,
        &row.branches
            .iter()
            .map(|b| b.probability.clone())
            .collect::<Vec<_>>(),
    )?;
    let branch = &row.branches[choice];
    sampled_receipt(
        tree,
        finished(tree, w, k, j, branch.result, &branch.overrides),
        k,
        j,
        branch.result,
        random,
    )
}

fn response_site(m: &Model, binding: BindingId, kind: &str) -> String {
    if let Some(r) = m.response_attempt(binding) {
        format!(
            "{kind}.{}.{}.attempt{}",
            m.process.name,
            m.site_name(r.command),
            r.attempt
        )
    } else {
        format!("{kind}.{}.{}", m.process.name, m.site_name(binding))
    }
}

fn sampled_receipt<'a>(
    tree: &Tree<'a>,
    mut w: World<'a>,
    k: usize,
    j: usize,
    actual: ValueId,
    random: &mut Random,
) -> Result<World<'a>, StopReason> {
    let c = w.cases[k].payload().ok_or(StopReason::SemanticAbort)?;
    let binding = c.frame.tasks[j].call.binding;
    let Some(r) = tree.m.response_attempt(binding) else {
        if tree.m.capabilities[c.frame.tasks[j].call.capability.0]
            .receipt_query
            .is_some()
        {
            let event = recovery::query(&c.frame, tree.m, binding, actual, &tree.values(&w, k), k)?;
            append(tree, &mut w, k, event, None);
        }
        return Ok(w);
    };
    use crate::ast::Receipt::{Delivered, Unknown};
    let observation = match &r.loss {
        ResponseLoss::None => Delivered,
        ResponseLoss::Sequence(xs) => xs[r.attempt as usize - 1],
        ResponseLoss::Nondet => return Err(StopReason::SemanticAbort),
        ResponseLoss::Probability { probability: p, .. } => {
            let weights = [Rational(Rational::one().0 - &p.0), p.clone()];
            if random.pick(
                k + 1,
                response_site(tree.m, binding, "receipt"),
                1,
                &weights,
            )? == 0
            {
                Delivered
            } else {
                Unknown
            }
        }
    };
    let old_unknown=c.frame.trace.iter().any(|o|matches!(&o.event,Occurrence::Channel(old) if old.command==r.command && old.attempt<r.attempt && !old.resolved));
    append(
        tree,
        &mut w,
        k,
        Occurrence::Channel(response::Receipt {
            #[cfg(test)]
            weight: Some(Rational::one()),
            command: r.command,
            binding,
            attempt: r.attempt,
            actual,
            observation,
            resolved: observation == Delivered && (r.idempotent || !old_unknown),
        }),
        None,
    );
    Ok(w)
}

fn create<'a>(
    c: &mut CaseFrame<'a>,
    calls: &[Call],
    join: Option<&'a Join>,
    now: Tick,
    metrics: bool,
) {
    let first = c.frame.tasks.len();
    for call in calls {
        let j = c.frame.tasks.len();
        c.frame.tasks.push(Task {
            frozen_performance: None,
            stale: BTreeSet::new(),
            metric_ready: metrics.then_some(now),
            checked_through: None,
            person: None,
            call: call.clone(),
            phase: Phase::Queued,
            start: None,
            stop: None,
            finish: None,
            ack: None,
        });
        c.queue.push(QueueEntry { task: j, at: now });
    }
    c.frame.waiting = Some(Waiting {
        tasks: first..c.frame.tasks.len(),
        join,
        winner: None,
    });
}

type SampledLeaf<'a> = (World<'a>, Option<SimulationCut>, Vec<Option<Tick>>);

fn sample_durations<'a>(
    tree: &Tree<'a>,
    w: &mut World<'a>,
    random: &mut Random,
) -> Result<bool, StopReason> {
    let Some(coins) = tree.coins(w, 4096)? else {
        return Ok(false);
    };
    for (k, j, p) in coins {
        let CaseSlot::Here(c) = &w.cases[k] else {
            unreachable!()
        };
        let task = &c.frame.tasks[j];
        let age = w.now - task.start.ok_or(StopReason::SemanticAbort)?;
        let occurrence = tree.m.capabilities[task.call.capability.0]
            .duration_distribution
            .as_ref()
            .ok_or(StopReason::SemanticAbort)?
            .entries
            .iter()
            .filter(|(at, p)| !p.is_zero() && *at <= age)
            .count();
        let site = format!(
            "duration.{}.{}",
            tree.m.process.name,
            tree.m.site_name(task.call.binding)
        );
        let finish = random.pick(
            k + 1,
            site,
            occurrence,
            &[p.clone(), Rational(Rational::one().0 - &p.0)],
        )? == 0;
        if let CaseSlot::Here(c) = &mut w.cases[k] {
            c.frame.tasks[j].checked_through = Some(age);
            c.frame.tasks[j].finish = finish.then_some(w.now);
        }
    }
    Ok(true)
}

// Resolve one management step, then revisit all cases before any dispatch.
// Uses this interpreter's trace, never the engine's state or sampler.
fn sample_contest_management<'a>(
    tree: &mut Tree<'a>,
    w: &mut World<'a>,
    random: &mut Random,
    nondet: &mut bool,
) -> Result<bool, StopReason> {
    for k in 0..w.cases.len() {
        let CaseSlot::Here(c) = &w.cases[k] else {
            continue;
        };
        if c.stopped(tree.m) {
            continue;
        }
        if let Some(id) = c.frame.contest_pending(tree.m) {
            let d = &tree.m.contests()[id];
            let Some(rates) = &d.rates else {
                *nondet = true;
                random
                    .origins
                    .push((k + 1, format!("contest{}", d.ordinal), d.span));
                return Err(StopReason::SemanticAbort);
            };
            c.frame
                .contest_target(tree.m, id)
                .unwrap()
                .at
                .checked_add(d.respond_by)
                .ok_or(StopReason::SemanticAbort)?;
            let filed = random.pick(
                k + 1,
                format!(
                    "contest.{}.clause{}.occurrence",
                    tree.m.process.name, d.ordinal
                ),
                1,
                &[
                    Rational(Rational::one().0 - &rates.probability.0),
                    rates.probability.clone(),
                ],
            )? == 1;
            append(tree, w, k, Occurrence::ContestChoice(id, filed), None);
            return Ok(true);
        }
        if c.end.is_some() {
            let mut states = tree.contest_management(w, k)?;
            if !states.is_empty() {
                require(states.len() == 1)?;
                *w = states.remove(0);
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn run_rep<'a>(
    tree: &mut Tree<'a>,
    mut w: World<'a>,
    random: &mut Random,
    nondet: &mut bool,
    observed_time: &mut Tick,
) -> Result<SampledLeaf<'a>, StopReason> {
    let m = tree.m;
    let mut cut = None;
    let mut dead = vec![None; w.cases.len()];
    'path: loop {
        random.priority_tie = false;
        *observed_time = w.now;
        w.steps += 1;
        if w.steps >= tree.limits.steps_per_path {
            return Err(StopReason::EnumerationLimit);
        }
        if w.cases.iter().any(|s| matches!(s, CaseSlot::Here(c) if c.frame.tasks.iter().filter(|t| !t.terminal()).count() as u64 > m.bounds.live_tasks)) { cut = Some(SimulationCut::LiveTasks); break; }
        if matches!(w.boundary, Boundary::Coins) {
            if !sample_durations(tree, &mut w, random)? {
                cut = Some(SimulationCut::Numeric);
                break;
            }
            while w.next < tree.times.len() && tree.times[w.next] == w.now {
                let k = w.next;
                w.next += 1;
                let waiting = w.cases.iter().filter(|s| matches!(s,CaseSlot::Here(c) if c.end.is_none() && !c.frame.tasks.iter().any(|t|t.start.is_some()))).count() as u64;
                let full = m.admission_queue.is_some_and(|q| waiting >= q.capacity)
                    || m.concurrent_cases.is_some_and(|q| {
                        w.cases
                            .iter()
                            .filter(|s| matches!(s, CaseSlot::Here(c) if c.end.is_none()))
                            .count() as u64
                            >= q.capacity
                    });
                let overflow = m
                    .admission_queue
                    .or(m.concurrent_cases)
                    .map_or(Overflow::Reject, |q| q.overflow);
                if full && overflow == Overflow::Reject {
                    w.cases[k] = CaseSlot::Declined;
                    continue;
                }
                let c = CaseFrame {
                    frame: Frame {
                        condition_waits: Vec::new(),
                        lost_updates: BTreeSet::new(),
                        state_snapshots: Vec::new(),
                        business: None,
                        state_values: m
                            .state_variables
                            .iter()
                            .map(|v| match (v.kind, v.initial) {
                                (_, _) if shared_state(m, v.kind) => StateValue::None,
                                (_, InitialValue::Fixed(x)) => x,
                                _ => StateValue::None,
                            })
                            .collect(),
                        now: w.now,
                        steps: 0,
                        control: vec![&m.process.body],
                        waiting: None,
                        tasks: Vec::new(),
                        adoption: None,
                        trace: Vec::new(),
                    },
                    queue: Vec::new(),
                    receipt: false,
                    end: None,
                };
                w.cases[k] = if !full {
                    CaseSlot::Here(c)
                } else {
                    match overflow {
                        Overflow::WaitExternalFifo | Overflow::WaitExternalPriority(_) => {
                            CaseSlot::Held(c)
                        }
                        Overflow::End(_) => CaseSlot::Overflow(c),
                        _ => unreachable!(),
                    }
                };
            }
            w.external_peak = w.external_peak.max(
                w.cases
                    .iter()
                    .filter(|s| matches!(s, CaseSlot::Held(_)))
                    .count() as u64,
            );
            // All independent probability factors precede every latent choice.
            random.origins.clear();
            for chance in [true, false] {
                if !chance {
                    for k in 0..w.cases.len() {
                        if !w.cases[k].payload().is_some_and(|c| !c.receipt) {
                            continue;
                        }
                        for v in &m.state_variables {
                            if v.initial == InitialValue::Nondet && reference_cardinality(m, v) > 1
                            {
                                let (case, site) = match v.kind {
                                    StateKind::ActorLatent(a) => (
                                        if m.actors[a.0].metadata.instance.is_some() {
                                            k + 1
                                        } else {
                                            0
                                        },
                                        format!("init.actor.{}.{}", m.actors[a.0].name, v.name),
                                    ),
                                    _ => (k + 1, format!("init.case.{}", v.name)),
                                };
                                random.origins.push((case, site, v.span));
                            }
                        }
                    }
                    if !random.origins.is_empty() {
                        *nondet = true;
                        return Err(StopReason::SemanticAbort);
                    }
                }
                for k in 0..w.cases.len() {
                    if !w.cases[k].payload().is_some_and(|c| !c.receipt) {
                        continue;
                    }
                    for (i, v) in m.state_variables.iter().enumerate() {
                        if shared_state(m, v.kind) && m.bounds.cases > 1 {
                            continue;
                        }
                        let x = match v.initial {
                            InitialValue::Distribution(id) if chance => {
                                let support = &m.initial_distributions[id];
                                let (case, site) = match v.kind {
                                    StateKind::ActorLatent(a) => (
                                        if m.actors[a.0].metadata.instance.is_some() {
                                            k + 1
                                        } else {
                                            0
                                        },
                                        format!("init.actor.{}.{}", m.actors[a.0].name, v.name),
                                    ),
                                    _ => (k + 1, format!("init.case.{}", v.name)),
                                };
                                let choice = random.pick(
                                    case,
                                    site,
                                    1,
                                    &support.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(),
                                )?;
                                support[choice].0
                            }
                            InitialValue::Nondet if !chance => {
                                if reference_cardinality(m, v) > 1 {
                                    let (case, site) = match v.kind {
                                        StateKind::ActorLatent(a) => (
                                            if m.actors[a.0].metadata.instance.is_some() {
                                                k + 1
                                            } else {
                                                0
                                            },
                                            format!("init.actor.{}.{}", m.actors[a.0].name, v.name),
                                        ),
                                        _ => (k + 1, format!("init.case.{}", v.name)),
                                    };
                                    random.origins.push((case, site, v.span));
                                    *nondet = true;
                                    return Err(StopReason::SemanticAbort);
                                }
                                if v.integer_max.is_some() {
                                    StateValue::Integer(0)
                                } else {
                                    StateValue::Value(ValueId(0))
                                }
                            }
                            _ => continue,
                        };
                        if shared_state(m, v.kind) {
                            w.shared[i] = x;
                        } else if let Some(c) = w.cases[k].payload_mut() {
                            c.frame.state_values[i] = x;
                        }
                    }
                }
            }
            for k in 0..w.cases.len() {
                if w.cases[k].payload().is_some_and(|c| !c.receipt) {
                    append(tree, &mut w, k, Occurrence::Receipt, None);
                    if let Some(c) = w.cases[k].payload_mut() {
                        c.receipt = true;
                    }
                }
            }
            w.boundary = Boundary::Run;
            if m.explicit_limits() {
                for k in 0..w.cases.len() {
                    if matches!(w.cases[k], CaseSlot::Here(_))
                        && !w.cases[k]
                            .payload()
                            .unwrap()
                            .frame
                            .trace
                            .iter()
                            .any(|o| matches!(o.event, Occurrence::Admission))
                    {
                        append(tree, &mut w, k, Occurrence::Admission, None);
                    } else if matches!(&w.cases[k], CaseSlot::Overflow(c) if c.end.is_none()) {
                        tree.ending(&mut w, k, m.overflow_end.ok_or(StopReason::SemanticAbort)?)?;
                    }
                }
            }
        }
        tree.observe(&w)?;
        if sample_contest_management(tree, &mut w, random, nondet)? {
            continue;
        }
        for k in 0..w.cases.len() {
            while let CaseSlot::Here(c) = &w.cases[k] {
                if c.end.is_some() || c.stopped(m) {
                    break;
                }
                if let Some(wake) = c.frame.sleep() {
                    if wake > w.now {
                        break;
                    }
                    append(tree, &mut w, k, Occurrence::Wake, None);
                    continue;
                }
                if let Some(wait) = &c.frame.waiting {
                    if !wait.tasks.clone().all(|j| c.frame.tasks[j].terminal())
                        || !tree.cases[k].winners(&c.frame, wait).is_empty()
                    {
                        break;
                    }
                    if wait.join.is_none()
                        && m.response_attempt(c.frame.tasks[wait.tasks.start].call.binding)
                            .is_some()
                    {
                        tree.response_progress(&mut w, k)?;
                        continue;
                    }
                    if let Some(Join::Any { if_none, .. }) = wait.join
                        && wait.winner.is_none()
                    {
                        tree.ending(&mut w, k, *if_none)?;
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
                let stmt = &block[0];
                if let Stmt::WaitUntil(condition) = &stmt.kind {
                    if shared_guard(m, condition) {
                        c.frame.condition_enter();
                        break;
                    }
                    if tree.cases[k].condition(&c.frame, condition, 0)? {
                        c.frame.condition_pass()?;
                        continue;
                    }
                    c.frame.condition_enter();
                    break;
                }
                if matches!(&stmt.kind, Stmt::If { condition, .. } if shared_guard(m, condition)) {
                    break;
                }
                *block = &block[1..];
                match &stmt.kind {
                    Stmt::MergeCopy { binding, source } => {
                        let value = c.frame.merge_value(m, *source)?;
                        require(!c.frame.trace.iter().any(|o| matches!(o.event, Occurrence::Merged(id, _, _, _) if id == *binding)))?;
                        append(
                            tree,
                            &mut w,
                            k,
                            Occurrence::Merged(*binding, *source, value, stmt.span),
                            None,
                        );
                    }
                    Stmt::LoopControl(control) => {
                        for (id, x) in &control.updates {
                            c.frame.state_values[id.0] = *x;
                        }
                    }
                    Stmt::ResolveEffect { command, evidence } => {
                        let event =
                            recovery::resolve(&c.frame, m, *command, *evidence, stmt.span, k)?;
                        append(tree, &mut w, k, event, None);
                    }
                    Stmt::StopReceiptUnknown => {
                        let event = recovery::stop(&c.frame, m)?;
                        append(tree, &mut w, k, event, None);
                        break;
                    }
                    Stmt::WaitUntil(_) => return Err(StopReason::SemanticAbort),
                    Stmt::Wait(duration) => {
                        let wake = w
                            .now
                            .checked_add(*duration)
                            .ok_or(StopReason::SemanticAbort)?;
                        append(tree, &mut w, k, Occurrence::Wait(wake), None);
                    }
                    Stmt::End(e) => {
                        tree.ending(&mut w, k, *e)?;
                        break;
                    }
                    Stmt::If {
                        condition,
                        then_body,
                        else_body,
                    } => {
                        let branch = if tree.condition(&w, k, condition)? {
                            then_body
                        } else {
                            else_body
                        };
                        if let CaseSlot::Here(c) = &mut w.cases[k] {
                            c.frame.control.push(branch);
                        }
                    }
                    Stmt::Response(call, r) => {
                        if let Some(event) = response::input(&c.frame, m, call, r, k)? {
                            append(tree, &mut w, k, event, None);
                        }
                        let CaseSlot::Here(c) = &mut w.cases[k] else {
                            unreachable!()
                        };
                        create(
                            c,
                            std::slice::from_ref(call),
                            None,
                            w.now,
                            m.metrics.is_some(),
                        );
                    }
                    Stmt::Call(call) | Stmt::Query(call, _) => create(
                        c,
                        std::slice::from_ref(call),
                        None,
                        w.now,
                        m.metrics.is_some(),
                    ),
                    Stmt::Fork { branches, join, .. } => {
                        create(c, branches, Some(join), w.now, m.metrics.is_some())
                    }
                }
                if w.cases.iter().any(|s|matches!(s,CaseSlot::Here(c) if c.frame.tasks.iter().filter(|t|!t.terminal()).count() as u64>m.bounds.live_tasks)){cut=Some(SimulationCut::LiveTasks);break 'path;}
            }
            // Re-stabilize global contest management after each case's control
            // step, not just End, and before dispatch/another case's live bound.
            // Legacy encounter refusal is global. This does not add RNG draws.
            if sample_contest_management(tree, &mut w, random, nondet)? {
                continue 'path;
            }
        }
        if sample_contest_management(tree, &mut w, random, nondet)? {
            continue;
        }
        random.origins.clear();
        let mut first = None;
        let mut priority_task_choices = 0usize;
        for k in 0..w.cases.len() {
            let CaseSlot::Here(c) = &w.cases[k] else {
                continue;
            };
            if c.stopped(m) {
                continue;
            }
            if c.frame.waiting.is_none()
                && c.frame.sleep().is_none()
                && let Some(stmt) = c.frame.control.iter().rev().find_map(|b| b.first())
                && let Stmt::WaitUntil(condition) = &stmt.kind
                && shared_guard(m, condition)
                && tree.condition(&w, k, condition)?
            {
                let mut next = w.clone();
                if let CaseSlot::Here(c) = &mut next.cases[k] {
                    c.frame.condition_pass()?;
                }
                keep(m, &mut first, (None, vec![(Rational::one(), next)]), nondet)?;
            }
            if c.frame.waiting.is_none()
                && c.frame.sleep().is_none()
                && let Some(stmt) = c.frame.control.iter().rev().find_map(|b| b.first())
                && let Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } = &stmt.kind
                && shared_guard(m, condition)
            {
                let branch = if tree.condition(&w, k, condition)? {
                    then_body
                } else {
                    else_body
                };
                let mut next = w.clone();
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
                    c.frame.control.push(branch);
                }
                keep(m, &mut first, (None, vec![(Rational::one(), next)]), nondet)?;
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
            for j in &due {
                let t = &c.frame.tasks[*j];
                random.origin(m, k + 1, t.call.binding, false);
                let cap = &m.capabilities[t.call.capability.0];
                let kernel = work_profiles::kernel(m, t)?;
                let finish_allowed = if t.finish == Some(w.now) {
                    let (allowed, unresolved) = tree.priority_finish_order(&w, k, *j);
                    random.priority_tie |= unresolved;
                    allowed
                } else {
                    false
                };
                if finish_allowed {
                    let mut frame = c.frame.clone();
                    frame.state_values = tree.values(&w, k);
                    if tree.cases[k].range_attempt(
                        &frame,
                        *j,
                        false,
                        (!m.legacy_route()).then_some(k as u64 + 1),
                    )? {
                        continue;
                    }
                    if kernel.is_some() {
                        keep(
                            m,
                            &mut first,
                            Enabled::Kernel {
                                case: k,
                                task: *j,
                                site: response_site(m, t.call.binding, "kernel"),
                            },
                            nondet,
                        )?;
                    } else {
                        let selected = if reference_stale(t, cap) {
                            vec![cap.version_check.as_ref().unwrap().reject]
                        } else if cap.receipt_query.is_some() {
                            recovery::project(m, t.call.capability, &tree.values(&w, k), k)?
                        } else if let Some(a) = &cap.atomic {
                            vec![if tree.condition(&w, k, &a.condition)? == a.commit_when {
                                a.commit
                            } else {
                                a.reject
                            }]
                        } else {
                            cap.results.clone()
                        };
                        for x in &selected {
                            if let Some(r) = m.response_attempt(t.call.binding) {
                                if matches!(r.loss, ResponseLoss::Nondet) {
                                    *nondet = true;
                                    return Err(StopReason::SemanticAbort);
                                }
                                keep(
                                    m,
                                    &mut first,
                                    Enabled::Response {
                                        case: k,
                                        task: *j,
                                        value: *x,
                                    },
                                    nondet,
                                )?;
                                continue;
                            }
                            if cap.receipt_query.is_some() {
                                keep(
                                    m,
                                    &mut first,
                                    Enabled::Response {
                                        case: k,
                                        task: *j,
                                        value: *x,
                                    },
                                    nondet,
                                )?;
                                continue;
                            }
                            keep(
                                m,
                                &mut first,
                                (
                                    None,
                                    vec![(Rational::one(), finished(tree, &w, k, *j, *x, &[]))],
                                ),
                                nondet,
                            )?;
                        }
                    }
                }
                if t.ack == Some(w.now) {
                    let mut next = w.clone();
                    if let CaseSlot::Here(c) = &mut next.cases[k] {
                        c.frame.tasks[*j].phase = Phase::Cancelled;
                        c.frame.tasks[*j].stop = Some(w.now);
                    }
                    if m.tracks_obligations() {
                        append(
                            tree,
                            &mut next,
                            k,
                            Occurrence::CancelAck(t.call.capability),
                            t.person,
                        );
                    }
                    keep(m, &mut first, (None, vec![(Rational::one(), next)]), nondet)?;
                }
            }
            if due.is_empty()
                && let Some(wait) = &c.frame.waiting
            {
                for winner in tree.cases[k].winners(&c.frame, wait) {
                    random.origin(m, k + 1, winner, true);
                    let mut next = w.clone();
                    if let CaseSlot::Here(c) = &mut next.cases[k] {
                        c.frame.adoption = Some(w.now);
                        c.frame.waiting.as_mut().unwrap().winner = Some(winner);
                        if matches!(
                            wait.join,
                            Some(Join::Any {
                                remaining: Remaining::CancelThenDrain,
                                ..
                            })
                        ) {
                            for j in wait.tasks.clone() {
                                let task = &mut c.frame.tasks[j];
                                if task.call.binding == winner || task.terminal() {
                                    continue;
                                }
                                random.origin(m, k + 1, task.call.binding, false);
                                match (task.phase, m.capabilities[task.call.capability.0].cancel) {
                                    (Phase::Queued, _) => {
                                        task.phase = Phase::Cancelled;
                                        task.stop = Some(w.now);
                                    }
                                    (_, Cancel::ForbiddenAfterStart) => {}
                                    (_, Cancel::Immediate) => {
                                        task.ack = Some(w.now);
                                    }
                                    (_, Cancel::Cooperative { ack_within }) => {
                                        let cap = &m.capabilities[task.call.capability.0];
                                        let finish = if let Some(d) = &cap.duration_distribution {
                                            task.start
                                                .unwrap()
                                                .checked_add(
                                                    d.entries
                                                        .iter()
                                                        .filter(|(_, p)| !p.is_zero())
                                                        .map(|(t, _)| *t)
                                                        .max()
                                                        .unwrap(),
                                                )
                                                .ok_or(StopReason::SemanticAbort)?
                                        } else {
                                            task.finish.unwrap()
                                        };
                                        let last = w
                                            .now
                                            .checked_add(ack_within)
                                            .ok_or(StopReason::SemanticAbort)?
                                            .min(finish.saturating_add(1))
                                            .min(m.bounds.clock.saturating_add(1));
                                        if last - w.now > 1 {
                                            *nondet = true;
                                            return Err(StopReason::SemanticAbort);
                                        }
                                        task.ack = Some(last);
                                    }
                                    _ => return Err(StopReason::SemanticAbort),
                                }
                            }
                        }
                    }
                    keep(m, &mut first, (None, vec![(Rational::one(), next)]), nondet)?;
                }
            }
            for j in 0..c.frame.tasks.len() {
                let eligible = tree.eligible(&w, k, j)?;
                if !eligible.is_empty()
                    && m.start_policy.as_ref().is_some_and(|p| {
                        p.priority.is_some()
                            && p.controls(m.capabilities[c.frame.tasks[j].call.capability.0].actor)
                    })
                {
                    priority_task_choices += 1;
                    random.priority_tie |= priority_task_choices > 1;
                }
                for person in eligible {
                    random.origin(m, k + 1, c.frame.tasks[j].call.binding, false);
                    let mut next = w.clone();
                    let cap_id = c.frame.tasks[j].call.capability;
                    let cap = &m.capabilities[cap_id.0];
                    if let Some(req) = &cap.requires
                        && !tree.condition(&w, k, &req.condition)?
                    {
                        if let CaseSlot::Here(c) = &mut next.cases[k] {
                            c.frame.tasks[j].phase = Phase::Refused(req.otherwise);
                            c.frame.tasks[j].stop = Some(w.now);
                        }
                        let binding = c.frame.tasks[j].call.binding;
                        tree.record_context(
                            &mut next,
                            k,
                            Occurrence::Refusal(cap_id, req.otherwise),
                            None,
                            (false, Some(binding)),
                        )?;
                        keep(m, &mut first, (None, vec![(Rational::one(), next)]), nondet)?;
                        break;
                    }
                    let mut frame = c.frame.clone();
                    frame.state_values = tree.values(&w, k);
                    if tree.cases[k].range_attempt(
                        &frame,
                        j,
                        true,
                        (!m.legacy_route()).then_some(k as u64 + 1),
                    )? {
                        continue;
                    }
                    let before = frame.state_values;
                    let frozen = work_profiles::select(m, cap_id, person, tree.backlog(&w, k, j))?;
                    if let CaseSlot::Here(c) = &mut next.cases[k] {
                        let t = &mut c.frame.tasks[j];
                        t.frozen_performance = frozen;
                        t.phase = Phase::Running;
                        t.stale.clear();
                        t.person = person;
                        t.start = Some(w.now);
                        if let Some(d) = &cap.duration_distribution {
                            for (at, p) in &d.entries {
                                if !p.is_zero() {
                                    w.now.checked_add(*at).ok_or(StopReason::SemanticAbort)?;
                                }
                            }
                        } else {
                            t.finish = Some(
                                w.now
                                    .checked_add(cap.duration)
                                    .ok_or(StopReason::SemanticAbort)?,
                            );
                        }
                    }
                    if let CaseSlot::Here(c) = &mut next.cases[k] {
                        c.frame.state_values = before.clone();
                        for w in &cap.start_writes {
                            c.frame.state_values[w.variable.0] =
                                reference_effect_at(w, &before, None, k, person)?;
                        }
                        for (i, v) in m.state_variables.iter().enumerate() {
                            if shared_state(m, v.kind) {
                                next.shared[i] = c.frame.state_values[i];
                                c.frame.state_values[i] = StateValue::None;
                            }
                        }
                    }
                    tree.notify(&mut next, k, j, &cap.start_writes);
                    append(tree, &mut next, k, Occurrence::Dispatch(cap_id), person);
                    let binding = c.frame.tasks[j].call.binding;
                    if m.site_contest(binding).is_some() || evidence::present(m) {
                        next.cases[k]
                            .payload_mut()
                            .unwrap()
                            .frame
                            .trace
                            .last_mut()
                            .unwrap()
                            .binding = Some(binding);
                    }
                    keep(m, &mut first, (None, vec![(Rational::one(), next)]), nondet)?;
                }
            }
        }
        if tree.cases.iter().any(|i| !i.statistics.ranges.is_empty()) {
            if first.is_some() {
                *nondet = true;
                return Err(StopReason::SemanticAbort);
            }
            break 'path;
        }
        if let Some(Enabled::Response { case, task, value }) = first {
            w = sampled_receipt(
                tree,
                finished(tree, &w, case, task, value, &[]),
                case,
                task,
                value,
                random,
            )?;
            continue;
        }
        if let Some(Enabled::Kernel { case, task, site }) = first {
            w = finish_kernel(tree, &w, case, task, site, random)?;
            continue;
        }
        if let Some(Enabled::Concrete((tag, choices))) = first {
            let i = if let Some((case, site)) = tag {
                random.pick(
                    case,
                    site,
                    1,
                    &choices.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
                )?
            } else {
                0
            };
            w = choices
                .into_iter()
                .nth(i)
                .ok_or(StopReason::SemanticAbort)?
                .1;
            continue;
        }
        if m.external_priority().is_some() {
            let admissions = tree.priority_admissions(&w);
            if admissions.len() > 1 {
                random.priority_tie = true;
                *nondet = true;
                return Err(StopReason::SemanticAbort);
            }
            if let Some(&k) = admissions.first() {
                let CaseSlot::Held(c) = std::mem::replace(&mut w.cases[k], CaseSlot::NotHere)
                else {
                    unreachable!()
                };
                w.cases[k] = CaseSlot::Here(c);
                append(tree, &mut w, k, Occurrence::Admission, None);
                continue;
            }
        }
        let mut resumed = false;
        for k in 0..w.cases.len() {
            if m.external_priority().is_some() {
                break;
            }
            if !matches!(w.cases[k], CaseSlot::Held(_)) {
                continue;
            }
            let waiting = w.cases.iter().filter(|s| matches!(s, CaseSlot::Here(c) if c.end.is_none() && !c.frame.tasks.iter().any(|t| t.start.is_some()))).count() as u64;
            let concurrent = w
                .cases
                .iter()
                .filter(|s| matches!(s, CaseSlot::Here(c) if c.end.is_none()))
                .count() as u64;
            if m.admission_queue.is_some_and(|q| waiting >= q.capacity)
                || m.concurrent_cases.is_some_and(|q| concurrent >= q.capacity)
            {
                break;
            }
            let CaseSlot::Held(c) = std::mem::replace(&mut w.cases[k], CaseSlot::NotHere) else {
                unreachable!()
            };
            w.cases[k] = CaseSlot::Here(c);
            append(tree, &mut w, k, Occurrence::Admission, None);
            resumed = true;
        }
        if resumed {
            continue;
        }
        let running=w.cases.iter().any(|s|matches!(s,CaseSlot::Here(c) if c.frame.tasks.iter().any(|t|t.phase==Phase::Running)));
        let sleeping = w
            .cases
            .iter()
            .any(|s| matches!(s,CaseSlot::Here(c) if c.frame.sleep().is_some()));
        let calendar_pending = tree.calendar_pending(&w)?;
        if !running && !sleeping && !calendar_pending && w.next == tree.times.len() {
            for (k, s) in w.cases.iter().enumerate() {
                if s.payload().is_some_and(|c| !c.stopped(m)) {
                    dead[k] = Some(w.now);
                }
            }
            break;
        }
        if m.horizon == Some(w.now) && w.now <= m.bounds.clock {
            cut = Some(SimulationCut::Horizon);
            break;
        }
        if w.now == m.bounds.clock {
            cut = Some(SimulationCut::Clock);
            break;
        }
        w.now = w.now.checked_add(1).ok_or(StopReason::SemanticAbort)?;
        w.boundary = Boundary::Coins;
        for s in &mut w.cases {
            if let Some(c) = s.payload_mut()
                && !c.stopped(m)
            {
                c.frame.now = w.now;
            }
        }
    }
    Ok((w, cut, dead))
}

struct QueryAt {
    arrival: Tick,
    now: Tick,
    clock: Tick,
    cut: Option<SimulationCut>,
    stopped: bool,
}
fn query_truth(
    slot: &CaseSlot<'_>,
    query: &ProbabilityQuery,
    at: &QueryAt,
) -> Result<QueryTruth, StopReason> {
    if let Some(case) = slot.payload() {
        for event in &case.frame.trace {
            if observation_hit(query, event, at.arrival)? {
                return Ok(QueryTruth::Hit);
            }
        }
    }
    if at.stopped
        || matches!(slot, CaseSlot::Declined)
        || slot.payload().is_some_and(|c| c.end.is_some())
    {
        return Ok(QueryTruth::Miss);
    }
    let expired = query.within.is_some_and(|deadline| {
        at.now >= at.arrival && at.now - at.arrival > deadline
            || matches!(at.cut, Some(SimulationCut::Clock | SimulationCut::Horizon))
                && at.arrival <= at.clock
                && deadline <= at.clock - at.arrival
    });
    Ok(if at.cut != Some(SimulationCut::Numeric) && expired {
        QueryTruth::Miss
    } else if at.cut.is_some() {
        QueryTruth::Unknown
    } else {
        QueryTruth::Miss
    })
}
fn observation_abort(
    mut out: SimulationOutcome,
    m: &Model,
    replication: u64,
    time: Tick,
) -> SimulationRun {
    out.diagnostics = vec![diag::Diagnostic {
        code: diag::U_SEM_0.into(),
        severity: diag::Severity::Error,
        message: format!("reference observation failed: replication {replication}, time {time}"),
        span: Some(m.process.span),
    }];
    out.truncated_replications = None;
    SimulationRun {
        stop: StopReason::SemanticAbort,
        outcome: out,
    }
}

fn sampled_contests(
    w: &World<'_>,
    m: &Model,
    cut: Option<SimulationCut>,
    range_stopped: bool,
) -> Vec<crate::outcome::ContestObservation> {
    let mut records = Vec::new();
    for (k, slot) in w.cases.iter().enumerate() {
        for (id, d) in m
            .contests()
            .iter()
            .enumerate()
            .filter(|(_, d)| d.rates.is_some())
        {
            let c = slot.payload();
            let target_at = c.and_then(|c| c.frame.contest_target(m, id)).map(|o| o.at);
            let filed = c.and_then(|c| {
                c.frame.trace.iter().find_map(|o| match o.event {
                    Occurrence::ContestChoice(n, filed) if n == id => Some(filed),
                    _ => None,
                })
            });
            let task = c.and_then(|c| {
                c.frame
                    .tasks
                    .iter()
                    .find(|t| t.call.binding == m.contest_site(id))
            });
            let response_at = c
                .and_then(|c| {
                    c.frame.trace.iter().find(|o| {
                        o.binding == Some(m.contest_site(id))
                            && matches!(o.event, Occurrence::Completion(_, _))
                    })
                })
                .map(|o| o.at);
            let excluded = !crate::responsibility::included(m, m.capabilities[d.route.0].actor);
            let terminal_route = task.is_some_and(Task::terminal);
            let failed = filed == Some(true)
                && response_at.is_none()
                && (excluded || terminal_route || (cut.is_none() && !range_stopped));
            let failure_at = if excluded {
                target_at
            } else if terminal_route {
                task.and_then(|t| t.stop)
            } else {
                (cut.is_none() && !range_stopped).then_some(w.now)
            };
            records.push(crate::outcome::ContestObservation {
                contest: id,
                case_id: k as u64 + 1,
                target_at,
                filed,
                target_known: target_at.is_some()
                    || (cut.is_none() && !range_stopped)
                    || matches!(slot, CaseSlot::Declined)
                    || c.is_some_and(|c| c.end.is_some()),
                started_at: task.and_then(|t| t.start),
                response_at,
                closed_at: response_at
                    .or_else(|| (filed == Some(false)).then_some(target_at).flatten())
                    .or_else(|| failed.then_some(failure_at).flatten()),
                failed,
            });
        }
    }
    records
}

pub(in crate::reference) fn simulate(m: &Model) -> SimulationRun {
    let mut out = SimulationOutcome {
        stochastic_arrivals: m.arrival_distribution.is_some(),
        ..SimulationOutcome::default()
    };
    let limits = Limits::default();
    let invalid = || SimulationRun {
        stop: StopReason::SemanticAbort,
        outcome: SimulationOutcome {
            diagnostics: vec![diag::Diagnostic {
                code: diag::U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: if m.unsupported_stochastic_arrival_combination() {
                    "stochastic_arrivals_feature_combination"
                } else {
                    "invalid reference simulation input"
                }
                .into(),
                span: Some(m.process.span),
            }],
            ..SimulationOutcome::default()
        },
    };
    let Some(spec) = &m.simulation else {
        return invalid();
    };
    if m.unsupported_stochastic_arrival_combination() {
        return invalid();
    }
    if !(1..=10000).contains(&spec.replications)
        || spec.confidence <= Rational::zero()
        || spec.confidence >= Rational::one()
        || m.verify.is_empty()
    {
        return invalid();
    }
    let times = m.arrivals.clone().unwrap_or_else(|| vec![0]);
    if m.arrival_distribution
        .as_ref()
        .is_some_and(|d| d.validate(m).is_err())
    {
        return invalid();
    }
    let n = times.len();
    let mut template = m.clone();
    template.arrival_distribution = None;
    template.bounds.cases = m.bounds.cases;
    if validate(&template, limits).is_err()
        || validate_observed_writers(m).is_err()
        || n == 0
        || n as u64 != m.bounds.cases
        || times.windows(2).any(|a| a[0] > a[1])
    {
        return invalid();
    }
    if n > 1
        && (m
            .state_variables
            .iter()
            .any(|v| shared_latent(m, v.kind) && !matches!(v.initial, InitialValue::Fixed(_)))
            || m.capabilities.iter().any(|c| {
                c.writes
                    .iter()
                    .map(|w| w.variable)
                    .chain(c.kernel_writes.iter().copied())
                    .any(|id| shared_latent(m, m.state_variables[id.0].kind))
            }))
    {
        return invalid();
    }
    if m.metrics.is_some() {
        let observations: Vec<_> = times
            .iter()
            .map(|arrival| metrics::Observation {
                condition_intervals: Vec::new(),
                admitted_at: None,
                wait_intervals: Vec::new(),
                intentional_wait: 0,
                tasks: &[],
                arrival: *arrival,
                terminal: None,
                rejected: false,
                deadlock: false,
                future: true,
            })
            .collect();
        let mut envelope = metrics::Envelope::default();
        envelope.add(metrics::project(m, &observations, 0));
        out.metric_template = envelope.finish("aborted");
    }
    out.capability_refusals = m.has_requires().then(Vec::new);
    let mut quality_observations = Vec::new();
    for replication in 1..=spec.replications {
        out.attempted_replications = replication;
        let mut random = Random {
            priority_tie: false,
            seed: spec.seed,
            replication,
            audit: Vec::new(),
            origins: Vec::new(),
        };
        let times = if let Some(d) = &m.arrival_distribution {
            let probabilities: Vec<_> = d.entries.iter().map(|(_, p)| p.clone()).collect();
            let selected = match random.pick(0, "arrival.schedule".into(), 1, &probabilities) {
                Ok(i) => i,
                Err(_) => return invalid(),
            };
            d.entries[selected].0.clone()
        } else {
            times.clone()
        };
        let mut realized = template.clone();
        if m.arrival_distribution.is_some() {
            realized.arrivals = Some(times.clone());
        }
        let m = &realized;
        let mut tree = Tree {
            walk_work: None,
            recovery_paths: Vec::new(),
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
            times: times.clone(),
            cases: (0..n)
                .map(|k| {
                    let mut i = interpreter(m, limits);
                    i.case_index = k;
                    i
                })
                .collect(),
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
        let w = World {
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
        let mut nondet = false;
        let mut observed_time = 0;
        let (w, cut, dead) = match run_rep(
            &mut tree,
            w,
            &mut random,
            &mut nondet,
            &mut observed_time,
        ) {
            Ok(x) => x,
            Err(stop) => {
                // Location text is built independently from lexical IR provenance.
                let sites: BTreeSet<_> =
                    random.origins.iter().map(|(_, s, _)| s.as_str()).collect();
                let cases: BTreeSet<_> = random.origins.iter().map(|(c, _, _)| *c).collect();
                let span = random
                    .origins
                    .iter()
                    .map(|(_, _, s)| *s)
                    .min_by_key(|s| s.start)
                    .unwrap_or(m.process.span);
                // The driver records the observed time separately below; no engine
                // diagnostics or formatting helper participates in this path.
                let at = observed_time;
                out.diagnostics = vec![diag::Diagnostic {
                    code: diag::U_SEM_0.into(),
                    severity: diag::Severity::Error,
                    span: Some(span),
                    message: if nondet {
                        let label = if random.priority_tie {
                            "unresolved simulation nondeterminism: priority_tie_remaining"
                        } else {
                            "simulation encountered unresolved nondeterminism"
                        };
                        format!(
                            "{label}: replication {replication}, time {at}, cases [{}], sites [{}]; at least two distinct successor actions remain",
                            cases
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(","),
                            sites.into_iter().collect::<Vec<_>>().join(",")
                        )
                    } else {
                        format!(
                            "simulation aborted: replication {replication}, time {at}: reference semantics"
                        )
                    },
                }];
                out.truncated_replications = None;
                return SimulationRun {
                    stop: if nondet && !m.uses_priority() && !m.uses_work_profiles() {
                        StopReason::Complete
                    } else {
                        stop
                    },
                    outcome: out,
                };
            }
        };
        if tree.observe(&w).is_err() {
            return observation_abort(out, m, replication, w.now);
        }
        let mut cases = Vec::new();
        if let Some(q) = &m.quality {
            let cut_kind = match cut {
                Some(SimulationCut::Clock) => ReferenceCut::Clock,
                Some(SimulationCut::LiveTasks) => ReferenceCut::LiveTasks,
                Some(SimulationCut::Numeric) => ReferenceCut::Numeric,
                Some(SimulationCut::Horizon) => ReferenceCut::Horizon,
                None if tree.cases.iter().any(|i| !i.statistics.ranges.is_empty()) => {
                    ReferenceCut::Range
                }
                None => ReferenceCut::None,
            };
            let mut truths = Vec::new();
            for p in &q.properties {
                let mut row = Vec::new();
                for (k, a) in times.iter().enumerate() {
                    let b = match quality_truth(m, p, &w.cases[k], k, *a, w.now, cut_kind) {
                        Ok(b) => b,
                        Err(_) => return observation_abort(out, m, replication, w.now),
                    };
                    row.push(if b.endpoints[0] == Rational::one() {
                        QueryTruth::Hit
                    } else if b.endpoints[0] != b.endpoints[1] || b.endpoints[2] != b.endpoints[3] {
                        QueryTruth::Unknown
                    } else {
                        QueryTruth::Miss
                    });
                }
                truths.push(row);
            }
            let keys = crate::quality::outcome_keys(m);
            let outcomes = (0..n)
                .map(|k| {
                    let key = match Tree::terminal_kind(&w, k) {
                        Some(Kind::End(e)) => ("end", Some(m.ends[e.0].clone())),
                        Some(Kind::Overflow(e)) => ("overflow", Some(m.ends[e.0].clone())),
                        Some(Kind::Rejected) => ("rejected", None),
                        Some(Kind::CutOff) => ("cut_off", None),
                        None if dead[k].is_some() => ("deadlock", None),
                        None if cut == Some(SimulationCut::Horizon) => ("cut_off", None),
                        _ => ("unfinished", None),
                    };
                    #[cfg(test)]
                    let key = crate::quality_impl_tests::outcome_lookup_key("reference", key);
                    keys.iter().position(|x| *x == key)
                })
                .collect::<Option<Vec<_>>>();
            let Some(outcomes) = outcomes else {
                return observation_abort(out, m, replication, w.now);
            };
            quality_observations.push(super::super::quality::Observation {
                truths,
                outcomes,
                computational: cut_kind != ReferenceCut::Horizon,
                arrivals: tree.first_observations(&w, cut_kind),
            });
        }
        for (k, arrival) in times.iter().enumerate() {
            let terminal = match Tree::terminal_kind(&w, k) {
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
            let mut violations = vec![false; m.contracts.len()];
            if let Some(c) = w.cases[k].payload() {
                let end = c.stopped(m) || dead[k].is_some();
                if tree.cases[k]
                    .inspect_trace(
                        &c.frame,
                        if cut == Some(SimulationCut::Clock) {
                            m.bounds.clock
                        } else {
                            w.now
                        },
                        matches!(cut, Some(SimulationCut::Clock | SimulationCut::Horizon)),
                        end,
                        dead[k].is_some(),
                    )
                    .is_err()
                {
                    return observation_abort(out, m, replication, w.now);
                }
                violations = tree.cases[k]
                    .contract_facts
                    .iter()
                    .map(|(_, v)| *v)
                    .collect();
            }
            let queries = m
                .probability_queries
                .iter()
                .map(|q| {
                    let truth = query_truth(
                        &w.cases[k],
                        q,
                        &QueryAt {
                            arrival: *arrival,
                            now: w.now,
                            clock: if cut == Some(SimulationCut::Horizon) {
                                w.now
                            } else {
                                m.bounds.clock
                            },
                            cut,
                            stopped: dead[k].is_some(),
                        },
                    )?;
                    Ok::<QueryTruth, StopReason>(
                        if truth == QueryTruth::Miss
                            && terminal.is_none()
                            && dead[k].is_none()
                            && tree.cases.iter().any(|i| !i.statistics.ranges.is_empty())
                            && !q.within.is_some_and(|d| w.now.saturating_sub(*arrival) > d)
                        {
                            QueryTruth::Unknown
                        } else {
                            truth
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>();
            let queries = match queries {
                Ok(q) => q,
                Err(_) => return observation_abort(out, m, replication, w.now),
            };
            cases.push(SimulationCase {
                cut_off: m
                    .horizon
                    .map(|_| cut == Some(SimulationCut::Horizon) && terminal.is_none()),
                case_id: k as u64 + 1,
                arrival: *arrival,
                terminal,
                queries,
                violations,
                deadlock: dead[k],
            });
        }
        for (yes, code, message) in [
            (
                cut == Some(SimulationCut::Numeric),
                diag::W_BUDGET_0,
                "probability denominator exceeded the exact arithmetic limit",
            ),
            (
                dead.iter().any(Option::is_some),
                diag::W_DEADLOCK_0,
                "a sampled nonterminal state has no enabled transition",
            ),
        ] {
            if yes && !out.diagnostics.iter().any(|d| d.code == code) {
                out.diagnostics.push(diag::Diagnostic {
                    code: code.into(),
                    severity: diag::Severity::Warning,
                    message: message.into(),
                    span: Some(m.process.span),
                });
            }
        }
        let mut refusals = Vec::new();
        for (k, slot) in w.cases.iter().enumerate() {
            if let CaseSlot::Here(c) = slot {
                for o in &c.frame.trace {
                    if let Occurrence::Refusal(id, value) = o.event {
                        let cap = &m.capabilities[id.0];
                        refusals.push(CapabilityRefusal {
                            case_id: k as u64 + 1,
                            call: m.site_name(o.binding.ok_or(StopReason::SemanticAbort).unwrap()),
                            capability: crate::responsibility::cap_name(m, id),
                            result: m.types[cap.return_type.0].members[value.0].clone(),
                            at: RefusalRange {
                                min: o.at,
                                max: o.at,
                            },
                            coverage: "sampled".into(),
                            observed_count: None,
                            samples: None,
                        });
                    }
                }
            }
        }
        let ranges: Vec<_> = tree
            .cases
            .iter()
            .flat_map(|i| i.statistics.ranges.clone())
            .collect();
        let range_stopped = !ranges.is_empty();
        if !ranges.is_empty() {
            out.range_stops.push((replication, ranges));
        }
        out.add_refusals(refusals);
        let response_recovery =
            m.recovery()
                .map(|_| {
                    use crate::recovery::Stop;
                    let rows: Vec<_> =
                        w.cases
                            .iter()
                            .enumerate()
                            .filter_map(|(k, slot)| {
                                slot.payload().map(|c| {
                                    (
                                        k,
                                        &c.frame,
                                        if c.end.is_some() {
                                            Stop::End
                                        } else if c.frame.trace.iter().any(|o| {
                                            matches!(o.event, Occurrence::ResponseStopped(_))
                                        }) {
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
            Err(_) => return observation_abort(out, m, replication, w.now),
        };
        out.replications.push(ReplicationOutcome {
            response_recovery,
            contests: sampled_contests(&w, m, cut, range_stopped),
            admission: m.explicit_limits().then(|| {
                let declined = w
                    .cases
                    .iter()
                    .filter(|s| matches!(s, CaseSlot::Declined))
                    .count() as u64;
                let spill = w
                    .cases
                    .iter()
                    .filter(|s| matches!(s, CaseSlot::Overflow(_)))
                    .count() as u64;
                let future = if cut.is_some() {
                    (n - w.next) as u64
                } else {
                    0
                };
                AdmissionSummary {
                    rejected_count: Range {
                        min: declined,
                        max: declined + future,
                    },
                    overflow_count: m.overflow_end.map(|_| Range {
                        min: spill,
                        max: spill + future,
                    }),
                    external_wait_max: m.external_wait().then(|| Range {
                        min: w.external_peak,
                        max: w.external_peak.max(
                            w.cases
                                .iter()
                                .filter(|s| matches!(s, CaseSlot::Held(_)))
                                .count() as u64
                                + future,
                        ),
                    }),
                }
            }),
            lost_updates: tree
                .cases
                .iter()
                .flat_map(|i| i.evidence.lost_updates.iter().copied())
                .collect(),
            scenario_observed_until: m.scenario.as_ref().map(|_| w.now),
            metrics: m.metrics.as_ref().map(|_| {
                metric_observation(
                    m,
                    &w,
                    &times,
                    cut.is_some(),
                    cut == Some(SimulationCut::Horizon),
                )
            }),
            replication,
            draws: random.audit,
            cases,
            cut,
            overcommit: tree.overcommit,
        });
        out.completed_replications = replication;
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
        .map(|_| super::super::quality::simulation(m, &quality_observations, false));
    SimulationRun {
        stop: StopReason::Complete,
        outcome: out,
    }
}

#[cfg(test)]
mod simulation_tests {
    use super::*;
    #[test]
    fn cf_reference_sample_signature_preserves_stale_and_sticky_loss() {
        let m = model("cf-version");
        let (tree, mut a) = setup(&m);
        let Stmt::Call(call) = &m.process.body[0].kind else {
            panic!()
        };
        let id = m.capabilities[0].version_check.as_ref().unwrap().variables[0];
        let mut frame = super::super::replay::initial(&m);
        tree.cases[0]
            .create(&mut frame, std::slice::from_ref(call), None)
            .unwrap();
        frame.tasks[0].phase = Phase::Running;
        frame.tasks[0].start = Some(0);
        frame.tasks[0].finish = Some(1);
        a.cases[0] = CaseSlot::Here(CaseFrame {
            frame,
            queue: vec![],
            receipt: true,
            end: None,
        });
        let mut b = a.clone();
        if let CaseSlot::Here(c) = &mut b.cases[0] {
            c.frame.tasks[0].stale.insert(id);
        }
        assert_ne!(signature(&m, &a).unwrap(), signature(&m, &b).unwrap());
        if let CaseSlot::Here(c) = &mut a.cases[0] {
            c.frame.tasks.clear();
            c.frame.waiting = None;
        }
        let mut b = a.clone();
        if let CaseSlot::Here(c) = &mut b.cases[0] {
            c.frame.lost_updates.insert(id);
        }
        assert_ne!(signature(&m, &a).unwrap(), signature(&m, &b).unwrap());
    }
    fn model(name: &str) -> Model {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        crate::syntax::parse(&std::fs::read_to_string(root.join(name).join("model.relca")).unwrap())
            .and_then(|a| crate::lower::lower(&a))
            .unwrap()
    }
    fn setup(m: &Model) -> (Tree<'_>, World<'_>) {
        let limits = Limits::default();
        let times = m.arrivals.clone().unwrap_or_else(|| vec![0]);
        let n = times.len();
        let tree = Tree {
            walk_work: None,
            recovery_paths: Vec::new(),
            response_exercise: BTreeMap::new(),
            response_receipts: Vec::new(),
            first_hits: Vec::new(),
            first_closed: Vec::new(),
            first_trees: Vec::new(),
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
            overflows: None,
            external_peaks: None,
            admissions: vec![None; n],
            unadmitted: vec![false; n],
            receipts: Vec::new(),
            duration_leaves: Vec::new(),
        };
        let w = World {
            now: 0,
            external_peak: 0,
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
        (tree, w)
    }
    fn random() -> Random {
        Random {
            priority_tie: false,
            seed: 0,
            replication: 1,
            audit: Vec::new(),
            origins: Vec::new(),
        }
    }
    #[test]
    fn percase_reference_fixed_bank_and_write_intermediate() {
        let m = model("pcase-shared-fixed");
        let (mut t, w) = setup(&m);
        let mut random = random();
        let (w, cut, dead) = run_rep(&mut t, w, &mut random, &mut false, &mut 0).unwrap();
        assert!(cut.is_none() && dead.iter().all(Option::is_none));
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
        assert_eq!(w.shared[doc], StateValue::None);
        assert_ne!(w.shared[shared], StateValue::None);
        for slot in &w.cases {
            let CaseSlot::Here(c) = slot else { panic!() };
            assert_eq!(c.frame.state_values[doc], StateValue::Value(ValueId(0)));
            assert_eq!(c.frame.state_values[shared], StateValue::None);
        }
        let mut m = model("pcase-write-ir");
        let doc = StateVarId(
            m.state_variables
                .iter()
                .position(|v| v.name == "difficulty")
                .unwrap(),
        );
        m.capabilities[0].kernel_writes.push(doc);
        for row in &mut m.capabilities[0].kernel.as_mut().unwrap().rows {
            row.branches[0]
                .overrides
                .push((doc, StateValue::Value(ValueId(1))));
        }
        let (t, mut w) = setup(&m);
        let Stmt::Call(call) = &m.process.body[0].kind else {
            panic!()
        };
        for slot in &mut w.cases {
            *slot = CaseSlot::Here(CaseFrame {
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
                    now: 1,
                    steps: 0,
                    control: vec![],
                    waiting: None,
                    tasks: vec![Task {
                        frozen_performance: None,
                        stale: BTreeSet::new(),
                        metric_ready: None,
                        checked_through: None,
                        person: None,
                        call: call.clone(),
                        phase: Phase::Running,
                        start: Some(0),
                        stop: None,
                        finish: Some(1),
                        ack: None,
                    }],
                    adoption: None,
                    trace: vec![],
                },
                queue: vec![],
                receipt: true,
                end: None,
            });
        }
        w.now = 1;
        let branch = &m.capabilities[0].kernel.as_ref().unwrap().rows[0].branches[0];
        let next = finished(&t, &w, 0, 0, branch.result, &branch.overrides);
        assert_eq!(t.values(&next, 0)[doc.0], StateValue::Value(ValueId(1)));
        assert_eq!(t.values(&next, 1)[doc.0], StateValue::Value(ValueId(0)));
        assert_eq!(next.shared[doc.0], StateValue::None);
        let next = finished(&t, &next, 1, 0, branch.result, &branch.overrides);
        assert_eq!(t.values(&next, 0)[doc.0], StateValue::Value(ValueId(1)));
        assert_eq!(t.values(&next, 1)[doc.0], StateValue::Value(ValueId(1)));
    }
    #[test]
    fn percase_reference_cancel_ack_does_not_apply_override() {
        let mut m = model("pcase-write-ir");
        let doc = StateVarId(
            m.state_variables
                .iter()
                .position(|v| v.name == "difficulty")
                .unwrap(),
        );
        m.capabilities[0].kernel_writes.push(doc);
        for row in &mut m.capabilities[0].kernel.as_mut().unwrap().rows {
            row.branches[0]
                .overrides
                .push((doc, StateValue::Value(ValueId(1))));
        }
        let (mut t, mut w) = setup(&m);
        w.next = 2;
        w.now = 1;
        w.boundary = Boundary::Run;
        let Stmt::Call(call) = &m.process.body[0].kind else {
            panic!()
        };
        let mut f = super::super::super::replay::initial(&m);
        f.now = 1;
        f.control = vec![&m.process.body[1..]];
        f.tasks = vec![Task {
            frozen_performance: None,
            stale: BTreeSet::new(),
            metric_ready: None,
            checked_through: None,
            person: None,
            call: call.clone(),
            phase: Phase::Running,
            start: Some(0),
            stop: None,
            finish: Some(5),
            ack: Some(1),
        }];
        f.waiting = Some(Waiting {
            tasks: 0..1,
            join: Some(&Join::All),
            winner: None,
        });
        w.cases = vec![
            CaseSlot::Here(CaseFrame {
                frame: f,
                queue: vec![],
                receipt: true,
                end: None,
            }),
            CaseSlot::Declined,
        ];
        let (w, cut, _) = run_rep(&mut t, w, &mut random(), &mut false, &mut 0).unwrap();
        assert!(cut.is_none());
        assert_eq!(t.values(&w, 0)[doc.0], StateValue::Value(ValueId(0)));
        let CaseSlot::Here(c) = &w.cases[0] else {
            panic!()
        };
        assert!(c.frame.tasks.iter().any(|t| t.phase == Phase::Cancelled));
    }
    #[test]
    fn s_reference_witness_is_selected_path_only_and_earliest_replication() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let source = std::fs::read_to_string(root.join("sim-keys/model.relca"))
            .unwrap()
            .replace(
                "  bounds",
                "  contract C { invariant A.c.finish -> y == second }\n  bounds",
            );
        let m = crate::syntax::parse(&source)
            .and_then(|a| crate::lower::lower(&a))
            .unwrap();
        let run = simulate(&m);
        assert_eq!(run.stop, StopReason::Complete);
        assert_eq!(run.outcome.completed_replications, 4);
        assert_eq!(
            run.outcome
                .replications
                .iter()
                .map(|r| r.cases[0].violations[0])
                .collect::<Vec<_>>(),
            [false, false, true, true]
        );
        assert_eq!(
            run.outcome
                .replications
                .iter()
                .find(|r| r.cases[0].violations[0])
                .unwrap()
                .replication,
            3
        );
    }
    fn active(m: &Model) -> CaseFrame<'_> {
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
                now: 0,
                steps: 0,
                control: Vec::new(),
                waiting: None,
                tasks: Vec::new(),
                adoption: None,
                trace: Vec::new(),
            },
            queue: Vec::new(),
            receipt: true,
            end: None,
        }
    }

    #[test]
    fn req_signature_distinguishes_sleep_deadlines_and_refused_values() {
        let m = model("sim-contract");
        let (_, mut w) = setup(&m);
        let mut c = active(&m);
        c.frame.trace.push(Observation {
            binding: None,
            state_values: c.frame.state_values.clone(),
            person: None,
            at: 0,
            event: Occurrence::Wait(2),
            values: vec![None; m.process.bindings.len()],
        });
        w.cases[0] = CaseSlot::Here(c.clone());
        let mut later = w.clone();
        if let CaseSlot::Here(c) = &mut later.cases[0] {
            c.frame.trace[0].event = Occurrence::Wait(3);
        }
        assert_ne!(signature(&m, &w).unwrap(), signature(&m, &later).unwrap());
        c.frame.trace.clear();
        c.frame.tasks.push(Task {
            frozen_performance: None,
            stale: BTreeSet::new(),
            metric_ready: None,
            checked_through: None,
            person: None,
            call: Call {
                args: Vec::new(),
                binding: BindingId(0),
                capability: CapabilityId(0),
                span: m.process.span,
            },
            phase: Phase::Refused(ValueId(0)),
            start: None,
            stop: Some(0),
            finish: None,
            ack: None,
        });
        w.cases[0] = CaseSlot::Here(c);
        let mut other = w.clone();
        if let CaseSlot::Here(c) = &mut other.cases[0] {
            c.frame.tasks[0].phase = Phase::Refused(ValueId(1));
        }
        assert_ne!(signature(&m, &w).unwrap(), signature(&m, &other).unwrap());
    }

    #[test]
    fn s_reference_key_boundary_and_signature_dedup() {
        let p = |s| Rational::decimal(s).unwrap();
        let k = BigInt::from(1) << 63;
        assert_eq!(select_integer(&k, &[p("0.5"), p("0.5")]).unwrap(), 1);
        assert_eq!(
            select_integer(
                &k,
                &[p("0.50000000000000000001"), p("0.49999999999999999999")]
            )
            .unwrap(),
            0
        );
        assert_eq!(select_integer(&0.into(), &[p("0"), p("1")]).unwrap(), 1);
        assert_eq!(
            select_integer(&BigInt::from(u64::MAX), &[p("0.5"), p("0.5")]).unwrap(),
            1
        );
        let m = model("sim-contract");
        let (_, w) = setup(&m);
        fn one(w: World<'_>) -> Action<'_> {
            (None, vec![(Rational::one(), w)])
        }
        assert!(same_action(&m, &one(w.clone()), &one(w.clone())));
        let mut t = w.clone();
        t.next = 1;
        assert!(!same_action(&m, &one(w.clone()), &one(t.clone())));
        let mut phase = w.clone();
        phase.boundary = Boundary::Admit;
        assert!(!same_action(&m, &one(w.clone()), &one(phase)));
        let a = (
            Some((1, "kernel.P.a".into())),
            vec![
                (p("0.2"), w.clone()),
                (p("0.3"), w.clone()),
                (p("0.5"), t.clone()),
            ],
        );
        let mut b = (
            Some((1, "kernel.P.a".into())),
            vec![(p("0.5"), w.clone()), (p("0.5"), t.clone())],
        );
        assert!(same_action(&m, &a, &b));
        b.0 = Some((1, "kernel.P.b".into()));
        assert!(!same_action(&m, &a, &b));
        b.0 = a.0.clone();
        b.1.reverse();
        assert!(!same_action(&m, &a, &b));
        let mut candidate = None;
        let mut nondet = false;
        keep(&m, &mut candidate, one(w.clone()), &mut nondet).unwrap();
        keep(&m, &mut candidate, one(w.clone()), &mut nondet).unwrap();
        assert!(keep(&m, &mut candidate, one(t), &mut nondet).is_err());
        let (tree, mut s) = setup(&m);
        s.cases[0] = CaseSlot::Here(active(&m));
        s.next = 1;
        s.boundary = Boundary::Run;
        let mut t = s.clone();
        append(
            &tree,
            &mut t,
            0,
            Occurrence::Completion(CapabilityId(0), ValueId(0)),
            None,
        );
        assert!(!same_action(&m, &one(s), &one(t)));
        let m = model("exec-authority");
        let (_, mut s) = setup(&m);
        let mut c = active(&m);
        let Stmt::Call(call) = &m.process.body[0].kind else {
            unreachable!()
        };
        create(&mut c, std::slice::from_ref(call), None, 0, false);
        c.frame.tasks[0].person = Some(PersonId(0));
        c.frame.tasks[0].phase = Phase::Running;
        c.frame.tasks[0].start = Some(0);
        c.frame.tasks[0].finish = Some(1);
        s.cases[0] = CaseSlot::Here(c);
        let mut t = s.clone();
        if let CaseSlot::Here(c) = &mut t.cases[0] {
            c.frame.tasks[0].person = Some(PersonId(1));
        }
        assert!(!same_action(&m, &one(s), &one(t)));
    }

    #[test]
    fn s_reference_hundred_initial_factors_order_and_no_joint_denominator() {
        let declarations = (0..100)
            .map(|i| {
                format!(
                    "z{i}:T ~ {{first:0.{},second:0.{}1}}",
                    "9".repeat(63),
                    "0".repeat(62)
                )
            })
            .collect::<Vec<_>>()
            .join(";\n");
        let src = format!(
            "model R version \"0.2\" {{type T=enum{{first,second}} latent case {{{declarations}}} process P {{end done}} bounds {{cases 1;live_tasks 1;clock 1min}} verify reachable end.done\nsimulation replications 1 seed 0 confidence 0.5\n}}"
        );
        let mut m = crate::syntax::parse(&src)
            .and_then(|a| crate::lower::lower(&a))
            .unwrap();
        let mut signatures = Vec::new();
        for reverse in [false, true] {
            if reverse {
                m.state_variables.reverse();
            }
            let (mut tree, w) = setup(&m);
            let mut random = random();
            let (w, cut, _) = run_rep(&mut tree, w, &mut random, &mut false, &mut 0).unwrap();
            assert!(cut.is_none());
            assert_eq!(random.audit.len(), 100);
            let CaseSlot::Here(c) = &w.cases[0] else {
                unreachable!()
            };
            let values: BTreeMap<_, _> = m
                .state_variables
                .iter()
                .zip(&c.frame.state_values)
                .map(|(v, x)| (v.name.clone(), *x))
                .collect();
            for v in &m.state_variables {
                let key = format!(
                    "relca-sim-v1|seed=0|rep=1|case=1|site=init.case.{}|n=1",
                    v.name
                );
                let digest = Sha256::digest(key.as_bytes());
                let integer = BigInt::from(u64::from_be_bytes(digest[..8].try_into().unwrap()));
                let d = BigInt::from(10).pow(63);
                let expected = if integer * d.clone() < (&d - 1) * (BigInt::from(1) << 64) {
                    StateValue::Value(ValueId(0))
                } else {
                    StateValue::Value(ValueId(1))
                };
                assert_eq!(values[&v.name], expected);
                assert!(random.audit.iter().any(|a| a.key == key));
            }
            signatures.push(values);
        }
        assert_eq!(signatures[0], signatures[1]);
    }
    #[test]
    fn s_reference_hazard_task_order_revisit_occurrence_and_cancel() {
        let mut m = model("sim-hazard");
        m.process.bindings.push(Binding {
            merge: None,
            repetition: None,
            name: "b".into(),
            ty: m.process.bindings[0].ty,
            span: m.process.span,
        });
        let Stmt::Call(a) = &m.process.body[0].kind else {
            unreachable!()
        };
        let mut b = a.clone();
        b.binding = BindingId(1);
        let mut tuples = Vec::new();
        for reverse in [false, true] {
            let (tree, mut w) = setup(&m);
            w.now = 1;
            w.next = 1;
            let mut c = active(&m);
            create(&mut c, &[a.clone(), b.clone()], None, 0, false);
            for t in &mut c.frame.tasks {
                t.phase = Phase::Running;
                t.start = Some(0);
            }
            if reverse {
                c.frame.tasks.reverse();
            }
            w.cases[0] = CaseSlot::Here(c);
            let mut random = random();
            assert!(sample_durations(&tree, &mut w, &mut random).unwrap());
            assert_eq!(random.audit.len(), 2);
            let CaseSlot::Here(c) = &w.cases[0] else {
                unreachable!()
            };
            let tuple: BTreeMap<_, _> = c
                .frame
                .tasks
                .iter()
                .map(|t| (t.call.binding, (t.finish, t.checked_through)))
                .collect();
            for t in &c.frame.tasks {
                let key = format!(
                    "relca-sim-v1|seed=0|rep=1|case=1|site=duration.P.{}|n=1",
                    m.process.bindings[t.call.binding.0].name
                );
                let digest = Sha256::digest(key.as_bytes());
                assert_eq!(t.finish, (digest[0] < 128).then_some(1));
                assert_eq!(t.checked_through, Some(1));
            }
            let survivors = c.frame.tasks.iter().filter(|t| t.finish.is_none()).count();
            assert!(sample_durations(&tree, &mut w, &mut random).unwrap());
            assert_eq!(random.audit.len(), 2);
            w.now = 3;
            assert!(sample_durations(&tree, &mut w, &mut random).unwrap());
            assert_eq!(random.audit.len(), 2 + survivors);
            for draw in random.audit.iter().skip(2) {
                assert!(draw.key.ends_with("|n=2"));
            }
            if let CaseSlot::Here(c) = &mut w.cases[0] {
                for t in &mut c.frame.tasks {
                    t.phase = Phase::Cancelled;
                    t.finish = None;
                }
            }
            let count = random.audit.len();
            assert!(sample_durations(&tree, &mut w, &mut random).unwrap());
            assert_eq!(random.audit.len(), count);
            tuples.push(tuple);
        }
        assert_eq!(tuples[0], tuples[1]);
    }
    #[test]
    fn s_reference_cancelled_kernel_no_draw_from_post_ack_trace() {
        let m = model("prob-cancel");
        let (mut tree, mut w) = setup(&m);
        let Stmt::Fork { branches, join, .. } = &m.process.body[0].kind else {
            unreachable!()
        };
        let mut c = active(&m);
        create(&mut c, branches, Some(join), 0, false);
        c.frame.control = vec![&m.process.body[1..]];
        c.frame.now = 2;
        c.frame.adoption = Some(1);
        c.frame.waiting.as_mut().unwrap().winner = Some(branches[0].binding);
        for (i, t) in c.frame.tasks.iter_mut().enumerate() {
            t.phase = if i == 0 {
                Phase::Finished(ValueId(0))
            } else {
                Phase::Cancelled
            };
            t.start = Some(0);
            t.stop = Some(if i == 0 { 1 } else { 2 });
            t.finish = Some(if i == 0 { 1 } else { 5 });
        }
        let empty = vec![None; m.process.bindings.len()];
        let mut completed = empty.clone();
        completed[branches[0].binding.0] = Some(ValueId(0));
        for (at, event, values) in [
            (0, Occurrence::Receipt, empty.clone()),
            (
                0,
                Occurrence::Dispatch(branches[0].capability),
                empty.clone(),
            ),
            (0, Occurrence::Dispatch(branches[1].capability), empty),
            (
                1,
                Occurrence::Completion(branches[0].capability, ValueId(0)),
                completed,
            ),
        ] {
            c.frame.trace.push(Observation {
                binding: None,
                state_values: c.frame.state_values.clone(),
                person: None,
                at,
                event,
                values,
            });
        }
        w.cases[0] = CaseSlot::Here(c);
        w.now = 2;
        w.next = 1;
        w.boundary = Boundary::Run;
        let mut random = random();
        let (w, cut, dead) = run_rep(&mut tree, w, &mut random, &mut false, &mut 0).unwrap();
        assert!(cut.is_none());
        assert!(dead.iter().all(Option::is_none));
        assert!(random.audit.is_empty());
        let CaseSlot::Here(c) = &w.cases[0] else {
            unreachable!()
        };
        assert!(c.end.is_some());
        let InitialValue::Fixed(initial) = m.state_variables[0].initial else {
            unreachable!()
        };
        assert_eq!(c.frame.state_values[0], initial);
    }
    #[test]
    fn s_reference_numeric_unknown_known_hit_and_overcommit() {
        let mut m = model("sim-hazard");
        let p = Rational(num_rational::BigRational::new(
            1.into(),
            (BigInt::from(1) << 4096usize) + 1,
        ));
        m.capabilities[0]
            .duration_distribution
            .as_mut()
            .unwrap()
            .entries = vec![(1, p.clone()), (3, Rational(Rational::one().0 - p.0))];
        m.probability_queries[0].within = Some(0);
        let out = simulate(&m);
        assert_eq!(out.stop, StopReason::Complete);
        for r in &out.outcome.replications {
            assert_eq!(r.cut, Some(SimulationCut::Numeric));
            assert_eq!(r.cases[0].queries, [QueryTruth::Unknown]);
        }
        let ProbabilityCondition::State(Cond::StateEq(target)) = m.probability_queries[0].condition
        else {
            unreachable!()
        };
        m.state_variables[0].initial = InitialValue::Fixed(target.value);
        for r in simulate(&m).outcome.replications {
            assert_eq!(r.cases[0].queries, [QueryTruth::Hit]);
        }
        let mut m = model("sim-hazard");
        m.actors[0].capacity = Some(0);
        let (mut tree, mut w) = setup(&m);
        let mut c = active(&m);
        let Stmt::Call(call) = &m.process.body[0].kind else {
            unreachable!()
        };
        create(&mut c, std::slice::from_ref(call), None, 0, false);
        c.frame.tasks[0].phase = Phase::Running;
        c.frame.tasks[0].start = Some(0);
        w.cases[0] = CaseSlot::Here(c);
        tree.observe(&w).unwrap();
        assert!(tree.overcommit);
        let at = QueryAt {
            arrival: 0,
            now: 99,
            clock: 100,
            cut: Some(SimulationCut::Numeric),
            stopped: false,
        };
        let q = &m.probability_queries[0];
        assert_eq!(
            query_truth(&CaseSlot::NotHere, q, &at).unwrap(),
            QueryTruth::Unknown
        );
        assert_eq!(
            query_truth(&CaseSlot::Declined, q, &at).unwrap(),
            QueryTruth::Miss
        );
        let mut c = active(&m);
        c.end = Some(EndId(0));
        assert_eq!(
            query_truth(&CaseSlot::Here(c), q, &at).unwrap(),
            QueryTruth::Miss
        );
    }
}
