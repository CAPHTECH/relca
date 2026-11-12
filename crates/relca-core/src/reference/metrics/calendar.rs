//! Independent capacity accounting: sweep occupancy deltas inside each declared
//! interval rather than borrowing the engine's global history partition.
use super::{Observation, absent, amount, rational};
use crate::{ir, metrics as w};
use num_bigint::BigInt;
use num_rational::BigRational;
use std::collections::BTreeMap;

pub(super) fn collect(
    m: &ir::Model,
    observations: &[Observation<'_>],
    until: u64,
    report: &mut w::BusinessMetrics,
) {
    let Some(t) = &m.resource_time else { return };
    let sum = |calendar: usize, target: usize, kind: u8| {
        let mut available = BigInt::from(0);
        let mut ordinary = BigInt::from(0);
        let mut extra = BigInt::from(0);
        for row in &t.calendars[calendar].intervals {
            let end = row.until.min(until);
            if row.from >= end {
                continue;
            }
            let mut events = BTreeMap::<u64, BigInt>::new();
            events.insert(row.from, BigInt::from(0));
            events.insert(end, BigInt::from(0));
            available += BigInt::from(row.capacity) * BigInt::from(end - row.from);
            for task in observations.iter().flat_map(|o| o.tasks) {
                if m.site_contest(task.call.binding).is_some() {
                    continue;
                }
                let Some(start) = task.start else { continue };
                let a = start.max(row.from);
                let b = task.stop.unwrap_or(until).min(end);
                if a >= b {
                    continue;
                }
                let cap = &m.capabilities[task.call.capability.0];
                let units: BigInt = match kind {
                    0 => BigInt::from(u64::from(cap.actor.0 == target)),
                    1 => cap
                        .uses
                        .iter()
                        .filter(|u| u.resource.0 == target)
                        .map(|u| BigInt::from(u.amount))
                        .sum(),
                    _ => BigInt::from(u64::from(task.person == Some(ir::PersonId(target)))),
                };
                *events.entry(a).or_default() += &units;
                *events.entry(b).or_default() -= units;
            }
            let mut used = BigInt::from(0);
            let mut previous = row.from;
            for (at, delta) in events {
                let duration = BigInt::from(at - previous);
                let limit = BigInt::from(row.capacity);
                ordinary += used.clone().min(limit.clone()) * &duration;
                extra += (&used - limit).max(BigInt::from(0)) * duration;
                used += delta;
                previous = at;
            }
        }
        (available, ordinary, extra)
    };
    let utilization = |n: BigInt, d: BigInt| {
        if d == BigInt::from(0) {
            absent(w::Reason::ZeroCapacity)
        } else {
            rational(BigRational::new(n, d))
        }
    };
    for row in &mut report.actors {
        let id = m
            .actors
            .iter()
            .position(|a| a.name == row.actor)
            .expect("reference Actor");
        if let Some(c) = t.actors[id] {
            let (capacity, regular, overtime) = sum(c, id, 0);
            row.scheduled_utilization = Some(utilization(regular.clone(), capacity.clone()));
            row.scheduled_capacity_time = Some(amount(capacity));
            row.scheduled_occupancy_time = Some(amount(regular));
            row.overtime_occupancy_time = Some(amount(overtime));
        }
    }
    for row in report.resources.iter_mut().flatten() {
        let id = m
            .resources
            .iter()
            .position(|r| r.name == row.resource)
            .expect("reference resource");
        if let Some(c) = t.resources[id] {
            let (capacity, regular, overtime) = sum(c, id, 1);
            row.scheduled_utilization = Some(utilization(regular.clone(), capacity.clone()));
            row.scheduled_capacity_time = Some(amount(capacity));
            row.scheduled_occupancy_time = Some(amount(regular));
            row.overtime_occupancy_time = Some(amount(overtime));
        }
    }
    for row in &mut report.people {
        let id = m
            .people
            .iter()
            .position(|p| p.name == row.person)
            .expect("reference person");
        if let Some(c) = t.people[id] {
            row.overtime_work = Some(amount(sum(c, id, 2).2));
        }
    }
    report.checked.extend([
        "calendar_start_gates".into(),
        "calendar_capacity_integrals".into(),
    ]);
    report.unchecked.retain(|s| s != "calendar");
    report.unchecked.extend([
        "calendar_preemption".into(),
        "person_overtime_pricing".into(),
    ]);
}
