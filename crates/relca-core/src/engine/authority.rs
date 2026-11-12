//! Authority-only validation and event predicate, independent of scheduling.
use super::*;

pub(super) fn validate(m: &ir::Model) -> EngineResult<()> {
    fn scope(c: &ir::Cond, m: &ir::Model) -> EngineResult<()> {
        let mut pending = vec![c];
        while let Some(c) = pending.pop() {
            match c {
                ir::Cond::Contains { set, element } => {
                    if *element != ir::Element::CurrentCase
                        || !m
                            .state_variables
                            .get(set.0)
                            .is_some_and(|v| v.case_set_size.is_some())
                    {
                        return Err("invalid membership scope".into());
                    }
                }
                ir::Cond::IntCompare { variable, .. } => {
                    if !m
                        .state_variables
                        .get(variable.0)
                        .is_some_and(|v| v.kind.is_observed() && v.integer_max.is_some())
                    {
                        return Err("invalid integer scope".into());
                    }
                }
                ir::Cond::StateEq(t) => {
                    let v = m
                        .state_variables
                        .get(t.variable.0)
                        .ok_or("unknown scope variable")?;
                    if v.case_set_size.is_some()
                        || v.kind.is_latent()
                        || !valid_state_value(v, t.value, m)
                    {
                        return Err("invalid Authority scope".into());
                    }
                }
                ir::Cond::And(ps) | ir::Cond::Or(ps) if ps.len() >= 2 => pending.extend(ps),
                _ => return Err("Authority scope must read observed case variables".into()),
            }
        }
        Ok(())
    }
    for a in &m.authorities {
        let r = &a.restrictions;
        if let Some(c) = &r.scope {
            scope(c, m)?;
        }
        if r.scope.is_some() != r.scope_span.is_some()
            || r.expires.is_some() != r.expires_span.is_some()
            || r.delegated_from.is_some() != r.delegated_span.is_some()
            || r.revocable_by.is_some() != r.revocable_span.is_some()
        {
            return Err("incomplete Authority metadata".into());
        }
        if let Some(revoker) = r.revocable_by {
            let valid = match revoker {
                ir::AuthorityRevoker::Person(p) => {
                    m.people.get(p.0).is_some_and(|p| !p.name.is_empty())
                }
                ir::AuthorityRevoker::Actor(a) => {
                    m.actors.get(a.0).is_some_and(|a| !a.name.is_empty())
                }
            };
            if !valid {
                return Err("invalid Authority revoker".into());
            }
        }
    }
    // Each node has one parent. Color traversal is linear and stack-safe.
    let mut colors = vec![0; m.authorities.len()];
    for i in 0..m.authorities.len() {
        let mut path = Vec::new();
        let mut next = Some(ir::AuthorityId(i));
        while let Some(id) = next {
            let a = m
                .authorities
                .get(id.0)
                .ok_or("Authority parent out of range")?;
            if colors[id.0] == 1 {
                return Err("Authority delegation cycle".into());
            }
            if colors[id.0] == 2 {
                break;
            }
            colors[id.0] = 1;
            path.push(id.0);
            next = a.restrictions.delegated_from;
        }
        for id in path.into_iter().rev() {
            let a = &m.authorities[id];
            let r = &a.restrictions;
            let mut expiry = r.expires;
            if let Some(parent) = r.delegated_from {
                let p = &m.authorities[parent.0];
                let has = |cap, person| {
                    p.grants
                        .iter()
                        .any(|g| g.capability == cap && g.persons.contains(&person))
                };
                if a.issued_by
                    .is_none_or(|person| a.grants.iter().any(|g| !has(g.capability, person)))
                    || a.grants
                        .iter()
                        .any(|g| g.persons.iter().any(|person| !has(g.capability, *person)))
                    || p.not_by.iter().any(|person| !a.not_by.contains(person))
                {
                    return Err("invalid Authority delegation".into());
                }
                if let (Some(c), Some(p)) = (r.expires, p.restrictions.effective_expires)
                    && c > p
                {
                    return Err("delegation widens expiry".into());
                }
                expiry = expiry.or(p.restrictions.effective_expires);
            }
            if expiry != r.effective_expires {
                return Err("invalid effective expiry".into());
            }
            colors[id] = 2;
        }
    }
    for c in &m.contracts {
        if let ir::ContractKind::Waive { authority, .. } = c.kind {
            let a = m
                .authorities
                .get(authority.0)
                .ok_or("unknown waiver Authority")?;
            if a.restrictions.restricted() {
                return Err("restricted waiver Authority".into());
            }
        }
    }
    Ok(())
}

pub(super) fn holds(
    m: &ir::Model,
    id: ir::AuthorityId,
    cap: ir::CapabilityId,
    person: ir::PersonId,
    time: ir::Tick,
    values: &[ir::StateValue],
    case_index: usize,
) -> EngineResult<bool> {
    let a = &m.authorities[id.0];
    let mut holds = a
        .grants
        .iter()
        .any(|g| g.capability == cap && g.persons.contains(&person));
    holds &= a.restrictions.effective_expires.is_none_or(|d| time <= d);
    let mut next = Some(id);
    while let Some(id) = next {
        let r = &m.authorities[id.0].restrictions;
        if let Some(c) = &r.scope {
            holds &= condition_at(c, &[], values, case_index)?;
        }
        next = r.delegated_from;
    }
    Ok(holds)
}
