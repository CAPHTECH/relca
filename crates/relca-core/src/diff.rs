//! ADR 0008 configuration comparison. Static call sets and intent claims are
//! separate from the dynamic report projections and their analysis verdicts.

use std::collections::BTreeSet;

use crate::{Verdict, ast, diag, lower, report};
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "../tests/support/configuration_diff_population.rs"]
pub(crate) mod extension_population;
#[path = "diff/intent.rs"]
mod intent;
#[path = "diff/static_projection.rs"]
mod static_projection;
mod work_profiles;
pub use intent::{
    Annotation, DutyRef, EffectWitness, Link, Preservation, PreservationSummary, PreservedProperty,
};
pub use static_projection::{
    AuthorityValue, Basis, ContestValue, DutyValue, GrantValue, ResourceValue, Subject,
};
pub(crate) use work_profiles::differences as q6_differences;
pub use work_profiles::{DegradationCondition, DegradationValue};

const CHECKED: [&str; 5] = [
    "comparison_mismatch",
    "actor",
    "relation",
    "intent_claims",
    "retire_loses",
];
const UNCHECKED: [&str; 9] = [
    "time_intent",
    "resource",
    "meaning",
    "authority",
    "visibility",
    "contest",
    "preserve",
    "override",
    "maintain",
];

macro_rules! declare_diff_report {
    ($( $(#[$attr:meta])* pub $field:ident: $ty:ty, )*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct DiffReport { $( $(#[$attr])* pub $field: $ty, )* }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct DiffWire { $( $(#[$attr])* $field: $ty, )* }
        impl<'de> Deserialize<'de> for DiffReport {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self,D::Error> {
                let wire = DiffWire::deserialize(d)?;
                let report = Self { $( $field: wire.$field, )* };
                report.ledger.validate(&serde_json::to_value(&report).map_err(serde::de::Error::custom)?)
                    .map_err(serde::de::Error::custom)?;
                Ok(report)
            }
        }
    };
}
declare_diff_report! {
    pub ledger: crate::ledger::Ledger,
    pub uncertainty: crate::uncertainty::Uncertainty,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preservation: Option<Preservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Vec<MetricComparison>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "crate::quality::ordered_optional_rows")]
    pub quality: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_outcomes: Option<Vec<CaseComparison>>,
    pub agency: AgencySides,
    pub verdict: Verdict,
    pub from: Side,
    pub to: Side,
    pub mismatch: Option<Mismatch>,
    pub differences: Vec<Difference>,
    pub terminals: Vec<TerminalComparison>,
    pub properties: Vec<PropertyComparison>,
    pub diagnostics: Vec<diag::Diagnostic>,
    pub checked: Vec<String>,
    pub unchecked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgencySides {
    pub from: Option<crate::agency::AgencyDerivation>,
    pub to: Option<crate::agency::AgencyDerivation>,
    pub differences: crate::agency::Differences,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricComparison {
    pub key: crate::comparison_projection::MetricKey,
    pub from: crate::comparison_projection::MetricCell,
    pub to: crate::comparison_projection::MetricCell,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Side {
    pub configuration: String,
    pub report: report::Report,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mismatch {
    pub from_cut: String,
    pub to_cut: String,
    pub differences: Vec<CutDifference>,
}

/// The field determines the payload type, keeping D1's heterogeneous wire closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "field", rename_all = "snake_case", deny_unknown_fields)]
pub enum CutDifference {
    Horizon {
        from: Vec<crate::ir::Horizon>,
        to: Vec<crate::ir::Horizon>,
    },
    Question {
        from: Option<String>,
        to: Option<String>,
    },
    Include {
        from: Vec<String>,
        to: Vec<String>,
    },
    Exclude {
        from: Vec<report::CutEntry>,
        to: Vec<report::CutEntry>,
    },
    Defer {
        from: Vec<report::CutEntry>,
        to: Vec<report::CutEntry>,
    },
    Externality {
        from: Vec<String>,
        to: Vec<String>,
    },
    Reentry {
        from: Vec<String>,
        to: Vec<String>,
    },
}

pub use ast::DiffKind as Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "DifferenceWire")]
pub struct Difference {
    pub kind: Kind,
    pub target: String,
    pub change: Change,
    pub reason: Option<String>,
    pub loses: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<DiffValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<DiffValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
}

/// Closed payload alternatives; null denotes inactive authority/resource only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DiffValue {
    Response(crate::response_inputs::Control),
    Performance(report::work_profiles::Profile),
    Degradation(DegradationValue),
    Producer(ProducerValue),
    Rule(RuleValue),
    Meaning(bool),
    Resource(ResourceValue),
    Authority(AuthorityValue),
    Time(Vec<CallMode>),
    Readers(Vec<String>),
    Qualification(Vec<String>),
    Contests(Vec<ContestValue>),
    Inactive,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducerValue {
    pub producer: String,
}

// Unlike Option, a missing legacy field is distinct from an explicit null.
#[derive(Default)]
enum ProducerField {
    #[default]
    Missing,
    Value(ProducerValue),
}
impl<'de> Deserialize<'de> for ProducerField {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        ProducerValue::deserialize(deserializer).map(Self::Value)
    }
}

fn identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn producer_name(name: &str) -> bool {
    name.split_once('.')
        .is_some_and(|(actor, capability)| identifier(actor) && identifier(capability))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleValue {
    pub version: String,
    pub values: Vec<RuleCell>,
    pub amendment: Option<crate::actor_table::Amendment>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleCell {
    pub name: String,
    pub value: crate::actor_table::RuleScalar,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallMode {
    Sequential,
    Fork,
}

// Decode by kind, not by ambiguous untagged arrays (in particular [] and null).
// Nullable is intentionally not Option at the field boundary: from/to must be
// present even when the corresponding side is inactive.
#[derive(Deserialize)]
#[serde(untagged)]
enum Nullable<T> {
    Value(T),
    Null(()),
}
impl<T> Nullable<T> {
    fn map_or<U>(self, default: U, value: impl FnOnce(T) -> U) -> U {
        match self {
            Self::Value(v) => value(v),
            Self::Null(()) => default,
        }
    }
}

macro_rules! difference_wire {
    ($($variant:ident $(($value:ty))?),* $(,)?) => {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum DifferenceWire {
            Relation {
                target: String, change: Change, reason: Option<String>, loses: Option<String>,
                #[serde(default)] annotations: Vec<Annotation>,
                #[serde(default)] from: ProducerField,
                #[serde(default)] to: ProducerField,
            },
            $($variant {
                target: String, change: Change, reason: Option<String>, loses: Option<String>,
                #[serde(default)] annotations: Vec<Annotation>,
                $(from: $value, to: $value,)?
            }),*
        }
    }
}
difference_wire! {
    Actor, Meaning(MeaningValue), Authority(Nullable<AuthorityValue>),
    Resource(Nullable<ResourceValue>), Time(Vec<CallMode>), Visibility(Vec<String>), Contest(Vec<ContestValue>),
    Rule(RuleValue),
    Qualification(Nullable<Vec<String>>), Performance(Nullable<report::work_profiles::Profile>),
    Degradation(Nullable<DegradationValue>),
}
impl TryFrom<DifferenceWire> for Difference {
    type Error = &'static str;
    fn try_from(wire: DifferenceWire) -> Result<Self, Self::Error> {
        macro_rules! row {
            ($kind:ident, $target:expr, $change:expr, $reason:expr, $loses:expr, $annotations:expr, $from:expr, $to:expr) => {
                Self {
                    kind: Kind::$kind,
                    target: $target,
                    change: $change,
                    reason: $reason,
                    loses: $loses,
                    annotations: $annotations,
                    from: $from,
                    to: $to,
                }
            };
        }
        let d = match wire {
            DifferenceWire::Qualification {
                target,
                change,
                reason,
                loses,
                annotations,
                from,
                to,
            } => row!(
                Qualification,
                target,
                change,
                reason,
                loses,
                annotations,
                Some(from.map_or(DiffValue::Inactive, DiffValue::Qualification)),
                Some(to.map_or(DiffValue::Inactive, DiffValue::Qualification))
            ),
            DifferenceWire::Performance {
                target,
                change,
                reason,
                loses,
                annotations,
                from,
                to,
            } => row!(
                Performance,
                target,
                change,
                reason,
                loses,
                annotations,
                Some(from.map_or(DiffValue::Inactive, DiffValue::Performance)),
                Some(to.map_or(DiffValue::Inactive, DiffValue::Performance))
            ),
            DifferenceWire::Degradation {
                target,
                change,
                reason,
                loses,
                annotations,
                from,
                to,
            } => row!(
                Degradation,
                target,
                change,
                reason,
                loses,
                annotations,
                Some(from.map_or(DiffValue::Inactive, DiffValue::Degradation)),
                Some(to.map_or(DiffValue::Inactive, DiffValue::Degradation))
            ),
            DifferenceWire::Rule {
                target,
                change,
                reason,
                loses,
                annotations,
                from,
                to,
            } => {
                if to
                    .amendment
                    .as_ref()
                    .is_none_or(|a| a.from != from.version || a.to != to.version)
                {
                    return Err("rule difference requires a directed amendment");
                }
                row!(
                    Rule,
                    target,
                    change,
                    reason,
                    loses,
                    annotations,
                    Some(DiffValue::Rule(from)),
                    Some(DiffValue::Rule(to))
                )
            }
            DifferenceWire::Actor {
                target,
                change,
                reason,
                loses,
                annotations,
            } => row!(
                Actor,
                target,
                change,
                reason,
                loses,
                annotations,
                None,
                None
            ),
            DifferenceWire::Relation {
                target,
                change,
                reason,
                loses,
                annotations,
                from,
                to,
            } => {
                let (from, to) = match (from, to) {
                    (ProducerField::Missing, ProducerField::Missing) => (None, None),
                    (ProducerField::Value(from), ProducerField::Value(to))
                        if change == Change::Modified
                            && identifier(&target)
                            && producer_name(&from.producer)
                            && producer_name(&to.producer)
                            && from != to =>
                    {
                        if annotations
                            .iter()
                            .any(|a| !matches!(a, Annotation::Preserve { .. }))
                        {
                            return Err("relation producer annotations must be preserve");
                        }
                        (
                            Some(DiffValue::Producer(from)),
                            Some(DiffValue::Producer(to)),
                        )
                    }
                    _ => return Err("invalid relation producer difference"),
                };
                row!(
                    Relation,
                    target,
                    change,
                    reason,
                    loses,
                    annotations,
                    from,
                    to
                )
            }
            DifferenceWire::Meaning {
                target,
                change,
                reason,
                loses,
                annotations,
                from,
                to,
            } => row!(
                Meaning,
                target,
                change,
                reason,
                loses,
                annotations,
                Some(from.into()),
                Some(to.into())
            ),
            DifferenceWire::Authority {
                target,
                change,
                reason,
                loses,
                annotations,
                from,
                to,
            } => row!(
                Authority,
                target,
                change,
                reason,
                loses,
                annotations,
                Some(from.map_or(DiffValue::Inactive, DiffValue::Authority)),
                Some(to.map_or(DiffValue::Inactive, DiffValue::Authority))
            ),
            DifferenceWire::Resource {
                target,
                change,
                reason,
                loses,
                annotations,
                from,
                to,
            } => row!(
                Resource,
                target,
                change,
                reason,
                loses,
                annotations,
                Some(from.map_or(DiffValue::Inactive, DiffValue::Resource)),
                Some(to.map_or(DiffValue::Inactive, DiffValue::Resource))
            ),
            DifferenceWire::Time {
                target,
                change,
                reason,
                loses,
                annotations,
                from,
                to,
            } => row!(
                Time,
                target,
                change,
                reason,
                loses,
                annotations,
                Some(DiffValue::Time(from)),
                Some(DiffValue::Time(to))
            ),
            DifferenceWire::Visibility {
                target,
                change,
                reason,
                loses,
                annotations,
                from,
                to,
            } => row!(
                Visibility,
                target,
                change,
                reason,
                loses,
                annotations,
                Some(DiffValue::Readers(from)),
                Some(DiffValue::Readers(to))
            ),
            DifferenceWire::Contest {
                target,
                change,
                reason,
                loses,
                annotations,
                from,
                to,
            } => row!(
                Contest,
                target,
                change,
                reason,
                loses,
                annotations,
                Some(DiffValue::Contests(from)),
                Some(DiffValue::Contests(to))
            ),
        };
        if matches!(d.kind, Kind::Actor | Kind::Relation) && d.from.is_none() {
            if d.change == Change::Modified {
                return Err("legacy differences cannot be modified");
            }
        } else {
            if d.kind == Kind::Meaning {
                let response = d.target.strip_prefix("response.").is_some_and(identifier);
                if [&d.from, &d.to].iter().any(|v| {
                    !matches!(
                        (response, v),
                        (true, Some(DiffValue::Response(_) | DiffValue::Inactive))
                            | (false, Some(DiffValue::Meaning(_)))
                    )
                }) {
                    return Err("meaning payload differs from target");
                }
            }
            if d.from == d.to {
                return Err("equal difference values");
            }
            let empty = match d.kind {
                Kind::Meaning if d.target.starts_with("response.") => DiffValue::Inactive,
                Kind::Meaning => DiffValue::Meaning(false),
                Kind::Visibility => DiffValue::Readers(vec![]),
                Kind::Contest => DiffValue::Contests(vec![]),
                Kind::Actor
                | Kind::Relation
                | Kind::Authority
                | Kind::Resource
                | Kind::Time
                | Kind::Rule
                | Kind::Qualification
                | Kind::Performance
                | Kind::Degradation => DiffValue::Inactive,
            };
            let expected = if matches!(d.kind, Kind::Time | Kind::Visibility) {
                Change::Modified
            } else if d.from.as_ref() == Some(&empty) {
                Change::Added
            } else if d.to.as_ref() == Some(&empty) {
                Change::Removed
            } else {
                Change::Modified
            };
            if d.change != expected {
                return Err("change does not agree with difference values");
            }
            if let (Some(DiffValue::Time(a)), Some(DiffValue::Time(b))) = (&d.from, &d.to)
                && (a.is_empty()
                    || b.is_empty()
                    || !a.windows(2).all(|w| w[0] < w[1])
                    || !b.windows(2).all(|w| w[0] < w[1]))
            {
                return Err("invalid call mode set");
            }
            for value in [&d.from, &d.to].into_iter().flatten() {
                if let DiffValue::Qualification(people) = value
                    && (people.iter().any(String::is_empty)
                        || !people.windows(2).all(|w| w[0] < w[1]))
                {
                    return Err("invalid qualification set");
                }
            }
        }
        Ok(d)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MeaningValue {
    Legacy(bool),
    Response(Box<crate::response_inputs::Control>),
    Inactive,
}
impl From<MeaningValue> for DiffValue {
    fn from(v: MeaningValue) -> Self {
        match v {
            MeaningValue::Legacy(v) => Self::Meaning(v),
            MeaningValue::Response(v) => Self::Response(*v),
            MeaningValue::Inactive => Self::Inactive,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    pub elapsed: report::Range,
    pub work: report::Range,
    pub result_at: Option<report::Range>,
}

impl From<&report::Terminal> for Metrics {
    fn from(terminal: &report::Terminal) -> Self {
        Self {
            elapsed: terminal.elapsed,
            work: terminal.work,
            result_at: terminal.result_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalComparison {
    pub end: String,
    pub from: Option<Metrics>,
    pub to: Option<Metrics>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyComparison {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cases: Option<Vec<CasePropertyComparison>>,
    pub id: String,
    pub from: Verdict,
    pub to: Verdict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseComparison {
    pub case_id: u64,
    pub arrival: u64,
    pub terminals: Vec<CaseTerminalComparison>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseTerminalComparison {
    pub kind: report::CaseTerminalKind,
    pub end: Option<String>,
    pub from: Option<CaseValue>,
    pub to: Option<CaseValue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseValue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_until: Option<report::Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_elapsed: Option<report::Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_work: Option<report::Range>,
    pub completed_at: Option<report::Range>,
    pub elapsed: Option<report::Range>,
    pub work: Option<report::Range>,
    pub result_at: Option<report::Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<crate::probability::ProbabilityRange>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CasePropertyComparison {
    pub case_id: u64,
    pub arrival: u64,
    pub from: PropertyValue,
    pub to: PropertyValue,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyValue {
    pub verdict: Verdict,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<crate::probability::ProbabilityRange>,
}

impl DiffReport {
    /// No comparison was performed. Also used for CLI input failure, for which
    /// source metadata is unavailable rather than the hash of an empty string.
    pub fn uncomputed(from: Side, to: Side) -> Self {
        let mut result = Self {
            ledger: crate::ledger::Ledger::aggregate("diff"),
            uncertainty: crate::uncertainty::Uncertainty::not_lowered(),
            preservation: None,
            metrics: None,
            quality: None,
            case_outcomes: None,
            agency: AgencySides {
                from: from.report.agency.clone(),
                to: to.report.agency.clone(),
                differences: crate::agency::Differences::unavailable(
                    crate::agency::DifferenceReason::ComparisonMismatch,
                ),
            },
            verdict: Verdict::Unsupported,
            from,
            to,
            mismatch: None,
            differences: Vec::new(),
            terminals: Vec::new(),
            properties: Vec::new(),
            diagnostics: Vec::new(),
            checked: Vec::new(),
            unchecked: Vec::new(),
        };
        result.finish();
        result
    }

    fn diagnostic(&mut self, code: &str, message: String, span: Option<diag::Span>) {
        self.diagnostics.push(diag::Diagnostic {
            code: code.into(),
            severity: diag::severity_of(code).expect("registered diff diagnostic"),
            message,
            span,
        });
    }

    fn finish(&mut self) {
        self.finish_with_agency_comparison(crate::agency::differences);
    }

    /// Private injection seam shares the actual comparison/withdrawal protocol.
    pub(crate) fn finish_with_agency_comparison(
        &mut self,
        compare: impl FnOnce(
            crate::agency::ComparisonSide<'_>,
            crate::agency::ComparisonSide<'_>,
        ) -> crate::agency::Differences,
    ) {
        let receipt_not_applicable = self
            .unchecked
            .iter()
            .any(|x| x == crate::comparison_projection::RECEIPT_NOT_APPLICABLE);
        use crate::agency::{ComparisonSide, DifferenceReason, Differences};
        self.agency.differences = if self.agency.from.is_none() || self.agency.to.is_none() {
            Differences::unavailable(DifferenceReason::AgencyUnavailable)
        } else if self.mismatch.is_some()
            || !CHECKED
                .iter()
                .all(|word| self.checked.iter().any(|c| c == word))
        {
            Differences::unavailable(DifferenceReason::ComparisonMismatch)
        } else {
            compare(
                ComparisonSide {
                    agency: self
                        .agency
                        .from
                        .as_ref()
                        .and_then(crate::agency::AgencyDerivation::as_derived),
                    responsibilities: self.from.report.responsibilities.as_deref(),
                },
                ComparisonSide {
                    agency: self
                        .agency
                        .to
                        .as_ref()
                        .and_then(crate::agency::AgencyDerivation::as_derived),
                    responsibilities: self.to.report.responsibilities.as_deref(),
                },
            )
        };
        self.unchecked = UNCHECKED
            .into_iter()
            .filter(|_| !self.checked.iter().any(|c| c == "annotations"))
            .chain(
                CHECKED
                    .into_iter()
                    .filter(|name| !self.checked.iter().any(|c| c == name)),
            )
            .map(String::from)
            .collect();
        if receipt_not_applicable {
            self.unchecked
                .push(crate::comparison_projection::RECEIPT_NOT_APPLICABLE.into());
        }
        if let Some(rows) = &self.metrics {
            if rows.is_empty() {
                self.unchecked.push("metric_tables".into());
            } else {
                self.unchecked.extend([
                    "metric_differences".into(),
                    "metric_ranking".into(),
                    "metric_semantic_alignment".into(),
                ]);
            }
        }
        if (self.from.report.responsibilities.is_some()
            || self.to.report.responsibilities.is_some())
            && !self.checked.iter().any(|c| c == "authority")
        {
            self.unchecked.push("responsibility".into());
        }
        if self.from.report.obligations.is_some() || self.to.report.obligations.is_some() {
            self.unchecked.push("obligation_tables".into());
        }
        if self.from.report.contest_runtime.is_some() || self.to.report.contest_runtime.is_some() {
            self.unchecked.push("contest_runtime_tables".into());
        }
        if [&self.from.report.metrics, &self.to.report.metrics]
            .into_iter()
            .flatten()
            .any(|m| m.overall.queue_wait_observed.is_some())
        {
            self.unchecked.push("waiting_tables".into());
        }
        // Projection is attempted only after the static comparison checks. An
        // aborted report cannot distinguish an absent terminal from unknown data.
        if self.case_outcomes.is_some() {
            for name in ["case_outcomes", "case_properties"] {
                if !self.checked.iter().any(|c| c == name) {
                    self.unchecked.push(name.into());
                }
            }
        } else if CHECKED
            .iter()
            .all(|name| self.checked.iter().any(|c| c == name))
            && (self.from.report.bounds.truncated.is_none()
                || self.to.report.bounds.truncated.is_none())
        {
            self.unchecked.push("terminals".into());
        }
        if matches!(&self.agency.differences.result, crate::agency::DifferenceStatus::NotDerived { reason } if *reason != DifferenceReason::DerivationLimit)
        {
            self.unchecked.push("agency_differences".into());
        }
        self.verdict = report::fold_verdict(
            &[
                report::Property {
                    notes: None,
                    cases: None,
                    probability: None,
                    id: "from".into(),
                    verdict: self.from.report.verdict,
                },
                report::Property {
                    notes: None,
                    cases: None,
                    probability: None,
                    id: "to".into(),
                    verdict: self.to.report.verdict,
                },
            ],
            &self.diagnostics,
        );
        let mut verdicts = vec![report::Property {
            notes: None,
            cases: None,
            probability: None,
            id: "sides".into(),
            verdict: self.verdict,
        }];
        for d in &self.differences {
            for a in &d.annotations {
                if let Annotation::Override { verdict, .. } = a {
                    verdicts.push(report::Property {
                        notes: None,
                        cases: None,
                        probability: None,
                        id: "override".into(),
                        verdict: *verdict,
                    });
                }
            }
        }
        self.verdict = report::fold_verdict(&verdicts, &self.diagnostics);
        self.seal();
    }
    fn seal(&mut self) {
        let host = serde_json::to_value(&self).expect("diff uncertainty context");
        self.uncertainty = crate::uncertainty::Uncertainty::aggregate(&host, "diff");
        let host = serde_json::to_value(&self).expect("diff wire");
        crate::ledger::complete(
            &mut self.ledger,
            &host,
            &mut self.verdict,
            &mut self.diagnostics,
        );
    }
}

/// Both sides use exactly the configuration-check pipeline. Static lowering
/// failure stops comparison, while an engine abort retains its valid call sets.
pub fn diff(src: &str, from: &str, to: &str) -> DiffReport {
    diff_with_scenario(src, from, to, None)
}

pub fn diff_with_scenario(src: &str, from: &str, to: &str, scenario: Option<&str>) -> DiffReport {
    let analyze =
        |name| report::check_selected_scenario_using(src, name, scenario, crate::engine::explore);
    let (from_report, from_model) = analyze(from);
    let (to_report, to_model) = analyze(to);
    let mut result = DiffReport::uncomputed(
        Side {
            configuration: from.into(),
            report: report::finite_driver(from_report, crate::engine::DEFAULT_STATE_BUDGET),
        },
        Side {
            configuration: to.into(),
            report: report::finite_driver(to_report, crate::engine::DEFAULT_STATE_BUDGET),
        },
    );
    if crate::syntax::parse(src).is_ok_and(|m| m.has_merges()) {
        result.diagnostic(
            diag::U_SEM_0,
            "merge_feature_combination: merge diff projection is not connected".into(),
            None,
        );
        result.finish();
        result.verdict = Verdict::Unsupported;
        return result;
    }
    let (Some(from_model), Some(to_model)) = (from_model, to_model) else {
        if crate::syntax::parse(src).is_ok_and(|m| m.items.iter().any(|i| matches!(i, ast::Item::Actor(a) if a.items.iter().any(|f| matches!(f, ast::ActorItem::Decides(_)))))) {
            result.unchecked.push("rule".into());
        }
        return result;
    };
    if crate::evidence_basis::present(&from_model.model)
        || crate::evidence_basis::present(&to_model.model)
    {
        result.diagnostic(
            diag::U_SEM_0,
            "evidence_basis_unsupported_command".into(),
            None,
        );
        result.finish();
        // The native side checks can establish violations, but no evidence
        // comparison was performed. Retain those facts without making this
        // unsupported command look like a supported, violated comparison.
        result.verdict = Verdict::Unsupported;
        result.seal();
        return result;
    }
    if from_model.model.has_loops() || to_model.model.has_loops() {
        result.diagnostic(
            diag::U_SEM_0,
            "loop_feature_combination: repeat diff projection is not connected".into(),
            None,
        );
        result.finish();
        return result;
    }
    if from_model.model.response_attempts().is_empty()
        != to_model.model.response_attempts().is_empty()
    {
        result
            .unchecked
            .push(crate::comparison_projection::RECEIPT_NOT_APPLICABLE.into());
    }
    if report::priority::comparison_mismatch(&from_model.model, &to_model.model) {
        result.diagnostic(
            diag::U_SEM_0,
            "priority_feature_combination: diff requires identical priority inputs and disciplines"
                .into(),
            None,
        );
        result.finish();
        return result;
    }
    if from_model.model.has_condition_wait() || to_model.model.has_condition_wait() {
        // Intent/override/maintenance and contest candidates do not model
        // condition releases. Do not silently reuse the If-only projection.
        result.diagnostic(
            diag::U_SEM_0,
            "condition_wait_diff_unsupported".into(),
            None,
        );
        result.finish();
        return result;
    }
    let has_rules = from_model
        .model
        .actors
        .iter()
        .chain(&to_model.model.actors)
        .any(|a| a.metadata.rule.is_some());
    // Receipt outcomes require the case projection even when only `to` uses
    // Q1. Do not broaden older non-Q1 comparison wire contracts here.
    if !from_model.model.legacy_route() || !to_model.model.response_attempts().is_empty() {
        result.case_outcomes = Some(Vec::new());
    }
    if from_model.model.quality.is_some() {
        result.quality = Some(Vec::new());
    }
    // Successful configuration lowering always resolves its mandatory Cut.
    let (report::Cut::Declared(a), report::Cut::Declared(b)) =
        (&result.from.report.cut, &result.to.report.cut)
    else {
        unreachable!("configuration lowering resolves its Cut")
    };
    result.checked.push(CHECKED[0].into());
    let ah = &result.from.report.cut_context.horizon;
    let bh = &result.to.report.cut_context.horizon;
    if a.name != b.name || ah != bh {
        if has_rules {
            result.unchecked.push("rule".into());
        }
        result.mismatch = Some(Mismatch {
            from_cut: a.name.clone(),
            to_cut: b.name.clone(),
            differences: cut_differences(a, b, ah, bh),
        });
        result.diagnostic(
            diag::E_CMP_0,
            "configurations refer to different Cuts".into(),
            None,
        );
        result.finish();
        return result;
    }
    result.differences = static_differences(&from_model, &to_model);
    result.differences.extend(static_projection::differences(
        src,
        &from_model.model,
        &to_model.model,
    ));
    result
        .differences
        .extend(response_differences(&from_model.model, &to_model.model));
    let rule_complete = rule_differences(&mut result, &from_model.model, &to_model.model);
    result
        .differences
        .extend(static_projection::producer_differences(
            &from_model.model,
            &to_model.model,
        ));
    match q6_differences(
        &from_model.model,
        result.from.report.work_profiles.as_ref(),
        &to_model.model,
        result.to.report.work_profiles.as_ref(),
    ) {
        Ok(rows) => result.differences.extend(rows),
        Err(message) => {
            result.diagnostic(diag::U_SEM_0, message, None);
            result.finish();
            return result;
        }
    }
    intent::apply(&mut result, &from_model, &to_model);
    result
        .checked
        .extend(CHECKED[1..].iter().map(|s| String::from(*s)));
    result.checked.extend(
        [
            "meaning",
            "authority",
            "resource",
            "time",
            "visibility",
            "contest",
            "preserve",
            "override",
            "maintain",
            "annotations",
        ]
        .map(String::from),
    );
    if result.case_outcomes.is_some() {
        project_case_reports(&mut result, &from_model.model, &to_model.model);
    } else {
        project_reports(&mut result);
    }
    intent::preservation(&mut result, &to_model);
    if from_model.model.metrics.is_some()
        && !result
            .diagnostics
            .iter()
            .any(|d| d.severity == diag::Severity::Error)
    {
        use crate::comparison_projection as p;
        result.metrics = Some(Vec::new());
        if let (Some(a), Some(b)) = (&result.from.report.metrics, &result.to.report.metrics)
            && a.status != crate::metrics::Status::Aborted
            && b.status != crate::metrics::Status::Aborted
        {
            match (
                p::validate_metrics(Some(a), &from_model.model, false)
                    .and_then(|_| p::metric_rows(a, &from_model.model)),
                p::validate_metrics(Some(b), &to_model.model, false)
                    .and_then(|_| p::metric_rows(b, &to_model.model)),
            ) {
                (Ok(a), Ok(b)) => {
                    let keys: std::collections::BTreeSet<_> =
                        a.keys().chain(b.keys()).cloned().collect();
                    result.metrics = Some(
                        keys.into_iter()
                            .map(|key| MetricComparison {
                                from: p::metric_cell(a.get(&key)),
                                to: p::metric_cell(b.get(&key)),
                                key,
                            })
                            .collect(),
                    );
                    result.checked.push("metric_tables".into());
                }
                _ => result.diagnostic(
                    diag::U_SEM_0,
                    "invalid metrics report projection".into(),
                    None,
                ),
            }
        }
    }
    if result.mismatch.is_none() {
        use crate::comparison_projection as p;
        match (
            p::quality_rows(
                result.from.report.quality.as_ref(),
                &from_model.model,
                false,
            ),
            p::quality_rows(result.to.report.quality.as_ref(), &to_model.model, false),
        ) {
            (Ok(a), Ok(b)) if a.len() == b.len() => {
                let rows=a.iter().zip(&b).map(|(a,b)| {
                    let mut row=serde_json::json!({"key":a.key,"from":{"status":"reported","value":a.value},"to":{"status":"reported","value":b.value}});
                    if let Some(d)=&a.denominator {row["denominator"]=d.clone();}row
                }).collect();
                if from_model.model.quality.is_some() {
                    result.quality = Some(rows);
                }
            }
            _ => result.diagnostic(
                diag::U_SEM_0,
                "invalid quality report projection".into(),
                None,
            ),
        }
    }
    result.finish_with_agency_comparison(|a, b| {
        crate::agency::differences_for_models(a, &from_model.model, b, &to_model.model)
    });
    if !from_model.model.resources.is_empty() || !to_model.model.resources.is_empty() {
        result.checked.retain(|word| word != "resource");
        if !result.unchecked.iter().any(|word| word == "resource") {
            result.unchecked.push("resource".into());
        }
    }
    if has_rules {
        if rule_complete {
            result.checked.push("rule".into());
        } else {
            result.unchecked.push("rule".into());
        }
    }
    result.unchecked.extend([
        "relation_state_producers".into(),
        "relation_binding_identity".into(),
    ]);
    result
}

fn rule_differences(
    result: &mut DiffReport,
    from: &crate::ir::Model,
    to: &crate::ir::Model,
) -> bool {
    fn value(m: &crate::ir::Model, rule: &crate::ir::RuleInfo) -> RuleValue {
        RuleValue {
            version: rule.effective_version.clone(),
            values: rule
                .values
                .iter()
                .map(|rv| {
                    let variable = &m.state_variables[rv.variable.0];
                    let crate::ir::InitialValue::Fixed(v) = variable.initial else {
                        unreachable!("validated rule initial")
                    };
                    RuleCell {
                        name: variable.name.clone(),
                        value: crate::actor_table::rule_scalar(m, variable, v),
                    }
                })
                .collect(),
            amendment: rule
                .amendment
                .as_ref()
                .map(|a| crate::actor_table::amendment(m, a)),
        }
    }
    let mut actors: Vec<_> = to
        .actors
        .iter()
        .filter(|a| a.metadata.rule.is_some())
        .collect();
    actors.sort_by(|a, b| a.name.cmp(&b.name));
    let mut complete = true;
    for actor in actors {
        let Some(a) = from
            .actors
            .iter()
            .find(|a| a.name == actor.name)
            .and_then(|a| a.metadata.rule.as_ref())
        else {
            complete = false;
            continue;
        };
        let b = actor.metadata.rule.as_ref().unwrap();
        let (a, b) = (value(from, a), value(to, b));
        if a.version == b.version && a.values == b.values {
            continue;
        }
        if b.amendment
            .as_ref()
            .is_none_or(|amendment| amendment.from != a.version || amendment.to != b.version)
        {
            complete = false;
            result.diagnostic(
                diag::E_CON_0,
                "rule version change has no declared amendment".into(),
                Some(actor.span),
            );
            continue;
        }
        result.differences.push(Difference {
            kind: Kind::Rule,
            target: actor.name.clone(),
            change: Change::Modified,
            reason: None,
            loses: None,
            from: Some(DiffValue::Rule(a)),
            to: Some(DiffValue::Rule(b)),
            annotations: Vec::new(),
        });
    }
    complete
}

fn static_differences(
    from: &lower::SelectedConfiguration,
    to: &lower::SelectedConfiguration,
) -> Vec<Difference> {
    let mut differences = Vec::new();
    for (kind, a, b) in [
        (Kind::Actor, &from.actors, &to.actors),
        (Kind::Relation, &from.relations, &to.relations),
    ] {
        for (change, targets) in [
            (Change::Added, b.difference(a)),
            (Change::Removed, a.difference(b)),
        ] {
            differences.extend(targets.map(|target| Difference {
                kind,
                target: target.clone(),
                change,
                reason: None,
                loses: None,
                from: None,
                to: None,
                annotations: Vec::new(),
            }));
        }
    }
    differences
}

fn project_reports(result: &mut DiffReport) {
    let a = &result.from.report;
    let b = &result.to.report;
    result.terminals.clear();
    if a.bounds.truncated.is_some() && b.bounds.truncated.is_some() {
        let ends: BTreeSet<_> = a
            .terminals
            .iter()
            .chain(&b.terminals)
            .map(|t| &t.end)
            .collect();
        result.terminals = ends
            .into_iter()
            .map(|end| TerminalComparison {
                end: end.clone(),
                from: a
                    .terminals
                    .iter()
                    .find(|t| &t.end == end)
                    .map(Metrics::from),
                to: b
                    .terminals
                    .iter()
                    .find(|t| &t.end == end)
                    .map(Metrics::from),
            })
            .collect();
    }
    let ids: BTreeSet<_> = a
        .properties
        .iter()
        .chain(&b.properties)
        .map(|p| &p.id)
        .collect();
    // Contract inventories have model-wide clause ordinals and interleaved
    // explicit verifies. Preserve that inventory order, not lexical id order.
    // Keep the pre-contract projection unchanged for legacy models.
    let ids: Vec<_> = if ids.iter().any(|id| id.starts_with("contract ")) {
        let mut seen = BTreeSet::new();
        a.properties
            .iter()
            .chain(&b.properties)
            .map(|p| &p.id)
            .filter(|id| seen.insert(*id))
            .collect()
    } else {
        ids.into_iter().collect()
    };
    let properties: Option<Vec<_>> = ids
        .into_iter()
        .map(|id| {
            Some(PropertyComparison {
                cases: None,
                id: id.clone(),
                from: a.properties.iter().find(|p| &p.id == id)?.verdict,
                to: b.properties.iter().find(|p| &p.id == id)?.verdict,
            })
        })
        .collect();
    // Verify is model-wide, but do not panic or publish a partial comparison if
    // a future report producer violates that shared inventory contract.
    result.properties = properties.unwrap_or_else(|| {
        result.diagnostic(
            diag::U_SEM_0,
            "configuration reports have different property inventories".into(),
            None,
        );
        Vec::new()
    });
}

pub(crate) fn cut_differences(
    a: &report::DeclaredCut,
    b: &report::DeclaredCut,
    ah: &[crate::ir::Horizon],
    bh: &[crate::ir::Horizon],
) -> Vec<CutDifference> {
    let mut differences = Vec::new();
    if ah != bh {
        differences.push(CutDifference::Horizon {
            from: ah.to_vec(),
            to: bh.to_vec(),
        });
    }
    if a.question != b.question {
        differences.push(CutDifference::Question {
            from: a.question.clone(),
            to: b.question.clone(),
        });
    }
    fn set(names: &[String]) -> BTreeSet<&String> {
        names.iter().collect()
    }
    if set(&a.include) != set(&b.include) {
        differences.push(CutDifference::Include {
            from: a.include.clone(),
            to: b.include.clone(),
        });
    }
    // Keyed by target and retaining all metadata. Keep duplicate entries visible
    // instead of overwriting them while normalizing description order.
    fn entries(values: &[report::CutEntry]) -> Vec<&report::CutEntry> {
        let mut entries: Vec<_> = values.iter().collect();
        entries.sort_by(|a, b| {
            (&a.target, a.actor, &a.reason, &a.loses)
                .cmp(&(&b.target, b.actor, &b.reason, &b.loses))
        });
        entries
    }
    if entries(&a.exclude) != entries(&b.exclude) {
        differences.push(CutDifference::Exclude {
            from: a.exclude.clone(),
            to: b.exclude.clone(),
        });
    }
    if entries(&a.defer) != entries(&b.defer) {
        differences.push(CutDifference::Defer {
            from: a.defer.clone(),
            to: b.defer.clone(),
        });
    }
    if a.externality != b.externality {
        differences.push(CutDifference::Externality {
            from: a.externality.clone(),
            to: b.externality.clone(),
        });
    }
    if a.reentry != b.reentry {
        differences.push(CutDifference::Reentry {
            from: a.reentry.clone(),
            to: b.reentry.clone(),
        });
    }
    differences
}

fn response_differences(from: &crate::ir::Model, to: &crate::ir::Model) -> Vec<Difference> {
    fn controls(
        m: &crate::ir::Model,
    ) -> std::collections::BTreeMap<String, crate::response_inputs::Control> {
        m.response_attempts()
            .into_iter()
            .filter(|(_, r)| r.attempt == 1)
            .map(|(_, r)| {
                (
                    m.site_name(r.command),
                    crate::response_inputs::control(m, r),
                )
            })
            .collect()
    }
    let (a, b) = (controls(from), controls(to));
    a.keys()
        .chain(b.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|command| {
            let (x, y) = (a.get(command), b.get(command));
            (x != y).then(|| Difference {
                kind: Kind::Meaning,
                target: format!("response.{command}"),
                change: match (x, y) {
                    (None, _) => Change::Added,
                    (_, None) => Change::Removed,
                    _ => Change::Modified,
                },
                reason: None,
                loses: None,
                from: Some(x.cloned().map_or(DiffValue::Inactive, DiffValue::Response)),
                to: Some(y.cloned().map_or(DiffValue::Inactive, DiffValue::Response)),
                annotations: vec![],
            })
        })
        .collect()
}

fn project_case_reports(result: &mut DiffReport, from: &crate::ir::Model, to: &crate::ir::Model) {
    use crate::comparison_projection::{self as projection, TerminalValue};
    let projected = (|| -> projection::Result<_> {
        let a = projection::exhaustive(&result.from.report, from)?;
        let b = projection::exhaustive(&result.to.report, to)?;
        if a.arrivals != b.arrivals
            || !a
                .properties
                .iter()
                .map(|p| &p.id)
                .filter(|id| id.as_str() != "receipt_resolution")
                .eq(b
                    .properties
                    .iter()
                    .map(|p| &p.id)
                    .filter(|id| id.as_str() != "receipt_resolution"))
        {
            return Err("configuration report inventories differ");
        }
        let mut properties = Vec::new();
        for p in &a.properties {
            let Some(q) = b.properties.iter().find(|q| q.id == p.id) else {
                continue;
            };
            let cases = if p.id == projection::SAFETY || p.values.contains_key(&None) {
                None
            } else {
                let mut rows = Vec::new();
                for (&case_id, &arrival) in &a.arrivals {
                    let value = |p: &projection::Property| -> projection::Result<PropertyValue> {
                        let v = p
                            .values
                            .get(&Some(case_id))
                            .ok_or("missing property case")?;
                        Ok(PropertyValue {
                            verdict: v.verdict,
                            probability: v.probability.clone(),
                        })
                    };
                    rows.push(CasePropertyComparison {
                        case_id,
                        arrival,
                        from: value(p)?,
                        to: value(q)?,
                    });
                }
                Some(rows)
            };
            properties.push(PropertyComparison {
                id: p.id.clone(),
                from: p.aggregate,
                to: q.aggregate,
                cases,
            });
        }
        let mut outcomes = Vec::new();
        if !a.aborted && !b.aborted {
            for (&case_id, &arrival) in &a.arrivals {
                let keys: BTreeSet<_> = a
                    .terminals
                    .keys()
                    .chain(b.terminals.keys())
                    .filter(|(id, _)| *id == case_id)
                    .map(|(_, k)| k)
                    .collect();
                let mut terminals = Vec::new();
                for key in keys {
                    let value =
                        |v: &projection::View| match v.terminals.get(&(case_id, key.clone())) {
                            Some(TerminalValue::Exhaustive(t)) => Some(CaseValue {
                                observed_until: t.observed_until,
                                observed_elapsed: t.observed_elapsed,
                                observed_work: t.observed_work,
                                completed_at: t.completed_at,
                                elapsed: t.elapsed,
                                work: t.work,
                                result_at: t.result_at,
                                probability: t.probability.clone(),
                            }),
                            _ => None,
                        };
                    let (kind, end) = key.fields();
                    terminals.push(CaseTerminalComparison {
                        kind,
                        end,
                        from: value(&a),
                        to: value(&b),
                    });
                }
                outcomes.push(CaseComparison {
                    case_id,
                    arrival,
                    terminals,
                });
            }
        }
        Ok((properties, outcomes, !a.aborted && !b.aborted))
    })();
    match projected {
        Ok((properties, outcomes, complete)) => {
            result.properties = properties;
            result.case_outcomes = Some(outcomes);
            if complete {
                result.checked.push("case_outcomes".into());
            }
            result.checked.push("case_properties".into());
        }
        Err(message) => {
            result.diagnostic(diag::U_SEM_0, message.into(), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_payloads_roundtrip_by_kind_including_null_and_empty_arrays() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        for name in extension_population::NAMES {
            let src = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
            let (from, to) = if name.starts_with("diff2-invoice") {
                ("AsIs", "ToBeA")
            } else {
                ("From", "To")
            };
            let d = diff(&src, from, to);
            let wire = serde_json::to_value(&d).unwrap();
            let decoded: DiffReport = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(decoded, d, "{name}");
            assert_eq!(serde_json::to_value(decoded).unwrap(), wire, "{name}");
            for row in d
                .differences
                .iter()
                .filter(|d| !matches!(d.kind, Kind::Actor | Kind::Relation))
            {
                let mut bad = serde_json::to_value(row).unwrap();
                bad.as_object_mut().unwrap().remove("from");
                assert!(
                    serde_json::from_value::<Difference>(bad).is_err(),
                    "required from for {name}"
                );
            }
        }
    }

    #[test]
    fn property_inventory_mismatch_fails_closed_without_panicking() {
        let src = include_str!("../../../fixtures/config-diff-parallel/model.relca");
        for from_side in [true, false] {
            for rename in [true, false] {
                let mut result = diff(src, "AsIs", "ToBeP");
                let terminals = result.terminals.clone();
                let side = if from_side {
                    &mut result.from
                } else {
                    &mut result.to
                };
                if rename {
                    side.report.properties[0].id = "one-sided property".into();
                } else {
                    side.report.properties.remove(0);
                }
                project_reports(&mut result);
                result.finish();
                assert!(result.properties.is_empty());
                assert_eq!(result.terminals, terminals);
                assert_eq!(result.verdict, Verdict::Unsupported);
                assert!(result.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
                assert!(
                    result
                        .from
                        .report
                        .diagnostics
                        .iter()
                        .all(|d| d.code != diag::U_SEM_0)
                );
                assert!(
                    result
                        .to
                        .report
                        .diagnostics
                        .iter()
                        .all(|d| d.code != diag::U_SEM_0)
                );
            }
        }
    }
}
