//! Path-local value provenance, allocated only for evidence contracts.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Issue {
    Snapshot,
    Contest,
}
impl Issue {
    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::Snapshot => "evidence_basis_conditional_snapshot",
            Self::Contest => "evidence_basis_contest_route",
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
    pub(super) fn adopt(&mut self, destination: ir::BindingId, source: ir::BindingId) {
        self.values.insert(
            destination,
            self.values.get(&source).cloned().unwrap_or_default(),
        );
    }
    pub(super) fn overrides(&mut self, binding: ir::BindingId, variables: Vec<ir::StateVarId>) {
        self.extra.insert(binding, variables);
    }
    fn inputs(&self, m: &ir::Model, binding: ir::BindingId, capability: ir::CapabilityId) -> Value {
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
        for clause in &m.contracts {
            if let ir::ContractKind::EvidenceBasis { calls, .. } = &clause.kind
                && let Some(c) = calls.iter().find(|c| c.binding == binding)
            {
                value.unknown.extend(&c.conditional_sources);
            }
        }
        value.unknown.retain(|r| !certain.contains(r));
        value
    }
}

pub(super) fn event(s: &mut State, event: Bundle, binding: Option<ir::BindingId>, m: &ir::Model) {
    let Some(b) = binding else {
        return;
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
        return;
    };
    match event {
        Bundle::Start(c, _) => {
            let value = h.inputs(m, b, c);
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
        }
        Bundle::CancelAck(_, _) => {
            h.started.remove(&b);
        }
        _ => {}
    }
}

pub(crate) fn diagnostics(
    m: &ir::Model,
    properties: &[outcome::PropertyOutcome],
) -> Vec<diag::Diagnostic> {
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
            let ir::ContractKind::EvidenceBasis {
                capability, calls, ..
            } = &clause.kind
            else {
                return None;
            };
            let issue = if route(m, *capability) {
                Issue::Contest
            } else if calls.iter().any(|c| !c.conditional_sources.is_empty()) {
                Issue::Snapshot
            } else {
                return None;
            };
            Some(diag::Diagnostic {
                code: diag::U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: format!("{}: {}", clause.property_id(id), issue.reason()),
                span: Some(clause.span),
            })
        })
        .collect()
}
