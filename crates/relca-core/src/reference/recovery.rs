//! Independent validation and finish-time observation, not engine helpers.
use super::*;

#[cfg(test)]
thread_local! {static BEFORE_RESOLVE:std::cell::RefCell<Option<(Frame<'static>,(usize,usize))>>=const{std::cell::RefCell::new(None)};}
#[cfg(test)]
pub(super) fn before_resolve(f: &Frame<'_>, site: (usize, usize)) {
    BEFORE_RESOLVE.with_borrow_mut(|v| {
        *v = Some((
            Frame {
                condition_waits: f.condition_waits.clone(),
                lost_updates: f.lost_updates.clone(),
                state_snapshots: f.state_snapshots.clone(),
                business: f.business,
                state_values: f.state_values.clone(),
                trace: f.trace.clone(),
                now: f.now,
                steps: f.steps,
                control: Vec::new(),
                waiting: None,
                tasks: f.tasks.clone(),
                adoption: f.adoption,
            },
            site,
        ))
    });
}
#[cfg(test)]
pub(super) fn capture(m: &Model, f: &Frame<'_>, case: usize, stop: crate::recovery::Stop) {
    use serde_json::json;
    if !crate::r2_test_adapter::active() {
        return;
    }
    let (command, _) = m.recovery().unwrap();
    if !f
        .trace
        .iter()
        .any(|o| matches!(o.event,Occurrence::ResponseInput(c,_) if c==command))
    {
        return;
    }
    let target = m
        .response_attempts()
        .into_iter()
        .find(|(_, r)| r.command == command)
        .unwrap()
        .1
        .command_call
        .capability;
    let commits = |t: &&Task| matches!(t.phase,Phase::Finished(v) if m.capabilities[t.call.capability.0].commits_result(v));
    let work = |t: &Task| {
        t.start.map_or(0, |start| {
            t.stop.unwrap_or(f.now).min(f.now).saturating_sub(start)
        })
    };
    let mut people = serde_json::Map::new();
    for name in ["c", "f", "q"] {
        let id = m.people.iter().position(|p| p.name == name).map(PersonId);
        people.insert(
            name.into(),
            json!(
                f.tasks
                    .iter()
                    .filter(|t| t.person.is_some() && t.person == id)
                    .map(work)
                    .sum::<u64>()
            ),
        );
    }
    let receipts: Vec<_> = f
        .trace
        .iter()
        .filter_map(|o| {
            if let Occurrence::Channel(r) = &o.event {
                (r.command == command).then_some(r)
            } else {
                None
            }
        })
        .collect();
    let mass = receipts.iter().try_fold(Rational::one(), |a, r| {
        r.weight.as_ref().map(|p| Rational(a.0 * &p.0))
    });
    let Cond::Contains { set, .. } = m.capabilities[target.0].atomic.as_ref().unwrap().condition
    else {
        panic!()
    };
    crate::r2_test_adapter::record(
        json!({"at":f.now,"work":f.tasks.iter().map(work).sum::<u64>(),"end_at":(stop==crate::recovery::Stop::End).then_some(f.now),"stop":stop,
        "charge_commits":f.tasks.iter().filter(|t|t.call.capability==target).filter(commits).count(),"refund_commits":f.tasks.iter().filter(|t|m.capabilities[t.call.capability.0].name=="refund").filter(commits).count(),"total_commits":f.tasks.iter().filter(commits).count(),
        "people":people,"key_present":matches!(f.state_values[set.0],StateValue::Set(bits) if bits&(1<<case)!=0),"pay_visible":response::visible_values(f,m,None)[command.0].is_some(),"original_receipt":receipts.first().map(|r|r.observation),"attempts":receipts.len(),"entries":f.trace.iter().filter(|o|matches!(o.event,Occurrence::RecoveryEnter(_))).count(),"arrivals":m.arrivals.as_ref().map_or(1,Vec::len),
        "query_evidence":f.trace.iter().any(|o|matches!(o.event,Occurrence::ReceiptQuery(q) if m.types[m.process.bindings[q.query.0].ty.0].members[q.value.0]!="still_unknown")),"decision_recorded":f.tasks.iter().any(|t|m.site_name(t.call.binding)=="decision" && matches!(t.phase,Phase::Finished(_))),"resolved":f.trace.iter().any(|o|matches!(o.event,Occurrence::ResolveEffect(_))),"tasks":f.tasks.iter().filter(|t|t.start.is_some()).count(),
        "query_work":f.tasks.iter().filter(|t|m.capabilities[t.call.capability.0].receipt_query.is_some()).map(work).sum::<u64>(),"decision_work":f.tasks.iter().filter(|t|m.site_name(t.call.binding)=="decision").map(work).sum::<u64>(),"path":receipts.iter().map(|r|if r.observation==crate::ast::Receipt::Unknown{'U'}else{'D'}).collect::<String>(),"mass":mass.map(|p|p.to_string())}),
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
    let query_statement = body[0].clone();
    let Stmt::Query(call, _) = query_statement.kind else {
        panic!()
    };
    let target = m.capabilities[call.capability.0]
        .receipt_query
        .as_ref()
        .unwrap()
        .target;
    let Cond::Contains { set, .. } = m.capabilities[target.0].atomic.as_ref().unwrap().condition
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
    let mut f = super::replay::initial(&m);
    let mut i = super::replay::interpreter(&m);
    f.now = 1;
    f.state_values[set.0] = StateValue::Set(if scenario.starts_with("present") {
        1
    } else if scenario == "other_case_key_only" {
        2
    } else {
        0
    });
    i.record(
        &mut f,
        Occurrence::ResponseInput(command, vec![ValueId(0)]),
        None,
    )
    .unwrap();
    i.record(
        &mut f,
        Occurrence::Channel(response::Receipt {
            weight: Some(Rational::one()),
            command,
            binding: m.response_attempts()[0].0.binding,
            attempt: 1,
            actual: ValueId(0),
            observation: crate::ast::Receipt::Unknown,
            resolved: false,
        }),
        None,
    )
    .unwrap();
    i.record(&mut f, Occurrence::RecoveryEnter(command), None)
        .unwrap();
    i.create(&mut f, std::slice::from_ref(&call), None).unwrap();
    let duration = m.capabilities[call.capability.0].duration;
    f.tasks[0].phase = Phase::Running;
    f.tasks[0].start = Some(f.now);
    f.tasks[0].finish = Some(f.now + duration);
    i.record_site(
        &mut f,
        Occurrence::Dispatch(call.capability),
        None,
        call.binding,
    )
    .unwrap();
    if scenario == "insert_after_start_before_finish" {
        let StateValue::Set(bits) = &mut f.state_values[set.0] else {
            panic!()
        };
        *bits |= 1;
    }
    f.now += duration;
    let before = f.state_values.clone();
    let values = project(&m, call.capability, &f.state_values, 0).unwrap();
    let rows:Vec<_>=values.into_iter().map(|v|{let mut next=f.clone();next.tasks[0].phase=Phase::Finished(v);next.tasks[0].stop=Some(next.now);i.finish_state(&mut next,0,v);i.record_site(&mut next,Occurrence::Completion(call.capability,v),None,call.binding).unwrap();let event=query(&next,&m,call.binding,v,&next.state_values,0).unwrap();i.record_site(&mut next,event,None,call.binding).unwrap();json!({"status":m.types[m.capabilities[call.capability.0].return_type.0].members[v.0],"evidence":m.types[m.capabilities[call.capability.0].return_type.0].members[v.0]!="still_unknown","at":next.now,"commits":next.trace.iter().filter(|o|o.matches_event(Event{capability:call.capability,kind:EventKind::Commit},&m)).count(),"writes":before.iter().zip(&next.state_values).filter(|(a,b)|a!=b).count(),"work":next.tasks[0].stop.unwrap()-next.tasks[0].start.unwrap()})}).collect();
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
    BEFORE_RESOLVE.with_borrow_mut(|v| *v = None);
    assert_eq!(crate::reference::explore(&m).stop, StopReason::Complete);
    let (mut f, site) = BEFORE_RESOLVE.with_borrow_mut(|v| v.take()).unwrap();
    let (mut command, body) = m.recovery().unwrap();
    let mut evidence = if let Stmt::Query(c, _) = &body[0].kind {
        c.binding
    } else {
        panic!()
    };
    let span = crate::diag::Span {
        start: site.0,
        end: site.1,
    };
    let other = BindingId(m.process.bindings.len());
    let mut b = m.process.bindings[command.0].clone();
    b.name = "other".into();
    m.process.bindings.push(b);
    let mut i = super::replay::interpreter(&m);
    i.record(
        &mut f,
        Occurrence::Channel(response::Receipt {
            weight: Some(Rational::one()),
            command: other,
            binding: other,
            attempt: 1,
            actual: ValueId(0),
            observation: crate::ast::Receipt::Unknown,
            resolved: false,
        }),
        None,
    )
    .unwrap();
    let before = f.now;
    let work = |f: &Frame<'_>| {
        f.tasks
            .iter()
            .filter_map(|t| {
                t.start
                    .map(|start| t.stop.unwrap_or(f.now).saturating_sub(start))
            })
            .sum::<u64>()
    };
    let before_work = work(&f);
    let mut case = 0;
    match fault {
        "valid" => {}
        "wrong_case" => case = 1,
        "wrong_command" => command = other,
        "forged_enum" => {
            evidence = BindingId(
                m.process
                    .bindings
                    .iter()
                    .position(|b| b.name == "decision")
                    .unwrap(),
            )
        }
        "evidence_before_query" => f
            .trace
            .retain(|o| !matches!(o.event, Occurrence::ReceiptQuery(_))),
        "double_resolve" => {
            let e = resolve(&f, &m, command, evidence, span, case).unwrap();
            i.record(&mut f, e, None).unwrap();
        }
        _ => {
            let q = f
                .trace
                .iter_mut()
                .find_map(|o| {
                    if let Occurrence::ReceiptQuery(q) = &mut o.event {
                        Some(q)
                    } else {
                        None
                    }
                })
                .unwrap();
            match fault {
                "wrong_key" => q.key = 1,
                "wrong_provider" => q.provider = ActorId(1),
                "wrong_query_site" => q.query_site.0 += 1,
                "still_unknown" => {
                    q.value = ValueId(
                        m.types[m.process.bindings[evidence.0].ty.0]
                            .members
                            .iter()
                            .position(|v| v == "still_unknown")
                            .unwrap(),
                    );
                }
                _ => panic!("fault"),
            }
        }
    }
    let Ok(event) = resolve(&f, &m, command, evidence, span, case) else {
        return json!({"rejected":true});
    };
    i.record(&mut f, event, None).unwrap();
    let target = m.response_attempts()[0].1.command_call.capability;
    json!({"unresolved":unresolved(&f).iter().map(|b|m.site_name(*b)).collect::<Vec<_>>(),"pay_visible":response::visible_values(&f,&m,None)[command.0].is_some(),"charge_commits":f.trace.iter().filter(|o|o.matches_event(Event{capability:target,kind:EventKind::Commit},&m)).count(),"refund_commits":f.trace.iter().filter(|o|matches!(o.event,Occurrence::Completion(c,_) if m.capabilities[c.0].name=="refund")).count(),"work_added":work(&f)-before_work,"time_added":f.now-before})
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct QueryEvidence {
    pub case: usize,
    pub command: BindingId,
    pub key: usize,
    pub provider: ActorId,
    pub capability: CapabilityId,
    pub query: BindingId,
    pub query_site: (usize, usize),
    pub at: Tick,
    pub value: ValueId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Resolution {
    pub command: BindingId,
    pub query: BindingId,
    pub case: usize,
    pub at: Tick,
    pub site: (usize, usize),
    pub value: ValueId,
}

#[cfg(test)]
#[derive(Clone)]
struct ReplayObservation {
    query_at: Option<u64>,
    decision_at: Option<u64>,
    resolve_at: Option<u64>,
    pay_visible: bool,
}
#[cfg(test)]
thread_local! {static REPLAY_OBSERVATION:std::cell::RefCell<Option<ReplayObservation>>=const {std::cell::RefCell::new(None)};}
#[cfg(test)]
pub(super) fn clear_replay_observation() {
    REPLAY_OBSERVATION.with_borrow_mut(|o| *o = None);
}
#[cfg(test)]
pub(super) fn observe_replay(m: &Model, f: &Frame<'_>) {
    let Some((command, _)) = m.recovery() else {
        return;
    };
    let query_at = f
        .trace
        .iter()
        .find_map(|o| matches!(o.event, Occurrence::ReceiptQuery(_)).then_some(o.at));
    let resolve_at = f
        .trace
        .iter()
        .find_map(|o| matches!(o.event, Occurrence::ResolveEffect(_)).then_some(o.at));
    let decision_at = f
        .tasks
        .iter()
        .find(|t| m.site_name(t.call.binding) == "decision")
        .and_then(|t| t.stop);
    let pay_visible = response::visible_values(f, m, None)[command.0].is_some();
    REPLAY_OBSERVATION.with_borrow_mut(|o| {
        *o = Some(ReplayObservation {
            query_at,
            decision_at,
            resolve_at,
            pay_visible,
        })
    });
}
#[cfg(test)]
pub(super) fn replay_probe(fault: &str) -> serde_json::Value {
    use crate::explain::Kind;
    use serde_json::json;
    let source = crate::r2_inputs::edit(&crate::r2_inputs::source("T01"), "by 7min", "by 6min");
    let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    let explained = crate::explain::explain(&source);
    let c = explained
        .explanation
        .counterexamples
        .into_iter()
        .find(|c| c.id.contains("Deadline"))
        .expect("actual deadline witness");
    let mut steps = c.steps;
    let q = steps
        .iter()
        .position(|s| s.kind == Kind::ReceiptQuery)
        .expect("query event");
    let r = steps
        .iter()
        .position(|s| s.kind == Kind::ResolveEffect)
        .expect("resolve event");
    match fault {
        "valid" => {}
        "swap_case" => steps[q].case_id = Some(2),
        "swap_command" => steps[q].command = Some("other".into()),
        "swap_key" => steps[q].saved_key = Some(2),
        "swap_evidence" => steps[r].evidence_ref = Some("other-query".into()),
        "omit_query" => {
            steps.remove(q);
            for (index, s) in steps.iter_mut().enumerate() {
                s.index = index;
            }
        }
        "change_status" => steps[q].result = Some("not_committed".into()),
        "change_resolve_time" => steps[r].time -= 1,
        "publish_unknown_ack" => {
            let s = steps
                .iter_mut()
                .find(|s| s.observed_receipt == Some(crate::ast::Receipt::Unknown))
                .unwrap();
            s.result = s.actual_result.clone();
        }
        _ => panic!("unknown replay fault {fault}"),
    }
    let result =
        super::replay::replay_choices(&m, &steps, &c.id, c.case_id, &vec![None; steps.len()]);
    if result.is_err() {
        return json!({"verified":false,"reason":"unsupported"});
    }
    let observed = REPLAY_OBSERVATION
        .with_borrow(|o| o.clone())
        .expect("actual successful replay observation");
    json!({"verified":true,"query_at":observed.query_at,"decision_at":observed.decision_at,"resolve_at":observed.resolve_at,"pay_visible":observed.pay_visible})
}
pub(super) fn unresolved(f: &Frame<'_>) -> BTreeSet<BindingId> {
    let mut open = BTreeSet::new();
    for o in &f.trace {
        match &o.event {
            Occurrence::Channel(r) => {
                if r.resolved {
                    open.remove(&r.command);
                } else {
                    open.insert(r.command);
                }
            }
            Occurrence::ResolveEffect(r) => {
                let justified=r.at==o.at && r.site.0<r.site.1 && f.trace.iter().any(|earlier|earlier.at<=r.at && matches!(earlier.event,Occurrence::ReceiptQuery(q) if q.query==r.query && q.command==r.command && q.case==r.case && q.value==r.value));
                if justified {
                    open.remove(&r.command);
                }
            }
            _ => {}
        }
    }
    open
}
pub(super) fn source(b: &Block, binding: BindingId) -> Option<(&Call, BindingId)> {
    for statement in b {
        match &statement.kind {
            Stmt::Query(call, command) if call.binding == binding => return Some((call, *command)),
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                if let Some(s) = source(then_body, binding).or_else(|| source(else_body, binding)) {
                    return Some(s);
                }
            }
            _ => {}
        }
    }
    None
}
pub(super) fn query(
    f: &Frame<'_>,
    m: &Model,
    binding: BindingId,
    value: ValueId,
    store: &[StateValue],
    case: usize,
) -> Result<Occurrence, StopReason> {
    let (original, body) = m.recovery().ok_or(StopReason::SemanticAbort)?;
    let (call, command) = source(body, binding).ok_or(StopReason::SemanticAbort)?;
    require(
        command == original
            && f.trace
                .iter()
                .any(|o| matches!(o.event,Occurrence::RecoveryEnter(c) if c==command))
            && unresolved(f).contains(&command)
            && !f
                .trace
                .iter()
                .any(|o| matches!(o.event,Occurrence::ReceiptQuery(q) if q.query==binding)),
    )?;
    let saved = f
        .trace
        .iter()
        .find_map(|o| match &o.event {
            Occurrence::ResponseInput(c, inputs) if *c == command => Some(inputs),
            _ => None,
        })
        .ok_or(StopReason::SemanticAbort)?;
    require(
        saved.len() == 1
            && saved[0].0 == case
            && project(m, call.capability, store, case)?.contains(&value),
    )?;
    let cap = &m.capabilities[call.capability.0];
    let declaration = cap
        .receipt_query
        .as_ref()
        .ok_or(StopReason::SemanticAbort)?;
    Ok(Occurrence::ReceiptQuery(QueryEvidence {
        case,
        command,
        key: case,
        provider: cap.actor,
        capability: declaration.target,
        query: binding,
        query_site: (call.span.start, call.span.end),
        at: f.now,
        value,
    }))
}
pub(super) fn resolve(
    f: &Frame<'_>,
    m: &Model,
    command: BindingId,
    evidence: BindingId,
    site: crate::diag::Span,
    case: usize,
) -> Result<Occurrence, StopReason> {
    let (original, body) = m.recovery().ok_or(StopReason::SemanticAbort)?;
    let (call, owner) = source(body, evidence).ok_or(StopReason::SemanticAbort)?;
    let q = f
        .trace
        .iter()
        .find_map(|o| match o.event {
            Occurrence::ReceiptQuery(q) if q.query == evidence => Some(q),
            _ => None,
        })
        .ok_or(StopReason::SemanticAbort)?;
    let cap = &m.capabilities[call.capability.0];
    let declaration = cap
        .receipt_query
        .as_ref()
        .ok_or(StopReason::SemanticAbort)?;
    let metadata = m
        .response_attempts()
        .into_iter()
        .find(|(_, r)| r.command == command)
        .map(|(_, r)| r)
        .ok_or(StopReason::SemanticAbort)?;
    let saved = f
        .trace
        .iter()
        .find_map(|o| match &o.event {
            Occurrence::ResponseInput(c, inputs) if *c == command => Some(inputs),
            _ => None,
        })
        .ok_or(StopReason::SemanticAbort)?;
    require(
        command == original
            && owner == command
            && q.command == command
            && q.case == case
            && q.key == case
            && saved.len() == 1
            && saved[0].0 == case
            && q.provider == cap.actor
            && q.provider == m.capabilities[metadata.command_call.capability.0].actor
            && q.capability == declaration.target
            && q.capability == metadata.command_call.capability
            && q.query_site == (call.span.start, call.span.end)
            && q.at <= f.now
            && unresolved(f).contains(&command)
            && !f
                .trace
                .iter()
                .any(|o| matches!(o.event,Occurrence::ResolveEffect(r) if r.command==command))
            && response::visible_values(f, m, None)
                .get(evidence.0)
                .copied()
                .flatten()
                == Some(q.value)
            && matches!(
                m.types[cap.return_type.0]
                    .members
                    .get(q.value.0)
                    .map(String::as_str),
                Some("committed_once" | "not_committed")
            ),
    )?;
    Ok(Occurrence::ResolveEffect(Resolution {
        command,
        query: evidence,
        case,
        at: f.now,
        site: (site.start, site.end),
        value: q.value,
    }))
}
pub(super) fn stop(f: &Frame<'_>, m: &Model) -> Result<Occurrence, StopReason> {
    require(!unresolved(f).is_empty())?;
    let (command, _) = m.recovery().ok_or(StopReason::SemanticAbort)?;
    require(
        f.trace
            .iter()
            .any(|o| matches!(o.event,Occurrence::RecoveryEnter(c) if c==command)),
    )?;
    Ok(Occurrence::ResponseStopped(command))
}

pub(super) fn path(
    m: &Model,
    cases: &[(usize, &Frame<'_>, crate::recovery::Stop)],
) -> Result<crate::recovery::Path, StopReason> {
    use crate::recovery as d;
    let Some((command, body)) = m.recovery() else {
        return Ok(d::Path {
            id: String::new(),
            commands: Vec::new(),
        });
    };
    fn inventory<'a>(b: &'a Block, calls: &mut Vec<&'a Call>) {
        for statement in b {
            match &statement.kind {
                Stmt::Call(c) | Stmt::Query(c, _) => calls.push(c),
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    inventory(then_body, calls);
                    inventory(else_body, calls);
                }
                _ => {}
            }
        }
    }
    let mut sites = Vec::new();
    inventory(body, &mut sites);
    let original = m
        .response_attempts()
        .into_iter()
        .find(|(_, r)| r.command == command)
        .ok_or(StopReason::SemanticAbort)?
        .1;
    let operation = &m.capabilities[original.command_call.capability.0];
    let cap_name = |id: CapabilityId| {
        let c = &m.capabilities[id.0];
        format!("{}.{}", m.actors[c.actor.0].name, c.name)
    };
    let mut commands = Vec::new();
    for (case, frame, stop_reason) in cases {
        let Some(saved) = frame.trace.iter().find_map(|o| match &o.event {
            Occurrence::ResponseInput(c, inputs) if *c == command => Some(inputs),
            _ => None,
        }) else {
            continue;
        };
        let proof_id = |q: &QueryEvidence| {
            crate::explain::recovery_evidence_id(
                q.case as u64 + 1,
                &m.site_name(q.command),
                &m.site_name(q.query),
                q.at,
            )
        };
        let status = |binding: BindingId, value: ValueId| {
            let b = m
                .process
                .bindings
                .get(binding.0)
                .ok_or(StopReason::SemanticAbort)?;
            let t = m.types.get(b.ty.0).ok_or(StopReason::SemanticAbort)?;
            d::Status::from_label(t.members.get(value.0).ok_or(StopReason::SemanticAbort)?)
                .ok_or(StopReason::SemanticAbort)
        };
        let queries = frame
            .trace
            .iter()
            .filter_map(|o| {
                if let Occurrence::ReceiptQuery(q) = o.event {
                    Some(q)
                } else {
                    None
                }
            })
            .map(|q| {
                let status = status(q.query, q.value)?;
                let id = proof_id(&q);
                Ok(d::Query {
                    id: id.clone(),
                    binding: m.site_name(q.query),
                    site: q.query_site.into(),
                    at: q.at,
                    status,
                    evidence: (status != d::Status::StillUnknown).then_some(id),
                })
            })
            .collect::<Result<_, StopReason>>()?;
        let resolutions = frame
            .trace
            .iter()
            .filter_map(|o| {
                if let Occurrence::ResolveEffect(r) = o.event {
                    Some(r)
                } else {
                    None
                }
            })
            .map(|r| {
                let q = frame
                    .trace
                    .iter()
                    .find_map(|o| match o.event {
                        Occurrence::ReceiptQuery(q) if q.query == r.query => Some(q),
                        _ => None,
                    })
                    .ok_or(StopReason::SemanticAbort)?;
                Ok(d::Resolution {
                    query_ref: proof_id(&q),
                    site: r.site.into(),
                    at: r.at,
                    fact: status(r.query, r.value)?,
                })
            })
            .collect::<Result<_, StopReason>>()?;
        let tasks = sites
            .iter()
            .filter_map(|call| {
                frame
                    .tasks
                    .iter()
                    .find(|t| t.call.binding == call.binding)
                    .map(|task| {
                        let result = match task.phase {
                            Phase::Finished(v) | Phase::Refused(v) => Some(
                                m.types[m.capabilities[task.call.capability.0].return_type.0]
                                    .members[v.0]
                                    .clone(),
                            ),
                            _ => None,
                        };
                        d::Task {
                            binding: m.site_name(call.binding),
                            capability: cap_name(task.call.capability),
                            executor: task.person.map(|p| m.people[p.0].name.clone()),
                            result,
                            started_at: task.start,
                            finished_at: if matches!(task.phase, Phase::Refused(_)) {
                                None
                            } else {
                                task.stop
                            },
                            site: call.span.into(),
                        }
                    })
            })
            .collect();
        commands.push(d::Command {
            case_id: *case as u64 + 1,
            command: m.site_name(command),
            provider: m.actors[operation.actor.0].name.clone(),
            capability: cap_name(original.command_call.capability),
            saved_key: saved.first().ok_or(StopReason::SemanticAbort)?.0 as u64 + 1,
            original_site: original.command_call.span.into(),
            queries,
            resolutions,
            tasks,
            unresolved: unresolved(frame).contains(&command)
                || !frame
                    .trace
                    .iter()
                    .any(|o| matches!(o.event,Occurrence::Channel(ref r) if r.command==command)),
            stop_reason: *stop_reason,
        });
    }
    Ok(d::Path {
        id: String::new(),
        commands,
    })
}

pub(super) fn validate(m: &Model) -> Execution {
    let mut selected = None;
    for (_, response) in m.response_attempts() {
        if response.attempt == 1 && response.recovery.is_some() {
            require(selected.replace(response).is_none())?;
        }
    }
    let Some(response) = selected else {
        return require(m.capabilities.iter().all(|c| c.receipt_query.is_none()));
    };
    require(response.idempotent && response.command_call.args.as_slice() == [CURRENT_CASE])?;
    let g_id = response.command_call.capability;
    let g = m
        .capabilities
        .get(g_id.0)
        .ok_or(StopReason::SemanticAbort)?;
    let atomic = g.atomic.as_ref().ok_or(StopReason::SemanticAbort)?;
    let (backing, parameter) = match atomic.condition {
        Cond::Contains {
            set,
            element: Element::Parameter(i),
        } => (set, i),
        _ => return Err(StopReason::SemanticAbort),
    };
    let v = m
        .state_variables
        .get(backing.0)
        .ok_or(StopReason::SemanticAbort)?;
    require(
        g.commits
            && !atomic.commit_when
            && g.start_writes.is_empty()
            && parameter == 0
            && g.params.len() == 1
            && g.params[0].ty == CASE_ID_TYPE
            && v.kind == StateKind::ActorObserved(g.actor)
            && v.case_set_size.is_some()
            && v.initial == InitialValue::Fixed(StateValue::Set(0)),
    )?;
    let own_writes: Vec<_> = atomic
        .writes
        .iter()
        .filter(|w| w.variable == backing)
        .collect();
    require(
        own_writes.len() == 1 && own_writes[0].source == WriteSource::InsertCase { parameter },
    )?;
    let private: BTreeSet<_> = g.finish_writes().map(|w| w.variable).collect();
    for (id, capability) in m.capabilities.iter().enumerate() {
        if id != g_id.0 {
            require(
                capability
                    .finish_writes()
                    .chain(&capability.start_writes)
                    .all(|w| !private.contains(&w.variable))
                    && capability
                        .kernel_writes
                        .iter()
                        .all(|v| !private.contains(v)),
            )?;
        }
        if let Some(source) = &capability.receipt_query {
            require(
                source.target == g_id
                    && source.key_parameter == 0
                    && capability.actor == g.actor
                    && capability.params.len() == 1
                    && capability.params[0].ty == CASE_ID_TYPE
                    && !capability.commits
                    && capability.atomic.is_none()
                    && capability.requires.is_none()
                    && capability.performance.is_none()
                    && capability.kernel.is_none()
                    && capability.version_check.is_none()
                    && capability.writes.is_empty()
                    && capability.start_writes.is_empty()
                    && capability.kernel_reads_latent.is_empty()
                    && capability.kernel_writes.is_empty(),
            )?;
            let t = m
                .types
                .get(capability.return_type.0)
                .ok_or(StopReason::SemanticAbort)?;
            require(
                t.members.len() == 3
                    && ["committed_once", "not_committed", "still_unknown"]
                        .iter()
                        .all(|n| t.members.iter().any(|s| s == n)),
            )?;
        }
    }
    for (_, r) in m.response_attempts() {
        require(r.command == response.command || r.command_call.capability != g_id)?;
    }
    let body = response.recovery.as_ref().unwrap();
    let mut query_count = 0;
    fn count(b: &Block, n: &mut usize) -> Execution {
        for s in b {
            match &s.kind {
                Stmt::Query(_, _) => *n += 1,
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    count(then_body, n)?;
                    count(else_body, n)?;
                }
                Stmt::Response(_, _) | Stmt::Fork { .. } | Stmt::WaitUntil(_) => {
                    return Err(StopReason::SemanticAbort);
                }
                _ => {}
            }
        }
        Ok(())
    }
    count(body, &mut query_count)?;
    require(query_count <= 32)?;
    fn outer(m: &Model, b: &Block, g: CapabilityId, p: &BTreeSet<StateVarId>) -> Execution {
        for s in b {
            match &s.kind {
                Stmt::Call(c) => {
                    require(
                        c.capability != g
                            && m.capabilities
                                .get(c.capability.0)
                                .is_some_and(|c| c.receipt_query.is_none()),
                    )?;
                    require(!private_reader(
                        m.capabilities
                            .get(c.capability.0)
                            .ok_or(StopReason::SemanticAbort)?,
                        p,
                        false,
                    ))?
                }
                Stmt::Response(c, _) => {
                    let cap = m
                        .capabilities
                        .get(c.capability.0)
                        .ok_or(StopReason::SemanticAbort)?;
                    require(
                        cap.receipt_query.is_none() && !private_reader(cap, p, c.capability == g),
                    )?;
                }
                Stmt::Fork { branches, .. } => {
                    for c in branches {
                        let cap = m
                            .capabilities
                            .get(c.capability.0)
                            .ok_or(StopReason::SemanticAbort)?;
                        require(
                            c.capability != g
                                && cap.receipt_query.is_none()
                                && !private_reader(cap, p, false),
                        )?;
                    }
                }
                Stmt::WaitUntil(c) => require(!private_read(c, p, BindingId(usize::MAX - 1)))?,
                Stmt::Query(..) | Stmt::ResolveEffect { .. } | Stmt::StopReceiptUnknown => {
                    return Err(StopReason::SemanticAbort);
                }
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    require(!private_read(condition, p, BindingId(usize::MAX - 1)))?;
                    outer(m, then_body, g, p)?;
                    outer(m, else_body, g, p)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    outer(m, &m.process.body, g_id, &private)?;
    // M2 permits already-received observations, but never a sibling branch or
    // a future binding. Compute the dominating prefix independently of engine.
    fn prefix(
        b: &Block,
        original: BindingId,
        mut known: BTreeSet<BindingId>,
        entry: &mut Option<BTreeSet<BindingId>>,
    ) -> Option<BTreeSet<BindingId>> {
        let mut alive = true;
        for s in b {
            match &s.kind {
                Stmt::Call(c) => {
                    known.insert(c.binding);
                }
                Stmt::Response(_, r) => {
                    if r.command == original && r.attempt == 1 {
                        *entry = Some(known.clone());
                    }
                    known.insert(r.command);
                }
                Stmt::Fork { branches, .. } => known.extend(branches.iter().map(|c| c.binding)),
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    let branches = (
                        prefix(then_body, original, known.clone(), entry),
                        prefix(else_body, original, known.clone(), entry),
                    );
                    match branches {
                        (Some(a), Some(b)) => known = a.intersection(&b).copied().collect(),
                        (Some(a), None) | (None, Some(a)) => known = a,
                        (None, None) => alive = false,
                    }
                }
                Stmt::End(_) => alive = false,
                _ => {}
            }
        }
        alive.then_some(known)
    }
    let mut entry = None;
    prefix(
        &m.process.body,
        response.command,
        BTreeSet::new(),
        &mut entry,
    );
    let visible = entry
        .ok_or(StopReason::SemanticAbort)?
        .into_iter()
        .map(|b| (b, 0))
        .collect();
    let pending = paths(
        m,
        body,
        response.command,
        g_id,
        &private,
        vec![Abstract {
            resolved: false,
            visible,
        }],
        0,
    )?;
    require(pending.is_empty())
}

#[derive(Clone)]
struct Abstract {
    resolved: bool,
    visible: BTreeMap<BindingId, u8>,
}
fn private_reader(c: &Capability, p: &BTreeSet<StateVarId>, original_atomic: bool) -> bool {
    c.read_variables
        .iter()
        .chain(&c.kernel_reads_latent)
        .any(|v| p.contains(v))
        || c.result_kernels()
            .any(|k| k.given.iter().any(|v| p.contains(v)))
        || c.requires
            .as_ref()
            .is_some_and(|r| private_read(&r.condition, p, BindingId(usize::MAX - 1)))
        || c.version_check
            .as_ref()
            .is_some_and(|v| v.variables.iter().any(|v| p.contains(v)))
        || !original_atomic
            && c.atomic
                .as_ref()
                .is_some_and(|a| private_read(&a.condition, p, BindingId(usize::MAX - 1)))
}
fn private_read(c: &Cond, variables: &BTreeSet<StateVarId>, original: BindingId) -> bool {
    match c {
        Cond::Eq(r) => r.binding == original,
        Cond::StateEq(s) => variables.contains(&s.variable),
        Cond::Contains { set, .. } => variables.contains(set),
        Cond::IntCompare { variable, .. } => variables.contains(variable),
        Cond::And(cs) | Cond::Or(cs) => cs.iter().any(|c| private_read(c, variables, original)),
    }
}
fn truth(c: &Cond, b: BindingId, v: ValueId) -> (bool, bool) {
    match c {
        Cond::Eq(r) if r.binding == b => (r.value == v, r.value != v),
        Cond::And(cs) => cs
            .iter()
            .map(|c| truth(c, b, v))
            .fold((true, false), |(a, b), (c, d)| (a && c, b || d)),
        Cond::Or(cs) => cs
            .iter()
            .map(|c| truth(c, b, v))
            .fold((false, true), |(a, b), (c, d)| (a || c, b && d)),
        _ => (true, true),
    }
}
fn paths(
    m: &Model,
    b: &Block,
    original: BindingId,
    g: CapabilityId,
    private: &BTreeSet<StateVarId>,
    mut states: Vec<Abstract>,
    depth: usize,
) -> Result<Vec<Abstract>, StopReason> {
    require(depth < 129)?;
    for s in b {
        require(!states.is_empty())?;
        let mut following = Vec::new();
        for mut state in states {
            match &s.kind {
                Stmt::Call(c) | Stmt::Query(c, _) => {
                    let cap = m
                        .capabilities
                        .get(c.capability.0)
                        .ok_or(StopReason::SemanticAbort)?;
                    require(
                        !private_reader(cap, private, false)
                            && c.args.iter().all(|b| {
                                *b == CURRENT_CASE
                                    || *b != original && state.visible.contains_key(b)
                            })
                            && cap
                                .read_variables
                                .iter()
                                .chain(&cap.kernel_reads_latent)
                                .all(|v| !private.contains(v))
                            && cap
                                .requires
                                .as_ref()
                                .is_none_or(|r| !private_read(&r.condition, private, original)),
                    )?;
                    let domain = match &s.kind {
                        Stmt::Query(_, command) => {
                            require(
                                *command == original
                                    && c.args == [CURRENT_CASE]
                                    && cap.receipt_query.as_ref().is_some_and(|q| q.target == g),
                            )?;
                            15
                        }
                        _ => {
                            require(cap.receipt_query.is_none() && c.capability != g)?;
                            0
                        }
                    };
                    require(
                        m.process
                            .bindings
                            .get(c.binding.0)
                            .is_some_and(|b| b.ty == cap.return_type),
                    )?;
                    state.visible.insert(c.binding, domain);
                    following.push(state);
                }
                Stmt::ResolveEffect { command, evidence } => {
                    let mask = state
                        .visible
                        .get(evidence)
                        .ok_or(StopReason::SemanticAbort)?;
                    // Ordinary enum producers have no status provenance, even if
                    // their declared values happen to spell the three status names.
                    require(
                        *command == original && !state.resolved && *mask != 0 && mask & 4 == 0,
                    )?;
                    state.resolved = true;
                    following.push(state);
                }
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    require(!private_read(condition, private, original))?;
                    for (branch, yes) in [(then_body, true), (else_body, false)] {
                        let mut child = state.clone();
                        for (binding, mask) in &mut child.visible {
                            if *mask == 0 {
                                continue;
                            }
                            let ty = m.process.bindings[binding.0].ty;
                            for (bit, name) in [
                                (1, "committed_once"),
                                (2, "not_committed"),
                                (4, "still_unknown"),
                            ] {
                                let value = ValueId(
                                    m.types[ty.0]
                                        .members
                                        .iter()
                                        .position(|v| v == name)
                                        .ok_or(StopReason::SemanticAbort)?,
                                );
                                let possible = truth(condition, *binding, value);
                                if !(if yes { possible.0 } else { possible.1 }) {
                                    *mask &= !bit;
                                }
                            }
                        }
                        following.extend(paths(
                            m,
                            branch,
                            original,
                            g,
                            private,
                            vec![child],
                            depth + 1,
                        )?);
                    }
                }
                Stmt::Wait(_) => following.push(state),
                Stmt::End(_) => require(state.resolved)?,
                Stmt::StopReceiptUnknown => require(!state.resolved)?,
                _ => return Err(StopReason::SemanticAbort),
            }
        }
        require(following.len() <= 65536)?;
        states = following;
    }
    Ok(states)
}

pub(super) fn project(
    m: &Model,
    cap: CapabilityId,
    store: &[StateValue],
    case: usize,
) -> Result<Vec<ValueId>, StopReason> {
    let capability = m.capabilities.get(cap.0).ok_or(StopReason::SemanticAbort)?;
    let query = capability
        .receipt_query
        .as_ref()
        .ok_or(StopReason::SemanticAbort)?;
    let charge = m
        .capabilities
        .get(query.target.0)
        .ok_or(StopReason::SemanticAbort)?;
    let atomic = charge.atomic.as_ref().ok_or(StopReason::SemanticAbort)?;
    let set = match atomic.condition {
        Cond::Contains { set, .. } => set,
        _ => return Err(StopReason::SemanticAbort),
    };
    require(case < 64)?;
    let present = match store.get(set.0) {
        Some(StateValue::Set(bits)) => bits & (1 << case) != 0,
        _ => return Err(StopReason::SemanticAbort),
    };
    let mut names = Vec::new();
    if query.availability != crate::ast::QueryAvailability::Unavailable {
        names.push(if present {
            "committed_once"
        } else {
            "not_committed"
        });
    }
    if query.availability != crate::ast::QueryAvailability::Available {
        names.push("still_unknown");
    }
    names
        .into_iter()
        .map(|s| {
            m.types[capability.return_type.0]
                .members
                .iter()
                .position(|v| v == s)
                .map(ValueId)
                .ok_or(StopReason::SemanticAbort)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round2_prefix_rejects_sibling_and_future_ir_arguments() {
        let results: Vec<_> = ["sibling", "after"]
            .into_iter()
            .map(|position| {
                let m = crate::r2_round2_inputs::bad_prefix(position);
                (position, validate(&m), crate::reference::explore(&m).stop)
            })
            .collect();
        assert!(
            results.iter().all(
                |(_, local, public)| *local == Err(StopReason::SemanticAbort)
                    && *public == StopReason::SemanticAbort
            ),
            "{results:?}"
        );
    }
    #[test]
    fn round2_ordinary_finish_writer_is_protected_in_reference() {
        let m = crate::r2_round2_inputs::outside_atomic_reader();
        assert_eq!(validate(&m), Err(StopReason::SemanticAbort));
    }
    #[test]
    fn round1_path_corrupt_query_status_aborts_without_panic() {
        let m =
            crate::lower::lower(&crate::syntax::parse(&crate::r2_inputs::source("R01")).unwrap())
                .unwrap();
        BEFORE_RESOLVE.with_borrow_mut(|v| *v = None);
        let result = crate::reference::explore(&m);
        assert_eq!(result.stop, StopReason::Complete);
        let (mut f, _) = BEFORE_RESOLVE.with_borrow_mut(|v| v.take()).unwrap();
        path(&m, &[(0, &f, crate::recovery::Stop::Truncated)]).unwrap();
        for o in &mut f.trace {
            if let Occurrence::ReceiptQuery(q) = &mut o.event {
                q.value = ValueId(usize::MAX);
            }
        }
        assert_eq!(
            path(&m, &[(0, &f, crate::recovery::Stop::Truncated)]).unwrap_err(),
            StopReason::SemanticAbort
        );
    }
    #[test]
    fn round1_version_checks_are_provider_readers() {
        let m =
            crate::lower::lower(&crate::syntax::parse(&crate::r2_inputs::source("R01")).unwrap())
                .unwrap();
        let (_, r) = m.response_attempts()[0];
        let mut c = m.capabilities[r.command_call.capability.0].clone();
        let Cond::Contains { set, .. } = c.atomic.as_ref().unwrap().condition else {
            panic!("G")
        };
        c.atomic = None;
        let private = BTreeSet::from([set]);
        assert!(!private_reader(&c, &private, false));
        c.version_check = Some(VersionCheck {
            variables: vec![set],
            reject: ValueId(1),
            resolve: None,
            span: c.span,
        });
        assert!(private_reader(&c, &private, false));
        assert!(private_reader(&c, &private, true));
    }
    #[test]
    fn r2_replay_rejects_single_record_corruptions() {
        let valid = crate::reference::r2_replay("valid");
        assert_eq!(
            valid,
            serde_json::json!({"verified":true,"query_at":3,"decision_at":6,"resolve_at":6,"pay_visible":false})
        );
        for fault in [
            "swap_case",
            "swap_command",
            "swap_key",
            "swap_evidence",
            "omit_query",
            "change_status",
            "change_resolve_time",
            "publish_unknown_ack",
        ] {
            assert_eq!(
                crate::reference::r2_replay(fault),
                serde_json::json!({"verified":false,"reason":"unsupported"}),
                "{fault}"
            );
        }
    }
    fn differences(
        a: &serde_json::Value,
        b: &serde_json::Value,
        path: &str,
        out: &mut Vec<String>,
    ) {
        use serde_json::Value;
        if a == b {
            return;
        }
        match (a, b) {
            (Value::Object(a), Value::Object(b)) => {
                for key in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
                    differences(
                        a.get(key).unwrap_or(&Value::Null),
                        b.get(key).unwrap_or(&Value::Null),
                        &format!("{path}/{key}"),
                        out,
                    );
                }
            }
            (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
                for (i, (a, b)) in a.iter().zip(b).enumerate() {
                    differences(a, b, &format!("{path}/{i}"), out);
                }
            }
            _ => out.push(format!("{path}: engine={a}; reference={b}")),
        }
    }
    #[test]
    fn r2_reference_and_engine_samples_keep_the_replication_unit() {
        for id in ["R01", "R02", "R07", "R08"] {
            let m =
                crate::lower::lower(&crate::syntax::parse(&crate::r2_inputs::source(id)).unwrap())
                    .unwrap();
            let e = crate::engine::simulate(&m);
            let r = crate::reference::simulate(&m);
            assert_eq!(r.stop, StopReason::Complete, "{id}");
            assert_eq!(e.completed_replications, 4);
            assert_eq!(r.outcome.completed_replications, 4);
            let mut diff = Vec::new();
            differences(
                &serde_json::json!(e.replications),
                &serde_json::json!(r.outcome.replications),
                "",
                &mut diff,
            );
            assert!(diff.is_empty(), "{id}: {diff:?}");
        }
    }
    #[test]
    fn r2_reference_query_projection_and_ir_defense() {
        let model =
            crate::lower::lower(&crate::syntax::parse(&crate::r2_inputs::source("R01")).unwrap())
                .unwrap();
        validate(&model).unwrap();
        let id = CapabilityId(
            model
                .capabilities
                .iter()
                .position(|c| c.receipt_query.is_some())
                .unwrap(),
        );
        let mut store: Vec<_> = model
            .state_variables
            .iter()
            .map(|v| match v.initial {
                InitialValue::Fixed(v) => v,
                _ => panic!("fixture default"),
            })
            .collect();
        let charge = model.capabilities[id.0]
            .receipt_query
            .as_ref()
            .unwrap()
            .target;
        let Cond::Contains { set, .. } = model.capabilities[charge.0]
            .atomic
            .as_ref()
            .unwrap()
            .condition
        else {
            panic!("set")
        };
        for (bits, name) in [
            (0, "not_committed"),
            (1, "committed_once"),
            (2, "not_committed"),
        ] {
            store[set.0] = StateValue::Set(bits);
            let observations = project(&model, id, &store, 0).unwrap();
            assert_eq!(observations.len(), 1);
            assert_eq!(
                model.types[model.capabilities[id.0].return_type.0].members[observations[0].0],
                name
            );
        }
        let mut corrupted = model.clone();
        corrupted.state_variables[set.0].initial = InitialValue::Fixed(StateValue::Set(1));
        assert_eq!(validate(&corrupted), Err(StopReason::SemanticAbort));
    }
}
