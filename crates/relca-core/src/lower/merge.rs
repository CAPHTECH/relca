//! Merge slots are results, not calls. Only the selected yield copies a value.
use super::*;

impl Lower {
    pub(super) fn index_merges(&mut self, model: &ast::Model) {
        fn walk(l: &mut Lower, b: &ast::Block) {
            for s in b {
                match &s.value {
                    ast::Stmt::Merge {
                        binding,
                        then_body,
                        else_body,
                        ..
                    } => {
                        l.merge_names.insert(binding.value.clone());
                        walk(l, then_body);
                        walk(l, else_body);
                    }
                    ast::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        walk(l, then_body);
                        walk(l, else_body);
                    }
                    ast::Stmt::Repeat { body, on_limit, .. } => {
                        let mut calls = Vec::new();
                        collect_calls(body, &mut calls);
                        collect_calls(on_limit, &mut calls);
                        l.unstable_merge_sources
                            .extend(calls.iter().map(|c| c.binding.value.clone()));
                        walk(l, body);
                        walk(l, on_limit);
                    }
                    ast::Stmt::Call(c) => {
                        if let Some(b) = &c.recovery {
                            walk(l, b);
                        }
                    }
                    _ => {}
                }
            }
        }
        for item in &model.items {
            if let ast::Item::Process(p) = item {
                walk(self, &p.body);
            }
        }
    }

    pub(super) fn collect_merge(&mut self, name: &ast::Ident) {
        if self
            .state_names
            .get(&name.value)
            .is_some_and(|id| self.state_variables[id.0].kind == ir::StateKind::ObservedCase)
        {
            // Ordinary aliases require the defining Capability to write the
            // observed field. A merge is not a writer and cannot bypass this.
            self.unsupported("observed binding alias requires a result writer", name.span);
        }
        if let Some(&previous) = self.binding_names.get(&name.value) {
            if self.ambiguous_bindings.insert(name.value.clone()) {
                self.unsupported(
                    "duplicate or shadowed binding",
                    self.bindings[previous.0].span,
                );
            }
            self.unsupported("duplicate or shadowed binding", name.span);
            self.calls.remove(&name.value);
            return;
        }
        let id = ir::BindingId(self.bindings.len());
        self.binding_names.insert(name.value.clone(), id);
        self.bindings.push(ir::Binding {
            merge: Some(ir::MergeSite {
                sources: Vec::new(),
            }),
            repetition: None,
            name: name.value.clone(),
            ty: ir::TypeId(0),
            span: name.span,
        });
    }

    pub(super) fn merge_types(&mut self, b: &ast::Block) {
        for s in b {
            match &s.value {
                ast::Stmt::Merge {
                    binding,
                    then_body,
                    else_body,
                    ..
                } => {
                    self.merge_types(then_body);
                    self.merge_types(else_body);
                    let mut types = BTreeSet::new();
                    let mut sources = Vec::new();
                    let mut unknown = false;
                    for b in [then_body, else_body] {
                        if let Some(ast::Spanned {
                            value: ast::Stmt::Yield(source),
                            ..
                        }) = b.last()
                        {
                            if let Some(&id) = self.binding_names.get(&source.value) {
                                sources.push(id);
                            }
                            unknown |= self.unknown_binding_types.contains(&source.value)
                                || !self.binding_types.contains_key(&source.value);
                            if let Some(ts) = self.binding_types.get(&source.value) {
                                types.extend(ts);
                            }
                        }
                    }
                    if unknown {
                        self.unknown_binding_types.insert(binding.value.clone());
                    } else if types.len() > 1 {
                        self.error(E_TYPE_0, "merge_type_mismatch", binding.span);
                    }
                    if let Some(&id) = self.binding_names.get(&binding.value) {
                        self.bindings[id.0].merge = Some(ir::MergeSite { sources });
                    }
                    if let Some(&id) = self.binding_names.get(&binding.value)
                        && let Some(&ty) = types.first()
                    {
                        self.bindings[id.0].ty = ty;
                        if self.is_record(ty) {
                            self.record_binding_names.insert(binding.value.clone());
                        }
                    }
                    self.binding_types.insert(binding.value.clone(), types);
                }
                ast::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    self.merge_types(then_body);
                    self.merge_types(else_body);
                }
                ast::Stmt::Repeat { body, on_limit, .. } => {
                    self.merge_types(body);
                    self.merge_types(on_limit);
                }
                _ => {}
            }
        }
    }

    pub(super) fn merge_block(
        &mut self,
        name: &ast::Ident,
        condition: &ast::Spanned<ast::Cond>,
        then_body: &ast::Block,
        else_body: &ast::Block,
        flow: &mut Flow,
    ) -> Option<ir::Stmt> {
        if self.loop_context.is_some() {
            self.unsupported("loop_feature_combination", name.span);
        }
        if self.in_recovery_handler {
            self.unsupported("merge_feature_combination", name.span);
        }
        for body in [then_body, else_body] {
            if !body
                .last()
                .is_some_and(|s| matches!(s.value, ast::Stmt::Yield(_) | ast::Stmt::End(_)))
            {
                self.error(diag::E_CON_0, "merge_fallthrough", name.span);
            }
            for s in body.iter().take(body.len().saturating_sub(1)) {
                if matches!(s.value, ast::Stmt::Yield(_)) {
                    self.error(diag::E_CON_0, "merge_unreachable_after_yield", s.span);
                }
            }
        }
        if [then_body.last(), else_body.last()]
            .iter()
            .all(|s| s.is_some_and(|s| matches!(s.value, ast::Stmt::End(_))))
        {
            self.error(diag::E_CON_0, "merge_no_continuing_branch", name.span);
        }
        let target = self.binding_names.get(&name.value).copied()?;
        let cond = self.condition(condition, flow);
        let truth = cond.as_ref().map(|c| self.truth(c)).unwrap_or((true, true));
        let (mut yes, mut no) = (flow.clone(), flow.clone());
        if !truth.0 {
            yes.histories.clear();
        }
        if !truth.1 {
            no.histories.clear();
        }
        let previous = self.merge_target.replace((target, self.block_depth + 1));
        let then_body = self.block(then_body, &mut yes);
        let else_body = self.block(else_body, &mut no);
        self.merge_target = previous;
        flow.available = match (yes.histories.is_empty(), no.histories.is_empty()) {
            (false, false) => yes.available.intersection(&no.available).copied().collect(),
            (false, true) => yes.available,
            (true, false) => no.available,
            (true, true) => flow.available.clone(),
        };
        flow.histories = yes.histories.union(&no.histories).copied().collect();
        if !flow.histories.is_empty() {
            flow.available.insert(target);
        }
        cond.map(|condition| ir::Stmt::If {
            condition,
            then_body,
            else_body,
        })
    }

    pub(super) fn merge_copy(
        &mut self,
        source: &ast::Ident,
        span: Span,
        flow: &Flow,
    ) -> Option<ir::Stmt> {
        let Some((binding, _)) = self
            .merge_target
            .filter(|(_, depth)| *depth == self.block_depth)
        else {
            self.error(diag::E_CON_0, "merge_yield_outside", span);
            return None;
        };
        if let Some((owner, field)) = source.value.split_once('.') {
            let variable = self
                .actor_names
                .get(owner)
                .and_then(|actor| self.local_names.get(&(*actor, field.into())))
                .or_else(|| self.state_names.get(owner));
            if variable.is_some_and(|id| self.state_variables[id.0].kind.is_latent()) {
                self.error(
                    diag::E_POL_LATENT,
                    "policy cannot read latent state",
                    source.span,
                );
            } else if variable.is_some()
                || self.binding_names.contains_key(owner)
                || self.type_names.get(owner).is_some_and(|ty| {
                    self.types
                        .get(ty.0)
                        .is_some_and(|t| t.members.iter().any(|v| v == field))
                })
            {
                self.unsupported("merge_source_unsupported", source.span);
            } else {
                self.error(E_NAME_0, "undefined merge source binding", source.span);
            }
            return None;
        }
        if self
            .state_names
            .get(&source.value)
            .is_some_and(|id| self.state_variables[id.0].kind.is_latent())
        {
            self.error(
                diag::E_POL_LATENT,
                "policy cannot read latent state",
                source.span,
            );
            return None;
        }
        let Some(&id) = self.binding_names.get(&source.value) else {
            if self.state_names.contains_key(&source.value)
                || self.types.iter().any(|t| t.members.contains(&source.value))
            {
                self.unsupported("merge_source_unsupported", source.span);
            } else {
                self.error(E_NAME_0, "undefined merge source binding", source.span);
            }
            return None;
        };
        if self.unstable_merge_sources.contains(&source.value) {
            self.unsupported("merge_feature_combination", source.span);
        }
        if self.check_flow && !flow.available.contains(&id) {
            self.unsupported(
                "result is not definitely available (not yet defined or possibly cancelled)",
                source.span,
            );
        }
        Some(ir::Stmt::MergeCopy {
            binding,
            source: id,
        })
    }

    pub(super) fn merge_condition_forbidden(&mut self, c: &ast::Spanned<ast::Cond>) -> bool {
        fn reads(c: &ast::Cond, l: &Lower) -> bool {
            match c {
                ast::Cond::Eq { binding, .. } | ast::Cond::Compare { binding, .. } => {
                    l.merge_names.contains(&binding.value)
                }
                ast::Cond::LocalEq { actor, .. } => {
                    l.merge_names.contains(&actor.value)
                        && !l.actor_names.contains_key(&actor.value)
                }
                ast::Cond::Group(c) => reads(&c.value, l),
                ast::Cond::And(cs) | ast::Cond::Or(cs) => cs.iter().any(|c| reads(&c.value, l)),
                _ => false,
            }
        }
        let forbidden = reads(&c.value, self);
        if forbidden {
            self.unsupported("merge_feature_combination: merge_read_position", c.span);
        }
        forbidden
    }

    pub(super) fn merge_read_name_forbidden(&mut self, name: &ast::Ident) -> bool {
        let forbidden = self.merge_names.contains(&name.value)
            || name.value.split_once('.').is_some_and(|(prefix, _)| {
                self.merge_names.contains(prefix) && !self.actor_names.contains_key(prefix)
            });
        if forbidden {
            self.unsupported("merge_feature_combination: merge_read_position", name.span);
        }
        forbidden
    }

    /// A value domain may be unioned; an evidence lineage may not. Keep this
    /// query separate from Agency's path-dependent source derivation.
    pub(super) fn merge_truth(&self, result: &ir::ResultMatch) -> (bool, bool) {
        let mut pending = vec![result.binding];
        let mut seen = BTreeSet::new();
        let mut values = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !seen.insert(id) {
                continue;
            }
            let Some(binding) = self.bindings.get(id.0) else {
                return (true, true);
            };
            if let Some(merge) = &binding.merge {
                pending.extend(merge.sources.iter().copied());
            } else if let Some(call) = self.calls.get(&binding.name).filter(|c| c.binding == id) {
                values.extend(self.capabilities[call.capability.0].possible_results());
            } else {
                return (true, true);
            }
        }
        if values.is_empty() {
            (true, true)
        } else {
            (
                values.contains(&result.value),
                values.iter().any(|v| *v != result.value),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hand_built_ast_cannot_bypass_merge_closure_or_yield_placement() {
        let base =
            crate::syntax::parse(include_str!("../../tests/merge_if/models/M1.relca")).unwrap();
        for (fault, reason) in [
            (0, "merge_fallthrough"),
            (1, "merge_unreachable_after_yield"),
            (2, "merge_no_continuing_branch"),
            (3, "merge_yield_outside"),
        ] {
            let mut model = base.clone();
            let process = model
                .items
                .iter_mut()
                .find_map(|i| {
                    if let ast::Item::Process(p) = i {
                        Some(p)
                    } else {
                        None
                    }
                })
                .unwrap();
            let ast::Stmt::Merge {
                condition,
                then_body,
                else_body,
                ..
            } = &mut process.body[2].value
            else {
                panic!();
            };
            let tail = then_body.last().unwrap().clone();
            match fault {
                0 => {
                    then_body.pop();
                }
                1 => then_body.push(ast::Spanned {
                    value: ast::Stmt::End(ast::Spanned {
                        value: "done_fixed".into(),
                        span: tail.span,
                    }),
                    span: tail.span,
                }),
                2 => {
                    then_body.last_mut().unwrap().value = ast::Stmt::End(ast::Spanned {
                        value: "done_fixed".into(),
                        span: tail.span,
                    });
                    else_body.last_mut().unwrap().value = ast::Stmt::End(ast::Spanned {
                        value: "done_as_is".into(),
                        span: tail.span,
                    });
                }
                _ => {
                    then_body.last_mut().unwrap().value = ast::Stmt::If {
                        condition: condition.clone(),
                        then_body: vec![tail.clone()],
                        else_body: vec![tail],
                    }
                }
            }
            let errors = lower(&model).unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|d| d.code == diag::E_CON_0 && d.message == reason),
                "fault {fault}: {errors:?}"
            );
        }
    }

    #[test]
    fn possible_origin_access_matches_duplicated_models_without_claiming_merged_lineage() {
        for (source, duplicated) in [
            (
                include_str!("../../tests/merge_if/models/M1.relca"),
                include_str!("../../tests/merge_if/witnesses/M1.relca"),
            ),
            (
                include_str!("../../tests/merge_if/models/M2.relca"),
                include_str!("../../tests/merge_if/witnesses/M2.relca"),
            ),
            (
                include_str!("../../tests/merge_if/models/M3.relca"),
                include_str!("../../tests/merge_if/witnesses/M3.relca"),
            ),
            (
                include_str!("../../tests/merge_if/models/M4.relca"),
                include_str!("../../tests/merge_if/witnesses/M4.relca"),
            ),
            (
                include_str!("../../tests/merge_if/models/M5.relca"),
                include_str!("../../tests/merge_if/witnesses/M5.relca"),
            ),
        ] {
            let m = lower(&crate::syntax::parse(source).unwrap()).unwrap();
            let d = lower(&crate::syntax::parse(duplicated).unwrap()).unwrap();
            let agency = m.agency.as_derived().unwrap();
            assert_eq!(agency.origin, d.agency.as_derived().unwrap().origin);
            assert!(
                agency
                    .lineage
                    .iter()
                    .all(|r| r.call != "a" && r.call != "d2")
            );
        }
    }

    #[test]
    fn merge_name_cannot_silently_alias_an_observed_case_field() {
        let mut src = include_str!("../../tests/merge_if/models/M1.relca").to_string();
        src.insert_str(
            src.rfind('}').unwrap(),
            "observed case { d2:optional<D> }\n",
        );
        let errors = lower(&crate::syntax::parse(&src).unwrap()).unwrap_err();
        assert!(
            errors.iter().any(|d| d.code == U_SEM_0
                && d.message == "observed binding alias requires a result writer")
        );
    }

    #[test]
    fn yield_spans_exclude_trailing_newlines_for_plain_and_qualified_sources() {
        for text in ["yield f", "yield Clerk.visible"] {
            let src = include_str!("../../tests/merge_if/models/M1.relca")
                .replace("yield f", &format!("{text}\n"));
            let model = crate::syntax::parse(&src).unwrap();
            let process = model
                .items
                .iter()
                .find_map(|i| {
                    if let ast::Item::Process(p) = i {
                        Some(p)
                    } else {
                        None
                    }
                })
                .unwrap();
            let ast::Stmt::Merge { then_body, .. } = &process.body[2].value else {
                panic!();
            };
            let span = then_body.last().unwrap().span;
            assert_eq!(&src[span.start..span.end], text);
        }
    }

    #[test]
    fn nested_merge_reaches_the_syntax_limit_without_stack_abort() {
        let mut body = "d=A.read();".to_string();
        for _ in 0..256 {
            body = format!("x=if d==ok {{ {body} yield d }} else {{ yield d }};");
        }
        let src = format!("model M version \"0.2\" {{ process P {{ {body} end done }} }}");
        let errors = crate::syntax::parse(&src).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|d| d.code == diag::U_SYN_0 && d.message.contains("nesting limit"))
        );
    }

    #[test]
    fn qualified_non_binding_sources_keep_latent_and_observed_separate() {
        let base = include_str!("../../tests/merge_if/models/M1.relca").replace(
            "actor Clerk { capacity 1",
            "actor Clerk { capacity 1; state visible:D=draft; latent secret:D=draft",
        );
        for (name, code, reason) in [
            ("Clerk.secret", diag::E_POL_LATENT, "latent"),
            ("Clerk.visible", U_SEM_0, "merge_source_unsupported"),
            ("D.repaired", U_SEM_0, "merge_source_unsupported"),
            ("missing.value", E_NAME_0, "undefined merge source"),
        ] {
            let src = base.replace("yield f", &format!("yield {name}"));
            let errors = lower(&crate::syntax::parse(&src).unwrap()).unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|d| d.code == code && d.message.contains(reason)),
                "{name}: {errors:?}"
            );
        }
    }

    #[test]
    fn merge_value_union_preserves_singleton_definite_availability() {
        let src = r#"model M version "0.2" {
            type T = enum { ok, no }
            actor A { capacity 1
                capability read() -> T { duration fixed 1min; result nondet { ok } }
                capability use(x:T) -> T { duration fixed 1min; result nondet { ok } }
            }
            process P {
                d=A.read(); d2=if d==ok { yield d } else { yield d }
                if d2==ok { b=A.read() } else { }
                c=A.use(b); end done
            }
            bounds { cases 1; live_tasks 1; clock 10min }
            verify reachable end.done
        }"#;
        let m = lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let e = crate::engine::explore(&m);
        assert_eq!(
            (
                e.terminals.len(),
                e.terminals[0].elapsed.min,
                e.terminals[0].elapsed.max
            ),
            (1, 3, 3)
        );
        let r = crate::reference::explore_with_limits(&m, crate::reference::Limits::default());
        assert_eq!(r.stop, crate::reference::StopReason::Complete);
        assert_eq!(e.terminals, r.outcome.terminals);
    }
}
