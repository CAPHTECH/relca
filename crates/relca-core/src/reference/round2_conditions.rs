use super::*;
const BASE: &str = r#"model M version "0.2" {
type V=enum {ok,no}
observed case {flag:bool=false}
actor A {capacity 1;capability c()->V {duration fixed 1min;result nondet {ok,no};requires flag==true else no}}
process P {r=A.c();end done}
bounds {cases 1;live_tasks 1;clock 10min}
verify reachable end.done
}"#;

#[test]
fn no_executor_does_not_evaluate_requires() {
    let mut m = crate::lower::lower(&crate::syntax::parse(BASE).unwrap()).unwrap();
    m.actors[0].executors = Some(vec![]);
    m.capabilities[0].requires.as_mut().unwrap().condition = Cond::StateEq(StateMatch {
        variable: StateVarId(9999),
        value: StateValue::None,
    });
    let mut i = replay::interpreter(&m);
    assert_eq!(i.run_inner(replay::initial(&m), 0), Ok(()));
}

#[test]
fn only_new_loop_zero_waits_are_ignored_by_the_trace_gate() {
    let source = r#"model M version "0.2" {
type V=enum {ok}
actor A {capacity 1;capability c()->V {duration fixed 1min;result nondet {ok}}}
process P {repeat work at_most 1 {wait 0min;r=A.c();leave}on_limit{end limit}end done}
bounds {cases 1;live_tasks 1;clock 10min}
verify reachable end.done
}"#;
    for (source, count) in [
        (source.to_owned(), 0),
        (
            source.replace(
                "repeat work at_most 1 {wait 0min;r=A.c();leave}on_limit{end limit}",
                "wait 0min;r=A.c();",
            ),
            1,
        ),
    ] {
        let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
        let mut i = replay::interpreter(&m);
        let mut f = replay::initial(&m);
        i.record(&mut f, Occurrence::Dispatch(CapabilityId(0)), None)
            .unwrap();
        assert_eq!(f.trace.len(), count);
    }
}
