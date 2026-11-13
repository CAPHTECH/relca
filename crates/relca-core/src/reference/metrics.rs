//! Independent interval reconstruction from the reference interpreter's past tasks.
//! This module deliberately has no dependency on engine instrumentation.
use super::{Phase, Task};
use crate::{ir, metrics as w};
use num_bigint::BigInt;
use num_rational::BigRational;
mod calendar;
mod tariff;
mod waits;
#[cfg(test)]
mod waits_tests;

#[cfg(test)]
mod metrics_tests {
    #[test]
    fn reference_collectors_do_not_depend_on_engine_code() {
        let forbidden = ["engine", "::"].concat();
        for source in [
            include_str!("../reference.rs"),
            include_str!("arrivals.rs"),
            include_str!("simulation.rs"),
            include_str!("metrics.rs"),
        ] {
            assert!(!source.contains(&forbidden));
        }
    }
}

pub(super) struct Observation<'a> {
    pub condition_intervals: Vec<(u64, Option<u64>)>,
    pub admitted_at: Option<u64>,
    pub wait_intervals: Vec<(u64, u64)>,
    pub intentional_wait: u64,
    pub tasks: &'a [Task],
    pub arrival: u64,
    pub terminal: Option<(ir::EndId, u64)>,
    pub rejected: bool,
    pub deadlock: bool,
    pub future: bool,
}

pub(super) fn amount(x: impl Into<BigInt>) -> w::Scalar {
    rational(BigRational::from_integer(x.into()))
}
fn rational(x: BigRational) -> w::Scalar {
    let x = w::MetricNumber(x);
    w::Scalar::Range(w::Range {
        min: Some(x.clone()),
        max: Some(x),
        reason: None,
    })
}
pub(super) fn absent(reason: w::Reason) -> w::Scalar {
    w::Scalar::Range(w::Range {
        min: None,
        max: None,
        reason: Some(reason),
    })
}
pub(super) fn count(x: u64) -> w::Count {
    w::Count {
        stats: amount(x),
        observed_total: None,
    }
}
fn cost(quantity: &BigInt, rate: Option<u64>) -> w::Scalar {
    match rate {
        Some(r) => amount(quantity * BigInt::from(r)),
        None => absent(w::Reason::NotConfigured),
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Envelope {
    template: Option<serde_json::Value>,
    columns: std::collections::BTreeMap<String, Option<(w::MetricNumber, w::MetricNumber)>>,
}
impl Envelope {
    pub fn add(&mut self, point: w::BusinessMetrics) {
        point.assert_wait_populations();
        fn columns(
            value: &serde_json::Value,
            path: String,
            out: &mut Vec<(String, Option<w::MetricNumber>)>,
        ) {
            match value {
                serde_json::Value::Object(fields) if fields.contains_key("min") => {
                    out.push((path, serde_json::from_value(fields["min"].clone()).ok()));
                }
                serde_json::Value::Object(fields) => {
                    for (key, value) in fields {
                        columns(value, format!("{path}/{key}"), out);
                    }
                }
                serde_json::Value::Array(values) => {
                    for (i, value) in values.iter().enumerate() {
                        columns(value, format!("{path}/{i}"), out);
                    }
                }
                _ => {}
            }
        }
        let raw = serde_json::to_value(point).expect("reference metrics");
        let mut values = Vec::new();
        columns(&raw, String::new(), &mut values);
        for (path, value) in values {
            let bound = self.columns.entry(path).or_default();
            if let Some(value) = value {
                match bound {
                    Some((lo, hi)) => {
                        *lo = lo.clone().min(value.clone());
                        *hi = hi.clone().max(value);
                    }
                    None => *bound = Some((value.clone(), value)),
                }
            }
        }
        if let Some(template) = &mut self.template {
            if raw["overall"].get("queue_wait_observed").is_some() {
                let mut typed: w::BusinessMetrics =
                    serde_json::from_value(template.clone()).expect("reference template");
                typed.union_wait_presence(&serde_json::from_value(raw).expect("reference point"));
                *template = serde_json::to_value(typed).expect("reference template");
            }
        } else {
            self.template = Some(raw);
        }
    }
    /// `status` is the coverage word: complete, truncated, aborted or
    /// (ADR 0029 U2) range_stopped.
    pub fn finish(self, status: &str) -> Option<w::BusinessMetrics> {
        let mut raw = self.template?;
        serde_json::from_value::<w::BusinessMetrics>(raw.clone())
            .expect("reference template")
            .assert_wait_populations();
        for (path, bound) in self.columns {
            let slot = raw.pointer_mut(&path).expect("reference metric column");
            if status != "complete" {
                *slot = serde_json::json!({"min":null,"max":null,"reason":status});
            } else if let Some((min, max)) = bound {
                *slot = serde_json::json!({"min":min,"max":max,"reason":null});
            }
        }
        raw["status"] = status.into();
        let mut result: w::BusinessMetrics =
            serde_json::from_value(raw).expect("reference extrema");
        if status != "complete" {
            result.withdraw_wait_bounds();
        }
        result.align_wait_reasons();
        Some(result)
    }
}

/// Source inventory is rebuilt independently, not taken from engine fee helpers.
fn inventory<'a>(block: &'a ir::Block, calls: &mut Vec<&'a ir::Call>, ends: &mut Vec<ir::EndId>) {
    for statement in block {
        let mut end = None;
        match &statement.kind {
            ir::Stmt::LoopControl(_) => {}
            ir::Stmt::Wait(_) | ir::Stmt::WaitUntil(_) => {}
            ir::Stmt::End(e) => end = Some(*e),
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
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                inventory(then_body, calls, ends);
                inventory(else_body, calls, ends);
            }
            ir::Stmt::Fork { branches, join, .. } => {
                calls.extend(branches);
                if let ir::Join::Any { if_none, .. } = join {
                    end = Some(*if_none);
                }
            }
        }
        if let Some(e) = end
            && !ends.contains(&e)
        {
            ends.push(e);
        }
    }
}

pub(super) fn validate(m: &ir::Model) -> bool {
    let Some(s) = &m.metrics else { return true };
    if s.window == 0
        || s.window < m.bounds.clock
        || s.currency.trim().is_empty()
        || s.actor_rates.len() != m.actors.len()
        || s.start_rates.len() != m.capabilities.len()
    {
        return false;
    }
    let (mut calls, mut ends) = (Vec::new(), Vec::new());
    inventory(&m.process.body, &mut calls, &mut ends);
    let mut expected_caps = vec![false; m.capabilities.len()];
    let mut expected_actors = vec![false; m.actors.len()];
    for call in calls {
        let Some(cap) = m.capabilities.get(call.capability.0) else {
            return false;
        };
        let Some(actor) = expected_actors.get_mut(cap.actor.0) else {
            return false;
        };
        *actor = true;
        expected_caps[call.capability.0] = true;
    }
    for contest in m.contests().iter().filter(|c| match c.target {
        crate::contest::ContestTarget::End(e) => ends.contains(&e),
        _ => c.target.active(m),
    }) {
        let Some(cap) = m.capabilities.get(contest.route.0) else {
            return false;
        };
        if cap.actor.0 >= expected_actors.len() {
            return false;
        }
        expected_caps[contest.route.0] = true;
        expected_actors[cap.actor.0] = true;
    }
    s.required_actors.iter().map(|a| a.0).eq(expected_actors
        .iter()
        .enumerate()
        .filter_map(|(i, x)| x.then_some(i)))
        && s.required_capabilities.iter().map(|c| c.0).eq(expected_caps
            .iter()
            .enumerate()
            .filter_map(|(i, x)| x.then_some(i)))
}

pub(super) fn project(
    m: &ir::Model,
    observations: &[Observation<'_>],
    until: u64,
) -> w::BusinessMetrics {
    project_with_horizon(m, observations, until, false)
}

pub(super) fn project_with_horizon(
    m: &ir::Model,
    observations: &[Observation<'_>],
    until: u64,
    horizon: bool,
) -> w::BusinessMetrics {
    let s = m.metrics.as_ref().expect("metrics enabled");
    let (mut calls, mut ends) = (Vec::new(), Vec::new());
    inventory(&m.process.body, &mut calls, &mut ends);
    calls.sort_by_key(|c| c.binding.0);
    if let Some(end) = m.overflow_end {
        ends.push(end);
    }
    let name = |id: usize| {
        format!(
            "{}.{}",
            m.actors[m.capabilities[id].actor.0].name, m.capabilities[id].name
        )
    };
    let all_occupancy = s
        .required_actors
        .iter()
        .all(|a| s.actor_rates[a.0].is_some());
    let all_starts = s
        .required_capabilities
        .iter()
        .all(|c| s.start_rates[c.0].is_some());
    let all_cost = all_occupancy && all_starts;
    let business_cost_configured = calls.iter().all(|c| {
        s.start_rates[c.capability.0].is_some()
            && s.actor_rates[m.capabilities[c.capability.0].actor.0].is_some()
    });
    let mut aw = vec![BigInt::from(0); m.actors.len()];
    let mut pw = vec![BigInt::from(0); m.people.len()];
    let mut starts = vec![0u64; m.capabilities.len()];
    let mut intervals = Vec::new();
    let mut cases = Vec::new();
    let mut completed = vec![0u64; ends.len()];
    let mut end_cost = vec![BigInt::from(0); ends.len()];
    let mut totals = [0u64; 4]; // rejected, unfinished, deadlocked, future
    let mut cutoff_total = 0u64;
    let mut total_cost = BigInt::from(0);
    for (i, o) in observations.iter().enumerate() {
        let cutoff = horizon && o.terminal.is_none() && !o.rejected && !o.deadlock && !o.future;
        cutoff_total += u64::from(cutoff);
        let unfinished = o.terminal.is_none() && !o.rejected && !o.deadlock && !cutoff;
        let counts = [
            u64::from(o.rejected),
            u64::from(unfinished),
            u64::from(o.deadlock),
            u64::from(o.future),
        ];
        for (sum, n) in totals.iter_mut().zip(counts) {
            *sum += n;
        }
        let mut work = BigInt::from(0);
        let mut fees = BigInt::from(0);
        let first = o
            .tasks
            .iter()
            .filter(|t| m.site_contest(t.call.binding).is_none())
            .filter_map(|t| t.start)
            .min();
        let mut tasks = Vec::new();
        for call in &calls {
            let task = o.tasks.iter().find(|t| t.call.binding == call.binding);
            let started = task.and_then(|t| t.start);
            let c = call.capability.0;
            let a = m.capabilities[c].actor.0;
            let length = if let Some(begin) = started {
                let task = task.unwrap();
                let end = task.stop.unwrap_or(until);
                intervals.push((begin, end, a, task.person));
                starts[c] += 1;
                if let Some(r) = s.start_rates[c] {
                    fees += BigInt::from(r);
                }
                BigInt::from(end - begin)
            } else {
                BigInt::from(0)
            };
            aw[a] += &length;
            if let Some(person) = task.and_then(|t| t.person) {
                pw[person.0] += &length;
            }
            work += &length;
            if let Some(r) = s.actor_rates[a] {
                fees += &length * BigInt::from(r);
            }
            let cancelled = task.is_some_and(|t| t.phase == Phase::Cancelled) && started.is_none();
            let wait = match (started, task.and_then(|t| t.metric_ready)) {
                (Some(begin), Some(ready)) => amount(begin - ready),
                _ => absent(if task.is_none() {
                    w::Reason::NeverCreated
                } else if task.is_some_and(|t| matches!(t.phase, Phase::Refused(_))) {
                    w::Reason::Refused
                } else if cancelled {
                    w::Reason::CancelledBeforeStart
                } else if o.deadlock {
                    w::Reason::Deadlock
                } else {
                    w::Reason::Unfinished
                }),
            };
            tasks.push(w::Task {
                repetition: m.loop_site(call.binding).cloned(),
                tariff_regular_work: None,
                tariff_overtime_work: None,
                tariff_regular_cost: None,
                tariff_overtime_cost: None,
                wait_causes: None,
                predecessor_wait: None,
                refused_count: m.has_requires().then(|| {
                    amount(u64::from(
                        task.is_some_and(|t| matches!(t.phase, Phase::Refused(_))),
                    ))
                }),
                refusal_wait: m.has_requires().then(|| {
                    if let Some(t) = task
                        && matches!(t.phase, Phase::Refused(_))
                    {
                        amount(t.stop.unwrap() - t.metric_ready.unwrap())
                    } else {
                        absent(w::Reason::NotRefused)
                    }
                }),
                binding: m.process.bindings[call.binding.0].name.clone(),
                capability: name(c),
                wait,
                work: amount(length),
                starts: amount(u64::from(started.is_some())),
                never_created_count: count(u64::from(task.is_none())),
                cancelled_before_start_count: count(u64::from(cancelled)),
                not_started_count: count(u64::from(task.is_some_and(|t| t.phase == Phase::Queued))),
            });
        }
        let business_fees = fees.clone();
        for task in o
            .tasks
            .iter()
            .filter(|t| m.site_contest(t.call.binding).is_some())
        {
            let c = task.call.capability.0;
            let a = m.capabilities[c].actor.0;
            let length = if let Some(start) = task.start {
                let end = task.stop.unwrap_or(until);
                intervals.push((start, end, a, task.person));
                starts[c] += 1;
                if let Some(rate) = s.start_rates[c] {
                    fees += BigInt::from(rate);
                }
                BigInt::from(end - start)
            } else {
                BigInt::from(0)
            };
            aw[a] += &length;
            if let Some(person) = task.person {
                pw[person.0] += &length;
            }
            work += &length;
            if let Some(rate) = s.actor_rates[a] {
                fees += &length * BigInt::from(rate);
            }
        }
        let mut terminals = Vec::new();
        let end_counts = ends
            .iter()
            .map(|e| w::EndCount {
                end: m.ends[e.0].clone(),
                count: count(u64::from(o.terminal.is_some_and(|(t, _)| t == *e))),
            })
            .collect();
        for (j, e) in ends.iter().enumerate() {
            let reached = o.terminal.filter(|(t, _)| t == e);
            if reached.is_some() {
                completed[j] += 1;
                end_cost[j] += &business_fees;
            }
            terminals.push(w::CaseTerminal {
                breakdown: None,
                kind: if Some(*e) == m.overflow_end {
                    w::TerminalKind::Overflow
                } else {
                    w::TerminalKind::End
                },
                end: Some(m.ends[e.0].clone()),
                lead_time: reached.map_or_else(
                    || absent(w::Reason::NoTerminal),
                    |(_, at)| amount(at - o.arrival),
                ),
                cost_per_case: if Some(*e) == m.overflow_end && reached.is_some() {
                    absent(w::Reason::NeverCreated)
                } else if cutoff {
                    absent(w::Reason::NoTerminal)
                } else if !business_cost_configured {
                    absent(w::Reason::NotConfigured)
                } else if reached.is_some() {
                    amount(business_fees.clone())
                } else {
                    absent(w::Reason::NoTerminal)
                },
                count: count(u64::from(reached.is_some())),
                excluded: w::Excluded {
                    cut_off: m.horizon.map(|_| count(u64::from(cutoff))),
                    unfinished: count(counts[1]),
                    deadlocked: count(counts[2]),
                    rejected: count(counts[0]),
                    other_end: count(u64::from(o.terminal.is_some() && reached.is_none())),
                },
            });
        }
        terminals.push(w::CaseTerminal {
            breakdown: None,
            kind: w::TerminalKind::Rejected,
            end: None,
            lead_time: absent(
                if s.breakdown_waits && !m.response_attempts().is_empty() && !o.rejected {
                    w::Reason::NoTerminal
                } else {
                    w::Reason::Rejected
                },
            ),
            cost_per_case: if cutoff {
                absent(w::Reason::NoTerminal)
            } else if !business_cost_configured {
                absent(w::Reason::NotConfigured)
            } else if counts[0] > 0 {
                amount(0)
            } else {
                absent(w::Reason::NoTerminal)
            },
            count: count(counts[0]),
            excluded: w::Excluded {
                cut_off: m.horizon.map(|_| count(u64::from(cutoff))),
                unfinished: count(counts[1]),
                deadlocked: count(counts[2]),
                rejected: count(0),
                other_end: count(u64::from(o.terminal.is_some())),
            },
        });
        cases.push(w::Case {
            arrival_time: None,
            dwell: None,
            joins: None,
            external_wait: m.external_wait().then(|| absent(w::Reason::Unfinished)),
            intentional_wait: m.has_metric_wait().then(|| amount(o.intentional_wait)),
            case_id: i as u64 + 1,
            arrival: Some(o.arrival),
            counts: w::CaseCounts {
                admitted: m.explicit_limits().then(|| count(0)),
                overflowed: m.overflow_end.map(|_| count(0)),
                cut_off: m.horizon.map(|_| count(u64::from(cutoff))),
                end_counts,
                rejected: count(counts[0]),
                unfinished: count(counts[1]),
                deadlocked: count(counts[2]),
                not_arrived: count(counts[3]),
            },
            first_start_wait: first.map_or_else(
                || {
                    absent(if o.rejected {
                        w::Reason::Rejected
                    } else if o.deadlock {
                        w::Reason::Deadlock
                    } else if o.terminal.is_some() {
                        w::Reason::NoTaskStarted
                    } else {
                        w::Reason::Unfinished
                    })
                },
                |at| amount(at - o.arrival),
            ),
            terminals,
            tasks,
            work: amount(work),
            observed_cost_total: if o
                .terminal
                .is_some_and(|(end, _)| Some(end) == m.overflow_end)
            {
                absent(w::Reason::NeverCreated)
            } else if all_cost {
                amount(fees.clone())
            } else {
                absent(w::Reason::NotConfigured)
            },
        });
        total_cost += fees;
    }
    // An independent interval sweep. Endpoints are half-open; instantaneous
    // tasks are additionally observed at their own start point.
    let mut actor_peak = vec![0u64; m.actors.len()];
    let mut person_peak = vec![0u64; m.people.len()];
    let mut peak = 0u64;
    for (begin, end, actor, person) in &intervals {
        let running: Vec<_> = intervals
            .iter()
            .filter(|(a, b, _, _)| a <= begin && begin < b)
            .collect();
        let instant = u64::from(begin == end);
        peak = peak.max(running.len() as u64 + instant);
        for (i, p) in actor_peak.iter_mut().enumerate() {
            *p = (*p).max(
                running.iter().filter(|t| t.2 == i).count() as u64
                    + if *actor == i { instant } else { 0 },
            );
        }
        for (i, p) in person_peak.iter_mut().enumerate() {
            *p = (*p).max(
                running
                    .iter()
                    .filter(|t| t.3 == Some(ir::PersonId(i)))
                    .count() as u64
                    + if *person == Some(ir::PersonId(i)) {
                        instant
                    } else {
                        0
                    },
            );
        }
    }
    let occupancy: BigInt = s
        .required_actors
        .iter()
        .filter_map(|id| s.actor_rates[id.0].map(|r| &aw[id.0] * BigInt::from(r)))
        .sum();
    let start_fees: BigInt = s
        .required_capabilities
        .iter()
        .filter_map(|id| s.start_rates[id.0].map(|r| BigInt::from(starts[id.0]) * BigInt::from(r)))
        .sum();
    let actors = s
        .required_actors
        .iter()
        .map(|id| w::Actor {
            tariff_regular_work: None,
            tariff_overtime_work: None,
            tariff_regular_cost: None,
            tariff_overtime_cost: None,
            scheduled_capacity_time: None,
            scheduled_occupancy_time: None,
            overtime_occupancy_time: None,
            scheduled_utilization: None,
            queue: None,
            actor: m.actors[id.0].name.clone(),
            capacity: m.actors[id.0].capacity,
            work: amount(aw[id.0].clone()),
            peak_tasks: amount(actor_peak[id.0]),
            utilization: match m.actors[id.0].capacity {
                None => absent(w::Reason::NoCapacity),
                Some(0) => absent(w::Reason::ZeroCapacity),
                Some(n) => rational(BigRational::new(
                    aw[id.0].clone(),
                    BigInt::from(n) * BigInt::from(s.window),
                )),
            },
            occupancy_cost: cost(&aw[id.0], s.actor_rates[id.0]),
        })
        .collect();
    let people = m
        .people
        .iter()
        .enumerate()
        .map(|(i, p)| w::Person {
            overtime_work: None,
            person: p.name.clone(),
            work: amount(pw[i].clone()),
            peak_tasks: amount(person_peak[i]),
        })
        .collect();
    let capabilities = s
        .required_capabilities
        .iter()
        .map(|id| w::Capability {
            capability: name(id.0),
            starts: amount(starts[id.0]),
            start_cost: cost(&BigInt::from(starts[id.0]), s.start_rates[id.0]),
        })
        .collect();
    let terminals = ends
        .iter()
        .enumerate()
        .map(|(i, e)| w::Terminal {
            kind: m.overflow_end.map(|overflow| {
                if *e == overflow {
                    w::TerminalKind::Overflow
                } else {
                    w::TerminalKind::End
                }
            }),
            end: m.ends[e.0].clone(),
            count: count(completed[i]),
            cost_per_case: if Some(*e) == m.overflow_end && completed[i] > 0 {
                absent(w::Reason::NeverCreated)
            } else if !business_cost_configured {
                absent(w::Reason::NotConfigured)
            } else if completed[i] == 0 {
                absent(w::Reason::NoTerminal)
            } else {
                rational(BigRational::new(
                    end_cost[i].clone(),
                    BigInt::from(completed[i]),
                ))
            },
            completed_cases_total: None,
        })
        .collect();
    let end_counts = ends
        .iter()
        .enumerate()
        .map(|(i, e)| w::EndCount {
            end: m.ends[e.0].clone(),
            count: count(completed[i]),
        })
        .collect();
    let resources = (!m.resources.is_empty()).then(|| {
        m.resources
            .iter()
            .enumerate()
            .map(|(id, r)| {
                let mut intervals = Vec::new();
                let mut occupancy = BigInt::from(0);
                for o in observations {
                    for t in o.tasks {
                        if let Some(start) = t.start {
                            let end = t.stop.unwrap_or(until);
                            for u in &m.capabilities[t.call.capability.0].uses {
                                if u.resource.0 == id {
                                    occupancy += BigInt::from(end - start) * BigInt::from(u.amount);
                                    intervals.push((start, end, BigInt::from(u.amount)));
                                }
                            }
                        }
                    }
                }
                let mut peak = BigInt::from(0);
                for (start, end, units) in &intervals {
                    let mut active: BigInt = intervals
                        .iter()
                        .filter(|(a, b, _)| a <= start && start < b)
                        .map(|(_, _, q)| q)
                        .sum();
                    if start == end {
                        active += units;
                    }
                    peak = peak.max(active);
                }
                w::Resource {
                    scheduled_capacity_time: None,
                    scheduled_occupancy_time: None,
                    overtime_occupancy_time: None,
                    scheduled_utilization: None,
                    resource: r.name.clone(),
                    capacity: r.capacity,
                    occupancy_time: amount(occupancy.clone()),
                    peak_units: amount(peak),
                    utilization: if r.capacity == 0 {
                        absent(w::Reason::ZeroCapacity)
                    } else {
                        rational(BigRational::new(
                            occupancy,
                            BigInt::from(r.capacity) * BigInt::from(s.window),
                        ))
                    },
                }
            })
            .collect()
    });
    let mut report = w::BusinessMetrics {
        contests: None,
        resources,
        rules: if m.resources.is_empty() {
            "business-metrics-p1-1"
        } else {
            "business-metrics-p1-resource-1"
        }
        .into(),
        status: w::Status::Complete,
        window: s.window,
        currency: s.currency.clone(),
        time_basis: "arrival_to_declared_end".into(),
        lead_time_scope: "terminal_cases_only".into(),
        observation_scope: if m.horizon.is_some() {
            "until_horizon_or_execution_stop"
        } else {
            "until_execution_stop_no_extra_drain"
        }
        .into(),
        statistical_unit: None,
        estimate_note: None,
        rates: w::Rates {
            actors: m
                .actors
                .iter()
                .enumerate()
                .map(|(i, a)| w::ActorRate {
                    actor: a.name.clone(),
                    per_minute: s.actor_rates[i],
                    reason: s.actor_rates[i]
                        .is_none()
                        .then_some(w::Reason::NotConfigured),
                    required: s.required_actors.contains(&ir::ActorId(i)),
                })
                .collect(),
            capabilities: m
                .capabilities
                .iter()
                .enumerate()
                .map(|(i, _)| w::StartRate {
                    capability: name(i),
                    per_start: s.start_rates[i],
                    reason: s.start_rates[i]
                        .is_none()
                        .then_some(w::Reason::NotConfigured),
                    required: s.required_capabilities.contains(&ir::CapabilityId(i)),
                })
                .collect(),
        },
        cases,
        terminals,
        actors,
        people,
        capabilities,
        overall: w::Overall {
            queue_wait_observed: None,
            intentional_wait: m
                .has_metric_wait()
                .then(|| amount(observations.iter().map(|o| o.intentional_wait).sum::<u64>())),
            counts: w::OverallCounts {
                receipt_unknown_count: None,
                admitted: m.explicit_limits().then(|| count(0)),
                overflowed: m.overflow_end.map(|_| count(0)),
                cut_off: m.horizon.map(|_| count(cutoff_total)),
                offered: count(observations.len() as u64),
                received: count(observations.len() as u64 - totals[0] - totals[3]),
                completed: count(completed.iter().sum()),
                rejected: count(totals[0]),
                unfinished: count(totals[1]),
                deadlocked: count(totals[2]),
                not_arrived: count(totals[3]),
                end_counts,
            },
            observed_until: amount(until),
            work: amount(aw.iter().sum::<BigInt>()),
            named_people_work: amount(pw.iter().sum::<BigInt>()),
            peak_tasks: amount(peak),
            occupancy_cost: if all_occupancy {
                amount(occupancy)
            } else {
                absent(w::Reason::NotConfigured)
            },
            start_cost: if all_starts {
                amount(start_fees)
            } else {
                absent(w::Reason::NotConfigured)
            },
            observed_cost_total: if all_cost {
                amount(total_cost.clone())
            } else {
                absent(w::Reason::NotConfigured)
            },
            observed_cost_per_arrival: if all_cost {
                rational(BigRational::new(
                    total_cost,
                    BigInt::from(observations.len()),
                ))
            } else {
                absent(w::Reason::NotConfigured)
            },
            business_value: w::BusinessValue {
                value: None,
                reason: w::Reason::NotConfigured,
            },
        },
        checked: [
            "lead_time_terminal_only",
            "first_start_wait",
            "task_ready_wait",
            "actor_work",
            "named_people_work",
            "peak_tasks",
            "capacity_window_utilization",
            "declared_rates",
            "observed_processing_cost",
            "outcome_counts",
        ]
        .into_iter()
        .chain(
            (!m.resources.is_empty())
                .then_some([
                    "resource_occupancy_time",
                    "resource_peak_units",
                    "resource_capacity_window_utilization",
                ])
                .into_iter()
                .flatten(),
        )
        .map(String::from)
        .collect(),
        unchecked: [
            "quality",
            "business_value",
            "waiting_decomposition",
            "queue_length_statistics",
            "quantiles",
            "exact_expectations",
            "metric_constraints",
            "metric_intervals",
            "anonymous_human_work",
            "calendar",
            "rework_loss",
            "introduction_cost",
            "terminal_meaning",
        ]
        .map(String::from)
        .to_vec(),
    };
    if s.breakdown_waits {
        waits::collect(m, observations, until, horizon, &mut report);
    }
    calendar::collect(m, observations, until, &mut report);
    tariff::collect(m, observations, until, &mut report);
    report
}

#[cfg(test)]
#[path = "../../tests/support/p3a_wait_reference_tests.rs"]
mod p3a_wait_reference_tests;
