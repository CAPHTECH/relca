//! Independent reward fold over the reference execution tree.
use super::*;
use crate::quality::{Arrival, FirstView};

impl FirstTree {
    fn mean_reward(
        &self,
        metric: usize,
        case: usize,
        given: bool,
        bits: u64,
    ) -> ([Option<Q>; 2], Option<&'static str>) {
        if let Some(view) = self.means.get(metric).and_then(|vs| vs.get(case)) {
            let v = &view.first;
            let Some(amount) = view.amount.as_ref() else {
                return ([None, None], Some("not_configured"));
            };
            if v.hit.is_some() {
                if amount.0.numer().bits() > bits || amount.0.denom().bits() > bits {
                    return ([Some(Q::zero()), None], Some("truncated"));
                }
                let x = amount.0.clone();
                return ([Some(x.clone()), Some(x)], None);
            }
            if let FirstView {
                censor: Some(Arrival::Censored { computational, .. }),
                closed: false,
                ..
            } = v
            {
                return (
                    [Some(if given { Q::zero() } else { amount.0.clone() }), None],
                    Some(if *computational {
                        "truncated"
                    } else {
                        "undetermined"
                    }),
                );
            }
            let x = given.then(Q::zero);
            return ([x.clone(), x], None);
        }
        let mut options = Vec::new();
        let mut reason = None;
        let mut sum = [Some(Q::zero()), Some(Q::zero())];
        let mut overflow = false;
        for (p, child) in &self.children {
            if p.is_zero() {
                continue;
            }
            let (value, r) = child.mean_reward(metric, case, given, bits);
            if r == Some("not_configured")
                || reason != Some("not_configured") && (r == Some("truncated") || reason.is_none())
            {
                reason = r;
            }
            if self.random {
                overflow |= !p.within_bits(bits);
                for z in 0..2 {
                    sum[z] = match (&sum[z], &value[z]) {
                        (Some(a), Some(b)) => {
                            let term = &p.0 * b;
                            let next = a + &term;
                            overflow |= term.numer().bits() > bits
                                || term.denom().bits() > bits
                                || next.numer().bits() > bits
                                || next.denom().bits() > bits;
                            Some(next)
                        }
                        _ => None,
                    };
                }
            } else {
                options.push(value);
            }
        }
        if self.random {
            return if overflow && reason != Some("not_configured") {
                ([Some(Q::zero()), None], Some("truncated"))
            } else {
                (sum, reason)
            };
        }
        let lower = options.iter().filter_map(|x| x[0].clone()).min();
        let upper = if options.iter().any(|x| x[1].is_none()) {
            None
        } else {
            options.iter().filter_map(|x| x[1].clone()).max()
        };
        ([lower, upper], reason)
    }
}

fn tree_mean(
    tree: &FirstTree,
    metric: usize,
    c: &MeanMetric,
    n: usize,
    bits: u64,
) -> (Value, Value) {
    let mut numerator = [Some(Q::zero()), Some(Q::zero())];
    let mut d = [Q::zero(), Q::zero()];
    let mut reason = None;
    for k in 0..n {
        let (xs, r) = tree.mean_reward(metric, k, c.given_reached, bits);
        if r == Some("not_configured")
            || reason != Some("not_configured") && (r == Some("truncated") || reason.is_none())
        {
            reason = r;
        }
        for z in 0..2 {
            numerator[z] = numerator[z]
                .as_ref()
                .zip(xs[z].as_ref())
                .map(|(a, b)| a + b);
        }
        if c.given_reached {
            let (mass, cut) = tree.evaluate(c.property.unwrap(), k, 0, 2, bits);
            d[0] += &mass[0];
            d[1] += &mass[3];
            if cut && reason != Some("not_configured") {
                reason = Some("truncated");
            }
        }
        if reason != Some("not_configured")
            && numerator
                .iter()
                .flatten()
                .chain(&d)
                .any(|x| x.numer().bits() > bits || x.denom().bits() > bits)
        {
            numerator = [Some(Q::zero()), None];
            reason = Some("truncated");
        }
    }
    let missing = |r, lb: Option<Q>| (empty(r), json!(lb.map(|x| x.to_string())));
    if reason == Some("not_configured") {
        return missing("not_configured", None);
    }
    if c.given_reached {
        if let Some(r) = reason {
            return missing(r, None);
        }
        if d[1].is_zero() {
            return missing("zero_denominator", None);
        }
        if d[0].is_zero() {
            return missing("denominator_may_be_zero", None);
        }
    } else {
        d = [integer(n as u64), integer(n as u64)];
        if numerator[0].is_none() {
            return missing("beyond_reached_mass", None);
        }
    }
    let low = numerator[0].as_ref().map(|x| x / &d[1]);
    if let Some(r) = reason {
        return missing(r, if c.given_reached { None } else { low });
    }
    if let (Some(l), Some(u)) = (&low, &numerator[1]) {
        let u = u / &d[0];
        return (interval(l, &u, l == &u, None), Value::Null);
    }
    missing("beyond_reached_mass", low)
}

#[cfg(test)]
#[test]
fn reference_mean_numeric_cut_withdraws_only_the_reward() {
    let c = MeanMetric {
        name: "Cost".into(),
        measure: MeanMeasure::Cost,
        property: None,
        given_reached: false,
    };
    let t = FirstTree {
        means: vec![vec![crate::quality::MeanView {
            first: FirstView {
                hit: Some(0),
                ..Default::default()
            },
            amount: Some(Rational(integer(83))),
        }]],
        ..Default::default()
    };
    assert_eq!(tree_mean(&t, 0, &c, 1, 4096).0["lower"], "83");
    let (v, lb) = tree_mean(&t, 0, &c, 1, 2);
    assert_eq!(v["reason"], "truncated");
    assert!(v["lower"].is_null());
    assert_eq!(lb, "0");
}

pub(in crate::reference) fn add_tree_means(
    out: &mut Quality,
    m: &Model,
    tree: Option<&FirstTree>,
    abort: bool,
) {
    let spec = m.quality.as_ref().unwrap();
    if !spec.means.is_empty() {
        out.means = Some(
            spec.means
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let (v, lb) = tree
                        .filter(|_| !abort && !m.actors.iter().any(|a| a.metadata.rule.is_some()))
                        .map_or_else(
                            || (empty("aborted"), Value::Null),
                            |t| tree_mean(t, i, c, m.bounds.cases as usize, 4096),
                        );
                    crate::quality::mean_row(m, c, v, lb)
                })
                .collect(),
        );
    }
}
