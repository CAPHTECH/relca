//! Task correspondence; missing observations never synthesize normal finishes.
use super::{
    checks::{Check, Tri},
    identity::Resolution,
    input::{CancelPhase, Event, Lifecycle},
};
use crate::ir;
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Task {
    pub q1_unsupported: bool,
    pub case_id: String,
    pub task_id: String,
    pub indices: Vec<usize>,
    pub capability: Option<ir::CapabilityId>,
    pub observed_capabilities: Vec<ir::CapabilityId>,
    pub person: Option<ir::PersonId>,
    pub start: Option<usize>,
    pub finish: Option<usize>,
    pub ack: Option<usize>,
    pub result: Option<ir::ValueId>,
    pub trusted: bool,
    /// Normal start/finish correspondence, independent of result availability.
    pub complete: bool,
}
pub struct Analysis {
    pub tasks: Vec<Task>,
    pub checks: Vec<Check>,
    pub edges: Vec<(usize, usize)>,
}

pub struct Features {
    pub outside_scope: Vec<&'static str>,
    pub unsupported: Vec<&'static str>,
}

/// ADR 0057's field ledger. No rest pattern: an added IR field must get an
/// explicit scope decision here before any log can be certified with it.
pub fn features(c: &ir::Capability, m: &ir::Model) -> Features {
    let ir::Capability {
        receipt_query,
        qualified_executors,
        performance,
        version_check,
        read_variables,
        atomic,
        uses,
        requires,
        start_writes,
        kernel,
        kernel_reads_latent,
        kernel_writes,
        writes,
        params,
        reads,
        return_type,
        duration_distribution,
        // Identity/span, permission, fixed duration, scalar results, commits
        // and cancellation boundaries are checked below and by the monitors.
        name: _,
        actor: _,
        span: _,
        permission: _,
        duration: _,
        results: _,
        commits: _,
        cancel: _,
    } = c;
    Features {
        // These fields carry static lineage/arguments/resource requests, not
        // lifecycle semantics in the current IR. State-dependent behaviour is
        // rejected separately rather than inferred from these declarations.
        outside_scope: [
            ("uses", !uses.is_empty()),
            ("params", !params.is_empty()),
            ("reads", !reads.is_empty()),
            ("read_variables", !read_variables.is_empty()),
        ]
        .into_iter()
        .filter_map(|(field, present)| present.then_some(field))
        .collect(),
        unsupported: [
            ("recovery_feature_combination", receipt_query.is_some()),
            ("q6_feature_combination", qualified_executors.is_some()),
            ("q6_feature_combination", performance.is_some()),
            ("requires", requires.is_some()),
            ("atomic", atomic.is_some()),
            ("version_check", version_check.is_some()),
            ("start_writes", !start_writes.is_empty()),
            ("writes", !writes.is_empty()),
            ("kernel", kernel.is_some()),
            ("kernel_reads_latent", !kernel_reads_latent.is_empty()),
            ("kernel_writes", !kernel_writes.is_empty()),
            ("duration_distribution", duration_distribution.is_some()),
            ("return_type", m.types[return_type.0].record.is_some()),
            (
                "q1_response",
                m.response_attempts().iter().any(|(call, _)| {
                    let cap = &m.capabilities[call.capability.0];
                    cap.actor == c.actor && cap.name == c.name
                }),
            ),
        ]
        .into_iter()
        .filter_map(|(field, present)| present.then_some(field))
        .collect(),
    }
}

pub fn supported(c: &ir::Capability, m: &ir::Model) -> bool {
    features(c, m).unsupported.is_empty()
}

pub fn analyze(m: &ir::Model, events: &[Event], resolved: &Resolution) -> Analysis {
    analyze_with_q1(m, events, resolved, &std::collections::BTreeSet::new())
}

pub(super) fn analyze_with_q1(
    m: &ir::Model,
    events: &[Event],
    resolved: &Resolution,
    q1: &std::collections::BTreeSet<ir::CapabilityId>,
) -> Analysis {
    let mut groups = BTreeMap::<(&str, &str), Vec<usize>>::new();
    for (i, e) in events.iter().enumerate() {
        groups.entry((&e.case_id, &e.task_id)).or_default().push(i);
    }
    let mut out = Analysis {
        tasks: Vec::new(),
        checks: Vec::new(),
        edges: Vec::new(),
    };
    for ((case_id, task_id), mut indices) in groups {
        // A representative supplies metadata, never an ordering decision. An
        // inconsistent Task is rejected regardless of which record sorts first.
        indices.sort_by(|a, b| events[*a].event_id.cmp(&events[*b].event_id));
        let first = &events[indices[0]];
        let identity = resolved.events[indices[0]];
        let cap = identity.capability.map(|c| &m.capabilities[c.0]);
        let span = cap.map(|c| c.span);
        let mut observed_capabilities: Vec<_> = indices
            .iter()
            .filter_map(|i| resolved.events[*i].capability)
            .collect();
        observed_capabilities.sort();
        observed_capabilities.dedup();
        let mut t = Task {
            q1_unsupported: observed_capabilities.iter().any(|id| q1.contains(id)),
            case_id: case_id.into(),
            task_id: task_id.into(),
            indices: indices.clone(),
            capability: identity.capability,
            observed_capabilities,
            person: identity.person,
            start: None,
            finish: None,
            ack: None,
            result: None,
            trusted: true,
            complete: false,
        };
        let mut phases = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for &i in &indices {
            match events[i].lifecycle {
                Lifecycle::Start => phases[0].push(i),
                Lifecycle::Finish => phases[1].push(i),
                Lifecycle::Cancel => match events[i].cancel_phase.expect("wire phase invariant") {
                    CancelPhase::Request => phases[2].push(i),
                    CancelPhase::Ack => phases[3].push(i),
                },
            }
        }
        t.start = phases[0].first().copied();
        t.finish = phases[1].first().copied();
        t.ack = phases[3].first().copied();
        // Capability identification is independent of Task correspondence.
        // Preserve every identified unsupported feature even if duplicates or
        // unmapped Actors later make the Task untrustworthy.
        for id in &t.observed_capabilities {
            let c = &m.capabilities[id.0];
            let mut unsupported = features(c, m).unsupported;
            if q1.contains(id) {
                unsupported.push("q1_source_contract");
            }
            for field in unsupported {
                out.checks.push(Check::new(
                    format!(
                        "task:{task_id}:capability:{}.{}:{field}",
                        m.actors[c.actor.0].name, c.name
                    ),
                    Some(case_id),
                    Tri::Unknown,
                    Some(&format!("unsupported_capability_{field}")),
                    &indices,
                    events,
                    Some(c.span),
                ));
            }
        }
        let mut emit = |suffix: &str, state, reason, evidence: &[usize]| {
            out.checks.push(Check::new(
                format!("task:{task_id}:{suffix}"),
                Some(case_id),
                state,
                reason,
                evidence,
                events,
                span,
            ));
        };
        if indices.iter().any(|i| !resolved.events[*i].mapped) {
            let reason = if indices
                .iter()
                .any(|i| resolved.events[*i].capability.is_none())
            {
                "unmapped_operation"
            } else {
                "unmapped_actor"
            };
            emit("mapping", Tri::Unknown, Some(reason), &indices);
            t.trusted = false;
        }
        if indices.iter().any(|i| {
            let e = &events[*i];
            e.operation != first.operation
                || resolved.events[*i].actor != identity.actor
                || resolved.events[*i].person != identity.person
        }) {
            emit(
                "identity",
                Tri::Unknown,
                Some("task_identity_mismatch"),
                &indices,
            );
            t.trusted = false;
        }
        if phases.iter().any(|p| p.len() > 1) {
            emit("phase", Tri::Unknown, Some("duplicate_phase"), &indices);
            t.trusted = false;
        }
        if t.finish.is_some() && t.ack.is_some() {
            emit(
                "terminal",
                Tri::Unknown,
                Some("conflicting_terminal"),
                &indices,
            );
            t.trusted = false;
        }
        if !t.trusted {
            out.tasks.push(t);
            continue;
        }
        if t.start.is_none() {
            emit("lifecycle", Tri::Unknown, Some("missing_start"), &indices);
            out.checks.last_mut().expect("emitted").missing_phase = Some("start".into());
            out.tasks.push(t);
            continue;
        }
        let start = t.start.expect("start observed");
        let c = cap.expect("trusted operation");
        // Mapping establishes who was observed, not whether that person has
        // permission. Match the explicit permit union, falling back only when
        // no permit was declared (the same rule as the reference interpreter).
        let allowed = c
            .permission
            .as_ref()
            .or(m.actors[c.actor.0].executors.as_ref());
        let permitted = t.person.zip(allowed).is_some_and(|(p, ps)| ps.contains(&p));
        emit(
            "permission",
            if permitted {
                Tri::Conformant
            } else {
                Tri::Violating
            },
            if permitted { None } else { Some("permission") },
            &indices,
        );
        let request = phases[2].first().copied();
        let mut edges = Vec::new();
        if let Some(f) = t.finish {
            edges.push((start, f));
        }
        if let Some(r) = request {
            edges.push((start, r));
            if let Some(a) = t.ack {
                edges.push((r, a));
            }
        }
        if let Some(a) = t.ack {
            edges.push((start, a));
        }
        let reversed = edges
            .iter()
            .any(|(a, b)| events[*a].time.earliest > events[*b].time.latest);
        if reversed {
            emit("lifecycle", Tri::Violating, Some("lifecycle"), &indices);
            out.tasks.push(t);
            continue;
        }
        out.edges.extend(edges);
        let c = cap.expect("trusted operation");
        if let Some(f) = t.finish {
            t.complete = true;
            emit("lifecycle", Tri::Conformant, None, &indices);
            let a = events[start].time;
            let b = events[f].time;
            if c.duration_distribution.is_none() {
                let (state, reason) = duration(a, b, c.duration);
                emit("duration", state, reason, &[start, f]);
            }
            if let Some(value) = &events[f].result {
                t.result = m.types[c.return_type.0]
                    .members
                    .iter()
                    .position(|s| s == value)
                    .map(ir::ValueId);
                if m.types[c.return_type.0].record.is_some() {
                    // A scalar log result cannot resolve a record value. The
                    // semantics row remains unsupported; person/boundary
                    // evidence is nevertheless independent of that payload.
                } else if t.result.is_none()
                    || (supported(c, m)
                        && t.result.is_some_and(|v| !c.finish_results().contains(&v)))
                {
                    emit("result", Tri::Violating, Some("result_domain"), &[f]);
                } else {
                    emit("result", Tri::Conformant, None, &[f]);
                }
            } else {
                emit("result", Tri::Unknown, Some("missing_result"), &[f]);
            }
        } else if let Some(ack) = t.ack {
            if c.cancel == ir::Cancel::ForbiddenAfterStart {
                emit(
                    "cancel",
                    Tri::Violating,
                    Some("cancel_boundary"),
                    &[start, ack],
                );
            } else if request.is_none() {
                emit(
                    "cancel",
                    Tri::Unknown,
                    Some("missing_cancel_request"),
                    &indices,
                );
            } else {
                let r = request.expect("request observed");
                match c.cancel {
                    ir::Cancel::Unspecified => emit(
                        "cancel",
                        Tri::Unknown,
                        Some("unsupported_cancel_semantics"),
                        &indices,
                    ),
                    ir::Cancel::ForbiddenAfterStart => unreachable!(),
                    ir::Cancel::Immediate | ir::Cancel::Cooperative { .. } => {
                        let rt = events[r].time;
                        let at = events[ack].time;
                        let lo = at.earliest.saturating_sub(rt.latest);
                        let hi = at.latest.saturating_sub(rt.earliest);
                        let (min, max) = match c.cancel {
                            ir::Cancel::Immediate => (0, 0),
                            ir::Cancel::Cooperative { ack_within } => (1, ack_within),
                            ir::Cancel::Unspecified | ir::Cancel::ForbiddenAfterStart => {
                                unreachable!()
                            }
                        };
                        let state = if hi < min || lo > max {
                            Tri::Violating
                        } else if lo >= min && hi <= max {
                            Tri::Conformant
                        } else {
                            Tri::Unknown
                        };
                        emit(
                            "cancel",
                            state,
                            match state {
                                Tri::Conformant => None,
                                Tri::Violating => Some("cancel_boundary"),
                                Tri::Unknown => Some("time_precision"),
                            },
                            &indices,
                        );
                    }
                }
            }
        } else {
            let reason = if request.is_some() {
                "missing_cancel_ack"
            } else {
                "missing_finish"
            };
            emit("lifecycle", Tri::Unknown, Some(reason), &indices);
            out.checks.last_mut().expect("emitted").missing_phase = Some(
                if request.is_some() {
                    "cancel_ack"
                } else {
                    "finish"
                }
                .into(),
            );
        }
        out.tasks.push(t);
    }
    out
}

fn duration(
    a: super::input::Time,
    b: super::input::Time,
    wanted: u64,
) -> (Tri, Option<&'static str>) {
    let min = b.earliest.saturating_sub(a.latest);
    let max = b.latest.saturating_sub(a.earliest);
    if wanted < min || wanted > max {
        (Tri::Violating, Some("duration"))
    } else if min == max {
        (Tri::Conformant, None)
    } else {
        (Tri::Unknown, Some("time_precision"))
    }
}

/// Match only trustworthy observations. A cancellation acknowledgement is
/// neither a normal finish nor a successful approval nor a commit.
pub fn matching(pattern: ir::Event, tasks: &[Task], m: &ir::Model) -> Vec<usize> {
    tasks
        .iter()
        .filter(|t| t.trusted && t.capability == Some(pattern.capability))
        .filter_map(|t| match pattern.kind {
            ir::EventKind::Start => t.start,
            ir::EventKind::Finish(value)
                if t.complete && value.is_none_or(|v| t.result == Some(v)) =>
            {
                t.finish
            }
            ir::EventKind::Commit
                if t.complete
                    && !t.q1_unsupported
                    && supported(&m.capabilities[pattern.capability.0], m)
                    && t.result.is_some_and(|r| {
                        let c = &m.capabilities[pattern.capability.0];
                        c.finish_results().contains(&r) && c.commits_result(r)
                    }) =>
            {
                t.finish
            }
            ir::EventKind::CancelAck => t.ack,
            ir::EventKind::Finish(_) | ir::EventKind::Commit => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::input::Time;
    use super::*;
    #[test]
    fn fixed_duration_uses_all_interval_differences() {
        assert_eq!(
            duration(
                Time {
                    earliest: 0,
                    latest: 1
                },
                Time {
                    earliest: 1,
                    latest: 1
                },
                1
            )
            .0,
            Tri::Unknown
        );
        assert_eq!(
            duration(
                Time {
                    earliest: 0,
                    latest: 0
                },
                Time {
                    earliest: 2,
                    latest: 2
                },
                1
            )
            .0,
            Tri::Violating
        );
        assert_eq!(
            duration(
                Time {
                    earliest: 0,
                    latest: 0
                },
                Time {
                    earliest: 1,
                    latest: 1
                },
                1
            )
            .0,
            Tri::Conformant
        );
    }
}
