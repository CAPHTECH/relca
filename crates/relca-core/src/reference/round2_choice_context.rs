use super::*;

#[test]
fn an_unfinished_group_keeps_its_prefix_leaf_and_original_abort_reason() {
    let source = r#"model Group version "0.2" {type T=enum {ok}
    latent case {coin:bool ~ {true:0.5,false:0.5}}
    actor A {capacity 1;capability c()->T {duration fixed 1min;result nondet {ok}}}
    process P {r=A.c();end done}bounds {cases 1;live_tasks 1;clock 5min}verify reachable end.done}"#;
    let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
    let mut i = replay::interpreter(&m);
    let f = replay::initial(&m);
    let mut work = Work::default();
    let root = work.slot(None);
    i.run_work = Some(work);
    i.probability_stack = vec![ReferenceChoices::default(), ReferenceChoices::default()];
    i.probability_leaf(&f, None, ReferenceCut::Range).unwrap();
    i.finish_activation(root, 0, Err(StopReason::EnumerationLimit))
        .unwrap();
    assert!(matches!(
        i.drain_paths(root),
        Err(StopReason::EnumerationLimit)
    ));
    assert!(
        i.statistics.completion_unknown,
        "the preceding leaf was skipped"
    );
}
#[test]
fn leaf_side_effects_wait_for_the_source_position_inside_a_probability_group() {
    let source = r#"model Group version "0.2" {type T=enum {ok}
    observed case {hit:bool=false}latent case {coin:bool ~ {true:0.5,false:0.5}}
    actor A {capacity 1;capability c()->T {duration fixed 1min;result nondet {ok}}}
    process P {r=A.c();end done}bounds {cases 1;live_tasks 1;clock 5min}
    verify probability reached hit==true <= 0.5}"#;
    let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
    let mut i = replay::interpreter(&m);
    let f = replay::initial(&m);
    let mut work = Work::default();
    let root = work.slot(None);
    i.run_work = Some(work);
    i.probability_stack = vec![ReferenceChoices::default(), ReferenceChoices::default()];
    i.probability_leaf(&f, None, ReferenceCut::Range).unwrap();
    assert!(
        !i.statistics.completion_unknown,
        "a producer leaf mutated statistics before its turn"
    );
    i.probability_finish(None);
    let count = m.probability_queries.len() + m.ends.len();
    i.probability_stack.last_mut().unwrap().children.push(
        (0..count)
            .map(|_| ReferenceBound {
                values: std::array::from_fn(|_| Rational::zero()),
                numeric: false,
            })
            .collect(),
    );
    i.finish_activation(root, 0, Ok(())).unwrap();
    let values = i.drain_paths(root).unwrap().unwrap();
    assert!(i.statistics.completion_unknown);
    assert_eq!(
        values[0].values,
        [
            Rational::zero(),
            Rational::zero(),
            Rational::zero(),
            Rational::one()
        ]
    );
}
