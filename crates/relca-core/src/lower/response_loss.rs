//! ADR 0056: declaration checks are independent of dynamic reachability.
use super::*;

impl Lower {
    pub(super) fn response_contract(&mut self, call: &ast::Call) {
        if call.response_loss.is_none() && call.retry.is_none() {
            return;
        }
        let attempts = call.retry.as_ref().map_or(1, |r| r.max_attempts.value);
        if attempts == 0 {
            self.error(
                diag::E_CON_0,
                "invalid_retry_contract: max_attempts must be positive",
                call.span,
            );
        } else if attempts > 32 {
            self.unsupported(
                "retry_expansion_limit: maximum is 32 total attempts",
                call.span,
            );
        }
        if let Some(loss) = &call.response_loss {
            match &loss.value {
                ast::ResponseLoss::Sequence(xs) if xs.len() as u64 != attempts => {
                    self.error(
                        diag::E_CON_0,
                        "invalid_retry_contract: sequence length must equal total attempts",
                        loss.span,
                    );
                }
                ast::ResponseLoss::Probability { probability, basis } => {
                    self.probability(probability);
                    if basis.as_ref().is_none_or(|b| b.value.trim().is_empty()) {
                        self.error(
                            diag::E_CON_0,
                            "response_loss probability requires a nonempty basis",
                            loss.span,
                        );
                    }
                }
                _ => {}
            }
        }
        if let Some(r) = &call.retry {
            self.ticks(&r.backoff);
            if let Some(&cap) = self
                .cap_names
                .get(&(call.actor.value.clone(), call.capability.value.clone()))
            {
                let ty = self.capabilities[cap.0].return_type;
                for result in &r.results {
                    self.value(ty, result);
                }
                if self.capabilities[cap.0].version_check.is_some() {
                    self.unsupported("retry_feature_combination: version_check retry", r.span);
                }
            }
        }
        if let Some(&cap) = self
            .cap_names
            .get(&(call.actor.value.clone(), call.capability.value.clone()))
        {
            let can_lose = match call.response_loss.as_ref().map(|x| &x.value) {
                Some(ast::ResponseLoss::None) => false,
                Some(ast::ResponseLoss::Sequence(xs)) => {
                    xs.iter().any(|x| x.value == ast::Receipt::Unknown)
                }
                Some(ast::ResponseLoss::Probability { probability, .. }) => {
                    self.probability(probability).is_none_or(|p| !p.is_zero())
                }
                _ => true,
            };
            let c = &self.capabilities[cap.0];
            let modeled = self.response_cap_declarations(cap).1;
            if attempts > 1
                && can_lose
                && c.commits
                && !modeled
                && !self.response_mechanism(cap, None)
            {
                self.unsupported("response_loss_resend_mechanism_unmodeled", call.span);
            }
        }
    }

    // Look up declarations by typed identity, not by Actor grain or a Cut name.
    pub(super) fn response_cap_declarations(
        &self,
        id: ir::CapabilityId,
    ) -> (Option<&ast::Ident>, bool) {
        self.response_caps
            .get(&id)
            .map_or((None, false), |(key, modeled)| (key.as_ref(), *modeled))
    }

    pub(super) fn response_mechanism(&self, id: ir::CapabilityId, key: Option<&str>) -> bool {
        let c = &self.capabilities[id.0];
        let Some(a) = &c.atomic else {
            return false;
        };
        let ir::Cond::Contains {
            set,
            element: ir::Element::Parameter(i),
        } = a.condition
        else {
            return false;
        };
        c.commits
            && !a.commit_when
            && c.start_writes.is_empty()
            && c.params
                .get(i)
                .is_some_and(|p| p.ty == ir::CASE_ID_TYPE && key.is_none_or(|key| p.name == key))
            && self.state_variables.get(set.0).is_some_and(|v| {
                v.case_set_size.is_some() && v.kind == ir::StateKind::ActorObserved(c.actor)
            })
            && a.writes.iter().any(|w| {
                w.variable == set && w.source == ir::WriteSource::InsertCase { parameter: i }
            })
    }

    pub(super) fn response_capability(&mut self, id: ir::CapabilityId, cap: &ast::Capability) {
        let mut key = None;
        let mut modeled = false;
        for item in &cap.items {
            match &item.value {
                ast::CapItem::Idempotency(k) => {
                    if key.replace(k.clone()).is_some() {
                        self.error(
                            diag::E_CON_0,
                            "invalid_retry_contract: duplicate idempotency",
                            item.span,
                        );
                    }
                }
                ast::CapItem::EffectScopeModeled => {
                    if modeled {
                        self.error(
                            diag::E_CON_0,
                            "invalid_retry_contract: duplicate effect_scope",
                            item.span,
                        );
                    }
                    modeled = true;
                }
                _ => {}
            }
        }
        if let Some(key) = &key {
            if !self.capabilities[id.0]
                .params
                .iter()
                .any(|p| p.name == key.value)
            {
                self.error(E_NAME_0, "undefined idempotency key parameter", key.span);
            } else if !self.response_mechanism(id, Some(&key.value)) {
                self.unsupported("idempotency_mechanism_unmodeled", key.span);
            }
        }
        self.response_caps.insert(id, (key, modeled));
    }

    pub(super) fn expand_response(&mut self, ast: &ast::Call, call: ir::Call) -> ir::Block {
        let n = ast.retry.as_ref().map_or(1, |r| r.max_attempts.value);
        // Errors have already been recorded; never allocate from an unchecked bound.
        if !(1..=32).contains(&n) {
            return Vec::new();
        }
        let loss = match ast.response_loss.as_ref().map(|x| &x.value) {
            Some(ast::ResponseLoss::None) => ir::ResponseLoss::None,
            Some(ast::ResponseLoss::Sequence(xs)) => {
                ir::ResponseLoss::Sequence(xs.iter().map(|x| x.value).collect())
            }
            Some(ast::ResponseLoss::Probability { probability, basis }) => {
                let Some(p) = self.probability(probability) else {
                    return Vec::new();
                };
                let Some(basis) = basis else {
                    return Vec::new();
                };
                ir::ResponseLoss::Probability {
                    probability: p,
                    basis: basis.value.clone(),
                }
            }
            _ => ir::ResponseLoss::Nondet,
        };
        let mut template = ir::ResponseAttempt {
            recovery: None,
            idempotency: self
                .response_cap_declarations(call.capability)
                .0
                .map(|k| k.value.clone()),
            effect_modeled: self.response_cap_declarations(call.capability).1,
            command_call: call.clone(),
            command: call.binding,
            attempt: 1,
            max_attempts: n,
            loss,
            retry_unknown: ast.retry.as_ref().is_some_and(|r| r.receipt_unknown),
            retry_results: Vec::new(),
            backoff: 0,
            exhausted: None,
            idempotent: self.response_mechanism(call.capability, None),
            retry_span: ast.retry.as_ref().map(|r| r.span),
        };
        if let Some(r) = &ast.retry {
            template.backoff = self.ticks(&r.backoff).unwrap_or(0);
            template.exhausted = self.end(&r.exhausted);
            let ty = self.capabilities[call.capability.0].return_type;
            template.retry_results = r.results.iter().filter_map(|x| self.value(ty, x)).collect();
        }
        (1..=n)
            .map(|attempt| {
                let mut internal = call.clone();
                internal.binding = ir::BindingId(self.bindings.len());
                self.bindings.push(ir::Binding {
                    merge: None,
                    repetition: None,
                    name: format!("@response:{}:{attempt}", call.binding.0),
                    ty: self.bindings[call.binding.0].ty,
                    span: call.span,
                });
                let mut metadata = template.clone();
                metadata.attempt = attempt;
                ir::Statement {
                    kind: ir::Stmt::Response(internal, Box::new(metadata)),
                    span: call.span,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_sites_keep_original_command_input_and_source() {
        let source = r#"model M version "0.2" {
            type V = enum { ok, fail }
            actor A { capacity 1; capability c() -> V {
                duration fixed 1min; result nondet { ok, fail }
            } }
            process P { r = A.c() response_loss none
                retry max_attempts 3 when result [fail] backoff fixed 2min exhausted end failed;
                end done }
            bounds { cases 1; live_tasks 1; clock 20min }
            verify safety no_resource_overcommit
        }"#;
        let m = lower(&crate::syntax::parse(source).unwrap()).unwrap();
        let mut sites = BTreeSet::new();
        for (index, statement) in m.process.body[..3].iter().enumerate() {
            let ir::Stmt::Response(call, metadata) = &statement.kind else {
                panic!("expanded attempt");
            };
            assert!(sites.insert(call.binding));
            assert_ne!(call.binding, metadata.command);
            assert_eq!(metadata.command, ir::BindingId(0));
            assert_eq!(metadata.attempt, index as u64 + 1);
            assert_eq!(metadata.max_attempts, 3);
            assert_eq!(metadata.backoff, 2);
            assert_eq!(metadata.loss, ir::ResponseLoss::None);
            assert_eq!(call.span, m.process.body[0].span);
            assert_eq!(call.args, Vec::new());
        }
    }
}
