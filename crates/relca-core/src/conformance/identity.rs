//! Explicit, non-inferential Actor/Person/operation resolution (ADR 0057).
use super::input::{Event, Failure, Header};
use crate::ir;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct Identity {
    pub capability: Option<ir::CapabilityId>,
    pub actor: Option<ir::ActorId>,
    pub person: Option<ir::PersonId>,
    pub mapped: bool,
}
#[derive(Debug, Serialize)]
pub struct UnmappedActor {
    pub source_actor_id: String,
    pub source_executor_id: Option<String>,
    pub mapped_actor: Option<String>,
    pub reason: &'static str,
    pub case_ids: Vec<String>,
    pub event_ids: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct UnmappedOperation {
    pub operation: String,
    pub case_ids: Vec<String>,
    pub event_ids: Vec<String>,
}
#[derive(Debug)]
pub struct Resolution {
    pub events: Vec<Identity>,
    pub unmapped_actor: Vec<UnmappedActor>,
    pub unmapped_operation: Vec<UnmappedOperation>,
}

pub fn resolve(m: &ir::Model, h: &Header, events: &[Event]) -> Result<Resolution, Failure> {
    let mut actors = BTreeMap::new();
    for entry in &h.actor_map {
        let id = m
            .actors
            .iter()
            .position(|a| a.name == entry.actor)
            .ok_or_else(|| Failure::invalid("bad_mapping"))?;
        actors.insert(entry.source_id.as_str(), ir::ActorId(id));
    }
    let mut persons = BTreeMap::new();
    for entry in &h.executor_map {
        let id = m
            .people
            .iter()
            .position(|p| p.name == entry.person)
            .ok_or_else(|| Failure::invalid("bad_mapping"))?;
        persons.insert(entry.source_id.as_str(), ir::PersonId(id));
    }
    let operations: BTreeMap<_, _> = m
        .capabilities
        .iter()
        .enumerate()
        .map(|(i, c)| {
            (
                format!("{}.{}", m.actors[c.actor.0].name, c.name),
                ir::CapabilityId(i),
            )
        })
        .collect();
    let mut out = Resolution {
        events: Vec::new(),
        unmapped_actor: Vec::new(),
        unmapped_operation: Vec::new(),
    };
    for e in events {
        let actor = actors.get(e.actor_id.as_str()).copied();
        let person = e
            .executor_id
            .as_deref()
            .and_then(|p| persons.get(p))
            .copied();
        let capability = operations.get(&e.operation).copied();
        let mut reasons = Vec::new();
        match actor {
            None => reasons.push("no_actor_mapping"),
            Some(a) => {
                if m.cut.as_ref().is_some_and(|c| !c.include.contains(&a)) {
                    reasons.push("actor_outside_cut");
                }
                if capability.is_some_and(|c| m.capabilities[c.0].actor != a) {
                    reasons.push("operation_actor_mismatch");
                }
                match person {
                    None => reasons.push("no_executor_mapping"),
                    Some(p)
                        if !m.actors[a.0]
                            .executors
                            .as_ref()
                            .is_some_and(|ps| ps.contains(&p)) =>
                    {
                        reasons.push("executor_actor_mismatch")
                    }
                    Some(_) => (),
                }
            }
        }
        let mapped = reasons.is_empty() && capability.is_some();
        for reason in reasons {
            let mapped_actor = actor.map(|a| m.actors[a.0].name.clone());
            if let Some(row) = out.unmapped_actor.iter_mut().find(|r| {
                r.source_actor_id == e.actor_id
                    && r.source_executor_id == e.executor_id
                    && r.mapped_actor == mapped_actor
                    && r.reason == reason
            }) {
                row.case_ids.push(e.case_id.clone());
                row.event_ids.push(e.event_id.clone());
            } else {
                out.unmapped_actor.push(UnmappedActor {
                    source_actor_id: e.actor_id.clone(),
                    source_executor_id: e.executor_id.clone(),
                    mapped_actor,
                    reason,
                    case_ids: vec![e.case_id.clone()],
                    event_ids: vec![e.event_id.clone()],
                });
            }
        }
        if capability.is_none() {
            if let Some(row) = out
                .unmapped_operation
                .iter_mut()
                .find(|r| r.operation == e.operation)
            {
                row.case_ids.push(e.case_id.clone());
                row.event_ids.push(e.event_id.clone());
            } else {
                out.unmapped_operation.push(UnmappedOperation {
                    operation: e.operation.clone(),
                    case_ids: vec![e.case_id.clone()],
                    event_ids: vec![e.event_id.clone()],
                });
            }
        }
        out.events.push(Identity {
            capability,
            actor,
            person,
            mapped,
        });
    }
    for row in &mut out.unmapped_actor {
        row.case_ids.sort();
        row.case_ids.dedup();
        row.event_ids.sort();
    }
    for row in &mut out.unmapped_operation {
        row.case_ids.sort();
        row.case_ids.dedup();
        row.event_ids.sort();
    }
    out.unmapped_actor.sort_by(|a, b| {
        (&a.source_actor_id, &a.source_executor_id, a.reason).cmp(&(
            &b.source_actor_id,
            &b.source_executor_id,
            b.reason,
        ))
    });
    out.unmapped_operation
        .sort_by(|a, b| a.operation.cmp(&b.operation));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonexistent_mapping_is_invalid_even_without_observations() {
        let src = "model M version \"0.2\" { type V = enum { ok } people [p] actor A { executors [p]; capability x() -> V { duration fixed 1min; result nondet { ok } } } process P { a = A.x(); end done } bounds { cases 1; live_tasks 1; clock 3min } verify reachable end.done }";
        let model = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let h: Header = serde_json::from_str(r#"{"kind":"header","schema":"relca-log/v1","clock":"c","time_unit":"min","window":{"from":0,"to":3},"cases":[],"actor_map":[{"source_id":"a","actor":"Ghost"}],"executor_map":[]}"#).unwrap();
        assert_eq!(resolve(&model, &h, &[]).unwrap_err().reason, "bad_mapping");
    }
}
