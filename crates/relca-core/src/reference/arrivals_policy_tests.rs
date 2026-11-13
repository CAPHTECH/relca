//! Independent local candidate/initial-world oracles, not engine helpers.
use super::*;
fn model(src: &str) -> Model {
    crate::syntax::parse(src)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap()
}
fn tree(m: &Model, n: usize) -> Tree<'_> {
    let limits = Limits::default();
    Tree {
        walk_work: None,
        recovery_paths: Vec::new(),
        response_exercise: BTreeMap::new(),
        response_receipts: Vec::new(),
        first_hits: Vec::new(),
        first_closed: Vec::new(),
        first_trees: Vec::new(),
        overflows: None,
        external_peaks: None,
        admissions: vec![None; n],
        unadmitted: vec![false; n],
        resource_worlds: None,
        depth: 0,
        m,
        limits,
        times: vec![0; n],
        cases: (0..n).map(|_| interpreter(m, limits)).collect(),
        terminals: vec![BTreeMap::new(); n],
        unresolved: vec![false; n],
        stats: Default::default(),
        truncated: false,
        numeric: false,
        deadlock: false,
        overcommit: false,
        chance: false,
        scheduler: false,
        policy_unresolved: false,
        rejects: None,
        initial_leaves: Vec::new(),
        receipts: Vec::new(),
        duration_leaves: Vec::new(),
    }
}
fn world(m: &Model, n: usize, receipt: bool) -> World<'_> {
    World {
        external_peak: 0,
        now: 0,
        next: n,
        boundary: Boundary::Run,
        steps: 0,
        shared: m
            .state_variables
            .iter()
            .map(|v| match v.initial {
                InitialValue::Fixed(x) if shared_state(m, v.kind) => x,
                _ => StateValue::None,
            })
            .collect(),
        cases: (0..n)
            .map(|_| {
                CaseSlot::Here(CaseFrame {
                    frame: super::super::replay::initial(m),
                    queue: Vec::new(),
                    receipt,
                    end: None,
                })
            })
            .collect(),
    }
}

#[test]
fn policy_reference_local_census_is_4_2_2_1_and_first_choice_is_not_a_history() {
    let source = r#"model M version "0.2" {type T=enum {ok} people [p,q] actor A {executors [p,q];capability work()->T {duration fixed 3min;result nondet {ok}}} process P {fork f {a=A.work();b=A.work()};join all f;end done} bounds {cases 1;live_tasks 2;clock 6min} policy fifo tie_by case_id EXT verify reachable end.done}"#;
    for (ext, count) in [
        ("", 4),
        ("then call_order", 2),
        ("assign first_idle", 2),
        ("then call_order assign first_idle", 1),
    ] {
        let m = model(&source.replace("EXT", ext));
        let t = tree(&m, 1);
        let mut w = world(&m, 1, true);
        let Stmt::Fork { branches, join, .. } = &m.process.body[0].kind else {
            panic!()
        };
        let CaseSlot::Here(c) = &mut w.cases[0] else {
            panic!()
        };
        t.cases[0]
            .create(&mut c.frame, branches, Some(join))
            .unwrap();
        c.frame.control = vec![&m.process.body[1..]];
        c.queue = vec![QueueEntry { task: 0, at: 0 }, QueueEntry { task: 1, at: 0 }];
        let sizes: Vec<_> = (0..2)
            .map(|j| t.eligible(&w, 0, j).unwrap().len())
            .collect();
        assert_eq!(sizes.iter().sum::<usize>(), count);
        if count == 1 {
            assert_eq!(sizes, [1, 0]);
            // Give the real walker exactly one clone, then intentionally stop
            // recursion at its child. Dispatch must have been observed before
            // that independent depth bound, not lost to eager discarded clones.
            let mut one = tree(&m, 1);
            one.limits.frames = 1;
            one.limits.branch_depth = 1;
            assert_eq!(
                one.walk(w.clone()).err(),
                Some(StopReason::EnumerationLimit)
            );
            assert_eq!(one.stats.frames, 1);
            assert_eq!(
                one.cases[0].statistics.observed.people,
                BTreeSet::from([PersonId(0)])
            );
            assert_eq!(
                one.cases[0].statistics.observed.capabilities,
                BTreeSet::from([CapabilityId(0)])
            );
            let CaseSlot::Here(c) = &mut w.cases[0] else {
                panic!()
            };
            c.frame.tasks[0].phase = Phase::Running;
            c.frame.tasks[0].person = Some(PersonId(0));
            c.frame.tasks[0].start = Some(0);
            c.frame.tasks[0].finish = Some(3);
            assert_eq!(t.eligible(&w, 0, 1).unwrap(), [Some(PersonId(1))]);
        }
    }
}

#[test]
fn policy_reference_joint_initial_worlds_have_all_four_quarter_tuples() {
    let src = include_str!("../../../../fixtures/pol-latent/model.relca");
    let m = model(src);
    let mut t = tree(&m, 2);
    let w = world(&m, 2, false);
    t.initials(&w, true).unwrap();
    let initial: Vec<_> = t
        .initial_leaves
        .iter()
        .filter(|(at, _, _)| *at == 0)
        .collect();
    assert_eq!(initial.len(), 4);
    let hard = m
        .state_variables
        .iter()
        .position(|v| v.name == "hard")
        .unwrap();
    let mut seen = BTreeSet::new();
    for (at, values, p) in initial {
        assert_eq!(*at, 0);
        assert_eq!(p.to_string(), "1/4");
        seen.insert(format!("{:?},{:?}", values[0][hard], values[1][hard]));
    }
    assert_eq!(seen.len(), 4);
    assert_eq!(
        t.terminals[0]
            .values()
            .next()
            .unwrap()
            .completed_at
            .unwrap(),
        Range { min: 2, max: 2 }
    );
    assert_eq!(
        t.terminals[1]
            .values()
            .next()
            .unwrap()
            .completed_at
            .unwrap(),
        Range { min: 4, max: 4 }
    );
}
