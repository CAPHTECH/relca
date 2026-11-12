//! Test-only projections of real reference Start transitions. No engine state
//! or selector helper is used to create or execute these reference states.
use super::*;
use serde_json::{Value, json};

#[derive(Clone)]
struct Snapshot {
    case: usize,
    queued: u64,
    now: Tick,
    steps: usize,
    tasks: Vec<Task>,
    values: Vec<StateValue>,
    trace: Vec<Observation>,
    condition_waits: Vec<(Tick, Option<Tick>)>,
    lost: BTreeSet<StateVarId>,
    snapshots: Vec<(Tick, Vec<StateValue>)>,
    business: Option<(EndId, Tick)>,
    adoption: Option<Tick>,
    control: Vec<Vec<diag::Span>>,
    waiting: Option<(std::ops::Range<usize>, Option<BindingId>)>,
    queue: Vec<QueueEntry>,
    receipt: bool,
    end: Option<EndId>,
    shared: Vec<StateValue>,
}
thread_local! {
    static STARTS:std::cell::RefCell<Option<Vec<Snapshot>>> = const {std::cell::RefCell::new(None)};
}
pub(super) fn started(m: &Model, before: &World<'_>, after: &World<'_>, k: usize, j: usize) {
    STARTS.with_borrow_mut(|slot| {
        let Some(rows) = slot else { return };
        let CaseSlot::Here(prior) = &before.cases[k] else {
            panic!()
        };
        let cap = &m.capabilities[prior.frame.tasks[j].call.capability.0];
        let queued = before
            .cases
            .iter()
            .enumerate()
            .filter_map(|(case, s)| match s {
                CaseSlot::Here(c) => Some((case, c)),
                _ => None,
            })
            .flat_map(|(case, c)| {
                c.frame
                    .tasks
                    .iter()
                    .enumerate()
                    .map(move |(task, t)| (case, task, t))
            })
            .filter(|(case, task, t)| {
                (*case, *task) != (k, j)
                    && t.phase == Phase::Queued
                    && m.capabilities[t.call.capability.0].actor == cap.actor
            })
            .count() as u64;
        let CaseSlot::Here(c) = &after.cases[k] else {
            panic!()
        };
        let f = &c.frame;
        assert!(f.waiting.as_ref().is_none_or(|w| w.join.is_none()));
        rows.push(Snapshot {
            case: k,
            queued,
            now: f.now,
            steps: f.steps,
            tasks: f.tasks.clone(),
            values: f.state_values.clone(),
            trace: f.trace.clone(),
            condition_waits: f.condition_waits.clone(),
            lost: f.lost_updates.clone(),
            snapshots: f.state_snapshots.clone(),
            business: f.business,
            adoption: f.adoption,
            control: f
                .control
                .iter()
                .map(|b| b.iter().map(|s| s.span).collect())
                .collect(),
            waiting: f.waiting.as_ref().map(|w| (w.tasks.clone(), w.winner)),
            queue: c.queue.clone(),
            receipt: c.receipt,
            end: c.end,
            shared: after.shared.clone(),
        });
    });
}
impl Snapshot {
    fn frame<'a>(&self, m: &'a Model) -> Frame<'a> {
        let control = self
            .control
            .iter()
            .map(|spans| {
                (0..=m.process.body.len())
                    .find_map(|start| {
                        let b = &m.process.body[start..];
                        (b.iter().map(|s| s.span).collect::<Vec<_>>() == *spans).then_some(b)
                    })
                    .expect("restore the actual borrowed control tail, not a synthetic program")
            })
            .collect();
        Frame {
            condition_waits: self.condition_waits.clone(),
            lost_updates: self.lost.clone(),
            state_snapshots: self.snapshots.clone(),
            business: self.business,
            state_values: self.values.clone(),
            trace: self.trace.clone(),
            now: self.now,
            steps: self.steps,
            control,
            waiting: self.waiting.as_ref().map(|(tasks, winner)| Waiting {
                tasks: tasks.clone(),
                winner: *winner,
                join: None,
            }),
            tasks: self.tasks.clone(),
            adoption: self.adoption,
        }
    }
    fn world<'a>(&self, m: &'a Model) -> World<'a> {
        World {
            external_peak: 0,
            now: self.now,
            next: 1,
            boundary: Boundary::Run,
            shared: self.shared.clone(),
            cases: vec![CaseSlot::Here(CaseFrame {
                frame: self.frame(m),
                queue: self.queue.clone(),
                receipt: self.receipt,
                end: self.end,
            })],
            steps: 0,
        }
    }
}
fn tree(m: &Model, times: Vec<Tick>) -> Tree<'_> {
    let n = times.len();
    let limits = Limits::default();
    Tree {
        response_exercise: BTreeMap::new(),
        response_receipts: vec![],
        first_hits: vec![],
        first_closed: vec![],
        first_trees: vec![],
        resource_worlds: None,
        depth: 0,
        m,
        limits,
        times,
        cases: (0..n).map(|_| interpreter(m, limits)).collect(),
        terminals: vec![BTreeMap::new(); n],
        unresolved: vec![false; n],
        stats: Statistics::default(),
        truncated: false,
        numeric: false,
        deadlock: false,
        overcommit: false,
        chance: false,
        scheduler: false,
        policy_unresolved: false,
        rejects: None,
        overflows: None,
        external_peaks: None,
        admissions: vec![None; n],
        unadmitted: vec![false; n],
        initial_leaves: vec![],
        receipts: vec![],
        duration_leaves: vec![],
    }
}
fn queued(m: &Model) -> CaseFrame<'_> {
    let mut frame = crate::reference::replay::initial(m);
    let Stmt::Call(call) = &m.process.body[0].kind else {
        panic!()
    };
    frame.control = vec![&m.process.body[1..]];
    let mut i = interpreter(m, Limits::default());
    i.record(&mut frame, Occurrence::Receipt, None).unwrap();
    i.create(&mut frame, std::slice::from_ref(call), None)
        .unwrap();
    CaseFrame {
        frame,
        queue: vec![QueueEntry { task: 0, at: 0 }],
        receipt: true,
        end: None,
    }
}
fn run_starts(m: &Model, world: World<'_>, times: Vec<Tick>, case: usize) -> Snapshot {
    STARTS.with_borrow_mut(|s| *s = Some(vec![]));
    tree(m, times).walk(world).unwrap();
    STARTS
        .with_borrow_mut(Option::take)
        .unwrap()
        .into_iter()
        .find(|s| s.case == case)
        .expect("actual reference Start")
}
fn single_start(m: &Model) -> Snapshot {
    let world = World {
        external_peak: 0,
        now: 0,
        next: 1,
        boundary: Boundary::Run,
        shared: vec![StateValue::None; m.state_variables.len()],
        cases: vec![CaseSlot::Here(queued(m))],
        steps: 0,
    };
    run_starts(m, world, vec![0], 0)
}
fn state(name: &str) -> (Model, Snapshot) {
    let src = crate::q6_acceptance::private_source("single");
    let mut m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
    m.start_policy = None;
    m.bounds.cases = 2;
    m.actors[0].capacity = Some(2);
    if matches!(
        name,
        "queued_other_actor" | "queued_other_capability_same_actor"
    ) {
        let mut cap = m.capabilities[0].clone();
        cap.name = "other".into();
        if name == "queued_other_actor" {
            let mut a = m.actors[0].clone();
            a.name = "Peer".into();
            cap.actor = ActorId(m.actors.len());
            m.actors.push(a);
            if let Some(metrics) = &mut m.metrics {
                metrics.actor_rates.push(None);
            }
        }
        m.capabilities.push(cap);
        if let Some(metrics) = &mut m.metrics {
            metrics.start_rates.push(None);
        }
    }
    let running = if name == "running_same_actor" {
        Some(single_start(&m))
    } else {
        None
    };
    let mut world = World {
        external_peak: 0,
        now: 0,
        next: 1,
        boundary: Boundary::Run,
        shared: vec![StateValue::None; m.state_variables.len()],
        cases: vec![CaseSlot::Here(queued(&m))],
        steps: 0,
    };
    let mut times = vec![0];
    if name != "self_only" {
        let mut other = queued(&m);
        let slot = match name {
            "running_same_actor" => {
                other.frame = running.as_ref().unwrap().frame(&m);
                CaseSlot::Here(other)
            }
            "held_same_actor" => CaseSlot::Held(other),
            "not_arrived" => CaseSlot::NotHere,
            "queued_other_actor" | "queued_other_capability_same_actor" => {
                other.frame.tasks[0].call.capability = CapabilityId(1);
                CaseSlot::Here(other)
            }
            "queued_in_stopped_case" => {
                let mut i = interpreter(&m, Limits::default());
                i.record(
                    &mut other.frame,
                    Occurrence::ResponseStopped(BindingId(0)),
                    None,
                )
                .unwrap();
                CaseSlot::Here(other)
            }
            "same_tick_first_start" | "same_tick_second_start" => CaseSlot::Here(other),
            _ => panic!("unknown {name}"),
        };
        world.cases.push(slot);
        times.push(u64::from(name == "not_arrived"));
        if name != "not_arrived" {
            world.next = 2;
        }
    }
    let target = usize::from(name == "same_tick_second_start");
    let snapshot = run_starts(&m, world, times, target);
    (m, snapshot)
}
pub(crate) fn backlog(name: &str) -> Value {
    let (m, s) = state(name);
    let kernel = work_profiles::kernel(&m, &s.tasks[0]).unwrap().unwrap();
    json!({"q":s.queued,"kernel":kernel.name})
}
fn bad(m: &Model, s: &Snapshot) -> String {
    let mut w = s.world(m);
    w.now = 4;
    w.cases[0].payload_mut().unwrap().frame.now = 4;
    let values = tree(m, vec![0]).walk(w).unwrap();
    assert!(
        values[0]
            .endpoints
            .iter()
            .all(|p| *p == values[0].endpoints[0])
    );
    values[0].endpoints[0].to_string()
}
pub(crate) fn identity() -> Value {
    let (lm, l) = state("self_only");
    let (hm, h) = state("same_tick_first_start");
    let lw = l.world(&lm);
    let hw = h.world(&hm);
    let ls = simulation::observed_signature(&lm, &lw);
    let hs = simulation::observed_signature(&hm, &hw);
    let clone_equal = l.tasks.clone() == h.tasks.clone();
    let mut lview = lw.clone();
    let mut hview = hw.clone();
    lview.cases[0].payload_mut().unwrap().frame.state_values = tree(&lm, vec![0]).values(&lw, 0);
    hview.cases[0].payload_mut().unwrap().frame.state_values = tree(&hm, vec![0]).values(&hw, 0);
    let view_store_equal =
        simulation::observed_signature(&lm, &lview) == simulation::observed_signature(&hm, &hview);
    let lr = crate::q6_acceptance::replay_selector("single", None).unwrap();
    let hr = crate::q6_acceptance::replay_selector("simultaneous", None).unwrap();
    assert_eq!(lr, l.tasks[0].frozen_performance);
    assert_eq!(hr, h.tasks[0].frozen_performance);
    json!({"equal":ls==hs,"hash_inputs_equal":ls==hs,"clone_equal":clone_equal,"view_store_equal":view_store_equal,
        "replay_equal":lr==hr,"low_bad":bad(&lm,&l),"high_bad":bad(&hm,&h)})
}
pub(crate) fn corruption(mode: &str, fault: &str) -> Value {
    let (m, mut s) = state("self_only");
    let replay = if mode == "replay" {
        Some(crate::q6_acceptance::finish_material())
    } else {
        None
    };
    if let Some((stage, choice)) = &replay {
        let mut frame = s.frame(&m);
        frame.now = 4;
        crate::reference::replay::apply(
            &m,
            &mut interpreter(&m, Limits::default()),
            &mut frame,
            stage,
            Some(choice),
        )
        .expect("valid replay control");
    }
    crate::q6_acceptance::corrupt(&mut s.tasks[0].frozen_performance, fault);
    let mut w = s.world(&m);
    w.now = 4;
    w.cases[0].payload_mut().unwrap().frame.now = 4;
    crate::q6_acceptance::begin_observations();
    let (aborted, draws) = match mode {
        "tree" => (tree(&m, vec![0]).walk(w).is_err(), 0),
        "sample" => simulation::observe_corrupt_sample(&mut tree(&m, vec![0]), w),
        "replay" => {
            let (stage, choice) = replay.as_ref().unwrap();
            (
                crate::reference::replay::apply(
                    &m,
                    &mut interpreter(&m, Limits::default()),
                    &mut w.cases[0].payload_mut().unwrap().frame,
                    stage,
                    Some(choice),
                )
                .is_err(),
                0,
            )
        }
        _ => panic!(),
    };
    let observed = crate::q6_acceptance::take_observations();
    let finishes = observed
        .events
        .iter()
        .filter(|(_, _, kind)| *kind == "finish")
        .count();
    json!({"aborted":aborted,"normal_finishes":finishes,"kernel_draws":draws})
}
