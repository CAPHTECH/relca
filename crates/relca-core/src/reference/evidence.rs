//! Independent, trace-local reconstruction. Neither the engine's proof table
//! nor the shared static Agency union is used by this path interpreter.
use super::*;

pub(super) fn present(m: &Model) -> bool {
    m.contracts
        .iter()
        .any(|c| matches!(c.kind, ContractKind::EvidenceBasis { .. }))
}

pub(super) fn validate(m: &Model, target: CapabilityId, required: &[ActorId]) -> Execution {
    require(m.capabilities.get(target.0).is_some() && !required.is_empty())?;
    require(required.iter().collect::<BTreeSet<_>>().len() == required.len())?;
    require(
        required
            .iter()
            .all(|a| m.actors.get(a.0).is_some_and(|a| a.source.is_some())),
    )?;
    require(!m.has_loops() && m.process.degradations.is_empty() && m.recovery().is_none())?;
    for cap in &m.capabilities {
        require(
            cap.reads
                .iter()
                .all(|a| m.actors.get(a.0).is_some_and(|a| a.source.is_some())),
        )?;
        require(cap.read_variables.iter().all(|v| {
            m.state_variables
                .get(v.0)
                .is_some_and(|v| v.kind == StateKind::ObservedCase)
        }))?;
    }
    Ok(())
}

fn call(body: &Block, binding: BindingId) -> Option<&Call> {
    for s in body {
        let found = match &s.kind {
            Stmt::Call(c) if c.binding == binding => Some(c),
            Stmt::Response(c, r) => {
                if c.binding == binding {
                    Some(c)
                } else if r.command == binding {
                    Some(&r.command_call)
                } else {
                    None
                }
            }
            Stmt::Fork { branches, .. } => branches.iter().find(|c| c.binding == binding),
            Stmt::If {
                then_body,
                else_body,
                ..
            } => call(then_body, binding).or_else(|| call(else_body, binding)),
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

fn sources(
    m: &Model,
    c: &Call,
    bound: &BTreeMap<BindingId, Provenance>,
    snapshots: &[Option<(BindingId, Provenance, bool)>],
    constraints: &BTreeMap<BindingId, BTreeSet<ActorId>>,
) -> Result<Provenance, StopReason> {
    let cap = &m.capabilities[c.capability.0];
    let mut result = Provenance::default();
    result.roots.extend(&cap.reads);
    let mut certain = result.roots.clone();
    for arg in &c.args {
        if *arg != CURRENT_CASE {
            let value = bound.get(arg).ok_or(StopReason::SemanticAbort)?;
            result.roots.extend(&value.roots);
            result.unknown.extend(&value.unknown);
            certain.extend(value.roots.difference(&value.unknown).copied());
        }
    }
    for variable in &cap.read_variables {
        if let Some((_, value, true)) = &snapshots[variable.0] {
            result.roots.extend(&value.roots);
            result.unknown.extend(&value.unknown);
        }
    }
    if let Some(mask) = constraints.get(&c.binding) {
        result.unknown.extend(mask);
    }
    result.unknown.retain(|r| !certain.contains(r));
    Ok(result)
}

#[derive(Clone, Default)]
struct Provenance {
    roots: BTreeSet<ActorId>,
    unknown: BTreeSet<ActorId>,
}

pub(super) fn facts(
    m: &Model,
    trace: &[Observation],
    target: CapabilityId,
    direct: bool,
    required: &[ActorId],
) -> Result<(bool, bool, bool), StopReason> {
    let mut bound: BTreeMap<BindingId, Provenance> = BTreeMap::new();
    let mut started = BTreeMap::new();
    let mut commands: BTreeMap<BindingId, Provenance> = BTreeMap::new();
    let mut snapshots = vec![None; m.state_variables.len()];
    let constraints = super::evidence_scope::constraints(m);
    let mut active = false;
    let mut broken = false;
    let mut unsupported = m.contests().iter().any(|c| c.route == target);
    for o in trace {
        match o.event {
            Occurrence::Dispatch(cap) | Occurrence::Refusal(cap, _) => {
                let binding = o.binding.ok_or(StopReason::SemanticAbort)?;
                let Some(c) = call(&m.process.body, binding) else {
                    if m.site_contest(binding).is_some() {
                        continue;
                    }
                    return Err(StopReason::SemanticAbort);
                };
                require(c.capability == cap)?;
                if matches!(o.event, Occurrence::Refusal(_, _)) {
                    // A refused result can be bound without a normal Finish;
                    // it is not an activation and supplies no snapshot write.
                    if let Some((_, r)) = m
                        .response_attempts()
                        .into_iter()
                        .find(|(c, _)| c.binding == binding)
                    {
                        commands.insert(r.command, Provenance::default());
                    }
                    bound.insert(binding, Provenance::default());
                } else {
                    let inputs = sources(m, c, &bound, &snapshots, &constraints)?;
                    for w in &m.capabilities[cap.0].start_writes {
                        snapshots[w.variable.0] = Some((binding, inputs.clone(), false));
                    }
                    started.insert(binding, inputs);
                }
            }
            Occurrence::Completion(cap, _) => {
                let binding = o.binding.ok_or(StopReason::SemanticAbort)?;
                if m.site_contest(binding).is_some() {
                    continue;
                }
                let actual = started.remove(&binding).ok_or(StopReason::SemanticAbort)?;
                if cap == target {
                    active = true;
                    broken |= required.iter().any(|s| {
                        if direct {
                            !m.capabilities[cap.0].reads.contains(s)
                        } else {
                            !actual.roots.contains(s) && !actual.unknown.contains(s)
                        }
                    });
                    unsupported |= !direct && required.iter().any(|s| actual.unknown.contains(s));
                }
                let capability = &m.capabilities[cap.0];
                for snapshot in snapshots.iter_mut().flatten() {
                    if snapshot.0 == binding {
                        snapshot.2 = true;
                    }
                }
                let rejected = capability
                    .version_check
                    .as_ref()
                    .is_some_and(|v| matches!(o.event,Occurrence::Completion(_,r) if r==v.reject));
                let completed = match o.event {
                    Occurrence::Completion(_, v) => v,
                    _ => unreachable!(),
                };
                let base = if rejected {
                    &[][..]
                } else {
                    capability
                        .atomic
                        .as_ref()
                        .map_or(capability.writes.as_slice(), |a| {
                            if a.commit == completed {
                                a.writes.as_slice()
                            } else {
                                &[]
                            }
                        })
                };
                for variable in base
                    .iter()
                    .map(|w| w.variable)
                    .chain(capability.kernel_writes.iter().copied())
                {
                    snapshots[variable.0] = Some((binding, actual.clone(), true));
                }
                if let Some((_, r)) = m
                    .response_attempts()
                    .into_iter()
                    .find(|(c, _)| c.binding == binding)
                {
                    commands.insert(r.command, actual.clone());
                }
                bound.insert(binding, actual);
            }
            Occurrence::AdoptResponse(command, _) => {
                bound.insert(
                    command,
                    commands
                        .get(&command)
                        .ok_or(StopReason::SemanticAbort)?
                        .clone(),
                );
            }
            Occurrence::Merged(binding, ..) => {
                // Stage one rejects evidence targets downstream of any merge
                // at lowering. Unrelated calls may still consume this value;
                // do not abort their trace or claim evidence for the merge.
                bound.insert(binding, Provenance::default());
            }
            _ => {}
        }
    }
    Ok((active, broken, unsupported))
}
