//! ADR 0063 descriptive replication table. No party acceptance is inferred.
use super::{BusinessMetrics, Reason, Sample, Scalar, required_nullable};
use crate::probability::Rational;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
mod collect;
pub(crate) use collect::summarize;

pub const RULES: &str = "contest-rates-sample-p2-1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "TableWire")]
pub struct Table {
    pub rules: String,
    pub basis: String,
    pub unit: String,
    pub scope: String,
    pub rows: Vec<Row>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableWire {
    rules: String,
    basis: String,
    unit: String,
    scope: String,
    rows: Vec<Row>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fraction {
    #[serde(deserialize_with = "required_nullable")]
    pub value: Option<Rational>,
    #[serde(deserialize_with = "required_nullable")]
    pub numerator: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub denominator: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub reason: Option<Reason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    pub case_id: u64,
    pub party: String,
    pub target: String,
    pub route: String,
    pub configured_probability: Rational,
    pub occurrence_basis: String,
    pub service_time_basis: String,
    pub independence: String,
    pub cohort: String,
    pub trials: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub target_observed: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub target_not_reached: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub target_unknown: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub filed: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub not_filed: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub choice_unknown: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub responded: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub late_responded: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub unresolved: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub route_failed: Option<u64>,
    pub filing_fraction: Fraction,
    pub service_time: Sample,
    pub queue_wait: Sample,
    pub response_elapsed: Sample,
}

impl TryFrom<TableWire> for Table {
    type Error = String;
    fn try_from(w: TableWire) -> Result<Self, Self::Error> {
        let table = Self {
            rules: w.rules,
            basis: w.basis,
            unit: w.unit,
            scope: w.scope,
            rows: w.rows,
        };
        table.validate()?;
        Ok(table)
    }
}

fn ensure(ok: bool) -> Result<(), String> {
    ok.then_some(())
        .ok_or_else(|| "invalid contest sample table".into())
}
fn sum(xs: &[Option<u64>]) -> Option<u64> {
    xs.iter().try_fold(0u64, |n, x| n.checked_add((*x)?))
}
fn fraction(n: u64, d: u64) -> Rational {
    Rational(num_rational::BigRational::new(n.into(), d.into()))
}
impl Table {
    pub(crate) fn apply_deferred(&mut self) {
        for row in &mut self.rows {
            if row.filing_fraction.reason.is_none()
                || row.filing_fraction.reason == Some(Reason::NotConfigured)
            {
                row.filing_fraction.value = None;
                row.filing_fraction.reason = Some(Reason::NotConfigured);
            }
            for sample in [
                &mut row.service_time,
                &mut row.queue_wait,
                &mut row.response_elapsed,
            ] {
                if sample.reason.is_none() || sample.reason == Some(Reason::NotConfigured) {
                    sample.mean = None;
                    sample.min = None;
                    sample.max = None;
                    sample.samples = 0;
                    sample.reason = Some(Reason::NotConfigured);
                }
            }
        }
    }
    fn validate(&self) -> Result<(), String> {
        ensure(
            self.rules == RULES
                && self.basis == "sample"
                && self.unit == "replication"
                && self.scope == "declared-model-not-party-representation"
                && !self.rows.is_empty(),
        )?;
        let mut seen = BTreeSet::new();
        for row in &self.rows {
            ensure(seen.insert((&row.id, row.case_id)))?;
            row.validate()?;
        }
        ensure(
            self.rows
                .windows(2)
                .all(|p| (&p[0].id, p[0].case_id) < (&p[1].id, p[1].case_id)),
        )
    }
    pub(super) fn validate_metrics(&self, metrics: &BusinessMetrics) -> Result<(), String> {
        self.validate()?;
        ensure(metrics.statistical_unit.as_deref() == Some("replication"))?;
        for row in &self.rows {
            metrics
                .cases
                .iter()
                .find(|c| c.case_id == row.case_id)
                .ok_or("missing contest case")?;
            ensure(
                metrics
                    .capabilities
                    .iter()
                    .any(|c| c.capability == row.route),
            )?;
            ensure(
                (metrics.status == super::Status::Aborted)
                    == (row.filing_fraction.reason == Some(Reason::Aborted)),
            )?;
        }
        // Per-case task rows intentionally contain only normal calls. Where
        // totals are known, normal responses cannot exceed starts left over
        // after those ordinary calls. This is a consistency witness, not an
        // authentication of the absent DSL clause-to-route mapping.
        fn total(s: &Scalar) -> Option<num_rational::BigRational> {
            match s {
                Scalar::Sample(s) => s
                    .mean
                    .as_ref()
                    .map(|x| &x.0 * num_bigint::BigInt::from(s.samples)),
                Scalar::Range(_) => None,
            }
        }
        for cap in &metrics.capabilities {
            let ordinary = metrics
                .cases
                .iter()
                .flat_map(|c| &c.tasks)
                .filter(|t| t.capability == cap.capability)
                .try_fold(num_rational::BigRational::from_integer(0.into()), |n, t| {
                    Some(n + total(&t.starts)?)
                });
            let responses = self
                .rows
                .iter()
                .filter(|r| r.route == cap.capability)
                .try_fold(0u64, |n, r| {
                    n.checked_add(sum(&[r.responded, r.late_responded])?)
                });
            if let (Some(starts), Some(ordinary), Some(responses)) =
                (total(&cap.starts), ordinary, responses)
            {
                ensure(
                    starts - ordinary >= num_rational::BigRational::from_integer(responses.into()),
                )?;
            }
        }
        Ok(())
    }
}
impl Row {
    fn validate(&self) -> Result<(), String> {
        ensure(
            !self.id.is_empty()
                && self.case_id > 0
                && self.trials > 0
                && !self.party.is_empty()
                && !self.target.is_empty()
                && !self.route.is_empty()
                && self.configured_probability.valid()
                && !self.configured_probability.is_zero()
                && !self.occurrence_basis.trim().is_empty()
                && !self.service_time_basis.trim().is_empty()
                && self.independence == "per_case_per_clause"
                && self.cohort == "normal_response_observed",
        )?;
        let f = &self.filing_fraction;
        let counts = [
            self.target_observed,
            self.target_not_reached,
            self.target_unknown,
            self.filed,
            self.not_filed,
            self.choice_unknown,
            self.responded,
            self.late_responded,
            self.unresolved,
            self.route_failed,
        ];
        let times = [&self.service_time, &self.queue_wait, &self.response_elapsed];
        if f.reason == Some(Reason::Aborted) {
            return ensure(
                counts.iter().all(Option::is_none)
                    && f.value.is_none()
                    && f.numerator.is_none()
                    && f.denominator.is_none()
                    && times
                        .iter()
                        .all(|s| s.reason == Some(Reason::Aborted) && s.samples == 0),
            );
        }
        ensure(counts.iter().all(Option::is_some))?;
        ensure(
            sum(&counts[..3]) == Some(self.trials)
                && sum(&counts[3..6]) == self.target_observed
                && sum(&counts[6..]) == self.filed,
        )?;
        ensure(f.numerator == self.filed && f.denominator == self.target_observed)?;
        let d = self.target_observed.unwrap();
        let filing_unknown = self.target_unknown != Some(0) || self.choice_unknown != Some(0);
        let response_unknown = filing_unknown || self.unresolved != Some(0);
        match f.reason {
            None => ensure(
                d > 0
                    && self.target_unknown == Some(0)
                    && self.choice_unknown == Some(0)
                    && f.value.as_ref() == Some(&fraction(self.filed.unwrap(), d)),
            )?,
            Some(Reason::NoSamples) => {
                ensure(d == 0 && self.target_unknown == Some(0) && f.value.is_none())?
            }
            Some(Reason::Truncated | Reason::Unfinished) => {
                ensure(filing_unknown && f.value.is_none())?
            }
            Some(Reason::NotConfigured) => ensure(!filing_unknown && d > 0 && f.value.is_none())?,
            _ => return ensure(false),
        }
        ensure(
            times
                .windows(2)
                .all(|p| p[0].samples == p[1].samples && p[0].reason == p[1].reason),
        )?;
        match self.service_time.reason {
            None => {
                ensure(!response_unknown)?;
                ensure(
                    Some(self.service_time.samples) == sum(&[self.responded, self.late_responded]),
                )?;
                let (s, q, e) = (&self.service_time, &self.queue_wait, &self.response_elapsed);
                ensure(
                    s.mean
                        .as_ref()
                        .zip(q.mean.as_ref())
                        .zip(e.mean.as_ref())
                        .is_some_and(|((s, q), e)| &s.0 + &q.0 == e.0),
                )?;
            }
            Some(Reason::NoSamples) => {
                ensure(!response_unknown && sum(&[self.responded, self.late_responded]) == Some(0))?
            }
            Some(Reason::Truncated | Reason::Unfinished) => ensure(response_unknown)?,
            Some(Reason::NotConfigured) => ensure(
                !response_unknown
                    && sum(&[self.responded, self.late_responded]).is_some_and(|n| n > 0),
            )?,
            _ => return ensure(false),
        }
        Ok(())
    }
}
