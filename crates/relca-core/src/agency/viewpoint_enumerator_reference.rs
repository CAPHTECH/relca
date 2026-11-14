//! Frozen pre-optimization F/L enumeration. This module is test-only and must
//! not dispatch through the production factorization or domain enumerator.
use super::*;

pub(super) fn assignments(
    vars: &[ir::BindingId],
    fixed: &Values,
    calls: &[&ir::Call],
    m: &ir::Model,
    b: &mut Budget,
    mut visit: impl FnMut(&Values, &mut Budget) -> Result<bool>,
) -> Result<bool> {
    b.copy(
        vars.len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(fixed.len()))
            .ok_or_else(invalid)?,
    )?;
    let vars: Vec<_> = vars
        .iter()
        .copied()
        .filter(|v| !fixed.contains_key(v))
        .collect();
    let mut indices = vec![0usize; vars.len()];
    let mut values = fixed.clone();
    let cap = |id: ir::BindingId| -> Result<&ir::Capability> {
        m.capabilities
            .get(calls.get(id.0).ok_or_else(invalid)?.capability.0)
            .ok_or_else(invalid)
    };
    for id in &vars {
        b.work()?;
        values.insert(*id, *cap(*id)?.results.first().ok_or_else(invalid)?);
    }
    loop {
        b.work()?;
        if visit(&values, b)? {
            return Ok(true);
        }
        let mut i = 0;
        loop {
            if i == vars.len() {
                return Ok(false);
            }
            b.work()?;
            let id = vars[i];
            let c = cap(id)?;
            let other = c
                .requires
                .as_ref()
                .map(|r| r.otherwise)
                .filter(|v| !c.results.contains(v));
            b.steps(c.results.len())?;
            indices[i] = indices[i].checked_add(1).ok_or_else(invalid)?;
            if let Some(v) = c
                .results
                .get(indices[i])
                .copied()
                .or_else(|| (indices[i] == c.results.len()).then_some(other).flatten())
            {
                values.insert(id, v);
                break;
            }
            indices[i] = 0;
            values.insert(id, *c.results.first().ok_or_else(invalid)?);
            i += 1;
        }
    }
}

pub(super) fn possibly_satisfied(
    p: &Path,
    calls: &[&ir::Call],
    m: &ir::Model,
    b: &mut Budget,
) -> Result<bool> {
    b.copy(p.guards.len())?;
    guard_storage(p, b)?;
    let relaxed = Path {
        guards: p
            .guards
            .iter()
            .filter(|g| !unknown_guard(g))
            .cloned()
            .collect(),
        ..Path::default()
    };
    let local = path_support(&relaxed, m, b)?;
    b.copy(local.len())?;
    assignments(
        &local.into_iter().collect::<Vec<_>>(),
        &Values::new(),
        calls,
        m,
        b,
        |v, b| satisfied(&relaxed, v, b),
    )
}
