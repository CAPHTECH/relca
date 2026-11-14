//! F uses exact relations over conjunctive components. Contexts retain the
//! legacy BTreeSet projection and new-binding freedom across distinct paths.
use super::*;

pub(super) fn derive(m: &ir::Model, b: &mut Budget, shared: bool) -> Result<Framing> {
    let result = (|| {
        let (calls, r) = recorded_shared(m, b, shared)?;
        from_recording(m, &calls, r, b)
    })();
    record_budget("agency.framing", b);
    result
}

pub(super) fn from_recording(
    m: &ir::Model,
    calls: &[&ir::Call],
    r: Recorder<'_>,
    b: &mut Budget,
) -> Result<Framing> {
    let capacity = ActorCapacity::new(m, calls, b)?;
    let infeasible = if precise_resources(m) {
        BTreeSet::new()
    } else {
        infeasible_capabilities(m)?
    };
    let mut ends = Vec::new();
    let mut actors = BTreeSet::new();
    let mut complete = true;
    for (i, end) in m.ends.iter().enumerate() {
        if m.overflow_end == Some(ir::EndId(i)) {
            continue;
        }
        b.location.phase = "framing reachability".into();
        b.location.effect = Some(end.clone());
        b.location.judgement = None;
        b.copy(1)?;
        let mut ps = Vec::new();
        for (id, p) in &r.sinks {
            b.work()?;
            if *id == Some(ir::EndId(i)) {
                b.copy(1)?;
                ps.push(p);
            }
        }
        let mut blocked = false;
        for p in &ps {
            if first_infeasible(p, calls, &infeasible, b)?.is_some() {
                blocked = true;
                break;
            }
        }
        if blocked {
            complete = false;
            ends.push(End::NotDerived {
                end: end.clone(),
                reason: StateReason::ResourceInfeasible,
            });
            continue;
        }
        let mut actor_blocked = false;
        for p in &ps {
            if capacity.first_stage(p, b)?.is_some() && possibly_satisfied(p, calls, m, b)? {
                actor_blocked = true;
                break;
            }
        }
        if actor_blocked {
            complete = false;
            ends.push(End::NotDerived {
                end: end.clone(),
                reason: StateReason::ActorCapacityInfeasible,
            });
            continue;
        }
        if ps.iter().any(|p| uncertain(p)) {
            complete = false;
            ends.push(End::NotDerived {
                end: end.clone(),
                reason: StateReason::StateGuards,
            });
            continue;
        }
        let mut support = BTreeSet::new();
        let mut reachable = Vec::new();
        for p in ps {
            let local = path_support(p, m, b)?;
            let factor = Factorization::build_viewpoint(p, &local, calls, m, b)?;
            if exists(p, &local, factor.as_ref(), &Values::new(), calls, m, b)? {
                for id in read_support(p, m, b)? {
                    b.insert(&mut support, id)?;
                }
                b.copy(1)?;
                reachable.push((p, local, factor));
            }
        }
        let mut selectors = Vec::new();
        let mut read_only = Vec::new();
        for j in &support {
            b.location.phase = "framing contexts".into();
            b.location.judgement = Some(m.process.bindings[j.0].name.clone());
            let mut contexts = BTreeSet::new();
            for (p, local, factor) in &reachable {
                b.copy(
                    support
                        .len()
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(local.len()))
                        .ok_or_else(invalid)?,
                )?;
                let vars: Vec<_> = local
                    .union(&support.intersection(&p.defined).copied().collect())
                    .copied()
                    .collect();
                let mut visit = |v: &Values, b: &mut Budget| {
                    b.copy(v.len())?;
                    let context: Values = v
                        .iter()
                        .filter(|(id, _)| {
                            **id != *j && support.contains(id) && p.defined.contains(id)
                        })
                        .map(|(id, v)| (*id, *v))
                        .collect();
                    if !contexts.contains(&context) {
                        b.context()?;
                        b.cells(1)?;
                        contexts.insert(context);
                    }
                    Ok(false)
                };
                if let Some(factor) = factor {
                    factor.enumerate(&vars, calls, m, b, &mut visit)?;
                } else {
                    assignments(&vars, &Values::new(), calls, m, b, |v, b| {
                        if satisfied(p, v, b)? {
                            visit(v, b)?;
                        }
                        Ok(false)
                    })?;
                }
            }
            b.location.phase = "framing stopping".into();
            let mut selecting = false;
            let mut bypass = false;
            let mut has_blocked = false;
            for context in &contexts {
                let mut may = false;
                let mut cannot = false;
                for value in &m.capabilities[calls[j.0].capability.0].results {
                    b.work()?;
                    let mut reaches = false;
                    for (p, local, factor) in &reachable {
                        // Only the fixed map is constructed here. exists()
                        // reserves local variables itself on the unfactored path.
                        b.copy(context.len() + 1)?;
                        let mut fixed: Values = context
                            .iter()
                            .filter(|(id, _)| p.defined.contains(id))
                            .map(|(id, v)| (*id, *v))
                            .collect();
                        fixed.insert(*j, *value);
                        if exists(p, local, factor.as_ref(), &fixed, calls, m, b)? {
                            reaches = true;
                            break;
                        }
                    }
                    may |= reaches;
                    cannot |= !reaches;
                    if !reaches {
                        for (_, p) in &r.sinks {
                            if !p.trace.as_ref().is_some_and(|t| t.blocked)
                                || !p.defined.contains(j)
                                || uncertain(p)
                            {
                                continue;
                            }
                            let local = path_support(p, m, b)?;
                            b.copy(context.len() + 1)?;
                            let mut fixed: Values = context
                                .iter()
                                .filter(|(id, _)| p.defined.contains(id))
                                .map(|(id, v)| (*id, *v))
                                .collect();
                            fixed.insert(*j, *value);
                            let factor = Factorization::build_viewpoint(p, &local, calls, m, b)?;
                            if exists(p, &local, factor.as_ref(), &fixed, calls, m, b)? {
                                has_blocked = true;
                                break;
                            }
                        }
                    }
                }
                if may {
                    if cannot {
                        selecting = true;
                    } else {
                        bypass = true;
                    }
                }
            }
            b.copy(3)?;
            let call = calls[j.0];
            let name = m.process.bindings[j.0].name.clone();
            let capability = cap_name(m, call.capability)?;
            if selecting {
                b.insert(&mut actors, m.capabilities[call.capability.0].actor)?;
                selectors.push(Selector {
                    call: name,
                    capability,
                    selects: if bypass {
                        Selects::SomeContexts
                    } else {
                        Selects::AllContexts
                    },
                    blocked: has_blocked.then_some(true),
                });
            } else {
                read_only.push(ReadOnly {
                    call: name,
                    capability,
                });
            }
        }
        ends.push(End::Derived {
            end: end.clone(),
            reachable: !reachable.is_empty(),
            selectors,
            read_only,
        });
    }
    let mut actor_names = Vec::new();
    for id in actors {
        push_name(&mut actor_names, &m.actors[id.0].name, b)?;
    }
    Ok(Framing::Derived {
        ends,
        actors: actor_names,
        complete,
        unchecked: framing_unchecked(),
    })
}

fn exists(
    p: &Path,
    local: &BTreeSet<ir::BindingId>,
    factor: Option<&Factorization>,
    fixed: &Values,
    calls: &[&ir::Call],
    m: &ir::Model,
    b: &mut Budget,
) -> Result<bool> {
    if let Some(factor) = factor {
        factor.exists(fixed, b)
    } else {
        b.copy(local.len())?;
        assignments(
            &local.iter().copied().collect::<Vec<_>>(),
            fixed,
            calls,
            m,
            b,
            |v, b| satisfied(p, v, b),
        )
    }
}
