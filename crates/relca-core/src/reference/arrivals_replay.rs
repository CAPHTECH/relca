//! Independent global prefix driver. It retains reference task intervals and
//! evaluates shared occupancy/FIFO before asking the local reference to step.
use super::replay::{self as local, Result, legal, reference};
use super::*;
use crate::explain::{self as x, Kind, ReplayReason, Stage};
#[cfg(test)]
#[path = "replay_policy_tests.rs"]
mod fixed_policy_tests;
#[cfg(test)]
#[path = "../../tests/support/q5_replay_seam.rs"]
mod q5_replay_seam;

pub(super) fn replay(
    m: &Model,
    steps: &[Stage],
    target: &str,
    case_id: Option<u64>,
    choices: &[Option<x::KernelChoice>],
) -> Result<()> {
    let mut validation = m.clone();
    validation.bounds.cases = m.bounds.cases;
    reference(validate(&validation, Limits::default()))?;
    reference(super::arrivals::validate_observed_writers(m))?;
    let times = m.arrivals.clone().unwrap_or_else(|| vec![0]);
    legal(
        !times.is_empty()
            && times.len() as u64 == m.bounds.cases
            && times.windows(2).all(|p| p[0] <= p[1]),
    )?;
    legal(!steps.is_empty() && steps[0].kind == Kind::Initial && steps[0].time == 0)?;
    let mut frames: Vec<Option<Frame<'_>>> = vec![None; times.len()];
    let mut interpreters: Vec<_> = times
        .iter()
        .enumerate()
        .map(|(k, _)| {
            let mut i = local::interpreter(m);
            i.case_index = k;
            i
        })
        .collect();
    let mut received = vec![false; times.len()];
    let mut admitted = vec![false; times.len()];
    let mut activated = vec![false; times.len()];
    let mut held = vec![false; times.len()];
    let mut overflow = vec![false; times.len()];
    let mut ended = vec![false; times.len()];
    let mut queued: Vec<BTreeMap<BindingId, Tick>> = vec![BTreeMap::new(); times.len()];
    let mut bank = local::initial(m).state_values;
    let state_invariants = m
        .contracts
        .iter()
        .any(|c| matches!(c.kind, ContractKind::StateInvariant { .. }));
    if state_invariants
        || m.state_variables
            .iter()
            .any(|v| matches!(v.kind, StateKind::ActorObserved(_) | StateKind::Rule(_)))
    {
        let initial = &steps[0];
        legal(initial.case_id.is_none())?;
        let mut expected = BTreeMap::new();
        for (n, v) in m.state_variables.iter().enumerate() {
            if super::arrivals::shared_state(m, v.kind) {
                let name = match v.kind {
                    StateKind::ActorObserved(a) | StateKind::Rule(a) => {
                        format!("{}.{}", m.actors[a.0].name, v.name)
                    }
                    _ => v.name.clone(),
                };
                let value = reference_display(m, v, bank[n]);
                expected.insert(
                    (name, v.span.start, v.span.end),
                    (
                        value,
                        if v.kind.is_observed() {
                            x::Visibility::Observed
                        } else {
                            x::Visibility::Latent
                        },
                    ),
                );
            }
        }
        let values: Vec<_> = initial
            .values
            .observed
            .iter()
            .chain(&initial.values.latent)
            .collect();
        legal(
            values.len() == expected.len()
                && values
                    .iter()
                    .map(|v| (&v.name, v.location.span.start, v.location.span.end))
                    .collect::<BTreeSet<_>>()
                    .len()
                    == values.len(),
        )?;
        legal(
            initial
                .values
                .observed
                .iter()
                .all(|v| v.visibility == x::Visibility::Observed)
                && initial
                    .values
                    .latent
                    .iter()
                    .all(|v| v.visibility == x::Visibility::Latent),
        )?;
        legal(values.iter().all(|v| {
            expected.get(&(v.name.clone(), v.location.span.start, v.location.span.end))
                == Some(&(v.value.clone(), v.visibility))
        }))?;
    }
    let (mut now, mut cursor, mut phase) = (0, 0, 0);
    for (index, s) in steps.iter().enumerate().skip(1) {
        legal(s.index == index)?;
        if phase == 6 {
            // Receipt must be followed by the activation layer, even when all
            // arrivals overflow and no process Task can expose the omission.
            legal(s.kind == Kind::Admitted)?;
        }
        if (state_invariants
            || m.state_variables
                .iter()
                .any(|v| matches!(v.kind, StateKind::ActorObserved(_) | StateKind::Rule(_))))
            && matches!(
                s.kind,
                Kind::DurationGate
                    | Kind::Admission
                    | Kind::Initialize
                    | Kind::Receive
                    | Kind::Advance
                    | Kind::Deadline
                    | Kind::Deadlock
            )
        {
            // Global administrative stages carry per-admission observations as
            // events, never a fictitious case snapshot in the stage itself.
            legal(
                s.case_id.is_none() && s.values.observed.is_empty() && s.values.latent.is_empty(),
            )?;
        }
        if index >= Limits::default().steps_per_path
            || s.time > Limits::default().steps_per_path as u64
        {
            return Err(ReplayReason::ReferenceLimit);
        }
        match phase {
            0 => {
                legal(s.kind == Kind::DurationGate && s.time == now)?;
                let mut choices = 0;
                for (k, slot) in frames.iter_mut().enumerate() {
                    let Some(f) = slot else {
                        continue;
                    };
                    let coins = reference(interpreters[k].duration_coins(f))?
                        .ok_or(ReplayReason::ReferenceLimit)?;
                    for (n, q) in coins {
                        choices += 1;
                        let task = &mut f.tasks[n];
                        let event = s
                            .events
                            .iter()
                            .find_map(|event| match event {
                                x::Event::Business(e)
                                    if e.case_id == Some(k as u64 + 1)
                                        && e.binding.as_deref()
                                            == Some(m.site_name(task.call.binding).as_str()) =>
                                {
                                    Some(e)
                                }
                                _ => None,
                            })
                            .ok_or(ReplayReason::IllegalTransition)?;
                        let finish = event.kind == Kind::Finish;
                        legal(if finish {
                            !q.is_zero()
                        } else {
                            event.kind == Kind::DurationGate && q != Rational::one()
                        })?;
                        task.checked_through =
                            Some(now - task.start.ok_or(ReplayReason::IllegalTransition)?);
                        task.finish = finish.then_some(now);
                    }
                }
                legal(choices == s.events.len())?;
                phase = 1;
            }
            1 => {
                legal(s.kind == Kind::Admission && s.time == now)?;
                let mut expected = Vec::new();
                while cursor < times.len() && times[cursor] == now {
                    let waiting = frames
                        .iter()
                        .enumerate()
                        .filter(|(k, f)| {
                            admitted[*k]
                                && !ended[*k]
                                && f.as_ref().is_none_or(|f| f.business.is_none())
                                && f.as_ref()
                                    .is_some_and(|f| !f.tasks.iter().any(|t| t.start.is_some()))
                        })
                        .count();
                    let concurrent = frames
                        .iter()
                        .enumerate()
                        .filter(|(k, f)| {
                            admitted[*k]
                                && !ended[*k]
                                && f.as_ref().is_some_and(|f| f.business.is_none())
                        })
                        .count() as u64;
                    let full = m
                        .admission_queue
                        .is_some_and(|q| waiting as u64 >= q.capacity)
                        || m.concurrent_cases.is_some_and(|q| concurrent >= q.capacity);
                    let action = m
                        .admission_queue
                        .or(m.concurrent_cases)
                        .map_or(Overflow::Reject, |q| q.overflow);
                    if !full || action != Overflow::Reject {
                        let mut f = local::initial(m);
                        for (i, v) in m.state_variables.iter().enumerate() {
                            if super::arrivals::shared_state(m, v.kind) {
                                f.state_values[i] = bank[i];
                            }
                        }
                        f.now = now;
                        frames[cursor] = Some(f);
                        admitted[cursor] = !full;
                        held[cursor] = full
                            && matches!(
                                action,
                                Overflow::WaitExternalFifo | Overflow::WaitExternalPriority(_)
                            );
                        overflow[cursor] = full && matches!(action, Overflow::End(_));
                    } else {
                        ended[cursor] = true;
                    }
                    expected.push(cursor as u64 + 1);
                    cursor += 1;
                }
                let actual: Vec<_> = s
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        x::Event::Business(e) if e.kind == Kind::Admission => e.case_id,
                        _ => None,
                    })
                    .collect();
                legal(actual == expected && actual.len() == s.events.len())?;
                phase = 2;
            }
            2 | 3 => {
                legal(s.kind == Kind::Initialize && s.time == now)?;
                let mut event_at = 0;
                for (k, slot) in frames.iter_mut().enumerate() {
                    let Some(f) = slot else {
                        continue;
                    };
                    if received[k] {
                        continue;
                    }
                    let Some(x::Event::Business(e)) = s.events.get(event_at) else {
                        return Err(ReplayReason::IllegalTransition);
                    };
                    legal(e.kind == Kind::Initialize && e.case_id == Some(k as u64 + 1))?;
                    event_at += 1;
                    let mut values = BTreeMap::new();
                    let declaration_aware = m.actors.iter().any(|a| a.is_per_case())
                        || state_invariants
                        || m.state_variables.iter().any(|v| {
                            matches!(v.kind, StateKind::ActorObserved(_) | StateKind::Rule(_))
                        });
                    let key = |start, end| {
                        if declaration_aware {
                            (start, end)
                        } else {
                            (0, 0)
                        }
                    };
                    while let Some(x::Event::Value(value)) = s.events.get(event_at) {
                        let old = values.insert(
                            (
                                value.name.as_str(),
                                key(value.location.span.start, value.location.span.end),
                            ),
                            value,
                        );
                        if declaration_aware {
                            legal(old.is_none())?;
                        }
                        event_at += 1;
                    }
                    legal(values.len() == m.state_variables.len())?;
                    for (n, var) in m.state_variables.iter().enumerate() {
                        let name = match var.kind {
                            StateKind::ActorObserved(a) | StateKind::Rule(a) => {
                                format!("{}.{}", m.actors[a.0].name, var.name)
                            }
                            _ => var.name.clone(),
                        };
                        let value = values
                            .get(&(name.as_str(), key(var.span.start, var.span.end)))
                            .ok_or(ReplayReason::StateMismatch)?;
                        legal(
                            !declaration_aware
                                || value.visibility
                                    == if var.kind.is_observed() {
                                        x::Visibility::Observed
                                    } else {
                                        x::Visibility::Latent
                                    },
                        )?;
                        let chosen = if let Some(n) = var.case_set_size {
                            local::decode_set(&value.value, n)?
                        } else if var.integer_max.is_some() {
                            StateValue::Integer(
                                value
                                    .value
                                    .parse()
                                    .map_err(|_| ReplayReason::StateMismatch)?,
                            )
                        } else if value.value == "none" {
                            StateValue::None
                        } else {
                            StateValue::Value(ValueId(
                                m.types[var.base.0]
                                    .members
                                    .iter()
                                    .position(|x| *x == value.value)
                                    .ok_or(ReplayReason::StateMismatch)?,
                            ))
                        };
                        let valid = match var.initial {
                            InitialValue::Distribution(id) if phase == 2 => m.initial_distributions
                                [id]
                                .iter()
                                .any(|(x, p)| *x == chosen && !p.is_zero()),
                            InitialValue::Nondet if phase == 3 => {
                                reference_value_valid(m, var, chosen)
                            }
                            _ => f.state_values[n] == chosen,
                        };
                        legal(valid)?;
                        f.state_values[n] = chosen;
                    }
                }
                legal(event_at == s.events.len())?;
                phase += 1;
            }
            4 => {
                legal(s.kind == Kind::Receive && s.time == now)?;
                let mut expected = Vec::new();
                for (k, slot) in frames.iter_mut().enumerate() {
                    if let Some(f) = slot
                        && !received[k]
                    {
                        reference(interpreters[k].record(f, Occurrence::Receipt, None))?;
                        received[k] = true;
                        expected.push(k as u64 + 1);
                    }
                }
                let actual: Vec<_> = s
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        x::Event::Business(e) if e.kind == Kind::Received => e.case_id,
                        _ => None,
                    })
                    .collect();
                legal(actual == expected && actual.len() == s.events.len())?;
                phase = if m.explicit_limits() { 6 } else { 5 };
            }
            _ => {
                if s.kind == Kind::Admitted {
                    legal(m.explicit_limits() && s.time == now && s.case_id.is_none())?;
                    let initial_activation = phase == 6;
                    if !initial_activation {
                        legal(!global_instant(
                            m,
                            &frames,
                            &interpreters,
                            &ended,
                            &queued,
                            &admitted,
                        )?)?;
                    }
                    let mut expected = Vec::new();
                    let order: Vec<_> = if !initial_activation && m.external_priority().is_some() {
                        legal(s.events.len() == 1)?;
                        let Some(x::Event::Business(event)) = s.events.first() else {
                            return Err(ReplayReason::IllegalTransition);
                        };
                        legal(event.kind == Kind::Admitted)?;
                        let k = event
                            .case_id
                            .and_then(|k| k.checked_sub(1))
                            .and_then(|k| usize::try_from(k).ok())
                            .ok_or(ReplayReason::IllegalTransition)?;
                        legal(super::policy::held(m, &held, &times).contains(&k))?;
                        vec![k]
                    } else {
                        (0..frames.len()).collect()
                    };
                    for k in order {
                        if initial_activation {
                            if overflow[k] && !ended[k] {
                                let f =
                                    frames[k].as_mut().ok_or(ReplayReason::IllegalTransition)?;
                                legal(received[k])?;
                                reference(interpreters[k].record(
                                    f,
                                    Occurrence::Terminal(
                                        m.overflow_end.ok_or(ReplayReason::IllegalTransition)?,
                                    ),
                                    None,
                                ))?;
                                reference(
                                    interpreters[k].inspect_trace(f, now, false, true, false),
                                )?;
                                f.control.clear();
                                ended[k] = true;
                                expected.push((k as u64 + 1, Kind::End));
                            }
                            if !admitted[k] || activated[k] {
                                continue;
                            }
                        } else {
                            if !held[k] {
                                continue;
                            }
                            let concurrent = frames
                                .iter()
                                .enumerate()
                                .filter(|(j, f)| {
                                    admitted[*j]
                                        && !ended[*j]
                                        && f.as_ref().is_some_and(|f| f.business.is_none())
                                })
                                .count() as u64;
                            let waiting = frames
                                .iter()
                                .enumerate()
                                .filter(|(j, f)| {
                                    admitted[*j]
                                        && !ended[*j]
                                        && f.as_ref().is_some_and(|f| {
                                            f.business.is_none()
                                                && !f.tasks.iter().any(|t| t.start.is_some())
                                        })
                                })
                                .count() as u64;
                            if m.admission_queue.is_some_and(|q| waiting >= q.capacity)
                                || m.concurrent_cases.is_some_and(|q| concurrent >= q.capacity)
                            {
                                break;
                            }
                            admitted[k] = true;
                            held[k] = false;
                        }
                        legal(received[k] && !activated[k] && !ended[k])?;
                        let f = frames[k].as_mut().ok_or(ReplayReason::IllegalTransition)?;
                        reference(interpreters[k].record(f, Occurrence::Admission, None))?;
                        activated[k] = true;
                        expected.push((k as u64 + 1, Kind::Admitted));
                    }
                    let actual: Vec<_> = s
                        .events
                        .iter()
                        .filter_map(|e| match e {
                            x::Event::Business(e) => e.case_id.map(|id| (id, e.kind)),
                            _ => None,
                        })
                        .collect();
                    legal(
                        actual == expected
                            && actual.len() == s.events.len()
                            && (initial_activation || !expected.is_empty()),
                    )?;
                    phase = 5;
                } else if s.kind == Kind::Deadlock {
                    if m.resource_time.is_some() {
                        legal(!calendar_pending(m, &frames, &queued, &ended, &admitted)?)?;
                    }
                    local::validate_resource_display(
                        m,
                        &frames
                            .iter()
                            .enumerate()
                            .filter_map(|(k, f)| f.as_ref().map(|f| (k as u64 + 1, f)))
                            .collect::<Vec<_>>(),
                        s,
                    )?;
                    legal(
                        s.time == now
                            && cursor == times.len()
                            && !global_instant(
                                m,
                                &frames,
                                &interpreters,
                                &ended,
                                &queued,
                                &admitted,
                            )?,
                    )?;
                    legal(
                        !frames
                            .iter()
                            .flatten()
                            .any(|f| f.tasks.iter().any(|t| t.phase == Phase::Running)),
                    )?;
                    for (k, slot) in frames.iter().enumerate() {
                        if let Some(f) = slot
                            && !ended[k]
                        {
                            interpreters[k].evidence.deadlocked = true;
                            reference(interpreters[k].inspect_trace(f, now, false, true, true))?;
                        }
                    }
                    if target == "diagnostic:W-DEADLOCK-0" {
                        return if index + 1 == steps.len() {
                            Ok(())
                        } else {
                            Err(ReplayReason::ViolationMismatch)
                        };
                    }
                } else if matches!(s.kind, Kind::Advance | Kind::Deadline | Kind::Horizon) {
                    if m.resource_time.is_some() || m.has_condition_wait() {
                        let calendar_pending = m.resource_time.is_some()
                            && calendar_pending(m, &frames, &queued, &ended, &admitted)?;
                        let future = times.get(cursor).is_some()
                            || calendar_pending
                            || frames.iter().flatten().any(|f| {
                                f.sleep().is_some()
                                    || f.tasks.iter().any(|t| t.phase == Phase::Running)
                            });
                        legal(future)?;
                    }
                    legal(
                        (s.time > now
                            || matches!(s.kind, Kind::Deadline | Kind::Horizon) && s.time == now)
                            && s.time <= m.bounds.clock
                            && m.horizon.is_none_or(|d| s.time <= d || d > m.bounds.clock)
                            && times.get(cursor).is_none_or(|t| *t >= s.time),
                    )?;
                    if s.kind == Kind::Deadline {
                        legal(s.time == m.bounds.clock)?;
                    }
                    if s.kind == Kind::Horizon {
                        legal(
                            m.horizon == Some(s.time)
                                && s.time == now
                                && cursor == times.len()
                                && !global_instant(
                                    m,
                                    &frames,
                                    &interpreters,
                                    &ended,
                                    &queued,
                                    &admitted,
                                )?,
                        )?;
                        legal(frames.iter().flatten().all(|f| {
                            !f.tasks.iter().any(|t| {
                                t.phase == Phase::Running
                                    && (t.finish == Some(now) || t.ack == Some(now))
                            })
                        }))?;
                        legal(
                            frames.iter().flatten().any(|f| {
                                f.sleep().is_some()
                                    || f.tasks.iter().any(|t| t.phase == Phase::Running)
                            }) || (m.resource_time.is_some()
                                && calendar_pending(m, &frames, &queued, &ended, &admitted)?),
                        )?;
                        for (k, frame) in frames.iter().enumerate() {
                            if let Some(f) = frame {
                                legal(
                                    reference(interpreters[k].duration_coins(f))?
                                        .is_some_and(|coins| coins.is_empty()),
                                )?;
                            }
                        }
                    }
                    while now < s.time {
                        legal(!global_instant(
                            m,
                            &frames,
                            &interpreters,
                            &ended,
                            &queued,
                            &admitted,
                        )?)?;
                        if m.external_wait() {
                            let concurrent = frames
                                .iter()
                                .enumerate()
                                .filter(|(j, f)| {
                                    admitted[*j]
                                        && !ended[*j]
                                        && f.as_ref().is_some_and(|f| f.business.is_none())
                                })
                                .count() as u64;
                            let waiting = frames
                                .iter()
                                .enumerate()
                                .filter(|(j, f)| {
                                    admitted[*j]
                                        && !ended[*j]
                                        && f.as_ref().is_some_and(|f| {
                                            f.business.is_none()
                                                && !f.tasks.iter().any(|t| t.start.is_some())
                                        })
                                })
                                .count() as u64;
                            legal(
                                !held.iter().any(|h| *h)
                                    || m.admission_queue.is_some_and(|q| waiting >= q.capacity)
                                    || m.concurrent_cases.is_some_and(|q| concurrent >= q.capacity),
                            )?;
                        }
                        now += 1;
                        for (k, slot) in frames.iter_mut().enumerate() {
                            if let Some(f) = slot {
                                f.now = now;
                                if now < s.time {
                                    legal(
                                        !f.tasks.iter().any(|t| {
                                            t.phase == Phase::Running
                                                && (t.finish == Some(now) || t.ack == Some(now))
                                        }) && reference(interpreters[k].duration_coins(f))?
                                            .is_some_and(|c| c.is_empty()),
                                    )?;
                                }
                                reference(interpreters[k].inspect_trace(
                                    f,
                                    now,
                                    s.kind == Kind::Deadline,
                                    false,
                                    false,
                                ))?;
                            }
                        }
                    }
                    if matches!(s.kind, Kind::Deadline | Kind::Horizon) {
                        for (k, slot) in frames.iter().enumerate() {
                            if let Some(f) = slot {
                                reference(
                                    interpreters[k].inspect_trace(f, now, true, false, false),
                                )?;
                            }
                        }
                    }
                    phase = 0;
                } else {
                    let k = s
                        .case_id
                        .and_then(|id| id.checked_sub(1))
                        .and_then(|id| usize::try_from(id).ok())
                        .filter(|k| *k < frames.len())
                        .ok_or(ReplayReason::IllegalTransition)?;
                    legal(
                        received[k]
                            && admitted[k]
                            && (!m.explicit_limits() || activated[k])
                            && !ended[k]
                            && s.time == now,
                    )?;
                    if matches!(s.kind, Kind::Start | Kind::Refuse)
                        || s.kind == Kind::RangeViolation
                            && s.state_range_violation
                                .as_ref()
                                .is_some_and(|w| w.phase == "start")
                    {
                        legal(global_start(m, &frames, &queued, k, s)?)?;
                    }
                    if m.case_priority.is_some()
                        && (matches!(s.kind, Kind::Finish | Kind::ContestResponse)
                            || (s.kind == Kind::RangeViolation
                                && s.state_range_violation
                                    .as_ref()
                                    .is_some_and(|w| w.phase == "finish")))
                    {
                        let f = frames[k].as_ref().ok_or(ReplayReason::IllegalTransition)?;
                        let j = local::task_index(f, m, s)?;
                        let views: Vec<_> = frames.iter().enumerate().map(|(i, frame)|
                            frame.as_ref().filter(|f| admitted[i] && !ended[i] && !f.trace.iter().any(|o|
                                matches!(o.event, Occurrence::ResponseStopped(id) if id.0 < m.process.bindings.len())))).collect();
                        legal(views[k].is_some())?;
                        legal(super::policy::finish_allowed(m, &views, k, j, now))?;
                    }
                    let frozen = if s.kind == Kind::Start {
                        let f = frames[k].as_ref().ok_or(ReplayReason::IllegalTransition)?;
                        let j = local::task_index(f, m, s)?;
                        let cap = &m.capabilities[f.tasks[j].call.capability.0];
                        let mut q = 0;
                        for (other, frame) in frames.iter().enumerate() {
                            if let Some(frame) = frame.as_ref().filter(|_| admitted[other]) {
                                for (index, task) in frame.tasks.iter().enumerate() {
                                    if (other, index) != (k, j)
                                        && task.phase == Phase::Queued
                                        && m.capabilities[task.call.capability.0].actor == cap.actor
                                    {
                                        q += 1;
                                    }
                                }
                            }
                        }
                        let person = s
                            .executor
                            .as_ref()
                            .map(|name| {
                                m.people
                                    .iter()
                                    .position(|p| &p.name == name)
                                    .map(PersonId)
                                    .ok_or(ReplayReason::IllegalTransition)
                            })
                            .transpose()?;
                        Some((
                            j,
                            reference(super::work_profiles::select(
                                m,
                                f.tasks[j].call.capability,
                                person,
                                q,
                            ))?,
                        ))
                    } else {
                        None
                    };
                    let f = frames[k].as_mut().ok_or(ReplayReason::IllegalTransition)?;
                    // A contest route Task's finish is the ContestResponse stage
                    // (ADR 0040); its writes are receipts like any finish.
                    let receipts =
                        if matches!(s.kind, Kind::Start | Kind::Finish | Kind::ContestResponse) {
                            let j = local::task_index(f, m, s)?;
                            let task = &f.tasks[j];
                            let cap = &m.capabilities[task.call.capability.0];
                            if s.kind == Kind::Start {
                                cap.start_writes.clone()
                            } else if reference_stale(task, cap) {
                                Vec::new()
                            } else if let Some(a) = &cap.atomic {
                                if s.actual_result.as_ref().or(s.result.as_ref())
                                    == m.types[cap.return_type.0].members.get(a.commit.0)
                                {
                                    a.writes.clone()
                                } else {
                                    Vec::new()
                                }
                            } else {
                                cap.writes.clone()
                            }
                        } else {
                            Vec::new()
                        };
                    local::apply(m, &mut interpreters[k], f, s, choices[index].as_ref())?;
                    if let Some((j, selector)) = frozen {
                        f.tasks[j].frozen_performance = selector;
                    }
                    if matches!(s.kind, Kind::Queue | Kind::ContestQueue) {
                        for t in &f.tasks {
                            queued[k].entry(t.call.binding).or_insert(now);
                        }
                    }
                    if s.kind == Kind::ExecutionClose
                        || s.kind == Kind::End && m.contests().is_empty()
                    {
                        ended[k] = true;
                    }
                    let shared: Vec<_> = m
                        .state_variables
                        .iter()
                        .enumerate()
                        .filter_map(|(n, var)| {
                            super::arrivals::shared_state(m, var.kind)
                                .then_some((n, f.state_values[n]))
                        })
                        .collect();
                    for (n, value) in &shared {
                        bank[*n] = *value;
                    }
                    for (other, frame) in frames
                        .iter_mut()
                        .enumerate()
                        .filter_map(|(k, f)| f.as_mut().map(|f| (k, f)))
                    {
                        if other != k {
                            let shared_writes: Vec<_> = receipts
                                .iter()
                                .filter(|r| {
                                    super::arrivals::shared_state(
                                        m,
                                        m.state_variables[r.variable.0].kind,
                                    )
                                })
                                .copied()
                                .collect();
                            reference_notify(frame, None, &shared_writes, m);
                        }
                        for (n, value) in &shared {
                            frame.state_values[*n] = *value;
                        }
                    }
                }
            }
        }
        local::validate_resource_display(
            m,
            &frames
                .iter()
                .enumerate()
                .filter_map(|(k, f)| f.as_ref().map(|f| (k as u64 + 1, f)))
                .collect::<Vec<_>>(),
            s,
        )?;
        if m.contracts
            .iter()
            .any(|c| matches!(c.kind, ContractKind::StateInvariant { .. }))
        {
            for (k, frame) in frames.iter_mut().enumerate() {
                if let Some(f) = frame
                    && received[k]
                {
                    if f.state_snapshots
                        .last()
                        .is_none_or(|(_, old)| *old != f.state_values)
                    {
                        f.state_snapshots.push((now, f.state_values.clone()));
                    }
                    reference(interpreters[k].inspect_trace(f, now, false, ended[k], false))?;
                }
            }
        }
        let broken = if let Some(k) = case_id
            .and_then(|k| k.checked_sub(1))
            .and_then(|k| usize::try_from(k).ok())
        {
            interpreters
                .get(k)
                .is_some_and(|i| local::target_broken(i, target))
        } else {
            false
        };
        if broken {
            return if index + 1 == steps.len() {
                let k = case_id.ok_or(ReplayReason::ViolationMismatch)? as usize - 1;
                local::validate_state_invariant_detail(
                    m,
                    frames[k].as_ref().ok_or(ReplayReason::ViolationMismatch)?,
                    steps,
                    target,
                    case_id,
                )?;
                #[cfg(test)]
                super::recovery::observe_replay(
                    m,
                    frames[k].as_ref().ok_or(ReplayReason::ViolationMismatch)?,
                );
                local::validate_response_ledger(
                    &mut interpreters[k],
                    frames[k].as_ref().ok_or(ReplayReason::ViolationMismatch)?,
                    steps,
                    target,
                    case_id,
                )
            } else {
                Err(ReplayReason::ViolationMismatch)
            };
        }
    }
    Err(ReplayReason::ViolationMismatch)
}

fn calendar_pending(
    m: &Model,
    frames: &[Option<Frame<'_>>],
    queued: &[BTreeMap<BindingId, Tick>],
    ended: &[bool],
    admitted: &[bool],
) -> Result<bool> {
    let views: Vec<_> = frames.iter().map(Option::as_ref).collect();
    let active: Vec<_> = frames.iter().enumerate().map(|(i, f)| admitted[i] && !ended[i]
        && f.as_ref().is_some_and(|f| !f.trace.iter().any(|o| matches!(o.event, Occurrence::ResponseStopped(id) if id.0 < m.process.bindings.len())))).collect();
    reference(super::policy::calendar_pending(m, &views, queued, &active))
}

fn global_start(
    m: &Model,
    frames: &[Option<Frame<'_>>],
    queued: &[BTreeMap<BindingId, Tick>],
    k: usize,
    s: &Stage,
) -> Result<bool> {
    let f = frames[k].as_ref().ok_or(ReplayReason::IllegalTransition)?;
    let Some(t) = f.tasks.iter().find(|t| {
        s.binding.as_deref() == Some(m.site_name(t.call.binding).as_str())
            && t.phase == Phase::Queued
    }) else {
        return Ok(false);
    };
    if m.start_policy.as_ref().is_some_and(StartPolicy::extended) {
        let views: Vec<_> = frames.iter().map(Option::as_ref).collect();
        let choices = reference(super::policy::starts(m, &views, queued))?;
        let j = f
            .tasks
            .iter()
            .position(|candidate| std::ptr::eq(candidate, t))
            .ok_or(ReplayReason::IllegalTransition)?;
        let Some(people) = choices.get(&(k, j)) else {
            return Ok(false);
        };
        let unstarted = s.kind == Kind::Refuse
            || s.kind == Kind::RangeViolation
                && s.state_range_violation
                    .as_ref()
                    .is_some_and(|w| w.phase == "start");
        return Ok(if unstarted {
            s.executor.is_none() && !people.is_empty()
        } else {
            people
                .iter()
                .any(|p| p.map(|p| m.people[p.0].name.as_str()) == s.executor.as_deref())
        });
    }
    let actor = m.capabilities[t.call.capability.0].actor;
    if let Some(qualified) = &m.capabilities[t.call.capability.0].qualified_executors
        && (qualified.is_empty()
            || s.executor
                .as_ref()
                .is_some_and(|name| !qualified.iter().any(|p| &m.people[p.0].name == name)))
    {
        return Ok(false);
    }
    let units = reference(reference_resource_units(
        frames.iter().flatten().flat_map(|f| f.tasks.iter()),
        m,
    ))?;
    for claim in &m.capabilities[t.call.capability.0].uses {
        let limit = m.resources[claim.resource.0].capacity;
        if units[claim.resource.0] > limit || claim.amount > limit - units[claim.resource.0] {
            return Ok(false);
        }
    }
    if s.kind == Kind::Refuse
        && let Some(people) = &m.actors[actor.0].executors
    {
        let any_free = people.iter().any(|p| {
            m.capabilities[t.call.capability.0]
                .qualified_executors
                .as_ref()
                .is_none_or(|qs| qs.contains(p))
                && super::policy::working(m, *p, f.now)
                && !frames
                    .iter()
                    .flatten()
                    .flat_map(|f| &f.tasks)
                    .any(|t| t.phase == Phase::Running && t.person == Some(*p))
        });
        if !any_free {
            return Ok(false);
        }
    }
    let mut occupied = 0;
    let key = (
        queued[k]
            .get(&t.call.binding)
            .copied()
            .ok_or(ReplayReason::IllegalTransition)?,
        k,
    );
    for (other, frame) in frames.iter().enumerate() {
        if let Some(frame) = frame {
            for t in &frame.tasks {
                if t.phase == Phase::Running {
                    if m.capabilities[t.call.capability.0].actor == actor {
                        occupied += 1;
                    }
                    if t.person
                        .is_some_and(|p| Some(m.people[p.0].name.as_str()) == s.executor.as_deref())
                    {
                        return Ok(false);
                    }
                }
                if m.start_policy.is_some()
                    && t.phase == Phase::Queued
                    && m.capabilities[t.call.capability.0].actor == actor
                    && (queued[other][&t.call.binding], other) < key
                {
                    return Ok(false);
                }
            }
        }
    }
    if !super::policy::calendar_slots(
        m,
        &m.capabilities[t.call.capability.0],
        f.now,
        occupied,
        &units,
    ) {
        return Ok(false);
    }
    if let Some(person) = s
        .executor
        .as_ref()
        .and_then(|name| m.people.iter().position(|p| p.name == *name))
        && !super::policy::working(m, PersonId(person), f.now)
    {
        return Ok(false);
    }
    Ok(m.actors[actor.0]
        .capacity
        .is_some_and(|capacity| occupied < capacity))
}
fn global_instant(
    m: &Model,
    frames: &[Option<Frame<'_>>],
    interpreters: &[Interpreter<'_>],
    ended: &[bool],
    queued: &[BTreeMap<BindingId, Tick>],
    admitted: &[bool],
) -> Result<bool> {
    for (k, slot) in frames.iter().enumerate() {
        if let Some(f) = slot {
            if ended[k] || !admitted[k] {
                continue;
            }
            if let Some(wake) = f.sleep() {
                if wake <= f.now {
                    return Ok(true);
                }
                if !crate::contest::has_effects(m) {
                    continue;
                }
            }
            if (f.business.is_some() || crate::contest::has_effects(m))
                && m.contests()
                    .iter()
                    .enumerate()
                    .filter(|(id, _)| f.contest_target(m, *id).is_some())
                    .any(|(id, _)| {
                        f.trace
                            .iter()
                            .find_map(|o| match o.event {
                                Occurrence::ContestChoice(n, filed) if n == id => Some(filed),
                                _ => None,
                            })
                            .is_none_or(|filed| {
                                filed
                                    && !f.trace.iter().any(
                                        |o| matches!(o.event,Occurrence::ContestQueue(n) if n==id),
                                    )
                            })
                    })
            {
                return Ok(true);
            }
            if f.business.is_none() && f.waiting.is_none() && f.sleep().is_none() {
                let statement = f.control.iter().rev().find_map(|b| b.first());
                if let Some(Statement {
                    kind: Stmt::WaitUntil(condition),
                    ..
                }) = statement
                {
                    if reference(interpreters[k].condition(f, condition, 0))? {
                        return Ok(true);
                    }
                } else {
                    return Ok(true);
                }
            }
            if f.tasks.iter().any(|t| {
                t.phase == Phase::Running && (t.finish == Some(f.now) || t.ack == Some(f.now))
            }) {
                return Ok(true);
            }
            if f.business.is_none()
                && f.waiting.as_ref().is_some_and(|w| {
                    w.tasks.clone().all(|n| f.tasks[n].terminal())
                        || !interpreters[k].winners(f, w).is_empty()
                })
            {
                return Ok(true);
            }
            for t in f.tasks.iter().filter(|t| t.phase == Phase::Queued) {
                let actor = &m.actors[m.capabilities[t.call.capability.0].actor.0];
                for executor in actor
                    .executors
                    .as_ref()
                    .map(|ps| {
                        ps.iter()
                            .map(|p| Some(m.people[p.0].name.clone()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|| vec![None])
                {
                    let s = Stage {
                        saved_key: None,
                        evidence_ref: None,
                        command: None,
                        attempt: None,
                        observed_receipt: None,
                        actual_result: None,
                        retry_location: None,
                        state_range_violation: None,
                        index: 0,
                        time: f.now,
                        case_id: Some(k as u64 + 1),
                        binding: Some(m.site_name(t.call.binding)),
                        actor: None,
                        capability: None,
                        executor,
                        result: None,
                        kind: Kind::Start,
                        events: Vec::new(),
                        location: x::Location::new("", diag::Span { start: 0, end: 0 })?,
                        values: x::Values::default(),
                        violation: None,
                        resource: None,
                        capability_location: None,
                        deadlock_detail: None,
                        shared_resources: Vec::new(),
                    };
                    if global_start(m, frames, queued, k, &s)? {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}
