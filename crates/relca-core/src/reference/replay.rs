//! Prefix replay uses the reference tree state and its own control stack.
use super::*;
use crate::explain::{self as x, Kind, ReplayReason, Stage};

pub(super) type Result<T> = std::result::Result<T, ReplayReason>;
pub(super) fn legal(valid: bool) -> Result<()> {
    valid.then_some(()).ok_or(ReplayReason::IllegalTransition)
}
pub(super) fn reference<T>(r: std::result::Result<T, StopReason>) -> Result<T> {
    r.map_err(|e| {
        if e == StopReason::EnumerationLimit {
            ReplayReason::ReferenceLimit
        } else {
            ReplayReason::IllegalTransition
        }
    })
}
pub(super) fn interpreter(m: &Model) -> Interpreter<'_> {
    Interpreter {
        case_index: 0,
        probability_stack: Vec::new(),
        run_work: None,
        probability_result: None,
        probability_scheduler: false,
        probability_chance: false,
        denominator_bits: 4096,
        duration_numeric_cut: false,
        contract_facts: vec![(false, false); m.contracts.len()],
        model: m,
        limits: Limits::default(),
        evidence: Evidence::default(),
        terminals: BTreeMap::new(),
        statistics: Statistics::default(),
    }
}

pub(super) fn decode_set(value: &x::DisplayValue, n: u8) -> Result<StateValue> {
    let x::DisplayValue::CaseSet(ids) = value else {
        return Err(ReplayReason::StateMismatch);
    };
    legal(ids.windows(2).all(|w| w[0] < w[1]) && ids.iter().all(|i| *i > 0 && *i <= u64::from(n)))?;
    Ok(StateValue::Set(
        ids.iter().fold(0, |bits, id| bits | (1u64 << (id - 1))),
    ))
}
pub(super) fn initial(m: &Model) -> Frame<'_> {
    Frame {
        condition_waits: Vec::new(),
        lost_updates: BTreeSet::new(),
        state_snapshots: Vec::new(),
        business: None,
        state_values: m
            .state_variables
            .iter()
            .map(|v| match v.initial {
                InitialValue::Fixed(x) => x,
                _ => {
                    if v.integer_max.is_some() {
                        StateValue::Integer(0)
                    } else {
                        StateValue::Value(ValueId(0))
                    }
                }
            })
            .collect(),
        trace: Vec::new(),
        now: 0,
        steps: 0,
        control: vec![m.process.body.as_slice()],
        waiting: None,
        tasks: Vec::new(),
        adoption: None,
    }
}
pub(super) fn next_statement<'a>(f: &mut Frame<'a>) -> Result<&'a Statement> {
    while f.control.last().is_some_and(|b| b.is_empty()) {
        f.control.pop();
    }
    let block = f
        .control
        .last_mut()
        .ok_or(ReplayReason::IllegalTransition)?;
    let stmt = &block[0];
    *block = &block[1..];
    Ok(stmt)
}
pub(super) fn task_index(f: &Frame<'_>, m: &Model, s: &Stage) -> Result<usize> {
    f.tasks
        .iter()
        .position(|t| s.binding.as_deref() == Some(m.site_name(t.call.binding).as_str()))
        .ok_or(ReplayReason::IllegalTransition)
}
pub(super) fn same_values(f: &Frame<'_>, m: &Model, s: &Stage) -> bool {
    let declaration_aware = m.actors.iter().any(|a| a.is_per_case())
        || m.contracts
            .iter()
            .any(|c| matches!(c.kind, ContractKind::StateInvariant { .. }))
        || m.state_variables
            .iter()
            .any(|v| matches!(v.kind, StateKind::ActorObserved(_) | StateKind::Rule(_)));
    let key = |start, end| {
        if declaration_aware {
            (start, end)
        } else {
            (0, 0)
        }
    };
    let mut actual = BTreeMap::new();
    for (var, value) in m.state_variables.iter().zip(&f.state_values) {
        actual.insert(
            (
                match var.kind {
                    StateKind::ActorObserved(a) | StateKind::Rule(a) => {
                        format!("{}.{}", m.actors[a.0].name, var.name)
                    }
                    _ => var.name.clone(),
                },
                key(var.span.start, var.span.end),
            ),
            (
                reference_display(m, var, *value),
                if var.kind.is_observed() {
                    x::Visibility::Observed
                } else {
                    x::Visibility::Latent
                },
            ),
        );
    }
    let values: Vec<_> = s.values.observed.iter().chain(&s.values.latent).collect();
    values.len() == actual.len()
        && s.values
            .observed
            .iter()
            .all(|v| v.visibility == x::Visibility::Observed)
        && s.values
            .latent
            .iter()
            .all(|v| v.visibility == x::Visibility::Latent)
        && values
            .iter()
            .map(|v| (&v.name, key(v.location.span.start, v.location.span.end)))
            .collect::<BTreeSet<_>>()
            .len()
            == values.len()
        && values.iter().all(|v| {
            actual.get(&(
                v.name.clone(),
                key(v.location.span.start, v.location.span.end),
            )) == Some(&(v.value.clone(), v.visibility))
        })
}
fn runnable<'a>(i: &Interpreter<'a>, f: &Frame<'a>) -> Result<bool> {
    for (n, t) in f.tasks.iter().enumerate() {
        if reference(i.can_start(f, n))? {
            let actor = &i.model.actors[i.model.capabilities[t.call.capability.0].actor.0];
            if actor.executors.as_ref().is_none_or(|ps| {
                ps.iter().any(|p| {
                    Interpreter::person_available(f, *p) && policy::working(i.model, *p, f.now)
                })
            }) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn due(f: &Frame<'_>) -> bool {
    f.tasks
        .iter()
        .any(|t| t.phase == Phase::Running && (t.finish == Some(f.now) || t.ack == Some(f.now)))
}
pub(super) fn instant<'a>(i: &Interpreter<'a>, f: &Frame<'a>) -> Result<bool> {
    if f.waiting.is_none()
        && f.sleep().is_none()
        && let Some(statement) = f.control.iter().rev().find_map(|b| b.first())
        && let Stmt::WaitUntil(condition) = &statement.kind
        && reference(i.condition(f, condition, 0))?
    {
        return Ok(true);
    }
    if f.sleep().is_some_and(|wake| wake <= f.now) {
        return Ok(true);
    }
    if f.business.is_some() || crate::contest::has_effects(i.model) {
        let management = i
            .model
            .contests()
            .iter()
            .enumerate()
            .filter(|(id, _)| f.contest_target(i.model, *id).is_some())
            .any(|(id, _)| {
                f.trace
                    .iter()
                    .find_map(|o| match o.event {
                        Occurrence::ContestChoice(n, filed) if n == id => Some(filed),
                        _ => None,
                    })
                    .is_none_or(|filed| {
                        filed
                            && !f
                                .trace
                                .iter()
                                .any(|o| matches!(o.event,Occurrence::ContestQueue(n) if n==id))
                    })
            });
        if management || f.business.is_some() {
            return Ok(due(f) || runnable(i, f)? || management);
        }
    }
    Ok(due(f)
        || runnable(i, f)?
        || f.waiting.as_ref().is_some_and(|w| {
            !i.winners(f, w).is_empty() || w.tasks.clone().all(|n| f.tasks[n].terminal())
        }))
}
pub(super) fn target_broken(i: &Interpreter<'_>, target: &str) -> bool {
    i.model
        .contracts
        .iter()
        .enumerate()
        .any(|(n, c)| c.property_id(ClauseId(n)) == target && i.contract_facts[n].1)
        || (target == "safety no_resource_overcommit" && i.evidence.overcommitted)
        || (target == "diagnostic:W-DEADLOCK-0" && i.evidence.deadlocked)
        || (target == "state_in_range" && !i.statistics.ranges.is_empty())
        || i.evidence
            .lost_updates
            .iter()
            .any(|id| i.model.loss_property_id(*id) == target)
}

#[cfg(test)]
pub(crate) fn replay(m: &Model, steps: &[Stage], target: &str, case_id: Option<u64>) -> Result<()> {
    replay_choices(m, steps, target, case_id, &vec![None; steps.len()])
}

pub(crate) fn replay_choices(
    m: &Model,
    steps: &[Stage],
    target: &str,
    case_id: Option<u64>,
    choices: &[Option<x::KernelChoice>],
) -> Result<()> {
    #[cfg(test)]
    super::recovery::clear_replay_observation();
    validate_target(steps, target)?;
    validate_d1_target(m, steps, target, case_id)?;
    validate_authority_target(m, steps, target)?;
    legal(choices.len() == steps.len())?;
    if !m.legacy_route() {
        return super::arrivals_replay::replay(m, steps, target, case_id, choices);
    }
    if crate::contest::has_effects(m)
        || m.state_variables
            .iter()
            .any(|v| matches!(v.kind, StateKind::ActorObserved(_) | StateKind::Rule(_)))
    {
        legal(
            case_id.is_none_or(|id| id == 1)
                && steps.iter().all(|s| {
                    s.case_id
                        == if matches!(
                            s.kind,
                            Kind::Advance | Kind::Deadlock | Kind::Deadline | Kind::Horizon
                        ) {
                            None
                        } else {
                            Some(1)
                        }
                }),
        )?;
    }
    let mut i = interpreter(m);
    reference(validate(m, i.limits))?;
    legal(!steps.is_empty() && steps[0].kind == Kind::Initial && steps[0].time == 0)?;
    let mut f = initial(m);
    if !m
        .state_variables
        .iter()
        .any(|v| !matches!(v.initial, InitialValue::Fixed(_)))
    {
        reference(i.record(&mut f, Occurrence::Receipt, None))?;
    }
    if !same_values(&f, m, &steps[0]) {
        return Err(ReplayReason::StateMismatch);
    }
    if target_broken(&i, target)
        && steps.len() != 1
        && m.contracts.iter().enumerate().any(|(n, c)| {
            matches!(c.kind, ContractKind::StateInvariant { .. })
                && c.property_id(ClauseId(n)) == target
        })
    {
        return Err(ReplayReason::ViolationMismatch);
    }
    for (index, stage) in steps.iter().enumerate().skip(1) {
        legal(stage.index == index)?;
        if index >= i.limits.steps_per_path || stage.time > i.limits.steps_per_path as u64 {
            return Err(ReplayReason::ReferenceLimit);
        }
        apply(m, &mut i, &mut f, stage, choices[index].as_ref())?;
        validate_resource_display(m, &[(1, &f)], stage)?;
        if target_broken(&i, target) && index + 1 != steps.len() {
            return Err(ReplayReason::ViolationMismatch);
        }
    }
    if target_broken(&i, target) {
        validate_state_invariant_detail(m, &f, steps, target, case_id)?;
        validate_response_ledger(&mut i, &f, steps, target, case_id)?;
        Ok(())
    } else {
        Err(ReplayReason::ViolationMismatch)
    }
}

/// Recompute from reference state, not the writer Stage or supplied detail.
pub(super) fn validate_state_invariant_detail(
    m: &Model,
    f: &Frame<'_>,
    steps: &[Stage],
    target: &str,
    case: Option<u64>,
) -> Result<()> {
    let Some(clause) = m
        .contracts
        .iter()
        .enumerate()
        .find(|(n, c)| c.property_id(ClauseId(*n)) == target)
        .map(|(_, c)| c)
    else {
        return Ok(());
    };
    let ContractKind::StateInvariant {
        antecedent,
        condition,
    } = &clause.kind
    else {
        return Ok(());
    };
    let last = steps.last().ok_or(ReplayReason::ViolationMismatch)?;
    let violation = last
        .violation
        .as_ref()
        .ok_or(ReplayReason::ViolationMismatch)?;
    let detail = violation
        .state_invariant
        .as_ref()
        .ok_or(ReplayReason::ViolationMismatch)?;
    legal(
        Some(detail.case_id) == case
            && violation.trigger_step.is_none()
            && violation.deadline.is_none()
            && violation.at_step == last.index
            && violation
                .property_location
                .as_ref()
                .is_some_and(|l| l.span == clause.span),
    )?;
    // Membership atoms read the explicit case context of the detail (ADR0035).
    let k = case
        .and_then(|c| c.checked_sub(1))
        .and_then(|c| usize::try_from(c).ok())
        .ok_or(ReplayReason::ViolationMismatch)?;
    let a = match antecedent {
        Some(c) => reference(snapshot_condition_at(c, &[], &f.state_values, k))?,
        None => true,
    };
    let b = reference(snapshot_condition_at(condition, &[], &f.state_values, k))?;
    legal(a && !b && detail.antecedent == a && detail.condition == b)?;
    fn collect(c: &Cond, ids: &mut BTreeSet<StateVarId>) {
        match c {
            Cond::StateEq(t) => {
                ids.insert(t.variable);
            }
            Cond::IntCompare { variable, .. } | Cond::Contains { set: variable, .. } => {
                ids.insert(*variable);
            }
            Cond::And(cs) | Cond::Or(cs) => {
                for c in cs {
                    collect(c, ids);
                }
            }
            Cond::Eq(_) => {}
        }
    }
    let mut ids = BTreeSet::new();
    if let Some(c) = antecedent {
        collect(c, &mut ids);
    }
    collect(condition, &mut ids);
    legal(detail.values.len() == ids.len())?;
    for (id, row) in ids.into_iter().zip(&detail.values) {
        let var = &m.state_variables[id.0];
        let name = match var.kind {
            StateKind::ActorLatent(a) | StateKind::ActorObserved(a) | StateKind::Rule(a) => {
                format!("{}.{}", m.actors[a.0].name, var.name)
            }
            _ => var.name.clone(),
        };
        let same = match (f.state_values[id.0], var.case_set_size) {
            (StateValue::Set(bits), Some(n)) => decode_set(&row.value, n)? == StateValue::Set(bits),
            (StateValue::Set(_), None) | (_, Some(_)) => false,
            (StateValue::None, None) => row.value == "none",
            (StateValue::Integer(n), None) => row.value == n.to_string(),
            (StateValue::Value(v), None) => row.value == m.types[var.base.0].members[v.0],
        };
        let visibility = if var.kind.is_observed() {
            x::Visibility::Observed
        } else {
            x::Visibility::Latent
        };
        legal(
            row.name == name
                && same
                && row.visibility == visibility
                && row.location.span == var.span,
        )?;
        // Coordinates must also agree with the captured declaration, independently
        // source-checked by the explain entry point before this state replay.
        if let Some(original) = steps
            .iter()
            .flat_map(|s| s.values.observed.iter().chain(&s.values.latent))
            .find(|v| v.location.span == var.span)
        {
            legal(row.location == original.location)?;
        }
    }
    Ok(())
}

fn validate_authority_target(m: &Model, steps: &[Stage], target: &str) -> Result<()> {
    let Some((_, clause)) = m
        .contracts
        .iter()
        .enumerate()
        .find(|(n, c)| c.property_id(ClauseId(*n)) == target)
    else {
        return Ok(());
    };
    let ContractKind::AuthorityBasis { authority, .. } = clause.kind else {
        return Ok(());
    };
    let a = m
        .authorities
        .get(authority.0)
        .ok_or(ReplayReason::ViolationMismatch)?;
    let last = steps.last().ok_or(ReplayReason::ViolationMismatch)?;
    let v = last
        .violation
        .as_ref()
        .ok_or(ReplayReason::ViolationMismatch)?;
    legal(v.deadline == a.restrictions.effective_expires && v.trigger_step.is_none())?;
    // Stage snapshots/executor/time are independently compared to the replayed
    // observation by apply (also through the arrivals replay route).
    legal(
        v.property_location
            .as_ref()
            .is_some_and(|l| l.span == clause.span),
    )
}

pub(crate) fn validate_authority_locations(
    src: &str,
    m: &Model,
    target: &str,
    related: &[x::Location],
) -> Result<()> {
    let Some((_, clause)) = m
        .contracts
        .iter()
        .enumerate()
        .find(|(n, c)| c.property_id(ClauseId(*n)) == target)
    else {
        return Ok(());
    };
    let ContractKind::AuthorityBasis { authority, .. } = clause.kind else {
        return Ok(());
    };
    let mut expected = Vec::new();
    let mut visited = BTreeSet::new();
    let mut next = Some(authority);
    while let Some(id) = next {
        legal(visited.insert(id))?;
        let a = m
            .authorities
            .get(id.0)
            .ok_or(ReplayReason::SourceMappingFailed)?;
        for span in [
            Some(a.span),
            a.restrictions.scope_span,
            a.restrictions.expires_span,
            a.restrictions.delegated_span,
        ]
        .into_iter()
        .flatten()
        {
            let location = x::Location::new(src, span)?;
            if !expected.contains(&location) {
                expected.push(location);
            }
        }
        next = a.restrictions.delegated_from;
    }
    if expected != related {
        return Err(ReplayReason::SourceMappingFailed);
    }
    Ok(())
}

pub(crate) fn validate_state_invariant_locations(
    src: &str,
    m: &Model,
    target: &str,
    steps: &[Stage],
    related: &[x::Location],
    choice: Option<&x::KernelChoice>,
) -> Result<()> {
    let Some(clause) = m
        .contracts
        .iter()
        .enumerate()
        .find(|(n, c)| c.property_id(ClauseId(*n)) == target)
        .map(|(_, c)| c)
    else {
        return Ok(());
    };
    if !matches!(clause.kind, ContractKind::StateInvariant { .. }) {
        return Ok(());
    }
    let expected = x::state_invariant_locations(src, m, target, steps, choice)?;
    if expected != related {
        return Err(ReplayReason::SourceMappingFailed);
    }
    let violation = steps
        .last()
        .and_then(|s| s.violation.as_ref())
        .ok_or(ReplayReason::ViolationMismatch)?;
    legal(violation.property_location.as_ref() == Some(&x::Location::new(src, clause.span)?))?;
    for value in &violation
        .state_invariant
        .as_ref()
        .ok_or(ReplayReason::ViolationMismatch)?
        .values
    {
        legal(value.location == x::Location::new(src, value.location.span)?)?;
    }
    Ok(())
}

pub(super) fn validate_response_ledger<'a>(
    i: &mut Interpreter<'a>,
    f: &Frame<'a>,
    steps: &[Stage],
    target: &str,
    case_id: Option<u64>,
) -> Result<()> {
    if !i.model.tracks_obligations()
        || !i
            .model
            .contracts
            .iter()
            .enumerate()
            .any(|(n, c)| c.property_id(ClauseId(n)) == target && c.response().is_some())
    {
        return Ok(());
    }
    let last = steps.last().ok_or(ReplayReason::ViolationMismatch)?;
    let violation = last
        .violation
        .as_ref()
        .ok_or(ReplayReason::ViolationMismatch)?;
    let trigger = violation
        .trigger_step
        .and_then(|n| steps.get(n))
        .ok_or(ReplayReason::ViolationMismatch)?;
    let reason = match last.kind {
        Kind::End => "terminal_unresolved",
        Kind::Deadlock => "deadlock_unresolved",
        Kind::Deadline => "clock",
        Kind::Horizon => "horizon",
        _ => "aborted",
    };
    i.statistics.obligations.clear();
    reference(i.collect_obligations(f, reason))?;
    i.statistics.obligations.iter().any(|row| row.response_id == target
        && trigger.events.iter().any(|event| matches!(event, x::Event::Business(e) if e.case_id == case_id && e.binding.as_deref().unwrap_or("received") == row.trigger_site))
        && row.outcomes.iter().any(|outcome| outcome.status == crate::obligation::Status::Violated && outcome.triggered_at == trigger.time && Some(outcome.deadline) == violation.deadline)).then_some(()).ok_or(ReplayReason::ViolationMismatch)
}

pub(super) fn apply<'a>(
    m: &'a Model,
    i: &mut Interpreter<'a>,
    f: &mut Frame<'a>,
    stage: &Stage,
    choice: Option<&x::KernelChoice>,
) -> Result<()> {
    response_metadata(m, stage)?;
    if stage.kind == Kind::ContestResponse {
        let n = task_index(f, m, stage)?;
        legal(m.site_contest(f.tasks[n].call.binding).is_some())?;
        let mut finish = stage.clone();
        finish.kind = Kind::Finish;
        return apply(m, i, f, &finish, choice);
    }
    if let Some(choice) = choice {
        legal(
            stage.kind == Kind::Finish
                && stage.binding.as_deref() == Some(choice.binding.as_str())
                && stage.case_id == Some(choice.case_id),
        )?;
    }
    if matches!(stage.kind, Kind::Start | Kind::Finish | Kind::CancelAck) {
        validate_task_stage(m, f, stage)?;
    }
    match stage.kind {
        Kind::RangeViolation => {
            let n = task_index(f, m, stage)?;
            let task = &f.tasks[n];
            let cap = &m.capabilities[task.call.capability.0];
            let witness = stage
                .state_range_violation
                .as_ref()
                .ok_or(ReplayReason::ViolationMismatch)?;
            let start = match witness.phase.as_str() {
                "start" => true,
                "finish" => false,
                _ => return Err(ReplayReason::ViolationMismatch),
            };
            legal(
                stage
                    .case_id
                    .is_some_and(|case| case > 0 && case <= m.bounds.cases)
                    && stage.actor.as_deref() == Some(m.actors[cap.actor.0].name.as_str())
                    && stage.capability.as_deref() == Some(cap.name.as_str())
                    && stage.result.is_none()
                    && stage.events.is_empty()
                    && stage.location.span == witness.write_span
                    && stage
                        .capability_location
                        .as_ref()
                        .is_some_and(|l| l.span == cap.span),
            )?;
            if start {
                legal(reference(i.can_start(f, n))?)?;
                if let Some(req) = &cap.requires {
                    legal(reference(i.condition(f, &req.condition, 0))?)?;
                }
            } else {
                legal(task.phase == Phase::Running && task.finish == Some(f.now))?;
            }
            legal(reference(i.range_attempt(
                f,
                n,
                start,
                if m.legacy_route() {
                    None
                } else {
                    stage.case_id
                },
            ))?)?;
            legal(i.statistics.ranges.iter().any(|r| r == witness))?;
        }
        Kind::ContestFiled | Kind::ContestDeclined | Kind::ContestQueue => {
            let id = m
                .contests()
                .iter()
                .enumerate()
                .find_map(|(id, _)| {
                    (stage.binding.as_deref() == Some(m.site_name(m.contest_site(id)).as_str()))
                        .then_some(id)
                })
                .ok_or(ReplayReason::IllegalTransition)?;
            let d = &m.contests()[id];
            let at = f
                .contest_target(m, id)
                .ok_or(ReplayReason::IllegalTransition)?
                .at;
            let cap = &m.capabilities[d.route.0];
            legal(
                stage.time == at
                    && f.now == at
                    && stage.location.span == d.span
                    && stage.actor.as_deref() == Some(m.actors[cap.actor.0].name.as_str())
                    && stage.capability.as_deref() == Some(cap.name.as_str())
                    && stage.executor.is_none()
                    && stage.result.is_none(),
            )?;
            legal(
                stage.events.len() == 1
                    && matches!(&stage.events[0],x::Event::Business(e) if e.kind==stage.kind && e.binding==stage.binding && e.location==stage.location && e.executor.is_none() && e.result.is_none()),
            )?;
            let choice = f.trace.iter().find_map(|o| match o.event {
                Occurrence::ContestChoice(n, filed) if n == id => Some(filed),
                _ => None,
            });
            if stage.kind == Kind::ContestQueue {
                legal(
                    choice == Some(true)
                        && !f
                            .trace
                            .iter()
                            .any(|o| matches!(o.event,Occurrence::ContestQueue(n) if n==id)),
                )?;
                legal(cap.params.is_empty())?;
                reference(i.record(f, Occurrence::ContestQueue(id), None))?;
                if crate::responsibility::included(m, cap.actor) {
                    let start = f
                        .tasks
                        .iter()
                        .position(|t| m.site_contest(t.call.binding).is_some())
                        .unwrap_or(f.tasks.len());
                    let waiting = f.waiting.clone();
                    reference(i.create(
                        f,
                        &[Call {
                            binding: m.contest_site(id),
                            capability: d.route,
                            args: Vec::new(),
                            span: d.span,
                        }],
                        None,
                    ))?;
                    if crate::contest::has_effects(m) {
                        f.waiting = waiting;
                    } else {
                        f.waiting.as_mut().unwrap().tasks.start = start;
                    }
                }
            } else {
                legal(choice.is_none())?;
                at.checked_add(d.respond_by)
                    .ok_or(ReplayReason::IllegalTransition)?;
                reference(i.record(
                    f,
                    Occurrence::ContestChoice(id, stage.kind == Kind::ContestFiled),
                    None,
                ))?;
            }
        }
        Kind::ExecutionClose => {
            f.business.ok_or(ReplayReason::IllegalTransition)?;
            legal(
                !f.trace
                    .iter()
                    .any(|o| matches!(o.event, Occurrence::ExecutionClose))
                    && stage.binding.is_none()
                    && stage.actor.is_none()
                    && stage.capability.is_none()
                    && stage.executor.is_none()
                    && stage.result.is_none()
                    && stage.events.is_empty(),
            )?;
            legal(
                f.tasks.iter().all(Task::terminal)
                    && m.contests()
                        .iter()
                        .enumerate()
                        .filter(|(id, _)| f.contest_target(m, *id).is_some())
                        .all(|(id, _)| {
                            f.trace
                                .iter()
                                .find_map(|o| match o.event {
                                    Occurrence::ContestChoice(n, filed) if n == id => Some(filed),
                                    _ => None,
                                })
                                .is_some_and(|filed| {
                                    !filed
                                        || f.tasks.iter().any(|t| {
                                            m.site_contest(t.call.binding) == Some(id)
                                                && matches!(t.phase, Phase::Finished(_))
                                        })
                                })
                        }),
            )?;
            reference(i.record(f, Occurrence::ExecutionClose, None))?;
            reference(i.inspect_trace(f, f.now, false, true, false))?;
        }
        Kind::Initialize => {
            legal(f.trace.is_empty())?;
            for (n, var) in m.state_variables.iter().enumerate() {
                let name = match var.kind {
                    StateKind::ActorObserved(a) | StateKind::Rule(a) => {
                        format!("{}.{}", m.actors[a.0].name, var.name)
                    }
                    _ => var.name.clone(),
                };
                let value = stage
                    .values
                    .observed
                    .iter()
                    .chain(&stage.values.latent)
                    .find(|v| {
                        v.name == name
                            && (!(m.actors.iter().any(|a| a.is_per_case())
                                || m.state_variables.iter().any(|v| {
                                    matches!(
                                        v.kind,
                                        StateKind::ActorObserved(_) | StateKind::Rule(_)
                                    )
                                }))
                                || v.location.span == var.span)
                    })
                    .ok_or(ReplayReason::StateMismatch)?;
                let chosen = if let Some(n) = var.case_set_size {
                    decode_set(&value.value, n)?
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
                            .position(|v| *v == value.value)
                            .ok_or(ReplayReason::StateMismatch)?,
                    ))
                };
                legal(match var.initial {
                    InitialValue::Fixed(x) => chosen == x,
                    InitialValue::Nondet => reference_value_valid(m, var, chosen),
                    InitialValue::Distribution(id) => m.initial_distributions[id]
                        .iter()
                        .any(|(x, p)| *x == chosen && !p.is_zero()),
                })?;
                f.state_values[n] = chosen;
            }
            reference(i.record(f, Occurrence::Receipt, None))?;
        }
        Kind::Queue => {
            legal(f.waiting.is_none())?;
            let stmt = next_statement(f)?;
            let (calls, join) = match &stmt.kind {
                Stmt::Call(c) | Stmt::Query(c, _) | Stmt::Response(c, _) => {
                    (std::slice::from_ref(c), None)
                }
                Stmt::Fork { branches, join, .. } => (branches.as_slice(), Some(join)),
                _ => return Err(ReplayReason::IllegalTransition),
            };
            legal(
                stage.location.span == stmt.span
                    || (calls.len() == 1 && stage.location.span == calls[0].span),
            )?;
            if let Stmt::Response(call, r) = &stmt.kind
                && let Some(event) = reference(super::response::input(f, m, call, r, i.case_index))?
            {
                reference(i.record(f, event, None))?;
            }
            if calls.iter().any(|c| {
                m.capabilities[c.capability.0].duration == 0
                    && m.capabilities[c.capability.0]
                        .duration_distribution
                        .is_none()
            }) {
                return Err(ReplayReason::ZeroDurationReferenceUnavailable);
            }
            reference(i.create(f, calls, join))?;
        }
        Kind::Start => {
            let n = task_index(f, m, stage)?;
            legal(
                (f.waiting.as_ref().is_some_and(|w| w.tasks.contains(&n))
                    || (crate::contest::has_effects(m)
                        && m.site_contest(f.tasks[n].call.binding).is_some()))
                    && reference(i.can_start(f, n))?,
            )?;
            let cap = &m.capabilities[f.tasks[n].call.capability.0];
            if let Some(req) = &cap.requires {
                legal(reference(i.condition(f, &req.condition, 0))?)?;
            }
            let person = stage
                .executor
                .as_ref()
                .map(|name| {
                    m.people
                        .iter()
                        .position(|p| p.name == *name)
                        .map(PersonId)
                        .ok_or(ReplayReason::IllegalTransition)
                })
                .transpose()?;
            legal(match (&m.actors[cap.actor.0].executors, person) {
                (Some(ps), Some(p)) => {
                    ps.contains(&p)
                        && Interpreter::person_available(f, p)
                        && policy::working(m, p, f.now)
                }
                (None, None) => true,
                _ => false,
            })?;
            f.tasks[n].phase = Phase::Running;
            f.tasks[n].stale.clear();
            f.tasks[n].person = person;
            f.tasks[n].start = Some(f.now);
            f.tasks[n].finish = if cap.duration_distribution.is_some() {
                None
            } else {
                Some(
                    f.now
                        .checked_add(cap.duration)
                        .ok_or(ReplayReason::IllegalTransition)?,
                )
            };
            let capability = f.tasks[n].call.capability;
            for w in &cap.start_writes {
                legal(reference_write_fits(m, w, &f.state_values))?;
                f.state_values[w.variable.0] = reference(reference_effect_at(
                    w,
                    &f.state_values,
                    None,
                    i.case_index,
                    person,
                ))?;
            }
            reference_notify(f, Some(n), &cap.start_writes, m);
            reference(i.record_site(
                f,
                Occurrence::Dispatch(capability),
                person,
                f.tasks[n].call.binding,
            ))?;
        }
        Kind::Refuse => {
            let n = task_index(f, m, stage)?;
            legal(reference(i.can_start(f, n))? && stage.executor.is_none())?;
            let task = &f.tasks[n];
            let cap = &m.capabilities[task.call.capability.0];
            legal(
                m.actors[cap.actor.0]
                    .executors
                    .as_ref()
                    .is_none_or(|ps| ps.iter().any(|p| Interpreter::person_available(f, *p))),
            )?;
            let req = cap
                .requires
                .as_ref()
                .ok_or(ReplayReason::IllegalTransition)?;
            legal(
                !reference(i.condition(f, &req.condition, 0))?
                    && stage.result.as_deref()
                        == Some(m.types[cap.return_type.0].members[req.otherwise.0].as_str()),
            )?;
            legal(!stage.events.iter().any(|e| matches!(e,x::Event::Business(e) if matches!(e.kind,Kind::Start|Kind::Finish|Kind::Commit|Kind::CancelAck))))?;
            let binding = task.call.binding;
            let id = task.call.capability;
            f.tasks[n].phase = Phase::Refused(req.otherwise);
            f.tasks[n].stop = Some(f.now);
            reference(i.record_site(f, Occurrence::Refusal(id, req.otherwise), None, binding))?;
        }
        Kind::ReceiptQuery => {
            let n = task_index(f, m, stage)?;
            let t = &f.tasks[n];
            let Phase::Finished(value) = t.phase else {
                return Err(ReplayReason::IllegalTransition);
            };
            let cap = &m.capabilities[t.call.capability.0];
            let binding = t.call.binding;
            legal(
                cap.receipt_query.is_some()
                    && stage.location.span == t.call.span
                    && stage.executor.is_none()
                    && stage.actor.as_deref() == Some(m.actors[cap.actor.0].name.as_str())
                    && stage.capability.as_deref() == Some(cap.name.as_str())
                    && stage.result.as_deref()
                        == Some(m.types[cap.return_type.0].members[value.0].as_str())
                    && stage.events.is_empty(),
            )?;
            let event = reference(super::recovery::query(
                f,
                m,
                binding,
                value,
                &f.state_values,
                i.case_index,
            ))?;
            let Occurrence::ReceiptQuery(q) = event else {
                return Err(ReplayReason::IllegalTransition);
            };
            let known = m.types[cap.return_type.0].members[value.0] != "still_unknown";
            let evidence = known.then(|| {
                x::recovery_evidence_id(
                    q.case as u64 + 1,
                    &m.site_name(q.command),
                    &m.site_name(q.query),
                    q.at,
                )
            });
            legal(stage.saved_key == Some(q.key as u64 + 1) && stage.evidence_ref == evidence)?;
            reference(i.record_site(f, Occurrence::ReceiptQuery(q), None, binding))?;
        }
        Kind::ResolveEffect => {
            legal(
                f.waiting.is_none()
                    && stage.events.is_empty()
                    && stage.actor.is_none()
                    && stage.capability.is_none()
                    && stage.executor.is_none(),
            )?;
            let statement = next_statement(f)?;
            let Stmt::ResolveEffect { command, evidence } = &statement.kind else {
                return Err(ReplayReason::IllegalTransition);
            };
            legal(
                stage.location.span == statement.span
                    && stage.binding.as_deref() == Some(m.site_name(*evidence).as_str()),
            )?;
            let event = reference(super::recovery::resolve(
                f,
                m,
                *command,
                *evidence,
                statement.span,
                i.case_index,
            ))?;
            let q = f
                .trace
                .iter()
                .find_map(|o| match o.event {
                    Occurrence::ReceiptQuery(q) if q.query == *evidence => Some(q),
                    _ => None,
                })
                .ok_or(ReplayReason::IllegalTransition)?;
            let ty = m.process.bindings[evidence.0].ty;
            legal(
                stage.result.as_deref() == Some(m.types[ty.0].members[q.value.0].as_str())
                    && stage.saved_key == Some(q.key as u64 + 1)
                    && stage.evidence_ref.as_deref()
                        == Some(
                            x::recovery_evidence_id(
                                q.case as u64 + 1,
                                &m.site_name(q.command),
                                &m.site_name(q.query),
                                q.at,
                            )
                            .as_str(),
                        ),
            )?;
            reference(i.record(f, event, None))?;
        }
        Kind::Wait => {
            if f.sleep().is_some()
                && m.response_attempts()
                    .iter()
                    .any(|(_, r)| r.retry_span == Some(stage.location.span))
            {
                legal(f.waiting.is_none())?;
                return legal(stage.time == f.now && same_values(f, m, stage));
            }
            legal(f.waiting.is_none() && f.sleep().is_none())?;
            while f.control.last().is_some_and(|b| b.is_empty()) {
                f.control.pop();
            }
            if let Some(stmt) = f.control.last().and_then(|b| b.first())
                && let Stmt::WaitUntil(condition) = &stmt.kind
            {
                legal(
                    stage.location.span == stmt.span && !reference(i.condition(f, condition, 0))?,
                )?;
                f.condition_enter();
                return legal(stage.time == f.now && same_values(f, m, stage));
            }
            let stmt = next_statement(f)?;
            let Stmt::Wait(duration) = stmt.kind else {
                return Err(ReplayReason::IllegalTransition);
            };
            legal(stage.location.span == stmt.span)?;
            let wake = f
                .now
                .checked_add(duration)
                .ok_or(ReplayReason::IllegalTransition)?;
            reference(i.record(f, Occurrence::Wait(wake), None))?;
        }
        Kind::Wake => {
            legal(f.sleep() == Some(f.now))?;
            reference(i.record(f, Occurrence::Wake, None))?;
        }
        Kind::Advance | Kind::Deadline => {
            let condition_waiting = f
                .control
                .iter()
                .rev()
                .find_map(|b| b.first())
                .is_some_and(|s| matches!(&s.kind, Stmt::WaitUntil(_)));
            legal(
                f.waiting.is_some()
                    || condition_waiting
                    || f.sleep().is_some()
                    || (crate::contest::has_effects(m)
                        && f.business.is_some()
                        && f.tasks.iter().any(|t| t.phase == Phase::Running)),
            )?;
            legal(stage.time >= f.now)?;
            if stage.kind == Kind::Advance {
                legal(stage.time > f.now)?;
            }
            while f.now < stage.time {
                legal(!instant(i, f)?)?;
                f.now += 1;
                if f.now < stage.time {
                    legal(
                        !due(f) && reference(i.duration_coins(f))?.is_some_and(|cs| cs.is_empty()),
                    )?;
                }
            }
            reference(i.inspect_trace(f, f.now, stage.kind == Kind::Deadline, false, false))?;
        }
        Kind::Finish | Kind::CancelAck => {
            let n = task_index(f, m, stage)?;
            let t = &f.tasks[n];
            legal(t.phase == Phase::Running)?;
            let cap_id = t.call.capability;
            let person = t.person;
            let cap = &m.capabilities[cap_id.0];
            let selected_kernel = reference(super::work_profiles::kernel(m, t))?;
            if stage.kind == Kind::CancelAck {
                legal(t.ack == Some(f.now) && stage.result.is_none())?;
                legal(!stage.events.iter().any(|e| matches!(e,x::Event::Business(e) if matches!(e.kind,Kind::Finish|Kind::Commit))))?;
                f.tasks[n].phase = Phase::Cancelled;
                if m.tracks_obligations() {
                    reference(i.record_site(
                        f,
                        Occurrence::CancelAck(cap_id),
                        person,
                        f.tasks[n].call.binding,
                    ))?;
                }
            } else {
                legal(t.finish == Some(f.now))?;
                let result = ValueId(
                    m.types[cap.return_type.0]
                        .members
                        .iter()
                        .position(|v| {
                            Some(v) == stage.actual_result.as_ref().or(stage.result.as_ref())
                        })
                        .ok_or(ReplayReason::EventMismatch)?,
                );
                let mut branches = Vec::new();
                if reference_stale(t, cap) {
                    if result == cap.version_check.as_ref().unwrap().reject && choice.is_none() {
                        branches.push(Vec::new());
                    }
                } else if let Some(kernel) = selected_kernel {
                    legal(choice.is_none_or(|c| c.frozen_performance == t.frozen_performance))?;
                    let (row_index, row) = kernel
                        .rows
                        .iter()
                        .enumerate()
                        .find(|(_, r)| {
                            r.values
                                .iter()
                                .zip(&kernel.given)
                                .all(|(x, v)| f.state_values[v.0] == *x)
                        })
                        .ok_or(ReplayReason::IllegalTransition)?;
                    if let Some(choice) = choice {
                        legal(choice.row == row_index && choice.branch < row.branches.len())?;
                    }
                    for (branch_index, b) in row.branches.iter().enumerate() {
                        if !b.probability.is_zero()
                            && b.result == result
                            && choice.is_none_or(|c| c.branch == branch_index)
                        {
                            branches.push(b.overrides.clone());
                        }
                    }
                } else if cap.receipt_query.is_some() {
                    if reference(super::recovery::project(
                        m,
                        cap_id,
                        &f.state_values,
                        i.case_index,
                    ))?
                    .contains(&result)
                        && choice.is_none()
                    {
                        branches.push(Vec::new());
                    }
                } else if let Some(a) = &cap.atomic {
                    let expected = if reference(i.condition(f, &a.condition, 0))? == a.commit_when {
                        a.commit
                    } else {
                        a.reject
                    };
                    if result == expected && choice.is_none() {
                        branches.push(Vec::new());
                    }
                } else if cap.results.contains(&result) && choice.is_none() {
                    branches.push(Vec::new());
                }
                for w in if reference_stale(t, cap) {
                    &[][..]
                } else {
                    cap.atomic.as_ref().map_or(cap.writes.as_slice(), |a| {
                        if result == a.commit {
                            a.writes.as_slice()
                        } else {
                            &[]
                        }
                    })
                } {
                    legal(reference_write_fits(m, w, &f.state_values))?;
                }
                let mut selected = None;
                for overrides in branches {
                    let mut candidate = f.clone();
                    candidate.tasks[n].phase = Phase::Finished(result);
                    i.finish_state(&mut candidate, n, result);
                    for (id, x) in overrides {
                        candidate.state_values[id.0] = x;
                    }
                    if same_values(&candidate, m, stage) {
                        selected = Some(candidate);
                        break;
                    }
                }
                *f = selected.ok_or(ReplayReason::StateMismatch)?;
                #[cfg(test)]
                crate::q6_acceptance::replayed(f.tasks[n].frozen_performance.clone());
                reference(i.record_site(
                    f,
                    Occurrence::Completion(cap_id, result),
                    person,
                    f.tasks[n].call.binding,
                ))?;
                if let Some(r) = m.response_attempt(f.tasks[n].call.binding) {
                    let observation = stage.observed_receipt.ok_or(ReplayReason::EventMismatch)?;
                    let was_unknown=f.trace.iter().any(|o|matches!(&o.event,Occurrence::Channel(old) if old.command==r.command && old.attempt<r.attempt && !old.resolved));
                    let receipt = super::response::Receipt {
                        #[cfg(test)]
                        weight: None,
                        command: r.command,
                        binding: f.tasks[n].call.binding,
                        attempt: r.attempt,
                        actual: result,
                        observation,
                        resolved: observation == crate::ast::Receipt::Delivered
                            && (r.idempotent || !was_unknown),
                    };
                    reference(i.record_site(
                        f,
                        Occurrence::Channel(receipt),
                        person,
                        f.tasks[n].call.binding,
                    ))?;
                }
            }
            f.tasks[n].stop = Some(f.now);
        }
        Kind::Adopt | Kind::CancelRequest => {
            if stage.kind == Kind::CancelRequest {
                let n = task_index(f, m, stage)?;
                let c = &m.capabilities[f.tasks[n].call.capability.0];
                legal((!c.uses.is_empty() || c.version_check.is_some() || c.finish_writes().any(|w| m.state_variables[w.variable.0].last_write_wins))
                    && stage.events.iter().any(|e| matches!(e, x::Event::Business(e) if e.kind == Kind::CancelRequest && e.binding == stage.binding)))?;
            }
            legal(!due(f))?;
            let wait = f
                .waiting
                .as_ref()
                .ok_or(ReplayReason::IllegalTransition)?
                .clone();
            let candidates = i.winners(f, &wait);
            let winner = stage
                .events
                .iter()
                .find_map(|event| {
                    let x::Event::Business(event) = event else {
                        return None;
                    };
                    if event.kind != Kind::Adopt {
                        return None;
                    }
                    candidates.iter().copied().find(|b| {
                        event.binding.as_deref() == Some(m.process.bindings[b.0].name.as_str())
                    })
                })
                .ok_or(ReplayReason::IllegalTransition)?;
            f.waiting.as_mut().unwrap().winner = Some(winner);
            f.adoption = Some(f.now);
            if matches!(
                wait.join,
                Some(Join::Any {
                    remaining: Remaining::CancelThenDrain,
                    ..
                })
            ) {
                for n in wait.tasks {
                    let t = &mut f.tasks[n];
                    if t.call.binding == winner || t.terminal() {
                        continue;
                    }
                    let cap = &m.capabilities[t.call.capability.0];
                    legal(cap.cancel != Cancel::Unspecified)?;
                    if t.phase == Phase::Queued {
                        t.phase = Phase::Cancelled;
                        t.stop = Some(f.now);
                    } else if cap.cancel == Cancel::Immediate {
                        legal(stage.events.iter().any(|event| matches!(event,x::Event::Business(e) if e.kind == Kind::CancelRequest && e.binding.as_deref() == Some(m.site_name(t.call.binding).as_str()) && e.ack_at == Some(f.now))))?;
                        t.ack = Some(f.now);
                    } else if let Cancel::Cooperative { ack_within } = cap.cancel {
                        let ack = stage
                            .events
                            .iter()
                            .find_map(|event| match event {
                                x::Event::Business(e)
                                    if e.kind == Kind::CancelRequest
                                        && e.binding.as_deref()
                                            == Some(
                                                m.process.bindings[t.call.binding.0].name.as_str(),
                                            ) =>
                                {
                                    e.ack_at
                                }
                                _ => None,
                            })
                            .ok_or(ReplayReason::IllegalTransition)?;
                        legal(ack > f.now && ack - f.now <= ack_within)?;
                        t.ack = Some(ack);
                    }
                }
            }
        }
        Kind::DurationGate => {
            let coins = reference(i.duration_coins(f))?.ok_or(ReplayReason::ReferenceLimit)?;
            legal(!coins.is_empty() && stage.events.len() == coins.len())?;
            for (n, q) in coins {
                let t = &mut f.tasks[n];
                let e = stage
                    .events
                    .iter()
                    .find_map(|event| match event {
                        x::Event::Business(e)
                            if e.binding.as_deref()
                                == Some(m.process.bindings[t.call.binding.0].name.as_str()) =>
                        {
                            Some(e)
                        }
                        _ => None,
                    })
                    .ok_or(ReplayReason::IllegalTransition)?;
                let finish = e.kind == Kind::Finish;
                legal(if finish {
                    !q.is_zero()
                } else {
                    e.kind == Kind::DurationGate && q != Rational::one()
                })?;
                t.checked_through = Some(f.now - t.start.ok_or(ReplayReason::IllegalTransition)?);
                t.finish = finish.then_some(f.now);
            }
        }
        Kind::Join => {
            let wait = f.waiting.as_ref().ok_or(ReplayReason::IllegalTransition)?;
            legal(wait.tasks.clone().all(|n| f.tasks[n].terminal()))?;
            for n in wait.tasks.clone() {
                let t = &f.tasks[n];
                if m.capabilities[t.call.capability.0].receipt_query.is_some()
                    && matches!(t.phase, Phase::Finished(_))
                {
                    legal(f.trace.iter().any(|o|matches!(o.event,Occurrence::ReceiptQuery(q) if q.query==t.call.binding)))?;
                }
            }
            legal(!matches!(wait.join, Some(Join::Any { .. })) || wait.winner.is_some())?;
            if !(wait.join.is_none()
                && wait.tasks.len() == 1
                && m.response_attempt(f.tasks[wait.tasks.start].call.binding)
                    .is_some())
            {
                f.waiting = None;
            }
        }
        Kind::LoopControl => {
            let stmt = next_statement(f)?;
            legal(stage.location.span == stmt.span)?;
            let Stmt::LoopControl(control) = &stmt.kind else {
                return Err(ReplayReason::IllegalTransition);
            };
            for (id, x) in &control.updates {
                f.state_values[id.0] = *x;
            }
        }
        Kind::Branch => {
            if let Some(wait) = &f.waiting
                && wait.join.is_none()
                && wait.tasks.len() == 1
                && let Some(r) = m.response_attempt(f.tasks[wait.tasks.start].call.binding)
            {
                legal(stage.location.span == r.command_call.span)?;
                let now = f.now;
                let (decision, events) = reference(super::response::progress(f, m, now))?;
                for event in events {
                    reference(i.record(f, event, None))?;
                }
                if let super::response::Progress::End(end) = decision {
                    reference(i.record(f, Occurrence::ResponseDestination(end), None))?;
                }
                return legal(stage.time == f.now && same_values(f, m, stage));
            }
            legal(f.waiting.is_none())?;
            while f.control.last().is_some_and(|b| b.is_empty()) {
                f.control.pop();
            }
            if let Some(stmt) = f.control.last().and_then(|b| b.first())
                && matches!(stmt.kind, Stmt::StopReceiptUnknown)
            {
                legal(stage.location.span == stmt.span)?;
                next_statement(f)?;
                let event = reference(super::recovery::stop(f, m))?;
                reference(i.record(f, event, None))?;
                return legal(stage.time == f.now && same_values(f, m, stage));
            }
            if let Some(stmt) = f.control.last().and_then(|b| b.first())
                && let Stmt::WaitUntil(condition) = &stmt.kind
            {
                legal(
                    stage.location.span == stmt.span && reference(i.condition(f, condition, 0))?,
                )?;
                reference(f.condition_pass())?;
                return legal(stage.time == f.now && same_values(f, m, stage));
            }
            let Stmt::If {
                condition,
                then_body,
                else_body,
            } = &next_statement(f)?.kind
            else {
                return Err(ReplayReason::IllegalTransition);
            };
            let body = if reference(i.condition(f, condition, 0))? {
                then_body
            } else {
                else_body
            };
            if f.control.len() >= i.limits.control_depth {
                return Err(ReplayReason::ReferenceLimit);
            }
            f.control.push(body);
        }
        Kind::End => {
            if m.recovery().is_some() {
                legal(super::recovery::unresolved(f).is_empty())?;
            }
            let exhausted = f.trace.last().and_then(|o| match o.event {
                Occurrence::ResponseDestination(e) => Some(e),
                _ => None,
            });
            let end = if let Some(end) = exhausted {
                end
            } else if let Some(w) = &f.waiting {
                legal(w.tasks.clone().all(|n| f.tasks[n].terminal()) && w.winner.is_none())?;
                let Some(Join::Any { if_none, .. }) = w.join else {
                    return Err(ReplayReason::IllegalTransition);
                };
                *if_none
            } else {
                let Stmt::End(end) = next_statement(f)?.kind else {
                    return Err(ReplayReason::IllegalTransition);
                };
                end
            };
            reference(i.record(f, Occurrence::Terminal(end), None))?;
            reference(i.inspect_trace(f, f.now, false, true, false))?;
            if !m.contests().is_empty() {
                legal(f.business.is_none())?;
                f.business = Some((end, f.now));
                f.control.clear();
                f.waiting = None;
            }
        }
        Kind::Deadlock => {
            legal(!instant(i, f)? && !f.tasks.iter().any(|t| t.phase == Phase::Running))?;
            if f.waiting.is_none()
                && !f
                    .control
                    .iter()
                    .rev()
                    .find_map(|b| b.first())
                    .is_some_and(|s| matches!(&s.kind, Stmt::WaitUntil(_)))
            {
                legal(next_statement(f).is_err())?;
            }
            i.evidence.deadlocked = true;
            reference(i.inspect_trace(f, f.now, false, true, true))?;
        }
        _ => return Err(ReplayReason::IllegalTransition),
    }
    legal(stage.time == f.now)?;
    i.evidence
        .lost_updates
        .extend(f.lost_updates.iter().copied());
    if !same_values(f, m, stage) {
        return Err(ReplayReason::StateMismatch);
    }
    reference(i.observe_capacity(f))?;
    Ok(())
}

fn validate_target(steps: &[Stage], target: &str) -> Result<()> {
    let last = steps.last().ok_or(ReplayReason::IllegalTransition)?;
    let violation = last
        .violation
        .as_ref()
        .ok_or(ReplayReason::ViolationMismatch)?;
    if violation.id != target || violation.at_step != last.index {
        return Err(ReplayReason::ViolationMismatch);
    }
    Ok(())
}

fn validate_d1_target(
    m: &Model,
    steps: &[Stage],
    target: &str,
    case_id: Option<u64>,
) -> Result<()> {
    if !m.tracks_obligations() {
        return Ok(());
    }
    let Some(clause) = m
        .contracts
        .iter()
        .enumerate()
        .find_map(|(n, c)| (c.property_id(ClauseId(n)) == target).then_some(c))
    else {
        return Ok(());
    };
    let violation = steps
        .last()
        .and_then(|s| s.violation.as_ref())
        .ok_or(ReplayReason::ViolationMismatch)?;
    if !violation
        .property_location
        .as_ref()
        .is_some_and(|l| l.span == clause.span)
    {
        return Err(ReplayReason::ViolationMismatch);
    }
    if let Some((trigger, _, within)) = clause.response() {
        let stage = violation
            .trigger_step
            .and_then(|n| steps.get(n))
            .ok_or(ReplayReason::ViolationMismatch)?;
        let matches = match trigger {
            Trigger::Received => stage.events.iter().any(|e| matches!(e, x::Event::Business(e) if matches!(e.kind, Kind::Received | Kind::Receive))),
            Trigger::Admitted => stage.events.iter().any(|e| matches!(e, x::Event::Business(e) if e.kind == Kind::Admitted)),
            Trigger::Event(pattern) => stage.events.iter().any(|e| {
                let x::Event::Business(e) = e else { return false };
                let cap = &m.capabilities[pattern.capability.0];
                e.actor.as_deref() == Some(m.actors[cap.actor.0].name.as_str()) && e.capability.as_deref() == Some(cap.name.as_str()) && match pattern.kind {
                    EventKind::Start => e.kind == Kind::Start,
                    EventKind::Commit => e.kind == Kind::Commit,
                    EventKind::CancelAck => e.kind == Kind::CancelAck,
                    EventKind::Finish(v) => e.kind == Kind::Finish && v.is_none_or(|v| e.result.as_deref() == Some(m.types[cap.return_type.0].members[v.0].as_str())),
                }
            }),
        };
        if !matches || stage.time.checked_add(within) != violation.deadline {
            return Err(ReplayReason::ViolationMismatch);
        }
    }
    if let ContractKind::ContestResponse { contest } = clause.kind {
        let d = &m.contests()[contest];
        let trigger = violation
            .trigger_step
            .and_then(|n| steps.get(n))
            .ok_or(ReplayReason::ViolationMismatch)?;
        legal(
            (match d.target {
                crate::contest::ContestTarget::End(_) => trigger.kind == Kind::End,
                crate::contest::ContestTarget::Commit(c) => {
                    let cap = &m.capabilities[c.0];
                    trigger.kind == Kind::Finish && trigger.events.iter().any(|e| matches!(e, x::Event::Business(e) if e.kind == Kind::Commit && e.actor.as_deref() == Some(m.actors[cap.actor.0].name.as_str()) && e.capability.as_deref() == Some(cap.name.as_str())))
                }
            }) && (!d.target.effect() || trigger.case_id == case_id)
                && trigger.time.checked_add(d.respond_by) == violation.deadline,
        )?;
    }
    Ok(())
}

fn response_metadata(m: &Model, s: &Stage) -> Result<()> {
    if matches!(s.kind, Kind::ReceiptQuery | Kind::ResolveEffect) {
        let (_, body) = m.recovery().ok_or(ReplayReason::IllegalTransition)?;
        let binding = m
            .process
            .bindings
            .iter()
            .position(|b| Some(b.name.as_str()) == s.binding.as_deref())
            .map(BindingId)
            .ok_or(ReplayReason::IllegalTransition)?;
        let (_, command) =
            super::recovery::source(body, binding).ok_or(ReplayReason::IllegalTransition)?;
        return legal(
            s.command.as_deref() == Some(m.site_name(command).as_str())
                && s.saved_key == s.case_id
                && s.case_id.is_some()
                && s.attempt.is_none()
                && s.observed_receipt.is_none()
                && s.actual_result.is_none()
                && s.retry_location.is_none(),
        );
    }
    legal(s.saved_key.is_none() && s.evidence_ref.is_none())?;
    let descriptor = m
        .response_attempts()
        .into_iter()
        .find(|(call, _)| s.binding.as_deref() == Some(m.site_name(call.binding).as_str()));
    if let Some((_, r)) = descriptor {
        legal(
            s.command.as_deref() == Some(m.site_name(r.command).as_str())
                && s.attempt == Some(r.attempt)
                && s.retry_location.as_ref().map(|l| l.span) == r.retry_span,
        )?;
        if s.kind == Kind::Finish {
            let observed = s.observed_receipt.ok_or(ReplayReason::EventMismatch)?;
            legal(s.actual_result.is_some())?;
            legal(match &r.loss {
                ResponseLoss::None => observed == crate::ast::Receipt::Delivered,
                ResponseLoss::Sequence(xs) => xs[r.attempt as usize - 1] == observed,
                ResponseLoss::Nondet => true,
                ResponseLoss::Probability { probability: p, .. } => {
                    if observed == crate::ast::Receipt::Unknown {
                        !p.is_zero()
                    } else {
                        p != &Rational::one()
                    }
                }
            })?;
            legal(if observed == crate::ast::Receipt::Unknown {
                s.result.is_none()
            } else {
                s.result == s.actual_result
            })?;
        } else {
            legal(s.actual_result.is_none() && s.observed_receipt.is_none())?;
        }
    } else {
        legal(
            s.command.is_none()
                && s.attempt.is_none()
                && s.observed_receipt.is_none()
                && s.actual_result.is_none()
                && s.retry_location.is_none(),
        )?;
    }
    Ok(())
}
fn validate_task_stage(m: &Model, f: &Frame<'_>, s: &Stage) -> Result<()> {
    let n = task_index(f, m, s)?;
    let task = &f.tasks[n];
    let cap = &m.capabilities[task.call.capability.0];
    legal(
        s.actor.as_deref() == Some(m.actors[cap.actor.0].name.as_str())
            && s.capability.as_deref() == Some(cap.name.as_str())
            && s.location.span == task.call.span
            && s.capability_location
                .as_ref()
                .is_some_and(|l| l.span == cap.span),
    )?;
    if s.kind != Kind::Start {
        legal(s.executor.as_deref() == task.person.map(|p| m.people[p.0].name.as_str()))?;
    }
    legal(s.kind == Kind::Finish || s.result.is_none())?;
    let kinds = if s.kind == Kind::Finish
        && s.actual_result
            .as_ref()
            .or(s.result.as_ref())
            .and_then(|r| {
                m.types[cap.return_type.0]
                    .members
                    .iter()
                    .position(|v| v == r)
            })
            .is_some_and(|r| cap.commits_result(ValueId(r)))
    {
        vec![Kind::Finish, Kind::Commit]
    } else {
        vec![s.kind]
    };
    legal(s.events.len() == kinds.len())?;
    for (event, kind) in s.events.iter().zip(kinds) {
        let x::Event::Business(event) = event else {
            return Err(ReplayReason::EventMismatch);
        };
        if event.kind != kind
            || event.case_id != s.case_id
            || event.binding != s.binding
            || event.actor != s.actor
            || event.capability != s.capability
            || event.executor != s.executor
            || event.result.as_ref() != s.actual_result.as_ref().or(s.result.as_ref())
            || event.location != s.location
            || event.ack_at.is_some()
        {
            return Err(ReplayReason::EventMismatch);
        }
    }
    Ok(())
}

/// Reconstruct displays from reference task phases, not captured JSON or runtime
/// occupancy helpers. The caller supplies the entire post-transition world.
pub(super) fn validate_resource_display(
    m: &Model,
    frames: &[(u64, &Frame<'_>)],
    s: &Stage,
) -> Result<()> {
    if m.resource_time.is_some() && matches!(s.kind, Kind::Start | Kind::Refuse) {
        let task = frames
            .iter()
            .filter(|(case, _)| Some(*case) == s.case_id)
            .flat_map(|(_, f)| &f.tasks)
            .find(|t| s.binding.as_deref() == Some(m.site_name(t.call.binding).as_str()))
            .ok_or(ReplayReason::IllegalTransition)?;
        let cap = &m.capabilities[task.call.capability.0];
        let mut units = reference(reference_resource_units(
            frames.iter().flat_map(|(_, f)| &f.tasks),
            m,
        ))?;
        let mut used = frames
            .iter()
            .flat_map(|(_, f)| &f.tasks)
            .filter(|t| {
                t.phase == Phase::Running && m.capabilities[t.call.capability.0].actor == cap.actor
            })
            .count() as u64;
        if s.kind == Kind::Start {
            used = used.checked_sub(1).ok_or(ReplayReason::IllegalTransition)?;
            for u in &cap.uses {
                units[u.resource.0] = units[u.resource.0]
                    .checked_sub(u.amount)
                    .ok_or(ReplayReason::IllegalTransition)?;
            }
        }
        legal(policy::calendar_slots(m, cap, s.time, used, &units))?;
        if s.kind == Kind::Refuse
            && let Some(ps) = &m.actors[cap.actor.0].executors
        {
            legal(ps.iter().any(|p| {
                policy::working(m, *p, s.time)
                    && !frames
                        .iter()
                        .flat_map(|(_, f)| &f.tasks)
                        .any(|t| t.phase == Phase::Running && t.person == Some(*p))
            }))?;
        }
    }
    if matches!(s.kind, Kind::Advance | Kind::Deadline) && m.has_condition_wait() {
        let pending = frames.iter().any(|(_, f)| {
            f.tasks.iter().any(|t| t.phase == Phase::Running)
                || f.sleep().is_some()
                || (m.resource_time.is_some() && policy::awaits_calendar(m, f))
        });
        let arrival = m
            .arrivals
            .as_ref()
            .is_some_and(|ts| ts.iter().any(|t| *t >= s.time));
        legal(pending || arrival)?;
    }
    let mut rows = Vec::new();
    for (id, resource) in m.resources.iter().enumerate() {
        let mut holders = Vec::new();
        for (case_id, frame) in frames {
            for task in frame.tasks.iter().filter(|t| t.phase == Phase::Running) {
                for claim in &m.capabilities[task.call.capability.0].uses {
                    if claim.resource.0 == id {
                        holders.push(x::SharedHolder {
                            case_id: *case_id,
                            binding: m.site_name(task.call.binding),
                            amount: claim.amount,
                        });
                    }
                }
            }
        }
        holders.sort_by_key(|h| (h.case_id, h.binding.clone()));
        let occupied = holders
            .iter()
            .try_fold(0u64, |n, h| n.checked_add(h.amount))
            .ok_or(ReplayReason::IllegalTransition)?;
        rows.push(x::SharedResource {
            resource: resource.name.clone(),
            capacity: resource.capacity,
            occupied,
            holders,
        });
    }
    let expected: Vec<_> = if matches!(
        s.kind,
        Kind::Queue
            | Kind::Start
            | Kind::Finish
            | Kind::ContestResponse
            | Kind::CancelRequest
            | Kind::CancelAck
            | Kind::Refuse
    ) {
        let frame = frames.iter().find(|(id, _)| Some(*id) == s.case_id);
        if s.kind == Kind::CancelRequest {
            // A bundled adoption/request edge shows every requested task's
            // resources; the requested set is read from the stage's events.
            let requested: Vec<_> = s
                .events
                .iter()
                .filter_map(|e| match e {
                    x::Event::Business(e)
                        if e.kind == Kind::CancelRequest && e.case_id == s.case_id =>
                    {
                        e.binding.as_deref()
                    }
                    _ => None,
                })
                .collect();
            let tasks: Vec<_> = frame
                .into_iter()
                .flat_map(|(_, f)| f.tasks.iter())
                .filter(|t| requested.contains(&m.site_name(t.call.binding).as_str()))
                .collect();
            rows.iter()
                .enumerate()
                .filter(|(id, _)| {
                    tasks.iter().any(|t| {
                        m.capabilities[t.call.capability.0]
                            .uses
                            .iter()
                            .any(|u| u.resource.0 == *id)
                    })
                })
                .map(|(_, r)| r.clone())
                .collect()
        } else {
            frame
                .and_then(|(_, f)| {
                    f.tasks.iter().find(|t| {
                        s.binding.as_deref() == Some(m.site_name(t.call.binding).as_str())
                    })
                })
                .map_or_else(Vec::new, |t| {
                    rows.iter()
                        .enumerate()
                        .filter(|(id, _)| {
                            m.capabilities[t.call.capability.0]
                                .uses
                                .iter()
                                .any(|u| u.resource.0 == *id)
                        })
                        .map(|(_, r)| r.clone())
                        .collect()
                })
        }
    } else {
        Vec::new()
    };
    if s.shared_resources != expected {
        return Err(ReplayReason::StateMismatch);
    }
    if s.kind == Kind::Deadlock {
        let detail = s
            .deadlock_detail
            .as_ref()
            .ok_or(ReplayReason::StateMismatch)?;
        let waiting: Vec<_> = frames
            .iter()
            .flat_map(|(id, f)| {
                f.tasks
                    .iter()
                    .filter(|t| t.phase == Phase::Queued)
                    .map(move |t| (*id, t))
            })
            .collect();
        let expected: Vec<_> = rows
            .iter()
            .enumerate()
            .filter(|(id, _)| {
                waiting.iter().any(|(_, t)| {
                    m.capabilities[t.call.capability.0]
                        .uses
                        .iter()
                        .any(|u| u.resource.0 == *id)
                })
            })
            .map(|(_, r)| r.clone())
            .collect();
        if detail.shared_resources != expected || detail.waiting_tasks.len() != waiting.len() {
            return Err(ReplayReason::StateMismatch);
        }
        for ((case, task), actual) in waiting.iter().zip(&detail.waiting_tasks) {
            let mut claims: Vec<_> = m.capabilities[task.call.capability.0].uses.iter().collect();
            claims.sort_by_key(|u| u.resource);
            let shortages: Vec<_> = claims
                .into_iter()
                .filter_map(|u| {
                    let r = &rows[u.resource.0];
                    let available = r.capacity.saturating_sub(r.occupied);
                    (u.amount > available).then(|| x::ResourceShortage {
                        resource: r.resource.clone(),
                        required: u.amount,
                        available,
                        missing: u.amount - available,
                    })
                })
                .collect();
            if actual.case_id != *case
                || actual.binding != m.site_name(task.call.binding)
                || actual.resource_shortages != shortages
            {
                return Err(ReplayReason::StateMismatch);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn req_reference_wait_overflow_is_semantic_abort_not_early_wake() {
        use super::*;
        let src = r#"model M version "0.2" { process P { wait 2min; end done } bounds { cases 1; live_tasks 0; clock 18446744073709551615min } verify reachable end.done }"#;
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let mut i = interpreter(&m);
        let mut f = initial(&m);
        f.now = u64::MAX;
        assert_eq!(i.run_inner(f, 0), Err(StopReason::SemanticAbort));
        assert!(i.terminals.is_empty());
    }
    #[test]
    fn req_replay_rejects_forged_refuse_wake_start_ack() {
        use super::*;
        for (base, clause, fault) in [
            (
                include_str!("../../../../fixtures/req-start.broken-false/model.relca"),
                "response case.received -> one_of(A.run.finish(ok)) by 8min after case.received otherwise unresolved",
                0,
            ),
            (
                include_str!("../../../../fixtures/req-wait-gap/model.relca"),
                "response case.received -> one_of(end.done) by 9min after case.received otherwise unresolved",
                1,
            ),
            (
                include_str!("../../../../fixtures/req-start/model.relca"),
                "response case.received -> one_of(A.run.finish(denied)) by 8min after case.received otherwise unresolved",
                2,
            ),
            (
                include_str!("../../../../fixtures/req-cancel-immediate/model.relca"),
                "response Slow.work.start -> one_of(Slow.work.finish) by 10min after Slow.work.start otherwise unresolved",
                3,
            ),
        ] {
            let source = base.replace(
                "  bounds",
                &format!("  contract Trace {{ {clause} }}\n  bounds"),
            );
            let mut m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
            let explained = crate::explain::explain(&source);
            let c = explained
                .explanation
                .counterexamples
                .iter()
                .find(|c| c.id.ends_with(" response"))
                .unwrap();
            assert_eq!(c.status, x::ReplayStatus::Verified);
            replay(&m, &c.steps, &c.id, c.case_id).unwrap();
            let mut steps = c.steps.clone();
            match fault {
                0 => {
                    let req = m.capabilities[0].requires.as_mut().unwrap();
                    if let Cond::StateEq(x) = &mut req.condition {
                        let InitialValue::Fixed(value) = m.state_variables[x.variable.0].initial
                        else {
                            panic!()
                        };
                        x.value = value;
                    } else {
                        panic!()
                    };
                }
                1 => {
                    let wake = steps.iter_mut().find(|s| s.kind == Kind::Wake).unwrap();
                    wake.time -= 1;
                }
                2 => {
                    let s = steps.iter_mut().find(|s| s.kind == Kind::Start).unwrap();
                    s.values
                        .observed
                        .iter_mut()
                        .find(|v| v.name == "begun")
                        .unwrap()
                        .value = "false".into();
                }
                _ => {
                    let e = steps
                        .iter_mut()
                        .flat_map(|s| &mut s.events)
                        .find_map(|e| match e {
                            x::Event::Business(e) if e.kind == Kind::CancelRequest => Some(e),
                            _ => None,
                        })
                        .unwrap();
                    e.ack_at = Some(e.ack_at.unwrap() + 1);
                }
            }
            assert!(
                replay(&m, &steps, &c.id, c.case_id).is_err(),
                "fault {fault}"
            );
        }
    }
    use super::*;
    #[test]
    fn waived_occurrence_cannot_be_forged_as_violation() {
        let src = r#"model M version "0.2" {
          type T = enum { ok }
          people [p,issuer]
          actor Gate { capacity 2; capability run() -> T { duration fixed 1min; result nondet { ok } } }
          actor Officer { executors [p]; capability release() -> T { duration fixed 1min; result nondet { ok } } }
          actor B { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }
          actor Wait { capacity 1; capability run() -> T { duration fixed 6min; result nondet { ok } } }
          permit [p] to Officer.release
          authority G { basis "grant"; grants [p] to Officer.release; issued_by issuer }
          process Flow { g1 = Gate.run(); g2 = Gate.run(); o = Officer.release(); g3 = Gate.run(); w = Wait.run(); end done }
          bounds { cases 1; live_tasks 3; clock 30min }
          verify safety no_resource_overcommit
          contract C { response delivery: Gate.run.finish -> one_of(B.run.finish) by 5min after Gate.run.finish otherwise unresolved; waive delivery on Officer.release.finish reason "exception" authorized_by G }
        }"#;
        for source in [
            src.to_owned(),
            src.replace("verify safety", "arrivals fixed [0min]\n verify safety"),
        ] {
            let (m, mut steps, target, case) = candidate(&source);
            let g1 = steps
                .iter()
                .position(|s| s.binding.as_deref() == Some("g1") && s.kind == Kind::Finish)
                .unwrap();
            let deadline = steps[g1].time + 5;
            let violation = steps.last_mut().unwrap().violation.as_mut().unwrap();
            violation.trigger_step = Some(g1);
            violation.deadline = Some(deadline);
            assert_eq!(
                replay(&m, &steps, &target, case),
                Err(ReplayReason::ViolationMismatch)
            );
        }
    }
    // Mutations F31a/F31b (ADR 0040): an effect trigger_step must be the commit
    // of the contested Capability in the same case, not another commit at the
    // same time. F31a: two cases commit post at 3; c1 serializes the routes, so
    // the second responds 5 > deadline 4. F31b: one case commits post and audit
    // at 3; recheck 3..5 responds 5 > deadline 4. Swapping the trigger to the
    // other commit keeps time and deadline equal, so only the case or the
    // Capability check can reject it.
    fn f31_response(src: &str) -> (Model, Vec<Stage>, String, Option<u64>) {
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let r = crate::explain::explain(src);
        let c = r
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id == "contract Controls #1 contest_response")
            .unwrap_or_else(|| {
                panic!(
                    "{:?}",
                    r.explanation
                        .counterexamples
                        .iter()
                        .map(|c| c.id.clone())
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(c.status, x::ReplayStatus::Verified);
        (m, c.steps.clone(), c.id.clone(), c.case_id)
    }
    fn f31_commits(steps: &[Stage], cap: &str) -> Vec<usize> {
        steps.iter().enumerate().filter(|(_, s)| s.kind == Kind::Finish && s.time == 3
            && s.events.iter().any(|e| matches!(e, x::Event::Business(e) if e.kind == Kind::Commit && e.capability.as_deref() == Some(cap))))
            .map(|(i, _)| i).collect()
    }
    #[test]
    fn f31a_effect_trigger_of_other_case_same_time_is_rejected() {
        let src = r#"model StandingEffects version "0.2" {
  type Verdict = enum { pass, fail }
  type Ack = enum { committed }
  people [c1, a1, cfo]
  actor Approver { capacity 2; capability approve() -> Verdict { duration fixed 2min; result nondet { pass } } }
  actor Ledger { capacity 2; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed } } }
  actor Clerk { executors [c1]; capability recheck() -> Verdict { duration fixed 1min; result nondet { pass } } }
  actor Vendor { control party; standing over [Ledger.post.commit] basis "posting claim" }
  process Flow { approval = Approver.approve(); ack = Ledger.post(); end posted }
  cut Scope { include [Approver, Ledger, Clerk, Vendor] }
  arrivals fixed [0min, 0min]
  bounds { cases 2; live_tasks 4; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { contest Vendor on Ledger.post.commit route Clerk.recheck reversible_by [a1] respond_by 1min }
}"#;
        let (m, steps, target, case) = f31_response(src);
        assert!(replay(&m, &steps, &target, case).is_ok());
        let trigger = steps
            .last()
            .unwrap()
            .violation
            .as_ref()
            .unwrap()
            .trigger_step
            .unwrap();
        let commits = f31_commits(&steps, "post");
        assert_eq!(commits.len(), 2, "both cases commit at 3");
        assert!(commits.contains(&trigger));
        let other = *commits.iter().find(|&&i| i != trigger).unwrap();
        assert_ne!(steps[other].case_id, steps[trigger].case_id);
        let mut bad = steps.clone();
        bad.last_mut()
            .unwrap()
            .violation
            .as_mut()
            .unwrap()
            .trigger_step = Some(other);
        assert!(replay(&m, &bad, &target, case).is_err());
    }
    #[test]
    fn f31b_effect_trigger_of_other_commit_capability_same_time_is_rejected() {
        let src = r#"model StandingEffects version "0.2" {
  type Verdict = enum { pass, fail }
  type Ack = enum { committed }
  people [c1, a1, cfo]
  actor Approver { executors [a1]; capability approve() -> Verdict { duration fixed 2min; result nondet { pass } } }
  actor Ledger { capacity 2; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed } }; capability audit() -> Ack { commits; duration fixed 1min; result nondet { committed } } }
  actor Clerk { executors [c1]; capability recheck() -> Verdict { duration fixed 2min; result nondet { pass } } }
  actor Vendor { control party; standing over [Ledger.post.commit] basis "posting claim" }
  process Flow { approval = Approver.approve(); fork f { ack = Ledger.post(); au = Ledger.audit() }; join all f; end posted }
  cut Scope { include [Approver, Ledger, Clerk, Vendor] }
  bounds { cases 1; live_tasks 4; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { contest Vendor on Ledger.post.commit route Clerk.recheck reversible_by [a1] respond_by 1min }
}"#;
        let (m, steps, target, case) = f31_response(src);
        assert!(replay(&m, &steps, &target, case).is_ok());
        let trigger = steps
            .last()
            .unwrap()
            .violation
            .as_ref()
            .unwrap()
            .trigger_step
            .unwrap();
        assert_eq!(f31_commits(&steps, "post"), vec![trigger]);
        let audit = f31_commits(&steps, "audit");
        assert_eq!(audit.len(), 1, "audit commits at 3 too");
        let mut bad = steps.clone();
        bad.last_mut()
            .unwrap()
            .violation
            .as_mut()
            .unwrap()
            .trigger_step = Some(audit[0]);
        assert!(replay(&m, &bad, &target, case).is_err());
    }
    fn candidate(src: &str) -> (Model, Vec<Stage>, String, Option<u64>) {
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let r = crate::explain::explain(src);
        let c = &r.explanation.counterexamples[0];
        assert_eq!(
            c.status,
            x::ReplayStatus::Verified,
            "{:?}",
            r.explanation.diagnostics
        );
        (m, c.steps.clone(), c.id.clone(), c.case_id)
    }
    #[test]
    fn effect_route_response_result_poison_is_checked_after_business_end() {
        let source = include_str!("../../../../fixtures/sov-basic/model.relca").replace(
            "contract Controls {",
            "contract Controls { invariant Clerk.recheck.finish(pass) -> approval == fail;",
        );
        for src in [
            source.clone(),
            source.replace("  bounds", "  arrivals fixed [0min]\n  bounds"),
        ] {
            let (m, steps, target, case) = candidate(&src);
            let response = steps
                .iter()
                .position(|s| s.kind == Kind::ContestResponse)
                .unwrap();
            assert_eq!(steps[response].time, 4);
            assert!(
                steps[..response]
                    .iter()
                    .any(|s| s.kind == Kind::End && s.time == 3)
            );
            for change_events in [false, true] {
                let mut bad = steps.clone();
                bad[response].result = Some("fail".into());
                if change_events {
                    for event in &mut bad[response].events {
                        if let x::Event::Business(e) = event {
                            e.result = Some("fail".into());
                        }
                    }
                }
                assert!(replay(&m, &bad, &target, case).is_err());
            }
        }
    }
    // P27 (ADR 0040 integration after ADR 0037): the state invariant
    // safe == true is violated by the effect route's finish safe = false at4,
    // after End posted3, so its counterexample must contain the response
    // Finish. The untouched trace replays. A response result changed on the
    // stage only (not its events) is an inconsistent trace, and a result that
    // is not a Verdict member is not a legal finish: both must be rejected.
    #[test]
    fn p27_state_invariant_counterexample_rejects_response_result_poison() {
        let source = include_str!("../../../../fixtures/sov-basic/model.relca")
            .replace(
                "  actor Approver",
                "  observed case { safe: bool = true }\n  actor Approver",
            )
            .replace(
                "capability recheck() -> Verdict { duration fixed 1min; result nondet { pass, fail } }",
                "capability recheck() -> Verdict { duration fixed 1min; result nondet { pass, fail }; writes safe; finish { safe = false } }",
            )
            .replace(
                "contract Controls { ",
                "contract Controls { invariant safe == true; ",
            );
        assert!(source.contains("finish { safe = false }"));
        assert!(source.contains("invariant safe == true"));
        for src in [
            source.clone(),
            source.replace("  bounds", "  arrivals fixed [0min]\n  bounds"),
        ] {
            let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
            let r = crate::explain::explain(&src);
            let c = r
                .explanation
                .counterexamples
                .iter()
                .find(|c| c.id == "contract Controls #1 invariant")
                .unwrap();
            assert_eq!(c.status, x::ReplayStatus::Verified);
            let (steps, target, case) = (c.steps.clone(), c.id.clone(), c.case_id);
            assert!(replay(&m, &steps, &target, case).is_ok());
            let response = steps
                .iter()
                .position(|s| s.kind == Kind::ContestResponse)
                .unwrap();
            assert_eq!(steps[response].time, 4);
            assert!(
                steps[..response]
                    .iter()
                    .any(|s| s.kind == Kind::End && s.time == 3)
            );
            let other = if steps[response].result.as_deref() == Some("pass") {
                "fail"
            } else {
                "pass"
            };
            let mut stage_only = steps.clone();
            stage_only[response].result = Some(other.into());
            assert!(replay(&m, &stage_only, &target, case).is_err());
            let mut bogus = steps.clone();
            bogus[response].result = Some("bogus".into());
            for event in &mut bogus[response].events {
                if let x::Event::Business(e) = event {
                    e.result = Some("bogus".into());
                }
            }
            assert!(replay(&m, &bogus, &target, case).is_err());
        }
    }
    #[test]
    fn effect_target_filing_queue_executor_and_early_close_poison_are_rejected() {
        let source = include_str!("../../../../fixtures/sov-concurrent.broken-short/model.relca");
        for src in [
            source.to_owned(),
            source.replace("  bounds", "  arrivals fixed [0min]\n  bounds"),
        ] {
            let (m, steps, target, case) = candidate(&src);
            assert!(replay(&m, &steps, &target, case).is_ok());
            let trigger = steps
                .last()
                .unwrap()
                .violation
                .as_ref()
                .unwrap()
                .trigger_step
                .unwrap();
            let filed = steps
                .iter()
                .position(|s| s.kind == Kind::ContestFiled)
                .unwrap();
            let queue = steps
                .iter()
                .position(|s| s.kind == Kind::ContestQueue)
                .unwrap();
            let start = steps
                .iter()
                .position(|s| s.kind == Kind::Start && s.binding.as_deref() == Some("contest:1"))
                .unwrap();
            for fault in 0..9 {
                let mut bad = steps.clone();
                match fault {
                    0 => bad[trigger].capability = Some("approve".into()),
                    1 => bad[trigger].time += 1,
                    2 => bad[trigger].case_id = Some(2),
                    3 => bad[filed].time += 1,
                    4 => bad.last_mut().unwrap().violation.as_mut().unwrap().deadline = Some(99),
                    5 => bad[queue].binding = Some("contest:999".into()),
                    6 => bad[start].executor = Some("a1".into()),
                    7 => bad[queue].kind = Kind::ContestFiled,
                    8 => bad[filed].kind = Kind::ExecutionClose,
                    _ => unreachable!(),
                }
                assert!(replay(&m, &bad, &target, case).is_err(), "poison {fault}");
            }
            assert!(replay(&m, &steps, &target, Some(2)).is_err());
            let mut wrong_case = steps.clone();
            for s in &mut wrong_case {
                if s.case_id.is_some() {
                    s.case_id = Some(2);
                }
            }
            assert!(replay(&m, &wrong_case, &target, Some(2)).is_err());
        }
    }
    #[test]
    fn d2_a8_rejects_forged_choice_queue_deadline_person_and_clock_in_both_drivers() {
        let source = include_str!("../../../../fixtures/ctx-contest.broken-late/model.relca");
        for src in [
            source.to_owned(),
            source.replace("  bounds", "  arrivals fixed [0min]\n  bounds"),
        ] {
            let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
            let report = crate::explain::explain(&src);
            let c = report
                .explanation
                .counterexamples
                .iter()
                .find(|c| c.id.ends_with(" contest_response"))
                .unwrap();
            assert_eq!(
                c.status,
                x::ReplayStatus::Verified,
                "{:?}",
                report.explanation.diagnostics
            );
            assert!(replay(&m, &c.steps, &c.id, c.case_id).is_ok());
            let filed = c
                .steps
                .iter()
                .position(|s| s.kind == Kind::ContestFiled)
                .unwrap();
            let queued = c
                .steps
                .iter()
                .position(|s| s.kind == Kind::ContestQueue)
                .unwrap();
            let started = c
                .steps
                .iter()
                .position(|s| s.kind == Kind::Start && s.binding.as_deref() == Some("contest:1"))
                .unwrap();
            for fault in 0..6 {
                let mut bad = c.steps.clone();
                match fault {
                    0 => bad[filed].kind = Kind::ContestDeclined,
                    1 => bad[filed].time += 1,
                    2 => bad[queued].binding = Some("contest999".into()),
                    3 => bad[queued].time += 1,
                    4 => bad[started].executor = Some("j".into()),
                    5 => bad.last_mut().unwrap().violation.as_mut().unwrap().deadline = Some(99),
                    _ => unreachable!(),
                }
                assert!(replay(&m, &bad, &c.id, c.case_id).is_err(), "fault {fault}");
            }
        }
    }
    #[test]
    fn d2_a8_response_and_execution_close_are_independently_replayed() {
        let src = include_str!("../../../../fixtures/ctx-contest/model.relca").replace(
            "contract Controls {",
            "contract Controls { invariant Route.respond.finish -> d == pass;",
        );
        let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        let report = crate::explain::explain(&src);
        let c = report
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id.ends_with(" invariant"))
            .unwrap();
        assert_eq!(c.status, x::ReplayStatus::Verified);
        let response = c
            .steps
            .iter()
            .position(|s| s.kind == Kind::ContestResponse)
            .unwrap();
        for fault in 0..4 {
            let mut bad = c.steps.clone();
            match fault {
                0 => bad[response].time += 1,
                1 => bad[response].executor = Some("j".into()),
                2 => bad[response].result = Some("fail".into()),
                3 => bad[response].kind = Kind::ExecutionClose,
                _ => unreachable!(),
            }
            assert!(
                replay(&m, &bad, &c.id, c.case_id).is_err(),
                "response fault {fault}"
            );
        }
        let prefix = |length: usize| {
            let mut i = interpreter(&m);
            let mut f = initial(&m);
            i.record(&mut f, Occurrence::Receipt, None).unwrap();
            for s in c.steps.iter().take(length).skip(1) {
                apply(&m, &mut i, &mut f, s, None).unwrap();
            }
            (i, f)
        };
        let mut close = c.steps[response].clone();
        close.kind = Kind::ExecutionClose;
        close.binding = None;
        close.actor = None;
        close.capability = None;
        close.executor = None;
        close.result = None;
        close.events.clear();
        close.violation = None;
        close.resource = None;
        close.capability_location = None;
        let (mut i, mut f) = prefix(c.steps.len());
        assert!(apply(&m, &mut i, &mut f, &close, None).is_ok());
        assert!(
            apply(&m, &mut i, &mut f, &close, None).is_err(),
            "closure cannot execute twice"
        );
        let (mut i, mut f) = prefix(response);
        assert!(
            apply(&m, &mut i, &mut f, &close, None).is_err(),
            "cannot close a running response"
        );
        let (mut i, mut f) = prefix(c.steps.len());
        close.time += 1;
        assert!(
            apply(&m, &mut i, &mut f, &close, None).is_err(),
            "forged closure clock"
        );
    }
    #[test]
    fn d1_a8_rejects_forged_selection_deadline_authority_person_and_time() {
        let src = include_str!("../../../../fixtures/ctx-waive/model.relca").replace("    waive", "    response audit: case.received -> one_of(B.run.finish) by 2min after case.received otherwise unresolved\n    waive");
        let (m, steps, id, case) = candidate(&src);
        assert!(replay(&m, &steps, &id, case).is_ok());
        let mut bad = steps.clone();
        bad[1].kind = Kind::End;
        assert!(replay(&m, &bad, &id, case).is_err(), "forged selection");
        let mut bad = steps.clone();
        *bad.last_mut()
            .unwrap()
            .violation
            .as_mut()
            .unwrap()
            .deadline
            .as_mut()
            .unwrap() += 1;
        assert_eq!(
            replay(&m, &bad, &id, case),
            Err(ReplayReason::ViolationMismatch)
        );
        let mut bad_model = m.clone();
        for c in &mut bad_model.contracts {
            if let ContractKind::Waive { authority, .. } = &mut c.kind {
                *authority = AuthorityId(usize::MAX);
            }
        }
        assert!(
            replay(&bad_model, &steps, &id, case).is_err(),
            "forged Authority"
        );
        let release = steps
            .iter()
            .position(|s| s.kind == Kind::Finish && s.actor.as_deref() == Some("Officer"))
            .unwrap();
        let mut bad = steps.clone();
        bad[release].executor = Some("issuer".into());
        assert!(replay(&m, &bad, &id, case).is_err(), "forged person");
        let mut bad = steps.clone();
        bad[release].time += 1;
        assert!(replay(&m, &bad, &id, case).is_err(), "forged time");
    }
    fn renumber(path: &mut [Stage]) {
        for (index, s) in path.iter_mut().enumerate() {
            s.index = index;
        }
        let last = path.last_mut().unwrap();
        if let Some(v) = &mut last.violation {
            v.at_step = last.index;
        }
    }
    #[test]
    fn rejects_time_after_join_outside_wait() {
        let src = include_str!("../../../../fixtures/explain-response/model.relca");
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let (_, capture) = crate::engine::trace::explore(src, &m);
        let mut path = vec![capture.initial.clone().unwrap()];
        for kind in [
            Kind::Queue,
            Kind::Start,
            Kind::Advance,
            Kind::Finish,
            Kind::Join,
        ] {
            path.push(
                capture
                    .edges
                    .iter()
                    .flatten()
                    .find(|e| e.stage.kind == kind)
                    .unwrap()
                    .stage
                    .clone(),
            );
        }
        let id = m.contracts[0].property_id(ClauseId(0));
        for kind in [Kind::Advance, Kind::Deadline] {
            let mut forged = path.clone();
            let mut last = forged[0].clone();
            last.kind = kind;
            last.time = 6;
            last.events.clear();
            last.violation = Some(x::Violation {
                state_invariant: None,
                id: id.clone(),
                property_location: None,
                at_step: 6,
                trigger_step: None,
                deadline: None,
                related_steps: vec![],
            });
            forged.push(last);
            renumber(&mut forged);
            assert_eq!(
                replay(&m, &forged, &id, Some(1)),
                Err(ReplayReason::IllegalTransition)
            );
        }
    }
    #[test]
    fn rejects_result_time_clause_and_deleted_or_swapped_operations() {
        let src =
            include_str!("../../../../fixtures/explain-once.broken-repeat-result/model.relca");
        let (m, path, id, case) = candidate(src);
        let mut bad = path.clone();
        bad.last_mut().unwrap().result = Some("not_a_member".into());
        assert!(replay(&m, &bad, &id, case).is_err());
        for delta in [-1i64, 1] {
            let mut bad = path.clone();
            let at = bad.iter().position(|s| s.kind == Kind::Advance).unwrap();
            bad[at].time = (bad[at].time as i64 + delta) as u64;
            assert!(replay(&m, &bad, &id, case).is_err());
        }
        let mut bad = path.clone();
        bad.last_mut().unwrap().violation.as_mut().unwrap().id = "another clause".into();
        assert_eq!(
            replay(&m, &bad, &id, case),
            Err(ReplayReason::ViolationMismatch)
        );
        let mut bad = path.clone();
        bad.remove(2);
        renumber(&mut bad);
        assert!(replay(&m, &bad, &id, case).is_err());
        let mut bad = path.clone();
        bad.swap(2, 3);
        renumber(&mut bad);
        assert!(replay(&m, &bad, &id, case).is_err());
    }
    #[test]
    fn global_prefix_cannot_omit_the_other_cases_resource_release() {
        let src = include_str!(
            "../../../../fixtures/arrivals-contracts.broken-short-response/model.relca"
        );
        let (m, mut path, id, case) = candidate(src);
        let at = path
            .iter()
            .position(|s| s.kind == Kind::Finish && s.case_id == Some(1))
            .unwrap();
        path.remove(at);
        renumber(&mut path);
        assert_eq!(
            replay(&m, &path, &id, case),
            Err(ReplayReason::IllegalTransition)
        );
    }
    #[test]
    fn same_person_cannot_be_occupied_twice_even_under_different_actors() {
        let src = "model M version \"0.2\" {type T=enum{ok} people[p,q] observed case{good:bool=false} actor A{executors[p];capability run()->T{duration fixed 1min;result nondet{ok}}} actor B{executors[p,q];capability run()->T{duration fixed 1min;result nondet{ok}}} process P{fork f{x=A.run();y=B.run()}join all f;end done} bounds{cases 1;live_tasks 2;clock 10min} contract C{invariant B.run.finish -> good == true}}";
        let (m, mut path, id, case) = candidate(src);
        let second = path
            .iter()
            .position(|s| s.kind == Kind::Start && s.actor.as_deref() == Some("B"))
            .unwrap();
        path[second].executor = Some("p".into());
        for event in &mut path[second].events {
            if let x::Event::Business(e) = event {
                e.executor = Some("p".into());
            }
        }
        let mut first = path[second].clone();
        let Stmt::Fork { branches, .. } = &m.process.body[0].kind else {
            unreachable!()
        };
        first.binding = Some("x".into());
        first.actor = Some("A".into());
        first.location = x::Location::new(src, branches[0].span).unwrap();
        first.capability_location = Some(x::Location::new(src, m.capabilities[0].span).unwrap());
        for event in &mut first.events {
            if let x::Event::Business(e) = event {
                e.binding = first.binding.clone();
                e.actor = first.actor.clone();
                e.location = first.location.clone();
            }
        }
        path.insert(second, first);
        renumber(&mut path);
        assert_eq!(
            replay(&m, &path, &id, case),
            Err(ReplayReason::IllegalTransition)
        );
    }
    #[test]
    fn zero_mass_kernel_result_is_not_a_legal_choice() {
        let src = include_str!("../../../../fixtures/prob-zero.broken-positive-write/model.relca");
        let (m, mut path, id, case) = candidate(src);
        let (_, capture) = crate::engine::trace::explore(src, &m);
        let (_, _, mut choices) = capture.shortest(&id).unwrap();
        assert!(choices.last().unwrap().is_some());
        choices.last_mut().unwrap().as_mut().unwrap().branch = 1;
        assert!(replay_choices(&m, &path, &id, case, &choices).is_err());
        let last = path.last_mut().unwrap();
        let cap = m
            .capabilities
            .iter()
            .find(|c| Some(c.name.as_str()) == last.capability.as_deref())
            .unwrap();
        let zero = cap
            .kernel
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .flat_map(|r| &r.branches)
            .find(|b| b.probability.is_zero())
            .unwrap();
        let result = m.types[cap.return_type.0].members[zero.result.0].clone();
        last.result = Some(result.clone());
        for event in &mut last.events {
            if let x::Event::Business(e) = event {
                e.result = Some(result.clone());
            }
        }
        assert!(replay(&m, &path, &id, case).is_err());
    }
    #[test]
    fn complete_hazard_product_and_cancel_semantics_are_required() {
        let src = include_str!("../../../../fixtures/duration-parallel/model.relca").replace(
            "  verify safety",
            "  contract C {invariant A.left.finish -> leftDone == false}\n  verify safety",
        );
        let (m, mut path, id, case) = candidate(&src);
        let gate = path
            .iter_mut()
            .find(|s| s.kind == Kind::DurationGate && s.events.len() == 2)
            .unwrap();
        gate.events.pop();
        assert!(replay(&m, &path, &id, case).is_err());
        let src=include_str!("../../../../fixtures/exec-cancel/model.relca").replace("  contract Controls {","  contract Controls {response Slow.work.start -> one_of(Slow.work.finish) by 10min after Slow.work.start otherwise unresolved");
        let (m, mut path, id, case) = candidate(&src);
        let ack = path.iter_mut().find(|s| s.kind == Kind::CancelAck).unwrap();
        ack.kind = Kind::Finish;
        ack.result = Some("ok".into());
        for event in &mut ack.events {
            if let x::Event::Business(e) = event {
                e.kind = Kind::Finish;
                e.result = Some("ok".into());
            }
        }
        assert!(replay(&m, &path, &id, case).is_err());
    }
    #[test]
    fn limits_are_not_silently_promoted_to_verified() {
        let src =
            include_str!("../../../../fixtures/explain-response.broken-late-finish/model.relca")
                .replace("fixed 6min", "fixed 10001min")
                .replace("clock 10min", "clock 20000min");
        let r = crate::explain::explain(&src);
        assert_eq!(r.report.verdict, Verdict::Violated);
        assert_eq!(
            r.explanation.counterexamples[0].reason,
            Some(ReplayReason::ReferenceLimit)
        );
        assert!(r.explanation.counterexamples[0].steps.is_empty());
    }
    #[test]
    fn separation_keeps_all_historical_left_executors() {
        let src = "model M version \"0.2\" {type T=enum{ok} people[p,q] actor L{executors[p,q];capability run()->T{duration fixed 1min;result nondet{ok}}} actor R{executors[p];capability run()->T{duration fixed 1min;result nondet{ok}}} process P{x=L.run();y=L.run();z=R.run();end done} bounds{cases 1;live_tasks 1;clock 10min} contract C{separation L.run.finish,R.run.finish}}";
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let mut i = interpreter(&m);
        let mut f = initial(&m);
        for (cap, person, time) in [
            (CapabilityId(0), PersonId(0), 1),
            (CapabilityId(0), PersonId(1), 2),
            (CapabilityId(1), PersonId(0), 3),
        ] {
            f.now = time;
            reference(i.record(
                &mut f,
                Occurrence::Completion(cap, ValueId(0)),
                Some(person),
            ))
            .unwrap();
        }
        assert!(i.contract_facts[0].1);
    }
}

#[cfg(test)]
mod resource_time_replay_impl_tests {
    use super::*;
    const SOURCE: &str = r#"model TimedReplay version "0.2" {
      type R=enum{ok}
      observed case {ready:bool=true}
      calendar Day through 21min on_close continue {[0min,3min) capacity 0;[3min,21min) capacity 1}
      actor Desk {capacity 1;calendar Day;capability run()->R{duration fixed 2min;result nondet{ok}}}
      process P {wait until ready == true;r=Desk.run();wait until ready == false;end done}
      bounds{cases 1;live_tasks 1;clock 20min}
      metrics{window 20min;currency "JPY";rate Desk per_minute 1;rate Desk.run per_start 0;breakdown waits}
      verify reachable end.done
    }"#;
    #[test]
    fn calendar_and_condition_deadlock_replays_and_changed_opening_is_rejected() {
        let explained = crate::explain::explain(SOURCE);
        assert_eq!(explained.report.verdict, crate::Verdict::Violated);
        let example = explained
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id == "diagnostic:W-DEADLOCK-0")
            .expect("deadlock counterexample");
        assert_eq!(
            example.replay.status,
            x::ReplayStatus::Verified,
            "{:?}",
            example.replay.reason
        );
        assert!(
            example
                .steps
                .iter()
                .any(|s| s.kind == Kind::Start && s.time == 3)
        );
        assert!(
            example
                .steps
                .iter()
                .any(|s| s.kind == Kind::Deadlock && s.time == 5)
        );
        let ast = crate::syntax::parse(SOURCE).unwrap();
        let mut model = crate::lower::lower(&ast).unwrap();
        replay(&model, &example.steps, &example.id, example.case_id).unwrap();
        let mut wrong_condition = model.clone();
        let Stmt::WaitUntil(Cond::StateEq(guard)) = &mut wrong_condition.process.body[0].kind
        else {
            panic!("condition witness")
        };
        let ty = wrong_condition.state_variables[guard.variable.0].base;
        let value = wrong_condition.types[ty.0]
            .members
            .iter()
            .position(|v| v == "false")
            .unwrap();
        guard.value = StateValue::Value(ValueId(value));
        assert!(
            replay(
                &wrong_condition,
                &example.steps,
                &example.id,
                example.case_id
            )
            .is_err(),
            "false condition must not replay as a pass"
        );
        let t = model.resource_time.as_mut().unwrap();
        t.calendars[0].intervals[0].until = 4;
        t.calendars[0].intervals[1].from = 4;
        model
            .resource_time
            .as_ref()
            .unwrap()
            .validate(&model)
            .unwrap();
        assert!(
            replay(&model, &example.steps, &example.id, example.case_id).is_err(),
            "closed scheduled start must not replay"
        );
    }
    #[test]
    fn shared_condition_race_deadlock_has_independently_verified_replay() {
        let source = include_str!("../../tests/support/q4/condition.relca");
        let source = source.replace(
            "finish { ready = true } }",
            "finish { ready = true } } capability clear() -> R {duration fixed 1min;result nondet {ok};start {ready = false}}",
        ).replace("if s == owner { end done }", "if s == owner { c = Board.clear(); end done }");
        let explained = crate::explain::explain(&source);
        let example = explained
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id == "diagnostic:W-DEADLOCK-0")
            .expect("shared deadlock witness");
        assert_eq!(
            example.replay.status,
            x::ReplayStatus::Verified,
            "{:?}",
            example.replay.reason
        );
        assert!(
            example
                .steps
                .iter()
                .any(|s| s.kind == Kind::Deadlock && s.time == 5)
        );
    }
}
