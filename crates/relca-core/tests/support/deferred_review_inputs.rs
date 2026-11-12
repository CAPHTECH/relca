//! ADR0053 review rulings A/B. Hand expectations: deferred_review_calculations.md.
#![allow(dead_code)]
use serde_json::{Value, json};

pub const QUALITY_CASES: [&str; 5] = [
    "truncated",
    "undetermined",
    "zero_denominator",
    "denominator_may_be_zero",
    "beyond_reached_mass",
];
pub const DEPENDENCY_CASES: [&str; 3] = ["contest", "shared_resource", "shared_person"];
pub fn scenario(name: &str) -> Option<&'static str> {
    (name == "undetermined").then_some("Day")
}
pub fn source(name: &str, deferred: bool) -> String {
    assert!(QUALITY_CASES.contains(&name));
    let cut = if deferred {
        "defer A reason \"later\" loses \"quality and time\""
    } else {
        ""
    };
    let result = if ["denominator_may_be_zero", "beyond_reached_mass"].contains(&name) {
        "pass, fail"
    } else {
        "pass"
    };
    let result_clause = format!("result nondet {{ {result} }}");
    let denominator = if ["zero_denominator", "denominator_may_be_zero"].contains(&name) {
        "expected(count(case.Never))"
    } else {
        "arrivals"
    };
    let property = if name == "beyond_reached_mass" {
        "Never"
    } else {
        "Done"
    };
    let bounds = if name == "undetermined" {
        "scenario Day { arrivals fixed [0min]; horizon 1min; bounds { cases 1; live_tasks 1; clock 5min } }"
    } else if name == "truncated" {
        "bounds { cases 1; live_tasks 1; clock 1min }"
    } else {
        "bounds { cases 1; live_tasks 1; clock 5min }"
    };
    format!(
        r#"model ReviewQuality version "0.2" {{
      type V = enum {{ pass, fail }}
      actor A {{ capacity 1; capability go() -> V {{ duration fixed 2min; {result_clause} }} }}
      process P {{ a = A.go(); if a == pass {{ end done }} else {{ end never }} }}
      process Q {{ a = A.go(); if a == pass {{ end done }} else {{ end never }} }}
      cut C {{ include [A]; {cut} }}
      property case.Done = reached(end.done)
      property case.Never = reached(end.never)
      metric cohort.Rate = expected(count(case.Done)) / {denominator}
      metric quantile.Q = quantile(0.95, case.{property})
      configuration Left {{ cut C; process P }}
      configuration Right {{ cut C; process Q }}
      compare [Left,Right] cut C hard {{ cohort.Rate >= 0; cohort.Rate >= 0.1 }} objectives {{ minimize cohort.Rate; minimize quantile.Q }}
      {bounds}
      simulation replications 3 seed 7 confidence 0.5
      verify safety no_resource_overcommit
    }}"#
    )
}
pub fn clock_fixture(deferred: bool) -> String {
    let src = include_str!("../../../../fixtures/qual-clock.broken-short-clock/model.relca");
    let src = if deferred {
        src.replace(
            "include [Checker, Ledger]",
            "include [Checker, Ledger]; defer Checker reason \"later\" loses \"quality\"",
        )
    } else {
        src.to_owned()
    };
    let i = src.rfind('}').unwrap();
    let other = src
        .lines()
        .find(|l| l.trim_start().starts_with("process Flow"))
        .unwrap()
        .replace("process Flow", "process Other");
    format!(
        "{} {other} configuration Left {{ cut C; process Flow }} configuration Right {{ cut C; process Other }} compare [Left,Right] cut C hard {{ cohort.WrongPerArrival >= 0 }} objectives {{ minimize cohort.WrongPerArrival }} {}",
        &src[..i],
        &src[i..]
    )
}
pub fn dependency_source(name: &str, deferred: bool) -> String {
    assert!(DEPENDENCY_CASES.contains(&name));
    if name == "contest" {
        let src = include_str!("../../../../fixtures/ctx-contest-metrics/model.relca")
            .replace("Judge", "A")
            .replace("judge", "go")
            .replace("Route", "Review")
            .replace("respond()", "reply()")
            .replace(".respond", ".reply")
            .replace("rate Review per_minute 1;", "rate Review per_minute 10;");
        return if deferred {
            src.replace(
                "include [A,Review,Party]",
                "include [A,Review,Party]; defer Review reason \"later\" loses \"route\"",
            )
        } else {
            src
        };
    }
    let defer = if deferred {
        "defer R reason \"later\" loses \"time\""
    } else {
        ""
    };
    let executor = if name == "shared_person" { "p" } else { "q" };
    let uses = if name == "shared_resource" {
        "uses Desk: 1;"
    } else {
        ""
    };
    let a_uses = if name == "shared_resource" {
        "uses Desk: 1, Solo: 1;"
    } else {
        "uses Solo: 1;"
    };
    format!(
        r#"model ReviewDependency version "0.2" {{
      type V = enum {{ pass }}
      people [p,q]
      resource Desk capacity 1
      resource Solo capacity 1
      actor A {{ executors [p]; capability x() -> V {{ {a_uses} duration fixed 1min; result nondet {{ pass }} }} }}
      actor R {{ executors [{executor}]; capability y() -> V {{ {uses} duration fixed 2min; result nondet {{ pass }} }} }}
      process P {{ fork f {{ a = A.x(); r = R.y() }} join all f; end done }}
      cut C {{ include [A,R]; {defer} }}
      metrics {{ window 10min; currency "JPY"; rate A per_minute 1; rate R per_minute 10; rate A.x per_start 0; rate R.y per_start 0; breakdown waits }}
      bounds {{ cases 1; live_tasks 2; clock 10min }}
      verify safety no_resource_overcommit
    }}"#
    )
}
pub fn row<'a>(rows: &'a Value, key: &str, name: &str) -> &'a Value {
    let rows: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r[key] == name)
        .collect();
    assert_eq!(rows.len(), 1, "{key}={name}");
    rows[0]
}
pub fn assert_null_quantity(v: &Value) {
    let o = v.as_object().expect("quantity exists");
    assert!(o.get("reason").is_some_and(|r| !r.is_null()), "reason: {v}");
    for k in ["min", "max", "mean", "lower", "upper", "point", "basis"] {
        if let Some(x) = o.get(k) {
            assert!(x.is_null(), "{k}: {v}");
        }
    }
    if o.contains_key("mean") {
        assert_eq!(v["samples"], 0);
    }
    for k in ["numerator", "denominator"] {
        if let Some(x) = o.get(k).filter(|x| x.is_object()) {
            assert_null_quantity(x);
        }
    }
}
pub fn assert_quality(q: &Value, name: &str, sim: bool, deferred: bool) {
    let reason = match name {
        "truncated" | "undetermined" | "zero_denominator" | "denominator_may_be_zero" => name,
        _ => "not_configured",
    };
    let c = &q["cohort"][0];
    if deferred {
        assert_eq!(c["value"]["reason"], reason);
        assert_null_quantity(&c["value"]);
        assert!(
            c["bounds_with_undetermined"].is_null(),
            "no deferred fallback interval: {c}"
        );
        for p in q["properties"].as_array().unwrap() {
            assert_null_quantity(&p["expected_count"]);
            if sim {
                assert_eq!(p["cases"][0]["trials"], 3);
                assert!(p["cases"][0].get("reason").is_none());
            } else {
                for case in p["cases"].as_array().unwrap() {
                    assert_null_quantity(case);
                }
            }
        }
        for o in q["outcomes_per_arrival"].as_array().unwrap() {
            assert_null_quantity(&o["expected_count"]);
            assert_null_quantity(&o["rate"]);
        }
        let quantile = &q["quantiles"][0];
        assert_null_quantity(&quantile["value"]);
        assert!(quantile["lower_bound"].is_null());
        if ["truncated", "undetermined", "beyond_reached_mass"].contains(&name) {
            assert_eq!(quantile["value"]["reason"], name);
        }
    } else {
        if ["truncated", "undetermined"].contains(&name) {
            assert_eq!(c["value"]["reason"], name);
            assert_eq!(
                c["bounds_with_undetermined"],
                json!({"lower":"0","upper":"1"})
            );
            if !sim {
                assert_eq!(q["properties"][0]["expected_count"]["lower"], "0");
                assert_eq!(q["properties"][0]["expected_count"]["upper"], "1");
            } else {
                assert_eq!(
                    c["value"]["numerator"],
                    json!({"mean":"0","min":"0","max":"0","samples":3,"reason":null})
                );
                assert_eq!(
                    c["value"]["denominator"],
                    json!({"mean":"1","min":"1","max":"1","samples":3,"reason":null})
                );
            }
        } else if ["zero_denominator", "denominator_may_be_zero"].contains(&name) {
            assert_eq!(c["value"]["reason"], name);
            if sim {
                assert_eq!(
                    c["value"]["numerator"],
                    json!({"mean":"1","min":"1","max":"1","samples":3,"reason":null})
                );
                assert_eq!(
                    c["value"]["denominator"],
                    json!({"mean":"0","min":"0","max":"0","samples":3,"reason":null})
                );
            }
        } else {
            assert_eq!(
                c["value"][if sim { "point" } else { "lower" }],
                if name == "beyond_reached_mass" {
                    "0"
                } else {
                    "1"
                }
            );
        }
        if name == "beyond_reached_mass" {
            assert_eq!(q["quantiles"][0]["value"]["reason"], name);
            assert_eq!(q["quantiles"][0]["lower_bound"], "2");
        }
    }
}
pub fn assert_comparison(v: &Value, name: &str, sim: bool, deferred: bool) {
    assert_eq!(v["plans"].as_array().expect("plans").len(), 2, "{v}");
    for p in v["plans"].as_array().unwrap() {
        assert_quality(&p["report"]["quality"], name, sim, deferred);
    }
    if !deferred {
        return;
    }
    for h in v["evaluation"]["hard"].as_array().expect("hard") {
        for c in h["values"].as_array().unwrap() {
            assert_eq!(c["verdict"], "inconclusive", "no reconstructed hard: {c}");
            assert!(c["interval"].is_null());
            assert!(c["basis"].is_null());
            assert_eq!(
                c["reason"],
                v["plans"][0]["report"]["quality"]["cohort"][0]["value"]["reason"]
            );
            assert!(c["source_reason"].is_null());
        }
    }
    for o in v["evaluation"]["objectives"].as_array().unwrap() {
        for (j, c) in o["values"].as_array().unwrap().iter().enumerate() {
            assert_null_quantity(c);
            let q = &v["plans"][j]["report"]["quality"];
            let source = if o["objective"] == "minimize cohort.Rate" {
                &q["cohort"][0]["value"]
            } else {
                &q["quantiles"][0]["value"]
            };
            assert_eq!(
                c["reason"], source["reason"],
                "objective must preserve source provenance"
            );
        }
    }
    for r in v["tables"]["quality"].as_array().unwrap() {
        if r["key"]["field"] == "unknown_count" {
            continue;
        }
        for c in r["values"].as_array().unwrap() {
            let x = &c["value"];
            if r["key"]["field"] == "bounds_with_undetermined" {
                assert!(x.is_null());
            } else if x.is_object() {
                assert_null_quantity(x);
            }
        }
    }
}
fn range(a: &str, b: &str) -> Value {
    json!({"min":a,"max":b,"reason":null})
}
pub fn assert_dependency(m: &Value, name: &str, deferred: bool) {
    let a = row(&m["actors"], "actor", "A");
    let cap = if name == "contest" { "A.go" } else { "A.x" };
    if !deferred {
        assert_eq!(a["work"], range("1", "1"));
        assert_eq!(a["occupancy_cost"], range("1", "1"));
        assert_eq!(
            row(&m["capabilities"], "capability", cap)["start_cost"],
            range("0", "0")
        );
        let (work, cost) = if name == "contest" {
            (range("1", "3"), range("1", "21"))
        } else {
            (range("3", "3"), range("21", "21"))
        };
        assert_eq!(m["overall"]["work"], work);
        assert_eq!(m["overall"]["observed_cost_total"], cost);
        assert_eq!(
            m["cases"][0]["terminals"][0]["lead_time"],
            if name == "contest" {
                range("1", "1")
            } else {
                range("3", "3")
            }
        );
        if name != "contest" {
            assert_eq!(a["queue"]["wait_total"]["stats"], range("0", "2"));
            assert_eq!(a["queue"]["length_max"], range("0", "1"));
            assert_eq!(
                a["queue"]["length_time_average"]["stats"],
                range("0", "2/3")
            );
            assert_eq!(
                row(&m["resources"], "resource", "Solo")["occupancy_time"],
                range("1", "1")
            );
            assert_eq!(
                row(&m["resources"], "resource", "Desk")["occupancy_time"],
                if name == "shared_resource" {
                    range("3", "3")
                } else {
                    range("0", "0")
                }
            );
        }
        return;
    }
    for k in [
        "work",
        "observed_cost_total",
        "observed_until",
        "named_people_work",
        "peak_tasks",
        "occupancy_cost",
        "start_cost",
        "observed_cost_per_arrival",
    ] {
        assert_null_quantity(&m["overall"][k]);
    }
    for a in m["actors"].as_array().unwrap() {
        assert_null_quantity(&a["work"]);
        assert_null_quantity(&a["occupancy_cost"]);
        for k in ["peak_tasks", "utilization"] {
            assert_null_quantity(&a[k]);
        }
        if let Some(q) = a.get("queue") {
            assert_null_quantity(&q["wait_total"]["stats"]);
            assert_null_quantity(&q["length_max"]);
            assert_null_quantity(&q["length_time_average"]["stats"]);
        }
    }
    for p in m["people"].as_array().unwrap() {
        assert_null_quantity(&p["work"]);
        assert_null_quantity(&p["peak_tasks"]);
    }
    for c in m["capabilities"].as_array().unwrap() {
        assert_null_quantity(&c["start_cost"]);
    }
    if let Some(rs) = m["resources"].as_array() {
        for r in rs {
            for k in ["occupancy_time", "peak_units", "utilization"] {
                assert_null_quantity(&r[k]);
            }
        }
    }
    for c in m["cases"].as_array().unwrap() {
        for k in ["work", "observed_cost_total", "first_start_wait"] {
            assert_null_quantity(&c[k]);
        }
        for t in c["terminals"].as_array().unwrap() {
            assert_null_quantity(&t["lead_time"]);
            assert_null_quantity(&t["cost_per_case"]);
        }
        for t in c["tasks"].as_array().unwrap() {
            assert_null_quantity(&t["work"]);
            assert_null_quantity(&t["wait"]);
            if let Some(causes) = t["wait_causes"].as_object() {
                for x in causes.values() {
                    assert_null_quantity(x);
                }
            }
        }
    }
}
