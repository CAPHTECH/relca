use super::*;

#[test]
fn legacy_condition_keeps_first_matching_task() {
    let m = crate::lower::lower(
        &crate::syntax::parse(
            r#"model Legacy version "0.2" {
        type V=enum {ok,bad}
        actor A {capacity 1;capability c()->V {duration fixed 1min;result nondet {ok,bad}}}
        process P {r=A.c();end done}
        bounds {cases 1;live_tasks 1;clock 10min}
        verify safety no_resource_overcommit
    }"#,
        )
        .unwrap(),
    )
    .unwrap();
    let interpreter = replay::interpreter(&m);
    let mut frame = replay::initial(&m);
    let Stmt::Call(call) = &m.process.body[0].kind else {
        panic!("call")
    };
    interpreter
        .create(&mut frame, std::slice::from_ref(call), None)
        .unwrap();
    frame.tasks[0].phase = Phase::Finished(ValueId(0));
    let mut second = frame.tasks[0].clone();
    second.phase = Phase::Finished(ValueId(1));
    frame.tasks.push(second);
    assert!(
        interpreter
            .condition(
                &frame,
                &Cond::Eq(ResultMatch {
                    binding: call.binding,
                    value: ValueId(0)
                }),
                0
            )
            .unwrap()
    );
}
