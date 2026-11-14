//! Bounded, analysis-only branch substitution. Never returned to either
//! interpreter: execution keeps one suffix site, zero-time copies and its RNG.
//! No fictitious calls/Actors or evidence sources are introduced.
use super::*;

pub(crate) struct Projection {
    pub model: ir::Model,
    pub capabilities: BTreeMap<ir::BindingId, ir::CapabilityId>,
    pub spans: BTreeMap<ir::BindingId, diag::Span>,
    pub data: BTreeSet<ir::BindingId>,
    pub control: BTreeSet<ir::BindingId>,
    pub ends: BTreeSet<ir::EndId>,
    pub variables: BTreeSet<ir::StateVarId>,
    pub warnings: Vec<diag::Diagnostic>,
    names: BTreeMap<String, ir::BindingId>,
}

impl Projection {
    pub fn affected(&self, id: ir::BindingId) -> bool {
        self.data.contains(&id) || self.control.contains(&id)
    }
    pub fn id(&self, name: &str) -> Option<ir::BindingId> {
        self.names.get(name).copied()
    }
    pub fn name(&self, m: &ir::Model, name: &str) -> String {
        self.id(name)
            .map_or_else(|| name.to_string(), |id| m.site_name(id))
    }
    pub fn message(&self, m: &ir::Model, message: &str) -> String {
        let mut out = message.to_string();
        for (name, id) in &self.names {
            out = out.replace(name, &m.site_name(*id));
        }
        out
    }
    pub fn mentions_affected(&self, text: &str) -> bool {
        self.names
            .iter()
            .any(|(name, id)| text.contains(name) && self.affected(*id))
    }
    pub fn cap_affected(&self, cap: ir::CapabilityId) -> bool {
        self.capabilities
            .iter()
            .any(|(id, c)| *c == cap && self.affected(*id))
    }
}

pub(crate) fn projection(m: &ir::Model) -> std::result::Result<Projection, &'static str> {
    raw(m).map_err(|e| match e {
        Error::Limit(_) => "derivation_limit",
        Error::InvalidIr(_) => "merge_control",
    })
}

pub(super) fn raw(m: &ir::Model) -> Result<Projection> {
    // Expansion is a static proof aid, not the executable model. Bound its
    // recursive frames independently of the parser and runtime depth limits.
    let mut limits = DEFAULT;
    limits.depth = 32;
    let mut b = viewpoints::budget(limits);
    b.location = Location::model(m, "merge independent projection");
    let mut indexed = vec![None; m.process.bindings.len()];
    collect(&m.process.body, &mut indexed, m, 0, &mut b)?;
    let calls: Vec<_> = indexed.iter().flatten().copied().collect();
    let extra = read_graph::read_parents(m, &calls)?;
    let mut data: BTreeSet<_> = m
        .process
        .bindings
        .iter()
        .enumerate()
        .filter_map(|(i, x)| x.merge.as_ref().map(|_| ir::BindingId(i)))
        .collect();
    let mut control = BTreeSet::new();
    let mut ends = BTreeSet::new();
    let mut variables = BTreeSet::new();
    let mut channels = Vec::new();
    let mut pending: Vec<_> = m.process.body.iter().collect();
    while let Some(s) = pending.pop() {
        match &s.kind {
            ir::Stmt::Query(c, command) => channels.push((*command, c.binding)),
            ir::Stmt::Response(c, r) => {
                channels.push((c.binding, r.command));
                if let Some(handler) = &r.recovery {
                    pending.extend(handler);
                }
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                pending.extend(then_body);
                pending.extend(else_body);
            }
            _ => {}
        }
    }
    loop {
        let before = (data.len(), control.len(), variables.len());
        for c in &calls {
            b.work()?;
            let cap = &m.capabilities[c.capability.0];
            let mut reads: BTreeSet<_> = cap.read_variables.iter().copied().collect();
            if let Some(r) = &cap.requires {
                state_reads(&r.condition, &mut reads);
            }
            for k in cap.result_kernels() {
                reads.extend(k.given.iter().copied());
            }
            if let Some(a) = &cap.atomic {
                state_reads(&a.condition, &mut reads);
            }
            if c.args
                .iter()
                .chain(&extra[c.binding.0])
                .any(|id| data.contains(id))
                || !reads.is_disjoint(&variables)
            {
                b.insert(&mut data, c.binding)?;
            }
            if data.contains(&c.binding) || control.contains(&c.binding) {
                for id in cap
                    .start_writes
                    .iter()
                    .chain(cap.finish_writes())
                    .map(|w| w.variable)
                    .chain(cap.kernel_writes.iter().copied())
                {
                    b.insert(&mut variables, id)?;
                }
                if let Some(alias) = m.loop_alias(c.binding) {
                    b.insert(&mut data, alias)?;
                }
                if let Some(r) = m.response_attempt(c.binding) {
                    b.insert(&mut data, r.command)?;
                }
            }
        }
        for (from, to) in &channels {
            if data.contains(from) || control.contains(from) {
                b.insert(&mut data, *to)?;
            }
        }
        controls(
            &m.process.body,
            false,
            &data,
            &variables,
            &mut control,
            &mut ends,
            &mut b,
            0,
        )?;
        if before == (data.len(), control.len(), variables.len()) {
            break;
        }
    }
    let mut builder = Builder {
        source: m,
        bindings: Vec::new(),
        original: Vec::new(),
        cached: BTreeMap::new(),
        base: BTreeMap::new(),
        used: BTreeSet::new(),
        data: &data,
        budget: b,
    };
    // Allocate stable sites before expanding alternatives. Read-parent order
    // and one judgement per independent original site are preserved.
    for c in &calls {
        let id = builder.allocate(c.binding, &BTreeMap::new())?;
        builder.base.insert(c.binding, id);
        if !data.contains(&c.binding) {
            builder.cached.insert(c.binding, id);
        }
    }
    let mut env = builder.cached.clone();
    let base_env = builder.base.clone();
    for (i, old) in builder.original.clone().iter().enumerate() {
        builder.loop_origin(ir::BindingId(i), *old, &base_env);
    }
    let seq = m.process.body.iter().collect::<Vec<_>>();
    let body = builder.block(&seq, &mut env, 0)?;
    let mut model = m.clone();
    model.process.body = body;
    model.process.bindings = builder.bindings;
    model.process.degradations.clear(); // Annotation output remains on the original model.
    model.agency = Agency::empty().into();
    let (a, warnings) =
        derive_strategy_shared(&model, DEFAULT, [DEFAULT; 3], DEFAULT, None, false, true)?;
    model.agency = a.into();
    let names = model
        .process
        .bindings
        .iter()
        .zip(&builder.original)
        .map(|(n, old)| (n.name.clone(), *old))
        .collect();
    let capabilities = calls.iter().map(|c| (c.binding, c.capability)).collect();
    let spans = calls.iter().map(|c| (c.binding, c.span)).collect();
    Ok(Projection {
        model,
        capabilities,
        spans,
        data,
        control,
        ends,
        variables,
        warnings,
        names,
    })
}

fn state_reads(c: &ir::Cond, out: &mut BTreeSet<ir::StateVarId>) {
    match c {
        ir::Cond::StateEq(x) => {
            out.insert(x.variable);
        }
        ir::Cond::IntCompare { variable, .. } | ir::Cond::Contains { set: variable, .. } => {
            out.insert(*variable);
        }
        ir::Cond::And(cs) | ir::Cond::Or(cs) => {
            for c in cs {
                state_reads(c, out);
            }
        }
        ir::Cond::Eq(_) => {}
    }
}
fn dependent(
    c: &ir::Cond,
    data: &BTreeSet<ir::BindingId>,
    vars: &BTreeSet<ir::StateVarId>,
) -> bool {
    match c {
        ir::Cond::Eq(x) => data.contains(&x.binding),
        ir::Cond::StateEq(x) => vars.contains(&x.variable),
        ir::Cond::IntCompare { variable, .. } | ir::Cond::Contains { set: variable, .. } => {
            vars.contains(variable)
        }
        ir::Cond::And(cs) | ir::Cond::Or(cs) => cs.iter().any(|c| dependent(c, data, vars)),
    }
}
fn may_stop(block: &ir::Block) -> bool {
    block.iter().any(|s| match &s.kind {
        ir::Stmt::MergeCopy { .. } | ir::Stmt::LoopControl(_) | ir::Stmt::Wait(_) => false,
        ir::Stmt::If {
            then_body,
            else_body,
            ..
        } => may_stop(then_body) || may_stop(else_body),
        _ => true,
    })
}
#[allow(clippy::too_many_arguments)]
fn controls(
    block: &ir::Block,
    mut taint: bool,
    data: &BTreeSet<ir::BindingId>,
    vars: &BTreeSet<ir::StateVarId>,
    out: &mut BTreeSet<ir::BindingId>,
    ends: &mut BTreeSet<ir::EndId>,
    b: &mut Budget,
    depth: usize,
) -> Result<()> {
    b.depth(depth)?;
    for s in block {
        b.work()?;
        match &s.kind {
            ir::Stmt::Call(c) | ir::Stmt::Query(c, _) | ir::Stmt::Response(c, _) => {
                if taint {
                    b.insert(out, c.binding)?;
                }
                if let ir::Stmt::Response(_, r) = &s.kind
                    && let Some(handler) = &r.recovery
                {
                    controls(
                        handler,
                        taint || data.contains(&c.binding),
                        data,
                        vars,
                        out,
                        ends,
                        b,
                        depth + 1,
                    )?;
                }
            }
            ir::Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                let dep = dependent(condition, data, vars);
                controls(then_body, taint || dep, data, vars, out, ends, b, depth + 1)?;
                controls(else_body, taint || dep, data, vars, out, ends, b, depth + 1)?;
                taint |= dep && (may_stop(then_body) || may_stop(else_body));
            }
            ir::Stmt::Fork { branches, .. } => {
                let dep = taint || branches.iter().any(|c| data.contains(&c.binding));
                if dep {
                    for c in branches {
                        b.insert(out, c.binding)?;
                    }
                    taint = true;
                }
            }
            ir::Stmt::End(e) if taint => {
                b.insert(ends, *e)?;
            }
            _ => {}
        }
    }
    Ok(())
}

struct Builder<'a> {
    source: &'a ir::Model,
    bindings: Vec<ir::Binding>,
    original: Vec<ir::BindingId>,
    cached: BTreeMap<ir::BindingId, ir::BindingId>,
    base: BTreeMap<ir::BindingId, ir::BindingId>,
    used: BTreeSet<ir::BindingId>,
    data: &'a BTreeSet<ir::BindingId>,
    budget: Budget,
}
impl Builder<'_> {
    fn loop_origin(
        &mut self,
        new: ir::BindingId,
        old: ir::BindingId,
        env: &BTreeMap<ir::BindingId, ir::BindingId>,
    ) {
        let Some(site) = &self.source.process.bindings[old.0].repetition else {
            return;
        };
        let Some(pub_id) = self
            .source
            .process
            .bindings
            .iter()
            .position(|b| b.name == site.call_site)
            .map(ir::BindingId)
        else {
            return;
        };
        let Some(alias) = env
            .get(&pub_id)
            .copied()
            .or_else(|| (pub_id == old).then_some(new))
        else {
            return;
        };
        if let Some(site) = &mut self.bindings[new.0].repetition {
            site.call_site = format!("@merge-analysis[{}]", alias.0);
        }
    }
    fn allocate(
        &mut self,
        old: ir::BindingId,
        env: &BTreeMap<ir::BindingId, ir::BindingId>,
    ) -> Result<ir::BindingId> {
        self.budget.copy(8)?;
        if self.bindings.len() >= 4096 {
            return Err(self.budget.location.limit(
                "merge_projection_sites",
                4096,
                self.bindings.len() + 1,
            ));
        }
        let mut binding = self
            .source
            .process
            .bindings
            .get(old.0)
            .ok_or_else(invalid)?
            .clone();
        let new = ir::BindingId(self.bindings.len());
        binding.name = format!("@merge-analysis[{}]", new.0);
        binding.merge = None;
        self.bindings.push(binding);
        self.original.push(old);
        self.loop_origin(new, old, env);
        Ok(new)
    }
    fn id(
        env: &BTreeMap<ir::BindingId, ir::BindingId>,
        old: ir::BindingId,
    ) -> Result<ir::BindingId> {
        if old == ir::CURRENT_CASE {
            Ok(old)
        } else {
            env.get(&old).copied().ok_or_else(invalid)
        }
    }
    fn call(
        &mut self,
        c: &ir::Call,
        env: &mut BTreeMap<ir::BindingId, ir::BindingId>,
    ) -> Result<ir::Call> {
        let binding = if !self.data.contains(&c.binding) {
            self.cached[&c.binding]
        } else if self.used.insert(c.binding) {
            self.base[&c.binding]
        } else {
            self.allocate(c.binding, env)?
        };
        self.loop_origin(binding, c.binding, env);
        let args = c
            .args
            .iter()
            .map(|id| Self::id(env, *id))
            .collect::<Result<Vec<_>>>()?;
        env.insert(c.binding, binding);
        Ok(ir::Call {
            binding,
            args,
            ..c.clone()
        })
    }
    fn condition(c: &ir::Cond, env: &BTreeMap<ir::BindingId, ir::BindingId>) -> Result<ir::Cond> {
        Ok(match c {
            ir::Cond::Eq(x) => ir::Cond::Eq(ir::ResultMatch {
                binding: Self::id(env, x.binding)?,
                value: x.value,
            }),
            ir::Cond::And(cs) => ir::Cond::And(
                cs.iter()
                    .map(|c| Self::condition(c, env))
                    .collect::<Result<_>>()?,
            ),
            ir::Cond::Or(cs) => ir::Cond::Or(
                cs.iter()
                    .map(|c| Self::condition(c, env))
                    .collect::<Result<_>>()?,
            ),
            c => c.clone(),
        })
    }
    fn needs_split(
        &mut self,
        branches: [&ir::Block; 2],
        suffix: &[&ir::Statement],
    ) -> Result<bool> {
        let mut assigned = BTreeSet::new();
        let mut todo: Vec<_> = branches.into_iter().flat_map(|b| b.iter()).collect();
        while let Some(s) = todo.pop() {
            self.budget.work()?;
            match &s.kind {
                ir::Stmt::MergeCopy { binding, .. } => {
                    assigned.insert(*binding);
                }
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => {
                    assigned.insert(c.binding);
                }
                ir::Stmt::Response(c, r) => {
                    assigned.insert(c.binding);
                    assigned.insert(r.command);
                    if let Some(handler) = &r.recovery {
                        todo.extend(handler);
                    }
                }
                ir::Stmt::Fork { branches, .. } => {
                    assigned.extend(branches.iter().map(|c| c.binding));
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    todo.extend(then_body);
                    todo.extend(else_body);
                }
                ir::Stmt::LoopControl(c) => {
                    assigned.extend(c.aliases.iter().map(|c| c.binding));
                }
                _ => {}
            }
        }
        assigned.retain(|id| self.data.contains(id));
        if assigned.is_empty() {
            return Ok(false);
        }
        let mut todo = suffix.to_vec();
        while let Some(s) = todo.pop() {
            self.budget.work()?;
            let mut reads = BTreeSet::new();
            match &s.kind {
                ir::Stmt::MergeCopy { source, .. } => {
                    reads.insert(*source);
                }
                ir::Stmt::Call(c) => {
                    reads.extend(c.args.iter().copied());
                }
                ir::Stmt::Query(c, command) => {
                    reads.extend(c.args.iter().copied());
                    reads.insert(*command);
                }
                ir::Stmt::Response(c, r) => {
                    reads.extend(c.args.iter().copied());
                    reads.extend(r.command_call.args.iter().copied());
                    if r.attempt > 1 {
                        reads.insert(r.command);
                    }
                    if let Some(handler) = &r.recovery {
                        todo.extend(handler);
                    }
                }
                ir::Stmt::Fork { branches, .. } => {
                    reads.extend(branches.iter().flat_map(|c| c.args.iter().copied()));
                }
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    condition_support(condition, &mut reads, self.source, 0, &mut self.budget)?;
                    todo.extend(then_body);
                    todo.extend(else_body);
                }
                ir::Stmt::WaitUntil(c) => {
                    condition_support(c, &mut reads, self.source, 0, &mut self.budget)?;
                }
                ir::Stmt::ResolveEffect { command, evidence } => {
                    reads.insert(*command);
                    reads.insert(*evidence);
                }
                ir::Stmt::LoopControl(c) => {
                    reads.extend(c.aliases.iter().flat_map(|c| c.args.iter().copied()));
                }
                _ => {}
            }
            if !assigned.is_disjoint(&reads) {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn block(
        &mut self,
        seq: &[&ir::Statement],
        env: &mut BTreeMap<ir::BindingId, ir::BindingId>,
        depth: usize,
    ) -> Result<ir::Block> {
        self.budget.depth(depth)?;
        let mut out = Vec::new();
        for (i, s) in seq.iter().enumerate() {
            self.budget.copy(4)?;
            let kind = match &s.kind {
                ir::Stmt::End(_) | ir::Stmt::StopReceiptUnknown => {
                    // Retain lexical declarations after an End for the static
                    // inventory, but never constrain a live path with an alias
                    // that this terminating branch intentionally did not yield.
                    for (old, new) in &self.base {
                        env.entry(*old).or_insert(*new);
                    }
                    for (j, binding) in self.source.process.bindings.iter().enumerate() {
                        if binding.merge.is_some() && !env.contains_key(&ir::BindingId(j)) {
                            let mut id = ir::BindingId(j);
                            let mut seen = BTreeSet::new();
                            while let Some(merge) = &self
                                .source
                                .process
                                .bindings
                                .get(id.0)
                                .ok_or_else(invalid)?
                                .merge
                            {
                                if !seen.insert(id) {
                                    return Err(invalid());
                                }
                                id = *merge.sources.first().ok_or_else(invalid)?;
                            }
                            env.insert(
                                ir::BindingId(j),
                                self.base.get(&id).copied().ok_or_else(invalid)?,
                            );
                        }
                    }
                    s.kind.clone()
                }
                ir::Stmt::MergeCopy { binding, source } => {
                    env.insert(*binding, Self::id(env, *source)?);
                    continue;
                }
                ir::Stmt::Call(c) => ir::Stmt::Call(self.call(c, env)?),
                ir::Stmt::Query(c, command) => {
                    ir::Stmt::Query(self.call(c, env)?, Self::id(env, *command)?)
                }
                ir::Stmt::Response(c, r) => {
                    let mut r = r.clone();
                    if r.attempt == 1 {
                        r.command_call = self.call(&r.command_call, env)?;
                    } else {
                        r.command_call.binding = Self::id(env, r.command)?;
                        r.command_call.args = r
                            .command_call
                            .args
                            .iter()
                            .map(|id| Self::id(env, *id))
                            .collect::<Result<_>>()?;
                    }
                    r.command = r.command_call.binding;
                    let c = self.call(c, env)?;
                    if let Some(body) = &r.recovery {
                        r.recovery = Some(self.block(
                            &body.iter().collect::<Vec<_>>(),
                            &mut env.clone(),
                            depth + 1,
                        )?);
                    }
                    ir::Stmt::Response(c, r)
                }
                ir::Stmt::LoopControl(c) => {
                    let mut c = c.clone();
                    c.aliases = c
                        .aliases
                        .iter()
                        .map(|c| self.call(c, env))
                        .collect::<Result<_>>()?;
                    ir::Stmt::LoopControl(c)
                }
                ir::Stmt::ResolveEffect { command, evidence } => ir::Stmt::ResolveEffect {
                    command: Self::id(env, *command)?,
                    evidence: Self::id(env, *evidence)?,
                },
                ir::Stmt::WaitUntil(c) => ir::Stmt::WaitUntil(Self::condition(c, env)?),
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    self.budget
                        .copy(env.len().checked_mul(2).ok_or_else(invalid)?)?;
                    if !self.needs_split([then_body, else_body], &seq[i + 1..])? {
                        out.push(ir::Statement {
                            span: s.span,
                            kind: ir::Stmt::If {
                                condition: Self::condition(condition, env)?,
                                then_body: self.block(
                                    &then_body.iter().collect::<Vec<_>>(),
                                    &mut env.clone(),
                                    depth + 1,
                                )?,
                                else_body: self.block(
                                    &else_body.iter().collect::<Vec<_>>(),
                                    &mut env.clone(),
                                    depth + 1,
                                )?,
                            },
                        });
                        continue;
                    }
                    let then_seq = then_body
                        .iter()
                        .chain(seq[i + 1..].iter().copied())
                        .collect::<Vec<_>>();
                    let else_seq = else_body
                        .iter()
                        .chain(seq[i + 1..].iter().copied())
                        .collect::<Vec<_>>();
                    out.push(ir::Statement {
                        span: s.span,
                        kind: ir::Stmt::If {
                            condition: Self::condition(condition, env)?,
                            then_body: self.block(&then_seq, &mut env.clone(), depth + 1)?,
                            else_body: self.block(&else_seq, &mut env.clone(), depth + 1)?,
                        },
                    });
                    return Ok(out);
                }
                ir::Stmt::Fork { id, branches, join } => {
                    let old_env = env.clone();
                    let mut mapped = BTreeMap::new();
                    let mut calls = Vec::new();
                    for c in branches {
                        let mut local = old_env.clone();
                        let new = self.call(c, &mut local)?;
                        mapped.insert(c.binding, new.binding);
                        env.insert(c.binding, new.binding);
                        calls.push(new);
                    }
                    let mut join = join.clone();
                    if let ir::Join::Any { matches, tie, .. } = &mut join {
                        for x in matches {
                            x.binding = mapped[&x.binding];
                        }
                        if let ir::Tie::By(ids) = tie {
                            for id in ids {
                                *id = mapped[id];
                            }
                        }
                    }
                    ir::Stmt::Fork {
                        id: *id,
                        branches: calls,
                        join,
                    }
                }
                c => c.clone(),
            };
            out.push(ir::Statement { kind, span: s.span });
        }
        Ok(out)
    }
}
