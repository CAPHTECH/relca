//! Independent ADR0031 old-IR walker. Never imports production diff or its helpers.
#![allow(dead_code)]
use relca_core::{ir, lower, responsibility as resp, syntax};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const NEW_CHECKED: [&str; 10] = [
    "meaning",
    "authority",
    "resource",
    "time",
    "visibility",
    "contest",
    "preserve",
    "override",
    "maintain",
    "annotations",
];
pub const REMOVE_UNCHECKED: [&str; 10] = [
    "time_intent",
    "resource",
    "meaning",
    "authority",
    "visibility",
    "contest",
    "preserve",
    "override",
    "maintain",
    "responsibility",
];

#[derive(Default)]
struct Shape {
    modes: BTreeMap<ir::CapabilityId, BTreeSet<u8>>,
    ends: BTreeSet<ir::EndId>,
}
fn walk(block: &ir::Block, s: &mut Shape) {
    for statement in block {
        match &statement.kind {
            ir::Stmt::Query(_, _)
            | ir::Stmt::ResolveEffect { .. }
            | ir::Stmt::StopReceiptUnknown => panic!("receipt recovery is outside this oracle"),
            ir::Stmt::WaitUntil(_) => panic!("old-IR diff oracle does not handle condition waits"),
            ir::Stmt::Response(_, _) => {
                panic!("old-IR diff oracle does not cover receipt or retry semantics")
            }
            ir::Stmt::Call(c) => {
                s.modes.entry(c.capability).or_default().insert(0);
            }
            ir::Stmt::Fork { branches, join, .. } => {
                for c in branches {
                    s.modes.entry(c.capability).or_default().insert(1);
                }
                if let ir::Join::Any { if_none, .. } = join {
                    s.ends.insert(*if_none);
                }
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                walk(then_body, s);
                walk(else_body, s);
            }
            ir::Stmt::End(e) => {
                s.ends.insert(*e);
            }
            ir::Stmt::Wait(_) => {}
        }
    }
}
fn capname(m: &ir::Model, id: ir::CapabilityId) -> String {
    let c = &m.capabilities[id.0];
    format!("{}.{}", m.actors[c.actor.0].name, c.name)
}
fn people(m: &ir::Model, ps: &[ir::PersonId]) -> Vec<String> {
    ps.iter()
        .map(|p| m.people[p.0].name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn condvars(c: &ir::Cond, vs: &mut BTreeSet<ir::StateVarId>) {
    match c {
        // ADR 0029: an integer comparison reads its one variable.
        ir::Cond::IntCompare { variable, .. } => {
            vs.insert(*variable);
        }
        ir::Cond::StateEq(x) => {
            vs.insert(x.variable);
        }
        // ADR 0035: membership reads its set variable.
        ir::Cond::Contains { set, .. } => {
            vs.insert(*set);
        }
        ir::Cond::Eq(_) => {}
        ir::Cond::And(cs) | ir::Cond::Or(cs) => {
            for c in cs {
                condvars(c, vs);
            }
        }
    }
}
fn formula(f: &resp::Formula, vs: &mut BTreeSet<ir::StateVarId>) {
    match f {
        resp::Formula::State(c) => condvars(c, vs),
        resp::Formula::Bool(x) => {
            vs.insert(x.variable);
        }
        resp::Formula::Not(f) => formula(f, vs),
        resp::Formula::And(fs) | resp::Formula::Or(fs) => {
            for f in fs {
                formula(f, vs);
            }
        }
    }
}
fn duty_applies(d: &resp::Duty, m: &ir::Model, id: ir::CapabilityId) -> bool {
    match &d.target {
        resp::Target::Capability(c) => *c == id,
        resp::Target::Maintenance(a) => *a == m.capabilities[id.0].actor,
        target => {
            let mut vs = BTreeSet::new();
            match target {
                resp::Target::Condition(c) => condvars(c, &mut vs),
                resp::Target::Formula(f) => formula(f, &mut vs),
                _ => unreachable!(),
            }
            let c = &m.capabilities[id.0];
            c.writes
                .iter()
                .chain(&c.start_writes)
                .any(|w| vs.contains(&w.variable))
                || c.kernel_writes.iter().any(|v| vs.contains(v))
        }
    }
}
fn authority(m: &ir::Model, src: &str, id: ir::CapabilityId) -> Value {
    // Permission (design v0.2 §7.2): the permit union, else the Actor's
    // executors (ir.rs), else null for anonymous executors without permit.
    let c = &m.capabilities[id.0];
    let permission = match (&c.permission, &m.actors[c.actor.0].executors) {
        (Some(ps), _) | (None, Some(ps)) => json!(people(m, ps)),
        (None, None) => Value::Null,
    };
    let mut grants = Vec::new();
    for a in &m.authorities {
        let ps: BTreeSet<_> = a
            .grants
            .iter()
            .filter(|g| g.capability == id)
            .flat_map(|g| g.persons.iter().copied())
            .collect();
        if ps.is_empty() {
            continue;
        }
        let r = &a.restrictions;
        let revoker = r.revocable_by.map(|s| match s {
            ir::AuthorityRevoker::Person(p) => json!({"kind":"person","name":m.people[p.0].name}),
            ir::AuthorityRevoker::Actor(a) => json!({"kind":"actor","name":m.actors[a.0].name}),
        });
        grants.push(json!({"authority":a.name,"persons":people(m,&ps.into_iter().collect::<Vec<_>>()),
            "basis":a.basis,"issued_by":a.issued_by.map(|p|&m.people[p.0].name),"not_by":people(m,&a.not_by),
            "scope":r.scope_span.map(|s|src[s.start..s.end].strip_prefix("scope").expect("scope field provenance").trim()),"expires":r.expires,"effective_expires":r.effective_expires,
            "revocable_by":revoker,"delegated_from":r.delegated_from.map(|a|&m.authorities[a.0].name)}));
    }
    grants.sort_by_key(|g| g["authority"].as_str().unwrap().to_owned());
    let mut basis = Vec::new();
    for (i, c) in m.contracts.iter().enumerate() {
        if let ir::ContractKind::AuthorityBasis {
            capability,
            authority,
        } = c.kind
            && capability == id
        {
            basis.push(json!({"id":c.property_id(ir::ClauseId(i)),"authority":m.authorities[authority.0].name}));
        }
    }
    basis.sort_by_key(|b| b["id"].as_str().unwrap().to_owned());
    let mut duties = Vec::new();
    if let Some(meta) = &m.responsibility {
        for (i, d) in meta
            .duties
            .iter()
            .enumerate()
            .filter(|(_, d)| duty_applies(d, m, id))
        {
            let description = &meta
                .responsibilities
                .iter()
                .find(|r| r.id == i + 1)
                .expect("existing stable duty id")
                .target;
            duties.push(json!({"id":i+1,"person":m.people[d.person.0].name,"target":description,"within":d.within}));
        }
    }
    if permission.is_null() && grants.is_empty() && basis.is_empty() && duties.is_empty() {
        Value::Null
    } else {
        json!({"permission":permission,"grants":grants,"authority_basis":basis,"responsibilities":duties})
    }
}
struct Projection {
    values: BTreeMap<(&'static str, String), Value>,
    modes: BTreeMap<String, Value>,
}
fn project(m: &ir::Model, src: &str) -> Projection {
    let mut shape = Shape::default();
    walk(&m.process.body, &mut shape);
    let mut active: BTreeSet<_> = shape.modes.keys().copied().collect();
    for c in m
        .contests()
        .iter()
        .filter(|c| c.target.end().is_some_and(|e| shape.ends.contains(&e)))
    {
        active.insert(c.route);
    }
    let mut values = BTreeMap::new();
    for e in &shape.ends {
        values.insert(("meaning", format!("end.{}", m.ends[e.0])), json!(true));
    }
    for id in &active {
        let value = authority(m, src, *id);
        if !value.is_null() {
            values.insert(("authority", capname(m, *id)), value);
        }
    }
    let owners: BTreeSet<_> = active.iter().map(|id| m.capabilities[id.0].actor).collect();
    for a in owners {
        let actor = &m.actors[a.0];
        values.insert(("resource",actor.name.clone()),json!({"capacity":actor.capacity,"executors":actor.executors.as_ref().map(|p|people(m,p))}));
    }
    for (i, a) in m
        .actors
        .iter()
        .enumerate()
        .filter(|(_, a)| a.source.is_some())
    {
        let readers: BTreeSet<_> = active
            .iter()
            .filter(|c| m.capabilities[c.0].reads.contains(&ir::ActorId(i)))
            .map(|c| m.actors[m.capabilities[c.0].actor.0].name.clone())
            .collect();
        values.insert(("visibility", a.name.clone()), json!(readers));
    }
    for e in &shape.ends {
        let mut records:Vec<_>=m.contests().iter().filter(|c|c.target.end()==Some(*e)).map(|c|json!({
            "id":c.property_id(),"party":m.actors[c.party.0].name,"route":capname(m,c.route),
            "reversible_by":people(m,&c.reversible_by),"respond_by":c.respond_by
        })).collect();
        records.sort_by_key(|r| r["id"].as_str().unwrap().to_owned());
        if !records.is_empty() {
            values.insert(("contest", format!("end.{}", m.ends[e.0])), json!(records));
        }
    }
    let modes = shape
        .modes
        .into_iter()
        .map(|(c, ms)| {
            (
                capname(m, c),
                json!(
                    ms.into_iter()
                        .map(|i| if i == 0 { "sequential" } else { "fork" })
                        .collect::<Vec<_>>()
                ),
            )
        })
        .collect();
    Projection { values, modes }
}

/// Return independent status and complete new rows, not a projection of any
/// newly implemented diff output. All inputs here use the frozen OLD syntax.
pub fn rows(src: &str, from: &str, to: &str) -> (String, Vec<Value>) {
    let Ok(ast) = syntax::parse(src) else {
        return ("parse_failure".into(), vec![]);
    };
    let (Ok(a), Ok(b)) = (
        lower::lower_configuration(&ast, from),
        lower::lower_configuration(&ast, to),
    ) else {
        return ("lower_failure".into(), vec![]);
    };
    if a.model.cut.as_ref().map(|c| (&c.name, &c.horizon))
        != b.model.cut.as_ref().map(|c| (&c.name, &c.horizon))
    {
        return ("cut_mismatch".into(), vec![]);
    }
    let a = project(&a.model, src);
    let b = project(&b.model, src);
    let mut out = vec![];
    for kind in [
        "meaning",
        "authority",
        "resource",
        "time",
        "visibility",
        "contest",
    ] {
        let empty = match kind {
            "meaning" => json!(false),
            "visibility" | "contest" => json!([]),
            _ => Value::Null,
        };
        let targets: BTreeSet<_> = if kind == "time" {
            a.modes
                .keys()
                .filter(|k| b.modes.contains_key(*k))
                .cloned()
                .collect()
        } else {
            a.values
                .keys()
                .chain(b.values.keys())
                .filter(|(k, _)| *k == kind)
                .map(|(_, t)| t.clone())
                .collect()
        };
        for target in targets {
            let av = if kind == "time" {
                &a.modes[&target]
            } else {
                a.values.get(&(kind, target.clone())).unwrap_or(&empty)
            };
            let bv = if kind == "time" {
                &b.modes[&target]
            } else {
                b.values.get(&(kind, target.clone())).unwrap_or(&empty)
            };
            if av == bv {
                continue;
            }
            let change = if kind == "time" || kind == "visibility" {
                "modified"
            } else if av == &empty {
                "added"
            } else if bv == &empty {
                "removed"
            } else {
                "modified"
            };
            out.push(json!({"kind":kind,"target":target,"change":change,"reason":null,"loses":null,"from":av,"to":bv}));
        }
    }
    ("compared".into(), out)
}
