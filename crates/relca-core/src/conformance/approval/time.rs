//! Exact observed occupation, not elapsed time, queueing or mental effort.
use super::{
    input::Spec,
    link::{Context, Link, correspondence, push, task_reasons},
    rate::{deferred, prior, status},
};
use crate::{
    conformance::{lifecycle::Task, order::Order},
    ir,
    metrics::MetricNumber,
};
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

pub(super) type Interval = (ir::PersonId, u64, u64);
#[derive(Debug, Serialize)]
pub(super) struct Time {
    pub status: &'static str,
    pub value: Option<MetricNumber>,
    pub reasons: Vec<String>,
    #[serde(skip)]
    pub intervals: Vec<Interval>,
}
pub(super) fn union(intervals: &[Interval]) -> MetricNumber {
    let mut by_person = BTreeMap::<_, Vec<_>>::new();
    for &(person, a, b) in intervals {
        by_person.entry(person).or_default().push((a, b));
    }
    let mut sum = MetricNumber::from(0);
    for ranges in by_person.values_mut() {
        ranges.sort_unstable();
        let mut merged = None;
        for &(a, b) in ranges.iter() {
            match merged {
                Some((start, end)) if a <= end => merged = Some((start, end.max(b))),
                Some((start, end)) => {
                    sum.0 += MetricNumber::from(end - start).0;
                    merged = Some((a, b));
                }
                None => merged = Some((a, b)),
            }
        }
        if let Some((a, b)) = merged {
            sum.0 += MetricNumber::from(b - a).0;
        }
    }
    sum
}
pub(super) fn measure(
    link: &Link<'_>,
    spec: &Spec,
    ctx: &Context<'_>,
    order: &Order,
    orphans: &[&Task],
) -> Time {
    let mut reasons = correspondence(link, spec, ctx);
    if deferred(spec, ctx, true) {
        push(&mut reasons, "not_configured");
    }
    if orphans.iter().any(|t| t.case_id == link.case_id) {
        push(&mut reasons, "unassigned_reinvestigation_task");
    }
    let ids = link
        .declared
        .and_then(|(_, e)| e.reinvestigation_task_ids.as_ref());
    if ids.is_none() {
        push(&mut reasons, "missing_reinvestigation_observation");
    }
    let bounds = link
        .proposal
        .and_then(|t| t.finish)
        .zip(link.approval.and_then(|t| t.start));
    if let Some((p, a)) = bounds {
        prior(p, a, ctx, order, &mut reasons);
    }
    let mut intervals = Vec::new();
    for task in &link.rechecks {
        let operation = task
            .and_then(|t| t.indices.first())
            .map(|i| ctx.events[*i].operation.as_str())
            .unwrap_or("");
        if task.is_some()
            && !spec
                .profile
                .reinvestigation_operations
                .iter()
                .any(|s| s == operation)
        {
            push(&mut reasons, "task_correspondence");
        }
        for s in task_reasons(*task, operation, "missing_reinvestigation_observation", ctx) {
            push(&mut reasons, &s);
        }
        let Some(t) = task else {
            continue;
        };
        let Some((start, finish)) = t.start.zip(t.finish) else {
            continue;
        };
        let s = ctx.events[start].time;
        let f = ctx.events[finish].time;
        if s.earliest != s.latest || f.earliest != f.latest {
            push(&mut reasons, "time_precision");
        }
        if let Some((p, a)) = bounds {
            prior(p, start, ctx, order, &mut reasons);
            // A positive overlap is an unsupported attribution, not a made-up
            // duration clipped at approval start. Ambiguous endpoints stay unknown.
            if f.earliest > ctx.events[a].time.latest {
                push(&mut reasons, "unsupported_reinvestigation_window");
            } else {
                prior(finish, a, ctx, order, &mut reasons);
            }
        }
        if let Some(person) = t.person {
            if s.earliest <= f.earliest {
                intervals.push((person, s.earliest, f.earliest));
            }
        } else {
            push(&mut reasons, "unmapped_actor");
        }
    }
    reasons.sort();
    let state = status(&reasons);
    let value = (state == "known").then(|| union(&intervals));
    if state != "known" {
        intervals.clear();
    }
    Time {
        status: if state == "known" && ids.is_some_and(Vec::is_empty) {
            "declared_empty_observation"
        } else {
            state
        },
        value,
        reasons,
        intervals,
    }
}
pub(super) fn cross_episode_overlap(times: &[Time]) -> bool {
    let mut people = BTreeMap::<_, Vec<_>>::new();
    for (episode, t) in times.iter().enumerate() {
        for &(person, a, b) in &t.intervals {
            if a < b {
                people.entry(person).or_default().push((a, b, episode));
            }
        }
    }
    for ranges in people.values_mut() {
        ranges.sort_unstable();
        let mut ends = BinaryHeap::new();
        let mut active = BTreeMap::<usize, usize>::new();
        for &(a, b, episode) in ranges.iter() {
            while ends.peek().is_some_and(|Reverse((end, _))| *end <= a) {
                let Reverse((_, e)) = ends.pop().unwrap();
                let count = active.get_mut(&e).unwrap();
                *count -= 1;
                if *count == 0 {
                    active.remove(&e);
                }
            }
            if active.keys().any(|e| *e != episode) {
                return true;
            }
            ends.push(Reverse((b, episode)));
            *active.entry(episode).or_default() += 1;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::super::link::{census, with_case};
    use super::*;
    #[test]
    fn i4_frozen_person_minutes_and_independent_missing_observations() {
        for (name, total, k, u) in [
            ("a01_baseline", "7", 3, 0),
            ("a02_ineligible", "7", 4, 0),
            ("a04_null", "5", 2, 1),
            ("a04_empty_control", "5", 3, 0),
            ("a05_missing_start", "5", 2, 1),
            ("a05_missing_finish", "5", 2, 1),
            ("a06_precision", "5", 2, 1),
            ("a07_same_tick", "5", 2, 1),
            ("a07_after_control", "7", 3, 0),
            ("a08_same_person_union", "4", 1, 0),
            ("a08_two_person_control", "5", 1, 0),
            ("a08_alias_control", "4", 1, 0),
            ("a15_orphan_recheck", "5", 2, 1),
            ("a18_defer", "0", 0, 3),
        ] {
            with_case(name, |s, c| {
                let x = census(s, c).unwrap();
                let o = Order::source(c.events, false).unwrap();
                let times: Vec<_> = x
                    .links
                    .iter()
                    .map(|l| measure(l, s, c, &o, &x.orphans))
                    .collect();
                let known: Vec<_> = times.iter().filter_map(|t| t.value.as_ref()).collect();
                let sum = known.iter().fold(MetricNumber::from(0), |mut sum, t| {
                    sum.0 += &t.0;
                    sum
                });
                assert_eq!(
                    (sum.0.to_string(), known.len(), times.len() - known.len()),
                    (total.into(), k, u),
                    "{name}"
                );
                assert!(!cross_episode_overlap(&times), "{name}");
            });
        }
        with_case("a18_cross_episode_overlap", |s, c| {
            let x = census(s, c).unwrap();
            let o = Order::source(c.events, false).unwrap();
            let times: Vec<_> = x
                .links
                .iter()
                .map(|l| measure(l, s, c, &o, &x.orphans))
                .collect();
            assert!(cross_episode_overlap(&times));
        });
    }
}
