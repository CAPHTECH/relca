//! Partial-order evidence, compressed strict-time edges, bounded order queries.
use super::input::{Event, Failure, MAX_ORDER_STATES};
use std::{
    cell::Cell,
    collections::{BTreeMap, VecDeque},
};

#[derive(Debug)]
pub struct Order {
    edges: Vec<Vec<usize>>,
    event_count: usize,
    work: Cell<usize>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Alternatives {
    pub yes: bool,
    pub no: bool,
    pub truncated: bool,
}
impl Order {
    pub fn source(events: &[Event], truncated: bool) -> Result<Self, Failure> {
        let ids: BTreeMap<_, _> = events
            .iter()
            .enumerate()
            .map(|(i, e)| (e.event_id.as_str(), i))
            .collect();
        let mut ticks: Vec<_> = events.iter().map(|e| e.time.latest).collect();
        ticks.sort_unstable();
        ticks.dedup();
        let n = events.len();
        let mut order = Self {
            edges: vec![Vec::new(); n + ticks.len()],
            event_count: n,
            work: Cell::new(0),
        };
        // Classify a cycle in declared causality separately from a DAG whose
        // edges are incompatible with the timestamp intervals.
        for (i, e) in events.iter().enumerate() {
            for after in &e.after {
                match ids.get(after.as_str()) {
                    Some(&j) => order.edges[j].push(i),
                    None if truncated => (), // Not a claim about unread evidence.
                    None => return Err(Failure::invalid("unknown_after")),
                }
            }
        }
        order.validate()?;
        // Prefix nodes represent all ends <= tick. At most 3N edges replace
        // the quadratic set of pairs a.latest < b.earliest (10000-event limit).
        for i in 1..ticks.len() {
            order.edges[n + i - 1].push(n + i);
        }
        for (i, e) in events.iter().enumerate() {
            order.edges[i].push(n + ticks.binary_search(&e.time.latest).expect("own latest"));
            let prior = ticks.partition_point(|t| *t < e.time.earliest);
            if prior > 0 {
                order.edges[n + prior - 1].push(i);
            }
        }
        order.validate_with_reason("inconsistent_time_evidence")?;
        Ok(order)
    }
    pub fn add_semantic(&mut self, edges: &[(usize, usize)]) -> Result<(), Failure> {
        for &(a, b) in edges {
            self.edges[a].push(b);
        }
        self.validate()
    }
    fn validate(&self) -> Result<(), Failure> {
        self.validate_with_reason("order_cycle")
    }
    fn validate_with_reason(&self, reason: &'static str) -> Result<(), Failure> {
        let mut degree = vec![0; self.edges.len()];
        for es in &self.edges {
            for &b in es {
                degree[b] += 1;
            }
        }
        let mut queue: VecDeque<_> = degree
            .iter()
            .enumerate()
            .filter_map(|(i, d)| (*d == 0).then_some(i))
            .collect();
        let mut count = 0;
        while let Some(a) = queue.pop_front() {
            count += 1;
            for &b in &self.edges[a] {
                degree[b] -= 1;
                if degree[b] == 0 {
                    queue.push_back(b);
                }
            }
        }
        if count != self.edges.len() {
            return Err(Failure {
                code: "E-LOG-1",
                reason,
            });
        }
        Ok(())
    }
    fn spend(&self) -> bool {
        let used = self.work.get();
        if used == MAX_ORDER_STATES {
            return false;
        }
        self.work.set(used + 1);
        true
    }
    /// The same run-wide budget covers reachability probes and extension search.
    /// None is not false: callers must retain the unexamined order as unknown.
    pub fn precedes(&self, a: usize, b: usize) -> Option<bool> {
        let mut seen = vec![false; self.edges.len()];
        let mut stack = vec![a];
        while let Some(x) = stack.pop() {
            if seen[x] {
                continue;
            }
            seen[x] = true;
            if !self.spend() {
                return None;
            }
            for &y in &self.edges[x] {
                if y == b {
                    return Some(true);
                }
                if !seen[y] {
                    stack.push(y);
                }
            }
        }
        Some(false)
    }
    /// All possible relative orders of the relevant observations. Projection
    /// keeps reachability through irrelevant events and compressed time nodes.
    /// Both outcomes are sufficient for unknown; a budget stop proves neither.
    pub fn alternatives(
        &self,
        nodes: &[usize],
        predicate: impl Fn(&[usize]) -> bool,
    ) -> Alternatives {
        let mut nodes = nodes.to_vec();
        nodes.sort_unstable();
        nodes.dedup();
        assert!(nodes.iter().all(|n| *n < self.event_count));
        let mut out = Alternatives {
            yes: false,
            no: false,
            truncated: false,
        };
        let mut predecessors = vec![Vec::new(); nodes.len()];
        for (i, &a) in nodes.iter().enumerate() {
            let mut seen = vec![false; self.edges.len()];
            let mut stack = self.edges[a].clone();
            while let Some(x) = stack.pop() {
                if seen[x] {
                    continue;
                }
                seen[x] = true;
                if !self.spend() {
                    out.truncated = true;
                    return out;
                }
                if let Ok(j) = nodes.binary_search(&x) {
                    predecessors[j].push(i);
                }
                stack.extend(&self.edges[x]);
            }
        }
        // Iterative backtracking: even a long total order cannot overflow the
        // call stack. Candidate iteration is display-independent, not evidence.
        let mut used = vec![false; nodes.len()];
        let mut path = Vec::<usize>::new();
        let mut next = vec![0usize];
        loop {
            if path.len() == nodes.len() {
                let events: Vec<_> = path.iter().map(|i| nodes[*i]).collect();
                if predicate(&events) {
                    out.yes = true;
                } else {
                    out.no = true;
                }
                if out.yes && out.no {
                    break;
                }
            } else {
                let level = path.len();
                let candidate = (next[level]..nodes.len())
                    .find(|&i| !used[i] && predecessors[i].iter().all(|p| used[*p]));
                if let Some(i) = candidate {
                    next[level] = i + 1;
                    used[i] = true;
                    path.push(i);
                    next.push(0);
                    if !self.spend() {
                        out.truncated = true;
                        break;
                    }
                    continue;
                }
            }
            next.pop();
            match path.pop() {
                Some(i) => used[i] = false,
                None => break,
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn graph(edges: &[&[usize]]) -> Order {
        Order {
            edges: edges.iter().map(|e| e.to_vec()).collect(),
            event_count: edges.len(),
            work: Cell::new(0),
        }
    }
    #[test]
    fn both_same_tick_extensions_survive_until_an_explicit_edge() {
        let a = graph(&[&[1, 2], &[], &[3], &[]]);
        assert_eq!(
            a.alternatives(&[1, 2], |p| p[0] == 1),
            Alternatives {
                yes: true,
                no: true,
                truncated: false
            }
        );
        let b = graph(&[&[1, 2], &[2], &[3], &[]]);
        assert_eq!(
            b.alternatives(&[1, 2], |p| p[0] == 1),
            Alternatives {
                yes: true,
                no: false,
                truncated: false
            }
        );
    }
    #[test]
    fn projection_preserves_hidden_paths_and_cycles_fail_closed() {
        let a = graph(&[&[1], &[2], &[]]);
        assert_eq!(a.precedes(0, 2), Some(true));
        assert!(!a.alternatives(&[0, 2], |p| p[0] == 0).no);
        assert!(graph(&[&[1], &[0]]).validate().is_err());
    }
    #[test]
    fn exploration_budget_does_not_invent_an_unvisited_outcome() {
        let a = graph(&[&[], &[], &[], &[], &[], &[], &[], &[], &[]]);
        let r = a.alternatives(&(0..9).collect::<Vec<_>>(), |_| true);
        assert!(r.truncated && r.yes && !r.no);
        let second = a.alternatives(&[0, 1], |_| true);
        assert!(second.truncated && !second.yes && !second.no);
    }
}
