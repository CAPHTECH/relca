//! Exact C1 relations over connected guard/binding components. No path merge.
use super::*;

pub(super) struct Factorization {
    support: BTreeSet<ir::BindingId>,
    solutions: Vec<Vec<Values>>,
    domain: Domain,
}

#[derive(Clone, Copy)]
enum Domain {
    Main,
    Viewpoint,
}
impl Domain {
    fn assignments(
        self,
        vars: &[ir::BindingId],
        fixed: &Values,
        calls: &[&ir::Call],
        m: &ir::Model,
        b: &mut Budget,
        visit: impl FnMut(&Values, &mut Budget) -> Result<bool>,
    ) -> Result<bool> {
        match self {
            Self::Main => assignments(vars, fixed, calls, m, b, visit),
            Self::Viewpoint => viewpoints::assignments(vars, fixed, calls, m, b, visit),
        }
    }
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
        Self::build_domain(p, local, calls, m, b, Domain::Main)
    }

    pub fn build_viewpoint(
        p: &Path<'_>,
        local: &BTreeSet<ir::BindingId>,
        calls: &[&ir::Call],
        m: &ir::Model,
        b: &mut Budget,
    ) -> Result<Option<Self>> {
        Self::build_domain(p, local, calls, m, b, Domain::Viewpoint)
    }

    fn build_domain(
        p: &Path<'_>,
        local: &BTreeSet<ir::BindingId>,
        calls: &[&ir::Call],
        m: &ir::Model,
        b: &mut Budget,
        domain: Domain,
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
            domain.assignments(&vars, &Values::new(), calls, m, b, |values, b| {
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
            domain,
        }))
    }

    pub fn exists(&self, fixed: &Values, b: &mut Budget) -> Result<bool> {
        for component in &self.solutions {
            // Viewpoint queries normally fix every variable in a component.
            // Scanning all satisfying rows here would square an inseparable
            // relation's size, regressing the old fixed-context enumerator.
            // Rows have identical keys and BTreeSet order from build_domain.
            // Keep the main derivation's traversal/accounting exactly intact.
            if matches!(self.domain, Domain::Viewpoint)
                && component.len() > 1
                && fixed.len() >= component[0].len()
            {
                b.steps(component[0].len())?;
                if component[0].keys().all(|id| fixed.contains_key(id)) {
                    let (mut low, mut high) = (0, component.len());
                    let mut found = false;
                    while low < high {
                        let mid = low + (high - low) / 2;
                        let mut order = std::cmp::Ordering::Equal;
                        for (id, value) in &component[mid] {
                            b.work()?;
                            order = value.cmp(fixed.get(id).ok_or_else(invalid)?);
                            if !order.is_eq() {
                                break;
                            }
                        }
                        match order {
                            std::cmp::Ordering::Less => low = mid + 1,
                            std::cmp::Ordering::Greater => high = mid,
                            std::cmp::Ordering::Equal => {
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        return Ok(false);
                    }
                    continue;
                }
            }
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
            if self
                .domain
                .assignments(&extra, &values, calls, m, b, &mut visit)?
            {
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

#[cfg(test)]
mod viewpoint_domain_tests {
    use super::*;

    #[test]
    fn factored_and_unfactored_viewpoint_domains_exclude_version_rejection() {
        let source = r#"model Domain version "0.2" {
            type V = enum { pass, fail }
            actor A { capacity 1; state marker: V = pass;
                capability start() -> V { duration fixed 1min; result nondet { pass } }
                capability judge() -> V { duration fixed 1min; result nondet { pass };
                    version_check [marker] else fail; writes marker; finish { marker = fail } }
            }
            process Flow { s = A.start(); j = A.judge(); end done }
            bounds { cases 1; live_tasks 2; clock 10min }
            verify safety no_resource_overcommit
        }"#;
        let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        let mut b = viewpoints::budget(DEFAULT);
        let mut indexed = vec![None; 2];
        collect_shared(&m.process.body, &mut indexed, &m, 0, &mut b, false).unwrap();
        let calls = indexed.into_iter().collect::<Option<Vec<_>>>().unwrap();
        let reject = m.capabilities[calls[1].capability.0]
            .version_check
            .as_ref()
            .unwrap()
            .reject;
        let local = BTreeSet::from([ir::BindingId(0), ir::BindingId(1)]);
        let p = Path {
            guards: vec![
                Guard::Domain(
                    ir::BindingId(0),
                    m.capabilities[calls[0].capability.0].results.clone(),
                ),
                Guard::Symbolic(
                    Symbolic::Eq(ir::ResultMatch {
                        binding: ir::BindingId(1),
                        value: reject,
                    }),
                    true,
                ),
            ],
            ..Path::default()
        };
        let main = Factorization::build(&p, &local, &calls, &m, &mut b)
            .unwrap()
            .unwrap();
        assert!(main.exists(&Values::new(), &mut b).unwrap());
        let viewpoint = Factorization::build_viewpoint(&p, &local, &calls, &m, &mut b)
            .unwrap()
            .unwrap();
        assert!(!viewpoint.exists(&Values::new(), &mut b).unwrap());
    }
}
