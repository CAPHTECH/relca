//! Forward-only bounded expansion. A public result alias is not a Task;
//! each round owns a reserved binding and therefore an independent history.
use super::*;

pub(super) struct LoopContext {
    pub round_variable: ir::StateVarId,
    pub active: ir::StateVarId,
    pub continuing: ir::StateVarId,
    pub round: u64,
    pub in_body: bool,
    pub leaves: Vec<Flow>,
    pub agains: Vec<Flow>,
}

impl Lower {
    pub(super) fn loop_round_in_scope(&self, id: ir::StateVarId) -> bool {
        self.loop_context
            .as_ref()
            .is_some_and(|c| c.in_body && c.round_variable == id)
    }

    pub(super) fn loop_call_scope(&mut self, call: &ir::Call) {
        fn reads(c: &ir::Cond, out: &mut Vec<ir::StateVarId>) {
            match c {
                ir::Cond::IntCompare { variable, .. } => out.push(*variable),
                ir::Cond::StateEq(s) => out.push(s.variable),
                ir::Cond::Contains { set, .. } => out.push(*set),
                ir::Cond::And(cs) | ir::Cond::Or(cs) => {
                    for c in cs {
                        reads(c, out);
                    }
                }
                ir::Cond::Eq(_) => {}
            }
        }
        let cap = &self.capabilities[call.capability.0];
        let mut ids: Vec<_> = cap
            .result_kernels()
            .flat_map(|k| k.given.iter().copied())
            .collect();
        if let Some(r) = &cap.requires {
            reads(&r.condition, &mut ids);
        }
        if ids.into_iter().any(|id| {
            self.loop_variables.values().any(|r| *r == id) && !self.loop_round_in_scope(id)
        }) {
            self.unsupported("loop_round_out_of_scope", call.span);
        }
    }

    pub(super) fn loop_result_alias(&self, binding: ir::BindingId) -> Option<ir::BindingId> {
        let site = self.bindings.get(binding.0)?.repetition.as_ref()?;
        self.bindings
            .iter()
            .position(|b| b.name == site.call_site)
            .map(ir::BindingId)
    }

    pub(super) fn loop_declarations(&mut self, model: &ast::Model) {
        if !model.has_repetition() {
            return;
        }
        fn walk(l: &mut Lower, block: &ast::Block, names: &mut BTreeSet<String>) {
            for s in block {
                match &s.value {
                    ast::Stmt::Repeat {
                        name,
                        at_most,
                        body,
                        on_limit,
                        ..
                    } => {
                        if !names.insert(name.value.clone())
                            || l.actor_names.contains_key(&name.value)
                            || l.global_bindings.contains(&name.value)
                            || l.end_names.contains_key(&name.value)
                            || l.state_names.contains_key(&name.value)
                        {
                            l.error(E_NAME_0, "loop name collision", name.span);
                        }
                        let n = at_most.value;
                        if n == 0 {
                            l.error(diag::E_CON_0, "loop limit must be positive", at_most.span);
                        }
                        if n > 32 {
                            l.unsupported("loop_expansion_limit", at_most.span);
                        }
                        // Preserve the written type domain even for an
                        // unsupported limit; expansion itself is guarded below.
                        let round = l.loop_variable(
                            format!("@loop:{}:round", name.value),
                            n.max(1),
                            1,
                            name.span,
                        );
                        l.loop_variables.insert(name.value.clone(), round);
                        l.loop_variable(format!("@loop:{}:active", name.value), 1, 1, name.span);
                        l.loop_variable(format!("@loop:{}:limit", name.value), 1, 0, name.span);
                        l.loop_variable(format!("@loop:{}:continue", name.value), 1, 1, name.span);
                        l.loop_variable(format!("@loop:{}:started", name.value), 1, 0, name.span);
                        walk(l, body, names);
                        walk(l, on_limit, names);
                    }
                    ast::Stmt::Merge {
                        then_body,
                        else_body,
                        ..
                    }
                    | ast::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        walk(l, then_body, names);
                        walk(l, else_body, names);
                    }
                    ast::Stmt::Call(c) => {
                        if let Some(b) = &c.recovery {
                            walk(l, b, names);
                        }
                    }
                    _ => {}
                }
            }
        }
        // Forward declarations are needed before resolving a kernel's given.
        for i in &model.items {
            if let ast::Item::Process(p) = i {
                let mut calls = Vec::new();
                collect_calls(&p.body, &mut calls);
                self.global_bindings
                    .extend(calls.iter().map(|c| c.binding.value.clone()));
                self.collect_ends(&p.body);
            }
        }
        let mut names = BTreeSet::new();
        for i in &model.items {
            if let ast::Item::Process(p) = i {
                walk(self, &p.body, &mut names);
            }
        }
    }

    fn loop_variable(
        &mut self,
        name: String,
        max: u64,
        initial: u64,
        span: Span,
    ) -> ir::StateVarId {
        let id = ir::StateVarId(self.state_variables.len());
        self.state_names.insert(name.clone(), id);
        self.state_variables.push(ir::StateVariable {
            last_write_wins: false,
            case_set_size: None,
            integer_max: Some(max),
            name,
            kind: ir::StateKind::ObservedCase,
            base: ir::TypeId(usize::MAX),
            optional: false,
            initial: ir::InitialValue::Fixed(ir::StateValue::Integer(initial)),
            span,
        });
        id
    }

    pub(super) fn collect_loop(
        &mut self,
        name: &ast::Ident,
        n: &ast::Integer,
        body: &ast::Block,
        limit: &ast::Block,
    ) {
        self.collect(body);
        self.collect(limit);
        let n = n.value;
        if !(1..=32).contains(&n) {
            return;
        }
        let mut calls = Vec::new();
        collect_calls(body, &mut calls);
        for round in 1..=n {
            for call in &calls {
                let mut call = (*call).clone();
                let call_site = call.binding.value.clone();
                call.binding.value = format!("@loop:{}:{round}:{}", name.value, call.binding.value);
                self.collect_call(&call);
                if let Some(id) = self.binding_names.get(&call.binding.value) {
                    self.bindings[id.0].repetition = Some(ir::LoopSite {
                        loop_name: name.value.clone(),
                        round: Some(round),
                        phase: ir::LoopPhase::Body,
                        call_site,
                    });
                }
            }
        }
        let mut limit_calls = Vec::new();
        collect_calls(limit, &mut limit_calls);
        for call in limit_calls {
            let mut call = call.clone();
            let call_site = call.binding.value.clone();
            call.binding.value = format!("@loop:{}:{n}:{}", name.value, call.binding.value);
            self.collect_call(&call);
            if let Some(id) = self.binding_names.get(&call.binding.value) {
                self.bindings[id.0].repetition = Some(ir::LoopSite {
                    loop_name: name.value.clone(),
                    round: None,
                    phase: ir::LoopPhase::OnLimit,
                    call_site,
                });
            }
        }
    }

    fn loop_paths(
        &mut self,
        body: &ast::Block,
        mut paths: BTreeSet<bool>,
        limit: bool,
    ) -> BTreeSet<bool> {
        for (index, s) in body.iter().enumerate() {
            if matches!(s.value, ast::Stmt::Again | ast::Stmt::Leave) && index + 1 < body.len() {
                self.error(
                    diag::E_CON_0,
                    "loop_unreachable_after_control",
                    body[index + 1].span,
                );
            }
            match &s.value {
                ast::Stmt::Call(c) => {
                    if c.response_loss.is_some()
                        || c.retry.is_some()
                        || c.recovery.is_some()
                        || c.query_command.is_some()
                    {
                        self.unsupported("loop_feature_combination", c.span);
                    }
                    if let Some(id) = self
                        .cap_names
                        .get(&(c.actor.value.clone(), c.capability.value.clone()))
                    {
                        let cap = &self.capabilities[id.0];
                        let min = cap
                            .duration_distribution
                            .as_ref()
                            .map_or(cap.duration, |d| {
                                d.entries.iter().map(|e| e.0).min().unwrap_or(0)
                            });
                        if cap.requires.is_none() && min > 0 && !paths.is_empty() {
                            paths = BTreeSet::from([true]);
                        }
                    }
                }
                ast::Stmt::Wait(d) => {
                    if self.ticks(d).is_some_and(|n| n > 0) && !paths.is_empty() {
                        paths = BTreeSet::from([true]);
                    }
                }
                ast::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    let yes = self.loop_paths(then_body, paths.clone(), limit);
                    let no = self.loop_paths(else_body, paths, limit);
                    paths = yes.union(&no).copied().collect();
                }
                ast::Stmt::Again => {
                    if limit {
                        self.error(diag::E_CON_0, "loop_again_in_limit", s.span);
                    } else if paths.contains(&false) {
                        self.error(diag::E_CON_0, "zero_time_iteration", s.span);
                    }
                    paths.clear();
                }
                ast::Stmt::Leave | ast::Stmt::End(_) => {
                    paths.clear();
                }
                _ => {
                    self.unsupported("loop_feature_combination", s.span);
                }
            }
        }
        paths
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn expand_loop(
        &mut self,
        name: &ast::Ident,
        n: &ast::Integer,
        gap: Option<&ast::Duration>,
        body: &ast::Block,
        limit: &ast::Block,
        flow: &mut Flow,
        span: Span,
    ) -> ir::Block {
        let n = n.value;
        if !(1..=32).contains(&n) {
            return vec![];
        }
        if self.loop_context.is_some() || self.in_recovery_handler {
            self.unsupported("loop_feature_combination", span);
            return vec![];
        }
        let gap = gap.and_then(|d| self.ticks(d)).unwrap_or(0);
        if !self
            .loop_paths(body, BTreeSet::from([gap > 0]), false)
            .is_empty()
        {
            self.error(diag::E_CON_0, "loop_fallthrough", span);
        }
        if !self
            .loop_paths(limit, BTreeSet::from([false]), true)
            .is_empty()
        {
            self.error(diag::E_CON_0, "loop_fallthrough", span);
        }
        let round_variable = self.loop_variables[&name.value];
        let active = self.state_names[&format!("@loop:{}:active", name.value)];
        let limit_variable = self.state_names[&format!("@loop:{}:limit", name.value)];
        let continuing = self.state_names[&format!("@loop:{}:continue", name.value)];
        let started = self.state_names[&format!("@loop:{}:started", name.value)];
        let original_names = self.binding_names.clone();
        let mut calls = Vec::new();
        collect_calls(body, &mut calls);
        let mut limit_calls = Vec::new();
        collect_calls(limit, &mut limit_calls);
        let alias_flow = Flow {
            available: self.binding_names.values().copied().collect(),
            ..flow.clone()
        };
        let aliases = calls
            .iter()
            .chain(&limit_calls)
            .filter_map(|c| self.call_arguments(c, &alias_flow))
            .collect();
        let mut output = vec![ir::Statement {
            kind: ir::Stmt::LoopControl(ir::LoopControl {
                updates: vec![],
                aliases,
            }),
            span,
        }];
        let mut entering = flow.clone();
        let mut leaves = Vec::new();
        for round in 1..=n {
            self.binding_names = original_names.clone();
            for c in &calls {
                let internal = format!("@loop:{}:{round}:{}", name.value, c.binding.value);
                if let Some(&id) = original_names.get(&internal) {
                    self.binding_names.insert(c.binding.value.clone(), id);
                }
            }
            self.loop_context = Some(LoopContext {
                round_variable,
                active,
                continuing,
                round,
                in_body: true,
                leaves: vec![],
                agains: vec![],
            });
            let mut round_flow = entering.clone();
            let mut expanded = Vec::new();
            if round > 1 && gap > 0 {
                expanded.push(ir::Statement {
                    kind: ir::Stmt::Wait(gap),
                    span,
                });
            }
            expanded.push(ir::Statement {
                kind: ir::Stmt::LoopControl(
                    vec![
                        (round_variable, ir::StateValue::Integer(round)),
                        (continuing, ir::StateValue::Integer(1)),
                        (started, ir::StateValue::Integer(1)),
                    ]
                    .into(),
                ),
                span,
            });
            expanded.extend(self.block(body, &mut round_flow));
            let mut context = self.loop_context.take().unwrap();
            for exit in context.leaves.iter_mut().chain(&mut context.agains) {
                for c in &calls {
                    if let (Some(&internal), Some(&public)) = (
                        self.binding_names.get(&c.binding.value),
                        original_names.get(&c.binding.value),
                    ) && exit.available.contains(&internal)
                    {
                        exit.available.insert(public);
                    }
                }
            }
            leaves.extend(context.leaves);
            entering = join_flows(context.agains, &entering);
            output.push(ir::Statement {
                kind: ir::Stmt::If {
                    condition: active_condition(active),
                    then_body: expanded,
                    else_body: vec![],
                },
                span,
            });
        }
        self.binding_names = original_names.clone();
        for c in &limit_calls {
            let internal = format!("@loop:{}:{n}:{}", name.value, c.binding.value);
            if let Some(&id) = original_names.get(&internal) {
                self.binding_names.insert(c.binding.value.clone(), id);
            }
        }
        self.loop_context = Some(LoopContext {
            round_variable,
            active,
            continuing,
            round: n,
            in_body: false,
            leaves: vec![],
            agains: vec![],
        });
        let mut limit_flow = entering.clone();
        let mut expanded = vec![ir::Statement {
            kind: ir::Stmt::LoopControl(
                vec![
                    (limit_variable, ir::StateValue::Integer(1)),
                    (continuing, ir::StateValue::Integer(1)),
                ]
                .into(),
            ),
            span,
        }];
        expanded.extend(self.block(limit, &mut limit_flow));
        let mut exits = self.loop_context.take().unwrap().leaves;
        for exit in &mut exits {
            for c in &limit_calls {
                if let (Some(&internal), Some(&public)) = (
                    self.binding_names.get(&c.binding.value),
                    original_names.get(&c.binding.value),
                ) && exit.available.contains(&internal)
                {
                    exit.available.insert(public);
                }
            }
        }
        leaves.extend(exits);
        self.binding_names = original_names;
        output.push(ir::Statement {
            kind: ir::Stmt::If {
                condition: active_condition(active),
                then_body: expanded,
                else_body: vec![],
            },
            span,
        });
        *flow = join_flows(leaves, flow);
        output
    }
}

fn active_condition(active: ir::StateVarId) -> ir::Cond {
    ir::Cond::IntCompare {
        variable: active,
        operator: ir::IntComparison::Eq,
        value: 1,
    }
}

fn join_flows(flows: Vec<Flow>, fallback: &Flow) -> Flow {
    let mut live = flows.into_iter().filter(|f| !f.histories.is_empty());
    let Some(mut result) = live.next() else {
        let mut f = fallback.clone();
        f.histories.clear();
        return f;
    };
    for f in live {
        result.available = result
            .available
            .intersection(&f.available)
            .copied()
            .collect();
        result.histories.extend(f.histories);
    }
    result
}
