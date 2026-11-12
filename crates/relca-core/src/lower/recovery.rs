//! Dedicated result sources are not ordinary nondeterministic enum producers.
use super::*;
#[cfg(test)]
include!("recovery/round2_tests.rs");

impl Lower {
    // Check declared writers before ordinary result-writer typing.
    pub(super) fn recovery_writers(&self, model: &ast::Model) -> Result<(), Vec<Diagnostic>> {
        let mut commands = Vec::new();
        for item in &model.items {
            if let ast::Item::Process(p) = item {
                let mut calls = Vec::new();
                collect_calls(&p.body, &mut calls);
                commands.extend(calls.into_iter().filter(|c| c.recovery.is_some()));
            }
        }
        for command in commands {
            let declarations: Vec<_> = model
                .items
                .iter()
                .filter_map(|i| {
                    if let ast::Item::Actor(a) = i {
                        Some(a)
                    } else {
                        None
                    }
                })
                .collect();
            let source = declarations
                .iter()
                .find(|a| a.name.value == command.actor.value)
                .and_then(|a| {
                    a.items.iter().find_map(|i| {
                        if let ast::ActorItem::Capability(c) = i {
                            (c.name.value == command.capability.value).then_some(c)
                        } else {
                            None
                        }
                    })
                });
            let Some(source) = source.filter(|c| {
                c.items
                    .iter()
                    .any(|i| matches!(i.value, ast::CapItem::Atomic { .. }))
            }) else {
                continue;
            };
            let owner = self.actor_names[&command.actor.value];
            let private: BTreeSet<_> = source
                .items
                .iter()
                .filter_map(|i| {
                    if let ast::CapItem::Writes(ws) = &i.value {
                        Some(ws)
                    } else {
                        None
                    }
                })
                .flatten()
                .filter_map(|w| self.target_id(w, owner))
                .collect();
            for a in &declarations {
                for field in &a.items {
                    if let ast::ActorItem::Capability(c) = field {
                        if c.span == source.span {
                            continue;
                        }
                        let owner = self.actor_names[&a.name.value];
                        if c.items
                            .iter()
                            .filter_map(|i| {
                                if let ast::CapItem::Writes(ws) = &i.value {
                                    Some(ws)
                                } else {
                                    None
                                }
                            })
                            .flatten()
                            .filter_map(|w| self.target_id(w, owner))
                            .any(|v| private.contains(&v))
                        {
                            return Err(vec![Diagnostic {
                                code: U_SEM_0.into(),
                                message: "recovery_query_unmodeled".into(),
                                severity: diag::Severity::Error,
                                span: Some(c.span),
                            }]);
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn recovery_ast(&self, p: &ast::Process) -> Result<(), Vec<Diagnostic>> {
        let mut calls = Vec::new();
        collect_calls(&p.body, &mut calls);
        let handlers: Vec<_> = calls.iter().filter(|c| c.recovery.is_some()).collect();
        let err = |code: &str, reason: &str, span| {
            vec![Diagnostic {
                code: code.into(),
                message: reason.into(),
                severity: diag::Severity::Error,
                span: Some(span),
            }]
        };
        // Validate syntax-level placement before lowering can erase suffixes.
        fn placement(b: &ast::Block, handler: bool) -> Result<(), Failure> {
            for s in b {
                match &s.value {
                    ast::Stmt::Call(c) => {
                        if c.query_command.is_some()
                            && (!handler || c.response_loss.is_some() || c.retry.is_some())
                        {
                            return Err((U_SEM_0, "recovery_feature_combination", c.span));
                        }
                        if let Some(body) = &c.recovery {
                            placement(body, true)?;
                        }
                    }
                    ast::Stmt::Fork { branches, .. } => {
                        if let Some(c) = branches
                            .iter()
                            .find(|c| c.query_command.is_some() || c.recovery.is_some())
                        {
                            return Err((U_SEM_0, "recovery_feature_combination", c.span));
                        }
                    }
                    ast::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        placement(then_body, handler)?;
                        placement(else_body, handler)?;
                    }
                    _ => {}
                }
            }
            Ok(())
        }
        placement(&p.body, false).map_err(|(code, reason, span)| err(code, reason, span))?;
        if handlers.is_empty() {
            if self.capabilities.iter().any(|c| c.receipt_query.is_some()) {
                return Err(err(U_SEM_0, "recovery_query_unmodeled", p.span));
            }
            return Ok(());
        }
        if handlers.len() != 1 {
            return Err(err(diag::E_CON_0, "invalid_recovery_contract", p.span));
        }
        let command = handlers[0];
        if command.response_loss.is_none() && command.retry.is_none() {
            return Err(err(
                diag::E_CON_0,
                "invalid_recovery_contract",
                command.span,
            ));
        }
        let cap = self
            .cap_names
            .get(&(
                command.actor.value.clone(),
                command.capability.value.clone(),
            ))
            .copied();
        let Some(cap) = cap.filter(|c| self.response_mechanism(*c, None)) else {
            return Err(err(U_SEM_0, "recovery_feature_combination", command.span));
        };
        if !self.errors.is_empty() {
            return Err(self.errors.clone());
        }
        let protected: BTreeSet<_> = self.capabilities[cap.0]
            .finish_writes()
            .map(|w| w.variable)
            .collect();
        let protected_names: BTreeSet<_> = protected
            .iter()
            .flat_map(|id| {
                let v = &self.state_variables[id.0];
                let mut names = vec![v.name.clone()];
                if let ir::StateKind::ActorObserved(a) = v.kind {
                    names.push(format!("{}.{}", self.actors[a.0].name, v.name));
                }
                names
            })
            .collect();
        let body = command.recovery.as_ref().unwrap();
        let mut handler_calls = Vec::new();
        collect_calls(body, &mut handler_calls);
        if handler_calls
            .iter()
            .filter(|c| c.query_command.is_some())
            .count()
            > 32
        {
            return Err(err(U_SEM_0, "recovery_expansion_limit", command.span));
        }
        for c in &handler_calls {
            if c.response_loss.is_some() || c.retry.is_some() || c.recovery.is_some() {
                return Err(err(U_SEM_0, "recovery_feature_combination", c.span));
            }
            if let Some(target) = &c.query_command {
                if target.value != command.binding.value {
                    let known = calls.iter().any(|c| c.binding.value == target.value);
                    return Err(err(
                        if known { E_TYPE_0 } else { E_NAME_0 },
                        if known {
                            "recovery_evidence_mismatch"
                        } else {
                            "unknown_recovery_reference"
                        },
                        c.span,
                    ));
                }
                let q = self
                    .cap_names
                    .get(&(c.actor.value.clone(), c.capability.value.clone()))
                    .and_then(|id| self.capabilities[id.0].receipt_query.as_ref());
                if q.is_none_or(|q| q.target != cap)
                    || c.args.len() != 1
                    || c.args[0].value != "case.id"
                {
                    return Err(err(E_TYPE_0, "recovery_evidence_mismatch", c.span));
                }
            }
        }
        fn names(c: &ast::Cond, out: &mut BTreeSet<String>) {
            match c {
                ast::Cond::Eq { binding, .. } | ast::Cond::Compare { binding, .. } => {
                    out.insert(binding.value.clone());
                }
                ast::Cond::LocalEq {
                    actor, variable, ..
                } => {
                    out.insert(format!("{}.{}", actor.value, variable.value));
                }
                ast::Cond::Contains { element, set } => {
                    for p in [element, set] {
                        out.insert(
                            p.value
                                .iter()
                                .map(|i| i.value.as_str())
                                .collect::<Vec<_>>()
                                .join("."),
                        );
                    }
                }
                ast::Cond::And(cs) | ast::Cond::Or(cs) => {
                    for c in cs {
                        names(&c.value, out);
                    }
                }
                ast::Cond::Group(c) => names(&c.value, out),
            }
        }
        let inspect = |c: &ast::Spanned<ast::Cond>| {
            let mut reads = BTreeSet::new();
            names(&c.value, &mut reads);
            if reads.iter().any(|r| {
                self.state_names
                    .get(r)
                    .is_some_and(|v| self.state_variables[v.0].kind.is_latent())
            }) {
                Some(err(
                    diag::E_POL_LATENT,
                    "policy condition reads latent state",
                    c.span,
                ))
            } else if reads.contains(&command.binding.value) {
                Some(err(E_TYPE_0, "recovery_evidence_mismatch", c.span))
            } else if reads.iter().any(|r| protected_names.contains(r)) {
                Some(err(E_TYPE_0, "recovery_provider_state_leak", c.span))
            } else {
                None
            }
        };
        fn conditions(
            b: &ast::Block,
            inspect: &impl Fn(&ast::Spanned<ast::Cond>) -> Option<Vec<Diagnostic>>,
        ) -> Result<(), Vec<Diagnostic>> {
            for s in b {
                match &s.value {
                    ast::Stmt::If {
                        condition,
                        then_body,
                        else_body,
                    } => {
                        if let Some(e) = inspect(condition) {
                            return Err(e);
                        }
                        conditions(then_body, inspect)?;
                        conditions(else_body, inspect)?;
                    }
                    ast::Stmt::WaitUntil(condition) => {
                        if let Some(e) = inspect(condition) {
                            return Err(e);
                        }
                    }
                    ast::Stmt::Call(c) => {
                        if let Some(body) = &c.recovery {
                            conditions(body, inspect)?;
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        }
        conditions(body, &inspect)?;
        conditions(&p.body, &|c| {
            let mut reads = BTreeSet::new();
            names(&c.value, &mut reads);
            reads
                .iter()
                .any(|r| protected_names.contains(r))
                .then(|| err(E_TYPE_0, "recovery_provider_state_leak", c.span))
        })?;
        for c in &calls {
            if c.args.iter().any(|arg| arg.value == command.binding.value)
                && handler_calls.iter().any(|h| h.span == c.span)
            {
                return Err(err(E_TYPE_0, "recovery_evidence_mismatch", c.span));
            }
            if let Some(id) = self
                .cap_names
                .get(&(c.actor.value.clone(), c.capability.value.clone()))
            {
                if *id == cap && c.span != command.span {
                    return Err(err(diag::E_CON_0, "invalid_recovery_contract", c.span));
                }
                if self.capabilities[id.0].receipt_query.is_some() && c.query_command.is_none() {
                    return Err(err(E_TYPE_0, "recovery_evidence_mismatch", c.span));
                }
                let c = &self.capabilities[id.0];
                if c.read_variables
                    .iter()
                    .chain(&c.kernel_reads_latent)
                    .any(|v| protected.contains(v))
                    || c.result_kernels()
                        .any(|k| k.given.iter().any(|v| protected.contains(v)))
                    || c.requires
                        .as_ref()
                        .is_some_and(|r| protected_condition(&r.condition, &protected))
                    || c.version_check
                        .as_ref()
                        .is_some_and(|v| v.variables.iter().any(|v| protected.contains(v)))
                    || *id != cap
                        && c.atomic
                            .as_ref()
                            .is_some_and(|a| protected_condition(&a.condition, &protected))
                {
                    return Err(err(E_TYPE_0, "recovery_provider_state_leak", c.span));
                }
                if *id != cap
                    && c.finish_writes()
                        .chain(&c.start_writes)
                        .any(|w| protected.contains(&w.variable))
                {
                    return Err(err(E_TYPE_0, "recovery_provider_state_leak", c.span));
                }
            }
        }
        let all: BTreeSet<_> = handler_calls
            .iter()
            .map(|c| c.binding.value.clone())
            .collect();
        check_body(body, &command.binding.value, &all, BTreeMap::new(), false)
            .map_err(|(code, reason, span)| err(code, reason, span))?;
        Ok(())
    }
    pub(super) fn query_declaration(
        &mut self,
        cap: &ast::Capability,
        actor: ir::ActorId,
        params: &[ir::Parameter],
        ty: Option<ir::TypeId>,
    ) -> Option<ir::ReceiptQuery> {
        let declarations: Vec<_> = cap
            .items
            .iter()
            .filter_map(|item| {
                if let ast::CapItem::ReceiptQuery {
                    target,
                    key,
                    availability,
                } = &item.value
                {
                    Some((target, key, availability))
                } else {
                    None
                }
            })
            .collect();
        let (target, key, availability) = declarations.first().copied()?;
        let bad = declarations.len() != 1
            || params.len() != 1
            || params[0].ty != ir::CASE_ID_TYPE
            || params[0].name != key.value
            || ty.is_none_or(|t| {
                let mut names = self.types[t.0].members.clone();
                names.sort();
                names != ["committed_once", "not_committed", "still_unknown"]
            })
            || cap.items.iter().any(|i| {
                !matches!(
                    i.value,
                    ast::CapItem::ReceiptQuery { .. }
                        | ast::CapItem::DurationFixed(_)
                        | ast::CapItem::DurationDistribution(_)
                        | ast::CapItem::Cancel(_)
                        | ast::CapItem::Uses(_)
                )
            });
        let target_id = self
            .cap_names
            .get(&(self.actors[actor.0].name.clone(), target.value.clone()))
            .copied();
        if bad || target_id.is_none_or(|id| !self.response_mechanism(id, None)) {
            self.unsupported("recovery_query_unmodeled", cap.span);
        }
        target_id.map(|target| ir::ReceiptQuery {
            target,
            key_parameter: 0,
            availability: *availability,
        })
    }
}

fn protected_condition(c: &ir::Cond, protected: &BTreeSet<ir::StateVarId>) -> bool {
    match c {
        ir::Cond::StateEq(x) => protected.contains(&x.variable),
        ir::Cond::Contains { set, .. } => protected.contains(set),
        ir::Cond::IntCompare { variable, .. } => protected.contains(variable),
        ir::Cond::And(cs) | ir::Cond::Or(cs) => {
            cs.iter().any(|c| protected_condition(c, protected))
        }
        ir::Cond::Eq(_) => false,
    }
}

type Failure = (&'static str, &'static str, Span);
type Domains = BTreeMap<String, BTreeSet<String>>;
fn possible(c: &ast::Cond, binding: &str, value: &str) -> (bool, bool) {
    match c {
        ast::Cond::Eq {
            binding: b,
            value: v,
        } if b.value == binding => (v.value == value, v.value != value),
        ast::Cond::Group(c) => possible(&c.value, binding, value),
        ast::Cond::And(cs) => {
            let xs: Vec<_> = cs
                .iter()
                .map(|c| possible(&c.value, binding, value))
                .collect();
            (xs.iter().all(|x| x.0), xs.iter().any(|x| x.1))
        }
        ast::Cond::Or(cs) => {
            let xs: Vec<_> = cs
                .iter()
                .map(|c| possible(&c.value, binding, value))
                .collect();
            (xs.iter().any(|x| x.0), xs.iter().all(|x| x.1))
        }
        _ => (true, true),
    }
}
fn check_body(
    b: &ast::Block,
    command: &str,
    all: &BTreeSet<String>,
    domains: Domains,
    resolved: bool,
) -> Result<(), Failure> {
    let flows = continuations(b, command, all, vec![(domains, resolved)], 0)?;
    if flows.is_empty() {
        Ok(())
    } else {
        Err((
            diag::E_SYN_0,
            "invalid_recovery_syntax",
            b.last().map_or(Span { start: 0, end: 0 }, |s| s.span),
        ))
    }
}
fn continuations(
    b: &ast::Block,
    command: &str,
    all: &BTreeSet<String>,
    mut flows: Vec<(Domains, bool)>,
    depth: usize,
) -> Result<Vec<(Domains, bool)>, Failure> {
    if depth > 128 {
        return Err((
            diag::U_SEM_0,
            "recovery_expansion_limit",
            b.last().map_or(Span { start: 0, end: 0 }, |s| s.span),
        ));
    }
    for s in b {
        if flows.is_empty() {
            return Err((diag::E_SYN_0, "invalid_recovery_syntax", s.span));
        }
        let mut next = Vec::new();
        for (mut domains, mut resolved) in flows {
            match &s.value {
                ast::Stmt::Call(c) => {
                    if c.query_command.is_some() {
                        domains.insert(
                            c.binding.value.clone(),
                            ["committed_once", "not_committed", "still_unknown"]
                                .into_iter()
                                .map(String::from)
                                .collect(),
                        );
                    }
                    next.push((domains, resolved));
                }
                ast::Stmt::ResolveEffect {
                    command: c,
                    evidence,
                } => {
                    if !all.contains(&evidence.value) {
                        return Err((E_NAME_0, "unknown_recovery_reference", s.span));
                    }
                    if c.value != command
                        || domains
                            .get(&evidence.value)
                            .is_none_or(|d| d.contains("still_unknown"))
                    {
                        return Err((E_TYPE_0, "recovery_evidence_mismatch", s.span));
                    }
                    if resolved {
                        return Err((diag::E_CON_0, "invalid_recovery_contract", s.span));
                    }
                    resolved = true;
                    next.push((domains, resolved));
                }
                ast::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    for (body, yes) in [(then_body, true), (else_body, false)] {
                        let mut d = domains.clone();
                        for (binding, domain) in &mut d {
                            domain.retain(|v| {
                                let p = possible(&condition.value, binding, v);
                                if yes { p.0 } else { p.1 }
                            });
                        }
                        next.extend(continuations(
                            body,
                            command,
                            all,
                            vec![(d, resolved)],
                            depth + 1,
                        )?);
                    }
                }
                ast::Stmt::End(_) | ast::Stmt::StopReceiptUnknown => {
                    if matches!(s.value, ast::Stmt::End(_)) != resolved {
                        return Err((diag::E_CON_0, "invalid_recovery_contract", s.span));
                    }
                }
                ast::Stmt::Wait(_) => next.push((domains, resolved)),
                _ => return Err((diag::U_SYN_0, "unsupported recovery statement", s.span)),
            }
        }
        if next.len() > 65536 {
            return Err((diag::U_SEM_0, "recovery_expansion_limit", s.span));
        }
        flows = next;
    }
    Ok(flows)
}

#[cfg(test)]
mod tests {
    #[test]
    fn provider_kernel_is_not_a_business_observation() {
        for (variable, reject) in [("charged_truth", true), ("refunded", false)] {
            let source = crate::r2_inputs::edit(
                &crate::r2_inputs::source("R01"),
                "result nondet {proceed}",
                "outcomes kernel Leak",
            );
            let source = crate::r2_inputs::append(
                &source,
                &format!(
                    "kernel Leak given [{variable}] {{false -> {{1.0: proceed}}; true -> {{1.0: withdraw}}}}"
                ),
            );
            let parsed = crate::syntax::parse(&source).unwrap();
            let result = crate::lower::lower(&parsed);
            if reject {
                let d = result.unwrap_err();
                assert_eq!(d.len(), 1);
                assert_eq!(d[0].code, crate::diag::E_TYPE_0);
                assert_eq!(d[0].message, "recovery_provider_state_leak");
            } else {
                result.unwrap();
            }
        }
    }
    #[test]
    fn query_forward_declaration_and_closed_common_continuation() {
        let source = crate::r2_inputs::source("R01");
        let mut ast = crate::syntax::parse(&source).unwrap();
        for item in &mut ast.items {
            if let crate::ast::Item::Actor(a) = item
                && a.name.value == "Psp"
            {
                let g=a.items.iter().position(|i|matches!(i,crate::ast::ActorItem::Capability(c) if c.name.value=="charge_keyed")).unwrap();
                let q=a.items.iter().position(|i|matches!(i,crate::ast::ActorItem::Capability(c) if c.name.value=="query_status")).unwrap();
                a.items.swap(g, q);
            }
        }
        crate::lower::lower(&ast).unwrap();
        let common = "status = Psp.query_status(case.id) for command pay\nif status == committed_once {resolve effect pay using status} else {stop receipt_unknown}\ndecision = CS.decide(status)\nsh = Shop.instruct_ship(decision)\nend shipped";
        let source = crate::r2_inputs::body(Some(common), crate::r2_inputs::LOSS);
        let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
        let e = crate::engine::explore(&m);
        let r = crate::reference::explore(&m);
        assert_eq!(r.stop, crate::reference::StopReason::Complete);
        assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
        assert_eq!(e.terminals[0].elapsed.min, 7);
    }
}
