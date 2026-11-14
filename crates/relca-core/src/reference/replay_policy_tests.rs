//! Direct illegal Stage probes: successful ordinary explain is not a substitute.
use super::*;
fn model(src: &str) -> Model {
    crate::syntax::parse(src)
        .and_then(|a| crate::lower::lower(&a))
        .unwrap()
}
fn source(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
fn frame(m: &Model) -> Frame<'_> {
    let mut f = local::initial(m);
    let i = local::interpreter(m);
    match &m.process.body[0].kind {
        Stmt::Call(call) => i.create(&mut f, std::slice::from_ref(call), None).unwrap(),
        Stmt::Fork { branches, join, .. } => i.create(&mut f, branches, Some(join)).unwrap(),
        _ => panic!(),
    }
    f.control = vec![&m.process.body[1..]];
    f
}
fn queued(frames: &[Option<Frame<'_>>]) -> Vec<BTreeMap<BindingId, Tick>> {
    frames
        .iter()
        .map(|f| {
            f.as_ref()
                .map(|f| f.tasks.iter().map(|t| (t.call.binding, 0)).collect())
                .unwrap_or_default()
        })
        .collect()
}
fn stage(binding: &str, executor: Option<&str>, kind: Kind) -> Stage {
    Stage {
        source: None,
        evidence_sources: None,
        saved_key: None,
        evidence_ref: None,
        command: None,
        attempt: None,
        observed_receipt: None,
        actual_result: None,
        retry_location: None,
        index: 0,
        time: 0,
        case_id: Some(1),
        binding: Some(binding.into()),
        actor: None,
        capability: None,
        executor: executor.map(str::to_owned),
        result: None,
        kind,
        events: Vec::new(),
        location: x::Location::new("", diag::Span { start: 0, end: 0 }).unwrap(),
        values: Default::default(),
        violation: None,
        resource: None,
        capability_location: None,
        deadlock_detail: None,
        shared_resources: Vec::new(),
        state_range_violation: None,
    }
}

#[test]
fn policy_replay_wrong_first_executor_and_y_first_are_illegal_not_mismatched_models() {
    let m = model(&source("pol-separation"));
    let frames = vec![Some(frame(&m))];
    let queue = queued(&frames);
    let binding = &m.process.bindings[0].name;
    assert_eq!(
        legal(
            global_start(
                &m,
                &frames,
                &queue,
                0,
                &stage(binding, Some("c1"), Kind::Start)
            )
            .unwrap()
        ),
        Ok(())
    );
    assert_eq!(
        legal(
            global_start(
                &m,
                &frames,
                &queue,
                0,
                &stage(binding, Some("c2"), Kind::Start)
            )
            .unwrap()
        ),
        Err(ReplayReason::IllegalTransition)
    );
    let mut open = m.clone();
    open.start_policy = Some(StartPolicy::default());
    assert_eq!(
        legal(
            global_start(
                &open,
                &frames,
                &queue,
                0,
                &stage(binding, Some("c2"), Kind::Start)
            )
            .unwrap()
        ),
        Ok(()),
        "adversarial Stage is otherwise legal"
    );
    for (name, binding) in [("pol-tie-actor", "b"), ("pol-resource-hol", "b")] {
        let m = model(&source(name));
        let frames = vec![Some(frame(&m))];
        let queue = queued(&frames);
        let bad = stage(binding, None, Kind::Start);
        assert_eq!(
            legal(global_start(&m, &frames, &queue, 0, &bad).unwrap()),
            Err(ReplayReason::IllegalTransition),
            "{name}"
        );
        let mut open = m.clone();
        open.start_policy = Some(StartPolicy::default());
        assert_eq!(
            legal(global_start(&open, &frames, &queue, 0, &bad).unwrap()),
            Ok(()),
            "{name}"
        );
        assert!(
            global_instant(
                &m,
                &frames,
                &[local::interpreter(&m)],
                &[false],
                &queue,
                &[true]
            )
            .unwrap()
        );
    }
}

#[test]
fn policy_replay_refuse_counts_as_minimum_and_requires_an_idle_person() {
    let src = r#"model M version "0.2" {type R=enum {ok,rejected} people [p] actor Ledger {executors [p];state posted:0..1=1;capability post()->R {requires posted==0 else rejected;duration fixed 1min;result nondet {ok}}} actor Tail {executors [p];capability work()->R {duration fixed 2min;result nondet {ok}}} process P {fork f {a=Ledger.post();b=Tail.work()};join all f;end done} bounds {cases 1;live_tasks 2;clock 5min} policy fifo tie_by case_id then call_order assign first_idle verify reachable end.done}"#;
    let m = model(src);
    let mut frames = vec![Some(frame(&m))];
    let queue = queued(&frames);
    let refuse = stage("a", None, Kind::Refuse);
    assert_eq!(
        legal(global_start(&m, &frames, &queue, 0, &refuse).unwrap()),
        Ok(())
    );
    assert_eq!(
        legal(global_start(&m, &frames, &queue, 0, &stage("b", Some("p"), Kind::Start)).unwrap()),
        Err(ReplayReason::IllegalTransition)
    );
    assert!(
        global_instant(
            &m,
            &frames,
            &[local::interpreter(&m)],
            &[false],
            &queue,
            &[true]
        )
        .unwrap()
    );
    let f = frames[0].as_mut().unwrap();
    f.tasks[1].phase = Phase::Running;
    f.tasks[1].person = Some(PersonId(0));
    f.tasks[1].start = Some(0);
    f.tasks[1].finish = Some(2);
    assert_eq!(
        legal(global_start(&m, &frames, &queue, 0, &refuse).unwrap()),
        Err(ReplayReason::IllegalTransition)
    );
    assert!(
        !global_instant(
            &m,
            &frames,
            &[local::interpreter(&m)],
            &[false],
            &queue,
            &[true]
        )
        .unwrap(),
        "Refuse cannot use a person held by another Actor"
    );
}

#[test]
fn policy_contest_suffix_ordinal_and_first_idle_are_replayed_without_binding_indexing() {
    let src = r#"model M version "0.2" {type T=enum {ok} people [p,q] actor Work {capacity 1;capability run()->T {duration fixed 1min;result nondet {ok}}} actor Route {CONTROL executors [p,q];capability respond()->T {duration fixed 1min;result nondet {ok}}} actor Party {control party;standing over [end.done] basis "claim"} process P {a=Work.run();end done} contract C {contest Party on end.done route Route.respond reversible_by [p] respond_by 4min;contest Party on end.done route Route.respond reversible_by [p] respond_by 4min} bounds {cases 1;live_tasks 2;clock 5min} policy fifo tie_by case_id then call_order assign first_idle verify reachable end.done}"#;
    for explicit in [false, true] {
        let m = model(&src.replace("CONTROL", if explicit { "control policy;" } else { "" }));
        assert_eq!(m.contests().len(), 2);
        let mut f = local::initial(&m);
        let calls: Vec<_> = m
            .contests()
            .iter()
            .enumerate()
            .map(|(i, c)| Call {
                args: Vec::new(),
                binding: m.contest_site(i),
                capability: c.route,
                span: m.process.span,
            })
            .collect();
        local::interpreter(&m).create(&mut f, &calls, None).unwrap();
        f.business = Some((EndId(0), 0));
        let frames = vec![Some(f)];
        let queue = queued(&frames);
        assert_eq!(m.site_name(calls[0].binding), "contest:1");
        assert!(
            global_start(
                &m,
                &frames,
                &queue,
                0,
                &stage("contest:1", Some("p"), Kind::Start)
            )
            .unwrap()
        );
        assert_eq!(
            global_start(
                &m,
                &frames,
                &queue,
                0,
                &stage("contest:1", Some("q"), Kind::Start)
            )
            .unwrap(),
            !explicit
        );
        assert_eq!(
            legal(
                global_start(
                    &m,
                    &frames,
                    &queue,
                    0,
                    &stage("contest:2", Some("p"), Kind::Start)
                )
                .unwrap()
            ),
            if explicit {
                Err(ReplayReason::IllegalTransition)
            } else {
                Ok(())
            }
        );
        let views: Vec<_> = frames.iter().map(Option::as_ref).collect();
        assert_eq!(
            super::super::policy::starts(&m, &views, &queue)
                .unwrap()
                .values()
                .map(Vec::len)
                .sum::<usize>(),
            if explicit { 1 } else { 4 }
        );
    }
}
