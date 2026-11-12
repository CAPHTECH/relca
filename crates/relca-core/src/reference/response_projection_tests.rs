//! Implementation-side equivalence tests, not an independent semantic oracle.
use super::*;

fn model(cases: usize, loss: &str, metrics: bool, deferred: bool) -> Model {
    let source = format!(
        r#"model M version "0.2" {{
        type Ack = enum {{ committed, duplicate }}
        actor L {{ capacity 1; state keys:set<CaseId>;
          capability post(key:CaseId)->Ack {{
            commits; idempotency key key; writes keys; duration fixed 1min;
            atomic {{ if key in keys {{result duplicate}} else {{keys+=key;result committed}} }}
          }}
        }}
        process P {{ r=L.post(case.id) response_loss {loss}
          retry max_attempts 2 when receipt unknown backoff fixed 1min exhausted end failed;
          end done }}
        bounds {{cases {cases};live_tasks 2;clock 8min}}
        arrivals fixed [{}]
        {}
        cut C {{ include [L] {} }}
        verify safety no_resource_overcommit
        verify reachable end.done
        }}"#,
        vec!["0min"; cases].join(","),
        if metrics {
            "metrics {window 8min;currency \"JPY\";rate L per_minute 3;rate L.post per_start 7}"
        } else {
            ""
        },
        if deferred {
            "defer Unknown reason \"unknown workload\" loses \"time and cost\""
        } else {
            ""
        },
    );
    crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap()
}

fn equivalent(m: &Model, limits: Limits) -> (ReferenceRun, ReferenceRun) {
    let full = explore_with_projection(m, limits, Projection::Full);
    let mut lean = explore_with_projection(m, limits, Projection::ResponseExercise);
    assert_eq!(full.stop, lean.stop);
    let mut expected = full.outcome.clone();
    expected.metrics = None;
    assert_eq!(expected, lean.outcome);
    // Includes frame/terminal/normal/cancellation counts, completion evidence,
    // obligations and all existing private counters. Both envelopes have been
    // consumed by report construction, so only the output-mode marker differs.
    lean.statistics.projection = Projection::Full;
    assert_eq!(
        format!("{:?}", full.statistics),
        format!("{:?}", lean.statistics)
    );
    (full, lean)
}

#[test]
fn response_projection_preserves_full_exploration_and_nonmetric_evidence() {
    for cases in [1, 2] {
        for loss in [
            "none",
            "nondet",
            "sequence [unknown,delivered]",
            "sequence [unknown,unknown]",
        ] {
            for metrics in [false, true] {
                for deferred in [false, true] {
                    let m = model(cases, loss, metrics, deferred);
                    let (full, lean) = equivalent(
                        &m,
                        Limits {
                            branch_depth: 512,
                            ..Limits::default()
                        },
                    );
                    assert_eq!(full.stop, StopReason::Complete);
                    assert_eq!(
                        full.outcome.response_exercise,
                        lean.outcome.response_exercise
                    );
                    assert_eq!(full.outcome.metrics.is_some(), metrics);
                    assert!(lean.outcome.metrics.is_none());
                }
            }
        }
    }
}

#[test]
fn response_projection_preserves_stops_and_validation_without_lowering_limits() {
    let mut m = model(2, "nondet", true, false);
    for limits in [
        Limits {
            frames: 0,
            ..Limits::default()
        },
        Limits {
            frames: 5,
            ..Limits::default()
        },
        Limits {
            branch_depth: 1,
            ..Limits::default()
        },
        Limits {
            steps_per_path: 1,
            ..Limits::default()
        },
        Limits {
            tasks_per_path: 1,
            ..Limits::default()
        },
    ] {
        let (full, _) = equivalent(&m, limits);
        assert_ne!(full.stop, StopReason::Complete, "{limits:?}");
    }
    m.bounds.clock = 1;
    let (full, _) = equivalent(&m, Limits::default());
    assert!(full.outcome.truncated);
    // A forged metric declaration must still be rejected, even though this
    // private projection does not collect the resulting metric table.
    m.metrics.as_mut().unwrap().window = 0;
    let (full, _) = equivalent(&m, Limits::default());
    assert_ne!(full.stop, StopReason::Complete);
}

#[test]
fn agency_projection_calls_one_independent_enumerator() {
    let m = model(1, "sequence [unknown,delivered]", true, false);
    let (run, calls) = crate::evaluation_spy::observe(|| explore_response_exercise(&m));
    assert_eq!(calls, [0, 0, 1, 0, 0, 0, 0]);
    assert_eq!(run.stop, StopReason::Complete);
    assert_eq!(run.outcome.response_exercise.as_ref().unwrap()[0].min, 2);
    assert_eq!(run.outcome.response_exercise.as_ref().unwrap()[0].max, 2);
}

#[test]
fn public_explore_keeps_full_metric_projection() {
    let m = model(1, "sequence [unknown,delivered]", true, false);
    let run = explore(&m);
    assert_eq!(run.stop, StopReason::Complete);
    assert!(run.outcome.metrics.is_some(), "public explore lost metrics");
    assert_eq!(run.statistics.projection, Projection::Full);
    let full = explore_projected(&m, Projection::Full);
    assert_eq!(run.outcome, full.outcome);
    assert_eq!(
        format!("{:?}", run.statistics),
        format!("{:?}", full.statistics)
    );
}

#[test]
fn public_explore_with_limits_keeps_full_metric_projection() {
    let m = model(2, "sequence [unknown,delivered]", true, false);
    let limits = Limits {
        branch_depth: 512,
        ..Limits::default()
    };
    let run = explore_with_limits(&m, limits);
    assert_eq!(run.stop, StopReason::Complete);
    assert!(
        run.outcome.metrics.is_some(),
        "public explore_with_limits lost metrics"
    );
    assert_eq!(run.statistics.projection, Projection::Full);
    let full = explore_with_projection(&m, limits, Projection::Full);
    assert_eq!(run.outcome, full.outcome);
    assert_eq!(
        format!("{:?}", run.statistics),
        format!("{:?}", full.statistics)
    );
}
