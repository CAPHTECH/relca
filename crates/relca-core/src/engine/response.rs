//! Receipt state belongs to the caller; actual completion remains server-side.
use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(super) struct History {
    pub recovery: Option<recovery::History>,
    pub started: BTreeMap<ir::BindingId, ir::CapabilityId>,
    pub actuals: BTreeMap<ir::BindingId, ir::ValueId>,
    pub rows: Vec<Observation>,
    pub inputs: BTreeMap<ir::BindingId, Vec<ir::ValueId>>,
    pub unresolved: BTreeSet<ir::BindingId>,
    pub stopped: bool,
}

impl History {
    pub(super) fn for_model(m: &ir::Model) -> Self {
        Self {
            recovery: m.recovery().map(|_| recovery::History::default()),
            ..Self::default()
        }
    }
}

pub(super) fn deliver(
    state: &State,
    binding: ir::BindingId,
    actual: ir::ValueId,
    model: &ir::Model,
    room: &mut dyn Generation,
) -> EngineResult<Vec<(Option<Rational>, State)>> {
    let Some(r) = model.response_attempt(binding) else {
        return Ok(vec![(Some(Rational::one()), state.clone())]);
    };
    use crate::ast::Receipt::{Delivered, Unknown};
    let mut alternatives = match &r.loss {
        ir::ResponseLoss::None => vec![(Some(Rational::one()), Delivered)],
        ir::ResponseLoss::Sequence(xs) => vec![(
            Some(Rational::one()),
            *xs.get(r.attempt as usize - 1)
                .ok_or("receipt sequence index")?,
        )],
        ir::ResponseLoss::Nondet => vec![(None, Delivered), (None, Unknown)],
        ir::ResponseLoss::Probability { probability: p, .. } => {
            let mut xs = Vec::new();
            if p != &Rational::one() {
                xs.push((Some(Rational(Rational::one().0 - &p.0)), Delivered));
            }
            if !p.is_zero() {
                xs.push((Some(p.clone()), Unknown));
            }
            xs
        }
    };
    if let Some(sample) = room.sampling() {
        if alternatives.iter().any(|(p, _)| p.is_none()) {
            return sample.reject();
        }
        if matches!(r.loss, ir::ResponseLoss::Probability { .. }) {
            let site = simulation::Sample::task_site(model, binding, "receipt");
            let selected = sample.choose(
                sample.case,
                &site,
                1,
                &alternatives
                    .iter()
                    .map(|(p, _)| p.clone().unwrap())
                    .collect::<Vec<_>>(),
            )?;
            let chosen = alternatives.remove(selected);
            alternatives = vec![(Some(Rational::one()), chosen.1)];
        }
    }
    room.take(alternatives.len())?;
    alternatives
        .into_iter()
        .map(|(weight, receipt)| {
            let mut next = state.clone();
            let h = next.response.as_mut().ok_or("missing receipt history")?;
            #[cfg(test)]
            if let Some(r) = &mut h.recovery {
                r.weights.push(weight.clone());
            }
            if receipt == Unknown {
                h.unresolved.insert(r.command);
            } else if r.idempotent {
                h.unresolved.remove(&r.command);
            }
            let resolved_effect = !h.unresolved.contains(&r.command);
            h.rows.push(Observation {
                expected_state: state.state_values.clone(),
                effect_retained: true,
                command: r.command,
                binding,
                attempt: r.attempt,
                at: next.time,
                actual,
                receipt,
                resolved_effect,
            });
            if receipt == Delivered {
                next.results[binding.0] = Some(actual);
            }
            Ok((weight, next))
        })
        .collect()
}

pub(super) fn kernel_actions(
    state: &State,
    i: usize,
    kernel: &ir::Kernel,
    m: &ir::Model,
    room: &mut dyn Generation,
    e: &mut Exploration,
    case_index: usize,
) -> EngineResult<Vec<Vec<(Rational, State)>>> {
    let raw = kernel_complete_in(state, i, kernel, m, room, e, case_index)?;
    let binding = state.tasks[i].binding;
    if m.response_attempt(binding).is_none() {
        return Ok(vec![raw]);
    }
    let mut actions: Vec<Vec<(Rational, State)>> = vec![Vec::new()];
    for (kernel_weight, server) in raw {
        let actual = *server
            .response
            .as_ref()
            .and_then(|h| h.actuals.get(&binding))
            .ok_or("missing server result")?;
        let channel = deliver(&server, binding, actual, m, room)?;
        if channel.iter().all(|(p, _)| p.is_some()) {
            room.take(
                actions
                    .len()
                    .checked_mul(channel.len())
                    .ok_or("response product overflow")?,
            )?;
            for action in &mut actions {
                for (p, s) in &channel {
                    action.push((
                        Rational(&kernel_weight.0 * &p.as_ref().unwrap().0),
                        s.clone(),
                    ));
                }
            }
        } else {
            let cells = actions
                .iter()
                .try_fold(0usize, |n, a| {
                    n.checked_add((a.len() + 1).checked_mul(channel.len())?)
                })
                .ok_or("response product overflow")?;
            room.take(cells)?;
            let mut extended = Vec::new();
            for action in actions {
                for (_, s) in &channel {
                    let mut a = action.clone();
                    a.push((kernel_weight.clone(), s.clone()));
                    extended.push(a);
                }
            }
            actions = extended;
        }
    }
    Ok(actions)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Observation {
    pub expected_state: Vec<ir::StateValue>,
    pub effect_retained: bool,
    pub command: ir::BindingId,
    pub binding: ir::BindingId,
    pub attempt: u64,
    pub at: u64,
    pub actual: ir::ValueId,
    pub receipt: crate::ast::Receipt,
    pub resolved_effect: bool,
}

pub(super) fn capture(
    state: &mut State,
    call: &ir::Call,
    model: &ir::Model,
    case_index: usize,
) -> EngineResult<()> {
    if state.response.is_none() {
        return Ok(());
    }
    let Some(r) = model.response_attempt(call.binding) else {
        return Ok(());
    };
    if let Some(saved) = state
        .response
        .as_ref()
        .and_then(|h| h.inputs.get(&r.command))
    {
        let cap = &model.capabilities[call.capability.0];
        if saved.len() != cap.params.len()
            || saved.iter().zip(&cap.params).any(|(value, p)| {
                if p.ty == ir::CASE_ID_TYPE {
                    value.0 != case_index
                } else {
                    value.0 >= model.types[p.ty.0].members.len()
                }
            })
        {
            return Err("invalid saved command inputs".into());
        }
        return Ok(());
    }
    if r.attempt != 1 {
        return Err("missing initial command input snapshot".into());
    }
    let inputs = call
        .args
        .iter()
        .map(|arg| {
            if *arg == ir::CURRENT_CASE {
                Ok(ir::ValueId(case_index))
            } else {
                state
                    .results
                    .get(arg.0)
                    .copied()
                    .flatten()
                    .ok_or_else(|| Halt::from("response input unavailable"))
            }
        })
        .collect::<EngineResult<Vec<_>>>()?;
    let h = state
        .response
        .as_mut()
        .ok_or("response history unavailable")?;
    h.inputs.insert(r.command, inputs);
    Ok(())
}

pub(super) fn advance(
    state: &mut State,
    binding: ir::BindingId,
    r: &ir::ResponseAttempt,
    retry: Option<usize>,
    failure: Option<usize>,
    next: usize,
) -> EngineResult<()> {
    let value = state.results.get(binding.0).copied().flatten();
    let h = state
        .response
        .as_mut()
        .ok_or("response history unavailable")?;
    let unknown = h
        .rows
        .iter()
        .any(|row| row.binding == binding && row.receipt == crate::ast::Receipt::Unknown);
    let again = if unknown {
        r.retry_unknown
    } else {
        value.is_some_and(|v| r.retry_results.contains(&v))
    };
    if again && let Some(retry) = retry {
        state.pc = retry;
        return Ok(());
    }
    if unknown || h.unresolved.contains(&r.command) {
        h.stopped = true;
        return Ok(());
    }
    let value = value.ok_or("response completed without receipt or refusal")?;
    state.results[r.command.0] = Some(value);
    if let Some(h) = &mut state.evidence {
        h.adopt(r.command, binding);
    }
    if again {
        state.pc = failure.ok_or("missing exhausted instruction")?;
    } else {
        state.pc = next;
    }
    Ok(())
}

pub(super) fn validate(m: &ir::Model) -> EngineResult<()> {
    recovery::validate(m)?;
    let attempts = m.response_attempts();
    if attempts.is_empty() {
        return Ok(());
    }
    fn consecutive(block: &ir::Block, depth: usize) -> EngineResult<()> {
        if depth > 256 {
            return Err("retry control depth exceeded".into());
        }
        let mut i = 0;
        while i < block.len() {
            match &block[i].kind {
                ir::Stmt::Response(_, r) => {
                    let n = usize::try_from(r.max_attempts)
                        .map_err(|_| Halt::from("retry expansion bound"))?;
                    if r.attempt != 1 || !(1..=32).contains(&n) || n > block.len() - i {
                        return Err("nonconsecutive retry expansion".into());
                    }
                    for (j, s) in block[i..i + n].iter().enumerate() {
                        if !matches!(&s.kind,ir::Stmt::Response(_,q) if q.command==r.command && q.attempt==j as u64+1)
                        {
                            return Err("nonconsecutive retry expansion".into());
                        }
                    }
                    i += n;
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    consecutive(then_body, depth + 1)?;
                    consecutive(else_body, depth + 1)?;
                    i += 1;
                }
                _ => i += 1,
            }
        }
        Ok(())
    }
    consecutive(&m.process.body, 0)?;
    if !m.response_attempts().is_empty()
        && (!m.contests().is_empty() || m.quality.as_ref().is_some_and(|q| !q.quantiles.is_empty()))
    {
        return Err("retry_feature_combination".into());
    }
    let mut commands: BTreeMap<ir::BindingId, Vec<(&ir::Call, &ir::ResponseAttempt)>> =
        BTreeMap::new();
    for (call, r) in attempts {
        let original = &r.command_call;
        let cap = m
            .capabilities
            .get(call.capability.0)
            .ok_or("response capability out of range")?;
        let mechanism = cap.atomic.as_ref().is_some_and(|a| {
            let ir::Cond::Contains {
                set,
                element: ir::Element::Parameter(i),
            } = a.condition
            else {
                return false;
            };
            cap.commits
                && !a.commit_when
                && cap.start_writes.is_empty()
                && cap.params.get(i).is_some_and(|p| p.ty == ir::CASE_ID_TYPE)
                && m.state_variables.get(set.0).is_some_and(|v| {
                    v.case_set_size.is_some() && v.kind == ir::StateKind::ActorObserved(cap.actor)
                })
                && a.writes.iter().any(|w| {
                    w.variable == set && w.source == ir::WriteSource::InsertCase { parameter: i }
                })
        });
        let can_lose = match &r.loss {
            ir::ResponseLoss::None => false,
            ir::ResponseLoss::Sequence(xs) => xs.contains(&crate::ast::Receipt::Unknown),
            ir::ResponseLoss::Probability { probability, .. } => !probability.is_zero(),
            ir::ResponseLoss::Nondet => true,
        };
        if r.command != original.binding
            || m.process
                .bindings
                .get(call.binding.0)
                .is_none_or(|b| b.name != format!("@response:{}:{}", r.command.0, r.attempt))
            || r.retry_span.is_some() && cap.version_check.is_some()
            || r.max_attempts > 1 && can_lose && cap.commits && !r.idempotent && !r.effect_modeled
            || r.idempotent != mechanism
            || r.idempotency.as_ref().is_some_and(|key| {
                !mechanism
                    || !cap.atomic.as_ref().is_some_and(|a| match a.condition {
                        ir::Cond::Contains {
                            element: ir::Element::Parameter(i),
                            ..
                        } => cap.params.get(i).is_some_and(|p| p.name == *key),
                        _ => false,
                    })
            })
            || r.command == call.binding
            || original.capability != call.capability
            || original.args != call.args
            || m.process
                .bindings
                .get(r.command.0)
                .is_none_or(|b| b.ty != cap.return_type)
            || !(1..=32).contains(&r.max_attempts)
            || r.attempt == 0
            || r.attempt > r.max_attempts
            || r.retry_results
                .iter()
                .any(|v| v.0 >= m.types[cap.return_type.0].members.len())
            || r.exhausted.is_some_and(|e| e.0 >= m.ends.len())
            || r.retry_span.is_some() != r.exhausted.is_some()
        {
            return Err("invalid response command identity or retry contract".into());
        }
        match &r.loss {
            ir::ResponseLoss::Sequence(xs) if xs.len() as u64 != r.max_attempts => {
                return Err("invalid response sequence".into());
            }
            ir::ResponseLoss::Probability {
                probability: p,
                basis,
            } if p < &Rational::zero() || p > &Rational::one() || basis.trim().is_empty() => {
                return Err("invalid response probability".into());
            }
            _ => {}
        }
        commands.entry(r.command).or_default().push((call, r));
    }
    for rows in commands.values() {
        let first = rows[0].1;
        if rows.len() as u64 != first.max_attempts {
            return Err("incomplete response expansion".into());
        }
        for (i, (_, r)) in rows.iter().enumerate() {
            let mut canonical = (*r).clone();
            canonical.attempt = 1;
            if canonical != *first || r.attempt != i as u64 + 1 {
                return Err("inconsistent response expansion".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::{Hash, Hasher};

    #[test]
    fn hidden_receipt_components_are_part_of_state_identity() {
        let mut h = History::default();
        h.rows.push(Observation {
            expected_state: Vec::new(),
            effect_retained: true,
            command: ir::BindingId(0),
            binding: ir::BindingId(1),
            attempt: 1,
            at: 1,
            actual: ir::ValueId(0),
            receipt: crate::ast::Receipt::Unknown,
            resolved_effect: false,
        });
        let hash = |h: &History| {
            let mut s = std::collections::hash_map::DefaultHasher::new();
            h.hash(&mut s);
            s.finish()
        };
        for component in 0..11 {
            let mut changed = h.clone();
            match component {
                0 => changed.rows[0].actual = ir::ValueId(1),
                1 => changed.rows[0].receipt = crate::ast::Receipt::Delivered,
                2 => changed.rows[0].attempt = 2,
                3 => changed.rows[0].command = ir::BindingId(2),
                4 => {
                    changed
                        .inputs
                        .insert(ir::BindingId(0), vec![ir::ValueId(1)]);
                }
                5 => {
                    changed.unresolved.insert(ir::BindingId(0));
                }
                6 => changed.rows[0].resolved_effect = true,
                7 => changed.stopped = true,
                8 => changed.rows[0].expected_state = vec![ir::StateValue::Integer(1)],
                9 => changed.rows[0].effect_retained = false,
                _ => {
                    changed
                        .started
                        .insert(ir::BindingId(1), ir::CapabilityId(0));
                }
            }
            assert_ne!(h, changed);
            assert_ne!(hash(&h), hash(&changed));
        }
    }

    #[test]
    fn public_retry_ir_rejects_bound_sequence_and_identity_forgery() {
        let source = include_str!("../../../../fixtures/k8-cut-g/model.relca").replace(
            "p = Ledger.post(case.id)",
            "p = Ledger.post(case.id) response_loss sequence [unknown, delivered] retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed",
        );
        let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
        assert!(validate(&m).is_ok());
        for fault in 0..10 {
            let mut changed = m.clone();
            let ir::Stmt::Response(call, r) = &mut changed.process.body[1].kind else {
                panic!("response");
            };
            match fault {
                0 => r.max_attempts = 0,
                1 => r.max_attempts = 33,
                2 => r.loss = ir::ResponseLoss::Sequence(vec![crate::ast::Receipt::Unknown]),
                3 => r.attempt = 2,
                4 => r.command = call.binding,
                5 => r.command_call.args.clear(),
                6 => r.command_call.capability = ir::CapabilityId(0),
                7 => r.idempotent = false,
                8 => changed.process.bindings[call.binding.0].name = "forged_call".into(),
                _ => {
                    changed.process.body.swap(1, 2);
                }
            }
            assert!(validate(&changed).is_err(), "forgery {fault}");
        }
    }

    #[test]
    fn completion_keeps_real_effect_but_never_publishes_unreceived_result() {
        let s = r#"model M version "0.2" {
            type V=enum { ok }
            observed case { n:0..2=0 }
            actor A { capacity 1; capability c() -> V { commits; effect_scope modeled;
                writes n; duration fixed 1min; result nondet { ok }; finish { n += 1 } } }
            process P { r=A.c() response_loss nondet; end done }
            contract C { at_most_once A.c.commit per case.id }
            bounds { cases 1; live_tasks 1; clock 10min }
            verify safety no_resource_overcommit
        }"#;
        let m = crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap();
        let (call, metadata) = m.response_attempts()[0];
        let mut state = State::initial(&m, 0);
        state.time = 1;
        state.work = 1;
        state.tasks.push(Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: call.binding,
            capability: call.capability,
            lifecycle: Lifecycle::Running {
                finish: 1,
                ack: None,
            },
        });
        let mut e = Exploration::default();
        let branches = complete_in(
            &state,
            Due {
                i: 0,
                finish: 1,
                ack: None,
            },
            &m,
            &mut Room(32),
            &mut e,
            0,
        )
        .unwrap();
        assert_eq!(branches.len(), 2);
        for branch in &branches {
            assert_eq!(branch.state_values[0], ir::StateValue::Integer(1));
            assert_eq!(branch.tasks[0].lifecycle, Lifecycle::Done);
            assert!(branch.results[metadata.command.0].is_none());
            assert_eq!(branch.monitors[0].count, 1);
            let h = branch.response.as_ref().unwrap();
            assert_eq!(h.rows.len(), 1);
            assert_eq!(h.rows[0].actual, ir::ValueId(0));
            assert_eq!(
                branch.results[call.binding.0].is_some(),
                h.rows[0].receipt == crate::ast::Receipt::Delivered
            );
        }
        assert_ne!(branches[0], branches[1]);
    }

    #[test]
    fn bounded_tasks_use_total_attempts_and_backoff_without_work() {
        for attempts in [1, 2, 32] {
            let source = format!(
                r#"model M version "0.2" {{
                type V=enum {{ pass, fail }}
                actor A {{ capacity 1; capability c() -> V {{ duration fixed 1min; result nondet {{ fail }} }} }}
                process P {{ v=A.c() response_loss none retry max_attempts {attempts} when result [fail] backoff fixed 3min exhausted end failed; end ok }}
                bounds {{ cases 1; live_tasks 1; clock 200min }}
                verify safety no_resource_overcommit
            }}"#
            );
            let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
            let o = explore(&m);
            assert!(
                o.diagnostics
                    .iter()
                    .all(|d| d.severity != diag::Severity::Error),
                "{:?}",
                o.diagnostics
            );
            assert!(!o.truncated);
            assert_eq!(o.terminals.len(), 1);
            let t = &o.terminals[0];
            assert_eq!(m.ends[t.end.0], "failed");
            assert_eq!(t.elapsed.min, attempts + 3 * (attempts - 1));
            assert_eq!(t.work.min, attempts);
            assert_eq!(t.work.max, attempts);
        }
    }

    #[test]
    fn unknown_count_is_unfinished_subset_and_preserves_observed_fees() {
        for unknown in [true, false] {
            let source = format!(
                r#"model M version "0.2" {{
                type V=enum {{ ok }}
                actor A {{ capacity 1; capability c() -> V {{ duration fixed 1min; result nondet {{ ok }} }} }}
                process P {{ v=A.c() response_loss sequence [{}]; end done }}
                bounds {{ cases 1; live_tasks 1; clock 10min }}
                metrics {{ window 10min; currency "JPY"; rate A per_minute 1; rate A.c per_start 2 }}
                verify safety no_resource_overcommit
            }}"#,
                if unknown { "unknown" } else { "delivered" }
            );
            let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
            let independent = crate::reference::explore(&m);
            assert_eq!(independent.stop, crate::reference::StopReason::Complete);
            for outcome in [explore(&m), independent.outcome] {
                let v = serde_json::to_value(outcome.metrics.unwrap()).unwrap();
                let n = |n: u64| serde_json::json!({"min":n.to_string(),"max":n.to_string(),"reason":null});
                assert_eq!(
                    v["overall"]["counts"]["completed"]["stats"],
                    n(u64::from(!unknown))
                );
                assert_eq!(
                    v["overall"]["counts"]["unfinished"]["stats"],
                    n(u64::from(unknown))
                );
                assert_eq!(
                    v["overall"]["counts"]["receipt_unknown_count"]["stats"],
                    n(u64::from(unknown))
                );
                assert_eq!(v["overall"]["counts"]["deadlocked"]["stats"], n(0));
                assert_eq!(v["overall"]["work"], n(1));
                assert_eq!(v["overall"]["observed_cost_total"], n(3));
            }
        }
    }

    #[test]
    fn receipt_random_keys_separate_attempts_and_match_independent_sampler() {
        let source = r#"model M version "0.2" {
            type Ack=enum { committed, duplicate }
            actor L {capacity 1; state keys:set<CaseId>; capability post(key:CaseId)->Ack {
                commits; idempotency key key; writes keys; duration fixed 1min;
                atomic {if key in keys {result duplicate} else {keys+=key; result committed}} } }
            process P {r=L.post(case.id) response_loss probability 0.5 basis "independent channel test assumption"
                retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed; end done}
            bounds {cases 1; live_tasks 1; clock 5min}
            simulation replications 100 seed 7 confidence 0.95
            verify safety no_resource_overcommit
        }"#;
        let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        let e = simulate(&m);
        let r = crate::reference::simulate(&m);
        assert_eq!(
            r.stop,
            crate::reference::StopReason::Complete,
            "{:?}",
            r.outcome.diagnostics
        );
        assert_eq!(e.completed_replications, 100, "{:?}", e.diagnostics);
        assert_eq!(e.replications, r.outcome.replications);
        assert!(e.replications.iter().any(|r| r.draws.len() == 2));
        for replication in e.replications {
            assert!(
                replication.draws[0]
                    .key
                    .contains("site=receipt.P.r.attempt1|n=1")
            );
            if replication.draws.len() == 2 {
                assert!(
                    replication.draws[1]
                        .key
                        .contains("site=receipt.P.r.attempt2|n=1")
                );
            }
        }
    }

    #[test]
    fn retry_freezes_first_arguments_without_freezing_mutable_kernel_state() {
        let source = r#"model M version "0.2" {type V=enum {pass,fail}
            latent case {env:bool=false}
            kernel K given [env] {false->{1.0:pass};true->{1.0:fail}}
            actor Input {capacity 1;capability read()->V {duration fixed 1min;result nondet {pass}}}
            actor C {capacity 1;capability run(x:V)->V {duration fixed 1min;outcomes kernel K;kernel_reads latent [env]}}
            process P {raw=Input.read();v=C.run(raw) response_loss none retry max_attempts 2 when result [fail] backoff fixed 0min exhausted end failed;end done}
            bounds {cases 1;live_tasks 1;clock 5min} verify safety no_resource_overcommit }"#;
        let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        let attempts = m.response_attempts();
        let (first, r) = attempts[0];
        let (second, _) = attempts[1];
        let mut s = State::initial(&m, 0);
        s.results[first.args[0].0] = Some(ir::ValueId(0));
        capture(&mut s, first, &m, 0).unwrap();
        s.results[first.args[0].0] = Some(ir::ValueId(1));
        let v = &m.state_variables[0];
        let changed = ir::ValueId(
            m.types[v.base.0]
                .members
                .iter()
                .position(|v| v == "true")
                .unwrap(),
        );
        s.state_values[0] = ir::StateValue::Value(changed);
        capture(&mut s, second, &m, 0).unwrap();
        assert_eq!(
            s.response.as_ref().unwrap().inputs[&r.command],
            vec![ir::ValueId(0)]
        );
        assert_eq!(s.state_values[0], ir::StateValue::Value(changed));
        s.time = 1;
        s.tasks.push(Task {
            frozen_performance: None,
            stale: vec![],
            executor: None,
            binding: second.binding,
            capability: second.capability,
            lifecycle: Lifecycle::Running {
                finish: 1,
                ack: None,
            },
        });
        let kernel = m.capabilities[second.capability.0].kernel.as_ref().unwrap();
        let result = kernel_complete_in(
            &s,
            0,
            kernel,
            &m,
            &mut Room(32),
            &mut Exploration::default(),
            0,
        )
        .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].1.response.as_ref().unwrap().actuals[&second.binding],
            ir::ValueId(1)
        );
    }

    #[test]
    fn reliable_response_endpoints_do_not_invent_a_cut_r_resend() {
        for delivery in [
            "sequence [delivered,delivered]",
            "probability 0.0 basis \"zero loss test assumption\"",
        ] {
            let s = format!(
                r#"model M version "0.2" {{type V=enum {{ok}}
                actor A {{capacity 1;capability post(key:CaseId)->V {{commits;duration fixed 1min;result nondet {{ok}}}}}}
                process P {{r=A.post(case.id) response_loss {delivery} retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed;end done}}
                contract C {{at_most_once A.post.commit per case.id}}
                bounds {{cases 1;live_tasks 1;clock 3min}} verify safety no_resource_overcommit }}"#
            );
            let m = crate::lower::lower(&crate::syntax::parse(&s).unwrap()).unwrap();
            assert!(validate(&m).is_ok());
            let e = explore(&m);
            let r = crate::reference::explore(&m);
            assert_eq!(r.stop, crate::reference::StopReason::Complete);
            assert_eq!(e.terminals[0].elapsed.min, 1);
            assert_eq!(r.outcome.terminals[0].elapsed.min, 1);
            assert_eq!(crate::report::check(&s).verdict, Verdict::Satisfied);
        }
    }
}
