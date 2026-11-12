//! ADR0034 review fixes: public acceptance for mutation survivors (O-c, F5,
//! F9, L7, L12) and thinly detected faults (F7, B1, K1). Every expectation is
//! derived by hand from the model text, not copied from a production run.
use relca_core::report;
use serde_json::{Value, json};

fn vp(s: &str) -> Value {
    let a = serde_json::to_value(report::check(s)).unwrap();
    assert!(a["agency"].is_object(), "input must lower: {a}");
    assert!(a["agency"]["viewpoints"].is_object(), "missing viewpoints");
    a["agency"]["viewpoints"].clone()
}
fn framing_unchecked() -> Value {
    json!(["type_domain_authority", "practical_framing"])
}
fn exercise(capability: &str, min: usize, max: usize, cancellation: bool) -> Value {
    json!({"capability":capability,"status":"derived","normal":{"min":min,"max":max},
    "contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":cancellation})
}

// O-c: normal.people is the union of every reader's executors, not the first.
#[test]
fn two_readers_of_one_source_union_their_executors() {
    let s = r#"model TwoReaders version "0.2" {
      type Entry = enum { entry }
      people [c1,u1]
      actor Invoice { source document }
      actor Clerk { executors [c1]; capability enter() -> Entry { reads [Invoice]; duration fixed 1min; result nondet { entry } } }
      actor Auditor { executors [u1]; capability audit() -> Entry { reads [Invoice]; duration fixed 1min; result nondet { entry } } }
      process Flow { e = Clerk.enter(); a = Auditor.audit(); end done }
      cut Scope { include [Invoice,Clerk,Auditor] }
      bounds { cases 1; live_tasks 2; clock 20min }
      verify safety no_resource_overcommit
    }"#;
    assert_eq!(
        vp(s)["observation"],
        json!({"status":"derived","sources":[{"actor":"Invoice",
            "normal":{"actors":["Clerk","Auditor"],"people":["c1","u1"],"unattributed_actors":[]},
            "contest":{"actors":[],"people":[],"unattributed_actors":[],"routes":[]}}],
            "people":[
              {"person":"c1","normal":{"direct":["Invoice"],"indirect":[]},"contest":{"direct":[]}},
              {"person":"u1","normal":{"direct":["Invoice"],"indirect":[]},"contest":{"direct":[]}}],
            "fallback":"unchecked"})
    );
}

// F5: one state-guarded incoming path is enough to leave the end not_derived,
// even when another incoming path to the same end reads only bindings.
#[test]
fn end_reached_by_binding_only_and_state_guarded_paths_is_not_derived() {
    let s = r#"model MixedEnd version "0.2" {
      type V = enum { pass,fail }
      observed case { flag: optional<V> }
      actor A { capacity 1; capability draw() -> V { duration fixed 1min; result nondet { pass,fail } } }
      process Flow {
        a = A.draw()
        if a == pass { end done } else { if flag == pass { end done } else { end returned } }
      }
      cut K { include [A] }
      bounds { cases 1; live_tasks 2; clock 10min }
      verify safety no_resource_overcommit
    }"#;
    assert_eq!(
        vp(s)["framing"],
        json!({"status":"derived","ends":[
            {"end":"done","status":"not_derived","reason":"state_guards"},
            {"end":"returned","status":"not_derived","reason":"state_guards"}],
            "actors":[],"complete":false,"unchecked":framing_unchecked()})
    );
}

// F9: a call that selects the end only in some contexts is still a framing actor.
#[test]
fn some_contexts_selectors_are_framing_actors() {
    let s = r#"model SomeOnly version "0.2" {
      type V = enum { pass,fail }
      actor A { capacity 1; capability draw() -> V { duration fixed 1min; result nondet { pass,fail } } }
      actor B { capacity 1; capability draw() -> V { duration fixed 1min; result nondet { pass,fail } } }
      process Flow { a = A.draw(); b = B.draw(); if a == pass || b == pass { end done } else {} }
      cut K { include [A,B] }
      bounds { cases 1; live_tasks 2; clock 10min }
      verify safety no_resource_overcommit
    }"#;
    assert_eq!(
        vp(s)["framing"],
        json!({"status":"derived","ends":[{"end":"done","status":"derived","reachable":true,
            "selectors":[
              {"call":"a","capability":"A.draw","selects":"some_contexts"},
              {"call":"b","capability":"B.draw","selects":"some_contexts"}],
            "read_only":[]}],
            "actors":["A","B"],"complete":true,"unchecked":framing_unchecked()})
    );
}

// L7: cancellation is collected only from satisfiable sinks; the fork under
// a == pass && a == fail never starts, so nothing is cancelled or exercised.
#[test]
fn unsatisfiable_cancel_then_drain_fork_is_neither_cancelled_nor_exercised() {
    let s = r#"model DeadFork version "0.2" {
      type V = enum { pass,fail }
      actor A { capacity 1; capability draw() -> V { duration fixed 1min; result nondet { pass,fail } } }
      actor B { capacity 1; capability draw() -> V { cancel immediate; duration fixed 1min; result nondet { pass,fail } } }
      actor C { capacity 1; capability draw() -> V { cancel immediate; duration fixed 1min; result nondet { pass,fail } } }
      process Flow {
        a = A.draw()
        if a == pass {
          if a == fail {
            fork f { b = B.draw(); c = C.draw() } join any f where result == pass tie by [b,c] remaining cancel_then_drain if none -> returned
            end won
          } else { end done }
        } else { end done }
        end returned
      }
      cut K { include [A,B,C] }
      bounds { cases 1; live_tasks 4; clock 20min }
      verify safety no_resource_overcommit
    }"#;
    assert_eq!(
        vp(s)["retention"]["capabilities"],
        json!([
            exercise("A.draw", 1, 1, false),
            exercise("B.draw", 0, 0, false),
            exercise("C.draw", 0, 0, false)
        ])
    );
}

// L12: a branch that falls off the end of the process body is still a sink.
#[test]
fn branch_falling_through_the_process_tail_is_counted() {
    let s = r#"model FallThrough version "0.2" {
      type V = enum { pass,fail }
      actor A { capacity 1; capability draw() -> V { duration fixed 1min; result nondet { pass,fail } } }
      actor B { capacity 1; capability draw() -> V { duration fixed 1min; result nondet { pass,fail } } }
      process Flow { a = A.draw(); if a == pass { end done } else { b = B.draw() } }
      cut K { include [A,B] }
      bounds { cases 1; live_tasks 2; clock 10min }
      verify safety no_resource_overcommit
    }"#;
    assert_eq!(
        vp(s)["retention"]["capabilities"],
        json!([
            exercise("A.draw", 1, 1, false),
            exercise("B.draw", 0, 1, false)
        ])
    );
}

// F7: the otherwise value of a requires is not a result the operation chooses.
// Gate always installs approval = pass, so Post is never refused; p can only
// be posted, reaches done in every context, and never selects an end.
#[test]
fn constant_writer_requires_otherwise_value_does_not_make_a_selector() {
    let s = r#"model GatePost version "0.2" {
      type T = enum { pass,fail,posted,denied }
      observed case { approval: optional<T> }
      actor Gate { capacity 1; capability open() -> T { duration fixed 1min; result nondet { pass }; writes approval; finish { approval = pass } } }
      actor Post { capacity 1; capability post() -> T { requires approval == pass else denied; duration fixed 1min; result nondet { posted } } }
      process Flow { g = Gate.open(); p = Post.post(); if p == posted { end done } else { end returned } }
      cut K { include [Gate,Post] }
      bounds { cases 1; live_tasks 2; clock 10min }
      verify safety no_resource_overcommit
    }"#;
    let v = vp(s);
    assert_eq!(
        v["framing"],
        json!({"status":"derived","ends":[
            {"end":"done","status":"derived","reachable":true,"selectors":[],
             "read_only":[{"call":"p","capability":"Post.post"}]},
            {"end":"returned","status":"derived","reachable":false,"selectors":[],"read_only":[]}],
            "actors":[],"complete":true,"unchecked":framing_unchecked()})
    );
    assert_eq!(
        v["retention"]["capabilities"],
        json!([
            exercise("Gate.open", 1, 1, false),
            exercise("Post.post", 1, 1, false)
        ])
    );
}

// K1: source rows follow actor declaration order, not name order.
#[test]
fn three_source_rows_keep_declaration_order() {
    let s = r#"model SourceOrder version "0.2" {
      type V = enum { pass,fail }
      people [m1,z1,a1]
      actor Mid { source record }
      actor Zed { source document }
      actor Ant { source document }
      actor ReadsMid { executors [m1]; capability work() -> V { reads [Mid]; duration fixed 1min; result nondet { pass,fail } } }
      actor ReadsZed { executors [z1]; capability work() -> V { reads [Zed]; duration fixed 1min; result nondet { pass,fail } } }
      actor ReadsAnt { executors [a1]; capability work() -> V { reads [Ant]; duration fixed 1min; result nondet { pass,fail } } }
      process Flow { m = ReadsMid.work(); z = ReadsZed.work(); a = ReadsAnt.work(); end done }
      cut K { include [Mid,Zed,Ant,ReadsMid,ReadsZed,ReadsAnt] }
      bounds { cases 1; live_tasks 3; clock 10min }
      verify safety no_resource_overcommit
    }"#;
    let row = |source: &str, reader: &str, person: &str| {
        json!({"actor":source,
            "normal":{"actors":[reader],"people":[person],"unattributed_actors":[]},
            "contest":{"actors":[],"people":[],"unattributed_actors":[],"routes":[]}})
    };
    assert_eq!(
        vp(s)["observation"]["sources"],
        json!([
            row("Mid", "ReadsMid", "m1"),
            row("Zed", "ReadsZed", "z1"),
            row("Ant", "ReadsAnt", "a1")
        ])
    );
}

// B1: framing and retention have independent budgets. Measured with a private
// probe at HEAD 70ef704 (not an expectation source): framing spends about 629k
// work and retention about 500k, so each fits the default 1,000,000 alone but
// not one shared budget. The expected tables below are derived by hand.
#[test]
fn framing_and_retention_do_not_share_one_budget() {
    let xs: Vec<String> = (0..9).map(|i| format!("x{i}")).collect();
    let actors: String = xs
        .iter()
        .map(|x| format!("actor A{x} {{ capacity 1; capability draw() -> V {{ duration fixed 1min; result nondet {{ pass,fail }} }} }}\n"))
        .collect();
    let calls: String = xs.iter().map(|x| format!("{x} = A{x}.draw(); ")).collect();
    let any = xs
        .iter()
        .map(|x| format!("{x} == pass"))
        .collect::<Vec<_>>()
        .join(" || ");
    let include = xs
        .iter()
        .map(|x| format!("A{x}"))
        .collect::<Vec<_>>()
        .join(",");
    let s = format!(
        r#"model Budgets version "0.2" {{
      type V = enum {{ pass,fail }}
      actor C {{ capacity 1; capability draw() -> V {{ duration fixed 1min; result nondet {{ pass,fail }} }} }}
      {actors}
      actor Ay {{ capacity 1; capability draw() -> V {{ duration fixed 1min; result nondet {{ pass,fail }} }} }}
      process Flow {{ c = C.draw(); if c == pass {{ {calls} if {any} {{ end done }} else {{ end returned }} }} else {{ y = Ay.draw(); if y == pass {{}} else {{}} }} }}
      cut K {{ include [C,{include},Ay] }}
      bounds {{ cases 1; live_tasks 11; clock 100min }}
      verify safety no_resource_overcommit
    }}"#
    );
    let v = vp(&s);
    let selectors = |x: &str| {
        let mut out = vec![json!({"call":"c","capability":"C.draw","selects":"all_contexts"})];
        for name in &xs {
            out.push(json!({"call":name,"capability":format!("A{name}.draw"),"selects":x}));
        }
        out
    };
    let mut actors = vec!["C".to_string()];
    actors.extend(xs.iter().map(|x| format!("A{x}")));
    assert_eq!(
        v["framing"],
        json!({"status":"derived","ends":[
            {"end":"done","status":"derived","reachable":true,"selectors":selectors("some_contexts"),"read_only":[]},
            {"end":"returned","status":"derived","reachable":true,"selectors":selectors("all_contexts"),"read_only":[]}],
            "actors":actors,"complete":true,"unchecked":framing_unchecked()})
    );
    let mut rows = vec![exercise("C.draw", 1, 1, false)];
    rows.extend(
        xs.iter()
            .map(|x| exercise(&format!("A{x}.draw"), 0, 1, false)),
    );
    rows.push(exercise("Ay.draw", 0, 1, false));
    assert_eq!(v["retention"]["status"], "derived", "{}", v["retention"]);
    assert_eq!(v["retention"]["capabilities"], json!(rows));
}
