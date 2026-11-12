//! Review inputs: deterministic event times, not captured implementation output.
#![allow(dead_code)]
#[path = "q9_inputs.rs"]
mod original;

pub fn source(name: &str) -> String {
    use original::edit;
    match name {
        "fixed" => original::source("fixed"),
        "legacy" => original::source("legacy-fixed"),
        "range-before-target" | "range-in-route" => {
            let s = edit(&source("fixed"), "  type T", "  observed case { n: 0..1 = 1 }\n  type T");
            let (from, to) = if name == "range-before-target" {
                ("duration fixed 1min; result nondet { fail }", "duration fixed 1min; result nondet { fail }; writes n; finish { n += 1 }")
            } else {
                ("duration fixed 2min; result nondet { pass }", "duration fixed 2min; result nondet { pass }; writes n; finish { n += 1 }")
            };
            edit(&s, from, to)
        }
        "permission-cut" => {
            let s = edit(&source("fixed"), "permit r to Reviewer.reply", "permit j to Reviewer.reply");
            edit(&s, "clock 20min", "clock 2min")
        }
        "legacy-overflow" => edit(&source("legacy"), "respond_by 3min", "respond_by 18446744073709551615min"),
        "legacy-live" | "legacy-live-control" => {
            let s = edit(&source("legacy"), "cases 1", "cases 2");
            let s = edit(&s, "  process Flow", "  arrivals fixed [0min,1min]\n  policy fifo tie_by case_id then call_order assign first_idle\n  process Flow");
            let s = edit(&s, "  type T", "  kernel K { * -> { 0.5:fail; 0.5:pass } }\n  type T");
            let s = edit(&s, "duration fixed 1min; result nondet { fail }", "duration fixed 1min; outcomes kernel K");
            let s = edit(&s, "process Flow { d = Judge.decide(); if d == fail { end returned } else { end posted } }", "process Flow { d = Judge.decide(); if d == fail { wait 2min; end returned } else { wait 1min; fork f { a = Reviewer.reply(); b = Reviewer.reply() } join all f; end posted } }");
            let s = edit(&s, "seed 1", "seed 5");
            let s = edit(&s, "replications 16", "replications 1");
            if name == "legacy-live" { edit(&s, "live_tasks 4", "live_tasks 1") } else { s }
        }
        "end-unstarted" => r#"model EndUnstarted version "0.2" {
  type T = enum { pass, denied }
  observed case { ready: bool = false }
  actor A { capacity 1; control policy
    capability reject() -> T { requires ready == true else denied; duration fixed 1min; result nondet { pass }; cancel immediate }
  }
  actor B { capacity 0; capability queued() -> T { duration fixed 2min; result nondet { pass }; cancel immediate } }
  process Flow {
    fork f { a = A.reject(); b = B.queued() }
    join any f where result == denied tie by [a,b] remaining cancel_then_drain if none -> other
    if ready == false { end done } else { end other }
  }
  arrivals fixed [0min]
  policy fifo tie_by case_id then call_order assign first_idle
  bounds { cases 1; live_tasks 2; clock 5min }
  metrics { window 5min; currency "JPY"; rate A per_minute 0; rate B per_minute 0; rate A.reject per_start 0; rate B.queued per_start 0 }
  verify reachable end.done
  simulation replications 16 seed 1 confidence 0.95
}"#.into(),
        _ => panic!("unknown review input {name}"),
    }
}
