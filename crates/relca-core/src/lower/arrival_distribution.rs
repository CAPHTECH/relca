//! Finite complete schedule distributions, never an implicit stochastic process.
use super::*;
impl Lower {
    pub(super) fn arrival_distribution(
        &mut self,
        a: &ast::ArrivalDistribution,
        count: u64,
    ) -> Option<ir::ArrivalDistribution> {
        let before = self.errors.len();
        if a.basis.as_ref().is_none_or(|b| b.value.trim().is_empty()) {
            self.error(diag::E_CON_0, "arrival_basis_missing", a.span);
        }
        if a.entries.len() > 256 || count > 10000 {
            self.unsupported("arrival_support_limit", a.span);
            return None;
        }
        let mut entries = Vec::new();
        let mut seen = BTreeSet::new();
        let mut sum = crate::probability::Rational::zero();
        for row in &a.entries {
            let times: Vec<_> = row.times.iter().filter_map(|d| self.ticks(d)).collect();
            if times.is_empty()
                || times.len() as u64 != count
                || times.windows(2).any(|w| w[0] > w[1])
                || self.horizon.is_some_and(|h| times.iter().any(|t| *t > h))
                || !seen.insert(times.clone())
            {
                self.error(diag::E_ARR_0, "invalid_arrival_schedule", row.span);
            }
            if let Some(p) = self.probability(&row.probability) {
                sum.0 += &p.0;
                entries.push((times, p));
            }
        }
        if a.entries.is_empty() {
            self.error(diag::E_ARR_0, "empty_arrival_support", a.span);
        }
        if sum != crate::probability::Rational::one() {
            self.error(
                diag::E_PROB_0,
                "arrival probabilities must sum to one",
                a.span,
            );
        }
        if self.errors.len() != before {
            return None;
        }
        Some(ir::ArrivalDistribution {
            basis: a.basis.as_ref()?.value.clone(),
            entries,
            span: a.span,
        })
    }
}
