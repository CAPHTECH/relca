//! First-hit observations live beside the execution graph, never in its key.
use super::*;
use num_traits::One;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Arrival {
    Hit(u128),
    Never,
    Censored {
        earliest: u128,
        latest: Option<u128>,
        computational: bool,
    },
}
impl Arrival {
    pub(crate) fn censor(earliest: u128, latest: Option<u128>, computational: bool) -> Self {
        if latest.is_some_and(|l| earliest > l) {
            Self::Never
        } else {
            Self::Censored {
                earliest,
                latest,
                computational,
            }
        }
    }
}
#[derive(Debug, Clone, Default)]
pub(crate) struct FirstView {
    pub hit: Option<u128>,
    pub closed: bool,
    pub censor: Option<Arrival>,
}
/// Only selected, received views are passed here. Sticky observations do not
/// consume randomness and are not attached to State/World/Frame.
pub(crate) fn record_first(slot: &mut Option<u128>, received: bool, hit: bool, elapsed: u128) {
    if received && hit && slot.is_none() {
        *slot = Some(elapsed);
    }
}

/// Observe a selected world, including received cases with no self trace. The
/// driver supplies current predicate results, never precomputed first hits.
pub(crate) fn observe_first_world(
    first: &mut [Option<u128>],
    arrivals: &[u64],
    now: u64,
    selected: bool,
    views: &[(bool, bool, Option<u64>)],
) {
    if !selected {
        return;
    }
    for ((slot, arrival), (received, hit, at)) in first.iter_mut().zip(arrivals).zip(views) {
        let at = at.unwrap_or(now);
        record_first(
            slot,
            *received && at >= *arrival,
            *hit,
            at.saturating_sub(*arrival) as u128,
        );
    }
}

pub(crate) fn quantile_row(
    m: &ir::Model,
    c: &ir::QuantileMetric,
    value: Value,
    lb: Value,
) -> Value {
    json!({"name":c.name,"p":c.p.to_string(),"property":m.quality.as_ref().unwrap().properties[c.property].name,
        "form":if c.given_reached {"reached_only"} else {"all_arrivals"},"value":value,"lower_bound":lb})
}
pub(crate) fn first_points(views: &[Vec<FirstView>]) -> Vec<u128> {
    let mut xs = BTreeSet::from([0]);
    for v in views.iter().flatten() {
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
    xs.into_iter().collect()
}
/// A, B and D are separate absorbing questions. In particular B absorbs an
/// early hit as FALSE; it cannot rediscover a later hit on the same path.
pub(crate) fn first_reduce(
    order: &[usize],
    actions: &impl Fn(usize) -> Vec<Vec<(Rational, usize)>>,
    views: &[FirstView],
    x: u128,
    question: u8,
    bits: u64,
) -> Solution {
    let mut computational = vec![false; views.len()];
    let mut table = vec![std::array::from_fn::<_, 4, _>(|_| Rational::zero()); views.len()];
    for &i in order.iter().rev() {
        let v = &views[i];
        if let Some(t) = v.hit {
            let hit = match question {
                0 => t <= x,
                1 => t > x,
                _ => true,
            };
            table[i] = std::array::from_fn(|_| {
                if hit {
                    Rational::one()
                } else {
                    Rational::zero()
                }
            });
            continue;
        }
        if v.closed {
            continue;
        }
        let mut choices = Vec::new();
        if let Some(Arrival::Censored {
            earliest,
            latest,
            computational: cut,
        }) = &v.censor
        {
            let possible = latest.is_none_or(|l| *earliest <= l)
                && match question {
                    0 => *earliest <= x,
                    1 => latest.is_none_or(|l| l > x),
                    _ => true,
                };
            computational[i] |= possible && *cut;
            choices.push(std::array::from_fn(|z| {
                if possible && z % 2 == 1 {
                    Rational::one()
                } else {
                    Rational::zero()
                }
            }));
        }
        for action in actions(i) {
            let mut b = std::array::from_fn::<_, 4, _>(|_| Rational::zero());
            let mut overflow = false;
            for (p, j) in action {
                computational[i] |= computational[j];
                overflow |= !p.within_bits(bits);
                for z in 0..4 {
                    let term = Rational(&p.0 * &table[j][z].0);
                    overflow |= !term.within_bits(bits);
                    b[z].0 += term.0;
                    overflow |= !b[z].within_bits(bits);
                }
            }
            if overflow {
                computational[i] = true;
                b = [
                    Rational::zero(),
                    Rational::one(),
                    Rational::zero(),
                    Rational::one(),
                ];
            }
            choices.push(b);
        }
        if !choices.is_empty() {
            table[i] = std::array::from_fn(|z| {
                if z < 2 {
                    choices.iter().map(|b| b[z].clone()).min().unwrap()
                } else {
                    choices.iter().map(|b| b[z].clone()).max().unwrap()
                }
            });
        }
    }
    Solution::new(table[0].clone(), computational[0])
}

pub(crate) struct Curves {
    pub xs: Vec<u128>,
    pub a: Vec<[BigRational; 4]>,
    pub b: Vec<[BigRational; 4]>,
    pub d: [BigRational; 4],
    pub exact: bool,
    pub reason: Option<&'static str>,
    pub n: u64,
}
pub(crate) fn first_curves(
    order: &[usize],
    actions: impl Fn(usize) -> Vec<Vec<(Rational, usize)>>,
    views: &[Vec<FirstView>],
    bits: u64,
) -> Curves {
    #[cfg(test)]
    let bits = TEST_BITS.with(|b| b.get().unwrap_or(bits));
    let xs = first_points(views);
    let mut out = Curves {
        xs: xs.clone(),
        a: vec![],
        b: vec![],
        d: std::array::from_fn(|_| BigRational::zero()),
        exact: true,
        reason: None,
        n: views.len() as u64,
    };
    let mut solve = |x, q| {
        let mut sum = std::array::from_fn::<_, 4, _>(|_| BigRational::zero());
        for v in views {
            let s = first_reduce(order, &actions, v, x, q, bits);
            out.exact &= s.b[0] == s.b[2] && s.b[1] == s.b[3];
            if s.reason == Some("truncated") || out.reason.is_none() {
                out.reason = s.reason;
            }
            for (dst, src) in sum.iter_mut().zip(s.b) {
                *dst += src.0;
            }
        }
        sum
    };
    let d = solve(0, 2);
    let mut a = Vec::new();
    let mut b = Vec::new();
    for x in xs {
        a.push(solve(x, 0));
        b.push(solve(x, 1));
    }
    out.d = d;
    out.a = a;
    out.b = b;
    out
}

#[cfg(test)]
thread_local! {
    pub(crate) static TEST_BITS: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
    pub(crate) static GRAPH_BUILDS: std::cell::Cell<u64> = const {std::cell::Cell::new(0)};
}

pub(crate) fn inverse_quantile(c: &ir::QuantileMetric, f: &Curves) -> (Value, Value) {
    let missing = |r, lb: Option<u128>| (absent(r), json!(lb.map(|x| x.to_string())));
    let unknown = f.d[0] != f.d[1] || f.d[2] != f.d[3] || f.reason.is_some();
    if c.given_reached {
        if f.d[3].is_zero() {
            return missing("zero_denominator", None);
        }
        if f.d[0].is_zero() {
            return missing(
                if unknown {
                    f.reason.unwrap_or("truncated")
                } else {
                    "denominator_may_be_zero"
                },
                None,
            );
        }
    } else if &f.d[3] / number(f.n) < c.p.0 {
        return missing("beyond_reached_mass", None);
    }
    // Envelopes for BOTH endpoints, not merely the outer possible range.
    let inverse = |z: usize| -> Option<u128> {
        f.xs.iter().enumerate().find_map(|(i, x)| {
            let a = &f.a[i][z];
            let den = if c.given_reached {
                a + &f.b[i][3 - z]
            } else {
                number(f.n)
            };
            (!den.is_zero() && a / den >= c.p.0).then_some(*x)
        })
    };
    let endpoints = std::array::from_fn::<_, 4, _>(inverse);
    if let (Some(l), Some(u)) = (endpoints[3], endpoints[0])
        && endpoints[3] == endpoints[2]
        && endpoints[1] == endpoints[0]
    {
        return (
            json!({"lower":l.to_string(),"upper":u.to_string(),"basis":if f.exact {"exact"} else {"per_case_relaxation"},"reason":null}),
            Value::Null,
        );
    }
    missing(
        if unknown {
            f.reason.unwrap_or("truncated")
        } else {
            "beyond_reached_mass"
        },
        endpoints[3],
    )
}

pub(crate) fn sample_quantile(
    p: &Rational,
    given: bool,
    observations: &[Arrival],
    abort: bool,
) -> (Value, Value) {
    let n = if abort { 0 } else { observations.len() as u64 };
    let censored = if abort {
        0
    } else {
        observations
            .iter()
            .filter(|v| matches!(v, Arrival::Censored { .. }))
            .count() as u64
    };
    let missing = |reason: &str, lb: Option<u128>| {
        (
            json!({"point":null,"observations":n,"censored":censored,"reason":reason}),
            json!(lb.map(|x| x.to_string())),
        )
    };
    if abort {
        return missing("aborted", None);
    }
    if n == 0 {
        return missing("no_samples", None);
    }
    let hits = observations
        .iter()
        .filter(|v| matches!(v, Arrival::Hit(_)))
        .count() as u64;
    let reason = if observations.iter().any(|v| {
        matches!(
            v,
            Arrival::Censored {
                computational: true,
                ..
            }
        )
    }) {
        "truncated"
    } else {
        "undetermined"
    };
    if given && hits == 0 {
        return missing(
            if censored == 0 {
                "zero_denominator"
            } else {
                reason
            },
            None,
        );
    }
    if !given && number(hits + censored) / number(n) < p.0 {
        return missing("beyond_reached_mass", None);
    }
    let mut xs = BTreeSet::from([0]);
    for o in observations {
        match o {
            Arrival::Hit(t) => {
                xs.insert(*t);
            }
            Arrival::Censored {
                earliest, latest, ..
            } => {
                xs.insert(*earliest);
                if let Some(t) = latest {
                    xs.insert(*t);
                }
            }
            _ => {}
        }
    }
    let mut lo = None;
    let mut hi = None;
    for x in xs {
        let h = observations
            .iter()
            .filter(|o| matches!(o,Arrival::Hit(t) if *t<=x))
            .count() as u64;
        let e = observations
            .iter()
            .filter(|o| matches!(o,Arrival::Censored{earliest,..} if *earliest<=x))
            .count() as u64;
        let late = observations
            .iter()
            .filter(|o| matches!(o,Arrival::Censored{latest,..} if latest.is_none_or(|t|t>x)))
            .count() as u64;
        let (dl, du) = if given {
            (hits + late, hits + e)
        } else {
            (n, n)
        };
        if lo.is_none() && du > 0 && number(h + e) / number(du) >= p.0 {
            lo = Some(x);
        }
        if hi.is_none() && dl > 0 && number(h) / number(dl) >= p.0 {
            hi = Some(x);
        }
    }
    if let Some(t) = lo
        && hi == Some(t)
    {
        (
            json!({"point":t.to_string(),"observations":n,"censored":censored,"reason":null}),
            Value::Null,
        )
    } else {
        missing(
            if censored > 0 {
                reason
            } else {
                "beyond_reached_mass"
            },
            lo,
        )
    }
}

fn time(v: &Value) -> Option<num_bigint::BigUint> {
    let s = v.as_str()?;
    if s.is_empty() || s.len() > 1 && s.starts_with('0') || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}
pub(super) fn quantile_wire(q: &Quality) -> Result<(), &'static str> {
    let Some(rows) = &q.quantiles else {
        if q.checked
            .iter()
            .chain(&q.unchecked)
            .any(|s| s == "quantiles")
        {
            return Err("quantile inventory without declaration");
        }
        return Ok(());
    };
    if rows.is_empty() {
        return Err("empty quantiles");
    }
    let sim = q.analysis == "simulation";
    let mut names: BTreeSet<_> = q.cohort.iter().filter_map(|v| v["name"].as_str()).collect();
    for row in rows {
        keys(
            row,
            &["name", "p", "property", "form", "value", "lower_bound"],
        )?;
        if !row["name"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && names.insert(s))
            || !row["property"]
                .as_str()
                .is_some_and(|s| q.properties.iter().any(|p| p["name"] == s))
            || !matches!(row["form"].as_str(), Some("all_arrivals" | "reached_only"))
        {
            return Err("invalid quantile definition");
        }
        let p = quantity_value(&row["p"]).ok_or("invalid quantile p")?;
        if p <= BigRational::zero() || p > BigRational::one() {
            return Err("quantile p out of range");
        }
        let v = &row["value"];
        let r = &v["reason"];
        let lb = &row["lower_bound"];
        if row["form"] == "all_arrivals"
            && matches!(
                r.as_str(),
                Some("zero_denominator" | "denominator_may_be_zero")
            )
            || sim && r == "denominator_may_be_zero"
            || !sim && r == "no_samples"
        {
            return Err("quantile reason/form/mode mismatch");
        }
        if !lb.is_null() && time(lb).is_none() {
            return Err("invalid quantile lower bound");
        }
        let finite = if sim {
            keys(v, &["point", "observations", "censored", "reason"])?;
            let n = v["observations"].as_u64().ok_or("invalid observations")?;
            let c = v["censored"].as_u64().ok_or("invalid censor count")?;
            if c > n || matches!(r.as_str(), Some("aborted" | "no_samples")) && (n != 0 || c != 0) {
                return Err("invalid quantile counts");
            }
            if n == 0 && !matches!(r.as_str(), Some("aborted" | "no_samples"))
                || c == 0 && matches!(r.as_str(), Some("truncated" | "undetermined"))
                || c > 0 && r == "zero_denominator"
                || row["form"] == "reached_only" && c == n && !v["point"].is_null()
            {
                return Err("missing quantile observation provenance");
            }
            if !v["point"].is_null() && time(&v["point"]).is_none() {
                return Err("invalid quantile point");
            }
            !v["point"].is_null()
        } else {
            keys(v, &["lower", "upper", "basis", "reason"])?;
            if v["lower"].is_null() {
                if !v["upper"].is_null() || !v["basis"].is_null() {
                    return Err("partial quantile range");
                }
                false
            } else {
                let l = time(&v["lower"]).ok_or("invalid quantile lower")?;
                let u = time(&v["upper"]).ok_or("invalid quantile upper")?;
                if l > u
                    || !matches!(v["basis"].as_str(), Some("exact" | "per_case_relaxation"))
                    || v["basis"] == "exact" && l != u
                {
                    return Err("invalid quantile range");
                }
                true
            }
        };
        if finite {
            if !r.is_null() || !lb.is_null() {
                return Err("finite quantile with reason/bound");
            }
        } else {
            if !matches!(
                r.as_str(),
                Some(
                    "zero_denominator"
                        | "denominator_may_be_zero"
                        | "truncated"
                        | "undetermined"
                        | "aborted"
                        | "no_samples"
                        | "beyond_reached_mass"
                        | "not_configured"
                )
            ) {
                return Err("invalid missing quantile reason");
            }
            if matches!(
                r.as_str(),
                Some(
                    "zero_denominator"
                        | "denominator_may_be_zero"
                        | "aborted"
                        | "no_samples"
                        | "not_configured"
                )
            ) && !lb.is_null()
            {
                return Err("spurious quantile bound");
            }
            if r == "beyond_reached_mass" && row["form"] != "all_arrivals" {
                return Err("conditional missing mass");
            }
        }
    }
    let missing = rows.iter().all(|r| {
        matches!(
            r["value"]["reason"].as_str(),
            Some("aborted" | "no_samples")
        )
    });
    let (inventory, other) = if missing {
        (&q.unchecked, &q.checked)
    } else {
        (&q.checked, &q.unchecked)
    };
    if inventory
        .iter()
        .filter(|s| s.as_str() == "quantiles")
        .count()
        != 1
        || other.iter().any(|s| s == "quantiles")
        || !missing && inventory.last().is_none_or(|s| s != "quantiles")
    {
        return Err("quantile checked/unchecked inventory");
    }
    Ok(())
}

#[cfg(test)]
#[path = "quantile_tests.rs"]
mod tests;
