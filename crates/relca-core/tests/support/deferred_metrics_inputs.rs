//! Hand-only ADR0053 inputs. Arithmetic was fixed in deferred_metrics_calculations.md.
#![allow(dead_code)]
use serde_json::{Value, json};

pub const NAMES: [&str; 12] = [
    "none",
    "actor",
    "capability",
    "opaque",
    "unrelated",
    "quality_only",
    "no_indicators",
    "dead_branch",
    "two_caps",
    "duplicate",
    "truncated",
    "unused",
];
pub const NEW_INLINE_CASES: [&str; 19] = [
    "none",
    "actor",
    "capability",
    "opaque",
    "unrelated",
    "quality_only",
    "no_indicators",
    "dead_branch",
    "two_caps",
    "duplicate",
    "truncated",
    "unused",
    "relabeled_capability",
    "shared_person",
    "static_cut_error",
    "simulation_refusal",
    "compare_open",
    "compare_hard",
    "compare_control",
];
pub fn relabeled_source() -> String {
    source("capability")
        .replace("actor A {", "actor Unit { grain document;")
        .replace("A.x", "Unit.x")
        .replace("include [A,R]", "include [Unit,R]")
        .replace("rate A per_minute", "rate Unit per_minute")
}
pub fn shared_person_source() -> String {
    source("actor").replace("actor R { executors [q]", "actor R { executors [p]")
}
pub fn static_error_source() -> String {
    source("actor").replace("include [A,R]", "include [A]")
}
pub fn refusal_source() -> String {
    source("actor").replace("result nondet { pass }", "result nondet { pass, fail }")
}
pub fn comparison_control() -> String {
    comparison_source(false).replace("defer R reason \"later\" loses \"load\"", "")
}
pub fn all_sources() -> Vec<(&'static str, String)> {
    let mut rows = NAMES
        .into_iter()
        .map(|n| (n, source(n)))
        .collect::<Vec<_>>();
    rows.extend([
        ("relabeled_capability", relabeled_source()),
        ("shared_person", shared_person_source()),
        ("static_cut_error", static_error_source()),
        ("simulation_refusal", refusal_source()),
        ("compare_open", comparison_source(false)),
        ("compare_hard", comparison_source(true)),
        ("compare_control", comparison_control()),
    ]);
    rows
}
pub fn source(name: &str) -> String {
    assert!(NAMES.contains(&name), "closed inline population: {name}");
    let defer = match name {
        "none" => "",
        "capability" | "two_caps" => "defer R.y reason \"later\" loses \"load\"",
        "opaque" => "defer NoSuchLayer.rates reason \"unrelated\" loses \"nothing\"",
        "duplicate" => {
            "defer R reason \"later\" loses \"load\"; defer R.y reason \"later\" loses \"load\""
        }
        _ => "defer R reason \"later\" loses \"load\"",
    };
    let body = match name {
        "unrelated" | "unused" => "a = A.x(); end done",
        "dead_branch" => "a = A.x(); if a == fail { r = R.y(); end other } else { end done }",
        "two_caps" => "a = A.x(); r = R.y(); z = R.z(); end done",
        _ => "a = A.x(); r = R.y(); end done",
    };
    let metrics = if ["quality_only", "no_indicators"].contains(&name) {
        ""
    } else {
        "metrics { window 12min; currency \"JPY\"; rate A per_minute 10; rate R per_minute 20; rate A.x per_start 1; rate R.y per_start 2; rate R.z per_start 4; breakdown waits }"
    };
    let quality = if name == "no_indicators" {
        ""
    } else {
        "property case.Done = reached(end.done)\nmetric cohort.Rate = expected(count(case.Done)) / arrivals\nmetric quantile.P95 = quantile(0.95, case.Done)"
    };
    let clock = if name == "truncated" { 4 } else { 12 };
    format!(
        r#"model Deferred version "0.2" {{
      type V = enum {{ pass, fail }}
      people [p, q]
      resource Desk capacity 1
      actor A {{ executors [p]; capability x() -> V {{ duration fixed 3min; result nondet {{ pass }}; cancel forbidden_after_start }} }}
      actor R {{ executors [q]; capability y() -> V {{ uses Desk: 1; duration fixed 2min; result nondet {{ pass }}; cancel forbidden_after_start }} capability z() -> V {{ duration fixed 4min; result nondet {{ pass }}; cancel forbidden_after_start }} }}
      cut C {{ include [A,R]; {defer} }}
      process P {{ {body} }}
      {metrics}
      {quality}
      bounds {{ cases 1; live_tasks 3; clock {clock}min }}
      simulation replications 3 seed 7 confidence 0.5
      verify reachable end.done
    }}"#
    )
}
pub fn scalar(n: &str, sim: bool) -> Value {
    if sim {
        json!({"mean":n,"min":n,"max":n,"samples":3,"reason":null})
    } else {
        json!({"min":n,"max":n,"reason":null})
    }
}
pub fn absent(reason: &str, sim: bool) -> Value {
    if sim {
        json!({"mean":null,"min":null,"max":null,"samples":0,"reason":reason})
    } else {
        json!({"min":null,"max":null,"reason":reason})
    }
}
pub fn row<'a>(rows: &'a Value, key: &str, name: &str) -> &'a Value {
    let found: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r.get(key).expect("declared row key must exist") == name)
        .collect();
    assert_eq!(found.len(), 1, "{key}={name}");
    found[0]
}
pub fn assert_metrics(m: &Value, name: &str, sim: bool) {
    if ["quality_only", "no_indicators"].contains(&name) {
        assert!(m.is_null());
        return;
    }
    assert_eq!(m["window"], 12);
    assert_eq!(m["currency"], "JPY");
    assert_eq!(row(&m["rates"]["actors"], "actor", "A")["per_minute"], 10);
    assert_eq!(row(&m["rates"]["actors"], "actor", "R")["per_minute"], 20);
    for (cap, price) in [("A.x", 1), ("R.y", 2), ("R.z", 4)] {
        assert_eq!(
            row(&m["rates"]["capabilities"], "capability", cap)["per_start"],
            price
        );
    }
    let affected = !["none", "unrelated", "unused"].contains(&name);
    let reason = if name == "truncated" {
        "truncated"
    } else {
        "not_configured"
    };
    let a = row(&m["actors"], "actor", "A");
    assert_eq!(
        a["work"],
        if affected {
            absent(reason, sim)
        } else {
            scalar("3", sim)
        }
    );
    assert_eq!(
        a["occupancy_cost"],
        if affected {
            absent(reason, sim)
        } else {
            scalar("30", sim)
        }
    );
    if ["unrelated", "unused"].contains(&name) {
        assert!(
            !m["actors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["actor"] == "R"),
            "defer must not invent an uncalled Actor row"
        );
    } else {
        let r = row(&m["actors"], "actor", "R");
        assert_eq!(
            r["work"],
            if name == "none" {
                scalar("2", sim)
            } else {
                absent(reason, sim)
            }
        );
    }
    assert_eq!(
        m["overall"]["work"],
        if affected {
            absent(reason, sim)
        } else {
            scalar(if name == "none" { "5" } else { "3" }, sim)
        }
    );
    assert_eq!(
        m["overall"]["observed_cost_total"],
        if affected {
            absent(reason, sim)
        } else {
            scalar(if name == "none" { "73" } else { "31" }, sim)
        }
    );
    assert_eq!(
        row(&m["cases"][0]["terminals"], "end", "done")["lead_time"],
        if affected {
            absent(reason, sim)
        } else {
            scalar(if name == "none" { "5" } else { "3" }, sim)
        }
    );
    if name == "dead_branch" {
        // The lexical first end.other is unreachable: U3 preserves its
        // applicability reason instead of replacing it with the Cut reason.
        assert_eq!(
            row(&m["cases"][0]["terminals"], "end", "other")["lead_time"],
            absent(if sim { "no_samples" } else { "no_terminal" }, sim)
        );
    }
    assert_eq!(m["overall"]["counts"]["offered"]["stats"], scalar("1", sim));
    assert_eq!(
        row(&m["capabilities"], "capability", "A.x")["start_cost"],
        if affected {
            absent(reason, sim)
        } else {
            scalar("1", sim)
        }
    );
    assert_eq!(
        row(&m["people"], "person", "p")["work"],
        if affected {
            absent(reason, sim)
        } else {
            scalar("3", sim)
        }
    );
    assert_eq!(
        row(&m["people"], "person", "q")["work"],
        if name == "none" {
            scalar("2", sim)
        } else {
            absent(reason, sim)
        }
    );
    assert_eq!(
        row(&m["resources"], "resource", "Desk")["occupancy_time"],
        if name == "none" {
            scalar("2", sim)
        } else {
            absent(reason, sim)
        }
    );
    let task_a = row(&m["cases"][0]["tasks"], "binding", "a");
    assert_eq!(
        task_a["work"],
        if affected {
            absent(reason, sim)
        } else {
            scalar("3", sim)
        }
    );
    assert_eq!(
        task_a["wait"],
        if affected {
            absent(reason, sim)
        } else {
            scalar("0", sim)
        }
    );
    assert_eq!(task_a["starts"], scalar("1", sim));
    if name == "two_caps" {
        assert_eq!(
            row(&m["cases"][0]["tasks"], "binding", "z")["work"],
            absent(reason, sim)
        );
        assert_eq!(
            row(&m["capabilities"], "capability", "R.z")["start_cost"],
            absent(reason, sim)
        );
    }
}
pub fn assert_quality(q: &Value, name: &str, sim: bool) {
    if name == "no_indicators" {
        assert!(q.is_null());
        return;
    }
    let affected = !["none", "unrelated", "unused"].contains(&name);
    let reason = if name == "truncated" {
        "truncated"
    } else {
        "not_configured"
    };
    let value = &q["cohort"][0]["value"];
    if affected {
        assert_eq!(value["reason"], reason);
        for key in if sim {
            &["point"][..]
        } else {
            &["lower", "upper", "basis"][..]
        } {
            assert!(value[*key].is_null(), "{key}: {value}");
        }
        assert_eq!(q["quantiles"][0]["value"]["reason"], reason);
        assert!(q["quantiles"][0]["lower_bound"].is_null());
        if !sim {
            assert_eq!(q["properties"][0]["cases"][0]["reason"], reason);
            assert!(q["properties"][0]["cases"][0]["min"].is_null());
        } else {
            assert!(q["properties"][0]["cases"][0].get("reason").is_none());
            assert_eq!(q["properties"][0]["cases"][0]["trials"], 3);
        }
        assert_eq!(q["properties"][0]["expected_count"]["reason"], reason);
    } else {
        assert_eq!(value[if sim { "point" } else { "lower" }], "1");
        assert_eq!(value["reason"], Value::Null);
        assert_eq!(
            q["quantiles"][0]["value"][if sim { "point" } else { "lower" }],
            if name == "none" { "5" } else { "3" }
        );
    }
}
pub fn assert_report(v: &Value, name: &str, sim: bool) {
    assert_metrics(&v["metrics"], name, sim);
    assert_quality(&v["quality"], name, sim);
    let applied = !["none", "no_indicators"].contains(&name);
    assert_eq!(
        v["cut"]["checked"]
            .as_array()
            .unwrap()
            .contains(&json!("deferred_metrics")),
        applied
    );
    assert_eq!(
        v["cut"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("deferred_metrics")),
        !applied
    );
    if applied {
        assert_eq!(
            v["cut"]["checked"].as_array().unwrap().last(),
            Some(&json!("deferred_metrics"))
        );
    }
    if !sim && name != "truncated" {
        assert_eq!(v["verdict"], "satisfied");
    }
}

pub fn comparison_source(hard: bool) -> String {
    let s = source("actor");
    let i = s.rfind('}').unwrap();
    format!(
        "{}\nprocess Q {{ a = A.x(); end done }}\nconfiguration Included {{ cut C; process P }}\nconfiguration Unrelated {{ cut C; process Q }}\ncompare [Included,Unrelated] cut C {} objectives {{ minimize metrics.overall.work; minimize cohort.Rate; minimize quantile.P95 }}\n{}",
        &s[..i],
        if hard {
            "hard { cohort.Rate >= 0.1 }"
        } else {
            ""
        },
        &s[i..]
    )
}
pub fn assert_comparison(v: &Value, hard: bool, sim: bool) {
    assert_eq!(v["plans"][0]["configuration"], "Included");
    assert_report(&v["plans"][0]["report"], "actor", sim);
    assert_report(&v["plans"][1]["report"], "unrelated", sim);
    for row in v["evaluation"]["objectives"].as_array().unwrap() {
        assert_eq!(row["values"][0]["reason"], "not_configured");
        assert!(row["values"][0]["lower"].is_null());
        assert_eq!(row["values"][1]["reason"], Value::Null);
    }
    if hard {
        assert_eq!(
            v["evaluation"]["hard"][0]["values"][0]["verdict"],
            "inconclusive"
        );
        assert_eq!(
            v["evaluation"]["hard"][0]["values"][0]["reason"],
            "not_configured"
        );
        assert!(v["evaluation"]["hard"][0]["values"][0]["interval"].is_null());
        assert_eq!(
            v["evaluation"]["hard"][0]["values"][1]["verdict"],
            "satisfied"
        );
    } else if !sim {
        assert_eq!(
            v["evaluation"]["pareto"]["non_dominated"],
            json!(["Included", "Unrelated"])
        );
        assert_eq!(
            v["evaluation"]["pareto"]["undetermined_pairs"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
    let work = v["tables"]["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["key"]["scope"] == "overall"
                && r["key"].get("metric").expect("MetricKey.metric") == "work"
        })
        .unwrap();
    assert_eq!(work["values"][0]["value"], absent("not_configured", sim));
    assert_eq!(work["values"][1]["value"], scalar("3", sim));
}
