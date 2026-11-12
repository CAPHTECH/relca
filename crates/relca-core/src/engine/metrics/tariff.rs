//! Task-local tariff classification, distinct from aggregate slot overrun.
use super::{
    CaseObservation,
    calendar::{calls, scalar},
};
use crate::ir;
use num_bigint::BigInt;
use num_rational::BigRational;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn money(n: &BigInt, rate: Option<u64>, optional_zero: bool) -> Option<BigInt> {
    if optional_zero && *n == BigInt::from(0) {
        Some(BigInt::from(0))
    } else {
        rate.map(|r| n * BigInt::from(r))
    }
}
fn price_value(n: Option<BigInt>) -> Value {
    n.map_or_else(
        || json!({"min":null,"max":null,"reason":"not_configured"}),
        scalar,
    )
}
pub(super) fn pricing(m: &ir::Model, cases: &[CaseObservation<'_>], until: u64, value: &mut Value) {
    let tables = m.resource_time.as_ref().unwrap();
    let spec = m.metrics.as_ref().unwrap();
    let mut sites = Vec::new();
    calls(&m.process.body, &mut sites);
    let mut ordinary = vec![BigInt::from(0); m.actors.len()];
    let mut extra = ordinary.clone();
    let mut per_case = Vec::new();
    let base_configured = spec
        .required_actors
        .iter()
        .all(|id| spec.actor_rates[id.0].is_some())
        && spec
            .required_capabilities
            .iter()
            .all(|id| spec.start_rates[id.0].is_some());
    for (index, case) in cases.iter().enumerate() {
        let mut cost = base_configured.then(|| BigInt::from(0));
        for call in &sites {
            let cap = &m.capabilities[call.capability.0];
            let history = case
                .state
                .and_then(|s| s.metric_history.as_ref())
                .map(|h| &h.tasks[call.binding.0]);
            let mut regular = BigInt::from(0);
            let mut overtime = BigInt::from(0);
            if let Some(h) = history
                && let Some(start) = h.start
            {
                let end = h.stop.unwrap_or(until).min(until);
                let mut points = BTreeSet::from([start.min(end), end]);
                for i in tables.calendars.iter().flat_map(|c| &c.intervals) {
                    if start < i.from && i.from < end {
                        points.insert(i.from);
                    }
                }
                let points: Vec<_> = points.into_iter().collect();
                for pair in points.windows(2) {
                    let (at, length) = (pair[0], BigInt::from(pair[1] - pair[0]));
                    let enough = |id: Option<usize>, need| {
                        id.is_none_or(|id| {
                            tables.calendars[id]
                                .capacity_at(at)
                                .is_some_and(|n| n >= need)
                        })
                    };
                    let fits = enough(tables.actors[cap.actor.0], 1)
                        && cap
                            .uses
                            .iter()
                            .all(|u| enough(tables.resources[u.resource.0], u.amount))
                        && h.executor.is_none_or(|p| enough(tables.people[p.0], 1));
                    if fits {
                        regular += length;
                    } else {
                        overtime += length;
                    }
                }
            }
            ordinary[cap.actor.0] += &regular;
            extra[cap.actor.0] += &overtime;
            let normal_cost = money(&regular, spec.actor_rates[cap.actor.0], false);
            let extra_cost = money(&overtime, tables.overtime_rates[cap.actor.0], true);
            let started = history.is_some_and(|h| h.start.is_some());
            if started {
                cost = cost.and_then(|c| {
                    Some(
                        c + normal_cost.clone()?
                            + extra_cost.clone()?
                            + BigInt::from(spec.start_rates[call.capability.0]?),
                    )
                });
            }
            if let Some(row) = value["cases"][index]["tasks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|r| r["binding"] == m.process.bindings[call.binding.0].name)
            {
                let work = row["work"].clone();
                row["tariff_regular_work"] = if started {
                    scalar(regular)
                } else {
                    work.clone()
                };
                row["tariff_overtime_work"] = if started {
                    scalar(overtime)
                } else {
                    work.clone()
                };
                row["tariff_regular_cost"] = if started {
                    price_value(normal_cost)
                } else {
                    work.clone()
                };
                row["tariff_overtime_cost"] = if started {
                    price_value(extra_cost)
                } else {
                    work
                };
            }
        }
        value["cases"][index]["observed_cost_total"] = price_value(cost.clone());
        if let Some(end) = case.end {
            for row in value["cases"][index]["terminals"].as_array_mut().unwrap() {
                if row["end"] == m.ends[end.0] {
                    row["cost_per_case"] = price_value(cost.clone());
                }
            }
        }
        per_case.push(cost);
    }
    let mut occupancy = Some(BigInt::from(0));
    for row in value["actors"].as_array_mut().unwrap() {
        let id = m
            .actors
            .iter()
            .position(|a| row["actor"] == a.name)
            .unwrap();
        let normal = money(&ordinary[id], spec.actor_rates[id], false);
        let over = money(&extra[id], tables.overtime_rates[id], true);
        let cost = normal.clone().and_then(|n| Some(n + over.clone()?));
        row["tariff_regular_work"] = scalar(ordinary[id].clone());
        row["tariff_overtime_work"] = scalar(extra[id].clone());
        row["tariff_regular_cost"] = price_value(normal);
        row["tariff_overtime_cost"] = price_value(over);
        row["occupancy_cost"] = price_value(cost.clone());
        occupancy = occupancy.and_then(|a| Some(a + cost?));
    }
    value["overall"]["occupancy_cost"] = price_value(occupancy);
    let total: Option<BigInt> = per_case.iter().cloned().sum();
    value["overall"]["observed_cost_total"] = price_value(total.clone());
    value["overall"]["observed_cost_per_arrival"] = total.map_or_else(
        || price_value(None),
        |n| {
            let s = BigRational::new(n, BigInt::from(cases.len())).to_string();
            json!({"min":s,"max":s,"reason":null})
        },
    );
    for row in value["terminals"].as_array_mut().unwrap() {
        let ids: Vec<_> = cases
            .iter()
            .enumerate()
            .filter(|(_, c)| c.end.is_some_and(|e| row["end"] == m.ends[e.0]))
            .map(|(i, _)| i)
            .collect();
        if ids.is_empty() {
            continue;
        }
        let cost: Option<BigInt> = ids.iter().map(|i| per_case[*i].clone()).sum();
        row["cost_per_case"] = cost.map_or_else(
            || price_value(None),
            |n| {
                let s = BigRational::new(n, BigInt::from(ids.len())).to_string();
                json!({"min":s,"max":s,"reason":null})
            },
        );
    }
}
