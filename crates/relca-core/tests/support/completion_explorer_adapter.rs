//! Data-only adapter. Never executes production semantics or falls back to them.
use super::completion_explorer::*;
use relca_core::ir;

#[derive(Debug, PartialEq, Eq)]
pub struct NotCovered(pub &'static str);
pub fn adapt(m: &ir::Model) -> Result<OwnModel, NotCovered> {
    if m.bounds.cases != 1
        || m.bounds.clock > 20
        || !(1..=3).contains(&m.bounds.live_tasks)
        || m.arrivals.is_some()
        || m.scenario.is_some()
        || m.horizon.is_some()
        || m.start_policy.is_some()
        || m.admission_queue.is_some()
        || m.concurrent_cases.is_some()
        || m.simulation.is_some()
        || m.quality.is_some()
        || m.metrics.is_some()
        || !m.state_variables.is_empty()
        || !m.kernels.is_empty()
        || !m.authorities.is_empty()
        || !m.contracts.is_empty()
        || m.responsibility.is_some()
        || !m.probability_queries.is_empty()
        || !m.initial_distributions.is_empty()
        || !m.concurrency_resolutions.is_empty()
        || !m.fallback_capabilities.is_empty()
        || m.comparison.is_some()
        || m.resources.len() > 1
        || m.resources.iter().any(|r| r.capacity > 2)
        || m.arrival_every.is_some()
        || m.overflow_end.is_some()
    {
        return Err(NotCovered("outside single-case finite fixed-time lane"));
    }
    if m.cut.as_ref().is_some_and(|c| {
        c.include.len() != m.actors.len() || !c.exclude.is_empty() || !c.defer.is_empty()
    }) {
        return Err(NotCovered("lane includes all Actors"));
    }
    if m.types
        .iter()
        .any(|t| t.record.is_some() || t.members.is_empty() || t.members.len() > 2)
        || m.verify.iter().any(|v| {
            !matches!(
                v.property,
                ir::Property::RecoverableEnd
                    | ir::Property::TerminatingEnd
                    | ir::Property::AlmostSureEnd
                    | ir::Property::ReachableEnd(_)
            )
        })
    {
        return Err(NotCovered("finite enums and completion queries only"));
    }
    let actors = m
        .actors
        .iter()
        .map(|a| {
            if a.capacity.is_none_or(|c| c > 2)
                || a.metadata.instance.is_some()
                || a.metadata.rule.is_some()
                || a.metadata.contains.is_some()
                || a.metadata.overlaps.is_some()
            {
                return Err(NotCovered("Actor shape"));
            }
            Ok(OwnActor {
                name: a.name.clone(),
                capacity: a.capacity.unwrap(),
                people: a
                    .executors
                    .as_ref()
                    .map(|ps| ps.iter().map(|p| m.people[p.0].name.clone()).collect()),
            })
        })
        .collect::<Result<_, _>>()?;
    let capabilities = m
        .capabilities
        .iter()
        .map(|c| {
            if c.duration == 0
                || c.duration > 8
                || c.duration_distribution.is_some()
                || c.kernel.is_some()
                || c.atomic.is_some()
                || c.version_check.is_some()
                || c.requires.is_some()
                || !c.start_writes.is_empty()
                || !c.writes.is_empty()
                || !c.kernel_writes.is_empty()
                || !c.kernel_reads_latent.is_empty()
                || !c.params.is_empty()
                || c.commits
                || c.permission.is_some()
                || c.cancel != ir::Cancel::ForbiddenAfterStart
                || c.uses.len() > 1
                || c.uses
                    .iter()
                    .any(|u| u.resource.0 != 0 || u.amount != 1 || m.resources.is_empty())
                || c.results.is_empty()
                || c.results.len() > 2
            {
                return Err(NotCovered("Capability shape"));
            }
            Ok(OwnCapability {
                name: format!("{}.{}", m.actors[c.actor.0].name, c.name),
                actor: c.actor.0,
                duration: c.duration,
                results: c
                    .results
                    .iter()
                    .map(|v| m.types[c.return_type.0].members[v.0].clone())
                    .collect(),
                uses: c.uses.first().map_or(0, |u| u.amount),
                location: (c.span.start, c.span.end),
            })
        })
        .collect::<Result<_, _>>()?;
    fn condition(c: &ir::Cond, m: &ir::Model) -> Result<OwnCondition, NotCovered> {
        Ok(match c {
            ir::Cond::Eq(r) => OwnCondition::Equal(
                r.binding.0,
                m.types[m.process.bindings[r.binding.0].ty.0].members[r.value.0].clone(),
            ),
            ir::Cond::And(cs) => OwnCondition::All(
                cs.iter()
                    .map(|c| condition(c, m))
                    .collect::<Result<_, _>>()?,
            ),
            ir::Cond::Or(cs) => OwnCondition::Any(
                cs.iter()
                    .map(|c| condition(c, m))
                    .collect::<Result<_, _>>()?,
            ),
            _ => return Err(NotCovered("only result conditions")),
        })
    }
    fn call(c: &ir::Call) -> Result<(usize, usize), NotCovered> {
        if !c.args.is_empty() {
            return Err(NotCovered("no arguments"));
        }
        Ok((c.binding.0, c.capability.0))
    }
    fn block(b: &ir::Block, m: &ir::Model) -> Result<Vec<Op>, NotCovered> {
        b.iter()
            .map(|s| {
                Ok(match &s.kind {
                    ir::Stmt::LoopControl(_) => return Err(NotCovered("bounded repetition")),
                    ir::Stmt::Query(_, _)
                    | ir::Stmt::ResolveEffect { .. }
                    | ir::Stmt::StopReceiptUnknown => {
                        panic!("receipt recovery is outside this oracle")
                    }
                    ir::Stmt::WaitUntil(_) => {
                        panic!("completion explorer adapter does not handle condition waits")
                    }
                    ir::Stmt::Response(_, _) => {
                        return Err(NotCovered("receipt and bounded retry semantics"));
                    }
                    ir::Stmt::Call(c) => Op::Calls(vec![call(c)?]),
                    ir::Stmt::Fork {
                        branches,
                        join: ir::Join::All,
                        ..
                    } if branches.len() as u64 <= m.bounds.live_tasks && branches.len() <= 3 => {
                        Op::Calls(branches.iter().map(call).collect::<Result<_, _>>()?)
                    }
                    ir::Stmt::Fork { .. } => return Err(NotCovered("join any or live-task cut")),
                    ir::Stmt::Wait(t) if *t <= 8 => Op::Wait(*t),
                    ir::Stmt::Wait(_) => return Err(NotCovered("long wait")),
                    ir::Stmt::End(e) => Op::End(m.ends[e.0].clone(), (s.span.start, s.span.end)),
                    ir::Stmt::If {
                        condition: c,
                        then_body,
                        else_body,
                    } => Op::If(
                        condition(c, m)?,
                        block(then_body, m)?,
                        block(else_body, m)?,
                        (s.span.start, s.span.end),
                    ),
                })
            })
            .collect()
    }
    fn ends_on_every_control_path(b: &ir::Block) -> bool {
        b.iter().any(|s| match &s.kind {
            ir::Stmt::Response(_, _) => false, // Explicitly outside this independent explorer.
            ir::Stmt::End(_) => true,
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => ends_on_every_control_path(then_body) && ends_on_every_control_path(else_body),
            _ => false,
        })
    }
    if !ends_on_every_control_path(&m.process.body) {
        return Err(NotCovered("explicit end required on every control path"));
    }
    if m.process.bindings.len() > 8 {
        return Err(NotCovered("small binding bound"));
    }
    Ok(OwnModel {
        actors,
        capabilities,
        people: m
            .people
            .iter()
            .map(|p| (p.name.clone(), (p.span.start, p.span.end)))
            .collect(),
        resource: m.resources.first().map(|r| (r.name.clone(), r.capacity)),
        bindings: m.process.bindings.iter().map(|b| b.name.clone()).collect(),
        body: block(&m.process.body, m)?,
        clock: m.bounds.clock,
    })
}
