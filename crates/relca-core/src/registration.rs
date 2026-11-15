//! Value-free records. All work is gated on explicit registration declarations.
use crate::agency::{RecordProvenance, RecordRegistrar, RecordSource};
use crate::{diag, ir};

pub(crate) fn routes(
    m: &ir::Model,
    graph: &crate::evidence_basis::SourceGraph,
    binding: ir::BindingId,
) -> Vec<RecordProvenance> {
    let calls = crate::responsibility::calls(m);
    let mut out = Vec::new();
    for access in &graph.records[binding.0] {
        let mut sources: Vec<_> = access
            .sources
            .iter()
            .map(|&s| RecordSource {
                source: m.actors[s.0].name.clone(),
                route: if access.registrars.iter().all(|b| {
                    calls
                        .get(b)
                        .is_some_and(|c| m.capabilities[c.capability.0].reads.contains(&s))
                }) {
                    "registrar_direct"
                } else {
                    "relayed"
                }
                .into(),
            })
            .collect();
        sources.sort_by(|a, b| a.source.cmp(&b.source));
        let registrars = access
            .registrars
            .iter()
            .filter_map(|b| {
                calls.get(b).map(|c| {
                    let cap = &m.capabilities[c.capability.0];
                    let mut people: Vec<_> = m.actors[cap.actor.0]
                        .executors
                        .iter()
                        .flatten()
                        .map(|p| m.people[p.0].name.clone())
                        .collect();
                    people.sort();
                    people.dedup();
                    RecordRegistrar {
                        call: m.site_name(*b),
                        people,
                    }
                })
            })
            .collect();
        out.push(RecordProvenance {
            record: m.actors[access.record.0].name.clone(),
            registrars,
            sources,
        });
    }
    out.sort_by(|a, b| a.record.cmp(&b.record));
    out
}

pub(crate) fn merge_routes(
    m: &ir::Model,
    out: &mut Vec<RecordProvenance>,
    routes: impl IntoIterator<Item = RecordProvenance>,
) {
    for route in routes {
        if let Some(row) = out.iter_mut().find(|r| r.record == route.record) {
            for registrar in route.registrars {
                if !row.registrars.contains(&registrar) {
                    row.registrars.push(registrar);
                }
            }
            for source in route.sources {
                if let Some(old) = row.sources.iter_mut().find(|s| s.source == source.source) {
                    if source.route == "relayed" {
                        old.route = source.route;
                    }
                } else {
                    row.sources.push(source);
                }
            }
        } else {
            out.push(route);
        }
    }
    out.sort_by(|a, b| a.record.cmp(&b.record));
    for row in out {
        row.sources.sort_by(|a, b| a.source.cmp(&b.source));
        row.registrars.sort_by_key(|r| {
            m.process
                .bindings
                .iter()
                .position(|b| b.name == r.call)
                .map(|i| m.process.bindings[i].span.start)
                .unwrap_or(usize::MAX)
        });
    }
}

pub(crate) fn present(m: &ir::Model) -> bool {
    m.capabilities.iter().any(|c| !c.registers.is_empty())
}

pub(crate) fn declared(m: &crate::ast::Model) -> bool {
    use crate::ast::{ActorItem, CapItem, Item};
    m.items.iter().any(|i| matches!(i, Item::Actor(a) if a.items.iter().any(|i|
        matches!(i, ActorItem::Capability(c) if c.items.iter().any(|i| matches!(i.value, CapItem::Registers(_)))))))
}

pub(crate) fn participates(c: &ir::Capability) -> bool {
    !c.registers.is_empty() || !c.read_records.is_empty()
}

fn error(code: &str, reason: &str, span: diag::Span) -> Vec<diag::Diagnostic> {
    vec![diag::Diagnostic {
        code: code.into(),
        severity: diag::Severity::Error,
        message: reason.into(),
        span: Some(span),
    }]
}

pub(crate) fn supported(m: &ir::Model) -> Result<(), Vec<diag::Diagnostic>> {
    if !present(m) {
        return Ok(());
    }
    for cap in &m.capabilities {
        for &id in cap.registers.iter().chain(&cap.read_records) {
            if !crate::responsibility::included(m, id) {
                return Err(error(
                    diag::E_CUT_1,
                    "registration references an Actor excluded by the cut",
                    cap.span,
                ));
            }
            if m.bounds.cases > 1 && !m.actors[id.0].is_per_case() {
                return Err(error(
                    diag::U_SEM_0,
                    "registration_feature_combination",
                    cap.span,
                ));
            }
        }
        if !cap.registers.is_empty() {
            if !cap.start_writes.is_empty() {
                return Err(error(
                    diag::U_SEM_0,
                    "registration_feature_combination",
                    cap.span,
                ));
            }
            if cap.finish_results().len() > 1 {
                return Err(error(
                    diag::U_SEM_0,
                    "registration_result_dependent",
                    cap.span,
                ));
            }
        }
    }
    fn block(m: &ir::Model, body: &ir::Block, recovery: bool) -> Result<(), Vec<diag::Diagnostic>> {
        for stmt in body {
            let check = |c: &ir::Call, response: bool| {
                if participates(&m.capabilities[c.capability.0]) {
                    if m.process.bindings[c.binding.0].repetition.is_some() {
                        return Err(error(diag::U_SEM_0, "loop_feature_combination", c.span));
                    }
                    if recovery || response {
                        return Err(error(
                            diag::U_SEM_0,
                            "registration_feature_combination",
                            c.span,
                        ));
                    }
                }
                Ok(())
            };
            match &stmt.kind {
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => check(c, false)?,
                ir::Stmt::Response(c, r) => {
                    check(c, true)?;
                    if let Some(body) = &r.recovery {
                        block(m, body, true)?;
                    }
                }
                ir::Stmt::Fork { branches, .. } => {
                    for c in branches {
                        check(c, false)?;
                    }
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    block(m, then_body, recovery)?;
                    block(m, else_body, recovery)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    block(m, &m.process.body, false)?;
    for route in &m.process.degradations {
        if route
            .normal_calls
            .iter()
            .chain(&route.degraded_calls)
            .any(|c| participates(&m.capabilities[c.capability.0]))
        {
            return Err(error(
                diag::U_SEM_0,
                "registration_feature_combination",
                m.process.span,
            ));
        }
    }
    for route in m.contests() {
        if participates(&m.capabilities[route.route.0]) {
            return Err(error(
                diag::U_SEM_0,
                "registration_feature_combination",
                route.span,
            ));
        }
    }
    Ok(())
}
