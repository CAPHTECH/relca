//! ADR 0019 wire types and exact nonnegative business quantities.
//! Runtime collectors belong to their respective interpreters, not this module.
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
pub mod contests;

#[cfg(test)]
#[path = "../tests/support/p3a_wait_history_hand.rs"]
pub(crate) mod p3a_wait_hand;

fn required_nullable<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(d)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MetricNumber(pub BigRational);

impl From<u64> for MetricNumber {
    fn from(value: u64) -> Self {
        Self(BigRational::from_integer(BigInt::from(value)))
    }
}

impl Serialize for MetricNumber {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for MetricNumber {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        let parsed = (|| {
            let (n, q) = text.split_once('/').unwrap_or((&text, "1"));
            if n.is_empty()
                || q.is_empty()
                || !n.bytes().chain(q.bytes()).all(|b| b.is_ascii_digit())
            {
                return None;
            }
            let n = BigInt::parse_bytes(n.as_bytes(), 10)?;
            let q = BigInt::parse_bytes(q.as_bytes(), 10)?;
            if q.is_zero() {
                return None;
            }
            let value = BigRational::new(n, q);
            (value.to_string() == text).then_some(Self(value))
        })();
        parsed.ok_or_else(|| D::Error::custom("expected canonical nonnegative metric fraction"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Refused,
    NotRefused,
    NotConfigured,
    Truncated,
    Aborted,
    NoSamples,
    NoTerminal,
    Rejected,
    NoTaskStarted,
    NeverCreated,
    CancelledBeforeStart,
    Unfinished,
    Deadlock,
    ZeroCapacity,
    NoCapacity,
    /// ADR 0029 U2: withdrawn because a branch stopped at an out-of-range write.
    RangeStopped,
    JoinAny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Range {
    pub min: Option<MetricNumber>,
    pub max: Option<MetricNumber>,
    pub reason: Option<Reason>,
}

impl<'de> Deserialize<'de> for Range {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            #[serde(deserialize_with = "required_nullable")]
            min: Option<MetricNumber>,
            #[serde(deserialize_with = "required_nullable")]
            max: Option<MetricNumber>,
            #[serde(deserialize_with = "required_nullable")]
            reason: Option<Reason>,
        }
        let f = Fields::deserialize(d)?;
        if !matches!((&f.min, &f.max, f.reason),
            (Some(a), Some(b), None) if a <= b)
            && !matches!((&f.min, &f.max, f.reason), (None, None, Some(_)))
        {
            return Err(D::Error::custom("invalid metric range or null reason"));
        }
        Ok(Self {
            min: f.min,
            max: f.max,
            reason: f.reason,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sample {
    pub mean: Option<MetricNumber>,
    pub min: Option<MetricNumber>,
    pub max: Option<MetricNumber>,
    pub samples: u64,
    pub reason: Option<Reason>,
}

impl<'de> Deserialize<'de> for Sample {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            #[serde(deserialize_with = "required_nullable")]
            mean: Option<MetricNumber>,
            #[serde(deserialize_with = "required_nullable")]
            min: Option<MetricNumber>,
            #[serde(deserialize_with = "required_nullable")]
            max: Option<MetricNumber>,
            samples: u64,
            #[serde(deserialize_with = "required_nullable")]
            reason: Option<Reason>,
        }
        let f = Fields::deserialize(d)?;
        let valid_value = matches!((&f.min, &f.mean, &f.max, f.reason),
            (Some(a), Some(x), Some(b), None) if a <= x && x <= b && f.samples > 0);
        let valid_null = matches!((&f.min, &f.mean, &f.max, f.reason),
            (None, None, None, Some(_)) if f.samples == 0);
        if !valid_value && !valid_null {
            return Err(D::Error::custom("invalid metric sample or null reason"));
        }
        Ok(Self {
            mean: f.mean,
            min: f.min,
            max: f.max,
            samples: f.samples,
            reason: f.reason,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    Range(Range),
    Sample(Sample),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Count {
    pub stats: Scalar,
    #[serde(deserialize_with = "required_nullable")]
    pub observed_total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndCount {
    pub end: String,
    pub count: Count,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseCounts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admitted: Option<Count>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflowed: Option<Count>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_off: Option<Count>,
    pub end_counts: Vec<EndCount>,
    pub rejected: Count,
    pub unfinished: Count,
    pub deadlocked: Count,
    pub not_arrived: Count,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverallCounts {
    /// A subset of unfinished, never an additional completed outcome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_unknown_count: Option<Count>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admitted: Option<Count>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflowed: Option<Count>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_off: Option<Count>,
    pub offered: Count,
    pub received: Count,
    pub completed: Count,
    pub rejected: Count,
    pub unfinished: Count,
    pub deadlocked: Count,
    pub not_arrived: Count,
    pub end_counts: Vec<EndCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Excluded {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_off: Option<Count>,
    pub unfinished: Count,
    pub deadlocked: Count,
    pub rejected: Count,
    pub other_end: Count,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalKind {
    End,
    Rejected,
    Overflow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseTerminal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakdown: Option<Breakdown>,
    pub kind: TerminalKind,
    #[serde(deserialize_with = "required_nullable")]
    pub end: Option<String>,
    pub lead_time: Scalar,
    pub cost_per_case: Scalar,
    pub count: Count,
    pub excluded: Excluded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repetition: Option<crate::ir::LoopSite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tariff_regular_work: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tariff_overtime_work: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tariff_regular_cost: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tariff_overtime_cost: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_causes: Option<WaitCauses>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predecessor_wait: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused_count: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_wait: Option<Scalar>,
    pub binding: String,
    pub capability: String,
    pub wait: Scalar,
    pub work: Scalar,
    pub starts: Scalar,
    pub never_created_count: Count,
    pub cancelled_before_start_count: Count,
    pub not_started_count: Count,
}

macro_rules! declare_case {
    ($( $(#[$attr:meta])* pub $field:ident: $ty:ty, )*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "CaseWire")]
        pub struct Case { $( $(#[$attr])* pub $field: $ty, )* }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct CaseWire { $( $(#[$attr])* $field: $ty, )* }
        impl TryFrom<CaseWire> for Case {
            type Error = String;
            fn try_from(w: CaseWire) -> Result<Self, Self::Error> {
                if w.arrival.is_some() == w.arrival_time.is_some() {
                    return Err("arrival must be fixed integer or null with arrival_time Sample".into());
                }
                Ok(Self { $( $field: w.$field, )* })
            }
        }
    };
}
declare_case! {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrival_time: Option<Sample>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dwell: Option<Dwell>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joins: Option<Vec<JoinRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_wait: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intentional_wait: Option<Scalar>,
    pub case_id: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub arrival: Option<u64>,
    pub counts: CaseCounts,
    pub first_start_wait: Scalar,
    pub terminals: Vec<CaseTerminal>,
    pub tasks: Vec<Task>,
    pub work: Scalar,
    pub observed_cost_total: Scalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terminal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<TerminalKind>,
    pub end: String,
    pub count: Count,
    pub cost_per_case: Scalar,
    #[serde(deserialize_with = "required_nullable")]
    pub completed_cases_total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tariff_regular_work: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tariff_overtime_work: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tariff_regular_cost: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tariff_overtime_cost: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_capacity_time: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_occupancy_time: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overtime_occupancy_time: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_utilization: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<ActorQueue>,
    pub actor: String,
    #[serde(deserialize_with = "required_nullable")]
    pub capacity: Option<u64>,
    pub work: Scalar,
    pub peak_tasks: Scalar,
    pub utilization: Scalar,
    pub occupancy_cost: Scalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_capacity_time: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_occupancy_time: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overtime_occupancy_time: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_utilization: Option<Scalar>,
    pub resource: String,
    pub capacity: u64,
    pub occupancy_time: Scalar,
    pub peak_units: Scalar,
    pub utilization: Scalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overtime_work: Option<Scalar>,
    pub person: String,
    pub work: Scalar,
    pub peak_tasks: Scalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub capability: String,
    pub starts: Scalar,
    pub start_cost: Scalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorRate {
    pub actor: String,
    #[serde(deserialize_with = "required_nullable")]
    pub per_minute: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub reason: Option<Reason>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartRate {
    pub capability: String,
    #[serde(deserialize_with = "required_nullable")]
    pub per_start: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub reason: Option<Reason>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rates {
    pub actors: Vec<ActorRate>,
    pub capabilities: Vec<StartRate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusinessValue {
    #[serde(deserialize_with = "required_nullable")]
    pub value: Option<MetricNumber>,
    pub reason: Reason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overall {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_wait_observed: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intentional_wait: Option<Scalar>,
    pub counts: OverallCounts,
    pub observed_until: Scalar,
    pub work: Scalar,
    pub named_people_work: Scalar,
    pub peak_tasks: Scalar,
    pub occupancy_cost: Scalar,
    pub start_cost: Scalar,
    pub observed_cost_total: Scalar,
    pub observed_cost_per_arrival: Scalar,
    pub business_value: BusinessValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Complete,
    Truncated,
    Aborted,
    /// ADR 0029 U2: range stop without a computational cut.
    RangeStopped,
}

macro_rules! declare_business_metrics {
    ($( $(#[$attr:meta])* pub $field:ident: $ty:ty, )*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "BusinessMetricsWire")]
        pub struct BusinessMetrics { $( $(#[$attr])* pub $field: $ty, )* }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct BusinessMetricsWire { $( $(#[$attr])* $field: $ty, )* }
        impl TryFrom<BusinessMetricsWire> for BusinessMetrics {
            type Error = String;
            fn try_from(w: BusinessMetricsWire) -> Result<Self, Self::Error> {
                let m = Self { $( $field: w.$field, )* };
                if let Some(table) = &m.contests { table.validate_metrics(&m)?; }
                Ok(m)
            }
        }
    };
}
declare_business_metrics! {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contests: Option<Box<contests::Table>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Vec<Resource>>,
    pub rules: String,
    pub status: Status,
    pub window: u64,
    pub currency: String,
    pub time_basis: String,
    pub lead_time_scope: String,
    pub observation_scope: String,
    #[serde(deserialize_with = "required_nullable")]
    pub statistical_unit: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub estimate_note: Option<String>,
    pub rates: Rates,
    pub cases: Vec<Case>,
    pub terminals: Vec<Terminal>,
    pub actors: Vec<Actor>,
    pub people: Vec<Person>,
    pub capabilities: Vec<Capability>,
    pub overall: Overall,
    pub checked: Vec<String>,
    pub unchecked: Vec<String>,
}

/// ADR 0048 DTOs. Bound is allowed only on explicitly observed quantities,
/// never on an ordinary Scalar or the queue maximum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueCauses {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar: Option<Scalar>,
    pub actor_capacity: Scalar,
    pub resource: Scalar,
    pub executor: Scalar,
    pub combined: Scalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Breakdown {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Scalar>,
    pub external: Scalar,
    pub in_service: Scalar,
    pub contest_route: Scalar,
    pub queue: QueueCauses,
    pub intentional: Scalar,
    pub unattributed: Scalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaitCauses {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Scalar>,
    pub actor_capacity: Scalar,
    pub resource: Scalar,
    pub executor: Scalar,
    pub combined: Scalar,
    pub unattributed: Scalar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bound {
    Lower,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedQuantity {
    pub stats: Scalar,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound: Option<Bound>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorQueue {
    pub wait_total: ObservedQuantity,
    pub length_max: Scalar,
    pub length_time_average: ObservedQuantity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dwell {
    pub until: Scalar,
    pub breakdown: Breakdown,
    pub bound: Bound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinMode {
    All,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinBranch {
    pub binding: String,
    pub join_wait: Scalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinRow {
    pub fork: String,
    pub mode: JoinMode,
    pub branches: Vec<JoinBranch>,
}

impl BusinessMetrics {
    /// Withdraw only applicable analyst quantities after the interpreter's own
    /// collection/reduction. Counts, row inventory and static inputs stay intact.
    pub(crate) fn apply_deferred(&mut self, m: &crate::ir::Model) {
        let Some(d) = m.deferred_metrics() else {
            return;
        };
        for row in &mut self.actors {
            let actor_deferred = m
                .actors
                .iter()
                .enumerate()
                .any(|(i, a)| a.name == row.actor && d.actor(m, crate::ir::ActorId(i)));
            if d.process || actor_deferred {
                row.work.defer();
                row.occupancy_cost.defer();
                row.peak_tasks.defer();
                row.utilization.defer();
                for s in [
                    &mut row.tariff_regular_work,
                    &mut row.tariff_overtime_work,
                    &mut row.tariff_regular_cost,
                    &mut row.tariff_overtime_cost,
                ]
                .into_iter()
                .flatten()
                {
                    s.defer();
                }
                for s in [
                    &mut row.scheduled_capacity_time,
                    &mut row.scheduled_occupancy_time,
                    &mut row.overtime_occupancy_time,
                    &mut row.scheduled_utilization,
                ]
                .into_iter()
                .flatten()
                {
                    s.defer();
                }
                if let Some(q) = &mut row.queue {
                    q.wait_total.stats.defer();
                    q.length_max.defer();
                    q.length_time_average.stats.defer();
                }
            }
        }
        for row in &mut self.people {
            if d.process
                || m.people
                    .iter()
                    .enumerate()
                    .any(|(i, p)| p.name == row.person && d.person(m, crate::ir::PersonId(i)))
            {
                row.work.defer();
                row.peak_tasks.defer();
                if let Some(s) = &mut row.overtime_work {
                    s.defer();
                }
            }
        }
        for row in self.resources.iter_mut().flatten() {
            if d.process
                || m.resources
                    .iter()
                    .enumerate()
                    .any(|(i, r)| r.name == row.resource && d.resource(m, crate::ir::ResourceId(i)))
            {
                row.occupancy_time.defer();
                for s in [
                    &mut row.scheduled_capacity_time,
                    &mut row.scheduled_occupancy_time,
                    &mut row.overtime_occupancy_time,
                    &mut row.scheduled_utilization,
                ]
                .into_iter()
                .flatten()
                {
                    s.defer();
                }
                row.peak_units.defer();
                row.utilization.defer();
            }
        }
        let deferred_cap = |name: &str| {
            m.capabilities.iter().enumerate().any(|(i, c)| {
                name == format!("{}.{}", m.actors[c.actor.0].name, c.name)
                    && d.capability(crate::ir::CapabilityId(i))
            })
        };
        for row in &mut self.capabilities {
            if d.process || deferred_cap(&row.capability) {
                row.start_cost.defer();
            }
        }
        for case in &mut self.cases {
            for task in &mut case.tasks {
                if d.process || deferred_cap(&task.capability) {
                    task.work.defer();
                    for s in [
                        &mut task.tariff_regular_work,
                        &mut task.tariff_overtime_work,
                        &mut task.tariff_regular_cost,
                        &mut task.tariff_overtime_cost,
                    ]
                    .into_iter()
                    .flatten()
                    {
                        s.defer();
                    }
                }
                if d.process {
                    task.wait.defer();
                    for s in [&mut task.predecessor_wait, &mut task.refusal_wait]
                        .into_iter()
                        .flatten()
                    {
                        s.defer();
                    }
                    if let Some(c) = &mut task.wait_causes {
                        for s in [&mut c.calendar, &mut c.condition].into_iter().flatten() {
                            s.defer();
                        }
                        for s in [
                            &mut c.actor_capacity,
                            &mut c.resource,
                            &mut c.executor,
                            &mut c.combined,
                            &mut c.unattributed,
                        ] {
                            s.defer();
                        }
                    }
                }
            }
            if d.process {
                case.work.defer();
                case.observed_cost_total.defer();
                case.first_start_wait.defer();
                for s in [&mut case.external_wait, &mut case.intentional_wait]
                    .into_iter()
                    .flatten()
                {
                    s.defer();
                }
                for terminal in &mut case.terminals {
                    terminal.lead_time.defer();
                    terminal.cost_per_case.defer();
                    if let Some(b) = &mut terminal.breakdown {
                        b.defer();
                    }
                }
                for join in case.joins.iter_mut().flatten() {
                    for branch in &mut join.branches {
                        branch.join_wait.defer();
                    }
                }
                if let Some(dwell) = &mut case.dwell {
                    dwell.until.defer();
                    dwell.breakdown.defer();
                }
            }
        }
        if d.process {
            for terminal in &mut self.terminals {
                terminal.cost_per_case.defer();
            }
            let o = &mut self.overall;
            for s in [
                &mut o.observed_until,
                &mut o.work,
                &mut o.named_people_work,
                &mut o.peak_tasks,
                &mut o.occupancy_cost,
                &mut o.start_cost,
                &mut o.observed_cost_total,
                &mut o.observed_cost_per_arrival,
            ] {
                s.defer();
            }
            for s in [&mut o.queue_wait_observed, &mut o.intentional_wait]
                .into_iter()
                .flatten()
            {
                s.defer();
            }
        }
    }

    /// Check each input before reducing numeric populations. Alignment may
    /// reconcile null reasons, but must never erase a numeric cause observation.
    pub(crate) fn assert_wait_populations(&self) {
        fn numeric(s: &Scalar) -> bool {
            match s {
                Scalar::Range(r) => r.min.is_some(),
                Scalar::Sample(s) => s.min.is_some(),
            }
        }
        for task in self.cases.iter().flat_map(|c| &c.tasks) {
            if !numeric(&task.wait)
                && let Some(c) = &task.wait_causes
            {
                assert!(
                    [
                        &c.actor_capacity,
                        &c.resource,
                        &c.executor,
                        &c.combined,
                        &c.unattributed
                    ]
                    .into_iter()
                    .all(|s| !numeric(s)),
                    "metric invariant: null Task.wait has numeric wait_causes"
                );
                assert!(
                    c.calendar.iter().chain(&c.condition).all(|s| !numeric(s)),
                    "metric invariant: null Task.wait has numeric wait_causes"
                );
            }
        }
    }

    /// Presence transport only; each interpreter computes and reduces its own
    /// numeric columns. Absence is not a zero-valued observation.
    pub(crate) fn union_wait_presence(&mut self, other: &Self) {
        self.assert_wait_populations();
        other.assert_wait_populations();
        if self.overall.queue_wait_observed.is_none() {
            return;
        }
        for case in &mut self.cases {
            if let Some(peer) = other.cases.iter().find(|c| c.case_id == case.case_id)
                && case.dwell.is_none()
            {
                case.dwell = peer.dwell.clone();
            }
        }
        for actor in &mut self.actors {
            if let Some(peer) = other.actors.iter().find(|a| a.actor == actor.actor)
                && let (Some(a), Some(b)) = (&mut actor.queue, &peer.queue)
            {
                a.wait_total.bound = a.wait_total.bound.or(b.wait_total.bound);
                a.length_time_average.bound =
                    a.length_time_average.bound.or(b.length_time_average.bound);
            }
        }
    }

    pub(crate) fn withdraw_wait_bounds(&mut self) {
        if self.overall.queue_wait_observed.is_none() {
            return;
        }
        for case in &mut self.cases {
            case.dwell = None;
        }
        for actor in &mut self.actors {
            if let Some(q) = &mut actor.queue {
                q.wait_total.bound = None;
                q.length_time_average.bound = None;
            }
        }
    }

    /// Keep null-only cause columns on the same legacy population/reason as
    /// Task.wait even when different unstarted leaves have different statuses.
    pub(crate) fn align_wait_reasons(&mut self) {
        self.assert_wait_populations();
        if self.overall.queue_wait_observed.is_none() {
            return;
        }
        for task in self.cases.iter_mut().flat_map(|c| &mut c.tasks) {
            let null = match &task.wait {
                Scalar::Range(r) => r.min.is_none(),
                Scalar::Sample(s) => s.min.is_none(),
            };
            if null && let Some(c) = &mut task.wait_causes {
                c.actor_capacity = task.wait.clone();
                c.resource = task.wait.clone();
                c.executor = task.wait.clone();
                c.combined = task.wait.clone();
                c.unattributed = task.wait.clone();
                for s in [&mut c.calendar, &mut c.condition].into_iter().flatten() {
                    *s = task.wait.clone();
                }
            }
        }
    }
}

impl Scalar {
    fn defer(&mut self) {
        match self {
            Self::Range(r) if r.reason.is_none() || r.reason == Some(Reason::NotConfigured) => {
                r.min = None;
                r.max = None;
                r.reason = Some(Reason::NotConfigured);
            }
            Self::Sample(s) if s.reason.is_none() || s.reason == Some(Reason::NotConfigured) => {
                s.mean = None;
                s.min = None;
                s.max = None;
                s.samples = 0;
                s.reason = Some(Reason::NotConfigured);
            }
            _ => {}
        }
    }
}
impl Breakdown {
    fn defer(&mut self) {
        for s in [&mut self.condition, &mut self.queue.calendar]
            .into_iter()
            .flatten()
        {
            s.defer();
        }
        for s in [
            &mut self.external,
            &mut self.in_service,
            &mut self.contest_route,
            &mut self.queue.actor_capacity,
            &mut self.queue.resource,
            &mut self.queue.executor,
            &mut self.queue.combined,
            &mut self.intentional,
            &mut self.unattributed,
        ] {
            s.defer();
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/waits_private_oracles.rs"]
pub(crate) mod wait_oracles;

#[cfg(test)]
pub(crate) mod wait_test_support {
    use super::*;
    use serde_json::{Value, json};

    pub fn review_family(prefix: &str, mut observe: impl FnMut(&str, &Value) -> Value) {
        let mut count = 0;
        for probe in super::wait_oracles::review_probes()
            .into_iter()
            .filter(|p| p.name.starts_with(prefix))
        {
            count += 1;
            assert_eq!(
                observe(probe.name, &probe.input),
                probe.expected,
                "{}",
                probe.name
            );
            let mut control = probe.input.clone();
            if prefix == "P10" {
                control["wait"] = json!({"min":"0","max":"0","reason":null});
            } else if prefix == "P11" {
                control["cases"][0]["admitted"] = json!(1);
            } else {
                continue;
            }
            assert_eq!(
                observe(probe.name, &control),
                json!({"rejected":false}),
                "{} control",
                probe.name
            );
        }
        assert!(count > 0, "review family must execute probes");
    }

    /// Catch only the named production invariant. An adapter failure must fail
    /// the test, not masquerade as the requested rejection.
    pub fn rejected(f: impl FnOnce() + std::panic::UnwindSafe, reason: &str) -> Value {
        match std::panic::catch_unwind(f) {
            Ok(()) => json!({"rejected":false}),
            Err(payload) => {
                let message = payload
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| payload.downcast_ref::<&str>().copied());
                assert_eq!(message, Some(reason), "unexpected rejection");
                json!({"rejected":true})
            }
        }
    }

    pub fn review_malformed(input: &Value, good: BusinessMetrics) -> Value {
        let mut bad = good.clone();
        let task = &mut bad.cases[0].tasks[input["task"].as_u64().unwrap() as usize];
        task.wait = serde_json::from_value(input["wait"].clone()).unwrap();
        // Preserve the real collector's causes; the oracle asserts the substrate.
        assert_eq!(
            serde_json::to_value(&task.wait_causes).unwrap(),
            input["wait_causes"]
        );
        let points = if input["seam"] == "envelope_finish" {
            vec![bad]
        } else if input["bad_first"] == true {
            vec![bad, good]
        } else {
            vec![good, bad]
        };
        rejected(
            || match input["seam"].as_str().unwrap() {
                "envelope_finish" | "envelope_merge" => {
                    crate::reference::waits_review_envelope(points)
                }
                "engine_merge" => crate::engine::waits_review_merge(points),
                _ => panic!("unknown review seam"),
            },
            "metric invariant: null Task.wait has numeric wait_causes",
        )
    }

    pub fn review_select(name: &str, point: BusinessMetrics) -> Value {
        let p = serde_json::to_value(point).unwrap();
        let c = &p["cases"][0];
        let task = |binding: &str| {
            c["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["binding"] == binding)
                .unwrap()
        };
        match &name[..3] {
            "P09" => json!({"case_combined":c["terminals"][0]["breakdown"]["queue"]["combined"],
                "case_unattributed":c["terminals"][0]["breakdown"]["unattributed"],
                "case_in_service":c["terminals"][0]["breakdown"]["in_service"],
                "x_actor_capacity":task("x")["wait_causes"]["actor_capacity"],
                "x_unattributed":task("x")["wait_causes"]["unattributed"],
                "y_actor_capacity":task("y")["wait_causes"]["actor_capacity"],
                "y_unattributed":task("y")["wait_causes"]["unattributed"]}),
            "P12" => {
                let q = &p["actors"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|a| a["actor"] == "A")
                    .unwrap()["queue"];
                json!({"target_wait":task("target")["wait"],"target_wait_causes":task("target")["wait_causes"],
                    "A_wait_total":q["wait_total"]["stats"],"A_length_max":q["length_max"],
                    "A_length_time_average":q["length_time_average"]["stats"]})
            }
            _ => panic!("unknown review selection"),
        }
    }
    pub fn sample(points: Vec<BusinessMetrics>, aborted: bool) -> BusinessMetrics {
        let mut outcome = crate::outcome::SimulationOutcome::default();
        for (i, point) in points.into_iter().enumerate() {
            outcome
                .replications
                .push(crate::outcome::ReplicationOutcome {
                    response_recovery: None,
                    contests: Vec::new(),
                    lost_updates: vec![],
                    admission: None,
                    scenario_observed_until: None,
                    metrics: Some(point),
                    replication: i as u64 + 1,
                    draws: vec![],
                    cases: vec![],
                    cut: None,
                    overcommit: false,
                });
        }
        crate::report::summarize_metrics(&outcome, aborted).unwrap()
    }
    /// Projection only: no interval arithmetic or alternative reducer here.
    pub fn select(name: &str, point: BusinessMetrics) -> Value {
        let p = serde_json::to_value(point).unwrap();
        let c = &p["cases"][0];
        let q = &p["actors"][0]["queue"];
        match &name[..3] {
            "P01" => {
                json!({"unattributed":c["terminals"][0]["breakdown"]["unattributed"],"in_service":c["terminals"][0]["breakdown"]["in_service"]})
            }
            "P02" => {
                json!({"wait_total":q["wait_total"]["stats"],"length_max":q["length_max"],"length_time_average":q["length_time_average"]["stats"],"wait_causes":c["tasks"][0]["wait_causes"]})
            }
            "P03" => {
                json!({"target_actor_capacity":c["tasks"][1]["wait_causes"]["actor_capacity"],"target_unattributed":c["tasks"][1]["wait_causes"]["unattributed"]})
            }
            "P04" => {
                json!({"A_wait_total":q["wait_total"]["stats"],"B_wait_total":p["actors"][1]["queue"]["wait_total"]["stats"],"overall":p["overall"]["queue_wait_observed"]})
            }
            "P05" => {
                json!({"dwell":if c["dwell"].is_object(){json!({"until":c["dwell"]["until"],"in_service":c["dwell"]["breakdown"]["in_service"],"bound":c["dwell"]["bound"]})}else{Value::Null}})
            }
            "P06" => {
                json!({"wait_total":q["wait_total"],"length_time_average":q["length_time_average"],"length_max_has_bound":q["length_max"].get("bound").is_some()})
            }
            "P07" => json!({"length_time_average":q["length_time_average"]["stats"]}),
            "P08" => {
                json!({"terminal_in_service":c["terminals"][0]["breakdown"]["in_service"],"wait_total":q["wait_total"],"dwell":c["dwell"],"join_wait":c["joins"][0]["branches"][0]["join_wait"],"predecessor_wait":c["tasks"][0]["predecessor_wait"],"overall":p["overall"]["queue_wait_observed"]})
            }
            _ => panic!("unknown private probe"),
        }
    }
}

#[cfg(test)]
mod metrics_tests {
    use super::*;
    #[test]
    fn canonical_business_numbers_are_not_probabilities() {
        for text in ["0", "8", "805", "8/3", "36893488147419103230"] {
            let wire = serde_json::to_string(text).unwrap();
            let n: MetricNumber = serde_json::from_str(&wire).unwrap();
            assert_eq!(serde_json::to_string(&n).unwrap(), wire);
        }
        for text in ["-1", "+1", "01", "2/2", "1/0", "1/-2", "1/1", "0/2", "1.5"] {
            assert!(serde_json::from_value::<MetricNumber>(serde_json::json!(text)).is_err());
        }
    }

    #[test]
    fn scalar_rejects_unknown_fields_and_inconsistent_statistics() {
        for value in [
            serde_json::json!({"min":"8","max":"8","reason":null}),
            serde_json::json!({"min":null,"max":null,"reason":"truncated"}),
            serde_json::json!({"mean":"8/3","min":"2","max":"4","samples":3,"reason":null}),
            serde_json::json!({"mean":null,"min":null,"max":null,"samples":0,"reason":"no_samples"}),
        ] {
            let scalar: Scalar = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(scalar).unwrap(), value);
        }
        for value in [
            serde_json::json!({"min":"8","max":"8","reason":null,"extra":0}),
            serde_json::json!({"min":"9","max":"8","reason":null}),
            serde_json::json!({"min":null,"max":null,"reason":null}),
            serde_json::json!({"min":"8","max":null,"reason":"truncated"}),
            serde_json::json!({"mean":"8","min":"2","max":"4","samples":3,"reason":null}),
            serde_json::json!({"mean":"3","min":"2","max":"4","samples":0,"reason":null}),
            serde_json::json!({"mean":null,"min":null,"max":null,"samples":1,"reason":"no_samples"}),
        ] {
            assert!(
                serde_json::from_value::<Scalar>(value.clone()).is_err(),
                "{value}"
            );
        }
    }
}
