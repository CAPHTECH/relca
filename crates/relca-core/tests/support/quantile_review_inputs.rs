//! Review regressions are inline: the closed 67-name fixture population is unchanged.
#![allow(dead_code)]
use serde_json::{Value, json};

pub const CLOSED: &str = r#"model QuantileClosedCase version "0.2" {
  type T = enum { ok }
  actor Ledger {
    capacity 1
    state n: 0..2 = 0 last_write_wins
    capability add() -> T {
      writes n
      finish { n += 1 }
      duration fixed 1min
      result nondet { ok }
    }
  }
  process P { r = Ledger.add(); end done }
  cut C { include [Ledger] }
  arrivals fixed [0min, 2min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 1; clock 5min }
  simulation replications 1 seed 1 confidence 0.5
  verify reachable end.done
  property case.Two = reached(Ledger.n == 2)
  metric quantile.All = quantile(1, case.Two)
  metric quantile.Given = quantile(1, case.Two given reached)
}
"#;

pub const DESK: &str = r#"model Q version "0.2" {
  type T = enum { ok }
  actor Desk {
    capacity 1
    state seen: bool = false last_write_wins
    state flag: bool = false last_write_wins
    capability first() -> T {
      writes seen
      finish { seen = true }
      duration fixed 1min
      result nondet { ok }
    }
    capability second() -> T {
      writes flag
      finish { flag = true }
      duration fixed 1min
      result nondet { ok }
    }
  }
  process P {
    if Desk.seen == true { s = Desk.second(); end done }
    else { f = Desk.first(); end done }
  }
  cut C { include [Desk] }
  arrivals fixed [0min, 5min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 1; clock 20min }
  simulation replications 1 seed 1 confidence 0.5
  verify reachable end.done
  property case.Flag = reached(Desk.flag == true)
  metric quantile.Q1 = quantile(1, case.Flag)
  metric quantile.Q1Done = quantile(1, case.Flag given reached)
}
"#;

pub const SELECTED: &str = r#"model SelectedSuccessor version "0.2" {
  type T = enum { ok }
  actor A { capacity 1; capability run() -> T { duration {2min:0.5, 4min:0.5}; result nondet { ok } } }
  process P { job = A.run(); end done }
  cut C { include [A] }
  arrivals fixed [0min]
  policy fifo tie_by case_id
  bounds { cases 1; live_tasks 1; clock 10min }
  simulation replications 1 seed 2 confidence 0.5
  verify reachable end.done
  property case.Done = reached(end.done)
  metric quantile.All = quantile(1, case.Done)
  metric quantile.Given = quantile(1, case.Done given reached)
}
"#;

pub fn selected_rows() -> Value {
    json!([
        {"name":"All","p":"1","property":"Done","form":"all_arrivals","value":{"point":"4","observations":1,"censored":0,"reason":null},"lower_bound":null},
        {"name":"Given","p":"1","property":"Done","form":"reached_only","value":{"point":"4","observations":1,"censored":0,"reason":null},"lower_bound":null}
    ])
}

pub fn edit(s: &str, from: &str, to: &str) -> String {
    assert_eq!(s.matches(from).count(), 1, "single edit witness");
    let out = s.replacen(from, to, 1);
    assert_ne!(out, s, "non-noop pair");
    out
}

pub fn rows(sampled: bool, positive: bool, desk: bool) -> Value {
    let value = |finite: bool| {
        if sampled {
            json!({"point":if finite {json!("1")} else {Value::Null},"observations":2,"censored":0,"reason":if finite {Value::Null} else {json!("beyond_reached_mass")}})
        } else if finite {
            json!({"lower":"1","upper":"1","basis":"exact","reason":null})
        } else {
            json!({"lower":null,"upper":null,"basis":null,"reason":"beyond_reached_mass"})
        }
    };
    let (all, given, property) = if desk {
        ("Q1", "Q1Done", "Flag")
    } else {
        ("All", "Given", "Two")
    };
    json!([
        {"name":all,"p":"1","property":property,"form":"all_arrivals","value":value(positive),"lower_bound":null},
        {"name":given,"p":"1","property":property,"form":"reached_only","value":value(true),"lower_bound":null}
    ])
}
