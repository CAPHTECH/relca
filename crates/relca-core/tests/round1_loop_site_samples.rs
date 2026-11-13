use relca_core::{engine, lower, reference, syntax};
use serde_json::Value;

fn without_origin(mut v: Value) -> Value {
    match &mut v {
        Value::Object(fields) => {
            fields.remove("repetition");
            for x in fields.values_mut() {
                *x = without_origin(x.take());
            }
        }
        Value::Array(xs) => {
            for x in xs {
                *x = without_origin(x.take());
            }
        }
        _ => {}
    }
    v
}

#[test]
fn on_limit_provenance_does_not_change_the_execution_or_random_site_keys() {
    let source = r#"model Sites version "0.2" {
        type V=enum {a,b}
        kernel K {*->{0.5:a;0.5:b}}
        actor A {capacity 1;capability c()->V {duration fixed 1min;outcomes kernel K}}
        process P {repeat work at_most 2 {r=A.c();again}on_limit{last=A.c();leave}
            if last==a {end a}else{end b}}
        bounds {cases 1;live_tasks 1;clock 20min}
        metrics {window 20min;currency "JPY";rate A per_minute 1;rate A.c per_start 1}
        simulation replications 16 seed 7 confidence 0.5
        verify safety no_resource_overcommit
    }"#;
    let model = lower::lower(&syntax::parse(source).unwrap()).unwrap();
    let mut unannotated = model.clone();
    for b in &mut unannotated.process.bindings {
        b.repetition = None;
    }
    for (full, plain) in [
        (engine::simulate(&model), engine::simulate(&unannotated)),
        (
            reference::simulate(&model).outcome,
            reference::simulate(&unannotated).outcome,
        ),
    ] {
        assert_eq!(full.completed_replications, 16);
        // Same seed/input captured from f77c7cde; provenance is not a new RNG site.
        let terminals = full
            .replications
            .iter()
            .map(|p| {
                p.cases[0]
                    .terminal
                    .as_ref()
                    .unwrap()
                    .end
                    .as_deref()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            terminals,
            [
                "a", "b", "a", "a", "b", "a", "a", "a", "a", "b", "b", "a", "a", "a", "a", "b"
            ]
        );
        assert_eq!(full.truncated_replications, plain.truncated_replications);
        for p in &full.replications {
            assert!(
                p.draws.iter().any(|d| d.key.contains("@loop:work:2:last")),
                "{:?}",
                p.draws
            );
            let tasks = &p.metrics.as_ref().unwrap().cases[0].tasks;
            let task = tasks
                .iter()
                .find(|t| t.binding == "@loop:work:2:last")
                .unwrap();
            let origin = task.repetition.as_ref().unwrap();
            assert_eq!(origin.phase, relca_core::ir::LoopPhase::OnLimit);
            assert_eq!(origin.round, None);
        }
        assert_eq!(
            without_origin(serde_json::to_value(full).unwrap()),
            without_origin(serde_json::to_value(plain).unwrap())
        );
    }
}
