//! ADR 0016 exact local binomial calculations. No floating point or libm.
//! The runtime probability-denominator bound does not apply to this module.
use crate::{Verdict, ir, probability::Rational};
use num_rational::BigRational;
use num_traits::Zero;
use serde::Serialize;
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Interval {
    pub lower: Rational,
    pub upper: Rational,
}

/// Compare the exact inclusive binomial tail with alpha. Use symmetry and a
/// complement to sum the shorter side; intermediate integers need no reduction.
pub(crate) fn tail_order(n: u32, x: u32, p: &Rational, alpha: &Rational, upper: bool) -> Ordering {
    assert!(x <= n && p.valid() && alpha.valid());
    let mut a = p.0.numer().to_biguint().unwrap();
    let d = p.0.denom().to_biguint().unwrap();
    let mut b = &d - &a;
    let mut x = if upper { n - x } else { x };
    if upper {
        std::mem::swap(&mut a, &mut b);
    }
    if a.is_zero() {
        return Rational::one().cmp(alpha);
    }
    if b.is_zero() {
        return if x == n {
            Rational::one()
        } else {
            Rational::zero()
        }
        .cmp(alpha);
    }
    if x == n {
        return Rational::one().cmp(alpha);
    }
    let complement = x > n / 2;
    if complement {
        x = n - x - 1;
        std::mem::swap(&mut a, &mut b);
    }
    let denominator = d.pow(n);
    let ad = alpha.0.denom().to_biguint().unwrap();
    let an = alpha.0.numer().to_biguint().unwrap();
    let target = if complement {
        (&ad - &an) * &denominator
    } else {
        an * &denominator
    };
    let mut term = b.pow(n);
    let mut sum = term.clone();
    for j in 0..x {
        // A positive partial sum already greater than the target is a sound
        // exact certificate. Equality cannot stop before the last term.
        if &sum * &ad > target {
            return if complement {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        term *= n - j;
        term *= &a;
        let divisor = &b * (j + 1);
        let next = &term / &divisor;
        assert_eq!(&next * &divisor, term, "nonintegral binomial recurrence");
        term = next;
        sum += &term;
    }
    let order = (sum * ad).cmp(&target);
    if complement { order.reverse() } else { order }
}

pub(crate) fn interval(n: u32, hits: u32, unknown: u32, alpha: &Rational) -> Interval {
    assert!(hits <= n && unknown <= n - hits);
    let endpoint = |x: u32, lower: bool| {
        if lower && x == 0 {
            return Rational::zero();
        }
        if !lower && x == n {
            return Rational::one();
        }
        let (mut lo, mut hi) = (0u32, 1u32 << 20);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            let p = Rational(BigRational::new(mid.into(), (1u32 << 20).into()));
            let order = tail_order(n, x, &p, alpha, lower);
            if order == Ordering::Equal {
                return p;
            }
            if (order == Ordering::Less) == lower {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Rational(BigRational::new(
            (if lower { lo } else { hi }).into(),
            (1u32 << 20).into(),
        ))
    };
    Interval {
        lower: endpoint(hits, true),
        upper: endpoint(hits + unknown, false),
    }
}

pub(crate) fn statistical_verdict(
    n: u32,
    hits: u32,
    unknown: u32,
    q: &ir::ProbabilityQuery,
    alpha: &Rational,
) -> Verdict {
    assert!(hits <= n && unknown <= n - hits);
    let x = hits + unknown;
    let (satisfied, violated) = match q.comparison {
        ir::Comparison::Le => (
            if x == n {
                q.bound == Rational::one()
            } else {
                tail_order(n, x, &q.bound, alpha, false) != Ordering::Greater
            },
            hits > 0 && tail_order(n, hits, &q.bound, alpha, true) == Ordering::Less,
        ),
        ir::Comparison::Ge => (
            if hits == 0 {
                q.bound.is_zero()
            } else {
                tail_order(n, hits, &q.bound, alpha, true) != Ordering::Greater
            },
            x < n && tail_order(n, x, &q.bound, alpha, false) == Ordering::Less,
        ),
    };
    if satisfied {
        Verdict::Satisfied
    } else if violated {
        Verdict::Violated
    } else {
        Verdict::Inconclusive
    }
}

#[cfg(test)]
mod simulation_tests {
    use super::*;
    fn p(s: &str) -> Rational {
        Rational::decimal(s).unwrap()
    }

    #[test]
    fn s_exact_tail_and_display() {
        for (x, f, g) in [(0, 1, 16), (1, 5, 15), (2, 11, 11), (3, 15, 5), (4, 16, 1)] {
            for (upper, numerator) in [(false, f), (true, g)] {
                let alpha = Rational(BigRational::new(numerator.into(), 16.into()));
                assert_eq!(tail_order(4, x, &p("0.5"), &alpha, upper), Ordering::Equal);
            }
        }
        let a = Rational(BigRational::new(1.into(), 16.into()));
        let value = interval(4, 2, 0, &a);
        assert_eq!(value.lower.to_string(), "28863/262144");
        assert_eq!(value.upper.to_string(), "233281/262144");
        let value = interval(4, 4, 0, &a);
        assert_eq!(value.lower.to_string(), "1/2");
        assert_eq!(value.upper.to_string(), "1");
    }

    #[test]
    #[ignore = "release performance gate, run explicitly with --release --ignored --nocapture"]
    fn s_statistics_release_performance() {
        let central = format!("0.5{}1", "0".repeat(61));
        for probability in [
            "0.001",
            "0.123457",
            "0.123456789012345678901234567890123456789012345678901234567890123",
            central.as_str(),
        ] {
            for (hits, unknown) in [(0, 0), (5000, 0), (10000, 0), (4500, 1000)] {
                for rows in [1u32, 3] {
                    let alpha = Rational(BigRational::new(1.into(), (40 * rows).into()));
                    let mut cache = std::collections::BTreeMap::new();
                    let start = std::time::Instant::now();
                    let _ = tail_order(10000, hits, &p(probability), &alpha, true);
                    let _ = tail_order(10000, hits + unknown, &p(probability), &alpha, false);
                    cache.insert((hits, unknown), interval(10000, hits, unknown, &alpha));
                    std::hint::black_box(&cache);
                    let seconds = start.elapsed().as_secs_f64();
                    println!(
                        "n=10000 p={probability} hits={hits} unknown={unknown} rows={rows} cold_seconds={seconds:.6}"
                    );
                    if probability.len() <= 8 {
                        assert!(seconds <= 10.0);
                    }
                    let warm = std::time::Instant::now();
                    std::hint::black_box(tail_order(10000, hits, &p(probability), &alpha, true));
                    std::hint::black_box(tail_order(
                        10000,
                        hits + unknown,
                        &p(probability),
                        &alpha,
                        false,
                    ));
                    std::hint::black_box(cache.get(&(hits, unknown)).unwrap());
                    let seconds = warm.elapsed().as_secs_f64();
                    println!(
                        "n=10000 p={probability} hits={hits} unknown={unknown} rows={rows} warm_seconds={seconds:.6}"
                    );
                    if probability.len() <= 8 {
                        assert!(seconds <= 10.0);
                    }
                }
            }
        }
    }
}
