//! ADR 0023 declaration metadata. Never consulted by the scheduler or monitors.
use crate::ir;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// The new population is excluded from frozen, older migration censuses by
/// exact name, not by a permissive prefix. Unknown additions must be classified.
#[cfg(test)]
pub(crate) const FIXTURES: [&str; 28] = [
    "act-arrival-units",
    "act-arrival-units.broken-later-first",
    "act-composite",
    "act-composite.broken-team-work",
    "act-control",
    "act-control.broken-explicit-environment",
    "act-cut-context",
    "act-cut-context.broken-no-cut",
    "act-debtor",
    "act-debtor.broken-unknown",
    "act-debtor-actor",
    "act-debtor-actor.broken-omitted",
    "act-debtor-value",
    "act-debtor-value.broken-other-person",
    "act-defaults",
    "act-defaults.broken-explicit-boundary",
    "act-grain",
    "act-grain.broken-swapped",
    "act-horizon",
    "act-horizon.broken-short-horizon",
    "act-legacy",
    "act-legacy.broken-longer-task",
    "act-overlap",
    "act-overlap.broken-relation",
    "act-units",
    "act-units.broken-duration-day",
    "act-via",
    "act-via.broken-upstream-only",
];
#[cfg(test)]
pub(crate) fn fixture(name: &str) -> bool {
    assert!(
        !name.starts_with("act-") || FIXTURES.contains(&name),
        "unclassified Actor fixture {name}"
    );
    FIXTURES.contains(&name)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub value: Value,
    pub status: Status,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Declared,
    Default,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub name: String,
    pub boundary: Field,
    pub grain: Field,
    pub contains: Field,
    pub overlaps: Field,
    pub capability: Field,
    pub capacity: Field,
    pub executors: Field,
    pub control: Field,
    pub source: Field,
    pub via_model: Field,
    pub state: Field,
    pub latent: Field,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<Rule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleScalar {
    Bool(bool),
    Enum(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleVersion {
    pub declared: String,
    pub effective: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleValue {
    pub name: String,
    pub declared: RuleScalar,
    pub effective: RuleScalar,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Amendment {
    pub from: String,
    pub to: String,
    pub by: String,
    pub under: String,
    pub under_version: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub version: RuleVersion,
    pub values: Vec<RuleValue>,
    pub amendment: Option<Amendment>,
    pub read_by: Vec<String>,
}

pub(crate) fn rule_scalar(
    m: &ir::Model,
    v: &ir::StateVariable,
    value: ir::StateValue,
) -> RuleScalar {
    let ir::StateValue::Value(value) = value else {
        unreachable!("validated Rule scalar")
    };
    let ty = &m.types[v.base.0];
    let name = &ty.members[value.0];
    if ty.name == "bool" {
        RuleScalar::Bool(name == "true")
    } else {
        RuleScalar::Enum(name.clone())
    }
}
pub(crate) fn amendment(m: &ir::Model, a: &ir::RuleAmendment) -> Amendment {
    let authority = &m.authorities[a.under.0];
    Amendment {
        from: a.from.clone(),
        to: a.to.clone(),
        by: m.people[a.by.0].name.clone(),
        under: authority.name.clone(),
        under_version: authority.under.as_ref().map(|b| b.version.clone()),
    }
}
pub(crate) fn rule_reads(c: &ir::Cond, m: &ir::Model, actor: ir::ActorId) -> bool {
    match c {
        ir::Cond::StateEq(x) => m.state_variables[x.variable.0].kind == ir::StateKind::Rule(actor),
        ir::Cond::And(cs) | ir::Cond::Or(cs) => cs.iter().any(|c| rule_reads(c, m, actor)),
        _ => false,
    }
}
fn rule(m: &ir::Model, actor: ir::ActorId) -> Option<Rule> {
    let info = m.actors[actor.0].metadata.rule.as_ref()?;
    let values = info
        .values
        .iter()
        .map(|v| {
            let variable = &m.state_variables[v.variable.0];
            let ir::InitialValue::Fixed(effective) = variable.initial else {
                unreachable!("validated Rule initial")
            };
            RuleValue {
                name: variable.name.clone(),
                declared: rule_scalar(m, variable, v.declared),
                effective: rule_scalar(m, variable, effective),
            }
        })
        .collect();
    let mut read_by = Vec::new();
    for c in &m.capabilities {
        if c.requires
            .as_ref()
            .is_some_and(|r| rule_reads(&r.condition, m, actor))
        {
            read_by.push(format!("{}.{}.requires", m.actors[c.actor.0].name, c.name));
        }
    }
    fn positions(
        b: &ir::Block,
        m: &ir::Model,
        a: ir::ActorId,
        n: &mut usize,
        out: &mut Vec<String>,
    ) {
        for s in b {
            if let ir::Stmt::If {
                condition,
                then_body,
                else_body,
            } = &s.kind
            {
                *n += 1;
                if rule_reads(condition, m, a) {
                    out.push(format!("{}.if#{}", m.process.name, *n));
                }
                positions(then_body, m, a, n, out);
                positions(else_body, m, a, n, out);
            }
        }
    }
    positions(&m.process.body, m, actor, &mut 0, &mut read_by);
    Some(Rule {
        version: RuleVersion {
            declared: info.declared_version.clone(),
            effective: info.effective_version.clone(),
        },
        values,
        amendment: info.amendment.as_ref().map(|a| amendment(m, a)),
        read_by,
    })
}

fn field(value: Value, declared: bool) -> Field {
    Field {
        value,
        status: if declared {
            Status::Declared
        } else {
            Status::Default
        },
    }
}

pub(crate) fn project(m: &ir::Model) -> Vec<Actor> {
    let mut called = BTreeSet::new();
    fn calls(b: &ir::Block, m: &ir::Model, out: &mut BTreeSet<ir::ActorId>) {
        for s in b {
            match &s.kind {
                ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => {
                    out.insert(m.capabilities[c.capability.0].actor);
                }
                ir::Stmt::Fork { branches, .. } => {
                    out.extend(
                        branches
                            .iter()
                            .map(|c| m.capabilities[c.capability.0].actor),
                    );
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    calls(then_body, m, out);
                    calls(else_body, m, out);
                }
                _ => {}
            }
        }
    }
    calls(&m.process.body, m, &mut called);
    m.actors.iter().enumerate().map(|(i,a)| {
        let meta = &a.metadata;
        let names = |ids: &Option<Vec<ir::ActorId>>| ids.as_ref().map(|ids| ids.iter().map(|id| m.actors[id.0].name.clone()).collect::<Vec<_>>());
        let caps: Vec<_> = m.capabilities.iter().filter(|c| c.actor.0 == i).map(|c| c.name.clone()).collect();
        let latent: Vec<_> = m.state_variables.iter().filter(|v| v.kind == ir::StateKind::ActorLatent(ir::ActorId(i))).map(|v| format!("{}.{}",a.name,v.name)).collect();
        let state: Vec<_> = m.state_variables.iter().filter(|v| v.kind == ir::StateKind::ActorObserved(ir::ActorId(i))).map(|v| format!("{}.{}",a.name,v.name)).collect();
        let executors = match &a.executors {
            Some(ids) => json!({"kind":"named", "ids":ids.iter().map(|id| m.people[id.0].name.clone()).collect::<Vec<_>>()}),
            None => a.capacity.map_or(Value::Null, |n| json!({"kind":"anonymous","count":n})),
        };
        Actor {
            rule: rule(m, ir::ActorId(i)),
            name: a.name.clone(),
            boundary: field(json!(meta.boundary.as_ref().unwrap_or(&a.name)), meta.boundary.is_some()),
            grain: field(json!(meta.grain), meta.grain.is_some()),
            contains: field(json!(names(&meta.contains)), meta.contains.is_some()),
            overlaps: field(json!(names(&meta.overlaps)), meta.overlaps.is_some()),
            capability: field(if caps.is_empty() { Value::Null } else { json!(caps) }, !caps.is_empty()),
            capacity: field(json!(a.capacity), meta.capacity_declared),
            executors: field(executors, a.executors.is_some()),
            control: field(json!(meta.control.as_deref().unwrap_or(if called.contains(&ir::ActorId(i)) { "policy" } else { "environment" })), meta.control.is_some()),
            source: field(a.source.map_or(Value::Null, |s| json!(match s { ir::SourceKind::Document => "document", ir::SourceKind::Record => "record", ir::SourceKind::Tacit => "tacit" })), a.source.is_some()),
            via_model: field(json!(meta.via_model), meta.via_model.is_some()),
            state: field(if state.is_empty() { Value::Null } else { json!(state) }, !state.is_empty()),
            latent: field(if latent.is_empty() { Value::Null } else { json!(latent) }, !latent.is_empty()),
        }
    }).collect()
}

/// Validate public IR before projecting/indexing it. Iterative graph walks avoid
/// stack growth from long chains and never allocate a transitive-closure matrix.
pub(crate) struct ValidationError {
    pub code: &'static str,
    pub message: &'static str,
    /// Owner of the offending contains clause. Lower maps this witness back to
    /// the original AST; execution-only IR builders need no source metadata.
    pub contains_actor: Option<ir::ActorId>,
}
impl From<&'static str> for ValidationError {
    fn from(message: &'static str) -> Self {
        Self {
            code: crate::diag::E_CON_0,
            message,
            contains_actor: None,
        }
    }
}

pub(crate) fn validate(m: &ir::Model) -> Result<(), ValidationError> {
    let n = m.actors.len();
    for (i, a) in m.actors.iter().enumerate() {
        let d = &a.metadata;
        if d.capacity_declared && a.capacity.is_none()
            || d.control
                .as_deref()
                .is_some_and(|s| !matches!(s, "policy" | "environment" | "party"))
            || a.executors
                .as_ref()
                .is_some_and(|ids| ids.iter().any(|id| id.0 >= m.people.len()))
            || d.via_model.as_ref().is_some_and(|s| {
                let parts: Vec<_> = s.split('@').collect();
                parts.len() != 2
                    || parts.iter().any(|p| p.is_empty())
                    || s.chars().any(|c| c.is_whitespace() || c.is_control())
            })
        {
            return Err("invalid Actor declaration metadata".into());
        }
        for ids in [&d.contains, &d.overlaps].into_iter().flatten() {
            let mut seen = BTreeSet::new();
            if ids
                .iter()
                .any(|id| id.0 >= n || id.0 == i || !seen.insert(*id))
            {
                return Err("invalid Actor relation index, self-reference or duplicate".into());
            }
        }
    }
    if m.capabilities.iter().any(|c| c.actor.0 >= n) {
        return Err("invalid Capability Actor index".into());
    }
    // Reject cycles before classifying a double-counted positive resource.
    // An iterative DFS identifies a real back edge, not just a node left over
    // after topological elimination (which could be downstream of a cycle).
    let mut color = vec![0u8; n];
    for root in 0..n {
        if color[root] != 0 {
            continue;
        }
        color[root] = 1;
        let mut stack = vec![(ir::ActorId(root), 0usize)];
        while let Some((actor, next)) = stack.last_mut() {
            let children = m.actors[actor.0]
                .metadata
                .contains
                .as_deref()
                .unwrap_or(&[]);
            if *next == children.len() {
                color[actor.0] = 2;
                stack.pop();
                continue;
            }
            let child = children[*next];
            *next += 1;
            if color[child.0] == 1 {
                return Err(ValidationError {
                    contains_actor: Some(*actor),
                    .."cyclic contains relation".into()
                });
            }
            if color[child.0] == 0 {
                color[child.0] = 1;
                stack.push((child, 0));
            }
        }
    }
    let positive: Vec<_> = (0..n)
        .map(|i| {
            m.capabilities.iter().any(|c| {
                c.actor.0 == i
                    && c.duration_distribution
                        .as_ref()
                        .map_or(c.duration > 0, |d| {
                            d.entries.iter().any(|(t, p)| *t > 0 && !p.is_zero())
                        })
            })
        })
        .collect();
    for root in 0..n {
        let mut seen = BTreeSet::new();
        let mut pending = vec![ir::ActorId(root)];
        while let Some(id) = pending.pop() {
            if let Some(children) = &m.actors[id.0].metadata.contains {
                for child in children {
                    if seen.insert(*child) {
                        if positive[root] && positive[child.0] {
                            return Err(ValidationError {
                                code: crate::diag::E_CAP_0,
                                message: "capacity counted twice by composite Actor and descendant",
                                contains_actor: Some(ir::ActorId(root)),
                            });
                        }
                        pending.push(*child);
                    }
                }
            }
        }
    }
    for c in &m.contracts {
        if let ir::ContractKind::NamedResponse {
            debtor: Some(d), ..
        } = c.kind
            && match d {
                ir::Debtor::Person(id) => id.0 >= m.people.len(),
                ir::Debtor::Actor(id) => id.0 >= n,
            }
        {
            return Err("invalid debtor index".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(src: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap()
    }
    #[test]
    fn malformed_public_metadata_fails_closed_before_indexing_in_both_backends() {
        let base = model(include_str!(
            "../../../fixtures/act-debtor-value/model.relca"
        ));
        for fault in 0..8 {
            let mut m = base.clone();
            match fault {
                0 => m.actors[0].metadata.contains = Some(vec![ir::ActorId(usize::MAX)]),
                1 => m.actors[0].metadata.overlaps = Some(vec![ir::ActorId(0)]),
                2 => {
                    m.actors[0].capacity = None;
                    m.actors[0].metadata.capacity_declared = true;
                }
                3 => m.actors[0].metadata.control = Some("arbitrary".into()),
                4 => m.actors[0].metadata.via_model = Some("name@@version".into()),
                5 => {
                    let ir::ContractKind::NamedResponse { debtor, .. } = &mut m.contracts[0].kind
                    else {
                        panic!()
                    };
                    *debtor = Some(ir::Debtor::Person(ir::PersonId(usize::MAX)));
                }
                6 => {
                    let ir::ContractKind::NamedResponse { debtor, .. } = &mut m.contracts[0].kind
                    else {
                        panic!()
                    };
                    *debtor = Some(ir::Debtor::Actor(ir::ActorId(usize::MAX)));
                }
                _ => {
                    m.actors.push(m.actors[0].clone());
                    m.actors[0].metadata.contains = Some(vec![ir::ActorId(1)]);
                    m.actors[1].metadata.contains = Some(vec![ir::ActorId(0)]);
                }
            }
            assert!(validate(&m).is_err());
            assert!(crate::agency::derive(&m).is_err());
            let e = crate::engine::explore(&m);
            let r = crate::reference::explore(&m);
            assert_eq!(
                r.stop,
                crate::reference::StopReason::SemanticAbort,
                "fault {fault}"
            );
            assert!(
                e.properties
                    .iter()
                    .all(|p| p.verdict == crate::Verdict::Unsupported),
                "fault {fault}"
            );
            assert!(e.diagnostics.iter().any(|d| d.code == crate::diag::U_SEM_0));
        }
    }
    #[test]
    fn reciprocal_overlap_has_one_effective_pair_but_only_explicit_rows_are_declared() {
        let src = include_str!("../../../fixtures/act-overlap/model.relca");
        let one = model(src);
        let two = model(&src.replace("actor Member {", "actor Member { overlaps [Team,Team];"));
        assert_eq!(one.overlap_pairs(), two.overlap_pairs());
        assert_eq!(one.overlap_pairs().len(), 1);
        assert_eq!(project(&one)[1].overlaps.status, Status::Default);
        assert_eq!(project(&two)[1].overlaps.status, Status::Declared);
    }
    #[test]
    #[should_panic(expected = "unclassified Actor fixture")]
    fn unknown_fixture_is_not_silently_excluded() {
        fixture("act-unknown");
    }
}
