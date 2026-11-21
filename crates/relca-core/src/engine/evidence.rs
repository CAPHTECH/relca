//! Path-local value provenance, allocated only for evidence contracts.
use super::*;

/// Report-only observations. Kept out of State and consulted only after the
/// existing property reduction has selected its verdict and diagnostic.
pub(super) type Findings =
    BTreeMap<(ir::ClauseId, ir::BindingId, Vec<ir::EvidenceExit>), BTreeSet<ir::ActorId>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Issue {
    Snapshot,
    Contest,
    VersionContest,
}
impl Issue {
    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::Snapshot => "evidence_basis_conditional_snapshot",
            Self::Contest => "evidence_basis_contest_route",
            Self::VersionContest => "same_version_contest_route",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) struct Value {
    pub roots: BTreeSet<ir::ActorId>,
    pub unknown: BTreeSet<ir::ActorId>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Snapshot {
    value: Value,
    writer: ir::BindingId,
    confirmed: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(super) struct History {
    pub values: BTreeMap<ir::BindingId, Value>,
    started: BTreeMap<ir::BindingId, Value>,
    snapshots: BTreeMap<ir::StateVarId, Snapshot>,
    extra: BTreeMap<ir::BindingId, Vec<ir::StateVarId>>,
    scoped: Option<Box<History>>,
}

pub(super) fn route(m: &ir::Model, capability: ir::CapabilityId) -> bool {
    m.contests().iter().any(|c| c.route == capability)
}
fn call(body: &ir::Block, b: ir::BindingId) -> Option<&ir::Call> {
    body.iter().find_map(|s| match &s.kind {
        ir::Stmt::Call(c) | ir::Stmt::Response(c, _) if c.binding == b => Some(c),
        ir::Stmt::Fork { branches, .. } => branches.iter().find(|c| c.binding == b),
        ir::Stmt::If {
            then_body,
            else_body,
            ..
        } => call(then_body, b).or_else(|| call(else_body, b)),
        _ => None,
    })
}
impl History {
    pub(super) fn new(m: &ir::Model) -> Self {
        Self {
            scoped: crate::evidence_basis::call_table(m)
                .iter()
                .any(|c| !c.exit_scopes.is_empty())
                .then(|| Box::new(Self::default())),
            ..Self::default()
        }
    }
    pub(super) fn finished(&self, b: ir::BindingId, scoped: bool) -> Option<&Value> {
        (if scoped {
            self.scoped.as_deref()?
        } else {
            self
        })
        .values
        .get(&b)
    }
    pub(super) fn adopt(&mut self, destination: ir::BindingId, source: ir::BindingId) {
        self.values.insert(
            destination,
            self.values.get(&source).cloned().unwrap_or_default(),
        );
        if let Some(h) = &mut self.scoped {
            h.adopt(destination, source);
        }
    }
    pub(super) fn overrides(&mut self, binding: ir::BindingId, variables: Vec<ir::StateVarId>) {
        if let Some(h) = &mut self.scoped {
            h.overrides(binding, variables.clone());
        }
        self.extra.insert(binding, variables);
    }
    fn inputs(
        &self,
        m: &ir::Model,
        binding: ir::BindingId,
        capability: ir::CapabilityId,
        state: Option<&[ir::StateValue]>,
    ) -> EngineResult<Value> {
        let cap = &m.capabilities[capability.0];
        let mut value = Value::default();
        let mut certain: BTreeSet<_> = cap.reads.iter().copied().collect();
        value.roots.extend(&cap.reads);
        if let Some(c) = call(&m.process.body, binding) {
            for arg in &c.args {
                if let Some(parent) = self.values.get(arg) {
                    value.roots.extend(&parent.roots);
                    value.unknown.extend(&parent.unknown);
                    certain.extend(parent.roots.difference(&parent.unknown).copied());
                }
            }
        }
        for v in &cap.read_variables {
            if let Some(snapshot) = self.snapshots.get(v)
                && snapshot.confirmed
            {
                value.roots.extend(&snapshot.value.roots);
                value.unknown.extend(&snapshot.value.unknown);
            }
        }
        if let Some(state) = state
            && m.site_contest(binding).is_none()
        {
            let scope = crate::evidence_basis::scope(
                m,
                binding,
                state,
                crate::evidence_basis::call_table(m),
            )
            .ok_or_else(|| Halt::from("evidence_basis missing exit scope"))?;
            if !cap.read_records.is_empty() {
                value.roots.extend(&scope.sources);
            }
            value.unknown.extend(&scope.conditional_sources);
        } else {
            for clause in &m.contracts {
                if let ir::ContractKind::EvidenceBasis { calls, .. } = &clause.kind
                    && let Some(c) = calls.iter().find(|c| c.binding == binding)
                {
                    // Records carry only the statically certain provenance. No
                    // extra executable state, timing or registration event is added.
                    if !cap.read_records.is_empty() {
                        value.roots.extend(&c.sources);
                    }
                    value.unknown.extend(&c.conditional_sources);
                }
            }
        }
        value.unknown.retain(|r| !certain.contains(r));
        Ok(value)
    }
}

pub(super) fn event(
    s: &mut State,
    event: Bundle,
    binding: Option<ir::BindingId>,
    m: &ir::Model,
) -> EngineResult<()> {
    let Some(b) = binding else {
        return Ok(());
    };
    let stale = match event {
        Bundle::Finish(c, _, _) => s
            .tasks
            .iter()
            .find(|t| t.binding == b)
            .is_some_and(|t| version_rejected(t, &m.capabilities[c.0])),
        _ => false,
    };
    let Some(h) = &mut s.evidence else {
        return Ok(());
    };
    if let Some(scoped) = &mut h.scoped {
        observe(scoped, event, b, m, stale, Some(&s.state_values))?;
    }
    observe(h, event, b, m, stale, None)
}

fn observe(
    h: &mut History,
    event: Bundle,
    b: ir::BindingId,
    m: &ir::Model,
    stale: bool,
    state: Option<&[ir::StateValue]>,
) -> EngineResult<()> {
    match event {
        Bundle::Start(c, _) => {
            let value = h.inputs(m, b, c, state)?;
            for w in &m.capabilities[c.0].start_writes {
                h.snapshots.insert(
                    w.variable,
                    Snapshot {
                        value: value.clone(),
                        writer: b,
                        confirmed: false,
                    },
                );
            }
            h.started.insert(b, value);
        }
        Bundle::Finish(c, result, _) => {
            let value = h.started.remove(&b).unwrap_or_default();
            for snapshot in h.snapshots.values_mut() {
                if snapshot.writer == b {
                    snapshot.confirmed = true;
                }
            }
            let cap = &m.capabilities[c.0];
            let writes = if stale {
                &[][..]
            } else {
                cap.atomic.as_ref().map_or(cap.writes.as_slice(), |a| {
                    if a.commit == result {
                        a.writes.as_slice()
                    } else {
                        &[]
                    }
                })
            };
            for variable in writes
                .iter()
                .map(|w| w.variable)
                .chain(h.extra.remove(&b).into_iter().flatten())
            {
                h.snapshots.insert(
                    variable,
                    Snapshot {
                        value: value.clone(),
                        writer: b,
                        confirmed: true,
                    },
                );
            }
            h.values.insert(b, value);
            if let Some(alias) = m.loop_alias(b) {
                h.values.insert(alias, h.values[&b].clone());
            }
        }
        Bundle::CancelAck(_, _) => {
            h.started.remove(&b);
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn diagnostics<'a>(
    m: &ir::Model,
    properties: &[outcome::PropertyOutcome],
    observations: impl IntoIterator<Item = &'a Findings>,
) -> Vec<diag::Diagnostic> {
    let observations: Vec<_> = observations.into_iter().collect();
    properties
        .iter()
        .filter_map(|p| {
            let ir::Property::Contract(id) = p.property else {
                return None;
            };
            if p.verdict != Verdict::Unsupported {
                return None;
            }
            let clause = &m.contracts[id.0];
            if let ir::ContractKind::SameVersion { first, second } = clause.kind {
                return (route(m, first.0) || route(m, second.0)).then(|| diag::Diagnostic {
                    presentation: Default::default(),
                    code: diag::U_SEM_0.into(),
                    severity: diag::Severity::Error,
                    message: "same_version_contest_route".into(),
                    span: Some(clause.span),
                });
            }
            let ir::ContractKind::EvidenceBasis {
                capability, calls, ..
            } = &clause.kind
            else {
                return None;
            };
            let issue = if route(m, *capability) {
                Issue::Contest
            } else if calls.iter().any(|c| {
                !c.conditional_sources.is_empty()
                    || c.exit_scopes
                        .iter()
                        .any(|s| !s.conditional_sources.is_empty())
            }) {
                Issue::Snapshot
            } else {
                return None;
            };
            Some(diag::Diagnostic {
                presentation: diag::Presentation {
                    detail: (issue == Issue::Snapshot)
                        .then(|| {
                            crate::evidence_basis::snapshot_detail(
                                m,
                                calls,
                                issue.reason(),
                                observations.iter().flat_map(|rows| rows.iter()).filter_map(
                                    |((clause, binding, exits), sources)| {
                                        (*clause == id).then_some((
                                            *binding,
                                            exits.as_slice(),
                                            sources,
                                        ))
                                    },
                                ),
                            )
                        })
                        .flatten()
                        .map(Box::new),
                    ..Default::default()
                },
                code: diag::U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: format!("{}: {}", clause.property_id(id), issue.reason()),
                span: Some(clause.span),
            })
        })
        .collect()
}
