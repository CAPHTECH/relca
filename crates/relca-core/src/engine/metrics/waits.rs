//! ADR 0048: retrospective interval partition. No scheduler/state mutations.
use super::{CaseObservation, TaskHistory};
use crate::ir;
use num_bigint::BigInt;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn amount(n: impl Into<BigInt>) -> Value {
    let s = n.into().to_string();
    json!({"min":s,"max":s,"reason":null})
}
fn causes(parts: &[BigInt; 6], extended: bool) -> Value {
    let mut v = json!({"actor_capacity":amount(parts[0].clone()),"resource":amount(parts[1].clone()),
        "executor":amount(parts[2].clone()),"combined":amount(parts[3].clone()),"unattributed":amount(parts[4].clone())});
    if extended {
        v["calendar"] = amount(parts[5].clone());
        v["condition"] = amount(0);
    }
    v
}
fn partition(parts: &[BigInt; 11], extended: bool) -> Value {
    let mut v = json!({"external":amount(parts[0].clone()),"in_service":amount(parts[1].clone()),
        "contest_route":amount(parts[2].clone()),"queue":{"actor_capacity":amount(parts[3].clone()),
        "resource":amount(parts[4].clone()),"executor":amount(parts[5].clone()),"combined":amount(parts[6].clone())},
        "intentional":amount(parts[7].clone()),"unattributed":amount(parts[8].clone())});
    if extended {
        v["queue"]["calendar"] = amount(parts[9].clone());
        v["condition"] = amount(parts[10].clone());
    }
    v
}
fn missing_partition(scalar: &Value, extended: bool) -> Value {
    let mut v = json!({"external":scalar,"in_service":scalar,"contest_route":scalar,
        "queue":{"actor_capacity":scalar,"resource":scalar,"executor":scalar,"combined":scalar},
        "intentional":scalar,"unattributed":scalar});
    if extended {
        v["queue"]["calendar"] = scalar.clone();
        v["condition"] = scalar.clone();
    }
    v
}

struct Past<'a> {
    case: usize,
    binding: ir::BindingId,
    cap: ir::CapabilityId,
    route: bool,
    h: &'a TaskHistory,
}
impl Past<'_> {
    fn running(&self, at: u64, until: u64) -> bool {
        self.h
            .start
            .is_some_and(|a| a <= at && at < self.h.stop.unwrap_or(until))
    }
    fn queued(&self, at: u64, until: u64) -> bool {
        self.h
            .ready
            .is_some_and(|a| a <= at && at < self.h.start.or(self.h.stop).unwrap_or(until))
    }
}

fn inventory<'a>(
    body: &'a ir::Block,
    calls: &mut Vec<&'a ir::Call>,
    forks: &mut Vec<&'a ir::Stmt>,
) {
    for s in body {
        match &s.kind {
            ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => calls.push(c),
            ir::Stmt::Response(c, r) => {
                calls.push(c);
                if r.attempt == 1
                    && let Some(body) = &r.recovery
                {
                    inventory(body, calls, forks);
                }
            }
            ir::Stmt::Fork { branches, .. } => {
                calls.extend(branches);
                forks.push(&s.kind);
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                inventory(then_body, calls, forks);
                inventory(else_body, calls, forks);
            }
            _ => {}
        }
    }
}

/// Cause of a queued task, using only occupancy in this same leaf. Permission,
/// Authority and the eventual assignee do not narrow the candidate executors.
fn cause(task: &Past<'_>, busy: &[&Past<'_>], m: &ir::Model, at: u64) -> usize {
    let cap = &m.capabilities[task.cap.0];
    let actor = &m.actors[cap.actor.0];
    let candidates = cap
        .qualified_executors
        .as_ref()
        .or(actor.executors.as_ref());
    let full_actor = actor.capacity.is_some_and(|n| {
        busy.iter()
            .filter(|t| m.capabilities[t.cap.0].actor == cap.actor)
            .count() as u64
            >= n
    });
    let full_resource = cap.uses.iter().any(|u| {
        let used: BigInt = busy
            .iter()
            .flat_map(|t| &m.capabilities[t.cap.0].uses)
            .filter(|x| x.resource == u.resource)
            .map(|x| BigInt::from(x.amount))
            .sum();
        used + BigInt::from(u.amount) > BigInt::from(m.resources[u.resource.0].capacity)
    });
    if let Some(tables) = &m.resource_time {
        let scheduled =
            |id: Option<usize>| id.map(|id| tables.calendars[id].capacity_at(at).unwrap_or(0));
        let actor_used = busy
            .iter()
            .filter(|t| m.capabilities[t.cap.0].actor == cap.actor)
            .count() as u64;
        let mut calendar = scheduled(tables.actors[cap.actor.0])
            .is_some_and(|n| actor.capacity.is_some_and(|p| n < p) && actor_used >= n);
        for u in &cap.uses {
            let used: BigInt = busy
                .iter()
                .flat_map(|t| &m.capabilities[t.cap.0].uses)
                .filter(|x| x.resource == u.resource)
                .map(|x| BigInt::from(x.amount))
                .sum();
            calendar |= scheduled(tables.resources[u.resource.0]).is_some_and(|n| {
                n < m.resources[u.resource.0].capacity
                    && used + BigInt::from(u.amount) > BigInt::from(n)
            });
        }
        let mut executor = false;
        if let Some(ps) = candidates {
            let unavailable = |p: &ir::PersonId| {
                let occupied = busy.iter().any(|t| t.h.executor == Some(*p));
                let off = scheduled(tables.people[p.0]) == Some(0);
                (occupied, off)
            };
            if !ps.is_empty()
                && ps.iter().all(|p| {
                    let (busy, off) = unavailable(p);
                    busy || off
                })
            {
                executor = ps.iter().any(|p| unavailable(p).0);
                calendar |= ps.iter().any(|p| unavailable(p).1);
            }
        }
        let flags = [full_actor, full_resource, executor, calendar];
        return match flags.iter().filter(|x| **x).count() {
            0 => 4,
            1 => [0, 1, 2, 5][flags.iter().position(|x| *x).unwrap()],
            _ => 3,
        };
    }
    match (full_actor, full_resource) {
        (true, true) => 3,
        (true, false) => 0,
        (false, true) => 1,
        _ if candidates.is_some_and(|people| {
            !people.is_empty()
                && people
                    .iter()
                    .all(|p| busy.iter().any(|t| t.h.executor == Some(*p)))
        }) =>
        {
            2
        }
        _ => 4,
    }
}

pub(super) fn augment(
    m: &ir::Model,
    cases: &[CaseObservation<'_>],
    until: u64,
    horizon: bool,
    value: &mut Value,
) {
    let extended = m.resource_time.is_some() || m.has_condition_wait();
    let mut calls = Vec::new();
    let mut forks = Vec::new();
    inventory(&m.process.body, &mut calls, &mut forks);
    let mut past = Vec::new();
    let mut points = BTreeSet::from([0, until]);
    if let Some(t) = &m.resource_time {
        for i in t.calendars.iter().flat_map(|c| &c.intervals) {
            points.insert(i.from.min(until));
            points.insert(i.until.min(until));
        }
    }
    for (i, c) in cases.iter().enumerate() {
        points.insert(c.arrival.min(until));
        if let Some(at) = c.admitted_at {
            points.insert(at.min(until));
        }
        if let Some(s) = c.state {
            points.insert(
                s.contest
                    .as_ref()
                    .and_then(|x| x.business.as_ref())
                    .map_or(s.time, |b| b.at)
                    .min(until),
            );
            if let Some(h) = &s.metric_history {
                for (begin, end) in h.conditions.iter().flatten() {
                    points.insert((*begin).min(until));
                    points.insert(end.unwrap_or(until).min(until));
                }
                for (begin, end) in h.waits.iter().flatten() {
                    points.insert((*begin).min(until));
                    points.insert((*end).min(until));
                }
                for call in &calls {
                    past.push(Past {
                        case: i,
                        binding: call.binding,
                        cap: call.capability,
                        route: false,
                        h: &h.tasks[call.binding.0],
                    });
                }
                for (id, contest) in m.contests().iter().enumerate() {
                    let binding = m.contest_site(id);
                    past.push(Past {
                        case: i,
                        binding,
                        cap: contest.route,
                        route: true,
                        h: &h.tasks[binding.0],
                    });
                }
            }
        }
    }
    for t in &past {
        for at in [t.h.ready, t.h.start, t.h.stop].into_iter().flatten() {
            points.insert(at.min(until));
        }
    }
    let points: Vec<_> = points.into_iter().collect();
    let mut parts = vec![std::array::from_fn::<_, 11, _>(|_| BigInt::from(0)); cases.len()];
    let mut dwell = parts.clone();
    let mut task_parts = vec![std::array::from_fn::<_, 6, _>(|_| BigInt::from(0)); past.len()];
    let mut queue_total = vec![BigInt::from(0); m.actors.len()];
    let mut queue_max = vec![0u64; m.actors.len()];
    for pair in points.windows(2) {
        let (at, end) = (pair[0], pair[1]);
        let length = BigInt::from(end - at);
        let busy: Vec<_> = past.iter().filter(|t| t.running(at, until)).collect();
        let mut q = vec![0u64; m.actors.len()];
        let mut case_causes = vec![Vec::new(); cases.len()];
        for (j, t) in past.iter().enumerate().filter(|(_, t)| t.queued(at, until)) {
            q[m.capabilities[t.cap.0].actor.0] += 1;
            let cause = cause(t, &busy, m, at);
            case_causes[t.case].push(cause);
            // A Task's legacy wait is known only after it starts, even though
            // an unstarted queue contributes to the observed integral.
            if t.h.start.is_some() {
                task_parts[j][cause] += &length;
            }
        }
        for (a, n) in q.into_iter().enumerate() {
            queue_total[a] += &length * BigInt::from(n);
            queue_max[a] = queue_max[a].max(n);
        }
        for (i, c) in cases.iter().enumerate() {
            if at < c.arrival {
                continue;
            }
            let bucket = if c.admitted_at.is_none_or(|t| at < t) {
                0
            } else if busy.iter().any(|t| t.case == i && !t.route) {
                1
            } else if busy.iter().any(|t| t.case == i && t.route) {
                2
            } else if let Some(first) = case_causes[i].first() {
                if case_causes[i].iter().any(|c| c != first) || *first == 3 {
                    6
                } else if *first == 4 {
                    8
                } else if *first == 5 {
                    9
                } else {
                    3 + first
                }
            } else if c
                .state
                .and_then(|s| s.metric_history.as_ref())
                .and_then(|h| h.conditions.as_ref())
                .is_some_and(|ws| ws.iter().any(|(a, b)| *a <= at && at < b.unwrap_or(until)))
            {
                10
            } else if c
                .state
                .and_then(|s| s.metric_history.as_ref())
                .and_then(|h| h.waits.as_ref())
                .is_some_and(|w| w.iter().any(|(a, b)| *a <= at && at < *b))
            {
                7
            } else {
                8
            };
            dwell[i][bucket] += &length;
            let business_at = c.state.map(|s| {
                s.contest
                    .as_ref()
                    .and_then(|x| x.business.as_ref())
                    .map_or(s.time, |b| b.at)
            });
            if c.end.is_some() && business_at.is_some_and(|stop| at < stop) {
                parts[i][bucket] += &length;
            }
        }
    }
    for (i, c) in cases.iter().enumerate() {
        let row = &mut value["cases"][i];
        for terminal in row["terminals"].as_array_mut().unwrap() {
            terminal["breakdown"] = if terminal["lead_time"]["min"].is_string() {
                partition(&parts[i], extended)
            } else {
                missing_partition(&terminal["lead_time"], extended)
            };
        }
        if horizon && c.end.is_none() && !c.rejected && !c.deadlocked && !c.not_arrived {
            row["dwell"] = json!({"until":amount(until),"breakdown":partition(&dwell[i], extended),"bound":"lower"});
        }
        for task in row["tasks"].as_array_mut().unwrap() {
            let binding = m
                .process
                .bindings
                .iter()
                .position(|b| task["binding"] == b.name)
                .unwrap();
            let found = past
                .iter()
                .enumerate()
                .find(|(_, t)| t.case == i && t.binding.0 == binding && !t.route);
            task["wait_causes"] = if task["wait"]["min"].is_string() {
                causes(&task_parts[found.unwrap().0], extended)
            } else {
                let n = &task["wait"];
                let mut v = json!({"actor_capacity":n,"resource":n,"executor":n,"combined":n,"unattributed":n});
                if extended {
                    v["calendar"] = n.clone();
                    v["condition"] = n.clone();
                }
                v
            };
            task["predecessor_wait"] = match (found.and_then(|(_, t)| t.h.ready), c.admitted_at) {
                (Some(ready), Some(admitted)) => amount(
                    ready
                        .checked_sub(admitted)
                        .expect("metric invariant: Task.ready precedes admission"),
                ),
                _ => json!({"min":null,"max":null,"reason":"never_created"}),
            };
        }
        row["joins"] = Value::Array(forks.iter().map(|fork| {
            let ir::Stmt::Fork { id, branches, join } = fork else { unreachable!() };
            let hs: Vec<_> = branches.iter().map(|b| past.iter().find(|t| t.case==i && t.binding==b.binding).map(|t|t.h)).collect();
            let created = hs.iter().any(|h| h.is_some_and(|h|h.ready.is_some()));
            let stops: Option<Vec<u64>> = hs.iter().map(|h| h.and_then(|h|h.stop)).collect();
            let last = stops.as_ref().and_then(|s|s.iter().max()).copied();
            json!({"fork":m.process.forks[id.0],"mode":if matches!(join,ir::Join::All){"all"}else{"any"},
                "branches":branches.iter().zip(hs).map(|(b,h)|json!({"binding":m.process.bindings[b.binding.0].name,
                    "join_wait":if !matches!(join,ir::Join::All) { json!({"min":null,"max":null,"reason":"join_any"}) }
                    else if let Some(last)=last { amount(last-h.unwrap().stop.unwrap()) }
                    else { json!({"min":null,"max":null,"reason":if !created {"never_created"}else if c.deadlocked {"deadlock"}else{"unfinished"}}) }
                })).collect::<Vec<_>>()})
        }).collect());
    }
    for (row, id) in value["actors"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(&m.metrics.as_ref().unwrap().required_actors)
    {
        let avg = if until == 0 {
            json!({"min":null,"max":null,"reason":"no_samples"})
        } else {
            let s = num_rational::BigRational::new(queue_total[id.0].clone(), BigInt::from(until))
                .to_string();
            json!({"min":s,"max":s,"reason":null})
        };
        row["queue"] = json!({"wait_total":{"stats":amount(queue_total[id.0].clone())},"length_max":amount(queue_max[id.0]),"length_time_average":{"stats":avg}});
        if horizon {
            row["queue"]["wait_total"]["bound"] = "lower".into();
            row["queue"]["length_time_average"]["bound"] = "lower".into();
        }
    }
    value["overall"]["queue_wait_observed"] = amount(queue_total.into_iter().sum::<BigInt>());
    if m.has_condition_wait() {
        value["checked"]
            .as_array_mut()
            .unwrap()
            .push(json!("condition_wait_partition"));
    }
    value["checked"].as_array_mut().unwrap().extend([
        json!("waiting_decomposition"),
        json!("queue_length_max_and_average"),
    ]);
    let unchecked = value["unchecked"].as_array_mut().unwrap();
    unchecked.retain(|w| w != "waiting_decomposition");
    for w in unchecked {
        if w == "queue_length_statistics" {
            *w = "queue_length_quantiles".into();
        }
    }
}
