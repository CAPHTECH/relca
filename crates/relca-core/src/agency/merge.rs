//! Candidate relevance only. These edges never propagate source evidence across
//! a merge; dependent lineage remains explicitly unavailable.
use super::*;
mod projection;
pub(crate) use projection::projection;

/// Restore only merge-independent facts from an exact, bounded static
/// substitution. Dependent J is never copied from this temporary projection.
pub(super) fn restore(
    m: &ir::Model,
    a: &mut Agency,
    view: &mut Viewpoints,
    warnings: &mut Vec<diag::Diagnostic>,
) -> Result<()> {
    use viewpoints::{Exercise, Framing, Retention, StateReason};
    let p = projection::raw(m)?;
    let legacy = p.model.agency.as_derived().ok_or_else(invalid)?;
    a.refusal = legacy
        .refusal
        .iter()
        .filter(|r| p.id(&r.call).is_some_and(|id| !p.affected(id)))
        .cloned()
        .collect();
    for r in &mut a.refusal {
        r.call = p.name(m, &r.call);
        for j in &mut r.judgements {
            if let Judgement::Process(j) = j {
                j.call = p.name(m, &j.call);
            }
        }
    }
    let legacy_view = legacy.viewpoints.as_ref().ok_or_else(invalid)?;
    if let (
        Some(effects::Effects::Derived { rows, .. }),
        Some(effects::Effects::Derived { rows: known, .. }),
    ) = (&mut view.effects, &legacy_view.effects)
    {
        for row in rows.iter_mut() {
            if let Some(k) = known.iter().find(|k| {
                p.name(m, &k.call) == row.call && p.id(&k.call).is_some_and(|id| !p.affected(id))
            }) {
                *row = k.clone();
                row.call = p.name(m, &row.call);
                if let Some(js) = &mut row.j {
                    for j in js {
                        j.call = p.name(m, &j.call);
                    }
                }
                if let Some(us) = &mut row.u {
                    for u in us {
                        if let effects::Refuser::Process(j) = u {
                            j.call = p.name(m, &j.call);
                        }
                    }
                }
                if let effects::Roots::Derived(roots) = &mut row.concentration.generation_roots {
                    for root in roots {
                        root.call = p.name(m, &root.call);
                    }
                }
            }
        }
    }
    if let Framing::Derived {
        ends,
        complete,
        unchecked,
        ..
    } = &legacy_view.framing
        && p.ends.len() < m.ends.len()
    {
        let mut ends = ends.clone();
        for end in &mut ends {
            let name = match end {
                viewpoints::End::Derived { end, .. } | viewpoints::End::NotDerived { end, .. } => {
                    end.clone()
                }
            };
            if m.ends
                .iter()
                .position(|e| *e == name)
                .is_some_and(|i| p.ends.contains(&ir::EndId(i)))
            {
                *end = viewpoints::End::NotDerived {
                    end: name,
                    reason: StateReason::MergeControl,
                };
            } else if let viewpoints::End::Derived {
                selectors,
                read_only,
                ..
            } = end
            {
                for s in selectors {
                    s.call = p.name(m, &s.call);
                }
                for r in read_only {
                    r.call = p.name(m, &r.call);
                }
            }
        }
        let selecting: BTreeSet<_> = ends
            .iter()
            .filter_map(|e| {
                if let viewpoints::End::Derived { selectors, .. } = e {
                    Some(selectors)
                } else {
                    None
                }
            })
            .flatten()
            .filter_map(|s| s.capability.split_once('.').map(|(a, _)| a.to_string()))
            .collect();
        let actors = m
            .actors
            .iter()
            .filter(|a| selecting.contains(&a.name))
            .map(|a| a.name.clone())
            .collect();
        view.framing = Framing::Derived {
            ends,
            actors,
            complete: *complete && p.ends.is_empty(),
            unchecked: unchecked.clone(),
        };
    }
    view.retention = legacy_view.retention.clone();
    if let Retention::Derived {
        capabilities,
        unchecked,
        ..
    } = &mut view.retention
    {
        for (i, cap) in capabilities.iter_mut().enumerate() {
            if p.cap_affected(ir::CapabilityId(i)) {
                let (capability, contest_route, fallback, join_any_cancellation) = match cap {
                    Exercise::Derived {
                        capability,
                        contest_route,
                        fallback,
                        join_any_cancellation,
                        ..
                    }
                    | Exercise::NotDerived {
                        capability,
                        contest_route,
                        fallback,
                        join_any_cancellation,
                        ..
                    } => (
                        capability.clone(),
                        *contest_route,
                        fallback.clone(),
                        *join_any_cancellation,
                    ),
                };
                if m.fallback_capabilities.contains(&ir::CapabilityId(i)) {
                    a.unchecked
                        .push(format!("W-AGY-3(merge_control): {capability}"));
                }
                *cap = Exercise::NotDerived {
                    capability,
                    reason: StateReason::MergeControl,
                    contest_route,
                    basis: "unknown".into(),
                    fallback,
                    join_any_cancellation,
                };
            }
        }
        if capabilities.iter().any(|c| {
            matches!(
                c,
                Exercise::NotDerived {
                    reason: StateReason::MergeControl,
                    ..
                }
            )
        }) {
            unchecked.push("merge_control".into());
        }
    }
    let affected_effects: BTreeSet<_> = p
        .capabilities
        .iter()
        .filter(|(id, cap)| p.affected(**id) && m.capabilities[cap.0].commits)
        .map(|(id, _)| *id)
        .collect();
    for id in &affected_effects {
        for code in ["U", "W-AGY-0", "W-AGY-1", "W-AGY-2", "W-RESP-2"] {
            a.unchecked
                .push(format!("{code}(merge_control): {}", m.site_name(*id)));
        }
    }
    if p.ends.is_empty() {
        a.unchecked.retain(|s| s != "F(merge_control)");
    }
    if affected_effects.is_empty() {
        a.unchecked.retain(|s| {
            ![
                "U(merge_control)",
                "W-AGY-0(merge_control)",
                "W-AGY-2(merge_control)",
                "W-RESP-2(merge_control)",
            ]
            .contains(&s.as_str())
        });
    } else {
        a.unchecked.push("W-AGY-1(merge_control)".into());
    }
    if !p.capabilities.keys().any(|id| p.affected(*id)) {
        a.unchecked.retain(|s| {
            ![
                "W-LIN-2(merge_control)",
                "common_cause(merge_control)",
                "model_uncertainties(merge_control)",
            ]
            .contains(&s.as_str())
        });
    }
    for r in &legacy.refusal {
        for j in r.judgements.iter().filter_map(Judgement::process) {
            if let Some(id) = p.id(&j.call).filter(|id| p.affected(*id)) {
                let mark = format!(
                    "W-RESP-0(merge_control): {} {}",
                    m.actors[m.capabilities[p.capabilities[&id].0].actor.0].name,
                    m.site_name(id)
                );
                if !a.unchecked.contains(&mark) {
                    a.unchecked.push(mark);
                }
            }
        }
    }
    for r in &a.refusal {
        for j in r
            .judgements
            .iter()
            .filter_map(Judgement::process)
            .filter(|j| j.stops != Stops::AllPaths)
        {
            if let Some((_, c)) = p
                .capabilities
                .iter()
                .find(|(id, _)| m.site_name(**id) == j.call)
            {
                let span = p
                    .capabilities
                    .iter()
                    .find(|(id, _)| m.site_name(**id) == j.call)
                    .and_then(|(id, _)| p.spans.get(id))
                    .copied()
                    .unwrap_or(m.capabilities[c.0].span);
                let warning = diag::Diagnostic {
                    code: if j.stops == Stops::None {
                        diag::W_AGY_0
                    } else {
                        diag::W_AGY_1
                    }
                    .into(),
                    severity: diag::Severity::Warning,
                    message: format!(
                        "{} judgement {} {} (bypasses: {})",
                        r.effect,
                        j.call,
                        if j.stops == Stops::None {
                            "does not stop the effect"
                        } else {
                            "has a bypass"
                        },
                        j.bypasses
                    ),
                    span: Some(span),
                };
                warnings.push(warning);
            }
        }
    }
    for w in &p.warnings {
        let keep = if w.code == diag::W_AGY_2 {
            p.capabilities.iter().any(|(id, cap)| {
                !p.affected(*id)
                    && m.capabilities[cap.0].commits
                    && w.span == p.spans.get(id).copied()
            })
        } else {
            w.code == diag::W_LIN_2 && !p.mentions_affected(&w.message)
        };
        if keep {
            let mut w = w.clone();
            w.message = p.message(m, &w.message);
            if !warnings.contains(&w) {
                warnings.push(w);
            }
        }
    }
    if let Some(CommonCauseDerivation::Derived(causes)) = &legacy.common_causes {
        let mut causes = causes.clone();
        for cause in &mut causes.causes {
            cause
                .targets
                .retain(|t| p.id(&t.call).is_some_and(|id| !p.affected(id)));
            for t in &mut cause.targets {
                t.call = p.name(m, &t.call);
            }
        }
        causes.causes.retain(|c| !c.targets.is_empty());
        a.common_causes = Some(CommonCauseDerivation::Derived(causes));
    }
    a.source_descriptions = legacy.source_descriptions.clone();
    if let Some(items) = &legacy.model_uncertainties {
        let mut items = items.clone();
        items.items.retain(|i| {
            p.id(&i.call).is_some_and(|id| !p.affected(id))
                && i.targets
                    .iter()
                    .all(|t| p.id(t).is_none_or(|id| !p.affected(id)))
        });
        for item in &mut items.items {
            item.call = p.name(m, &item.call);
            for t in &mut item.targets {
                *t = p.name(m, t);
            }
            if let common_cause::Suppression::Correlation { judgements } = &mut item.suppression {
                for j in judgements {
                    *j = p.name(m, j);
                }
            }
        }
        a.model_uncertainties = Some(items);
    }
    Ok(())
}

/// O is possible access aggregated by Actor, as in the duplicated-branch
/// model. Its union is not a judgement's evidence lineage and must never feed
/// J, independence, or warning suppression.
pub(super) fn indirect_origins(
    m: &ir::Model,
    calls: &[&ir::Call],
    extra: &[Vec<ir::BindingId>],
    b: &mut Budget,
) -> Result<Vec<BTreeSet<ir::ActorId>>> {
    let n = m.process.bindings.len();
    b.cells(n.checked_mul(5).ok_or_else(invalid)?)?;
    b.copy(m.actors.len())?;
    let mut parents = vec![Vec::new(); n];
    let mut sources = vec![BTreeSet::new(); n];
    for (i, binding) in m.process.bindings.iter().enumerate() {
        if let Some(merge) = &binding.merge {
            b.copy(merge.sources.len())?;
            parents[i] = merge.sources.clone();
        }
    }
    for c in calls {
        for id in c
            .args
            .iter()
            .chain(&extra[c.binding.0])
            .filter(|id| **id != ir::CURRENT_CASE)
        {
            b.copy(1)?;
            parents[c.binding.0].push(*id);
        }
        for source in &m.capabilities[c.capability.0].reads {
            b.insert(&mut sources[c.binding.0], *source)?;
        }
    }
    let mut degree: Vec<_> = parents.iter().map(Vec::len).collect();
    let mut children = vec![Vec::new(); n];
    for (i, ps) in parents.iter().enumerate() {
        for p in ps {
            b.copy(1)?;
            children.get_mut(p.0).ok_or_else(invalid)?.push(i);
        }
    }
    let mut ready: VecDeque<_> = degree
        .iter()
        .enumerate()
        .filter_map(|(i, d)| (*d == 0).then_some(i))
        .collect();
    let mut visited = 0;
    while let Some(i) = ready.pop_front() {
        visited += 1;
        b.work()?;
        for child in &children[i] {
            b.copy(sources[i].len())?;
            let inherited = sources[i].clone();
            for source in inherited {
                b.insert(&mut sources[*child], source)?;
            }
            degree[*child] -= 1;
            if degree[*child] == 0 {
                ready.push_back(*child);
            }
        }
    }
    if visited != n {
        return Err(invalid());
    }
    let mut indirect = vec![BTreeSet::new(); m.actors.len()];
    for c in calls {
        for p in &parents[c.binding.0] {
            for source in &sources[p.0] {
                b.insert(
                    &mut indirect[m.capabilities[c.capability.0].actor.0],
                    *source,
                )?;
            }
        }
    }
    Ok(indirect)
}

pub(super) fn candidates(
    m: &ir::Model,
    calls: &[&ir::Call],
    extra: &[Vec<ir::BindingId>],
    b: &mut Budget,
) -> Result<BTreeSet<ir::BindingId>> {
    b.copy(m.process.bindings.len())?;
    let mut parents = vec![BTreeSet::new(); m.process.bindings.len()];
    for c in calls {
        for id in c
            .args
            .iter()
            .chain(&extra[c.binding.0])
            .filter(|id| **id != ir::CURRENT_CASE)
        {
            b.insert(&mut parents[c.binding.0], *id)?;
        }
    }
    fn walk(
        block: &ir::Block,
        guards: &BTreeSet<ir::BindingId>,
        parents: &mut [BTreeSet<ir::BindingId>],
        m: &ir::Model,
        b: &mut Budget,
        depth: usize,
    ) -> Result<()> {
        b.depth(depth)?;
        for s in block {
            b.work()?;
            match &s.kind {
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    b.copy(guards.len())?;
                    let mut inner = guards.clone();
                    condition_support(condition, &mut inner, m, 0, b)?;
                    walk(then_body, &inner, parents, m, b, depth + 1)?;
                    walk(else_body, &inner, parents, m, b, depth + 1)?;
                }
                ir::Stmt::MergeCopy { binding, source } => {
                    b.insert(&mut parents[binding.0], *source)?;
                    for id in guards {
                        b.insert(&mut parents[binding.0], *id)?;
                    }
                }
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) | ir::Stmt::Response(c, _) => {
                    for id in guards {
                        b.insert(&mut parents[c.binding.0], *id)?;
                    }
                }
                ir::Stmt::Fork { branches, .. } => {
                    for c in branches {
                        for id in guards {
                            b.insert(&mut parents[c.binding.0], *id)?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    walk(&m.process.body, &BTreeSet::new(), &mut parents, m, b, 0)?;
    let mut pending: Vec<_> = calls
        .iter()
        .filter(|c| m.capabilities[c.capability.0].commits)
        .map(|c| c.binding)
        .collect();
    let mut relevant = BTreeSet::new();
    while let Some(id) = pending.pop() {
        b.work()?;
        for parent in &parents[id.0] {
            if !relevant.contains(parent) {
                b.insert(&mut relevant, *parent)?;
                b.copy(1)?;
                pending.push(*parent);
            }
        }
    }
    Ok(calls
        .iter()
        .filter(|c| {
            relevant.contains(&c.binding)
                && (c.args.iter().any(|id| *id != ir::CURRENT_CASE)
                    || !extra[c.binding.0].is_empty())
        })
        .map(|c| c.binding)
        .collect())
}
