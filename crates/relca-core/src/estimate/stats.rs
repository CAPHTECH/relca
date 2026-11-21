//! Exact frequencies, decimal allocation and local binomial intervals.
use super::{
    samples::Group,
    wire::{LogMetadata, Spec, Stats, Status},
};
use crate::{
    ir,
    probability::Rational,
    simulation::{self, Interval},
};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{ToPrimitive, Zero};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) fn fraction(n: impl Into<BigInt>, d: impl Into<BigInt>) -> Rational {
    Rational(BigRational::new(n.into(), d.into()))
}

pub(super) struct Calculator<'a> {
    spec: &'a Spec,
    log: &'a LogMetadata,
    alpha: Option<Rational>,
    cache: BTreeMap<(u32, u32, u32), Interval>,
}
impl<'a> Calculator<'a> {
    pub(super) fn new(spec: &'a Spec, log: &'a LogMetadata, alpha: Option<Rational>) -> Self {
        Self {
            spec,
            log,
            alpha,
            cache: BTreeMap::new(),
        }
    }

    pub(super) fn provenance(&self, event_lines: Value) -> Value {
        json!({"log_sha256":self.log.sha256,"window":self.log.window,"case_count":self.log.case_count,"event_lines":event_lines})
    }

    fn intervals(
        &mut self,
        n: u32,
        hits: impl Iterator<Item = u32>,
        unknown: u32,
    ) -> Option<Vec<Interval>> {
        let alpha = self.alpha.as_ref().filter(|_| n != 0)?;
        Some(
            hits.map(|h| {
                self.cache
                    .entry((n, h, unknown))
                    .or_insert_with(|| simulation::interval(n, h, unknown, alpha))
                    .clone()
            })
            .collect(),
        )
    }

    fn common(&self, group: &Group<'_>, unknown: u32) -> Stats {
        let n = group.samples.len() as u32;
        let censored = group.samples.iter().filter(|x| x.censored).count() as u32;
        Stats {
            n: Some(n),
            unknown: Some(unknown),
            censored: Some(censored),
            coverage: Some(if unknown != 0 || group.omitted.is_some() {
                "partial"
            } else {
                "complete"
            }),
            provenance: self.provenance(group.event_lines()),
            ..Stats::default()
        }
    }

    fn gate(&self, n: u32, known: u32, group: &Group<'_>) -> Option<&'static str> {
        if group.missing_prefix {
            group.omitted.or(Some("partial_coverage"))
        } else if n == 0 {
            Some("no_samples")
        } else if u64::from(known) < self.spec.min_samples {
            Some("insufficient_samples")
        } else {
            None
        }
    }

    fn dsl(&self, out: &mut Stats, exact: &[Rational]) -> Vec<Option<String>> {
        if let Some((values, error)) = largest_remainder(exact, self.spec.decimals) {
            out.status = Status::Derived;
            out.rounding_error_bound = Some(error);
            values.into_iter().map(Some).collect()
        } else {
            out.coverage = Some("partial");
            out.reason = Some("dsl_precision_insufficient");
            vec![None; exact.len()]
        }
    }

    pub(super) fn kernel(&mut self, group: &Group<'_>, branches: &[(&str, &Rational)]) -> Stats {
        let mut counts = BTreeMap::<_, u32>::new();
        for name in group.samples.iter().filter_map(|x| x.result) {
            *counts.entry(name).or_default() += 1;
        }
        let hits: Vec<_> = branches
            .iter()
            .map(|(name, _)| counts.get(name).copied().unwrap_or(0))
            .collect();
        let n = group.samples.len() as u32;
        let known: u32 = hits.iter().sum();
        let unknown = n - known;
        let mut out = self.common(group, unknown);
        out.counts = json!(
            branches
                .iter()
                .zip(&hits)
                .map(|((result, _), hits)| json!({"result":result,"hits":hits}))
                .collect::<Vec<_>>()
        );
        let bounds = (!group.missing_prefix)
            .then(|| self.intervals(n, hits.iter().copied(), unknown))
            .flatten();
        out.interval = json!(bounds);
        let gate = self
            .gate(n, known, group)
            .or(hits.contains(&0).then_some("zero_count_branch"));
        out.reason = group.round_fault.or(gate);
        out.sensitivity_reason = (branches.len() > 2).then_some("multi_branch_row");
        if let Some(bounds) = &bounds {
            out.declared_vs_candidate = json!({"matches":null,
                "within_interval":branches.iter().zip(bounds).all(|((_,p),b)| *p >= &b.lower && *p <= &b.upper)});
        }
        if gate.is_some() {
            return out;
        }
        let exact: Vec<_> = hits.iter().map(|&h| fraction(h, known)).collect();
        out.reason = out.reason.or(group.omitted).or_else(|| {
            (unknown != 0).then(|| {
                group
                    .samples
                    .iter()
                    .find_map(|x| x.reason)
                    .unwrap_or("result_domain")
            })
        });
        let dsl = self.dsl(&mut out, &exact);
        out.candidate =
            json!(branches.iter().zip(&exact).zip(dsl).map(|(((result,_),exact),dsl)|
            json!({"result":result,"exact":exact,"dsl":dsl})).collect::<Vec<_>>());
        out.declared_vs_candidate["matches"] =
            json!(branches.iter().zip(&exact).all(|((_, p), a)| *p == a));
        if branches.len() == 2
            && let Some(bounds) = bounds.as_ref()
        {
            let pair = |x: &Rational| json!([x, Rational(Rational::one().0 - &x.0)]);
            out.sensitivity_points = json!({"lower":pair(&bounds[0].lower),"candidate":exact,"upper":pair(&bounds[0].upper)});
        }
        out
    }

    pub(super) fn duration(&mut self, group: &Group<'_>, cap: &ir::Capability) -> Stats {
        let mut samples: Vec<_> = group.samples.iter().filter_map(|x| x.minutes).collect();
        samples.sort_unstable();
        let n = group.samples.len() as u32;
        let observed = samples.len() as u32;
        let unknown = n - observed;
        let mut out = self.common(group, unknown);
        let mut hits = BTreeMap::<u64, u32>::new();
        for &minutes in &samples {
            *hits.entry(minutes).or_default() += 1;
        }
        out.counts = json!(
            hits.iter()
                .map(|(minutes, hits)| json!({"minutes":minutes,"hits":hits}))
                .collect::<Vec<_>>()
        );
        let summary = summary(&samples, unknown != 0 || group.omitted.is_some());
        out.duration_summary = summary.as_ref().map_or(Value::Null, |s| s.0.clone());
        let bounds = if group.missing_prefix
            || hits.len() as u64 > self.spec.max_support
            || hits.contains_key(&0)
        {
            None
        } else {
            self.intervals(n, hits.values().copied(), unknown)
        };
        let declared = |minutes| {
            cap.duration_distribution.as_ref().map_or_else(
                || fraction(u32::from(cap.duration == minutes), 1),
                |d| {
                    d.entries
                        .iter()
                        .find(|(m, _)| *m == minutes)
                        .map_or_else(Rational::zero, |(_, p)| p.clone())
                },
            )
        };
        if let Some(bounds) = &bounds {
            out.interval = json!(
                hits.keys()
                    .zip(bounds)
                    .map(|(minutes, b)| json!({"minutes":minutes,"lower":b.lower,"upper":b.upper}))
                    .collect::<Vec<_>>()
            );
            if !hits.is_empty() {
                out.declared_vs_candidate = json!({"matches":null,"within_interval":hits.keys().zip(bounds).all(|(&m,b)| {
                    let p = declared(m);
                    p >= b.lower && p <= b.upper
                })});
            }
        }
        let gate = self
            .gate(n, observed, group)
            .or((out.censored.unwrap_or(0) > 0).then_some("censored"))
            .or_else(|| {
                group
                    .samples
                    .iter()
                    .find(|x| x.minutes.is_none())
                    .and_then(|x| x.reason)
            })
            .or(hits.contains_key(&0).then_some("zero_duration_observed"))
            .or((hits.len() as u64 > self.spec.max_support).then_some("support_too_large"));
        out.reason = group.round_fault.or(gate).or(group.omitted);
        if gate.is_some() || observed == 0 {
            return out;
        }
        let exact: Vec<_> = hits.values().map(|&h| fraction(h, observed)).collect();
        let dsl = self.dsl(&mut out, &exact);
        out.candidate = json!(
            hits.keys()
                .zip(&exact)
                .zip(dsl)
                .map(|((minutes, exact), dsl)| json!({"minutes":minutes,"exact":exact,"dsl":dsl}))
                .collect::<Vec<_>>()
        );
        out.declared_vs_candidate["matches"] =
            json!(hits.keys().zip(&exact).all(|(&m, p)| declared(m) == *p));
        if let Some((_, mean)) = summary {
            out.sensitivity_points = json!({"lower":samples[0].to_string(),"candidate":mean,"upper":samples.last().unwrap().to_string()});
        }
        out
    }
}

fn summary(xs: &[u64], completed_only: bool) -> Option<(Value, Rational)> {
    let min = xs.first()?.to_string();
    let max = xs.last()?.to_string();
    let n = xs.len();
    let mean = fraction(xs.iter().map(|&v| u128::from(v)).sum::<u128>(), n);
    let median = |xs: &[u64]| {
        let high = xs.len() / 2;
        let low = (xs.len() - 1) / 2;
        fraction(u128::from(xs[low]) + u128::from(xs[high]), 2u32)
    };
    let (q1, q3) = if n == 1 {
        (median(xs), median(xs))
    } else {
        (median(&xs[..n / 2]), median(&xs[n.div_ceil(2)..]))
    };
    Some((
        json!({"completed_only":completed_only,"min":min,"q1":q1,"median":median(xs),"q3":q3,"max":max,"mean":mean,
        "mean_interval":null,"mean_interval_reason":"no_statistical_bound","fixed_representative":"not_selected"}),
        mean,
    ))
}

fn largest_remainder(exact: &[Rational], decimals: u32) -> Option<(Vec<String>, Rational)> {
    let d = decimals as usize;
    let scale = BigInt::from(10u8).pow(decimals);
    let zero = BigInt::zero();
    let (mut units, remainders): (Vec<_>, Vec<_>) = exact
        .iter()
        .map(|p| {
            let scaled = &p.0 * &scale;
            let floor = scaled.to_integer();
            let remainder = scaled - &floor;
            (floor, remainder)
        })
        .unzip();
    let left = (&scale - units.iter().fold(zero, |s, x| s + x))
        .to_usize()
        .expect("mass sums to one");
    let mut indices: Vec<_> = (0..exact.len()).collect();
    indices.sort_by(|&a, &b| remainders[b].cmp(&remainders[a]).then(a.cmp(&b)));
    for i in indices.into_iter().take(left) {
        units[i] += 1;
    }
    if units
        .iter()
        .zip(exact)
        .any(|(rounded, p)| rounded.is_zero() && !p.is_zero())
    {
        return None;
    }
    let values = units
        .into_iter()
        .map(|number| {
            if d == 0 {
                number.to_string()
            } else {
                let padded = format!("{:0>width$}", number.to_string(), width = d + 1);
                let (whole, fraction) = padded.split_at(padded.len() - d);
                format!("{whole}.{fraction}")
            }
        })
        .collect();
    Some((values, fraction(1u32, scale)))
}
