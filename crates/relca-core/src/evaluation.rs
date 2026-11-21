//! ADR 0047 post-processing. No state, transition, RNG or nested report mutation.
use crate::{
    Verdict,
    compare::{Analysis, Plan, SelectedReport},
    ir,
    lower::SelectedConfiguration,
    metrics::{MetricNumber, Sample, Scalar},
    probability::Rational,
};
use num_rational::BigRational;
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, &'static str>;

#[derive(Debug, Clone, Serialize)]
pub struct Evaluation {
    pub rules: &'static str,
    pub declared: Declared,
    pub hard: Vec<HardRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<Statistics>,
    pub plans: Vec<PlanStatus>,
    pub objectives: Vec<ObjectiveRow>,
    pub pareto: Pareto,
}
#[derive(Debug, Clone, Serialize)]
pub struct Declared {
    pub plans: Vec<String>,
    pub under: Option<Vec<String>>,
    pub scenario: Option<String>,
    pub cut: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct HardRow {
    pub constraint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cohort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean: Option<String>,
    pub op: &'static str,
    pub bound: MetricNumber,
    pub values: Vec<HardCell>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Bounds {
    pub lower: MetricNumber,
    pub upper: MetricNumber,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckBounds {
    pub lower: MetricNumber,
    pub upper: Option<MetricNumber>,
}
impl From<Bounds> for CheckBounds {
    fn from(i: Bounds) -> Self {
        Self {
            lower: i.lower,
            upper: Some(i.upper),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricCheck {
    pub constraint: String,
    pub verdict: Verdict,
    #[serde(deserialize_with = "metric_check_basis")]
    pub basis: Option<String>,
    pub interval: Option<CheckBounds>,
    #[serde(deserialize_with = "metric_check_reason")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<Rational>,
}

fn closed_check_word<'de, D: serde::Deserializer<'de>>(
    d: D,
    words: &[&str],
) -> std::result::Result<Option<String>, D::Error> {
    let word = Option::<String>::deserialize(d)?;
    if word.as_deref().is_some_and(|s| !words.contains(&s)) {
        return Err(serde::de::Error::custom("unknown metric check word"));
    }
    Ok(word)
}
fn metric_check_basis<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<String>, D::Error> {
    closed_check_word(
        d,
        &[
            "exact",
            "per_case_relaxation",
            "with_undetermined",
            "statistical",
            "lower_bound",
        ],
    )
}
fn metric_check_reason<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<String>, D::Error> {
    closed_check_word(
        d,
        &[
            "aborted",
            "no_samples",
            "no_statistical_bound",
            "truncated",
            "undetermined",
            "zero_denominator",
            "denominator_may_be_zero",
            "beyond_reached_mass",
            "not_configured",
        ],
    )
}
#[derive(Debug, Clone, Serialize)]
pub struct HardCell {
    pub configuration: String,
    pub verdict: Verdict,
    pub basis: Option<String>,
    pub interval: Option<CheckBounds>,
    pub reason: Option<String>,
    pub source_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point: Option<Option<MetricNumber>>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Statistics {
    pub interval_method: &'static str,
    pub multiplicity: Multiplicity,
    pub confidence: Rational,
}
#[derive(Debug, Clone, Serialize)]
pub struct Multiplicity {
    pub method: &'static str,
    pub m: u64,
    pub alpha: Option<Rational>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PlanStatus {
    pub configuration: String,
    pub verify_verdict: Verdict,
    pub hard_verdict: Verdict,
    pub status: &'static str,
    pub reasons: Vec<PlanReason>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PlanReason {
    pub kind: &'static str,
    pub constraint: Option<String>,
    pub verdict: Verdict,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ObjectiveRow {
    pub objective: String,
    pub direction: ir::Direction,
    pub values: Vec<ObjectiveCell>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ObjectiveCell {
    Exhaustive {
        configuration: String,
        lower: Option<MetricNumber>,
        upper: Option<MetricNumber>,
        basis: Option<String>,
        reason: Option<String>,
    },
    Point {
        configuration: String,
        point: Option<MetricNumber>,
        reason: Option<String>,
    },
    Sample {
        configuration: String,
        sample: Sample,
        reason: Option<String>,
    },
}
impl ObjectiveCell {
    fn bounds(&self) -> Option<(&BigRational, &BigRational)> {
        match self {
            Self::Exhaustive {
                lower: Some(l),
                upper: Some(u),
                ..
            } => Some((&l.0, &u.0)),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Pareto {
    pub status: &'static str,
    pub reason: Option<&'static str>,
    pub among: Vec<String>,
    pub non_dominated: Vec<String>,
    pub dominated: Vec<Dominated>,
    pub undetermined_pairs: Vec<UndeterminedPair>,
    pub not_evaluated: Vec<NotEvaluated>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Dominated {
    pub configuration: String,
    pub dominated_by: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct UndeterminedPair {
    pub from: String,
    pub to: String,
    pub reason: &'static str,
}
#[derive(Debug, Clone, Serialize)]
pub struct NotEvaluated {
    pub configuration: String,
    pub status: &'static str,
}

/// Validate public IR before a projection can dereference any quality index.
pub(crate) fn validate_ir(m: &ir::Model) -> Result<()> {
    if let Some(q) = &m.quality {
        for c in &q.metrics {
            if c.numerator >= q.properties.len()
                || matches!(c.denominator, ir::Denominator::Property(i) if i >= q.properties.len())
            {
                return Err("invalid quality property index");
            }
        }
        for c in &q.quantiles {
            if c.property >= q.properties.len()
                || c.p.0.denom() <= &num_bigint::BigInt::zero()
                || !c.p.valid()
                || c.p.is_zero()
            {
                return Err("invalid quantile declaration");
            }
        }
        for c in &q.means {
            if c.property.is_some_and(|i| i >= q.properties.len())
                || c.property.is_none()
                    && (c.measure == ir::MeanMeasure::Elapsed || c.given_reached)
            {
                return Err("invalid mean declaration");
            }
        }
        for p in &q.properties {
            if matches!(&p.target, ir::QualityTarget::Ends(es) if es.iter().any(|e| e.0 >= m.ends.len()))
            {
                return Err("invalid quality end index");
            }
        }
    }
    if let Some(c) = &m.comparison {
        if m.bounds.cases == 0
            || m.bounds.cases != m.arrivals.as_ref().map_or(1, |a| a.len() as u64)
        {
            return Err("invalid comparison case inventory");
        }
        if m.simulation.as_ref().is_some_and(|s| {
            s.replications == 0
                || s.replications > 10000
                || !s.confidence.valid()
                || s.confidence.is_zero()
                || s.confidence == Rational::one()
        }) {
            return Err("invalid comparison simulation clause");
        }
        let plans: Vec<_> = c.plans.iter().map(String::as_str).collect();
        if !crate::compare::valid_plans(&plans) {
            return Err("invalid comparison plan inventory");
        }
        if c.under.as_ref().is_some_and(|ns| {
            ns.is_empty() || ns.iter().collect::<std::collections::BTreeSet<_>>().len() != ns.len()
        }) {
            return Err("invalid comparison scenario inventory");
        }
        let count = m.quality.as_ref().map_or(0, |q| q.metrics.len());
        let quantile_count = m.quality.as_ref().map_or(0, |q| q.quantiles.len());
        let mean_count = m.quality.as_ref().map_or(0, |q| q.means.len());
        for (i, h) in c.hard.iter().enumerate() {
            let h = &h.constraint;
            let target_valid = match h.target {
                ir::MetricRef::Cohort(n) => n < count && h.bound.0 <= BigRational::one(),
                ir::MetricRef::Mean(n) => n < mean_count,
            };
            if !target_valid
                || h.bound.0 < BigRational::zero()
                || h.bound.0.denom() <= &num_bigint::BigInt::zero()
                || c.hard[..i].iter().any(|a| &a.constraint == h)
            {
                return Err("invalid hard constraint");
            }
        }
        for (i, o) in c.objectives.iter().enumerate() {
            if matches!(o.target, ir::ObjectiveRef::Cohort(n) if n >= count)
                || matches!(o.target, ir::ObjectiveRef::Quantile(n) if n >= quantile_count)
                || matches!(o.target, ir::ObjectiveRef::Mean(n) if n >= mean_count)
                || c.objectives[..i]
                    .iter()
                    .any(|a| a.target == o.target && a.direction == o.direction)
            {
                return Err("invalid comparison objective");
            }
        }
    }
    Ok(())
}

pub(crate) fn interval_verdict(
    op: ir::HardComparison,
    l: &BigRational,
    u: &BigRational,
    b: &BigRational,
) -> Verdict {
    use ir::HardComparison::*;
    let (s, v) = match op {
        Le => (u <= b, l > b),
        Lt => (u < b, l >= b),
        Ge => (l >= b, u < b),
        Gt => (l > b, u <= b),
    };
    if s {
        Verdict::Satisfied
    } else if v {
        Verdict::Violated
    } else {
        Verdict::Inconclusive
    }
}
fn quantity(v: &Value) -> Result<MetricNumber> {
    serde_json::from_value(v.clone()).map_err(|_| "invalid evaluation quantity")
}
fn nullable_quantity(v: &Value) -> Result<Option<MetricNumber>> {
    if v.is_null() {
        Ok(None)
    } else {
        quantity(v).map(Some)
    }
}
fn range(v: &Value) -> Result<Option<Bounds>> {
    match (
        nullable_quantity(&v["lower"])?,
        nullable_quantity(&v["upper"])?,
    ) {
        (Some(lower), Some(upper)) if lower <= upper => Ok(Some(Bounds { lower, upper })),
        (None, None) => Ok(None),
        _ => Err("invalid evaluation interval"),
    }
}
fn quality(p: &Plan) -> Option<&crate::quality::Quality> {
    match &p.report {
        SelectedReport::Exhaustive(r) => r.quality.as_ref(),
        SelectedReport::Simulation(r) => r.quality.as_ref(),
    }
}
fn text_reason(v: &Value) -> Option<String> {
    v.as_str().map(String::from)
}

fn missing_verdict(reason: Option<&str>) -> Verdict {
    if matches!(reason, Some("aborted" | "no_samples")) {
        Verdict::Unsupported
    } else {
        Verdict::Inconclusive
    }
}

fn cohort_cell(
    c: &Value,
    m: &ir::Model,
    op: ir::HardComparison,
    bound: &BigRational,
) -> Result<HardCell> {
    let reason = text_reason(&c["value"]["reason"]);
    let deferred = m.deferred_metrics().is_some_and(|d| d.process);
    let mut interval = if deferred { None } else { range(&c["value"])? };
    let mut basis = if deferred {
        None
    } else {
        text_reason(&c["value"]["basis"])
    };
    if !deferred
        && interval.is_none()
        && matches!(reason.as_deref(), Some("truncated" | "undetermined"))
        && !c["bounds_with_undetermined"].is_null()
    {
        interval = range(&c["bounds_with_undetermined"])?;
        basis = interval.as_ref().map(|_| "with_undetermined".into());
    }
    let verdict = interval.as_ref().map_or_else(
        || missing_verdict(reason.as_deref()),
        |i| interval_verdict(op, &i.lower.0, &i.upper.0, bound),
    );
    Ok(HardCell {
        configuration: String::new(),
        verdict,
        basis: interval.as_ref().and(basis),
        interval: interval.map(CheckBounds::from),
        reason,
        source_reason: None,
        point: None,
    })
}

struct SimContext {
    alpha: Option<Rational>,
    cache: BTreeMap<(u32, u32, u32), Bounds>,
}
impl SimContext {
    fn sum(
        &mut self,
        q: &crate::quality::Quality,
        m: &ir::Model,
        property: usize,
    ) -> Result<Bounds> {
        let sim = m.simulation.as_ref().ok_or("missing simulation clause")?;
        let r = u32::try_from(sim.replications).map_err(|_| "invalid replications")?;
        let cases = q
            .properties
            .get(property)
            .ok_or("invalid simulation property index")?["cases"]
            .as_array()
            .ok_or("missing case inventory")?;
        let arrivals = m.arrivals.as_deref().unwrap_or(&[0]);
        if cases.len() as u64 != m.bounds.cases || cases.len() != arrivals.len() {
            return Err("simulation case inventory differs");
        }
        let mut l = BigRational::zero();
        let mut u = BigRational::zero();
        for (k, c) in cases.iter().enumerate() {
            if c["case_id"].as_u64() != Some(k as u64 + 1)
                || c["arrival"].as_u64() != Some(arrivals[k])
                || c["trials"].as_u64() != Some(sim.replications)
            {
                return Err("simulation case identity or trials differ");
            }
            let hits = c["hits"].as_u64().ok_or("missing hits")?;
            let unknown = c["unknown"].as_u64().ok_or("missing unknown")?;
            if hits.checked_add(unknown).ok_or("observation overflow")? > sim.replications {
                return Err("invalid observations");
            }
            let key = (r, hits as u32, unknown as u32);
            if !self.cache.contains_key(&key) {
                let i = crate::simulation::interval(
                    r,
                    key.1,
                    key.2,
                    self.alpha.as_ref().ok_or("missing alpha")?,
                );
                self.cache.insert(
                    key,
                    Bounds {
                        lower: MetricNumber(i.lower.0),
                        upper: MetricNumber(i.upper.0),
                    },
                );
            }
            let i = &self.cache[&key];
            l += &i.lower.0;
            u += &i.upper.0;
        }
        Ok(Bounds {
            lower: MetricNumber(l),
            upper: MetricNumber(u),
        })
    }
    fn cohort(
        &mut self,
        q: &crate::quality::Quality,
        m: &ir::Model,
        index: usize,
        op: ir::HardComparison,
        bound: &BigRational,
    ) -> Result<HardCell> {
        let metric = m
            .quality
            .as_ref()
            .and_then(|s| s.metrics.get(index))
            .ok_or("missing cohort spec")?;
        let c = q.cohort.get(index).ok_or("missing cohort")?;
        let source = text_reason(&c["value"]["reason"]);
        let point = Some(nullable_quantity(&c["value"]["point"])?);
        let mut cell = HardCell {
            configuration: String::new(),
            verdict: Verdict::Inconclusive,
            basis: None,
            interval: None,
            reason: source.clone(),
            source_reason: None,
            point,
        };
        if matches!(source.as_deref(), Some("aborted" | "no_samples")) {
            cell.verdict = Verdict::Unsupported;
            return Ok(cell);
        }
        // Retained hits describe the Core samples, not a configured quality
        // estimate. Do not reconstruct a statistical hard bound from them.
        if m.deferred_metrics().is_some_and(|d| d.process)
            || source.as_deref() == Some("not_configured")
        {
            cell.point = Some(None);
            return Ok(cell);
        }
        let n = self.sum(q, m, metric.numerator)?;
        let d = match metric.denominator {
            ir::Denominator::Arrivals => {
                let n = MetricNumber::from(m.bounds.cases);
                Bounds {
                    lower: n.clone(),
                    upper: n,
                }
            }
            ir::Denominator::Property(i) => self.sum(q, m, i)?,
        };
        if d.lower.0.is_zero() {
            cell.reason = Some("denominator_may_be_zero".into());
            if source != cell.reason {
                cell.source_reason = source;
            }
        } else {
            let i = Bounds {
                lower: MetricNumber(n.lower.0 / d.upper.0),
                upper: MetricNumber(n.upper.0 / d.lower.0),
            };
            cell.verdict = interval_verdict(op, &i.lower.0, &i.upper.0, bound);
            cell.basis = Some("statistical".into());
            cell.interval = Some(i.into());
        }
        Ok(cell)
    }
}

fn validated_mean_row<'a>(
    q: &'a crate::quality::Quality,
    m: &ir::Model,
    i: usize,
) -> Result<&'a Value> {
    let spec = m.quality.as_ref().ok_or("missing mean spec")?;
    let def = spec.means.get(i).ok_or("invalid mean index")?;
    let rows = q.means.as_ref().ok_or("missing means")?;
    let row = rows.get(i).ok_or("missing mean")?;
    if rows.len() != spec.means.len()
        || row["name"] != def.name
        || row["measure"] != def.measure.name()
        || row["property"].as_str() != def.property.map(|i| spec.properties[i].name.as_str())
        || row["form"]
            != if def.given_reached {
                "reached_only"
            } else {
                "all_arrivals"
            }
    {
        return Err("mean definition differs");
    }
    Ok(row)
}

fn mean_check(
    q: Option<&crate::quality::Quality>,
    m: &ir::Model,
    i: usize,
    h: &ir::MetricConstraint,
    sampled: bool,
) -> Result<MetricCheck> {
    let mut row = MetricCheck {
        constraint: String::new(),
        verdict: Verdict::Unsupported,
        basis: None,
        interval: None,
        alpha: None,
        reason: Some(
            if sampled {
                "no_statistical_bound"
            } else {
                "aborted"
            }
            .into(),
        ),
    };
    if let Some(q) = q {
        let source = validated_mean_row(q, m, i)?;
        if !sampled {
            mean_cell(&mut row, source, h)?;
        }
    }
    Ok(row)
}

fn mean_cell(check: &mut MetricCheck, row: &Value, h: &ir::MetricConstraint) -> Result<()> {
    let value = &row["value"];
    check.reason = text_reason(&value["reason"]);
    if let Some(i) = range(value)? {
        check.verdict = interval_verdict(h.op, &i.lower.0, &i.upper.0, &h.bound.0);
        check.basis = text_reason(&value["basis"]);
        check.interval = Some(CheckBounds {
            lower: i.lower,
            upper: Some(i.upper),
        });
    } else if let Some(lb) = nullable_quantity(&row["lower_bound"])? {
        use ir::HardComparison::*;
        check.verdict = match h.op {
            Le if lb > h.bound => Verdict::Violated,
            Lt if lb >= h.bound => Verdict::Violated,
            Ge if lb >= h.bound => Verdict::Satisfied,
            Gt if lb > h.bound => Verdict::Satisfied,
            _ => Verdict::Inconclusive,
        };
        check.basis = Some("lower_bound".into());
        check.interval = Some(CheckBounds {
            lower: lb,
            upper: None,
        });
    } else {
        check.verdict = missing_verdict(check.reason.as_deref());
    }
    Ok(())
}

/// Planned two-sided Bernoulli intervals, including a separate denominator
/// interval for ratios whose denominator is a case property.
pub(crate) fn simulation_rows(m: &ir::Model) -> Option<u64> {
    let mut rows = u64::try_from(m.probability_queries.len()).ok()?;
    for h in &m.metric_checks {
        if let ir::MetricRef::Cohort(i) = h.target {
            let c = m.quality.as_ref()?.metrics.get(i)?;
            rows = rows.checked_add(if c.denominator == ir::Denominator::Arrivals {
                1
            } else {
                2
            })?;
        }
    }
    rows.checked_mul(m.bounds.cases)
}

pub(crate) fn metric_checks(
    m: &ir::Model,
    quality: Option<&crate::quality::Quality>,
    sampled: bool,
    alpha: Option<&Rational>,
) -> Vec<MetricCheck> {
    let mut context = SimContext {
        alpha: alpha.cloned(),
        cache: BTreeMap::new(),
    };
    m.metric_checks
        .iter()
        .map(|h| {
            let name = m.quality.as_ref().and_then(|q| match h.target {
                ir::MetricRef::Cohort(i) => q.metrics.get(i).map(|c| format!("cohort.{}", c.name)),
                ir::MetricRef::Mean(i) => q.means.get(i).map(|c| format!("mean.{}", c.name)),
            });
            let mut check = MetricCheck {
                constraint: format!(
                    "{} {} {}",
                    name.as_deref().unwrap_or("unknown"),
                    h.op.text(),
                    h.bound.0
                ),
                verdict: Verdict::Unsupported,
                basis: None,
                interval: None,
                reason: Some("aborted".into()),
                alpha: None,
            };
            match h.target {
                ir::MetricRef::Mean(i) => {
                    if let Ok(row) = mean_check(quality, m, i, h, sampled) {
                        check = MetricCheck {
                            constraint: check.constraint,
                            ..row
                        };
                    }
                }
                ir::MetricRef::Cohort(i) => {
                    if let Some(q) = quality {
                        let cell = if sampled {
                            context.cohort(q, m, i, h.op, &h.bound.0)
                        } else {
                            q.cohort
                                .get(i)
                                .ok_or("missing cohort")
                                .and_then(|row| cohort_cell(row, m, h.op, &h.bound.0))
                        };
                        if let Ok(c) = cell {
                            if sampled
                                && (c.basis.is_some()
                                    || c.reason.as_deref() == Some("denominator_may_be_zero"))
                            {
                                check.alpha = alpha.cloned();
                            }
                            check.verdict = c.verdict;
                            check.basis = c.basis;
                            check.reason = c.reason;
                            check.interval = c.interval;
                        }
                    }
                }
            }
            check
        })
        .collect()
}

/// Only the declared value is an objective; censor lower bounds are not values.
fn objective_value(
    p: &Plan,
    m: &ir::Model,
    target: &ir::ObjectiveRef,
    analysis: Analysis,
) -> Result<ObjectiveCell> {
    let configuration = p.configuration.clone();
    let deferred = m.deferred_metrics().is_some_and(|d| d.process);
    match target {
        ir::ObjectiveRef::Cohort(i) | ir::ObjectiveRef::Quantile(i) | ir::ObjectiveRef::Mean(i) => {
            let q = quality(p).ok_or("missing objective quality")?;
            let c = match target {
                ir::ObjectiveRef::Mean(_) => validated_mean_row(q, m, *i)?,
                ir::ObjectiveRef::Quantile(_) => {
                    let spec = m.quality.as_ref().ok_or("missing objective quality spec")?;
                    let metric = spec
                        .quantiles
                        .get(*i)
                        .ok_or("invalid objective quantile index")?;
                    let rows = q.quantiles.as_ref().ok_or("missing objective quantiles")?;
                    if rows.len() != spec.quantiles.len() {
                        return Err("objective quantile inventory differs");
                    }
                    let row = rows.get(*i).ok_or("missing objective quantile")?;
                    if row["name"] != metric.name
                        || row["p"] != metric.p.to_string()
                        || row["property"] != spec.properties[metric.property].name
                        || row["form"]
                            != if metric.given_reached {
                                "reached_only"
                            } else {
                                "all_arrivals"
                            }
                    {
                        return Err("objective quantile definition differs");
                    }
                    row
                }
                _ => q.cohort.get(*i).ok_or("missing objective cohort")?,
            };
            let v = &c["value"];
            let reason = text_reason(&v["reason"]);
            if analysis == Analysis::Simulation {
                Ok(ObjectiveCell::Point {
                    configuration,
                    point: if deferred {
                        None
                    } else {
                        nullable_quantity(&v["point"])?
                    },
                    reason,
                })
            } else {
                Ok(ObjectiveCell::Exhaustive {
                    configuration,
                    lower: if deferred {
                        None
                    } else {
                        nullable_quantity(&v["lower"])?
                    },
                    upper: if deferred {
                        None
                    } else {
                        nullable_quantity(&v["upper"])?
                    },
                    basis: if deferred {
                        None
                    } else {
                        text_reason(&v["basis"])
                    },
                    reason,
                })
            }
        }
        ir::ObjectiveRef::Overall(field) => {
            let metrics = match &p.report {
                SelectedReport::Exhaustive(r) => r.metrics.as_ref(),
                SelectedReport::Simulation(r) => r.metrics.as_ref(),
            };
            let scalar = metrics.map(|m| {
                let o = &m.overall;
                match field {
                    ir::OverallObjective::Work => &o.work,
                    ir::OverallObjective::NamedPeopleWork => &o.named_people_work,
                    ir::OverallObjective::PeakTasks => &o.peak_tasks,
                    ir::OverallObjective::ObservedCostTotal => &o.observed_cost_total,
                    ir::OverallObjective::ObservedCostPerArrival => &o.observed_cost_per_arrival,
                }
            });
            if let Some(scalar) = scalar {
                // Scalar's deserializer validates even hand-constructed public reports.
                let scalar: Scalar = serde_json::from_value(
                    serde_json::to_value(scalar).map_err(|_| "invalid objective scalar")?,
                )
                .map_err(|_| "invalid objective scalar")?;
                match (scalar, analysis) {
                    (Scalar::Range(r), Analysis::Exhaustive) => Ok(ObjectiveCell::Exhaustive {
                        configuration,
                        basis: r
                            .min
                            .as_ref()
                            .filter(|_| !deferred)
                            .map(|_| "path_range".into()),
                        lower: r.min.filter(|_| !deferred),
                        upper: r.max.filter(|_| !deferred),
                        reason: r
                            .reason
                            .map(|r| serde_json::to_value(r).unwrap().as_str().unwrap().into()),
                    }),
                    (Scalar::Sample(mut sample), Analysis::Simulation) => {
                        if deferred {
                            sample.mean = None;
                            sample.min = None;
                            sample.max = None;
                            sample.samples = 0;
                        }
                        Ok(ObjectiveCell::Sample {
                            configuration,
                            reason: sample
                                .reason
                                .map(|r| serde_json::to_value(r).unwrap().as_str().unwrap().into()),
                            sample,
                        })
                    }
                    _ => Err("objective scalar analysis differs"),
                }
            } else if m.metrics.is_some() {
                Err("missing objective metrics")
            } else if analysis == Analysis::Exhaustive {
                Ok(ObjectiveCell::Exhaustive {
                    configuration,
                    lower: None,
                    upper: None,
                    basis: None,
                    reason: Some("not_configured".into()),
                })
            } else {
                Ok(ObjectiveCell::Sample {
                    configuration,
                    sample: Sample {
                        mean: None,
                        min: None,
                        max: None,
                        samples: 0,
                        reason: Some(crate::metrics::Reason::NotConfigured),
                    },
                    reason: Some("not_configured".into()),
                })
            }
        }
    }
}
pub(crate) fn objective_name(m: &ir::Model, target: &ir::ObjectiveRef) -> String {
    match target {
        ir::ObjectiveRef::Cohort(i) => {
            format!("cohort.{}", m.quality.as_ref().unwrap().metrics[*i].name)
        }
        ir::ObjectiveRef::Quantile(i) => {
            format!(
                "quantile.{}",
                m.quality.as_ref().unwrap().quantiles[*i].name
            )
        }
        ir::ObjectiveRef::Mean(i) => format!("mean.{}", m.quality.as_ref().unwrap().means[*i].name),
        ir::ObjectiveRef::Overall(f) => format!("metrics.overall.{}", f.text()),
    }
}

fn dominance(a: usize, b: usize, objectives: &[ObjectiveRow]) -> bool {
    let mut strict = false;
    for o in objectives {
        let (Some((al, au)), Some((bl, bu))) = (o.values[a].bounds(), o.values[b].bounds()) else {
            return false;
        };
        let (weak, strong) = match o.direction {
            ir::Direction::Minimize => (au <= bl, au < bl),
            ir::Direction::Maximize => (al >= bu, al > bu),
        };
        if !weak {
            return false;
        }
        strict |= strong;
    }
    strict
}
fn pareto(plans: &[PlanStatus], objectives: &[ObjectiveRow], analysis: Analysis) -> Pareto {
    let among: Vec<_> = plans
        .iter()
        .enumerate()
        .filter(|(_, p)| p.status == "admissible")
        .map(|(i, _)| i)
        .collect();
    let reason = if analysis == Analysis::Simulation {
        Some("simulation_estimates")
    } else if objectives.is_empty() {
        Some("no_objectives")
    } else if among.is_empty() {
        Some("no_admissible_plans")
    } else {
        None
    };
    let mut out = Pareto {
        status: if reason.is_some() {
            "not_computed"
        } else {
            "computed"
        },
        reason,
        among: among
            .iter()
            .map(|i| plans[*i].configuration.clone())
            .collect(),
        non_dominated: vec![],
        dominated: vec![],
        undetermined_pairs: vec![],
        not_evaluated: plans
            .iter()
            .filter(|p| p.status != "admissible")
            .map(|p| NotEvaluated {
                configuration: p.configuration.clone(),
                status: p.status,
            })
            .collect(),
    };
    if reason.is_some() {
        return out;
    }
    for &b in &among {
        let dominated_by: Vec<_> = among
            .iter()
            .filter(|a| **a != b && dominance(**a, b, objectives))
            .map(|a| plans[*a].configuration.clone())
            .collect();
        if dominated_by.is_empty() {
            out.non_dominated.push(plans[b].configuration.clone());
        } else {
            out.dominated.push(Dominated {
                configuration: plans[b].configuration.clone(),
                dominated_by,
            });
        }
    }
    for (pos, &a) in among.iter().enumerate() {
        for &b in &among[pos + 1..] {
            if dominance(a, b, objectives) || dominance(b, a, objectives) {
                continue;
            }
            let missing = objectives
                .iter()
                .any(|o| o.values[a].bounds().is_none() || o.values[b].bounds().is_none());
            let overlap = objectives.iter().any(|o| {
                let (Some((al, au)), Some((bl, bu))) = (o.values[a].bounds(), o.values[b].bounds())
                else {
                    return false;
                };
                al <= bu && bl <= au && (al != au || bl != bu)
            });
            if missing || overlap {
                out.undetermined_pairs.push(UndeterminedPair {
                    from: plans[a].configuration.clone(),
                    to: plans[b].configuration.clone(),
                    reason: if missing {
                        "missing_value"
                    } else {
                        "overlapping_intervals"
                    },
                });
            }
        }
    }
    out
}

pub(crate) fn evaluate(
    plans: &[Plan],
    metadata: &[SelectedConfiguration],
    analysis: Analysis,
) -> Result<Evaluation> {
    if plans.len() != metadata.len() || plans.is_empty() {
        return Err("evaluation inventory differs");
    }
    let m = &metadata[0].model;
    let spec = m.comparison.as_ref().ok_or("missing comparison spec")?;
    for (p, s) in plans.iter().zip(metadata) {
        validate_ir(&s.model)?;
        if s.model.comparison.as_ref() != Some(spec) {
            return Err("comparison specs differ");
        }
        crate::comparison_projection::quality_rows(
            quality(p),
            &s.model,
            analysis == Analysis::Simulation,
        )?;
        if let Some(q) = quality(p) {
            for c in &q.cohort {
                if !c["bounds_with_undetermined"].is_null() {
                    range(&c["bounds_with_undetermined"])?;
                }
            }
            if analysis == Analysis::Simulation {
                let replications = s
                    .model
                    .simulation
                    .as_ref()
                    .ok_or("missing simulation clause")?
                    .replications;
                for property in &q.properties {
                    let missing = matches!(
                        property["expected_count"]["reason"].as_str(),
                        Some("aborted" | "no_samples")
                    );
                    let expected = if missing { 0 } else { replications };
                    let deferred = s.model.deferred_metrics().is_some_and(|d| d.process);
                    if property["expected_count"]["reason"] == "not_configured" && !deferred {
                        return Err("unconfigured quality without a deferred layer");
                    }
                    if property["expected_count"]["samples"].as_u64()
                        != Some(if deferred { 0 } else { expected })
                        || property["unknown_count"]["samples"].as_u64() != Some(expected)
                        || property["cases"]
                            .as_array()
                            .ok_or("missing quality cases")?
                            .iter()
                            .any(|c| c["trials"].as_u64() != Some(expected))
                    {
                        return Err("quality replication inventory differs");
                    }
                }
            }
        }
    }
    let mut statistics = None;
    let mut context = SimContext {
        alpha: None,
        cache: BTreeMap::new(),
    };
    if analysis == Analysis::Simulation {
        let sim = m.simulation.as_ref().ok_or("missing simulation clause")?;
        if sim.replications == 0
            || sim.replications > 10000
            || !sim.confidence.valid()
            || sim.confidence.is_zero()
            || sim.confidence == Rational::one()
            || m.bounds.cases == 0
        {
            return Err("invalid simulation bounds");
        }
        let mut count = 0u64;
        for s in metadata {
            let sm = &s.model;
            let ss = sm.simulation.as_ref().ok_or("missing simulation clause")?;
            if ss.replications != sim.replications
                || ss.confidence != sim.confidence
                || sm.bounds.cases == 0
            {
                return Err("simulation clauses differ");
            }
            for h in &spec.hard {
                let ir::MetricRef::Cohort(i) = h.constraint.target else {
                    continue;
                };
                let c = if matches!(
                    sm.quality.as_ref().unwrap().metrics[i].denominator,
                    ir::Denominator::Property(_)
                ) {
                    2
                } else {
                    1
                };
                count = count
                    .checked_add(
                        sm.bounds
                            .cases
                            .checked_mul(c)
                            .ok_or("multiplicity overflow")?,
                    )
                    .ok_or("multiplicity overflow")?;
            }
        }
        let alpha = if count == 0 {
            None
        } else {
            Some(Rational(
                (BigRational::one() - &sim.confidence.0)
                    / BigRational::from_integer(num_bigint::BigInt::from(count) * 2u8),
            ))
        };
        context.alpha = alpha.clone();
        statistics = Some(Statistics {
            interval_method: "clopper-pearson-per-case-sum",
            multiplicity: Multiplicity {
                method: "bonferroni",
                m: count,
                alpha,
            },
            confidence: sim.confidence.clone(),
        });
    }
    let mut hard = Vec::new();
    for h in &spec.hard {
        let h = &h.constraint;
        let spec = m.quality.as_ref().unwrap();
        let (cohort, mean, label) = match h.target {
            ir::MetricRef::Cohort(i) => (
                Some(spec.metrics[i].name.clone()),
                None,
                format!("cohort.{}", spec.metrics[i].name),
            ),
            ir::MetricRef::Mean(i) => (
                None,
                Some(spec.means[i].name.clone()),
                format!("mean.{}", spec.means[i].name),
            ),
        };
        let values = plans
            .iter()
            .zip(metadata)
            .map(|(p, s)| {
                let sampled = analysis == Analysis::Simulation;
                let q = quality(p).ok_or("missing quality")?;
                let mut cell = match h.target {
                    ir::MetricRef::Cohort(i) if sampled => {
                        context.cohort(q, &s.model, i, h.op, &h.bound.0)?
                    }
                    ir::MetricRef::Cohort(i) => {
                        cohort_cell(&q.cohort[i], &s.model, h.op, &h.bound.0)?
                    }
                    ir::MetricRef::Mean(i) => {
                        let row = mean_check(Some(q), &s.model, i, h, sampled)?;
                        HardCell {
                            configuration: String::new(),
                            verdict: row.verdict,
                            basis: row.basis,
                            interval: row.interval,
                            reason: row.reason,
                            source_reason: None,
                            point: None,
                        }
                    }
                };
                cell.configuration.clone_from(&p.configuration);
                Ok(cell)
            })
            .collect::<Result<_>>()?;
        hard.push(HardRow {
            constraint: format!("{label} {} {}", h.op.text(), h.bound.0),
            cohort,
            mean,
            op: h.op.text(),
            bound: h.bound.clone(),
            values,
        });
    }
    let statuses: Vec<_> = plans
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let verify = p.report.verdict();
            let hv = hard
                .iter()
                .map(|h| h.values[i].verdict)
                .max_by_key(|v| crate::report::precedence(*v))
                .unwrap_or(Verdict::Satisfied);
            let status = if verify == Verdict::Violated || hv == Verdict::Violated {
                "excluded"
            } else if verify == Verdict::Satisfied && hv == Verdict::Satisfied {
                "admissible"
            } else {
                "undetermined"
            };
            let mut reasons = Vec::new();
            if verify != Verdict::Satisfied {
                reasons.push(PlanReason {
                    kind: "verify",
                    constraint: None,
                    verdict: verify,
                    reason: None,
                });
            }
            for h in &hard {
                let c = &h.values[i];
                if c.verdict != Verdict::Satisfied {
                    reasons.push(PlanReason {
                        kind: "hard",
                        constraint: Some(h.constraint.clone()),
                        verdict: c.verdict,
                        reason: c.reason.clone(),
                    });
                }
            }
            PlanStatus {
                configuration: p.configuration.clone(),
                verify_verdict: verify,
                hard_verdict: hv,
                status,
                reasons,
            }
        })
        .collect();
    let objectives = spec
        .objectives
        .iter()
        .map(|o| {
            let name = objective_name(m, &o.target);
            Ok(ObjectiveRow {
                objective: format!(
                    "{} {name}",
                    if o.direction == ir::Direction::Minimize {
                        "minimize"
                    } else {
                        "maximize"
                    }
                ),
                direction: o.direction,
                values: plans
                    .iter()
                    .zip(metadata)
                    .map(|(p, s)| objective_value(p, &s.model, &o.target, analysis))
                    .collect::<Result<_>>()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let pareto = pareto(&statuses, &objectives, analysis);
    Ok(Evaluation {
        rules: "compare-evaluation-p1-1",
        declared: Declared {
            plans: spec.plans.clone(),
            under: spec.under.clone(),
            scenario: m.scenario.as_ref().map(|s| s.name.clone()),
            cut: spec.cut.clone(),
        },
        hard,
        statistics,
        plans: statuses,
        objectives,
        pareto,
    })
}

pub(crate) fn shared_objectives(m: &ir::Model) -> Vec<(String, String)> {
    let spec = m.comparison.as_ref().unwrap();
    let mut out = Vec::new();
    for (i, a) in spec.objectives.iter().enumerate() {
        for b in &spec.objectives[i + 1..] {
            let shared = match (&a.target, &b.target) {
                (ir::ObjectiveRef::Overall(a), ir::ObjectiveRef::Overall(b)) => {
                    *a != ir::OverallObjective::PeakTasks && *b != ir::OverallObjective::PeakTasks
                }
                (ir::ObjectiveRef::Cohort(a), ir::ObjectiveRef::Cohort(b)) => {
                    let q = m.quality.as_ref().unwrap();
                    let a = &q.metrics[*a];
                    let b = &q.metrics[*b];
                    let props = |c: &ir::CohortMetric| {
                        let mut ps = vec![c.numerator];
                        if let ir::Denominator::Property(i) = c.denominator {
                            ps.push(i)
                        }
                        ps
                    };
                    props(a).iter().any(|p| props(b).contains(p))
                }
                (ir::ObjectiveRef::Quantile(a), ir::ObjectiveRef::Quantile(b)) => {
                    let q = m.quality.as_ref().unwrap();
                    q.quantiles[*a].property == q.quantiles[*b].property
                }
                (ir::ObjectiveRef::Quantile(a), ir::ObjectiveRef::Cohort(b))
                | (ir::ObjectiveRef::Cohort(b), ir::ObjectiveRef::Quantile(a)) => {
                    let q = m.quality.as_ref().unwrap();
                    let property = q.quantiles[*a].property;
                    let cohort = &q.metrics[*b];
                    cohort.numerator == property
                        || cohort.denominator == ir::Denominator::Property(property)
                }
                (ir::ObjectiveRef::Mean(a), ir::ObjectiveRef::Quantile(b))
                | (ir::ObjectiveRef::Quantile(b), ir::ObjectiveRef::Mean(a)) => {
                    let q = m.quality.as_ref().unwrap();
                    q.means[*a].property == Some(q.quantiles[*b].property)
                }
                (ir::ObjectiveRef::Mean(a), ir::ObjectiveRef::Cohort(b))
                | (ir::ObjectiveRef::Cohort(b), ir::ObjectiveRef::Mean(a)) => {
                    let q = m.quality.as_ref().unwrap();
                    q.means[*a].property.is_some_and(|p| {
                        let c = &q.metrics[*b];
                        c.numerator == p || c.denominator == ir::Denominator::Property(p)
                    })
                }
                (
                    ir::ObjectiveRef::Mean(i),
                    ir::ObjectiveRef::Overall(ir::OverallObjective::ObservedCostPerArrival),
                )
                | (
                    ir::ObjectiveRef::Overall(ir::OverallObjective::ObservedCostPerArrival),
                    ir::ObjectiveRef::Mean(i),
                ) => {
                    let mean = &m.quality.as_ref().unwrap().means[*i];
                    mean.measure == ir::MeanMeasure::Cost && mean.property.is_none()
                }
                _ => false,
            };
            if shared {
                out.push((objective_name(m, &a.target), objective_name(m, &b.target)));
            }
        }
    }
    out
}

#[cfg(test)]
mod quantile_objective_tests {
    use super::*;
    use serde_json::json;

    fn source() -> String {
        include_str!("../../../fixtures/eval-contention/model.relca")
            .replace("minimize metrics.overall.work", "minimize quantile.Q")
            .replace("  compare [", "  metric quantile.Q = quantile(0.5, case.OnTime)\n  simulation replications 1 seed 1 confidence 0.5\n  compare [")
    }

    fn inputs(analysis: Analysis) -> (Vec<Plan>, Vec<SelectedConfiguration>) {
        let src = source();
        ["One", "Two"]
            .into_iter()
            .map(|name| {
                let (report, selected) = match analysis {
                    Analysis::Exhaustive => {
                        let (r, m) = crate::report::check_selected_scenario_using(
                            &src,
                            name,
                            None,
                            crate::engine::explore,
                        );
                        (SelectedReport::Exhaustive(r), m)
                    }
                    Analysis::Simulation => {
                        let (r, m) = crate::report::simulate_selected_metadata(&src, name);
                        (SelectedReport::Simulation(r), m)
                    }
                };
                (
                    Plan {
                        configuration: name.into(),
                        report,
                    },
                    selected.unwrap(),
                )
            })
            .unzip()
    }

    #[test]
    fn quantile_objective_both_modes_transport_only_values_and_reject_forged_sources() {
        for analysis in [Analysis::Exhaustive, Analysis::Simulation] {
            let (plans, metadata) = inputs(analysis);
            let value = json!(evaluate(&plans, &metadata, analysis).unwrap());
            assert_eq!(value["objectives"][0]["objective"], "minimize quantile.Q");
            assert_eq!(
                value["objectives"][0]["values"][0],
                if analysis == Analysis::Simulation {
                    json!({"configuration":"One","point":"3","reason":null})
                } else {
                    json!({"configuration":"One","lower":"3","upper":"3","basis":"exact","reason":null})
                }
            );
            for fault in ["missing", "extra", "name", "p", "property", "form", "value"] {
                let mut bad = plans.clone();
                let q = match &mut bad[1].report {
                    SelectedReport::Exhaustive(r) => r.quality.as_mut().unwrap(),
                    SelectedReport::Simulation(r) => r.quality.as_mut().unwrap(),
                };
                let rows = q.quantiles.as_mut().unwrap();
                match fault {
                    "missing" => rows.clear(),
                    "extra" => rows.push(rows[0].clone()),
                    "name" | "p" | "property" | "form" => rows[0][fault] = json!("wrong"),
                    "value" => {
                        rows[0]["value"].as_object_mut().unwrap().remove("reason");
                    }
                    _ => unreachable!(),
                }
                assert!(
                    evaluate(&bad, &metadata, analysis).is_err(),
                    "{analysis:?}/{fault}"
                );
            }
            for fault in ["index", "property", "level", "duplicate", "no-quality"] {
                let mut bad = metadata.clone();
                for selected in &mut bad {
                    let m = &mut selected.model;
                    match fault {
                        "index" => {
                            m.comparison.as_mut().unwrap().objectives[0].target =
                                ir::ObjectiveRef::Quantile(usize::MAX)
                        }
                        "property" => {
                            m.quality.as_mut().unwrap().quantiles[0].property = usize::MAX
                        }
                        "level" => m.quality.as_mut().unwrap().quantiles[0].p = Rational::zero(),
                        "duplicate" => {
                            let os = &mut m.comparison.as_mut().unwrap().objectives;
                            os.push(os[0].clone());
                        }
                        "no-quality" => m.quality = None,
                        _ => unreachable!(),
                    }
                }
                assert!(
                    evaluate(&plans, &bad, analysis).is_err(),
                    "{analysis:?}/{fault}"
                );
            }
        }
    }

    #[test]
    fn quantile_objective_name_resolution_and_duplicates_fail_closed() {
        for (from, to, code) in [
            (
                "minimize quantile.Q",
                "minimize quantile.Unknown",
                crate::diag::E_NAME_0,
            ),
            (
                "minimize quantile.Q",
                "minimize quantile.Q; minimize quantile.Q",
                crate::diag::U_SEM_0,
            ),
        ] {
            let r = crate::report::check_configuration(&source().replace(from, to), "One");
            assert!(r.diagnostics.iter().any(|d| d.code == code), "{r:?}");
        }
    }
}
