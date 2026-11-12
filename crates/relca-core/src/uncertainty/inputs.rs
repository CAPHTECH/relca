//! Snapshot effective distributions only; never parse again or draw randomness.
use super::wire::*;
use crate::ir;

fn value(m: &ir::Model, variable: &ir::StateVariable, v: ir::StateValue) -> Option<String> {
    Some(match v {
        ir::StateValue::Value(v) => m.types.get(variable.base.0)?.members.get(v.0)?.clone(),
        ir::StateValue::None => "none".into(),
        ir::StateValue::Integer(v) => v.to_string(),
        ir::StateValue::Set(v) => format!("case_set_bits:{v}"),
    })
}

fn assignment(m: &ir::Model, id: ir::StateVarId, v: ir::StateValue) -> Option<Assignment> {
    let variable = m.state_variables.get(id.0)?;
    Some(Assignment {
        variable: variable.name.clone(),
        value: value(m, variable, v)?,
    })
}

pub(super) fn collect(m: &ir::Model) -> Option<Vec<Evidence>> {
    let mut items = Vec::new();
    for v in &m.state_variables {
        if let ir::InitialValue::Distribution(id) = v.initial {
            let entries = m
                .initial_distributions
                .get(id)?
                .iter()
                .map(|(val, p)| {
                    Some(Mass {
                        value: value(m, v, *val)?,
                        probability: p.clone(),
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            items.push(Evidence::InitialDistribution {
                variable: v.name.clone(),
                instance: if v
                    .kind
                    .actor()
                    .is_none_or(|a| m.actors.get(a.0).is_some_and(ir::Actor::is_per_case))
                {
                    Instance::PerCase
                } else {
                    Instance::SingleCase
                },
                span: v.span.into(),
                entries,
            });
        }
    }
    for c in &m.capabilities {
        let capability = format!("{}.{}", m.actors.get(c.actor.0)?.name, c.name);
        let mut seen_kernels = std::collections::BTreeSet::new();
        for kernel in c.result_kernels().filter(|k| seen_kernels.insert(&k.name)) {
            let given = kernel
                .given
                .iter()
                .map(|id| m.state_variables.get(id.0).map(|v| v.name.clone()))
                .collect::<Option<Vec<_>>>()?;
            let rows = kernel
                .rows
                .iter()
                .map(|row| {
                    if row.values.len() != kernel.given.len() {
                        return None;
                    }
                    let conditions = kernel
                        .given
                        .iter()
                        .zip(&row.values)
                        .map(|(id, v)| assignment(m, *id, *v))
                        .collect::<Option<Vec<_>>>()?;
                    let branches = row
                        .branches
                        .iter()
                        .map(|b| {
                            Some(Branch {
                                probability: b.probability.clone(),
                                result: m
                                    .types
                                    .get(c.return_type.0)?
                                    .members
                                    .get(b.result.0)?
                                    .clone(),
                                writes: b
                                    .overrides
                                    .iter()
                                    .map(|(id, v)| assignment(m, *id, *v))
                                    .collect::<Option<Vec<_>>>()?,
                            })
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some(KernelRow {
                        conditions,
                        branches,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            items.push(Evidence::Kernel {
                capability: capability.clone(),
                name: kernel.name.clone(),
                basis: kernel.basis.clone(),
                given,
                span: c.span.into(),
                rows,
            });
        }
        if let Some(d) = &c.duration_distribution {
            items.push(Evidence::DurationDistribution {
                capability,
                span: c.span.into(),
                entries: d
                    .entries
                    .iter()
                    .map(|(minutes, p)| DurationMass {
                        minutes: *minutes,
                        probability: p.clone(),
                    })
                    .collect(),
            });
        }
    }
    // response_attempts excludes the public non-executing result alias.
    for (call, attempt) in m.response_attempts() {
        if let ir::ResponseLoss::Probability { probability, basis } = &attempt.loss {
            items.push(Evidence::ResponseChannel {
                binding: m.process.bindings.get(call.binding.0)?.name.clone(),
                attempt: attempt.attempt,
                max_attempts: attempt.max_attempts,
                loss_probability: probability.clone(),
                basis: basis.clone(),
                span: call.span.into(),
            });
        }
    }
    if let Some(d) = &m.arrival_distribution {
        items.push(Evidence::ArrivalDistribution {
            basis: d.basis.clone(),
            span: d.span.into(),
            entries: d
                .entries
                .iter()
                .map(|(times, p)| ArrivalMass {
                    times: times.clone(),
                    probability: p.clone(),
                })
                .collect(),
        });
    }
    for d in m.contests() {
        if let Some(rates) = &d.rates {
            items.push(Evidence::ContestOccurrence {
                clause: d.property_id(),
                probability: rates.probability.clone(),
                occurrence_basis: rates.occurrence_basis.clone(),
                service_time_basis: rates.service_time_basis.clone(),
                span: d.span.into(),
            });
        }
    }
    Some(items)
}
