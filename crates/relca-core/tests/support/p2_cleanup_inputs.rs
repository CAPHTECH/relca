//! Cleanup inputs. Expectations are derived in p2_cleanup_calculations.md.
#![allow(dead_code)]
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

pub fn dir() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.tmp")
        .join(format!(
            "p2ct-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir_all(&p).unwrap();
    p
}
pub fn edit(s: &str, a: &str, z: &str) -> String {
    assert_ne!(a, z);
    assert_eq!(s.matches(a).count(), 1, "unique edit: {a}");
    s.replacen(a, z, 1)
}
pub const SIMPLE: &str = r#"model Cleanup version "0.2" {
  type R = enum { ok }
  actor A { capacity 1; control policy; capability run() -> R {
    duration fixed 2min; result nondet { ok }
  } }
  process Flow { a = A.run(); end done }
  cut C { include [A] }
  arrivals fixed [0min]
  policy observable fifo tie_by case_id then call_order assign first_idle
  bounds { cases 1; live_tasks 4; clock 10min }
  simulation replications 2 seed 1 confidence 0.5
  verify probability reached end.done >= 0
}"#;
pub fn stochastic() -> String {
    edit(
        SIMPLE,
        "arrivals fixed [0min]",
        r#"arrivals distribution basis "assumed arrivals" { 1: fixed [0min] }"#,
    )
}
pub const DEGRADE: &str = r#"model Prefix version "0.2" {
  type R = enum { ok, fail }
  actor Probe { capacity 1; capability check() -> R { duration fixed 1min; result nondet { ok, fail } } }
  actor Clerk { capacity 1; capability run() -> R { duration fixed 3min; result nondet { ok } } }
  actor Fast { capacity 1; capability run() -> R { duration fixed 1min; result nondet { ok } } }
  process Flow { status = Probe.check();
    degrade Manual when status == fail { c = Clerk.run(); end done }
    else { f = Fast.run(); end done }
  }
  cut C { include [Probe,Clerk,Fast] }
  bounds { cases 1; live_tasks 4; clock 20min }
  verify reachable end.done
}"#;
pub fn prefix_two() -> String {
    edit(
        DEGRADE,
        "status = Probe.check();",
        "pre = Probe.check(); status = Probe.check();",
    )
}
pub fn withdrawn() -> String {
    edit(
        &edit(
            DEGRADE,
            "  bounds {",
            "  arrivals fixed [0min,0min]\n  policy fifo tie_by case_id then call_order assign first_idle\n  bounds {",
        ),
        "cases 1",
        "cases 2",
    )
}
pub fn prefix_diff() -> String {
    let s = edit(DEGRADE, "process Flow", "process Before");
    let extra = r#"process After { pre = Probe.check(); status = Probe.check();
      degrade Manual when status == fail { c = Clerk.run(); end done } else { f = Fast.run(); end done }
    }
    configuration From { cut C; process Before }
    configuration To { cut C; process After }"#;
    let at = s.rfind('}').unwrap();
    format!("{}\n{extra}\n}}", &s[..at])
}
pub fn withdrawn_diff() -> String {
    edit(
        &edit(
            &prefix_diff(),
            "  bounds {",
            "  arrivals fixed [0min,0min]\n  policy fifo tie_by case_id then call_order assign first_idle\n  bounds {",
        ),
        "cases 1",
        "cases 2",
    )
}
pub const WORK: &str = r#"model Work version "0.2" {
  type R = enum { ok }
  people [p,q]
  kernel K basis "assumed not measured" { * -> { 1:ok } }
  performance P basis "assumed selection" { load none; executor p { kernel K }; executor q { kernel K } }
  actor A { executors [p,q]; control policy
    capability run() -> R { qualified_executors [p,q]; duration fixed 2min; outcomes performance P }
    capability old() -> R { duration fixed 2min; outcomes kernel K }
  }
  process Flow { a = A.run(); end done }
  process Old { a = A.old(); end done }
  cut C { include [A] }
  configuration From { cut C; process Old }
  configuration To { cut C; process Flow }
  arrivals fixed [0min]
  policy observable fifo tie_by case_id then call_order assign first_idle
  bounds { cases 1; live_tasks 4; clock 10min }
  verify probability reached end.done >= 0
}"#;
pub fn point() -> String {
    edit(
        &edit(
            &edit(WORK, "configuration From { cut C; process Old }", ""),
            "configuration To { cut C; process Flow }",
            "",
        ),
        "process Old { a = A.old(); end done }",
        "",
    )
}
pub fn suite(n: usize) -> PathBuf {
    let d = dir();
    fs::write(d.join("model.relca"), SIMPLE).unwrap();
    let points: Vec<_> = (0..n).map(|i| json!({"id":format!("p{i}"),"model":"model.relca","configuration":null,"scenario":null})).collect();
    let path = d.join("manifest.json");
    fs::write(&path, json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"finite cleanup witnesses"},"points":points,"targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":null}).to_string()).unwrap();
    path
}

pub const KNOWN_FAILURE: &str = r#"model KnownFailure version "0.2" {
  type R = enum { ok, denied }
  observed case { ready: bool = false; n: 0..1 = 1 }
  people [p,r]
  actor Judge { executors [p]; control policy; capability decide() -> R { duration fixed 1min; result nondet { ok } } }
  actor Reply { executors [r]; control policy
    capability reject() -> R { requires ready == true else denied; duration fixed 1min; result nondet { ok } }
    capability break_range() -> R { writes n; duration fixed 2min; finish { n += 1 }; result nondet { ok } }
  }
  actor Party { control party; standing over [end.done] basis "assumed standing" }
  authority G { basis "assumed decision"; grants p to Judge.decide }
  process Flow { a = Judge.decide(); end done }
  cut C { include [Judge,Reply,Party] }
  arrivals fixed [0min]
  policy observable fifo tie_by case_id then call_order assign first_idle
  bounds { cases 1; live_tasks 4; clock 10min }
  metrics { window 10min; currency "JPY" }
  simulation replications 2 seed 1 confidence 0.5
  verify reachable end.done
  contract Controls {
    contest Party on end.done route Reply.reject reversible_by [p] respond_by 5min
      rates { occurrence probability 1 basis "assumed occurrence"; service_time basis "assumed service"; independence per_case_per_clause }
    contest Party on end.done route Reply.break_range reversible_by [p] respond_by 5min
      rates { occurrence probability 1 basis "assumed occurrence"; service_time basis "assumed service"; independence per_case_per_clause }
  }
}"#;
pub const MANAGEMENT: &str = r#"model Management version "0.2" {
  type R = enum { ok }
  people [p,r]
  actor Judge { executors [p]; control policy; capability decide() -> R { duration fixed 1min; result nondet { ok } } }
  actor Reply { executors [r]; control policy; capability run() -> R { duration fixed 2min; result nondet { ok } } }
  actor Party { control party; standing over [end.done] basis "assumed standing" }
  authority G { basis "assumed decision"; grants p to Judge.decide }
  process Flow { a = Judge.decide(); end done }
  cut C { include [Judge,Reply,Party] }
  arrivals fixed [0min,2min]
  policy observable fifo tie_by case_id then call_order assign first_idle
  bounds { cases 2; live_tasks 4; clock 10min }
  metrics { window 10min; currency "JPY" }
  simulation replications 2 seed 1 confidence 0.5
  verify reachable end.done
  contract Controls {
    contest Party on end.done route Reply.run reversible_by [p] respond_by 5min
      rates { occurrence probability 1 basis "assumed occurrence"; service_time basis "assumed service"; independence per_case_per_clause }
  }
}"#;
