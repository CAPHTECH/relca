//! Independent retrospective analysis of reference Tasks and trace intervals.
use super::{Observation, absent, amount, rational};
use crate::reference::Task;
use crate::{ir, metrics as w};
use num_bigint::BigInt;
use num_rational::BigRational;

fn occupied(task: &Task, at: u64, end: u64) -> bool {
    match task.start {
        Some(start) => start <= at && at < task.stop.unwrap_or(end),
        None => false,
    }
}
fn pending(task: &Task, at: u64, end: u64) -> bool {
    match task.metric_ready {
        Some(ready) => ready <= at && at < task.start.or(task.stop).unwrap_or(end),
        None => false,
    }
}

/// The reference computes shortages directly from its Task inventory, without
/// borrowing engine histories, occupancy counters or cause helpers.
fn shortage(task: &Task, all: &[&Task], m: &ir::Model, at: u64, end: u64) -> usize {
    let operation = &m.capabilities[task.call.capability.0];
    let owner = &m.actors[operation.actor.0];
    let active: Vec<_> = all
        .iter()
        .copied()
        .filter(|t| occupied(t, at, end))
        .collect();
    let capacity_blocked = owner.capacity.is_some_and(|capacity| {
        let n = active
            .iter()
            .filter(|t| m.capabilities[t.call.capability.0].actor == operation.actor)
            .count();
        BigInt::from(n) >= BigInt::from(capacity)
    });
    let mut resource_blocked = false;
    for need in &operation.uses {
        let mut remaining = BigInt::from(m.resources[need.resource.0].capacity);
        for t in &active {
            for use_ in &m.capabilities[t.call.capability.0].uses {
                if use_.resource == need.resource {
                    remaining -= BigInt::from(use_.amount);
                }
            }
        }
        resource_blocked |= remaining < BigInt::from(need.amount);
    }
    if let Some(tables) = &m.resource_time {
        let capacity = |id: usize| {
            tables.calendars[id]
                .intervals
                .iter()
                .find(|i| i.from <= at && at < i.until)
                .expect("validated calendar")
                .capacity
        };
        let mut schedule = false;
        if let Some(id) = tables.actors[operation.actor.0] {
            let slots = capacity(id);
            let used = active
                .iter()
                .filter(|t| m.capabilities[t.call.capability.0].actor == operation.actor)
                .count();
            schedule |= owner.capacity.is_some_and(|physical| slots < physical)
                && BigInt::from(used) >= BigInt::from(slots);
        }
        for claim in &operation.uses {
            if let Some(id) = tables.resources[claim.resource.0] {
                let mut remaining = BigInt::from(capacity(id));
                for task in &active {
                    for u in &m.capabilities[task.call.capability.0].uses {
                        if u.resource == claim.resource {
                            remaining -= u.amount;
                        }
                    }
                }
                schedule |= capacity(id) < m.resources[claim.resource.0].capacity
                    && remaining < BigInt::from(claim.amount);
            }
        }
        let mut person = false;
        if let Some(ps) = operation
            .qualified_executors
            .as_ref()
            .or(owner.executors.as_ref())
        {
            let busy = |p: &ir::PersonId| active.iter().any(|t| t.person == Some(*p));
            let off = |p: &ir::PersonId| tables.people[p.0].is_some_and(|id| capacity(id) == 0);
            if !ps.is_empty() && ps.iter().all(|p| busy(p) || off(p)) {
                person = ps.iter().any(busy);
                schedule |= ps.iter().any(off);
            }
        }
        let mut causes = Vec::new();
        for (flag, code) in [
            (capacity_blocked, 0),
            (resource_blocked, 1),
            (person, 2),
            (schedule, 5),
        ] {
            if flag {
                causes.push(code);
            }
        }
        return match causes.as_slice() {
            [] => 4,
            [one] => *one,
            _ => 3,
        };
    }
    if capacity_blocked && resource_blocked {
        return 3;
    }
    if capacity_blocked {
        return 0;
    }
    if resource_blocked {
        return 1;
    }
    if let Some(candidates) = operation
        .qualified_executors
        .as_ref()
        .or(owner.executors.as_ref())
        && !candidates.is_empty()
        && candidates
            .iter()
            .all(|p| active.iter().any(|t| t.person == Some(*p)))
    {
        return 2;
    }
    4
}

fn split(values: &[BigInt; 11], extended: bool) -> w::Breakdown {
    w::Breakdown {
        condition: extended.then(|| amount(values[10].clone())),
        external: amount(values[0].clone()),
        in_service: amount(values[1].clone()),
        contest_route: amount(values[2].clone()),
        queue: w::QueueCauses {
            calendar: extended.then(|| amount(values[9].clone())),
            actor_capacity: amount(values[3].clone()),
            resource: amount(values[4].clone()),
            executor: amount(values[5].clone()),
            combined: amount(values[6].clone()),
        },
        intentional: amount(values[7].clone()),
        unattributed: amount(values[8].clone()),
    }
}
fn unknown(s: &w::Scalar, extended: bool) -> w::Breakdown {
    w::Breakdown {
        condition: extended.then(|| s.clone()),
        external: s.clone(),
        in_service: s.clone(),
        contest_route: s.clone(),
        queue: w::QueueCauses {
            calendar: extended.then(|| s.clone()),
            actor_capacity: s.clone(),
            resource: s.clone(),
            executor: s.clone(),
            combined: s.clone(),
        },
        intentional: s.clone(),
        unattributed: s.clone(),
    }
}
fn wait_parts(xs: &[BigInt; 6], extended: bool) -> w::WaitCauses {
    w::WaitCauses {
        calendar: extended.then(|| amount(xs[5].clone())),
        condition: extended.then(|| amount(0)),
        actor_capacity: amount(xs[0].clone()),
        resource: amount(xs[1].clone()),
        executor: amount(xs[2].clone()),
        combined: amount(xs[3].clone()),
        unattributed: amount(xs[4].clone()),
    }
}
fn no_wait(s: &w::Scalar, extended: bool) -> w::WaitCauses {
    w::WaitCauses {
        calendar: extended.then(|| s.clone()),
        condition: extended.then(|| s.clone()),
        actor_capacity: s.clone(),
        resource: s.clone(),
        executor: s.clone(),
        combined: s.clone(),
        unattributed: s.clone(),
    }
}

fn joins(body: &ir::Block, m: &ir::Model, o: &Observation<'_>, out: &mut Vec<w::JoinRow>) {
    for stmt in body {
        match &stmt.kind {
            ir::Stmt::Response(_, _) => {} // Sequential attempts/backoff are not joins; their Task and Wait histories are collected separately.
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                joins(then_body, m, o, out);
                joins(else_body, m, o, out);
            }
            ir::Stmt::Fork { id, branches, join } => {
                let actual: Vec<_> = branches
                    .iter()
                    .map(|b| o.tasks.iter().find(|t| t.call.binding == b.binding))
                    .collect();
                let latest = actual
                    .iter()
                    .try_fold(0, |time, t| Some(time.max(t.and_then(|t| t.stop)?)));
                let started = actual.iter().any(|t| t.is_some());
                let mode = if matches!(join, ir::Join::All) {
                    w::JoinMode::All
                } else {
                    w::JoinMode::Any
                };
                out.push(w::JoinRow {
                    fork: m.process.forks[id.0].clone(),
                    mode,
                    branches: branches
                        .iter()
                        .zip(actual)
                        .map(|(b, t)| w::JoinBranch {
                            binding: m.process.bindings[b.binding.0].name.clone(),
                            join_wait: if mode == w::JoinMode::Any {
                                absent(w::Reason::JoinAny)
                            } else if let Some(last) = latest {
                                amount(last - t.unwrap().stop.unwrap())
                            } else {
                                absent(if !started {
                                    w::Reason::NeverCreated
                                } else if o.deadlock {
                                    w::Reason::Deadlock
                                } else {
                                    w::Reason::Unfinished
                                })
                            },
                        })
                        .collect(),
                });
            }
            _ => {}
        }
    }
}

pub(super) fn collect(
    m: &ir::Model,
    observations: &[Observation<'_>],
    end: u64,
    horizon: bool,
    report: &mut w::BusinessMetrics,
) {
    let all: Vec<_> = observations.iter().flat_map(|o| o.tasks).collect();
    let extended = m.resource_time.is_some() || m.has_condition_wait();
    let mut boundaries = vec![0, end];
    if let Some(tables) = &m.resource_time {
        for i in tables.calendars.iter().flat_map(|c| &c.intervals) {
            boundaries.extend([i.from.min(end), i.until.min(end)]);
        }
    }
    for o in observations {
        for &(a, b) in &o.condition_intervals {
            boundaries.extend([a.min(end), b.unwrap_or(end).min(end)]);
        }
        boundaries.push(o.arrival.min(end));
        boundaries.extend(o.admitted_at.map(|t| t.min(end)));
        boundaries.extend(o.terminal.map(|(_, t)| t.min(end)));
        for &(a, b) in &o.wait_intervals {
            boundaries.extend([a.min(end), b.min(end)]);
        }
    }
    for t in &all {
        boundaries.extend(
            [t.metric_ready, t.start, t.stop]
                .into_iter()
                .flatten()
                .map(|n| n.min(end)),
        );
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    let segments: Vec<_> = boundaries.windows(2).map(|p| (p[0], p[1] - p[0])).collect();
    for (row, o) in report.cases.iter_mut().zip(observations) {
        let mut terminal_values = std::array::from_fn::<_, 11, _>(|_| BigInt::from(0));
        let mut observed_values = terminal_values.clone();
        for &(at, length) in &segments {
            if at < o.arrival {
                continue;
            }
            let pending_causes: Vec<_> = o
                .tasks
                .iter()
                .filter(|t| pending(t, at, end))
                .map(|t| shortage(t, &all, m, at, end))
                .collect();
            let index = if o.admitted_at.is_none_or(|admitted| at < admitted) {
                0
            } else if o
                .tasks
                .iter()
                .any(|t| m.site_contest(t.call.binding).is_none() && occupied(t, at, end))
            {
                1
            } else if o
                .tasks
                .iter()
                .any(|t| m.site_contest(t.call.binding).is_some() && occupied(t, at, end))
            {
                2
            } else if !pending_causes.is_empty() {
                let mut kinds = pending_causes.clone();
                kinds.sort_unstable();
                kinds.dedup();
                if kinds.len() > 1 || kinds[0] == 3 {
                    6
                } else if kinds[0] == 4 {
                    8
                } else if kinds[0] == 5 {
                    9
                } else {
                    3 + kinds[0]
                }
            } else if o
                .condition_intervals
                .iter()
                .any(|&(a, b)| a <= at && at < b.unwrap_or(end))
            {
                10
            } else if o.wait_intervals.iter().any(|&(a, b)| a <= at && at < b) {
                7
            } else {
                8
            };
            observed_values[index] += BigInt::from(length);
            if o.terminal.is_some_and(|(_, stop)| at < stop) {
                terminal_values[index] += BigInt::from(length);
            }
        }
        for terminal in &mut row.terminals {
            let numeric = matches!(&terminal.lead_time,w::Scalar::Range(r) if r.min.is_some());
            terminal.breakdown = Some(if numeric {
                split(&terminal_values, extended)
            } else {
                unknown(&terminal.lead_time, extended)
            });
        }
        if horizon && o.terminal.is_none() && !o.rejected && !o.deadlock && !o.future {
            row.dwell = Some(w::Dwell {
                until: amount(end),
                breakdown: split(&observed_values, extended),
                bound: w::Bound::Lower,
            });
        }
        let mut join_rows = Vec::new();
        joins(&m.process.body, m, o, &mut join_rows);
        row.joins = Some(join_rows);
        for r in &mut row.tasks {
            let task = o.tasks.iter().find(|t| {
                m.process
                    .bindings
                    .get(t.call.binding.0)
                    .is_some_and(|b| b.name == r.binding)
            });
            r.predecessor_wait = Some(match (task.and_then(|t| t.metric_ready), o.admitted_at) {
                (Some(ready), Some(admitted)) => amount(
                    ready
                        .checked_sub(admitted)
                        .expect("metric invariant: Task.ready precedes admission"),
                ),
                _ => absent(w::Reason::NeverCreated),
            });
            r.wait_causes = Some(
                if let Some(t) = task
                    && let (Some(ready), Some(start)) = (t.metric_ready, t.start)
                {
                    let mut xs = std::array::from_fn::<_, 6, _>(|_| BigInt::from(0));
                    for &(at, length) in &segments {
                        if ready <= at && at < start {
                            xs[shortage(t, &all, m, at, end)] += BigInt::from(length);
                        }
                    }
                    wait_parts(&xs, extended)
                } else {
                    no_wait(&r.wait, extended)
                },
            );
        }
    }
    let mut overall = BigInt::from(0);
    for (row, id) in report
        .actors
        .iter_mut()
        .zip(&m.metrics.as_ref().unwrap().required_actors)
    {
        let mut total = BigInt::from(0);
        let mut max = 0;
        for &(at, length) in &segments {
            let n = all
                .iter()
                .filter(|t| m.capabilities[t.call.capability.0].actor == *id && pending(t, at, end))
                .count();
            total += BigInt::from(length) * BigInt::from(n);
            max = max.max(n);
        }
        overall += &total;
        let bound = horizon.then_some(w::Bound::Lower);
        let average = if end == 0 {
            absent(w::Reason::NoSamples)
        } else {
            rational(BigRational::new(total.clone(), BigInt::from(end)))
        };
        row.queue = Some(w::ActorQueue {
            wait_total: w::ObservedQuantity {
                stats: amount(total),
                bound,
            },
            length_max: amount(max),
            length_time_average: w::ObservedQuantity {
                stats: average,
                bound,
            },
        });
    }
    report.overall.queue_wait_observed = Some(amount(overall));
    if m.has_condition_wait() {
        report.checked.push("condition_wait_partition".into());
    }
    report.checked.extend([
        "waiting_decomposition".into(),
        "queue_length_max_and_average".into(),
    ]);
    report.unchecked.retain(|s| s != "waiting_decomposition");
    if let Some(word) = report
        .unchecked
        .iter_mut()
        .find(|s| s.as_str() == "queue_length_statistics")
    {
        *word = "queue_length_quantiles".into();
    }
}
