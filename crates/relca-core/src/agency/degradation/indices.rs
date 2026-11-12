//! Validate every index consumed by the independent degradation pass first.
use super::super::{Error, Result};
use crate::ir;

fn require(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidIr(format!("degradation: {reason}")))
    }
}

pub(crate) fn validate(m: &ir::Model) -> Result<()> {
    for a in &m.actors {
        require(
            a.executors.iter().flatten().all(|p| p.0 < m.people.len()),
            "executor index",
        )?;
    }
    for c in &m.capabilities {
        require(c.actor.0 < m.actors.len(), "capability Actor index")?;
        require(c.return_type.0 < m.types.len(), "return type index")?;
        require(
            c.reads.iter().all(|a| a.0 < m.actors.len()),
            "source Actor index",
        )?;
        require(
            c.uses.iter().all(|u| u.resource.0 < m.resources.len()),
            "resource index",
        )?;
        require(
            c.qualified_executors
                .iter()
                .flatten()
                .chain(c.permission.iter().flatten())
                .all(|p| p.0 < m.people.len()),
            "qualification/permission index",
        )?;
    }
    if let Some(t) = &m.resource_time {
        require(
            t.actors.len() == m.actors.len()
                && t.people.len() == m.people.len()
                && t.resources.len() == m.resources.len(),
            "calendar mapping length",
        )?;
        require(
            t.actors
                .iter()
                .chain(&t.people)
                .chain(&t.resources)
                .flatten()
                .all(|i| *i < t.calendars.len()),
            "calendar index",
        )?;
    }
    let call = |c: &ir::Call| -> Result<()> {
        require(
            c.capability.0 < m.capabilities.len(),
            "call capability index",
        )?;
        require(c.binding.0 < m.process.bindings.len(), "call binding index")?;
        require(
            c.args
                .iter()
                .all(|a| *a == ir::CURRENT_CASE || a.0 < m.process.bindings.len()),
            "argument index",
        )
    };
    let mut blocks = vec![(&m.process.body, 0usize)];
    while let Some((body, depth)) = blocks.pop() {
        require(depth <= 128, "control nesting exceeds static IR range")?;
        for s in body {
            match &s.kind {
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => call(c)?,
                ir::Stmt::Response(c, r) => {
                    call(c)?;
                    call(&r.command_call)?;
                    if let Some(b) = &r.recovery {
                        blocks.push((b, depth + 1));
                    }
                }
                ir::Stmt::Fork { branches, .. } => {
                    for c in branches {
                        call(c)?;
                    }
                }
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    let mut conditions = vec![(condition, 0usize)];
                    while let Some((c, level)) = conditions.pop() {
                        require(level <= 128, "condition nesting exceeds static IR range")?;
                        match c {
                            ir::Cond::Eq(x) => {
                                let binding =
                                    m.process.bindings.get(x.binding.0).ok_or_else(|| {
                                        Error::InvalidIr("degradation: guard binding index".into())
                                    })?;
                                require(
                                    m.types
                                        .get(binding.ty.0)
                                        .is_some_and(|t| x.value.0 < t.members.len()),
                                    "guard value/type index",
                                )?;
                            }
                            ir::Cond::And(cs) | ir::Cond::Or(cs) => {
                                conditions.extend(cs.iter().map(|c| (c, level + 1)))
                            }
                            _ => (), // Timeless degradation refuses state guards without indexing them.
                        }
                    }
                    blocks.push((then_body, depth + 1));
                    blocks.push((else_body, depth + 1));
                }
                ir::Stmt::End(e) => require(e.0 < m.ends.len(), "End index")?,
                _ => (),
            }
        }
    }
    for r in &m.process.degradations {
        require(r.producer.0 < m.capabilities.len(), "producer index")?;
        let binding = m
            .process
            .bindings
            .get(r.condition.binding.0)
            .ok_or_else(|| Error::InvalidIr("degradation: condition binding index".into()))?;
        require(
            m.types
                .get(binding.ty.0)
                .is_some_and(|t| r.condition.value.0 < t.members.len()),
            "condition value/type index",
        )?;
        require(
            m.process
                .body
                .iter()
                .any(|s| s.span == r.span && matches!(s.kind, ir::Stmt::If { .. })),
            "route span",
        )?;
    }
    Ok(())
}
