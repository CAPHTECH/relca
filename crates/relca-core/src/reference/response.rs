//! Independent history interpretation for ADR 0056; no engine transition helpers.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Receipt {
    #[cfg(test)]
    pub weight: Option<Rational>,
    pub command: BindingId,
    pub binding: BindingId,
    pub attempt: u64,
    pub actual: ValueId,
    pub observation: crate::ast::Receipt,
    pub resolved: bool,
}

pub(super) fn validate(m: &Model) -> Execution {
    recovery::validate(m)?;
    let attempts = m.response_attempts();
    if attempts.is_empty() {
        return Ok(());
    }
    fn layout(block: &Block, depth: usize) -> Execution {
        require(depth <= 256)?;
        let mut skip = 0;
        for (index, s) in block.iter().enumerate() {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            match &s.kind {
                Stmt::Response(_, r) => {
                    require((1..=32).contains(&r.max_attempts) && r.attempt == 1)?;
                    let end = index
                        .checked_add(r.max_attempts as usize)
                        .ok_or(StopReason::SemanticAbort)?;
                    require(end <= block.len())?;
                    for (j, next) in block[index..end].iter().enumerate() {
                        let Stmt::Response(_, n) = &next.kind else {
                            return Err(StopReason::SemanticAbort);
                        };
                        require(n.command == r.command && n.attempt == 1 + j as u64)?;
                    }
                    skip = r.max_attempts as usize - 1;
                }
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    layout(then_body, depth + 1)?;
                    layout(else_body, depth + 1)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    layout(&m.process.body, 0)?;
    require(
        m.response_attempts().is_empty()
            || (m.contests().is_empty()
                && m.quality.as_ref().is_none_or(|q| q.quantiles.is_empty())),
    )?;
    let mut next = BTreeMap::new();
    for (call, r) in attempts {
        let c = m
            .capabilities
            .get(call.capability.0)
            .ok_or(StopReason::SemanticAbort)?;
        require(r.idempotency.as_ref().is_none_or(|key| {
            r.idempotent
                && c.atomic.as_ref().is_some_and(|a| {
                    if let Cond::Contains {
                        element: Element::Parameter(index),
                        ..
                    } = a.condition
                    {
                        c.params.get(index).is_some_and(|p| p.name == *key)
                    } else {
                        false
                    }
                })
        }))?;
        let can_lose = match &r.loss {
            ResponseLoss::None => false,
            ResponseLoss::Sequence(xs) => xs.contains(&crate::ast::Receipt::Unknown),
            ResponseLoss::Probability { probability, .. } => probability != &Rational::zero(),
            ResponseLoss::Nondet => true,
        };
        require(
            m.process
                .bindings
                .get(call.binding.0)
                .is_some_and(|b| b.name == format!("@response:{}:{}", r.command.0, r.attempt))
                && !(r.retry_span.is_some() && c.version_check.is_some())
                && !(r.max_attempts > 1
                    && can_lose
                    && c.commits
                    && !r.idempotent
                    && !r.effect_modeled)
                && (1..=32).contains(&r.max_attempts)
                && r.command == r.command_call.binding
                && call.binding != r.command
                && call.capability == r.command_call.capability
                && call.args == r.command_call.args
                && m.process
                    .bindings
                    .get(r.command.0)
                    .is_some_and(|b| b.ty == c.return_type)
                && r.exhausted.is_none_or(|e| e.0 < m.ends.len())
                && r.exhausted.is_some() == r.retry_span.is_some(),
        )?;
        let index = next
            .entry(r.command)
            .or_insert((1, r.max_attempts, r.clone()));
        require(r.attempt == index.0 && r.max_attempts == index.1)?;
        let mut normalized = r.clone();
        normalized.attempt = 1;
        require(normalized == index.2)?;
        index.0 += 1;
        match &r.loss {
            ResponseLoss::Sequence(xs) => require(xs.len() as u64 == r.max_attempts)?,
            ResponseLoss::Probability {
                probability: p,
                basis,
            } => require(p.valid() && !basis.trim().is_empty())?,
            _ => {}
        }
        require(
            r.retry_results
                .iter()
                .all(|v| v.0 < m.types[c.return_type.0].members.len()),
        )?;
        let mechanism = c.atomic.as_ref().is_some_and(|a| match a.condition {
            Cond::Contains {
                set,
                element: Element::Parameter(i),
            } => {
                !a.commit_when
                    && c.commits
                    && c.start_writes.is_empty()
                    && c.params.get(i).is_some_and(|p| p.ty == CASE_ID_TYPE)
                    && m.state_variables.get(set.0).is_some_and(|v| {
                        v.kind == StateKind::ActorObserved(c.actor) && v.case_set_size.is_some()
                    })
                    && a.writes.iter().any(|w| {
                        w.variable == set && w.source == WriteSource::InsertCase { parameter: i }
                    })
            }
            _ => false,
        });
        require(r.idempotent == mechanism)?;
    }
    for (_, (seen, count, _)) in next {
        require(seen == count + 1)?;
    }
    Ok(())
}

pub(super) fn visible_values(
    f: &Frame<'_>,
    m: &Model,
    extra: Option<&Occurrence>,
) -> Vec<Option<ValueId>> {
    let mut values = vec![None; m.process.bindings.len()];
    for t in &f.tasks {
        match t.phase {
            Phase::Refused(v) => {
                if let Some(slot) = values.get_mut(t.call.binding.0) {
                    *slot = Some(v);
                }
            }
            Phase::Finished(v) if m.response_attempt(t.call.binding).is_none() => {
                if let Some(slot) = values.get_mut(t.call.binding.0) {
                    *slot = Some(v);
                }
            }
            _ => {}
        }
    }
    for event in f.trace.iter().map(|o| &o.event).chain(extra) {
        match event {
            Occurrence::Channel(r) if r.observation == crate::ast::Receipt::Delivered => {
                values[r.binding.0] = Some(r.actual)
            }
            Occurrence::Merged(id, _, v, _) | Occurrence::AdoptResponse(id, v) => {
                values[id.0] = Some(*v)
            }
            _ => {}
        }
    }
    for t in &f.tasks {
        if let Some(alias) = m.loop_alias(t.call.binding)
            && let Phase::Finished(v) | Phase::Refused(v) = t.phase
        {
            values[alias.0] = Some(v);
        }
    }
    values
}

pub(super) enum Progress {
    Next,
    Unknown,
    End(EndId),
}

pub(super) fn input(
    f: &Frame<'_>,
    m: &Model,
    call: &Call,
    r: &ResponseAttempt,
    case_index: usize,
) -> Result<Option<Occurrence>, StopReason> {
    if let Some(saved) = f.trace.iter().find_map(|o| match &o.event {
        Occurrence::ResponseInput(id, args) if *id == r.command => Some(args),
        _ => None,
    }) {
        let cap = &m.capabilities[call.capability.0];
        require(saved.len() == cap.params.len())?;
        for (value, param) in saved.iter().zip(&cap.params) {
            require(if param.ty == CASE_ID_TYPE {
                value.0 == case_index
            } else {
                value.0 < m.types[param.ty.0].members.len()
            })?;
        }
        return Ok(None);
    }
    let values = visible_values(f, m, None);
    let args = call
        .args
        .iter()
        .map(|id| {
            if *id == CURRENT_CASE {
                Ok(ValueId(case_index))
            } else {
                values
                    .get(id.0)
                    .copied()
                    .flatten()
                    .ok_or(StopReason::SemanticAbort)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    require(r.attempt == 1)?;
    Ok(Some(Occurrence::ResponseInput(r.command, args)))
}

pub(super) fn progress<'a>(
    f: &mut Frame<'a>,
    m: &'a Model,
    now: Tick,
) -> Result<(Progress, Vec<Occurrence>), StopReason> {
    let wait = f.waiting.as_ref().ok_or(StopReason::SemanticAbort)?;
    require(wait.tasks.len() == 1)?;
    let t = &f.tasks[wait.tasks.start];
    let r = m
        .response_attempt(t.call.binding)
        .ok_or(StopReason::SemanticAbort)?;
    let observation = f.trace.iter().rev().find_map(|o| match &o.event {
        Occurrence::Channel(p) if p.binding == t.call.binding => Some(p),
        _ => None,
    });
    let unknown = observation.is_some_and(|p| p.observation == crate::ast::Receipt::Unknown);
    let actual = match t.phase {
        Phase::Finished(v) | Phase::Refused(v) => v,
        _ => return Err(StopReason::SemanticAbort),
    };
    let retry = if unknown {
        r.retry_unknown
    } else {
        r.retry_results.contains(&actual)
    };
    f.waiting = None;
    if retry && r.attempt < r.max_attempts {
        let wake = now
            .checked_add(r.backoff)
            .ok_or(StopReason::SemanticAbort)?;
        return Ok((Progress::Next, vec![Occurrence::Wait(wake)]));
    }
    let unresolved = f
        .trace
        .iter()
        .rev()
        .find_map(|o| match &o.event {
            Occurrence::Channel(p) if p.command == r.command => Some(!p.resolved),
            _ => None,
        })
        .unwrap_or(false);
    if unknown || unresolved {
        if let Some(body) = &r.recovery {
            require(
                !f.trace
                    .iter()
                    .any(|o| matches!(o.event, Occurrence::RecoveryEnter(_))),
            )?;
            f.control.clear();
            f.control.push(body);
            return Ok((Progress::Next, vec![Occurrence::RecoveryEnter(r.command)]));
        }
        return Ok((
            Progress::Unknown,
            vec![Occurrence::ResponseStopped(r.command)],
        ));
    }
    if let Some(block) = f.control.last_mut() {
        while block.first().is_some_and(
            |s| matches!(&s.kind, Stmt::Response(_, other) if other.command == r.command),
        ) {
            *block = &block[1..];
        }
    }
    let event = Occurrence::AdoptResponse(r.command, actual);
    if retry {
        Ok((
            Progress::End(r.exhausted.ok_or(StopReason::SemanticAbort)?),
            vec![event],
        ))
    } else {
        Ok((Progress::Next, vec![event]))
    }
}

impl<'a> Interpreter<'a> {
    pub(super) fn response_finish(
        &mut self,
        mut f: Frame<'a>,
        i: usize,
        depth: usize,
    ) -> Execution {
        let task = &f.tasks[i];
        let Some(r) = self.model.response_attempt(task.call.binding) else {
            if self.model.capabilities[task.call.capability.0]
                .receipt_query
                .is_some()
            {
                let Phase::Finished(actual) = task.phase else {
                    return Err(StopReason::SemanticAbort);
                };
                let binding = task.call.binding;
                let event = recovery::query(
                    &f,
                    self.model,
                    binding,
                    actual,
                    &f.state_values,
                    self.case_index,
                )?;
                self.record_site(&mut f, event, None, binding)?;
            }
            return self.run(f, depth);
        };
        let Phase::Finished(actual) = task.phase else {
            return Err(StopReason::SemanticAbort);
        };
        let mut choices = Vec::new();
        use crate::ast::Receipt::{Delivered, Unknown};
        match &r.loss {
            ResponseLoss::None => choices.push((Some(Rational::one()), Delivered)),
            ResponseLoss::Sequence(xs) => {
                choices.push((Some(Rational::one()), xs[r.attempt as usize - 1]))
            }
            ResponseLoss::Nondet => choices.extend([(None, Delivered), (None, Unknown)]),
            ResponseLoss::Probability { probability: p, .. } => {
                if p != &Rational::one() {
                    choices.push((Some(Rational(Rational::one().0 - &p.0)), Delivered));
                }
                if !p.is_zero() {
                    choices.push((Some(p.clone()), Unknown));
                }
            }
        }
        let probabilistic = choices.iter().all(|(p, _)| p.is_some());
        let weights: Vec<_> = choices.iter().filter_map(|(p, _)| p.clone()).collect();
        self.probability_stack.push(ReferenceChoices::default());
        let binding = task.call.binding;
        let person = task.person;
        for (_weight, observation) in choices {
            self.defer_successor(&f,depth,move |this,_,mut next| {
            let was_unknown = next.trace.iter().any(|o| matches!(&o.event, Occurrence::Channel(p) if p.command == r.command && p.attempt < r.attempt && !p.resolved));
            let receipt = Receipt {
                #[cfg(test)]
                weight: _weight,
                command: r.command,
                binding,
                attempt: r.attempt,
                actual,
                observation,
                resolved: observation == Delivered && (r.idempotent || !was_unknown),
            };
            this.record_site(
                &mut next,
                Occurrence::Channel(receipt),
                person,
                binding,
            )?;
            this.run(next,depth)
            })?;
        }
        self.probability_finish(probabilistic.then_some(weights.as_slice()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_visibility_does_not_expose_server_phase() {
        let source = include_str!("../../../../fixtures/k8-cut-g/model.relca").replace(
            "p = Ledger.post(case.id)",
            "p = Ledger.post(case.id) response_loss sequence [unknown]",
        );
        let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
        validate(&m).unwrap();
        let (call, rule) = m.response_attempts()[0];
        let actual = m.capabilities[call.capability.0]
            .atomic
            .as_ref()
            .unwrap()
            .commit;
        let mut f = Frame {
            condition_waits: Vec::new(),
            lost_updates: BTreeSet::new(),
            state_snapshots: Vec::new(),
            business: None,
            state_values: Vec::new(),
            trace: Vec::new(),
            now: 1,
            steps: 0,
            control: Vec::new(),
            waiting: None,
            tasks: vec![Task {
                frozen_performance: None,
                stale: BTreeSet::new(),
                metric_ready: None,
                checked_through: None,
                person: None,
                call: call.clone(),
                phase: Phase::Finished(actual),
                start: Some(0),
                stop: Some(1),
                finish: Some(1),
                ack: None,
            }],
            adoption: None,
        };
        assert!(visible_values(&f, &m, None).iter().all(Option::is_none));
        let receipt = Receipt {
            weight: None,
            command: rule.command,
            binding: call.binding,
            attempt: 1,
            actual,
            observation: crate::ast::Receipt::Unknown,
            resolved: false,
        };
        f.trace.push(Observation {
            binding: Some(call.binding),
            state_values: Vec::new(),
            person: None,
            at: 1,
            event: Occurrence::Channel(receipt.clone()),
            values: Vec::new(),
        });
        assert!(visible_values(&f, &m, None).iter().all(Option::is_none));
        let mut known = receipt;
        known.observation = crate::ast::Receipt::Delivered;
        assert_eq!(
            visible_values(&f, &m, Some(&Occurrence::Channel(known)))[call.binding.0],
            Some(actual)
        );
        assert!(visible_values(&f, &m, None)[rule.command.0].is_none());
        assert_eq!(
            visible_values(
                &f,
                &m,
                Some(&Occurrence::AdoptResponse(rule.command, actual))
            )[rule.command.0],
            Some(actual)
        );
        let mut forged = m.clone();
        let Stmt::Response(_, r) = &mut forged.process.body[1].kind else {
            panic!("response");
        };
        r.max_attempts = 33;
        assert!(validate(&forged).is_err());
        let mut renamed = m.clone();
        renamed.process.bindings[call.binding.0].name = "forged_call".into();
        assert!(validate(&renamed).is_err());
    }

    #[test]
    fn reference_reuses_shared_keys_and_has_independent_bounded_control() {
        let base = include_str!("../../../../fixtures/k8-cut-g/model.relca").replace(
            "p = Ledger.post(case.id)",
            "p = Ledger.post(case.id) response_loss sequence [unknown, delivered] retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed",
        );
        for arrivals in [false, true] {
            let source = if arrivals {
                base.replace("cases 1", "cases 2").replacen(
                    "bounds {",
                    "arrivals fixed [0min,4min] policy fifo tie_by case_id bounds {",
                    1,
                )
            } else {
                base.clone()
            };
            let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
            let reference = super::super::explore(&m);
            assert_eq!(reference.stop, StopReason::Complete);
            assert!(!reference.outcome.truncated);
            if arrivals {
                let cases = reference.outcome.case_outcomes.unwrap();
                assert_eq!(cases.len(), 2);
                for (k, c) in cases.iter().enumerate() {
                    assert_eq!(c.terminals.len(), 1);
                    assert_eq!(c.terminals[0].completed_at.unwrap().min, 3 + 4 * k as u64);
                    assert_eq!(c.terminals[0].elapsed.unwrap().min, 3);
                    assert_eq!(c.terminals[0].work.unwrap().min, 3);
                }
            } else {
                assert_eq!(reference.outcome.terminals.len(), 1);
                assert_eq!(reference.outcome.terminals[0].elapsed.min, 3);
                assert_eq!(reference.outcome.terminals[0].work.min, 3);
            }
        }
    }

    #[test]
    fn reference_input_snapshot_is_not_reread_on_later_attempts() {
        let source = r#"model M version "0.2" {type V=enum {pass,fail}
            actor A {capacity 1;capability read()->V {duration fixed 1min;result nondet {pass}}
                capability check(x:V)->V {duration fixed 1min;result nondet {fail}}}
            process P {raw=A.read();v=A.check(raw) response_loss none retry max_attempts 2 when result [fail] backoff fixed 0min exhausted end failed;end done}
            bounds {cases 1;live_tasks 1;clock 5min} verify safety no_resource_overcommit }"#;
        let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        let Stmt::Call(raw) = &m.process.body[0].kind else {
            panic!("raw")
        };
        let attempts = m.response_attempts();
        let (first, r) = attempts[0];
        let (second, last) = attempts[1];
        let mut f = super::super::replay::initial(&m);
        f.tasks.push(Task {
            frozen_performance: None,
            stale: BTreeSet::new(),
            metric_ready: None,
            checked_through: None,
            person: None,
            call: raw.clone(),
            phase: Phase::Finished(ValueId(0)),
            start: Some(0),
            stop: Some(1),
            finish: Some(1),
            ack: None,
        });
        let event = input(&f, &m, first, r, 0).unwrap().unwrap();
        f.trace.push(Observation {
            binding: None,
            state_values: Vec::new(),
            person: None,
            at: 1,
            event,
            values: Vec::new(),
        });
        f.tasks[0].phase = Phase::Finished(ValueId(1));
        assert!(input(&f, &m, second, last, 0).unwrap().is_none());
        assert!(
            matches!(&f.trace[0].event,Occurrence::ResponseInput(id,args) if *id==r.command && args==&vec![ValueId(0)])
        );
    }
}
