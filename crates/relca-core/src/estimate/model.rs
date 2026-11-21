//! Slot inventory and a conservative check of which call sites identify rounds.
use super::wire::Slot;
use crate::{
    ast::{self, ActorItem, CapItem, Item, Stmt},
    diag, ir,
    probability::Rational,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
pub(super) enum Site<'a> {
    Ordinary,
    Repeat { name: &'a str, limit: usize },
    Unusable(&'static str),
}
#[derive(Clone, Copy)]
pub(super) enum Condition {
    Unconditional,
    Round(usize),
    Unsupported(&'static str),
}
pub(super) enum Kind<'a> {
    Duration(&'a ir::Capability),
    Kernel {
        branches: Vec<(&'a str, &'a Rational)>,
        condition: Condition,
    },
    Unsupported(&'static str),
}
pub(super) struct Definition<'a> {
    pub(super) slot: Slot,
    pub(super) site: Site<'a>,
    pub(super) cap: Option<usize>,
    pub(super) kind: Kind<'a>,
}
impl Definition<'_> {
    pub(super) fn planned_branches(&self, width: u64) -> u128 {
        let copies = match self.site {
            Site::Repeat { limit, .. } => limit as u128,
            _ => 1,
        };
        match &self.kind {
            Kind::Duration(_) => u128::from(width) * copies,
            Kind::Kernel {
                branches,
                condition,
            } => {
                branches.len() as u128
                    * if matches!(condition, Condition::Unconditional) {
                        copies
                    } else {
                        1
                    }
            }
            Kind::Unsupported(_) => 0,
        }
    }
}

type Qualified<'a> = (&'a str, &'a str);
type Sites<'a> = BTreeMap<Qualified<'a>, Vec<Site<'a>>>;
type Extra = Vec<(&'static str, diag::Span)>;

#[derive(Clone, Copy)]
struct Placement<'a> {
    name: &'a str,
    limit: usize,
    direct: bool,
}

fn names(call: &ast::Call) -> Qualified<'_> {
    (&call.actor.value, &call.capability.value)
}
fn call_mechanism(call: &ast::Call) -> Option<&'static str> {
    if call.query_command.is_some() {
        Some("query")
    } else if call.recovery.is_some() {
        Some("recovery")
    } else if call.retry.is_some() {
        Some("retry")
    } else {
        call.response_loss.as_ref().map(|_| "response_loss")
    }
}
struct Inventory<'a> {
    sites: Sites<'a>,
    extras: Extra,
    startable: BTreeSet<Qualified<'a>>,
    serial_process: bool,
}
impl<'a> Inventory<'a> {
    fn call(&mut self, call: &'a ast::Call, placement: Option<Placement<'a>>, prefix: bool) {
        let site = match placement {
            Some(p)
                if p.direct
                    && prefix
                    && self.serial_process
                    && call_mechanism(call).is_none()
                    && self.startable.contains(&names(call)) =>
            {
                Site::Repeat {
                    name: p.name,
                    limit: p.limit,
                }
            }
            Some(_) => Site::Unusable("round_not_identifiable"),
            None => call_mechanism(call).map_or(Site::Ordinary, Site::Unusable),
        };
        self.sites.entry(names(call)).or_default().push(site);
        if let Some(r) = &call.response_loss {
            self.extras.push(("response_loss", r.span));
        }
        if let Some(r) = &call.retry {
            self.extras.push(("backoff", r.backoff.span));
        }
        if let Some(body) = &call.recovery {
            self.block(
                body,
                placement.map(|p| Placement { direct: false, ..p }),
                false,
            );
        }
    }

    fn block(&mut self, body: &'a ast::Block, placement: Option<Placement<'a>>, top: bool) {
        let mut prefix = true;
        let nested = placement.map(|p| Placement { direct: false, ..p });
        for s in body {
            match &s.value {
                Stmt::Call(c) => self.call(c, placement, prefix),
                Stmt::Fork { branches, .. } => {
                    for c in branches {
                        self.call(c, nested, false);
                    }
                }
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                }
                | Stmt::Merge {
                    then_body,
                    else_body,
                    ..
                } => {
                    self.block(then_body, nested, false);
                    self.block(else_body, nested, false);
                }
                Stmt::Repeat {
                    name,
                    at_most,
                    gap,
                    body,
                    on_limit,
                } => {
                    let p = Placement {
                        name: &name.value,
                        limit: at_most.value as usize,
                        direct: top,
                    };
                    self.extras.push(("business_rule", at_most.span));
                    if let Some(g) = gap {
                        self.extras.push(("gap", g.span));
                    }
                    self.block(body, Some(p), false);
                    self.block(on_limit, Some(Placement { direct: false, ..p }), false);
                }
                Stmt::Wait(_) | Stmt::WaitUntil(_) => self.extras.push(("wait", s.span)),
                Stmt::Again
                | Stmt::Leave
                | Stmt::End(_)
                | Stmt::Yield(_)
                | Stmt::ResolveEffect { .. }
                | Stmt::StopReceiptUnknown => (),
            }
            prefix &= match &s.value {
                Stmt::Wait(_) => true,
                Stmt::Call(c) => call_mechanism(c).is_none() && self.startable.contains(&names(c)),
                _ => false,
            };
        }
    }
}

pub(super) fn slots<'a>(src: &str, ast: &'a ast::Model, m: &'a ir::Model) -> Vec<Definition<'a>> {
    let starts = diag::line_starts(src);
    let slot = |id: String, kind, target: String, span| Slot {
        id,
        kind,
        target,
        span,
        location: diag::Location::indexed(src, &starts, span),
    };
    let mut inventory = Inventory {
        sites: BTreeMap::new(),
        extras: Vec::new(),
        // Serial within each case: competition can delay a Task without
        // letting that case advance past it into another iteration.
        serial_process: m.process.forks.is_empty()
            && m.resource_time.is_none()
            && m.horizon.is_none()
            && m.process.degradations.is_empty()
            && m.contests().is_empty()
            && m.response_attempts().is_empty()
            && m.cut.as_ref().is_none_or(|c| c.defer.is_empty()),
        startable: m
            .capabilities
            .iter()
            .filter_map(|c| {
                let a = &m.actors[c.actor.0];
                (c.requires.is_none()
                    && c.uses.is_empty()
                    && c.qualified_executors.is_none()
                    && c.performance.is_none()
                    && c.receipt_query.is_none()
                    && a.capacity.is_some_and(|n| n > 0)
                    && a.executors.as_ref().is_some_and(|ps| {
                        !ps.is_empty()
                            && c.permission
                                .as_ref()
                                .is_none_or(|xs| ps.iter().any(|p| xs.contains(p)))
                    }))
                .then_some((a.name.as_str(), c.name.as_str()))
            })
            .collect(),
    };
    let mut actors = BTreeMap::new();
    let mut kernels = BTreeMap::new();
    for i in &ast.items {
        match i {
            Item::Process(p) if p.name.value == m.process.name => {
                inventory.block(&p.body, None, true)
            }
            Item::Kernel(k) => {
                kernels.insert(k.name.value.as_str(), k);
            }
            Item::Actor(a) => {
                for item in &a.items {
                    match item {
                        ActorItem::Capability(c) => {
                            actors.insert((a.name.value.as_str(), c.name.value.as_str()), c);
                        }
                        ActorItem::Capacity(c) => inventory.extras.push(("business_rule", c.span)),
                        ActorItem::Calendar(c) => inventory.extras.push(("business_rule", c.span)),
                        _ => (),
                    }
                }
            }
            Item::ArrivalsFixed(x) => inventory.extras.push(("arrival", x.span)),
            Item::ArrivalsEvery(x) => inventory.extras.push(("arrival", x.span)),
            Item::ArrivalsDistribution(x) => inventory.extras.push(("arrival", x.span)),
            Item::Performance(x) => inventory.extras.push(("performance", x.span)),
            Item::Queue(x) | Item::Concurrent(x) => {
                inventory.extras.push(("business_rule", x.span))
            }
            Item::Resource(x) => inventory.extras.push(("business_rule", x.span)),
            Item::Calendar(x) => inventory.extras.push(("business_rule", x.span)),
            Item::PersonCalendar { calendar, .. } => {
                inventory.extras.push(("business_rule", calendar.span))
            }
            Item::Contract(c) => {
                for cl in &c.clauses {
                    match &cl.value {
                        ast::ContractClause::Contest {
                            rates, respond_by, ..
                        } => {
                            if let Some(r) = rates {
                                inventory.extras.push(("contest_rates", r.span));
                            }
                            inventory.extras.push(("business_rule", respond_by.span));
                        }
                        ast::ContractClause::Response { duration, .. } => {
                            inventory.extras.push(("business_rule", duration.span))
                        }
                        ast::ContractClause::AtMostOnce { .. } => {
                            inventory.extras.push(("business_rule", cl.span))
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
    }
    let mut defs = Vec::new();
    for (i, cap) in m.capabilities.iter().enumerate() {
        let a = m.actors[cap.actor.0].name.as_str();
        let n = cap.name.as_str();
        if m.cut
            .as_ref()
            .is_some_and(|c| !c.include.contains(&cap.actor))
        {
            continue;
        }
        let site = if m.contests().iter().any(|c| c.route.0 == i) {
            Site::Unusable("ambiguous_call_site")
        } else {
            match inventory.sites.get(&(a, n)).map(Vec::as_slice) {
                None | Some([]) => Site::Unusable("no_call_site"),
                Some([s]) => *s,
                Some(_) => Site::Unusable("ambiguous_call_site"),
            }
        };
        let body = actors.get(&(a, n)).map_or(&[][..], |c| c.items.as_slice());
        let duration_span = body
            .iter()
            .find(|i| {
                matches!(
                    i.value,
                    CapItem::DurationFixed(_) | CapItem::DurationDistribution(_)
                )
            })
            .map_or(cap.span, |i| i.span);
        defs.push(Definition {
            slot: slot(
                format!("duration.{a}.{n}"),
                "duration",
                format!("{a}.{n}.duration"),
                duration_span,
            ),
            site,
            cap: Some(i),
            kind: Kind::Duration(cap),
        });
        if let Some(nondet) = body
            .iter()
            .find(|i| matches!(i.value, CapItem::ResultNondet(_)))
        {
            let id = format!("nondet.{a}.{n}");
            defs.push(Definition {
                slot: slot(id.clone(), "unsupported", id, nondet.span),
                site,
                cap: Some(i),
                kind: Kind::Unsupported("nondeterministic"),
            });
        }
        if let Some(k) = &cap.kernel {
            for (index, r) in k.rows.iter().enumerate() {
                let span = kernels
                    .get(k.name.as_str())
                    .and_then(|k| k.rows.get(index))
                    .map_or(cap.span, |r| r.span);
                let branches: Vec<_> = r
                    .branches
                    .iter()
                    .map(|b| {
                        (
                            m.types[cap.return_type.0].members[b.result.0].as_str(),
                            &b.probability,
                        )
                    })
                    .collect();
                let (condition, distinct) = (
                    condition(m, site, k, r),
                    branches
                        .iter()
                        .map(|(n, _)| n)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == branches.len(),
                );
                let condition = if !matches!(condition, Condition::Unsupported(_))
                    && (!distinct || m.types[cap.return_type.0].record.is_some())
                {
                    Condition::Unsupported("branches_not_distinguishable")
                } else {
                    condition
                };
                defs.push(Definition {
                    slot: slot(
                        format!("kernel.{a}.{n}.{}.{}", k.name, index + 1),
                        "kernel_row",
                        format!("kernel.{}", k.name),
                        span,
                    ),
                    site,
                    cap: Some(i),
                    kind: Kind::Kernel {
                        branches,
                        condition,
                    },
                });
            }
        }
    }
    for (i, v) in m.state_variables.iter().enumerate() {
        if matches!(v.initial, ir::InitialValue::Distribution(_)) {
            let name = m
                .declared_state_name(ir::StateVarId(i))
                .unwrap_or_else(|| v.name.clone());
            defs.push(Definition {
                slot: slot(format!("initial.{name}"), "unsupported", name, v.span),
                site: Site::Ordinary,
                cap: None,
                kind: Kind::Unsupported(if v.kind.is_latent() {
                    "latent_initial"
                } else {
                    "observed_initial"
                }),
            });
        }
    }
    inventory
        .extras
        .sort_unstable_by_key(|&(reason, s)| (s.start, s.end, reason));
    inventory.extras.dedup();
    let mut ord = BTreeMap::<_, usize>::new();
    for (reason, span) in inventory.extras {
        let ordinal = ord.entry(reason).or_default();
        *ordinal += 1;
        let id = format!("unsupported.{reason}.{ordinal}");
        defs.push(Definition {
            slot: slot(id.clone(), "unsupported", id, span),
            site: Site::Ordinary,
            cap: None,
            kind: Kind::Unsupported(reason),
        });
    }
    defs.sort_by(|a, b| a.slot.id.cmp(&b.slot.id));
    defs
}

fn condition(m: &ir::Model, site: Site<'_>, k: &ir::Kernel, row: &ir::KernelRow) -> Condition {
    if k.given.is_empty() {
        return Condition::Unconditional;
    }
    if k.given.iter().any(|&v| {
        m.declared_state_variable(v)
            .is_some_and(|v| v.kind.is_latent() && !v.is_loop_round())
    }) {
        return Condition::Unsupported("latent_condition");
    }
    if let ([var], [ir::StateValue::Integer(round)]) = (k.given.as_slice(), row.values.as_slice())
        && let Some(var) = m
            .declared_state_variable(*var)
            .filter(|v| v.is_loop_round())
    {
        return match site {
            Site::Repeat { name, limit }
                if *round <= limit as u64
                    && var.loop_round_declaration_name() == format!("{name}.round") =>
            {
                Condition::Round(*round as usize)
            }
            Site::Unusable(reason) => Condition::Unsupported(reason),
            _ => Condition::Unsupported("round_not_identifiable"),
        };
    }
    Condition::Unsupported("observed_condition")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revise3_inventory_names_the_call_mechanism() {
        // Inventory is driven by the unexpanded AST, including suffixes whose
        // combinations are checked independently during lowering.
        let cases = [
            ("for command pay", "query"),
            ("on receipt_unknown { stop receipt_unknown }", "recovery"),
            (
                "retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end done",
                "retry",
            ),
            (
                "response_loss none retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end done",
                "retry",
            ),
            ("response_loss nondet", "response_loss"),
        ];
        let actual: Vec<_> = cases
            .iter()
            .map(|(suffix, _)| {
                let source = format!(
                    "model M version \"0.2\" {{ process P {{ r = A.run() {suffix}; end done }} }}"
                );
                let ast = crate::syntax::parse(&source).unwrap();
                let Item::Process(p) = &ast.items[0] else {
                    panic!("process required");
                };
                let mut inventory = Inventory {
                    sites: BTreeMap::new(),
                    extras: Vec::new(),
                    startable: BTreeSet::new(),
                    serial_process: false,
                };
                inventory.block(&p.body, None, true);
                let [Site::Unusable(reason)] = inventory.sites[&("A", "run")].as_slice() else {
                    panic!("call not refused: {suffix}");
                };
                *reason
            })
            .collect();
        assert_eq!(actual, cases.map(|(_, reason)| reason));
    }
}
