use super::*;

#[test]
fn finite_commands_keep_case_identity_and_shared_key_bank() {
    let source = r#"model M version "0.2" {
        type Ack=enum { committed, duplicate }
        actor L { capacity 1; state keys:set<CaseId>
            capability post(key:CaseId) -> Ack { commits; idempotency key key;
                writes keys; duration fixed 1min;
                atomic { if key in keys { result duplicate } else { keys += key; result committed } }
            }
        }
        process P { r=L.post(case.id) response_loss sequence [unknown, delivered]
            retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed; end done }
        arrivals fixed [0min,4min]
        policy fifo tie_by case_id
        bounds { cases 2; live_tasks 2; clock 10min }
        verify safety no_resource_overcommit
    }"#;
    let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
    let times = arrivals(&m).unwrap();
    let mut facts = Facts::new(2);
    let nodes = graph(&m, &times, 10000, &mut facts).unwrap();
    let finished: Vec<_> = nodes
        .iter()
        .filter(|n| {
            n.state
                .cases
                .iter()
                .all(|slot| matches!(slot, Slot::Admitted(c) if c.terminal.is_some()))
        })
        .collect();
    assert!(!finished.is_empty());
    for node in finished {
        for (k, slot) in node.state.cases.iter().enumerate() {
            let Slot::Admitted(c) = slot else {
                panic!("admitted");
            };
            let history = c.local.response.as_ref().unwrap();
            assert_eq!(history.rows.len(), 2);
            assert_eq!(
                history.inputs.values().next().unwrap(),
                &vec![ir::ValueId(k)]
            );
            assert_eq!(history.rows[0].at, 1 + 4 * k as u64);
            assert_eq!(history.rows[1].at, 2 + 4 * k as u64);
            assert!(history.unresolved.is_empty());
            assert_eq!(c.local.work, 2);
            assert_eq!(node.state.view(k, &m).unwrap().response, c.local.response);
        }
        let mut stored = (*node.state).clone();
        let before = stored.cases[0].payload().unwrap().local.response.clone();
        stored.store(1, stored.view(1, &m).unwrap(), &m);
        assert_eq!(before, stored.cases[0].payload().unwrap().local.response);
    }
}
