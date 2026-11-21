//! Static prefix facts on the expanded repeat tree. Only lower's two private
//! guards are interpreted here. Every user predicate still has both paths.
use super::{Site, ir};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Clone)]
struct Prefix {
    must: BTreeSet<ir::BindingId>,
    may: BTreeSet<ir::BindingId>,
}

impl Prefix {
    fn merge(&mut self, other: &Self) {
        self.must.retain(|id| other.must.contains(id));
        self.may.extend(&other.may);
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Control {
    active: bool,
    continuing: bool,
    current: Option<ir::EvidenceExit>,
    exits: Vec<ir::EvidenceExit>,
}

const ENTRY: Control = Control {
    active: true,
    continuing: true,
    current: None,
    exits: Vec::new(),
};
type Paths = BTreeMap<Control, Prefix>;

fn add(paths: &mut Paths, tag: Control, facts: Prefix) {
    paths
        .entry(tag)
        .and_modify(|old| old.merge(&facts))
        .or_insert(facts);
}

fn common(paths: &Paths) -> Prefix {
    let mut all = paths.values();
    let Some(mut facts) = all.next().cloned() else {
        return Prefix::default();
    };
    for one in all {
        facts.merge(one);
    }
    facts
}

struct Walker<'a> {
    model: &'a ir::Model,
    facts: BTreeMap<ir::BindingId, Prefix>,
    scoped: Option<Scoped>,
    overflow: bool,
}

type Contexts = BTreeMap<Vec<ir::EvidenceExit>, Prefix>;
type Scoped = BTreeMap<ir::BindingId, Contexts>;
const MAX_CONTEXTS: usize = 1024;

// The active guard tests only `== 1`. For a loop with several written
// leaves, use inactive values 2.. for their lexical ordinals. Round clones
// share a source span. Do not alter the control of unrelated models.
pub(super) fn mark_leaves(m: &mut ir::Model) {
    fn walk(
        body: &mut ir::Block,
        vars: &[ir::StateVariable],
        visit: &mut impl FnMut(ir::StateVarId, usize, &mut ir::StateValue),
    ) {
        use ir::{StateValue::Integer, Stmt};
        for stmt in body {
            match &mut stmt.kind {
                Stmt::LoopControl(c) => {
                    for (id, value) in &mut c.updates {
                        if vars[id.0].loop_role() == Some("active") && *value != Integer(1) {
                            visit(*id, stmt.span.start, value);
                        }
                    }
                }
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    walk(then_body, vars, visit);
                    walk(else_body, vars, visit);
                }
                _ => {}
            }
        }
    }
    let mut leaves = BTreeMap::<_, BTreeSet<_>>::new();
    walk(
        &mut m.process.body,
        &m.state_variables,
        &mut |id, pos, _| {
            leaves.entry(id).or_default().insert(pos);
        },
    );
    leaves.retain(|_, positions| positions.len() > 1);
    for (id, positions) in &leaves {
        m.state_variables[id.0].integer_max = Some(positions.len() as u64 + 1);
    }
    walk(
        &mut m.process.body,
        &m.state_variables,
        &mut |id, pos, value| {
            if let Some(positions) = leaves.get(&id) {
                *value = ir::StateValue::Integer(positions.range(..pos).count() as u64 + 2);
            }
        },
    );
}

impl Walker<'_> {
    fn role(&self, v: ir::StateVarId) -> Option<&str> {
        self.model.state_variables.get(v.0)?.loop_role()
    }

    fn call(&mut self, binding: ir::BindingId, paths: &mut Paths, sequential: bool) {
        self.record(binding, paths);
        for facts in paths.values_mut() {
            facts.may.insert(binding);
            if sequential {
                facts.must.insert(binding);
            }
        }
    }

    fn record(&mut self, binding: ir::BindingId, paths: &Paths) {
        if let Some(scoped) = &mut self.scoped {
            let contexts = scoped.entry(binding).or_default();
            for (tag, facts) in paths {
                contexts
                    .entry(tag.exits.clone())
                    .and_modify(|old| old.merge(facts))
                    .or_insert_with(|| facts.clone());
            }
        } else {
            self.facts.insert(binding, common(paths));
        }
    }

    fn block(&mut self, block: &ir::Block, mut paths: Paths, ended: &mut Paths) -> Paths {
        for stmt in block {
            if self.overflow || (self.scoped.is_some() && paths.len() > MAX_CONTEXTS) {
                self.overflow = true;
                return Paths::new();
            }
            match &stmt.kind {
                ir::Stmt::Call(c) => self.call(c.binding, &mut paths, true),
                ir::Stmt::Response(c, r) => {
                    if r.attempt == 1 {
                        self.call(r.command, &mut paths, true);
                    }
                    self.call(c.binding, &mut paths, r.attempt == 1);
                }
                ir::Stmt::Fork { branches, .. } => {
                    // Siblings are unordered. As in the legacy prefix rule,
                    // their synchronized writes are candidates, not a chosen
                    // last writer. Registration's conflict gate stays separate.
                    for c in branches {
                        let mut before = paths.clone();
                        for facts in before.values_mut() {
                            facts.may.extend(
                                branches
                                    .iter()
                                    .map(|b| b.binding)
                                    .filter(|b| *b != c.binding),
                            );
                        }
                        self.record(c.binding, &before);
                    }
                    for prefix in paths.values_mut() {
                        prefix.may.extend(branches.iter().map(|b| b.binding));
                    }
                }
                ir::Stmt::LoopControl(control) => {
                    let mut next = Paths::new();
                    for (mut tag, facts) in paths {
                        if control.updates.is_empty() {
                            tag = Control {
                                exits: tag.exits,
                                ..ENTRY
                            };
                        }
                        for (id, value) in &control.updates {
                            let on = *value == ir::StateValue::Integer(1);
                            match self.role(*id) {
                                Some("active") => {
                                    tag.active = on;
                                    if !on && let Some(mut exit) = tag.current.take() {
                                        if let ir::StateValue::Integer(n) = *value {
                                            exit.leave_site = n.checked_sub(1).filter(|n| *n != 0);
                                        }
                                        tag.exits.push(exit);
                                    }
                                }
                                Some("continue") => tag.continuing = on,
                                Some("round") if self.scoped.is_some() => {
                                    if let ir::StateValue::Integer(round) = *value {
                                        tag.current = Some(ir::EvidenceExit {
                                            variable: *id,
                                            round,
                                            on_limit: false,
                                            leave_site: None,
                                        });
                                    }
                                }
                                Some("limit") if on => {
                                    if let Some(exit) = &mut tag.current {
                                        exit.on_limit = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                        add(&mut next, tag, facts);
                    }
                    paths = next;
                }
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    let guard = match condition {
                        ir::Cond::IntCompare {
                            variable,
                            operator: ir::IntComparison::Eq,
                            value: 1,
                        } => self.role(*variable),
                        _ => None,
                    };
                    let at_limit = guard == Some("active") && then_body.first().is_some_and(|s| {
                        matches!(&s.kind, ir::Stmt::LoopControl(c) if c.updates.iter().any(|(id, _)| self.role(*id) == Some("limit")))
                    });
                    let mut t = Paths::new();
                    let mut e = Paths::new();
                    // An ordinary if does not make its writers dominate the
                    // following lexical block, even if the other arm ends.
                    let incoming = guard.is_none().then(|| paths.clone());
                    for (tag, facts) in paths {
                        let take = match guard {
                            Some("active") => Some(tag.active),
                            Some("continue") => Some(tag.continuing),
                            _ => None,
                        };
                        if take != Some(false) {
                            add(&mut t, tag.clone(), facts.clone());
                        }
                        if take != Some(true) {
                            add(&mut e, tag, facts);
                        }
                    }
                    let ordinary = guard.is_none();
                    let mut arm_ends = Paths::new();
                    paths = self.block(then_body, t, &mut arm_ends);
                    for (tag, facts) in self.block(else_body, e, &mut arm_ends) {
                        add(&mut paths, tag, facts);
                    }
                    for (tag, facts) in arm_ends {
                        // A terminated ordinary arm contributes lexical may
                        // only to the same continuation, never to a leave or
                        // again selected in a different arm.
                        if ordinary && let Some(live) = paths.get_mut(&tag) {
                            live.may.extend(&facts.may);
                        }
                        add(ended, tag, facts);
                    }
                    if let Some(incoming) = incoming {
                        for (tag, facts) in &mut paths {
                            if let Some(before) = incoming.get(tag) {
                                facts.must.retain(|binding| before.must.contains(binding));
                            }
                        }
                    }
                    if at_limit && !paths.is_empty() {
                        let mut reset = Paths::new();
                        for (tag, facts) in paths {
                            add(
                                &mut reset,
                                Control {
                                    exits: tag.exits,
                                    ..ENTRY
                                },
                                facts,
                            );
                        }
                        paths = reset;
                    }
                }
                ir::Stmt::End(_) | ir::Stmt::StopReceiptUnknown => {
                    for (tag, facts) in std::mem::take(&mut paths) {
                        add(ended, tag, facts);
                    }
                }
                _ => {}
            }
        }
        paths
    }
}

pub(super) fn collect(m: &ir::Model, sites: &mut [Site<'_>]) {
    let mut walker = Walker {
        model: m,
        facts: BTreeMap::new(),
        scoped: None,
        overflow: false,
    };
    walker.block(
        &m.process.body,
        Paths::from([(ENTRY, Prefix::default())]),
        &mut Paths::new(),
    );
    for s in sites {
        let p = walker.facts.remove(&s.call.binding).unwrap_or_default();
        s.repeated_prefix = p.must;
        s.possible_prefix = Some(p.may);
    }
}

/// One static profile per completed exit tuple, not per ordinary branch. The
/// empty profile covers producers evaluated before the first selected exit.
pub(super) struct Profile<'a> {
    pub key: Vec<ir::EvidenceExit>,
    pub sites: Vec<Site<'a>>,
    pub local: BTreeSet<ir::BindingId>,
}

pub(super) fn profiles<'a>(
    m: &ir::Model,
    sites: &'a [Site<'a>],
) -> Option<impl Iterator<Item = Profile<'a>>> {
    let mut walker = Walker {
        model: m,
        facts: BTreeMap::new(),
        scoped: Some(Scoped::new()),
        overflow: false,
    };
    walker.block(
        &m.process.body,
        Paths::from([(ENTRY, Prefix::default())]),
        &mut Paths::new(),
    );
    let scopes = walker.scoped.unwrap();
    let keys: BTreeSet<_> = scopes.values().flat_map(|p| p.keys().cloned()).collect();
    if walker.overflow || keys.len() > MAX_CONTEXTS {
        return None;
    }
    Some(keys.into_iter().map(move |key| {
        let mut local = BTreeSet::new();
        let calls = sites
            .iter()
            .filter_map(|s| {
                let contexts = scopes.get(&s.call.binding)?;
                if contexts.contains_key(&key) {
                    local.insert(s.call.binding);
                }
                let facts = (0..=key.len())
                    .rev()
                    .find_map(|i| contexts.get(&key[..i]))?;
                Some(Site {
                    repeated_prefix: facts.must.clone(),
                    possible_prefix: Some(facts.may.clone()),
                    ..s.clone()
                })
            })
            .collect();
        Profile {
            key,
            sites: calls,
            local,
        }
    }))
}
