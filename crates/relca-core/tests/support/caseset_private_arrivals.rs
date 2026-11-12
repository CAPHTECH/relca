//! Internal cfg(test) child of engine/arrivals.rs.
use super::*;
fn model(name: &str) -> ir::Model {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap();
    crate::syntax::parse(&source)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap()
}

#[test]
fn aset_shared_bit_values_are_distinct_eq_hashset_states_without_hash_uniqueness_claim() {
    let m = model("aset-cases");
    let base = initial(&m, 2);
    let id = m
        .state_variables
        .iter()
        .position(|v| v.name == "keys")
        .unwrap();
    let mut states = HashSet::new();
    for bits in [0, 1, 2, 3] {
        let mut s = base.clone();
        s.shared[id] = ir::StateValue::Set(bits);
        states.insert(s);
    }
    assert_eq!(
        states.len(),
        4,
        "Eq and HashSet identity, not accidental hash inequality"
    );
    assert_eq!(base.shared[id], ir::StateValue::Set(0));
    assert!(base.cases.iter().all(|s| matches!(s, Slot::Future)));
}

#[test]
fn aset_sixty_four_is_finite_without_allocating_the_power_set() {
    let m = model("aset-limit");
    let v = m.state_variables.iter().find(|v| v.name == "keys").unwrap();
    assert_eq!(v.cardinality(&m.types), 18_446_744_073_709_551_616u128);
    assert!(v.contains(&m.types, ir::StateValue::Set(1u64 << 63)));
    let a = initial(&m, 64);
    let id = m
        .state_variables
        .iter()
        .position(|v| v.name == "keys")
        .unwrap();
    let mut b = a.clone();
    b.shared[id] = ir::StateValue::Set(1u64 << 63);
    assert_ne!(a, b);
    assert_eq!(HashSet::from([a, b]).len(), 2);
}

#[test]
fn aset_contains_alone_is_a_shared_branch_read_not_only_its_or_sibling() {
    let m = model("aset-branch-sharing");
    let ir::Stmt::If { condition, .. } = &m.process.body[0].kind else {
        panic!("expected membership branch")
    };
    assert!(m.reads_shared_observed(condition));
}

#[test]
fn aset_contains_only_branch_and_due_finish_share_the_same_candidate_layer() {
    let source = "model Layer version \"0.2\" {type T=enum{ok};actor Ledger{state keys:set<CaseId>};actor Gate{capacity 1;state flag:bool=false;capability set()->T{writes flag;finish{flag=true};duration fixed 1min;result nondet{ok}}};process P{a=Gate.set();wait 1min;if case.id in Ledger.keys{end inside}else{end outside}};cut C{include[Ledger,Gate]};bounds{cases 2;live_tasks 1;clock 5min};arrivals fixed[0min,0min];policy fifo tie_by case_id;verify reachable end.outside}";
    let m = crate::syntax::parse(source)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap();
    let mut program = vec![Instruction::Stop];
    compile(&m.process.body, 0, &mut program);
    let branch = program
        .iter()
        .position(|i| matches!(i, Instruction::Branch { .. }))
        .unwrap();
    let times = arrivals(&m).unwrap();
    let mut facts = Facts::new(2);
    let nodes = graph(&m, &times, 10_000, &mut facts).unwrap();
    let mut witnessed = false;
    // Gate case1 finishes1, case1 waits to2. Gate case2 runs1..2.
    // Thus at2 case1's Contains-only branch competes with case2's due finish.
    for node in &nodes {
        if node.state.time != 2 {
            continue;
        }
        let Slot::Admitted(first) = &node.state.cases[0] else {
            continue;
        };
        let Slot::Admitted(second) = &node.state.cases[1] else {
            continue;
        };
        if first.local.pc != branch
            || !second
                .local
                .tasks
                .iter()
                .any(|t| matches!(t.lifecycle, Lifecycle::Running { finish: 2, .. }))
        {
            continue;
        }
        let mut changes = BTreeSet::new();
        for action in &node.actions {
            for (_, to) in action {
                let successor = &nodes[*to].state;
                let Slot::Admitted(a) = &successor.cases[0] else {
                    panic!("case1 lost")
                };
                let Slot::Admitted(b) = &successor.cases[1] else {
                    panic!("case2 lost")
                };
                changes.insert((
                    a.local.pc != branch,
                    b.local.tasks.iter().any(|t| t.lifecycle == Lifecycle::Done),
                ));
            }
        }
        witnessed |= changes.contains(&(true, false)) && changes.contains(&(false, true));
    }
    assert!(
        witnessed,
        "Contains-only Branch and due Finish must be alternative actions from one node, not a management-closure branch plus later finish"
    );
}
