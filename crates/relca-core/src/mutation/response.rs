//! Declared command fault sites; source order and the old shared budget remain
//! independent of the displayed kind order.
use super::*;

#[derive(Clone)]
struct Candidate {
    process: usize,
    path: Vec<(usize, bool)>,
    index: usize,
    site: Site,
    exhausted: String,
}

impl Candidate {
    fn apply(&self, model: &ast::Model, resend: bool) -> ast::Model {
        let mut edited = model.clone();
        let ast::Item::Process(p) = &mut edited.items[self.process] else {
            unreachable!()
        };
        let ast::Stmt::Call(call) = &mut block_at(&mut p.body, &self.path)[self.index].value else {
            unreachable!()
        };
        let span = call.span;
        call.response_loss = Some(ast::Spanned {
            span,
            value: ast::ResponseLoss::Sequence(
                if resend {
                    vec![ast::Receipt::Unknown, ast::Receipt::Delivered]
                } else {
                    vec![ast::Receipt::Unknown]
                }
                .into_iter()
                .map(|value| ast::Spanned { value, span })
                .collect(),
            ),
        });
        call.retry = resend.then(|| ast::Retry {
            max_attempts: ast::Spanned { value: 2, span },
            receipt_unknown: true,
            results: vec![],
            backoff: ast::Duration {
                amount: ast::Spanned { value: 0, span },
                unit: ast::Spanned {
                    value: "min".into(),
                    span,
                },
                span,
            },
            exhausted: ast::Spanned {
                value: self.exhausted.clone(),
                span,
            },
            span,
        });
        edited
    }
}

fn enumerate(src: &str, ast: &ast::Model, m: &ir::Model) -> (Vec<Candidate>, Vec<SkippedSite>) {
    struct Walk<'a> {
        src: &'a str,
        m: &'a ir::Model,
        process: usize,
        name: String,
        candidates: Vec<Candidate>,
        skipped: Vec<SkippedSite>,
        exhausted: String,
    }
    impl Walk<'_> {
        fn call(&mut self, c: &ast::Call, path: &[(usize, bool)], index: usize, fork: bool) {
            let Some(cap) = self.m.capabilities.iter().find(|cap| {
                cap.name == c.capability.value
                    && self.m.actors[cap.actor.0].name == c.actor.value
                    && cap.commits
            }) else {
                return;
            };
            let mut s = site(self.src, c.span);
            s.process = Some(self.name.clone());
            s.actor = Some(c.actor.value.clone());
            s.capability = Some(c.capability.value.clone());
            s.affected_bindings = vec![c.binding.value.clone()];
            s.statement_path = Some(format!("{path:?}/{index}"));
            let reason = if fork {
                Some("fork_branch")
            } else if c.retry.is_some() {
                Some("existing_retry")
            } else if c
                .response_loss
                .as_ref()
                .is_some_and(|r| !matches!(r.value, ast::ResponseLoss::None))
            {
                Some("existing_response_loss")
            } else if c.response_loss.is_none() {
                Some("undeclared_site")
            } else if !cap
                .params
                .iter()
                .zip(&c.args)
                .any(|(p, a)| p.ty == ir::CASE_ID_TYPE && a.value == "case.id")
            {
                Some("unsupported_input")
            } else {
                None
            };
            if let Some(reason) = reason {
                self.skipped.push(SkippedSite {
                    cohort: None,
                    status: "not_applicable",
                    reason,
                    site: Some(s),
                });
            } else {
                self.candidates.push(Candidate {
                    process: self.process,
                    path: path.to_vec(),
                    index,
                    site: s,
                    exhausted: self.exhausted.clone(),
                });
            }
        }
        fn block(&mut self, b: &ast::Block, path: &mut Vec<(usize, bool)>) {
            for (index, s) in b.iter().enumerate() {
                match &s.value {
                    ast::Stmt::Call(c) => self.call(c, path, index, false),
                    ast::Stmt::Fork { branches, .. } => {
                        for c in branches {
                            self.call(c, path, index, true)
                        }
                    }
                    ast::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        path.push((index, true));
                        self.block(then_body, path);
                        path.pop();
                        path.push((index, false));
                        self.block(else_body, path);
                        path.pop();
                    }
                    _ => {}
                }
            }
        }
    }
    let mut end = "receipt_exhausted".to_owned();
    let mut n = 0;
    while m.ends.contains(&end) {
        n += 1;
        end = format!("receipt_exhausted{n}");
    }
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for (process, item) in ast.items.iter().enumerate() {
        if let ast::Item::Process(p) = item
            && p.name.value == m.process.name
        {
            let mut walk = Walk {
                src,
                m,
                process,
                name: p.name.value.clone(),
                candidates: vec![],
                skipped: vec![],
                exhausted: end.clone(),
            };
            walk.block(&p.body, &mut Vec::new());
            out.extend(walk.candidates);
            skipped.extend(walk.skipped);
        }
    }
    (out, skipped)
}

pub(super) fn run(
    src: &str,
    ast: &ast::Model,
    context: &ir::Model,
    original: &report::Report,
    r: &mut MutationReport,
    budget: (&mut usize, usize),
    execute: &mut impl FnMut(&str, &ast::Model) -> report::Report,
) {
    let (attempts, limit) = budget;
    let (candidates, skipped) = enumerate(src, ast, context);
    for index in [3, 6] {
        let row = &mut r.mutations[index];
        row.skipped_sites = Some(skipped.clone());
        let mut counts = BTreeMap::new();
        for s in &skipped {
            *counts.entry(s.reason.to_owned()).or_insert(0) += 1;
        }
        row.skipped_counts = Some(counts);
        row.status = if candidates.is_empty() {
            "not_applicable"
        } else {
            "generated"
        };
        row.reason = candidates.is_empty().then_some(if skipped.is_empty() {
            "no_matching_site"
        } else {
            "no_supported_site"
        });
        for c in &candidates {
            let mut mutant = MutantRow {
                id: format!(
                    "{}:{}:{}",
                    row.kind,
                    c.site.process.as_deref().unwrap(),
                    c.site.span.start
                ),
                site: c.site.clone(),
                operator: if index == 3 {
                    "lose_response_and_resend"
                } else {
                    "lose_response"
                },
                result: "not_run",
                reason: Some("mutant_limit"),
                report: None,
                assigned_changes: vec![],
                other_changes: vec![],
            };
            if *attempts < limit {
                *attempts += 1;
                let edited = c.apply(ast, index == 3);
                let report = execute(src, &edited);
                let comparison = compare_fault(index, &c.site, context, original, &report, &edited);
                mutant.result = comparison.result;
                mutant.reason = comparison.reason;
                mutant.assigned_changes = comparison.assigned;
                mutant.other_changes = comparison.other;
                mutant.report = Some(report);
            } else {
                r.run.limit_reached = true;
            }
            row.mutants.push(mutant);
        }
    }
    if !r.run.limit_reached {
        stage(r, "command_resend");
        stage(r, "receipt_unknown");
    }
}

fn compare_fault(
    index: usize,
    site: &Site,
    context: &ir::Model,
    before: &report::Report,
    after: &report::Report,
    edited: &ast::Model,
) -> Comparison {
    if after.case_outcomes.as_ref().is_some_and(|cs| {
        cs.iter().any(|c| {
            c.terminals
                .iter()
                .any(|t| matches!(t.kind, report::CaseTerminalKind::CutOff))
        })
    }) || after
        .response_analysis
        .as_ref()
        .is_some_and(|a| a.coverage != report::ResponseCoverage::Complete)
    {
        if let Some(c) = comparison_failure(before, after, false) {
            return c;
        }
        return Comparison {
            result: "inconclusive",
            reason: Some("assigned_check_unavailable"),
            assigned: vec![],
            other: vec![],
        };
    }
    let complete_receipt_exception = index == 6
        && after.bounds.truncated == Some(false)
        && after.verdict == Verdict::Inconclusive
        && after
            .properties
            .iter()
            .any(|p| p.id == "receipt_resolution" && p.verdict == Verdict::Inconclusive)
        && after
            .properties
            .iter()
            .filter(|p| p.id != "receipt_resolution")
            .all(|p| {
                matches!(
                    p.verdict,
                    Verdict::Satisfied | Verdict::Violated | Verdict::Vacuous
                )
            })
        && after.state_range_violations.is_empty();
    if !complete_receipt_exception && let Some(c) = comparison_failure(before, after, false) {
        return c;
    }
    let mut c = Comparison {
        result: "undetected",
        reason: Some("no_assigned_check"),
        assigned: vec![],
        other: vec![],
    };
    other_changes(before, after, &mut c.other);
    if index == 3 {
        let cap = context.capabilities.iter().position(|cap| {
            site.actor.as_deref() == Some(context.actors[cap.actor.0].name.as_str())
                && site.capability.as_deref() == Some(cap.name.as_str())
        });
        let Some(cap) = cap else {
            return c;
        };
        let mut applicable = false;
        for (i, clause) in context.contracts.iter().enumerate() {
            if !matches!(clause.kind,ir::ContractKind::AtMostOnce {event} if event.capability.0==cap && event.kind==ir::EventKind::Commit)
            {
                continue;
            }
            let id = clause.property_id(ir::ClauseId(i));
            let (Some(b), Some(a)) = (
                before.properties.iter().find(|p| p.id == id),
                after.properties.iter().find(|p| p.id == id),
            ) else {
                c.result = "inconclusive";
                c.reason = Some("assigned_check_unavailable");
                return c;
            };
            applicable = true;
            if matches!(b.verdict, Verdict::Satisfied | Verdict::Vacuous)
                && a.verdict == Verdict::Violated
            {
                c.result = "detected";
                c.reason = None;
                property_change(&mut c.assigned, &id, "verdict", b.verdict, a.verdict);
            } else if b.verdict == Verdict::Satisfied && a.verdict == Verdict::Satisfied {
                c.assigned.push(Change {
                    check: "at_most_once".into(),
                    key: id,
                    field: "verdict".into(),
                    before: Payload::Text("satisfied".into()),
                    after: Payload::Text("satisfied".into()),
                    direction: "held",
                });
            }
        }
        if applicable && c.result != "detected" {
            c.reason = Some("no_worsening");
        }
    } else {
        if after.response_receipts.is_none()
            || after.response_analysis.is_none()
            || after.case_outcomes.is_none()
            || !after.state_range_violations.is_empty()
        {
            c.result = "inconclusive";
            c.reason = Some("assigned_check_unavailable");
            return c;
        }
        let command = &site.affected_bindings[0];
        let model = lower::lower_with_selection(
            edited,
            None,
            context.scenario.as_ref().map(|s| s.name.as_str()),
        );
        let proof = model
            .ok()
            .and_then(|m| engine::receipt_unknown_held(&m, command));
        match proof {
            None => {
                c.result = "inconclusive";
                c.reason = Some("assigned_check_unavailable");
            }
            Some((false, _)) => {}
            Some((true, true)) => {
                let wire = after.response_receipts.as_ref().unwrap().iter().any(|r| {
                    r.command == *command
                        && r.receipt == ast::Receipt::Unknown
                        && r.observed_result.is_none()
                });
                if !wire {
                    c.result = "inconclusive";
                    c.reason = Some("assigned_check_unavailable");
                    return c;
                }
                if !after
                    .properties
                    .iter()
                    .any(|p| p.id == "receipt_resolution" && p.verdict == Verdict::Inconclusive)
                {
                    c.reason = Some("receipt_promoted_to_success");
                    return c;
                }
                c.result = "detected";
                c.reason = None;
                c.assigned.push(Change {
                    check: "receipt_unknown".into(),
                    key: command.clone(),
                    field: "joint_path_binding_unknown_effect_retained_no_end_or_completion".into(),
                    before: Payload::Null,
                    after: Payload::Bool(true),
                    direction: "held",
                });
            }
            Some((true, false)) => {
                c.reason = Some("receipt_promoted_to_success");
            }
        }
    }
    c.other.retain(|other| {
        !c.assigned
            .iter()
            .any(|assigned| assigned_projection(assigned, other))
    });
    c
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
