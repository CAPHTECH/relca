//! Hand-only cross-feature expectations; see quantile_onto_calculations.md.
#![allow(dead_code)]
#[path = "quantile_integration_inputs.rs"]
pub mod i;
use serde_json::{Value, json};

pub const WAIT_UNCHECKED: [&str; 12] = [
    "quality",
    "business_value",
    "queue_length_quantiles",
    "quantiles",
    "exact_expectations",
    "metric_constraints",
    "metric_intervals",
    "anonymous_human_work",
    "calendar",
    "rework_loss",
    "introduction_cost",
    "terminal_meaning",
];
pub const OBJECTIVES: [&str; 10] = [
    "qtl-objective",
    "qtl-objective.broken-no-policy",
    "qtl-pareto",
    "qtl-pareto.broken-pricier-parallel",
    "qtl-missing-rate",
    "qtl-missing-rate.broken-missing-rate",
    "qtl-null-objective",
    "qtl-null-objective.broken-pass-only",
    "qtl-interval-objective",
    "qtl-interval-objective.broken-fifo",
];
pub fn wait_source(route: &str) -> String {
    let s = i::deferred("mutation-F35-after-A7")["source"]
        .as_str()
        .unwrap()
        .to_owned();
    match route {
        "normal" => s,
        "refusal" => i::h::replace(&s, "  policy fifo tie_by case_id\n", ""),
        "late_abort" => i::h::replace(
            &i::h::replace(&s, "enum { ok }", "enum { ok, other }"),
            "result nondet { ok }",
            "result nondet { ok, other }",
        ),
        "missing_clause" => i::h::replace(
            &s,
            "  simulation replications 1 seed 1 confidence 0.5\n",
            "",
        ),
        "structural" => i::h::replace(&s, "verify reachable end.done", "verify terminating end"),
        _ => panic!("closed waiting route {route}"),
    }
}
pub fn fields(v: &Value) -> Vec<&str> {
    v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["field"].as_str().unwrap())
        .collect()
}
pub fn scheduler_source() -> String {
    i::b6::append(
        &wait_source("refusal"),
        "latent case { coin: bool ~ { false:0.5, true:0.5 } }\nverify probability reached coin == true <= 1",
    )
}
pub fn assert_scheduler(v: &Value) {
    let fs = fields(v);
    let indices: Vec<_> = [
        "quality.scheduler",
        "metrics.waiting",
        "quality.quantile",
        "probability.scheduler",
    ]
    .iter()
    .map(|f| {
        fs.iter()
            .position(|x| x == f)
            .expect("non-vacuous scheduler row")
    })
    .collect();
    assert!(indices.windows(2).all(|p| p[0] < p[1]));
    assert_eq!(
        v["quality"]["quantiles"][0]["value"],
        json!({"lower":"3","upper":"6","basis":"per_case_relaxation","reason":null})
    );
}
pub fn assert_wait(v: &Value, sim: bool, route: &str) {
    let fs = fields(v);
    let pos = |f: &str| {
        let xs: Vec<_> = fs
            .iter()
            .enumerate()
            .filter(|(_, x)| **x == f)
            .map(|(n, _)| n)
            .collect();
        assert_eq!(xs.len(), 1, "one {f}: {fs:?}");
        xs[0]
    };
    let old = [
        "quality.definition",
        "quality.denominator",
        "quality.form",
        if sim {
            "quality.estimation"
        } else {
            "quality.scheduler"
        },
    ];
    let ps: Vec<_> = old.iter().map(|f| pos(f)).collect();
    assert!(ps.windows(2).all(|p| p[0] + 1 == p[1]));
    let q = pos("quality.quantile");
    let metrics = !matches!(route, "missing_clause" | "structural");
    if metrics {
        let w = pos("metrics.waiting");
        assert!(ps[3] < w && w < q, "old -> waiting -> quantile: {fs:?}");
        assert!(v["metrics"].is_object());
        assert_eq!(v["metrics"]["unchecked"], json!(WAIT_UNCHECKED));
    } else {
        assert!(ps[3] < q);
        assert!(!fs.contains(&"metrics.waiting"));
        assert!(v.get("metrics").is_none());
    }
    if fs.contains(&"probability.scheduler") {
        assert!(q < pos("probability.scheduler"));
    }
    for (n, f) in fs.iter().enumerate() {
        if !matches!(
            *f,
            "quality.quantile" | "metrics.waiting" | "probability.scheduler"
        ) {
            assert!(
                n < q && (!metrics || n < pos("metrics.waiting")),
                "all old rows precede waiting/quantile"
            );
        }
    }
    let row = &v["quality"]["quantiles"][0];
    assert_eq!(row["name"], "Median");
    if route == "normal" {
        assert_eq!(v["verdict"], "satisfied");
        assert_eq!(
            row["value"],
            if sim {
                json!({"point":"3","observations":2,"censored":0,"reason":null})
            } else {
                i::point(Some("3"), None)
            }
        );
    } else {
        assert!(matches!(
            v["verdict"].as_str(),
            Some("invalid" | "unsupported")
        ));
        assert_eq!(row["value"]["reason"], "aborted");
        assert!(row["lower_bound"].is_null());
        if sim {
            assert!(row["value"]["point"].is_null());
        }
    }
}
pub fn assert_old(v: &Value, control: &Value) {
    assert_eq!(
        i::h::old_projection(v.clone()),
        i::h::old_projection(control.clone())
    );
}
pub fn compare_source(name: &str) -> (String, Vec<String>) {
    let d = i::deferred(name);
    (
        d["source"].as_str().unwrap().into(),
        d["invocation"]["plans"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_str().unwrap().into())
            .collect(),
    )
}
fn cell(configuration: &str, lower: Value, upper: Value, basis: Value, reason: Value) -> Value {
    json!({"configuration":configuration,"lower":lower,"upper":upper,"basis":basis,"reason":reason})
}
fn exact(c: &str, n: &str) -> Value {
    cell(c, json!(n), json!(n), json!("exact"), Value::Null)
}
fn metric_range(c: &str, n: &str) -> Value {
    cell(c, json!(n), json!(n), json!("path_range"), Value::Null)
}
pub fn expected_objectives(name: &str) -> Value {
    let d = i::deferred(name);
    let (_, plans) = compare_source(name);
    let mut rows = vec![];
    let q = &d["semantic_oracle"]["quantiles"];
    let qname = if q.is_object() { "Median" } else { "Lead" };
    let vals: Vec<_> = plans
        .iter()
        .enumerate()
        .map(|(j, c)| {
            if q.is_object() {
                let v = &q[c][0]["value"];
                cell(
                    c,
                    v["lower"].clone(),
                    v["upper"].clone(),
                    v["basis"].clone(),
                    v["reason"].clone(),
                )
            } else {
                exact(c, if j == 0 { "8" } else { "5" })
            }
        })
        .collect();
    rows.push(
        json!({"objective":format!("minimize quantile.{qname}"),"direction":"minimize","values":vals}),
    );
    if qname == "Lead" {
        let vals: Vec<_> = plans
            .iter()
            .enumerate()
            .map(|(j, c)| {
                if name == "qtl-missing-rate.broken-missing-rate" {
                    cell(
                        c,
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        json!("not_configured"),
                    )
                } else {
                    metric_range(
                        c,
                        if j == 1 && name == "qtl-pareto.broken-pricier-parallel" {
                            "1102"
                        } else {
                            "805"
                        },
                    )
                }
            })
            .collect();
        rows.push(json!({"objective":"minimize metrics.overall.observed_cost_total","direction":"minimize","values":vals}));
    }
    json!(rows)
}
pub fn assert_objective(v: &Value, name: &str) {
    let d = i::deferred(name);
    assert_eq!(
        v["verdict"], d["expected"]["verdict"],
        "objective input {name}: {v}"
    );
    let e = &v["evaluation"];
    assert!(e.is_object(), "real B7 evaluation required {name}: {v}");
    assert_eq!(
        e["objectives"],
        expected_objectives(name),
        "actual B7 objective envelope, not LB"
    );
    let (_, plans) = compare_source(name);
    for (j, c) in plans.iter().enumerate() {
        assert_eq!(e["plans"][j]["configuration"], json!(c));
        assert_eq!(
            e["plans"][j]["status"],
            if name == "qtl-objective.broken-no-policy" && j == 0 {
                "undetermined"
            } else {
                "admissible"
            }
        );
        let expected_q = &d["semantic_oracle"]["quantiles"][c];
        if expected_q.is_array() {
            assert_eq!(v["plans"][j]["report"]["quality"]["quantiles"], *expected_q);
        }
    }
    let frontier = match name {
        "qtl-objective" => json!(["One"]),
        "qtl-objective.broken-no-policy" => json!(["Two"]),
        "qtl-pareto" | "qtl-missing-rate" => json!(["ToBeP"]),
        "qtl-pareto.broken-pricier-parallel" | "qtl-missing-rate.broken-missing-rate" => {
            json!(["AsIs", "ToBeP"])
        }
        "qtl-null-objective" | "qtl-interval-objective" => json!(["Maybe", "Sure"]),
        _ => json!(["Maybe"]),
    };
    assert_eq!(e["pareto"]["non_dominated"], frontier);
    assert_eq!(e["pareto"]["status"], "computed");
    assert_eq!(
        e["pareto"]["among"],
        if name == "qtl-objective.broken-no-policy" {
            json!(["Two"])
        } else {
            json!(plans)
        }
    );
    let pairs = match name {
        "qtl-missing-rate.broken-missing-rate" => {
            json!([{"from":"AsIs","to":"ToBeP","reason":"missing_value"}])
        }
        "qtl-null-objective" => json!([{"from":"Maybe","to":"Sure","reason":"missing_value"}]),
        "qtl-interval-objective" => {
            json!([{"from":"Maybe","to":"Sure","reason":"overlapping_intervals"}])
        }
        _ => json!([]),
    };
    assert_eq!(e["pareto"]["undetermined_pairs"], pairs);
    assert_eq!(
        e["pareto"]["not_evaluated"],
        if name == "qtl-objective.broken-no-policy" {
            json!([{"configuration":"One","status":"undetermined"}])
        } else {
            json!([])
        }
    );
    if name.starts_with("qtl-objective") {
        assert_eq!(e["hard"][0]["cohort"], "OnTimePerArrival");
        assert_eq!(
            e["hard"][0]["values"][0]["verdict"],
            if name.ends_with("no-policy") {
                "inconclusive"
            } else {
                "satisfied"
            }
        );
    } else {
        assert_eq!(e["hard"], json!([]));
    }
}
pub fn assert_hard_refusal(v: &Value) {
    assert_eq!(v["verdict"], "unsupported");
    assert!(v.get("evaluation").is_none());
    let mut found = false;
    for p in v["plans"].as_array().unwrap() {
        found |= p["report"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| {
                d["code"] == "U-SYN-0"
                    && d["message"] == "hard constraints apply to cohort ratios only"
            });
    }
    assert!(found, "B7 exact U-SYN-0 must survive: {v}");
}
pub fn rule_source() -> String {
    let s = i::rule_legacy();
    let flow = s
        .lines()
        .find(|l| l.starts_with("process Flow "))
        .unwrap()
        .replace("process Flow ", "process Flow2 ");
    let s = i::h::replace(
        &s,
        "configuration ToBeB { cut C; process Flow",
        "configuration ToBeB { cut C; process Flow2",
    );
    i::b6::append(
        &s,
        &format!(
            "{flow}\nmetric cohort.RequiredRate = expected(count(case.Required)) / arrivals\ncompare [AsIs, ToBeB] cut C hard {{ cohort.RequiredRate >= 0.5 }} objectives {{ minimize cohort.RequiredRate }}"
        ),
    )
}
pub fn existing_objective_source() -> String {
    i::h::replace(
        i::deferred("qtl-pareto.broken-pricier-parallel")["source"]
            .as_str()
            .unwrap(),
        "minimize quantile.Lead",
        "minimize metrics.overall.work",
    )
}
pub fn assert_existing_objective(v: &Value) {
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(
        v["evaluation"]["objectives"],
        json!([
        {"objective":"minimize metrics.overall.work","direction":"minimize","values":[metric_range("AsIs","8"),metric_range("ToBeP","8")]},
        {"objective":"minimize metrics.overall.observed_cost_total","direction":"minimize","values":[metric_range("AsIs","805"),metric_range("ToBeP","1102")]}
        ])
    );
    assert_eq!(v["evaluation"]["pareto"]["non_dominated"], json!(["AsIs"]));
    for (j, t) in ["8", "5"].iter().enumerate() {
        assert_eq!(
            v["plans"][j]["report"]["quality"]["quantiles"][0]["value"],
            i::point(Some(t), None)
        );
    }
}
pub fn assert_rule(v: &Value, sim: bool) {
    assert_eq!(v["verdict"], if sim { "inconclusive" } else { "violated" });
    for (j, c) in ["AsIs", "ToBeB"].iter().enumerate() {
        assert_eq!(v["plans"][j]["configuration"], *c);
        i::assert_rule_report(&v["plans"][j]["report"], sim, j == 0);
        assert_eq!(
            v["evaluation"]["hard"][0]["values"][j]["verdict"],
            if sim {
                "inconclusive"
            } else if j == 0 {
                "satisfied"
            } else {
                "violated"
            },
            "Rule hard/quantile crossing: {v}"
        );
        assert_eq!(
            v["evaluation"]["plans"][j]["status"],
            if sim {
                "undetermined"
            } else if j == 0 {
                "admissible"
            } else {
                "excluded"
            }
        );
        let objective = &v["evaluation"]["objectives"][0]["values"][j];
        if v["evaluation"]["objectives"][0]["objective"] == "minimize quantile.All" {
            assert_eq!(
                objective["reason"],
                if j == 0 {
                    Value::Null
                } else {
                    json!("beyond_reached_mass")
                }
            );
            continue;
        }
        assert_eq!(objective["reason"], Value::Null);
        if sim {
            assert_eq!(objective["point"], if j == 0 { "1" } else { "0" });
        } else {
            assert_eq!(objective["lower"], if j == 0 { "1" } else { "0" });
            assert_eq!(objective["upper"], objective["lower"]);
        }
    }
}

// Review additions: hand arithmetic is fixed in quantile_onto_calculations.md.
pub fn review_source(name: &str) -> String {
    let (state, property, metric, arrivals, clock, horizon, objectives) = match name {
        "sample_censor" => (
            "",
            "reached(end.done)",
            "metric quantile.Median = quantile(0.5, case.Done)\nmetric quantile.P95 = quantile(0.95, case.Done)",
            "0min, 0min",
            3,
            "",
            "minimize quantile.Median; minimize quantile.P95",
        ),
        "zero" | "reached" => (
            if name == "zero" {
                "observed case { hit: bool = false }"
            } else {
                "observed case { hit: bool = true }"
            },
            "reached(hit == true)",
            "metric quantile.Given = quantile(0.5, case.Done given reached)",
            "0min",
            10,
            "",
            "minimize quantile.Given",
        ),
        "maybe" => (
            "latent case { hit: bool }",
            "reached(hit == true)",
            "metric quantile.Given = quantile(0.5, case.Done given reached)",
            "0min",
            10,
            "",
            "minimize quantile.Given",
        ),
        "horizon" => (
            "",
            "reached(end.done)",
            "metric quantile.All = quantile(0.5, case.Done)\nmetric quantile.Given = quantile(0.5, case.Done given reached)",
            "0min",
            10,
            "horizon 1min",
            "minimize quantile.All; minimize quantile.Given",
        ),
        _ => panic!("closed review source {name}"),
    };
    let population = format!(
        "arrivals fixed [{arrivals}]\n{horizon}\nbounds {{ cases {}; live_tasks 2; clock {clock}min }}",
        if name == "sample_censor" { 2 } else { 1 }
    );
    let population = if name == "horizon" {
        format!("scenario Day {{ {population} }}")
    } else {
        population
    };
    let under = if name == "horizon" { "under [Day]" } else { "" };
    format!(
        r#"model Review version "0.2" {{
type T = enum {{ ok }}
{state}
actor A {{ capacity 1; capability run() -> T {{ duration fixed 2min; result nondet {{ ok }} }} }}
process P {{ job = A.run(); end done }}
process Q {{ job = A.run(); end done }}
cut C {{ include [A] }}
configuration One {{ cut C; process P }}
configuration Two {{ cut C; process Q }}
{population}
policy fifo tie_by case_id
simulation replications 1 seed 1 confidence 0.5
verify reachable end.done
property case.Done = {property}
{metric}
compare [One, Two] {under} cut C objectives {{ {objectives} }}
}}
"#
    )
}

pub fn objective_free(source: &str) -> String {
    let start = source.rfind("objectives {").unwrap();
    let end = start + source[start..].find('}').unwrap();
    format!(
        "{}{}{}",
        &source[..start],
        " ".repeat(end + 1 - start),
        &source[end + 1..]
    )
}

pub fn assert_objective_noninterference(v: &Value, control: &Value) {
    assert_eq!(v["verdict"], control["verdict"]);
    assert_eq!(v["diagnostics"], control["diagnostics"]);
    for j in 0..2 {
        let mut original = v["plans"][j]["report"].clone();
        let mut masked = control["plans"][j]["report"].clone();
        original["model"]["sha256"] = Value::Null;
        masked["model"]["sha256"] = Value::Null;
        assert_eq!(original, masked);
    }
}

fn assert_review_pareto(v: &Value, sim: bool, admissible: bool, missing: bool) {
    let p = &v["evaluation"]["pareto"];
    for j in 0..2 {
        assert_eq!(
            v["evaluation"]["plans"][j]["status"],
            if admissible {
                "admissible"
            } else {
                "undetermined"
            }
        );
    }
    assert_eq!(
        p["among"],
        if admissible {
            json!(["One", "Two"])
        } else {
            json!([])
        }
    );
    assert_eq!(
        p["not_evaluated"],
        if admissible {
            json!([])
        } else {
            json!([{"configuration":"One","status":"undetermined"},{"configuration":"Two","status":"undetermined"}])
        }
    );
    assert_eq!(p["dominated"], json!([]));
    assert_eq!(
        p["status"],
        if !sim && admissible {
            "computed"
        } else {
            "not_computed"
        }
    );
    assert_eq!(
        p["reason"],
        if sim {
            json!("simulation_estimates")
        } else if admissible {
            Value::Null
        } else {
            json!("no_admissible_plans")
        }
    );
    assert_eq!(
        p["non_dominated"],
        if !sim && admissible {
            json!(["One", "Two"])
        } else {
            json!([])
        }
    );
    assert_eq!(
        p["undetermined_pairs"],
        if !sim && admissible && missing {
            json!([{"from":"One","to":"Two","reason":"missing_value"}])
        } else {
            json!([])
        }
    );
}

pub fn assert_review(v: &Value, name: &str, sim: bool) {
    let censored = matches!(name, "sample_censor" | "horizon");
    assert_eq!(
        v["verdict"],
        if censored {
            "inconclusive"
        } else {
            "satisfied"
        },
        "review {name}/{sim}: {v}"
    );
    let reason = match name {
        "sample_censor" => Some("truncated"),
        "zero" => Some("zero_denominator"),
        "maybe" => Some("denominator_may_be_zero"),
        "horizon" => Some("undetermined"),
        "reached" => None,
        _ => unreachable!(),
    };
    let labels: &[&str] = match name {
        "sample_censor" => &["Median", "P95"],
        "horizon" => &["All", "Given"],
        _ => &["Given"],
    };
    let mut expected = Vec::new();
    for (n, label) in labels.iter().enumerate() {
        let finite = if name == "sample_censor" && n == 0 {
            Some("2")
        } else if name == "reached" {
            Some("0")
        } else {
            None
        };
        let why = if finite.is_some() { None } else { reason };
        let mut cells = Vec::new();
        for (j, configuration) in ["One", "Two"].iter().enumerate() {
            let value = if sim {
                json!({"point":finite,"observations":if name == "sample_censor" { 2 } else { 1 },"censored":u64::from(censored),"reason":why})
            } else {
                i::point(finite, why)
            };
            let lb = match (name, n) {
                ("sample_censor", 1) => json!("4"),
                ("horizon", 0) => json!("2"),
                _ => Value::Null,
            };
            let q = &v["plans"][j]["report"]["quality"]["quantiles"][n];
            assert_eq!(q["name"], *label);
            assert_eq!(q["value"], value, "nested {name}/{sim}/{label}");
            assert_eq!(q["lower_bound"], lb);
            cells.push(if sim {
                json!({"configuration":configuration,"point":finite,"reason":why})
            } else {
                cell(
                    configuration,
                    json!(finite),
                    json!(finite),
                    if finite.is_some() {
                        json!("exact")
                    } else {
                        Value::Null
                    },
                    json!(why),
                )
            });
        }
        expected.push(json!({"objective":format!("minimize quantile.{label}"),"direction":"minimize","values":cells}));
    }
    assert_eq!(
        v["evaluation"]["objectives"],
        json!(expected),
        "value, never lower_bound"
    );
    assert_eq!(v["evaluation"]["hard"], json!([]));
    assert_review_pareto(v, sim, !censored, reason.is_some());
}

pub fn rule_quantile_source() -> String {
    i::h::replace(
        &rule_source(),
        "objectives { minimize cohort.RequiredRate }",
        "objectives { minimize quantile.All }",
    )
}

pub fn assert_rule_quantile(v: &Value, sim: bool) {
    assert_rule(v, sim);
    let cells: Vec<_> = ["AsIs", "ToBeB"]
        .iter()
        .enumerate().map(|(j,c)| {
            if sim {
                json!({"configuration":c,"point":if j==0 {json!("0")}else{Value::Null},"reason":if j==0 {Value::Null}else{json!("beyond_reached_mass")}})
            } else {
                if j==0 {exact(c,"0")}else{cell(c, Value::Null, Value::Null, Value::Null, json!("beyond_reached_mass"))}
            }
        })
        .collect();
    assert_eq!(
        v["evaluation"]["objectives"],
        json!([{"objective":"minimize quantile.All","direction":"minimize","values":cells}])
    );
    let p = &v["evaluation"]["pareto"];
    assert_eq!(p["status"], if sim { "not_computed" } else { "computed" });
    assert_eq!(
        p["reason"],
        if sim {
            json!("simulation_estimates")
        } else {
            Value::Null
        }
    );
    assert_eq!(p["among"], if sim { json!([]) } else { json!(["AsIs"]) });
    assert_eq!(p["undetermined_pairs"], json!([]));
    assert_eq!(
        p["not_evaluated"],
        if sim {
            json!([{"configuration":"AsIs","status":"undetermined"},{"configuration":"ToBeB","status":"undetermined"}])
        } else {
            json!([{"configuration":"ToBeB","status":"excluded"}])
        }
    );
}

pub fn shared_pair(kind: &str) -> (&'static str, &'static str, bool) {
    match kind {
        "quantiles" => ("quantile.Q", "quantile.Q95", true),
        "numerator" => ("quantile.Q", "cohort.DoneRate", true),
        "denominator" => ("quantile.Q", "cohort.Inverse", true),
        "reversed" => ("cohort.Inverse", "quantile.Q", true),
        "different_quantiles" => ("quantile.Q", "quantile.OtherQ", false),
        "different_cohort" => ("quantile.Q", "cohort.OtherRate", false),
        "metric" => ("quantile.Q", "metrics.overall.work", false),
        _ => panic!("closed shared kind {kind}"),
    }
}

pub fn shared_source(kind: &str) -> String {
    let s = review_source("reached");
    let (a, b, _) = shared_pair(kind);
    let s = i::h::replace(
        &s,
        "property case.Done = reached(hit == true)",
        "property case.Done = reached(end.done)\nproperty case.Other = reached(hit == true)",
    );
    let s = i::h::replace(
        &s,
        "metric quantile.Given = quantile(0.5, case.Done given reached)",
        "metric quantile.Q = quantile(0.5, case.Done)\nmetric quantile.Q95 = quantile(0.95, case.Done)\nmetric quantile.OtherQ = quantile(0.5, case.Other)\nmetric cohort.DoneRate = expected(count(case.Done)) / arrivals\nmetric cohort.Inverse = expected(count(case.Other)) / expected(count(case.Done))\nmetric cohort.OtherRate = expected(count(case.Other)) / arrivals\nmetrics { window 10min; currency \"JPY\"; rate A per_minute 1; rate A.run per_start 0 }",
    );
    i::h::replace(
        &s,
        "minimize quantile.Given",
        &format!("minimize {a}; maximize {b}"),
    )
}

pub fn assert_shared(v: &Value, kind: &str, sim: bool) {
    assert_eq!(v["verdict"], "satisfied", "shared {kind}: {v}");
    assert_review_pareto(v, sim, true, false);
    let (a, b, shared) = shared_pair(kind);
    for (j, target) in [a, b].iter().enumerate() {
        let number = match *target {
            "quantile.Q" | "quantile.Q95" | "metrics.overall.work" => "2",
            "quantile.OtherQ" => "0",
            _ => "1",
        };
        let values: Vec<_> = ["One", "Two"]
            .iter()
            .map(|c| {
                if sim && *target == "metrics.overall.work" {
                    json!({"configuration":c,"sample":{"mean":number,"min":number,"max":number,"samples":1,"reason":null},"reason":null})
                } else if sim {
                    json!({"configuration":c,"point":number,"reason":null})
                } else if *target == "metrics.overall.work" {
                    metric_range(c, number)
                } else {
                    exact(c, number)
                }
            })
            .collect();
        let direction = if j == 0 { "minimize" } else { "maximize" };
        assert_eq!(
            v["evaluation"]["objectives"][j],
            json!({"objective":format!("{direction} {target}"),"direction":direction,"values":values})
        );
    }
    let base = "Non-dominated means not certainly dominated, not proved Pareto optimal. Objectives are not added. Simulation dominance is not computed. Shared observations: ";
    let want = if shared {
        format!("{base}({a}, {b})")
    } else {
        format!("{base}none among the declared objectives.")
    };
    let rows: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["field"] == "compare.pareto")
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["affects"], want, "closed pair {kind}");
}
