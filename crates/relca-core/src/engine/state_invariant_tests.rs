//! Implementation-side checks of the generation seam, separate from the hand oracles.
use super::*;

fn model(src: &str) -> ir::Model {
    crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap()
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}

fn assert_normalized(s: &State, m: &ir::Model) {
    if !s.initialized || s.exceeds_live_bound(m.bounds.live_tasks) {
        return;
    }
    let mut normalized = s.clone();
    update_state_invariants(&mut normalized, m, 0).unwrap();
    assert_eq!(s, &normalized, "normalization must precede key insertion");
}

#[test]
fn state_invariant_generated_handoffs_are_normalized_before_hashing_and_capture() {
    use crate::explain::Kind;
    let mut sources: Vec<_> = [
        "sinv-initial.broken-initial",
        "sinv-start-effect.broken-order",
        "sinv-transient.broken-same-time",
        "sinv-cancel",
        "sinv-range.broken-twice",
        "sinv-mutation",
        "sinv-seed.broken-nondet",
        "sinv-seed-mass",
    ]
    .into_iter()
    .map(fixture)
    .collect();
    let cancel = fixture("sinv-cancel");
    sources.push(cancel.replace("cooperative ack_within 1min", "immediate"));
    let initial = fixture("sinv-initial");
    sources.push(initial.replace("r = Fix.fix()", "wait 1min; r = Fix.fix()"));
    sources.push(
        fixture("ctx-contest")
            .replace(
                "  people",
                "  observed case { probe: bool = true }\n  people",
            )
            .replace(
                "  bounds",
                "  contract StateProbe { invariant probe }\n  bounds",
            ),
    );
    let mut kinds = BTreeSet::new();
    let mut events = BTreeSet::new();
    for src in sources {
        let m = model(&src);
        if m.uses_probability() {
            let mut e = Exploration::default();
            let (nodes, _) = probability_graph(&m, 10000, &mut e).unwrap();
            for n in nodes {
                assert_normalized(&n.state, &m);
            }
            continue;
        }
        let mut program = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut program);
        let mut seed = State::initial(&m, entry);
        let mut e = Exploration::default();
        if seed.initialized {
            emit(&mut seed, Bundle::Received, &m, &mut e).unwrap();
        }
        let mut capture = trace::Capture::new(&src, &m);
        let mut pending = vec![seed];
        let mut seen = HashSet::new();
        while let Some(s) = pending.pop() {
            assert_normalized(&s, &m);
            if !seen.insert(s.clone()) {
                continue;
            }
            match step_observed(s.clone(), &program, &m, &mut Room(10000), &mut e).unwrap() {
                Step::Next(xs) => {
                    for x in xs {
                        assert_normalized(&x, &m);
                        let stage = capture.classify(&s, &x, &m).unwrap();
                        kinds.insert(format!("{:?}", stage.kind));
                        for event in stage.events {
                            if let crate::explain::Event::Business(e) = event {
                                kinds.insert(format!("{:?}", e.kind));
                            }
                        }
                        pending.push(x);
                    }
                }
                Step::Terminal(end) => {
                    if s.contest.as_ref().is_none_or(|c| c.business.is_none()) {
                        let mut closed = s.clone();
                        emit(&mut closed, Bundle::End(end), &m, &mut e).unwrap();
                        assert_normalized(&closed, &m);
                    }
                    capture.leaf(&s, &m, Kind::End);
                    assert!(capture.failure.is_none());
                    kinds.insert(format!("{:?}", capture.edges[0].last().unwrap().stage.kind));
                }
                _ => {}
            }
        }
        for (_, bundle, _) in e.state_events {
            events.insert(match bundle {
                Bundle::Received => "receipt",
                Bundle::Admitted => "admitted",
                Bundle::Start(..) => "start",
                Bundle::Finish(..) => "finish",
                Bundle::CancelAck(..) => "ack",
                Bundle::End(..) => "end",
            });
        }
    }
    assert!(BTreeSet::from(["receipt", "start", "finish", "end"]).is_subset(&events));
    for kind in [
        Kind::Initialize,
        Kind::CancelRequest,
        Kind::CancelAck,
        Kind::Advance,
        Kind::Wait,
        Kind::Wake,
        Kind::ContestFiled,
        Kind::ContestDeclined,
        Kind::ContestQueue,
        Kind::ContestResponse,
        Kind::ExecutionClose,
        Kind::Commit,
    ] {
        assert!(
            kinds.contains(&format!("{kind:?}")),
            "missing transition {kind:?}: {kinds:?}"
        );
    }
}

#[test]
fn state_invariant_received_then_bad_budget_keeps_witness_and_zero_never_creates_one() {
    let src = fixture("sinv-sticky.broken-bad");
    let m = model(&src);
    let id = m.contracts[0].property_id(ir::ClauseId(0));
    let (zero, capture) = trace::explore_with_budget(&src, &m, 0);
    assert!(zero.truncated && capture.shortest(&id).is_none());
    let mut measured = None;
    for budget in 1..100 {
        let (o, capture) = trace::explore_with_budget(&src, &m, budget);
        if o.truncated && o.properties[0].verdict == Verdict::Violated {
            assert!(o.diagnostics.iter().any(|d| d.code == diag::W_BUDGET_0));
            let (steps, case, choices) = capture.shortest(&id).unwrap();
            assert!(steps.last().is_some_and(|s| s.time == 1));
            // Capture's raw path has no final DTO yet: the public explain and
            // private replay suites separately exercise enriched replay.
            assert_eq!(case, Some(1));
            assert_eq!(choices.len(), steps.len());
            measured = Some(budget);
            break;
        }
    }
    println!("received_then_bad_budget={measured:?}");
    assert!(
        measured.is_some(),
        "no retained in-bound violation across budget cuts"
    );
}

#[test]
fn state_invariant_capture_limit_remains_200000_and_retracts_partial_paths() {
    let src = fixture("sinv-approval.broken-bypass");
    let m = model(&src);
    let mut capture = trace::Capture::new(&src, &m);
    assert_eq!(capture.capture_limit, 200_000);
    capture.capture_limit = 2;
    let (o, capture) = trace::explore_captured(&m, 10000, capture);
    assert_eq!(o.properties[0].verdict, Verdict::Violated);
    assert_eq!(
        capture.failure,
        Some(crate::explain::ReplayReason::CaptureLimit)
    );
    assert!(
        capture
            .shortest(&m.contracts[0].property_id(ir::ClauseId(0)))
            .is_none()
    );
}

#[test]
fn state_invariant_converging_business_states_retain_distinct_reachable_histories() {
    let src = r#"model M version "0.2" {
      type V=enum{pass,fail}
      observed case{a:bool=false;b:bool=false}
      actor A{capacity 2; capability bad()->V{duration fixed 1min;result nondet{pass};writes a;finish{a=true}}
        capability good()->V{duration fixed 1min;result nondet{pass};writes b;finish{b=true}}}
      process P{fork f{x=A.bad();y=A.good()} join all f; z=A.good();end done}
      cut C{include[A]} bounds{cases 1;live_tasks 2;clock 4min}
      contract C{invariant a -> b}
    }"#;
    let m = model(src);
    let o = explore(&m);
    assert_eq!(o.properties[0].verdict, Verdict::Violated);
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    let mut seed = State::initial(&m, entry);
    let mut e = Exploration::default();
    emit(&mut seed, Bundle::Received, &m, &mut e).unwrap();
    let mut pending = vec![seed];
    let mut seen = HashSet::new();
    let mut convergence = BTreeMap::<String, BTreeSet<bool>>::new();
    while let Some(s) = pending.pop() {
        if !seen.insert(s.clone()) {
            continue;
        }
        let mut business = s.clone();
        let violated = business.monitors[0].violated;
        business.monitors.clear();
        convergence
            .entry(format!("{business:?}"))
            .or_default()
            .insert(violated);
        if let Step::Next(xs) = step_observed(s, &program, &m, &mut Room(10000), &mut e).unwrap() {
            pending.extend(xs);
        }
    }
    assert!(
        convergence.values().any(|histories| histories.len() == 2),
        "same business state must retain safe and violated reachable histories"
    );
    assert_eq!(o.terminals.len(), 1);
    assert_eq!(o.terminals[0].work.min, 3);
}

fn visited(m: &ir::Model) -> usize {
    assert!(m.legacy_route() && !m.uses_probability());
    let mut e = Exploration::default();
    explore_into(
        m,
        true,
        DEFAULT_STATE_BUDGET,
        std::convert::identity,
        &mut e,
    )
    .unwrap();
    e.census.0
}

#[test]
fn state_invariant_monitor_state_growth_matches_hand_census() {
    // Node rules: receipt, queue, each start, each time advance, each finish,
    // the join that hands results to the process and each `if` step are nodes;
    // reaching `end` records a terminal and adds no node.
    // sinv-approval: receipt, queue j, start j, advance to 2 (4). finish j pass
    // or fail (2), join each (2). fail: if -> end returned (1). pass: if -> call p,
    // queue p, start p, advance to 3, finish p (registered=true activates the
    // clause), join (6). 4 + 2 + 2 + 1 + 6 = 15. Activation happens on one path
    // only, so the monitor bits split no state and a reachability request in
    // place of the clause (one request is required) keeps 15.
    let src = fixture("sinv-approval");
    let without = src.replace(
        "contract Controls { invariant registered -> approval == pass }",
        "verify reachable end.posted",
    );
    assert_ne!(src, without);
    assert_eq!(visited(&model(&src)), 15);
    assert_eq!(visited(&model(&without)), 15);
    // sinv-transient.broken-same-time: receipt, fork queue, start a then b or
    // b then a (2), both running, advance to 3 (6). The same-time finishes
    // interleave: po first exposes (pass, none) and sets violated, receipt first
    // does not (2). After the second finish the business states coincide but the
    // sticky violated bit differs (2), and so do the two joins (2): 12. Without
    // the clause those two pairs merge: 6 + 2 + 1 + 1 = 10.
    let src = fixture("sinv-transient.broken-same-time");
    let without = src.replace(
        "contract Controls { invariant po == pass -> receipt == pass }",
        "verify reachable end.done",
    );
    assert_ne!(src, without);
    assert_eq!(visited(&model(&src)), 12);
    assert_eq!(visited(&model(&without)), 10);
}
