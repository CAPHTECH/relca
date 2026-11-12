//! ADR 0036 private CENSUS/CLASS/IR/ORDER/REFUSE/LABEL/TIE obligations.
use super::*;

fn source(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
fn model(src: &str) -> ir::Model {
    crate::syntax::parse(src)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap()
}
fn seed(m: &ir::Model) -> (Global, Vec<Instruction>, usize) {
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    let Instruction::Wait { calls, .. } = &program[entry] else {
        panic!("first instruction must queue calls")
    };
    let mut local = State::initial(m, entry);
    local.initialized = true;
    local.waiting = true;
    local.tasks = calls
        .iter()
        .map(|c| Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: c.binding,
            capability: c.capability,
            lifecycle: Lifecycle::Queued,
        })
        .collect();
    let mut s = initial(m, 1);
    s.cursor = 1;
    s.phase = Phase::Mixed;
    s.cases[0] = Slot::Admitted(
        Case {
            admitted_at: None,
            queued_at: vec![0; calls.len()],
            local,
            ever_started: false,
            received: true,
            terminal: None,
        }
        .into(),
    );
    (s, program, entry)
}
fn step(
    s: &Global,
    m: &ir::Model,
    p: &[Instruction],
    entry: usize,
    budget: usize,
) -> (Vec<Action>, usize, Facts) {
    let mut r = Room(budget);
    let mut f = Facts::new(s.cases.len());
    let xs = transition(s, m, &[0], p, entry, &mut r, &mut f)
        .unwrap()
        .unwrap();
    (xs, budget - r.0, f)
}

#[test]
fn policy_census_4_2_2_1_charges_only_selected_children_and_retains_cofinishes() {
    let src = r#"model M version "0.2" { type T=enum {ok} people [p,q] actor A {executors [p,q]; capability work()->T {duration fixed 3min; result nondet {ok}}} process P {fork f {a=A.work(); b=A.work()}; join all f; end done} bounds {cases 1; live_tasks 2; clock 6min} policy fifo tie_by case_id EXT verify reachable end.done }"#;
    for (ext, expected) in [
        ("", 4),
        ("then call_order", 2),
        ("assign first_idle", 2),
        ("then call_order assign first_idle", 1),
    ] {
        let m = model(&src.replace("EXT", ext));
        let (s, p, entry) = seed(&m);
        let (xs, charged, _) = step(&s, &m, &p, entry, expected);
        assert_eq!((xs.len(), charged), (expected, expected));
        if expected == 1 {
            let (ys, charged, _) = step(&xs[0][0].1, &m, &p, entry, 1);
            assert_eq!((ys.len(), charged), (1, 1));
            let mut due = ys[0][0].1.clone();
            due.time = 3;
            let (finishes, charged, _) = step(&due, &m, &p, entry, 2);
            assert_eq!(
                (finishes.len(), charged),
                (2, 2),
                "finish orders are environment choices"
            );
            for action in finishes {
                let Slot::Admitted(c) = &action[0].1.cases[0] else {
                    panic!()
                };
                assert_eq!(
                    c.local
                        .tasks
                        .iter()
                        .filter(|t| t.lifecycle == Lifecycle::Done)
                        .count(),
                    1
                );
            }
        }
    }
}

#[test]
fn policy_class_metadata_erasure_preserves_outcome_keys_generated_and_held() {
    for name in ["pol-separation", "pol-cross-class-person", "pol-permission"] {
        let m = model(&source(name));
        let out = super::super::explore(&m);
        let a = arrivals(&m).unwrap();
        let mut f = Facts::new(a.len());
        let nodes = graph(&m, &a, 100_000, &mut f).unwrap();
        let keys: HashSet<_> = nodes.iter().map(|n| n.state.clone()).collect();
        for erase in [true, false] {
            let mut changed = m.clone();
            for actor in &mut changed.actors {
                if erase {
                    actor.metadata = Default::default();
                } else {
                    actor.metadata.control = Some("environment".into());
                }
            }
            assert_eq!(super::super::explore(&changed), out);
            let mut after = Facts::new(a.len());
            let nodes = graph(&changed, &a, 100_000, &mut after).unwrap();
            assert_eq!(after.census, f.census);
            assert_eq!(
                nodes
                    .iter()
                    .map(|n| n.state.clone())
                    .collect::<HashSet<_>>(),
                keys,
                "metadata must not enter state identity"
            );
        }
    }
}

#[test]
fn policy_ir_classification_and_call_order_are_independently_fail_closed() {
    let m = model(&source("pol-separation"));
    for fault in 0..13 {
        let mut bad = m.clone();
        let p = bad.start_policy.as_mut().unwrap();
        match fault {
            0 => p.classes = None,
            1 => p
                .classes
                .as_mut()
                .unwrap()
                .policy
                .push(ir::ActorId(m.actors.len())),
            2 => {
                let c = p.classes.as_mut().unwrap();
                c.policy.push(c.policy[0]);
            }
            3 => {
                let c = p.classes.as_mut().unwrap();
                c.environment.push(c.policy[0]);
            }
            4 => {
                p.classes.as_mut().unwrap().policy.pop();
            }
            5 => {
                p.call_order = false;
                p.assign_first_idle = false;
            }
            6 => {
                if let ir::Stmt::Call(c) = &mut bad.process.body[0].kind {
                    c.binding = ir::BindingId(m.process.bindings.len());
                } else {
                    panic!()
                }
            }
            7 => p.executor_order.clear(),
            8 => p.executor_order[0] = None,
            9 => p.executor_order[0] = Some(vec![ir::PersonId(m.people.len())]),
            10 => p.executor_order[0] = Some(vec![ir::PersonId(0), ir::PersonId(0)]),
            11 => p.executor_order[0] = Some(vec![ir::PersonId(0)]),
            _ => {
                p.call_order = false;
                p.assign_first_idle = false;
                p.classes = None;
            }
        }
        let e = super::super::explore(&bad);
        assert!(
            e.properties
                .iter()
                .all(|p| p.verdict == Verdict::Unsupported),
            "fault{fault}"
        );
        assert!(e.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
        assert_eq!(
            crate::reference::explore(&bad).stop,
            crate::reference::StopReason::SemanticAbort,
            "fault{fault}"
        );
    }
    let empty = model(
        r#"model M version "0.2" {process P {end done} bounds {cases 1;live_tasks 1;clock 1min} policy fifo tie_by case_id then call_order verify reachable end.done}"#,
    );
    assert!(
        super::super::explore(&empty)
            .properties
            .iter()
            .all(|p| p.verdict == Verdict::Satisfied)
    );
    assert_eq!(
        crate::reference::explore(&empty).stop,
        crate::reference::StopReason::Complete
    );
    let all_environment = model(&source("pol-control.broken-change"));
    assert!(
        all_environment
            .start_policy
            .as_ref()
            .unwrap()
            .classes
            .as_ref()
            .unwrap()
            .policy
            .is_empty()
    );
    assert_eq!(
        crate::reference::explore(&all_environment).stop,
        crate::reference::StopReason::Complete
    );
    assert!(
        !super::super::explore(&all_environment)
            .diagnostics
            .iter()
            .any(|d| d.code == diag::U_SEM_0)
    );
}

const REFUSE: &str = r#"model M version "0.2" {type R=enum {ok,rejected} people [p] actor Ledger {executors [p]; state posted:0..1=1; capability post()->R {requires posted==0 else rejected; duration fixed 1min;result nondet {ok}}} actor Tail {capacity 1; capability work()->R {duration fixed 2min; result nondet {ok}}} process P {fork f {a=Ledger.post();b=Tail.work()}; join all f;end done} cut C {include [Ledger,Tail]} bounds {cases 1;live_tasks 2;clock 5min} policy fifo tie_by case_id then call_order assign first_idle verify reachable end.done}"#;

#[test]
fn policy_refuse_is_one_edge_one_effect_no_holder_then_next_start() {
    let m = model(REFUSE);
    let (s, p, entry) = seed(&m);
    let (xs, charged, _) = step(&s, &m, &p, entry, 1);
    assert_eq!((xs.len(), charged), (1, 1));
    let n = &xs[0][0].1;
    let Slot::Admitted(c) = &n.cases[0] else {
        panic!()
    };
    assert_eq!(c.local.work, 0);
    assert_eq!(c.local.tasks[0].lifecycle, Lifecycle::Done);
    assert_eq!(c.local.tasks[0].executor, None);
    assert_eq!(c.local.tasks[1].lifecycle, Lifecycle::Queued);
    assert_eq!(n.resource_occupancy(&m).unwrap(), Vec::<u64>::new());
    let mut capture = trace::Capture::new(REFUSE, &m);
    capture.global_context(n, &m);
    let stage = capture.global_label(&s, n, &m).unwrap();
    assert_eq!(stage.kind, crate::explain::Kind::Refuse);
    assert_eq!(stage.executor, None);
    assert_eq!(
        stage
            .events
            .iter()
            .filter(|e| matches!(e, crate::explain::Event::Business(_)))
            .count(),
        1
    );
    assert!(stage.resource.as_ref().is_none_or(|r| r.holders.is_empty()));
    let (next, charged, _) = step(n, &m, &p, entry, 1);
    assert_eq!((next.len(), charged), (1, 1));
    let Slot::Admitted(c) = &next[0][0].1.cases[0] else {
        panic!()
    };
    assert!(c.local.tasks[1].lifecycle.running());
    assert!(!c.local.tasks[0].lifecycle.running());
}

#[test]
fn policy_actor_hol_does_not_skip_a_resource_blocked_earlier_same_case_call() {
    let src = r#"model M version "0.2" {type T=enum {ok} resource R capacity 0 actor A {capacity 2;capability x()->T {uses R:1;duration fixed 2min;result nondet {ok}} capability y()->T {duration fixed 1min;result nondet {ok}}} process P {fork f {a=A.x();b=A.y()};join all f;end done} bounds {cases 1;live_tasks 2;clock 5min} policy fifo tie_by case_id then call_order verify reachable end.done}"#;
    let m = model(src);
    let (s, p, entry) = seed(&m);
    assert!(
        s.fixed_starts(&m).unwrap().is_empty(),
        "same-Actor HOL applies even when the later call claims no shared resource"
    );
    let mut f = Facts::new(1);
    assert!(
        transition(&s, &m, &[0], &p, entry, &mut Room(1), &mut f)
            .unwrap()
            .is_none()
    );
    assert!(f.deadlock);
    assert_eq!(
        crate::reference::explore(&m).outcome.case_outcomes.unwrap()[0]
            .terminals
            .len(),
        0
    );
}

#[test]
fn policy_labels_keep_unknown_suffixes_and_do_not_create_scheduler_rows() {
    let m = model(&source("pol-label-cut"));
    let complete = explore(&m, 100_000);
    assert!(complete.probability_scheduler);
    assert_eq!(complete.probability_policy_unresolved, Some(false));
    let mut observed_cut = false;
    for budget in 1..200 {
        let out = explore(&m, budget);
        if out.truncated && out.probability_scheduler {
            assert_eq!(out.probability_policy_unresolved, Some(true));
            observed_cut = true;
            break;
        }
    }
    assert!(
        observed_cut,
        "budget cut must occur after observed chance and nondeterminism"
    );
    let zero = explore(&m, 0);
    assert!(
        !zero.probability_scheduler,
        "classification cannot invent a scheduler row"
    );

    // An explicit random/nondeterministic initial prefix precedes both failures.
    let prefix = r#"model M version "0.2" {type T=enum {ok} latent case {coin:bool~{true:0.5,false:0.5}; choice:bool} actor A {capacity 1;state x:0..1=1;capability run()->T {duration fixed 1min;result nondet {ok}; writes x;finish {x+=1}}} process P {wait 1min;a=A.run();end done} bounds {cases 1;live_tasks 1;clock 4min} policy fifo tie_by case_id then call_order verify safety no_resource_overcommit}"#;
    let range = model(prefix);
    let out = explore(&range, 100_000);
    assert!(out.probability_scheduler);
    assert_eq!(out.state_range_violations.len(), 1);
    assert_eq!(out.probability_policy_unresolved, Some(true));
    let mut abort = range.clone();
    abort.capabilities[0].duration = u64::MAX;
    let out = explore(&abort, 100_000);
    assert!(out.probability_scheduler);
    assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
    assert_eq!(out.probability_policy_unresolved, Some(true));

    let tiny = format!("0.{}1", "0".repeat(62));
    let rest = format!("0.{}", "9".repeat(63));
    let mut body = String::new();
    for i in 0..20 {
        body.push_str(&format!("x{i}=A.flip();if x{i}==pass {{"));
    }
    body.push_str("end yes");
    body.push_str(&"} else {end no}".repeat(20));
    let src = format!(
        r#"model M version "0.2" {{type T=enum {{pass,fail}} latent case {{coin:bool~{{true:0.5,false:0.5}};choice:bool}} kernel K{{* -> {{{tiny}:pass;{rest}:fail}}}} actor A{{capacity 1;capability flip()->T{{duration fixed 1min;outcomes kernel K}}}} process P{{{body}}} bounds{{cases 1;live_tasks 1;clock 25min}} policy fifo tie_by case_id then call_order verify probability reached end.yes <= 1}}"#
    );
    let numeric = model(&src);
    let out = explore(&numeric, 100_000);
    assert!(out.probability_scheduler);
    assert!(out.truncated);
    assert_eq!(out.probability_policy_unresolved, Some(true));
    // The 20 nested flips walk the reference deeper than the 2MiB test-thread
    // stack in a debug build once ADR0035/0037 and ADR0039 share its
    // walk_inner (it passed on each branch alone); larger stack, as in the
    // other deep reference walks (actor_table_cut_debtor*.rs).
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn_scoped(scope, || {
                for input in [&range, &abort, &numeric] {
                    let r = crate::reference::explore(input);
                    assert!(r.outcome.probability_scheduler);
                    assert_eq!(r.outcome.probability_policy_unresolved, Some(true));
                }
            })
            .unwrap()
            .join()
            .unwrap();
    });
}

#[test]
fn policy_tie_classification_counts_winners_before_ack_children() {
    for (name, unresolved) in [
        ("pol-tie-ack", false),
        ("pol-single-winner", false),
        ("pol-tie-nondet.broken-change", true),
    ] {
        let m = model(&source(name));
        let a = arrivals(&m).unwrap();
        let mut f = Facts::new(a.len());
        graph(&m, &a, 100_000, &mut f).unwrap();
        assert_eq!(f.policy_unresolved, unresolved, "{name}");
        assert!(f.scheduler && f.chance);
        assert_eq!(
            explore(&m, 100_000).probability_policy_unresolved,
            Some(unresolved)
        );
    }
}

#[test]
fn policy_latent_initial_product_is_four_quarters_not_a_priority_input() {
    let m = model(&source("pol-latent"));
    let mut p = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut p);
    let mut s = initial(&m, 2);
    s.phase = Phase::Admission;
    let mut f = Facts::new(2);
    let admitted = transition(&s, &m, &[0, 0], &p, entry, &mut Room(100), &mut f)
        .unwrap()
        .unwrap();
    let xs = initialize(&admitted[0][0].1, &m, &mut Room(4), &mut f, true).unwrap();
    assert_eq!(xs.len(), 1);
    assert_eq!(xs[0].len(), 4);
    let hard = m
        .state_variables
        .iter()
        .position(|v| v.name == "hard")
        .unwrap();
    let mut seen = BTreeSet::new();
    for (weight, s) in &xs[0] {
        assert_eq!(weight.to_string(), "1/4");
        let tuple: Vec<_> = s
            .cases
            .iter()
            .map(|slot| {
                let Slot::Admitted(c) = slot else { panic!() };
                c.local.state_values[hard]
            })
            .collect();
        seen.insert(format!("{tuple:?}"));
    }
    assert_eq!(seen.len(), 4);
}

#[test]
fn policy_contest_synthetic_suffix_orders_routes_and_uses_declared_executor_order() {
    let original = source("ctx-contest-resource");
    let src=original.replace("[j1,j2,p]","[j1,j2,p,q]").replace("actor Route { executors [p]","actor Route { control policy; executors [q,p]").replace("policy fifo tie_by case_id","policy fifo tie_by case_id then call_order assign first_idle").replace("respond_by 4min }","respond_by 4min; contest Party on end.returned route Route.respond reversible_by [j1] respond_by 4min }");
    assert_ne!(src, original);
    let m = model(&src);
    assert_eq!(m.contests().len(), 2);
    let mut s = initial(&m, 1);
    s.phase = Phase::Mixed;
    s.cursor = 1;
    let mut local = State::initial(&m, 0);
    local.initialized = true;
    local.tasks = m
        .contests()
        .iter()
        .enumerate()
        .map(|(i, c)| Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: m.contest_site(i),
            capability: c.route,
            lifecycle: Lifecycle::Queued,
        })
        .collect();
    s.cases[0] = Slot::Admitted(
        Case {
            admitted_at: None,
            local,
            queued_at: vec![0, 0],
            ever_started: true,
            received: true,
            terminal: m.contests()[0].target.end(),
        }
        .into(),
    );
    let choices = s.fixed_starts(&m).unwrap();
    let q = m.people.iter().position(|p| p.name == "q").unwrap();
    assert_eq!(
        choices,
        BTreeMap::from([((0, 0), vec![Some(ir::PersonId(q))])])
    );
    assert!(m.contest_site(0).0 >= m.process.bindings.len());
    let mut reordered = s.clone();
    let Slot::Admitted(c) = &mut reordered.cases[0] else {
        panic!()
    };
    c.local.tasks.swap(0, 1);
    assert_eq!(
        reordered.fixed_starts(&m).unwrap(),
        BTreeMap::from([((0, 1), vec![Some(ir::PersonId(q))])]),
        "contest ordinal, not runtime vector position"
    );
}
