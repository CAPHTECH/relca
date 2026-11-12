//! Test-only, bounded, work-conserving semantics. No Relca execution helpers.
//! Strings and small integers are our own data, not production control states.
use std::collections::{BTreeMap, BTreeSet};

pub type Location = (usize, usize);
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OwnActor {
    pub name: String,
    pub capacity: u64,
    pub people: Option<Vec<String>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnCapability {
    pub name: String,
    pub actor: usize,
    pub duration: u64,
    pub results: Vec<String>,
    pub uses: u64,
    pub location: Location,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OwnCondition {
    Equal(usize, String),
    All(Vec<OwnCondition>),
    Any(Vec<OwnCondition>),
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Op {
    Calls(Vec<(usize, usize)>), // (binding, capability); one group = join all
    If(OwnCondition, Vec<Op>, Vec<Op>, Location),
    Wait(u64),
    End(String, Location),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnModel {
    pub actors: Vec<OwnActor>,
    pub capabilities: Vec<OwnCapability>,
    pub people: Vec<(String, Location)>,
    pub resource: Option<(String, u64)>,
    pub bindings: Vec<String>,
    pub body: Vec<Op>,
    pub clock: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TaskStatus {
    Queued,
    Running(u64, Option<String>),
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct OwnTask {
    binding: usize,
    capability: usize,
    status: TaskStatus,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct OwnState {
    time: u64,
    work: u64,
    control: Vec<Op>,
    results: Vec<Option<String>>,
    tasks: Vec<OwnTask>,
    wake: Option<u64>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Observation {
    pub ends: BTreeSet<String>,
    pub started: BTreeSet<String>,
    pub arms: BTreeSet<(Location, bool)>,
    pub assigned: BTreeSet<String>,
    pub terminals: BTreeSet<(String, u64, u64)>,
    pub deadlock: bool,
    pub cut: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truth {
    Yes,
    No,
    Unknown,
}
impl Observation {
    // This language fragment is a finite DAG: each call/wait/control step is
    // consumed once. Every failed maximal path ends at a deadlock or clock cut.
    // Thus deadlock witnesses refute both AG EF end and AF end; otherwise cuts
    // leave both unknown. No fairness or probabilistic argument is smuggled in.
    pub fn completion(&self) -> Truth {
        if self.deadlock {
            Truth::No
        } else if self.cut {
            Truth::Unknown
        } else {
            Truth::Yes
        }
    }
    pub fn reachable(&self, end: &str) -> Truth {
        if self.ends.contains(end) {
            Truth::Yes
        } else if self.cut {
            Truth::Unknown
        } else {
            Truth::No
        }
    }
}
pub fn compare(expected: &Observation, actual: &Observation) -> Result<(), String> {
    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "independent exploration mismatch\nexpected {expected:?}\nactual {actual:?}"
        ))
    }
}
pub fn declared_ends(m: &OwnModel) -> BTreeSet<String> {
    fn walk(ops: &[Op], out: &mut BTreeSet<String>) {
        for op in ops {
            match op {
                Op::End(e, _) => {
                    out.insert(e.clone());
                }
                Op::If(_, yes, no, _) => {
                    walk(yes, out);
                    walk(no, out);
                }
                _ => {}
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(&m.body, &mut out);
    out
}
pub fn overall(m: &OwnModel, o: &Observation) -> Truth {
    if o.completion() == Truth::No || declared_ends(m).iter().any(|e| o.reachable(e) == Truth::No) {
        Truth::No
    } else {
        o.completion()
    }
}
pub fn completion_probability(o: &Observation) -> (&'static str, &'static str) {
    // Pure nondeterminism has extremal mass zero or one, never a uniform mix.
    (
        if o.deadlock || o.cut { "0" } else { "1" },
        if !o.ends.is_empty() || o.cut {
            "1"
        } else {
            "0"
        },
    )
}
fn condition(c: &OwnCondition, values: &[Option<String>]) -> bool {
    match c {
        OwnCondition::Equal(b, v) => values[*b].as_ref().expect("definitely available") == v,
        OwnCondition::All(cs) => cs.iter().all(|c| condition(c, values)),
        OwnCondition::Any(cs) => cs.iter().any(|c| condition(c, values)),
    }
}
pub fn explore(m: &OwnModel) -> Observation {
    let initial = OwnState {
        time: 0,
        work: 0,
        control: m.body.clone(),
        results: vec![None; m.bindings.len()],
        tasks: vec![],
        wake: None,
    };
    let mut todo = vec![initial];
    let mut visited = BTreeSet::new();
    let mut out = Observation::default();
    while let Some(mut s) = todo.pop() {
        if !visited.insert(s.clone()) {
            continue;
        }
        assert!(
            visited.len() < 100_000,
            "test oracle budget is not a successful empty result"
        );
        if s.tasks.iter().all(|t| t.status == TaskStatus::Done) {
            s.tasks.clear();
        }
        if s.wake == Some(s.time) {
            s.wake = None;
        }
        if s.tasks.is_empty() && s.wake.is_none() {
            assert!(!s.control.is_empty(), "covered processes explicitly end");
            match s.control.remove(0) {
                Op::End(end, _) => {
                    out.ends.insert(end.clone());
                    out.terminals.insert((end, s.time, s.work));
                    continue;
                }
                Op::If(c, mut yes, no, loc) => {
                    let take = condition(&c, &s.results);
                    out.arms.insert((loc, take));
                    if !take {
                        yes = no;
                    }
                    yes.append(&mut s.control);
                    s.control = yes;
                }
                Op::Calls(calls) => {
                    s.tasks = calls
                        .into_iter()
                        .map(|(binding, capability)| OwnTask {
                            binding,
                            capability,
                            status: TaskStatus::Queued,
                        })
                        .collect();
                }
                Op::Wait(t) => s.wake = Some(s.time + t),
            }
            todo.push(s);
            continue;
        }
        let mut successors = vec![];
        for (i, t) in s.tasks.iter().enumerate() {
            let cap = &m.capabilities[t.capability];
            match &t.status {
                TaskStatus::Running(at, _) if *at == s.time => {
                    for value in &cap.results {
                        let mut n = s.clone();
                        n.tasks[i].status = TaskStatus::Done;
                        n.results[t.binding] = Some(value.clone());
                        successors.push(n);
                    }
                }
                TaskStatus::Queued => {
                    let running: Vec<_> = s
                        .tasks
                        .iter()
                        .filter(|x| matches!(x.status, TaskStatus::Running(..)))
                        .collect();
                    let occupancy = running
                        .iter()
                        .filter(|x| m.capabilities[x.capability].actor == cap.actor)
                        .count() as u64;
                    let used: u64 = running
                        .iter()
                        .map(|x| m.capabilities[x.capability].uses)
                        .sum();
                    if occupancy >= m.actors[cap.actor].capacity
                        || m.resource
                            .as_ref()
                            .is_some_and(|(_, capacity)| used + cap.uses > *capacity)
                    {
                        continue;
                    }
                    let pool = m.actors[cap.actor]
                        .people
                        .clone()
                        .map_or_else(|| vec![None], |ps| ps.into_iter().map(Some).collect());
                    for person in pool {
                        if person.is_some()
                            && running.iter().any(
                                |x| matches!(&x.status, TaskStatus::Running(_, p) if *p == person),
                            )
                        {
                            continue;
                        }
                        out.started.insert(cap.name.clone());
                        if let Some(p) = &person {
                            out.assigned.insert(p.clone());
                        }
                        let mut n = s.clone();
                        n.tasks[i].status = TaskStatus::Running(s.time + cap.duration, person);
                        successors.push(n);
                    }
                }
                _ => {}
            }
        }
        if !successors.is_empty() {
            todo.extend(successors);
            continue;
        }
        let next = s
            .tasks
            .iter()
            .filter_map(|t| match t.status {
                TaskStatus::Running(at, _) => Some(at),
                _ => None,
            })
            .chain(s.wake)
            .min();
        match next {
            None => out.deadlock = true,
            Some(at) if at > m.clock => out.cut = true,
            Some(at) => {
                let occupied = s
                    .tasks
                    .iter()
                    .filter(|t| matches!(t.status, TaskStatus::Running(..)))
                    .count() as u64;
                s.work += (at - s.time) * occupied;
                s.time = at;
                todo.push(s);
            }
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Warning {
    pub code: String,
    pub target: String,
    pub reason: String,
    pub location: Location,
}
pub fn warnings(m: &OwnModel, o: &Observation) -> Vec<Warning> {
    if o.cut {
        return vec![];
    }
    fn inventory(
        body: &[Op],
        calls: &mut BTreeSet<usize>,
        arms: &mut Vec<Location>,
        ends: &mut BTreeMap<String, Location>,
    ) {
        for op in body {
            match op {
                Op::Calls(cs) => calls.extend(cs.iter().map(|(_, c)| c)),
                Op::If(_, yes, no, loc) => {
                    arms.push(*loc);
                    inventory(yes, calls, arms, ends);
                    inventory(no, calls, arms, ends);
                }
                Op::End(name, loc) => {
                    ends.entry(name.clone()).or_insert(*loc);
                }
                Op::Wait(_) => {}
            }
        }
    }
    let (mut calls, mut arms, mut ends) = (BTreeSet::new(), vec![], BTreeMap::new());
    inventory(&m.body, &mut calls, &mut arms, &mut ends);
    let mut result = vec![];
    let mut add = |code: &str, target: String, reason: &str, location| {
        result.push(Warning {
            code: code.into(),
            target,
            reason: reason.into(),
            location,
        })
    };
    for (i, c) in m.capabilities.iter().enumerate() {
        if !o.started.contains(&c.name) {
            add(
                "W-UNREACH-0",
                c.name.clone(),
                if calls.contains(&i) {
                    "never_started"
                } else {
                    "not_called"
                },
                c.location,
            );
        }
    }
    for loc in arms {
        for take in [true, false] {
            if !o.arms.contains(&(loc, take)) {
                add(
                    "W-UNREACH-1",
                    if take { "then" } else { "else" }.into(),
                    "never_taken",
                    loc,
                );
            }
        }
    }
    for (end, loc) in ends {
        if !o.ends.contains(&end) {
            add("W-UNREACH-2", end, "never_reached", loc);
        }
    }
    for (p, loc) in &m.people {
        if m.actors
            .iter()
            .any(|a| a.people.as_ref().is_some_and(|ps| ps.contains(p)))
            && !o.assigned.contains(p)
        {
            add("W-UNREACH-3", p.clone(), "never_assigned", *loc);
        }
    }
    result.sort();
    result
}
