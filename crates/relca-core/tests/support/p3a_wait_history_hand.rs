//! Fixed two-case interval oracles; adapters only construct actual collector input.
use serde_json::{Value, json};
pub fn source(kind: &str) -> String {
    let (people, a, b, uses) = match kind {
        "actor" => ("", "capacity 1", "capacity 1", ""),
        "executor" => ("people [p]", "executors [p]", "executors [p]", ""),
        "resource" => (
            "resource Desk capacity 1",
            "capacity 1",
            "capacity 1",
            "uses Desk:1;",
        ),
        _ => panic!(),
    };
    let blocker = if kind == "actor" { "A" } else { "B" };
    format!(
        r#"model HandHistory version "0.2" {{ type T=enum{{ok}} {people}
      actor A {{ {a}; capability run()->T {{ {uses} duration fixed 1min; result nondet{{ok}} }} }}
      actor B {{ {b}; capability run()->T {{ {uses} duration fixed 1min; result nondet{{ok}} }} }}
      process Flow {{ a1=A.run(); wait 3min; a2=A.run(); blocker={blocker}.run(); end done }}
      arrivals fixed [0min,4min] policy fifo tie_by case_id
      bounds {{cases 2; live_tasks 3; clock 20min}}
      metrics {{window 20min; currency "JPY"; breakdown waits}}
      verify safety no_resource_overcommit }}"#
    )
}
pub fn check(v: &Value, kind: &str, delay: u64, stop: &str) {
    let scalar = |n: u64| json!({"min":n.to_string(),"max":n.to_string(),"reason":null});
    let terminal = &v["cases"][0]["terminals"][0];
    if stop == "truncated" {
        assert_eq!(
            terminal["breakdown"]["in_service"],
            json!({"min":null,"max":null,"reason":"truncated"})
        );
        assert!(v["cases"][0].get("dwell").is_none());
        return;
    }
    let task = v["cases"][0]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["binding"] == "a2")
        .unwrap();
    assert_eq!(task["wait"], scalar(delay));
    for key in [
        "actor_capacity",
        "executor",
        "resource",
        "combined",
        "unattributed",
    ] {
        assert_eq!(
            task["wait_causes"][key],
            scalar(if key == kind { delay } else { 0 }),
            "{kind}/{key}"
        );
    }
    if stop == "unknown" {
        assert_eq!(terminal["lead_time"]["reason"], "no_terminal");
        assert_eq!(terminal["breakdown"]["in_service"]["reason"], "no_terminal");
        assert!(
            v["cases"][0].get("dwell").is_none(),
            "unknown is not horizon"
        );
    } else if stop == "horizon" {
        assert_eq!(v["cases"][0]["dwell"]["bound"], "lower");
        assert_eq!(v["cases"][0]["dwell"]["breakdown"]["in_service"], scalar(2));
    } else {
        assert_eq!(terminal["lead_time"], scalar(5 + delay));
        assert_eq!(terminal["breakdown"]["in_service"], scalar(2));
        assert_eq!(terminal["breakdown"]["intentional"], scalar(3));
        assert_eq!(terminal["breakdown"]["queue"][kind], scalar(delay));
        assert!(v["cases"][0].get("dwell").is_none());
    }
}
