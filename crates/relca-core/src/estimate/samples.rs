//! Task projection; conform's verdicts are not estimator inputs.
use crate::{
    conformance::{
        identity::Resolution,
        input::{Event, Header, Lifecycle, Time},
        lifecycle::Task,
    },
    ir,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
pub(super) struct Sample<'a> {
    pub(super) result: Option<&'a str>,
    pub(super) minutes: Option<u64>,
    pub(super) censored: bool,
    pub(super) reason: Option<&'static str>,
    pub(super) lines: (usize, usize),
}

#[derive(Clone, Copy)]
struct Observation<'a> {
    start: Option<Time>,
    eligible: bool,
    order_ambiguous: bool,
    sample: Sample<'a>,
}

#[derive(Default)]
pub(super) struct Observations<'a>(BTreeMap<&'a str, Vec<Observation<'a>>>);

pub(super) fn collect<'a>(
    events: &'a [Event],
    tasks: &'a [Task],
    resolved: &Resolution,
    n_cap: usize,
) -> Vec<Observations<'a>> {
    let mut table: Vec<_> = (0..n_cap).map(|_| Observations::default()).collect();
    for t in tasks {
        let s = t.start.map(|i| &events[i]);
        let f = t.finish.map(|i| &events[i]);
        let mapped = t.indices.iter().all(|&i| resolved.events[i].mapped);
        let identity = t.indices.first().map(|&i| resolved.events[i]);
        let consistent = t.indices.iter().all(|&i| {
            let a = resolved.events[i];
            identity.is_some_and(|b| {
                a.capability == b.capability && a.actor == b.actor && a.person == b.person
            })
        });
        let censored = t.trusted && s.is_some() && f.is_none() && t.ack.is_none();
        let multi_start = t
            .indices
            .iter()
            .filter(|&&i| events[i].lifecycle == Lifecycle::Start)
            .count()
            > 1;
        let reason;
        let mut result = None;
        let mut minutes = None;
        if !mapped {
            reason = Some(
                if t.indices
                    .iter()
                    .any(|&i| resolved.events[i].capability.is_none())
                {
                    "unmapped_operation"
                } else {
                    "unmapped_actor"
                },
            );
        } else if !consistent {
            reason = Some("task_identity_mismatch");
        } else if !t.trusted {
            reason = Some(if f.is_some() && t.ack.is_some() {
                "conflicting_terminal"
            } else {
                "duplicate_phase"
            });
        } else if let Some(s) = s {
            if let Some(f) = f {
                if !t.complete {
                    reason = Some("lifecycle");
                } else if s.time.earliest != s.time.latest || f.time.earliest != f.time.latest {
                    reason = Some("time_precision");
                } else {
                    minutes = f.time.earliest.checked_sub(s.time.earliest);
                    if t.result.is_some() {
                        result = f.result.as_deref();
                    }
                    reason = if minutes.is_none() {
                        Some("lifecycle")
                    } else if f.result.is_none() {
                        Some("missing_result")
                    } else if result.is_none() {
                        Some("result_domain")
                    } else {
                        None
                    };
                }
            } else {
                reason = Some(if censored { "censored" } else { "cancelled" });
            }
        } else {
            reason = Some("missing_start");
        }
        let lines = t.indices.iter().fold((usize::MAX, 0), |(lo, hi), &i| {
            (lo.min(events[i].line), hi.max(events[i].line))
        });
        for &ir::CapabilityId(id) in &t.observed_capabilities {
            table[id]
                .0
                .entry(t.case_id.as_str())
                .or_default()
                .push(Observation {
                    start: s.map(|e| e.time),
                    eligible: mapped && consistent && s.is_some(),
                    order_ambiguous: multi_start,
                    sample: Sample {
                        result,
                        minutes,
                        censored,
                        reason,
                        lines,
                    },
                });
        }
    }
    table
}

#[derive(Default)]
pub(super) struct Group<'a> {
    pub(super) samples: Vec<Sample<'a>>,
    pub(super) omitted: Option<&'static str>,
    pub(super) round_fault: Option<&'static str>,
    pub(super) missing_prefix: bool,
}
impl Group<'_> {
    pub(super) fn event_lines(&self) -> Value {
        event_lines(self.samples.iter().map(|s| s.lines))
    }
}
fn event_lines(lines: impl Iterator<Item = (usize, usize)>) -> Value {
    let (lo, hi) = lines.fold((usize::MAX, 0), |(lo, hi), (a, b)| (lo.min(a), hi.max(b)));
    if hi == 0 {
        Value::Null
    } else {
        json!({"from":lo,"to":hi})
    }
}

impl<'a> Observations<'a> {
    pub(super) fn evidence_lines(&self) -> Value {
        event_lines(self.0.values().flatten().map(|o| o.sample.lines))
    }

    pub(super) fn marginal_counts(&self) -> Value {
        let mut counts = BTreeMap::<_, u32>::new();
        for o in self.0.values().flatten() {
            if o.eligible
                && let Some(r) = o.sample.result
            {
                *counts.entry(r).or_default() += 1;
            }
        }
        json!(
            counts
                .into_iter()
                .map(|(result, hits)| json!({"result":result,"hits":hits}))
                .collect::<Vec<_>>()
        )
    }

    pub(super) fn occurrences(&self, header: &Header) -> Value {
        let mut hist = BTreeMap::<_, usize>::new();
        for case in &header.cases {
            let n = self
                .0
                .get(case.case_id.as_str())
                .map_or(0, |xs| xs.iter().filter(|o| o.eligible).count());
            *hist.entry(n).or_default() += 1;
        }
        json!(
            hist.into_iter()
                .map(|(observations, cases)| json!({"observations":observations,"cases":cases}))
                .collect::<Vec<_>>()
        )
    }

    pub(super) fn ordinary(&self) -> (Group<'a>, bool) {
        let mut group = Group::default();
        let mut multiple = false;
        for obs in self.0.values() {
            let before = group.samples.len();
            for o in obs {
                if o.eligible {
                    group.samples.push(o.sample);
                } else if group.omitted.is_none() {
                    group.omitted = o.sample.reason;
                }
            }
            multiple |= group.samples.len() - before > 1;
        }
        (group, multiple)
    }

    pub(super) fn rounds(
        &self,
        header: &Header,
        limit: usize,
        unknown_operations: &BTreeSet<&str>,
    ) -> Vec<Group<'a>> {
        let mut groups: Vec<_> = (0..limit).map(|_| Group::default()).collect();
        for case in &header.cases {
            let obs = self
                .0
                .get(case.case_id.as_str())
                .map_or(&[][..], Vec::as_slice);
            let mut starts: Vec<_> = obs.iter().filter(|o| o.eligible).collect();
            let omitted = obs
                .iter()
                .find(|o| !o.eligible)
                .and_then(|o| o.sample.reason)
                .or(unknown_operations
                    .contains(case.case_id.as_str())
                    .then_some("unmapped_operation"));
            if omitted.is_some() || case.from_case_start != Some(true) {
                // A missing start can shift the number of all later Task starts.
                // Do not manufacture observations for the unseen predecessors.
                for group in &mut groups {
                    group.missing_prefix = true;
                    group.omitted = group.omitted.or(omitted).or(Some("partial_coverage"));
                }
                continue;
            }
            starts.sort_by_key(|o| o.start.expect("eligible start").earliest);
            let fault = if starts.len() > limit {
                Some("rounds_exceed_model_limit")
            } else if starts.iter().any(|o| o.order_ambiguous)
                || starts.windows(2).any(|pair| {
                    pair[0].start.expect("eligible start").latest
                        >= pair[1].start.expect("eligible start").earliest
                })
            {
                Some("round_order_ambiguous")
            } else {
                None
            };
            let lines = starts.iter().fold((usize::MAX, 0), |(lo, hi), o| {
                (lo.min(o.sample.lines.0), hi.max(o.sample.lines.1))
            });
            for (group, observation) in groups.iter_mut().zip(starts) {
                if let Some(reason) = fault {
                    group.round_fault = group.round_fault.or(Some(reason));
                    group.samples.push(Sample {
                        result: None,
                        minutes: None,
                        censored: false,
                        reason: Some(reason),
                        lines,
                    });
                } else {
                    group.samples.push(observation.sample);
                }
            }
        }
        groups
    }
}
