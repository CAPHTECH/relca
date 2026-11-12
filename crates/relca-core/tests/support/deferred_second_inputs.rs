//! Hand expectations are in deferred_second_calculations.md, never harvested.
#![allow(dead_code)]
use serde_json::{Value, json};
pub const NAMES: [&str; 3] = ["immediate", "horizon", "unfinished_route"];
pub fn scenario(name: &str) -> Option<&'static str> {
    ["horizon", "unfinished_route"]
        .contains(&name)
        .then_some("Day")
}
pub fn source(name: &str, deferred: bool) -> String {
    if ["unfinished_route", "if_none"].contains(&name) {
        let mut s = include_str!("../../../../fixtures/ctx-contest-metrics/model.relca")
            .replace("Judge", "A")
            .replace("judge", "x")
            .replace("Route", "Review")
            .replace("respond()", "reply()")
            .replace(".respond", ".reply")
            .replace("rate Review per_minute 1;", "rate Review per_minute 10;");
        if name == "unfinished_route" {
            s = s.replace("duration fixed 2min", "duration fixed 5min")
                .replace("bounds { cases 1; live_tasks 2; clock 10min }", "scenario Day { arrivals fixed [0min]; horizon 2min; bounds { cases 1; live_tasks 2; clock 10min } }");
        } else {
            s = s.replace("process Flow { d = A.x(); if d == fail { end returned } else { end posted } }", "process Flow { fork f { d = A.x() } join any f where result == pass tie by [d] remaining drain if none -> returned; end posted }");
            s = s.replace("cut C {", "process EndDeclaration { end returned } configuration Left { cut C; process Flow } cut C {");
        }
        if deferred {
            s = s.replace(
                "include [A,Review,Party]",
                &format!(
                    "include [A,Review,Party]; defer {} reason \"later\" loses \"time\"",
                    if name == "if_none" { "Review" } else { "A" }
                ),
            );
        }
        return s;
    }
    assert!(["immediate", "horizon"].contains(&name));
    let process = if name == "immediate" {
        "fork f { a = A.x(); r = R.y() } join any f where result == pass tie by [r,a] remaining cancel_then_drain if none -> done; end done"
    } else {
        "r = R.y(); a = A.x(); end done"
    };
    let bounds = if name == "horizon" {
        "scenario Day { arrivals fixed [0min]; horizon 3min; bounds { cases 1; live_tasks 2; clock 10min } }"
    } else {
        "bounds { cases 1; live_tasks 2; clock 10min }"
    };
    let defer = if deferred {
        "defer R reason \"later\" loses \"time\""
    } else {
        ""
    };
    format!(
        r#"model SecondReview version "0.2" {{
      type V = enum {{ pass, fail }}
      people [p,q]
      resource Solo capacity 1
      actor A {{ executors [p]; capability x() -> V {{ uses Solo: 1; duration fixed 5min; result nondet {{ fail }}; cancel immediate }} }}
      actor R {{ executors [q]; capability y() -> V {{ duration fixed 2min; result nondet {{ pass }}; cancel immediate }} }}
      process P {{ {process} }}
      cut C {{ include [A,R]; {defer} }}
      metrics {{ window 10min; currency "JPY"; rate A per_minute 1; rate R per_minute 10; rate A.x per_start 3; rate R.y per_start 4; breakdown waits }}
      {bounds}
      verify safety no_resource_overcommit
    }}"#
    )
}
fn range(a: &str, b: &str) -> Value {
    json!({"min":a,"max":b,"reason":null})
}
pub fn assert_control(m: &Value, name: &str) {
    let a = m["actors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["actor"] == "A")
        .unwrap();
    if name == "unfinished_route" {
        let route = m["actors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["actor"] == "Review")
            .unwrap();
        assert_eq!(route["work"], range("0", "1"));
        assert_eq!(route["occupancy_cost"], range("0", "10"));
        assert_eq!(m["overall"]["observed_cost_total"], range("1", "11"));
    } else {
        let (work, cost, total) = if name == "immediate" {
            ("2", "29", "4")
        } else {
            ("1", "28", "3")
        };
        assert_eq!(a["work"], range(work, work));
        assert_eq!(a["occupancy_cost"], range(work, work));
        assert_eq!(m["overall"]["observed_cost_total"], range(cost, cost));
        assert_eq!(m["overall"]["work"], range(total, total));
        assert_eq!(
            m["cases"][0]["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["binding"] == "a")
                .unwrap()["starts"],
            range("1", "1")
        );
    }
}
// Every Scalar is classified by its path, not Actor name or observed value.
// Only count Scalars may retain numbers. Unknown Scalar fields fail closed.
pub fn assert_all_quantities(m: &Value) {
    fn walk(v: &Value, path: &str, quantities: &mut usize, counts: &mut usize) {
        if let Some(o) = v.as_object() {
            if o.contains_key("min") && o.contains_key("max") && o.contains_key("reason") {
                let parts: Vec<_> = path.split('/').collect();
                let field = *parts.last().unwrap();
                let parent = parts[parts.len() - 2];
                let count = ["starts", "refused_count"].contains(&field)
                    || field == "stats" && !["wait_total", "length_time_average"].contains(&parent);
                if count {
                    *counts += 1;
                } else {
                    assert!(
                        [
                            "work",
                            "occupancy_cost",
                            "start_cost",
                            "peak_tasks",
                            "utilization",
                            "occupancy_time",
                            "peak_units",
                            "wait",
                            "predecessor_wait",
                            "refusal_wait",
                            "first_start_wait",
                            "external_wait",
                            "intentional_wait",
                            "observed_cost_total",
                            "observed_cost_per_arrival",
                            "cost_per_case",
                            "lead_time",
                            "observed_until",
                            "named_people_work",
                            "queue_wait_observed",
                            "join_wait",
                            "length_max",
                            "stats",
                            "until",
                            "external",
                            "in_service",
                            "contest_route",
                            "actor_capacity",
                            "resource",
                            "executor",
                            "combined",
                            "unattributed",
                            "intentional"
                        ]
                        .contains(&field),
                        "unclassified {path}"
                    );
                    for key in ["min", "max", "mean"] {
                        if let Some(x) = o.get(key) {
                            assert!(x.is_null(), "numeric time/cost {path}/{key}: {v}");
                        }
                    }
                    assert!(o["reason"].is_string(), "null without reason {path}");
                    if o.contains_key("mean") {
                        assert_eq!(o["samples"], 0);
                    }
                    *quantities += 1;
                }
                return;
            }
            for (key, x) in o {
                walk(x, &format!("{path}/{key}"), quantities, counts);
            }
        } else if let Some(xs) = v.as_array() {
            for (i, x) in xs.iter().enumerate() {
                walk(x, &format!("{path}/{i}"), quantities, counts);
            }
        }
    }
    let (mut quantities, mut counts) = (0, 0);
    walk(m, "", &mut quantities, &mut counts);
    assert!(
        quantities > 20 && counts > 5,
        "must exercise whole metric inventory"
    );
}

pub fn assert_preserved_structure_and_counts(control: &Value, deferred: &Value) {
    fn erase_quantities(v: &mut Value, path: &str) {
        if let Some(o) = v.as_object_mut() {
            if o.contains_key("min") && o.contains_key("max") && o.contains_key("reason") {
                let parts: Vec<_> = path.split('/').collect();
                let field = *parts.last().unwrap();
                let parent = parts[parts.len() - 2];
                let count = ["starts", "refused_count"].contains(&field)
                    || field == "stats" && !["wait_total", "length_time_average"].contains(&parent);
                if !count {
                    *v = json!("time-or-cost");
                }
                return;
            }
            for (key, x) in o {
                erase_quantities(x, &format!("{path}/{key}"));
            }
        } else if let Some(xs) = v.as_array_mut() {
            for (i, x) in xs.iter_mut().enumerate() {
                erase_quantities(x, &format!("{path}/{i}"));
            }
        }
    }
    let mut old = control.clone();
    let mut new = deferred.clone();
    erase_quantities(&mut old, "");
    erase_quantities(&mut new, "");
    assert_eq!(
        old, new,
        "all count cells, static fields, rows and ordering stay exact"
    );
}
