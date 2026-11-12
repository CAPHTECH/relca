use super::*;

/// Resolve only the already frozen row. This never consults the live queue.
pub(super) fn kernel<'a>(m: &'a ir::Model, task: &Task) -> EngineResult<Option<&'a ir::Kernel>> {
    let cap = m
        .capabilities
        .get(task.capability.0)
        .ok_or("invalid_q6_ir")?;
    let Some(source) = &cap.performance else {
        if task.frozen_performance.is_some() {
            return Err("invalid_q6_ir".into());
        }
        return Ok(cap.kernel.as_ref());
    };
    let frozen = task.frozen_performance.as_ref().ok_or("invalid_q6_ir")?;
    if frozen.profile != source.profile || frozen.executor != task.executor {
        return Err("invalid_q6_ir".into());
    }
    let profile = m.performances.get(frozen.profile).ok_or("invalid_q6_ir")?;
    let row = profile.rows.get(frozen.row).ok_or("invalid_q6_ir")?;
    let kernel = source.kernels.get(frozen.row).ok_or("invalid_q6_ir")?;
    if row.executor != frozen.executor
        || m.kernels
            .get(row.kernel)
            .is_none_or(|k| k.name != kernel.name)
    {
        return Err("invalid_q6_ir".into());
    }
    Ok(Some(kernel))
}

pub(super) fn validate(m: &ir::Model) -> EngineResult<()> {
    if !m.uses_work_profiles() && m.performances.is_empty() {
        return Ok(());
    }
    let check = |ok| {
        if ok {
            Ok(())
        } else {
            Err(Halt::from("invalid_q6_ir"))
        }
    };
    check(
        m.performances.len() <= 256
            && m.performances.iter().map(|p| p.rows.len()).sum::<usize>() <= 4096,
    )?;
    let mut names = BTreeSet::new();
    check(
        m.kernels.iter().all(|k| !k.name.is_empty())
            && m.kernels
                .iter()
                .map(|k| &k.name)
                .collect::<BTreeSet<_>>()
                .len()
                == m.kernels.len(),
    )?;
    for p in &m.performances {
        check(
            !p.name.is_empty()
                && names.insert(&p.name)
                && !p.basis.trim().is_empty()
                && !p.rows.is_empty(),
        )?;
        let mut partitions = BTreeMap::<_, Vec<_>>::new();
        for row in &p.rows {
            check(
                row.executor.is_none_or(|id| id.0 < m.people.len()) && row.kernel < m.kernels.len(),
            )?;
            partitions.entry(row.executor).or_default().push(row);
        }
        for rows in partitions.values() {
            check(rows[0].from == 0 && rows.last().is_some_and(|r| r.until.is_none()))?;
            check(
                rows.iter().all(|r| r.until.is_none_or(|to| to > r.from))
                    && rows.windows(2).all(|rs| rs[0].until == Some(rs[1].from)),
            )?;
            check(p.queued || rows.len() == 1)?;
        }
    }
    let mut bindings = BTreeMap::new();
    for c in &m.capabilities {
        let a = m.actors.get(c.actor.0).ok_or("invalid_q6_ir")?;
        if let Some(qs) = &c.qualified_executors {
            check(a.executors.is_some() && qs.iter().collect::<BTreeSet<_>>().len() == qs.len())?;
            check(qs.iter().all(|p| {
                p.0 < m.people.len() && a.executors.as_ref().is_some_and(|ps| ps.contains(p))
            }))?;
        }
        let Some(source) = &c.performance else {
            continue;
        };
        check(
            c.kernel.is_none()
                && c.atomic.is_none()
                && c.version_check.is_none()
                && c.duration_distribution.is_none(),
        )?;
        let p = m.performances.get(source.profile).ok_or("invalid_q6_ir")?;
        check(source.kernels.len() == p.rows.len())?;
        let people = c.qualified_executors.as_ref().or(a.executors.as_ref());
        check(people.is_none_or(|ps| !ps.is_empty()))?;
        let expected: BTreeSet<_> = people.map_or_else(
            || [None].into_iter().collect(),
            |ps| ps.iter().map(|p| Some(*p)).collect(),
        );
        check(expected == p.rows.iter().map(|r| r.executor).collect())?;
        let mut signature = None;
        for (row, k) in p.rows.iter().zip(&source.kernels) {
            let template = &m.kernels[row.kernel];
            check(k.basis == template.basis && k.rows.len() == template.rows.len())?;
            for (actual, declared) in k.rows.iter().zip(&template.rows) {
                check(
                    actual.values == declared.values
                        && actual.branches.len() == declared.branches.len()
                        && actual
                            .branches
                            .iter()
                            .zip(&declared.branches)
                            .all(|(a, b)| a.probability == b.probability),
                )?;
            }
            check(k.name == m.kernels[row.kernel].name && k.given == m.kernels[row.kernel].given)?;
            validate_kernel(k, m).map_err(|_| Halt::from("invalid_q6_ir"))?;
            let targets: BTreeSet<_> = k
                .rows
                .iter()
                .flat_map(|r| &r.branches)
                .flat_map(|b| &b.overrides)
                .map(|(v, _)| *v)
                .collect();
            let current = (&k.given, targets);
            check(signature.as_ref().is_none_or(|old| *old == current))?;
            signature = Some(current);
        }
        let first = source.kernels.first().ok_or("invalid_q6_ir")?;
        let mut reads = c.kernel_reads_latent.clone();
        reads.sort();
        let targets = first
            .rows
            .iter()
            .flat_map(|r| &r.branches)
            .flat_map(|b| &b.overrides)
            .map(|(v, _)| *v)
            .collect::<BTreeSet<_>>();
        let contract = (
            c.return_type,
            c.params.iter().map(|p| p.ty).collect::<Vec<_>>(),
            first.given.clone(),
            reads,
            targets,
        );
        check(
            bindings
                .get(&source.profile)
                .is_none_or(|old| *old == contract),
        )?;
        bindings.insert(source.profile, contract);
    }
    Ok(())
}
