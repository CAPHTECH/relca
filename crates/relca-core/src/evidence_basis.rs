//! Contract evidence is separate from the may-lineage used by Agency.
use crate::{diag, ir};
use std::collections::{BTreeMap, BTreeSet};
mod repetition;

pub(crate) enum SourceInput<'a> {
    Parent(ir::BindingId),
    Direct(&'a [ir::ActorId]),
}

/// One lineage rule for Agency's may graph and the contract's must graph.
/// Only snapshot edges differ. Visiting, rather than allocating a new union,
/// preserves Agency's exact insert/copy/work accounting and traversal order.
pub(crate) fn derive_sources<E>(
    call: &ir::Call,
    cap: &ir::Capability,
    snapshots: &[ir::BindingId],
    mut visit: impl FnMut(SourceInput<'_>) -> Result<(), E>,
) -> Result<(), E> {
    for &parent in call.args.iter().chain(snapshots) {
        if parent != ir::CURRENT_CASE {
            visit(SourceInput::Parent(parent))?;
        }
    }
    visit(SourceInput::Direct(&cap.reads))
}

fn union(
    call: &ir::Call,
    cap: &ir::Capability,
    snapshots: &[ir::BindingId],
    inherited: &[BTreeSet<ir::ActorId>],
) -> BTreeSet<ir::ActorId> {
    let mut set = BTreeSet::new();
    let result: Result<(), std::convert::Infallible> =
        derive_sources(call, cap, snapshots, |input| {
            match input {
                SourceInput::Parent(p) => set.extend(&inherited[p.0]),
                SourceInput::Direct(sources) => set.extend(sources),
            }
            Ok(())
        });
    result.unwrap();
    set
}

pub(crate) fn always_writes(c: &ir::Capability, v: ir::StateVarId) -> bool {
    c.start_writes.iter().any(|w| w.variable == v)
        || (c.atomic.is_none()
            && c.version_check.is_none()
            && (c.writes.iter().any(|w| w.variable == v)
                || (!c.result_kernels().collect::<Vec<_>>().is_empty()
                    && c.result_kernels().all(|k| {
                        k.rows.iter().all(|r| {
                            r.branches
                                .iter()
                                .all(|b| b.overrides.iter().any(|(x, _)| *x == v))
                        })
                    }))))
}

pub(crate) fn present(m: &ir::Model) -> bool {
    m.contracts
        .iter()
        .any(|c| matches!(c.kind, ir::ContractKind::EvidenceBasis { .. }))
}

fn unsupported(reason: &str, span: diag::Span) -> Vec<diag::Diagnostic> {
    vec![diag::Diagnostic {
        presentation: Default::default(),
        code: diag::U_SEM_0.into(),
        severity: diag::Severity::Error,
        message: reason.into(),
        span: Some(span),
    }]
}

pub(crate) fn writes(c: &ir::Capability, v: ir::StateVarId) -> bool {
    c.start_writes
        .iter()
        .chain(c.finish_writes())
        .any(|w| w.variable == v)
        || c.kernel_writes.contains(&v)
        || c.structural_kernel().is_some_and(|k| {
            k.rows
                .iter()
                .flat_map(|r| &r.branches)
                .flat_map(|b| &b.overrides)
                .any(|(x, _)| *x == v)
        })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SiteKind {
    Call,
    OptionalAttempt,
    ResultAlias,
}

#[derive(Clone)]
struct Site<'a> {
    call: &'a ir::Call,
    prefix: BTreeSet<ir::BindingId>,
    repeated_prefix: BTreeSet<ir::BindingId>,
    possible_prefix: Option<BTreeSet<ir::BindingId>>,
    kind: SiteKind,
}

fn exclusive(body: &ir::Block, a: diag::Span, b: diag::Span) -> bool {
    let contains = |block: &ir::Block, p: diag::Span| {
        block
            .iter()
            .any(|s| s.span.start <= p.start && p.end <= s.span.end)
    };
    body.iter().any(|s| match &s.kind {
        ir::Stmt::If {
            then_body,
            else_body,
            ..
        } => {
            (contains(then_body, a) && contains(else_body, b))
                || (contains(else_body, a) && contains(then_body, b))
                || exclusive(then_body, a, b)
                || exclusive(else_body, a, b)
        }
        _ => false,
    })
}

/// The same/outer block prefix dominates; a completed fork is deliberately not
/// promoted into that prefix in stage one. Each branch still inherits its outer
/// prefix. No condition, requires, wait, or latent kernel edge is an input.
fn collect<'a>(body: &'a ir::Block, prefix: &BTreeSet<ir::BindingId>, out: &mut Vec<Site<'a>>) {
    let mut prefix = prefix.clone();
    for s in body {
        match &s.kind {
            ir::Stmt::Call(c) => {
                out.push(Site {
                    call: c,
                    prefix: prefix.clone(),
                    repeated_prefix: BTreeSet::new(),
                    possible_prefix: None,
                    kind: SiteKind::Call,
                });
                prefix.insert(c.binding);
            }
            ir::Stmt::Response(c, r) => {
                if r.attempt == 1 {
                    // This binding carries an adopted result. It is never a
                    // Task and cannot itself emit a Finish or write a snapshot.
                    out.push(Site {
                        call: &r.command_call,
                        prefix: prefix.clone(),
                        repeated_prefix: BTreeSet::new(),
                        possible_prefix: None,
                        kind: SiteKind::ResultAlias,
                    });
                    prefix.insert(r.command);
                }
                out.push(Site {
                    call: c,
                    prefix: prefix.clone(),
                    repeated_prefix: BTreeSet::new(),
                    possible_prefix: None,
                    kind: if r.attempt == 1 {
                        SiteKind::Call
                    } else {
                        SiteKind::OptionalAttempt
                    },
                });
                prefix.insert(c.binding);
            }
            ir::Stmt::Fork { branches, .. } => {
                for c in branches {
                    out.push(Site {
                        call: c,
                        prefix: prefix.clone(),
                        repeated_prefix: BTreeSet::new(),
                        possible_prefix: None,
                        kind: SiteKind::Call,
                    });
                }
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                collect(then_body, &prefix, out);
                collect(else_body, &prefix, out);
            }
            _ => {}
        }
    }
}

fn sites(m: &ir::Model) -> Vec<Site<'_>> {
    let mut calls = Vec::new();
    collect(&m.process.body, &BTreeSet::new(), &mut calls);
    if m.has_loops()
        && (present(m)
            || calls.iter().any(|s| {
                m.loop_site(s.call.binding).is_some()
                    && crate::registration::participates(&m.capabilities[s.call.capability.0])
            }))
    {
        repetition::collect(m, &mut calls);
    }
    calls
}

/// Diagnostic-only view of the prepared lineage table, not another proof of
/// the property. Slots belonging to other capabilities are provenance, not
/// target calls; an exit-scoped target uses its per-exit summaries.
pub(crate) fn snapshot_detail<'a>(
    m: &ir::Model,
    calls: &[ir::EvidenceCall],
    reason: &str,
    findings: impl Iterator<
        Item = (
            ir::BindingId,
            &'a [ir::EvidenceExit],
            &'a BTreeSet<ir::ActorId>,
        ),
    >,
) -> Option<diag::Detail> {
    let sites = sites(m);
    let by_id: std::collections::BTreeMap<_, _> =
        sites.iter().map(|s| (s.call.binding, s.call)).collect();
    let by_name: std::collections::BTreeMap<_, _> = m
        .process
        .bindings
        .iter()
        .enumerate()
        .map(|(i, b)| (b.name.as_str(), ir::BindingId(i)))
        .collect();
    let label = |id: ir::BindingId| -> Option<String> {
        if let Some(site) = m.loop_site(id) {
            return Some(site.call_site.clone());
        }
        if let Some(attempt) = m.response_attempt(id) {
            let command = &m.process.bindings.get(attempt.command.0)?.name;
            return Some(format!("{command} (attempt {})", attempt.attempt));
        }
        Some(m.process.bindings.get(id.0)?.name.clone())
    };
    let mut blockers = Vec::new();
    for (binding, exits, sources) in findings {
        let call = by_id.get(&binding)?;
        let c = calls.iter().find(|c| c.binding == binding)?;
        let edges = c
            .exit_scopes
            .iter()
            .find(|s| s.exits == exits)
            .map_or(&c.excluded_edges, |s| &s.excluded_edges);
        let rendered_exits = exits
            .iter()
            .map(|exit| {
                let declaration = m
                    .state_variables
                    .get(exit.variable.0)?
                    .loop_round_declaration_name();
                Some(diag::EvidenceExit {
                    loop_name: declaration.strip_suffix(".round")?.into(),
                    round: exit.round,
                    phase: if exit.on_limit {
                        ir::LoopPhase::OnLimit
                    } else {
                        ir::LoopPhase::Body
                    },
                    leave_site: exit.leave_site,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let mut push = |role, name, span| {
            let mut b = diag::Blocker::at(role, name, span);
            b.exits.clone_from(&rendered_exits);
            blockers.push(b);
        };
        push("target_call", Some(label(binding)?), Some(call.span));
        for source in sources {
            let actor = m.actors.get(source.0)?;
            push(
                "conditional_source",
                Some(actor.name.clone()),
                Some(actor.span),
            );
        }
        for edge in edges {
            let Some((writer, reader)) = edge.split_once(" -> ") else {
                continue;
            };
            let (Some(&writer), Some(&reader)) = (by_name.get(writer), by_name.get(reader)) else {
                continue;
            };
            push(
                "excluded_edge",
                Some(format!("{} -> {}", label(writer)?, label(reader)?)),
                by_id.get(&writer).map(|c| c.span),
            );
        }
        for (exit, rendered) in exits.iter().zip(rendered_exits) {
            let mut row = diag::Blocker::at("exit", None, exit_span(m, exit));
            row.exits.push(rendered);
            blockers.push(row);
        }
    }
    (!blockers.is_empty()).then(|| diag::Detail {
        reason: reason.into(),
        kind: diag::DetailKind::PropertyUndecided,
        cause: None,
        blockers,
        see: "docs/guide/dsl.md#根拠源を求める契約".into(),
    })
}

fn exit_span(m: &ir::Model, exit: &ir::EvidenceExit) -> Option<diag::Span> {
    fn find(body: &ir::Block, active: ir::StateVarId, tag: u64) -> Option<diag::Span> {
        body.iter().find_map(|s| match &s.kind {
            ir::Stmt::LoopControl(c)
                if c.updates.contains(&(active, ir::StateValue::Integer(tag))) =>
            {
                Some(s.span)
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => find(then_body, active, tag).or_else(|| find(else_body, active, tag)),
            _ => None,
        })
    }
    let prefix = m
        .state_variables
        .get(exit.variable.0)?
        .name
        .strip_suffix(":round")?;
    let active = m
        .state_variables
        .iter()
        .position(|v| v.name.strip_prefix(prefix) == Some(":active"))?;
    find(
        &m.process.body,
        ir::StateVarId(active),
        exit.leave_site?.checked_add(1)?,
    )
}

pub(crate) fn prepare(m: &mut ir::Model) -> Result<(), Vec<diag::Diagnostic>> {
    if !present(m) {
        return Ok(());
    }
    let span = m
        .contracts
        .iter()
        .find(|c| matches!(c.kind, ir::ContractKind::EvidenceBasis { .. }))
        .unwrap()
        .span;
    if !m.process.degradations.is_empty() || m.recovery().is_some() {
        let mut blockers: Vec<_> = m
            .process
            .degradations
            .iter()
            .map(|d| diag::Blocker::at("degradation", Some(d.name.clone()), Some(d.span)))
            .collect();
        let cause = if blockers.is_empty() {
            "recovery"
        } else {
            "degradation"
        };
        for (_, attempt) in m.response_attempts() {
            if attempt.attempt == 1
                && let Some(body) = &attempt.recovery
            {
                let name = m
                    .process
                    .bindings
                    .get(attempt.command.0)
                    .map(|b| b.name.clone());
                blockers.push(diag::Blocker::at(
                    "response",
                    name.clone(),
                    Some(attempt.command_call.span),
                ));
                let handler = body.first().zip(body.last()).map(|(a, b)| diag::Span {
                    start: a.span.start,
                    end: b.span.end,
                });
                blockers.push(diag::Blocker::at("recovery_handler", name, handler));
            }
        }
        return Err(described(
            "evidence_basis_feature_combination",
            "evidence_basis_feature_combination",
            span,
            cause,
            blockers,
        ));
    }
    for c in &m.contracts {
        if let ir::ContractKind::EvidenceBasis { sources, .. } = &c.kind
            && sources
                .iter()
                .any(|s| !crate::responsibility::included(m, *s))
        {
            return Err(vec![diag::Diagnostic {
                presentation: Default::default(),
                code: diag::E_CUT_1.into(),
                severity: diag::Severity::Error,
                message: "evidence_basis references a source Actor excluded by the cut".into(),
                span: Some(c.span),
            }]);
        }
    }
    let calls = sites(m);
    let graph = graph_for(m, &calls, false, false);
    for clause in &m.contracts {
        if let ir::ContractKind::EvidenceBasis { capability, .. } = &clause.kind {
            if let Some(site) = calls
                .iter()
                .find(|s| s.call.capability == *capability && graph.record_loop[s.call.binding.0])
            {
                let mut rows = unsupported("loop_feature_combination", clause.span);
                let trace = graph_for(m, &calls, false, true).diagnostic_paths.unwrap();
                let origins = &trace[site.call.binding.0].via_record;
                let mut d = rows.remove(0).loop_detail(
                    "record_evidence_path",
                    diag::DetailKind::AnalysisNotConnected,
                    None,
                    Some(site.call.span),
                );
                d.presentation.detail.as_mut().unwrap().blockers.extend(
                    m.state_variables.iter().filter_map(|v| {
                        let name = v.name.strip_prefix("@loop:")?.strip_suffix(":round")?;
                        origins
                            .contains(name)
                            .then(|| diag::Blocker::repeat_from_name(v.span))
                    }),
                );
                return Err(vec![d]);
            }
            if calls
                .iter()
                .any(|s| s.call.capability == *capability && graph.merged[s.call.binding.0])
            {
                let trace = graph_for(m, &calls, false, true).diagnostic_paths.unwrap();
                let mut blockers = Vec::new();
                for site in calls
                    .iter()
                    .filter(|s| s.call.capability == *capability && graph.merged[s.call.binding.0])
                {
                    blockers.push(diag::Blocker::for_call(
                        "target_call",
                        m,
                        site.call,
                        site.call.binding,
                    ));
                    for id in &trace[site.call.binding.0].merges {
                        let binding = &m.process.bindings[id.0];
                        blockers.push(diag::Blocker::at(
                            "merge_binding",
                            Some(binding.name.clone()),
                            Some(binding.span),
                        ));
                    }
                }
                return Err(described(
                    "merge_control: evidence_basis target has merged inputs",
                    "merge_control",
                    clause.span,
                    "merged_evidence_input",
                    blockers,
                ));
            }
        }
    }
    let alias_target = |capability| {
        calls
            .iter()
            .any(|s| s.call.capability == capability && graph.loop_input[s.call.binding.0])
    };
    let scoped = m.contracts.iter().any(|c| matches!(c.kind, ir::ContractKind::EvidenceBasis { capability, direct: false, .. } if alias_target(capability)));
    if m.contracts.iter().any(|c| matches!(c.kind, ir::ContractKind::EvidenceBasis { capability, .. } if alias_target(capability))) {
        repetition::mark_leaves(m);
    }
    let calls = sites(m);
    let mut scopes = BTreeMap::<_, Vec<_>>::new();
    if scoped {
        for profile in repetition::profiles(m, &calls)
            .ok_or_else(|| unsupported("loop_expansion_limit", span))?
        {
            let projected = graph_for(m, &profile.sites, true, false);
            for binding in profile.local {
                let i = binding.0;
                scopes.entry(binding).or_default().push(ir::EvidenceScope {
                    exits: profile.key.clone(),
                    sources: projected.must[i].iter().copied().collect(),
                    possible_sources: projected.may[i].iter().copied().collect(),
                    conditional_sources: projected.conditional[i].iter().copied().collect(),
                    excluded_edges: projected.excluded[i].iter().cloned().collect(),
                });
            }
        }
    }
    let calls: Vec<_> = calls
        .iter()
        .map(|s| (s.call.binding, s.call.capability))
        .collect();
    for clause in &mut m.contracts {
        if let ir::ContractKind::EvidenceBasis {
            capability,
            direct,
            calls: table,
            ..
        } = &mut clause.kind
        {
            table.clear();
            for &(binding, call_capability) in &calls {
                let i = binding.0;
                let direct_target = *direct && call_capability == *capability;
                table.push(ir::EvidenceCall {
                    binding,
                    alias_input: graph.loop_input[i],
                    exit_scopes: scopes.get(&binding).cloned().unwrap_or_default(),
                    sources: if direct_target {
                        m.capabilities[capability.0]
                            .reads
                            .iter()
                            .copied()
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect()
                    } else {
                        graph.must[i].iter().copied().collect()
                    },
                    possible_sources: if direct_target {
                        m.capabilities[capability.0].reads.clone()
                    } else {
                        graph.may[i].iter().copied().collect()
                    },
                    conditional_sources: graph.conditional[i].iter().copied().collect(),
                    excluded_edges: if direct_target {
                        vec![]
                    } else {
                        graph.excluded[i].iter().cloned().collect()
                    },
                });
            }
        }
    }
    Ok(())
}

fn described(
    message: &str,
    reason: &str,
    span: diag::Span,
    cause: &str,
    blockers: Vec<diag::Blocker>,
) -> Vec<diag::Diagnostic> {
    let mut rows = unsupported(message, span);
    vec![rows.remove(0).described(
        reason,
        cause,
        diag::DetailKind::AnalysisNotConnected,
        blockers,
        "docs/guide/dsl.md#根拠源を求める契約",
    )]
}

pub(crate) fn exits(m: &ir::Model, state: &[ir::StateValue]) -> Vec<ir::EvidenceExit> {
    use ir::StateValue::Integer;
    m.state_variables
        .iter()
        .enumerate()
        .filter_map(|(i, v)| {
            let name = v
                .name
                .strip_suffix(":round")
                .filter(|_| v.is_loop_round())?;
            let value = |role| {
                m.state_variables
                    .iter()
                    .position(|v| {
                        v.name.strip_prefix(name).and_then(|s| s.strip_prefix(':')) == Some(role)
                    })
                    .and_then(|i| state.get(i))
                    .copied()
            };
            let Integer(round) = *state.get(i)? else {
                return None;
            };
            let Integer(active) = value("active")? else {
                return None;
            };
            (active != 1).then(|| ir::EvidenceExit {
                variable: ir::StateVarId(i),
                round,
                on_limit: value("limit") == Some(Integer(1)),
                leave_site: active.checked_sub(1).filter(|n| *n != 0),
            })
        })
        .collect()
}

pub(crate) fn call_table(m: &ir::Model) -> &[ir::EvidenceCall] {
    m.contracts
        .iter()
        .find_map(|c| match &c.kind {
            ir::ContractKind::EvidenceBasis { calls, .. } => Some(calls.as_slice()),
            _ => None,
        })
        .unwrap_or_default()
}

pub(crate) fn scope<'a>(
    m: &ir::Model,
    binding: ir::BindingId,
    states: &[ir::StateValue],
    calls: &'a [ir::EvidenceCall],
) -> Option<&'a ir::EvidenceScope> {
    let key = exits(m, states);
    calls
        .iter()
        .find(|c| c.binding == binding)?
        .exit_scopes
        .iter()
        .find(|s| s.exits == key)
}

#[derive(Clone)]
pub(crate) struct RecordAccess {
    pub record: ir::ActorId,
    pub registrars: Vec<ir::BindingId>,
    pub sources: BTreeSet<ir::ActorId>,
}

pub(crate) struct SourceGraph {
    pub must: Vec<BTreeSet<ir::ActorId>>,
    pub may: Vec<BTreeSet<ir::ActorId>>,
    conditional: Vec<BTreeSet<ir::ActorId>>,
    excluded: Vec<BTreeSet<String>>,
    pub merged: Vec<bool>,
    diagnostic_paths: Option<Vec<DiagnosticPath>>,
    loop_input: Vec<bool>,
    record_loop: Vec<bool>,
    pub records: Vec<Vec<RecordAccess>>,
    pub record_parents: Vec<Vec<ir::BindingId>>,
}

#[derive(Default)]
struct DiagnosticPath {
    merges: BTreeSet<ir::BindingId>,
    loops: BTreeSet<String>,
    via_record: BTreeSet<String>,
}

impl DiagnosticPath {
    fn inherit(&mut self, from: &Self, reads_record: bool) {
        self.merges.extend(&from.merges);
        self.loops.extend(from.loops.iter().cloned());
        self.via_record.extend(from.via_record.iter().cloned());
        if reads_record {
            self.via_record.extend(from.loops.iter().cloned());
        }
    }
}

#[derive(Clone, Copy)]
enum ReadTarget {
    Variable(ir::StateVarId),
    Record(ir::ActorId),
}
impl ReadTarget {
    fn writes(self, cap: &ir::Capability) -> bool {
        match self {
            Self::Variable(v) => writes(cap, v),
            Self::Record(r) => cap.registers.contains(&r),
        }
    }
    fn always_writes(self, cap: &ir::Capability) -> bool {
        match self {
            Self::Variable(v) => always_writes(cap, v),
            Self::Record(r) => cap.registers.contains(&r),
        }
    }
}

/// Source-wise must intersection and last-writer may union. Used for contracts
/// and records, not substituted for the legacy observed-variable Agency graph.
pub(crate) fn graph(m: &ir::Model) -> SourceGraph {
    let calls = sites(m);
    graph_for(m, &calls, false, false)
}

fn graph_for(
    m: &ir::Model,
    calls: &[Site<'_>],
    scoped: bool,
    trace_diagnostics: bool,
) -> SourceGraph {
    // Retry Task slots are allocated after the public result bindings. Their
    // numeric IDs are not lexical order; only the process traversal orders
    // writers before readers and source propagation before dependent calls.
    let n = m.process.bindings.len();
    let mut must = vec![BTreeSet::new(); n];
    let mut may = must.clone();
    let mut bound_must = must.clone();
    let mut bound_may = must.clone();
    let mut conditional = must.clone();
    let mut excluded: Vec<BTreeSet<String>> = vec![BTreeSet::new(); n];
    let mut records = vec![Vec::new(); n];
    let mut record_parents = vec![Vec::new(); n];
    // A merge is not an evidence-producing call. Track only whether one flows
    // into a target's inputs, never invent a union/intersection of its roots.
    // Control conditions are deliberately not evidence inputs (ADR 0070).
    let mut merged: Vec<_> = m
        .process
        .bindings
        .iter()
        .map(|b| b.merge.is_some())
        .collect();
    // Only collect names on the post-rejection pass; the boolean above
    // remains the sole input to the admission rule.
    let mut diagnostic_paths: Option<Vec<_>> = trace_diagnostics.then(|| {
        merged
            .iter()
            .enumerate()
            .map(|(i, is_merge)| DiagnosticPath {
                merges: is_merge.then_some(ir::BindingId(i)).into_iter().collect(),
                ..DiagnosticPath::default()
            })
            .collect()
    });
    let mut loop_input = vec![false; n];
    let mut record_loop = vec![false; n];
    for binding in (0..n).map(ir::BindingId) {
        if let Some(alias) = m.loop_alias(binding) {
            loop_input[alias.0] = true;
            if let (Some(paths), Some(site)) = (&mut diagnostic_paths, m.loop_site(binding)) {
                paths[alias.0].loops.insert(site.loop_name.clone());
            }
        }
    }
    for (i, site) in calls.iter().enumerate() {
        let call = site.call;
        let prefix = &site.prefix;
        let cap = &m.capabilities[call.capability.0];
        let mut certain = union(call, cap, &[], &bound_must);
        let mut possible = union(call, cap, &[], &bound_may);
        let mut uncertain = BTreeSet::new();
        let mut edges = BTreeSet::new();
        let mut merge_input = false;
        let mut traced = DiagnosticPath::default();
        let mut aliased = false;
        let mut via_record = false;
        for arg in call.args.iter().filter(|a| **a != ir::CURRENT_CASE) {
            edges.extend(excluded[arg.0].iter().cloned());
            merge_input |= merged[arg.0];
            if let Some(paths) = &diagnostic_paths {
                traced.inherit(&paths[arg.0], false);
            }
            aliased |= loop_input[arg.0];
            via_record |= record_loop[arg.0];
            if scoped && let Some(prior) = &site.possible_prefix {
                let mut common = None::<BTreeSet<_>>;
                for b in calls[..i]
                    .iter()
                    .rev()
                    .map(|s| s.call.binding)
                    .filter(|&b| prior.contains(&b) && m.loop_alias(b) == Some(*arg))
                {
                    possible.extend(&bound_may[b.0]);
                    let roots = &bound_must[b.0];
                    common = Some(common.map_or_else(
                        || roots.clone(),
                        |old| old.intersection(roots).copied().collect(),
                    ));
                    if site.repeated_prefix.contains(&b) {
                        certain.extend(common.take().unwrap());
                        break;
                    }
                }
            }
        }
        for target in cap
            .read_variables
            .iter()
            .copied()
            .map(ReadTarget::Variable)
            .chain(cap.read_records.iter().copied().map(ReadTarget::Record))
        {
            let mut candidates = Vec::new();
            let mut guaranteed = false;
            for parent in calls[..i].iter().rev() {
                if parent.kind == SiteKind::ResultAlias {
                    continue;
                }
                let repeated = m.loop_site(parent.call.binding).is_some();
                if site
                    .possible_prefix
                    .as_ref()
                    .filter(|_| repeated || scoped)
                    .map_or_else(
                        || exclusive(&m.process.body, parent.call.span, call.span),
                        |prior| !prior.contains(&parent.call.binding),
                    )
                {
                    continue;
                }
                let writer = parent.call;
                let wc = &m.capabilities[writer.capability.0];
                if !target.writes(wc) {
                    continue;
                }
                candidates.push(writer.binding);
                let dominates = if (repeated || scoped) && site.possible_prefix.is_some() {
                    &site.repeated_prefix
                } else {
                    prefix
                };
                if dominates.contains(&writer.binding)
                    && parent.kind == SiteKind::Call
                    && wc.requires.is_none()
                    && target.always_writes(wc)
                {
                    guaranteed = true;
                    break;
                }
            }
            let mut shared = if guaranteed {
                candidates
                    .first()
                    .map(|b| must[b.0].clone())
                    .unwrap_or_default()
            } else {
                BTreeSet::new()
            };
            let mut all = BTreeSet::new();
            for &writer in &candidates {
                merge_input |= merged[writer.0];
                if let Some(paths) = &diagnostic_paths {
                    traced.inherit(&paths[writer.0], matches!(target, ReadTarget::Record(_)));
                }
                aliased |= loop_input[writer.0];
                via_record |= record_loop[writer.0]
                    || (loop_input[writer.0] && matches!(target, ReadTarget::Record(_)));
                shared = shared.intersection(&must[writer.0]).copied().collect();
                all.extend(&may[writer.0]);
                edges.extend(excluded[writer.0].iter().cloned());
            }
            let optional_writer = candidates.len() != 1 || !guaranteed;
            // Even a mandatory registrar may only possibly have the source.
            // Keep that candidate for the contract, not for via_records.
            if optional_writer || matches!(target, ReadTarget::Record(_)) {
                uncertain.extend(all.difference(&shared).copied());
            }
            if optional_writer {
                for writer in &candidates {
                    edges.insert(format!(
                        "{} -> {}",
                        m.process.bindings[writer.0].name, m.process.bindings[call.binding.0].name
                    ));
                }
            }
            certain.extend(&shared);
            if let ReadTarget::Record(record) = target {
                for &writer in &candidates {
                    if !record_parents[call.binding.0].contains(&writer) {
                        record_parents[call.binding.0].push(writer);
                    }
                }
                if !shared.is_empty() {
                    let mut registrars = candidates.clone();
                    registrars.reverse();
                    records[call.binding.0].push(RecordAccess {
                        record,
                        registrars,
                        sources: shared,
                    });
                }
            }
            possible.extend(all);
        }
        uncertain.retain(|s| !certain.contains(s));
        must[call.binding.0] = certain;
        may[call.binding.0] = possible;
        conditional[call.binding.0] = uncertain;
        bound_must[call.binding.0] = if cap.requires.is_none() {
            must[call.binding.0].clone()
        } else {
            BTreeSet::new()
        };
        bound_may[call.binding.0] = may[call.binding.0].clone();
        excluded[call.binding.0] = edges;
        merged[call.binding.0] = merge_input;
        loop_input[call.binding.0] |= aliased;
        record_loop[call.binding.0] = via_record;
        if let Some(alias) = m.loop_alias(call.binding) {
            merged[alias.0] |= merge_input;
            record_loop[alias.0] |= via_record;
            if let Some(paths) = &mut diagnostic_paths {
                paths[alias.0].inherit(&traced, false);
            }
        }
        if let Some(paths) = &mut diagnostic_paths {
            paths[call.binding.0] = traced;
        }
    }
    SourceGraph {
        must,
        may,
        conditional,
        excluded,
        merged,
        diagnostic_paths,
        loop_input,
        record_loop,
        records,
        record_parents,
    }
}

/// Execution must not trust a forged public-IR proof table. Ordinary models
/// return before cloning or doing any evidence graph work.
pub(crate) fn validate(m: &ir::Model) -> Result<(), &'static str> {
    if !present(m) {
        return Ok(());
    }
    let bad = "invalid evidence_basis table";
    for cap in &m.capabilities {
        if cap
            .reads
            .iter()
            .any(|s| m.actors.get(s.0).is_none_or(|a| a.source.is_none()))
            || cap.read_variables.iter().any(|v| {
                m.state_variables
                    .get(v.0)
                    .is_none_or(|v| v.kind != ir::StateKind::ObservedCase)
            })
        {
            return Err(bad);
        }
    }
    for c in &m.contracts {
        if let ir::ContractKind::EvidenceBasis {
            capability,
            sources,
            ..
        } = &c.kind
            && (m.capabilities.get(capability.0).is_none()
                || sources.is_empty()
                || sources.iter().collect::<BTreeSet<_>>().len() != sources.len()
                || sources
                    .iter()
                    .any(|s| m.actors.get(s.0).is_none_or(|a| a.source.is_none())))
        {
            return Err(bad);
        }
    }
    let mut expected = m.clone();
    prepare(&mut expected).map_err(|_| bad)?;
    if expected.contracts != m.contracts
        || expected.process.body != m.process.body
        || expected.state_variables != m.state_variables
    {
        return Err(bad);
    }
    Ok(())
}
