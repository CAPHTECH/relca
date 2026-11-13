//! Explicit continuations for a bounded expanded process. The path interpreter
//! still walks its own source tree and time ticks; no engine program, visited
//! map, state interning, or schedule sampler is reused here.
use super::*;
use std::rc::Rc;
#[cfg(test)]
#[path = "round2_choice_context.rs"]
mod round2_choice_context;
type Action<'a> = Box<dyn FnOnce(&mut Interpreter<'a>, &Frame<'a>, Frame<'a>) -> Execution + 'a>;
type InPlace<'a> = Box<dyn FnOnce(&mut Interpreter<'a>, &Frame<'a>) -> Execution + 'a>;
type Owner = Rc<std::cell::Cell<Option<usize>>>;

struct Choice {
    values: ReferenceChoices,
    weights: Option<Vec<Rational>>,
}

pub(super) enum CancelStep<'a> {
    Run(Frame<'a>, usize),
    Acks {
        frame: Frame<'a>,
        index: usize,
        end: usize,
        winner: BindingId,
        depth: usize,
        max_delay: u64,
    },
}

enum Job<'a> {
    InPlace {
        owner: Owner,
        base: Rc<Frame<'a>>,
        action: InPlace<'a>,
    },
    Action {
        id: usize,
        base: Rc<Frame<'a>>,
        depth: usize,
        action: Action<'a>,
    },
    Reduce(usize),
    FoldAck {
        parent: usize,
        child: usize,
    },
    Abort(StopReason),
    Run {
        id: usize,
        frame: Frame<'a>,
        depth: usize,
    },
    Cancel {
        id: usize,
        frame: Frame<'a>,
        index: usize,
        end: usize,
        winner: BindingId,
        depth: usize,
    },
    Ack {
        parent: usize,
        frame: Frame<'a>,
        index: usize,
        end: usize,
        winner: BindingId,
        depth: usize,
        delay: u64,
        max_delay: u64,
    },
}

#[derive(Default)]
pub(super) struct Work<'a> {
    pending: Vec<Job<'a>>,
    choices: Vec<Option<Choice>>,
    free: Vec<usize>,
    base: Option<(Owner, Rc<Frame<'a>>)>,
}

impl<'a> Work<'a> {
    fn bind_unfinished(&mut self, root: usize, contexts: Vec<ReferenceChoices>) {
        for (i, values) in contexts.into_iter().enumerate() {
            let id = if i == 0 { root } else { self.slot(None) };
            values.owner.set(Some(id));
            self.choices[id] = Some(Choice {
                values,
                weights: None,
            });
        }
    }
    fn snapshot(&mut self, owner: Owner, base: &Frame<'a>) -> Rc<Frame<'a>> {
        if let Some((key, value)) = &self.base
            && Rc::ptr_eq(key, &owner)
        {
            return value.clone();
        }
        let value = Rc::new(base.clone());
        self.base = Some((owner, value.clone()));
        value
    }
    fn action(
        &mut self,
        base: &Frame<'a>,
        depth: usize,
        action: Action<'a>,
        owner: Owner,
    ) -> usize {
        let base = self.snapshot(owner, base);
        let id = self.slot(None);
        self.pending.push(Job::Action {
            id,
            base,
            depth,
            action,
        });
        id
    }
    fn slot(&mut self, value: Option<Choice>) -> usize {
        if let Some(id) = self.free.pop() {
            self.choices[id] = value;
            id
        } else {
            let id = self.choices.len();
            self.choices.push(value);
            id
        }
    }
    fn reduced(&mut self, id: usize) -> Result<Vec<ReferenceBound>, StopReason> {
        let choice = self.choices[id].take().ok_or(StopReason::SemanticAbort)?;
        self.free.push(id);
        choice.values.leaf.ok_or(StopReason::SemanticAbort)
    }
    pub(super) fn enqueue(&mut self, frame: Frame<'a>, depth: usize) -> usize {
        let id = self.slot(None);
        self.pending.push(Job::Run { id, frame, depth });
        id
    }

    pub(super) fn enqueue_cancel(
        &mut self,
        frame: Frame<'a>,
        index: usize,
        end: usize,
        winner: BindingId,
        depth: usize,
    ) -> usize {
        let id = self.slot(None);
        self.pending.push(Job::Cancel {
            id,
            frame,
            index,
            end,
            winner,
            depth,
        });
        id
    }

    pub(super) fn choice(
        &mut self,
        values: ReferenceChoices,
        weights: Option<Vec<Rational>>,
    ) -> usize {
        let id = self.slot(Some(Choice { values, weights }));
        self.choices[id]
            .as_ref()
            .unwrap()
            .values
            .owner
            .set(Some(id));
        self.pending.push(Job::Reduce(id));
        id
    }
}

impl<'a> Interpreter<'a> {
    // Resume the source producer's choice, rather than introducing a child
    // choice. A Range leaf must retain its original overriding placement.
    pub(super) fn defer_in_place<F>(&mut self, base: &Frame<'a>, action: F) -> Execution
    where
        F: FnOnce(&mut Interpreter<'a>, &Frame<'a>) -> Execution + 'a,
    {
        if let Some(work) = &mut self.run_work {
            let owner = self
                .probability_stack
                .last()
                .expect("source choice context")
                .owner
                .clone();
            let base = work.snapshot(owner.clone(), base);
            work.pending.push(Job::InPlace {
                owner,
                base,
                action: Box::new(action),
            });
            Ok(())
        } else {
            action(self, base)
        }
    }
    pub(super) fn defer_successor<F>(
        &mut self,
        base: &Frame<'a>,
        depth: usize,
        action: F,
    ) -> Execution
    where
        F: FnOnce(&mut Interpreter<'a>, &Frame<'a>, Frame<'a>) -> Execution + 'a,
    {
        if let Some(work) = &mut self.run_work {
            let owner = self
                .probability_stack
                .last()
                .expect("source choice context")
                .owner
                .clone();
            let id = work.action(base, depth, Box::new(action), owner);
            self.probability_stack
                .last_mut()
                .expect("lazy path parent")
                .deferred_children
                .push(id);
            Ok(())
        } else {
            let next = self.successor(base, depth)?;
            action(self, base, next)
        }
    }

    fn finish_activation(&mut self, id: usize, siblings: usize, outcome: Execution) -> Execution {
        if let Err(reason) = outcome {
            let contexts = std::mem::take(&mut self.probability_stack);
            let work = self.run_work.as_mut().unwrap();
            work.bind_unfinished(id, contexts);
            let children = work.pending.split_off(siblings);
            work.pending.push(Job::Abort(reason));
            work.pending.extend(children.into_iter().rev());
            return Ok(());
        }
        require(self.probability_stack.len() == 1)?;
        let values = self.probability_stack.pop().unwrap();
        values.owner.set(Some(id));
        let work = self.run_work.as_mut().unwrap();
        work.choices[id] = Some(Choice {
            values,
            weights: None,
        });
        let children = work.pending.split_off(siblings);
        work.pending.push(Job::Reduce(id));
        work.pending.extend(children.into_iter().rev());
        self.statistics.path_choice_peak = self.statistics.path_choice_peak.max(work.choices.len());
        Ok(())
    }
    pub(super) fn run_iterative(&mut self, frame: Frame<'a>, depth: usize) -> Execution {
        let outer = std::mem::take(&mut self.probability_stack);
        self.run_work = Some(Work::default());
        let root = self.run_work.as_mut().unwrap().enqueue(frame, depth);
        let result = self.drain_paths(root);
        self.run_work = None;
        self.probability_stack = outer;
        if let Some(value) = result? {
            self.probability_publish(value);
        }
        Ok(())
    }

    fn drain_paths(&mut self, root: usize) -> Result<Option<Vec<ReferenceBound>>, StopReason> {
        while let Some(job) = self.run_work.as_mut().unwrap().pending.pop() {
            let (id, frame, depth) = match job {
                Job::InPlace {
                    owner,
                    base,
                    action,
                } => {
                    let owner = owner.get().ok_or(StopReason::SemanticAbort)?;
                    let mut choice = self.run_work.as_mut().unwrap().choices[owner]
                        .take()
                        .ok_or(StopReason::SemanticAbort)?;
                    self.probability_stack = vec![std::mem::take(&mut choice.values)];
                    let work = self.run_work.as_mut().unwrap();
                    work.base = None;
                    let siblings = work.pending.len();
                    let outcome = action(self, &base);
                    if outcome.is_ok() {
                        require(self.probability_stack.len() == 1)?;
                        choice.values = self.probability_stack.pop().unwrap();
                        self.run_work.as_mut().unwrap().choices[owner] = Some(choice);
                    } else {
                        let contexts = std::mem::take(&mut self.probability_stack);
                        self.run_work
                            .as_mut()
                            .unwrap()
                            .bind_unfinished(owner, contexts);
                    }
                    let work = self.run_work.as_mut().unwrap();
                    let children = work.pending.split_off(siblings);
                    if let Err(reason) = outcome {
                        work.pending.push(Job::Abort(reason));
                    }
                    work.pending.extend(children.into_iter().rev());
                    continue;
                }
                Job::Action {
                    id,
                    base,
                    depth,
                    action,
                } => {
                    self.probability_stack = vec![ReferenceChoices::default()];
                    self.run_work.as_mut().unwrap().base = None;
                    let siblings = self.run_work.as_ref().unwrap().pending.len();
                    let outcome = match self.successor(&base, depth) {
                        Ok(next) => action(self, &base, next),
                        Err(e) => Err(e),
                    };
                    self.finish_activation(id, siblings, outcome)?;
                    continue;
                }
                Job::Reduce(id) => {
                    let mut choice = self.run_work.as_mut().unwrap().choices[id]
                        .take()
                        .ok_or(StopReason::SemanticAbort)?;
                    for child in std::mem::take(&mut choice.values.deferred_children) {
                        let result = self.run_work.as_mut().unwrap().reduced(child)?;
                        choice.values.children.push(result);
                    }
                    let result =
                        if self.model.uses_probability() || self.model.completion_requested() {
                            self.probability_reduce(choice.values, choice.weights.as_deref())
                        } else {
                            Vec::new()
                        };
                    self.run_work.as_mut().unwrap().choices[id] = Some(Choice {
                        values: ReferenceChoices {
                            leaf: Some(result),
                            ..ReferenceChoices::default()
                        },
                        weights: None,
                    });
                    continue;
                }
                Job::FoldAck { parent, child } => {
                    let child = self.run_work.as_mut().unwrap().reduced(child)?;
                    let previous = self.run_work.as_mut().unwrap().choices[parent]
                        .as_mut()
                        .unwrap()
                        .values
                        .children
                        .pop();
                    let result = if let Some(previous) = previous {
                        if self.model.uses_probability() || self.model.completion_requested() {
                            self.probability_reduce(
                                ReferenceChoices {
                                    children: vec![previous, child],
                                    ..ReferenceChoices::default()
                                },
                                None,
                            )
                        } else {
                            Vec::new()
                        }
                    } else {
                        child
                    };
                    self.run_work.as_mut().unwrap().choices[parent]
                        .as_mut()
                        .unwrap()
                        .values
                        .children
                        .push(result);
                    continue;
                }
                Job::Abort(reason) => return Err(reason),
                Job::Run { id, frame, depth } => (id, frame, depth),
                Job::Cancel {
                    id,
                    frame,
                    index,
                    end,
                    winner,
                    depth,
                } => {
                    match self.cancel_step(frame, index, end, winner, depth)? {
                        CancelStep::Run(frame, depth) => self
                            .run_work
                            .as_mut()
                            .unwrap()
                            .pending
                            .push(Job::Run { id, frame, depth }),
                        CancelStep::Acks {
                            frame,
                            index,
                            end,
                            winner,
                            depth,
                            max_delay,
                        } => {
                            let work = self.run_work.as_mut().unwrap();
                            work.choices[id] = Some(Choice {
                                values: ReferenceChoices::default(),
                                weights: None,
                            });
                            if max_delay > 0 {
                                work.pending.push(Job::Ack {
                                    parent: id,
                                    frame,
                                    index,
                                    end,
                                    winner,
                                    depth,
                                    delay: 1,
                                    max_delay,
                                });
                            } else {
                                work.pending.push(Job::Reduce(id));
                            }
                        }
                    }
                    continue;
                }
                Job::Ack {
                    parent,
                    frame,
                    index,
                    end,
                    winner,
                    depth,
                    delay,
                    max_delay,
                } => {
                    // One acknowledgement candidate, then its complete subtree,
                    // before reserving/cloning the next candidate. In particular
                    // u64::MAX domains must not be eagerly batched to the limit.
                    let mut next = self.successor(&frame, depth + 1)?;
                    next.tasks[index].ack = Some(
                        frame
                            .now
                            .checked_add(delay)
                            .ok_or(StopReason::SemanticAbort)?,
                    );
                    #[cfg(test)]
                    if self.model.capabilities[frame.tasks[index].call.capability.0]
                        .duration_distribution
                        .is_some()
                    {
                        self.statistics
                            .duration_ack_candidates
                            .push(next.tasks[index].ack.unwrap());
                    }
                    let work = self.run_work.as_mut().unwrap();
                    if delay < max_delay {
                        work.pending.push(Job::Ack {
                            parent,
                            frame,
                            index,
                            end,
                            winner,
                            depth,
                            delay: delay + 1,
                            max_delay,
                        });
                    } else {
                        work.pending.push(Job::Reduce(parent));
                    }
                    let child = work.slot(None);
                    work.pending.push(Job::FoldAck { parent, child });
                    work.pending.push(Job::Cancel {
                        id: child,
                        frame: next,
                        index: index + 1,
                        end,
                        winner,
                        depth: depth + 1,
                    });
                    continue;
                }
            };
            self.probability_stack = vec![ReferenceChoices::default()];
            self.run_work.as_mut().unwrap().base = None;
            let siblings = self.run_work.as_ref().unwrap().pending.len();
            // Each run_inner invocation returns before the next continuation
            // starts. The existing successor/reserve logical depth and task
            // limits are deliberately unchanged.
            let outcome = self.run_inner(frame, depth);
            self.finish_activation(id, siblings, outcome)?;
        }
        if !self.model.uses_probability() && !self.model.completion_requested() {
            return Ok(None);
        }
        Ok(Some(self.run_work.as_mut().unwrap().reduced(root)?))
    }
}
