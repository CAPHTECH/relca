//! Existential sink envelopes. State-dependent result choices are local to a
//! sink; only ordinary result bindings are shared by the taint overlap test.
use super::*;

pub(super) fn exercised<'a>(
    m: &'a ir::Model,
    calls: &[&'a ir::Call],
    r: Recorder<'a>,
    b: &mut Budget,
) -> Result<Retention> {
    let routes = routes(m, b)?;
    let capacity = ActorCapacity::new(m, calls, b)?;
    b.location.phase = "retention exercise".into();
    let n = m.capabilities.len();
    b.copy(n.checked_mul(5).ok_or_else(invalid)?)?;
    let mut mins = vec![usize::MAX; n];
    let mut maxs = vec![0; n];
    let mut taint = vec![false; n];
    let mut blocked = vec![false; n];
    let mut actor_blocked = vec![false; n];
    let infeasible = if precise_resources(m) {
        BTreeSet::new()
    } else {
        infeasible_capabilities(m)?
    };
    let mut cancellation = BTreeSet::new();
    let mut ps = Vec::new();
    let mut unknown_results = BTreeSet::new();
    let mut has_unknown = false;
    for (_, mut p) in r.sinks {
        // Preserve the legacy conservative resource withdrawal, including
        // lexically recorded suffixes of an infeasible path.
        if let Some(k) = first_infeasible(&p, calls, &infeasible, b)? {
            for id in p.defined.range(k..) {
                b.work()?;
                blocked[calls.get(id.0).ok_or_else(invalid)?.capability.0] = true;
            }
        }
        has_unknown |= uncertain(&p);
        b.copy(p.guards.len())?;
        p.guards.retain(|g| !unknown_guard(g));
        let local = path_support(&p, m, b)?;
        for id in &p.trace.as_ref().ok_or_else(invalid)?.unknown_results {
            b.insert(&mut unknown_results, *id)?;
        }
        b.copy(1)?;
        ps.push((p, local));
    }
    let mut sat = Vec::new();
    for (p, local) in &ps {
        if !exists(p, local, &Values::new(), calls, m, b)? {
            continue;
        }
        if let Some(stage) = capacity.first_stage(p, b)? {
            for id in &p.defined {
                b.work()?;
                if capacity.zero.contains(id) || capacity.stages[id.0] > stage {
                    actor_blocked[calls[id.0].capability.0] = true;
                }
            }
        }
        b.copy(n)?;
        let mut counts = vec![0usize; n];
        let trace = p.trace.as_ref().ok_or_else(invalid)?;
        for id in &trace.unstarted_zero {
            b.work()?;
            actor_blocked[calls[id.0].capability.0] = true;
        }
        for id in &trace.started {
            b.work()?;
            let i = calls.get(id.0).ok_or_else(invalid)?.capability.0;
            counts[i] = counts[i].checked_add(1).ok_or_else(invalid)?;
        }
        for id in &trace.cancellation {
            b.insert(&mut cancellation, calls[id.0].capability)?;
        }
        for i in 0..n {
            b.work()?;
            mins[i] = mins[i].min(counts[i]);
            maxs[i] = maxs[i].max(counts[i]);
        }
        b.copy(1)?;
        sat.push((p, local, counts));
    }
    if sat.is_empty() {
        return Err(invalid());
    }
    if has_unknown {
        b.location.phase = "retention state overlap".into();
        for (i, (p, local, counts)) in sat.iter().enumerate() {
            for (q, other, other_counts) in &sat[..i] {
                b.steps(n)?;
                if !(0..n).any(|k| !taint[k] && counts[k] != other_counts[k]) {
                    continue;
                }
                b.copy(local.len())?;
                let common: Vec<_> = local
                    .intersection(other)
                    .filter(|id| !unknown_results.contains(id))
                    .copied()
                    .collect();
                if assignments(&common, &Values::new(), calls, m, b, |fixed, b| {
                    Ok(exists(p, local, fixed, calls, m, b)?
                        && exists(q, other, fixed, calls, m, b)?)
                })? {
                    for k in 0..n {
                        b.work()?;
                        taint[k] |= counts[k] != other_counts[k];
                    }
                }
            }
        }
    }
    let mut capabilities = Vec::new();
    for i in 0..n {
        b.copy(5)?;
        let id = ir::CapabilityId(i);
        let capability = cap_name(m, id)?;
        let contest_route = routes.contains(&id);
        let join_any_cancellation = cancellation.contains(&id);
        let fallback = if m.fallback_capabilities.contains(&id) {
            Fallback::KeptByIntent
        } else {
            Fallback::Unchecked
        };
        capabilities.push(if blocked[i] || actor_blocked[i] || taint[i] {
            Exercise::NotDerived {
                capability,
                reason: if blocked[i] {
                    StateReason::ResourceInfeasible
                } else if actor_blocked[i] {
                    StateReason::ActorCapacityInfeasible
                } else {
                    StateReason::StateGuards
                },
                contest_route,
                basis: "unknown".into(),
                fallback,
                join_any_cancellation,
            }
        } else {
            Exercise::Derived {
                capability,
                normal: Count {
                    min: mins[i],
                    max: maxs[i],
                },
                contest_route,
                basis: "unknown".into(),
                fallback,
                join_any_cancellation,
            }
        });
    }
    Ok(Retention::Derived {
        capabilities,
        notice: NOTICE.into(),
        unchecked: retention_unchecked(),
    })
}

fn exists(
    p: &Path,
    local: &BTreeSet<ir::BindingId>,
    fixed: &Values,
    calls: &[&ir::Call],
    m: &ir::Model,
    b: &mut Budget,
) -> Result<bool> {
    if let Some(factor) = Factorization::build_viewpoint(p, local, calls, m, b)? {
        return factor.exists(fixed, b);
    }
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
