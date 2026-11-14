//! ADR 0024: lexical dependency evidence, not a probabilistic independence test.
//! The existing Agency graph and its budget are read-only inputs to this pass.
use super::*;

const SCOPE: &str = "selected_process_lexical_calls";
const CAUSE_NOTICE: &str = "条件付き独立は given の下でだけ仮定され、無条件には独立でない";
const BASIS_NOTICE: &str = "basis は分析者の宣言であり、測定根拠の正しさは検査していない";
const TACIT_NOTICE: &str = "tacit は分析者の宣言であり、根拠源の妥当性は検査していない";
const CORRELATION_NOTICE: &str = "相関は先行する判定結果への条件付けであり、残余相関の推定ではない";
const LIMITS: Limits = Limits {
    work: 1_000_000,
    cells: 1_000_000,
    depth: 128,
    paths: 0,
    contexts: 0,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommonCauses {
    pub rules: String,
    pub scope: String,
    pub notice: String,
    pub causes: Vec<CommonCause>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommonCauseDerivation {
    Derived(CommonCauses),
    NotDerived(NotDerivedCommonCauses),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotDerivedCommonCauses {
    pub status: NotDerivedStatus,
    pub reason: LimitReason,
    pub limit: DerivationLimit,
}
impl CommonCauseDerivation {
    pub fn as_derived(&self) -> Option<&CommonCauses> {
        match self {
            Self::Derived(c) => Some(c),
            Self::NotDerived(_) => None,
        }
    }
    pub fn as_derived_mut(&mut self) -> Option<&mut CommonCauses> {
        match self {
            Self::Derived(c) => Some(c),
            Self::NotDerived(_) => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommonCause {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<ir::ActorInstance>,
    pub actor: String,
    pub latent: String,
    pub targets: Vec<Target>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub call: String,
    pub capability: String,
    pub kernel: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelUncertainties {
    pub rules: String,
    pub scope: String,
    pub items: Vec<ModelUncertainty>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelUncertainty {
    #[serde(flatten)]
    pub suppression: Suppression,
    pub call: String,
    pub capability: String,
    pub kernel: String,
    pub targets: Vec<String>,
    pub error_latents: Vec<String>,
    pub notice: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Suppression {
    #[serde(rename = "wlin2_basis")]
    Basis { basis: String },
    #[serde(rename = "wlin2_tacit")]
    Tacit { sources: Vec<String> },
    #[serde(rename = "wlin2_correlation")]
    Correlation { judgements: Vec<String> },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDescription {
    pub actor: String,
    pub kind: String,
    pub description: String,
}

fn target(m: &ir::Model, c: &ir::Call) -> Target {
    let cap = &m.capabilities[c.capability.0];
    Target {
        call: m.process.bindings[c.binding.0].name.clone(),
        capability: format!("{}.{}", m.actors[cap.actor.0].name, cap.name),
        kernel: cap.kernel.as_ref().expect("kernel call").name.clone(),
    }
}

fn latent_name(m: &ir::Model, id: ir::StateVarId) -> String {
    let v = &m.state_variables[id.0];
    match v.kind {
        ir::StateKind::ActorLatent(a) => format!("{}.{}", m.actors[a.0].name, v.name),
        _ => v.name.clone(),
    }
}

fn table_cells(b: &mut Budget, n: usize, width: usize) -> Result<()> {
    b.cells(n.checked_mul(width).ok_or_else(invalid)?)
}

// Must-normal-finish dataflow, not lexical binding order. Branch-local facts
// survive a merge only on every continuing path. A cancelled loser has no result.
fn before_start(
    block: &ir::Block,
    finished: Option<BTreeSet<ir::BindingId>>,
    before: &mut [BTreeSet<ir::BindingId>],
    b: &mut Budget,
    depth: usize,
) -> Result<Option<BTreeSet<ir::BindingId>>> {
    before_start_shared(block, finished, before, b, depth, &mut None)
}
fn before_start_shared(
    block: &ir::Block,
    mut finished: Option<BTreeSet<ir::BindingId>>,
    before: &mut [BTreeSet<ir::BindingId>],
    b: &mut Budget,
    depth: usize,
    seen: &mut Option<Vec<bool>>,
) -> Result<Option<BTreeSet<ir::BindingId>>> {
    b.depth(depth)?;
    for stmt in block {
        b.location.span = Some(stmt.span);
        b.work()?;
        match &stmt.kind {
            ir::Stmt::MergeCopy { .. } => {}
            ir::Stmt::Wait(_) | ir::Stmt::WaitUntil(_) => {}
            ir::Stmt::Call(c) | ir::Stmt::Query(c, _) | ir::Stmt::Response(c, _) => {
                if let Some(f) = &mut finished {
                    b.copy(f.len())?;
                    if seen.as_ref().is_some_and(|s| s[c.binding.0]) {
                        b.copy(before[c.binding.0].len())?;
                        before[c.binding.0].retain(|id| f.contains(id));
                    } else {
                        before[c.binding.0] = f.clone();
                    }
                    if let Some(seen) = seen {
                        seen[c.binding.0] = true;
                    }
                    b.insert(f, c.binding)?;
                }
            }
            ir::Stmt::Fork { branches, join, .. } => {
                if let Some(f) = &mut finished {
                    for c in branches {
                        b.copy(f.len())?;
                        if seen.as_ref().is_some_and(|s| s[c.binding.0]) {
                            b.copy(before[c.binding.0].len())?;
                            before[c.binding.0].retain(|id| f.contains(id));
                        } else {
                            before[c.binding.0] = f.clone();
                        }
                        if let Some(seen) = seen {
                            seen[c.binding.0] = true;
                        }
                    }
                    for c in branches {
                        b.work()?;
                        let normal = matches!(join, ir::Join::All)
                            || matches!(
                                join,
                                ir::Join::Any {
                                    remaining: ir::Remaining::Drain,
                                    ..
                                }
                            )
                            || branches.len() == 1;
                        if normal {
                            b.insert(f, c.binding)?;
                        }
                    }
                }
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                b.copy(finished.as_ref().map_or(0, BTreeSet::len))?;
                let left =
                    before_start_shared(then_body, finished.clone(), before, b, depth + 1, seen)?;
                let right = before_start_shared(else_body, finished, before, b, depth + 1, seen)?;
                finished = match (left, right) {
                    (Some(mut l), Some(r)) => {
                        b.steps(l.len())?;
                        l.retain(|id| r.contains(id));
                        Some(l)
                    }
                    (l, r) => l.or(r),
                };
            }
            ir::Stmt::ResolveEffect { .. } => {}
            ir::Stmt::LoopControl(_) => {}
            ir::Stmt::End(_) | ir::Stmt::StopReceiptUnknown => finished = None,
        }
    }
    Ok(finished)
}

pub(super) fn derive(
    m: &ir::Model,
    calls: &[&ir::Call],
    ancestors: &[Ancestors],
    subjects: &[BTreeSet<ir::ActorId>],
    agency: &mut Agency,
    warnings: &mut Vec<diag::Diagnostic>,
) -> Result<()> {
    soft_with_limits(m, calls, ancestors, subjects, agency, warnings, LIMITS)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn derive_shared(
    m: &ir::Model,
    calls: &[&ir::Call],
    ancestors: &[Ancestors],
    subjects: &[BTreeSet<ir::ActorId>],
    agency: &mut Agency,
    warnings: &mut Vec<diag::Diagnostic>,
    shared: bool,
) -> Result<()> {
    if shared {
        soft_mode(
            m, calls, ancestors, subjects, agency, warnings, LIMITS, true,
        )
    } else {
        derive(m, calls, ancestors, subjects, agency, warnings)
    }
}

fn soft_with_limits(
    m: &ir::Model,
    calls: &[&ir::Call],
    ancestors: &[Ancestors],
    subjects: &[BTreeSet<ir::ActorId>],
    agency: &mut Agency,
    warnings: &mut Vec<diag::Diagnostic>,
    limits: Limits,
) -> Result<()> {
    soft_mode(
        m, calls, ancestors, subjects, agency, warnings, limits, false,
    )
}
#[allow(clippy::too_many_arguments)]
fn soft_mode(
    m: &ir::Model,
    calls: &[&ir::Call],
    ancestors: &[Ancestors],
    subjects: &[BTreeSet<ir::ActorId>],
    agency: &mut Agency,
    warnings: &mut Vec<diag::Diagnostic>,
    limits: Limits,
    shared: bool,
) -> Result<()> {
    let before = warnings.len();
    match if shared {
        with_limits_mode(
            m, calls, ancestors, subjects, agency, warnings, limits, true,
        )
    } else {
        with_limits(m, calls, ancestors, subjects, agency, warnings, limits)
    } {
        Ok(()) => Ok(()),
        Err(Error::Limit(limit)) => {
            // The pass may have produced rows or warnings before the limit.
            // Withdraw all dependent output atomically, not just its last row.
            warnings.truncate(before);
            agency.common_causes =
                Some(CommonCauseDerivation::NotDerived(NotDerivedCommonCauses {
                    status: NotDerivedStatus::NotDerived,
                    reason: LimitReason::DerivationLimit,
                    limit: *limit,
                }));
            agency.model_uncertainties = None;
            agency.unchecked.extend([
                "common_causes: not_derived (derivation_limit)".into(),
                "model_uncertainties: not_derived (derivation_limit)".into(),
            ]);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[allow(clippy::too_many_arguments)]
fn with_limits(
    m: &ir::Model,
    calls: &[&ir::Call],
    ancestors: &[Ancestors],
    subjects: &[BTreeSet<ir::ActorId>],
    agency: &mut Agency,
    warnings: &mut Vec<diag::Diagnostic>,
    limits: Limits,
) -> Result<()> {
    with_limits_mode(
        m, calls, ancestors, subjects, agency, warnings, limits, false,
    )
}
#[allow(clippy::too_many_arguments)]
fn with_limits_mode(
    m: &ir::Model,
    calls: &[&ir::Call],
    ancestors: &[Ancestors],
    subjects: &[BTreeSet<ir::ActorId>],
    agency: &mut Agency,
    warnings: &mut Vec<diag::Diagnostic>,
    limits: Limits,
    shared: bool,
) -> Result<()> {
    let mut descriptions = Vec::new();
    for a in &m.actors {
        if let Some(description) = &a.metadata.source_description {
            if a.source != Some(ir::SourceKind::Tacit) {
                return Err(invalid());
            }
            descriptions.push(SourceDescription {
                actor: a.name.clone(),
                kind: "tacit".into(),
                description: description.clone(),
            });
        }
    }
    agency.source_descriptions = (!descriptions.is_empty()).then_some(descriptions);
    // Without a selected conditional kernel there is no candidate for either
    // graph analysis. In particular kernel-free legacy inputs use no new budget.
    if !calls.iter().any(|c| {
        m.capabilities[c.capability.0]
            .kernel
            .as_ref()
            .is_some_and(|k| !k.given.is_empty())
    }) {
        return Ok(());
    }
    let mut b = Budget {
        limits,
        work: 0,
        cells: 0,
        contexts: 0,
        paths: 0,
        location: Location::model(m, "common_cause"),
    };
    let n = calls.len();
    table_cells(&mut b, n, 7)?;
    let mut before = vec![BTreeSet::new(); n];
    if shared {
        table_cells(&mut b, n, 1)?;
        before_start_shared(
            &m.process.body,
            Some(BTreeSet::new()),
            &mut before,
            &mut b,
            0,
            &mut Some(vec![false; n]),
        )?;
    } else {
        before_start(
            &m.process.body,
            Some(BTreeSet::new()),
            &mut before,
            &mut b,
            0,
        )?;
    }
    let mut evidence = Vec::with_capacity(n);
    for (i, c) in calls.iter().enumerate() {
        b.copy(subjects[i].len())?;
        let mut sources = subjects[i].clone();
        for s in &m.capabilities[c.capability.0].reads {
            b.insert(&mut sources, *s)?;
        }
        evidence.push(sources);
    }
    let mut depths = vec![None; n];
    fn depth(
        i: usize,
        calls: &[&ir::Call],
        memo: &mut [Option<usize>],
        b: &mut Budget,
        level: usize,
    ) -> Result<usize> {
        b.work()?;
        b.depth(level)?;
        if let Some(d) = memo[i] {
            return Ok(d);
        }
        let mut d = 0;
        for a in &calls[i].args {
            if *a == ir::CURRENT_CASE {
                continue;
            }
            d = d.max(
                depth(
                    a.0,
                    calls,
                    memo,
                    b,
                    level.checked_add(1).ok_or_else(invalid)?,
                )?
                .checked_add(1)
                .ok_or_else(invalid)?,
            );
        }
        b.depth(d)?;
        memo[i] = Some(d);
        Ok(d)
    }
    for i in 0..n {
        depth(i, calls, &mut depths, &mut b, 0)?;
    }
    // Validate public IR indices before indexing any latent or result metadata.
    for v in &m.state_variables {
        b.work()?;
        // Sets and bounded integers use builtin tags, not EnumType indices.
        // Accept only the tag belonging to that domain, never arbitrary indices.
        let valid_base = match (v.case_set_size, v.integer_max) {
            (Some(_), None) => v.base == ir::CASE_ID_TYPE,
            (None, Some(_)) => v.base == ir::TypeId(usize::MAX),
            (None, None) => v.base.0 < m.types.len(),
            (Some(_), Some(_)) => false,
        };
        if !valid_base || matches!(v.kind, ir::StateKind::ActorLatent(a) if a.0 >= m.actors.len()) {
            return Err(invalid());
        }
    }
    let mut causes: BTreeMap<ir::StateVarId, BTreeSet<usize>> = BTreeMap::new();
    let mut groups: BTreeMap<ir::BindingId, BTreeSet<usize>> = BTreeMap::new();
    let mut errors = vec![BTreeMap::<ir::BindingId, BTreeSet<ir::StateVarId>>::new(); n];
    let mut result_vars = vec![BTreeSet::new(); n];
    for (i, c) in calls.iter().enumerate() {
        b.work()?;
        let cap = &m.capabilities[c.capability.0];
        for id in &cap.kernel_writes {
            b.work()?;
            let v = m.state_variables.get(id.0).ok_or_else(invalid)?;
            if v.kind == ir::StateKind::ObservedCase {
                return Err(invalid());
            }
        }
        let Some(k) = &cap.kernel else { continue };
        for id in &k.given {
            b.work()?;
            let v = m.state_variables.get(id.0).ok_or_else(invalid)?;
            if matches!(v.kind, ir::StateKind::ActorLatent(_)) {
                b.cells(1)?;
                b.insert(causes.entry(*id).or_default(), i)?;
            }
        }
        if k.given.is_empty() {
            continue;
        }
        for w in &cap.writes {
            b.work()?;
            let v = m.state_variables.get(w.variable.0).ok_or_else(invalid)?;
            if v.kind != ir::StateKind::ObservedCase
                || v.base != cap.return_type
                || w.source != ir::WriteSource::Result
            {
                continue;
            }
            let mut same_result = true;
            for row in &k.rows {
                b.work()?;
                for branch in &row.branches {
                    b.work()?;
                    for (id, value) in &branch.overrides {
                        b.work()?;
                        if id.0 >= m.state_variables.len() {
                            return Err(invalid());
                        }
                        if *id == w.variable && *value != ir::StateValue::Value(branch.result) {
                            same_result = false;
                        }
                    }
                }
            }
            if same_result {
                b.insert(&mut result_vars[i], w.variable)?;
            }
        }
        for p in ancestors[i].iter() {
            b.work()?;
            let producer = &m.capabilities[calls[p.0].capability.0];
            for id in &producer.kernel_writes {
                b.steps(k.given.len().checked_add(1).ok_or_else(invalid)?)?;
                if k.given.contains(id) {
                    b.cells(2)?;
                    b.insert(errors[i].entry(p).or_default(), *id)?;
                    b.insert(groups.entry(p).or_default(), i)?;
                }
            }
        }
    }
    let mut rows = Vec::new();
    for (id, users) in causes {
        if users.len() < 2 {
            continue;
        }
        b.copy(users.len().checked_add(1).ok_or_else(invalid)?)?;
        let v = &m.state_variables[id.0];
        let ir::StateKind::ActorLatent(a) = v.kind else {
            return Err(invalid());
        };
        rows.push(CommonCause {
            instance: m.actors[a.0].metadata.instance,
            actor: m.actors[a.0].name.clone(),
            latent: latent_name(m, id),
            targets: users.into_iter().map(|i| target(m, calls[i])).collect(),
        });
    }
    agency.common_causes =
        (!rows.is_empty()).then_some(CommonCauseDerivation::Derived(CommonCauses {
            rules: "common-cause-p1-1".into(),
            scope: SCOPE.into(),
            notice: CAUSE_NOTICE.into(),
            causes: rows,
        }));
    let mut items = Vec::new();
    for (i, c) in calls.iter().enumerate() {
        b.work()?;
        let cap = &m.capabilities[c.capability.0];
        let Some(k) = &cap.kernel else { continue };
        let mut targets = BTreeSet::new();
        let mut latents = BTreeSet::new();
        let mut tacit_targets = BTreeSet::new();
        let mut tacit_latents = BTreeSet::new();
        let mut tacit_sources = BTreeSet::new();
        let mut correlation_targets = BTreeSet::new();
        let mut correlation_latents = BTreeSet::new();
        let mut judgements = BTreeSet::new();
        for (p, err) in &errors[i] {
            b.work()?;
            let group = &groups[p];
            if group.len() < 2 {
                continue;
            }
            let mut peer_sources = BTreeSet::new();
            for j in group {
                b.work()?;
                if *j == i || ancestors[*j].contains(&c.binding) {
                    continue;
                }
                for source in &evidence[*j] {
                    b.insert(&mut peer_sources, *source)?;
                }
            }
            let mut new_tacit = BTreeSet::new();
            let mut new_explicit = false;
            for s in &evidence[i] {
                b.work()?;
                if !peer_sources.contains(s) {
                    if m.actors[s.0].source == Some(ir::SourceKind::Tacit) {
                        b.insert(&mut new_tacit, *s)?;
                    } else {
                        new_explicit = true;
                    }
                }
            }
            if new_explicit {
                continue;
            }
            if !new_tacit.is_empty() {
                b.insert(&mut tacit_targets, *p)?;
                for id in err {
                    b.insert(&mut tacit_latents, *id)?;
                }
                for s in new_tacit {
                    b.insert(&mut tacit_sources, s)?;
                }
                continue;
            }
            let mut correlated = BTreeSet::new();
            for j in group {
                b.work()?;
                if *j == i {
                    continue;
                }
                // Mutual given references do not establish a correlation model,
                // even if one of the two calls happens to be lexically earlier.
                let other = m.capabilities[calls[*j].capability.0]
                    .kernel
                    .as_ref()
                    .ok_or_else(invalid)?;
                let mut mutual = false;
                for id in &result_vars[i] {
                    b.steps(other.given.len().checked_add(1).ok_or_else(invalid)?)?;
                    mutual |= other.given.contains(id);
                }
                if mutual
                    || !(ancestors[i].contains(&calls[*j].binding)
                        || before[i].contains(&calls[*j].binding))
                {
                    continue;
                }
                for id in &result_vars[*j] {
                    b.steps(k.given.len().checked_add(1).ok_or_else(invalid)?)?;
                    if k.given.contains(id) {
                        b.insert(&mut correlated, *j)?;
                    }
                }
            }
            if !correlated.is_empty() {
                b.insert(&mut correlation_targets, *p)?;
                for id in err {
                    b.insert(&mut correlation_latents, *id)?;
                }
                for j in correlated {
                    b.insert(&mut judgements, j)?;
                }
                continue;
            }
            b.insert(&mut targets, *p)?;
            for id in err {
                b.insert(&mut latents, *id)?;
            }
        }
        for (ts, ls, suppression, notice) in [
            (
                &tacit_targets,
                &tacit_latents,
                Suppression::Tacit {
                    sources: {
                        b.copy(tacit_sources.len())?;
                        tacit_sources
                            .iter()
                            .map(|s| m.actors[s.0].name.clone())
                            .collect()
                    },
                },
                TACIT_NOTICE,
            ),
            (
                &correlation_targets,
                &correlation_latents,
                Suppression::Correlation {
                    judgements: {
                        b.copy(judgements.len())?;
                        judgements
                            .iter()
                            .map(|j| m.process.bindings[calls[*j].binding.0].name.clone())
                            .collect()
                    },
                },
                CORRELATION_NOTICE,
            ),
        ] {
            if ts.is_empty() {
                continue;
            }
            b.copy(
                ts.len()
                    .checked_add(ls.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(invalid)?,
            )?;
            let t = target(m, c);
            items.push(ModelUncertainty {
                suppression,
                call: t.call,
                capability: t.capability,
                kernel: t.kernel,
                targets: ts
                    .iter()
                    .map(|p| m.process.bindings[p.0].name.clone())
                    .collect(),
                error_latents: ls.iter().map(|id| latent_name(m, *id)).collect(),
                notice: notice.into(),
            });
        }
        if targets.is_empty() {
            continue;
        }
        b.copy(
            targets
                .len()
                .checked_add(latents.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(invalid)?,
        )?;
        let t = target(m, c);
        let targets: Vec<_> = targets
            .into_iter()
            .map(|p| m.process.bindings[p.0].name.clone())
            .collect();
        if let Some(basis) = &k.basis {
            items.push(ModelUncertainty {
                suppression: Suppression::Basis {
                    basis: basis.clone(),
                },
                call: t.call,
                capability: t.capability,
                kernel: t.kernel,
                targets,
                error_latents: latents.into_iter().map(|id| latent_name(m, id)).collect(),
                notice: BASIS_NOTICE.into(),
            });
        } else {
            warnings.push(diag::Diagnostic {
                code: diag::W_LIN_2.into(),
                severity: diag::Severity::Warning,
                message: format!(
                    "{} {} kernel {} targets [{}]: conditional independence under given assumes detection without a new evidence source",
                    t.call, t.capability, t.kernel, targets.join(", ")
                ),
                span: Some(c.span),
            });
        }
    }
    agency.model_uncertainties = (!items.is_empty()).then_some(ModelUncertainties {
        rules: "wlin2-suppression-p1-1".into(),
        scope: SCOPE.into(),
        items,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncertainty_wire_roundtrips_all_three_disjoint_payloads() {
        for (suppression, notice) in [
            (
                Suppression::Basis {
                    basis: String::new(),
                },
                BASIS_NOTICE,
            ),
            (
                Suppression::Tacit {
                    sources: vec!["Experience".into()],
                },
                TACIT_NOTICE,
            ),
            (
                Suppression::Correlation {
                    judgements: vec!["aV".into()],
                },
                CORRELATION_NOTICE,
            ),
        ] {
            let row = ModelUncertainty {
                suppression,
                call: "bV".into(),
                capability: "B.check".into(),
                kernel: "KB".into(),
                targets: vec!["e".into()],
                error_latents: vec!["bad".into()],
                notice: notice.into(),
            };
            let value = serde_json::to_value(&row).unwrap();
            assert_eq!(
                serde_json::from_value::<ModelUncertainty>(value.clone()).unwrap(),
                row
            );
            let mut mixed = value;
            mixed["unknown"] = serde_json::json!(true);
            assert!(serde_json::from_value::<ModelUncertainty>(mixed).is_err());
        }
    }

    #[test]
    fn must_finish_dataflow_handles_conditional_continuations_and_any_cancellation() {
        let m = model();
        let call = |i| ir::Statement {
            span: m.process.body[0].span,
            kind: ir::Stmt::Call(ir::Call {
                args: vec![],
                binding: ir::BindingId(i),
                capability: ir::CapabilityId(0),
                span: m.process.body[0].span,
            }),
        };
        let stmt = |kind| ir::Statement {
            kind,
            span: m.process.body[0].span,
        };
        let fork = |join| {
            stmt(ir::Stmt::Fork {
                id: ir::ForkId(0),
                branches: [1, 2]
                    .into_iter()
                    .map(|i| {
                        let ir::Stmt::Call(c) = call(i).kind else {
                            unreachable!()
                        };
                        c
                    })
                    .collect(),
                join,
            })
        };
        let any = |remaining| ir::Join::Any {
            matches: vec![],
            tie: ir::Tie::Nondet,
            remaining,
            if_none: ir::EndId(0),
        };
        let run_flow = |block: ir::Block, limits| {
            let mut b = Budget {
                limits,
                work: 0,
                cells: 0,
                contexts: 0,
                paths: 0,
                location: Location::model(&m, "common_cause"),
            };
            b.cells(4)?;
            let mut before = vec![BTreeSet::new(); 4];
            before_start(&block, Some(BTreeSet::new()), &mut before, &mut b, 0)?;
            Ok::<_, Error>(before)
        };
        for (join, expected) in [
            (ir::Join::All, vec![0, 1, 2]),
            (any(ir::Remaining::Drain), vec![0, 1, 2]),
            (any(ir::Remaining::CancelThenDrain), vec![0]),
        ] {
            let before = run_flow(vec![call(0), fork(join), call(3)], LIMITS).unwrap();
            assert_eq!(before[1], BTreeSet::from([ir::BindingId(0)]));
            assert_eq!(
                before[2], before[1],
                "fork siblings never precede each other"
            );
            assert_eq!(before[3], expected.into_iter().map(ir::BindingId).collect());
        }
        for (else_body, expected) in [
            (vec![call(2)], vec![0]),
            (vec![stmt(ir::Stmt::End(ir::EndId(0)))], vec![0, 1]),
        ] {
            let block = vec![
                call(0),
                stmt(ir::Stmt::If {
                    condition: ir::Cond::Eq(ir::ResultMatch {
                        binding: ir::BindingId(0),
                        value: ir::ValueId(0),
                    }),
                    then_body: vec![call(1)],
                    else_body,
                }),
                call(3),
            ];
            let before = run_flow(block.clone(), LIMITS).unwrap();
            assert_eq!(before[3], expected.into_iter().map(ir::BindingId).collect());
            for field in ["work", "cells"] {
                let with = |k| {
                    let mut l = LIMITS;
                    if field == "work" {
                        l.work = k;
                    } else {
                        l.cells = k;
                    }
                    run_flow(block.clone(), l)
                };
                let k = (1..256).find(|k| with(*k).is_ok()).unwrap();
                assert_eq!(with(k).unwrap(), before);
                assert!(
                    with(k - 1).is_err(),
                    "{field}: added control analysis is bounded"
                );
            }
        }
        let before = run_flow(
            vec![stmt(ir::Stmt::End(ir::EndId(0))), call(0), call(3)],
            LIMITS,
        )
        .unwrap();
        assert!(
            before.iter().all(BTreeSet::is_empty),
            "dead lexical suffix proves no ordering"
        );
    }

    fn model() -> ir::Model {
        crate::lower::lower(
            &crate::syntax::parse(include_str!("../../../../fixtures/lin-basis/model.relca"))
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn common_cause_limit_withdraws_only_dependent_output_and_not_invalid_ir() {
        let m = model();
        let mut agency = super::super::derive(&m).unwrap().0;
        let origin = agency.origin.clone();
        let refusal = agency.refusal.clone();
        let mut indexed = vec![None; m.process.bindings.len()];
        collect(
            &m.process.body,
            &mut indexed,
            &m,
            0,
            &mut viewpoints::budget(DEFAULT),
        )
        .unwrap();
        let calls: Vec<_> = indexed.into_iter().collect::<Option<_>>().unwrap();
        let ancestors = vec![Ancestors::Reference(BTreeSet::new()); calls.len()];
        let subjects = vec![BTreeSet::new(); calls.len()];
        let mut warnings = Vec::new();
        soft_with_limits(
            &m,
            &calls,
            &ancestors,
            &subjects,
            &mut agency,
            &mut warnings,
            Limits { work: 0, ..LIMITS },
        )
        .unwrap();
        assert_eq!(agency.origin, origin);
        assert_eq!(agency.refusal, refusal);
        assert!(agency.model_uncertainties.is_none());
        assert!(warnings.is_empty());
        let CommonCauseDerivation::NotDerived(l) = agency.common_causes.as_ref().unwrap() else {
            panic!();
        };
        assert_eq!(l.limit.counter, "work");
        assert_eq!(l.limit.phase, "common_cause");
        let mut bad = m.clone();
        bad.actors[0].metadata.source_description = Some("not tacit".into());
        assert!(matches!(
            soft_with_limits(
                &bad,
                &calls,
                &ancestors,
                &subjects,
                &mut agency,
                &mut warnings,
                Limits { work: 0, ..LIMITS }
            ),
            Err(Error::InvalidIr(_))
        ));
    }

    fn run(m: &ir::Model, limits: Limits) -> Result<(Agency, Vec<diag::Diagnostic>)> {
        let mut old = Budget {
            limits: DEFAULT,
            work: 0,
            cells: 0,
            contexts: 0,
            paths: 0,
            location: Location::model(m, "common_cause"),
        };
        let mut indexed = vec![None; m.process.bindings.len()];
        collect(&m.process.body, &mut indexed, m, 0, &mut old)?;
        let calls: Vec<_> = indexed.into_iter().collect::<Option<_>>().unwrap();
        let mut ancestors = vec![BTreeSet::new(); calls.len()];
        let mut sources = vec![BTreeSet::new(); calls.len()];
        let mut subjects = sources.clone();
        // These unit fixtures have sequential, topologically ordered bindings.
        for (i, c) in calls.iter().enumerate() {
            for arg in &c.args {
                assert!(arg.0 < i);
                let previous = ancestors[arg.0].clone();
                ancestors[i].extend(previous);
                ancestors[i].insert(*arg);
                subjects[i].extend(sources[arg.0].iter().copied());
            }
            sources[i] = subjects[i].clone();
            sources[i].extend(&m.capabilities[c.capability.0].reads);
        }
        let mut agency = Agency::empty();
        let mut warnings = Vec::new();
        let ancestors: Vec<_> = ancestors.into_iter().map(Ancestors::Reference).collect();
        with_limits(
            m,
            &calls,
            &ancestors,
            &subjects,
            &mut agency,
            &mut warnings,
            limits,
        )?;
        Ok((agency, warnings))
    }

    #[test]
    fn state_type_validation_accepts_tagged_domains_but_rejects_invalid_indices() {
        let m = model();
        let expected = run(&m, LIMITS).unwrap();
        for (set_size, integer_max, base, initial) in [
            (Some(1), None, ir::CASE_ID_TYPE, ir::StateValue::Set(0)),
            (
                None,
                Some(3),
                ir::TypeId(usize::MAX),
                ir::StateValue::Integer(0),
            ),
        ] {
            let mut valid = m.clone();
            valid.state_variables.push(ir::StateVariable {
                last_write_wins: false,
                case_set_size: set_size,
                integer_max,
                name: "unrelated_state".into(),
                kind: if set_size.is_some() {
                    ir::StateKind::ActorObserved(ir::ActorId(0))
                } else {
                    ir::StateKind::ObservedCase
                },
                base,
                optional: false,
                initial: ir::InitialValue::Fixed(initial),
                span: m.state_variables[0].span,
            });
            assert_eq!(run(&valid, LIMITS).unwrap(), expected);
            assert_eq!(
                super::super::derive(&valid).unwrap(),
                super::super::derive(&m).unwrap()
            );
            for bad_base in [ir::TypeId(m.types.len()), ir::TypeId(usize::MAX - 2)] {
                let mut bad = valid.clone();
                bad.state_variables.last_mut().unwrap().base = bad_base;
                assert_eq!(
                    run(&bad, LIMITS).unwrap_err().diagnostic().code,
                    diag::U_SEM_0
                );
                assert_eq!(super::super::derive(&bad).unwrap_err().code, diag::U_SEM_0);
            }
            let mut mixed = valid.clone();
            let v = mixed.state_variables.last_mut().unwrap();
            v.case_set_size = Some(1);
            v.integer_max = Some(3);
            assert_eq!(
                run(&mixed, LIMITS).unwrap_err().diagnostic().code,
                diag::U_SEM_0
            );
            let mut wrong_tag = valid;
            wrong_tag.state_variables.last_mut().unwrap().base = if set_size.is_some() {
                ir::TypeId(usize::MAX)
            } else {
                ir::CASE_ID_TYPE
            };
            assert_eq!(
                run(&wrong_tag, LIMITS).unwrap_err().diagnostic().code,
                diag::U_SEM_0
            );
        }
        for bad_base in [
            ir::TypeId(m.types.len()),
            ir::TypeId(usize::MAX - 2),
            ir::CASE_ID_TYPE,
            ir::TypeId(usize::MAX),
        ] {
            let mut bad = m.clone();
            bad.state_variables[0].base = bad_base;
            assert_eq!(
                run(&bad, LIMITS).unwrap_err().diagnostic().code,
                diag::U_SEM_0
            );
            assert_eq!(super::super::derive(&bad).unwrap_err().code, diag::U_SEM_0);
        }
    }

    #[test]
    fn graph_work_cells_and_depth_have_exact_k_k_plus_one_boundaries() {
        let m = model();
        let expected = run(&m, LIMITS).unwrap();
        assert_eq!(
            (LIMITS.work, LIMITS.cells, LIMITS.depth),
            (1_000_000, 1_000_000, 128)
        );
        for field in ["work", "cells", "depth"] {
            let with = |k| {
                let mut l = LIMITS;
                match field {
                    "work" => l.work = k,
                    "cells" => l.cells = k,
                    _ => l.depth = k,
                }
                l
            };
            let k = (0..1024).find(|k| run(&m, with(*k)).is_ok()).unwrap();
            assert!(k > 0, "{field} is charged");
            assert_eq!(run(&m, with(k)).unwrap(), expected, "{field} exact k");
            assert_eq!(
                run(&m, with(k + 1)).unwrap(),
                expected,
                "{field} spare unit"
            );
            assert_eq!(
                run(&m, with(k - 1)).unwrap_err().diagnostic().code,
                diag::U_SEM_0,
                "{field} k+1 required units do not fit k"
            );
            let Error::Limit(l) = run(&m, with(k - 1)).unwrap_err() else {
                panic!("expected typed common_cause limit");
            };
            assert_eq!(l.counter, field);
            assert_eq!(l.limit, k - 1);
            assert!(l.requested.unwrap() >= k);
            assert_eq!(l.phase, "common_cause");
            assert_eq!(l.process, m.process.name);
            assert!(l.span.is_some());
        }
        // Check the actual production counter ceilings, not only tiny injected limits.
        let mut b = Budget {
            limits: LIMITS,
            work: 0,
            cells: 0,
            contexts: 0,
            paths: 0,
            location: Location::model(&m, "common_cause"),
        };
        b.steps(LIMITS.work).unwrap();
        assert!(b.work().is_err());
        b.cells(LIMITS.cells).unwrap();
        assert!(b.cells(1).is_err());
        let mut overflow = Budget {
            limits: Limits {
                work: usize::MAX,
                cells: usize::MAX,
                ..LIMITS
            },
            work: usize::MAX,
            cells: usize::MAX,
            contexts: 0,
            paths: 0,
            location: Location::model(&m, "common_cause"),
        };
        assert!(overflow.work().is_err());
        assert!(overflow.cells(1).is_err());
        let mut product = Budget {
            limits: LIMITS,
            work: 0,
            cells: 0,
            contexts: 0,
            paths: 0,
            location: Location::model(&m, "common_cause"),
        };
        assert!(table_cells(&mut product, usize::MAX, 5).is_err());
        assert_eq!(
            product.cells, 0,
            "overflow must precede allocation/charging"
        );
    }

    #[test]
    fn dependency_depth_128_accepts_129_rejects_and_shared_ancestors_are_memoized() {
        let mut m = model();
        let ir::Stmt::Call(mut template) = m.process.body[1].kind.clone() else {
            panic!()
        };
        let mut cap = m.capabilities[template.capability.0].clone();
        cap.params[0].ty = cap.return_type;
        let cid = ir::CapabilityId(m.capabilities.len());
        m.capabilities.push(cap);
        template.capability = cid;
        m.process.body.clear();
        m.process.bindings.clear();
        for i in 0..=129 {
            let mut c = template.clone();
            c.binding = ir::BindingId(i);
            c.args = if i == 0 {
                vec![]
            } else {
                vec![ir::BindingId(i - 1)]
            };
            m.process.bindings.push(ir::Binding {
                merge: None,
                repetition: None,
                name: format!("v{i}"),
                ty: m.capabilities[cid.0].return_type,
                span: c.span,
            });
            m.process.body.push(ir::Statement {
                span: c.span,
                kind: ir::Stmt::Call(c),
            });
            if i == 128 {
                assert!(run(&m, LIMITS).is_ok());
            }
        }
        assert_eq!(
            run(&m, LIMITS).unwrap_err().diagnostic().code,
            diag::U_SEM_0
        );
    }

    #[test]
    fn kernel_free_legacy_graph_needs_zero_new_budget_and_preserves_old_boundaries() {
        let m = crate::lower::lower(
            &crate::syntax::parse(include_str!(
                "../../../../fixtures/agency-origin/model.relca"
            ))
            .unwrap(),
        )
        .unwrap();
        assert!(m.capabilities.iter().all(|c| c.kernel.is_none()));
        assert_eq!(
            run(&m, LIMITS).unwrap(),
            run(
                &m,
                Limits {
                    work: 0,
                    cells: 0,
                    depth: 0,
                    ..LIMITS
                }
            )
            .unwrap()
        );
        // The old budget suite exercises exact work/cells/contexts/paths/depth
        // boundaries through derive_with_limits. Also exercise all such edges here
        // with the new pass enabled, without removing it from the public path.
        let expected = super::super::derive(&m).unwrap();
        for field in ["work", "cells", "depth", "paths", "contexts"] {
            let with = |k| {
                let mut l = DEFAULT;
                match field {
                    "work" => l.work = k,
                    "cells" => l.cells = k,
                    "depth" => l.depth = k,
                    "paths" => l.paths = k,
                    _ => l.contexts = k,
                };
                l
            };
            let mut lo = 0;
            let mut hi = 1_000_000;
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if derive_with_limits(&m, with(mid)).is_ok() {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }
            assert!(lo > 0);
            assert_eq!(derive_with_limits(&m, with(lo)).unwrap(), expected);
            assert!(derive_with_limits(&m, with(lo - 1)).is_err());
        }
    }
}
