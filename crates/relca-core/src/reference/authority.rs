//! Independent static validation and per-observation Authority interpretation.
use super::*;

pub(super) fn validate(m: &Model) -> Result<(), StopReason> {
    let check = |ok| {
        if ok {
            Ok(())
        } else {
            Err(StopReason::SemanticAbort)
        }
    };
    for a in &m.authorities {
        let r = &a.restrictions;
        check(
            r.scope.is_some() == r.scope_span.is_some()
                && r.expires.is_some() == r.expires_span.is_some()
                && r.delegated_from.is_some() == r.delegated_span.is_some()
                && r.revocable_by.is_some() == r.revocable_span.is_some(),
        )?;
        if let Some(c) = &r.scope {
            let mut atoms = vec![c];
            while let Some(atom) = atoms.pop() {
                match atom {
                    Cond::Contains { set, element } => require(
                        *element == Element::CurrentCase
                            && m.state_variables
                                .get(set.0)
                                .is_some_and(|v| v.case_set_size.is_some()),
                    )?,
                    Cond::IntCompare { variable, .. } => require(
                        m.state_variables
                            .get(variable.0)
                            .is_some_and(|v| v.integer_max.is_some() && v.kind.is_observed()),
                    )?,
                    Cond::StateEq(t) => {
                        let v = m
                            .state_variables
                            .get(t.variable.0)
                            .ok_or(StopReason::SemanticAbort)?;
                        check(
                            v.case_set_size.is_none()
                                && v.kind.is_observed()
                                && reference_value_valid(m, v, t.value),
                        )?;
                    }
                    Cond::And(ps) | Cond::Or(ps) => {
                        check(ps.len() >= 2)?;
                        atoms.extend(ps);
                    }
                    Cond::Eq(_) => return Err(StopReason::SemanticAbort),
                }
            }
        }
        match r.revocable_by {
            Some(AuthorityRevoker::Person(p)) => {
                check(m.people.get(p.0).is_some_and(|p| !p.name.is_empty()))?
            }
            Some(AuthorityRevoker::Actor(a)) => {
                check(m.actors.get(a.0).is_some_and(|a| !a.name.is_empty()))?
            }
            None => (),
        }
    }
    let mut done = BTreeSet::new();
    for root in 0..m.authorities.len() {
        let mut chain = Vec::new();
        let mut visiting = BTreeSet::new();
        let mut cursor = Some(AuthorityId(root));
        while let Some(id) = cursor {
            if done.contains(&id) {
                break;
            }
            check(visiting.insert(id))?;
            let a = m.authorities.get(id.0).ok_or(StopReason::SemanticAbort)?;
            chain.push(id);
            cursor = a.restrictions.delegated_from;
        }
        for id in chain.into_iter().rev() {
            let a = &m.authorities[id.0];
            let r = &a.restrictions;
            let inherited = if let Some(parent) = r.delegated_from {
                let p = &m.authorities[parent.0];
                let issuer = a.issued_by.ok_or(StopReason::SemanticAbort)?;
                for grant in &a.grants {
                    let people: BTreeSet<_> = p
                        .grants
                        .iter()
                        .filter(|g| g.capability == grant.capability)
                        .flat_map(|g| g.persons.iter().copied())
                        .collect();
                    check(
                        people.contains(&issuer)
                            && grant.persons.iter().all(|person| people.contains(person)),
                    )?;
                }
                check(p.not_by.iter().all(|person| a.not_by.contains(person)))?;
                if let (Some(child), Some(parent)) = (r.expires, p.restrictions.effective_expires) {
                    check(child <= parent)?;
                }
                p.restrictions.effective_expires
            } else {
                None
            };
            check(r.effective_expires == r.expires.or(inherited))?;
            done.insert(id);
        }
    }
    for clause in &m.contracts {
        if let ContractKind::Waive { authority, .. } = clause.kind {
            let a = m
                .authorities
                .get(authority.0)
                .ok_or(StopReason::SemanticAbort)?;
            check(!a.restrictions.restricted())?;
        }
    }
    Ok(())
}

pub(super) fn event_holds(
    m: &Model,
    authority: AuthorityId,
    capability: CapabilityId,
    person: PersonId,
    o: &Observation,
    case_index: usize,
) -> Result<bool, StopReason> {
    let grant = &m.authorities[authority.0];
    let mut valid = grant
        .grants
        .iter()
        .any(|g| g.capability == capability && g.persons.contains(&person));
    if let Some(deadline) = grant.restrictions.effective_expires {
        valid &= o.at <= deadline;
    }
    let mut cursor = Some(authority);
    while let Some(id) = cursor {
        let record = &m.authorities[id.0].restrictions;
        if let Some(c) = &record.scope {
            valid &= snapshot_condition_at(c, &o.values, &o.state_values, case_index)?;
        }
        cursor = record.delegated_from;
    }
    Ok(valid)
}

#[cfg(test)]
mod tests {
    #[test]
    fn authority_replay_rejects_deadline_executor_time_snapshot_and_ancestor_tampering() {
        let source = include_str!(
            "../../../../fixtures/auth-inherited-scope.broken-parent-scope/model.relca"
        );
        for src in [
            source.to_owned(),
            source.replace("  bounds", "  arrivals fixed [0min]\n  bounds"),
        ] {
            let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
            let r = crate::explain::explain(&src);
            let c = r
                .explanation
                .counterexamples
                .iter()
                .find(|c| c.id.ends_with("authority_basis"))
                .unwrap();
            assert_eq!(c.status, crate::explain::ReplayStatus::Verified);
            let replay = |steps: &[crate::explain::Stage]| {
                super::super::replay::replay(&m, steps, &c.id, c.case_id)
            };
            assert!(replay(&c.steps).is_ok());
            for fault in 0..6 {
                let mut bad = c.steps.clone();
                let last = bad.last_mut().unwrap();
                match fault {
                    0 => last.violation.as_mut().unwrap().deadline = Some(4),
                    1 => last.violation.as_mut().unwrap().deadline = Some(6),
                    2 => last.executor = Some("r".into()),
                    3 => last.time += 1,
                    4 => {
                        last.values
                            .observed
                            .iter_mut()
                            .find(|v| v.name == "approved")
                            .unwrap()
                            .value = "false".into()
                    }
                    5 => {
                        last.violation
                            .as_mut()
                            .unwrap()
                            .property_location
                            .as_mut()
                            .unwrap()
                            .span
                            .start += 1
                    }
                    _ => unreachable!(),
                }
                assert!(replay(&bad).is_err(), "fault {fault}");
            }
            assert!(
                super::super::replay::validate_authority_locations(
                    &src,
                    &m,
                    &c.id,
                    &c.related_locations
                )
                .is_ok()
            );
            let parent_scope = m
                .authorities
                .iter()
                .find(|a| a.name == "Parent")
                .unwrap()
                .restrictions
                .scope_span
                .unwrap();
            let mut bad = c.related_locations.clone();
            bad.iter_mut()
                .find(|l| l.span == parent_scope)
                .unwrap()
                .span
                .start += 1;
            assert!(
                super::super::replay::validate_authority_locations(&src, &m, &c.id, &bad).is_err()
            );
            let mut missing = c.related_locations.clone();
            missing.retain(|l| l.span != parent_scope);
            assert!(
                super::super::replay::validate_authority_locations(&src, &m, &c.id, &missing)
                    .is_err()
            );
        }
    }
}
