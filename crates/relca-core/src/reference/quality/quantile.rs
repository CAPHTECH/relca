//! Reference step-functions: independently fold the execution tree at each
//! observed breakpoint. No dependency on the engine CDF or reason reducer.
use super::*;
use crate::quality::{Arrival, FirstView};
use std::collections::BTreeSet;

#[derive(Clone, Default)]
pub(in crate::reference) struct FirstTree {
    pub views: Vec<Vec<FirstView>>,
    pub random: bool,
    pub children: Vec<(Rational, FirstTree)>,
}
pub(in crate::reference) fn remember(
    first: &mut Option<u128>,
    received: bool,
    matches: bool,
    t: u128,
) {
    if first.is_none() && received && matches {
        *first = Some(t);
    }
}
pub(in crate::reference) fn observe_first_world(
    first: &mut [Option<u128>],
    arrivals: &[u64],
    now: u64,
    selected: bool,
    views: &[(bool, bool, Option<u64>)],
) {
    if !selected {
        return;
    }
    for k in 0..first.len() {
        let (received, hit, event) = views[k];
        let time = event.unwrap_or(now);
        remember(
            &mut first[k],
            received && time >= arrivals[k],
            hit,
            time.saturating_sub(arrivals[k]) as u128,
        );
    }
}
impl FirstTree {
    fn points(&self, property: usize, xs: &mut BTreeSet<u128>) {
        if let Some(vs) = self.views.get(property) {
            for v in vs {
                if let Some(t) = v.hit {
                    xs.insert(t);
                }
                if let Some(Arrival::Censored {
                    earliest, latest, ..
                }) = &v.censor
                {
                    xs.insert(*earliest);
                    if let Some(t) = latest {
                        xs.insert(*t);
                    }
                }
            }
        }
        for (_, c) in &self.children {
            c.points(property, xs);
        }
    }
    pub(in crate::reference) fn evaluate(
        &self,
        property: usize,
        case: usize,
        x: u128,
        question: u8,
        bits: u64,
    ) -> ([Q; 4], bool) {
        let zero = || std::array::from_fn(|_| Q::zero());
        let v = self.views.get(property).and_then(|v| v.get(case));
        if let Some(v) = v {
            if let Some(t) = v.hit {
                let yes = question == 2 || question == 0 && t <= x || question == 1 && t > x;
                return (std::array::from_fn(|_| integer(u64::from(yes))), false);
            }
            if v.closed {
                return (zero(), false);
            }
        }
        let mut unknown = false;
        let mut options = Vec::new();
        if let Some(FirstView {
            censor:
                Some(Arrival::Censored {
                    earliest,
                    latest,
                    computational,
                }),
            ..
        }) = v
        {
            let allowed = latest.is_none_or(|t| *earliest <= t)
                && match question {
                    0 => *earliest <= x,
                    1 => latest.is_none_or(|t| t > x),
                    _ => true,
                };
            options.push(std::array::from_fn(|i| {
                integer(u64::from(allowed && i % 2 == 1))
            }));
            unknown |= allowed && *computational;
        }
        if self.random {
            let mut total: [Q; 4] = zero();
            let mut failed = false;
            for (p, c) in &self.children {
                let (b, cut) = c.evaluate(property, case, x, question, bits);
                unknown |= cut;
                failed |= !p.within_bits(bits);
                for i in 0..4 {
                    let product = &p.0 * &b[i];
                    failed |= !Rational(product.clone()).within_bits(bits);
                    total[i] += product;
                    failed |= !Rational(total[i].clone()).within_bits(bits);
                }
            }
            if failed {
                unknown = true;
                total = [integer(0), integer(1), integer(0), integer(1)];
            }
            if !self.children.is_empty() {
                options.push(total);
            }
        } else {
            for (_, c) in &self.children {
                let (b, cut) = c.evaluate(property, case, x, question, bits);
                unknown |= cut;
                options.push(b);
            }
        }
        if options.is_empty() {
            return (zero(), unknown);
        }
        (
            std::array::from_fn(|i| {
                if i < 2 {
                    options.iter().map(|b| b[i].clone()).min().unwrap()
                } else {
                    options.iter().map(|b| b[i].clone()).max().unwrap()
                }
            }),
            unknown,
        )
    }
}
pub(in crate::reference) fn tree_quantile(
    tree: &FirstTree,
    c: &QuantileMetric,
    n: usize,
    bits: u64,
) -> (Value, Value) {
    #[cfg(test)]
    let bits = TEST_BITS.with(|b| b.get().unwrap_or(bits));
    let mut xs = BTreeSet::from([0]);
    tree.points(c.property, &mut xs);
    let mut computational = false;
    let mut exact = true;
    let mut sum = |x, q| {
        let mut total: [Q; 4] = std::array::from_fn(|_| Q::zero());
        for k in 0..n {
            let (b, cut) = tree.evaluate(c.property, k, x, q, bits);
            computational |= cut;
            exact &= b[0] == b[2] && b[1] == b[3];
            for i in 0..4 {
                total[i] += &b[i];
            }
        }
        total
    };
    let d = sum(0, 2);
    let curves = xs
        .iter()
        .map(|x| (*x, sum(*x, 0), sum(*x, 1)))
        .collect::<Vec<_>>();
    let unknown = d[0] != d[1]
        || d[2] != d[3]
        || curves
            .iter()
            .any(|(_, a, b)| a[0] != a[1] || a[2] != a[3] || b[0] != b[1] || b[2] != b[3]);
    let reason = if computational {
        "truncated"
    } else {
        "undetermined"
    };
    let missing = |r, lb: Option<u128>| (empty(r), json!(lb.map(|t| t.to_string())));
    if c.given_reached && d[3].is_zero() {
        return missing("zero_denominator", None);
    }
    if c.given_reached && d[0].is_zero() {
        return missing(
            if unknown {
                reason
            } else {
                "denominator_may_be_zero"
            },
            None,
        );
    }
    if !c.given_reached && &d[3] / integer(n as u64) < c.p.0 {
        return missing("beyond_reached_mass", None);
    }
    let mut crossing = [None; 4];
    for (x, a, b) in &curves {
        for j in 0..4 {
            let divisor = if c.given_reached {
                &a[j] + &b[3 - j]
            } else {
                integer(n as u64)
            };
            if crossing[j].is_none() && !divisor.is_zero() && &a[j] / divisor >= c.p.0 {
                crossing[j] = Some(*x);
            }
        }
    }
    if let (Some(l), Some(u)) = (crossing[3], crossing[0])
        && crossing[2] == Some(l)
        && crossing[1] == Some(u)
    {
        return (
            json!({"lower":l.to_string(),"upper":u.to_string(),"basis":if exact {"exact"} else {"per_case_relaxation"},"reason":null}),
            Value::Null,
        );
    }
    missing(
        if unknown {
            reason
        } else {
            "beyond_reached_mass"
        },
        crossing[3],
    )
}

#[cfg(test)]
thread_local! {
    pub(in crate::reference) static TEST_BITS: std::cell::Cell<Option<u64>> = const {std::cell::Cell::new(None)};
    pub(in crate::reference) static GRAPH_BUILDS: std::cell::Cell<u64> = const {std::cell::Cell::new(0)};
}
pub(in crate::reference) fn add_tree_quantiles(
    out: &mut Quality,
    m: &Model,
    tree: Option<&FirstTree>,
    abort: bool,
) {
    let spec = m.quality.as_ref().unwrap();
    if !spec.quantiles.is_empty() {
        out.quantiles = Some(
            spec.quantiles
                .iter()
                .map(|c| {
                    let (v, lb) = match tree.filter(|_| !abort) {
                        None => (empty("aborted"), Value::Null),
                        Some(tree) => tree_quantile(tree, c, m.bounds.cases as usize, 4096),
                    };
                    crate::quality::quantile_row(m, c, v, lb)
                })
                .collect(),
        );
    }
}
pub(in crate::reference) fn sample_inverse(
    p: &Rational,
    given: bool,
    pool: &[Arrival],
    abort: bool,
) -> (Value, Value) {
    let total = if abort { 0 } else { pool.len() as u64 };
    let count = if abort {
        0
    } else {
        pool.iter()
            .filter(|o| matches!(o, Arrival::Censored { .. }))
            .count() as u64
    };
    let absent = |r, lb: Option<u128>| {
        (
            json!({"point":null,"observations":total,"censored":count,"reason":r}),
            json!(lb.map(|t| t.to_string())),
        )
    };
    if abort {
        return absent("aborted", None);
    }
    if total == 0 {
        return absent("no_samples", None);
    }
    let mut xs = BTreeSet::from([0]);
    let mut hits = 0;
    let mut computation = false;
    for o in pool {
        match o {
            Arrival::Hit(t) => {
                hits += 1;
                xs.insert(*t);
            }
            Arrival::Censored {
                earliest,
                latest,
                computational,
            } => {
                computation |= *computational;
                xs.insert(*earliest);
                if let Some(t) = latest {
                    xs.insert(*t);
                }
            }
            Arrival::Never => {}
        }
    }
    let r = if computation {
        "truncated"
    } else {
        "undetermined"
    };
    if given && hits == 0 {
        return absent(if count == 0 { "zero_denominator" } else { r }, None);
    }
    if !given && integer(hits + count) / integer(total) < p.0 {
        return absent("beyond_reached_mass", None);
    }
    let mut optimistic = None;
    let mut pessimistic = None;
    for t in xs {
        let mut early = 0;
        let mut possible = 0;
        let mut after = 0;
        for o in pool {
            match o {
                Arrival::Hit(v) if *v <= t => early += 1,
                Arrival::Censored {
                    earliest, latest, ..
                } => {
                    possible += u64::from(*earliest <= t);
                    after += u64::from(latest.is_none_or(|v| v > t));
                }
                _ => {}
            }
        }
        let high_den = if given { hits + possible } else { total };
        let low_den = if given { hits + after } else { total };
        if optimistic.is_none()
            && high_den > 0
            && integer(early + possible) / integer(high_den) >= p.0
        {
            optimistic = Some(t);
        }
        if pessimistic.is_none() && low_den > 0 && integer(early) / integer(low_den) >= p.0 {
            pessimistic = Some(t);
        }
    }
    if let Some(t) = optimistic
        && pessimistic == Some(t)
    {
        return (
            json!({"point":t.to_string(),"observations":total,"censored":count,"reason":null}),
            Value::Null,
        );
    }
    absent(
        if count > 0 { r } else { "beyond_reached_mass" },
        optimistic,
    )
}

#[cfg(test)]
#[path = "quantile_tests.rs"]
mod tests;
