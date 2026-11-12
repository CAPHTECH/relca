//! Independent tariff integration by unioning the Task's off-schedule intervals.
use super::{Observation, absent, amount, rational};
use crate::{ir, metrics as w};
use num_bigint::BigInt;
use num_rational::BigRational;

fn charge(work: &BigInt, rate: Option<u64>, extra: bool) -> Option<BigInt> {
    match (rate, extra && *work == BigInt::from(0)) {
        (_, true) => Some(BigInt::from(0)),
        (Some(r), _) => Some(work * BigInt::from(r)),
        _ => None,
    }
}
fn cost(n: Option<BigInt>) -> w::Scalar {
    n.map_or_else(|| absent(w::Reason::NotConfigured), amount)
}

pub(super) fn collect(
    m: &ir::Model,
    observations: &[Observation<'_>],
    until: u64,
    report: &mut w::BusinessMetrics,
) {
    let Some(tables) = &m.resource_time else {
        return;
    };
    let fees = m.metrics.as_ref().unwrap();
    let configured = fees
        .required_actors
        .iter()
        .all(|a| fees.actor_rates[a.0].is_some())
        && fees
            .required_capabilities
            .iter()
            .all(|c| fees.start_rates[c.0].is_some());
    let mut normal_totals = vec![BigInt::from(0); m.actors.len()];
    let mut extra_totals = normal_totals.clone();
    let mut case_costs = Vec::new();
    for (row, observation) in report.cases.iter_mut().zip(observations) {
        let mut total = configured.then(|| BigInt::from(0));
        for task_row in &mut row.tasks {
            let actual = observation.tasks.iter().find(|t| {
                m.process
                    .bindings
                    .get(t.call.binding.0)
                    .is_some_and(|b| b.name == task_row.binding)
            });
            let Some(task) = actual.filter(|t| t.start.is_some()) else {
                task_row.tariff_regular_work = Some(task_row.work.clone());
                task_row.tariff_overtime_work = Some(task_row.work.clone());
                task_row.tariff_regular_cost = Some(task_row.work.clone());
                task_row.tariff_overtime_cost = Some(task_row.work.clone());
                continue;
            };
            let cap = &m.capabilities[task.call.capability.0];
            let start = task.start.unwrap();
            let end = task.stop.unwrap_or(until).min(until);
            let mut off = Vec::new();
            let mut append = |calendar: Option<usize>, need: u64| {
                if let Some(id) = calendar {
                    for interval in &tables.calendars[id].intervals {
                        if interval.capacity < need {
                            let a = interval.from.max(start);
                            let b = interval.until.min(end);
                            if a < b {
                                off.push((a, b));
                            }
                        }
                    }
                }
            };
            append(tables.actors[cap.actor.0], 1);
            for usage in &cap.uses {
                append(tables.resources[usage.resource.0], usage.amount);
            }
            if let Some(p) = task.person {
                append(tables.people[p.0], 1);
            }
            off.sort_unstable();
            let mut last = start;
            let mut overtime = BigInt::from(0);
            for (a, b) in off {
                if b > last {
                    overtime += b - a.max(last);
                    last = b;
                }
            }
            let regular = BigInt::from(end - start) - &overtime;
            normal_totals[cap.actor.0] += &regular;
            extra_totals[cap.actor.0] += &overtime;
            let normal_cost = charge(&regular, fees.actor_rates[cap.actor.0], false);
            let extra_cost = charge(&overtime, tables.overtime_rates[cap.actor.0], true);
            total = total.and_then(|mut n| {
                n += normal_cost.clone()?;
                n += extra_cost.clone()?;
                n += BigInt::from(fees.start_rates[task.call.capability.0]?);
                Some(n)
            });
            task_row.tariff_regular_work = Some(amount(regular));
            task_row.tariff_overtime_work = Some(amount(overtime));
            task_row.tariff_regular_cost = Some(cost(normal_cost));
            task_row.tariff_overtime_cost = Some(cost(extra_cost));
        }
        row.observed_cost_total = cost(total.clone());
        if let Some((end, _)) = observation.terminal {
            for terminal in &mut row.terminals {
                if terminal.end.as_deref() == Some(m.ends[end.0].as_str()) {
                    terminal.cost_per_case = cost(total.clone());
                }
            }
        }
        case_costs.push(total);
    }
    let mut occupancy = Some(BigInt::from(0));
    for actor in &mut report.actors {
        let i = m.actors.iter().position(|a| a.name == actor.actor).unwrap();
        let normal = charge(&normal_totals[i], fees.actor_rates[i], false);
        let extra = charge(&extra_totals[i], tables.overtime_rates[i], true);
        let sum = normal.clone().and_then(|n| extra.clone().map(|e| n + e));
        occupancy = occupancy.and_then(|n| sum.clone().map(|e| n + e));
        actor.occupancy_cost = cost(sum);
        actor.tariff_regular_work = Some(amount(normal_totals[i].clone()));
        actor.tariff_overtime_work = Some(amount(extra_totals[i].clone()));
        actor.tariff_regular_cost = Some(cost(normal));
        actor.tariff_overtime_cost = Some(cost(extra));
    }
    report.overall.occupancy_cost = cost(occupancy);
    let total: Option<BigInt> = case_costs.iter().cloned().sum();
    report.overall.observed_cost_total = cost(total.clone());
    report.overall.observed_cost_per_arrival = total.map_or_else(
        || cost(None),
        |n| rational(BigRational::new(n, BigInt::from(observations.len()))),
    );
    for terminal in &mut report.terminals {
        let ids: Vec<_> = observations
            .iter()
            .enumerate()
            .filter(|(_, o)| o.terminal.is_some_and(|(e, _)| terminal.end == m.ends[e.0]))
            .map(|(i, _)| i)
            .collect();
        if !ids.is_empty() {
            let total: Option<BigInt> = ids.iter().map(|i| case_costs[*i].clone()).sum();
            terminal.cost_per_case = total.map_or_else(
                || cost(None),
                |n| rational(BigRational::new(n, BigInt::from(ids.len()))),
            );
        }
    }
}
