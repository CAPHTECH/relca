use super::*;

#[test]
fn fixed_rule_guard_adds_zero_shared_read_candidates() {
    let src = r#"model M version "0.2" { actor R { version "r1"; decides enabled: bool = true } process P { if R.enabled == true { end yes } else { end no } } cut C { include [R] } bounds { cases 2; live_tasks 2; clock 2min } arrivals fixed [0min,0min] verify safety no_resource_overcommit }"#;
    let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
    let kind = m.state_variables[0].kind;
    assert!(!m.is_shared_state(kind));
    assert!(!m.is_shared_observed(kind));
    let ir::Stmt::If { condition, .. } = &m.process.body[0].kind else {
        panic!()
    };
    assert!(!m.reads_shared_observed(condition));
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    let mut state = initial(&m, 2);
    state.cursor = 2;
    state.phase = Phase::Mixed;
    for k in 0..2 {
        let mut local = State::initial(&m, entry);
        local.initialized = true;
        state.cases[k] = Slot::Admitted(
            Case {
                admitted_at: None,
                queued_at: Vec::new(),
                local,
                ever_started: false,
                received: true,
                terminal: None,
            }
            .into(),
        );
    }
    let added_candidates = state.cases.iter().filter(|slot| matches!(slot, Slot::Admitted(c) if matches!(&program[c.local.pc], Instruction::Branch { condition, .. } if m.reads_shared_observed(condition)))).count();
    assert_eq!(added_candidates, 0);
    let actions = transition(
        &state,
        &m,
        &[0, 0],
        &program,
        entry,
        &mut Room(100),
        &mut Facts::new(2),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        actions.len(),
        1,
        "Rule Branch is deterministic management, not two shared-read choices"
    );
    let Slot::Admitted(first) = &actions[0][0].1.cases[0] else {
        panic!()
    };
    assert_ne!(first.local.pc, entry);
}
