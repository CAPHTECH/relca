//! Hand-derived mutation survivors, outside the closed qtl fixture population.
//! Calculation witnesses and mutation mapping: ../quantile_mutation_fixes.md.
#![allow(dead_code)]
use serde_json::{Value, json};

pub struct Case {
    pub id: &'static str,
    pub source: String,
    pub sampled: bool,
    pub verdict: &'static str,
    pub exit: i32,
    pub rows: Value,
}

pub fn edit(s: &str, old: &str, new: &str) -> String {
    assert_eq!(s.matches(old).count(), 1, "unique edit {old}");
    s.replacen(old, new, 1)
}
fn fixture(s: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(s)
            .join("model.relca"),
    )
    .unwrap()
}
fn range(t: &str) -> Value {
    json!({"lower":t,"upper":t,"basis":"exact","reason":null})
}
fn missing(reason: &str) -> Value {
    json!({"lower":null,"upper":null,"basis":null,"reason":reason})
}
fn sample(point: Option<&str>, n: u64, c: u64, reason: Option<&str>) -> Value {
    json!({"point":point,"observations":n,"censored":c,"reason":reason})
}
fn row(name: &str, p: &str, given: bool, value: Value, lb: Option<&str>) -> Value {
    json!({"name":name,"p":p,"property":"Done","form":if given {"reached_only"} else {"all_arrivals"},"value":value,"lower_bound":lb})
}
pub fn live(arrivals: bool) -> String {
    format!(
        r#"model QL version "0.2" {{
  type T = enum {{ ok }}
  actor A {{ capacity 1; capability run() -> T {{ duration fixed 2min; result nondet {{ ok }} }} }}
  actor B {{ capacity 1; capability run() -> T {{ duration fixed 2min; result nondet {{ ok }} }} }}
  process P {{ fork f {{ a = A.run(); b = B.run() }} join all f; end done }}
  cut C {{ include [A, B] }}
  {}
  bounds {{ cases 1; live_tasks 1; clock 10min }}
  simulation replications 1 seed 1 confidence 0.5
  verify reachable end.done
  property case.Done = reached(end.done)
  metric quantile.Median = quantile(0.5, case.Done)
}}
"#,
        if arrivals {
            "policy fifo tie_by case_id"
        } else {
            ""
        }
    )
}
pub fn arithmetic() -> String {
    let third = "0.333333333333333333333333333333333333333333333333333333333333333";
    let rest = "0.666666666666666666666666666666666666666666666666666666666666667";
    let calls = (0..19)
        .map(|i| format!("x{i} = A.c(); if x{i} == yes {{}} else {{ end done }}; "))
        .collect::<String>();
    format!(
        r#"model Digits version "0.2" {{
  type V = enum {{ yes, no }}
  latent case {{ hit: bool = false }}
  kernel k {{ * -> {{ {third}: yes; {rest}: no }} }}
  kernel last {{ * -> {{ {third}: yes with hit = true; {rest}: no with hit = false }} }}
  actor A {{ capacity 1; capability c() -> V {{ duration fixed 1min; outcomes kernel k }}; capability l() -> V {{ duration fixed 1min; outcomes kernel last; writes hit }} }}
  actor Z {{ capacity 0; capability z() -> V {{ duration fixed 1min; result nondet {{ yes }} }} }}
  process Flow {{ {calls}z = A.l(); if z == yes {{ end done }} else {{ w = Z.z(); end done }} }}
  cut C {{ include [A, Z] }}
  bounds {{ cases 1; live_tasks 1; clock 30min }}
  verify reachable end.done
  property case.All = reached(hit == true)
  metric quantile.AllMedian = quantile(0.5, case.All given reached)
}}
"#
    )
}

pub const CLOSED_CUT: &str = r#"model RC version "0.2" {
  type V = enum { pass, fail }
  actor A { capacity 1; capability check() -> V { duration fixed 2min; result nondet { fail } } }
  process P { v = A.check(); if v == pass { end done } else { end returned } }
  cut C { include [A] }
  arrivals fixed [0min, 5min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 1; clock 6min }
  verify reachable end.done
  property case.Done = reached(end.done)
  metric quantile.Q25 = quantile(0.25, case.Done)
  metric quantile.Q75 = quantile(0.75, case.Done)
  metric quantile.Q50Done = quantile(0.5, case.Done given reached)
}
"#;

pub fn cases() -> Vec<Case> {
    let mut out = vec![];
    let within = edit(&fixture("qtl-within"), "within 4min", "within 3min");
    let within = edit(
        &within,
        "  bounds",
        "  policy fifo tie_by case_id\n  bounds",
    );
    out.push(Case {
        id: "f16a",
        source: within,
        sampled: false,
        verdict: "satisfied",
        exit: 0,
        rows: json!([
            row("Median", "1/2", false, range("2"), None),
            row("P95", "19/20", false, missing("beyond_reached_mass"), None),
            row("P95Done", "19/20", true, range("2"), None)
        ]),
    });
    for arrivals in [false, true] {
        for sampled in [false, true] {
            out.push(Case {
                id: match (arrivals, sampled) {
                    (false, false) => "f18l",
                    (false, true) => "f18s1",
                    (true, false) => "f18r",
                    (true, true) => "f18s",
                },
                source: live(arrivals),
                sampled,
                verdict: "inconclusive",
                exit: 2,
                rows: json!([row(
                    "Median",
                    "1/2",
                    false,
                    if sampled {
                        sample(None, 1, 1, Some("truncated"))
                    } else {
                        missing("truncated")
                    },
                    Some("0")
                )]),
            });
        }
    }
    let short = fixture("qtl-sample-sim.broken-short-clock");
    out.push(Case {
        id: "f19sa_censor_never",
        source: edit(&short, "  bounds", "  policy fifo tie_by case_id\n  bounds"),
        sampled: true,
        verdict: "inconclusive",
        exit: 2,
        rows: json!([
            row("Median", "1/2", false, sample(Some("2"), 4, 2, None), None),
            row(
                "P95",
                "19/20",
                false,
                sample(None, 4, 2, Some("truncated")),
                Some("4")
            )
        ]),
    });
    let offset = edit(
        &fixture("qtl-offset"),
        "  bounds",
        "  simulation replications 1 seed 1 confidence 0.5\n  bounds",
    );
    out.push(Case {
        id: "f25s",
        source: offset,
        sampled: true,
        verdict: "satisfied",
        exit: 0,
        rows: json!([
            row("Median", "1", false, sample(Some("6"), 2, 0, None), None),
            row("MedianDone", "1", true, sample(Some("6"), 2, 0, None), None)
        ]),
    });
    let initial = edit(
        &fixture("qtl-initial"),
        "  policy fifo tie_by case_id\n",
        "",
    );
    out.push(Case {
        id: "f24i",
        source: initial,
        sampled: false,
        verdict: "satisfied",
        exit: 0,
        rows: json!([
            row("Median", "1/2", false, range("0"), None),
            row("MedianDone", "1/2", true, range("0"), None)
        ]),
    });
    let deadlock = edit(
        &fixture("qtl-deadlock.broken-zero-capacity"),
        "  policy fifo tie_by case_id\n",
        "",
    );
    out.push(Case {
        id: "f40b",
        source: deadlock,
        sampled: false,
        verdict: "violated",
        exit: 1,
        rows: json!([
            row("Median", "1/2", false, missing("beyond_reached_mass"), None),
            row("MedianDone", "1/2", true, missing("zero_denominator"), None)
        ]),
    });
    for arrivals in [false, true] {
        let s = edit(&fixture("qtl-within"), "clock 10min", "clock 3min");
        out.push(Case {
            id: if arrivals {
                "within_censor_arrivals"
            } else {
                "within_censor_legacy"
            },
            source: if arrivals {
                edit(&s, "  bounds", "  policy fifo tie_by case_id\n  bounds")
            } else {
                s
            },
            sampled: false,
            verdict: "inconclusive",
            exit: 2,
            rows: json!([
                row("Median", "1/2", false, range("2"), None),
                row("P95", "19/20", false, missing("truncated"), Some("4")),
                row("P95Done", "19/20", true, missing("truncated"), Some("2"))
            ]),
        });
    }
    for sampled in [false, true] {
        let s = fixture(if sampled {
            "qtl-sample-sim"
        } else {
            "qtl-identifiable"
        });
        out.push(Case {
            id: if sampled {
                "float_p_sample"
            } else {
                "float_p_exhaustive"
            },
            source: edit(&s, "quantile(0.5,", "quantile(0.50000000000000001,"),
            sampled,
            verdict: if sampled { "inconclusive" } else { "satisfied" },
            exit: if sampled { 2 } else { 0 },
            rows: json!([
                row(
                    "Median",
                    "50000000000000001/100000000000000000",
                    false,
                    if sampled {
                        sample(Some("4"), 4, 0, None)
                    } else {
                        range("4")
                    },
                    None
                ),
                row(
                    "P95",
                    "19/20",
                    false,
                    if sampled {
                        sample(Some("4"), 4, 0, None)
                    } else {
                        range("4")
                    },
                    None
                )
            ]),
        });
    }
    out.push(Case {
        id: "never_censor_reference",
        source: CLOSED_CUT.into(),
        sampled: false,
        // PRE-C7 baseline-relca also returns V/1: case1 cannot reach done.
        verdict: "violated",
        exit: 1,
        rows: json!([
            row("Q25", "1/4", false, missing("truncated"), Some("2")),
            row("Q75", "3/4", false, missing("beyond_reached_mass"), None),
            row("Q50Done", "1/2", true, missing("truncated"), None)
        ]),
    });
    out
}
