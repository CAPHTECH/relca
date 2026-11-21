//! Reconstruct rewards from past reference tasks, independently of engine histories.
use super::*;
use crate::probability::Rational;

pub(in crate::reference) fn mean_amount(
    m: &ir::Model,
    tasks: &[Task],
    until: u64,
    measure: ir::MeanMeasure,
) -> Option<Rational> {
    let s = m.metrics.as_ref()?;
    if measure == ir::MeanMeasure::Cost
        && (!s
            .required_capabilities
            .iter()
            .all(|c| s.start_rates[c.0].is_some())
            || !s
                .required_actors
                .iter()
                .all(|a| s.actor_rates[a.0].is_some()))
    {
        return None;
    }
    let mut value = BigInt::from(0);
    for t in tasks {
        let Some(start) = t.start else { continue };
        let end = t.stop.unwrap_or(until).min(until);
        let work = BigInt::from(end.saturating_sub(start));
        match measure {
            ir::MeanMeasure::Work => value += work,
            ir::MeanMeasure::NamedPeopleWork => {
                if t.person.is_some() {
                    value += work;
                }
            }
            ir::MeanMeasure::Cost => {
                let cap = &m.capabilities[t.call.capability.0];
                let mut overtime = BigInt::from(0);
                if let Some(tables) = &m.resource_time {
                    let mut intervals = Vec::new();
                    let mut add = |id: Option<usize>, need| {
                        if let Some(id) = id {
                            for i in &tables.calendars[id].intervals {
                                let (a, b) = (start.max(i.from), end.min(i.until));
                                if i.capacity < need && a < b {
                                    intervals.push((a, b));
                                }
                            }
                        }
                    };
                    add(tables.actors[cap.actor.0], 1);
                    for u in &cap.uses {
                        add(tables.resources[u.resource.0], u.amount);
                    }
                    if let Some(p) = t.person {
                        add(tables.people[p.0], 1);
                    }
                    intervals.sort_unstable();
                    let mut last = start;
                    for (a, b) in intervals {
                        if b > last {
                            overtime += b - a.max(last);
                            last = b;
                        }
                    }
                }
                value += (&work - &overtime) * BigInt::from(s.actor_rates[cap.actor.0]?);
                if overtime != BigInt::from(0) {
                    value += overtime
                        * BigInt::from(m.resource_time.as_ref()?.overtime_rates[cap.actor.0]?);
                }
                value += BigInt::from(s.start_rates[t.call.capability.0]?);
            }
            ir::MeanMeasure::Elapsed => unreachable!("elapsed is observed independently"),
        }
    }
    Some(Rational(BigRational::from_integer(value)))
}
