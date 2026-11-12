use super::*;

pub(super) fn supported(m: &ir::Model, span: Span) -> Result<(), Vec<Diagnostic>> {
    if m.uses_work_profiles()
        && (m.arrival_distribution.is_some()
            || !m.contests().is_empty()
            || m.response_attempts().iter().any(|(_, r)| {
                !matches!(r.loss, ir::ResponseLoss::None)
                    || r.max_attempts != 1
                    || r.retry_span.is_some()
            }))
    {
        return Err(vec![Diagnostic {
            code: U_SEM_0.into(),
            severity: diag::Severity::Error,
            message: "q6_feature_combination: unconnected arrival, contest or response control"
                .into(),
            span: Some(span),
        }]);
    }
    Ok(())
}

impl Lower {
    pub(super) fn performance_bindings(&mut self) {
        let mut contracts = BTreeMap::new();
        let mut mismatches = Vec::new();
        for cap in &self.capabilities {
            let Some(source) = &cap.performance else {
                continue;
            };
            let Some(first) = source.kernels.first() else {
                continue;
            };
            let mut reads = cap.kernel_reads_latent.clone();
            reads.sort();
            let targets = first
                .rows
                .iter()
                .flat_map(|r| &r.branches)
                .flat_map(|b| &b.overrides)
                .map(|(v, _)| *v)
                .collect::<BTreeSet<_>>();
            let contract = (
                cap.return_type,
                cap.params.iter().map(|p| p.ty).collect::<Vec<_>>(),
                first.given.clone(),
                reads,
                targets,
            );
            if contracts
                .get(&source.profile)
                .is_some_and(|prior| *prior != contract)
            {
                mismatches.push(cap.span);
            } else {
                contracts.insert(source.profile, contract);
            }
        }
        for span in mismatches {
            self.error(E_TYPE_0, "performance_contract_mismatch", span);
        }
    }
    pub(super) fn performance_declarations(&mut self, model: &ast::Model) {
        let declarations: Vec<_> = model
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Performance(p) => Some(p),
                _ => None,
            })
            .collect();
        if declarations.len() > 256
            || declarations
                .iter()
                .flat_map(|p| &p.rows)
                .map(|r| r.bands.len())
                .sum::<usize>()
                > 4096
        {
            self.unsupported("q6_capacity_limit", model.span);
            return;
        }
        for p in declarations {
            if self.performances.iter().any(|old| old.name == p.name.value) {
                self.unsupported("q6_feature_combination: duplicate performance", p.span);
            }
            if p.basis.value.trim().is_empty() {
                self.error(diag::E_CON_0, "performance_basis_missing", p.basis.span);
            }
            let mut rows = Vec::new();
            let mut people = BTreeSet::new();
            if p.rows.is_empty() {
                self.error(E_TYPE_0, "performance_coverage", p.span);
            }
            for executor in &p.rows {
                let person = if executor.executor.value == "anonymous" {
                    None
                } else {
                    match self.person_names.get(&executor.executor.value).copied() {
                        Some(person) => Some(person),
                        None => {
                            self.error(
                                E_NAME_0,
                                "unknown performance executor",
                                executor.executor.span,
                            );
                            continue;
                        }
                    }
                };
                if !people.insert(person) || executor.bands.is_empty() {
                    self.error(E_TYPE_0, "performance_coverage", p.span);
                }
                let mut next = Some(0);
                for b in &executor.bands {
                    if next != Some(b.from) || b.until.is_some_and(|end| end <= b.from) {
                        self.error(E_TYPE_0, "performance_coverage", b.kernel.span);
                    }
                    next = b.until;
                    if let Some(kernel) = self.kernels.iter().position(|k| k.name == b.kernel.value)
                    {
                        rows.push(ir::PerformanceRow {
                            executor: person,
                            from: b.from,
                            until: b.until,
                            kernel,
                        });
                    } else {
                        self.error(E_NAME_0, "unknown performance kernel", b.kernel.span);
                    }
                }
                if next.is_some() || !p.queued && executor.bands.len() != 1 {
                    self.error(E_TYPE_0, "performance_coverage", p.span);
                }
            }
            self.performances.push(ir::Performance {
                name: p.name.value.clone(),
                basis: p.basis.value.clone(),
                queued: p.queued,
                rows,
            });
        }
    }

    pub(super) fn performance_use(
        &mut self,
        cap: &ast::Capability,
        actor: ir::ActorId,
        ty: Option<ir::TypeId>,
        writes: &[ir::StateWrite],
        qualified: Option<&[ir::PersonId]>,
    ) -> Option<ir::PerformanceSource> {
        let sources: Vec<_> = cap
            .items
            .iter()
            .filter_map(|i| match &i.value {
                ast::CapItem::OutcomesPerformance(n) => Some(n),
                _ => None,
            })
            .collect();
        let name = *sources.first()?;
        if qualified.is_some() && self.actors[actor.0].executors.is_none() {
            return None;
        }
        if sources.len() != 1
            || cap.items.iter().any(|i| {
                matches!(
                    i.value,
                    ast::CapItem::OutcomesKernel(_)
                        | ast::CapItem::ResultNondet(_)
                        | ast::CapItem::Atomic { .. }
                        | ast::CapItem::VersionCheck { .. }
                        | ast::CapItem::DurationDistribution(_)
                )
            })
            || qualified.is_some_and(<[ir::PersonId]>::is_empty)
        {
            self.unsupported(
                "q6_feature_combination: performance result source",
                cap.span,
            );
            return None;
        }
        let Some(profile) = self.performances.iter().position(|p| p.name == name.value) else {
            self.error(E_NAME_0, "unknown performance profile", name.span);
            return None;
        };
        let p = self.performances[profile].clone();
        let expected: BTreeSet<_> = qualified
            .or(self.actors[actor.0].executors.as_deref())
            .map_or_else(
                || [None].into_iter().collect(),
                |ps| ps.iter().map(|p| Some(*p)).collect(),
            );
        if p.rows.iter().map(|r| r.executor).collect::<BTreeSet<_>>() != expected {
            self.error(E_TYPE_0, "performance_coverage", cap.span);
            return None;
        }
        // Check the static contract before typing each use. In particular a
        // missing override is a selector contract mismatch, not a default value.
        let signature = |kernel: &ast::KernelDecl| {
            kernel
                .rows
                .iter()
                .flat_map(|r| &r.branches)
                .flat_map(|b| &b.assignments)
                .map(|a| target_name(&a.target))
                .collect::<BTreeSet<_>>()
        };
        let mut contract = None;
        for row in &p.rows {
            let declaration = &self.kernel_declarations[&self.kernels[row.kernel].name];
            let current = (
                self.kernels[row.kernel].given.clone(),
                signature(declaration),
            );
            if contract
                .as_ref()
                .is_some_and(|previous| *previous != current)
            {
                self.error(E_TYPE_0, "performance_contract_mismatch", cap.span);
                return None;
            }
            contract = Some(current);
        }
        let mut kernels = Vec::new();
        for row in &p.rows {
            let mut use_ = cap.clone();
            use_.items
                .retain(|i| !matches!(i.value, ast::CapItem::OutcomesPerformance(_)));
            use_.items.push(ast::Spanned {
                value: ast::CapItem::OutcomesKernel(ast::Spanned {
                    value: self.kernels[row.kernel].name.clone(),
                    span: name.span,
                }),
                span: name.span,
            });
            if let (Some(kernel), _, _) = self.kernel_use(&use_, actor, ty, writes) {
                kernels.push(kernel);
            }
        }
        Some(ir::PerformanceSource { profile, kernels })
    }
}

#[cfg(test)]
mod tests {
    use crate::{lower, q6_acceptance::input, syntax};

    #[test]
    fn equal_given_variables_at_different_source_spans_share_a_contract() {
        let s=input::source("given-finish").replace("load none; executor anonymous { kernel K }",
            "load queued_same_actor_at_start; executor anonymous { [0,1): kernel K; [1,infinity): kernel K2 }");
        let s = input::append(
            &s,
            "kernel K2 given [flag] { false -> { 1: ok with hit = false }; true -> { 0.5: ok with hit = true; 0.5: ok with hit = false } }",
        );
        let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
        let kernels = &m
            .capabilities
            .iter()
            .find(|c| c.name == "sample")
            .unwrap()
            .performance
            .as_ref()
            .unwrap()
            .kernels;
        assert_eq!(kernels.len(), 2);
        assert_eq!(kernels[0].given, kernels[1].given);
    }

    #[test]
    fn declared_latent_read_contract_is_not_replaced_by_kernel_given_subset() {
        let s = input::source("people")
            .replace(
                "latent case { bad: bool = false }",
                "latent case { bad: bool = false; extra: bool = false }",
            )
            .replace(
                "outcomes performance P",
                "outcomes performance P; kernel_reads latent [extra]",
            );
        let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
        let id = crate::ir::StateVarId(
            m.state_variables
                .iter()
                .position(|v| v.name == "extra")
                .unwrap(),
        );
        assert!(m.capabilities[0].kernel_reads_latent.contains(&id));
    }

    #[test]
    fn a_profile_cannot_silently_change_its_bound_operation_contract() {
        for signature in ["other(x: R) -> R", "other() -> S"] {
            let s = input::source("people").replace("include [Desk]", "include [Desk,Other]");
            let s = input::append(
                &s,
                &format!(
                    "type S = enum {{ ok, bad, denied }} actor Other {{ executors [p,q]; capability {signature} {{ duration fixed 2min; outcomes performance P; writes bad }} }}"
                ),
            );
            let errors = lower::lower(&syntax::parse(&s).unwrap())
                .expect_err("profile input/output contract changes");
            assert!(
                errors.iter().any(|d| d.code == crate::diag::E_TYPE_0
                    && d.message.contains("performance_contract_mismatch")),
                "{errors:?}"
            );
        }
    }
}
