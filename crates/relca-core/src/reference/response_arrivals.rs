use super::*;

impl<'a> Tree<'a> {
    pub(super) fn response_observe(
        &mut self,
        w: &World<'a>,
        k: usize,
        event: &Occurrence,
    ) -> Execution {
        if let Occurrence::Channel(r) = event {
            let call = self
                .m
                .response_attempts()
                .into_iter()
                .find(|(c, _)| c.binding == r.binding)
                .ok_or(StopReason::SemanticAbort)?
                .0;
            let cap = &self.m.capabilities[call.capability.0];
            let result = if r.observation == crate::ast::Receipt::Delivered {
                Some(self.m.types[cap.return_type.0].members[r.actual.0].clone())
            } else {
                None
            };
            let row = crate::outcome::ResponseReceipt {
                case_id: k as u64 + 1,
                command: self.m.site_name(r.command),
                attempt: r.attempt,
                capability: format!("{}.{}", self.m.actors[cap.actor.0].name, cap.name),
                receipt: r.observation,
                at: singleton(w.now),
                observed_result: result,
                resolved_effect: r.resolved,
            };
            if let Some(old) = self.response_receipts.iter_mut().find(|old| {
                old.case_id == row.case_id
                    && old.command == row.command
                    && old.attempt == row.attempt
                    && old.receipt == row.receipt
                    && old.observed_result == row.observed_result
                    && old.resolved_effect == row.resolved_effect
            }) {
                extend(&mut old.at, w.now);
            } else {
                self.response_receipts.push(row);
            }
        }
        if matches!(event, Occurrence::ResponseStopped(_)) {
            let c = w.cases[k].payload().ok_or(StopReason::SemanticAbort)?;
            let work = c
                .frame
                .tasks
                .iter()
                .filter_map(|t| t.start.map(|start| t.stop.unwrap_or(w.now) - start))
                .sum();
            let row = self.terminals[k]
                .entry(Kind::ReceiptUnknown)
                .or_insert(CaseTerminal {
                    kind: Kind::ReceiptUnknown,
                    completed_at: None,
                    elapsed: None,
                    work: None,
                    result_at: None,
                    probability: None,
                    observed_until: Some(singleton(w.now)),
                    observed_elapsed: Some(singleton(w.now - self.times[k])),
                    observed_work: Some(singleton(work)),
                });
            extend(row.observed_until.as_mut().unwrap(), w.now);
            extend(
                row.observed_elapsed.as_mut().unwrap(),
                w.now - self.times[k],
            );
            extend(row.observed_work.as_mut().unwrap(), work);
        }
        Ok(())
    }
    pub(super) fn response_progress(&mut self, w: &mut World<'a>, k: usize) -> Execution {
        let c = w.cases[k].payload_mut().ok_or(StopReason::SemanticAbort)?;
        let (decision, events) = response::progress(&mut c.frame, self.m, w.now)?;
        for event in events {
            self.record(w, k, event, None, false)?;
        }
        if let response::Progress::End(end) = decision {
            self.ending(w, k, end)?;
        }
        Ok(())
    }

    pub(super) fn response_finish(
        &mut self,
        w: World<'a>,
        k: usize,
        binding: BindingId,
        actual: ValueId,
    ) -> Result<Vec<Bound>, StopReason> {
        let Some(r) = self.m.response_attempt(binding) else {
            let frame = &w.cases[k].payload().ok_or(StopReason::SemanticAbort)?.frame;
            let cap = frame
                .tasks
                .iter()
                .find(|t| t.call.binding == binding)
                .ok_or(StopReason::SemanticAbort)?
                .call
                .capability;
            if self.m.capabilities[cap.0].receipt_query.is_some() {
                let event =
                    recovery::query(frame, self.m, binding, actual, &self.values(&w, k), k)?;
                let mut next = w;
                self.record_context(&mut next, k, event, None, (false, Some(binding)))?;
                return self.walk(next);
            }
            return self.walk(w);
        };
        use crate::ast::Receipt::{Delivered, Unknown};
        let choices = match &r.loss {
            ResponseLoss::None => vec![(Some(Rational::one()), Delivered)],
            ResponseLoss::Sequence(xs) => vec![(Some(Rational::one()), xs[r.attempt as usize - 1])],
            ResponseLoss::Nondet => vec![(None, Delivered), (None, Unknown)],
            ResponseLoss::Probability { probability: p, .. } => {
                let mut options = Vec::new();
                if p != &Rational::one() {
                    options.push((Some(Rational(Rational::one().0 - &p.0)), Delivered));
                }
                if !p.is_zero() {
                    options.push((Some(p.clone()), Unknown));
                }
                options
            }
        };
        let chance = choices.iter().all(|(p, _)| p.is_some());
        // A deterministic channel is part of the atomic completion, not an
        // extra recursive choice. Preserve the caller's semantic depth budget.
        if choices.len() == 1 {
            let observation = choices[0].1;
            let mut next = w;
            let c = next.cases[k].payload().ok_or(StopReason::SemanticAbort)?;
            let was_unknown = c.frame.trace.iter().any(|o| matches!(&o.event, Occurrence::Channel(old) if old.command == r.command && old.attempt < r.attempt && !old.resolved));
            let receipt = response::Receipt {
                #[cfg(test)]
                weight: choices[0].0.clone(),
                command: r.command,
                binding,
                attempt: r.attempt,
                actual,
                observation,
                resolved: observation == Delivered && (r.idempotent || !was_unknown),
            };
            self.record_context(
                &mut next,
                k,
                Occurrence::Channel(receipt),
                None,
                (false, Some(binding)),
            )?;
            return self.walk_inner(next);
        }
        let mut children = Vec::new();
        for (p, observation) in choices {
            let mut next = self.copy(&w)?;
            let c = next.cases[k].payload().ok_or(StopReason::SemanticAbort)?;
            let was_unknown = c.frame.trace.iter().any(|o| matches!(&o.event, Occurrence::Channel(old) if old.command == r.command && old.attempt < r.attempt && !old.resolved));
            let receipt = response::Receipt {
                #[cfg(test)]
                weight: p.clone(),
                command: r.command,
                binding,
                attempt: r.attempt,
                actual,
                observation,
                resolved: observation == Delivered && (r.idempotent || !was_unknown),
            };
            self.record_context(
                &mut next,
                k,
                Occurrence::Channel(receipt),
                None,
                (false, Some(binding)),
            )?;
            children.push((p.unwrap_or_else(Rational::one), self.walk(next)?));
        }
        Ok(self.reduce(children, chance))
    }
}
