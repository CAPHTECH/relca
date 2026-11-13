use super::super::arrivals_tests::{tree, world};
use super::*;

fn model() -> Model {
    crate::lower::lower(
        &crate::syntax::parse(include_str!(
            "../../../../../fixtures/qtl-first-hit/model.relca"
        ))
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn siblings_share_one_immutable_source_world_snapshot() {
    let m = model();
    let w = world(&m, 1);
    let mut t = tree(&m, Limits::default(), vec![0]);
    t.walk_work = Some(Work::default());
    for _ in 0..2 {
        t.defer_world(&w, |this, w| this.leaf(w, ReferenceCut::Range))
            .unwrap();
    }
    let jobs = &t.walk_work.as_ref().unwrap().jobs;
    let [Job::Action { base: a, .. }, Job::Action { base: b, .. }] = jobs.as_slice() else {
        panic!("deferred worlds")
    };
    assert!(
        Rc::ptr_eq(a, b),
        "siblings cloned the source World independently"
    );
}

#[test]
fn cancellation_siblings_do_not_materialize_worlds_before_visiting_them() {
    let source = include_str!("../../../../../fixtures/coop-cancel/model.relca")
        .replace("ack_within 1min", "ack_within 128min");
    let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    let mut w = world(&m, 2);
    let mut t = tree(&m, Limits::default(), vec![0, 0]);
    let Stmt::Fork { branches, join, .. } = &m.process.body[0].kind else {
        panic!("fork")
    };
    let f = &mut w.cases[0].payload_mut().unwrap().frame;
    f.control.clear();
    t.cases[0].create(f, branches, Some(join)).unwrap();
    f.tasks[0].phase = Phase::Finished(ValueId(0));
    f.tasks[0].start = Some(0);
    f.tasks[0].stop = Some(3);
    f.tasks[1].phase = Phase::Running;
    f.tasks[1].start = Some(0);
    f.tasks[1].finish = Some(5);
    w.now = 3;
    t.walk_work = Some(Work::default());
    t.cancel_choices(&w, 0, branches[0].binding).unwrap();
    let eager = t
        .walk_work
        .as_ref()
        .unwrap()
        .jobs
        .iter()
        .filter(|j| matches!(j, Job::World { .. }))
        .count();
    assert_eq!(
        eager, 0,
        "cancellation eagerly cloned {eager} sibling Worlds"
    );
    assert!(
        t.walk_work.as_ref().unwrap().jobs.len() <= 4,
        "unvisited cancellation siblings retained per-width jobs"
    );
}

#[test]
fn immediate_range_leaf_is_after_the_prior_deferred_sibling_in_first_trees() {
    let m = model();
    let w = world(&m, 1);
    let mut t = tree(&m, Limits::default(), vec![0]);
    t.first_hits = vec![vec![None; m.quality.as_ref().unwrap().properties.len()]];
    t.first_closed = vec![vec![false; m.quality.as_ref().unwrap().properties.len()]];
    t.walk_work = Some(Work::default());
    let Branch::Deferred(root) = t
        .defer_world(&w, |this, w| {
            let hit = this.defer_world(w, |this, w| {
                let mut w = w.clone();
                let yes = this.m.types[this.m.state_variables[0].base.0]
                    .members
                    .iter()
                    .position(|v| v == "true")
                    .unwrap();
                w.cases[0].payload_mut().unwrap().frame.state_values[0] =
                    StateValue::Value(ValueId(yes));
                this.leaf(&w, ReferenceCut::Range)
            })?;
            let range = this.leaf(w, ReferenceCut::Range)?;
            Ok(this.reduce(
                vec![(Rational::one(), hit), (Rational::one(), range)],
                false,
            ))
        })
        .unwrap()
    else {
        panic!("deferred root")
    };
    t.drain_worlds(root).unwrap();
    let tree = t.first_trees.last().unwrap();
    assert!(!tree.random);
    assert_eq!(tree.children.len(), 2);
    assert_eq!(tree.children[0].1.views[0][0].hit, Some(0));
    assert_eq!(tree.children[1].1.views[0][0].hit, None);

    // The native producer order from 8c3765f4: completely visit the first
    // subtree, restore its path-local observations, then form the Range leaf.
    let mut native = super::super::arrivals_tests::tree(&m, Limits::default(), vec![0]);
    let count = m.quality.as_ref().unwrap().properties.len();
    native.first_hits = vec![vec![None; count]];
    native.first_closed = vec![vec![false; count]];
    let mut hit_world = w.clone();
    let yes = m.types[m.state_variables[0].base.0]
        .members
        .iter()
        .position(|v| v == "true")
        .unwrap();
    hit_world.cases[0].payload_mut().unwrap().frame.state_values[0] =
        StateValue::Value(ValueId(yes));
    let first = native.leaf(&hit_world, ReferenceCut::Range).unwrap();
    native.first_hits = vec![vec![None; count]];
    native.first_closed = vec![vec![false; count]];
    let last = native.leaf(&w, ReferenceCut::Range).unwrap();
    native.reduce(
        vec![(Rational::one(), first), (Rational::one(), last)],
        false,
    );
    fn shape(t: &crate::reference::quality::FirstTree) -> serde_json::Value {
        serde_json::json!({"random":t.random,"views":t.views.iter().map(|row|row.iter().map(|v|format!("{v:?}")).collect::<Vec<_>>()).collect::<Vec<_>>(),"children":t.children.iter().map(|(p,c)|serde_json::json!([p.to_string(),shape(c)])).collect::<Vec<_>>()})
    }
    assert_eq!(shape(tree), shape(native.first_trees.last().unwrap()));
}
