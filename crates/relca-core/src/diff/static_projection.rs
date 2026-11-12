//! Lexical configuration projection. Normal calls and active contest routes are
//! deliberately separate: routes must not change the legacy actor/relation rows.
use super::{CallMode, Change, DiffValue, Difference, Kind, ProducerValue};
use crate::{ir, responsibility as resp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceValue {
    pub capacity: Option<u64>,
    pub executors: Option<Vec<String>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityValue {
    /// The capability's effective Permission: its permit union, else the
    /// Actor's executors, else null (anonymous executors without permit).
    pub permission: Option<Vec<String>>,
    pub grants: Vec<GrantValue>,
    pub authority_basis: Vec<Basis>,
    pub responsibilities: Vec<DutyValue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    pub kind: String,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantValue {
    pub authority: String,
    pub persons: Vec<String>,
    pub basis: String,
    pub issued_by: Option<String>,
    pub not_by: Vec<String>,
    pub scope: Option<String>,
    pub expires: Option<u64>,
    pub effective_expires: Option<u64>,
    pub revocable_by: Option<Subject>,
    pub delegated_from: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Basis {
    pub id: String,
    pub authority: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DutyValue {
    pub id: usize,
    pub person: String,
    pub target: resp::TargetDescription,
    pub within: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContestValue {
    pub id: String,
    pub party: String,
    pub route: String,
    pub reversible_by: Vec<String>,
    pub respond_by: u64,
}

#[derive(Default)]
pub(super) struct Shape {
    pub modes: BTreeMap<ir::CapabilityId, BTreeSet<CallMode>>,
    pub ends: BTreeSet<ir::EndId>,
    pub calls: BTreeMap<ir::BindingId, ir::Call>,
    pub result_reads: BTreeSet<ir::BindingId>,
}
impl Shape {
    pub fn new(m: &ir::Model) -> Self {
        let mut shape = Self::default();
        shape.walk(&m.process.body);
        shape
    }
    fn call(&mut self, c: &ir::Call, mode: CallMode) {
        self.modes.entry(c.capability).or_default().insert(mode);
        self.calls.insert(c.binding, c.clone());
        self.result_reads
            .extend(c.args.iter().copied().filter(|b| *b != ir::CURRENT_CASE));
    }
    fn condition(&mut self, c: &ir::Cond) {
        match c {
            ir::Cond::Eq(result) => {
                self.result_reads.insert(result.binding);
            }
            ir::Cond::And(cs) | ir::Cond::Or(cs) => {
                for c in cs {
                    self.condition(c);
                }
            }
            ir::Cond::StateEq(_) | ir::Cond::IntCompare { .. } | ir::Cond::Contains { .. } => {}
        }
    }
    fn walk(&mut self, block: &ir::Block) {
        for s in block {
            match &s.kind {
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => self.call(c, CallMode::Sequential),
                ir::Stmt::ResolveEffect { evidence, .. } => {
                    self.result_reads.insert(*evidence);
                }
                ir::Stmt::StopReceiptUnknown => {}
                ir::Stmt::Response(_, r) => {
                    if r.attempt == 1 {
                        self.call(&r.command_call, CallMode::Sequential);
                        self.ends.extend(r.exhausted);
                        if let Some(body) = &r.recovery {
                            self.walk(body);
                        }
                    }
                }
                ir::Stmt::Fork { branches, join, .. } => {
                    for c in branches {
                        self.call(c, CallMode::Fork);
                    }
                    match join {
                        ir::Join::All => {}
                        ir::Join::Any {
                            matches, if_none, ..
                        } => {
                            self.result_reads.extend(matches.iter().map(|r| r.binding));
                            self.ends.insert(*if_none);
                        }
                    }
                }
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    self.condition(condition);
                    self.walk(then_body);
                    self.walk(else_body);
                }
                ir::Stmt::End(e) => {
                    self.ends.insert(*e);
                }
                ir::Stmt::Wait(_) | ir::Stmt::WaitUntil(_) => {}
            }
        }
    }
    pub fn active(&self, m: &ir::Model) -> BTreeSet<ir::CapabilityId> {
        self.modes
            .keys()
            .copied()
            .chain(
                m.contests()
                    .iter()
                    .filter(|c| match c.target {
                        crate::contest::ContestTarget::End(e) => self.ends.contains(&e),
                        crate::contest::ContestTarget::Commit(cap) => self.modes.contains_key(&cap),
                    })
                    .map(|c| c.route),
            )
            .collect()
    }
}
pub(super) fn capname(m: &ir::Model, id: ir::CapabilityId) -> String {
    let cap = &m.capabilities[id.0];
    format!("{}.{}", m.actors[cap.actor.0].name, cap.name)
}

/// Only normal process bindings mentioned by arguments or process result conditions
/// (If and join any matches) participate, including dead branches and suffixes after End;
/// it is not a reaching-definition analysis of state writes or synthetic routes.
fn read_producers(m: &ir::Model) -> BTreeMap<String, String> {
    let shape = Shape::new(m);
    shape
        .calls
        .iter()
        .filter(|(binding, _)| shape.result_reads.contains(binding))
        .map(|(binding, call)| {
            (
                m.process.bindings[binding.0].name.clone(),
                capname(m, call.capability),
            )
        })
        .collect()
}

pub(super) fn producer_differences(from: &ir::Model, to: &ir::Model) -> Vec<Difference> {
    let a = read_producers(from);
    let b = read_producers(to);
    a.into_iter()
        .filter_map(|(target, from)| {
            let to = b.get(&target)?;
            (from != *to).then(|| Difference {
                kind: Kind::Relation,
                target,
                change: Change::Modified,
                reason: None,
                loses: None,
                from: Some(DiffValue::Producer(ProducerValue { producer: from })),
                to: Some(DiffValue::Producer(ProducerValue {
                    producer: to.clone(),
                })),
                annotations: vec![],
            })
        })
        .collect()
}
fn people(m: &ir::Model, ids: impl IntoIterator<Item = ir::PersonId>) -> Vec<String> {
    ids.into_iter()
        .map(|p| m.people[p.0].name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn condvars(c: &ir::Cond, out: &mut BTreeSet<ir::StateVarId>) {
    match c {
        // ADR 0029: the same read set as responsibility's condition walk.
        ir::Cond::IntCompare { variable, .. } | ir::Cond::Contains { set: variable, .. } => {
            out.insert(*variable);
        }
        ir::Cond::StateEq(x) => {
            out.insert(x.variable);
        }
        ir::Cond::Eq(_) => {}
        ir::Cond::And(cs) | ir::Cond::Or(cs) => {
            for c in cs {
                condvars(c, out);
            }
        }
    }
}
fn formula_vars(f: &resp::Formula, out: &mut BTreeSet<ir::StateVarId>) {
    match f {
        resp::Formula::State(c) => condvars(c, out),
        resp::Formula::Bool(x) => {
            out.insert(x.variable);
        }
        resp::Formula::Not(f) => formula_vars(f, out),
        resp::Formula::And(fs) | resp::Formula::Or(fs) => {
            for f in fs {
                formula_vars(f, out);
            }
        }
    }
}
fn applies(target: &resp::Target, m: &ir::Model, id: ir::CapabilityId) -> bool {
    let cap = &m.capabilities[id.0];
    let mut vars = BTreeSet::new();
    match target {
        resp::Target::Capability(c) => return *c == id,
        resp::Target::Maintenance(a) => return *a == cap.actor,
        resp::Target::Condition(c) => condvars(c, &mut vars),
        resp::Target::Formula(f) => formula_vars(f, &mut vars),
    }
    cap.writes
        .iter()
        .chain(&cap.start_writes)
        .any(|w| vars.contains(&w.variable))
        || cap.kernel_writes.iter().any(|v| vars.contains(v))
}
fn authority(m: &ir::Model, src: &str, id: ir::CapabilityId) -> DiffValue {
    let cap = &m.capabilities[id.0];
    let permission = cap
        .permission
        .as_ref()
        .or(m.actors[cap.actor.0].executors.as_ref())
        .map(|ps| people(m, ps.iter().copied()));
    let mut grants = Vec::new();
    for a in &m.authorities {
        let persons = people(
            m,
            a.grants
                .iter()
                .filter(|g| g.capability == id)
                .flat_map(|g| g.persons.iter().copied()),
        );
        if persons.is_empty() {
            continue;
        }
        let r = &a.restrictions;
        grants.push(GrantValue {
            authority: a.name.clone(),
            persons,
            basis: a.basis.clone(),
            issued_by: a.issued_by.map(|p| m.people[p.0].name.clone()),
            not_by: people(m, a.not_by.iter().copied()),
            scope: r.scope_span.map(|s| {
                src[s.start..s.end]
                    .strip_prefix("scope")
                    .expect("scope provenance")
                    .trim()
                    .to_owned()
            }),
            expires: r.expires,
            effective_expires: r.effective_expires,
            revocable_by: r.revocable_by.map(|s| match s {
                ir::AuthorityRevoker::Person(p) => Subject {
                    kind: "person".into(),
                    name: m.people[p.0].name.clone(),
                },
                ir::AuthorityRevoker::Actor(a) => Subject {
                    kind: "actor".into(),
                    name: m.actors[a.0].name.clone(),
                },
            }),
            delegated_from: r.delegated_from.map(|a| m.authorities[a.0].name.clone()),
        });
    }
    grants.sort_by(|a, b| a.authority.cmp(&b.authority));
    let mut authority_basis = Vec::new();
    for (i, c) in m.contracts.iter().enumerate() {
        if let ir::ContractKind::AuthorityBasis {
            capability,
            authority,
        } = c.kind
            && capability == id
        {
            authority_basis.push(Basis {
                id: c.property_id(ir::ClauseId(i)),
                authority: m.authorities[authority.0].name.clone(),
            });
        }
    }
    authority_basis.sort_by(|a, b| (&a.id, &a.authority).cmp(&(&b.id, &b.authority)));
    let mut responsibilities = Vec::new();
    if let Some(meta) = &m.responsibility {
        for (i, d) in meta
            .duties
            .iter()
            .enumerate()
            .filter(|(_, d)| applies(&d.target, m, id))
        {
            let description = &meta
                .responsibilities
                .iter()
                .find(|r| r.id == i + 1)
                .expect("stable responsibility id")
                .target;
            responsibilities.push(DutyValue {
                id: i + 1,
                person: m.people[d.person.0].name.clone(),
                target: description.clone(),
                within: d.within,
            });
        }
    }
    if permission.is_none()
        && grants.is_empty()
        && authority_basis.is_empty()
        && responsibilities.is_empty()
    {
        DiffValue::Inactive
    } else {
        DiffValue::Authority(AuthorityValue {
            permission,
            grants,
            authority_basis,
            responsibilities,
        })
    }
}
fn projection(src: &str, m: &ir::Model) -> BTreeMap<(Kind, String), DiffValue> {
    let shape = Shape::new(m);
    let active = shape.active(m);
    let mut out = BTreeMap::new();
    for e in &shape.ends {
        out.insert(
            (Kind::Meaning, format!("end.{}", m.ends[e.0])),
            DiffValue::Meaning(true),
        );
    }
    for id in &active {
        let value = authority(m, src, *id);
        if value != DiffValue::Inactive {
            out.insert((Kind::Authority, capname(m, *id)), value);
        }
    }
    let owners: BTreeSet<_> = active.iter().map(|c| m.capabilities[c.0].actor).collect();
    for id in owners {
        let a = &m.actors[id.0];
        out.insert(
            (Kind::Resource, a.name.clone()),
            DiffValue::Resource(ResourceValue {
                capacity: a.capacity,
                executors: a.executors.as_ref().map(|ps| people(m, ps.iter().copied())),
            }),
        );
    }
    for (id, a) in m
        .actors
        .iter()
        .enumerate()
        .filter(|(_, a)| a.source.is_some())
    {
        let readers = active
            .iter()
            .filter(|c| m.capabilities[c.0].reads.contains(&ir::ActorId(id)))
            .map(|c| m.actors[m.capabilities[c.0].actor.0].name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        out.insert(
            (Kind::Visibility, a.name.clone()),
            DiffValue::Readers(readers),
        );
    }
    let targets: BTreeSet<_> = m
        .contests()
        .iter()
        .filter(|c| match c.target {
            crate::contest::ContestTarget::End(e) => shape.ends.contains(&e),
            crate::contest::ContestTarget::Commit(cap) => shape.modes.contains_key(&cap),
        })
        .map(|c| c.target)
        .collect();
    for target in targets {
        let mut records: Vec<_> = m
            .contests()
            .iter()
            .filter(|c| c.target == target)
            .map(|c| ContestValue {
                id: c.property_id(),
                party: m.actors[c.party.0].name.clone(),
                route: capname(m, c.route),
                reversible_by: people(m, c.reversible_by.iter().copied()),
                respond_by: c.respond_by,
            })
            .collect();
        records.sort_by(|a, b| a.id.cmp(&b.id));
        if !records.is_empty() {
            out.insert(
                (
                    Kind::Contest,
                    if target.effect() {
                        target.name(m)
                    } else {
                        format!("end.{}", target.name(m))
                    },
                ),
                DiffValue::Contests(records),
            );
        }
    }
    for (id, modes) in shape.modes {
        out.insert(
            (Kind::Time, capname(m, id)),
            DiffValue::Time(modes.into_iter().collect()),
        );
    }
    out
}
pub(super) fn differences(src: &str, from: &ir::Model, to: &ir::Model) -> Vec<Difference> {
    let a = projection(src, from);
    let b = projection(src, to);
    let keys: BTreeSet<_> = a.keys().chain(b.keys()).cloned().collect();
    let mut out = Vec::new();
    for (kind, target) in keys {
        let empty = match kind {
            Kind::Meaning => DiffValue::Meaning(false),
            Kind::Visibility => DiffValue::Readers(vec![]),
            Kind::Contest => DiffValue::Contests(vec![]),
            Kind::Actor
            | Kind::Relation
            | Kind::Authority
            | Kind::Resource
            | Kind::Time
            | Kind::Rule
            | Kind::Qualification
            | Kind::Performance
            | Kind::Degradation => DiffValue::Inactive,
        };
        let key = (kind, target.clone());
        if kind == Kind::Time && (!a.contains_key(&key) || !b.contains_key(&key)) {
            continue;
        }
        let from = a.get(&key).unwrap_or(&empty);
        let to = b.get(&key).unwrap_or(&empty);
        if from == to {
            continue;
        }
        let change = if matches!(kind, Kind::Time | Kind::Visibility) {
            Change::Modified
        } else if from == &empty {
            Change::Added
        } else if to == &empty {
            Change::Removed
        } else {
            Change::Modified
        };
        out.push(Difference {
            kind,
            target,
            change,
            reason: None,
            loses: None,
            from: Some(from.clone()),
            to: Some(to.clone()),
            annotations: vec![],
        });
    }
    out
}

#[cfg(test)]
mod effect_inventory_tests {
    use super::*;
    #[test]
    fn lexical_effect_route_alone_supplies_visibility_and_authority_inventory() {
        let src = include_str!("../../../../fixtures/sov-basic/model.relca")
            .replace("  actor Vendor", "  actor Document { source document }\n  actor Vendor")
            .replace("capability recheck() -> Verdict {", "capability recheck() -> Verdict { reads [Document];")
            .replace("  permit a1", "  authority RouteGrant { basis \"route\"; grants c1 to Clerk.recheck; issued_by cfo }\n  permit a1")
            .replace("Clerk, Vendor]", "Clerk, Vendor, Document]");
        let lower = |src: &str| crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let m = lower(&src);
        let rows = projection(&src, &m);
        assert!(rows.contains_key(&(Kind::Authority, "Clerk.recheck".into())));
        assert_eq!(
            rows[&(Kind::Visibility, "Document".into())],
            DiffValue::Readers(vec!["Clerk".into()])
        );
        let inactive = src.replace("ack = Ledger.post();", "");
        let m = lower(&inactive);
        let rows = projection(&inactive, &m);
        assert!(!rows.contains_key(&(Kind::Authority, "Clerk.recheck".into())));
        assert_eq!(
            rows[&(Kind::Visibility, "Document".into())],
            DiffValue::Readers(vec![])
        );
    }
}
