//! Exact C1 relations over connected guard/binding components. No path merge.
use super::*;

pub(super) struct Factorization {
    support: BTreeSet<ir::BindingId>,
    solutions: Vec<Vec<Values>>,
}

fn conjuncts<'a>(guard: Guard<'a>, out: &mut Vec<Guard<'a>>, b: &mut Budget) -> Result<()> {
    b.work()?;
    match guard {
        // Only a true conjunction can be split. In particular Or, false And,
        // and join-any remain one relation even when they mention many bindings.
        Guard::If(ir::Cond::And(parts), true) => {
            for c in parts {
                conjuncts(Guard::If(c, true), out, b)?;
            }
        }
        Guard::Symbolic(Symbolic::And(parts), true) => {
            for c in parts {
                conjuncts(Guard::Symbolic(c, true), out, b)?;
            }
        }
        g => {
            b.cells(1)?;
            out.push(g);
        }
    }
    Ok(())
}

fn root(parents: &mut [usize], mut i: usize, b: &mut Budget) -> Result<usize> {
    let start = i;
    while parents[i] != i {
        b.work()?;
        i = parents[i];
    }
    let root = i;
    i = start;
    while parents[i] != i {
        let next = parents[i];
        parents[i] = root;
        i = next;
    }
    Ok(root)
}

impl Factorization {
    /// With one reachable path the projected contexts are a disjoint product.
    /// Count the other components, rather than materializing that product. Only
    /// the judgement's own component needs a projected context relation.
    pub fn counts(
        &self,
        j: ir::BindingId,
        domain: &[ir::ValueId],
        b: &mut Budget,
    ) -> Result<(usize, usize)> {
        let selected = self
            .solutions
            .iter()
            .position(|rows| rows.first().is_some_and(|r| r.contains_key(&j)));
        let (stopping, bypasses): (usize, usize) = if let Some(index) = selected {
            let rows = &self.solutions[index];
            let mut contexts = BTreeSet::new();
            for row in rows {
                b.copy(row.len())?;
                let context: Values = row
                    .iter()
                    .filter(|(id, _)| **id != j)
                    .map(|(id, v)| (*id, *v))
                    .collect();
                if !contexts.contains(&context) {
                    b.context()?;
                    contexts.insert(context);
                }
            }
            b.location.phase = "refusal stopping".into();
            let mut stopping = 0;
            let mut bypasses = 0;
            for context in contexts {
                let mut may = false;
                let mut cannot = false;
                for value in domain {
                    let mut reaches = false;
                    for row in rows {
                        b.steps(row.len().saturating_add(1))?;
                        if row.get(&j) == Some(value)
                            && context.iter().all(|(id, v)| row.get(id) == Some(v))
                        {
                            reaches = true;
                            break;
                        }
                    }
                    may |= reaches;
                    cannot |= !reaches;
                }
                if may {
                    if cannot {
                        stopping += 1;
                    } else {
                        bypasses += 1;
                    }
                }
            }
            (stopping, bypasses)
        } else {
            (0, 1)
        };
        // The caller needs only zero/nonzero for stopping, but the exact bypass
        // count. Do not overflow a discarded stopping-context cardinality.
        let mut count = bypasses;
        if count != 0 {
            for (i, rows) in self.solutions.iter().enumerate() {
                if Some(i) != selected {
                    b.work()?;
                    count = count.checked_mul(rows.len()).ok_or_else(|| {
                        b.location
                            .unrepresentable("count_representation", usize::MAX)
                    })?;
                }
            }
        }
        Ok((usize::from(stopping != 0), count))
    }

    pub fn build(
        p: &Path<'_>,
        local: &BTreeSet<ir::BindingId>,
        calls: &[&ir::Call],
        m: &ir::Model,
        b: &mut Budget,
    ) -> Result<Option<Self>> {
        if p.guards.is_empty() {
            return Ok(None);
        }
        viewpoints::guard_storage(p, b)?;
        let mut atoms = Vec::new();
        for g in &p.guards {
            conjuncts(g.clone(), &mut atoms, b)?;
        }
        if atoms.len() < 2 {
            return Ok(None);
        }
        // Discover the connected components before assembling guard vectors.
        // Repeatedly copying an already merged prefix would be quadratic even
        // when the final graph has only one component and needs no factorization.
        b.copy(atoms.len().saturating_mul(2))?;
        let mut parents: Vec<_> = (0..atoms.len()).collect();
        let mut owners = BTreeMap::new();
        let mut scopes = Vec::new();
        for (i, atom) in atoms.iter().enumerate() {
            b.cells(1)?;
            let component = Path {
                guards: vec![atom.clone()],
                ..Path::default()
            };
            let support = path_support(&component, m, b)?;
            for id in &support {
                b.work()?;
                if let Some(previous) = owners.get(id).copied() {
                    let a = root(&mut parents, i, b)?;
                    let other = root(&mut parents, previous, b)?;
                    if a != other {
                        parents[a.max(other)] = a.min(other);
                    }
                } else {
                    b.cells(1)?;
                    owners.insert(*id, i);
                }
            }
            scopes.push(support);
        }
        let mut components: BTreeMap<usize, (Path<'_>, BTreeSet<ir::BindingId>)> = BTreeMap::new();
        for (i, (atom, support)) in atoms.into_iter().zip(scopes).enumerate() {
            let key = root(&mut parents, i, b)?;
            if !components.contains_key(&key) {
                b.cells(1)?;
            }
            let (path, ids) = components.entry(key).or_default();
            b.cells(1)?;
            path.guards.push(atom);
            for id in support {
                b.insert(ids, id)?;
            }
        }
        // Keep the original accounting and enumerator for an inseparable Or.
        // C1 is not a new algorithm for a single, exponential component.
        if components.len() < 2 {
            return Ok(None);
        }
        b.copy(components.len())?;
        let mut solutions = Vec::new();
        for (component, support) in components.into_values() {
            b.copy(support.len())?;
            let vars: Vec<_> = support.into_iter().collect();
            let mut rows = BTreeSet::new();
            assignments(&vars, &Values::new(), calls, m, b, |values, b| {
                if satisfied(&component, values, b)? && !rows.contains(values) {
                    b.copy(values.len().saturating_add(1))?;
                    rows.insert(values.clone());
                }
                Ok(false)
            })?;
            b.copy(rows.len())?;
            solutions.push(rows.into_iter().collect());
        }
        b.copy(local.len())?;
        Ok(Some(Self {
            support: local.clone(),
            solutions,
        }))
    }

    pub fn exists(&self, fixed: &Values, b: &mut Budget) -> Result<bool> {
        for component in &self.solutions {
            let mut found = false;
            for row in component {
                b.steps(row.len().saturating_add(1))?;
                if row
                    .iter()
                    .all(|(id, v)| fixed.get(id).is_none_or(|x| x == v))
                {
                    found = true;
                    break;
                }
            }
            if !found {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Visit the exact Cartesian product of *satisfying* local tuples. Extra
    /// context bindings from other paths stay unconstrained, not default-filled.
    pub fn enumerate(
        &self,
        vars: &[ir::BindingId],
        calls: &[&ir::Call],
        m: &ir::Model,
        b: &mut Budget,
        mut visit: impl FnMut(&Values, &mut Budget) -> Result<bool>,
    ) -> Result<bool> {
        if self.solutions.iter().any(Vec::is_empty) {
            return Ok(false);
        }
        b.copy(vars.len())?;
        let extra: Vec<_> = vars
            .iter()
            .filter(|id| !self.support.contains(id))
            .copied()
            .collect();
        b.copy(self.solutions.len())?;
        let mut indices = vec![0; self.solutions.len()];
        loop {
            b.copy(self.support.len())?;
            let mut values = Values::new();
            for (component, index) in self.solutions.iter().zip(&indices) {
                values.extend(component[*index].iter().map(|(id, v)| (*id, *v)));
            }
            if assignments(&extra, &values, calls, m, b, &mut visit)? {
                return Ok(true);
            }
            let mut i = 0;
            loop {
                b.work()?;
                if i == indices.len() {
                    return Ok(false);
                }
                indices[i] += 1;
                if indices[i] < self.solutions[i].len() {
                    break;
                }
                indices[i] = 0;
                i += 1;
            }
        }
    }
}
