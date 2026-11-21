//! Scalar projection of this node's task history, without building a report.
use super::*;
use crate::outcome::SimulationCut;
use crate::probability::Rational;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;

pub(in crate::engine) fn mean_amount(
    m: &ir::Model,
    state: Option<&State>,
    until: u64,
    measure: ir::MeanMeasure,
) -> Option<Rational> {
    let fees = m.metrics.as_ref()?;
    if measure == ir::MeanMeasure::Cost
        && (fees
            .required_actors
            .iter()
            .any(|a| fees.actor_rates[a.0].is_none())
            || fees
                .required_capabilities
                .iter()
                .any(|c| fees.start_rates[c.0].is_none()))
    {
        return None;
    }
    let mut sum = BigInt::from(0);
    let mut calls = Vec::new();
    calendar::calls(&m.process.body, &mut calls);
    let mut sites: Vec<_> = calls
        .into_iter()
        .map(|c| (c.binding, c.capability))
        .collect();
    sites.extend(
        m.contests()
            .iter()
            .enumerate()
            .map(|(i, c)| (m.contest_site(i), c.route)),
    );
    if let Some(history) = state.and_then(|s| s.metric_history.as_ref()) {
        for (binding, capability) in sites {
            let h = &history.tasks[binding.0];
            let Some(start) = h.start else { continue };
            let stop = h.stop.unwrap_or(until).min(until);
            let occupied = BigInt::from(stop.saturating_sub(start));
            match measure {
                ir::MeanMeasure::Work => sum += occupied,
                ir::MeanMeasure::NamedPeopleWork => {
                    if h.executor.is_some() {
                        sum += occupied;
                    }
                }
                ir::MeanMeasure::Cost => {
                    let cap = &m.capabilities[capability.0];
                    let mut cost = &occupied * BigInt::from(fees.actor_rates[cap.actor.0]?);
                    if let Some(tables) = &m.resource_time {
                        let mut points = std::collections::BTreeSet::from([start, stop]);
                        for i in tables.calendars.iter().flat_map(|c| &c.intervals) {
                            for t in [i.from, i.until] {
                                if start < t && t < stop {
                                    points.insert(t);
                                }
                            }
                        }
                        let points: Vec<_> = points.into_iter().collect();
                        cost = BigInt::from(0);
                        for pair in points.windows(2) {
                            let enough = |id: Option<usize>, need| {
                                id.is_none_or(|i| {
                                    tables.calendars[i]
                                        .capacity_at(pair[0])
                                        .is_some_and(|n| n >= need)
                                })
                            };
                            let regular = enough(tables.actors[cap.actor.0], 1)
                                && cap
                                    .uses
                                    .iter()
                                    .all(|u| enough(tables.resources[u.resource.0], u.amount))
                                && h.executor.is_none_or(|p| enough(tables.people[p.0], 1));
                            let rate = if regular {
                                fees.actor_rates[cap.actor.0]
                            } else {
                                tables.overtime_rates[cap.actor.0]
                            }?;
                            cost += BigInt::from(pair[1] - pair[0]) * BigInt::from(rate);
                        }
                    }
                    sum += cost + BigInt::from(fees.start_rates[capability.0]?);
                }
                ir::MeanMeasure::Elapsed => unreachable!("elapsed is a first-hit clock"),
            }
        }
    }
    Some(Rational(BigRational::from_integer(sum)))
}

/// First-hit amounts live outside the simulated State (and its equality).
#[derive(Clone, PartialEq, Eq)]
pub(in crate::engine) struct MeanPath(Vec<Vec<Option<Option<Rational>>>>);

impl MeanPath {
    pub(in crate::engine) fn new(m: &ir::Model, cases: usize) -> Self {
        Self(vec![vec![None; cases]; mean_specs(m).len()])
    }

    pub(in crate::engine) fn observe(
        &mut self,
        m: &ir::Model,
        case: usize,
        hit: impl Fn(usize) -> Option<u128>,
        state: Option<&State>,
        at: u64,
        closed: bool,
    ) {
        for (c, row) in mean_specs(m).iter().zip(&mut self.0) {
            if row[case].is_some() {
                continue;
            }
            let t = c.property.and_then(&hit);
            if t.is_some() || c.property.is_none() && closed {
                row[case] = Some(if c.measure == ir::MeanMeasure::Elapsed {
                    t.map(|n| Rational(BigRational::from_integer(n.into())))
                } else {
                    mean_amount(m, state, at, c.measure)
                });
            }
        }
    }
}

fn mean_specs(m: &ir::Model) -> &[ir::MeanMetric] {
    m.quality.as_ref().map_or(&[], |q| &q.means)
}

#[derive(Default)]
struct Tally {
    sum: BigRational,
    observed: u64,
    unconfigured: bool,
    missed: bool,
}

pub(in crate::engine) struct MeanSamples {
    tallies: Vec<Tally>,
    censored: u64,
    truncated: bool,
}

impl MeanSamples {
    pub(in crate::engine) fn new(m: &ir::Model) -> Self {
        Self {
            tallies: mean_specs(m)
                .iter()
                .map(|c| Tally {
                    unconfigured: c.measure != ir::MeanMeasure::Elapsed
                        && mean_amount(m, None, 0, c.measure).is_none(),
                    ..Tally::default()
                })
                .collect(),
            censored: 0,
            truncated: false,
        }
    }

    pub(in crate::engine) fn add(
        &mut self,
        path: MeanPath,
        cut: Option<SimulationCut>,
        range_stopped: bool,
    ) {
        if let Some(row) = path.0.first()
            && (cut.is_some() || range_stopped)
        {
            self.censored += row.len() as u64;
            self.truncated |= range_stopped || cut != Some(SimulationCut::Horizon);
            return;
        }
        for (tally, cases) in self.tallies.iter_mut().zip(path.0) {
            for case in cases {
                match case {
                    Some(Some(v)) => {
                        tally.sum += v.0;
                        tally.observed += 1;
                    }
                    Some(None) => tally.unconfigured = true,
                    None => tally.missed = true,
                }
            }
        }
    }

    pub(in crate::engine) fn finish(self, m: &ir::Model, q: &mut crate::quality::Quality) {
        if self.tallies.is_empty() || m.actors.iter().any(|a| a.metadata.rule.is_some()) {
            return;
        }
        q.means = Some(
            mean_specs(m)
                .iter()
                .zip(self.tallies)
                .map(|(c, t)| {
                    let reason = if self.truncated {
                        Some("truncated")
                    } else if self.censored != 0 {
                        Some("undetermined")
                    } else if t.unconfigured {
                        Some("not_configured")
                    } else if t.missed && !c.given_reached {
                        Some("beyond_reached_mass")
                    } else if t.observed == 0 {
                        Some("zero_denominator")
                    } else {
                        None
                    };
                    let point = reason.is_none().then(|| {
                        debug_assert!(!t.observed.is_zero());
                        (t.sum / BigInt::from(t.observed)).to_string()
                    });
                    crate::quality::sample_mean_row(m, c, point, t.observed, self.censored, reason)
                })
                .collect(),
        );
    }
}
