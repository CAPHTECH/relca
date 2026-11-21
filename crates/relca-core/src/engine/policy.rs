//! Fixed-policy declaration validation and lexical order; never execution state.
use super::*;

pub(super) fn queued_rank(ranks: &[usize], site: ir::BindingId) -> EngineResult<usize> {
    ranks
        .get(site.0)
        .copied()
        .filter(|&rank| rank != usize::MAX)
        .ok_or_else(|| "policy queued Task has no lexical rank".into())
}

#[cfg(test)]
#[test]
fn result_alias_never_supplies_a_queued_rank() {
    assert_eq!(queued_rank(&[0, usize::MAX], ir::BindingId(0)).unwrap(), 0);
    for site in [1, 2] {
        assert_eq!(
            queued_rank(&[0, usize::MAX], ir::BindingId(site)),
            Err("policy queued Task has no lexical rank".into())
        );
    }
}

/// Strict precedence among currently queued controlled Tasks, not a sort of all
/// events. Large ranks use comparison, never negation or subtraction.
pub(super) fn priority_before(
    m: &ir::Model,
    a: (ir::Tick, usize, usize),
    b: (ir::Tick, usize, usize),
) -> bool {
    let input = m.case_priority.as_ref().expect("validated priority table");
    let p = m.start_policy.as_ref().expect("validated priority policy");
    match input.ranks[a.1].cmp(&input.ranks[b.1]) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => {
            p.priority == Some(ir::QueueTie::FifoCaseId)
                && ((a.0, a.1) < (b.0, b.1)
                    || (p.call_order && (a.0, a.1) == (b.0, b.1) && a.2 < b.2))
        }
    }
}

pub(super) fn validate_priority(m: &ir::Model) -> EngineResult<()> {
    let bad = "invalid_priority_ir";
    let used = m.external_priority().is_some()
        || m.start_policy
            .as_ref()
            .is_some_and(|p| p.priority.is_some())
        || m.capabilities.iter().any(|c| {
            c.version_check
                .as_ref()
                .is_some_and(|v| v.resolve.is_some())
        });
    let Some(table) = &m.case_priority else {
        return if used { Err(bad.into()) } else { Ok(()) };
    };
    if table.ranks.len() as u64 != m.bounds.cases || table.ranks.is_empty() {
        return Err(bad.into());
    }
    let mut seen = BTreeSet::new();
    for group in [
        &table.classes.policy,
        &table.classes.environment,
        &table.classes.party,
    ] {
        if group.windows(2).any(|w| w[0] >= w[1])
            || group
                .iter()
                .any(|a| a.0 >= m.actors.len() || !seen.insert(*a))
        {
            return Err(bad.into());
        }
    }
    if seen.len() != m.actors.len() {
        return Err(bad.into());
    }
    if let Some(p) = &m.start_policy
        && p.priority.is_some()
        && (p.classes.as_ref() != Some(&table.classes)
            || (p.priority == Some(ir::QueueTie::Nondet) && p.call_order)
            || validate(m).is_err())
    {
        return Err(bad.into());
    }
    let limits: Vec<_> = m
        .admission_queue
        .iter()
        .chain(m.concurrent_cases.iter())
        .collect();
    if limits.windows(2).any(|w| w[0].overflow != w[1].overflow)
        || limits
            .iter()
            .any(|q| matches!(q.overflow, ir::Overflow::WaitExternalPriority(_)) && q.capacity == 0)
    {
        return Err(bad.into());
    }
    for c in &m.capabilities {
        let Some(v) = &c.version_check else { continue };
        if v.resolve.is_some()
            && (c.actor.0 >= m.actors.len() || !table.classes.policy.contains(&c.actor) || v.variables.is_empty()
                || v.variables.iter().any(|id| m.state_variables.get(id.0).is_none_or(|s|
                    !matches!(s.kind, ir::StateKind::ActorObserved(a) if a == c.actor && !m.actors[a.0].is_per_case())))) {
                return Err(bad.into());
        }
        for other in &m.capabilities {
            if let Some(w) = &other.version_check
                && c.actor == other.actor
                && v.variables.iter().any(|id| w.variables.contains(id))
                && v.resolve != w.resolve
            {
                return Err(bad.into());
            }
        }
    }
    Ok(())
}

pub(super) fn ranks(m: &ir::Model) -> EngineResult<Vec<usize>> {
    if m.start_policy
        .as_ref()
        .is_some_and(|p| p.extended() && p.priority.is_none())
        && !m.uses_work_profiles()
        && !m.response_attempts().is_empty()
    {
        return Err("policy_feature_combination: policy ordering with response_loss or retry requires priority or a work profile".into());
    }
    fn walk(
        body: &ir::Block,
        sites: &mut Vec<ir::BindingId>,
        loop_aliases: &mut Vec<ir::BindingId>,
    ) {
        for s in body {
            match &s.kind {
                ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => sites.push(c.binding),
                ir::Stmt::Fork { branches, .. } => sites.extend(branches.iter().map(|c| c.binding)),
                ir::Stmt::LoopControl(control) => {
                    loop_aliases.extend(control.aliases.iter().map(|c| c.binding));
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    walk(then_body, sites, loop_aliases);
                    walk(else_body, sites, loop_aliases);
                }
                _ => {}
            }
        }
    }
    let mut sites = Vec::new();
    let mut loop_aliases = Vec::new();
    walk(&m.process.body, &mut sites, &mut loop_aliases);
    let mut ranks = vec![usize::MAX; m.process.bindings.len() + m.contests().len()];
    for (rank, site) in sites.iter().enumerate() {
        if site.0 >= m.process.bindings.len() || ranks[site.0] != usize::MAX {
            return Err("invalid policy call order".into());
        }
        ranks[site.0] = rank;
    }
    if m.start_policy
        .as_ref()
        .is_some_and(|p| p.priority.is_some())
        || m.uses_work_profiles()
    {
        // A command's public result binding is an alias, never a queued Task.
        // Finite attempts retain their own lexical sites and original CaseId.
        for (call, response) in m.response_attempts() {
            if sites.contains(&response.command) {
                return Err("invalid policy response alias".into());
            }
            let rank = *ranks
                .get(call.binding.0)
                .ok_or("invalid policy response site")?;
            let alias = ranks
                .get_mut(response.command.0)
                .ok_or("invalid policy response alias")?;
            *alias = (*alias).min(rank);
        }
    }
    // Expansion already orders each real call in each round, then on_limit.
    // A published result alias has no queued Task and must not receive a rank.
    let published: BTreeSet<_> = sites
        .iter()
        .filter_map(|&site| m.loop_alias(site))
        .collect();
    let aliases: BTreeSet<_> = loop_aliases.iter().copied().collect();
    if aliases.len() != loop_aliases.len()
        || aliases != published
        || aliases.iter().any(|a| ranks.get(a.0) != Some(&usize::MAX))
    {
        return Err("invalid repeat policy result alias".into());
    }
    if ranks[..m.process.bindings.len()]
        .iter()
        .enumerate()
        .any(|(id, rank)| *rank == usize::MAX && !aliases.contains(&ir::BindingId(id)))
    {
        return Err("invalid policy call order".into());
    }
    for i in 0..m.contests().len() {
        ranks[m.contest_site(i).0] = sites.len() + i;
    }
    Ok(ranks)
}

pub(super) fn validate(m: &ir::Model) -> EngineResult<()> {
    let Some(p) = &m.start_policy else {
        return Ok(());
    };
    if p.extended() != p.classes.is_some() {
        return Err("invalid start policy classification".into());
    }
    if !p.extended() && !p.executor_order.is_empty() {
        return Err("invalid start policy classification".into());
    }
    if let Some(c) = &p.classes {
        if p.executor_order.len() != m.actors.len() {
            return Err("invalid start policy classification".into());
        }
        for (actor, order) in m.actors.iter().zip(&p.executor_order) {
            match (&actor.executors, order) {
                (None, None) => {}
                (Some(expected), Some(actual)) => {
                    let actual_set: BTreeSet<_> = actual.iter().copied().collect();
                    if actual_set.len() != actual.len()
                        || actual.iter().any(|p| p.0 >= m.people.len())
                        || actual_set != expected.iter().copied().collect()
                    {
                        return Err("invalid start policy classification".into());
                    }
                }
                _ => return Err("invalid start policy classification".into()),
            }
        }
        let mut seen = BTreeSet::new();
        for group in [&c.policy, &c.environment, &c.party] {
            if group.windows(2).any(|w| w[0] >= w[1])
                || group
                    .iter()
                    .any(|a| a.0 >= m.actors.len() || !seen.insert(*a))
            {
                return Err("invalid start policy classification".into());
            }
        }
        if seen.len() != m.actors.len() {
            return Err("invalid start policy classification".into());
        }
        ranks(m)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "priority_classification_tests.rs"]
mod priority_classification_tests;

#[cfg(test)]
#[test]
fn repeat_fork_ranks_match_the_written_out_order_without_aliases() {
    let source = include_str!("../../tests/support/repeat_policy_fork.relca");
    let ast = crate::syntax::parse(source).unwrap();
    for (plan, names) in [
        (
            "Loop",
            [
                "@loop:wave:1:a",
                "@loop:wave:1:b",
                "@loop:wave:2:a",
                "@loop:wave:2:b",
                "@loop:wave:2:c",
                "d",
            ],
        ),
        ("Hand", ["a1", "b1", "a2", "b2", "c1", "d1"]),
    ] {
        let m = crate::lower::lower_with_selection(&ast, Some(plan), None).unwrap();
        let ranks = ranks(&m).unwrap();
        assert_eq!(
            ranks.iter().filter(|&&r| r != usize::MAX).count(),
            names.len()
        );
        for (rank, name) in names.into_iter().enumerate() {
            let id = m
                .process
                .bindings
                .iter()
                .position(|b| b.name == name)
                .unwrap();
            assert_eq!(ranks[id], rank, "{plan}: {name}");
        }
    }
}
#[cfg(test)]
#[path = "priority_tests.rs"]
mod priority_tests;
