use super::*;

#[test]
fn response_history_maps_instruction_spans_and_real_commits_without_leaking_receipt() {
    let src = r#"model M version "0.2" { type V=enum {ok}
        observed case { n:0..2=0 }
        actor A { capacity 1; capability c()->V { commits; effect_scope modeled;
            writes n; duration fixed 1min; result nondet {ok}; finish {n+=1} } }
        process P {r=A.c() response_loss sequence [unknown,delivered]
            retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed; end done}
        contract C {at_most_once A.c.commit per case.id}
        bounds { cases 1; live_tasks 1; clock 3min } verify safety no_resource_overcommit }"#;
    let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
    let (out, trace) = explore(src, &m);
    assert!(!out.truncated);
    assert!(trace.failure.is_none(), "{:?}", trace.failure);
    let (steps, _, _) = trace.shortest("contract C #1 at_most_once").unwrap();
    for attempt in [1, 2] {
        let finish = steps
            .iter()
            .find(|s| s.attempt == Some(attempt) && s.kind == Kind::Finish)
            .unwrap();
        assert_eq!(finish.command.as_deref(), Some("r"));
        assert_eq!(finish.actual_result.as_deref(), Some("ok"));
        assert_eq!(
            finish.observed_receipt,
            Some(if attempt == 1 {
                crate::ast::Receipt::Unknown
            } else {
                crate::ast::Receipt::Delivered
            })
        );
        assert_eq!(finish.result.is_none(), attempt == 1);
        assert!(
            finish
                .events
                .iter()
                .any(|e| matches!(e,Event::Business(e) if e.kind==Kind::Commit))
        );
        assert!(finish.retry_location.is_some());
        assert!(finish.location.span.start < finish.location.span.end);
    }
    let explained = crate::explain::explain(src);
    let counterexample = explained
        .explanation
        .counterexamples
        .iter()
        .find(|c| c.id == "contract C #1 at_most_once")
        .unwrap();
    assert_eq!(
        counterexample.status,
        crate::explain::ReplayStatus::Verified
    );
    crate::reference::replay::replay(
        &m,
        &counterexample.steps,
        &counterexample.id,
        counterexample.case_id,
    )
    .unwrap();
    for fault in 0..5 {
        let mut poisoned = counterexample.steps.clone();
        let s = poisoned
            .iter_mut()
            .find(|s| s.attempt == Some(1) && s.kind == Kind::Finish)
            .unwrap();
        match fault {
            0 => s.attempt = Some(2),
            1 => s.observed_receipt = Some(crate::ast::Receipt::Delivered),
            2 => s.actual_result = Some("forged".into()),
            3 => s.command = Some("other_command".into()),
            _ => {
                s.values
                    .observed
                    .iter_mut()
                    .find(|v| v.name == "n")
                    .unwrap()
                    .value = "0".into()
            }
        }
        assert!(
            crate::reference::replay::replay(
                &m,
                &poisoned,
                &counterexample.id,
                counterexample.case_id
            )
            .is_err(),
            "fault {fault}"
        );
    }
}
