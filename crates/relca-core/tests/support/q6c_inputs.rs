//! Q6c hand models; no runtime output is used to construct expectations.
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
            "q6ct-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir_all(&p).unwrap();
    p
}
pub fn edit(s: &str, from: &str, to: &str) -> String {
    assert_ne!(from, to);
    assert_eq!(s.matches(from).count(), 1, "single edit {from}");
    s.replacen(from, to, 1)
}
pub fn append(s: &str, tail: &str) -> String {
    let at = s.rfind('}').unwrap();
    format!("{}\n{tail}\n}}", &s[..at])
}
pub fn work() -> String {
    r#"model Connect version "0.2" {
      type R = enum { ok, bad }
      people [p,q]
      latent case { bad: bool = false }
      kernel Low basis "low assumption" { * -> { 0.9:ok with bad=false; 0.1:bad with bad=true } }
      kernel Half basis "high assumption" { * -> { 0.5:ok with bad=false; 0.5:bad with bad=true } }
      performance P basis "selector assumption" { load queued_same_actor_at_start
        executor q { [0,1): kernel Low; [1,infinity): kernel Half }
      }
      actor Desk { control policy; executors [p,q]
        capability legacy() -> R { writes bad; duration fixed 2min; outcomes kernel Low }
        capability run() -> R { qualified_executors [q]; writes bad; duration fixed 2min; outcomes performance P }
      }
      process Before { r = Desk.legacy(); end done }
      process After { r = Desk.run(); end done }
      cut C { include [Desk] }
      configuration From { cut C; process Before }
      configuration To { cut C; process After }
      arrivals fixed [0min]
      policy observable fifo tie_by case_id then call_order assign first_idle
      bounds { cases 1; live_tasks 4; clock 20min }
      metrics { window 20min; currency "JPY" }
      simulation replications 16 seed 1 confidence 0.5
      verify probability reached bad == true <= 1
    }"#.into()
}
pub fn bands() -> String {
    let s = edit(
        &work(),
        "process Before { r = Desk.legacy(); end done }",
        "process Before { r = Desk.run(); end done }",
    );
    let s = edit(
        &s,
        "process After { r = Desk.run(); end done }",
        "process After { r = Desk.alternate(); end done }",
    );
    let s = edit(&s, "capability legacy()", "capability alternate()");
    let s = edit(
        &s,
        "writes bad; duration fixed 2min; outcomes kernel Low",
        "qualified_executors [q]; writes bad; duration fixed 2min; outcomes performance Q",
    );
    append(
        &s,
        r#"performance Q basis "selector assumption" { load queued_same_actor_at_start
        executor q { [0,2): kernel Low; [2,infinity): kernel Half } }"#,
    )
}
pub fn point(kind: &str) -> String {
    let s = edit(
        &work(),
        "process Before { r = Desk.legacy(); end done }",
        "",
    );
    let s = edit(&s, "configuration From { cut C; process Before }", "");
    let s = edit(&s, "configuration To { cut C; process After }", "");
    match kind {
        "base" => s,
        "probability" => edit(
            &s,
            "0.9:ok with bad=false; 0.1:bad with bad=true",
            "0.75:ok with bad=false; 0.25:bad with bad=true",
        ),
        "value" => edit(&s, "0.1:bad with bad=true", "0.1:bad with bad=false"),
        "basis" => edit(
            &edit(&s, "selector assumption", "changed selector assumption"),
            "low assumption",
            "changed low assumption",
        ),
        "qualification" => edit(
            &edit(&s, "qualified_executors [q]", "qualified_executors [p]"),
            "executor q {",
            "executor p {",
        ),
        "band" => edit(
            &s,
            "[0,1): kernel Low; [1,infinity): kernel Half",
            "[0,2): kernel Low; [2,infinity): kernel Half",
        ),
        "load" => edit(
            &edit(&s, "load queued_same_actor_at_start", "load none"),
            "[0,1): kernel Low; [1,infinity): kernel Half",
            "kernel Low",
        ),
        "process" => edit(&s, "r = Desk.run()", "r = Desk.legacy()"),
        "query" => edit(&s, "bad == true <= 1", "bad == false <= 1"),
        _ => panic!("unknown point {kind}"),
    }
}
pub fn degradation() -> String {
    let s = crate::b_input::base();
    let before = "process Before { status = Probe.check(); if status == fail { c = Clerk.enter(); end done } else { f = Fast.run(); end done } }";
    let s = edit(&s, "process Flow {", "process After {");
    append(
        &s,
        &format!(
            "{before}\nconfiguration From {{ cut Scope; process Before }}\nconfiguration To {{ cut Scope; process After }}"
        ),
    )
}
pub fn unconnected(kind: &str, feature: &str) -> String {
    let s = if kind == "contest" {
        crate::contest_input::source("fixed")
    } else {
        r#"model Guard version "0.2" {
          type R = enum { ok }
          people [p]
          actor Desk { control policy; executors [p]; capability run() -> R {
            duration fixed 2min; result nondet { ok }
          } }
          process Flow { r = Desk.run(); end done }
          cut C { include [Desk] }
          arrivals fixed [0min]
          policy observable fifo tie_by case_id then call_order assign first_idle
          bounds { cases 1; live_tasks 4; clock 20min }
          simulation replications 16 seed 1 confidence 0.5
          verify reachable end.done
        }"#
        .into()
    };
    let s = match kind {
        "s1" => edit(
            &s,
            "arrivals fixed [0min]",
            r#"arrivals distribution basis "arrival assumption" { 1: fixed [0min] }"#,
        ),
        "retry" => edit(
            &edit(
                &s,
                "policy observable fifo tie_by case_id then call_order assign first_idle",
                "",
            ),
            "r = Desk.run();",
            "r = Desk.run() response_loss sequence [delivered, delivered] retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed;",
        ),
        "conform" => edit(
            &edit(
                &s,
                "verify reachable end.done",
                "contract Controls { at_most_once Desk.run.commit per case.id }",
            ),
            "duration fixed 2min;",
            "commits; duration fixed 2min;",
        ),
        "mutate" | "contest" => s,
        _ => panic!("unknown mode {kind}"),
    };
    let (person, marker, result) = if kind == "contest" {
        ("r", "result nondet { pass }", "pass")
    } else {
        ("p", "result nondet { ok }", "ok")
    };
    match feature {
        "old" => s,
        "qualification" => edit(
            &s,
            marker,
            &format!("qualified_executors [{person}]; {marker}"),
        ),
        "performance" => append(
            &edit(&s, marker, "outcomes performance P"),
            &format!(
                r#"kernel K {{ * -> {{ 1:{result} }} }}
            performance P basis "test assumption" {{ load none; executor {person} {{ kernel K }} }}"#
            ),
        ),
        _ => panic!("unknown feature {feature}"),
    }
}
pub fn refused_comparison(kind: &str, one_side: bool) -> String {
    let s = unconnected(kind, "qualification");
    let before = if one_side {
        "r = Desk.legacy(); end done"
    } else if kind == "retry" {
        "r = Desk.run() response_loss sequence [delivered, delivered] retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed; end done"
    } else {
        "r = Desk.run(); end done"
    };
    let s = if one_side {
        edit(
            &s,
            "actor Desk {",
            "actor Desk { capability legacy() -> R { duration fixed 2min; result nondet { ok } }",
        )
    } else {
        s
    };
    append(
        &s,
        &format!(
            "process Before {{ {before} }}\nconfiguration From {{ cut C; process Before }}\nconfiguration To {{ cut C; process Flow }}"
        ),
    )
}
pub fn suite(sources: &[(&str, String)]) -> PathBuf {
    let d = dir();
    let points: Vec<_> = sources
        .iter()
        .map(|(id, source)| {
            let name = format!("{id}.relca");
            fs::write(d.join(&name), source).unwrap();
            json!({"id":id,"model":name,"configuration":null,"scenario":null})
        })
        .collect();
    let v = json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"Explicit Q6c hand points, no implicit Cartesian product"},
        "points":points,"targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":null});
    let path = d.join("manifest.json");
    fs::write(&path, v.to_string()).unwrap();
    path
}
