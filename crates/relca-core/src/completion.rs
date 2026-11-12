//! Sidecar analysis only: no executable state, transition generation, or budget
//! accounting lives here. Targets absorb before unknowns and route successors.
use crate::{
    Verdict, diag, ir,
    probability::{ProbabilityRange, Rational},
};
use std::collections::BTreeSet;

pub(crate) const EDGE_LIMIT: usize = 2_000_000;
#[cfg(test)]
pub(crate) const FIXTURES: [&str; 24] = [
    "done-arrivals",
    "done-arrivals.broken-arrival-after-clock",
    "done-branches",
    "done-branches.broken-enable-then",
    "done-call-kind",
    "done-call-kind.broken-call-unused",
    "done-clock",
    "done-clock.broken-short-clock",
    "done-end-vs-close",
    "done-end-vs-close.broken-pre-end-delay",
    "done-nondet",
    "done-nondet.broken-nondet-option",
    "done-people",
    "done-people.broken-remove-executor",
    "done-probability",
    "done-probability.broken-block-failure",
    "done-recovery",
    "done-recovery.broken-zero-capacity",
    "done-support",
    "done-support.broken-positive-support",
    "done-vacuity",
    "done-vacuity.broken-enable-registration",
    "done-within",
    "done-within.broken-tight-deadline",
];
#[derive(Default)]
pub(crate) struct Node {
    pub actions: Vec<Vec<(Rational, usize)>>,
    pub target: bool,
    pub unknown: bool,
}
#[derive(Default)]
pub(crate) struct Graph {
    pub nodes: Vec<Node>,
    pub edges: usize,
    pub exceeded: bool,
    pub known_failure: bool,
}
impl Graph {
    pub fn node(&mut self) -> usize {
        // Once admission failed no further sidecar storage is needed. Node zero
        // is only a scratch tag; analyze uses solely the retained failure flag.
        if self.exceeded {
            return 0;
        }
        let id = self.nodes.len();
        self.nodes.push(Node {
            unknown: true,
            ..Node::default()
        });
        id
    }
    pub fn action(&mut self, from: usize, action: Vec<(Rational, usize)>) {
        if self.exceeded {
            return;
        }
        if action.len() > EDGE_LIMIT.saturating_sub(self.edges) {
            self.exceeded = true;
            return;
        }
        self.edges += action.len();
        self.nodes[from].actions.push(action);
    }
    pub fn analyze(&self, aborted: bool) -> Analysis {
        if self.exceeded {
            return Analysis {
                aborted,
                probability_failure: self.known_failure,
                recoverable: if self.known_failure {
                    Verdict::Violated
                } else {
                    Verdict::Unsupported
                },
                terminating: if self.known_failure {
                    Verdict::Violated
                } else {
                    Verdict::Unsupported
                },
                probability: None,
            };
        }
        if self.nodes.is_empty() {
            return Analysis {
                aborted,
                probability_failure: false,
                recoverable: if aborted {
                    Verdict::Unsupported
                } else {
                    Verdict::Inconclusive
                },
                terminating: if aborted {
                    Verdict::Unsupported
                } else {
                    Verdict::Inconclusive
                },
                probability: Some((
                    [
                        Rational::zero(),
                        Rational::one(),
                        Rational::zero(),
                        Rational::one(),
                    ],
                    false,
                )),
            };
        }
        let n = self.nodes.len();
        let mut reachable = vec![false; n];
        let mut todo = vec![0];
        let mut reverse = vec![Vec::new(); n];
        while let Some(i) = todo.pop() {
            if std::mem::replace(&mut reachable[i], true) {
                continue;
            }
            if self.nodes[i].target {
                continue;
            }
            for a in &self.nodes[i].actions {
                for (p, j) in a {
                    if !p.is_zero() {
                        reverse[*j].push(i);
                        todo.push(*j);
                    }
                }
            }
        }
        let mut possible = vec![false; n];
        let mut todo: Vec<_> = (0..n)
            .filter(|&i| reachable[i] && (self.nodes[i].target || self.nodes[i].unknown))
            .collect();
        while let Some(i) = todo.pop() {
            if std::mem::replace(&mut possible[i], true) {
                continue;
            }
            todo.extend(reverse[i].iter().copied());
        }
        let unknown =
            (0..n).any(|i| reachable[i] && !self.nodes[i].target && self.nodes[i].unknown);
        let recovery_bad = (0..n).any(|i| reachable[i] && !possible[i]);
        let deadlock = (0..n).any(|i| {
            reachable[i]
                && !self.nodes[i].target
                && !self.nodes[i].unknown
                && self.nodes[i].actions.is_empty()
        });
        // Kahn on the target-absorbed support detects real cycles, including
        // self loops with an exit. No fairness assumption removes those paths.
        let mut degree = vec![0; n];
        for (i, reached) in reachable.iter().enumerate() {
            if *reached && !self.nodes[i].target {
                for a in &self.nodes[i].actions {
                    for (p, j) in a {
                        if !p.is_zero() {
                            degree[*j] += 1;
                        }
                    }
                }
            }
        }
        let mut ready: Vec<_> = (0..n).filter(|&i| reachable[i] && degree[i] == 0).collect();
        let mut order = Vec::new();
        while let Some(i) = ready.pop() {
            order.push(i);
            if self.nodes[i].target {
                continue;
            }
            for a in &self.nodes[i].actions {
                for (p, j) in a {
                    if !p.is_zero() {
                        degree[*j] -= 1;
                        if degree[*j] == 0 {
                            ready.push(*j);
                        }
                    }
                }
            }
        }
        let cycle = order.len() != reachable.iter().filter(|x| **x).count();
        let verdict = |bad| {
            if bad {
                Verdict::Violated
            } else if unknown {
                if aborted {
                    Verdict::Unsupported
                } else {
                    Verdict::Inconclusive
                }
            } else {
                Verdict::Satisfied
            }
        };
        let probability = if cycle {
            None
        } else {
            let mut values = vec![std::array::from_fn::<_, 4, _>(|_| Rational::zero()); n];
            let mut numeric = vec![false; n];
            for &i in order.iter().rev() {
                let node = &self.nodes[i];
                if node.target {
                    values[i] = std::array::from_fn(|_| Rational::one());
                    continue;
                }
                let mut actions = Vec::new();
                if node.unknown {
                    actions.push([
                        Rational::zero(),
                        Rational::one(),
                        Rational::zero(),
                        Rational::one(),
                    ]);
                }
                for a in &node.actions {
                    let mut v = std::array::from_fn::<_, 4, _>(|_| Rational::zero());
                    for (p, j) in a {
                        numeric[i] |= numeric[*j] || !p.within_bits(4096);
                        for (k, x) in v.iter_mut().enumerate() {
                            let product = Rational(&p.0 * &values[*j][k].0);
                            numeric[i] |= !product.within_bits(4096);
                            x.0 += product.0;
                            numeric[i] |= !x.within_bits(4096);
                        }
                    }
                    actions.push(v);
                }
                if numeric[i] {
                    values[i] = [
                        Rational::zero(),
                        Rational::one(),
                        Rational::zero(),
                        Rational::one(),
                    ];
                } else if !actions.is_empty() {
                    for k in 0..4 {
                        values[i][k] = if k < 2 {
                            actions.iter().map(|a| a[k].clone()).min().unwrap()
                        } else {
                            actions.iter().map(|a| a[k].clone()).max().unwrap()
                        };
                    }
                }
            }
            Some((values[0].clone(), numeric[0]))
        };
        Analysis {
            aborted,
            probability_failure: false,
            recoverable: verdict(recovery_bad),
            terminating: verdict(deadlock || cycle),
            probability,
        }
    }
}
pub(crate) struct Analysis {
    aborted: bool,
    probability_failure: bool,
    pub recoverable: Verdict,
    pub terminating: Verdict,
    pub probability: Option<([Rational; 4], bool)>,
}
impl Analysis {
    pub fn property(&self, p: ir::Property) -> (Verdict, Option<ProbabilityRange>) {
        match p {
            ir::Property::RecoverableEnd => (self.recoverable, None),
            ir::Property::TerminatingEnd => (self.terminating, None),
            ir::Property::AlmostSureEnd => match &self.probability {
                Some((b, numeric)) => (
                    if *numeric {
                        Verdict::Inconclusive
                    } else if b[0] == Rational::one() {
                        Verdict::Satisfied
                    } else if b[1] < Rational::one() {
                        Verdict::Violated
                    } else if self.aborted {
                        Verdict::Unsupported
                    } else {
                        Verdict::Inconclusive
                    },
                    Some(ProbabilityRange {
                        min: b[0].clone(),
                        max: b[3].clone(),
                    }),
                ),
                None => (
                    if self.probability_failure {
                        Verdict::Violated
                    } else {
                        Verdict::Unsupported
                    },
                    Some(ProbabilityRange {
                        min: Rational::zero(),
                        max: Rational::one(),
                    }),
                ),
            },
            _ => unreachable!(),
        }
    }
}

pub(crate) fn apply(out: &mut crate::outcome::Outcome, m: &ir::Model, g: &Graph, aborted: bool) {
    let analysis = g.analyze(aborted);
    for p in &mut out.properties {
        if p.property.completion() {
            let (v, b) = analysis.property(p.property);
            p.verdict = v;
            p.probability = b;
        }
    }
    if g.exceeded {
        out.unchecked.push("completion_graph".into());
        for v in &m.verify {
            if v.property.completion() {
                out.postprocessing_diagnostics.push(diag::Diagnostic {
                    code: diag::U_SEM_0.into(),
                    severity: diag::Severity::Error,
                    message: "completion graph edge limit exceeded; completion_graph is unchecked"
                        .into(),
                    span: Some(v.span),
                });
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Observed {
    pub capabilities: BTreeSet<ir::CapabilityId>,
    pub people: BTreeSet<ir::PersonId>,
    pub arms: BTreeSet<(usize, usize, Vec<usize>, bool)>,
    pub ends: BTreeSet<ir::EndId>,
}
impl Observed {
    pub fn branch(&mut self, s: diag::Span, path: &[usize], yes: bool) {
        self.arms.insert((s.start, s.end, path.to_vec(), yes));
    }
    pub fn branch_statement(&mut self, m: &ir::Model, target: &ir::Statement, yes: bool) -> bool {
        fn find(block: &ir::Block, target: &ir::Statement, prefix: &[usize]) -> Option<Vec<usize>> {
            for (i, s) in block.iter().enumerate() {
                let mut path = prefix.to_vec();
                path.push(i);
                if std::ptr::eq(s, target) {
                    return Some(path);
                }
                if let ir::Stmt::Response(_, r) = &s.kind
                    && let Some(body) = &r.recovery
                {
                    let mut child = path.clone();
                    child.push(2);
                    if let Some(p) = find(body, target, &child) {
                        return Some(p);
                    }
                }
                if let ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } = &s.kind
                {
                    for (side, body) in [then_body, else_body].into_iter().enumerate() {
                        let mut child = path.clone();
                        child.push(side);
                        if let Some(p) = find(body, target, &child) {
                            return Some(p);
                        }
                    }
                }
            }
            None
        }
        if let Some(path) = find(&m.process.body, target, &[]) {
            self.branch(target.span, &path, yes);
            true
        } else {
            false
        }
    }
    pub fn merge(&mut self, other: &Self) {
        self.capabilities.extend(&other.capabilities);
        self.people.extend(&other.people);
        self.arms.extend(other.arms.iter().cloned());
        self.ends.extend(&other.ends);
    }
    pub fn warnings(&self, m: &ir::Model) -> Vec<diag::Diagnostic> {
        let mut calls = BTreeSet::new();
        let mut arms = Vec::new();
        let mut ends = Vec::new();
        fn lexical(
            b: &ir::Block,
            prefix: &[usize],
            calls: &mut BTreeSet<ir::CapabilityId>,
            arms: &mut Vec<(Vec<usize>, diag::Span)>,
            ends: &mut Vec<(ir::EndId, diag::Span)>,
        ) {
            for (i, s) in b.iter().enumerate() {
                let mut path = prefix.to_vec();
                path.push(i);
                match &s.kind {
                    ir::Stmt::Wait(_)
                    | ir::Stmt::WaitUntil(_)
                    | ir::Stmt::ResolveEffect { .. }
                    | ir::Stmt::StopReceiptUnknown => {}
                    ir::Stmt::Call(c) | ir::Stmt::Query(c, _) | ir::Stmt::Response(c, _) => {
                        calls.insert(c.capability);
                    }
                    ir::Stmt::Fork { branches, join, .. } => {
                        calls.extend(branches.iter().map(|c| c.capability));
                        if let ir::Join::Any { if_none, .. } = join {
                            ends.push((*if_none, s.span));
                        }
                    }
                    ir::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        arms.push((path.clone(), s.span));
                        let mut child = path.clone();
                        child.push(0);
                        lexical(then_body, &child, calls, arms, ends);
                        let mut child = path;
                        child.push(1);
                        lexical(else_body, &child, calls, arms, ends);
                    }
                    ir::Stmt::End(e) => ends.push((*e, s.span)),
                }
            }
        }
        lexical(&m.process.body, &[], &mut calls, &mut arms, &mut ends);
        calls.extend(m.contests().iter().map(|c| c.route));
        let included = |a: ir::ActorId| m.cut.as_ref().is_none_or(|c| c.include.contains(&a));
        let mut out = Vec::new();
        let mut warn = |code: &str, span, detail: String| {
            out.push(diag::Diagnostic {
                code: code.into(),
                severity: diag::Severity::Warning,
                span: Some(span),
                message: format!("process {}: {detail}", m.process.name),
            })
        };
        for (i, c) in m.capabilities.iter().enumerate() {
            let id = ir::CapabilityId(i);
            if included(c.actor) && !self.capabilities.contains(&id) {
                warn(
                    diag::W_UNREACH_0,
                    c.span,
                    format!(
                        "{}.{} {}",
                        m.actors[c.actor.0].name,
                        c.name,
                        if calls.contains(&id) {
                            "never_started"
                        } else {
                            "not_called"
                        }
                    ),
                );
            }
        }
        for (path, s) in arms {
            for yes in [true, false] {
                if !self.arms.contains(&(s.start, s.end, path.clone(), yes)) {
                    warn(
                        diag::W_UNREACH_1,
                        s,
                        format!(
                            "if {} arm is never taken",
                            if yes { "then" } else { "else" }
                        ),
                    );
                }
            }
        }
        let mut seen = BTreeSet::new();
        for (e, s) in ends {
            if seen.insert(e) && !self.ends.contains(&e) {
                warn(
                    diag::W_UNREACH_2,
                    s,
                    format!("end.{} is never reached", m.ends[e.0]),
                );
            }
        }
        let pool: BTreeSet<_> = m
            .actors
            .iter()
            .enumerate()
            .filter(|(i, _)| included(ir::ActorId(*i)))
            .filter_map(|(_, a)| a.executors.as_ref())
            .flatten()
            .copied()
            .collect();
        for (i, p) in m.people.iter().enumerate() {
            let id = ir::PersonId(i);
            if pool.contains(&id) && !self.people.contains(&id) {
                warn(
                    diag::W_UNREACH_3,
                    p.span,
                    format!("executor {} is never assigned", p.name),
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn m22_exit_self_loop_is_recoverable_but_not_terminating_and_closed_loop_is_neither() {
        let mut g = Graph::default();
        let s = g.node();
        let t = g.node();
        g.nodes[s].unknown = false;
        g.nodes[t].unknown = false;
        g.nodes[t].target = true;
        g.action(s, vec![(Rational::one(), s)]);
        g.action(s, vec![(Rational::one(), t)]);
        let a = g.analyze(false);
        assert_eq!(a.recoverable, Verdict::Satisfied);
        assert_eq!(a.terminating, Verdict::Violated);
        // A single removed exit turns the same real self loop into a closed SCC.
        g.nodes[s].actions.pop();
        let a = g.analyze(false);
        assert_eq!(a.recoverable, Verdict::Violated);
        assert_eq!(a.terminating, Verdict::Violated);
    }
    #[test]
    fn actual_edge_limit_k_and_k_plus_one_preserves_known_failure() {
        assert_eq!(EDGE_LIMIT, 2_000_000);
        let mut g = Graph::default();
        let s = g.node();
        let t = g.node();
        g.nodes[s].unknown = false;
        g.nodes[t].target = true;
        // A valid chance action: 2M strictly positive branches sum to one.
        g.action(
            s,
            vec![(Rational::decimal("0.0000005").unwrap(), t); EDGE_LIMIT],
        );
        assert_eq!(g.edges, EDGE_LIMIT);
        assert!(!g.exceeded);
        assert_eq!(g.nodes[s].actions[0].len(), EDGE_LIMIT);
        g.action(s, vec![(Rational::one(), t)]);
        assert!(g.exceeded);
        assert_eq!(g.edges, EDGE_LIMIT);
        assert_eq!(g.nodes[s].actions.len(), 1);
        let a = g.analyze(false);
        assert_eq!(a.recoverable, Verdict::Unsupported);
        assert_eq!(a.terminating, Verdict::Unsupported);
        g.known_failure = true;
        let a = g.analyze(false);
        assert_eq!(a.recoverable, Verdict::Violated);
        assert_eq!(a.terminating, Verdict::Violated);
        assert_eq!(a.property(ir::Property::AlmostSureEnd).0, Verdict::Violated);
    }
    #[test]
    fn pending_is_not_deadlock_and_known_witness_wins_over_unknown_and_abort() {
        let mut g = Graph::default();
        let s = g.node();
        let t = g.node();
        let u = g.node();
        g.nodes[s].unknown = false;
        g.nodes[t].unknown = false;
        g.nodes[t].target = true;
        g.action(s, vec![(Rational::one(), t)]);
        g.action(s, vec![(Rational::one(), u)]);
        assert_eq!(g.analyze(false).recoverable, Verdict::Inconclusive);
        assert_eq!(g.analyze(false).terminating, Verdict::Inconclusive);
        let bad = g.node();
        g.nodes[bad].unknown = false;
        g.action(s, vec![(Rational::one(), bad)]);
        for abort in [false, true] {
            let a = g.analyze(abort);
            assert_eq!(a.recoverable, Verdict::Violated);
            assert_eq!(a.terminating, Verdict::Violated);
            assert_eq!(a.property(ir::Property::AlmostSureEnd).0, Verdict::Violated);
        }
        // Unknown route work after an absorbing business target is irrelevant.
        g.nodes[s].target = true;
        let a = g.analyze(false);
        assert_eq!(a.recoverable, Verdict::Satisfied);
        assert_eq!(a.terminating, Verdict::Satisfied);
        assert_eq!(
            a.property(ir::Property::AlmostSureEnd).0,
            Verdict::Satisfied
        );
    }
}
