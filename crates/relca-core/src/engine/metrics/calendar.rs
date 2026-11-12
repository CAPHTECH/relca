//! ADR 0059 capacity integrals from engine path-local histories.
use super::{CaseObservation, TaskHistory};
use crate::ir;
use num_bigint::BigInt;
use num_rational::BigRational;
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    Actor,
    Resource,
    Person,
}

pub(super) fn scalar(n: BigInt) -> Value {
    let s = n.to_string();
    json!({"min":s,"max":s,"reason":null})
}

pub(super) fn calls<'a>(b: &'a ir::Block, out: &mut Vec<&'a ir::Call>) {
    for s in b {
        match &s.kind {
            ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => out.push(c),
            ir::Stmt::Response(c, r) => {
                out.push(c);
                if r.attempt == 1
                    && let Some(b) = &r.recovery
                {
                    calls(b, out);
                }
            }
            ir::Stmt::Fork { branches, .. } => out.extend(branches),
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                calls(then_body, out);
                calls(else_body, out);
            }
            ir::Stmt::Wait(_)
            | ir::Stmt::WaitUntil(_)
            | ir::Stmt::End(_)
            | ir::Stmt::ResolveEffect { .. }
            | ir::Stmt::StopReceiptUnknown => (),
        }
    }
}

pub(super) fn augment(m: &ir::Model, cases: &[CaseObservation<'_>], until: u64, value: &mut Value) {
    let Some(tables) = &m.resource_time else {
        return;
    };
    let mut sites = Vec::new();
    calls(&m.process.body, &mut sites);
    let mut past: Vec<(&ir::Call, &TaskHistory)> = Vec::new();
    let mut points = BTreeSet::from([0, until]);
    for case in cases {
        if let Some(h) = case.state.and_then(|s| s.metric_history.as_ref()) {
            for c in &sites {
                let task = &h.tasks[c.binding.0];
                for at in [task.start, task.stop].into_iter().flatten() {
                    points.insert(at.min(until));
                }
                past.push((c, task));
            }
        }
    }
    for i in tables.calendars.iter().flat_map(|c| &c.intervals) {
        points.insert(i.from.min(until));
        points.insert(i.until.min(until));
    }
    let points: Vec<_> = points.into_iter().collect();
    let integrate = |calendar: usize, target: usize, kind: TargetKind| {
        let mut capacity = BigInt::from(0);
        let mut regular = BigInt::from(0);
        let mut overtime = BigInt::from(0);
        for pair in points.windows(2) {
            let (at, dt) = (pair[0], BigInt::from(pair[1] - pair[0]));
            let slots = BigInt::from(
                tables.calendars[calendar]
                    .capacity_at(at)
                    .expect("validated calendar"),
            );
            let mut used = BigInt::from(0);
            for (call, task) in &past {
                if !task
                    .start
                    .is_some_and(|start| start <= at && at < task.stop.unwrap_or(until))
                {
                    continue;
                }
                let cap = &m.capabilities[call.capability.0];
                match kind {
                    TargetKind::Actor => {
                        if cap.actor.0 == target {
                            used += 1;
                        }
                    }
                    TargetKind::Resource => {
                        for u in &cap.uses {
                            if u.resource.0 == target {
                                used += u.amount;
                            }
                        }
                    }
                    TargetKind::Person => {
                        if task.executor == Some(ir::PersonId(target)) {
                            used += 1;
                        }
                    }
                }
            }
            capacity += &slots * &dt;
            regular += used.clone().min(slots.clone()) * &dt;
            overtime += (used - slots).max(BigInt::from(0)) * &dt;
        }
        (capacity, regular, overtime)
    };
    let mut fill = |key: &str, name_key: &str, references: &[Option<usize>], kind: TargetKind| {
        if let Some(rows) = value[key].as_array_mut() {
            for row in rows {
                let id = match kind {
                    TargetKind::Actor => m.actors.iter().position(|a| row[name_key] == a.name),
                    TargetKind::Resource => {
                        m.resources.iter().position(|r| row[name_key] == r.name)
                    }
                    TargetKind::Person => m.people.iter().position(|p| row[name_key] == p.name),
                }
                .expect("collector target");
                if let Some(calendar) = references[id] {
                    let (capacity, regular, overtime) = integrate(calendar, id, kind);
                    if kind == TargetKind::Person {
                        row["overtime_work"] = scalar(overtime);
                        continue;
                    }
                    row["scheduled_capacity_time"] = scalar(capacity.clone());
                    row["scheduled_occupancy_time"] = scalar(regular.clone());
                    row["overtime_occupancy_time"] = scalar(overtime);
                    row["scheduled_utilization"] = if capacity == BigInt::from(0) {
                        json!({"min":null,"max":null,"reason":"zero_capacity"})
                    } else {
                        let s = BigRational::new(regular, capacity).to_string();
                        json!({"min":s,"max":s,"reason":null})
                    };
                }
            }
        }
    };
    fill("actors", "actor", &tables.actors, TargetKind::Actor);
    fill(
        "resources",
        "resource",
        &tables.resources,
        TargetKind::Resource,
    );
    fill("people", "person", &tables.people, TargetKind::Person);
    value["checked"].as_array_mut().unwrap().extend([
        json!("calendar_start_gates"),
        json!("calendar_capacity_integrals"),
    ]);
    value["unchecked"]
        .as_array_mut()
        .unwrap()
        .retain(|v| v != "calendar");
    value["unchecked"].as_array_mut().unwrap().extend([
        json!("calendar_preemption"),
        json!("person_overtime_pricing"),
    ]);
}
