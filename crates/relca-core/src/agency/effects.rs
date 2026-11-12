//! J/U/E is descriptive reach, not Permission or Authority certification.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effects {
    Derived {
        rows: Vec<Effect>,
        unchecked: Vec<String>,
    },
    NotDerived {
        reason: viewpoints::LimitReason,
        unchecked: Vec<String>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Carrier {
    pub actors: Vec<String>,
    pub people: Vec<String>,
    pub unattributed_actors: Vec<String>,
    pub permitted: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Judge {
    pub call: String,
    pub capability: String,
    #[serde(flatten)]
    pub carrier: Carrier,
    pub stops: Stops,
    pub bypasses: usize,
}
/// U can be a process refuser or a fixed Rule constraint. Only process J has
/// a call, a carrier and a counterfactual stopping classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Refuser {
    Process(Judge),
    Rule(RuleJudgement),
}
pub(super) fn decode_refusers<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<Vec<Refuser>>, D::Error> {
    let rows = Option::<Vec<Refuser>>::deserialize(d)?;
    let mut rules = BTreeSet::new();
    for row in rows.iter().flatten() {
        if let Refuser::Rule(r) = row
            && !rules.insert(&r.actor)
        {
            return Err(serde::de::Error::custom("duplicate Rule refuser"));
        }
    }
    Ok(rows)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unknown {
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Truth {
    Known(bool),
    Unknown(Unknown),
}
impl Truth {
    fn unknown() -> Self {
        Self::Unknown(Unknown::Unknown)
    }
    fn yes(&self) -> bool {
        *self == Self::Known(true)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRefuser {
    pub actor: String,
    pub capability: String,
    pub reads: Vec<String>,
    pub on_normal_path: Truth,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Underived {
    pub status: NotDerived,
    pub reason: viewpoints::StateReason,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotDerived {
    NotDerived,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Derived {
    Derived,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProcessJudgements {
    Derived(Derived),
    NotDerived(Underived),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Roots {
    Derived(Vec<ViaCall>),
    NotDerived(Underived),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Concentration {
    pub single_actor: Truth,
    pub single_person: Truth,
    pub single_generation_root: Truth,
    pub generation_roots: Roots,
    pub signal: String,
    pub mechanism: String,
    pub check: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub effect: String,
    pub call: String,
    pub status: Derived,
    pub e: Carrier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub j: Option<Vec<Judge>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "decode_refusers"
    )]
    pub u: Option<Vec<Refuser>>,
    pub state_refusers: Vec<StateRefuser>,
    pub process_judgements: ProcessJudgements,
    pub concentration: Concentration,
}
fn underived() -> Underived {
    Underived {
        status: NotDerived::NotDerived,
        reason: viewpoints::StateReason::StateGuards,
    }
}
fn carrier(m: &ir::Model, c: &ir::Capability, b: &mut Budget) -> Result<Carrier> {
    let a = m.actors.get(c.actor.0).ok_or_else(invalid)?;
    b.copy(2)?;
    let mut out = Carrier {
        actors: vec![a.name.clone()],
        people: Vec::new(),
        unattributed_actors: Vec::new(),
        permitted: Vec::new(),
    };
    if a.executors.is_none() {
        b.copy(1)?;
        out.unattributed_actors.push(a.name.clone());
    }
    let permit = c.permission.as_ref().or(a.executors.as_ref());
    for (i, p) in m.people.iter().enumerate() {
        b.work()?;
        if a.executors
            .as_ref()
            .is_some_and(|ps| ps.contains(&ir::PersonId(i)))
        {
            b.copy(1)?;
            out.people.push(p.name.clone());
        }
        if permit.is_some_and(|ps| ps.contains(&ir::PersonId(i))) {
            b.copy(1)?;
            out.permitted.push(p.name.clone());
        }
    }
    Ok(out)
}
fn inserts(c: &ir::Capability, set: ir::StateVarId) -> bool {
    c.start_writes
        .iter()
        .chain(c.finish_writes())
        .any(|w| w.variable == set && matches!(w.source, ir::WriteSource::InsertCase { .. }))
}
fn sibling_inserts(
    body: &ir::Block,
    id: ir::BindingId,
    set: ir::StateVarId,
    m: &ir::Model,
) -> bool {
    body.iter().any(|s| match &s.kind {
        ir::Stmt::Response(_, _) => false, // A sequential attempt has no fork siblings.
        ir::Stmt::Fork { branches, .. } if branches.iter().any(|c| c.binding == id) => branches
            .iter()
            .any(|c| c.binding != id && inserts(&m.capabilities[c.capability.0], set)),
        ir::Stmt::If {
            then_body,
            else_body,
            ..
        } => sibling_inserts(then_body, id, set, m) || sibling_inserts(else_body, id, set, m),
        _ => false,
    })
}
fn state_reads(
    c: &ir::Cond,
    out: &mut BTreeSet<ir::StateVarId>,
    b: &mut Budget,
    depth: usize,
) -> Result<()> {
    viewpoints::depth(b, depth)?;
    match c {
        ir::Cond::Contains { set, .. } => b.insert(out, *set)?,
        ir::Cond::StateEq(x) => b.insert(out, x.variable)?,
        ir::Cond::IntCompare { variable, .. } => b.insert(out, *variable)?,
        ir::Cond::And(cs) | ir::Cond::Or(cs) => {
            for c in cs {
                state_reads(c, out, b, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn atomic_refuser(
    m: &ir::Model,
    c: &ir::Call,
    calls: &[&ir::Call],
    b: &mut Budget,
) -> Result<Option<StateRefuser>> {
    let cap = &m.capabilities[c.capability.0];
    // ADR 0043 (boundary-review ruling M1): an ADR 0041 version_check makes the
    // owner a state refuser too. Its reject is decided by the shared history
    // at commit, so it never certifies on_normal_path false.
    let versioned = cap.version_check.as_ref();
    if cap.atomic.is_none() && versioned.is_none() {
        return Ok(None);
    }
    let mut vars = BTreeSet::new();
    if let Some(a) = &cap.atomic {
        state_reads(&a.condition, &mut vars, b, 0)?;
    }
    for v in versioned.iter().flat_map(|v| &v.variables) {
        b.insert(&mut vars, *v)?;
    }
    let mechanism = versioned.is_none()
        && match cap.atomic.as_ref().map(|a| (a, &a.condition)) {
            Some((
                a,
                ir::Cond::Contains {
                    set,
                    element: ir::Element::Parameter(parameter),
                },
            )) => {
                let v = m.state_variables.get(set.0).ok_or_else(invalid)?;
                !a.commit_when && v.kind == ir::StateKind::ActorObserved(cap.actor)
                && v.case_set_size.is_some() && v.initial == ir::InitialValue::Fixed(ir::StateValue::Set(0))
                && cap.params.get(*parameter).is_some_and(|p| p.ty == ir::CASE_ID_TYPE)
                && c.args.get(*parameter) == Some(&ir::CURRENT_CASE)
                && a.writes.iter().any(|w| w.variable == *set && matches!(w.source, ir::WriteSource::InsertCase { parameter:p } if p == *parameter))
                && !calls[..c.binding.0].iter().any(|w| inserts(&m.capabilities[w.capability.0], *set))
                && !cap.start_writes.iter().any(|w| w.variable == *set && matches!(w.source, ir::WriteSource::InsertCase { .. }))
                && !sibling_inserts(&m.process.body,c.binding,*set,m)
            }
            _ => false,
        };
    let mut reads = Vec::new();
    for id in vars {
        b.copy(1)?;
        let v = &m.state_variables[id.0];
        reads.push(format!("{}.{}", m.actors[cap.actor.0].name, v.name));
    }
    b.copy(3)?;
    Ok(Some(StateRefuser {
        actor: m.actors[cap.actor.0].name.clone(),
        capability: viewpoints::cap_name(m, c.capability)?,
        reads,
        on_normal_path: if mechanism {
            Truth::Known(false)
        } else {
            Truth::unknown()
        },
    }))
}

/// ADR0009 counterfactual evaluator for the extra atomic block only, followed
/// by the fixed Rule constraints (not counterfactual judgements). The old
/// refusal rows, warnings, and their budget never pass through this function.
fn atomic_judgements(
    m: &ir::Model,
    effect: &ir::Call,
    calls: &[&ir::Call],
    ancestors: &[BTreeSet<ir::BindingId>],
    extra: &[Vec<ir::BindingId>],
    b: &mut Budget,
) -> Result<Option<Vec<Judgement>>> {
    let mut effects = BTreeMap::new();
    paths(
        &m.process.body,
        vec![Path::default()],
        &mut effects,
        m,
        b,
        None,
        0,
    )?;
    let ps = effects
        .get(&effect.binding)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if ps.iter().any(|p| viewpoints::uncertain(p)) {
        return Ok(None);
    }
    let mut support = BTreeSet::new();
    let mut reachable = Vec::new();
    for p in ps {
        let local = path_support(p, m, b)?;
        b.copy(local.len())?;
        let vars: Vec<_> = local.iter().copied().collect();
        if assignments(&vars, &Values::new(), calls, m, b, |v, b| {
            satisfied(p, v, b)
        })? {
            for id in &local {
                b.insert(&mut support, *id)?;
            }
            b.copy(1)?;
            reachable.push((p, local));
        }
    }
    let mut out = Vec::new();
    for j in ancestors[effect.binding.0].union(&support) {
        b.work()?;
        if !calls[j.0].args.iter().any(|a| *a != ir::CURRENT_CASE) && extra[j.0].is_empty() {
            continue;
        }
        if reachable.is_empty() {
            continue;
        }
        let mut contexts = BTreeSet::new();
        for (p, _) in &reachable {
            b.copy(support.len())?;
            let vars: Vec<_> = support.intersection(&p.defined).copied().collect();
            assignments(&vars, &Values::new(), calls, m, b, |v, b| {
                if satisfied(p, v, b)? {
                    b.copy(v.len())?;
                    let context: Values = v
                        .iter()
                        .filter(|(id, _)| **id != *j)
                        .map(|(id, v)| (*id, *v))
                        .collect();
                    if !contexts.contains(&context) {
                        b.context()?;
                        b.cells(1)?;
                        contexts.insert(context);
                    }
                }
                Ok(false)
            })?;
        }
        let mut stopping = 0;
        let mut bypasses = 0;
        for context in contexts {
            let mut may = false;
            let mut cannot = false;
            for value in &m.capabilities[calls[j.0].capability.0].finish_results() {
                let mut reaches = false;
                for (p, local) in &reachable {
                    b.copy(context.len() + local.len() + 1)?;
                    let mut fixed: Values = context
                        .iter()
                        .filter(|(id, _)| p.defined.contains(id))
                        .map(|(id, v)| (*id, *v))
                        .collect();
                    fixed.insert(*j, *value);
                    let vars: Vec<_> = local.iter().copied().collect();
                    if assignments(&vars, &fixed, calls, m, b, |v, b| satisfied(p, v, b))? {
                        reaches = true;
                        break;
                    }
                }
                may |= reaches;
                cannot |= !reaches;
            }
            if may {
                if cannot {
                    stopping += 1;
                } else {
                    bypasses += 1;
                }
            }
        }
        b.copy(3)?;
        out.push(Judgement::Process(ProcessJudgement {
            call: m.process.bindings[j.0].name.clone(),
            capability: viewpoints::cap_name(m, calls[j.0].capability)?,
            stops: if stopping == 0 {
                Stops::None
            } else if bypasses == 0 {
                Stops::AllPaths
            } else {
                Stops::Conditional
            },
            bypasses,
        }));
    }
    // Rule U is normative path support, not a person's counterfactual ability.
    // Use all lexical paths, including ones the selected Rule makes unreachable.
    // Keep Actor/value declaration order and append after the process items.
    for actor in &m.actors {
        if let Some(rule) = &actor.metadata.rule {
            b.work()?;
            let mut reads = Vec::new();
            for v in &rule.values {
                b.work()?;
                let mut used = false;
                for p in ps {
                    b.work()?;
                    used |= p.rule_sources.contains(&v.variable);
                }
                if used {
                    b.copy(1)?;
                    reads.push(format!(
                        "{}.{}",
                        actor.name, m.state_variables[v.variable.0].name
                    ));
                }
            }
            if !reads.is_empty() {
                b.copy(3)?;
                out.push(Judgement::Rule(RuleJudgement {
                    kind: RuleKind::Rule,
                    actor: actor.name.clone(),
                    version: rule.effective_version.clone(),
                    reads,
                }));
            }
        }
    }
    Ok(Some(out))
}

pub(super) fn derive(
    m: &ir::Model,
    a: &Agency,
    limits: Limits,
) -> (Effects, Vec<diag::Diagnostic>) {
    let mut unchecked = vec!["authority".into(), "fallback_path".into()];
    fn state_only(body: &ir::Block, m: &ir::Model) -> bool {
        let cap_state_only = |c: &ir::Call| {
            m.capabilities.get(c.capability.0).is_some_and(|cap| {
                !cap.commits
                    && cap
                        .start_writes
                        .iter()
                        .chain(cap.finish_writes())
                        .map(|w| w.variable)
                        .chain(cap.kernel_writes.iter().copied())
                        .chain(
                            cap.structural_kernel()
                                .into_iter()
                                .flat_map(|k| &k.rows)
                                .flat_map(|r| &r.branches)
                                .flat_map(|b| &b.overrides)
                                .map(|(v, _)| *v),
                        )
                        .any(|id| {
                            m.state_variables
                                .get(id.0)
                                .is_some_and(|v| matches!(v.kind, ir::StateKind::ActorObserved(_)))
                        })
            })
        };
        body.iter().any(|s| match &s.kind {
            ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => cap_state_only(c),
            ir::Stmt::Fork { branches, .. } => branches.iter().any(cap_state_only),
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => state_only(then_body, m) || state_only(else_body, m),
            _ => false,
        })
    }
    if state_only(&m.process.body, m) {
        unchecked.push("state_write_effects".into());
    }
    match rows(m, a, &mut viewpoints::budget(limits)) {
        Ok((rows, warnings)) => (Effects::Derived { rows, unchecked }, warnings),
        Err(_) => (
            Effects::NotDerived {
                reason: viewpoints::LimitReason::DerivationLimit,
                unchecked,
            },
            Vec::new(),
        ),
    }
}
fn rows(m: &ir::Model, a: &Agency, b: &mut Budget) -> Result<(Vec<Effect>, Vec<diag::Diagnostic>)> {
    viewpoints::depth(b, 3)?;
    b.copy(m.process.bindings.len())?;
    let mut indexed = vec![None; m.process.bindings.len()];
    collect(&m.process.body, &mut indexed, m, 0, b)?;
    let calls = indexed
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)?;
    for c in &calls {
        let cap = m.capabilities.get(c.capability.0).ok_or_else(invalid)?;
        for id in cap
            .start_writes
            .iter()
            .chain(cap.finish_writes())
            .map(|w| w.variable)
            .chain(cap.kernel_writes.iter().copied())
            .chain(
                cap.structural_kernel()
                    .into_iter()
                    .flat_map(|k| &k.rows)
                    .flat_map(|r| &r.branches)
                    .flat_map(|b| &b.overrides)
                    .map(|(v, _)| *v),
            )
        {
            b.work()?;
            m.state_variables.get(id.0).ok_or_else(invalid)?;
        }
    }
    let extra = read_graph::read_parents(m, &calls)?;
    b.copy(calls.len().checked_mul(2).ok_or_else(invalid)?)?;
    let mut ancestors = vec![BTreeSet::new(); calls.len()];
    let mut sources = vec![BTreeSet::new(); calls.len()];
    // Lowered calls are lexical. A topological relaxation also supports public
    // IR argument order, rejecting cycles rather than indexing a partial cache.
    let mut done = BTreeSet::new();
    while done.len() < calls.len() {
        let mut progress = false;
        for c in &calls {
            b.work()?;
            if done.contains(&c.binding) {
                continue;
            }
            let parents = c
                .args
                .iter()
                .chain(&extra[c.binding.0])
                .filter(|id| **id != ir::CURRENT_CASE);
            if !parents.clone().all(|id| done.contains(id)) {
                continue;
            }
            for id in parents {
                b.copy(ancestors[id.0].len() + sources[id.0].len() + 1)?;
                let inherited = ancestors[id.0].clone();
                ancestors[c.binding.0].extend(inherited);
                ancestors[c.binding.0].insert(*id);
                let inherited = sources[id.0].clone();
                sources[c.binding.0].extend(inherited);
            }
            for id in &m.capabilities[c.capability.0].reads {
                b.insert(&mut sources[c.binding.0], *id)?;
            }
            b.insert(&mut done, c.binding)?;
            progress = true;
        }
        if !progress {
            return Err(invalid());
        }
    }
    let mut rows = Vec::new();
    let mut warnings = Vec::new();
    for c in &calls {
        b.work()?;
        let cap = &m.capabilities[c.capability.0];
        if !cap.commits {
            continue;
        }
        let effect = viewpoints::cap_name(m, c.capability)?;
        let name = m.process.bindings[c.binding.0].name.clone();
        b.copy(3)?;
        let e = carrier(m, cap, b)?;
        let state_refusers = atomic_refuser(m, c, &calls, b)?
            .into_iter()
            .collect::<Vec<_>>();
        let judgements = if state_refusers.is_empty() {
            let legacy = a
                .refusal
                .iter()
                .find(|r| r.call == name && r.effect == effect)
                .map(|r| r.judgements.clone());
            if legacy.is_none() && a.rules == "agency-p1-rule-1" {
                // The legacy Rule projection omits a known empty refusal only
                // when this effect's own requires is fixed true on every path.
                // Re-establish that fact here; absence alone is never evidence.
                let mut effect_paths = BTreeMap::new();
                paths(
                    &m.process.body,
                    vec![Path::default()],
                    &mut effect_paths,
                    m,
                    b,
                    None,
                    0,
                )?;
                let ps = effect_paths
                    .get(&c.binding)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                if !ps.is_empty()
                    && ps
                        .iter()
                        .all(|p| p.requires_fixed_by_rule && !viewpoints::uncertain(p))
                {
                    Some(Vec::new())
                } else {
                    None
                }
            } else {
                legacy
            }
        } else if state_refusers[0].on_normal_path == Truth::Known(false) {
            atomic_judgements(m, c, &calls, &ancestors, &extra, b)?
        } else {
            None
        };
        let known = judgements.is_some();
        let mut actors = BTreeSet::new();
        let mut people = BTreeSet::new();
        let mut anonymous = false;
        let mut add = |id: ir::ActorId, b: &mut Budget| -> Result<()> {
            b.insert(&mut actors, id)?;
            match &m.actors[id.0].executors {
                Some(ps) => {
                    for p in ps {
                        b.insert(&mut people, *p)?;
                    }
                }
                None => anonymous = true,
            }
            Ok(())
        };
        add(cap.actor, b)?;
        let mut jrows = Vec::new();
        let mut roots = BTreeSet::new();
        let mut relevant = sources[c.binding.0].clone();
        b.copy(relevant.len())?;
        if let Some(js) = &judgements {
            for j in js.iter().filter_map(Judgement::process) {
                b.work()?;
                let jc = calls
                    .iter()
                    .find(|c| m.process.bindings[c.binding.0].name == j.call)
                    .ok_or_else(invalid)?;
                let jcap = &m.capabilities[jc.capability.0];
                add(jcap.actor, b)?;
                for s in &sources[jc.binding.0] {
                    b.insert(&mut relevant, *s)?;
                }
                for id in &ancestors[jc.binding.0] {
                    b.work()?;
                    if calls[id.0].args.iter().all(|id| *id == ir::CURRENT_CASE)
                        && extra[id.0].is_empty()
                    {
                        b.insert(&mut roots, *id)?;
                    }
                }
                b.copy(3)?;
                jrows.push(Judge {
                    call: j.call.clone(),
                    capability: j.capability.clone(),
                    carrier: carrier(m, jcap, b)?,
                    stops: j.stops,
                    bypasses: j.bypasses,
                });
            }
        }
        let mut rule_u = Vec::new();
        if let Some(js) = &judgements {
            for j in js {
                if let Judgement::Rule(r) = j {
                    // Fixed rules constrain the effect but are not carriers of
                    // its judgement/execution. Counting them dilutes concentration.
                    b.copy(r.reads.len() + 3)?;
                    rule_u.push(Refuser::Rule(r.clone()));
                }
            }
        }
        let single_person = if people.len() >= 2 {
            Truth::Known(false)
        } else if anonymous || !known || !rule_u.is_empty() {
            Truth::unknown()
        } else {
            Truth::Known(people.len() == 1)
        };
        // O contributes Actors, not people, to the Actor concentration signal.
        for call in &calls {
            b.work()?;
            let cap = &m.capabilities[call.capability.0];
            if cap.reads.iter().any(|s| relevant.contains(s)) {
                b.insert(&mut actors, cap.actor)?;
            }
        }
        let has_u = !state_refusers.is_empty()
            || !rule_u.is_empty()
            || jrows.iter().any(|j| j.stops != Stops::None);
        let single_actor = if actors.len() > 1 {
            Truth::Known(false)
        } else if !known {
            Truth::unknown()
        } else {
            Truth::Known(actors.len() == 1 && has_u)
        };
        let generation_roots = if known {
            let mut out = Vec::new();
            for id in roots {
                b.copy(3)?;
                out.push(ViaCall {
                    call: m.process.bindings[id.0].name.clone(),
                    capability: viewpoints::cap_name(m, calls[id.0].capability)?,
                });
            }
            Roots::Derived(out)
        } else {
            Roots::NotDerived(underived())
        };
        let single_generation_root = match &generation_roots {
            Roots::Derived(rs) => Truth::Known(rs.len() == 1),
            _ => Truth::unknown(),
        };
        if single_actor.yes() || single_person.yes() {
            b.copy(1)?;
            warnings.push(diag::Diagnostic { code:diag::W_AGY_2.into(), severity:diag::Severity::Warning,
                message:format!("{effect} の判定・拒否・実行が {} に集まる(兆候)。想定機構: 単独の主体が自ら判定して実行する。確認: モデル外の統制とログ",if single_actor.yes() { "Actor" } else { "人" }),span:Some(c.span) });
        }
        let u = if known {
            b.copy(jrows.len())?;
            let mut u: Vec<_> = jrows
                .iter()
                .filter(|j| j.stops != Stops::None)
                .cloned()
                .map(Refuser::Process)
                .collect();
            u.extend(rule_u);
            Some(u)
        } else {
            None
        };
        b.copy(8)?;
        rows.push(Effect {
            effect,
            call: name,
            status: Derived::Derived,
            e,
            j: known.then_some(jrows),
            u,
            state_refusers,
            process_judgements: if known {
                ProcessJudgements::Derived(Derived::Derived)
            } else {
                ProcessJudgements::NotDerived(underived())
            },
            concentration: Concentration {
                single_actor,
                single_person,
                single_generation_root,
                generation_roots,
                signal: "判定・拒否・実行の担い手と生成根の集中の兆候".into(),
                mechanism: "同じ担い手又は生成根への依存が構成上重なる".into(),
                check: "モデル外の統制・原本参照・ログで確認する".into(),
            },
        });
    }
    Ok((rows, warnings))
}
