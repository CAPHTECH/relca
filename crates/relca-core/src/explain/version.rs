//! Map the selected prefix to value-version evidence, without adding facts to
//! ordinary check output. The prefix is separately replayed by the reference.
use super::*;
use std::collections::BTreeMap;

fn call<'a>(m: &ir::Model, body: &'a ir::Block, binding: &str) -> Option<&'a ir::Call> {
    body.iter().find_map(|s| match &s.kind {
        ir::Stmt::Call(c) if m.site_name(c.binding) == binding => Some(c),
        ir::Stmt::Fork { branches, .. } => {
            branches.iter().find(|c| m.site_name(c.binding) == binding)
        }
        ir::Stmt::If {
            then_body,
            else_body,
            ..
        } => call(m, then_body, binding).or_else(|| call(m, else_body, binding)),
        _ => None,
    })
}

fn value(
    m: &ir::Model,
    stage: &Stage,
    target: (ir::CapabilityId, usize),
    aliases: &BTreeMap<String, String>,
) -> Result<VersionValue, ReplayReason> {
    let binding = stage
        .binding
        .as_ref()
        .ok_or(ReplayReason::SourceMappingFailed)?;
    let c = call(m, &m.process.body, binding).ok_or(ReplayReason::SourceMappingFailed)?;
    let arg = *c
        .args
        .get(target.1)
        .ok_or(ReplayReason::SourceMappingFailed)?;
    if arg == ir::CURRENT_CASE {
        return Err(ReplayReason::SourceMappingFailed);
    }
    let name = m.site_name(arg);
    let cap = &m.capabilities[target.0.0];
    Ok(VersionValue {
        binding: binding.clone(),
        capability: format!("{}.{}", m.actors[cap.actor.0].name, cap.name),
        parameter: cap.params[target.1].name.clone(),
        version: aliases.get(&name).cloned().unwrap_or(name),
    })
}

fn normal_finish(
    m: &ir::Model,
    stage: &Stage,
    target: ir::CapabilityId,
) -> Result<bool, ReplayReason> {
    let cap = &m.capabilities[target.0];
    if cap.version_check.is_none() && cap.atomic.is_none() {
        return Ok(true);
    }
    let result = stage
        .result
        .as_ref()
        .and_then(|r| {
            m.types[cap.return_type.0]
                .members
                .iter()
                .position(|v| v == r)
        })
        .map(ir::ValueId)
        .ok_or(ReplayReason::SourceMappingFailed)?;
    Ok(crate::same_version::normally_completed(cap, result))
}

pub(super) fn for_trace(
    m: &ir::Model,
    id: &str,
    steps: &[Stage],
) -> Result<Option<Version>, ReplayReason> {
    let Some((first, second)) = m.contracts.iter().enumerate().find_map(|(i, c)| {
        if c.property_id(ir::ClauseId(i)) != id {
            return None;
        }
        match c.kind {
            ir::ContractKind::SameVersion { first, second } => Some((first, second)),
            _ => None,
        }
    }) else {
        return Ok(None);
    };
    let last = steps.last().ok_or(ReplayReason::ViolationMismatch)?;
    let matches = |s: &Stage, cap: ir::CapabilityId| {
        let c = &m.capabilities[cap.0];
        s.actor.as_deref() == Some(m.actors[c.actor.0].name.as_str())
            && s.capability.as_deref() == Some(c.name.as_str())
    };
    if last.kind != Kind::Finish || !matches(last, second.0) || !normal_finish(m, last, second.0)? {
        return Err(ReplayReason::ViolationMismatch);
    }
    let mut aliases = BTreeMap::new();
    let mut examined = None;
    let mut first_started = BTreeMap::new();
    let mut second_started = BTreeMap::new();
    for s in steps.iter().filter(|s| s.case_id == last.case_id) {
        let Some(binding) = &s.binding else { continue };
        if s.kind == Kind::Merged {
            let source = s.source.as_ref().ok_or(ReplayReason::SourceMappingFailed)?;
            aliases.insert(
                binding.clone(),
                aliases
                    .get(source)
                    .cloned()
                    .unwrap_or_else(|| source.clone()),
            );
        }
        if s.kind == Kind::Start {
            if matches(s, first.0) {
                first_started.insert(binding.clone(), value(m, s, first, &aliases)?);
            }
            if matches(s, second.0) {
                second_started.insert(
                    binding.clone(),
                    (value(m, s, second, &aliases)?, examined.clone()),
                );
            }
        }
        if s.kind == Kind::Finish {
            if s.index == last.index {
                let (used, examined): (VersionValue, Option<VersionValue>) = second_started
                    .remove(binding)
                    .ok_or(ReplayReason::SourceMappingFailed)?;
                if examined.as_ref().is_some_and(|a| a.version == used.version) {
                    return Err(ReplayReason::ViolationMismatch);
                }
                return Ok(Some(Version { used, examined }));
            }
            if matches(s, first.0) {
                let finished = first_started
                    .remove(binding)
                    .ok_or(ReplayReason::SourceMappingFailed)?;
                if normal_finish(m, s, first.0)? {
                    examined = Some(finished);
                }
            }
        }
    }
    Err(ReplayReason::SourceMappingFailed)
}
