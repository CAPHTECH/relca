//! Resolve declarations, never dispatch Tasks. All scenarios enter this function.
use super::*;

pub(super) fn lower(
    source: &ast::Model,
    m: &mut ir::Model,
    called: &BTreeSet<ir::ActorId>,
) -> Result<(), Vec<Diagnostic>> {
    let declarations: Vec<_> = source
        .items
        .iter()
        .filter_map(|i| {
            if let ast::Item::CasePriority(p) = i {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    let used = m
        .start_policy
        .as_ref()
        .is_some_and(|p| p.priority.is_some())
        || m.external_priority().is_some()
        || m.capabilities.iter().any(|c| {
            c.version_check
                .as_ref()
                .is_some_and(|v| v.resolve.is_some())
        });
    if declarations.is_empty() && !used {
        return Ok(());
    }
    let mut errors = Vec::new();
    let mut refuse = |code: &str, reason: &str, span: Span| {
        errors.push(Diagnostic {
            code: code.into(),
            severity: diag::Severity::Error,
            message: reason.into(),
            span: Some(span),
        });
    };
    if declarations.len() > 1 {
        refuse(U_SEM_0, "duplicate_case_priority", declarations[1].span);
    }
    let mut classes = ir::PolicyClasses::default();
    for (i, actor) in m.actors.iter().enumerate() {
        let id = ir::ActorId(i);
        match actor
            .metadata
            .control
            .as_deref()
            .unwrap_or(if called.contains(&id) {
                "policy"
            } else {
                "environment"
            }) {
            "policy" => classes.policy.push(id),
            "environment" => classes.environment.push(id),
            "party" => classes.party.push(id),
            _ => refuse(U_SEM_0, "invalid_priority_ir", actor.span),
        }
    }
    if let Some(table) = declarations.first() {
        if table.value.len() as u64 != m.bounds.cases {
            refuse(diag::E_ARR_0, "priority_case_count_mismatch", table.span);
        }
        m.case_priority = Some(ir::CasePriority {
            ranks: table.value.iter().map(|n| n.value).collect(),
            classes: classes.clone(),
            span: table.span,
        });
    }
    if used && m.case_priority.is_none() {
        refuse(U_SEM_0, "priority_not_configured", source.span);
    }
    for cap in &m.capabilities {
        let Some(check) = &cap.version_check else {
            continue;
        };
        if check.resolve.is_some() && (!classes.policy.contains(&cap.actor)
            || check.variables.is_empty()
            || check.variables.iter().any(|v| !matches!(m.state_variables[v.0].kind, ir::StateKind::ActorObserved(a) if a == cap.actor && !m.actors[a.0].is_per_case()))) {
            refuse(U_SEM_0, "priority_resolution_scope_unsupported", check.span);
        }
        for other in &m.capabilities {
            if let Some(b) = &other.version_check
                && cap.actor == other.actor
                && check.variables.iter().any(|v| b.variables.contains(v))
                && check.resolve != b.resolve
            {
                refuse(U_SEM_0, "priority_resolution_mismatch", check.span);
            }
        }
    }
    if errors.is_empty() {
        if let Some(wire) = crate::report::priority::record(m) {
            if let Some(p) = &m.start_policy
                && let Some(tie) = p.priority
            {
                for a in m
                    .assumptions
                    .iter_mut()
                    .filter(|a| a.field == "policy.queue")
                {
                    a.value = format!(
                        "{}priority descending tie {}{}",
                        if p.observable { "observable " } else { "" },
                        match tie {
                            ir::QueueTie::Nondet => "nondet",
                            ir::QueueTie::FifoCaseId => "fifo tie_by case_id",
                        },
                        if p.call_order { " then call_order" } else { "" }
                    );
                    a.affects = "policy_queued_only: compare current policy Queued Tasks by descending case rank, with only the explicit tie. Actor/uses predecessors block overtaking even when stopped; people are not reserved. Only executable candidates compete for the next start/refusal. environment/party and cross-class choices remain nondeterministic without an implicit FIFO. Running work and Tasks created later in the same instant are not reordered.".into();
                }
            }
            m.assumptions.push(ir::Assumption {
                field: "case_priority".into(),
                value: serde_json::to_string(&wire).expect("priority declaration wire"),
                affects: "CaseId order; descending; known at receipt and immutable. Only policy_queued_only starts, held_readmission_only external admission, and due_checked_intersection_only version resolution are opt-in. Omitted ties are nondet. Initial admission remains CaseId order; running work, stale receipts, acknowledgments, future finishes and environment/party choices are not reordered. The model's admission/assignment rules are not separate Actors. Bounds are not an optimal implementable policy. Head-of-line waiting without physical obstruction remains unattributed, not a causal priority_wait metric.".into(),
            });
            if !used {
                m.assumptions.push(ir::Assumption {
                    field: "case_priority.usage".into(),
                    value: "unused".into(),
                    affects: "The table alone does not change any ordering.".into(),
                });
            }
        }
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn priority_assumptions_replace_legacy_fifo_claim_only_for_opted_in_starts() {
        for (policy, expected) in [
            (
                "policy observable priority descending",
                "observable priority descending tie nondet",
            ),
            (
                "policy priority descending tie fifo tie_by case_id then call_order",
                "priority descending tie fifo tie_by case_id then call_order",
            ),
        ] {
            let source = format!(
                r#"model M version "0.2" {{
                type R = enum {{ ok }} actor A {{ capacity 1; capability c() -> R {{ duration fixed 1min; result nondet {{ ok }} }} }}
                process Flow {{ r = A.c(); end done }}
                bounds {{ cases 1; live_tasks 2; clock 5min }} case_priority fixed [9]
                {policy} verify safety no_resource_overcommit
            }}"#
            );
            let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
            let row = m
                .assumptions
                .iter()
                .find(|a| a.field == "policy.queue")
                .unwrap();
            assert_eq!(row.value, expected);
            assert!(row.affects.contains("policy_queued_only"));
            assert!(row.affects.contains("environment/party"));
            assert!(!row.affects.contains("by queue-entry time then case ID"));
        }
    }
}
