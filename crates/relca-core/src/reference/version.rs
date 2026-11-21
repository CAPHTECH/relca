//! Independent prefix interpretation: look backwards from each completion to
//! its dispatch, and from that dispatch to the last completed examination.
//! No engine history, state key, or cached contract proof is consulted.
use super::*;

fn argument(body: &Block, binding: BindingId, parameter: usize) -> Result<BindingId, StopReason> {
    let mut blocks = vec![body.as_slice()];
    while let Some(block) = blocks.pop() {
        for s in block {
            match &s.kind {
                Stmt::Call(c) if c.binding == binding => {
                    return c
                        .args
                        .get(parameter)
                        .copied()
                        .ok_or(StopReason::SemanticAbort);
                }
                Stmt::Fork { branches, .. } => {
                    if let Some(c) = branches.iter().find(|c| c.binding == binding) {
                        return c
                            .args
                            .get(parameter)
                            .copied()
                            .ok_or(StopReason::SemanticAbort);
                    }
                }
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    blocks.push(else_body);
                    blocks.push(then_body);
                }
                _ => {}
            }
        }
    }
    Err(StopReason::SemanticAbort)
}

fn origin(m: &Model, trace: &[Observation], mut binding: BindingId) -> BindingId {
    for o in trace.iter().rev() {
        if let Occurrence::Merged(destination, source, ..) = o.event
            && destination == binding
        {
            binding = source;
        }
        if matches!(
            o.event,
            Occurrence::Completion(..) | Occurrence::Refusal(..)
        ) && let Some(source) = o.binding
            && m.loop_alias(source) == Some(binding)
        {
            binding = source;
        }
    }
    binding
}

fn input(
    m: &Model,
    trace: &[Observation],
    binding: BindingId,
    target: (CapabilityId, usize),
) -> Result<(usize, BindingId), StopReason> {
    let start = trace
        .iter()
        .rposition(|o| {
            o.binding == Some(binding)
                && matches!(o.event, Occurrence::Dispatch(c) if c == target.0)
        })
        .ok_or(StopReason::SemanticAbort)?;
    let arg = argument(&m.process.body, binding, target.1)?;
    require(arg != CURRENT_CASE)?;
    Ok((start, origin(m, &trace[..start], arg)))
}

fn normal_completion(m: &Model, o: &Observation, target: CapabilityId) -> bool {
    let Occurrence::Completion(cap, value) = o.event else {
        return false;
    };
    if cap != target {
        return false;
    }
    // Independently interpret the declared rejection branches, not the
    // engine's history or an enum spelling such as fail/duplicate/conflict.
    let cap = &m.capabilities[cap.0];
    if cap
        .version_check
        .as_ref()
        .is_some_and(|v| value == v.reject)
    {
        return false;
    }
    match &cap.atomic {
        Some(a) => value == a.commit,
        None => true,
    }
}

pub(super) fn facts(
    m: &Model,
    trace: &[Observation],
    first: (CapabilityId, usize),
    second: (CapabilityId, usize),
) -> Result<(bool, bool, bool), StopReason> {
    let unsupported = m
        .contests()
        .iter()
        .any(|c| c.route == first.0 || c.route == second.0);
    if unsupported {
        return Ok((false, false, true));
    }
    let mut active = false;
    let mut broken = false;
    for (end, o) in trace.iter().enumerate() {
        if !normal_completion(m, o, second.0) {
            continue;
        }
        active = true;
        let (start, used) = input(
            m,
            &trace[..end],
            o.binding.ok_or(StopReason::SemanticAbort)?,
            second,
        )?;
        let examined = trace[..start]
            .iter()
            .rposition(|o| normal_completion(m, o, first.0));
        if let Some(a) = examined {
            let (_, checked) = input(
                m,
                &trace[..a],
                trace[a].binding.ok_or(StopReason::SemanticAbort)?,
                first,
            )?;
            broken |= checked != used;
        } else {
            broken = true;
        }
    }
    Ok((active, broken, false))
}
