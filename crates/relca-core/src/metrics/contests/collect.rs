use super::super::MetricNumber;
use super::*;
use crate::{ir, outcome::SimulationOutcome};

fn absent(reason: Reason) -> Sample {
    Sample {
        mean: None,
        min: None,
        max: None,
        samples: 0,
        reason: Some(reason),
    }
}
fn sample(values: &[u64], withdrawn: Option<Reason>) -> Sample {
    if let Some(reason) = withdrawn {
        return absent(reason);
    }
    if values.is_empty() {
        return absent(Reason::NoSamples);
    }
    let sum: num_bigint::BigInt = values.iter().copied().map(num_bigint::BigInt::from).sum();
    Sample {
        mean: Some(MetricNumber(num_rational::BigRational::new(
            sum,
            values.len().into(),
        ))),
        min: values.iter().min().copied().map(Into::into),
        max: values.iter().max().copied().map(Into::into),
        samples: values.len() as u64,
        reason: None,
    }
}
fn increment(n: &mut Option<u64>) {
    *n = n.map(|n| n + 1);
}

fn unknown_reason(previous: Option<Reason>, cut: Option<crate::outcome::SimulationCut>) -> Reason {
    // Only call for a row that was unknown in this replication. A cut in a
    // known replication (or another row) cannot explain this row's unknowns.
    if previous == Some(Reason::Truncated) || cut.is_some() {
        Reason::Truncated
    } else {
        Reason::Unfinished
    }
}

fn row(m: &ir::Model, id: usize, case_id: u64, trials: u64, aborted: bool) -> Row {
    let d = &m.contests()[id];
    let rates = d.rates.as_ref().unwrap();
    let count = (!aborted).then_some(0);
    let reason = if aborted {
        Reason::Aborted
    } else {
        Reason::NoSamples
    };
    Row {
        id: d.property_id(),
        case_id,
        party: m.actors[d.party.0].name.clone(),
        target: d.target.name(m),
        route: crate::responsibility::cap_name(m, d.route),
        configured_probability: rates.probability.clone(),
        occurrence_basis: rates.occurrence_basis.clone(),
        service_time_basis: rates.service_time_basis.clone(),
        independence: rates.independence.clone(),
        cohort: "normal_response_observed".into(),
        trials,
        target_observed: count,
        target_not_reached: count,
        target_unknown: count,
        filed: count,
        not_filed: count,
        choice_unknown: count,
        responded: count,
        late_responded: count,
        unresolved: count,
        route_failed: count,
        filing_fraction: Fraction {
            value: None,
            numerator: count,
            denominator: count,
            reason: Some(reason),
        },
        service_time: absent(reason),
        queue_wait: absent(reason),
        response_elapsed: absent(reason),
    }
}

/// Summarize only independent replications. Do not infer frequency from the
/// exhaustive alternatives table or turn unobserved outcomes into zero counts.
pub(crate) fn summarize(
    m: &ir::Model,
    sampled: &SimulationOutcome,
    aborted: bool,
) -> Result<Option<Table>, String> {
    let ids: Vec<_> = m
        .contests()
        .iter()
        .enumerate()
        .filter_map(|(id, d)| d.rates.as_ref().map(|_| id))
        .collect();
    if m.metrics.is_none() || ids.is_empty() {
        return Ok(None);
    }
    let trials = m
        .simulation
        .as_ref()
        .ok_or("missing contest simulation configuration")?
        .replications;
    if !aborted {
        ensure(
            sampled.completed_replications == trials && sampled.replications.len() as u64 == trials,
        )?;
        for (i, rep) in sampled.replications.iter().enumerate() {
            ensure(
                rep.replication == i as u64 + 1
                    && rep.contests.len() as u64 == ids.len() as u64 * m.bounds.cases,
            )?;
        }
    }
    let mut rows = Vec::new();
    for id in ids {
        for case in 1..=m.bounds.cases {
            let mut row = row(m, id, case, trials, aborted);
            if !aborted {
                let mut service = Vec::new();
                let mut queue = Vec::new();
                let mut elapsed = Vec::new();
                let mut filing_withdrawn = None;
                let mut response_withdrawn = None;
                for rep in &sampled.replications {
                    let observations: Vec<_> = rep
                        .contests
                        .iter()
                        .filter(|c| c.contest == id && c.case_id == case)
                        .collect();
                    ensure(observations.len() == 1)?;
                    let c = observations[0];
                    let filing_unknown =
                        !c.target_known || (c.target_at.is_some() && c.filed.is_none());
                    if filing_unknown {
                        filing_withdrawn = Some(unknown_reason(filing_withdrawn, rep.cut));
                    }
                    if filing_unknown
                        || (c.filed == Some(true) && c.response_at.is_none() && !c.failed)
                    {
                        response_withdrawn = Some(unknown_reason(response_withdrawn, rep.cut));
                    }
                    let Some(target) = c.target_at else {
                        ensure(
                            c.filed.is_none()
                                && c.started_at.is_none()
                                && c.response_at.is_none()
                                && !c.failed,
                        )?;
                        increment(if c.target_known {
                            &mut row.target_not_reached
                        } else {
                            &mut row.target_unknown
                        });
                        continue;
                    };
                    ensure(c.target_known)?;
                    increment(&mut row.target_observed);
                    match c.filed {
                        None => {
                            increment(&mut row.choice_unknown);
                            ensure(c.started_at.is_none() && c.response_at.is_none() && !c.failed)?;
                        }
                        Some(false) => {
                            increment(&mut row.not_filed);
                            ensure(c.started_at.is_none() && c.response_at.is_none() && !c.failed)?;
                        }
                        Some(true) => {
                            increment(&mut row.filed);
                            if let Some(response) = c.response_at {
                                let start = c.started_at.ok_or("response without route start")?;
                                ensure(!c.failed && target <= start && start <= response)?;
                                service.push(response - start);
                                queue.push(start - target);
                                elapsed.push(response - target);
                                let deadline = target
                                    .checked_add(m.contests()[id].respond_by)
                                    .ok_or("contest deadline overflow")?;
                                increment(if response <= deadline {
                                    &mut row.responded
                                } else {
                                    &mut row.late_responded
                                });
                            } else {
                                increment(if c.failed {
                                    &mut row.route_failed
                                } else {
                                    &mut row.unresolved
                                });
                            }
                        }
                    }
                }
                let d = row.target_observed.unwrap();
                let unknown = row.target_unknown != Some(0) || row.choice_unknown != Some(0);
                // A cut elsewhere cannot make this row's fully known cohort unknown.
                row.service_time = sample(&service, response_withdrawn);
                row.queue_wait = sample(&queue, response_withdrawn);
                row.response_elapsed = sample(&elapsed, response_withdrawn);
                row.filing_fraction = Fraction {
                    value: (!unknown && d > 0).then(|| fraction(row.filed.unwrap(), d)),
                    numerator: row.filed,
                    denominator: row.target_observed,
                    reason: if unknown {
                        Some(filing_withdrawn.unwrap_or(Reason::Unfinished))
                    } else if d == 0 {
                        Some(Reason::NoSamples)
                    } else {
                        None
                    },
                };
            }
            rows.push(row);
        }
    }
    rows.sort_by(|a, b| (&a.id, a.case_id).cmp(&(&b.id, b.case_id)));
    let table = Table {
        rules: RULES.into(),
        basis: "sample".into(),
        unit: "replication".into(),
        scope: "declared-model-not-party-representation".into(),
        rows,
    };
    table.validate()?;
    Ok(Some(table))
}
