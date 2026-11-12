//! Observations of actual private states/transitions for the fixed Q6 seam.
use super::*;
use serde_json::{Value, json};

struct Started {
    model: ir::Model,
    state: Global,
    times: Vec<u64>,
    program: Vec<Instruction>,
    entry: usize,
    case: usize,
    queued: u64,
}

fn start(
    s: &Global,
    m: &ir::Model,
    times: &[u64],
    program: &[Instruction],
    entry: usize,
    case: usize,
) -> Global {
    let actions = transition(
        s,
        m,
        times,
        program,
        entry,
        &mut Room(10000),
        &mut Facts::new(times.len()),
    )
    .unwrap()
    .unwrap();
    actions
        .into_iter()
        .flatten()
        .map(|(_, s)| s)
        .find(
            |s| matches!(&s.cases[case],Slot::Admitted(c) if c.local.tasks[0].lifecycle.running()),
        )
        .expect("actual Start successor")
}

fn started(name: &str) -> Started {
    let src = crate::q6_acceptance::private_source("single");
    let mut m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
    let mut facts = Facts::new(1);
    let nodes = graph(&m, &[0], 10000, &mut facts).unwrap();
    let mut s = nodes.iter().find(|n| n.state.phase == Phase::Mixed
        && matches!(&n.state.cases[0],Slot::Admitted(c) if c.local.tasks.len()==1 && c.local.tasks[0].lifecycle==Lifecycle::Queued))
        .expect("real queued state").state.as_ref().clone();
    let running = nodes
        .iter()
        .find_map(|n| match &n.state.cases[0] {
            Slot::Admitted(c)
                if n.state.time == 0
                    && c.local.tasks.len() == 1
                    && c.local.tasks[0].lifecycle.running() =>
            {
                Some(c.clone())
            }
            _ => None,
        })
        .unwrap();
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    m.start_policy = None;
    m.actors[0].capacity = Some(2);
    m.bounds.cases = 2;
    let mut times = vec![0];
    if name != "self_only" {
        let Slot::Admitted(base) = &s.cases[0] else {
            unreachable!()
        };
        let mut other = base.clone();
        let slot = match name {
            "running_same_actor" => Slot::Admitted(running),
            "held_same_actor" => Slot::Held(other),
            "not_arrived" => Slot::Future,
            "queued_other_actor" | "queued_other_capability_same_actor" => {
                let mut cap = m.capabilities[0].clone();
                cap.name = "other".into();
                if name == "queued_other_actor" {
                    let mut a = m.actors[0].clone();
                    a.name = "Peer".into();
                    cap.actor = ir::ActorId(m.actors.len());
                    m.actors.push(a);
                    if let Some(metrics) = &mut m.metrics {
                        metrics.actor_rates.push(None);
                    }
                }
                let id = ir::CapabilityId(m.capabilities.len());
                m.capabilities.push(cap);
                if let Some(metrics) = &mut m.metrics {
                    metrics.start_rates.push(None);
                }
                other.local.tasks[0].capability = id;
                Slot::Admitted(other)
            }
            "queued_in_stopped_case" => {
                other.terminal = Some(ir::EndId(0));
                Slot::Admitted(other)
            }
            "same_tick_first_start" | "same_tick_second_start" => Slot::Admitted(other),
            _ => panic!("unknown state distinction {name}"),
        };
        s.cases.push(slot);
        times.push(u64::from(name == "not_arrived"));
        if name != "not_arrived" {
            s.cursor = 2;
        }
    }
    let case = if name == "same_tick_second_start" {
        s = start(&s, &m, &times, &program, entry, 0);
        1
    } else {
        0
    };
    let queued = s.queued_for_performance(case, 0, m.capabilities[0].actor, &m);
    let state = start(&s, &m, &times, &program, entry, case);
    Started {
        model: m,
        state,
        times,
        program,
        entry,
        case,
        queued,
    }
}

pub(crate) fn backlog(name: &str) -> Value {
    let s = started(name);
    let task = &s.state.cases[s.case].payload().unwrap().local.tasks[0];
    let kernel = super::super::work_profiles::kernel(&s.model, task)
        .unwrap()
        .unwrap();
    json!({"q":s.queued,"kernel":kernel.name})
}

#[derive(Default)]
struct HashInput(Vec<u8>);
impl std::hash::Hasher for HashInput {
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
    fn finish(&self) -> u64 {
        0
    }
}
fn hash_input(task: &Task) -> Vec<u8> {
    use std::hash::Hash;
    let mut h = HashInput::default();
    task.hash(&mut h);
    h.0
}
fn finished_bad(s: &Started) -> String {
    let mut view = s.state.view(s.case, &s.model).unwrap();
    view.time = 4;
    let kernel = super::super::work_profiles::kernel(&s.model, &view.tasks[0])
        .unwrap()
        .unwrap();
    let action = kernel_complete_in(
        &view,
        0,
        kernel,
        &s.model,
        &mut Room(100),
        &mut Exploration::default(),
        s.case,
    )
    .unwrap();
    let mut mass = Rational::zero();
    for (p, next) in action {
        if query_hit(&s.model.probability_queries[0], &next, None, 0).unwrap() {
            mass.0 += p.0;
        }
    }
    mass.to_string()
}
pub(crate) fn identity() -> Value {
    let mut low = started("self_only");
    let mut high = started("same_tick_first_start");
    let lt = low.state.cases[low.case].payload().unwrap().local.tasks[0].clone();
    let ht = high.state.cases[high.case].payload().unwrap().local.tasks[0].clone();
    let clone_equal = lt.clone() == ht.clone();
    let lv = low.state.view(low.case, &low.model).unwrap();
    let hv = high.state.view(high.case, &high.model).unwrap();
    low.state.store(low.case, lv, &low.model);
    high.state.store(high.case, hv, &high.model);
    let view_store_equal = low.state.view(low.case, &low.model).unwrap().tasks[0]
        == high.state.view(high.case, &high.model).unwrap().tasks[0];
    let lr = crate::q6_acceptance::replay_selector("single", None).unwrap();
    let hr = crate::q6_acceptance::replay_selector("simultaneous", None).unwrap();
    assert_eq!(lr, lt.frozen_performance);
    assert_eq!(hr, ht.frozen_performance);
    json!({"equal":lt==ht,"hash_inputs_equal":hash_input(&lt)==hash_input(&ht),"clone_equal":clone_equal,
        "view_store_equal":view_store_equal,"replay_equal":lr==hr,"low_bad":finished_bad(&low),"high_bad":finished_bad(&high)})
}

pub(crate) fn corruption(mode: &str, fault: &str) -> Value {
    if mode == "replay" {
        let result = crate::q6_acceptance::replay_selector("single", Some(fault));
        let observed = crate::q6_acceptance::take_observations();
        return json!({"aborted":result.is_err(),"normal_finishes":observed.replayed.len(),"kernel_draws":observed.draws.len()});
    }
    let mut s = started("self_only");
    let task = &mut s.state.cases[s.case].payload_mut().unwrap().local.tasks[0];
    crate::q6_acceptance::corrupt(&mut task.frozen_performance, fault);
    s.state.time = 4;
    let mut events = Exploration::default();
    let (aborted, draws) = if mode == "tree" {
        let mut facts = Facts::new(s.times.len());
        let result = transition(
            &s.state,
            &s.model,
            &s.times,
            &s.program,
            s.entry,
            &mut Room(100),
            &mut facts,
        );
        events = facts.per.remove(s.case);
        (result.is_err(), events.kernel_draws)
    } else {
        let mut sample = simulation::Sample::new(1, 1);
        let result = sample_kernel_in(
            &s.state.view(s.case, &s.model).unwrap(),
            0,
            &s.model,
            &mut sample,
            &mut events,
            s.case,
        );
        (result.is_err(), sample.draws.len())
    };
    let finishes = events
        .state_events
        .iter()
        .filter(|(_, e, _)| matches!(e, Bundle::Finish(..)))
        .count();
    json!({"aborted":aborted,"normal_finishes":finishes,"kernel_draws":draws})
}
