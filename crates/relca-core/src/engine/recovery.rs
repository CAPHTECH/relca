//! Exact G query correspondence, checked again on public IR before execution.
use super::*;
const INVALID: &str = "invalid_recovery_ir";

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(super) struct History {
    #[cfg(test)]
    pub weights: TestWeights,
    pub active: Option<ir::BindingId>,
    pub entered_at: Option<u64>,
    pub entries: usize,
    pub queries: Vec<Query>,
    pub resolutions: Vec<Resolution>,
    pub tasks: BTreeMap<ir::BindingId, TaskFact>,
}

/// Observation-only instrumentation must not change State's identity in tests.
#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub(super) struct TestWeights(Vec<Option<Rational>>);
#[cfg(test)]
impl PartialEq for TestWeights {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
#[cfg(test)]
impl Eq for TestWeights {}
#[cfg(test)]
impl std::hash::Hash for TestWeights {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}
#[cfg(test)]
impl std::ops::Deref for TestWeights {
    type Target = Vec<Option<Rational>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[cfg(test)]
impl std::ops::DerefMut for TestWeights {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
pub(super) fn capture(
    m: &ir::Model,
    state: &State,
    case: usize,
    at: u64,
    stop: crate::recovery::Stop,
) {
    use serde_json::json;
    if !crate::r2_test_adapter::active() {
        return;
    }
    let (command, _) = m.recovery().unwrap();
    let Some(h) = &state.response else {
        return;
    };
    if !h.inputs.contains_key(&command) {
        return;
    }
    let r = h.recovery.as_ref().unwrap();
    let original = m
        .response_attempts()
        .into_iter()
        .find(|(_, r)| r.command == command)
        .unwrap()
        .1;
    let g = original.command_call.capability;
    let work = |t: &TaskFact| {
        t.start.map_or(0, |start| {
            t.finish.unwrap_or(at).min(at).saturating_sub(start)
        })
    };
    let mut people = serde_json::Map::new();
    for name in ["c", "f", "q"] {
        let id = m
            .people
            .iter()
            .position(|p| p.name == name)
            .map(ir::PersonId);
        people.insert(
            name.into(),
            json!(
                r.tasks
                    .values()
                    .filter(|t| t.executor.is_some() && t.executor == id)
                    .map(work)
                    .sum::<u64>()
            ),
        );
    }
    let ir::Cond::Contains { set, .. } = m.capabilities[g.0].atomic.as_ref().unwrap().condition
    else {
        panic!("G")
    };
    let key_present =
        matches!(state.state_values[set.0],ir::StateValue::Set(bits) if bits&(1<<case)!=0);
    let mass = r.weights.iter().try_fold(Rational::one(), |a, p| {
        p.as_ref().map(|p| Rational(a.0 * &p.0))
    });
    crate::r2_test_adapter::record(
        json!({"at":at,"work":r.tasks.values().map(work).sum::<u64>(),"end_at":(stop==crate::recovery::Stop::End).then_some(at),"stop":stop,
        "charge_commits":r.tasks.values().filter(|t|t.capability==g && t.committed).count(),"refund_commits":r.tasks.values().filter(|t|m.capabilities[t.capability.0].name=="refund" && t.committed).count(),"total_commits":r.tasks.values().filter(|t|t.committed).count(),
        "people":people,"key_present":key_present,"pay_visible":state.results[command.0].is_some(),"original_receipt":h.rows.first().map(|r|r.receipt),"attempts":h.rows.len(),"entries":r.entries,"arrivals":m.arrivals.as_ref().map_or(1,Vec::len),
        "query_evidence":r.queries.iter().any(|q|q.definitive),"decision_recorded":r.tasks.iter().any(|(b,t)|m.site_name(*b)=="decision" && t.finish.is_some()),"resolved":!r.resolutions.is_empty(),"tasks":h.started.len(),
        "query_work":r.tasks.values().filter(|t|m.capabilities[t.capability.0].receipt_query.is_some()).map(work).sum::<u64>(),"decision_work":r.tasks.iter().filter(|(b,_)|m.site_name(**b)=="decision").map(|(_,t)|work(t)).sum::<u64>(),
        "path":h.rows.iter().map(|r|if r.receipt==crate::ast::Receipt::Unknown{'U'}else{'D'}).collect::<String>(),"mass":mass.map(|p|p.to_string())}),
    );
}

#[cfg(test)]
pub(super) fn projection_probe(scenario: &str) -> serde_json::Value {
    use serde_json::json;
    let source = if scenario == "other_case_key_only" {
        crate::r2_inputs::desk()
    } else {
        crate::r2_inputs::source("R01")
    };
    let mut m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    let (command, body) = m.recovery().unwrap();
    let (call, _) = query_call(
        body,
        m.process
            .bindings
            .iter()
            .position(|b| b.name == "status")
            .map(ir::BindingId)
            .unwrap(),
    )
    .unwrap();
    let call = call.clone();
    let target = m.capabilities[call.capability.0]
        .receipt_query
        .as_ref()
        .unwrap()
        .target;
    let ir::Cond::Contains { set, .. } =
        m.capabilities[target.0].atomic.as_ref().unwrap().condition
    else {
        panic!()
    };
    m.capabilities[call.capability.0]
        .receipt_query
        .as_mut()
        .unwrap()
        .availability = if scenario.contains("unavailable") {
        crate::ast::QueryAvailability::Unavailable
    } else if scenario.contains("nondet") {
        crate::ast::QueryAvailability::Nondet
    } else {
        crate::ast::QueryAvailability::Available
    };
    let mut s = State::initial(&m, 0);
    s.initialized = true;
    s.time = 1;
    s.state_values[set.0] = ir::StateValue::Set(if scenario.starts_with("present") {
        1
    } else if scenario == "other_case_key_only" {
        2
    } else {
        0
    });
    let h = s.response.as_mut().unwrap();
    h.inputs.insert(command, vec![ir::ValueId(0)]);
    h.unresolved.insert(command);
    h.recovery.as_mut().unwrap().active = Some(command);
    queued(&mut s, &call);
    task_event(
        &mut s,
        Bundle::Start(call.capability, None),
        Some(call.binding),
        &m,
    );
    s.tasks.push(Task {
        frozen_performance: None,
        stale: Vec::new(),
        executor: None,
        binding: call.binding,
        capability: call.capability,
        lifecycle: Lifecycle::Running {
            finish: 1 + m.capabilities[call.capability.0].duration,
            ack: None,
        },
    });
    if scenario == "insert_after_start_before_finish" {
        s.state_values[set.0] = write_value(
            &ir::StateWrite {
                variable: set,
                source: ir::WriteSource::InsertCase { parameter: 0 },
            },
            &s.state_values,
            None,
            0,
            None,
        )
        .unwrap();
    }
    let end = 1 + m.capabilities[call.capability.0].duration;
    let mut e = Exploration::default();
    let Step::Next(mut xs) = advance_state(s, end, &m, &mut Room(10000), &mut e).unwrap() else {
        panic!()
    };
    let s = xs.remove(0);
    let before = s.state_values.clone();
    let rows:Vec<_>=complete_in(&s,Due{i:0,finish:end,ack:None},&m,&mut Room(10000),&mut e,0).unwrap().into_iter().map(|s|{let h=s.response.as_ref().unwrap().recovery.as_ref().unwrap();let q=h.queries.last().unwrap();json!({"status":m.types[m.capabilities[call.capability.0].return_type.0].members[q.value.0],"evidence":q.definitive,"at":s.time,"commits":h.tasks.values().filter(|t|t.committed).count(),"writes":before.iter().zip(&s.state_values).filter(|(a,b)|a!=b).count(),"work":s.work})}).collect();
    if rows.len() == 1 {
        rows[0].clone()
    } else {
        json!(rows)
    }
}

#[cfg(test)]
pub(super) fn resolve_probe(fault: &str) -> serde_json::Value {
    use serde_json::json;
    let mut m =
        crate::lower::lower(&crate::syntax::parse(&crate::r2_inputs::source("R01")).unwrap())
            .unwrap();
    let mut p = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut p);
    let mut s = State::initial(&m, entry);
    let mut e = Exploration::default();
    let mut room = Room(100000);
    let (mut command, mut evidence, site) = loop {
        if let Instruction::ResolveEffect {
            command,
            evidence,
            site,
            ..
        } = &p[s.pc]
        {
            break (*command, *evidence, *site);
        }
        let Step::Next(mut next) = step_observed(s, &p, &m, &mut room, &mut e).unwrap() else {
            panic!()
        };
        assert_eq!(next.len(), 1);
        s = next.remove(0);
    };
    let other = ir::BindingId(m.process.bindings.len());
    let mut b = m.process.bindings[command.0].clone();
    b.name = "other".into();
    m.process.bindings.push(b);
    s.response.as_mut().unwrap().unresolved.insert(other);
    let before = (s.time, s.work);
    let mut case = 0;
    match fault {
        "valid" => {}
        "wrong_case" => case = 1,
        "wrong_command" => command = other,
        "forged_enum" => {
            evidence = ir::BindingId(
                m.process
                    .bindings
                    .iter()
                    .position(|b| b.name == "decision")
                    .unwrap(),
            )
        }
        "evidence_before_query" => s
            .response
            .as_mut()
            .unwrap()
            .recovery
            .as_mut()
            .unwrap()
            .queries
            .clear(),
        "double_resolve" => {
            resolve(&mut s, command, evidence, site, &m, case).unwrap();
        }
        _ => {
            let q = s
                .response
                .as_mut()
                .unwrap()
                .recovery
                .as_mut()
                .unwrap()
                .queries
                .last_mut()
                .unwrap();
            match fault {
                "wrong_key" => q.key = 1,
                "wrong_provider" => q.provider = ir::ActorId(1),
                "wrong_query_site" => q.site.0 += 1,
                "still_unknown" => {
                    let ty = m.process.bindings[evidence.0].ty;
                    q.value = ir::ValueId(
                        m.types[ty.0]
                            .members
                            .iter()
                            .position(|s| s == "still_unknown")
                            .unwrap(),
                    );
                    s.results[evidence.0] = Some(q.value);
                }
                _ => panic!("fault"),
            }
        }
    }
    if resolve(&mut s, command, evidence, site, &m, case).is_err() {
        return json!({"rejected":true});
    }
    let h = s.response.as_ref().unwrap();
    let r = h.recovery.as_ref().unwrap();
    let target = m.response_attempts()[0].1.command_call.capability;
    json!({"unresolved":h.unresolved.iter().map(|b|m.site_name(*b)).collect::<Vec<_>>(),"pay_visible":s.results[command.0].is_some(),"charge_commits":r.tasks.values().filter(|t|t.capability==target&&t.committed).count(),"refund_commits":r.tasks.values().filter(|t|m.capabilities[t.capability.0].name=="refund"&&t.committed).count(),"work_added":s.work-before.1,"time_added":s.time-before.0})
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Query {
    pub case: usize,
    pub command: ir::BindingId,
    pub key: usize,
    pub provider: ir::ActorId,
    pub target: ir::CapabilityId,
    pub binding: ir::BindingId,
    pub site: (usize, usize),
    pub at: u64,
    pub value: ir::ValueId,
    pub definitive: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Resolution {
    pub command: ir::BindingId,
    pub query: ir::BindingId,
    pub site: (usize, usize),
    pub at: u64,
    pub value: ir::ValueId,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TaskFact {
    pub capability: ir::CapabilityId,
    pub executor: Option<ir::PersonId>,
    pub ready: u64,
    pub start: Option<u64>,
    pub finish: Option<u64>,
    pub result: Option<ir::ValueId>,
    pub committed: bool,
}

pub(super) fn queued(state: &mut State, call: &ir::Call) {
    if let Some(h) = state.response.as_mut().and_then(|h| h.recovery.as_mut()) {
        h.tasks.entry(call.binding).or_insert(TaskFact {
            capability: call.capability,
            executor: None,
            ready: state.time,
            start: None,
            finish: None,
            result: None,
            committed: false,
        });
    }
}

pub(super) fn path(
    m: &ir::Model,
    cases: &[(usize, &State, crate::recovery::Stop)],
) -> EngineResult<crate::recovery::Path> {
    use crate::recovery as d;
    let Some((command, body)) = m.recovery() else {
        return Ok(d::Path {
            id: String::new(),
            commands: Vec::new(),
        });
    };
    fn calls<'a>(b: &'a ir::Block, out: &mut Vec<&'a ir::Call>) {
        for s in b {
            match &s.kind {
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => out.push(c),
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    calls(then_body, out);
                    calls(else_body, out);
                }
                _ => {}
            }
        }
    }
    let mut sites = Vec::new();
    calls(body, &mut sites);
    let metadata = m
        .response_attempts()
        .into_iter()
        .find(|(_, r)| r.command == command)
        .ok_or(INVALID)?
        .1;
    let original = &m.capabilities[metadata.command_call.capability.0];
    let cap_name = |id: ir::CapabilityId| {
        let c = &m.capabilities[id.0];
        format!("{}.{}", m.actors[c.actor.0].name, c.name)
    };
    let mut commands = Vec::new();
    for (case, state, stop_reason) in cases {
        let Some(h) = &state.response else {
            continue;
        };
        let Some(saved) = h.inputs.get(&command) else {
            continue;
        };
        let Some(history) = &h.recovery else {
            continue;
        };
        let id = |q: &Query| {
            crate::explain::recovery_evidence_id(
                q.case as u64 + 1,
                &m.site_name(q.command),
                &m.site_name(q.binding),
                q.at,
            )
        };
        let status = |binding: ir::BindingId, value: ir::ValueId| {
            let b = m.process.bindings.get(binding.0).ok_or(INVALID)?;
            let t = m.types.get(b.ty.0).ok_or(INVALID)?;
            d::Status::from_label(t.members.get(value.0).ok_or(INVALID)?).ok_or(INVALID)
        };
        let queries = history
            .queries
            .iter()
            .filter(|q| q.command == command)
            .map(|q| {
                let status = status(q.binding, q.value)?;
                let id = id(q);
                Ok(d::Query {
                    id: id.clone(),
                    binding: m.site_name(q.binding),
                    site: q.site.into(),
                    at: q.at,
                    status,
                    evidence: q.definitive.then_some(id),
                })
            })
            .collect::<EngineResult<_>>()?;
        let resolutions = history
            .resolutions
            .iter()
            .filter(|r| r.command == command)
            .map(|r| {
                let q = history
                    .queries
                    .iter()
                    .find(|q| q.binding == r.query)
                    .ok_or(INVALID)?;
                Ok(d::Resolution {
                    query_ref: id(q),
                    site: r.site.into(),
                    at: r.at,
                    fact: status(r.query, r.value)?,
                })
            })
            .collect::<EngineResult<_>>()?;
        let tasks = sites
            .iter()
            .filter_map(|call| {
                history.tasks.get(&call.binding).map(|t| d::Task {
                    binding: m.site_name(call.binding),
                    capability: cap_name(t.capability),
                    executor: t.executor.map(|p| m.people[p.0].name.clone()),
                    result: t.result.map(|v| {
                        m.types[m.capabilities[t.capability.0].return_type.0].members[v.0].clone()
                    }),
                    started_at: t.start,
                    finished_at: t.finish,
                    site: call.span.into(),
                })
            })
            .collect();
        commands.push(d::Command {
            case_id: *case as u64 + 1,
            command: m.site_name(command),
            provider: m.actors[original.actor.0].name.clone(),
            capability: cap_name(metadata.command_call.capability),
            saved_key: saved.first().ok_or(INVALID)?.0 as u64 + 1,
            original_site: metadata.command_call.span.into(),
            queries,
            resolutions,
            tasks,
            unresolved: h.unresolved.contains(&command)
                || !h.rows.iter().any(|r| r.command == command),
            stop_reason: *stop_reason,
        });
    }
    Ok(d::Path {
        id: String::new(),
        commands,
    })
}

pub(super) fn enter(
    state: &mut State,
    command: ir::BindingId,
    pc: Option<usize>,
) -> EngineResult<()> {
    let Some(pc) = pc else {
        return Ok(());
    };
    let h = state.response.as_mut().ok_or(INVALID)?;
    if !h.stopped {
        return Ok(());
    }
    require(h.unresolved.contains(&command))?;
    let r = h.recovery.as_mut().ok_or(INVALID)?;
    require(r.active.is_none() && r.entries == 0)?;
    r.active = Some(command);
    r.entries += 1;
    r.entered_at = Some(state.time);
    h.stopped = false;
    state.pc = pc;
    Ok(())
}
pub(super) fn stop(state: &mut State) -> EngineResult<()> {
    let h = state.response.as_mut().ok_or(INVALID)?;
    require(!h.unresolved.is_empty() && h.recovery.as_ref().is_some_and(|r| r.active.is_some()))?;
    h.stopped = true;
    Ok(())
}
fn query_call(b: &ir::Block, binding: ir::BindingId) -> Option<(&ir::Call, ir::BindingId)> {
    for s in b {
        match &s.kind {
            ir::Stmt::Query(c, command) if c.binding == binding => return Some((c, *command)),
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                if let Some(c) =
                    query_call(then_body, binding).or_else(|| query_call(else_body, binding))
                {
                    return Some(c);
                }
            }
            _ => {}
        }
    }
    None
}
pub(super) fn observe_query(
    state: &mut State,
    binding: ir::BindingId,
    capability: ir::CapabilityId,
    value: ir::ValueId,
    m: &ir::Model,
    case: usize,
) -> EngineResult<()> {
    let (_, body) = m.recovery().ok_or(INVALID)?;
    let (call, command) = query_call(body, binding).ok_or(INVALID)?;
    let cap = m.capabilities.get(capability.0).ok_or(INVALID)?;
    let source = cap.receipt_query.as_ref().ok_or(INVALID)?;
    let h = state.response.as_mut().ok_or(INVALID)?;
    let saved = h.inputs.get(&command).ok_or(INVALID)?;
    require(
        saved.len() == 1
            && saved[0].0 == case
            && call.capability == capability
            && h.unresolved.contains(&command),
    )?;
    let r = h.recovery.as_mut().ok_or(INVALID)?;
    require(r.active == Some(command) && !r.queries.iter().any(|q| q.binding == binding))?;
    let label = m.types[cap.return_type.0]
        .members
        .get(value.0)
        .ok_or(INVALID)?;
    require(project(m, capability, &state.state_values, case)?.contains(&value))?;
    require(["committed_once", "not_committed", "still_unknown"].contains(&label.as_str()))?;
    r.queries.push(Query {
        case,
        command,
        key: case,
        provider: cap.actor,
        target: source.target,
        binding,
        site: (call.span.start, call.span.end),
        at: state.time,
        value,
        definitive: label != "still_unknown",
    });
    Ok(())
}
pub(super) fn resolve(
    state: &mut State,
    command: ir::BindingId,
    evidence: ir::BindingId,
    site: diag::Span,
    m: &ir::Model,
    case: usize,
) -> EngineResult<()> {
    let (owner, body) = m.recovery().ok_or(INVALID)?;
    let (call, qowner) = query_call(body, evidence).ok_or(INVALID)?;
    let source = m.capabilities[call.capability.0]
        .receipt_query
        .as_ref()
        .ok_or(INVALID)?;
    let original = m
        .response_attempts()
        .into_iter()
        .find(|(_, r)| r.command == command)
        .map(|(_, r)| r)
        .ok_or(INVALID)?;
    let h = state.response.as_mut().ok_or(INVALID)?;
    let saved = h.inputs.get(&command).ok_or(INVALID)?;
    let r = h.recovery.as_mut().ok_or(INVALID)?;
    let q = r
        .queries
        .iter()
        .find(|q| q.binding == evidence)
        .ok_or(INVALID)?;
    require(
        owner == command
            && qowner == command
            && r.active == Some(command)
            && q.case == case
            && q.command == command
            && saved.len() == 1
            && saved[0].0 == case
            && q.key == case
            && q.provider == m.capabilities[original.command_call.capability.0].actor
            && q.provider == m.capabilities[call.capability.0].actor
            && q.target == source.target
            && q.target == original.command_call.capability
            && q.site == (call.span.start, call.span.end)
            && q.at <= state.time
            && q.definitive
            && matches!(
                m.types[m.capabilities[call.capability.0].return_type.0]
                    .members
                    .get(q.value.0)
                    .map(String::as_str),
                Some("committed_once" | "not_committed")
            )
            && state.results.get(evidence.0).copied().flatten() == Some(q.value)
            && h.unresolved.contains(&command)
            && !r.resolutions.iter().any(|r| r.command == command),
    )?;
    r.resolutions.push(Resolution {
        command,
        query: evidence,
        site: (site.start, site.end),
        at: state.time,
        value: q.value,
    });
    require(h.unresolved.remove(&command))?;
    Ok(())
}
pub(super) fn task_event(
    state: &mut State,
    bundle: Bundle,
    binding: Option<ir::BindingId>,
    m: &ir::Model,
) {
    let Some(binding) = binding else {
        return;
    };
    let Some(history) = state.response.as_mut().and_then(|h| h.recovery.as_mut()) else {
        return;
    };
    match bundle {
        Bundle::Start(capability, executor) => {
            history.tasks.insert(
                binding,
                TaskFact {
                    capability,
                    executor,
                    ready: history.tasks.get(&binding).map_or(state.time, |t| t.ready),
                    start: Some(state.time),
                    finish: None,
                    result: None,
                    committed: false,
                },
            );
        }
        Bundle::Finish(capability, value, _) => {
            if let Some(t) = history.tasks.get_mut(&binding) {
                t.finish = Some(state.time);
                t.result = Some(value);
                t.committed = m.capabilities[capability.0].commits_result(value);
            }
        }
        Bundle::CancelAck(_, _) => {
            if let Some(t) = history.tasks.get_mut(&binding) {
                t.finish = Some(state.time);
            }
        }
        _ => {}
    }
}

fn require(ok: bool) -> EngineResult<()> {
    if ok { Ok(()) } else { Err(INVALID.into()) }
}

pub(super) fn validate(m: &ir::Model) -> EngineResult<()> {
    let Some((command, body)) = m.recovery() else {
        return require(m.capabilities.iter().all(|c| c.receipt_query.is_none()));
    };
    let owners: Vec<_> = m
        .response_attempts()
        .into_iter()
        .filter(|(_, r)| r.attempt == 1 && r.recovery.is_some())
        .collect();
    require(owners.len() == 1)?;
    let (_, r) = owners[0];
    require(r.idempotent && r.command_call.args == [ir::CURRENT_CASE])?;
    let target = r.command_call.capability;
    let g = m.capabilities.get(target.0).ok_or(INVALID)?;
    let a = g.atomic.as_ref().ok_or(INVALID)?;
    let ir::Cond::Contains {
        set,
        element: ir::Element::Parameter(key),
    } = a.condition
    else {
        return Err(INVALID.into());
    };
    let variable = m.state_variables.get(set.0).ok_or(INVALID)?;
    require(
        g.commits
            && !a.commit_when
            && g.params.len() == 1
            && key == 0
            && g.params[key].ty == ir::CASE_ID_TYPE
            && g.start_writes.is_empty()
            && variable.kind == ir::StateKind::ActorObserved(g.actor)
            && variable.case_set_size.is_some()
            && variable.initial == ir::InitialValue::Fixed(ir::StateValue::Set(0)),
    )?;
    require(
        a.writes.iter().filter(|w| w.variable == set).count() == 1
            && a.writes.iter().any(|w| {
                w.variable == set && w.source == ir::WriteSource::InsertCase { parameter: key }
            }),
    )?;
    let protected: BTreeSet<_> = g.finish_writes().map(|w| w.variable).collect();
    for (id, c) in m.capabilities.iter().enumerate() {
        if id != target.0 {
            require(
                !c.finish_writes()
                    .chain(&c.start_writes)
                    .any(|w| protected.contains(&w.variable))
                    && !c.kernel_writes.iter().any(|v| protected.contains(v)),
            )?;
        }
        if let Some(q) = &c.receipt_query {
            let names = m.types.get(c.return_type.0).ok_or(INVALID)?;
            let members: BTreeSet<_> = names.members.iter().map(String::as_str).collect();
            require(
                q.target == target
                    && c.actor == g.actor
                    && q.key_parameter == 0
                    && c.params.len() == 1
                    && c.params[0].ty == ir::CASE_ID_TYPE
                    && members
                        == BTreeSet::from(["committed_once", "not_committed", "still_unknown"])
                    && names.members.len() == 3
                    && !c.commits
                    && c.requires.is_none()
                    && c.atomic.is_none()
                    && c.kernel.is_none()
                    && c.performance.is_none()
                    && c.start_writes.is_empty()
                    && c.writes.is_empty()
                    && c.kernel_writes.is_empty()
                    && c.kernel_reads_latent.is_empty()
                    && c.version_check.is_none(),
            )?;
        }
    }
    let mut queries = BTreeMap::new();
    fn inventory<'a>(
        b: &'a ir::Block,
        out: &mut BTreeMap<ir::BindingId, (&'a ir::Call, ir::BindingId)>,
        depth: usize,
    ) -> EngineResult<()> {
        require(depth <= 128)?;
        for s in b {
            match &s.kind {
                ir::Stmt::Query(c, command) => {
                    require(out.insert(c.binding, (c, *command)).is_none())?;
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    inventory(then_body, out, depth + 1)?;
                    inventory(else_body, out, depth + 1)?;
                }
                ir::Stmt::Call(_)
                | ir::Stmt::Wait(_)
                | ir::Stmt::ResolveEffect { .. }
                | ir::Stmt::End(_)
                | ir::Stmt::StopReceiptUnknown => {}
                _ => return Err(INVALID.into()),
            }
        }
        Ok(())
    }
    inventory(body, &mut queries, 0)?;
    require(queries.len() <= 32)?;
    for (c, owner) in queries.values() {
        let cap = m.capabilities.get(c.capability.0).ok_or(INVALID)?;
        require(
            *owner == command
                && c.args == [ir::CURRENT_CASE]
                && cap
                    .receipt_query
                    .as_ref()
                    .is_some_and(|q| q.target == target),
        )?;
    }
    for (_, other) in m.response_attempts() {
        require(other.command == command || other.command_call.capability != target)?;
    }
    fn outer(
        b: &ir::Block,
        m: &ir::Model,
        target: ir::CapabilityId,
        protected: &BTreeSet<ir::StateVarId>,
        mut visible: BTreeSet<ir::BindingId>,
        inherited: &mut Option<BTreeSet<ir::BindingId>>,
        command: ir::BindingId,
    ) -> EngineResult<Option<BTreeSet<ir::BindingId>>> {
        let mut alive = true;
        for s in b {
            match &s.kind {
                ir::Stmt::Call(c) => {
                    require(c.capability != target)?;
                    require(!private_reader(
                        m.capabilities.get(c.capability.0).ok_or(INVALID)?,
                        protected,
                        false,
                    ))?;
                    require(m.capabilities[c.capability.0].receipt_query.is_none())?;
                    visible.insert(c.binding);
                }
                ir::Stmt::Response(c, r) => {
                    require(!private_reader(
                        m.capabilities.get(c.capability.0).ok_or(INVALID)?,
                        protected,
                        c.capability == target,
                    ))?;
                    require(m.capabilities[c.capability.0].receipt_query.is_none())?;
                    if r.command == command && r.attempt == 1 {
                        *inherited = Some(visible.clone());
                    }
                    visible.insert(r.command);
                }
                ir::Stmt::Fork { branches, .. } => {
                    for c in branches {
                        let cap = m.capabilities.get(c.capability.0).ok_or(INVALID)?;
                        require(
                            c.capability != target
                                && cap.receipt_query.is_none()
                                && !private_reader(cap, protected, false),
                        )?;
                        visible.insert(c.binding);
                    }
                }
                ir::Stmt::WaitUntil(c) => require(!leaks(c, protected))?,
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    require(!leaks(condition, protected))?;
                    let yes = outer(
                        then_body,
                        m,
                        target,
                        protected,
                        visible.clone(),
                        inherited,
                        command,
                    )?;
                    let no = outer(
                        else_body,
                        m,
                        target,
                        protected,
                        visible.clone(),
                        inherited,
                        command,
                    )?;
                    match (yes, no) {
                        (Some(a), Some(b)) => visible = a.intersection(&b).copied().collect(),
                        (Some(a), None) | (None, Some(a)) => visible = a,
                        (None, None) => alive = false,
                    }
                }
                ir::Stmt::Query(_, _)
                | ir::Stmt::ResolveEffect { .. }
                | ir::Stmt::StopReceiptUnknown => return Err(INVALID.into()),
                ir::Stmt::End(_) => alive = false,
                ir::Stmt::Wait(_) => {}
            }
        }
        Ok(alive.then_some(visible))
    }
    let mut inherited = None;
    outer(
        &m.process.body,
        m,
        target,
        &protected,
        BTreeSet::new(),
        &mut inherited,
        command,
    )?;
    let known = inherited
        .ok_or(INVALID)?
        .into_iter()
        .map(|b| {
            let ty = m.process.bindings.get(b.0).ok_or(INVALID)?.ty;
            let t = m.types.get(ty.0).ok_or(INVALID)?;
            Ok((b, (0..t.members.len()).map(ir::ValueId).collect()))
        })
        .collect::<EngineResult<_>>()?;
    let paths = walk(
        body,
        vec![Flow {
            known,
            resolved: false,
        }],
        command,
        &queries,
        m,
        &protected,
        0,
    )?;
    require(paths.is_empty())
}

fn leaks(c: &ir::Cond, protected: &BTreeSet<ir::StateVarId>) -> bool {
    match c {
        ir::Cond::StateEq(x) => protected.contains(&x.variable),
        ir::Cond::IntCompare { variable, .. } => protected.contains(variable),
        ir::Cond::Contains { set, .. } => protected.contains(set),
        ir::Cond::And(cs) | ir::Cond::Or(cs) => cs.iter().any(|c| leaks(c, protected)),
        ir::Cond::Eq(_) => false,
    }
}
fn private_reader(c: &ir::Capability, p: &BTreeSet<ir::StateVarId>, original_atomic: bool) -> bool {
    c.read_variables
        .iter()
        .chain(&c.kernel_reads_latent)
        .any(|v| p.contains(v))
        || c.result_kernels()
            .any(|k| k.given.iter().any(|v| p.contains(v)))
        || c.requires.as_ref().is_some_and(|r| leaks(&r.condition, p))
        || c.version_check
            .as_ref()
            .is_some_and(|v| v.variables.iter().any(|v| p.contains(v)))
        || !original_atomic && c.atomic.as_ref().is_some_and(|a| leaks(&a.condition, p))
}
#[derive(Clone)]
struct Flow {
    known: BTreeMap<ir::BindingId, BTreeSet<ir::ValueId>>,
    resolved: bool,
}
fn possible(c: &ir::Cond, binding: ir::BindingId, value: ir::ValueId) -> (bool, bool) {
    match c {
        ir::Cond::Eq(x) if x.binding == binding => (x.value == value, x.value != value),
        ir::Cond::And(cs) => {
            let p: Vec<_> = cs.iter().map(|c| possible(c, binding, value)).collect();
            (p.iter().all(|x| x.0), p.iter().any(|x| x.1))
        }
        ir::Cond::Or(cs) => {
            let p: Vec<_> = cs.iter().map(|c| possible(c, binding, value)).collect();
            (p.iter().any(|x| x.0), p.iter().all(|x| x.1))
        }
        _ => (true, true),
    }
}
fn walk(
    b: &ir::Block,
    mut flows: Vec<Flow>,
    command: ir::BindingId,
    queries: &BTreeMap<ir::BindingId, (&ir::Call, ir::BindingId)>,
    m: &ir::Model,
    protected: &BTreeSet<ir::StateVarId>,
    depth: usize,
) -> EngineResult<Vec<Flow>> {
    require(depth <= 128)?;
    for s in b {
        require(!flows.is_empty())?;
        let mut next = Vec::new();
        for mut f in flows {
            match &s.kind {
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => {
                    let cap = m.capabilities.get(c.capability.0).ok_or(INVALID)?;
                    require(
                        !private_reader(cap, protected, false)
                            && c.args.iter().all(|arg| {
                                *arg == ir::CURRENT_CASE
                                    || *arg != command && f.known.contains_key(arg)
                            })
                            && !cap
                                .read_variables
                                .iter()
                                .chain(&cap.kernel_reads_latent)
                                .any(|v| protected.contains(v))
                            && cap
                                .requires
                                .as_ref()
                                .is_none_or(|r| !leaks(&r.condition, protected))
                            && matches!(&s.kind, ir::Stmt::Query(_, _))
                                == cap.receipt_query.is_some(),
                    )?;
                    let ty = m.process.bindings.get(c.binding.0).ok_or(INVALID)?.ty;
                    require(ty == cap.return_type)?;
                    f.known.insert(
                        c.binding,
                        (0..m.types[ty.0].members.len()).map(ir::ValueId).collect(),
                    );
                    next.push(f);
                }
                ir::Stmt::ResolveEffect {
                    command: owner,
                    evidence,
                } => {
                    let (call, qowner) = queries.get(evidence).ok_or(INVALID)?;
                    let domain = f.known.get(evidence).ok_or(INVALID)?;
                    require(
                        *owner == command
                            && *qowner == command
                            && !f.resolved
                            && domain.iter().all(|v| {
                                matches!(
                                    m.types[m.capabilities[call.capability.0].return_type.0]
                                        .members
                                        .get(v.0)
                                        .map(String::as_str),
                                    Some("committed_once" | "not_committed")
                                )
                            }),
                    )?;
                    f.resolved = true;
                    next.push(f);
                }
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    require(!leaks(condition, protected))?;
                    for (body, yes) in [(then_body, true), (else_body, false)] {
                        let mut child = f.clone();
                        for (binding, domain) in &mut child.known {
                            domain.retain(|v| {
                                let p = possible(condition, *binding, *v);
                                if yes { p.0 } else { p.1 }
                            });
                        }
                        next.extend(walk(
                            body,
                            vec![child],
                            command,
                            queries,
                            m,
                            protected,
                            depth + 1,
                        )?);
                    }
                }
                ir::Stmt::End(_) => require(f.resolved)?,
                ir::Stmt::StopReceiptUnknown => require(!f.resolved)?,
                ir::Stmt::Wait(_) => next.push(f),
                _ => return Err(INVALID.into()),
            }
        }
        require(next.len() <= 65536)?;
        flows = next;
    }
    Ok(flows)
}

/// Called only at normal finish; the current backing set is deliberately not
/// replaced with a start-time kernel snapshot.
pub(super) fn project(
    m: &ir::Model,
    capability: ir::CapabilityId,
    values: &[ir::StateValue],
    case: usize,
) -> EngineResult<Vec<ir::ValueId>> {
    let cap = m.capabilities.get(capability.0).ok_or(INVALID)?;
    let query = cap.receipt_query.as_ref().ok_or(INVALID)?;
    let g = m.capabilities.get(query.target.0).ok_or(INVALID)?;
    let ir::Cond::Contains { set, .. } = g.atomic.as_ref().ok_or(INVALID)?.condition else {
        return Err(INVALID.into());
    };
    let ir::StateValue::Set(keys) = values.get(set.0).ok_or(INVALID)? else {
        return Err(INVALID.into());
    };
    require(case < 64)?;
    let exact = if keys & (1u64 << case) != 0 {
        "committed_once"
    } else {
        "not_committed"
    };
    let labels = match query.availability {
        crate::ast::QueryAvailability::Available => vec![exact],
        crate::ast::QueryAvailability::Unavailable => vec!["still_unknown"],
        crate::ast::QueryAvailability::Nondet => vec![exact, "still_unknown"],
    };
    labels
        .into_iter()
        .map(|name| {
            m.types[cap.return_type.0]
                .members
                .iter()
                .position(|v| v == name)
                .map(ir::ValueId)
                .ok_or_else(|| INVALID.into())
        })
        .collect()
}

#[cfg(test)]
pub(super) fn history_observe(source: &str) -> serde_json::Value {
    use serde_json::json;
    let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    let mut state = State::initial(&m, entry);
    let mut scratch = Exploration::default();
    let mut room = Room(1000000);
    let end = loop {
        if state.response.as_ref().is_some_and(|h| h.stopped) {
            break None;
        }
        match step_observed(state.clone(), &program, &m, &mut room, &mut scratch).unwrap() {
            Step::Next(mut next) => {
                assert_eq!(next.len(), 1, "deterministic observation seam");
                state = next.remove(0);
            }
            Step::Terminal(end) => break Some(end),
            Step::Cut | Step::Deadlock => {
                panic!("history seam needs a complete deterministic path")
            }
        }
    };
    let h = state.response.as_ref().unwrap();
    let recovery = h.recovery.as_ref().unwrap();
    let (_, response) = m.response_attempts()[0];
    let charge = response.command_call.capability;
    let charge_commits = recovery
        .tasks
        .values()
        .filter(|t| t.capability == charge && t.committed)
        .count();
    let end_at = end.map(|_| state.time);
    if response.max_attempts > 1 {
        return json!({"attempts":h.rows.len(),"handler_entries":recovery.entries,"arrivals":m.arrivals.as_ref().map_or(1,Vec::len),"charge_commits":charge_commits,"end_at":end_at,"work":state.work});
    }
    let refund_commits = recovery
        .tasks
        .values()
        .filter(|t| m.capabilities[t.capability.0].name == "refund" && t.committed)
        .count();
    let mut people = serde_json::Map::new();
    for name in ["c", "f", "q"] {
        let id = ir::PersonId(m.people.iter().position(|p| p.name == name).unwrap());
        let work: u64 = recovery
            .tasks
            .values()
            .filter(|t| t.executor == Some(id))
            .filter_map(|t| t.start.map(|start| t.finish.unwrap_or(state.time) - start))
            .sum();
        people.insert(name.into(), json!(work));
    }
    let ir::Cond::Contains { set, .. } =
        m.capabilities[charge.0].atomic.as_ref().unwrap().condition
    else {
        panic!("G")
    };
    let key_present = matches!(state.state_values[set.0],ir::StateValue::Set(bits) if bits&1!=0);
    json!({"charge_commits":charge_commits,"refund_commits":refund_commits,"total_commits":recovery.tasks.values().filter(|t|t.committed).count(),"end_at":end_at,"work":state.work,"people":people,"key_present":key_present,"original_receipt":h.rows[0].receipt,"pay_visible":state.results[response.command.0].is_some()})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outcome::Range;
    #[test]
    fn round2_path_corruption_is_an_error_not_a_panic() {
        let m =
            crate::lower::lower(&crate::syntax::parse(&crate::r2_inputs::source("R01")).unwrap())
                .unwrap();
        let (command, body) = m.recovery().unwrap();
        let binding = ir::BindingId(
            m.process
                .bindings
                .iter()
                .position(|b| b.name == "status")
                .unwrap(),
        );
        let (call, _) = query_call(body, binding).unwrap();
        let cap = &m.capabilities[call.capability.0];
        let target = cap.receipt_query.as_ref().unwrap().target;
        let mut s = State::initial(&m, 0);
        let h = s.response.as_mut().unwrap();
        h.inputs.insert(command, vec![ir::ValueId(0)]);
        h.recovery.as_mut().unwrap().queries.push(Query {
            case: 0,
            command,
            key: 0,
            provider: cap.actor,
            target,
            binding,
            site: (call.span.start, call.span.end),
            at: 3,
            value: ir::ValueId(0),
            definitive: true,
        });
        path(&m, &[(0, &s, crate::recovery::Stop::Truncated)]).unwrap();
        let mut bad = s.clone();
        bad.response
            .as_mut()
            .unwrap()
            .recovery
            .as_mut()
            .unwrap()
            .queries[0]
            .value = ir::ValueId(usize::MAX);
        assert!(matches!(
            path(&m, &[(0, &bad, crate::recovery::Stop::Truncated)]),
            Err(Halt::Unsupported(INVALID))
        ));
        let history = bad.response.as_mut().unwrap().recovery.as_mut().unwrap();
        history.queries.clear();
        history.resolutions.push(Resolution {
            command,
            query: binding,
            site: (1, 2),
            at: 6,
            value: ir::ValueId(0),
        });
        assert!(path(&m, &[(0, &bad, crate::recovery::Stop::Truncated)]).is_err());
        bad.response
            .as_mut()
            .unwrap()
            .recovery
            .as_mut()
            .unwrap()
            .resolutions
            .clear();
        bad.response
            .as_mut()
            .unwrap()
            .inputs
            .insert(command, vec![]);
        assert!(path(&m, &[(0, &bad, crate::recovery::Stop::Truncated)]).is_err());
    }
    #[test]
    fn round2_prefix_rejects_sibling_and_future_ir_arguments() {
        let results: Vec<_> = ["sibling", "after"]
            .into_iter()
            .map(|position| {
                let m = crate::r2_round2_inputs::bad_prefix(position);
                (
                    position,
                    validate(&m).is_err(),
                    crate::engine::explore(&m)
                        .diagnostics
                        .iter()
                        .any(|d| d.message == INVALID),
                )
            })
            .collect();
        assert!(
            results.iter().all(|(_, local, public)| *local && *public),
            "{results:?}"
        );
    }
    #[test]
    fn round2_ordinary_finish_writer_is_protected_in_engine() {
        let m = crate::r2_round2_inputs::outside_atomic_reader();
        assert!(validate(&m).is_err());
    }
    #[test]
    fn round1_observation_weights_do_not_change_state_identity() {
        use std::hash::{Hash, Hasher};
        let a = History::default();
        let mut b = a.clone();
        b.weights.push(Some(Rational::one()));
        assert_eq!(a, b);
        let hash = |h: &History| {
            let mut s = std::collections::hash_map::DefaultHasher::new();
            h.hash(&mut s);
            s.finish()
        };
        assert_eq!(hash(&a), hash(&b));
    }
    #[test]
    fn round1_version_checks_are_provider_readers() {
        let m =
            crate::lower::lower(&crate::syntax::parse(&crate::r2_inputs::source("R01")).unwrap())
                .unwrap();
        let (_, r) = m.response_attempts()[0];
        let mut c = m.capabilities[r.command_call.capability.0].clone();
        let ir::Cond::Contains { set, .. } = c.atomic.as_ref().unwrap().condition else {
            panic!("G")
        };
        c.atomic = None;
        let private = BTreeSet::from([set]);
        assert!(!private_reader(&c, &private, false));
        c.version_check = Some(ir::VersionCheck {
            variables: vec![set],
            reject: ir::ValueId(1),
            resolve: None,
            span: c.span,
        });
        assert!(private_reader(&c, &private, false));
        assert!(
            private_reader(&c, &private, true),
            "G is exempt only for its atomic condition"
        );
    }
    #[test]
    fn r2_engine_refund_history_and_people() {
        for (id, time, refund, c, f, q) in [
            ("R01", 7, 0, 3, 0, 3),
            ("R04", 9, 1, 4, 1, 4),
            ("R05", 10, 1, 5, 1, 4),
        ] {
            let source = crate::r2_inputs::edit(
                &crate::r2_inputs::source(id),
                "capacity 1; control policy; state keys",
                "executors [q]; capacity 1; control policy; state keys",
            );
            let actual = crate::engine::r2_history(&source);
            assert_eq!(
                actual,
                serde_json::json!({"charge_commits":1,"refund_commits":refund,"total_commits":1+refund,"end_at":time,"work":time,"people":{"c":c,"f":f,"q":q},"key_present":true,"original_receipt":"unknown","pay_visible":false})
            );
        }
    }
    #[test]
    fn r2_engine_recovery_hand_timelines() {
        for (id, time, work, end) in [
            ("R01", 7, 7, true),
            ("R02", 3, 3, false),
            ("R03", 1, 1, false),
            ("R07", 9, 7, true),
            ("R08", 8, 8, true),
        ] {
            let m =
                crate::lower::lower(&crate::syntax::parse(&crate::r2_inputs::source(id)).unwrap())
                    .unwrap();
            let o = explore(&m);
            assert!(!o.truncated, "{id}");
            assert!(
                o.diagnostics
                    .iter()
                    .all(|d| d.severity != diag::Severity::Error),
                "{id}: {:?}",
                o.diagnostics
            );
            if end {
                assert_eq!(o.terminals.len(), 1, "{id}");
                assert_eq!(
                    o.terminals[0].elapsed,
                    Range {
                        min: time,
                        max: time
                    }
                );
                assert_eq!(
                    o.terminals[0].work,
                    Range {
                        min: work,
                        max: work
                    }
                );
            } else {
                assert!(o.terminals.is_empty());
                let t = &o.case_outcomes.as_ref().unwrap()[0].terminals[0];
                assert_eq!(
                    t.observed_until,
                    Some(Range {
                        min: time,
                        max: time
                    })
                );
                assert_eq!(
                    t.observed_work,
                    Some(Range {
                        min: work,
                        max: work
                    })
                );
            }
        }
    }
    #[test]
    fn r2_engine_query_projection_and_ir_defense() {
        let m =
            crate::lower::lower(&crate::syntax::parse(&crate::r2_inputs::source("R01")).unwrap())
                .unwrap();
        validate(&m).unwrap();
        let query = m
            .capabilities
            .iter()
            .position(|c| c.receipt_query.is_some())
            .unwrap();
        let mut values: Vec<_> = m
            .state_variables
            .iter()
            .map(|v| match v.initial {
                ir::InitialValue::Fixed(v) => v,
                _ => panic!("fixed fixture"),
            })
            .collect();
        let g = m.capabilities[query].receipt_query.as_ref().unwrap().target;
        let ir::Cond::Contains { set, .. } = m.capabilities[g.0].atomic.as_ref().unwrap().condition
        else {
            panic!("G")
        };
        for (bits, expected) in [
            (0, "not_committed"),
            (1, "committed_once"),
            (2, "not_committed"),
        ] {
            values[set.0] = ir::StateValue::Set(bits);
            let result = project(&m, ir::CapabilityId(query), &values, 0).unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(
                m.types[m.capabilities[query].return_type.0].members[result[0].0],
                expected
            );
        }
        for fault in 0..4 {
            let mut bad = m.clone();
            match fault {
                0 => {
                    bad.state_variables[set.0].initial =
                        ir::InitialValue::Fixed(ir::StateValue::Set(1))
                }
                1 => bad.capabilities[query].commits = true,
                2 => bad.capabilities[query].receipt_query = None,
                _ => bad.capabilities[query].params.clear(),
            }
            assert!(validate(&bad).is_err(), "{fault}");
        }
    }
}
