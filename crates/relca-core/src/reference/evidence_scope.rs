//! Independent declaration-scope check for the stage-one snapshot limitation.
//! Actual value roots are still reconstructed from observations in evidence.rs.
use super::*;

struct Site<'a> {
    call: &'a Call,
    before: BTreeSet<BindingId>,
    physical: bool,
    mandatory: bool,
}

// Independently follow the expanded control tree, merging paths with the same
// pair of private flags. Actual roots are still read from the replay trace.
fn loop_prefixes(m: &Model) -> BTreeMap<BindingId, (BTreeSet<BindingId>, BTreeSet<BindingId>)> {
    type Seen = (BTreeSet<BindingId>, BTreeSet<BindingId>);
    type Paths = [Option<Seen>; 4];
    fn add(into: &mut Option<Seen>, from: Seen) {
        if let Some((sure, may)) = into {
            *sure = sure.intersection(&from.0).copied().collect();
            may.extend(from.1);
        } else {
            *into = Some(from);
        }
    }
    fn joined(p: &Paths) -> Seen {
        let mut combined = None;
        for state in p.iter().flatten() {
            add(&mut combined, state.clone());
        }
        combined.unwrap_or_default()
    }
    fn flag(m: &Model, id: StateVarId) -> u8 {
        match m.state_variables.get(id.0).and_then(|v| v.loop_role()) {
            Some("active") => 1,
            Some("continue") => 2,
            _ => 0,
        }
    }
    fn walk(
        m: &Model,
        b: &Block,
        mut paths: Paths,
        out: &mut BTreeMap<BindingId, Seen>,
        ended: &mut Paths,
    ) -> Paths {
        for s in b {
            match &s.kind {
                Stmt::Call(c) | Stmt::Response(c, _) => {
                    let normal = !matches!(&s.kind, Stmt::Response(_, r) if r.attempt != 1);
                    if let Stmt::Response(_, r) = &s.kind
                        && r.attempt == 1
                    {
                        out.insert(r.command, joined(&paths));
                        for (sure, may) in paths.iter_mut().flatten() {
                            sure.insert(r.command);
                            may.insert(r.command);
                        }
                    }
                    out.insert(c.binding, joined(&paths));
                    for (sure, may) in paths.iter_mut().flatten() {
                        if normal {
                            sure.insert(c.binding);
                        }
                        may.insert(c.binding);
                    }
                }
                Stmt::Fork { branches, .. } => {
                    let common = joined(&paths);
                    let ids: BTreeSet<_> = branches.iter().map(|c| c.binding).collect();
                    for c in branches {
                        let (sure, mut may) = common.clone();
                        if paths.iter().any(Option::is_some) {
                            may.extend(ids.iter().copied().filter(|b| *b != c.binding));
                        }
                        out.insert(c.binding, (sure, may));
                    }
                    for (_, may) in paths.iter_mut().flatten() {
                        may.extend(&ids);
                    }
                }
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    let mask = match condition {
                        Cond::IntCompare {
                            variable,
                            operator: IntComparison::Eq,
                            value: 1,
                        } => flag(m, *variable) as usize,
                        _ => 0,
                    };
                    let t = std::array::from_fn(|i| {
                        (mask == 0 || i & mask != 0)
                            .then(|| paths[i].clone())
                            .flatten()
                    });
                    let e = std::array::from_fn(|i| {
                        (i & mask == 0).then(|| paths[i].clone()).flatten()
                    });
                    let incoming = (mask == 0).then_some(paths);
                    let mut arms = Paths::default();
                    paths = walk(m, then_body, t, out, &mut arms);
                    for (i, facts) in walk(m, else_body, e, out, &mut arms)
                        .into_iter()
                        .enumerate()
                    {
                        if let Some(facts) = facts {
                            add(&mut paths[i], facts);
                        }
                    }
                    for (i, facts) in arms.into_iter().enumerate() {
                        if let Some(facts) = facts {
                            if mask == 0
                                && let Some((_, may)) = &mut paths[i]
                            {
                                may.extend(&facts.1);
                            }
                            add(&mut ended[i], facts);
                        }
                    }
                    if let Some(incoming) = incoming {
                        for (after, before) in paths.iter_mut().zip(incoming) {
                            if let (Some((sure, _)), Some((original, _))) = (after, before) {
                                sure.retain(|b| original.contains(b));
                            }
                        }
                    }
                }
                Stmt::LoopControl(c) => {
                    let mut next: Paths = Default::default();
                    for (i, facts) in paths.into_iter().enumerate() {
                        if let Some(facts) = facts {
                            let mut j = if c.updates.is_empty() { 3 } else { i };
                            for (id, value) in &c.updates {
                                let bit = flag(m, *id) as usize;
                                j = if *value == StateValue::Integer(1) {
                                    j | bit
                                } else {
                                    j & !bit
                                };
                            }
                            add(&mut next[j], facts);
                        }
                    }
                    paths = next;
                }
                Stmt::End(_) | Stmt::StopReceiptUnknown => {
                    for (i, facts) in std::mem::take(&mut paths).into_iter().enumerate() {
                        if let Some(facts) = facts {
                            add(&mut ended[i], facts);
                        }
                    }
                }
                _ => {}
            }
        }
        paths
    }
    let mut out = BTreeMap::new();
    walk(
        m,
        &m.process.body,
        [None, None, None, Some(Seen::default())],
        &mut out,
        &mut Paths::default(),
    );
    out
}
fn sites<'a>(body: &'a Block, before: &BTreeSet<BindingId>, out: &mut Vec<Site<'a>>) {
    let mut before = before.clone();
    for s in body {
        match &s.kind {
            Stmt::Call(c) => {
                out.push(Site {
                    call: c,
                    before: before.clone(),
                    physical: true,
                    mandatory: true,
                });
                before.insert(c.binding);
            }
            Stmt::Response(c, r) => {
                if r.attempt == 1 {
                    out.push(Site {
                        call: &r.command_call,
                        before: before.clone(),
                        physical: false,
                        mandatory: false,
                    });
                    before.insert(r.command);
                }
                out.push(Site {
                    call: c,
                    before: before.clone(),
                    physical: true,
                    mandatory: r.attempt == 1,
                });
                before.insert(c.binding);
            }
            Stmt::Fork { branches, .. } => {
                for c in branches {
                    out.push(Site {
                        call: c,
                        before: before.clone(),
                        physical: true,
                        mandatory: true,
                    });
                }
            }
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                sites(then_body, &before, out);
                sites(else_body, &before, out);
            }
            _ => {}
        }
    }
}
fn writes(c: &Capability, v: StateVarId) -> bool {
    c.start_writes
        .iter()
        .chain(c.finish_writes())
        .any(|w| w.variable == v)
        || c.kernel_writes.contains(&v)
        || c.result_kernels().any(|k| {
            k.rows.iter().any(|r| {
                r.branches
                    .iter()
                    .any(|b| b.overrides.iter().any(|(x, _)| *x == v))
            })
        })
}
fn certain_write(c: &Capability, v: StateVarId) -> bool {
    c.start_writes.iter().any(|w| w.variable == v)
        || (c.atomic.is_none()
            && c.version_check.is_none()
            && (c.writes.iter().any(|w| w.variable == v)
                || (c.result_kernels().next().is_some()
                    && c.result_kernels().all(|k| {
                        k.rows.iter().all(|r| {
                            r.branches
                                .iter()
                                .all(|b| b.overrides.iter().any(|(x, _)| *x == v))
                        })
                    }))))
}
#[derive(Clone, Copy)]
enum Read {
    Variable(StateVarId),
    Record(ActorId),
}
impl Read {
    fn writes(self, c: &Capability) -> bool {
        match self {
            Self::Variable(v) => writes(c, v),
            Self::Record(r) => c.registers.contains(&r),
        }
    }
    fn certain(self, c: &Capability) -> bool {
        match self {
            Self::Variable(v) => certain_write(c, v),
            Self::Record(r) => c.registers.contains(&r),
        }
    }
}
#[derive(Default)]
pub(super) struct Constraints {
    pub masks: BTreeMap<BindingId, BTreeSet<ActorId>>,
    pub registrar_must: BTreeMap<BindingId, BTreeSet<ActorId>>,
}

pub(super) fn constraints(m: &Model) -> Constraints {
    let mut all = Vec::new();
    sites(&m.process.body, &BTreeSet::new(), &mut all);
    let loop_prefixes = m.has_loops().then(|| loop_prefixes(m));
    let mut normal: BTreeMap<BindingId, (BTreeSet<ActorId>, BTreeSet<ActorId>)> = BTreeMap::new();
    let mut values = normal.clone();
    let mut constraints = Constraints::default();
    for (index, s) in all.iter().enumerate() {
        let c = &m.capabilities[s.call.capability.0];
        let mut sure: BTreeSet<_> = c.reads.iter().copied().collect();
        let mut possible = sure.clone();
        for a in &s.call.args {
            if let Some((yes, maybe)) = values.get(a) {
                sure.extend(yes);
                possible.extend(maybe);
            }
        }
        let mut mask = BTreeSet::new();
        for input in c
            .read_variables
            .iter()
            .copied()
            .map(Read::Variable)
            .chain(c.read_records.iter().copied().map(Read::Record))
        {
            let mut last = Vec::new();
            let mut closed = false;
            for w in all[..index].iter().rev().filter(|w| w.physical) {
                fn disjoint(body: &Block, a: diag::Span, b: diag::Span) -> bool {
                    fn inside(block: &Block, p: diag::Span) -> bool {
                        block
                            .iter()
                            .any(|s| s.span.start <= p.start && s.span.end >= p.end)
                    }
                    body.iter().any(|s| match &s.kind {
                        Stmt::If {
                            then_body,
                            else_body,
                            ..
                        } => {
                            (inside(then_body, a) && inside(else_body, b))
                                || (inside(else_body, a) && inside(then_body, b))
                                || disjoint(then_body, a, b)
                                || disjoint(else_body, a, b)
                        }
                        _ => false,
                    })
                }
                let prefix = loop_prefixes
                    .as_ref()
                    .and_then(|p| p.get(&s.call.binding))
                    .filter(|_| m.loop_site(w.call.binding).is_some());
                if prefix.map_or_else(
                    || disjoint(&m.process.body, w.call.span, s.call.span),
                    |(_, may)| !may.contains(&w.call.binding),
                ) {
                    continue;
                }
                let wc = &m.capabilities[w.call.capability.0];
                if !input.writes(wc) {
                    continue;
                }
                last.push(w.call.binding);
                if w.mandatory
                    && wc.requires.is_none()
                    && prefix
                        .map_or(&s.before, |(sure, _)| sure)
                        .contains(&w.call.binding)
                    && input.certain(wc)
                {
                    closed = true;
                    break;
                }
            }
            let mut intersection = if closed {
                last.first()
                    .and_then(|b| normal.get(b))
                    .map(|p| p.0.clone())
                    .unwrap_or_default()
            } else {
                BTreeSet::new()
            };
            let mut union = BTreeSet::new();
            for b in &last {
                if let Some((yes, maybe)) = normal.get(b) {
                    intersection = intersection.intersection(yes).copied().collect();
                    union.extend(maybe);
                }
            }
            // A certain registration is not proof that all of its possible
            // inputs were certainly available to the registrar.
            if !closed || last.len() != 1 || matches!(input, Read::Record(_)) {
                mask.extend(union.difference(&intersection).copied());
            }
            sure.extend(intersection);
            possible.extend(union);
        }
        mask.retain(|r| !sure.contains(r));
        constraints.masks.insert(s.call.binding, mask);
        if !c.registers.is_empty() {
            constraints
                .registrar_must
                .insert(s.call.binding, sure.clone());
        }
        normal.insert(s.call.binding, (sure.clone(), possible.clone()));
        values.insert(
            s.call.binding,
            (
                if c.requires.is_some() {
                    BTreeSet::new()
                } else {
                    sure
                },
                possible,
            ),
        );
    }
    constraints
}
