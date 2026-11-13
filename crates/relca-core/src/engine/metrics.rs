//! Opt-in, bounded, path-local instrumentation. No random choices or transitions.
use super::{Lifecycle, State};
use crate::ir;
mod calendar;
mod tariff;
mod waits;
#[cfg(test)]
mod waits_tests;

pub(super) fn validate(m: &ir::Model) -> Result<(), &'static str> {
    let Some(spec) = &m.metrics else {
        return Ok(());
    };
    if spec.window == 0
        || spec.window < m.bounds.clock
        || spec.currency.trim().is_empty()
        || spec.actor_rates.len() != m.actors.len()
        || spec.start_rates.len() != m.capabilities.len()
    {
        return Err("invalid metrics declaration");
    }
    fn visit(b: &ir::Block, caps: &mut std::collections::BTreeSet<usize>) {
        for s in b {
            match &s.kind {
                ir::Stmt::LoopControl(_) => {}
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => {
                    caps.insert(c.capability.0);
                }
                ir::Stmt::Response(c, r) => {
                    caps.insert(c.capability.0);
                    if r.attempt == 1
                        && let Some(b) = &r.recovery
                    {
                        visit(b, caps);
                    }
                }
                ir::Stmt::Fork { branches, .. } => {
                    caps.extend(branches.iter().map(|c| c.capability.0))
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    visit(then_body, caps);
                    visit(else_body, caps);
                }
                ir::Stmt::End(_)
                | ir::Stmt::Wait(_)
                | ir::Stmt::WaitUntil(_)
                | ir::Stmt::ResolveEffect { .. }
                | ir::Stmt::StopReceiptUnknown => {}
            }
        }
    }
    let mut caps = std::collections::BTreeSet::new();
    visit(&m.process.body, &mut caps);
    fn has_end(b: &ir::Block, end: ir::EndId) -> bool {
        b.iter().any(|s| match &s.kind {
            ir::Stmt::Response(_, r) => r.exhausted == Some(end),
            ir::Stmt::End(e) => *e == end,
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => has_end(then_body, end) || has_end(else_body, end),
            ir::Stmt::Fork {
                join: ir::Join::Any { if_none, .. },
                ..
            } => *if_none == end,
            _ => false,
        })
    }
    caps.extend(
        m.contests()
            .iter()
            .filter(|c| match c.target {
                crate::contest::ContestTarget::End(e) => has_end(&m.process.body, e),
                _ => c.target.active(m),
            })
            .map(|c| c.route.0),
    );
    let mut actors = std::collections::BTreeSet::new();
    for id in &caps {
        let cap = m
            .capabilities
            .get(*id)
            .ok_or("invalid metrics capability index")?;
        if cap.actor.0 >= m.actors.len() {
            return Err("invalid metrics Actor index");
        }
        actors.insert(cap.actor.0);
    }
    if spec
        .required_capabilities
        .iter()
        .map(|c| c.0)
        .collect::<Vec<_>>()
        != caps.into_iter().collect::<Vec<_>>()
        || spec.required_actors.iter().map(|a| a.0).collect::<Vec<_>>()
            != actors.into_iter().collect::<Vec<_>>()
    {
        return Err("metrics required fees differ from lexical calls");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Status {
    Refused,
    NeverCreated,
    Queued,
    Running,
    Finished,
    CancelledBeforeStart,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TaskHistory {
    pub status: Status,
    pub ready: Option<u64>,
    pub start: Option<u64>,
    pub stop: Option<u64>,
    pub executor: Option<ir::PersonId>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Peak {
    pub tasks: u64,
    pub actors: Vec<u64>,
    pub people: Vec<u64>,
    pub resources: Vec<num_bigint::BigInt>,
}

impl std::hash::Hash for Peak {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.tasks.hash(h);
        self.actors.hash(h);
        self.people.hash(h);
        if !self.resources.is_empty() {
            self.resources.hash(h);
        }
    }
}
impl std::fmt::Debug for Peak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("Peak");
        d.field("tasks", &self.tasks)
            .field("actors", &self.actors)
            .field("people", &self.people);
        if !self.resources.is_empty() {
            d.field("resources", &self.resources);
        }
        d.finish()
    }
}

impl Peak {
    pub fn new(m: &ir::Model) -> Self {
        Self {
            tasks: 0,
            actors: vec![0; m.actors.len()],
            people: vec![0; m.people.len()],
            resources: vec![num_bigint::BigInt::from(0); m.resources.len()],
        }
    }

    /// Occupancy is half-open: a task whose occupancy ends at the state's
    /// time no longer counts there, so a same-time start ordered before that
    /// completion is not a concurrent task. An instantaneous task still counts
    /// at its own start (ADR 0019 addenda U8 and 2).
    pub fn observe<'a>(
        &mut self,
        states: impl Iterator<Item = (&'a State, &'a [TaskHistory])>,
        m: &ir::Model,
    ) {
        let mut actors = vec![0; m.actors.len()];
        let mut people = vec![0; m.people.len()];
        let mut resources = vec![num_bigint::BigInt::from(0); m.resources.len()];
        let mut total = 0;
        for (s, history) in states {
            for t in &s.tasks {
                let ending = match t.lifecycle {
                    Lifecycle::Running { finish, ack } => {
                        ack.map_or(finish, |a| a.min(finish)) <= s.time
                    }
                    Lifecycle::DistributedRunning { due, ack, .. } => due || ack == Some(s.time),
                    _ => false,
                };
                let instant = history[t.binding.0].start == Some(s.time);
                if t.lifecycle.running() && (!ending || instant) {
                    total += 1;
                    actors[m.capabilities[t.capability.0].actor.0] += 1;
                    for u in &m.capabilities[t.capability.0].uses {
                        resources[u.resource.0] += num_bigint::BigInt::from(u.amount);
                    }
                    if let Some(p) = t.executor {
                        people[p.0] += 1;
                    }
                }
            }
        }
        self.tasks = self.tasks.max(total);
        for (peak, n) in self.resources.iter_mut().zip(resources) {
            if n > *peak {
                *peak = n;
            }
        }
        for (peak, n) in self.actors.iter_mut().zip(actors) {
            *peak = (*peak).max(n);
        }
        for (peak, n) in self.people.iter_mut().zip(people) {
            *peak = (*peak).max(n);
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct History {
    pub conditions: Option<Vec<(u64, Option<u64>)>>,
    pub waits: Option<Vec<(u64, u64)>>,
    pub tasks: Vec<TaskHistory>,
    pub peak: Peak,
}

impl std::hash::Hash for History {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.tasks.hash(h);
        self.peak.hash(h);
        if let Some(waits) = &self.conditions {
            waits.hash(h);
        }
        if let Some(waits) = &self.waits {
            waits.hash(h);
        }
    }
}
impl std::fmt::Debug for History {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("History");
        d.field("tasks", &self.tasks).field("peak", &self.peak);
        if let Some(waits) = &self.conditions {
            d.field("conditions", waits);
        }
        if let Some(waits) = &self.waits {
            d.field("waits", waits);
        }
        d.finish()
    }
}

impl History {
    pub fn new(m: &ir::Model) -> Self {
        Self {
            conditions: m.has_condition_wait().then(Vec::new),
            waits: m.has_metric_wait().then(Vec::new),
            tasks: vec![
                TaskHistory {
                    status: Status::NeverCreated,
                    ready: None,
                    start: None,
                    stop: None,
                    executor: None,
                };
                m.process.bindings.len() + m.contests().len()
            ],
            peak: Peak::new(m),
        }
    }
}

/// Called at actual lifecycle seams, including zero-time running states.
pub(super) fn observe(s: &mut State, m: &ir::Model) {
    // Rates expose closure even when metrics are undeclared. Record it at the
    // same real Task lifecycle seam, before the optional metric-history gate.
    if let Some(contest) = &mut s.contest {
        for task in s.tasks.iter().filter(|t| t.lifecycle == Lifecycle::Done) {
            if let Some(id) = m.site_contest(task.binding)
                && m.contests()[id].rates.is_some()
                && contest.routes[id].response_at.is_none()
            {
                contest.routes[id].failure_at.get_or_insert(s.time);
            }
        }
    }
    let Some(mut history) = s.metric_history.take() else {
        return;
    };
    for t in &s.tasks {
        let h = &mut history.tasks[t.binding.0];
        match t.lifecycle {
            Lifecycle::Queued => {
                if h.status == Status::NeverCreated {
                    h.ready = Some(s.time);
                    h.status = Status::Queued;
                }
            }
            lifecycle if lifecycle.running() => {
                if h.start.is_none() {
                    h.start = Some(s.time);
                }
                h.executor = t.executor;
                h.status = Status::Running;
            }
            Lifecycle::Done
                if !matches!(
                    h.status,
                    Status::Finished
                        | Status::CancelledBeforeStart
                        | Status::Cancelled
                        | Status::Refused
                ) =>
            {
                h.stop = Some(s.time);
                h.status = if h.start.is_none()
                    && s.results.get(t.binding.0).is_some_and(|r| r.is_some())
                {
                    Status::Refused
                } else if h.start.is_none() {
                    Status::CancelledBeforeStart
                } else if s.results.get(t.binding.0).is_some_and(|r| r.is_some())
                    || m.site_contest(t.binding).is_some_and(|id| {
                        s.contest
                            .as_ref()
                            .is_some_and(|c| c.routes[id].response_at.is_some())
                    })
                {
                    Status::Finished
                } else {
                    Status::Cancelled
                };
            }
            _ => {}
        }
    }
    let History { tasks, peak, .. } = &mut history;
    peak.observe(std::iter::once((&*s, &tasks[..])), m);
    s.metric_history = Some(history);
}

pub(super) struct CaseObservation<'a> {
    pub admitted_at: Option<u64>,
    pub state: Option<&'a State>,
    pub arrival: u64,
    pub end: Option<ir::EndId>,
    pub rejected: bool,
    pub deadlocked: bool,
    pub not_arrived: bool,
}

pub(super) fn merge(
    accumulated: &mut Option<crate::metrics::BusinessMetrics>,
    point: crate::metrics::BusinessMetrics,
) {
    use serde_json::Value;
    point.assert_wait_populations();
    fn merge_value(a: &mut Value, b: &Value) {
        match (a, b) {
            (Value::Object(a), Value::Object(b))
                if a.contains_key("min") && a.contains_key("max") =>
            {
                if b["min"].is_null() {
                    return;
                }
                if a["min"].is_null() {
                    *a = b.clone();
                    return;
                }
                let number = |v: &Value| {
                    serde_json::from_value::<crate::metrics::MetricNumber>(v.clone())
                        .expect("metric scalar")
                };
                if number(&b["min"]) < number(&a["min"]) {
                    a.insert("min".into(), b["min"].clone());
                }
                if number(&b["max"]) > number(&a["max"]) {
                    a.insert("max".into(), b["max"].clone());
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                for (key, value) in a.iter_mut() {
                    if let Some(other) = b.get(key) {
                        merge_value(value, other);
                    }
                }
            }
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len());
                for (a, b) in a.iter_mut().zip(b) {
                    merge_value(a, b);
                }
            }
            _ => {}
        }
    }
    if let Some(old) = accumulated {
        old.union_wait_presence(&point);
        let mut value = serde_json::to_value(&*old).expect("metric wire");
        merge_value(
            &mut value,
            &serde_json::to_value(point).expect("metric wire"),
        );
        *old = serde_json::from_value(value).expect("merged metrics");
        old.align_wait_reasons();
    } else {
        *accumulated = Some(point);
    }
}

pub(super) fn finish(
    value: Option<crate::metrics::BusinessMetrics>,
    m: &ir::Model,
    coverage: crate::obligation::Coverage,
) -> Option<crate::metrics::BusinessMetrics> {
    m.metrics.as_ref()?;
    if m.bounds.cases == 0
        || m.arrivals
            .as_ref()
            .is_some_and(|a| a.len() as u64 != m.bounds.cases)
        || m.bounds.cases != 1 && m.arrivals.is_none()
    {
        return None;
    }
    if super::validate(m).is_err() {
        return None;
    }
    let mut value = value.unwrap_or_else(|| {
        let state = State::initial(m, 0);
        let cases: Vec<_> = (0..m.bounds.cases as usize)
            .map(|i| CaseObservation {
                admitted_at: Some(0),
                state: Some(&state),
                arrival: m.arrivals.as_ref().map_or(0, |a| a[i]),
                end: None,
                rejected: false,
                deadlocked: false,
                not_arrived: false,
            })
            .collect();
        snapshot(m, &cases, &Peak::new(m), 0)
    });
    value.assert_wait_populations();
    if m.explicit_limits() {
        let zero = || crate::metrics::Count {
            stats: crate::metrics::Scalar::Range(crate::metrics::Range {
                min: Some(0u64.into()),
                max: Some(0u64.into()),
                reason: None,
            }),
            observed_total: None,
        };
        value.overall.counts.admitted.get_or_insert_with(zero);
        if m.overflow_end.is_some() {
            value.overall.counts.overflowed.get_or_insert_with(zero);
        }
        for c in &mut value.cases {
            c.counts.admitted.get_or_insert_with(zero);
            if m.overflow_end.is_some() {
                c.counts.overflowed.get_or_insert_with(zero);
            }
            if m.external_wait() {
                c.external_wait.get_or_insert({
                    crate::metrics::Scalar::Range(crate::metrics::Range {
                        min: None,
                        max: None,
                        reason: Some(crate::metrics::Reason::Unfinished),
                    })
                });
            }
        }
    }
    if coverage == crate::obligation::Coverage::Complete {
        return Some(value);
    }
    let mut value = serde_json::to_value(value).expect("metric wire");
    fn invalidate(value: &mut serde_json::Value, reason: &str) {
        match value {
            serde_json::Value::Object(fields)
                if fields.contains_key("min") && fields.contains_key("max") =>
            {
                fields.insert("min".into(), serde_json::Value::Null);
                fields.insert("max".into(), serde_json::Value::Null);
                fields.insert("reason".into(), reason.into());
            }
            serde_json::Value::Object(fields) => {
                for v in fields.values_mut() {
                    invalidate(v, reason);
                }
            }
            serde_json::Value::Array(values) => {
                for v in values {
                    invalidate(v, reason);
                }
            }
            _ => {}
        }
    }
    let reason = coverage.word();
    let mut typed: crate::metrics::BusinessMetrics =
        serde_json::from_value(value).expect("metric wire");
    typed.withdraw_wait_bounds();
    value = serde_json::to_value(typed).expect("metric wire");
    invalidate(&mut value, reason);
    value["status"] = reason.into();
    Some(serde_json::from_value(value).expect("invalidated metrics"))
}

/// A leaf is projected before any extrema are merged, preserving correlations.
pub(super) fn snapshot(
    m: &ir::Model,
    cases: &[CaseObservation<'_>],
    peak: &Peak,
    until: u64,
) -> crate::metrics::BusinessMetrics {
    snapshot_with_horizon(m, cases, peak, until, false)
}

pub(super) fn snapshot_with_horizon(
    m: &ir::Model,
    cases: &[CaseObservation<'_>],
    peak: &Peak,
    until: u64,
    horizon: bool,
) -> crate::metrics::BusinessMetrics {
    use crate::metrics as wire;
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use serde_json::{Value, json};
    fn scalar(x: BigRational) -> Value {
        json!({"min": x.to_string(), "max": x.to_string(), "reason": null})
    }
    fn integer(x: u64) -> Value {
        scalar(BigRational::from_integer(BigInt::from(x)))
    }
    fn null(reason: &str) -> Value {
        json!({"min":null,"max":null,"reason":reason})
    }
    fn count(x: u64) -> Value {
        json!({"stats":integer(x),"observed_total":null})
    }
    fn ratio(n: &BigInt, d: BigInt) -> Value {
        scalar(BigRational::new(n.clone(), d))
    }
    fn money(work: &BigInt, rate: Option<u64>) -> Value {
        rate.map_or_else(
            || null("not_configured"),
            |r| scalar(BigRational::from_integer(work * BigInt::from(r))),
        )
    }
    fn inventory<'a>(b: &'a ir::Block, calls: &mut Vec<&'a ir::Call>, ends: &mut Vec<ir::EndId>) {
        for s in b {
            match &s.kind {
                ir::Stmt::LoopControl(_) | ir::Stmt::Wait(_) | ir::Stmt::WaitUntil(_) => {}
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => calls.push(c),
                ir::Stmt::Response(c, r) => {
                    calls.push(c);
                    if r.attempt == 1
                        && let Some(b) = &r.recovery
                    {
                        inventory(b, calls, ends);
                    }
                }
                ir::Stmt::ResolveEffect { .. } | ir::Stmt::StopReceiptUnknown => {}
                ir::Stmt::Fork { branches, join, .. } => {
                    calls.extend(branches);
                    if let ir::Join::Any { if_none, .. } = join
                        && !ends.contains(if_none)
                    {
                        ends.push(*if_none);
                    }
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    inventory(then_body, calls, ends);
                    inventory(else_body, calls, ends);
                }
                ir::Stmt::End(e) => {
                    if !ends.contains(e) {
                        ends.push(*e);
                    }
                }
            }
        }
    }
    let spec = m.metrics.as_ref().expect("enabled collector");
    let mut calls = Vec::new();
    let mut ends = Vec::new();
    inventory(&m.process.body, &mut calls, &mut ends);
    calls.sort_by_key(|c| c.binding.0);
    if let Some(end) = m.overflow_end {
        ends.push(end);
    }
    let cap_name = |id: usize| {
        format!(
            "{}.{}",
            m.actors[m.capabilities[id].actor.0].name, m.capabilities[id].name
        )
    };
    let configured_actor = spec
        .required_actors
        .iter()
        .all(|id| spec.actor_rates[id.0].is_some());
    let configured_start = spec
        .required_capabilities
        .iter()
        .all(|id| spec.start_rates[id.0].is_some());
    let configured_business = calls.iter().all(|c| {
        spec.actor_rates[m.capabilities[c.capability.0].actor.0].is_some()
            && spec.start_rates[c.capability.0].is_some()
    });
    let mut actor_work = vec![BigInt::from(0); m.actors.len()];
    let mut person_work = vec![BigInt::from(0); m.people.len()];
    let mut resource_work = vec![BigInt::from(0); m.resources.len()];
    let mut starts = vec![0u64; m.capabilities.len()];
    let mut case_rows = Vec::new();
    let mut case_costs = Vec::new();
    let mut end_counts = vec![0u64; ends.len()];
    let mut end_costs = vec![BigInt::from(0); ends.len()];
    let (mut rejected, mut unfinished, mut deadlocked, mut not_arrived) = (0u64, 0u64, 0u64, 0u64);
    let mut cut_off = 0u64;
    for (index, case) in cases.iter().enumerate() {
        let overflow = case.end.is_some() && case.end == m.overflow_end;
        let history = case.state.and_then(|s| s.metric_history.as_ref());
        let mut work = BigInt::from(0);
        let mut cost = BigInt::from(0);
        let mut first = None;
        let mut tasks = Vec::new();
        for call in &calls {
            let h = history.map(|h| &h.tasks[call.binding.0]);
            let ready = h.and_then(|h| h.ready);
            let start = h.and_then(|h| h.start);
            let duration = start.map_or(0, |s| h.and_then(|h| h.stop).unwrap_or(until) - s);
            let occupied = BigInt::from(duration);
            let cap = call.capability.0;
            let actor = m.capabilities[cap].actor.0;
            actor_work[actor] += &occupied;
            for u in &m.capabilities[cap].uses {
                resource_work[u.resource.0] += &occupied * BigInt::from(u.amount);
            }
            if let Some(p) = h.and_then(|h| h.executor) {
                person_work[p.0] += &occupied;
            }
            work += &occupied;
            if let Some(rate) = spec.actor_rates[actor] {
                cost += &occupied * BigInt::from(rate);
            }
            if let Some(s) = start {
                first = Some(first.map_or(s, |old: u64| old.min(s)));
                starts[cap] += 1;
                if let Some(rate) = spec.start_rates[cap] {
                    cost += BigInt::from(rate);
                }
            }
            let status = h.map_or(Status::NeverCreated, |h| h.status);
            let wait = match (start, ready) {
                (Some(s), Some(r)) => integer(s - r),
                _ => null(if status == Status::Refused {
                    "refused"
                } else if status == Status::CancelledBeforeStart {
                    "cancelled_before_start"
                } else if status == Status::NeverCreated {
                    "never_created"
                } else if case.deadlocked {
                    "deadlock"
                } else {
                    "unfinished"
                }),
            };
            tasks.push(json!({"binding":m.process.bindings[call.binding.0].name,"capability":cap_name(cap),
                "wait":wait,"work":scalar(BigRational::from_integer(occupied)),"starts":integer(u64::from(start.is_some())),
                "never_created_count":count(u64::from(status==Status::NeverCreated)),
                "cancelled_before_start_count":count(u64::from(status==Status::CancelledBeforeStart)),
                "not_started_count":count(u64::from(status==Status::Queued))}));
            if let Some(origin) = m.loop_site(call.binding) {
                tasks.last_mut().unwrap()["repetition"] =
                    serde_json::to_value(origin).expect("loop source mapping");
            }
            if m.has_requires() {
                let row = tasks.last_mut().unwrap();
                row["refused_count"] = integer(u64::from(status == Status::Refused));
                row["refusal_wait"] = if status == Status::Refused {
                    integer(h.unwrap().stop.unwrap() - ready.unwrap())
                } else {
                    null("not_refused")
                };
            }
        }
        let business_cost = cost.clone();
        for (id, declaration) in m.contests().iter().enumerate() {
            let Some(h) = history.and_then(|h| h.tasks.get(m.contest_site(id).0)) else {
                continue;
            };
            let occupied = BigInt::from(h.start.map_or(0, |start| h.stop.unwrap_or(until) - start));
            let cap = declaration.route.0;
            let actor = m.capabilities[cap].actor.0;
            actor_work[actor] += &occupied;
            for u in &m.capabilities[cap].uses {
                resource_work[u.resource.0] += &occupied * BigInt::from(u.amount);
            }
            if let Some(p) = h.executor {
                person_work[p.0] += &occupied;
            }
            work += &occupied;
            if let Some(rate) = spec.actor_rates[actor] {
                cost += &occupied * BigInt::from(rate);
            }
            if h.start.is_some() {
                starts[cap] += 1;
                if let Some(rate) = spec.start_rates[cap] {
                    cost += BigInt::from(rate);
                }
            }
        }
        let business_at = case
            .state
            .and_then(|s| s.contest.as_ref())
            .and_then(|c| c.business.as_ref())
            .map(|b| b.at);
        let is_cut_off = horizon
            && case.end.is_none()
            && !case.rejected
            && !case.deadlocked
            && !case.not_arrived;
        cut_off += u64::from(is_cut_off);
        let is_unfinished = case.end.is_none() && !case.rejected && !case.deadlocked && !is_cut_off;
        rejected += u64::from(case.rejected);
        unfinished += u64::from(is_unfinished);
        deadlocked += u64::from(case.deadlocked);
        not_arrived += u64::from(case.not_arrived);
        let case_end_counts: Vec<_> = ends
            .iter()
            .map(|e| json!({"end":m.ends[e.0],"count":count(u64::from(case.end==Some(*e)))}))
            .collect();
        let mut terminals = Vec::new();
        for (i, e) in ends.iter().enumerate() {
            let hit = case.end == Some(*e);
            if hit {
                end_counts[i] += 1;
                end_costs[i] += &business_cost;
            }
            terminals.push(json!({"kind":if Some(*e)==m.overflow_end { "overflow" } else { "end" },"end":m.ends[e.0],
                "lead_time":if hit { integer(business_at.unwrap_or(case.state.expect("terminal state").time)-case.arrival) } else { null("no_terminal") },
                "cost_per_case":if Some(*e)==m.overflow_end && hit { null("never_created") } else if is_cut_off { null("no_terminal") } else if !configured_business { null("not_configured") }
                    else if hit { scalar(BigRational::from_integer(business_cost.clone())) } else { null("no_terminal") },
                "count":count(u64::from(hit)),"excluded":{"unfinished":count(u64::from(is_unfinished)),
                    "deadlocked":count(u64::from(case.deadlocked)),"rejected":count(u64::from(case.rejected)),
                    "other_end":count(u64::from(case.end.is_some()&&!hit))}}));
        }
        terminals.push(json!({"kind":"rejected","end":null,"lead_time":(null(if spec.breakdown_waits && !m.response_attempts().is_empty() && !case.rejected {"no_terminal"} else {"rejected"})),
            "cost_per_case":if is_cut_off { null("no_terminal") } else if !configured_business { null("not_configured") }
                else if case.rejected { integer(0) } else { null("no_terminal") },
            "count":count(u64::from(case.rejected)),"excluded":{"unfinished":count(u64::from(is_unfinished)),
                "deadlocked":count(u64::from(case.deadlocked)),"rejected":count(0),"other_end":count(u64::from(case.end.is_some()))}}));
        let first_wait = first.map_or_else(
            || {
                null(if case.rejected {
                    "rejected"
                } else if case.deadlocked {
                    "deadlock"
                } else if case.end.is_some() {
                    "no_task_started"
                } else {
                    "unfinished"
                })
            },
            |s| integer(s - case.arrival),
        );
        case_rows.push(json!({"case_id":index as u64+1,"arrival":case.arrival,
            "counts":{"end_counts":case_end_counts,"rejected":count(u64::from(case.rejected)),
                "unfinished":count(u64::from(is_unfinished)),"deadlocked":count(u64::from(case.deadlocked)),"not_arrived":count(u64::from(case.not_arrived))},
            "first_start_wait":first_wait,"terminals":terminals,"tasks":tasks,"work":scalar(BigRational::from_integer(work)),
            "observed_cost_total":if overflow { null("never_created") } else if configured_actor&&configured_start { scalar(BigRational::from_integer(cost.clone())) } else { null("not_configured") }}));
        if m.horizon.is_some() {
            let row = case_rows.last_mut().unwrap();
            row["counts"]["cut_off"] = count(u64::from(is_cut_off));
            for t in row["terminals"].as_array_mut().unwrap() {
                t["excluded"]["cut_off"] = count(u64::from(is_cut_off));
            }
        }
        if m.has_metric_wait() {
            let wait: u64 = history
                .and_then(|h| h.waits.as_ref())
                .into_iter()
                .flatten()
                .map(|(begin, end)| until.min(*end).saturating_sub(*begin))
                .sum();
            case_rows.last_mut().unwrap()["intentional_wait"] = integer(wait);
        }
        case_costs.push(cost);
    }
    let total_work: BigInt = actor_work.iter().sum();
    let named_work: BigInt = person_work.iter().sum();
    let total_cost: BigInt = case_costs.iter().sum();
    let occupancy_cost: BigInt = spec
        .required_actors
        .iter()
        .filter_map(|id| spec.actor_rates[id.0].map(|r| &actor_work[id.0] * BigInt::from(r)))
        .sum();
    let start_cost: BigInt = spec
        .required_capabilities
        .iter()
        .filter_map(|id| {
            spec.start_rates[id.0].map(|r| BigInt::from(starts[id.0]) * BigInt::from(r))
        })
        .sum();
    let actors: Vec<_> = spec.required_actors.iter().map(|id| {
        let a = &m.actors[id.0];
        json!({"actor":a.name,"capacity":a.capacity,"work":scalar(BigRational::from_integer(actor_work[id.0].clone())),
            "peak_tasks":integer(peak.actors[id.0]),"utilization":match a.capacity {
                None => null("no_capacity"), Some(0) => null("zero_capacity"),
                Some(cap) => ratio(&actor_work[id.0],BigInt::from(cap)*BigInt::from(spec.window)),
            },"occupancy_cost":money(&actor_work[id.0],spec.actor_rates[id.0])})
    }).collect();
    let people: Vec<_> = m.people.iter().enumerate().map(|(i,p)|json!({"person":p.name,
        "work":scalar(BigRational::from_integer(person_work[i].clone())),"peak_tasks":integer(peak.people[i])})).collect();
    let capabilities: Vec<_> = spec.required_capabilities.iter().map(|id|json!({"capability":cap_name(id.0),
        "starts":integer(starts[id.0]),"start_cost":money(&BigInt::from(starts[id.0]),spec.start_rates[id.0])})).collect();
    let rates_actors: Vec<_> = m.actors.iter().enumerate().map(|(i,a)|json!({"actor":a.name,"per_minute":spec.actor_rates[i],
        "reason":if spec.actor_rates[i].is_none(){Some("not_configured")}else{None},"required":spec.required_actors.contains(&ir::ActorId(i))})).collect();
    let rates_caps: Vec<_> = m.capabilities.iter().enumerate().map(|(i,_)|json!({"capability":cap_name(i),"per_start":spec.start_rates[i],
        "reason":if spec.start_rates[i].is_none(){Some("not_configured")}else{None},"required":spec.required_capabilities.contains(&ir::CapabilityId(i))})).collect();
    let terminals: Vec<_> = ends.iter().enumerate().map(|(i,e)|json!({"end":m.ends[e.0],"count":count(end_counts[i]),
        "cost_per_case":if Some(*e)==m.overflow_end && end_counts[i]>0 {null("never_created")}else if !configured_business{null("not_configured")}else if end_counts[i]==0{null("no_terminal")}
            else{ratio(&end_costs[i],BigInt::from(end_counts[i]))},"completed_cases_total":null})).collect();
    let global_end_counts: Vec<_> = ends
        .iter()
        .enumerate()
        .map(|(i, e)| json!({"end":m.ends[e.0],"count":count(end_counts[i])}))
        .collect();
    let mut value = json!({"rules":"business-metrics-p1-1","status":"complete","window":spec.window,"currency":spec.currency,
        "time_basis":"arrival_to_declared_end","lead_time_scope":"terminal_cases_only","observation_scope":"until_execution_stop_no_extra_drain",
        "statistical_unit":null,"estimate_note":null,"rates":{"actors":rates_actors,"capabilities":rates_caps},
        "cases":case_rows,"terminals":terminals,"actors":actors,"people":people,"capabilities":capabilities,
        "overall":{"counts":{"offered":count(cases.len() as u64),"received":count(cases.len() as u64-rejected-not_arrived),
            "completed":count(end_counts.iter().sum()),"rejected":count(rejected),"unfinished":count(unfinished),"deadlocked":count(deadlocked),
            "not_arrived":count(not_arrived),"end_counts":global_end_counts},"observed_until":integer(until),
            "work":scalar(BigRational::from_integer(total_work)),"named_people_work":scalar(BigRational::from_integer(named_work)),
            "peak_tasks":integer(peak.tasks),"occupancy_cost":if configured_actor{scalar(BigRational::from_integer(occupancy_cost))}else{null("not_configured")},
            "start_cost":if configured_start{scalar(BigRational::from_integer(start_cost))}else{null("not_configured")},
            "observed_cost_total":if configured_actor&&configured_start{scalar(BigRational::from_integer(total_cost.clone()))}else{null("not_configured")},
            "observed_cost_per_arrival":if configured_actor&&configured_start{ratio(&total_cost,BigInt::from(cases.len()))}else{null("not_configured")},
            "business_value":{"value":null,"reason":"not_configured"}},
        "checked":["lead_time_terminal_only","first_start_wait","task_ready_wait","actor_work","named_people_work","peak_tasks","capacity_window_utilization","declared_rates","observed_processing_cost","outcome_counts"],
        "unchecked":["quality","business_value","waiting_decomposition","queue_length_statistics","quantiles","exact_expectations","metric_constraints","metric_intervals","anonymous_human_work","calendar","rework_loss","introduction_cost","terminal_meaning"]});
    if !m.response_attempts().is_empty() {
        let unknown = cases
            .iter()
            .filter(|c| {
                c.state
                    .is_some_and(|s| s.response.as_ref().is_some_and(|h| h.stopped))
            })
            .count() as u64;
        value["overall"]["counts"]["receipt_unknown_count"] = count(unknown);
    }
    if m.overflow_end.is_some() {
        for (row, end) in value["terminals"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(&ends)
        {
            row["kind"] = if Some(*end) == m.overflow_end {
                "overflow"
            } else {
                "end"
            }
            .into();
        }
    }
    if m.horizon.is_some() {
        value["observation_scope"] = "until_horizon_or_execution_stop".into();
        value["overall"]["counts"]["cut_off"] = count(cut_off);
    }
    if !m.resources.is_empty() {
        value["rules"] = "business-metrics-p1-resource-1".into();
        value["resources"] = Value::Array(m.resources.iter().enumerate().map(|(i, r)| json!({
            "resource": r.name, "capacity": r.capacity,
            "occupancy_time": scalar(BigRational::from_integer(resource_work[i].clone())),
            "peak_units": scalar(BigRational::from_integer(peak.resources[i].clone())),
            "utilization": if r.capacity == 0 { null("zero_capacity") } else { ratio(&resource_work[i], BigInt::from(r.capacity) * BigInt::from(spec.window)) }
        })).collect());
        for word in [
            "resource_occupancy_time",
            "resource_peak_units",
            "resource_capacity_window_utilization",
        ] {
            value["checked"].as_array_mut().unwrap().push(word.into());
        }
    }
    if m.has_metric_wait() {
        let sum: BigInt = value["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                c["intentional_wait"]["min"]
                    .as_str()
                    .unwrap()
                    .parse::<BigInt>()
                    .unwrap()
            })
            .sum();
        value["overall"]["intentional_wait"] = scalar(BigRational::from_integer(sum));
    }
    if spec.breakdown_waits {
        waits::augment(m, cases, until, horizon, &mut value);
    }
    calendar::augment(m, cases, until, &mut value);
    if m.resource_time.is_some() {
        tariff::pricing(m, cases, until, &mut value);
    }
    serde_json::from_value::<wire::BusinessMetrics>(value)
        .expect("collector constructs closed metrics")
}

#[cfg(test)]
#[path = "../../tests/support/p3a_wait_engine_tests.rs"]
mod p3a_wait_engine_tests;

#[cfg(test)]
mod metrics_tests {
    use super::super::{Instruction, Room, Step, Task, compile, step};
    use super::*;
    use crate::metrics as w;
    use std::hash::{Hash, Hasher};

    fn model() -> ir::Model {
        crate::lower::lower(
            &crate::syntax::parse(include_str!("../../../../fixtures/met-serial/model.relca"))
                .unwrap(),
        )
        .unwrap()
    }

    fn fixture(name: &str) -> ir::Model {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures")
                .join(name)
                .join("model.relca"),
        )
        .unwrap();
        crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap()
    }

    #[test]
    fn forged_metric_specs_are_rejected_by_both_interpreters_without_partial_metrics() {
        let original = model();
        for index in 0..9 {
            let mut m = original.clone();
            let spec = m.metrics.as_mut().unwrap();
            match index {
                0 => spec.window = 0,
                1 => spec.window = m.bounds.clock - 1,
                2 => spec.currency = "  ".into(),
                3 => spec.required_actors.push(ir::ActorId(usize::MAX)),
                4 => spec
                    .required_capabilities
                    .push(ir::CapabilityId(usize::MAX)),
                5 => spec.required_actors.push(spec.required_actors[0]),
                6 => {
                    spec.required_capabilities.pop();
                }
                7 => spec.actor_rates.clear(),
                8 => spec.start_rates.clear(),
                _ => unreachable!(),
            }
            let e = super::super::explore(&m);
            assert!(
                e.diagnostics.iter().any(|d| d.code == crate::diag::U_SEM_0),
                "fault {index}"
            );
            assert!(e.metrics.is_none());
            let r = crate::reference::explore(&m);
            assert_eq!(
                r.stop,
                crate::reference::StopReason::SemanticAbort,
                "fault {index}"
            );
            assert!(r.outcome.metrics.is_none());
        }
    }

    #[test]
    fn clock_prefix_closes_only_metric_intervals_without_a_finish_or_draw() {
        let mut m = fixture("met-sample");
        m.bounds.clock = 3;
        m.capabilities[0].duration_distribution = None;
        m.capabilities[0].duration = 5;
        let mut program = vec![Instruction::Stop];
        let pc = compile(&m.process.body, 0, &mut program);
        let mut state = State::initial(&m, pc);
        let mut room = Room(100);
        loop {
            match step(state.clone(), &program, &m, &mut room).unwrap() {
                Step::Next(mut next) => state = next.remove(0),
                Step::Cut => break,
                _ => panic!("expected clock cut"),
            }
        }
        assert_eq!((state.time, state.work), (0, 0));
        assert!(state.tasks[0].lifecycle.running());
        assert_eq!(state.metric_history.as_ref().unwrap().tasks[0].stop, None);
        let result = super::super::simulate(&m);
        assert!(result.replications.iter().all(|r| r.draws.is_empty()));
        for rep in result.replications {
            let value = serde_json::to_value(rep.metrics.unwrap()).unwrap();
            assert_eq!(value["overall"]["observed_until"]["min"], "3");
            assert_eq!(value["overall"]["work"]["min"], "3");
            assert_eq!(value["overall"]["observed_cost_total"]["min"], "24");
        }
    }

    #[test]
    fn enabled_room_cuts_do_not_publish_partial_extrema() {
        let m = model();
        let mut evidence = super::super::Exploration::default();
        super::super::explore_into(&m, true, 10000, std::convert::identity, &mut evidence).unwrap();
        let (visited, generated, held) = evidence.census;
        assert_eq!((visited, generated, held), (11, 10, 11));
        for budget in 0..=held + 1 {
            let out = super::super::explore_with_budget(&m, budget);
            let metrics = out.metrics.unwrap();
            if budget < held {
                assert!(out.truncated, "budget {budget}");
                let w::Scalar::Range(range) = metrics.overall.work else {
                    panic!("range")
                };
                assert_eq!(
                    (range.min, range.max, range.reason),
                    (None, None, Some(w::Reason::Truncated))
                );
            } else {
                assert!(!out.truncated);
            }
        }
    }

    #[test]
    fn enabled_room_boundaries_cover_fork_duration_ack_people_and_arrivals() {
        for name in [
            "met-parallel",
            "met-positive-support",
            "met-cancel",
            "met-person",
            "met-fifo",
        ] {
            let m = fixture(name);
            let held = if !m.legacy_route() {
                super::super::arrivals::compatibility_counts(&m).2
            } else {
                let mut e = super::super::Exploration::default();
                if m.uses_probability() {
                    super::super::probability_graph(&m, 10000, &mut e).unwrap();
                } else {
                    super::super::explore_into(&m, true, 10000, std::convert::identity, &mut e)
                        .unwrap();
                }
                e.census.2
            };
            assert!(held > 1, "{name}");
            for budget in 0..=held + 1 {
                let out = super::super::explore_with_budget(&m, budget);
                let metric = out.metrics.expect(name);
                if out.truncated {
                    let w::Scalar::Range(work) = metric.overall.work else {
                        panic!("range")
                    };
                    assert!(work.min.is_none() && work.max.is_none(), "{name}/{budget}");
                    assert_eq!(work.reason, Some(w::Reason::Truncated));
                }
            }
            assert!(
                !super::super::explore_with_budget(&m, held + 1).truncated,
                "{name}"
            );
        }
    }

    #[test]
    fn enabled_person_history_splits_a_past_assignment_merge() {
        let source = include_str!("../../../../fixtures/met-person/model.relca")
            .replace("capacity 1; executors [p]", "capacity 2; executors [p, q]")
            .replace("fork f { a = A.a(); b = B.b() }; join all f;", "a = A.a();");
        let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
        let mut enabled = super::super::Exploration::default();
        super::super::explore_into(&m, true, 10000, std::convert::identity, &mut enabled).unwrap();
        let mut plain = m.clone();
        plain.metrics = None;
        let mut disabled = super::super::Exploration::default();
        super::super::explore_into(&plain, true, 10000, std::convert::identity, &mut disabled)
            .unwrap();
        assert!(
            enabled.census.0 > disabled.census.0,
            "enabled {:?}, disabled {:?}",
            enabled.census,
            disabled.census
        );
        let raw = serde_json::to_value(enabled.metrics.unwrap()).unwrap();
        for p in raw["people"].as_array().unwrap() {
            assert_eq!(p["work"]["min"], "0");
            assert_eq!(p["work"]["max"], "3");
        }
        assert_eq!(raw["overall"]["named_people_work"]["min"], "3");
        assert_eq!(raw["overall"]["named_people_work"]["max"], "3");
        println!(
            "METRICS-HISTORY-COUNTS enabled={:?} disabled={:?}",
            enabled.census, disabled.census
        );
    }

    #[test]
    fn leaf_executor_assignments_keep_person_correlations() {
        let mut m = model();
        m.people = vec![
            ir::Person {
                name: "p".into(),
                span: m.process.span,
            },
            ir::Person {
                name: "q".into(),
                span: m.process.span,
            },
        ];
        m.actors[0].executors = Some(vec![ir::PersonId(0), ir::PersonId(1)]);
        m.process.body.truncate(1);
        m.process.body.push(ir::Statement {
            kind: ir::Stmt::End(ir::EndId(0)),
            span: m.process.span,
        });
        for executor in [ir::PersonId(0), ir::PersonId(1)] {
            let mut state = State::initial(&m, 0);
            state.tasks.push(Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: ir::BindingId(0),
                capability: ir::CapabilityId(0),
                lifecycle: Lifecycle::Queued,
            });
            observe(&mut state, &m);
            state.tasks[0].executor = Some(executor);
            state.tasks[0].lifecycle = Lifecycle::Running {
                finish: 3,
                ack: None,
            };
            observe(&mut state, &m);
            state.time = 3;
            state.tasks[0].lifecycle = Lifecycle::Done;
            state.results[0] = Some(ir::ValueId(0));
            observe(&mut state, &m);
            let history = state.metric_history.as_ref().unwrap();
            assert_eq!(history.tasks[0].executor, Some(executor));
            let point = snapshot(
                &m,
                &[CaseObservation {
                    admitted_at: Some(0),
                    state: Some(&state),
                    arrival: 0,
                    end: Some(ir::EndId(0)),
                    rejected: false,
                    deadlocked: false,
                    not_arrived: false,
                }],
                &history.peak,
                3,
            );
            let point = serde_json::to_value(point).unwrap();
            assert_eq!(point["people"][executor.0]["work"]["min"], "3");
            assert_eq!(point["people"][1 - executor.0]["work"]["min"], "0");
            assert_eq!(point["overall"]["named_people_work"]["min"], "3");
        }
    }

    #[test]
    fn capacity_absence_zero_and_large_fee_arithmetic_remain_distinct() {
        let mut m = model();
        m.actors[0].capacity = None;
        let state = State::initial(&m, 0);
        let observation = CaseObservation {
            admitted_at: Some(0),
            state: Some(&state),
            arrival: 0,
            end: None,
            rejected: false,
            deadlocked: true,
            not_arrived: false,
        };
        let value = snapshot(&m, &[observation], &Peak::new(&m), 0);
        let raw = serde_json::to_value(value).unwrap();
        assert_eq!(raw["actors"][0]["utilization"]["reason"], "no_capacity");
        m.actors[0].capacity = Some(0);
        let value = snapshot(
            &m,
            &[CaseObservation {
                admitted_at: Some(0),
                state: Some(&state),
                arrival: 0,
                end: None,
                rejected: false,
                deadlocked: true,
                not_arrived: false,
            }],
            &Peak::new(&m),
            0,
        );
        assert_eq!(
            serde_json::to_value(value).unwrap()["actors"][0]["utilization"]["reason"],
            "zero_capacity"
        );
        m.actors[0].capacity = Some(u64::MAX);
        m.metrics.as_mut().unwrap().window = u64::MAX;
        m.metrics.as_mut().unwrap().actor_rates[0] = Some(u64::MAX);
        let mut state = state;
        state.metric_history.as_mut().unwrap().tasks[0] = TaskHistory {
            status: Status::Finished,
            ready: Some(0),
            start: Some(0),
            stop: Some(2),
            executor: None,
        };
        state.time = 2;
        let value = snapshot(
            &m,
            &[CaseObservation {
                admitted_at: Some(0),
                state: Some(&state),
                arrival: 0,
                end: Some(ir::EndId(0)),
                rejected: false,
                deadlocked: false,
                not_arrived: false,
            }],
            &Peak::new(&m),
            2,
        );
        assert_eq!(
            serde_json::to_value(value).unwrap()["actors"][0]["occupancy_cost"]["min"],
            "36893488147419103230"
        );
    }

    fn hash(s: &State) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    }

    #[test]
    fn each_history_field_splits_state_identity() {
        let m = model();
        let original = State::initial(&m, 0);
        for index in 0..9 {
            let mut changed = original.clone();
            let history = changed.metric_history.as_mut().unwrap();
            match index {
                0 => history.tasks[0].ready = Some(0),
                1 => history.tasks[0].start = Some(0),
                2 => history.tasks[0].stop = Some(0),
                3 => history.tasks[0].executor = Some(ir::PersonId(0)),
                4 => history.tasks[0].status = Status::Queued,
                5 => history.peak.tasks = 1,
                6 => history.peak.actors[0] = 1,
                7 => history.peak.people.push(1),
                8 => history.tasks[1].status = Status::CancelledBeforeStart,
                _ => unreachable!(),
            }
            assert_ne!(original, changed, "field {index}");
            assert_ne!(hash(&original), hash(&changed), "field {index}");
        }
    }

    #[test]
    fn sampling_uses_only_selected_history_and_adds_no_draws() {
        let m = fixture("met-sample");
        let enabled = super::super::simulate(&m);
        let mut plain = m.clone();
        plain.metrics = None;
        let disabled = super::super::simulate(&plain);
        assert_eq!(enabled.replications.len(), 4);
        let mut draw_count = 0;
        for (i, (a, b)) in enabled
            .replications
            .iter()
            .zip(&disabled.replications)
            .enumerate()
        {
            assert_eq!(a.draws, b.draws);
            assert_eq!(a.cases, b.cases);
            assert_eq!(a.cut, b.cut);
            draw_count += a.draws.len();
            for (j, draw) in a.draws.iter().enumerate() {
                assert_eq!(
                    draw.key,
                    format!(
                        "relca-sim-v1|seed=41020|rep={}|case=1|site=duration.P.job|n={}",
                        i + 1,
                        j + 1
                    )
                );
            }
            let metric = serde_json::to_value(a.metrics.as_ref().unwrap()).unwrap();
            assert_eq!(metric["overall"]["work"]["min"], ["2", "4", "2", "4"][i]);
            assert_eq!(metric["capabilities"][0]["starts"]["min"], "1");
        }
        assert_eq!(draw_count, 6);
        let mut prices = m;
        prices.metrics.as_mut().unwrap().actor_rates[0] = Some(u64::MAX);
        let repriced = super::super::simulate(&prices);
        for (a, b) in enabled.replications.iter().zip(&repriced.replications) {
            assert_eq!(a.draws, b.draws);
            assert_eq!(a.cases, b.cases);
        }
    }

    #[test]
    fn metrics_legacy_census() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let inputs = [
            ("compare-legacy", "AsIs"),
            ("compare-legacy", "ToBeP"),
            ("resp-diff", "A"),
            ("resp-diff", "B"),
        ];
        let mut census = Vec::new();
        for (name, selection) in inputs {
            let source = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
            let mut m = crate::lower::lower_configuration(
                &crate::syntax::parse(&source).unwrap(),
                selection,
            )
            .unwrap()
            .model;
            assert!(m.metrics.is_none());
            let static_rows: Vec<_> = m
                .verify
                .iter()
                .filter_map(|v| {
                    if let ir::Property::Contest(id) = v.property {
                        Some(crate::outcome::PropertyOutcome {
                            cases: None,
                            probability: None,
                            property: v.property,
                            verdict: crate::contest::verdict(&m, id),
                        })
                    } else {
                        None
                    }
                })
                .collect();
            if !m.contests().is_empty() {
                // D2 intentionally expands contest models. Preserve the old
                // fixed census on the business-only skeleton, and separately
                // check that opting in really adds the runtime table/states.
                let mut expanded = super::super::Exploration::default();
                super::super::explore_into(&m, true, 10000, std::convert::identity, &mut expanded)
                    .unwrap();
                assert!(expanded.outcome(&m).contest_runtime.is_some());
                let skeleton = source
                    .lines()
                    .filter(|line| !line.trim_start().starts_with("contest "))
                    .collect::<Vec<_>>()
                    .join("\n");
                m = crate::lower::lower_configuration(
                    &crate::syntax::parse(&skeleton).unwrap(),
                    selection,
                )
                .unwrap()
                .model;
            }
            let mut evidence = super::super::Exploration::default();
            super::super::explore_into(
                &m,
                true,
                10000,
                |state| {
                    assert!(state.metric_history.is_none());
                    state
                },
                &mut evidence,
            )
            .unwrap();
            let (visited, generated, held) = evidence.census;
            let mut budgets = Vec::new();
            for budget in 0..=held + 1 {
                let mut out = super::super::explore_with_budget(&m, budget);
                out.properties.extend(static_rows.clone());
                assert!(out.metrics.is_none());
                budgets.push(super::super::legacy_outcome_debug(out));
            }
            let fingerprint = budgets
                .join("\n")
                .bytes()
                .fold(0xcbf29ce484222325u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
                });
            census.push(format!(
                "{name}/{selection} {visited} {generated} {held} {fingerprint:016x}"
            ));
        }
        println!("METRICS-LEGACY-CENSUS\n{}", census.join("\n"));
        // Additional observation, not a regenerated pre-feature baseline table.
        assert_eq!(
            census.join("\n"),
            "compare-legacy/AsIs 11 10 11 798275ff3aa9048d\ncompare-legacy/ToBeP 10 10 10 05849da4ba155dc6\nresp-diff/A 15 14 15 11807b7a208ab86a\nresp-diff/B 15 14 15 64e32c6d683b4cda"
        );
    }

    #[test]
    fn disabled_history_does_not_add_a_hash_discriminant() {
        let mut m = model();
        m.metrics = None;
        let state = State::initial(&m, 0);
        assert!(state.metric_history.is_none());
        let mut expected = std::collections::hash_map::DefaultHasher::new();
        state.state_values.hash(&mut expected);
        state.initialized.hash(&mut expected);
        state.monitors.hash(&mut expected);
        state.time.hash(&mut expected);
        state.pc.hash(&mut expected);
        state.results.hash(&mut expected);
        state.tasks.hash(&mut expected);
        state.waiting.hash(&mut expected);
        state.adopted.hash(&mut expected);
        state.work.hash(&mut expected);
        state.result_at.hash(&mut expected);
        assert_eq!(hash(&state), expected.finish());
    }

    #[test]
    fn serial_history_survives_wait_clear_and_zero_length_peak() {
        let m = model();
        let mut program = vec![Instruction::Stop];
        let pc = compile(&m.process.body, 0, &mut program);
        let mut state = State::initial(&m, pc);
        let mut room = Room(10000);
        loop {
            match step(state.clone(), &program, &m, &mut room).unwrap() {
                Step::Next(mut next) => {
                    assert_eq!(next.len(), 1);
                    state = next.pop().unwrap();
                }
                Step::Terminal(_) => break,
                _ => panic!("unexpected stop"),
            }
        }
        assert!(state.tasks.is_empty());
        let h = state.metric_history.unwrap();
        assert_eq!(
            (h.tasks[0].ready, h.tasks[0].start, h.tasks[0].stop),
            (Some(0), Some(0), Some(3))
        );
        assert_eq!(
            (h.tasks[1].ready, h.tasks[1].start, h.tasks[1].stop),
            (Some(3), Some(3), Some(8))
        );
        assert_eq!(h.peak.tasks, 1);
        let mut zero = State::initial(&m, 0);
        zero.tasks.push(Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: ir::BindingId(0),
            capability: ir::CapabilityId(0),
            lifecycle: Lifecycle::Queued,
        });
        observe(&mut zero, &m);
        zero.tasks[0].lifecycle = Lifecycle::Running {
            finish: 0,
            ack: None,
        };
        observe(&mut zero, &m);
        zero.tasks[0].lifecycle = Lifecycle::Done;
        zero.results[0] = Some(ir::ValueId(0));
        observe(&mut zero, &m);
        let h = zero.metric_history.unwrap();
        assert_eq!(h.peak.tasks, 1);
        assert_eq!((h.tasks[0].start, h.tasks[0].stop), (Some(0), Some(0)));
    }

    #[test]
    fn cancellation_requests_keep_occupancy_and_unstarted_leaf_is_not_charged() {
        let source = r#"model Cancel version "0.2" {
            type T=enum {pass,ok}
            actor A {capacity 1;
                capability win() -> T {duration fixed 3min;result nondet {pass};cancel forbidden_after_start}
                capability work() -> T {duration fixed 7min;result nondet {ok};cancel cooperative ack_within 2min}}
            process P {fork f {fast=A.win();slow=A.work()}
                join any f where result==pass tie by [fast,slow] remaining cancel_then_drain if none -> none_end
                end done;end none_end}
            bounds {cases 1;live_tasks 2;clock 10min}
            metrics {window 10min;currency "JPY";rate A per_minute 2;rate A.win per_start 5;rate A.work per_start 7}
            verify reachable end.done;verify safety no_resource_overcommit}"#;
        let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        let mut program = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut program);
        let mut pending = vec![State::initial(&m, entry)];
        let mut seen = std::collections::HashSet::new();
        let (mut requested, mut unstarted) = (false, false);
        while let Some(s) = pending.pop() {
            if !seen.insert(s.clone()) {
                continue;
            }
            if s.time == 3
                && s.tasks.iter().any(|t| {
                    matches!(
                        t.lifecycle,
                        Lifecycle::Running {
                            ack: Some(4 | 5),
                            ..
                        }
                    )
                })
            {
                requested = true;
                let h = s.metric_history.as_ref().unwrap();
                assert_eq!(h.tasks[1].stop, None);
                assert_eq!(h.tasks[1].status, Status::Running);
                assert_eq!(s.occupancy(&m), vec![1]);
            }
            match step(s.clone(), &program, &m, &mut Room(10000)).unwrap() {
                Step::Next(xs) => pending.extend(xs),
                Step::Terminal(e) => {
                    let h = s.metric_history.as_ref().unwrap();
                    if h.tasks[1].status == Status::CancelledBeforeStart {
                        unstarted = true;
                        assert_eq!((h.tasks[1].start, h.tasks[1].stop), (None, Some(3)));
                        let point = snapshot(
                            &m,
                            &[CaseObservation {
                                admitted_at: Some(0),
                                state: Some(&s),
                                arrival: 0,
                                end: Some(e),
                                rejected: false,
                                deadlocked: false,
                                not_arrived: false,
                            }],
                            &h.peak,
                            s.time,
                        );
                        let value = serde_json::to_value(point).unwrap();
                        assert_eq!(value["capabilities"][1]["starts"]["min"], "0");
                        assert_eq!(
                            value["cases"][0]["tasks"][1]["wait"]["reason"],
                            "cancelled_before_start"
                        );
                        assert_eq!(value["overall"]["observed_cost_total"]["min"], "11");
                    }
                }
                _ => panic!("unexpected cut"),
            }
        }
        assert!(requested && unstarted);
    }
}
