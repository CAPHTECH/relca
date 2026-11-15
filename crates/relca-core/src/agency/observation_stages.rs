//! Structural call-site access. This output never feeds lineage or independence.
use super::*;

pub(crate) struct StageGraph<'a> {
    pub subjects: &'a [BTreeSet<ir::ActorId>],
    pub sources: Option<&'a [BTreeSet<ir::ActorId>]>,
    pub merge_dependent: &'a [bool],
}

// Visit executable sites in process order, not allocation order. In particular,
// loop result aliases and adopted response values are not extra operations.
fn calls<'a>(
    body: &'a ir::Block,
    out: &mut Vec<&'a ir::Call>,
    b: &mut Budget,
    d: usize,
) -> Result<()> {
    depth(b, d)?;
    for s in body {
        b.work()?;
        match &s.kind {
            ir::Stmt::Call(c) | ir::Stmt::Query(c, _) | ir::Stmt::Response(c, _) => {
                b.copy(1)?;
                out.push(c);
                if let ir::Stmt::Response(_, r) = &s.kind
                    && let Some(body) = &r.recovery
                {
                    calls(body, out, b, d + 1)?;
                }
            }
            ir::Stmt::Fork { branches, .. } => {
                b.copy(branches.len())?;
                out.extend(branches);
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                calls(then_body, out, b, d + 1)?;
                calls(else_body, out, b, d + 1)?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn derive(
    m: &ir::Model,
    graph: Option<StageGraph<'_>>,
    b: &mut Budget,
) -> Result<Vec<ObservationStage>> {
    // Private strategy/limit probes have no caller-owned graph. Rebuild only in
    // those tests; production always borrows the already computed subjects.
    #[cfg(test)]
    let owned = graph.is_none().then(|| test_graph(m)).transpose()?;
    #[cfg(test)]
    let graph = graph.or_else(|| {
        owned
            .as_ref()
            .map(|(subjects, merge_dependent)| StageGraph {
                subjects,
                sources: None,
                merge_dependent,
            })
    });
    let graph = graph.ok_or_else(invalid)?;
    let mut sites = Vec::new();
    calls(&m.process.body, &mut sites, b, 0)?;
    let records = crate::registration::present(m).then(|| crate::evidence_basis::graph(m));
    let variable_parents = sites
        .iter()
        .any(|c| !m.capabilities[c.capability.0].read_records.is_empty())
        .then(|| read_graph::variable_parents_by_binding(m, &sites, b.limits))
        .transpose()?;
    let mut stages: Vec<ObservationStage> = Vec::new();
    let mut indices = BTreeMap::new();
    let mut fallback = BTreeSet::new();
    for route in &m.process.degradations {
        for c in &route.degraded_calls {
            b.insert(&mut fallback, c.binding)?;
        }
    }
    for c in sites {
        if fallback.contains(&c.binding) {
            continue;
        }
        let id = m
            .response_attempt(c.binding)
            .map_or(c.binding, |r| r.command);
        let id = m.loop_alias(id).unwrap_or(id);
        let cap = m.capabilities.get(c.capability.0).ok_or_else(invalid)?;
        let actor = m.actors.get(cap.actor.0).ok_or_else(invalid)?;
        let direct = cap
            .reads
            .iter()
            .map(|id| {
                b.work()?;
                Ok(m.actors
                    .get(id.0)
                    .filter(|a| a.source.is_some())
                    .ok_or_else(invalid)?
                    .name
                    .clone())
            })
            .collect::<Result<BTreeSet<_>>>()?;
        let mut via = BTreeSet::new();
        let merged = *graph.merge_dependent.get(c.binding.0).ok_or_else(invalid)?;
        if !merged {
            let outputs;
            let subjects = if let Some(parents) = &variable_parents
                && !cap.read_records.is_empty()
            {
                let sources = graph.sources.ok_or_else(invalid)?;
                let mut ids = BTreeSet::new();
                for p in c
                    .args
                    .iter()
                    .chain(parents.get(&c.binding).ok_or_else(invalid)?)
                {
                    if *p != ir::CURRENT_CASE {
                        ids.extend(sources.get(p.0).ok_or_else(invalid)?.iter().copied());
                    }
                }
                outputs = ids;
                &outputs
            } else {
                graph.subjects.get(c.binding.0).ok_or_else(invalid)?
            };
            for source in subjects {
                b.work()?;
                let name = &m.actors.get(source.0).ok_or_else(invalid)?.name;
                if !direct.contains(name) {
                    b.insert(&mut via, name.clone())?;
                }
            }
        }
        if let Some(&i) = indices.get(&id) {
            let row: &mut ObservationStage = &mut stages[i];
            if merged {
                row.via_outputs = None;
                row.via_outputs_status = "not_derived".into();
                row.via_outputs_reason = Some("merge_control".into());
            }
            if let Some(outputs) = &mut row.via_outputs {
                b.copy(via.len())?;
                outputs.extend(via);
                outputs.sort();
                outputs.dedup();
            }
            continue;
        }
        let mut people = Vec::new();
        for p in actor.executors.iter().flatten() {
            push_name(&mut people, &m.people.get(p.0).ok_or_else(invalid)?.name, b)?;
        }
        people.sort();
        people.dedup();
        b.copy(direct.len() + 1)?;
        b.copy(1)?;
        indices.insert(id, stages.len());
        stages.push(ObservationStage {
            call: m.site_name(id),
            actor: actor.name.clone(),
            capability: cap_name(m, c.capability)?,
            unattributed: people.is_empty(),
            people,
            direct: direct.into_iter().collect(),
            via_records: records
                .as_ref()
                .map(|g| crate::registration::routes(m, g, c.binding)),
            via_outputs: (!merged).then(|| via.into_iter().collect()),
            via_outputs_status: if merged { "not_derived" } else { "derived" }.into(),
            via_outputs_reason: merged.then(|| "merge_control".into()),
        });
    }
    Ok(stages)
}

#[cfg(test)]
fn test_graph(m: &ir::Model) -> Result<(Vec<BTreeSet<ir::ActorId>>, Vec<bool>)> {
    let mut b = budget(DEFAULT);
    let mut indexed = vec![None; m.process.bindings.len()];
    collect_shared(&m.process.body, &mut indexed, m, 0, &mut b, true)?;
    let calls: Vec<_> = indexed.iter().flatten().copied().collect();
    let extra = read_graph::read_parents(m, &calls)?;
    let n = indexed.len();
    let mut subjects = vec![BTreeSet::new(); n];
    let mut sources = subjects.clone();
    let mut merged: Vec<_> = m
        .process
        .bindings
        .iter()
        .map(|b| b.merge.is_some())
        .collect();
    let mut done: Vec<_> = indexed.iter().map(Option::is_none).collect();
    for _ in 0..n {
        let mut progress = false;
        for c in &calls {
            let i = c.binding.0;
            let parents: Vec<_> = c
                .args
                .iter()
                .chain(&extra[i])
                .filter(|p| **p != ir::CURRENT_CASE)
                .collect();
            if done[i] || !parents.iter().all(|p| done.get(p.0) == Some(&true)) {
                continue;
            }
            for p in parents {
                subjects[i].extend(sources[p.0].iter().copied());
                merged[i] |= merged[p.0];
            }
            sources[i] = subjects[i].clone();
            sources[i].extend(
                &m.capabilities
                    .get(c.capability.0)
                    .ok_or_else(invalid)?
                    .reads,
            );
            done[i] = true;
            progress = true;
        }
        if done.iter().all(|d| *d) {
            return Ok((subjects, merged));
        }
        if !progress {
            break;
        }
    }
    Err(invalid())
}
