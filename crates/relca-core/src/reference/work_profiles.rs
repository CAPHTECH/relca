use super::*;

#[cfg(test)]
pub(super) fn observe(at: u64, m: &Model, event: &Occurrence) {
    let (cap, kind) = match event {
        Occurrence::Dispatch(c) => (*c, "start"),
        Occurrence::Completion(c, _) => (*c, "finish"),
        Occurrence::CancelAck(c) => (*c, "ack"),
        _ => return,
    };
    crate::q6_acceptance::event(at, m, cap, kind);
}

pub(super) fn kernel<'a>(m: &'a Model, task: &Task) -> Result<Option<&'a Kernel>, StopReason> {
    let operation = m
        .capabilities
        .get(task.call.capability.0)
        .ok_or(StopReason::SemanticAbort)?;
    match &operation.performance {
        None => {
            require(task.frozen_performance.is_none())?;
            Ok(operation.kernel.as_ref())
        }
        Some(source) => {
            let saved = task
                .frozen_performance
                .as_ref()
                .ok_or(StopReason::SemanticAbort)?;
            require(saved.profile == source.profile && saved.executor == task.person)?;
            let table = m
                .performances
                .get(saved.profile)
                .ok_or(StopReason::SemanticAbort)?;
            let row = table.rows.get(saved.row).ok_or(StopReason::SemanticAbort)?;
            let declaration = m.kernels.get(row.kernel).ok_or(StopReason::SemanticAbort)?;
            let kernel = source
                .kernels
                .get(saved.row)
                .ok_or(StopReason::SemanticAbort)?;
            require(row.executor == task.person && kernel.name == declaration.name)?;
            Ok(Some(kernel))
        }
    }
}

pub(super) fn select(
    m: &Model,
    capability: CapabilityId,
    person: Option<PersonId>,
    queued: u64,
) -> Result<Option<FrozenPerformance>, StopReason> {
    let operation = m
        .capabilities
        .get(capability.0)
        .ok_or(StopReason::SemanticAbort)?;
    let Some(source) = &operation.performance else {
        return Ok(None);
    };
    let profile = m
        .performances
        .get(source.profile)
        .ok_or(StopReason::SemanticAbort)?;
    let mut selected = None;
    for (index, band) in profile.rows.iter().enumerate() {
        if band.executor == person
            && (!profile.queued || band.from <= queued && band.until.is_none_or(|end| queued < end))
        {
            require(selected.is_none())?;
            selected = Some(FrozenPerformance {
                profile: source.profile,
                row: index,
                executor: person,
            });
        }
    }
    selected.map(Some).ok_or(StopReason::SemanticAbort)
}

/// Independent public-IR validation; no engine validator or selector helper.
pub(super) fn validate(m: &Model) -> Execution {
    if !m.uses_work_profiles() && m.performances.is_empty() {
        return Ok(());
    }
    require(m.performances.len() <= 256)?;
    let mut kernel_names = BTreeSet::new();
    for kernel in &m.kernels {
        require(!kernel.name.is_empty() && kernel_names.insert(&kernel.name))?;
    }
    let mut n = 0usize;
    let mut names = BTreeSet::new();
    for profile in &m.performances {
        n = n
            .checked_add(profile.rows.len())
            .ok_or(StopReason::SemanticAbort)?;
        require(
            n <= 4096
                && !profile.rows.is_empty()
                && !profile.basis.trim().is_empty()
                && !profile.name.is_empty()
                && names.insert(&profile.name),
        )?;
        let people: BTreeSet<_> = profile.rows.iter().map(|row| row.executor).collect();
        for person in people {
            require(person.is_none_or(|p| p.0 < m.people.len()))?;
            let mut end = Some(0);
            let mut bands = 0;
            for row in profile.rows.iter().filter(|r| r.executor == person) {
                require(end == Some(row.from) && row.until.is_none_or(|to| row.from < to))?;
                require(row.kernel < m.kernels.len())?;
                bands += 1;
                end = row.until;
            }
            require(end.is_none() && (profile.queued || bands == 1))?;
        }
    }
    let mut bound_contracts = BTreeMap::new();
    for operation in &m.capabilities {
        let actor = m
            .actors
            .get(operation.actor.0)
            .ok_or(StopReason::SemanticAbort)?;
        if let Some(people) = &operation.qualified_executors {
            let declared = actor.executors.as_ref().ok_or(StopReason::SemanticAbort)?;
            for (index, person) in people.iter().enumerate() {
                require(
                    person.0 < m.people.len()
                        && declared.contains(person)
                        && !people[..index].contains(person),
                )?;
            }
        }
        if let Some(use_) = &operation.performance {
            require(
                operation.kernel.is_none()
                    && operation.atomic.is_none()
                    && operation.version_check.is_none()
                    && operation.duration_distribution.is_none(),
            )?;
            let profile = m
                .performances
                .get(use_.profile)
                .ok_or(StopReason::SemanticAbort)?;
            require(use_.kernels.len() == profile.rows.len())?;
            let expected: Vec<_> = match operation
                .qualified_executors
                .as_ref()
                .or(actor.executors.as_ref())
            {
                Some(ids) => {
                    require(!ids.is_empty())?;
                    ids.iter().map(|id| Some(*id)).collect()
                }
                None => vec![None],
            };
            require(
                profile.rows.iter().all(|r| expected.contains(&r.executor))
                    && expected
                        .iter()
                        .all(|p| profile.rows.iter().any(|r| r.executor == *p)),
            )?;
            let mut first = None;
            for (index, kernel) in use_.kernels.iter().enumerate() {
                let original = &m.kernels[profile.rows[index].kernel];
                require(
                    kernel.basis == original.basis && kernel.rows.len() == original.rows.len(),
                )?;
                for (a, b) in kernel.rows.iter().zip(&original.rows) {
                    require(a.values == b.values && a.branches.len() == b.branches.len())?;
                    for (a, b) in a.branches.iter().zip(&b.branches) {
                        require(a.probability == b.probability)?;
                    }
                }
                require(kernel.name == original.name && kernel.given == original.given)?;
                reference_kernel(kernel, m)?;
                let mut assigned: Vec<_> = kernel
                    .rows
                    .iter()
                    .flat_map(|r| &r.branches)
                    .flat_map(|b| &b.overrides)
                    .map(|(variable, _)| *variable)
                    .collect();
                assigned.sort();
                assigned.dedup();
                let contract = (kernel.given.clone(), assigned);
                if let Some(first) = &first {
                    require(first == &contract)?;
                } else {
                    first = Some(contract);
                }
            }
            let mut reads = operation.kernel_reads_latent.clone();
            reads.sort();
            let first = use_.kernels.first().ok_or(StopReason::SemanticAbort)?;
            let assigned = first
                .rows
                .iter()
                .flat_map(|r| &r.branches)
                .flat_map(|b| &b.overrides)
                .map(|(id, _)| *id)
                .collect::<BTreeSet<_>>();
            let signature = (
                operation.params.iter().map(|p| p.ty).collect::<Vec<_>>(),
                operation.return_type,
                reads,
                first.given.clone(),
                assigned,
            );
            if let Some(prior) = bound_contracts.get(&use_.profile) {
                require(*prior == signature)?;
            } else {
                bound_contracts.insert(use_.profile, signature);
            }
        }
    }
    Ok(())
}
