#![allow(dead_code)]
#[path = "read_write_oracle.rs"]
pub mod h;
use serde_json::{Value, json};
pub const EXCLUDED: &str = "concurrency.read_write_excluded";
pub const EXCLUDED_AFFECTS: &str = "一覧の対象外の静的な読取り/書込み対の種類と数。単一案件のfork兄弟もsingle_caseに含む。到達や同時実行を確定しない。潜在の読取りは数えない。候補の不在は競合なしの証明ではない。";

// Independent IR census. Hand counts below are not derived from this function.
pub fn excluded_counts(m: &relca_core::ir::Model) -> [usize; 2] {
    use relca_core::ir::{StateKind, StateVarId};
    let mut out = [0; 2];
    for (i, v) in m.state_variables.iter().enumerate() {
        let category = match v.kind {
            StateKind::ObservedCase => 1,
            StateKind::ActorObserved(a) if m.actors[a.0].is_per_case() => continue,
            StateKind::ActorObserved(_) if m.bounds.cases == 1 => 0,
            _ => continue,
        };
        let id = StateVarId(i);
        let cap_readers = m
            .capabilities
            .iter()
            .filter(|c| {
                c.requires
                    .as_ref()
                    .is_some_and(|r| h::has(&r.condition, id))
                    || c.atomic.as_ref().is_some_and(|a| h::has(&a.condition, id))
            })
            .count();
        let mut ifs = vec![];
        h::if_readers(&m.process.body, id, &m.process.name, &mut ifs);
        let writers = m
            .capabilities
            .iter()
            .filter(|c| {
                c.start_writes
                    .iter()
                    .chain(&c.writes)
                    .chain(c.atomic.iter().flat_map(|a| &a.writes))
                    .any(|w| w.variable == id)
            })
            .count();
        out[category] += (cap_readers + ifs.len()) * writers;
    }
    out
}

pub fn disclosure(counts: [usize; 2]) -> Vec<Value> {
    if counts == [0; 2] {
        return vec![];
    }
    vec![
        json!({"field":EXCLUDED,"value":format!("single_case={}; case_local={}",counts[0],counts[1]),"affects":EXCLUDED_AFFECTS}),
    ]
}
pub fn cases() -> Vec<(&'static str, String, [usize; 2], usize)> {
    let single = h::BASE
        .replace("cases 2", "cases 1")
        .replace("  arrivals fixed [0min, 0min]\n", "")
        .replace("live_tasks 1", "live_tasks 2")
        .replace(
            "a = A.r(); b = A.w();",
            "fork f { a = A.r(); b = A.w() } join all f;",
        );
    let two = single.replace("cases 1", "cases 2").replace(
        "  policy fifo",
        "  arrivals fixed [0min, 0min]\n  policy fifo",
    );
    let local = h::cases()
        .into_iter()
        .find(|(n, _, _)| *n == "case-local")
        .unwrap()
        .1;
    let per = per_case_source();
    let mixed = h::BASE
        .replace(
            "  actor A",
            "  observed case { z: bool = false }\n  actor A",
        )
        .replace("requires x == false", "requires x == false && z == false")
        .replace("writes x", "writes [x, z]")
        .replace("x = true }", "x = true; z = true }");
    let extra_reader = "capability s() -> R { requires x == false else rejected; duration fixed 1min; result nondet { ok } }\n";
    let local_two = local.replace(
        "    capability w()",
        &format!("    {extra_reader}    capability w()"),
    );
    let local_three = local_two.replace(
        "    capability w()",
        &format!(
            "    {}    capability w()",
            extra_reader.replace("s()", "t()")
        ),
    );
    let single_two = h::cases()
        .into_iter()
        .find(|(n, _, _)| *n == "variable-order")
        .unwrap()
        .1
        .replace("cases 2", "cases 1")
        .replace("  arrivals fixed [0min, 0min]\n", "");
    let mut mixed_five = mixed
        .replace("cases 2", "cases 1")
        .replace("  arrivals fixed [0min, 0min]\n", "");
    for cap in ["s", "t", "u", "v"] {
        mixed_five=mixed_five.replace("    capability w()",&format!("    capability {cap}() -> R {{ requires z == false else rejected; duration fixed 1min; result nondet {{ ok }} }}\n    capability w()"));
    }
    vec![
        ("single-fork", single, [1, 0], 0),
        ("two-fork", two, [0, 0], 1),
        ("case-local", local.clone(), [0, 1], 0),
        (
            "case-local-repeated",
            local.replace("requires x == false", "requires x == false || x == false"),
            [0, 1],
            0,
        ),
        ("per-case", per, [0, 0], 0),
        ("mixed", mixed, [0, 1], 1),
        ("case-local-two", local_two, [0, 2], 0),
        ("case-local-three", local_three, [0, 3], 0),
        ("single-two", single_two, [2, 0], 0),
        ("mixed-single-five", mixed_five, [1, 5], 0),
        (
            "empty",
            h::BASE.replacen("requires x", "requires y", 1),
            [0, 0],
            0,
        ),
    ]
}
pub fn assert_disclosure(v: &Value, counts: [usize; 2], candidates: usize) {
    let rows = v["assumptions"].as_array().unwrap();
    let got: Vec<_> = rows
        .iter()
        .filter(|r| r["field"] == EXCLUDED)
        .cloned()
        .collect();
    assert_eq!(got, disclosure(counts), "hand-counted excluded triples");
    assert_eq!(h::candidate_rows(v).len(), candidates);
    if !got.is_empty() {
        let at = rows.iter().position(|r| r["field"] == EXCLUDED).unwrap();
        if candidates > 0 {
            assert_eq!(rows[at - 1]["field"], h::FIELD);
        }
        assert!(!rows[at + 1..].iter().any(|r| r["field"] == h::FIELD));
        let anchor = h::concurrency_anchor(v);
        assert_eq!(
            at,
            anchor + candidates,
            "excluded disclosure follows the concurrency anchor and all candidates"
        );
    }
}
pub fn surface_source() -> String {
    cases().into_iter().find(|(n,_,_,_)|*n=="mixed").unwrap().1.replace("  bounds","  property case.Done = reached(end.done)\n  metrics { window 10min; currency \"JPY\"; breakdown waits }\n  simulation replications 2 seed 1 confidence 0.95\n  bounds")
}
pub fn per_case_source() -> String {
    r#"model Per version "0.2" {
      type R = enum { ok }
      actor Doc { instance per_case; state flag: bool = false }
      actor A { capacity 1; capability set() -> R { duration fixed 1min; result nondet { ok } } }
      process P { if Doc.flag == false { a = A.set(); end done } else { end done } }
      cut C { include [Doc, A] }
      bounds { cases 2; live_tasks 1; clock 5min }
      arrivals fixed [0min, 0min]
      policy fifo tie_by case_id
      verify reachable end.done
    }"#
    .into()
}
pub fn per_case_refusals() -> Vec<String> {
    let base = per_case_source();
    vec![
        base.replace("state flag: bool = false }","state flag: bool = false; capacity 1; capability w() -> R { writes flag; duration fixed 1min; result nondet { ok }; finish { flag = true } } }"),
        base.replace("capability set() -> R {", "capability set() -> R { writes Doc.flag; finish { Doc.flag = true }")
    ]
}
pub fn set_source() -> String {
    r#"model Sets version "0.2" {
      type R = enum { ok, rejected }
      actor A { capacity 2; state keys: set<CaseId>; state other: set<CaseId>
        capability r() -> R { requires case.id in keys || case.id in keys else rejected; duration fixed 1min; result nondet { ok } }
        capability w(key: CaseId) -> R { writes keys; duration fixed 1min
          atomic { if key in keys { result rejected } else { keys += key; result ok } }
        }
      }
      process P { a = A.r(); b = A.w(case.id); end done }
      cut C { include [A] }
      bounds { cases 2; live_tasks 1; clock 5min }
      arrivals fixed [0min, 0min]
      policy fifo tie_by case_id
      verify reachable end.done
    }"#.into()
}
pub fn removal_changes(field: &str, value: &str, affects: &str, added: bool) -> Vec<Value> {
    [("affects",affects),("field",field),("value",value)].into_iter().map(|(leaf,payload)| {
        json!({"check":"assumptions","key":format!("/field={};value={}/{leaf}",json!(field),json!(value)),"field":"value","before":if added {Value::Null}else{json!(payload)},"after":if added {json!(payload)}else{Value::Null},"direction":"changed"})
    }).collect()
}
pub fn assert_outer_removed(v: &Value) {
    let (_, src, values) = h::cases()
        .into_iter()
        .find(|(n, _, _)| *n == "two-if-sites")
        .unwrap();
    assert_eq!(
        h::candidate_rows(&v["original"])
            .iter()
            .map(|r| r["value"].as_str().unwrap())
            .collect::<Vec<_>>(),
        values
    );
    let mutant = v["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|k| k["mutants"].as_array().unwrap())
        .find(|m| {
            m["operator"] == "bypass_guard"
                && m["site"]["statement_path"] == "body/0"
                && m["site"]["retained_branch"] == "then"
        })
        .unwrap();
    assert_eq!(
        h::candidate_rows(&mutant["report"])
            .iter()
            .map(|r| r["value"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![values[1].as_str()]
    );
    let got: Vec<_> = mutant["other_changes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["check"] == "assumptions" && r["key"].as_str().unwrap().contains(h::FIELD))
        .cloned()
        .collect();
    assert_eq!(
        got,
        removal_changes(h::FIELD, &values[0], h::AFFECTS, false),
        "outer candidate disappeared while final inner row stayed fixed; source={src}"
    );
}
