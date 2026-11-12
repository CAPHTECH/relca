//! Independent reference dispatch, reused by tree, sample and replay interpreters.
use super::*;
type StartCandidates = BTreeMap<(usize, usize), Vec<Option<PersonId>>>;

pub(super) fn finish_allowed(
    m: &Model,
    frames: &[Option<&Frame<'_>>],
    case: usize,
    task: usize,
    now: Tick,
) -> bool {
    finish_order(m, frames, case, task, now).0
}

pub(super) fn finish_order(
    m: &Model,
    frames: &[Option<&Frame<'_>>],
    case: usize,
    task: usize,
    now: Tick,
) -> (bool, bool) {
    let own = &frames[case].expect("finish candidate").tasks[task];
    let capability = &m.capabilities[own.call.capability.0];
    let Some(check) = &capability.version_check else {
        return (true, false);
    };
    let Some(order) = check.resolve else {
        return (true, false);
    };
    let ranks = &m
        .case_priority
        .as_ref()
        .expect("validated resolution table")
        .ranks;
    let mut allowed = true;
    let mut unresolved = false;
    for (k, frame) in frames.iter().enumerate() {
        if let Some(frame) = frame {
            for (index, peer) in frame.tasks.iter().enumerate() {
                if peer.phase != Phase::Running || peer.finish != Some(now) {
                    continue;
                }
                let cap = &m.capabilities[peer.call.capability.0];
                if cap.actor != capability.actor {
                    continue;
                }
                let Some(receipt) = &cap.version_check else {
                    continue;
                };
                if receipt.resolve != Some(order)
                    || !check
                        .variables
                        .iter()
                        .any(|v| receipt.variables.contains(v))
                {
                    continue;
                }
                if ranks[k] > ranks[case]
                    || (ranks[k] == ranks[case] && order == ResolutionTie::CaseId && k < case)
                {
                    allowed = false;
                }
                if (k != case || index != task)
                    && ranks[k] == ranks[case]
                    && (order == ResolutionTie::Nondet || k == case)
                {
                    unresolved = true;
                }
            }
        }
    }
    (allowed, unresolved)
}

pub(super) fn held(m: &Model, waiting: &[bool], arrivals: &[Tick]) -> Vec<usize> {
    let Some(order) = m.external_priority() else {
        return Vec::new();
    };
    let input = m
        .case_priority
        .as_ref()
        .expect("validated external priority");
    let highest = waiting
        .iter()
        .enumerate()
        .filter(|(_, held)| **held)
        .map(|(k, _)| input.ranks[k])
        .max();
    let mut result: Vec<_> = waiting
        .iter()
        .enumerate()
        .filter_map(|(k, held)| (*held && Some(input.ranks[k]) == highest).then_some(k))
        .collect();
    if order == QueueTie::FifoCaseId {
        result.sort_by_key(|k| (arrivals[*k], *k));
        result.truncate(1);
    }
    result
}

// Independent strict relation. Equal ranks without an explicit tie do not
// constrain each other, even if the pending vector has a stable iteration order.
fn precedes(m: &Model, left: (Tick, usize, usize), right: (Tick, usize, usize)) -> bool {
    let priority = m
        .case_priority
        .as_ref()
        .expect("validated reference priority");
    let policy = m.start_policy.as_ref().expect("validated reference policy");
    let higher = priority.ranks[left.1];
    let lower = priority.ranks[right.1];
    if higher != lower {
        return higher > lower;
    }
    if policy.priority != Some(QueueTie::FifoCaseId) {
        return false;
    }
    if (left.0, left.1) != (right.0, right.1) {
        return (left.0, left.1) < (right.0, right.1);
    }
    policy.call_order && left.2 < right.2
}

pub(super) fn validate_priority(m: &Model) -> Execution {
    let requested = m
        .start_policy
        .as_ref()
        .is_some_and(|p| p.priority.is_some())
        || m.external_priority().is_some()
        || m.capabilities.iter().any(|c| {
            c.version_check
                .as_ref()
                .is_some_and(|v| v.resolve.is_some())
        });
    match &m.case_priority {
        None => require(!requested),
        Some(input) => {
            require(!input.ranks.is_empty() && input.ranks.len() as u64 == m.bounds.cases)?;
            let groups = [
                &input.classes.policy,
                &input.classes.environment,
                &input.classes.party,
            ];
            let mut actors = vec![false; m.actors.len()];
            for group in groups {
                require(group.windows(2).all(|w| w[0] < w[1]))?;
                for actor in group {
                    require(actor.0 < actors.len())?;
                    require(!actors[actor.0])?;
                    actors[actor.0] = true;
                }
            }
            require(actors.into_iter().all(|x| x))?;
            if let Some(p) = &m.start_policy
                && p.priority.is_some()
            {
                require(p.classes.as_ref() == Some(&input.classes))?;
                require(p.priority != Some(QueueTie::Nondet) || !p.call_order)?;
                validate(m)?;
            }
            let mut external = None;
            for q in m.admission_queue.iter().chain(m.concurrent_cases.iter()) {
                if let Some(order) = external {
                    require(q.overflow == order)?;
                }
                external = Some(q.overflow);
                if matches!(q.overflow, Overflow::WaitExternalPriority(_)) {
                    require(q.capacity > 0)?;
                }
            }
            for cap in &m.capabilities {
                let Some(check) = &cap.version_check else {
                    continue;
                };
                if check.resolve.is_some() {
                    require(input.classes.policy.contains(&cap.actor))?;
                    require(cap.actor.0 < m.actors.len() && !check.variables.is_empty())?;
                    for id in &check.variables {
                        let state = m
                            .state_variables
                            .get(id.0)
                            .ok_or(StopReason::SemanticAbort)?;
                        require(
                            matches!(state.kind, StateKind::ActorObserved(owner) if owner == cap.actor && !m.actors[owner.0].is_per_case()),
                        )?;
                    }
                }
                for other in &m.capabilities {
                    if other.actor == cap.actor
                        && let Some(peer) = &other.version_check
                        && peer.variables.iter().any(|id| check.variables.contains(id))
                    {
                        require(peer.resolve == check.resolve)?;
                    }
                }
            }
            Ok(())
        }
    }
}

/// A future opening cannot release a task hidden behind a permanently
/// unstartable FIFO/call-order predecessor. Inspect the whole reference queue.
pub(super) fn calendar_pending(
    m: &Model,
    frames: &[Option<&Frame<'_>>],
    queue: &[BTreeMap<BindingId, Tick>],
    active_cases: &[bool],
) -> Result<bool, StopReason> {
    require(frames.len() == queue.len() && frames.len() == active_cases.len())?;
    let ordered = m.start_policy.as_ref().is_some_and(|p| p.call_order);
    let ranks = if ordered { lexical(m)? } else { Vec::new() };
    let mut pending = Vec::new();
    for (case, frame) in frames.iter().enumerate() {
        if let Some(frame) = frame {
            for task in &frame.tasks {
                if task.phase != Phase::Queued {
                    continue;
                }
                let at = *queue[case]
                    .get(&task.call.binding)
                    .ok_or(StopReason::SemanticAbort)?;
                let rank = if ordered {
                    ranks
                        .iter()
                        .position(|b| *b == task.call.binding)
                        .ok_or(StopReason::SemanticAbort)?
                } else {
                    0
                };
                pending.push(((at, case, rank), task.call.capability));
            }
        }
    }
    for (own, id) in &pending {
        // Keep stopped queued Tasks as predecessors, just like policy::starts
        // and the engine; candidate liveness is a separate test.
        if !active_cases[own.1] {
            continue;
        }
        let cap = &m.capabilities[id.0];
        if !m.actors[cap.actor.0].capacity.is_some_and(|n| n > 0)
            || cap.qualified_executors.as_ref().is_some_and(Vec::is_empty)
            || cap
                .uses
                .iter()
                .any(|u| u.amount > m.resources[u.resource.0].capacity)
        {
            continue;
        }
        let fifo_block = m
            .start_policy
            .as_ref()
            .is_some_and(|p| p.priority.is_none())
            && pending.iter().any(|(key, id)| {
                m.capabilities[id.0].actor == cap.actor && (key.0, key.1) < (own.0, own.1)
            });
        let order_block = m.start_policy.as_ref().is_some_and(|p| {
            p.call_order
                && p.priority.is_none()
                && p.controls(cap.actor)
                && pending.iter().any(|(key, id)| {
                    let before = &m.capabilities[id.0];
                    key < own
                        && p.controls(before.actor)
                        && (before.actor == cap.actor
                            || before
                                .uses
                                .iter()
                                .any(|u| cap.uses.iter().any(|v| u.resource == v.resource)))
                })
        });
        let priority_block = m.start_policy.as_ref().is_some_and(|p| {
            p.priority.is_some()
                && p.controls(cap.actor)
                && pending.iter().any(|(key, id)| {
                    let before = &m.capabilities[id.0];
                    p.controls(before.actor)
                        && precedes(m, *key, *own)
                        && (before.actor == cap.actor
                            || before
                                .uses
                                .iter()
                                .any(|u| cap.uses.iter().any(|v| u.resource == v.resource)))
                })
        });
        if !fifo_block && !order_block && !priority_block {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn awaits_calendar(m: &Model, f: &Frame<'_>) -> bool {
    f.tasks.iter().any(|t| {
        let cap = &m.capabilities[t.call.capability.0];
        t.phase == Phase::Queued
            && !cap.qualified_executors.as_ref().is_some_and(Vec::is_empty)
            && m.actors[cap.actor.0].capacity.is_some_and(|n| n > 0)
            && cap
                .uses
                .iter()
                .all(|u| u.amount <= m.resources[u.resource.0].capacity)
    })
}

/// Independent eligibility calculation, also used outside a fixed policy.
pub(super) fn calendar_slots(
    m: &Model,
    cap: &Capability,
    now: Tick,
    used: u64,
    units: &[u64],
) -> bool {
    let Some(t) = &m.resource_time else {
        return true;
    };
    if let Some(id) = t.actors[cap.actor.0] {
        let slots = t.calendars[id]
            .intervals
            .iter()
            .find(|i| i.from <= now && now < i.until);
        if slots.is_none_or(|i| used >= i.capacity) {
            return false;
        }
    }
    for claim in &cap.uses {
        if let Some(id) = t.resources[claim.resource.0] {
            let slots = t.calendars[id]
                .intervals
                .iter()
                .find(|i| i.from <= now && now < i.until);
            if slots.is_none_or(|i| {
                units[claim.resource.0] > i.capacity
                    || claim.amount > i.capacity - units[claim.resource.0]
            }) {
                return false;
            }
        }
    }
    true
}

pub(super) fn working(m: &Model, person: PersonId, now: Tick) -> bool {
    m.resource_time.as_ref().is_none_or(|t| {
        t.people[person.0].is_none_or(|id| {
            t.calendars[id]
                .intervals
                .iter()
                .any(|i| i.from <= now && now < i.until && i.capacity == 1)
        })
    })
}

fn lexical(m: &Model) -> Result<Vec<BindingId>, StopReason> {
    fn visit(body: &Block, result: &mut Vec<BindingId>) {
        for statement in body {
            match &statement.kind {
                Stmt::Call(call) | Stmt::Response(call, _) => result.push(call.binding),
                Stmt::Fork { branches, .. } => {
                    result.extend(branches.iter().map(|call| call.binding))
                }
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    visit(then_body, result);
                    visit(else_body, result);
                }
                _ => {}
            }
        }
    }
    let mut result = Vec::new();
    visit(&m.process.body, &mut result);
    let unique: BTreeSet<_> = result.iter().copied().collect();
    let mut represented = unique.clone();
    if m.start_policy
        .as_ref()
        .is_some_and(|p| p.priority.is_some())
        || m.uses_work_profiles()
    {
        for (_, response) in m.response_attempts() {
            require(
                response.command.0 < m.process.bindings.len()
                    && !unique.contains(&response.command),
            )?;
            represented.insert(response.command);
        }
    }
    require(
        represented.len() == m.process.bindings.len()
            && unique.len() == result.len()
            && result.iter().all(|b| b.0 < m.process.bindings.len()),
    )?;
    result.extend((0..m.contests().len()).map(|i| m.contest_site(i)));
    Ok(result)
}

pub(super) fn validate(m: &Model) -> Execution {
    if let Some(p) = &m.start_policy {
        require(p.extended() == p.classes.is_some())?;
        if p.extended() {
            require(p.executor_order.len() == m.actors.len())?;
            for (i, actor) in m.actors.iter().enumerate() {
                match (&actor.executors, &p.executor_order[i]) {
                    (None, None) => {}
                    (Some(ids), Some(order)) => {
                        require(order.iter().all(|person| person.0 < m.people.len()))?;
                        let set: BTreeSet<_> = order.iter().collect();
                        require(set.len() == order.len() && set == ids.iter().collect())?;
                    }
                    _ => require(false)?,
                }
            }
        } else {
            require(p.executor_order.is_empty())?;
        }
        if let Some(c) = &p.classes {
            let mut actors = vec![false; m.actors.len()];
            for class in [&c.policy, &c.environment, &c.party] {
                require(class.windows(2).all(|pair| pair[0] < pair[1]))?;
                for a in class {
                    require(a.0 < actors.len())?;
                    require(!actors[a.0])?;
                    actors[a.0] = true;
                }
            }
            require(actors.into_iter().all(|seen| seen))?;
            lexical(m)?;
        }
    }
    Ok(())
}

pub(super) fn starts(
    m: &Model,
    frames: &[Option<&Frame<'_>>],
    queue: &[BTreeMap<BindingId, Tick>],
) -> Result<StartCandidates, StopReason> {
    let p = m.start_policy.as_ref().ok_or(StopReason::SemanticAbort)?;
    let order = lexical(m)?;
    let mut pending = Vec::new();
    let mut occupied = vec![0u64; m.actors.len()];
    let mut people = BTreeSet::new();
    for (case, frame) in frames.iter().enumerate() {
        if let Some(frame) = frame {
            for (index, task) in frame.tasks.iter().enumerate() {
                if task.phase == Phase::Running {
                    occupied[m.capabilities[task.call.capability.0].actor.0] += 1;
                    if let Some(person) = task.person {
                        people.insert(person);
                    }
                } else if task.phase == Phase::Queued {
                    let at = *queue[case]
                        .get(&task.call.binding)
                        .ok_or(StopReason::SemanticAbort)?;
                    let rank = order
                        .iter()
                        .position(|b| *b == task.call.binding)
                        .ok_or(StopReason::SemanticAbort)?;
                    pending.push(((at, case, rank), index, task.call.capability));
                }
            }
        }
    }
    pending.sort_by_key(|(key, _, _)| *key);
    let quantities =
        reference_resource_units(frames.iter().flatten().flat_map(|f| f.tasks.iter()), m)?;
    let mut chosen = BTreeMap::new();
    let mut policy_taken = false;
    for &(key, index, capability) in &pending {
        if p.priority.is_some()
            && frames[key.1].is_some_and(|f| {
                f.trace.iter().any(|o|
            matches!(o.event, Occurrence::ResponseStopped(id) if id.0 < m.process.bindings.len()))
            })
        {
            continue;
        }
        let cap = &m.capabilities[capability.0];
        let actor = &m.actors[cap.actor.0];
        let controlled = p.controls(cap.actor);
        let now = frames[key.1].ok_or(StopReason::SemanticAbort)?.now;
        if !calendar_slots(m, cap, now, occupied[cap.actor.0], &quantities) {
            continue;
        }
        if p.priority.is_none() && controlled && p.call_order && policy_taken {
            continue;
        }
        if occupied[cap.actor.0] >= actor.capacity.ok_or(StopReason::SemanticAbort)? {
            continue;
        }
        if cap.uses.iter().any(|u| {
            quantities[u.resource.0] > m.resources[u.resource.0].capacity
                || u.amount > m.resources[u.resource.0].capacity - quantities[u.resource.0]
        }) {
            continue;
        }
        if pending.iter().any(|&(earlier, _, other)| {
            let other = &m.capabilities[other.0];
            (p.priority.is_none()
                && other.actor == cap.actor
                && (earlier.0, earlier.1) < (key.0, key.1))
                || (controlled
                    && p.priority.is_none()
                    && p.call_order
                    && earlier < key
                    && p.controls(other.actor)
                    && (other.actor == cap.actor
                        || other
                            .uses
                            .iter()
                            .any(|a| cap.uses.iter().any(|b| a.resource == b.resource))))
                || (controlled
                    && p.priority.is_some()
                    && p.controls(other.actor)
                    && precedes(m, earlier, key)
                    && (other.actor == cap.actor
                        || other
                            .uses
                            .iter()
                            .any(|u| cap.uses.iter().any(|v| u.resource == v.resource))))
        }) {
            continue;
        }
        let executors = if controlled && p.assign_first_idle {
            &p.executor_order[cap.actor.0]
        } else {
            &actor.executors
        };
        let mut free: Vec<_> = match executors {
            None => vec![None],
            Some(ids) => ids
                .iter()
                .filter(|person| {
                    cap.qualified_executors
                        .as_ref()
                        .is_none_or(|eligible| eligible.contains(person))
                        && !people.contains(person)
                        && working(m, **person, now)
                })
                .map(|id| Some(*id))
                .collect(),
        };
        if controlled && p.assign_first_idle {
            free.truncate(1);
        }
        if free.is_empty() {
            continue;
        }
        chosen.insert((key.1, index), free);
        policy_taken |= p.priority.is_none() && controlled && p.call_order;
    }
    if p.priority.is_some() {
        let available = chosen.clone();
        chosen.retain(|&(case, index), _| {
            let task = &frames[case].expect("candidate frame").tasks[index];
            if !p.controls(m.capabilities[task.call.capability.0].actor) {
                return true;
            }
            let key = pending
                .iter()
                .find(|(key, i, _)| key.1 == case && *i == index)
                .expect("pending candidate")
                .0;
            !pending.iter().any(|(other, i, cap)| {
                available.contains_key(&(other.1, *i))
                    && p.controls(m.capabilities[cap.0].actor)
                    && precedes(m, *other, key)
            })
        });
    }
    Ok(chosen)
}
