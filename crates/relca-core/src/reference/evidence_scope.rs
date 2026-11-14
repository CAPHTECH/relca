//! Independent declaration-scope check for the stage-one snapshot limitation.
//! Actual value roots are still reconstructed from observations in evidence.rs.
use super::*;

struct Site<'a> {
    call: &'a Call,
    before: BTreeSet<BindingId>,
    physical: bool,
    mandatory: bool,
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
pub(super) fn constraints(m: &Model) -> BTreeMap<BindingId, BTreeSet<ActorId>> {
    let mut all = Vec::new();
    sites(&m.process.body, &BTreeSet::new(), &mut all);
    let mut normal: BTreeMap<BindingId, (BTreeSet<ActorId>, BTreeSet<ActorId>)> = BTreeMap::new();
    let mut values = normal.clone();
    let mut masks = BTreeMap::new();
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
        for &v in &c.read_variables {
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
                if disjoint(&m.process.body, w.call.span, s.call.span) {
                    continue;
                }
                let wc = &m.capabilities[w.call.capability.0];
                if !writes(wc, v) {
                    continue;
                }
                last.push(w.call.binding);
                if w.mandatory
                    && wc.requires.is_none()
                    && s.before.contains(&w.call.binding)
                    && certain_write(wc, v)
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
            if !closed || last.len() != 1 {
                mask.extend(union.difference(&intersection).copied());
            }
            sure.extend(intersection);
            possible.extend(union);
        }
        mask.retain(|r| !sure.contains(r));
        masks.insert(s.call.binding, mask);
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
    masks
}
