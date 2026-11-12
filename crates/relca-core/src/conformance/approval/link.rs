//! Candidate census is a union, never a user-selectable denominator.
use super::input::{Episode, Spec};
use crate::{
    conformance::{
        checks::{Check, Tri},
        input::{Event, Failure, Header},
        lifecycle::{self, Task},
    },
    ir,
};
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::conformance) struct Context<'a> {
    pub model: &'a ir::Model,
    pub header: &'a Header,
    pub events: &'a [Event],
    pub tasks: &'a [Task],
    pub checks: &'a [Check],
}
pub(super) struct Link<'a> {
    pub case_id: &'a str,
    pub approval_id: &'a str,
    pub declared: Option<(usize, &'a Episode)>,
    pub approval: Option<&'a Task>,
    pub proposal: Option<&'a Task>,
    pub rechecks: Vec<Option<&'a Task>>,
    pub indices: Vec<usize>,
}
pub(super) struct Census<'a> {
    pub links: Vec<Link<'a>>,
    pub no_approval: Vec<&'a str>,
    pub orphans: Vec<&'a Task>,
}
pub(super) fn census<'a>(spec: &'a Spec, ctx: &Context<'a>) -> Result<Census<'a>, Failure> {
    let tasks: BTreeMap<_, _> = ctx
        .tasks
        .iter()
        .map(|t| ((t.case_id.as_str(), t.task_id.as_str()), t))
        .collect();
    let mut candidates = BTreeMap::new();
    let mut assigned = BTreeSet::new();
    for (i, e) in spec.episodes.iter().enumerate() {
        if !ctx.header.cases.iter().any(|s| s.case_id == e.case_id) {
            return Err(Failure::invalid("approval_spec_bad_field"));
        }
        candidates.insert(
            (e.case_id.as_str(), e.approval_task_id.as_str()),
            Some((i, e)),
        );
        for id in e.reinvestigation_task_ids.iter().flatten() {
            assigned.insert((e.case_id.as_str(), id.as_str()));
        }
    }
    for e in ctx.events {
        if e.operation == spec.profile.approval_operation {
            candidates.entry((&e.case_id, &e.task_id)).or_insert(None);
        }
    }
    let mut links = Vec::new();
    for ((case_id, approval_id), declared) in candidates {
        let approval = tasks.get(&(case_id, approval_id)).copied();
        let proposal = declared
            .and_then(|(_, e)| e.proposal_task_id.as_deref())
            .and_then(|id| tasks.get(&(case_id, id)).copied());
        let rechecks: Vec<_> = declared
            .into_iter()
            .flat_map(|(_, e)| e.reinvestigation_task_ids.iter().flatten())
            .map(|id| tasks.get(&(case_id, id.as_str())).copied())
            .collect();
        let mut indices = Vec::new();
        for t in approval
            .into_iter()
            .chain(proposal)
            .chain(rechecks.iter().copied().flatten())
        {
            indices.extend_from_slice(&t.indices);
        }
        indices.sort_unstable();
        indices.dedup();
        links.push(Link {
            case_id,
            approval_id,
            declared,
            approval,
            proposal,
            rechecks,
            indices,
        });
    }
    let no_approval = ctx
        .header
        .cases
        .iter()
        .filter(|s| !links.iter().any(|l| l.case_id == s.case_id))
        .map(|s| s.case_id.as_str())
        .collect();
    let orphans = ctx
        .tasks
        .iter()
        .filter(|t| {
            !assigned.contains(&(t.case_id.as_str(), t.task_id.as_str()))
                && t.indices.iter().any(|i| {
                    spec.profile
                        .reinvestigation_operations
                        .contains(&ctx.events[*i].operation)
                })
        })
        .collect();
    Ok(Census {
        links,
        no_approval,
        orphans,
    })
}
pub(super) fn push(reasons: &mut Vec<String>, reason: &str) {
    if !reasons.iter().any(|s| s == reason) {
        reasons.push(reason.into());
    }
}
pub(super) fn task_reasons(
    task: Option<&Task>,
    operation: &str,
    missing: &str,
    ctx: &Context<'_>,
) -> Vec<String> {
    let Some(t) = task else {
        return vec![missing.into()];
    };
    let mut reasons = Vec::new();
    if t.indices
        .iter()
        .any(|i| ctx.events[*i].operation != operation)
    {
        push(&mut reasons, "task_correspondence");
    }
    if !t.trusted {
        push(&mut reasons, "task_correspondence");
    }
    if t.start.is_none() {
        push(&mut reasons, "missing_start");
    }
    if t.finish.is_none() {
        push(
            &mut reasons,
            if t.ack.is_some() {
                "task_correspondence"
            } else {
                "missing_finish"
            },
        );
    }
    if t.ack.is_some() || !t.complete {
        push(&mut reasons, "task_correspondence");
    }
    if t.q1_unsupported
        || t.observed_capabilities
            .iter()
            .any(|c| !lifecycle::supported(&ctx.model.capabilities[c.0], ctx.model))
    {
        push(&mut reasons, "unsupported_approval_semantics");
    }
    for c in ctx.checks.iter().filter(|c| {
        c.id.starts_with("task:")
            && c.case_id.as_deref() == Some(&t.case_id)
            && !c.evidence.is_empty()
            && c.evidence.iter().all(|e| e.task_id == t.task_id)
    }) {
        if c.conformance != Tri::Conformant {
            push(
                &mut reasons,
                c.reason.as_deref().unwrap_or("task_correspondence"),
            );
        }
    }
    reasons
}
pub(super) fn correspondence(link: &Link<'_>, spec: &Spec, ctx: &Context<'_>) -> Vec<String> {
    let Some((_, e)) = link.declared else {
        return vec!["missing_episode_link".into()];
    };
    let mut reasons = task_reasons(
        link.approval,
        &spec.profile.approval_operation,
        "missing_start",
        ctx,
    );
    for s in task_reasons(
        link.proposal,
        &spec.profile.proposal_operation,
        "missing_proposal",
        ctx,
    ) {
        push(&mut reasons, &s);
    }
    if e.proposal_task_id.is_none() {
        push(&mut reasons, "missing_proposal");
    }
    if let Some((p, a)) = link
        .proposal
        .and_then(|t| t.finish)
        .zip(link.approval.and_then(|t| t.start))
    {
        match (&ctx.events[p].output_version, &ctx.events[a].input_version) {
            (Some(p), Some(a)) if p == a => (),
            (Some(_), Some(_)) => push(&mut reasons, "version_mismatch"),
            _ => push(&mut reasons, "missing_version"),
        }
    }
    reasons
}

#[cfg(test)]
pub(super) fn with_case<T>(name: &'static str, f: impl FnOnce(&Spec, &Context<'_>) -> T) -> T {
    let (_, m, text, sidecar) = super::prepared(name);
    let p = super::input::read(sidecar.as_bytes()).unwrap();
    let s = p.spec.unwrap();
    s.validate(&m).unwrap();
    let log = crate::conformance::input::read(text.as_bytes()).unwrap();
    let h = log.header.as_ref().unwrap();
    let ids = crate::conformance::identity::resolve(&m, h, &log.events).unwrap();
    let life = lifecycle::analyze(&m, &log.events, &ids);
    f(
        &s,
        &Context {
            model: &m,
            header: h,
            events: &log.events,
            tasks: &life.tasks,
            checks: &life.checks,
        },
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn i2_union_case_keys_orphans_and_correspondence() {
        for (name, size, absent) in [
            ("a03_unobserved_case", 3, 1),
            ("a10_two_decisions", 2, 0),
            ("a11_missing_link", 3, 0),
            ("a15_header_only", 0, 3),
        ] {
            with_case(name, |s, c| {
                let x = census(s, c).unwrap();
                assert_eq!(
                    (x.links.len(), x.no_approval.len()),
                    (size, absent),
                    "{name}"
                );
            });
        }
        with_case("a15_orphan_recheck", |s, c| {
            let x = census(s, c).unwrap();
            assert_eq!(
                (x.orphans[0].case_id.as_str(), x.orphans[0].task_id.as_str()),
                ("c1", "r1")
            );
        });
        for (name, reason) in [
            ("a09_missing_approval_start", "missing_start"),
            ("a09_cancel_ack", "task_correspondence"),
            ("a10_duplicate_finish", "duplicate_phase"),
            ("a12_version_mismatch", "version_mismatch"),
            ("a15_unmapped", "unmapped_actor"),
        ] {
            with_case(name, |s, c| {
                let x = census(s, c).unwrap();
                assert!(
                    correspondence(&x.links[0], s, c)
                        .iter()
                        .any(|r| r == reason),
                    "{name}"
                );
            });
        }
    }
}
