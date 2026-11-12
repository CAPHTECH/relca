//! ADR 0029 review-fix models shared by the core and CLI acceptance. Each
//! model reaches one out-of-range write on one nondeterministic branch; the
//! `with_cut` variant adds one more branch that runs past the clock.
#![allow(dead_code)]

pub const CONTEST: &str = r#"model ContestRange version "0.2" {
  type T = enum { pass, fail, late }
  observed case { n: 0..0 = 0 }
  people [j,p,b]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 1min; result nondet { pass, fail } } }
  actor Route { executors [p]; capability respond() -> T { duration fixed 2min; result nondet { pass } } }
  actor Bump { executors [b]; capability bump() -> T { duration fixed 1min; result nondet { pass }; writes n; finish { n += 1 } } }
  actor Slow { executors [b]; capability go() -> T { duration fixed 20min; result nondet { pass } } }
  actor Party { control party; standing over [end.returned] basis "claim" }
  permit j to Judge.judge
  permit p to Route.respond
  permit b to Bump.bump
  permit b to Slow.go
  authority G { basis "decision"; grants j to Judge.judge }
  process Flow { d = Judge.judge(); if d == fail { end returned } else { if d == pass { x = Bump.bump(); end posted } else { s = Slow.go(); end posted } } }
  cut C { include [Judge,Route,Party,Bump,Slow] }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify safety no_resource_overcommit
  contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 2min }
}
"#;

pub const METRICS: &str = r#"model MetricRange version "0.2" {
  type T = enum { pass, fail, late }
  observed case { n: 0..0 = 0 }
  people [j,b]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 1min; result nondet { pass, fail } } }
  actor Bump { executors [b]; capability bump() -> T { duration fixed 1min; result nondet { pass }; writes n; finish { n += 1 } } }
  actor Slow { executors [b]; capability go() -> T { duration fixed 20min; result nondet { pass } } }
  process Flow { d = Judge.judge(); if d == fail { end returned } else { if d == pass { x = Bump.bump(); end posted } else { s = Slow.go(); end posted } } }
  cut C { include [Judge,Bump,Slow] }
  bounds { cases 1; live_tasks 2; clock 10min }
  metrics { window 10min; currency "JPY"; rate Judge per_minute 1; rate Bump per_minute 1; rate Slow per_minute 1; rate Judge.judge per_start 0; rate Bump.bump per_start 0; rate Slow.go per_start 0 }
  verify safety no_resource_overcommit
}
"#;

pub const REQUIRES: &str = r#"model RequiresRange version "0.2" {
  type T = enum { ok, no, late }
  observed case { n: 0..0 = 0 }
  people [p,q]
  actor Gate { executors [p]; capability check() -> T { duration fixed 1min; result nondet { ok, no } } }
  actor Worker { executors [p]; capability run() -> T { duration fixed 1min; result nondet { ok }; requires n >= 1 else no } }
  actor Bump { executors [p]; capability bump() -> T { duration fixed 1min; result nondet { ok }; writes n; finish { n += 1 } } }
  actor Slow { executors [p]; capability go() -> T { duration fixed 20min; result nondet { ok } } }
  process Flow { g = Gate.check(); if g == ok { b = Bump.bump(); end done } else { if g == no { w = Worker.run(); end refused } else { s = Slow.go(); end done } } }
  cut C { include [Gate,Worker,Bump,Slow] }
  bounds { cases 1; live_tasks 1; clock 5min }
  contract Controls { response case.received -> one_of(Bump.bump.finish) by 3min after case.received otherwise unresolved debtor q }
  verify safety no_resource_overcommit
}
"#;

pub fn with_arrivals(s: &str) -> String {
    let a = s.replace(
        "  bounds {",
        "  arrivals fixed [0min]\n  policy fifo tie_by case_id\n  bounds {",
    );
    assert_ne!(a, s);
    a
}

/// Adds the clock-cut branch (`late`) to the range-stop model.
pub fn with_cut(s: &str) -> String {
    let c = s
        .replace(
            "result nondet { pass, fail }",
            "result nondet { pass, fail, late }",
        )
        .replace("result nondet { ok, no }", "result nondet { ok, no, late }");
    assert_ne!(c, s);
    c
}
