//! ADR0035 mutation survivors (M07, R04, M14, N02, G01, M12, M13). Each test
//! carries its hand derivation; expectations are not copied from an engine run.
#[path = "support/actor_observed_acceptance.rs"]
mod h;
use relca_core::{engine, ir, reference, report};
use serde_json::{Value, json};

fn verdict<'a>(v: &'a Value, id: &str) -> &'a Value {
    &v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("missing property {id}: {v}"))["verdict"]
}

fn ends(v: &Value, case: u64) -> Vec<String> {
    let c = v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["case_id"] == case)
        .unwrap();
    let mut xs: Vec<_> = c["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["end"].as_str().unwrap().to_owned())
        .collect();
    xs.sort();
    xs
}

const ORDER: &str = r#"model Ord version "0.2" {
  type Ack = enum { committed, rejected_duplicate }
  type L = enum { saw_two, saw_less }
  people [c1,c2]
  actor Ledger { capacity 1; state keys: set<CaseId>; state total: 0..2 = 0; capability post(key: CaseId) -> Ack {
      commits
      writes [keys, total]
      duration fixed 1min
      atomic {
        if key in keys { result rejected_duplicate }
        else { keys += key; total += 1; result committed }
      }
      cancel forbidden_after_start
  } }
  actor Reader { capacity 1; state seen: set<CaseId>; capability look(key: CaseId) -> L {
      commits
      writes [seen]
      duration fixed 1min
      atomic {
        if Ledger.total == 2 { seen += key; result saw_two }
        else { result saw_less }
      }
      cancel forbidden_after_start
  } }
  process Flow { a = Ledger.post(case.id); b = Reader.look(case.id); if b == saw_two { end two } else { end less } }
  cut K { include [Ledger, Reader] }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 3; clock 20min }
  verify reachable end.less
  verify reachable end.two
}"#;

/// M07 (engine) and R04 (reference): a same-tick finish of a higher case must
/// not be ordered only after a lower case's finish.
///
/// Hand derivation (unit min). Ledger and Reader have capacity 1, FIFO by
/// case id. Case 1 posts [0,1): keys={1}, total=1. At t=1 case 1 starts look
/// [1,2) and case 2 starts post [1,2). Both finish at t=2 in either order and
/// the atomic condition reads the state just before its own finish:
/// - case 1 look first: total==1, saw_less, case 1 ends less at 2; then case 2
///   post makes total=2.
/// - case 2 post first: total=2; case 1 look sees total==2, saw_two, ends two
///   at 2.
///
/// Case 2 looks [2,3) after its own post, total==2 in both orders, so it ends
/// two at 3. Case 1: {less, two} at 2, elapsed 2, work 1+1=2. Case 2: {two} at
/// 3, elapsed 3, work 2. Per case reachable (ADR 0015): end.two is reached by
/// both cases (satisfied); end.less only by case 1 (violated). Dropping the
/// "case 2 first" order loses case 1's two, so end.two would become violated.
#[test]
fn same_tick_finish_of_a_later_case_is_ordered_both_ways_in_engine_and_reference() {
    let m = h::model(ORDER);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for out in [&e, &r.outcome] {
        let v = h::projection(&m, out);
        assert_eq!(ends(&v, 1), ["less", "two"]);
        assert_eq!(ends(&v, 2), ["two"]);
        h::terminal(&v, 1, "less", (2, 2), (2, 2), (2, 2));
        h::terminal(&v, 1, "two", (2, 2), (2, 2), (2, 2));
        h::terminal(&v, 2, "two", (3, 3), (3, 3), (2, 2));
    }
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
    assert_eq!(e.properties, r.outcome.properties);
    let v = json!(report::check(ORDER));
    assert_eq!(verdict(&v, "reachable end.two"), "satisfied");
    assert_eq!(verdict(&v, "reachable end.less"), "violated");
}

/// M14: executor.id is the finishing task's own executor, not the first task
/// that has an executor.
///
/// Hand derivation (unit min). fork: Prep (only executor c1) [0,5), Clerk (only
/// executor c2) [0,3). Clerk finishes at 3 with input_by = executor.id = c2
/// while Prep (c1) is still running. join all at 5; input_by==c2, so the only
/// end is by_c2 at 5, elapsed 5, work 5+3=8. end.other is unreachable
/// (violated). Writing the running Prep's c1 would reach other instead.
#[test]
fn executor_id_is_the_finishing_tasks_executor_while_another_person_is_running() {
    let s = r#"model Exe version "0.2" {
  type Ack = enum { committed, rejected_duplicate }
  people [c1,c2]
  observed case { input_by:optional<PersonId> }
  actor Prep {executors [c1]; capability prep()->Ack {duration fixed 5min;result nondet{committed}}}
  actor Clerk {executors [c2]; capability enter()->Ack {writes input_by;finish{input_by=executor.id};duration fixed 3min;result nondet{committed}}}
  process Flow { fork f { p=Prep.prep(); a=Clerk.enter() }; join all f;if input_by==c2 {end by_c2}else{end other} }
  cut K { include [Prep, Clerk] }
  bounds { cases 1; live_tasks 3; clock 20min }
  verify reachable end.other
  verify reachable end.by_c2
}"#;
    let v = json!(report::check(s));
    assert_eq!(verdict(&v, "reachable end.other"), "violated");
    assert_eq!(verdict(&v, "reachable end.by_c2"), "satisfied");
    let rows = v["terminals"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{v}");
    assert_eq!(rows[0]["end"], "by_c2");
    assert_eq!(rows[0]["elapsed"], json!({"min":5,"max":5}));
    assert_eq!(rows[0]["work"], json!({"min":8,"max":8}));
    let m = h::model(s);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.terminals, r.outcome.terminals);
}

/// N02: the recognised mechanism needs the rejection on the TRUE side of
/// `key in keys` (ADR 0035 U12 condition 3).
///
/// Hand derivation. The branches are swapped: commit (with keys += key) only
/// when the key is already present. keys starts empty and is only grown on the
/// commit side, so both posts reject: b == rejected_duplicate, end dup. No
/// commit event occurs, so at_most_once is vacuous; condition 3 fails, so the
/// row carries mechanism_unmodeled (the verdict is not changed by the note).
#[test]
fn inverted_atomic_branches_are_not_a_recognised_duplicate_mechanism() {
    let s = h::source("aset-dup");
    let s = h::change(
        &s,
        "if key in keys { result rejected_duplicate }\n        else { keys += key; registered = true; result committed }",
        "if key in keys { keys += key; registered = true; result committed }\n        else { result rejected_duplicate }",
    );
    let v = json!(report::check(&s));
    let row = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "contract C #1 at_most_once")
        .unwrap();
    assert_eq!(row["verdict"], "vacuous");
    assert_eq!(row["notes"], json!(["mechanism_unmodeled"]));
    assert_eq!(verdict(&v, "reachable end.dup"), "satisfied");
}

/// G01: an atomic branch write is Unknown to later symbolic guards.
///
/// Hand derivation. decision starts no. Ledger.prime writes decision = ok only
/// on its commit branch, chosen by `key in keys` over a set the Agency does not
/// enumerate (ADR 0035 Q4), so after prime decision is Unknown, not the
/// constant no. Poster.post's `requires decision==ok` is therefore not decided:
/// no refusal is claimed and state_guards stays unchecked. At run time keys is
/// empty, prime commits and decision=ok, so post is never refused (no
/// capability refusal); keeping the constant no would claim a false refusal.
#[test]
fn atomic_branch_write_is_unknown_to_a_later_requires_not_the_initial_constant() {
    let s = r#"model CaseSet version "0.2" {
  type Ack = enum { committed, rejected_duplicate }
  type T = enum { ok, no }
  people [c1,c2]
  observed case { decision:T=no }
  actor Source{source record}; actor Seed{capacity 1;capability make()->T{reads[Source];duration fixed 1min;result nondet{ok}}};actor Ledger{capacity 1;state keys:set<CaseId>;capability prime(key:CaseId, e:T)->Ack{writes [keys, decision];duration fixed 1min;atomic { if key in keys { result rejected_duplicate } else { keys += key; decision = ok; result committed } }}};actor Poster{capacity 1;capability post()->Ack{commits;requires decision==ok else rejected_duplicate;duration fixed 1min;result nondet{committed}}}
  process Flow { e=Seed.make();p=Ledger.prime(case.id, e);a=Poster.post();end done }
  cut K { include [Source,Seed,Ledger,Poster] }
  bounds { cases 1; live_tasks 3; clock 20min }
  verify safety no_resource_overcommit
}"#;
    let v = json!(report::check(s));
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(v["agency"]["refusal"], json!([]), "{v}");
    assert!(
        v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("state_guards"))
    );
    assert_eq!(v["capability_refusals"], json!([]), "{v}");
    assert_eq!(v["terminals"][0]["elapsed"], json!({"min":3,"max":3}));
}

fn membership_conditions(body: &mut [ir::Statement], hit: &mut usize) {
    fn cond(c: &mut ir::Cond, hit: &mut usize) {
        match c {
            ir::Cond::Contains { element, .. } => {
                assert_eq!(*element, ir::Element::CurrentCase);
                *element = ir::Element::Parameter(0);
                *hit += 1;
            }
            ir::Cond::And(cs) | ir::Cond::Or(cs) => cs.iter_mut().for_each(|c| cond(c, hit)),
            _ => {}
        }
    }
    for s in body {
        if let ir::Stmt::If {
            condition,
            then_body,
            else_body,
        } = &mut s.kind
        {
            cond(condition, hit);
            membership_conditions(then_body, hit);
            membership_conditions(else_body, hit);
        }
    }
}

fn refused_before_exploration(m: &ir::Model, what: &str) {
    let e = engine::explore(m);
    let r = reference::explore(m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort, "{what}");
    for out in [&e, &r.outcome] {
        assert!(
            out.diagnostics.iter().any(|d| d.code == "U-SEM-0"),
            "{what}: {out:?}"
        );
        assert!(out.terminals.is_empty(), "{what}: {out:?}");
        assert!(
            out.case_outcomes
                .as_ref()
                .is_none_or(|cs| cs.iter().all(|c| c.terminals.is_empty()))
        );
    }
}

/// M12: a process condition has no capability parameter, so a public IR whose
/// `if` reads `Contains { element: Parameter(0) }` is malformed. Engine and
/// reference both refuse it before exploring, on the legacy route (aset-policy)
/// and the arrivals route (aset-branch-sharing). The unmodified controls
/// explore to their ends, so only the element makes the difference.
#[test]
fn process_membership_with_a_parameter_element_is_refused_by_engine_and_reference() {
    for name in ["aset-policy", "aset-branch-sharing"] {
        let good = h::model(&h::source(name));
        let e = engine::explore(&good);
        assert!(
            !e.terminals.is_empty()
                || e.case_outcomes
                    .as_ref()
                    .is_some_and(|cs| cs.iter().all(|c| !c.terminals.is_empty())),
            "control {name}: {e:?}"
        );
        let r = reference::explore(&good);
        assert_eq!(r.stop, reference::StopReason::Complete, "control {name}");
        let mut bad = good.clone();
        let mut hit = 0;
        membership_conditions(&mut bad.process.body, &mut hit);
        assert_eq!(hit, 1, "{name}");
        assert_ne!(bad, good);
        refused_before_exploration(&bad, name);
    }
}

/// M13: an executor.id write on an Actor without executors is malformed IR
/// even when no call to it is reachable. The call to Clerk.enter sits in the
/// branch input_by==c1, and input_by starts none, so the control ends by_c2
/// at 0 without running it; validation, not the run, must refuse the IR.
#[test]
fn executor_write_on_an_actor_without_executors_is_refused_before_exploration() {
    let s = h::source("aset-executor");
    let s = h::change(
        &s,
        "a=Clerk.enter();if input_by==c1 {end by_c1}",
        "if input_by==c1 {a=Clerk.enter();end by_c1}",
    );
    let good = h::model(&s);
    let e = engine::explore(&good);
    let r = reference::explore(&good);
    assert_eq!(r.stop, reference::StopReason::Complete);
    let v = h::projection(&good, &e);
    assert_eq!(v["terminals"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["terminals"][0]["end"], "by_c2");
    assert_eq!(e.terminals, r.outcome.terminals);
    let mut bad = good.clone();
    let clerk = bad.actors.iter().position(|a| a.name == "Clerk").unwrap();
    assert!(bad.actors[clerk].executors.is_some());
    bad.actors[clerk].executors = None;
    bad.actors[clerk].capacity = Some(1);
    assert!(
        bad.capabilities
            .iter()
            .filter(|c| c.actor.0 == clerk)
            .flat_map(|c| c.finish_writes())
            .any(|w| w.source == ir::WriteSource::Executor)
    );
    refused_before_exploration(&bad, "anonymous executor write");
}
