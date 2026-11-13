//! Source-world continuations, with a streaming postorder fold.
use super::*;
use std::rc::Rc;
#[cfg(test)]
#[path = "round2_boundaries.rs"]
mod round2_boundaries;
#[cfg(test)]
#[path = "round3_boundaries.rs"]
mod round3_boundaries;

type Action<'a> = Box<dyn FnOnce(&mut Tree<'a>, &World<'a>) -> Result<Branch, StopReason> + 'a>;
pub(super) enum SeriesStep {
    Child(Rational, Branch),
    Replace(Branch),
}
type Generate<'a> =
    Box<dyn FnMut(&mut Tree<'a>, &World<'a>, usize) -> Result<SeriesStep, StopReason> + 'a>;
struct Series<'a> {
    id: usize,
    next: usize,
    count: usize,
    base: Rc<World<'a>>,
    depth: usize,
    hits: Vec<Vec<Option<u128>>>,
    closed: Vec<Vec<bool>>,
    make: Generate<'a>,
}

pub(super) enum Branch {
    Ready(Vec<Bound>),
    Deferred(usize),
    // A producer can emit several nondeterministic siblings without adding
    // a choice node (and thus without changing the quality tree's shape).
    Siblings(Vec<(Rational, Branch)>),
}
impl Branch {
    pub(super) fn ready(v: Vec<Bound>) -> Self {
        Self::Ready(v)
    }
    pub(super) fn values(self) -> Vec<Bound> {
        let Self::Ready(v) = self else {
            panic!("unreduced world")
        };
        v
    }
}
impl std::ops::Deref for Branch {
    type Target = [Bound];
    fn deref(&self) -> &[Bound] {
        let Self::Ready(v) = self else {
            panic!("unreduced world")
        };
        v
    }
}
enum Node {
    Ready(Vec<Bound>),
    Siblings(Vec<(Rational, Branch)>),
    ResolvedSiblings(Vec<(Rational, Vec<Bound>)>),
    Choice {
        children: Vec<(Rational, Branch)>,
        random: bool,
        alias: bool,
    },
}
enum Job<'a> {
    Series(Box<Series<'a>>),
    Action {
        id: usize,
        base: Rc<World<'a>>,
        depth: usize,
        hits: Vec<Vec<Option<u128>>>,
        closed: Vec<Vec<bool>>,
        action: Action<'a>,
    },
    World {
        id: usize,
        world: World<'a>,
        depth: usize,
        hits: Vec<Vec<Option<u128>>>,
        closed: Vec<Vec<bool>>,
    },
    Reduce(usize),
    Abort(StopReason),
}
#[derive(Default)]
pub(super) struct Work<'a> {
    jobs: Vec<Job<'a>>,
    nodes: Vec<Option<Node>>,
    free: Vec<usize>,
    base: Option<Rc<World<'a>>>,
}
impl<'a> Work<'a> {
    fn source_snapshot(&mut self, base: &World<'a>) -> Rc<World<'a>> {
        debug_assert!(
            self.base.as_deref().is_none_or(|snapshot| snapshot == base),
            "source World changed after the first defer in an activation"
        );
        self.base
            .get_or_insert_with(|| Rc::new(base.clone()))
            .clone()
    }
    fn slot(&mut self, n: Option<Node>) -> usize {
        if let Some(i) = self.free.pop() {
            self.nodes[i] = n;
            i
        } else {
            let i = self.nodes.len();
            self.nodes.push(n);
            i
        }
    }
    pub(super) fn enqueue(
        &mut self,
        world: World<'a>,
        depth: usize,
        hits: Vec<Vec<Option<u128>>>,
        closed: Vec<Vec<bool>>,
    ) -> Branch {
        let id = self.slot(None);
        self.jobs.push(Job::World {
            id,
            world,
            depth,
            hits,
            closed,
        });
        Branch::Deferred(id)
    }
    pub(super) fn choice(&mut self, children: Vec<(Rational, Branch)>, random: bool) -> Branch {
        let id = self.slot(Some(Node::Choice {
            children,
            random,
            alias: false,
        }));
        self.jobs.push(Job::Reduce(id));
        Branch::Deferred(id)
    }
    fn take(&mut self, id: usize) -> Result<Vec<Bound>, StopReason> {
        let Some(Node::Ready(v)) = self.nodes[id].take() else {
            return Err(StopReason::SemanticAbort);
        };
        self.free.push(id);
        Ok(v)
    }
    fn children(
        &mut self,
        branches: Vec<(Rational, Branch)>,
    ) -> Result<Vec<(Rational, Vec<Bound>)>, StopReason> {
        let mut ready = Vec::new();
        for (p, b) in branches {
            match b {
                Branch::Ready(v) => ready.push((p, v)),
                Branch::Siblings(bs) => ready.extend(
                    self.children(bs)?
                        .into_iter()
                        .map(|(q, v)| (Rational(&p.0 * q.0), v)),
                ),
                Branch::Deferred(id) => {
                    let node = self.nodes[id].take().ok_or(StopReason::SemanticAbort)?;
                    self.free.push(id);
                    match node {
                        Node::Ready(v) => ready.push((p, v)),
                        Node::ResolvedSiblings(xs) => {
                            ready.extend(xs.into_iter().map(|(q, v)| (Rational(&p.0 * q.0), v)))
                        }
                        _ => return Err(StopReason::SemanticAbort),
                    }
                }
            }
        }
        Ok(ready)
    }
}
impl<'a> Tree<'a> {
    pub(super) fn branch_series<F>(
        &mut self,
        base: &World<'a>,
        count: usize,
        random: Option<bool>,
        mut make: F,
    ) -> Result<Branch, StopReason>
    where
        F: FnMut(&mut Tree<'a>, &World<'a>, usize) -> Result<SeriesStep, StopReason> + 'a,
    {
        if let Some(work) = &mut self.walk_work {
            let base = work.source_snapshot(base);
            let node = if let Some(random) = random {
                Node::Choice {
                    children: Vec::new(),
                    random,
                    alias: false,
                }
            } else {
                Node::Siblings(Vec::new())
            };
            let id = work.slot(Some(node));
            work.jobs.push(Job::Series(Box::new(Series {
                id,
                next: 0,
                count,
                base,
                depth: self.depth,
                hits: self.first_hits.clone(),
                closed: self.first_closed.clone(),
                make: Box::new(make),
            })));
            return Ok(Branch::Deferred(id));
        }
        let mut children = Vec::new();
        for i in 0..count {
            match make(self, base, i)? {
                SeriesStep::Child(p, b) => children.push((p, b)),
                SeriesStep::Replace(b) => return Ok(b),
            }
        }
        Ok(if let Some(random) = random {
            self.reduce(children, random)
        } else {
            Branch::Siblings(children)
        })
    }
    pub(super) fn defer_world<F>(
        &mut self,
        base: &World<'a>,
        action: F,
    ) -> Result<Branch, StopReason>
    where
        F: FnOnce(&mut Tree<'a>, &World<'a>) -> Result<Branch, StopReason> + 'a,
    {
        if let Some(work) = &mut self.walk_work {
            let id = work.slot(None);
            let base = work.source_snapshot(base);
            work.jobs.push(Job::Action {
                id,
                base,
                depth: self.depth,
                hits: self.first_hits.clone(),
                closed: self.first_closed.clone(),
                action: Box::new(action),
            });
            Ok(Branch::Deferred(id))
        } else {
            action(self, base)
        }
    }

    fn finish_world(&mut self, id: usize, start: usize, result: Result<Branch, StopReason>) {
        let work = self.walk_work.as_mut().unwrap();
        let children = work.jobs.split_off(start);
        match result {
            Ok(Branch::Ready(v)) => work.nodes[id] = Some(Node::Ready(v)),
            Ok(Branch::Siblings(children)) => {
                work.nodes[id] = Some(Node::Siblings(children));
                work.jobs.push(Job::Reduce(id));
            }
            Ok(b) => {
                work.nodes[id] = Some(Node::Choice {
                    children: vec![(Rational::one(), b)],
                    random: false,
                    alias: true,
                });
                work.jobs.push(Job::Reduce(id));
            }
            Err(e) => work.jobs.push(Job::Abort(e)),
        }
        work.jobs.extend(children.into_iter().rev());
    }
    pub(super) fn walk_iterative(&mut self, world: World<'a>) -> Result<Branch, StopReason> {
        let saved = (
            self.depth,
            self.first_hits.clone(),
            self.first_closed.clone(),
        );
        self.walk_work = Some(Work::default());
        let Branch::Deferred(root) = self.walk_work.as_mut().unwrap().enqueue(
            world,
            self.depth + 1,
            self.first_hits.clone(),
            self.first_closed.clone(),
        ) else {
            unreachable!()
        };
        let result = self.drain_worlds(root);
        self.walk_work = None;
        (self.depth, self.first_hits, self.first_closed) = saved;
        result.map(Branch::Ready)
    }
    pub(super) fn walk_same_depth(&mut self, world: World<'a>) -> Result<Branch, StopReason> {
        if let Some(work) = &mut self.walk_work {
            Ok(work.enqueue(
                world,
                self.depth,
                self.first_hits.clone(),
                self.first_closed.clone(),
            ))
        } else {
            self.walk_inner(world)
        }
    }
    fn drain_worlds(&mut self, root: usize) -> Result<Vec<Bound>, StopReason> {
        while let Some(job) = self.walk_work.as_mut().unwrap().jobs.pop() {
            match job {
                Job::Series(mut series) => {
                    if series.next == series.count {
                        self.walk_work
                            .as_mut()
                            .unwrap()
                            .jobs
                            .push(Job::Reduce(series.id));
                        continue;
                    }
                    self.depth = series.depth;
                    self.first_hits = series.hits.clone();
                    self.first_closed = series.closed.clone();
                    let work = self.walk_work.as_mut().unwrap();
                    work.base = None;
                    let start = work.jobs.len();
                    let step = (series.make)(self, &series.base, series.next);
                    let work = self.walk_work.as_mut().unwrap();
                    let jobs = work.jobs.split_off(start);
                    match step {
                        Ok(SeriesStep::Child(p, b)) => {
                            let children = match &mut work.nodes[series.id] {
                                Some(Node::Choice { children, .. })
                                | Some(Node::Siblings(children)) => children,
                                _ => return Err(StopReason::SemanticAbort),
                            };
                            children.push((p, b));
                            series.next += 1;
                            work.jobs.push(Job::Series(series));
                        }
                        Ok(SeriesStep::Replace(b)) => {
                            let children = match work.nodes[series.id].take() {
                                Some(Node::Choice { children, .. })
                                | Some(Node::Siblings(children)) => children,
                                _ => return Err(StopReason::SemanticAbort),
                            };
                            drop(work.children(children)?);
                            work.nodes[series.id] = Some(Node::Choice {
                                children: vec![(Rational::one(), b)],
                                random: false,
                                alias: true,
                            });
                            work.jobs.push(Job::Reduce(series.id));
                        }
                        Err(reason) => work.jobs.push(Job::Abort(reason)),
                    }
                    work.jobs.extend(jobs.into_iter().rev());
                }
                Job::Abort(e) => return Err(e),
                Job::Action {
                    id,
                    base,
                    depth,
                    hits,
                    closed,
                    action,
                } => {
                    self.depth = depth;
                    self.first_hits = hits;
                    self.first_closed = closed;
                    self.walk_work.as_mut().unwrap().base = None;
                    let start = self.walk_work.as_ref().unwrap().jobs.len();
                    let result = action(self, &base);
                    self.finish_world(id, start, result);
                }
                Job::World {
                    id,
                    world,
                    depth,
                    hits,
                    closed,
                } => {
                    self.depth = depth;
                    self.first_hits = hits;
                    self.first_closed = closed;
                    self.walk_work.as_mut().unwrap().base = None;
                    let start = self.walk_work.as_ref().unwrap().jobs.len();
                    let result = self.walk_inner(world);
                    self.finish_world(id, start, result);
                }
                Job::Reduce(id) => {
                    let node = self.walk_work.as_mut().unwrap().nodes[id]
                        .take()
                        .ok_or(StopReason::SemanticAbort)?;
                    let (children, random, alias) = match node {
                        Node::Choice {
                            children,
                            random,
                            alias,
                        } => (children, random, alias),
                        Node::Siblings(children) => {
                            let ready = self.walk_work.as_mut().unwrap().children(children)?;
                            self.walk_work.as_mut().unwrap().nodes[id] =
                                Some(Node::ResolvedSiblings(ready));
                            continue;
                        }
                        _ => return Err(StopReason::SemanticAbort),
                    };
                    let mut ready = self.walk_work.as_mut().unwrap().children(children)?;
                    let v = if alias {
                        if ready.len() != 1 {
                            self.walk_work.as_mut().unwrap().nodes[id] =
                                Some(Node::ResolvedSiblings(ready));
                            continue;
                        }
                        ready.pop().ok_or(StopReason::SemanticAbort)?.1
                    } else {
                        self.reduce_ready(ready, random)
                    };
                    self.walk_work.as_mut().unwrap().nodes[id] = Some(Node::Ready(v));
                }
            }
        }
        self.walk_work.as_mut().unwrap().take(root)
    }
}
