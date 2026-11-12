//! Test-side IR relation, deliberately independent of the production collector.
#![allow(dead_code)]
use relca_core::{ir, lower, syntax};
use serde_json::{Value, json};

pub const FIELD: &str = "concurrency.read_write";
pub const AFFECTS: &str = "共有の観測stateの静的な読取り/書込み候補。到達や同時実行を確定しない。到達不能な枝や資源による直列化も含みうる。全順序を展開し、古い値を読む経路を残す。複数案件の共有Actor stateだけを対象とし、単一案件・案件局所・per_case・潜在は一覧の対象外。候補の不在は競合なしの証明ではない。";
pub const VERIFY_REASON: &str =
    "relca verify: not implemented; outside Phase 1; external backend is not connected\n";
pub const BASE: &str = r#"model ReadWrite version "0.2" {
  type R = enum { ok, rejected }
  actor A {
    capacity 2
    state x: bool = false last_write_wins
    state y: bool = false
    capability r() -> R {
      requires x == false else rejected
      duration fixed 1min
      result nondet { ok }
    }
    capability w() -> R {
      writes x
      duration fixed 1min
      result nondet { ok }
      finish { x = true }
    }
  }
  process P { a = A.r(); b = A.w(); end done }
  cut C { include [A] }
  bounds { cases 2; live_tasks 1; clock 10min }
  arrivals fixed [0min, 0min]
  policy fifo tie_by case_id
  verify reachable end.done
}"#;

pub fn model(src: &str) -> ir::Model {
    lower::lower(&syntax::parse(src).expect("syntax substrate"))
        .expect("existing accepted syntax substrate")
}

pub fn has(c: &ir::Cond, id: ir::StateVarId) -> bool {
    match c {
        ir::Cond::Contains { set, .. } => *set == id,
        ir::Cond::IntCompare { variable, .. } => *variable == id,
        ir::Cond::StateEq(s) => s.variable == id,
        ir::Cond::Eq(_) => false,
        ir::Cond::And(cs) | ir::Cond::Or(cs) => cs.iter().any(|c| has(c, id)),
    }
}

pub fn if_readers(b: &ir::Block, id: ir::StateVarId, process: &str, out: &mut Vec<String>) {
    for s in b {
        if let ir::Stmt::If {
            condition,
            then_body,
            else_body,
        } = &s.kind
        {
            if has(condition, id) {
                out.push(format!("process.{process}@if:{}", s.span.start));
            }
            if_readers(then_body, id, process, out);
            if_readers(else_body, id, process, out);
        }
    }
}

pub fn rows(m: &ir::Model) -> Vec<Value> {
    if m.bounds.cases < 2 {
        return vec![];
    }
    let name = |c: &ir::Capability| format!("{}.{}", m.actors[c.actor.0].name, c.name);
    let mut out = vec![];
    for (i, v) in m.state_variables.iter().enumerate() {
        let ir::StateKind::ActorObserved(a) = v.kind else {
            continue;
        };
        if m.actors[a.0].is_per_case() {
            continue;
        }
        let id = ir::StateVarId(i);
        let mut readers: Vec<_> = m
            .capabilities
            .iter()
            .filter(|c| {
                c.requires.as_ref().is_some_and(|r| has(&r.condition, id))
                    || c.atomic.as_ref().is_some_and(|r| has(&r.condition, id))
            })
            .map(name)
            .collect();
        if_readers(&m.process.body, id, &m.process.name, &mut readers);
        for r in readers {
            for w in &m.capabilities {
                // Do not call a production read/write candidate helper, or
                // finish_writes: enumerate both ordinary and atomic writes here.
                if w.start_writes
                    .iter()
                    .chain(&w.writes)
                    .chain(w.atomic.iter().flat_map(|a| &a.writes))
                    .any(|w| w.variable == id)
                {
                    out.push(json!({"field":FIELD,"value":format!("{}.{}: {} -> {}",m.actors[a.0].name,v.name,r,name(w)),"affects":AFFECTS}));
                }
            }
        }
    }
    out
}

pub fn candidate_rows(v: &Value) -> Vec<Value> {
    v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["field"] == FIELD)
        .cloned()
        .collect()
}

pub fn concurrency_anchor(v: &Value) -> usize {
    let a = v["assumptions"].as_array().unwrap();
    let base_affects = [
        "initial value is drawn from the declared distribution, independently of other initial distributions; the distribution is an analyst assumption",
        "all values are explored nondeterministically; no distribution or uniform probability is assumed",
        "動的な撤回は検査していない。撤回者の記録だけであり、撤回の発生・効力・当事者の受容を検査しない。",
        "outcomes and writes are drawn from the declared rows; separate draws are conditionally independent given the listed variables; the rows are an analyst assumption",
        "duration is drawn from the declared distribution, independently of outcomes and of other durations; the distribution is an analyst assumption",
    ];
    a.iter()
        .rposition(|r| r["field"].as_str().unwrap().ends_with(".concurrency"))
        .or_else(|| {
            a.iter().rposition(|r| {
                let field = r["field"].as_str().unwrap();
                field == "schedule"
                    || field == "initial.actor_case_independence"
                    || field.ends_with(".instance")
                    || r["affects"]
                        .as_str()
                        .is_some_and(|text| base_affects.contains(&text))
            })
        })
        .map_or(0, |i| i + 1)
}

pub fn assert_candidate_tail(v: &Value, wanted: &[Value]) {
    let actual = candidate_rows(v);
    assert_eq!(actual, wanted, "exact surface candidate rows");
    let a = v["assumptions"].as_array().unwrap();
    let at = concurrency_anchor(v);
    assert_eq!(
        &a[at..at + actual.len()],
        actual,
        "immediately after existing concurrency or its base-model slot"
    );
}

pub fn assert_rows(src: &str, v: &Value, expected: &[&str]) {
    let wanted: Vec<_> = expected
        .iter()
        .map(|s| json!({"field":FIELD,"value":s,"affects":AFFECTS}))
        .collect();
    assert_eq!(
        rows(&model(src)),
        wanted,
        "independent relation vs hand oracle"
    );
    assert_candidate_tail(v, &wanted);
}

pub fn cases() -> Vec<(&'static str, String, Vec<String>)> {
    let one = vec!["A.x: A.r -> A.w".into()];
    let negative = BASE.replacen("requires x", "requires y", 1);
    let atomic = BASE.replace("  actor A", "  observed case { touched: bool = false }\n  actor A")
        .replacen("requires x == false else rejected", "writes touched; atomic { if x == false { touched = true; result ok } else { result rejected } }", 1)
        .replacen("      result nondet { ok }\n", "", 1);
    let process = negative.replace("process P { a = A.r(); b = A.w(); end done }", "process P { if A.x == false { a = A.r(); b = A.w(); end done } else { c = A.r(); d = A.w(); end done } }");
    let pos = process.find("if A.x").unwrap();
    let second_reader = BASE.replace("    capability w()", "    capability s() -> R { requires x == false else rejected; duration fixed 1min; result nondet { ok } }\n    capability w()");
    let self_read = negative.replace(
        "      writes x",
        "      requires x == false else rejected\n      writes x",
    );
    let multi = BASE
        .replace(
            "state y: bool = false",
            "state y: bool = false last_write_wins",
        )
        .replace("requires x == false", "requires x == false && y == false")
        .replace("writes x", "writes [x, y]")
        .replace("x = true }", "x = true; y = true }");
    let local = BASE
        .replace(
            "    state x: bool = false last_write_wins\n    state y: bool = false\n",
            "",
        )
        .replace(
            "  actor A",
            "  observed case { x: bool = false; y: bool = false }\n  actor A",
        );
    let single = BASE
        .replace("cases 2", "cases 1")
        .replace("  arrivals fixed [0min, 0min]\n", "");
    let lock = BASE
        .replace("  actor A", "  resource gate capacity 1\n  actor A")
        .replace(
            "      duration fixed 1min",
            "      uses gate: 1\n      duration fixed 1min",
        );
    let atomic_negative = atomic.replace("if x == false", "if y == false");
    let process_negative = process.replace("if A.x == false", "if A.y == false");
    let per_case = process_negative
        .replace(
            "  actor A",
            "  actor Doc { instance per_case; state flag: bool = false }\n  actor A",
        )
        .replace("include [A]", "include [A, Doc]")
        .replace("if A.y == false", "if Doc.flag == false");
    let evaluation = negative.replace("  process P", "  property case.Seen = reached(A.x == true)\n  contract Keep { invariant A.w.finish -> A.x == true }\n  process P");
    let both = atomic.replace(
        "writes touched; atomic",
        "requires x == false else rejected; writes touched; atomic",
    );
    let byte_source = process
        .replace(
            "  actor A",
            "  actor Note { boundary \"前置の日本語\" }\n  actor A",
        )
        .replace("include [A]", "include [A, Note]");
    let byte_pos = byte_source.find("if A.x").unwrap();
    let integer = BASE
        .replace(
            "state x: bool = false last_write_wins",
            "state x: 0..2 = 0 last_write_wins",
        )
        .replace("requires x == false", "requires x < 1")
        .replace("x = true", "x = 1");
    let two_if = BASE.replacen("requires x", "requires y", 1)
        .replace("process P { a = A.r(); b = A.w(); end done }", "process P { if A.x == false { if A.x == false { a = A.r(); b = A.w(); end done } else { c = A.r(); d = A.w(); end done } } else { e = A.r(); f = A.w(); end done } }");
    let two_positions: Vec<_> = two_if
        .match_indices("if A.x")
        .map(|(p, _)| format!("A.x: process.P@if:{p} -> A.w"))
        .collect();
    let second_writer = BASE.to_owned()
        .replace("    capability w()", "    capability z() -> R { writes x; duration fixed 1min; result nondet { ok }; finish { x = true } }\n    capability w()");
    vec![
        ("requires", BASE.into(), one.clone()),
        ("broken-unwritten", negative, vec![]),
        (
            "repeated-read",
            BASE.replace("requires x == false", "requires x == false && x == false"),
            one.clone(),
        ),
        ("atomic", atomic, one.clone()),
        ("atomic-broken-unwritten", atomic_negative, vec![]),
        ("both-capability-conditions", both, one.clone()),
        (
            "process-if",
            process,
            vec![format!("A.x: process.P@if:{pos} -> A.w")],
        ),
        ("process-if-broken-unwritten", process_negative, vec![]),
        (
            "utf8-byte-position",
            byte_source,
            vec![format!("A.x: process.P@if:{byte_pos} -> A.w")],
        ),
        (
            "unused-reader",
            second_reader,
            vec!["A.x: A.r -> A.w".into(), "A.x: A.s -> A.w".into()],
        ),
        ("self-pair", self_read, vec!["A.x: A.w -> A.w".into()]),
        (
            "start-writer",
            BASE.replace("      writes x\n", "")
                .replace("finish { x = true }", "start { x = true }"),
            one.clone(),
        ),
        (
            "variable-order",
            multi,
            vec!["A.x: A.r -> A.w".into(), "A.y: A.r -> A.w".into()],
        ),
        ("single-case", single, vec![]),
        ("case-local", local, vec![]),
        ("per-case", per_case, vec![]),
        ("evaluation-only", evaluation, vec![]),
        ("integer-comparison", integer, one.clone()),
        ("two-if-sites", two_if, two_positions),
        (
            "writer-declaration-order",
            second_writer,
            vec!["A.x: A.r -> A.z".into(), "A.x: A.r -> A.w".into()],
        ),
        (
            "serialized-actor",
            BASE.replace("capacity 2", "capacity 1"),
            one.clone(),
        ),
        ("serialized-resource", lock, one),
    ]
}

pub fn surface_source() -> String {
    BASE.replace(
        "  bounds",
        "  simulation replications 2 seed 1 confidence 0.95\n  bounds",
    )
}

pub fn configuration_source() -> String {
    surface_source().replace("  process P", "  configuration Left { cut C; process P }\n  configuration Right { cut C; process P }\n  process P")
}
