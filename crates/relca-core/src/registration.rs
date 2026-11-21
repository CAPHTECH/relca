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
    let mut iterations = calls.keys().filter(|b| m.loop_alias(**b) == Some(binding));
    let folded = iterations.next().map(|first| {
        let mut rows = graph.records[first.0].clone();
        for iteration in iterations {
            rows.retain_mut(|row| {
                let Some(next) = graph.records[iteration.0]
                    .iter()
                    .find(|r| r.record == row.record)
                else {
                    return false;
                };
                row.sources.retain(|s| next.sources.contains(s));
                row.registrars.extend(&next.registrars);
                !row.sources.is_empty()
            });
        }
        rows
    });
    for access in folded.as_deref().unwrap_or(&graph.records[binding.0]) {
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
        let mut registrars: Vec<_> = access
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
                        call: m
                            .loop_site(*b)
                            .map_or_else(|| m.site_name(*b), |s| s.call_site.clone()),
                        people,
                    }
                })
            })
            .collect();
        if m.has_loops() {
            let mut seen = std::collections::BTreeSet::new();
            registrars.retain(|r| seen.insert(r.call.clone()));
        }
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

pub(crate) fn participates(c: &ir::Capability) -> bool {
    !c.registers.is_empty() || !c.read_records.is_empty()
}

fn error(code: &str, reason: &str, span: diag::Span) -> Vec<diag::Diagnostic> {
    vec![diag::Diagnostic {
        presentation: Default::default(),
        code: code.into(),
        severity: diag::Severity::Error,
        message: reason.into(),
        span: Some(span),
    }]
}

const COMBINATION: &str = "registration_feature_combination";
const FINISH_DEPENDENT: &str = "registration_result_dependent";

fn described(
    m: &ir::Model,
    cap: &ir::Capability,
    span: diag::Span,
    cause: &str,
    call: Option<(&ir::Call, ir::BindingId)>,
    context: Option<diag::Blocker>,
) -> Vec<diag::Diagnostic> {
    let reason = if cause == "multiple_finish_results" {
        FINISH_DEPENDENT
    } else {
        COMBINATION
    };
    let name = |c: &ir::Capability| format!("{}.{}", m.actors[c.actor.0].name, c.name);
    let records: std::collections::BTreeSet<_> = cap
        .registers
        .iter()
        .chain(&cap.read_records)
        .copied()
        .filter(|id| cause != "shared_record" || !m.actors[id.0].is_per_case())
        .collect();
    let mut blockers: Vec<_> = records
        .iter()
        .map(|id| {
            let a = &m.actors[id.0];
            diag::Blocker::at("record", Some(a.name.clone()), Some(a.span))
        })
        .collect();
    let mut cap_ids = std::collections::BTreeSet::new();
    for (i, c) in m.capabilities.iter().enumerate() {
        if c.registers.iter().any(|id| records.contains(id)) {
            cap_ids.insert(ir::CapabilityId(i));
            blockers.push(diag::Blocker::at("registrar", Some(name(c)), Some(c.span)));
        }
    }
    if cap.registers.is_empty() {
        blockers.push(diag::Blocker::at("reader", Some(name(cap)), Some(cap.span)));
    }
    let at_call =
        |c: &ir::Call, binding: ir::BindingId| diag::Blocker::for_call("call", m, c, binding);
    if let Some((c, binding)) = call {
        blockers.push(at_call(c, binding));
    } else if context.is_none() {
        fn visit(body: &ir::Block, emit: &mut impl FnMut(&ir::Call, ir::BindingId)) {
            for s in body {
                match &s.kind {
                    ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => emit(c, c.binding),
                    ir::Stmt::Response(c, r) => {
                        emit(c, r.command);
                        if let Some(handler) = &r.recovery {
                            visit(handler, emit);
                        }
                    }
                    ir::Stmt::Fork { branches, .. } => {
                        branches.iter().for_each(|c| emit(c, c.binding))
                    }
                    ir::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        visit(then_body, emit);
                        visit(else_body, emit);
                    }
                    _ => {}
                }
            }
        }
        visit(&m.process.body, &mut |c, binding| {
            if cap_ids.contains(&c.capability) || std::ptr::eq(&m.capabilities[c.capability.0], cap)
            {
                blockers.push(at_call(c, binding));
            }
        });
    }
    blockers.extend(context);
    let mut rows = error(diag::U_SEM_0, reason, span);
    vec![rows.remove(0).described(
        reason,
        cause,
        if reason == FINISH_DEPENDENT {
            diag::DetailKind::Rewrite
        } else {
            diag::DetailKind::AnalysisNotConnected
        },
        blockers,
        "docs/guide/dsl.md#知識を記録へ登録する",
    )]
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
                return Err(described(m, cap, cap.span, "shared_record", None, None));
            }
        }
        if !cap.registers.is_empty() {
            if !cap.start_writes.is_empty() {
                return Err(described(m, cap, cap.span, "start_writes", None, None));
            }
            if cap.finish_results().len() > 1 {
                return Err(described(
                    m,
                    cap,
                    cap.span,
                    "multiple_finish_results",
                    None,
                    None,
                ));
            }
        }
    }
    fn block(m: &ir::Model, body: &ir::Block, recovery: bool) -> Result<(), Vec<diag::Diagnostic>> {
        for stmt in body {
            let check = |c: &ir::Call, response: Option<ir::BindingId>| {
                let cap = &m.capabilities[c.capability.0];
                if participates(cap) && (recovery || response.is_some()) {
                    let cause = if recovery {
                        "recovery"
                    } else {
                        "response_or_retry"
                    };
                    return Err(described(
                        m,
                        cap,
                        c.span,
                        cause,
                        Some((c, response.unwrap_or(c.binding))),
                        None,
                    ));
                }
                Ok(())
            };
            match &stmt.kind {
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => check(c, None)?,
                ir::Stmt::Response(c, r) => {
                    check(c, Some(r.command))?;
                    if let Some(body) = &r.recovery {
                        block(m, body, true)?;
                    }
                }
                ir::Stmt::Fork { branches, .. } => {
                    for c in branches {
                        check(c, None)?;
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
    let mut degradation = None::<Vec<diag::Diagnostic>>;
    for route in &m.process.degradations {
        for call in route
            .normal_calls
            .iter()
            .chain(&route.degraded_calls)
            .filter(|c| participates(&m.capabilities[c.capability.0]))
        {
            let mut rows = described(
                m,
                &m.capabilities[call.capability.0],
                m.process.span,
                "degradation",
                Some((call, call.binding)),
                Some(diag::Blocker::at(
                    "degradation",
                    Some(route.name.clone()),
                    Some(route.span),
                )),
            );
            if let Some(first) = &mut degradation {
                first[0]
                    .presentation
                    .detail
                    .as_mut()
                    .unwrap()
                    .blockers
                    .append(&mut rows[0].presentation.detail.as_mut().unwrap().blockers);
            } else {
                degradation = Some(rows);
            }
        }
    }
    if let Some(rows) = degradation {
        return Err(rows);
    }
    for route in m.contests() {
        if participates(&m.capabilities[route.route.0]) {
            let cap = &m.capabilities[route.route.0];
            let name = format!("{}.{}", m.actors[cap.actor.0].name, cap.name);
            return Err(described(
                m,
                cap,
                route.span,
                "contest_route",
                None,
                Some(diag::Blocker::at(
                    "contest_route",
                    Some(name),
                    Some(route.span),
                )),
            ));
        }
    }
    Ok(())
}
