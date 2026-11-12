//! Independent private arrival-product, signature, provenance and replay checks.
use super::*;

fn model(src: &str) -> Model {
    crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap()
}
fn setup(m: &Model) -> (Tree<'_>, World<'_>) {
    let limits = Limits::default();
    let times = m.arrivals.clone().unwrap();
    let n = times.len();
    let tree = Tree {
        response_exercise: BTreeMap::new(),
        response_receipts: Vec::new(),
        first_hits: Vec::new(),
        first_closed: Vec::new(),
        first_trees: Vec::new(),
        resource_worlds: Some(Vec::new()),
        depth: 0,
        m,
        limits,
        times,
        cases: (0..n).map(|_| interpreter(m, limits)).collect(),
        terminals: vec![BTreeMap::new(); n],
        unresolved: vec![false; n],
        stats: Statistics::default(),
        truncated: false,
        numeric: false,
        deadlock: false,
        overcommit: false,
        chance: false,
        scheduler: false,
        policy_unresolved: false,
        rejects: None,
        overflows: None,
        external_peaks: None,
        admissions: vec![None; n],
        unadmitted: vec![false; n],
        initial_leaves: Vec::new(),
        receipts: Vec::new(),
        duration_leaves: Vec::new(),
    };
    let world = World {
        external_peak: 0,
        now: 0,
        next: 0,
        boundary: Boundary::Coins,
        steps: 0,
        shared: m
            .state_variables
            .iter()
            .map(|v| match v.initial {
                InitialValue::Fixed(x) if shared_state(m, v.kind) => x,
                _ => StateValue::None,
            })
            .collect(),
        cases: vec![CaseSlot::NotHere; n],
    };
    (tree, world)
}

#[test]
fn b02_reference_initial_product_has_eight_arrival_tuples_and_held_receipts() {
    let src = include_str!("../../../../fixtures/lim-concurrent-wait/model.relca").replace(
        "  observed case",
        "  latent case { flag:bool ~ {true:0.5,false:0.5} }\n  observed case",
    );
    let m = model(&src);
    let (mut tree, world) = setup(&m);
    tree.walk(world).unwrap();
    let leaves: Vec<_> = tree
        .initial_leaves
        .iter()
        .filter(|(at, states, _)| *at == 0 && states.len() == 3)
        .collect();
    assert_eq!(leaves.len(), 8);
    let tuples: std::collections::HashSet<_> =
        leaves.iter().map(|(_, states, _)| states.clone()).collect();
    assert_eq!(tuples.len(), 8);
    assert!(
        leaves
            .iter()
            .all(|(_, _, p)| *p == Rational::decimal("0.125").unwrap())
    );
    assert!(
        tree.initial_leaves
            .iter()
            .filter(|(at, _, _)| *at > 0)
            .all(|(_, states, p)| states.is_empty() && *p == Rational::one())
    );
    let worlds = tree.resource_worlds.unwrap();
    assert!(worlds.iter().any(|w| matches!(&w.cases[2],CaseSlot::Held(c) if c.receipt && c.frame.tasks.is_empty() && c.frame.trace.iter().any(|o| matches!(o.event,Occurrence::Receipt) && o.at==0))));
    assert!(
        worlds
            .iter()
            .all(|w| w.cases.iter().all(|s| s.payload().is_none_or(|c| c
                .frame
                .trace
                .iter()
                .filter(|o| matches!(o.event, Occurrence::Receipt))
                .count()
                <= 1)))
    );
}

#[test]
fn b03_reference_signature_distinguishes_slots_admission_times_peaks_and_receipt() {
    let m = model(include_str!(
        "../../../../fixtures/lim-concurrent-wait/model.relca"
    ));
    let (mut tree, w) = setup(&m);
    tree.walk(w).unwrap();
    let worlds = tree.resource_worlds.as_ref().unwrap();
    let base = worlds
        .iter()
        .find(|w| matches!(&w.cases[1],CaseSlot::Held(c) if c.receipt))
        .unwrap();
    let old = signature(&m, base).unwrap();
    let mut changed = base.clone();
    let c = changed.cases[1].payload().unwrap().clone();
    changed.cases[1] = CaseSlot::Here(c.clone());
    assert_ne!(old, signature(&m, &changed).unwrap());
    changed.cases[1] = CaseSlot::Overflow(c.clone());
    assert_ne!(old, signature(&m, &changed).unwrap());
    changed = base.clone();
    changed.external_peak += 1;
    assert_ne!(old, signature(&m, &changed).unwrap());
    changed = base.clone();
    changed.cases[1].payload_mut().unwrap().receipt = false;
    assert_ne!(old, signature(&m, &changed).unwrap());
    changed = base.clone();
    append(&tree, &mut changed, 1, Occurrence::Admission, None);
    let admitted = signature(&m, &changed).unwrap();
    assert_ne!(old, admitted);
    changed.cases[1]
        .payload_mut()
        .unwrap()
        .frame
        .trace
        .last_mut()
        .unwrap()
        .at = 1;
    assert_ne!(admitted, signature(&m, &changed).unwrap());
}

#[test]
fn b08_reference_admission_event_deadline_addition_refuses_overflow() {
    let m = model(include_str!(
        "../../../../fixtures/lim-admitted-deadline/model.relca"
    ));
    let (mut tree, mut w) = setup(&m);
    w.now = u64::MAX;
    w.cases[0] = CaseSlot::Here(tree.new_case(u64::MAX));
    assert_eq!(
        tree.record(&mut w, 0, Occurrence::Admission, None, false),
        Err(StopReason::SemanticAbort)
    );
}

#[test]
fn b12_independent_replay_rejects_fifo_time_and_receipt_forgeries() {
    let src = include_str!("../../../../fixtures/lim-readmit/model.relca");
    let m = model(src);
    let explained = crate::explain::explain(src);
    let c = explained.explanation.counterexamples.first().unwrap();
    assert_eq!(c.replay.status, crate::explain::ReplayStatus::Verified);
    let choices = vec![None; c.steps.len()];
    let run = |steps: &[crate::explain::Stage]| {
        crate::reference::arrivals_replay::replay(&m, steps, &c.id, c.case_id, &choices)
    };
    assert!(run(&c.steps).is_ok());
    let mut forged = c.steps.clone();
    let receive = forged
        .iter_mut()
        .find(|s| s.kind == crate::explain::Kind::Receive)
        .unwrap();
    receive.events.remove(0);
    assert!(
        run(&forged).is_err(),
        "receipt flag cannot be fabricated by skipping the event"
    );
    let mut forged = c.steps.clone();
    let stage = forged
        .iter_mut()
        .find(|s| s.kind == crate::explain::Kind::Admitted && s.time == 2)
        .unwrap();
    stage.time = 1;
    assert!(
        run(&forged).is_err(),
        "admitted_at cannot precede available room"
    );
    let mut forged = c.steps.clone();
    let event = forged
        .iter_mut()
        .filter(|s| s.kind == crate::explain::Kind::Admitted)
        .flat_map(|s| &mut s.events)
        .find_map(|e| match e {
            crate::explain::Event::Business(e) if e.case_id == Some(2) => Some(e),
            _ => None,
        })
        .unwrap();
    event.case_id = Some(3);
    assert!(run(&forged).is_err(), "FIFO cannot be swapped");
    let mut forged = c.steps.clone();
    let value = forged
        .iter_mut()
        .filter(|s| s.kind == crate::explain::Kind::Initialize)
        .flat_map(|s| &mut s.events)
        .find_map(|e| match e {
            crate::explain::Event::Value(v) if v.name == "hit" => Some(v),
            _ => None,
        })
        .unwrap();
    value.value = "true".into();
    assert!(
        run(&forged).is_err(),
        "fixed arrival state cannot be replaced on readmission"
    );
}

#[test]
fn b15_overflow_activation_cannot_be_skipped_for_a_later_deadline_witness() {
    let src = include_str!("../../../../fixtures/lim-overflow.broken-hidden/model.relca")
        .replace("queue capacity 1", "queue capacity 0");
    let m = model(&src);
    let explained = crate::explain::explain(&src);
    let c = explained.explanation.counterexamples.first().unwrap();
    assert_eq!(c.replay.status, crate::explain::ReplayStatus::Verified);
    let mut forged = c.steps.clone();
    let last = forged.last_mut().unwrap();
    assert_eq!(last.kind, crate::explain::Kind::Admitted);
    last.kind = crate::explain::Kind::Deadline;
    last.time = m.bounds.clock;
    last.events.clear();
    let choices = vec![None; forged.len()];
    assert!(
        crate::reference::arrivals_replay::replay(&m, &forged, &c.id, c.case_id, &choices).is_err(),
        "overflow is immediate: skipping activation is not a legal deadline-only prefix"
    );
}
