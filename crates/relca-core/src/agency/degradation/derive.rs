use super::*;
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, &'static str>;
#[derive(Clone)]
struct Path {
    values: BTreeMap<ir::BindingId, ir::ValueId>,
    starts: Vec<u64>,
    end: Option<ir::EndId>,
}
struct Budget(usize);
impl Budget {
    fn spend(&mut self) -> Result<()> {
        self.0 = self.0.checked_sub(1).ok_or("derivation_limit")?;
        Ok(())
    }
    fn copy(&mut self, n: usize) -> Result<()> {
        self.0 = self.0.checked_sub(n).ok_or("derivation_limit")?;
        Ok(())
    }
}

fn truth(c: &ir::Cond, p: &Path, b: &mut Budget) -> Result<bool> {
    b.spend()?;
    match c {
        ir::Cond::Eq(x) => Ok(*p.values.get(&x.binding).ok_or("unknown_binding")? == x.value),
        ir::Cond::And(xs) => {
            let mut yes = true;
            for x in xs {
                yes &= truth(x, p, b)?;
            }
            Ok(yes)
        }
        ir::Cond::Or(xs) => {
            let mut yes = false;
            for x in xs {
                yes |= truth(x, p, b)?;
            }
            Ok(yes)
        }
        _ => Err("shared_state_guard"),
    }
}
fn walk(
    block: &[ir::Statement],
    mut paths: Vec<Path>,
    m: &ir::Model,
    budget: &mut Budget,
) -> Result<Vec<Path>> {
    for s in block {
        budget.spend()?;
        let mut next = Vec::new();
        for mut p in paths {
            budget.spend()?;
            if p.end.is_some() {
                next.push(p);
                continue;
            }
            match &s.kind {
                ir::Stmt::Call(c) => call(c, p, m, budget, &mut next)?,
                ir::Stmt::Response(_, r) => call(&r.command_call, p, m, budget, &mut next)?,
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    let branch = if truth(condition, &p, budget)? {
                        then_body
                    } else {
                        else_body
                    };
                    next.extend(walk(branch, vec![p], m, budget)?);
                }
                ir::Stmt::End(e) => {
                    p.end = Some(*e);
                    next.push(p);
                }
                _ => return Err("unsupported_control"),
            }
        }
        paths = next;
    }
    Ok(paths)
}
fn call(
    c: &ir::Call,
    mut p: Path,
    m: &ir::Model,
    b: &mut Budget,
    out: &mut Vec<Path>,
) -> Result<()> {
    let cap = &m.capabilities[c.capability.0];
    if cap.requires.is_some() {
        return Err("requires");
    }
    p.starts[c.capability.0] = p.starts[c.capability.0]
        .checked_add(1)
        .ok_or("derivation_limit")?;
    // Kernel declarations contribute every branch, including zero mass and
    // every given row. Without a kernel, preserve the declared result set.
    let domain: BTreeSet<_> = if cap.kernel.is_some() || cap.performance.is_some() {
        cap.kernel
            .iter()
            .chain(cap.performance.iter().flat_map(|p| &p.kernels))
            .flat_map(|kernel| &kernel.rows)
            .flat_map(|row| &row.branches)
            .map(|branch| branch.result)
            .collect()
    } else if cap.results.is_empty() {
        (0..m.types[cap.return_type.0].members.len())
            .map(ir::ValueId)
            .collect()
    } else {
        cap.possible_results().into_iter().collect()
    };
    for v in domain {
        b.spend()?;
        b.copy(p.starts.len().saturating_add(p.values.len()))?;
        let mut branch = p.clone();
        branch.values.insert(c.binding, v);
        out.push(branch);
    }
    Ok(())
}
fn counts(m: &ir::Model, paths: &[&Path]) -> Vec<Count> {
    (0..m.capabilities.len())
        .map(|i| Count {
            capability: cap_name(m, i),
            min: paths.iter().map(|p| p.starts[i]).min().unwrap_or(0),
            max: paths.iter().map(|p| p.starts[i]).max().unwrap_or(0),
        })
        .collect()
}
fn ends(block: &[ir::Statement], out: &mut BTreeSet<ir::EndId>) {
    for s in block {
        match &s.kind {
            ir::Stmt::End(e) => {
                out.insert(*e);
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                ends(then_body, out);
                ends(else_body, out);
            }
            _ => (),
        }
    }
}
fn coverage(block: &[ir::Statement], m: &ir::Model) -> Option<u64> {
    block.iter().try_fold(0u64, |sum, s| {
        let upper = match &s.kind {
            ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => {
                let cap = &m.capabilities[c.capability.0];
                cap.duration_distribution
                    .as_ref()
                    .map_or(cap.duration, |d| {
                        d.entries.iter().map(|(t, _)| *t).max().unwrap_or(0)
                    })
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => coverage(then_body, m)?.checked_add(coverage(else_body, m)?)?,
            _ => 0,
        };
        sum.checked_add(upper)
    })
}
/// A declaration envelope, not a sampled/positive-mass arrival projection.
fn arrival_upper(m: &ir::Model) -> Option<u64> {
    let mut upper = 0;
    if let Some(times) = &m.arrivals {
        upper = *times.iter().max()?;
    }
    if let Some(every) = &m.arrival_every {
        if every.every == 0 {
            return None;
        }
        let last = every
            .from
            .checked_add(every.every.checked_mul(every.count.checked_sub(1)?)?)?;
        if last > every.until {
            return None;
        }
        upper = upper.max(last);
    }
    if let Some(distribution) = &m.arrival_distribution {
        if distribution.entries.is_empty() {
            return None;
        }
        for (times, _) in &distribution.entries {
            upper = upper.max(*times.iter().max()?);
        }
    }
    Some(upper)
}
fn reason(block: &ir::Block, m: &ir::Model, latest: Option<u64>) -> Option<&'static str> {
    // A coarse declaration-only upper bound, independent of clock/horizon and
    // kernel probabilities. If it does not fit a constant calendar's coverage,
    // do not turn the lexically present later call into a guaranteed Start.
    fn state(c: &ir::Cond) -> bool {
        match c {
            ir::Cond::Eq(_) => false,
            ir::Cond::And(xs) | ir::Cond::Or(xs) => xs.iter().any(state),
            _ => true,
        }
    }
    for s in block {
        match &s.kind {
            ir::Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                if state(condition) {
                    return Some("shared_state_guard");
                }
                if let Some(r) =
                    reason(then_body, m, latest).or_else(|| reason(else_body, m, latest))
                {
                    return Some(r);
                }
            }
            ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => {
                let cap = &m.capabilities[c.capability.0];
                let actor = &m.actors[cap.actor.0];
                if cap.qualified_executors.as_ref().is_some_and(Vec::is_empty) {
                    return Some("qualification_unavailable");
                }
                if cap.history_dependent_result() {
                    return Some("shared_state_guard");
                }
                if cap.requires.is_some() {
                    return Some("requires");
                }
                if actor.capacity == Some(0)
                    || actor.executors.as_ref().is_some_and(Vec::is_empty)
                    || cap
                        .uses
                        .iter()
                        .any(|u| u.amount > m.resources[u.resource.0].capacity)
                {
                    return Some("resource_impossible");
                }
                if let Some(time) = &m.resource_time {
                    let restricted = |id: Option<usize>, capacity| {
                        id.is_some_and(|i| {
                            latest.is_none_or(|bound| bound >= time.calendars[i].through)
                                || time.calendars[i]
                                    .intervals
                                    .iter()
                                    .any(|v| v.capacity < capacity)
                        })
                    };
                    if restricted(time.actors[cap.actor.0], 1)
                        || cap
                            .qualified_executors
                            .as_deref()
                            .or(actor.executors.as_deref())
                            .into_iter()
                            .flatten()
                            .any(|p| restricted(time.people[p.0], 1))
                        || cap
                            .uses
                            .iter()
                            .any(|u| restricted(time.resources[u.resource.0], u.amount))
                    {
                        return Some("calendar");
                    }
                }
            }
            _ => (),
        }
    }
    None
}
fn side(
    branch: &ir::Block,
    suffix: &ir::Block,
    prefixes: &[Path],
    selected: bool,
    route: &ir::Degradation,
    m: &ir::Model,
    b: &mut Budget,
) -> Result<Side> {
    if m.uses_priority() {
        return Ok(absent("priority"));
    }
    let mut block = branch.clone();
    block.extend(suffix.clone());
    let at = m
        .process
        .body
        .iter()
        .position(|s| s.span == route.span)
        .ok_or("invalid_q6_ir")?;
    let latest = arrival_upper(m)
        .and_then(|offset| offset.checked_add(coverage(&m.process.body[..at], m)?))
        .and_then(|t| t.checked_add(coverage(&block, m)?));
    if let Some(r) = reason(&block, m, latest) {
        return Ok(absent(r));
    }
    let mut initial = Vec::new();
    for p in prefixes {
        if p.end.is_none() && truth(&ir::Cond::Eq(route.condition), p, b)? == selected {
            let mut p = p.clone();
            p.starts.fill(0);
            initial.push(p);
        }
    }
    let paths = walk(&block, initial, m, b)?;
    let mut targets = BTreeSet::new();
    ends(&block, &mut targets);
    let mut targets: Vec<_> = targets.into_iter().collect();
    targets.sort_by_key(|e| &m.ends[e.0]);
    let rows = targets
        .into_iter()
        .map(|e| {
            let ps: Vec<_> = paths.iter().filter(|p| p.end == Some(e)).collect();
            let counts = counts(m, &ps);
            let mut may: Vec<_> = counts
                .iter()
                .filter(|c| c.max > 0)
                .map(|c| c.capability.clone())
                .collect();
            may.sort();
            let mut must: Vec<_> = counts
                .iter()
                .filter(|c| c.min > 0)
                .map(|c| c.capability.clone())
                .collect();
            must.sort();
            End {
                end: m.ends[e.0].clone(),
                reachable: !ps.is_empty(),
                counts,
                may_require: may,
                must_require: must,
            }
        })
        .collect();
    let used: BTreeSet<_> = paths
        .iter()
        .flat_map(|p| {
            p.starts
                .iter()
                .enumerate()
                .filter(|(_, n)| **n > 0)
                .map(|(i, _)| i)
        })
        .collect();
    let mut observation = Vec::new();
    for i in used {
        b.spend()?;
        let cap = &m.capabilities[i];
        let actor = &m.actors[cap.actor.0];
        let people =
            |ids: &Vec<ir::PersonId>| ids.iter().map(|p| m.people[p.0].name.clone()).collect();
        for source in &cap.reads {
            if m.actors[source.0].source.is_some() {
                observation.push(Observation {
                    actor: actor.name.clone(),
                    capability: cap_name(m, i),
                    source: m.actors[source.0].name.clone(),
                    executors: actor.executors.as_ref().map(people),
                    qualified: cap.qualified_executors.as_ref().map(people),
                    permitted: cap
                        .permission
                        .as_ref()
                        .or(actor.executors.as_ref())
                        .map(people),
                });
            }
        }
    }
    Ok(Side {
        status: "derived".into(),
        reason: None,
        ends: Some(rows),
        observation: Some(observation),
    })
}
pub(crate) fn derive(m: &ir::Model, limit: usize) -> Degradation {
    let result = (|| {
        let mut b = Budget(limit);
        b.spend()?;
        // Prefix and sides have the same interference limitation. The wire
        // cannot represent an unknown prefix inside a derived route.
        if m.arrivals
            .as_ref()
            .map_or(m.bounds.cases > 1, |xs| xs.len() > 1)
            || m.arrival_every.as_ref().is_some_and(|e| e.count > 1)
            || m.arrival_distribution
                .as_ref()
                .is_some_and(|d| d.entries.iter().any(|(xs, _)| xs.len() > 1))
        {
            return Err("multiple_case_interference");
        }
        // Arrival time is needed to certify calendar coverage, not to enumerate
        // timeless structural guard paths when no calendar exists.
        let offset = if m.resource_time.is_some() {
            arrival_upper(m).ok_or("arrival_coverage")?
        } else {
            0
        };
        let mut routes = Vec::new();
        for r in &m.process.degradations {
            let at = m
                .process
                .body
                .iter()
                .position(|s| s.span == r.span)
                .ok_or("invalid_q6_ir")?;
            let ir::Stmt::If {
                then_body,
                else_body,
                ..
            } = &m.process.body[at].kind
            else {
                return Err("invalid_q6_ir");
            };
            let prefix = m.process.body[..at].to_vec();
            let suffix = m.process.body[at + 1..].to_vec();
            let latest = coverage(&prefix, m).and_then(|t| offset.checked_add(t));
            if let Some(r) = reason(&prefix, m, latest) {
                return Err(r);
            }
            let prefixes = walk(
                &prefix,
                vec![Path {
                    values: BTreeMap::new(),
                    starts: vec![0; m.capabilities.len()],
                    end: None,
                }],
                m,
                &mut b,
            )?;
            let common_prefix = counts(
                m,
                &prefixes
                    .iter()
                    .filter(|p| p.end.is_none())
                    .collect::<Vec<_>>(),
            );
            let degraded = side(then_body, &suffix, &prefixes, true, r, m, &mut b)?;
            let normal = side(else_body, &suffix, &prefixes, false, r, m, &mut b)?;
            let binding = m
                .process
                .bindings
                .get(r.condition.binding.0)
                .ok_or("invalid_q6_ir")?;
            let ty = m.types.get(binding.ty.0).ok_or("invalid_q6_ir")?;
            let value = ty.members.get(r.condition.value.0).ok_or("invalid_q6_ir")?;
            if r.producer.0 >= m.capabilities.len() {
                return Err("invalid_q6_ir");
            }
            routes.push(Route {
                name: r.name.clone(),
                condition: Condition {
                    binding: binding.name.clone(),
                    value: value.clone(),
                },
                producer: cap_name(m, r.producer.0),
                common_prefix,
                normal,
                degraded,
            });
        }
        Ok(routes)
    })();
    match result {
        Ok(routes) => table(Some(routes), None),
        Err(reason) => table(None, Some(reason)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arrival_envelope_covers_fixed_every_and_all_candidates_without_weights() {
        let src = "model Envelope version \"0.2\" { type R = enum { ok } actor A { capacity 1; capability go() -> R { duration fixed 1min; result nondet { ok } } } process P { r = A.go(); end done } bounds { cases 1; live_tasks 1; clock 30min } verify reachable end.done }";
        let mut m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        assert_eq!(arrival_upper(&m), Some(0));
        m.arrivals = Some(vec![9]);
        assert_eq!(arrival_upper(&m), Some(9));
        m.arrival_every = Some(ir::FixedEveryRecord {
            every: 3,
            from: 5,
            until: 11,
            count: 3,
        });
        assert_eq!(arrival_upper(&m), Some(11));
        m.arrival_distribution = Some(ir::ArrivalDistribution {
            basis: "candidate envelope".into(),
            entries: vec![
                (vec![5], crate::probability::Rational::one()),
                (vec![17], crate::probability::Rational::zero()),
            ],
            span: m.process.span,
        });
        assert_eq!(arrival_upper(&m), Some(17));
        m.arrival_distribution
            .as_mut()
            .unwrap()
            .entries
            .push((vec![], crate::probability::Rational::zero()));
        assert_eq!(arrival_upper(&m), None);
        m.arrival_distribution = None;
        m.arrival_every.as_mut().unwrap().count = 0;
        assert_eq!(arrival_upper(&m), None);
    }
}
