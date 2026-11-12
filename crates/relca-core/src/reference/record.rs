//! Reference-side record validation, deliberately not using lower's codec or
//! engine's guard. Enumerate the Cartesian product in declaration order.
use super::*;

pub(super) fn validate(m: &Model) -> Execution {
    for t in &m.types {
        let Some(record) = &t.record else {
            continue;
        };
        require(!record.fields.is_empty())?;
        let mut seen = BTreeSet::new();
        let mut count = 1usize;
        for f in &record.fields {
            let d = m.types.get(f.ty.0).ok_or(StopReason::SemanticAbort)?;
            require(
                !f.name.is_empty()
                    && seen.insert(&f.name)
                    && d.record.is_none()
                    && d.name != "PersonId"
                    && d.name != "CaseId"
                    && !d.members.is_empty()
                    && d.members.iter().collect::<BTreeSet<_>>().len() == d.members.len(),
            )?;
            count = count
                .checked_mul(d.members.len())
                .ok_or(StopReason::SemanticAbort)?;
            require(count <= 1024)?;
        }
        require(
            count == t.members.len() && t.members.iter().collect::<BTreeSet<_>>().len() == count,
        )?;
        let mut tuples = vec![Vec::<String>::new()];
        for f in &record.fields {
            let mut expanded = Vec::new();
            for prefix in tuples {
                for value in &m.types[f.ty.0].members {
                    let mut next = prefix.clone();
                    next.push(format!("{} = {}", f.name, value));
                    expanded.push(next);
                }
            }
            tuples = expanded;
        }
        for (parts, member) in tuples.iter().zip(&t.members) {
            require(*member == format!("{} {{ {} }}", t.name, parts.join(", ")))?;
        }
    }
    for v in &m.state_variables {
        if m.types.get(v.base.0).is_some_and(|t| t.record.is_some()) {
            require(
                v.kind == StateKind::ObservedCase
                    && v.integer_max.is_none()
                    && matches!(v.initial, InitialValue::Fixed(_)),
            )?;
        }
    }
    // ADR 0035 atomic: no record result and no record state write.
    for c in &m.capabilities {
        let Some(a) = &c.atomic else {
            continue;
        };
        require(
            m.types
                .get(c.return_type.0)
                .is_none_or(|t| t.record.is_none()),
        )?;
        for w in &a.writes {
            let v = m
                .state_variables
                .get(w.variable.0)
                .ok_or(StopReason::SemanticAbort)?;
            require(m.types.get(v.base.0).is_none_or(|t| t.record.is_none()))?;
        }
    }
    for k in m
        .kernels
        .iter()
        .chain(m.capabilities.iter().flat_map(|c| c.result_kernels()))
    {
        for id in &k.given {
            let v = m
                .state_variables
                .get(id.0)
                .ok_or(StopReason::SemanticAbort)?;
            require(m.types.get(v.base.0).is_none_or(|t| t.record.is_none()))?;
        }
    }
    Ok(())
}
