//! Closed ADR 0060 classification and effective-input payloads.
use crate::{diag::Span, probability::Rational};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}
impl From<Span> for SourceSpan {
    fn from(span: Span) -> Self {
        Self {
            start: span.start,
            end: span.end,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rules {
    #[serde(rename = "uncertainty-p2-1")]
    P2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Available,
    NotConfigured,
    NotApplicable,
    Unavailable,
    References,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    NoProbabilisticModel,
    ExhaustiveAnalysis,
    NoAnalysis,
    ObservableTraceOnly,
    NotLowered,
    InputUnavailable,
    EffectiveInputsUnavailable,
    AggregatePerSource,
    NoSourceReports,
}

fn required_option<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Class {
    pub status: Status,
    #[serde(deserialize_with = "required_option")]
    pub reason: Option<Reason>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Item {
    Reference(String),
    Evidence(Evidence),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Evidence {
    InitialDistribution {
        variable: String,
        instance: Instance,
        span: SourceSpan,
        entries: Vec<Mass>,
    },
    Kernel {
        capability: String,
        name: String,
        #[serde(deserialize_with = "required_option")]
        basis: Option<String>,
        given: Vec<String>,
        span: SourceSpan,
        rows: Vec<KernelRow>,
    },
    DurationDistribution {
        capability: String,
        span: SourceSpan,
        entries: Vec<DurationMass>,
    },
    ResponseChannel {
        binding: String,
        attempt: u64,
        max_attempts: u64,
        loss_probability: Rational,
        basis: String,
        span: SourceSpan,
    },
    ArrivalDistribution {
        basis: String,
        span: SourceSpan,
        entries: Vec<ArrivalMass>,
    },
    ContestOccurrence {
        clause: String,
        probability: Rational,
        occurrence_basis: String,
        service_time_basis: String,
        span: SourceSpan,
    },
    ContestEvidenceLimit {
        clause: String,
        qualification: ContestQualification,
    },
    Assumption {
        at: String,
    },
    ModelContext {
        at: String,
    },
    StatisticalQuery {
        at: String,
    },
    SamplingMethod {
        at: String,
    },
    PointEstimate {
        at: String,
        reason: EstimateReason,
    },
    ModelAssumption {
        name: ModelAssumption,
        subjects: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Instance {
    PerCase,
    SingleCase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstimateReason {
    IntervalNotConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelAssumption {
    AnalystAssumptionsNotExhaustive,
    InitialProductIndependence,
    DurationResultConditionalIndependence,
    ResponseChannelIndependence,
    ContestOccurrenceIndependence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContestQualification {
    #[serde(rename = "analyst-declared-not-validated")]
    AnalystDeclaredNotValidated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mass {
    pub value: String,
    pub probability: Rational,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurationMass {
    pub minutes: u64,
    pub probability: Rational,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrivalMass {
    pub times: Vec<u64>,
    pub probability: Rational,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub variable: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Branch {
    pub probability: Rational,
    pub result: String,
    pub writes: Vec<Assignment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelRow {
    pub conditions: Vec<Assignment>,
    pub branches: Vec<Branch>,
}

impl Class {
    pub(crate) fn absent(status: Status, reason: Reason) -> Self {
        Self {
            status,
            reason: Some(reason),
            items: Vec::new(),
        }
    }
    pub(crate) fn available(items: Vec<Evidence>) -> Self {
        Self {
            status: Status::Available,
            reason: None,
            items: items.into_iter().map(Item::Evidence).collect(),
        }
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        let valid = match (self.status, self.reason) {
            (Status::Available, None) => {
                !self.items.is_empty() && self.items.iter().all(|i| matches!(i, Item::Evidence(_)))
            }
            (Status::References, Some(Reason::AggregatePerSource)) => {
                !self.items.is_empty() && self.items.iter().all(|i| matches!(i, Item::Reference(_)))
            }
            (Status::NotConfigured, Some(Reason::NoProbabilisticModel))
            | (
                Status::NotApplicable,
                Some(Reason::ExhaustiveAnalysis | Reason::NoAnalysis | Reason::ObservableTraceOnly),
            )
            | (
                Status::Unavailable,
                Some(
                    Reason::NoAnalysis
                    | Reason::NotLowered
                    | Reason::InputUnavailable
                    | Reason::EffectiveInputsUnavailable
                    | Reason::NoSourceReports,
                ),
            ) => self.items.is_empty(),
            _ => false,
        };
        if !valid {
            return Err("context_mismatch: uncertainty status/reason/items".into());
        }
        for item in &self.items {
            if let Item::Evidence(e) = item {
                e.validate()?;
            }
        }
        Ok(())
    }
}

fn distribution<'a>(probabilities: impl Iterator<Item = &'a Rational>) -> bool {
    let mut total = Rational::zero();
    let mut count = 0;
    for p in probabilities {
        if !p.valid() {
            return false;
        }
        total.0 += &p.0;
        count += 1;
    }
    count > 0 && total == Rational::one()
}

impl Evidence {
    pub(crate) fn at(&self) -> Option<&str> {
        match self {
            Self::Assumption { at }
            | Self::ModelContext { at }
            | Self::StatisticalQuery { at }
            | Self::SamplingMethod { at }
            | Self::PointEstimate { at, .. } => Some(at),
            _ => None,
        }
    }
    fn validate(&self) -> Result<(), String> {
        let valid = match self {
            Self::InitialDistribution {
                variable,
                span,
                entries,
                ..
            } => {
                !variable.is_empty()
                    && span.start <= span.end
                    && entries.iter().all(|m| !m.value.is_empty())
                    && distribution(entries.iter().map(|m| &m.probability))
            }
            Self::Kernel {
                capability,
                name,
                span,
                rows,
                given,
                ..
            } => {
                !capability.is_empty()
                    && !name.is_empty()
                    && span.start <= span.end
                    && !rows.is_empty()
                    && rows.iter().all(|r| {
                        r.conditions.len() == given.len()
                            && r.conditions
                                .iter()
                                .zip(given)
                                .all(|(a, g)| a.variable == *g)
                            && distribution(r.branches.iter().map(|b| &b.probability))
                    })
            }
            Self::DurationDistribution {
                capability,
                span,
                entries,
            } => {
                !capability.is_empty()
                    && span.start <= span.end
                    && distribution(entries.iter().map(|m| &m.probability))
            }
            Self::ResponseChannel {
                binding,
                attempt,
                max_attempts,
                loss_probability,
                basis,
                span,
            } => {
                !binding.is_empty()
                    && *attempt > 0
                    && attempt <= max_attempts
                    && loss_probability.valid()
                    && !basis.trim().is_empty()
                    && span.start <= span.end
            }
            Self::ArrivalDistribution {
                basis,
                span,
                entries,
            } => {
                !basis.trim().is_empty()
                    && span.start <= span.end
                    && distribution(entries.iter().map(|m| &m.probability))
                    && entries
                        .iter()
                        .all(|e| !e.times.is_empty() && e.times.windows(2).all(|p| p[0] <= p[1]))
            }
            Self::ContestOccurrence {
                clause,
                probability,
                occurrence_basis,
                service_time_basis,
                span,
            } => {
                !clause.is_empty()
                    && probability.valid()
                    && !probability.is_zero()
                    && !occurrence_basis.trim().is_empty()
                    && !service_time_basis.trim().is_empty()
                    && span.start <= span.end
            }
            Self::ContestEvidenceLimit { clause, .. } => !clause.is_empty(),
            Self::Assumption { at } => at.starts_with("/assumptions/"),
            Self::ModelContext { at } => {
                at == "/ledger/execution/limits"
                    || at.starts_with('/')
                        && !at.starts_with("/uncertainty")
                        && !at.starts_with("/ledger")
            }
            Self::StatisticalQuery { at } => at.starts_with("/properties/"),
            Self::SamplingMethod { at } => at == "/simulation",
            Self::PointEstimate { at, .. } => {
                at.starts_with("/metrics")
                    || at.starts_with("/quality")
                    || at.starts_with("/terminals/")
                    || at.starts_with("/case_outcomes/")
                    || at.starts_with("/properties/")
                    || at == "/admission"
            }
            Self::ModelAssumption { .. } => true,
        };
        if valid {
            Ok(())
        } else {
            Err("context_mismatch: uncertainty evidence".into())
        }
    }
}
