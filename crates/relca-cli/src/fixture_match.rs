//! Fixture-runner comparison: complete ADR 0002 wire first, then ADR 0003
//! partial expectations and negative diagnostic constraints. Included by the
//! integration runner so its self-tests exercise the very same entry point.

use serde_json::Value;
use std::collections::BTreeMap;
#[path = "../tests/support/ledger_shape.rs"]
mod ledger_shape;
#[allow(dead_code)]
pub fn original_report_fields(v: &Value) -> Value {
    ledger_shape::original_report(v)
}

/// ADR0044 appended closed population, never a prefix-based exemption.
#[allow(dead_code)]
pub fn rule_actor_fixture(name: &str) -> bool {
    serde_json::from_str::<Vec<String>>(include_str!(
        "../../relca-core/tests/support/rule_names.json"
    ))
    .expect("closed Rule population")
    .iter()
    .any(|n| n == name)
}

fn rule_amendment_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        ("from", Str),
        ("to", Str),
        ("by", Str),
        ("under", Str),
        ("under_version", Null(Box::new(Str))),
    ])
}
fn rule_actor_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        ("version", Obj(vec![("declared", Str), ("effective", Str)])),
        (
            "values",
            Arr(Box::new(Obj(vec![
                ("name", Str),
                ("declared", OneOf(vec![Bool, Str])),
                ("effective", OneOf(vec![Bool, Str])),
            ]))),
        ),
        ("amendment", Null(Box::new(rule_amendment_schema()))),
        ("read_by", Arr(Box::new(Str))),
    ])
}
fn rule_diff_cell_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        ("version", Str),
        (
            "values",
            Arr(Box::new(Obj(vec![
                ("name", Str),
                ("value", OneOf(vec![Bool, Str])),
            ]))),
        ),
        ("amendment", Null(Box::new(rule_amendment_schema()))),
    ])
}
fn rule_judgement_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        ("kind", Lit(&["rule"])),
        ("actor", Str),
        ("version", Str),
        ("reads", Arr(Box::new(Str))),
    ])
}
fn judgement_schema() -> Ty {
    use Ty::*;
    OneOf(vec![
        Obj(vec![
            ("call", Str),
            ("capability", Str),
            ("stops", Lit(&["all_paths", "conditional", "none"])),
            ("bypasses", Int),
        ]),
        rule_judgement_schema(),
    ])
}

/// Closed new shape, independent of production DTOs and inference.
#[allow(dead_code)]
pub fn validate_rule_judgement_wire(v: &Value) -> Result<(), String> {
    let ty = rule_judgement_schema();
    check(v, &ty, "rule judgement")?;
    complete(v, &ty, "rule judgement")?;
    let actor = v["actor"].as_str().unwrap();
    if actor.is_empty()
        || v["version"].as_str().unwrap().is_empty()
        || v["reads"].as_array().unwrap().is_empty()
    {
        return Err("empty Rule judgement identity".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for read in v["reads"].as_array().unwrap() {
        let s = read.as_str().unwrap();
        if !s.starts_with(&format!("{actor}."))
            || s.split('.').count() != 2
            || s.ends_with('.')
            || !seen.insert(s)
        {
            return Err("invalid/duplicate Rule reads".into());
        }
    }
    Ok(())
}

/// Rule values are bool/member strings only; nonempty ordered declaration
/// identity is preserved, no invented integer/record/default/mutable state.
#[allow(dead_code)]
pub fn validate_rule_actor_wire(v: &Value) -> Result<(), String> {
    let ty = rule_actor_schema();
    check(v, &ty, "actor.rule")?;
    complete(v, &ty, "actor.rule")?;
    if v["version"]["declared"].as_str().unwrap().is_empty()
        || v["version"]["effective"].as_str().unwrap().is_empty()
        || v["values"].as_array().unwrap().is_empty()
    {
        return Err("empty Rule version/values".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for r in v["values"].as_array().unwrap() {
        if r["name"].as_str().unwrap().is_empty()
            || !seen.insert(r["name"].as_str().unwrap())
            || r["declared"].is_boolean() != r["effective"].is_boolean()
            || r["declared"].as_str() == Some("")
            || r["effective"].as_str() == Some("")
        {
            return Err("invalid/duplicate Rule value".into());
        }
    }
    let mut reads = std::collections::BTreeSet::new();
    for read in v["read_by"].as_array().unwrap() {
        if read.as_str().unwrap().is_empty() || !reads.insert(read.as_str().unwrap()) {
            return Err("invalid/duplicate Rule read_by".into());
        }
    }
    if v["amendment"].is_null() {
        if v["version"]["declared"] != v["version"]["effective"]
            || v["values"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["declared"] != r["effective"])
        {
            return Err("unamended Rule differs from declaration".into());
        }
    } else if v["amendment"]["from"] != v["version"]["declared"]
        || v["amendment"]["to"] != v["version"]["effective"]
        || v["amendment"]["from"] == v["amendment"]["to"]
    {
        return Err("Rule amendment direction/version mismatch".into());
    }
    Ok(())
}

#[allow(dead_code)]
pub fn validate_rule_diff_cell_wire(v: &Value) -> Result<(), String> {
    let ty = rule_diff_cell_schema();
    check(v, &ty, "rule diff cell")?;
    complete(v, &ty, "rule diff cell")?;
    if v["version"].as_str().unwrap().is_empty() || v["values"].as_array().unwrap().is_empty() {
        return Err("empty Rule diff cell".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for r in v["values"].as_array().unwrap() {
        if r["name"].as_str().unwrap().is_empty()
            || !seen.insert(r["name"].as_str().unwrap())
            || r["value"].as_str() == Some("")
        {
            return Err("duplicate Rule diff value".into());
        }
    }
    Ok(())
}
#[path = "../tests/support/evaluation_wire.rs"]
mod evaluation_wire;
include!("../tests/support/quantile_fixture_table.rs");
include!("../tests/support/deferred_wire.rs");

/// ADR0039 appended closed population; unknown lim names are not ignored.
#[allow(dead_code)]
pub const LIMITS_FIXTURES: &[&str] = include!("../../relca-core/tests/support/limits_names.rs");
/// ADR 0040: appended closed population (not a prefix exemption).
#[allow(dead_code)]
pub fn effect_standing_fixture(name: &str) -> bool {
    serde_json::from_str::<Vec<String>>(include_str!(
        "../../relca-core/tests/support/effect_standing_names.json"
    ))
    .expect("closed effect Standing population")
    .iter()
    .any(|n| n == name)
}

fn quality_reason() -> Ty {
    Ty::Null(Box::new(Ty::Lit(&[
        "zero_denominator",
        "denominator_may_be_zero",
        "undetermined",
        "truncated",
        "aborted",
        "no_samples",
        "not_configured",
    ])))
}

fn quality_range() -> Ty {
    use Ty::*;
    Obj(vec![
        ("lower", Null(Box::new(MetricNumber))),
        ("upper", Null(Box::new(MetricNumber))),
        (
            "basis",
            Null(Box::new(Lit(&["exact", "per_case_relaxation"]))),
        ),
        ("reason", quality_reason()),
    ])
}

fn quality_sample() -> Ty {
    use Ty::*;
    Obj(vec![
        ("mean", Null(Box::new(MetricNumber))),
        ("min", Null(Box::new(MetricNumber))),
        ("max", Null(Box::new(MetricNumber))),
        ("samples", Int),
        ("reason", quality_reason()),
    ])
}

fn quality_bounds() -> Ty {
    use Ty::*;
    Null(Box::new(Obj(vec![
        ("lower", MetricNumber),
        ("upper", MetricNumber),
    ])))
}

fn quality_denominator() -> Ty {
    use Ty::*;
    OneOf(vec![
        Obj(vec![("kind", Lit(&["arrivals"])), ("count", Int)]),
        Obj(vec![
            ("kind", Lit(&["property"])),
            ("property", Str),
            ("covers", Null(Box::new(Arr(Box::new(Str))))),
            ("reason", Null(Box::new(Lit(&["state_condition"])))),
        ]),
    ])
}

fn quality_ratio(sampled: bool) -> Ty {
    use Ty::*;
    if !sampled {
        return quality_range();
    }
    Obj(vec![
        ("point", Null(Box::new(MetricNumber))),
        ("numerator", quality_sample()),
        ("denominator", quality_sample()),
        ("samples", Int),
        ("reason", quality_reason()),
    ])
}

fn quality_outcome_rate(sampled: bool) -> Ty {
    use Ty::*;
    if !sampled {
        return quality_range();
    }
    Obj(vec![
        ("point", Null(Box::new(MetricNumber))),
        ("samples", Int),
        ("reason", quality_reason()),
    ])
}

fn quality_schema(sampled: bool) -> Ty {
    use Ty::*;
    let expected = || {
        if sampled {
            quality_sample()
        } else {
            quality_range()
        }
    };
    let mut case = vec![("case_id", Int), ("arrival", Int)];
    if sampled {
        case.extend([("hits", Int), ("unknown", Int), ("trials", Int)]);
    } else {
        case.extend([
            ("min", Null(Box::new(RationalText))),
            ("max", Null(Box::new(RationalText))),
            ("reason", quality_reason()),
        ]);
    }
    let mut property = vec![
        ("name", Str),
        ("condition", Str),
        ("within", Null(Box::new(Int))),
        ("cases", Arr(Box::new(Obj(case)))),
        ("expected_count", expected()),
    ];
    if sampled {
        property.push(("unknown_count", quality_sample()));
    }
    let words = || {
        Arr(Box::new(Lit(&[
            "case_reachability",
            "expected_counts",
            "ratio_of_expected_counts",
            "denominator_disclosure",
            "outcomes_per_arrival",
            "deadline_attainment",
            "expectation_of_ratio",
            "cohort_any_probability",
            "metric_intervals",
            "metric_constraints",
            "numerator_subset_of_denominator",
            "business_value",
            "rework_loss",
            "quantiles",
        ])))
    };
    Obj(vec![
        ("rules", Lit(&["quality-cohort-p1-1"])),
        (
            "analysis",
            if sampled {
                Lit(&["simulation"])
            } else {
                Lit(&["exhaustive"])
            },
        ),
        ("properties", Arr(Box::new(Obj(property)))),
        (
            "cohort",
            Arr(Box::new(Obj(vec![
                ("name", Str),
                ("form", Lit(&["ratio_of_expected_counts"])),
                ("numerator", Str),
                ("denominator", quality_denominator()),
                ("value", quality_ratio(sampled)),
                ("bounds_with_undetermined", quality_bounds()),
            ]))),
        ),
        (
            "quantiles",
            OptionalField(Box::new(Arr(Box::new(quantile_schema(sampled))))),
        ),
        (
            "outcomes_per_arrival",
            Arr(Box::new(Obj(vec![
                (
                    "kind",
                    Lit(&[
                        "end",
                        "overflow",
                        "rejected",
                        "unfinished",
                        "deadlock",
                        "cut_off",
                    ]),
                ),
                ("end", Null(Box::new(Str))),
                ("expected_count", expected()),
                ("rate", quality_outcome_rate(sampled)),
            ]))),
        ),
        ("checked", words()),
        ("unchecked", words()),
    ])
}

/// One closed row form per (analysis, scope, field). The value
/// form is fixed by the row key, so a row is never accepted under another
/// value form (a union of value forms would let `complete` pick the first
/// form whose keys merely overlap). Only cohort rows carry `denominator`; the
/// outcome subject is the outcome kind and only outcome rows may name an end.
/// Comparing (`compare`) admits both analyses here and
/// `validate_quality_table` fixes the analysis; `diff` is exhaustive only.
fn quality_table_schema(comparing: bool) -> Ty {
    let modes: &[bool] = if comparing { &[false, true] } else { &[false] };
    Ty::Arr(Box::new(Ty::OneOf(
        modes
            .iter()
            .flat_map(|&sampled| quality_table_rows(comparing, sampled))
            .collect(),
    )))
}

fn quality_table_form(scope: &str, field: &str, sampled: bool) -> Ty {
    match (scope, field) {
        (_, "expected_count") if sampled => quality_sample(),
        (_, "expected_count") => quality_range(),
        ("property", "unknown_count") => quality_sample(),
        ("cohort", "value") => quality_ratio(sampled),
        ("cohort", "bounds_with_undetermined") => quality_bounds(),
        _ => quality_outcome_rate(sampled),
    }
}

fn quality_table_rows(comparing: bool, sampled: bool) -> Vec<Ty> {
    use Ty::*;
    let mut forms: Vec<(&'static [&'static str], &'static [&'static str])> =
        vec![(&["property"], &["expected_count"])];
    if sampled {
        forms.push((&["property"], &["unknown_count"]));
    }
    forms.extend([
        (&["cohort"][..], &["value"][..]),
        (&["cohort"], &["bounds_with_undetermined"]),
        (&["outcome"], &["expected_count"]),
        (&["outcome"], &["rate"]),
    ]);
    forms
        .into_iter()
        .map(|(scope, field)| {
            let outcome = scope[0] == "outcome";
            let value = || Null(Box::new(quality_table_form(scope[0], field[0], sampled)));
            let mut fields = vec![(
                "key",
                Obj(vec![
                    ("scope", Lit(scope)),
                    (
                        "subject",
                        if outcome {
                            Lit(&[
                                "end",
                                "overflow",
                                "rejected",
                                "unfinished",
                                "deadlock",
                                "cut_off",
                            ])
                        } else {
                            Str
                        },
                    ),
                    ("end", Null(Box::new(if outcome { Str } else { Lit(&[]) }))),
                    ("field", Lit(field)),
                ]),
            )];
            if !outcome && scope[0] == "cohort" {
                fields.push(("denominator", quality_denominator()));
            }
            if comparing {
                fields.push((
                    "values",
                    Arr(Box::new(Obj(vec![
                        ("configuration", Str),
                        ("status", Lit(&["reported", "not_applicable"])),
                        ("value", value()),
                    ]))),
                ));
            } else {
                let cell = || {
                    Obj(vec![
                        ("status", Lit(&["reported", "not_applicable"])),
                        ("value", value()),
                    ])
                };
                fields.extend([("from", cell()), ("to", cell())]);
            }
            Obj(fields)
        })
        .collect()
}

/// Cross-field rules of the quality table beyond its closed
/// row forms: the analysis is fixed per table, `not_applicable` carries no
/// value, a reported cell has a value except on `bounds_with_undetermined`,
/// and an outcome row names an end exactly for the end/overflow kinds.
fn validate_quality_table(
    table: &Value,
    comparing: bool,
    sampled: bool,
    plans: Option<&[Value]>,
) -> Result<(), String> {
    let schema = Ty::Arr(Box::new(Ty::OneOf(quality_table_rows(comparing, sampled))));
    check(table, &schema, "quality table")?;
    complete(table, &schema, "quality table")?;
    for row in table.as_array().unwrap() {
        let key = &row["key"];
        if key["scope"] == "outcome"
            && ["end", "overflow"].contains(&key["subject"].as_str().unwrap())
                == key["end"].is_null()
        {
            return Err("quality table outcome end identity".into());
        }
        let cells: Vec<&Value> = if comparing {
            row["values"].as_array().unwrap().iter().collect()
        } else {
            vec![&row["from"], &row["to"]]
        };
        for cell in cells {
            let empty = cell["value"].is_null();
            if cell["status"] == "not_applicable" && !empty
                || cell["status"] == "reported"
                    && empty
                    && key["field"] != "bounds_with_undetermined"
            {
                return Err("quality table cell status/value".into());
            }
        }
    }
    if let Some(plans) = plans {
        // This is a transport oracle, not a second quality evaluator. Preserve
        // the nested declaration order, including reason-bearing nulls and the
        // optional CutOff outcome; do not derive the inventory from the table.
        let sources: Vec<_> = plans
            .iter()
            .map(|p| quality_table_source(&p["report"], sampled))
            .collect::<Result<_, _>>()?;
        let first = sources.first().ok_or("quality table without plans")?;
        if sources.iter().any(|s| {
            s.len() != first.len()
                || s.iter().zip(first).any(|(a, b)| {
                    a["key"] != b["key"] || a.get("denominator") != b.get("denominator")
                })
        }) {
            return Err("nested quality inventories/denominators differ".into());
        }
        let expected: Vec<_> = first
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut row = serde_json::json!({"key":r["key"],"values":plans.iter().zip(&sources)
                    .map(|(p,s)|serde_json::json!({"configuration":p["configuration"],"status":"reported","value":s[i]["value"]})).collect::<Vec<_>>()});
                if let Some(d) = r.get("denominator") {
                    row["denominator"] = d.clone();
                }
                row
            })
            .collect();
        if table != &Value::Array(expected) {
            return Err("quality table inventory/order/plan/value transport".into());
        }
    }
    Ok(())
}

fn quality_table_source(report: &Value, sampled: bool) -> Result<Vec<Value>, String> {
    let Some(q) = report.get("quality") else {
        return Ok(Vec::new());
    };
    validate_quality_wire(q, sampled)?;
    let mut rows = Vec::new();
    for (section, scope, fields) in [
        (
            "properties",
            "property",
            if sampled {
                &["expected_count", "unknown_count"][..]
            } else {
                &["expected_count"][..]
            },
        ),
        (
            "cohort",
            "cohort",
            &["value", "bounds_with_undetermined"][..],
        ),
        (
            "outcomes_per_arrival",
            "outcome",
            &["expected_count", "rate"][..],
        ),
    ] {
        for source in q[section].as_array().ok_or("quality source section")? {
            for field in fields {
                let mut row = serde_json::json!({"key":{"scope":scope,
                    "subject":if scope == "outcome" {&source["kind"]} else {&source["name"]},
                    "end":if scope == "outcome" {source["end"].clone()} else {Value::Null},
                    "field":field},"value":source[*field]});
                if scope == "cohort" {
                    row["denominator"] = source["denominator"].clone();
                }
                rows.push(row);
            }
        }
    }
    Ok(rows)
}

/// Closed quality wire independent of production DTO defaults and numeric enums.
#[allow(dead_code)]
pub fn validate_quality_wire(q: &Value, sampled: bool) -> Result<(), String> {
    let schema = quality_schema(sampled);
    check(q, &schema, "quality")?;
    complete(q, &schema, "quality")?;
    let properties = q["properties"].as_array().unwrap();
    let mut names = std::collections::BTreeSet::new();
    for p in properties {
        if !names.insert(p["name"].as_str().unwrap()) {
            return Err("duplicate quality property".into());
        }
        for (i, c) in p["cases"].as_array().unwrap().iter().enumerate() {
            if c["case_id"].as_u64() != Some(i as u64 + 1) {
                return Err("quality case inventory/order".into());
            }
            if sampled
                && c["hits"].as_u64().unwrap() + c["unknown"].as_u64().unwrap()
                    > c["trials"].as_u64().unwrap()
            {
                return Err("quality hit/unknown trials".into());
            }
        }
    }
    let mut metrics = std::collections::BTreeSet::new();
    for m in q["cohort"].as_array().unwrap() {
        if !metrics.insert(m["name"].as_str().unwrap())
            || !names.contains(m["numerator"].as_str().unwrap())
        {
            return Err("quality metric identity/reference".into());
        }
        let d = &m["denominator"];
        if d["kind"] == "property" {
            if !names.contains(d["property"].as_str().unwrap()) {
                return Err("unknown quality denominator".into());
            }
            if d["covers"].is_null() != (d["reason"] == "state_condition") {
                return Err("quality covers reason".into());
            }
        }
        let v = &m["value"];
        if sampled {
            if v["point"].is_null() == v["reason"].is_null() {
                return Err("quality point reason".into());
            }
        } else {
            let missing = v["lower"].is_null();
            if missing != v["upper"].is_null()
                || missing != v["basis"].is_null()
                || missing == v["reason"].is_null()
            {
                return Err("quality null endpoints/basis/reason".into());
            }
        }
    }
    validate_quantiles(q, sampled, &names, &mut metrics)?;
    let mut outcomes = std::collections::BTreeSet::new();
    for o in q["outcomes_per_arrival"].as_array().unwrap() {
        if !outcomes.insert((o["kind"].as_str(), o["end"].as_str())) {
            return Err("duplicate quality outcome key".into());
        }
        if ["end", "overflow"].contains(&o["kind"].as_str().unwrap()) == o["end"].is_null() {
            return Err("quality outcome end identity".into());
        }
    }
    Ok(())
}

fn commit_target_name(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        let parts: Vec<_> = s.split('.').collect();
        parts.len() == 3
            && parts[2] == "commit"
            && parts[..2].iter().all(|p| {
                let mut chars = p.chars();
                chars
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                    && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
            })
    })
}
/// ADR0042: appended closed population; never changes an older denominator.
#[allow(dead_code)]
pub const SCENARIO_FIXTURES: &[&str] = include!("../../relca-core/tests/support/scenario_names.rs");
include!("../../relca-core/tests/support/agency_second_half_names.rs");

/// ADR0048: appended closed population; an unknown wait-prefix is not exempt.
#[allow(dead_code)]
pub const WAITS_FIXTURES: &[&str] = include!("../../relca-core/tests/support/waits_names.rs");

/// ADR 0026: twenty-second closed population, sixteen single-edit pairs.
#[allow(dead_code)]
pub const REQUIRES_FIXTURES: [&str; 32] = [
    "req-after-queue",
    "req-after-queue.broken-close",
    "req-agency",
    "req-agency.broken-remove-guard",
    "req-bypass",
    "req-bypass.broken-conjunction",
    "req-cancel-immediate",
    "req-cancel-immediate.broken-cooperative",
    "req-explicit-else",
    "req-explicit-else.broken-no-else",
    "req-kernel",
    "req-kernel.broken-enable",
    "req-no-capacity",
    "req-no-capacity.broken-zero",
    "req-observed",
    "req-observed.broken-latent",
    "req-same-time",
    "req-same-time.broken-no-close",
    "req-start",
    "req-start-kept",
    "req-start-kept.broken-forbidden",
    "req-start-siblings",
    "req-start-siblings.broken-same-target",
    "req-start.broken-false",
    "req-wait-admission",
    "req-wait-admission.broken-small-queue",
    "req-wait-arrivals",
    "req-wait-arrivals.broken-no-wait",
    "req-wait-clock",
    "req-wait-clock.broken-short-clock",
    "req-wait-gap",
    "req-wait-gap.broken-zero",
];

/// ADR 0029 U2: the coverage word an exhaustive report must carry. A range
/// stop without a computational cut is `range_stopped`; the flag that admits
/// it occurs only beside the implicit integer property (validate_integer_wire).
fn exhaustive_word(v: &Value) -> &'static str {
    if v["bounds"]["truncated"].is_null() {
        "aborted"
    } else if v["bounds"]["truncated"] == true {
        "truncated"
    } else if v["bounds"]["range_violation_stops"] == true {
        "range_stopped"
    } else {
        "complete"
    }
}

fn capability_refusals_schema(sampled: bool) -> Ty {
    use Ty::*;
    let mut fields = vec![
        ("case_id", Int),
        ("call", Str),
        ("capability", Str),
        ("result", Str),
        ("at", Obj(vec![("min", Int), ("max", Int)])),
        (
            "coverage",
            if sampled {
                Lit(&["sampled"])
            } else {
                Lit(&["complete", "truncated", "aborted", "range_stopped"])
            },
        ),
    ];
    if sampled {
        fields.extend([("observed_count", Int), ("samples", Int)]);
    }
    Arr(Box::new(Obj(fields)))
}

/// Closed table and cross-field invariants, independent of new production DTOs.
#[allow(dead_code)]
pub fn validate_requires_wire(v: &Value) -> Result<(), String> {
    if let Some(table) = v.get("capability_refusals") {
        let sampled = v["mode"] == "simulation";
        let schema = capability_refusals_schema(sampled);
        check(table, &schema, "capability_refusals")?;
        complete(table, &schema, "capability_refusals")?;
        if !v["bounds"]["cases"].is_u64() {
            return Err("static failure cannot emit capability_refusals".into());
        }
        let mut keys = std::collections::BTreeSet::new();
        let mut last_case = 0;
        for r in table.as_array().unwrap() {
            let case = r["case_id"].as_u64().unwrap();
            let call = r["call"].as_str().unwrap();
            if case == 0
                || case > v["bounds"]["cases"].as_u64().unwrap()
                || case < last_case
                || call.is_empty()
                || !keys.insert((case, call))
                || r["capability"].as_str().unwrap().split('.').count() != 2
                || r["capability"]
                    .as_str()
                    .unwrap()
                    .split('.')
                    .any(str::is_empty)
                || r["result"].as_str().unwrap().is_empty()
                || r["at"]["min"].as_u64() > r["at"]["max"].as_u64()
            {
                return Err("invalid/duplicate/unordered capability refusal identity/time".into());
            }
            last_case = case;
            if sampled {
                let count = r["observed_count"].as_u64().unwrap();
                let samples = r["samples"].as_u64().unwrap();
                if count == 0
                    || count > samples
                    || v["simulation"]["completed_replications"].as_u64() != Some(samples)
                {
                    return Err(
                        "sampled refusal count must count selected completed replications".into(),
                    );
                }
            } else {
                if r["coverage"] != exhaustive_word(v) {
                    return Err("capability refusal coverage/bounds mismatch".into());
                }
            }
        }
    }
    Ok(())
}

/// ADR 0027: twenty hand-derived single-change Authority pairs.
#[allow(dead_code)]
pub const AUTHORITY_FIXTURES: [&str; 40] = [
    "auth-scope",
    "auth-scope.broken-false-branch",
    "auth-scope-post",
    "auth-scope-post.broken-finish-value",
    "auth-scope-latent",
    "auth-scope-latent.broken-latent",
    "auth-expiry",
    "auth-expiry.broken-minute-early",
    "auth-expiry-cut",
    "auth-expiry-cut.broken-short-clock",
    "auth-expiry-absolute",
    "auth-expiry-absolute.broken-earlier-expiry",
    "auth-delegation",
    "auth-delegation.broken-wider-grants",
    "auth-delegation-forward",
    "auth-delegation-forward.broken-cycle",
    "auth-delegation-not-by",
    "auth-delegation-not-by.broken-forbidden-person",
    "auth-delegation-name",
    "auth-delegation-name.broken-missing-parent",
    "auth-delegation-expiry",
    "auth-delegation-expiry.broken-longer-child",
    "auth-delegation-issuer",
    "auth-delegation-issuer.broken-issuer",
    "auth-self-grant",
    "auth-self-grant.broken-self-issued",
    "auth-waive",
    "auth-waive.broken-restricted-authority",
    "auth-contest",
    "auth-contest.broken-restricted-reversal",
    "auth-revocation",
    "auth-revocation.broken-missing-record",
    "auth-inherited-scope",
    "auth-inherited-scope.broken-parent-scope",
    "auth-inherited-expiry",
    "auth-inherited-expiry.broken-parent-earlier",
    "auth-requires-integration",
    "auth-requires-integration.broken-starts",
    "auth-known-violation-cut",
    "auth-known-violation-cut.broken-clock",
];

/// ADR 0028: fifteen pairs, separate from the frozen ADR 0018 population.
#[allow(dead_code)]
pub const RESPONSIBILITY_EXTENSION_FIXTURES: [&str; 30] = [
    "resp2-atomic",
    "resp2-atomic.broken-consequent",
    "resp2-bool",
    "resp2-bool.broken-enum",
    "resp2-classification",
    "resp2-classification.broken-operator",
    "resp2-collision",
    "resp2-collision.broken-capability",
    "resp2-design",
    "resp2-design.broken-unknown",
    "resp2-direction",
    "resp2-direction.broken-reversed",
    "resp2-duration",
    "resp2-duration.broken-short",
    "resp2-equivalence",
    "resp2-equivalence.broken-wrong-antecedent",
    "resp2-maintenance",
    "resp2-maintenance.broken-indirect-only",
    "resp2-refusal",
    "resp2-refusal.broken-bypass",
    "resp2-relay",
    "resp2-relay.broken-short",
    "resp2-unchecked",
    "resp2-unchecked.broken-within",
    "resp2-unknown-origin",
    "resp2-unknown-origin.broken-no-source",
    "resp2-unknown-time",
    "resp2-unknown-time.broken-no-direct",
    "resp2-within",
    "resp2-within.broken-short",
];

/// ADR 0029: appended independent population, twenty-eight pairs.
#[allow(dead_code)]
pub const INTEGER_FIXTURES: [&str; 56] = [
    "int-two-add",
    "int-two-add.broken-assignment",
    "int-upper",
    "int-upper.broken-overflow",
    "int-underflow",
    "int-underflow.broken-underflow",
    "int-default",
    "int-default.broken-default",
    "int-type",
    "int-type.broken-enum",
    "int-latent-policy",
    "int-latent-policy.broken-latent",
    "int-lt",
    "int-lt.broken-boundary",
    "int-le",
    "int-le.broken-boundary",
    "int-gt",
    "int-gt.broken-boundary",
    "int-ge",
    "int-ge.broken-boundary",
    "int-ne",
    "int-ne.broken-boundary",
    "int-fork",
    "int-fork.broken-same-target",
    "int-start-finish",
    "int-start-finish.broken-zero-start",
    "int-set-stop",
    "int-set-stop.broken-outward",
    "int-dist",
    "int-dist.broken-threshold",
    "int-nondet",
    "int-nondet.broken-threshold",
    "int-class-int",
    "int-class-int.broken-unbounded",
    "int-class-param",
    "int-class-param.broken-caseid",
    "int-class-caseid",
    "int-class-caseid.broken-reference",
    "int-class-reads",
    "int-class-reads.broken-observed",
    "int-class-stateid",
    "int-class-stateid.broken-caseid",
    "int-kernel-given",
    "int-kernel-given.broken-integer",
    "int-requires",
    "int-requires.broken-refused",
    "int-kernel-finish",
    "int-kernel-finish.broken-overflow",
    "int-atomic",
    "int-atomic.broken-overflow",
    "int-cancel",
    "int-cancel.broken-drain",
    "int-range-vs-clock",
    "int-range-vs-clock.broken-clock",
    "int-arrivals",
    "int-arrivals.broken-overflow",
];

/// ADR 0033: appended closed resource population, never absorbed into older groups.
#[allow(dead_code)]
pub const RESOURCE_FIXTURES: [&str; 60] = [
    "rsrc-exact",
    "rsrc-exact.broken-one-unit",
    "rsrc-shared",
    "rsrc-shared.broken-missing-use",
    "rsrc-weighted",
    "rsrc-weighted.broken-less-capacity",
    "rsrc-atomic",
    "rsrc-atomic.broken-excess-demand",
    "rsrc-reuse",
    "rsrc-reuse.broken-excess-next",
    "rsrc-cooperative",
    "rsrc-cooperative.broken-drain",
    "rsrc-immediate",
    "rsrc-immediate.broken-cooperative",
    "rsrc-request-held",
    "rsrc-request-held.broken-longer-ack",
    "rsrc-refusal",
    "rsrc-refusal.broken-enabled",
    "rsrc-gate-after-wait",
    "rsrc-gate-after-wait.broken-no-use",
    "rsrc-competition",
    "rsrc-competition.broken-same-actor",
    "rsrc-global",
    "rsrc-global.broken-two-units",
    "rsrc-people",
    "rsrc-people.broken-two-people",
    "rsrc-over-demand",
    "rsrc-over-demand.broken-too-much",
    "rsrc-unused-zero",
    "rsrc-unused-zero.broken-used",
    "rsrc-members",
    "rsrc-members.broken-members",
    "rsrc-actor-use",
    "rsrc-actor-use.broken-actor-use",
    "rsrc-collision",
    "rsrc-collision.broken-same-name",
    "rsrc-zero-amount",
    "rsrc-zero-amount.broken-zero",
    "rsrc-unknown",
    "rsrc-unknown.broken-unknown",
    "rsrc-duplicate",
    "rsrc-duplicate.broken-duplicate",
    "rsrc-second-use",
    "rsrc-second-use.broken-second-use",
    "rsrc-actor-capacity",
    "rsrc-actor-capacity.broken-no-capacity",
    "rsrc-all-start",
    "rsrc-all-start.broken-no-proof",
    "rsrc-clock",
    "rsrc-clock.broken-short-clock",
    "rsrc-release-fork",
    "rsrc-release-fork.broken-parallel",
    "rsrc-queued-cancel",
    "rsrc-queued-cancel.broken-startable",
    "rsrc-huge",
    "rsrc-huge.broken-fits",
    "rsrc-zero-time",
    "rsrc-zero-time.broken-positive",
    "rsrc-half-open",
    "rsrc-half-open.broken-longer",
];

fn state_range_violation_schema() -> Ty {
    use Ty::*;
    let span = || Obj(vec![("start", Int), ("end", Int)]);
    Obj(vec![
        ("case_id", Null(Box::new(Int))),
        ("time", Int),
        ("binding", Str),
        ("actor", Str),
        ("capability", Str),
        ("phase", Lit(&["start", "finish"])),
        ("variable", Str),
        ("operator", Lit(&["=", "+=", "-="])),
        ("before", Str),
        ("operand", Str),
        ("attempted", Str),
        ("lower", Lit(&["0"])),
        ("upper", Str),
        ("write_span", span()),
        ("declaration_span", span()),
    ])
}

/// Closed ADR0029 table plus independent exact integer consistency checks.
/// Source-length/UTF-8 bounds are checked by acceptance against the actual DSL.
#[allow(dead_code)]
pub fn validate_state_range_violation(w: &Value) -> Result<(), String> {
    let schema = state_range_violation_schema();
    check(w, &schema, "range witness")?;
    complete(w, &schema, "range witness")?;
    let number = |key: &str| -> Result<i128, String> {
        let text = w[key].as_str().ok_or("range decimal string")?;
        let n: i128 = text.parse().map_err(|_| "range exact integer")?;
        if n.to_string() != text {
            return Err("range noncanonical integer".into());
        }
        Ok(n)
    };
    let before = number("before")?;
    let operand = number("operand")?;
    let attempted = number("attempted")?;
    let upper = number("upper")?;
    if upper < 0
        || upper > i128::from(u64::MAX)
        || before < 0
        || before > upper
        || operand < -i128::from(u64::MAX)
        || operand > i128::from(u64::MAX)
        || (0..=upper).contains(&attempted)
        || match w["operator"].as_str().unwrap() {
            "=" => Some(operand),
            "+=" => before.checked_add(operand),
            "-=" => before.checked_sub(operand),
            _ => None,
        } != Some(attempted)
    {
        return Err("range arithmetic/boundary mismatch".into());
    }
    for key in ["binding", "actor", "capability", "variable"] {
        if w[key].as_str().unwrap().is_empty() {
            return Err("range empty identity".into());
        }
    }
    for key in ["write_span", "declaration_span"] {
        if w[key]["start"].as_u64() >= w[key]["end"].as_u64() {
            return Err("range empty/reversed span".into());
        }
    }
    if w["case_id"] == 0 {
        return Err("range case identity is one-based or null".into());
    }
    Ok(())
}

#[allow(dead_code)]
pub fn validate_integer_wire(v: &Value) -> Result<(), String> {
    let properties = v["properties"].as_array().ok_or("integer properties")?;
    let ps: Vec<_> = properties
        .iter()
        .filter(|p| p["id"] == "state_in_range")
        .collect();
    let flag = v["bounds"].get("range_violation_stops");
    let witnesses = v.get("state_range_violations");
    if ps.is_empty() {
        if flag.is_some()
            || witnesses.is_some()
            || v["simulation"].get("range_stopped_replications").is_some()
            || v["case_outcomes"]
                .as_array()
                .is_some_and(|cs| cs.iter().any(|c| c.get("range_stopped_count").is_some()))
        {
            return Err("integer fields without implicit property".into());
        }
        return Ok(());
    }
    // ADR 0041: the implicit no_lost_update rows follow state_in_range.
    let before_loss = properties.iter().rev().find(|p| {
        !p["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("no_lost_update "))
    });
    if ps.len() != 1 || before_loss != ps.first().copied() || ps[0].get("probability").is_some() {
        return Err("implicit range property must occur once, last, without probability".into());
    }
    if flag.is_some_and(|f| *f != true) {
        return Err("range flag must be absent or true".into());
    }
    if flag.is_some() != witnesses.is_some() {
        return Err("range flag/witness mismatch".into());
    }
    if let Some(ws) = witnesses {
        let ws = ws.as_array().ok_or("range witnesses array")?;
        if ws.is_empty() || ps[0]["verdict"] != "violated" {
            return Err("range witness requires known violation".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for w in ws {
            validate_state_range_violation(w)?;
            if w["time"].as_u64() > v["bounds"]["clock"].as_u64()
                || w["case_id"]
                    .as_u64()
                    .is_some_and(|c| c > v["bounds"]["cases"].as_u64().unwrap_or(0))
                || !seen.insert((
                    w["case_id"].to_string(),
                    w["binding"].to_string(),
                    w["phase"].to_string(),
                    w["variable"].to_string(),
                ))
            {
                return Err("range witness time/case/duplicate".into());
            }
        }
    } else if ps[0]["verdict"] == "violated" {
        return Err("range violation without witness".into());
    }
    if v["mode"] == "simulation" && v["simulation"]["status"] == "completed" {
        let n = v["simulation"]["completed_replications"]
            .as_u64()
            .ok_or("range samples")?;
        let stopped = v["simulation"]["range_stopped_replications"]
            .as_u64()
            .ok_or("range stopped replication count")?;
        if stopped > n || (stopped > 0) != flag.is_some() {
            return Err("range replication accounting".into());
        }
        for c in v["case_outcomes"].as_array().ok_or("range cases")? {
            if c["range_stopped_count"]
                .as_u64()
                .is_none_or(|k| k > stopped)
            {
                return Err("range case accounting".into());
            }
        }
        if (stopped == 0 && (ps[0]["verdict"] != "inconclusive" || ps[0]["basis"] != "sample"))
            || (stopped > 0 && ps[0]["basis"] != "witness")
        {
            return Err("range sample is not a proof".into());
        }
    }
    Ok(())
}

/// ADR 0031: append-only population, shared with the independent core census.
#[path = "../../relca-core/tests/support/configuration_diff_population.rs"]
mod diff2_population;
#[allow(unused_imports)]
pub use diff2_population::NAMES as CONFIGURATION_DIFF_EXTENSION_FIXTURES;
#[allow(unused_imports)]
pub use diff2_population::hand_warnings as configuration_diff_hand_warnings;

/// ADR 0025: twenty-first closed population, twelve single-change pairs.
#[allow(dead_code)]
pub const COMPLETION_FIXTURES: [&str; 24] = [
    "done-arrivals",
    "done-arrivals.broken-arrival-after-clock",
    "done-branches",
    "done-branches.broken-enable-then",
    "done-call-kind",
    "done-call-kind.broken-call-unused",
    "done-clock",
    "done-clock.broken-short-clock",
    "done-end-vs-close",
    "done-end-vs-close.broken-pre-end-delay",
    "done-nondet",
    "done-nondet.broken-nondet-option",
    "done-people",
    "done-people.broken-remove-executor",
    "done-probability",
    "done-probability.broken-block-failure",
    "done-recovery",
    "done-recovery.broken-zero-capacity",
    "done-support",
    "done-support.broken-positive-support",
    "done-vacuity",
    "done-vacuity.broken-enable-registration",
    "done-within",
    "done-within.broken-tight-deadline",
];

fn structural_completion(id: &str) -> bool {
    ["recoverable end", "terminating end", "almost_sure end"].contains(&id)
}
fn property_probability(id: &str) -> bool {
    id == "almost_sure end" || id.starts_with("probability reached #")
}

/// Independent new-field invariants. Also usable with hand-authored minimal
/// JSON in negative controls, without depending on a production DTO or solver.
#[allow(dead_code)]
pub fn validate_completion_wire(v: &Value) -> Result<(), String> {
    validate_authority_record(v)?;
    let properties = v["properties"].as_array().ok_or("completion properties")?;
    let simulation = v["mode"] == "simulation";
    let runtime = v["bounds"]["cases"].is_u64();
    // ADR 0042: a horizon is a business cut (truncated stays false), but the
    // suffix after it is not inspected, so reachability stays unchecked.
    // Boundary review M2: any leaf stopped by the horizon counts, not only one
    // with a business cutoff row. A stopped leaf holds an execution that is not
    // closed: either a case without a business End (a cut_off row), or a case
    // after its End whose filed contest route has no response yet. The wire
    // shows the latter as a contest alternative with status unresolved; that
    // status needs a cut (clock or horizon), deadlock gives violated, and a
    // closed route responded or was not filed. observed_until.max == horizon
    // is not used: a run whose executions all close in the layer at D also
    // observes D without any horizon leaf.
    let horizon_cut = v["case_outcomes"].as_array().is_some_and(|cs| {
        cs.iter().any(|c| {
            c["terminals"]
                .as_array()
                .is_some_and(|ts| ts.iter().any(|t| t["kind"] == "cut_off"))
        })
    }) || (v["scenario"]["horizon"].is_u64()
        && v["contest_runtime"].as_array().is_some_and(|rows| {
            rows.iter().any(|r| {
                r["alternatives"]
                    .as_array()
                    .is_some_and(|alts| alts.iter().any(|a| a["status"] == "unresolved"))
            })
        }));
    let incomplete = runtime
        && (v["bounds"]["truncated"] == true
            || v["bounds"]["truncated"].is_null()
            || v["bounds"]["range_violation_stops"] == true
            || horizon_cut);
    let words = v
        .get("unchecked")
        .map(|u| u.as_array().ok_or("completion unchecked array"))
        .transpose()?;
    if let Some(words) = words {
        if words.is_empty()
            || words
                .iter()
                .map(Value::to_string)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != words.len()
        {
            return Err("completion unchecked must be nonempty and unique".into());
        }
        for word in words {
            if !(word == "unreachable"
                || (!simulation && word == "completion_graph")
                || (simulation && word == "obligation_outcomes")
                || word == "dynamic_authority_revocation")
            {
                return Err("unknown completion unchecked word".into());
            }
        }
    }
    let has = |w: &str| words.is_some_and(|a| a.iter().any(|v| v == w));
    if has("unreachable") != (runtime && (simulation || incomplete)) {
        return Err(
            "unreachable unchecked must exactly track cut/abort/sample, not static failure".into(),
        );
    }
    if has("completion_graph")
        && (!runtime
            || !properties.iter().any(|p| {
                p["id"].as_str().is_some_and(structural_completion)
                    && (p["verdict"] == "unsupported"
                        || (p["verdict"] == "violated"
                            && v["diagnostics"]
                                .as_array()
                                .is_some_and(|ds| ds.iter().any(|d| d["code"] == "W-DEADLOCK-0"))))
            })
            || !v["diagnostics"].as_array().is_some_and(|ds| {
                ds.iter()
                    .any(|d| d["code"] == "U-SEM-0" && d["span"].is_object())
            }))
    {
        return Err(
            "completion_graph requires an unresolved structural query or retained deadlock witness, and located diagnostic"
                .into(),
        );
    }
    for d in v["diagnostics"]
        .as_array()
        .ok_or("completion diagnostics")?
    {
        if d["code"]
            .as_str()
            .is_some_and(|c| c.starts_with("W-UNREACH-"))
            && (!["W-UNREACH-0", "W-UNREACH-1", "W-UNREACH-2", "W-UNREACH-3"]
                .iter()
                .any(|c| d["code"] == *c)
                || d["severity"] != "warning"
                || !runtime
                || simulation
                || incomplete
                || d["span"]["start"]
                    .as_u64()
                    .zip(d["span"]["end"].as_u64())
                    .is_none_or(|(a, b)| a >= b))
        {
            return Err("unreachable warning requires complete exhaustive exploration and precise nonempty span".into());
        }
    }
    for p in properties {
        let id = p["id"].as_str().ok_or("completion property id")?;
        if ["recoverable", "terminating", "almost_sure"]
            .iter()
            .any(|prefix| id.starts_with(prefix))
            && !structural_completion(id)
        {
            return Err("unknown completion property id".into());
        }
        if !structural_completion(id) {
            continue;
        }
        if simulation || p["verdict"] == "vacuous" {
            return Err("structural completion is neither sampled nor vacuous".into());
        }
        let cases = p.get("cases");
        if cases.is_some() != v.get("case_outcomes").is_some() {
            return Err("completion case routing".into());
        }
        if p.get("probability").is_some() != (id == "almost_sure end" && cases.is_none()) {
            return Err("completion probability presence".into());
        }
        let rows: Vec<&Value> = if let Some(cs) = cases {
            let cs = cs.as_array().ok_or("completion cases")?;
            let outcomes = v["case_outcomes"].as_array().ok_or("completion outcomes")?;
            if cs.is_empty()
                || cs.len() != outcomes.len()
                || cs
                    .iter()
                    .zip(outcomes)
                    .any(|(c, o)| c["case_id"] != o["case_id"] || c["arrival"] != o["arrival"])
            {
                return Err("completion case identity/arrival/inventory".into());
            }
            cs.iter().collect()
        } else {
            vec![p]
        };
        for row in rows {
            if row["verdict"] == "vacuous"
                || row.get("probability").is_some() != (id == "almost_sure end")
            {
                return Err("completion case probability/verdict".into());
            }
            if let Some(q) = row.get("probability") {
                validate_probability(q)?;
            }
        }
    }
    Ok(())
}

/// The closed ADR 0044 tail rows: `<R>.rule`, `<R>.amendment` and
/// `authority.<A>.change.<R>.<person>`.
fn rule_tail_assumption(field: &str) -> bool {
    let plain = |name: &str| !name.is_empty() && !name.contains('.');
    if let Some(rest) = field.strip_prefix("authority.") {
        let parts: Vec<&str> = rest.split('.').collect();
        return parts.len() == 4 && parts[1] == "change" && parts.iter().all(|p| !p.is_empty());
    }
    field.strip_suffix(".rule").is_some_and(plain)
        || field.strip_suffix(".amendment").is_some_and(plain)
}

/// ADR 0044 boundary review m2: the closed tail rows of every Report (also
/// nested), independent of the arrivals route. Inside the tail each Rule's
/// `<R>.rule` row comes in report Actor order and its optional `<R>.amendment`
/// row follows it directly; the unused rule-change grant rows come last. The R
/// of every tail row is a Rule Actor of this Report (an Actor row carrying
/// `rule`), and `.rule`/`.amendment` value and affects are nonempty strings.
/// The Report carries no Authority declarations, so the A of
/// `authority.<A>.change.<R>.<person>` is not checked here.
fn validate_rule_tail_wire(v: &Value) -> Result<(), String> {
    let rules: Vec<&str> = v["actors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a.get("rule").is_some_and(|r| !r.is_null()))
        .filter_map(|a| a["name"].as_str())
        .collect();
    let Some(rows) = v.get("assumptions").and_then(Value::as_array) else {
        return Ok(());
    };
    let mut last_rule: Option<usize> = None;
    let mut amended = false;
    let mut grants = false;
    for row in rows {
        let Some(field) = row["field"].as_str() else {
            continue;
        };
        if !rule_tail_assumption(field) {
            continue;
        }
        if let Some(rest) = field.strip_prefix("authority.") {
            let parts: Vec<&str> = rest.split('.').collect();
            if !rules.contains(&parts[2]) {
                return Err("rule tail: change record names no Rule of this Report".into());
            }
            grants = true;
            continue;
        }
        if row["value"].as_str().is_none_or(str::is_empty)
            || row["affects"].as_str().is_none_or(str::is_empty)
        {
            return Err("rule tail: empty or non-string value/affects".into());
        }
        if let Some(r) = field.strip_suffix(".rule") {
            let index = rules
                .iter()
                .position(|x| *x == r)
                .ok_or("rule tail: .rule row names no Rule of this Report")?;
            if grants || last_rule.is_some_and(|l| l >= index) {
                return Err("rule tail: rule row order".into());
            }
            last_rule = Some(index);
            amended = false;
        } else {
            let r = field.strip_suffix(".amendment").unwrap();
            if !rules.contains(&r) {
                return Err("rule tail: .amendment row names no Rule of this Report".into());
            }
            if grants || amended || last_rule.map(|l| rules[l]) != Some(r) {
                return Err("rule tail: amendment row must directly follow its rule row".into());
            }
            amended = true;
        }
    }
    Ok(())
}

/// Independent record/marker pairing, applied to every Report (also nested).
/// This is not evidence that dynamic withdrawal has been executed or checked.
fn validate_authority_record(v: &Value) -> Result<bool, String> {
    let marker = v["unchecked"]
        .as_array()
        .is_some_and(|ws| ws.iter().any(|w| w == "dynamic_authority_revocation"));
    let record = v["assumptions"].as_array().is_some_and(|rows| {
        rows.iter().any(|r| {
            r["field"]
                .as_str()
                .is_some_and(|f| f.starts_with("authority."))
        })
    });
    // Do not impose new shape requirements on unrelated legacy projections.
    // A lone new record still enters this path, to reject a missing marker.
    if !marker && !record {
        return Ok(false);
    }
    let mut fields = std::collections::BTreeSet::new();
    let mut change_fields = std::collections::BTreeSet::new();
    // This helper is also used with minimal completion-only projections. A
    // missing array means no records there; complete Report schemas still make
    // assumptions mandatory, and a marker without a record is rejected below.
    for row in v
        .get("assumptions")
        .map(|a| a.as_array().ok_or("authority assumptions array"))
        .transpose()?
        .into_iter()
        .flatten()
    {
        let field = row["field"].as_str().ok_or("authority assumption field")?;
        if !field.starts_with("authority.") {
            continue;
        }
        // ADR 0044: an unused rule-change grant is a closed, separate record
        // `authority.<A>.change.<R>.<person>`; it is not a revocation record.
        let parts: Vec<&str> = field["authority.".len()..].split('.').collect();
        if parts.len() == 4 && parts[1] == "change" {
            if parts
                .iter()
                .any(|p| p.is_empty() || p.chars().any(char::is_whitespace))
                || row["value"] != "unused_in_selected_configuration"
                || row["affects"]
                    != "declared rule-change grant is not used by the selected configuration; no enactment or exercise is inferred"
                || !change_fields.insert(field)
                || row.as_object().is_none_or(|r| r.len() != 3)
            {
                return Err("invalid or duplicate unused rule-change grant record".into());
            }
            continue;
        }
        let name = field
            .strip_prefix("authority.")
            .and_then(|s| s.strip_suffix(".revocable_by"))
            .ok_or("unknown authority assumption")?;
        let value = row["value"].as_str().ok_or("authority revoker value")?;
        let identity = value
            .strip_prefix("person:")
            .or_else(|| value.strip_prefix("actor:"))
            .ok_or("authority revoker tag")?;
        let affects = row["affects"]
            .as_str()
            .ok_or("authority revocation boundary")?;
        if name.is_empty()
            || name.contains('.')
            || name.chars().any(char::is_whitespace)
            || identity.is_empty()
            || identity.contains(':')
            || identity.chars().any(char::is_whitespace)
            || !fields.insert(field)
            || !affects.contains("動的")
            || !affects.contains("撤回")
            || !affects.contains("検査していない")
            || row.as_object().is_none_or(|r| {
                r.len() != 3
                    || !r.contains_key("field")
                    || !r.contains_key("value")
                    || !r.contains_key("affects")
            })
        {
            return Err("invalid or duplicate authority revocation record".into());
        }
    }
    let words = v
        .get("unchecked")
        .map(|w| w.as_array().ok_or("authority unchecked array"))
        .transpose()?;
    let markers = words.map_or(0, |ws| {
        ws.iter()
            .filter(|w| **w == "dynamic_authority_revocation")
            .count()
    });
    if markers != usize::from(!fields.is_empty())
        || (markers == 1
            && words.and_then(|ws| ws.last())
                != Some(&serde_json::json!("dynamic_authority_revocation")))
    {
        return Err("authority revocation marker/record mismatch or order".into());
    }
    Ok(!fields.is_empty())
}

/// Only the new root marker's set change is admitted under this check name.
/// It never becomes an assigned/detection change.
#[allow(dead_code)]
pub fn validate_completion_changes(rows: &Value, assigned: bool) -> Result<(), String> {
    for row in rows.as_array().ok_or("completion changes")? {
        if row["check"] != "unchecked" {
            continue;
        }
        if assigned
            || row["key"] != ""
            || row["field"] != "value"
            || row["direction"] != "changed"
            || row["before"] == row["after"]
        {
            return Err("invalid completion unchecked change identity".into());
        }
        for key in ["before", "after"] {
            if row[key].is_null() {
                continue;
            }
            let a = row[key].as_array().ok_or("completion change word set")?;
            if a.is_empty()
                || a.iter()
                    .any(|w| w != "unreachable" && w != "completion_graph")
                || a.windows(2).any(|w| w[0].as_str() >= w[1].as_str())
            {
                return Err("invalid completion change word set".into());
            }
        }
    }
    Ok(())
}

/// ADR 0024: twentieth closed population, twelve single-change pairs.
#[allow(dead_code)]
pub const COMMON_CAUSE_FIXTURES: [&str; 24] = [
    "lin-basis",
    "lin-basis.broken-no-basis",
    "lin-case-latent",
    "lin-case-latent.broken-actor-local",
    "lin-correlation",
    "lin-correlation.broken-unrelated-given",
    "lin-five-kernels",
    "lin-five-kernels.broken-given-only-read",
    "lin-invoice",
    "lin-invoice.broken-po-evidence",
    "lin-no-error-writer",
    "lin-no-error-writer.broken-connect-error-writer",
    "lin-numeric",
    "lin-numeric.broken-zero-detection",
    "lin-same-actor",
    "lin-same-actor.broken-read-removed",
    "lin-same-source-different-target",
    "lin-same-source-different-target.broken-use-same-target",
    "lin-shared-kernel",
    "lin-shared-kernel.broken-drop-call",
    "lin-single",
    "lin-single.broken-second-call",
    "lin-tacit",
    "lin-tacit.broken-unused-tacit",
];

const COMMON_CAUSE_NOTICE: &str = "条件付き独立は given の下でだけ仮定され、無条件には独立でない";
const BASIS_NOTICE: &str = "basis は分析者の宣言であり、測定根拠の正しさは検査していない";
const TACIT_NOTICE: &str = "tacit は分析者の宣言であり、根拠源の妥当性は検査していない";
const CORRELATION_NOTICE: &str = "相関は先行する判定結果への条件付けであり、残余相関の推定ではない";

/// Independent closed schema, not derived from the new production DTO.
#[allow(dead_code)]
pub fn validate_common_cause_agency(a: &Value) -> Result<(), String> {
    check(a, &nullable_agency(), "agency")?;
    complete(a, &nullable_agency(), "agency")?;
    if a.is_null() {
        return Ok(());
    }
    if let Some(v) = a.get("viewpoints") {
        validate_viewpoints(v)?;
    }
    let unique_nonempty = |v: &Value, key: Option<&str>, minimum: usize| -> Result<(), String> {
        let rows = v.as_array().ok_or("expected nonempty array")?;
        if rows.len() < minimum {
            return Err("empty/undersized common-cause array".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for r in rows {
            let id = key.map_or(r, |k| &r[k]);
            if !seen.insert(id.to_string()) || id.as_str() == Some("") {
                return Err("empty/duplicate semantic key".into());
            }
        }
        Ok(())
    };
    if let Some(c) = a.get("common_causes") {
        if c["notice"] != COMMON_CAUSE_NOTICE {
            return Err("common cause notice".into());
        }
        unique_nonempty(&c["causes"], Some("latent"), 1)?;
        for row in c["causes"].as_array().unwrap() {
            unique_nonempty(&row["targets"], Some("call"), 2)?;
            if row["actor"].as_str() == Some("")
                || !row["latent"]
                    .as_str()
                    .unwrap()
                    .starts_with(&format!("{}.", row["actor"].as_str().unwrap()))
            {
                return Err("Actor-qualified latent required".into());
            }
        }
    }
    if let Some(u) = a.get("model_uncertainties") {
        unique_nonempty(&u["items"], Some("call"), 1)?;
        for row in u["items"].as_array().unwrap() {
            let notice = match row["kind"].as_str() {
                Some("wlin2_basis") => BASIS_NOTICE,
                Some("wlin2_tacit") => {
                    unique_nonempty(&row["sources"], None, 1)?;
                    TACIT_NOTICE
                }
                Some("wlin2_correlation") => {
                    unique_nonempty(&row["judgements"], None, 1)?;
                    CORRELATION_NOTICE
                }
                _ => return Err("unknown suppression kind".into()),
            };
            if row["notice"] != notice {
                return Err("suppression notice".into());
            }
            unique_nonempty(&row["targets"], None, 1)?;
            unique_nonempty(&row["error_latents"], None, 1)?;
        }
    }
    if let Some(d) = a.get("source_descriptions") {
        unique_nonempty(d, Some("actor"), 1)?;
    }
    Ok(())
}

/// ADR 0023: nineteenth closed population (28 = runtime24 + static4).
#[allow(dead_code)]
pub const ACTOR_TABLE_FIXTURES: [&str; 28] = [
    "act-arrival-units",
    "act-arrival-units.broken-later-first",
    "act-composite",
    "act-composite.broken-team-work",
    "act-control",
    "act-control.broken-explicit-environment",
    "act-cut-context",
    "act-cut-context.broken-no-cut",
    "act-debtor",
    "act-debtor-actor",
    "act-debtor-actor.broken-omitted",
    "act-debtor-value",
    "act-debtor-value.broken-other-person",
    "act-debtor.broken-unknown",
    "act-defaults",
    "act-defaults.broken-explicit-boundary",
    "act-grain",
    "act-grain.broken-swapped",
    "act-horizon",
    "act-horizon.broken-short-horizon",
    "act-legacy",
    "act-legacy.broken-longer-task",
    "act-overlap",
    "act-overlap.broken-relation",
    "act-units",
    "act-units.broken-duration-day",
    "act-via",
    "act-via.broken-upstream-only",
];

const CUT_NOTICES: [&str; 2] = [
    "Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない",
    "この結果を運用の範囲決定に使う場合は採用経路を別に追うこと",
];
fn horizon_schema() -> Ty {
    use Ty::*;
    Arr(Box::new(Obj(vec![
        ("from", Null(Box::new(Str))),
        ("to", Str),
        ("within", Int),
        ("structural_only", Bool),
    ])))
}
fn cut_context_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        ("notices", Arr(Box::new(Str))),
        ("reentry", Arr(Box::new(Str))),
        ("horizon", horizon_schema()),
    ])
}
fn actor_table_schema() -> Ty {
    use Ty::*;
    let field = |value| {
        Obj(vec![
            ("value", value),
            ("status", Lit(&["declared", "default"])),
        ])
    };
    let strings = || Null(Box::new(Arr(Box::new(Str))));
    Null(Box::new(Arr(Box::new(Obj(vec![
        ("name", Str),
        ("boundary", field(Str)),
        ("grain", field(Null(Box::new(Str)))),
        ("contains", field(strings())),
        ("overlaps", field(strings())),
        ("capability", field(strings())),
        ("capacity", field(Null(Box::new(Int)))),
        (
            "executors",
            field(Null(Box::new(OneOf(vec![
                Obj(vec![("kind", Lit(&["named"])), ("ids", Arr(Box::new(Str)))]),
                Obj(vec![("kind", Lit(&["anonymous"])), ("count", Int)]),
            ])))),
        ),
        ("control", field(Lit(&["policy", "environment", "party"]))),
        (
            "source",
            field(Null(Box::new(Lit(&["document", "record", "tacit"])))),
        ),
        ("via_model", field(Null(Box::new(Str)))),
        ("state", field(strings())),
        ("latent", field(strings())),
        ("rule", OptionalField(Box::new(rule_actor_schema()))),
    ])))))
}
fn via_model_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        (
            "rules",
            Lit(&["via-model-read-paths-1", "via-model-read-paths-reads-1"]),
        ),
        (
            "basis",
            Lit(&["追跡であって、AI を介した変化の認定ではない"]),
        ),
        ("scope", Lit(&["selected_process_lexical_reads"])),
        ("truncated", Bool),
        ("path_count_lower_bound", Int),
        (
            "paths",
            Arr(Box::new(Obj(vec![
                ("actor", Str),
                ("model", Str),
                ("start_call", Str),
                (
                    "calls",
                    Arr(Box::new(Obj(vec![("call", Str), ("capability", Str)]))),
                ),
                (
                    "target",
                    OneOf(vec![
                        Obj(vec![
                            ("kind", Lit(&["U"])),
                            ("call", Str),
                            ("effect_call", Str),
                            ("stops", Lit(&["all_paths", "conditional"])),
                        ]),
                        Obj(vec![("kind", Lit(&["E"])), ("call", Str)]),
                    ]),
                ),
            ]))),
        ),
        (
            "unchecked",
            Arr(Box::new(Lit(&[
                "transformation_recognition",
                "state_guard_control_paths",
                "contest_paths",
                "fallback_paths",
                "path_enumeration_limit",
                "path_work_limit",
                "path_cell_limit",
                "path_depth_limit",
            ]))),
        ),
    ])
}

/// Validate metadata without using production DTOs, default derivation or graph
/// traversal. Called for exhaustive AND simulation, including nested reports.
#[allow(dead_code)]
pub fn validate_actor_context_wire(v: &Value) -> Result<(), String> {
    validate_actor_capacity_reason(&v["agency"], &v["actors"])?;
    for (key, ty) in [
        ("actors", actor_table_schema()),
        ("cut_context", cut_context_schema()),
    ] {
        let x = v
            .get(key)
            .ok_or_else(|| format!("actor/cut wire: missing {key}"))?;
        check(x, &ty, key)?;
        complete(x, &ty, key)?;
    }
    if let Some(actors) = v["actors"].as_array() {
        for actor in actors {
            if let Some(r) = actor.get("rule") {
                validate_rule_actor_wire(r)?;
            }
        }
    }
    validate_rule_tail_wire(v)?;
    for row in v
        .pointer("/agency/viewpoints/effects/rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        for item in row["u"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|j| j.get("kind").is_some())
        {
            validate_rule_judgement_wire(item)?;
            validate_rule_item_actor(item, &v["actors"])?;
        }
    }
    if let Some(refusal) = v["agency"]["refusal"].as_array() {
        for effect in refusal {
            let mut rules = std::collections::BTreeSet::new();
            for j in effect["judgements"]
                .as_array()
                .ok_or("missing refusal judgements")?
            {
                if j.get("kind").is_some() {
                    validate_rule_judgement_wire(j)?;
                    if !rules.insert(j["actor"].as_str().unwrap()) {
                        return Err("duplicate Rule judgement".into());
                    }
                }
            }
        }
    }
    let context = &v["cut_context"];
    if context["notices"] != serde_json::json!(CUT_NOTICES) {
        return Err("cut_context: exact ordered notices required".into());
    }
    if v["cut"].is_object() {
        if context["reentry"] != v["cut"]["reentry"] {
            return Err("cut_context: reentry differs from cut".into());
        }
    } else if context["reentry"] != serde_json::json!([])
        || context["horizon"] != serde_json::json!([])
    {
        return Err("cut_context: unresolved cut cannot invent context".into());
    }
    let mut horizons = std::collections::BTreeSet::new();
    for h in context["horizon"].as_array().unwrap() {
        if !horizons.insert(h.to_string())
            || h["to"].as_str() == Some("")
            || h["from"].as_str() == Some("")
        {
            return Err("cut_context: empty/duplicate horizon".into());
        }
    }
    let mut names = std::collections::BTreeSet::new();
    let mut declared_via = std::collections::BTreeMap::new();
    if let Some(actors) = v["actors"].as_array() {
        for a in actors {
            let name = a["name"].as_str().unwrap();
            if name.is_empty() || !names.insert(name) {
                return Err("actors: empty/duplicate name".into());
            }
            for key in [
                "grain",
                "contains",
                "overlaps",
                "capability",
                "source",
                "via_model",
                "latent",
                "state",
            ] {
                let f = &a[key];
                if (f["status"] == "default") != f["value"].is_null() {
                    return Err(format!("actors: {key} status/value mismatch"));
                }
                if let Some(items) = f["value"].as_array() {
                    let unique: std::collections::BTreeSet<_> =
                        items.iter().map(Value::to_string).collect();
                    if unique.len() != items.len() {
                        return Err(format!("actors: duplicate {key}"));
                    }
                }
            }
            if let Some(items) = a["state"]["value"].as_array()
                && (items.is_empty()
                    || items.iter().any(|s| {
                        s.as_str().is_none_or(|s| {
                            !s.starts_with(&format!("{name}."))
                                || s.split('.').count() != 2
                                || s.ends_with('.')
                        })
                    }))
            {
                return Err("actors: nonempty owner-qualified state required".into());
            }
            if a["control"]["status"] == "default" && a["control"]["value"] == "party" {
                return Err("actors: party cannot be default".into());
            }
            if a["capacity"]["status"] == "declared" && a["capacity"]["value"].is_null() {
                return Err("actors: declared capacity null".into());
            }
            let ex = &a["executors"];
            if ex["status"] == "declared" {
                if ex["value"]["kind"] != "named" {
                    return Err("actors: declared executors must be named".into());
                }
                let ids = ex["value"]["ids"].as_array().unwrap();
                if ids.is_empty()
                    || ids.len()
                        != ids
                            .iter()
                            .map(Value::to_string)
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                    || a["capacity"]["value"].as_u64() != Some(ids.len() as u64)
                {
                    return Err("actors: named executor count/capacity mismatch".into());
                }
            } else if !ex["value"].is_null() {
                if ex["value"]["kind"] != "anonymous"
                    || ex["value"]["count"] != a["capacity"]["value"]
                {
                    return Err("actors: anonymous capacity mismatch".into());
                }
            } else if !a["capacity"]["value"].is_null() {
                return Err("actors: capacity needs executors projection".into());
            }
            if let Some(model) = a["via_model"]["value"].as_str() {
                let parts: Vec<_> = model.split('@').collect();
                if parts.len() != 2
                    || parts.iter().any(|s| s.is_empty())
                    || model.chars().any(|c| c.is_whitespace() || c.is_control())
                {
                    return Err("actors: malformed via model".into());
                }
                declared_via.insert(name, model);
            }
        }
        for a in actors {
            for key in ["contains", "overlaps"] {
                if let Some(items) = a[key]["value"].as_array() {
                    for item in items {
                        if !names.contains(item.as_str().unwrap()) || item == &a["name"] {
                            return Err("actors: unresolved/self relationship".into());
                        }
                    }
                }
            }
        }
    }
    if declared_via.is_empty() != v.get("via_model_paths").is_none() {
        return Err("via paths: presence must match via declarations".into());
    }
    if let Some(paths) = v.get("via_model_paths") {
        check(paths, &via_model_schema(), "via_model_paths")?;
        complete(paths, &via_model_schema(), "via_model_paths")?;
        let rows = paths["paths"].as_array().unwrap();
        let count = paths["path_count_lower_bound"].as_u64().unwrap();
        if rows.len() > 4096
            || count < rows.len() as u64
            || (paths["truncated"] == false && count != rows.len() as u64)
        {
            return Err("via paths: count/truncation mismatch".into());
        }
        let unchecked = paths["unchecked"].as_array().unwrap();
        if unchecked.len()
            != unchecked
                .iter()
                .map(Value::to_string)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        {
            return Err("via paths: duplicate unchecked".into());
        }
        let has_limit = unchecked
            .iter()
            .any(|x| x.as_str().unwrap().ends_with("_limit"));
        if (paths["truncated"] == true) != has_limit {
            return Err("via paths: truncation requires limit reason".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for row in rows {
            if !seen.insert(row.to_string())
                || declared_via.get(row["actor"].as_str().unwrap()).copied()
                    != row["model"].as_str()
            {
                return Err("via paths: duplicate/undeclared origin".into());
            }
            let calls = row["calls"].as_array().unwrap();
            if calls.is_empty()
                || calls[0]["call"] != row["start_call"]
                || calls.last().unwrap()["call"] != row["target"]["call"]
            {
                return Err("via paths: invalid endpoints".into());
            }
            if calls[0]["capability"].as_str().unwrap().split('.').next() != row["actor"].as_str() {
                return Err("via paths: start capability actor".into());
            }
            let unique: std::collections::BTreeSet<_> =
                calls.iter().map(|c| c["call"].to_string()).collect();
            if unique.len() != calls.len() {
                return Err("via paths: call cycle".into());
            }
        }
    }
    Ok(())
}

/// ADR 0022 D1: closed seventeenth population, not an open ctx-* catch-all.
#[allow(dead_code)]
pub const EXCLUDES_WAIVE_FIXTURES: [&str; 12] = [
    "ctx-excludes",
    "ctx-excludes.broken-start",
    "ctx-nonwaivable",
    "ctx-nonwaivable.broken-waive",
    "ctx-obligation",
    "ctx-obligation.broken-excludes-target",
    "ctx-waive",
    "ctx-waive-authority",
    "ctx-waive-authority.broken-no-authority",
    "ctx-waive-names",
    "ctx-waive-names.broken-unknown-response",
    "ctx-waive.broken-after-deadline",
];

/// ADR 0022 D2: the eighteenth closed population.
#[allow(dead_code)]
pub const CONTEST_RUNTIME_FIXTURES: [&str; 12] = [
    "ctx-contest",
    "ctx-contest-branches",
    "ctx-contest-branches.broken-zero-capacity",
    "ctx-contest-clock",
    "ctx-contest-clock.broken-short-clock",
    "ctx-contest-metrics",
    "ctx-contest-metrics.broken-missing-route-rate",
    "ctx-contest-origin",
    "ctx-contest-origin.broken-arrival-clock",
    "ctx-contest-resource",
    "ctx-contest-resource.broken-slow-route",
    "ctx-contest.broken-late",
];

fn contest_runtime_schema() -> Ty {
    use Ty::*;
    let range = || Null(Box::new(Obj(vec![("min", Int), ("max", Int)])));
    Arr(Box::new(Obj(vec![
        ("id", Str),
        ("static_id", Str),
        ("case_id", Int),
        ("party", Str),
        ("target", Str),
        ("target_kind", OptionalField(Box::new(Lit(&["effect"])))),
        ("route", Str),
        ("respond_by", Int),
        ("basis", Lit(&["モデル内での期限内応答"])),
        (
            "coverage",
            Lit(&["complete", "truncated", "aborted", "range_stopped"]),
        ),
        (
            "alternatives",
            Arr(Box::new(Obj(vec![
                ("filed", Bool),
                ("target_at", Int),
                ("filed_at", Null(Box::new(Int))),
                ("deadline", Null(Box::new(Int))),
                (
                    "status",
                    Lit(&["not_filed", "responded", "violated", "unresolved"]),
                ),
                ("response_at", Null(Box::new(Int))),
                ("closed_at", Null(Box::new(Int))),
                ("executor", Null(Box::new(Str))),
                ("result", Null(Box::new(Str))),
                (
                    "violation_reason",
                    Null(Box::new(Lit(&[
                        "late_response",
                        "deadline_passed_unresolved",
                        "missing_route",
                        "permission",
                        "deadlock",
                        "cancelled",
                        "refused",
                    ]))),
                ),
                ("route_work", range()),
                ("route_wait", range()),
                ("route_cost", metric_scalar_schema()),
            ]))),
        ),
    ])))
}

fn contest_summary_schema() -> Ty {
    Ty::Obj(vec![("business_ended_open", metric_scalar_schema())])
}

/// Independent closed schema plus temporal and identity constraints. This is
/// called for every nested report through validate_wire, not only fixtures.
#[allow(dead_code)]
pub fn validate_contest_runtime_wire(v: &Value) -> Result<(), String> {
    let Some(table) = v.get("contest_runtime") else {
        if v.get("contest_summary").is_some()
            || (v["mode"] != "simulation"
                && v["properties"].as_array().is_some_and(|ps| {
                    ps.iter()
                        .any(|p| p["id"].as_str().is_some_and(|id| id.ends_with(" contest")))
                }))
        {
            return Err(
                "contest_runtime: exhaustive contest must carry runtime and summary tables".into(),
            );
        }
        return Ok(());
    };
    check(table, &contest_runtime_schema(), "contest_runtime")?;
    complete(table, &contest_runtime_schema(), "contest_runtime")?;
    let summary = v
        .get("contest_summary")
        .ok_or("contest_runtime: missing contest_summary")?;
    check(summary, &contest_summary_schema(), "contest_summary")?;
    complete(summary, &contest_summary_schema(), "contest_summary")?;
    let count = &summary["business_ended_open"];
    if v["bounds"]["truncated"] == true {
        if !count["min"].is_null() || !count["max"].is_null() || count["reason"] != "truncated" {
            return Err("contest_summary: cut count must be truncated".into());
        }
    } else if exhaustive_word(v) == "range_stopped" {
        if !count["min"].is_null() || !count["max"].is_null() || count["reason"] != "range_stopped"
        {
            return Err("contest_summary: range-stopped count must be range_stopped".into());
        }
    } else if count["reason"].is_null() {
        let min = count["min"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or("contest_summary: integer count")?;
        let max = count["max"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or("contest_summary: integer count")?;
        if min > max || max > v["bounds"]["cases"].as_u64().unwrap_or(0) {
            return Err("contest_summary: invalid count range".into());
        }
    } else if count["reason"] != "aborted" || !count["min"].is_null() || !count["max"].is_null() {
        return Err("contest_summary: unsupported reason".into());
    }
    let properties = v["properties"]
        .as_array()
        .ok_or("contest_runtime: properties")?;
    let mut identities = std::collections::BTreeSet::new();
    for row in table.as_array().unwrap() {
        let path = v["contest_paths"]
            .as_array()
            .and_then(|ps| ps.iter().find(|p| p["id"] == row["static_id"]));
        if let Some(path) = path {
            if path.get("effect").is_some() {
                if row.get("target_kind") != Some(&Value::String("effect".into()))
                    || row["target"] != path["effect"]
                    || !commit_target_name(&row["target"])
                {
                    return Err("contest_runtime: effect target/tag mismatch".into());
                }
            } else if row.get("target_kind").is_some() || row["target"] != path["end"] {
                return Err("contest_runtime: terminal target/tag mismatch".into());
            }
        } else if row.get("target_kind").is_some() != commit_target_name(&row["target"]) {
            return Err("contest_runtime: commit target requires its effect tag".into());
        }
        if v["mode"] != "simulation"
            && (row["coverage"] == "range_stopped") != (exhaustive_word(v) == "range_stopped")
        {
            return Err("contest_runtime: range_stopped coverage/bounds mismatch".into());
        }
        let case = row["case_id"].as_u64().unwrap();
        let static_id = row["static_id"].as_str().unwrap();
        // The runtime id numbers contests in model order (engine.rs and
        // reference.rs: contest index + 1), not by the clause ordinal in the
        // static id; they coincide only when each contest is its contract's
        // first clause.
        let ordinal = properties
            .iter()
            .filter_map(|p| p["id"].as_str())
            .filter(|id| id.ends_with(" contest"))
            .position(|id| id == static_id)
            .map(|i| i + 1)
            .ok_or("contest_runtime: ordinal")?;
        if case == 0
            || case > v["bounds"]["cases"].as_u64().unwrap_or(0)
            || row["id"] != format!("case{case}:contest{ordinal}")
            || !identities.insert((case, static_id))
        {
            return Err("contest_runtime: invalid/duplicate identity".into());
        }
        let pos = properties
            .iter()
            .position(|p| p["id"] == static_id)
            .ok_or("contest_runtime: static row missing")?;
        if properties
            .get(pos + 1)
            .is_none_or(|p| p["id"] != format!("{static_id}_response"))
        {
            return Err("contest_runtime: runtime row must follow static row".into());
        }
        let mut alternatives = std::collections::BTreeSet::new();
        for a in row["alternatives"].as_array().unwrap() {
            if !alternatives.insert(a.to_string()) {
                return Err("contest_runtime: duplicate alternative".into());
            }
            let target = a["target_at"].as_u64().unwrap();
            let filed = a["filed"].as_bool().unwrap();
            if !filed {
                if a["status"] != "not_filed"
                    || a["closed_at"] != target
                    || [
                        "filed_at",
                        "deadline",
                        "response_at",
                        "executor",
                        "result",
                        "violation_reason",
                        "route_work",
                        "route_wait",
                    ]
                    .iter()
                    .any(|k| !a[k].is_null())
                {
                    return Err("contest_runtime: declined is not a response".into());
                }
                // A declined branch created no route work on any path, cut or not.
                if a["route_cost"]
                    != serde_json::json!({"min":null,"max":null,"reason":"never_created"})
                {
                    return Err("contest_runtime: declined branch cost is never_created".into());
                }
                continue;
            }
            if a["filed_at"] != target
                || a["deadline"].as_u64() != target.checked_add(row["respond_by"].as_u64().unwrap())
            {
                return Err("contest_runtime: deadline must originate at target end".into());
            }
            let response = a["response_at"].as_u64();
            if response.is_some_and(|t| t < target) {
                return Err("contest_runtime: response before target".into());
            }
            match a["status"].as_str().unwrap() {
                "responded"
                    if response.is_none()
                        || response > a["deadline"].as_u64()
                        || a["closed_at"] != a["response_at"]
                        || a["result"].is_null()
                        || !a["violation_reason"].is_null() =>
                {
                    return Err("contest_runtime: invalid on-time response".into());
                }
                "violated" if a["violation_reason"].is_null() => {
                    return Err("contest_runtime: violation needs reason".into());
                }
                "unresolved"
                    if !a["closed_at"].is_null()
                        || !a["response_at"].is_null()
                        || !a["result"].is_null()
                        || row["coverage"] == "complete" =>
                {
                    return Err("contest_runtime: unresolved requires cut".into());
                }
                "not_filed" => return Err("contest_runtime: filed cannot be not_filed".into()),
                _ => {}
            }
            // deadline_passed_unresolved explains a violated row only through an
            // unresolved branch whose deadline lies before the cut.
            match (
                a["status"].as_str().unwrap(),
                a["violation_reason"].as_str(),
            ) {
                ("violated", Some("deadline_passed_unresolved")) => {
                    return Err("contest_runtime: a violated branch has a concrete reason".into());
                }
                ("unresolved", Some(r)) if r != "deadline_passed_unresolved" => {
                    return Err("contest_runtime: unresolved branch reason".into());
                }
                _ => {}
            }
            if a["status"] == "violated" {
                match a["violation_reason"].as_str().unwrap() {
                    "late_response"
                        if response.is_none()
                            || response <= a["deadline"].as_u64()
                            || a["result"].is_null()
                            || a["closed_at"] != a["response_at"] =>
                    {
                        return Err("contest_runtime: impossible late response".into());
                    }
                    "missing_route" | "deadlock" | "cancelled" | "refused"
                        if response.is_some()
                            || !a["result"].is_null()
                            || !a["closed_at"].is_null() =>
                    {
                        return Err(
                            "contest_runtime: unresponded violation cannot have result/closure"
                                .into(),
                        );
                    }
                    "permission" if a["executor"].is_null() => {
                        return Err(
                            "contest_runtime: permission violation needs actual executor".into(),
                        );
                    }
                    _ => {}
                }
            }
            metric_tree(
                &a["route_cost"],
                false,
                row["coverage"].as_str().unwrap(),
                0,
            )?;
            for key in ["route_work", "route_wait"] {
                if !a[key].is_null() && a[key]["min"].as_u64() > a[key]["max"].as_u64() {
                    return Err("contest_runtime: reversed range".into());
                }
            }
        }
    }
    Ok(())
}

fn obligation_schema() -> Ty {
    use Ty::*;
    Arr(Box::new(Obj(vec![
        ("id", Str),
        ("response_id", Str),
        ("response_label", Null(Box::new(Str))),
        ("case_id", Int),
        ("trigger_site", Str),
        ("trigger", Str),
        ("targets", Arr(Box::new(Str))),
        ("debtor", Null(Box::new(Str))),
        ("debtor_status", Lit(&["not_configured", "declared"])),
        ("non_waivable", Bool),
        (
            "coverage",
            Lit(&["complete", "truncated", "aborted", "range_stopped"]),
        ),
        (
            "outcomes",
            Arr(Box::new(Obj(vec![
                (
                    "status",
                    Lit(&["fulfilled", "waived", "violated", "unresolved"]),
                ),
                ("triggered_at", Int),
                ("deadline", Int),
                ("resolved_at", Null(Box::new(Int))),
                ("reason", Null(Box::new(Str))),
                ("by_clause", Null(Box::new(Str))),
                ("executor", Null(Box::new(Str))),
                ("authority", Null(Box::new(Str))),
            ]))),
        ),
    ])))
}

/// Validate the complete new table independently of production serde and the
/// engine. Used by all report-containing commands through validate_wire.
#[allow(dead_code)]
pub fn validate_obligations_wire(v: &Value) -> Result<(), String> {
    let Some(table) = v.get("obligations") else {
        return Ok(());
    };
    check(table, &obligation_schema(), "obligations")?;
    complete(table, &obligation_schema(), "obligations")?;
    if v["properties"].as_array().is_some_and(Vec::is_empty) {
        return Err("obligations: static rejection must omit table".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut sites = std::collections::BTreeSet::new();
    for row in table.as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let case = row["case_id"].as_u64().unwrap();
        let response = row["response_id"].as_str().unwrap();
        let site = row["trigger_site"].as_str().unwrap();
        let ordinal = response
            .split('#')
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .ok_or("obligations: response ordinal missing")?;
        if id != format!("case{case}:response{ordinal}:{site}") {
            return Err("obligations: noncanonical instance id".into());
        }
        if (row["debtor_status"] == "not_configured") != row["debtor"].is_null()
            || row["debtor"].as_str() == Some("")
            || id.is_empty()
            || case == 0
            || site.is_empty()
            || !response.ends_with(" response")
            || !ids.insert(id)
            || !sites.insert((case, response, site))
        {
            return Err("obligations: duplicate/invalid instance identity".into());
        }
        if row["trigger"].as_str().unwrap().is_empty()
            || row["targets"].as_array().unwrap().is_empty()
        {
            return Err("obligations: missing trigger or targets".into());
        }
        let properties = v["properties"]
            .as_array()
            .ok_or("obligations: missing properties")?;
        if !properties.iter().any(|p| p["id"] == response) {
            return Err("obligations: response row missing".into());
        }
        if v["bounds"]["cases"].as_u64().is_some_and(|n| case > n) {
            return Err("obligations: case outside bounds".into());
        }
        let coverage = row["coverage"].as_str().unwrap();
        if (coverage == "complete" && exhaustive_word(v) != "complete")
            || (coverage == "truncated" && v["bounds"]["truncated"] != true)
            || (coverage == "aborted" && !v["bounds"]["truncated"].is_null())
            || (coverage == "range_stopped") != (exhaustive_word(v) == "range_stopped")
        {
            return Err("obligations: coverage/bounds mismatch".into());
        }
        let mut outcomes = std::collections::BTreeSet::new();
        for o in row["outcomes"].as_array().unwrap() {
            if !outcomes.insert(o.to_string()) {
                return Err("obligations: duplicate outcome".into());
            }
            let trigger = o["triggered_at"].as_u64().unwrap();
            let deadline = o["deadline"].as_u64().unwrap();
            let resolved = o["resolved_at"].as_u64();
            if deadline < trigger || resolved.is_some_and(|r| r < trigger) {
                return Err("obligations: impossible times".into());
            }
            let has_metadata = ["reason", "by_clause", "executor", "authority"]
                .iter()
                .all(|k| o[k].as_str().is_some_and(|s| !s.is_empty()));
            let null_waiver = ["by_clause", "executor", "authority"]
                .iter()
                .all(|k| o[k].is_null());
            match o["status"].as_str().unwrap() {
                "waived" => {
                    if row["non_waivable"] != false
                        || !has_metadata
                        || resolved.is_none_or(|r| r > deadline)
                        || !properties.iter().any(|p| {
                            p["id"] == o["by_clause"]
                                && p["id"].as_str().is_some_and(|s| s.ends_with(" waive"))
                        })
                    {
                        return Err("obligations: invalid waiver".into());
                    }
                }
                "fulfilled" => {
                    if !null_waiver
                        || !o["reason"].is_null()
                        || resolved.is_none_or(|r| r > deadline)
                    {
                        return Err("obligations: fulfillment is not waiver".into());
                    }
                }
                "violated" => {
                    if !null_waiver
                        || resolved.is_none()
                        || !["expiry", "terminal_unresolved", "deadlock_unresolved"]
                            .contains(&o["reason"].as_str().unwrap_or(""))
                        || (o["reason"] == "expiry" && resolved != Some(deadline))
                    {
                        return Err("obligations: invalid violation reason/time".into());
                    }
                }
                "unresolved" => {
                    // ADR 0029 U2: a branch stopped at an out-of-range write
                    // leaves the obligation unknown, only beside a range stop.
                    let range = o["reason"] == "state_range_violation"
                        && v["bounds"]["range_violation_stops"] == true;
                    // ADR0042 ruling: a complete observation window is not
                    // fulfillment. This exception is unavailable to old
                    // reports and to obligations already due at D.
                    let business_horizon = coverage == "complete"
                        && v["bounds"]["truncated"] == false
                        && o["reason"] == "horizon"
                        && v["scenario"]["horizon"]
                            .as_u64()
                            .is_some_and(|d| deadline > d);
                    if !null_waiver
                        || resolved.is_some()
                        || coverage == "complete" && !business_horizon
                        || !range
                            && !business_horizon
                            && !["clock", "live_tasks", "state_budget", "aborted"]
                                .contains(&o["reason"].as_str().unwrap_or(""))
                    {
                        return Err("obligations: unresolved is a cut, not success".into());
                    }
                }
                _ => unreachable!(),
            }
        }
        if outcomes.is_empty() {
            return Err("obligations: empty instance outcomes".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod scenario_obligation_wire_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn complete_observation_can_leave_only_a_strictly_future_horizon_obligation_unresolved() {
        let v = json!({"bounds":{"cases":1,"truncated":false},"scenario":{"name":"Day","horizon":2},
            "properties":[{"id":"contract C #1 response","verdict":"inconclusive",
                "cases":[{"case_id":1,"arrival":0,"verdict":"inconclusive"}]}],
            "obligations":[{"id":"case1:response1:received","response_id":"contract C #1 response",
                "response_label":null,"case_id":1,"trigger_site":"received","trigger":"case.received",
                "targets":["A.run.finish"],"debtor":null,"debtor_status":"not_configured",
                "non_waivable":false,"coverage":"complete","outcomes":[{"status":"unresolved",
                    "triggered_at":0,"deadline":3,"resolved_at":null,"reason":"horizon",
                    "by_clause":null,"executor":null,"authority":null}]}]});
        validate_obligations_wire(&v).unwrap();
        assert_eq!(v["properties"][0]["cases"][0]["verdict"], "inconclusive");
        for (pointer, new) in [
            ("/bounds/truncated", json!(true)),
            ("/scenario/horizon", Value::Null),
            ("/obligations/0/outcomes/0/deadline", json!(2)),
            ("/obligations/0/outcomes/0/reason", json!("clock")),
        ] {
            let mut bad = v.clone();
            let at = bad.pointer_mut(pointer).unwrap();
            assert_ne!(*at, new);
            *at = new;
            assert!(
                validate_obligations_wire(&bad).is_err(),
                "missing horizon permission condition {pointer}"
            );
        }
        let mut old = v.clone();
        old.as_object_mut().unwrap().remove("scenario");
        assert!(
            validate_obligations_wire(&old).is_err(),
            "no new permission on an old report"
        );
        let mut earlier = v.clone();
        earlier["obligations"][0]["outcomes"][0]["deadline"] = json!(1);
        assert!(
            validate_obligations_wire(&earlier).is_err(),
            "already due is not unknown"
        );
    }
}

/// ADR 0021: sixteenth population, independent of the old 243 directories.
#[allow(dead_code)] // Included by other command-specific integration targets.
pub const EXPLAIN_FIXTURES: [&str; 12] = [
    "explain-deadlock",
    "explain-deadlock.broken-zero-capacity",
    "explain-executors",
    "explain-executors.broken-shared-worker",
    "explain-invariant",
    "explain-invariant.broken-cut-guard",
    "explain-once",
    "explain-once.broken-repeat-result",
    "explain-requires",
    "explain-requires.broken-cut-guard",
    "explain-response",
    "explain-response.broken-late-finish",
];

/// ADR 0020: fifteenth fixed population. Unknown mut-* is never ignored.
pub const MUTATION_FIXTURES: [&str; 12] = [
    "mut-empty",
    "mut-empty.broken-invalid",
    "mut-evidence",
    "mut-evidence.broken-no-independent-read",
    "mut-gate",
    "mut-gate.broken-unsupported",
    "mut-guard",
    "mut-guard.broken-no-contract",
    "mut-limit",
    "mut-limit.broken-many-sites",
    "mut-origin",
    "mut-origin.broken-route-only",
];
const MUTATION_KINDS: [&str; 10] = [
    "evidence_missing",
    "authority_expired",
    "hold_delayed",
    "command_resent",
    "summary_reused",
    "stale_definition_write",
    "receipt_unknown",
    "old_promise_denominator_removed",
    "refusal_path_cut",
    "original_access_removed",
];
const MUTATION_NAMES: [&str; 10] = [
    "Evidence の欠落",
    "Authority の失効",
    "Hold の遅延",
    "同じ Command の再送",
    "同じ要約の多重利用",
    "定義更新中の旧版書込み",
    "外部 Receipt の unknown",
    "旧約束を分母から消す",
    "拒否経路の切断",
    "原本到達の削除",
];
const MUTATION_UNSUPPORTED: [(usize, &str); 4] = [
    (3, "command_resend_unavailable"),
    (5, "rule_version_unavailable"),
    (6, "receipt_unknown_semantics_unavailable"),
    (7, "quality_denominator_unavailable"),
];

const MUTATION_ASSUMPTIONS: [(&str, &str); 9] = [
    ("mutation.purpose", "weakness_candidates_not_quality_score"),
    ("mutation.detection", "assigned_checks_only"),
    ("mutation.original", "not_established_correct"),
    ("mutation.analysis", "exhaustive_only"),
    ("mutation.limit", "computational_128"),
    ("mutation.provenance", "original_source_plus_ast_edit"),
    ("mutation.guard_scope", "terminal_guard_bypass"),
    (
        "mutation.authority_scope",
        "expires_zero_on_basis_authorities",
    ),
    ("mutation.summary_scope", "duplicate_producer_one_consumer"),
];
const MUTATION_SCOPE_AFFECTS: [&str; 2] = [
    "authority_basis が名指す Authority だけの明示の期限を0minにする。時刻0の事象は期限内。動的失効と名指されない Authority は未検査。",
    "結果を2つ以上の呼出しに渡す作る側を直後に複製し、2番目以降の消費者を1つだけ付け替える。caseの変数と潜在は共有され再び書かれる。summary_reused は既存の悪化検出の例外で、根拠集合の一致とgroup判定数の非減少を検出とする。この種は Relca の系譜の規則を確かめるもので、モデルの強さを示さない。fork枝の作る側、共有副作用、後のprocess guardが読む観測変数の書き手を除く。guardとreadsは付け替えない。other_changesはquality・via_model_paths等の閉じた欄の外を記録しない。",
];

/// ADR 0019: fourteenth fixed population, 24 dirs / 28 selected inputs.
pub const METRICS_FIXTURES: [&str; 24] = [
    "met-cancel",
    "met-cancel.broken-drain",
    "met-compare",
    "met-compare.broken-longer-task",
    "met-diff",
    "met-diff.broken-free-start",
    "met-exclusions",
    "met-exclusions.broken-larger-queue",
    "met-fifo",
    "met-fifo.broken-shorter-second-step",
    "met-legacy",
    "met-legacy.broken-enabled",
    "met-missing-rate",
    "met-missing-rate.broken-free-rate",
    "met-parallel",
    "met-parallel.broken-one-slot",
    "met-person",
    "met-person.broken-shared-person",
    "met-positive-support",
    "met-positive-support.broken-positive-early-time",
    "met-sample",
    "met-sample.broken-short-clock",
    "met-serial",
    "met-serial.broken-longer-task",
];

/// Keep the pre-configuration population fixed; new fixtures have a separate
/// invocation-aware population and never replace a legacy check regression.
pub const LEGACY_FIXTURES: [&str; 16] = [
    "any-drain",
    "any-drain.broken-missing-cancel",
    "coop-cancel",
    "coop-cancel.broken-short-clock",
    "cut-declared",
    "cut-declared.broken-excluded-call",
    "cut-declared.broken-missing-loses",
    "cut-declared.broken-missing-member",
    "nondet-race",
    "nondet-race.broken-quick-never-passes",
    "parallel-time",
    "parallel-time.broken-missing-join",
    "resource-contention",
    "resource-contention.broken-zero-capacity",
    "serial-time",
    "serial-time.broken-missing-capacity",
];

/// Separate fixed population; never absorbed into the 16 legacy / 12 config cases.
pub const AGENCY_FIXTURES: [&str; 9] = [
    "agency-any",
    "agency-any.broken-single-valued-a",
    "agency-lineage",
    "agency-lineage.broken-missing-evidence",
    "agency-lineage.broken-reused-source",
    "agency-origin",
    "agency-origin.broken-no-direct-read",
    "agency-refusal",
    "agency-refusal.broken-cut-guard",
];

/// ADR 0010: fifth fixed population, independent of all pre-contract cases.
pub const CONTRACT_FIXTURES: [&str; 19] = [
    "contract-cancel",
    "contract-cancel.broken-ignore-cancel",
    "contract-cut",
    "contract-cut.broken-late-target",
    "contract-diff",
    "contract-diff.broken-zero-deadline",
    "contract-invariant",
    "contract-invariant.broken-cut-guard",
    "contract-once",
    "contract-once.broken-repeat-result",
    "contract-order",
    "contract-order.broken-parallel",
    "contract-requires",
    "contract-requires.broken-cut-guard",
    "contract-response",
    "contract-response.broken-late-finish",
    "contract-response.broken-short-clock",
    "contract-vacuous",
    "contract-vacuous.broken-enable-branch",
];

/// ADR 0011: sixth fixed population, never folded into legacy or contract cases.
pub const EXECUTOR_FIXTURES: [&str; 14] = [
    "exec-anonymous",
    "exec-anonymous.broken-no-executor",
    "exec-authority",
    "exec-authority.broken-missing-grant",
    "exec-cancel",
    "exec-cancel.broken-ignore-cancel",
    "exec-permitted",
    "exec-permitted.broken-narrow-permit",
    "exec-separation",
    "exec-separation.broken-shared-approver",
    "exec-shared-person",
    "exec-shared-person.broken-reused-person",
    "exec-vacuous",
    "exec-vacuous.broken-shared-approver",
];

/// ADR 0012: seventh fixed population, sixteen inputs, thirteen runtime checks.
pub const STATE_FIXTURES: [&str; 16] = [
    "state-agency",
    "state-agency.broken-state-guard",
    "state-cancel",
    "state-cancel.broken-ignore-cancel",
    "state-invoice-write",
    "state-invoice-write.broken-failed-approval",
    "state-latent-product",
    "state-latent-product.broken-partial-assertion",
    "state-name-alias",
    "state-name-alias.broken-missing-write",
    "state-observed-range",
    "state-observed-range.broken-fixed-route",
    "state-policy",
    "state-policy.broken-latent-difficulty",
    "state-writer-order",
    "state-writer-order.broken-parallel-writers",
];

/// ADR 0013: eighth fixed population, 18 inputs / 16 runtime / 2 static errors.
pub const PROB_FIXTURES: [&str; 16] = [
    "prob-cancel",
    "prob-cancel.broken-ignore-cancel",
    "prob-cut",
    "prob-cut.broken-short-clock",
    "prob-finish-read",
    "prob-finish-read.broken-late-writer",
    "prob-invoice",
    "prob-invoice.broken-ai-transfer",
    "prob-mdp",
    "prob-mdp.broken-tight-upper",
    "prob-rows",
    "prob-rows.broken-missing-difficult",
    "prob-sum",
    "prob-sum.broken-mass",
    "prob-zero",
    "prob-zero.broken-positive-write",
];

/// ADR 0014: ninth fixed population; eighteen checks, two static rejections.
pub const DURATION_FIXTURES: [&str; 18] = [
    "duration-cancel",
    "duration-cancel.broken-ignore-cancel",
    "duration-clock",
    "duration-clock.broken-short-clock",
    "duration-contention",
    "duration-contention.broken-extra-capacity",
    "duration-duplicate",
    "duration-duplicate.broken-duplicate-value",
    "duration-no-lookahead",
    "duration-no-lookahead.broken-late-choice",
    "duration-parallel",
    "duration-parallel.broken-tight-upper",
    "duration-serial",
    "duration-serial.broken-tight-upper",
    "duration-sum",
    "duration-sum.broken-mass",
    "duration-zero",
    "duration-zero.broken-late-positive",
];

/// ADR 0015: tenth, disjoint population: 24 inputs / 20 runtime / 4 static.
pub const ARRIVALS_FIXTURES: [&str; 24] = [
    "arrivals-contracts",
    "arrivals-contracts.broken-short-response",
    "arrivals-count",
    "arrivals-count.broken-case-count",
    "arrivals-initial-product",
    "arrivals-initial-product.broken-fixed-initial",
    "arrivals-order",
    "arrivals-order.broken-descending",
    "arrivals-reject-mass",
    "arrivals-reject-mass.broken-larger-queue",
    "arrivals-relative-within",
    "arrivals-relative-within.broken-tight-deadline",
    "arrivals-required",
    "arrivals-required.broken-missing-arrivals",
    "arrivals-same-time-slot",
    "arrivals-same-time-slot.broken-later-arrival",
    "arrivals-shared-fixed",
    "arrivals-shared-fixed.broken-unset-shared",
    "arrivals-simultaneous",
    "arrivals-simultaneous.broken-no-policy",
    "arrivals-staggered",
    "arrivals-staggered.broken-extra-capacity",
    "arrivals-zero-queue",
    "arrivals-zero-queue.broken-positive-queue",
];

/// Eleventh fixed population. Never reclassify an existing input as simulation.
pub const SIM_FIXTURES: [&str; 24] = [
    "sim-actor-initial",
    "sim-actor-initial.broken-missing-distribution",
    "sim-contract",
    "sim-contract.broken-short-response",
    "sim-cut",
    "sim-cut.broken-short-clock",
    "sim-deadlock",
    "sim-deadlock.broken-zero-capacity",
    "sim-fifo",
    "sim-fifo.broken-no-policy",
    "sim-hazard",
    "sim-hazard.broken-short-clock",
    "sim-keys",
    "sim-keys.broken-flipped-query",
    "sim-range",
    "sim-range.broken-zero-replications",
    "sim-reachable",
    "sim-reachable.broken-unseen-end",
    "sim-rejection",
    "sim-rejection.broken-positive-queue",
    "sim-required",
    "sim-required.broken-missing-simulation",
    "sim-statistical",
    "sim-statistical.broken-tight-threshold",
];

/// Twelfth population. An explicit list, not a catch-all prefix exclusion.
pub const COMPARE_FIXTURES: [&str; 16] = [
    "compare-arrivals",
    "compare-arrivals.broken-slow-plan",
    "compare-case-diff",
    "compare-case-diff.broken-same-kernel",
    "compare-cut",
    "compare-cut.broken-other-cut",
    "compare-legacy",
    "compare-legacy.broken-longer-task",
    "compare-random",
    "compare-random.broken-alias-process",
    "compare-reject",
    "compare-reject.broken-zero-queue",
    "compare-terminal",
    "compare-terminal.broken-other-end",
    "compare-unlock",
    "compare-unlock.broken-excluded-call",
];

/// ADR 0018: thirteenth population; old twelve remain exactly 188 dirs.
pub const RESPONSIBILITY_FIXTURES: [&str; 19] = [
    "resp-contest",
    "resp-contest.broken-no-authority",
    "resp-contest.broken-no-permission",
    "resp-contest.broken-route-excluded",
    "resp-contest.broken-zero-deadline",
    "resp-cut",
    "resp-cut.broken-no-explicit-cut",
    "resp-diff",
    "resp-diff.broken-no-authorized-decision",
    "resp-missing-duty",
    "resp-missing-duty.broken-no-duty",
    "resp-origin",
    "resp-origin.broken-no-direct-read",
    "resp-refusal",
    "resp-refusal.broken-cut-guard",
    "resp-standing",
    "resp-standing.broken-not-party",
    "resp-state",
    "resp-state.broken-no-direct-read",
];

#[derive(Debug, PartialEq, Eq)]
pub enum Analysis {
    Exhaustive,
    Simulation,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Invocation {
    Scenario {
        base: Box<Invocation>,
        scenario: String,
    },
    Explain,
    Mutate {
        set: Option<String>,
    },
    LegacyCheck,
    Check {
        configuration: String,
    },
    Diff {
        from: String,
        to: String,
    },
    Simulate {
        configuration: Option<String>,
    },
    Compare {
        plans: Vec<String>,
        mode: Analysis,
    },
}

impl Invocation {
    pub fn parse(value: Option<&Value>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::LegacyCheck);
        };
        let object = value.as_object().ok_or("invocation must be an object")?;
        if let Some(scenario) = object.get("scenario") {
            let scenario = scenario
                .as_str()
                .filter(|s| !s.is_empty() && !s.starts_with("--"))
                .ok_or("scenario must be a nonempty selector")?
                .to_owned();
            let mut base = value.clone();
            base.as_object_mut().unwrap().remove("scenario");
            return Ok(Self::Scenario {
                base: Box::new(Self::parse(Some(&base))?),
                scenario,
            });
        }
        let string = |key| identifier(value, key, "invocation").map(str::to_owned);
        let (allowed, invocation) = match string("command")?.as_str() {
            "explain" => (&["command"][..], Self::Explain),
            "mutate" => (
                &["command", "set"][..],
                Self::Mutate {
                    set: object
                        .get("set")
                        .map(|v| {
                            v.as_str()
                                .filter(|s| *s == "bpr-default")
                                .map(str::to_owned)
                                .ok_or_else(|| "unknown mutation set".to_owned())
                        })
                        .transpose()?,
                },
            ),
            "compare" => {
                let plans: Vec<String> = object
                    .get("plans")
                    .and_then(Value::as_array)
                    .ok_or("plans must be an array")?
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .filter(|s| valid_plan_name(s))
                            .map(str::to_owned)
                            .ok_or_else(|| "plan must be an identifier".to_owned())
                    })
                    .collect::<Result<_, _>>()?;
                let unique: std::collections::BTreeSet<_> = plans.iter().collect();
                if plans.len() < 2 || unique.len() != plans.len() {
                    return Err("at least two distinct plans".into());
                }
                let mode = match string("mode")?.as_str() {
                    "exhaustive" => Analysis::Exhaustive,
                    "simulation" => Analysis::Simulation,
                    _ => return Err("unknown compare analysis".into()),
                };
                (
                    &["command", "plans", "mode"][..],
                    Self::Compare { plans, mode },
                )
            }
            "simulate" => (
                &["command", "configuration"][..],
                Self::Simulate {
                    configuration: object
                        .get("configuration")
                        .map(|_| string("configuration"))
                        .transpose()?,
                },
            ),
            "check" => (
                &["command", "configuration"][..],
                if object.contains_key("configuration") {
                    Self::Check {
                        configuration: string("configuration")?,
                    }
                } else {
                    Self::LegacyCheck
                },
            ),
            "diff" => (
                &["command", "from", "to"][..],
                Self::Diff {
                    from: string("from")?,
                    to: string("to")?,
                },
            ),
            command => return Err(format!("unknown invocation command {command}")),
        };
        if object.keys().any(|key| !allowed.contains(&key.as_str())) {
            return Err("unknown invocation key".into());
        }
        if matches!(&invocation, Self::Simulate { configuration: Some(name) } if name.is_empty() || name.starts_with("--"))
        {
            return Err("simulation configuration must be a nonempty name".into());
        }
        Ok(invocation)
    }

    pub fn args(&self, path: &std::path::Path) -> Vec<String> {
        let path = path
            .to_str()
            .expect("fixture path must be UTF-8")
            .to_owned();
        match self {
            Self::Scenario { base, scenario } => {
                let mut args = base.args(std::path::Path::new(&path));
                args.extend(["--scenario".into(), scenario.clone()]);
                args
            }
            Self::Explain => vec!["explain".into(), path],
            Self::Mutate { set } => {
                let mut args = vec!["mutate".into(), path];
                if let Some(set) = set {
                    args.extend(["--set".into(), set.clone()]);
                }
                args
            }
            Self::Compare { plans, mode } => vec![
                "compare".into(),
                path,
                "--plans".into(),
                plans.join(","),
                "--mode".into(),
                match mode {
                    Analysis::Exhaustive => "exhaustive",
                    Analysis::Simulation => "simulation",
                }
                .into(),
            ],
            Self::Simulate { configuration } => {
                let mut args = vec!["simulate".into(), path];
                if let Some(name) = configuration {
                    args.extend(["--configuration".into(), name.clone()]);
                }
                args
            }
            Self::LegacyCheck => vec!["check".into(), path],
            Self::Check { configuration } => vec![
                "check".into(),
                path,
                "--configuration".into(),
                configuration.clone(),
            ],
            Self::Diff { from, to } => vec![
                "diff".into(),
                path,
                "--from".into(),
                from.clone(),
                "--to".into(),
                to.clone(),
            ],
        }
    }

    pub fn validate_expected(&self, expected: &Value) -> Result<(), String> {
        match self {
            Self::Scenario { base, .. } => base.validate_expected(expected),
            Self::Explain => check(expected, &explain_schema(), "explain expectation"),
            Self::Mutate { .. } => check(expected, &mutation_schema(), "mutation expectation"),
            Self::Compare { .. } => check(expected, &compare_schema(), "$"),
            Self::Simulate { .. } => check(expected, &simulation_schema(), "$"),
            Self::Diff { .. } => validate_diff_expected(expected),
            _ => validate_expected(expected),
        }
    }

    pub fn compare(&self, expected: &Value, actual: &Value) -> Result<(), String> {
        self.validate_read_write_selection(actual)?;
        match self {
            Self::Scenario { base, .. } => base.compare(expected, actual),
            Self::Explain => {
                self.validate_expected(expected)?;
                validate_explain_wire(actual)?;
                compare_mutation_values(expected, actual, "explain")
            }
            Self::Mutate { .. } => {
                self.validate_expected(expected)?;
                validate_mutation_wire(actual)?;
                compare_mutation_values(expected, actual, "mutation")
            }
            Self::Compare { .. } => compare_compare_report(expected, actual),
            Self::Simulate { .. } => {
                self.validate_expected(expected)?;
                validate_simulation_wire(actual)?;
                compare(expected, actual, "$")
            }
            Self::Diff { .. } => compare_diff_report(expected, actual),
            _ => compare_report(expected, actual),
        }
    }

    fn validate_read_write_selection(&self, actual: &Value) -> Result<(), String> {
        match self {
            Self::Scenario { base, .. } => base.validate_read_write_selection(actual),
            Self::Check { configuration } => validate_read_write_pin(actual, Some(configuration)),
            Self::Simulate { configuration } => {
                validate_read_write_pin(actual, configuration.as_deref())
            }
            Self::Compare { plans, .. } => {
                for (i, configuration) in plans.iter().enumerate() {
                    if let Some(report) = actual["plans"][i].get("report") {
                        validate_read_write_pin(report, Some(configuration))?;
                    }
                }
                Ok(())
            }
            Self::Diff { from, to } => {
                if let Some(report) = actual["from"].get("report") {
                    validate_read_write_pin(report, Some(from))?;
                }
                if let Some(report) = actual["to"].get("report") {
                    validate_read_write_pin(report, Some(to))?;
                }
                Ok(())
            }
            Self::LegacyCheck | Self::Explain => validate_read_write_pin(actual, None),
            Self::Mutate { .. } => Ok(()),
        }
    }
}

const EXPLAIN_KINDS: &[&str] = &[
    "horizon",
    "initial",
    "initialize",
    "admission",
    "admitted",
    "readmission",
    "overflow",
    "receive",
    "received",
    "queue",
    "start",
    "refuse",
    "wait",
    "wake",
    "advance",
    "duration_gate",
    "finish",
    "commit",
    "adopt",
    "cancel_request",
    "cancel_ack",
    "branch",
    "join",
    "end",
    "deadlock",
    "deadline",
    "write",
    "contest_filed",
    "contest_declined",
    "contest_queue",
    "contest_response",
    "execution_close",
    "range_violation",
];
const REPLAY_REASONS: &[&str] = &[
    "zero_duration_reference_unavailable",
    "reference_limit",
    "illegal_transition",
    "event_mismatch",
    "state_mismatch",
    "violation_mismatch",
    "source_mapping_failed",
];

fn location_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        ("span", Obj(vec![("start", Int), ("end", Int)])),
        ("line", Int),
        ("column", Int),
        ("end_line", Int),
        ("end_column", Int),
    ])
}

fn explanation_schema() -> Ty {
    use Ty::*;
    let strings = || Arr(Box::new(Str));
    let ns = || Null(Box::new(Str));
    let ni = || Null(Box::new(Int));
    let loc = || Null(Box::new(location_schema()));
    let reason = || Null(Box::new(Lit(REPLAY_REASONS)));
    let resource = || {
        Obj(vec![
            ("actor", Str),
            ("capacity", Int),
            ("anonymous", Bool),
            (
                "holders",
                Arr(Box::new(Obj(vec![
                    ("case_id", Int),
                    ("binding", Str),
                    ("executor", ns()),
                ]))),
            ),
        ])
    };
    let value = || {
        Obj(vec![
            ("name", Str),
            ("value", OneOf(vec![Str, Arr(Box::new(Int))])),
            ("visibility", Lit(&["observed", "latent"])),
            ("location", location_schema()),
        ])
    };
    let common_event = || {
        vec![
            ("kind", Lit(EXPLAIN_KINDS)),
            ("case_id", ni()),
            ("binding", ns()),
            ("actor", ns()),
            ("capability", ns()),
            ("executor", ns()),
            ("result", ns()),
            ("location", location_schema()),
        ]
    };
    let event = || {
        let mut cancel = common_event();
        cancel.push(("ack_at", Int));
        OneOf(vec![Obj(common_event()), Obj(cancel), value()])
    };
    let violation = || {
        Obj(vec![
            (
                "state_invariant",
                OptionalField(Box::new(Obj(vec![
                    ("case_id", Int),
                    ("antecedent", Bool),
                    ("condition", Bool),
                    ("values", Arr(Box::new(value()))),
                ]))),
            ),
            ("id", Str),
            ("property_location", loc()),
            ("at_step", Int),
            ("trigger_step", ni()),
            ("deadline", ni()),
            ("related_steps", Arr(Box::new(Int))),
        ])
    };
    let stage = Obj(vec![
        (
            "state_range_violation",
            OptionalField(Box::new(state_range_violation_schema())),
        ),
        ("index", Int),
        ("time", Int),
        ("case_id", ni()),
        ("binding", ns()),
        ("actor", ns()),
        ("capability", ns()),
        ("executor", ns()),
        ("result", ns()),
        ("kind", Lit(EXPLAIN_KINDS)),
        ("events", Arr(Box::new(event()))),
        ("location", location_schema()),
        (
            "values",
            Obj(vec![
                ("observed", Arr(Box::new(value()))),
                ("latent", Arr(Box::new(value()))),
            ]),
        ),
        ("violation", Null(Box::new(violation()))),
        ("resource", Null(Box::new(resource()))),
        (
            "shared_resources",
            OptionalField(Box::new(shared_resources_schema())),
        ),
        ("capability_location", loc()),
        (
            "deadlock_detail",
            Null(Box::new(Obj(vec![
                (
                    "waiting_tasks",
                    Arr(Box::new(Obj(vec![
                        ("case_id", Int),
                        ("binding", Str),
                        ("actor", Str),
                        (
                            "resource_shortages",
                            OptionalField(Box::new(Arr(Box::new(Obj(vec![
                                ("resource", Str),
                                ("required", Int),
                                ("available", Int),
                                ("missing", Int),
                            ]))))),
                        ),
                    ]))),
                ),
                ("resources", Arr(Box::new(resource()))),
                (
                    "shared_resources",
                    OptionalField(Box::new(shared_resources_schema())),
                ),
                (
                    "obligations",
                    Arr(Box::new(Obj(vec![
                        ("case_id", Int),
                        ("id", Str),
                        ("trigger_step", Int),
                        ("deadline", Int),
                    ]))),
                ),
                (
                    "joins",
                    Arr(Box::new(Obj(vec![
                        ("case_id", Int),
                        ("bindings", strings()),
                    ]))),
                ),
            ]))),
        ),
    ]);
    Obj(vec![
        ("version", Lit(&["invoice-counterexample-v1"])),
        ("status", Lit(&["complete", "blocked", "unsupported"])),
        (
            "shortest_scope",
            Null(Box::new(Lit(&["model", "explored_graph"]))),
        ),
        (
            "counterexamples",
            Keyed(
                "id",
                Box::new(Obj(vec![
                    ("id", Str),
                    ("case_id", ni()),
                    ("status", Lit(&["verified", "unsupported"])),
                    ("reason", reason()),
                    ("property_location", loc()),
                    ("context_location", location_schema()),
                    ("related_locations", Arr(Box::new(location_schema()))),
                    ("transition_count", ni()),
                    ("steps", Arr(Box::new(stage))),
                    (
                        "replay",
                        Obj(vec![
                            ("status", Lit(&["verified", "unsupported"])),
                            ("reason", reason()),
                        ]),
                    ),
                ])),
            ),
        ),
        (
            "omitted",
            Keyed(
                "id",
                Box::new(Obj(vec![
                    ("id", Str),
                    ("verdict", Verdict),
                    (
                        "reason",
                        Lit(&[
                            "probabilistic_property",
                            "non_path_property",
                            "static_property",
                        ]),
                    ),
                ])),
            ),
        ),
        (
            "assumptions",
            Arr(Box::new(Obj(vec![
                ("field", Str),
                ("value", Str),
                ("affects", Str),
            ]))),
        ),
        ("diagnostics", Arr(Box::new(Diagnostic))),
        ("checked", strings()),
        ("unchecked", strings()),
    ])
}

fn explain_schema() -> Ty {
    let Ty::Obj(mut fields) = report_schema() else {
        unreachable!()
    };
    fields.push(("explanation", explanation_schema()));
    Ty::Obj(fields)
}

/// Independent closed wire validator, deliberately not production DTO serde.
pub fn validate_explain_wire(v: &Value) -> Result<(), String> {
    validate_caseset_notes(v)?;
    check(v, &explain_schema(), "explain")?;
    complete(v, &explain_schema(), "explain")?;
    let mut root = v.clone();
    root.as_object_mut().unwrap().remove("explanation");
    validate_wire(&root)?;
    let ex = &v["explanation"];
    let cs = ex["counterexamples"].as_array().unwrap();
    let properties = v["properties"].as_array().unwrap();
    let target = |id: &str| {
        id == "safety no_resource_overcommit"
            || id == "state_in_range"
            || id.starts_with("no_lost_update ")
            || (id.starts_with("contract ")
                && [
                    " invariant",
                    " at_most_once",
                    " requires_event",
                    " response",
                    " contest_response",
                    " separation",
                    " permitted",
                    " authority_basis",
                ]
                .iter()
                .any(|suffix| id.ends_with(suffix)))
    };
    let expected_ids: Vec<_> = properties
        .iter()
        .filter(|p| p["verdict"] == "violated" && target(p["id"].as_str().unwrap()))
        .map(|p| p["id"].as_str().unwrap())
        .chain(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "W-DEADLOCK-0")
                .then_some("diagnostic:W-DEADLOCK-0"),
        )
        .collect();
    let ids: Vec<_> = cs.iter().map(|c| c["id"].as_str().unwrap()).collect();
    if ids != expected_ids {
        return Err("missing/extra/reordered target candidates".into());
    }
    let omitted: Vec<_> = properties
        .iter()
        .filter(|p| !target(p["id"].as_str().unwrap()))
        .map(|p| {
            let id = p["id"].as_str().unwrap();
            let reason = if p.get("probability").is_some()
                || id.starts_with("probability ")
                || id == "almost_sure end"
            {
                "probabilistic_property"
            } else if id.ends_with(" contest") {
                "static_property"
            } else {
                "non_path_property"
            };
            serde_json::json!({"id":id,"verdict":p["verdict"],"reason":reason})
        })
        .collect();
    if ex["omitted"] != serde_json::json!(omitted) {
        return Err("omitted properties lost/reclassified/reordered".into());
    }
    let checked = ex["checked"].as_array().unwrap();
    let unchecked = ex["unchecked"].as_array().unwrap();
    for word in checked {
        if unchecked.contains(word) {
            return Err("checked also unchecked".into());
        }
    }
    for word in [
        "probabilistic_causes",
        "simulation_counterexamples",
        "real_world_validity",
    ] {
        if !unchecked.contains(&serde_json::json!(word)) {
            return Err(format!("missing boundary {word}"));
        }
    }
    if ex["status"] != "blocked" {
        for (field, value) in [
            ("explain.purpose", "example_not_cause"),
            ("explain.latent", "analyst_only"),
            ("explain.replay", "independent_reference_prefix"),
        ] {
            let rows: Vec<_> = ex["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|a| a["field"] == field)
                .collect();
            if rows.len() != 1 || rows[0]["value"] != value {
                return Err(format!("missing/incorrect assumption {field}"));
            }
        }
        let rows: Vec<_> = ex["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["field"] == "explain.shortest")
            .collect();
        if rows.len() != 1 || rows[0]["value"] != ex["shortest_scope"] {
            return Err("shortest assumption mismatch".into());
        }
    }
    let mut failed = false;
    for c in cs {
        let steps = c["steps"].as_array().unwrap();
        if c["status"] == "unsupported" {
            failed = true;
            if !steps.is_empty()
                || !c["transition_count"].is_null()
                || c["reason"].is_null()
                || c["replay"]["status"] != "unsupported"
                || c["replay"]["reason"] != c["reason"]
            {
                return Err("unverified path published or missing failure reason".into());
            }
            continue;
        }
        if !c["reason"].is_null()
            || c["replay"]["status"] != "verified"
            || !c["replay"]["reason"].is_null()
            || steps.is_empty()
            || c["transition_count"].as_u64() != Some((steps.len() - 1) as u64)
        {
            return Err("verified path/replay/count mismatch".into());
        }
        for (i, s) in steps.iter().enumerate() {
            if let Some(rows) = s.get("shared_resources") {
                validate_shared_resource_snapshots(rows)?;
            }
            if let Some(rows) = s["deadlock_detail"].get("shared_resources") {
                validate_shared_resource_snapshots(rows)?;
            }
            if let Some(waiting) = s["deadlock_detail"]["waiting_tasks"].as_array() {
                for task in waiting {
                    if let Some(shortages) = task.get("resource_shortages") {
                        let rows = shortages.as_array().ok_or("shortages must be array")?;
                        if rows.is_empty() {
                            return Err("empty shortages must be omitted".into());
                        }
                        unique_composite(rows, &["resource"])?;
                        for r in rows {
                            let required = r["required"].as_u64().ok_or("required")?;
                            let available = r["available"].as_u64().ok_or("available")?;
                            if required <= available
                                || r["missing"].as_u64() != required.checked_sub(available)
                            {
                                return Err("resource shortage arithmetic".into());
                            }
                        }
                    }
                }
            }
            if s["index"].as_u64() != Some(i as u64)
                || (i > 0 && s["time"].as_u64() < steps[i - 1]["time"].as_u64())
            {
                return Err("noncontiguous path or reversed clock".into());
            }
            if i == 0 && s["kind"] != "initial" {
                return Err("missing initial stage".into());
            }
            for visibility in ["observed", "latent"] {
                for value in s["values"][visibility].as_array().unwrap() {
                    if value["visibility"] != visibility {
                        return Err("mixed latent/observed values".into());
                    }
                    validate_caseset_value(value, v["bounds"]["cases"].as_u64())?;
                }
            }
            for event in s["events"].as_array().unwrap() {
                if event.get("value").is_some() && event.get("visibility").is_some() {
                    validate_caseset_value(event, v["bounds"]["cases"].as_u64())?;
                }
            }
            if s["kind"] == "cancel_ack"
                && (!s["result"].is_null()
                    || s["events"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|e| e["kind"] == "finish" || e["kind"] == "commit"))
            {
                return Err("cancellation is not normal finish".into());
            }
            if s["kind"] == "refuse"
                && (!s["executor"].is_null()
                    || s["result"].is_null()
                    || s["events"].as_array().unwrap().iter().any(|e| {
                        ["start", "finish", "commit", "cancel_ack"]
                            .iter()
                            .any(|k| e["kind"] == *k)
                    }))
            {
                return Err("refusal is not allocation, start, finish, commit or ack".into());
            }
            if (s["kind"] == "range_violation") != s.get("state_range_violation").is_some() {
                return Err("range stage requires exactly its attempt payload".into());
            }
            if let Some(w) = s.get("state_range_violation") {
                validate_state_range_violation(w)?;
                if w["time"] != s["time"]
                    || s["events"] != serde_json::json!([])
                    || !s["result"].is_null()
                    || c["id"] != "state_in_range"
                    || i + 1 != steps.len()
                    || !c["property_location"].is_null()
                {
                    return Err("range attempt is not a successful business event".into());
                }
            }
        }
        let last = steps.last().unwrap();
        if last["violation"]["id"] != c["id"]
            || last["violation"]["at_step"] != c["transition_count"]
            || last["violation"]["property_location"] != c["property_location"]
        {
            return Err("target not at last stage".into());
        }
    }
    if failed != (ex["status"] == "unsupported") {
        return Err("explanation failure fold".into());
    }
    if failed && checked.contains(&serde_json::json!("reference_replay")) {
        return Err("failed replay marked checked".into());
    }
    // Every nested location is checked, including events, values and relations.
    fn locations(v: &Value) -> Result<(), String> {
        match v {
            Value::Object(o) => {
                if o.contains_key("end_line")
                    && (v["line"].as_u64() == Some(0)
                        || v["column"].as_u64() == Some(0)
                        || v["end_column"].as_u64() == Some(0)
                        || v["span"]["start"].as_u64() > v["span"]["end"].as_u64()
                        || v["line"].as_u64() > v["end_line"].as_u64()
                        || (v["line"] == v["end_line"]
                            && v["column"].as_u64() > v["end_column"].as_u64()))
                {
                    return Err("invalid location".into());
                }
                for x in o.values() {
                    locations(x)?;
                }
            }
            Value::Array(a) => {
                for x in a {
                    locations(x)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    locations(ex)
}

fn mutation_schema() -> Ty {
    use Ty::*;
    let strings = || Arr(Box::new(Str));
    let nullable = || Null(Box::new(Str));
    let payload = || Null(Box::new(OneOf(vec![Str, Bool, Int, strings()])));
    let change = || {
        Obj(vec![
            ("check", Str),
            ("key", Str),
            ("field", Str),
            ("before", payload()),
            ("after", payload()),
            (
                "direction",
                Lit(&["worsened", "improved", "changed", "unavailable", "held"]),
            ),
        ])
    };
    let site = || {
        Obj(vec![
            ("cohort", OptionalField(Box::new(Str))),
            ("removed_end", OptionalField(Box::new(Str))),
            ("process", nullable()),
            ("actor", nullable()),
            ("capability", nullable()),
            ("source", nullable()),
            ("affected_bindings", strings()),
            ("statement_path", nullable()),
            ("retained_branch", Null(Box::new(Lit(&["then", "else"])))),
            ("span", Obj(vec![("start", Int), ("end", Int)])),
            ("line", Int),
            ("column", Int),
            ("authority", OptionalField(Box::new(Str))),
            ("duplicate_binding", OptionalField(Box::new(Str))),
        ])
    };
    let mutant = || {
        Obj(vec![
            ("id", Str),
            ("site", site()),
            (
                "operator",
                Lit(&[
                    "remove_read",
                    "bypass_guard",
                    "expire_authority",
                    "duplicate_summary",
                    "remove_denominator_end",
                ]),
            ),
            (
                "result",
                Lit(&[
                    "detected",
                    "undetected",
                    "inconclusive",
                    "invalid_mutant",
                    "not_run",
                ]),
            ),
            (
                "reason",
                Null(Box::new(Lit(&[
                    "no_assigned_check",
                    "no_worsening",
                    "truncated",
                    "runtime_unsupported",
                    "original_truncated",
                    "assigned_check_unavailable",
                    "static_invalid",
                    "static_unsupported",
                    "mutant_limit",
                    "independence_credited",
                ]))),
            ),
            ("report", Null(Box::new(report_schema()))),
            ("assigned_changes", Arr(Box::new(change()))),
            ("other_changes", Arr(Box::new(change()))),
        ])
    };
    Obj(vec![
        ("verdict", Verdict),
        ("mode", Lit(&["mutation"])),
        ("ledger", Ledger),
        ("uncertainty", Uncertainty),
        ("analysis", Lit(&["exhaustive"])),
        ("set", Lit(&["bpr-default"])),
        ("original", Null(Box::new(report_schema()))),
        ("original_verdict", Null(Box::new(Verdict))),
        (
            "run",
            Obj(vec![
                ("completion", Lit(&["complete", "inconclusive", "blocked"])),
                (
                    "reason",
                    Null(Box::new(Lit(&[
                        "mutant_limit",
                        "original_not_decidable",
                        "invalid_arguments",
                        "input_failure",
                        "mutant_incomplete",
                    ]))),
                ),
                ("mutant_limit", Int),
                ("limit_reached", Bool),
            ]),
        ),
        (
            "mutations",
            Arr(Box::new(Obj(vec![
                ("kind", Lit(&MUTATION_KINDS)),
                ("name", Str),
                (
                    "status",
                    Lit(&["generated", "not_applicable", "unsupported", "blocked"]),
                ),
                (
                    "reason",
                    Null(Box::new(Lit(&[
                        "command_resend_unavailable",
                        "rule_version_unavailable",
                        "receipt_unknown_semantics_unavailable",
                        "quality_denominator_unavailable",
                        "state_condition",
                        "no_matching_site",
                        "no_supported_site",
                        "original_not_decidable",
                        "invalid_arguments",
                        "input_failure",
                    ]))),
                ),
                ("assigned_checks", strings()),
                ("mutants", Arr(Box::new(mutant()))),
                (
                    "skipped_sites",
                    OptionalField(Box::new(Arr(Box::new(OneOf(vec![
                        Obj(vec![
                            ("cohort", Str),
                            ("status", Lit(&["not_applicable"])),
                            ("reason", Lit(&["state_condition", "no_matching_site"])),
                        ]),
                        Obj(vec![
                            ("site", site()),
                            ("status", Lit(&["not_applicable"])),
                            (
                                "reason",
                                Lit(&[
                                    "undeclared_site",
                                    "existing_retry",
                                    "existing_response_loss",
                                    "fork_branch",
                                    "unsupported_input",
                                ]),
                            ),
                        ]),
                    ]))))),
                ),
                (
                    "skipped_counts",
                    OptionalField(Box::new(Obj(vec![
                        ("undeclared_site", OptionalField(Box::new(Int))),
                        ("existing_retry", OptionalField(Box::new(Int))),
                        ("existing_response_loss", OptionalField(Box::new(Int))),
                        ("fork_branch", OptionalField(Box::new(Int))),
                        ("unsupported_input", OptionalField(Box::new(Int))),
                    ]))),
                ),
            ]))),
        ),
        (
            "assumptions",
            Arr(Box::new(Obj(vec![
                ("field", Str),
                ("value", Str),
                ("affects", Str),
            ]))),
        ),
        ("diagnostics", Arr(Box::new(Diagnostic))),
        ("checked", strings()),
        ("unchecked", strings()),
    ])
}

/// Closed recursively, including the ordinary nested Report. This deliberately
/// does not deserialize into the proposed production DTO (no shared bug).
pub fn validate_mutation_wire(v: &Value) -> Result<(), String> {
    if v["mutations"][7]["status"] == "generated" || v["mutations"][7]["status"] == "not_applicable"
    {
        return validate_mutation_wire(&evaluation_wire::mutation_inverse(v)?);
    }
    for row in v["mutations"].as_array().into_iter().flatten() {
        let response_kind =
            ["command_resent", "receipt_unknown"].contains(&row["kind"].as_str().unwrap_or(""));
        if row.get("skipped_sites").is_some() && !response_kind {
            return Err("skipped_sites is only valid for enabled kind7".into());
        }
        if row.get("skipped_counts").is_some() && !response_kind {
            return Err("skipped_counts is only valid for response operators".into());
        }
        for m in row["mutants"].as_array().into_iter().flatten() {
            if m["operator"] == "remove_denominator_end"
                || m["site"].get("cohort").is_some()
                || m["site"].get("removed_end").is_some()
            {
                return Err("denominator fields are only valid for enabled kind7".into());
            }
        }
    }
    check(v, &mutation_schema(), "mutation wire")?;
    complete(v, &mutation_schema(), "mutation wire")?;
    // ADR 0034: viewpoints may be absent only from a legacy wire as a whole;
    // the original and every mutant report of one run agree on its presence.
    let has_viewpoints =
        |r: &Value| r["agency"].is_object() && r["agency"].get("viewpoints").is_some();
    let original = has_viewpoints(&v["original"]);
    for m in v["mutations"].as_array().into_iter().flatten() {
        for x in m["mutants"].as_array().into_iter().flatten() {
            if x["report"]["agency"].is_object() && has_viewpoints(&x["report"]) != original {
                return Err(
                    "agency viewpoints presence differs between original and mutant".into(),
                );
            }
        }
    }
    let assumptions = v["assumptions"].as_array().unwrap();
    if assumptions.len() != MUTATION_ASSUMPTIONS.len()
        || assumptions
            .iter()
            .zip(MUTATION_ASSUMPTIONS)
            .any(|(a, (field, value))| a["field"] != field || a["value"] != value)
        || assumptions[7]["affects"] != MUTATION_SCOPE_AFFECTS[0]
        || assumptions[8]["affects"] != MUTATION_SCOPE_AFFECTS[1]
    {
        return Err("mutation assumption order/content".into());
    }
    for (field, value) in MUTATION_ASSUMPTIONS {
        let rows: Vec<_> = assumptions.iter().filter(|a| a["field"] == field).collect();
        if rows.len() != 1 || rows[0]["value"] != value {
            return Err(format!("missing/duplicate/incorrect assumption {field}"));
        }
    }
    for word in [
        "coverage_quality",
        "real_world_model_correctness",
        "simulation_detection",
        "via_model",
        "command_resend",
        "rule_version",
        "receipt_unknown",
        "quality_denominator",
        "W-LIN-2",
        "requires",
        "guard_operator_scope",
        "contest_route_handoff",
        "contest_occurrence",
        "contest_response_deadline",
        "responsibility_time_frame",
        "authority_dynamic_expiry",
        "authority_expiry_unnamed",
        "summary_reuse_scope",
        "quality_changes",
    ] {
        if ["command_resend", "receipt_unknown"].contains(&word)
            && v["checked"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(word))
        {
            continue;
        }
        if !v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(word))
        {
            return Err(format!("missing unchecked boundary {word}"));
        }
    }
    if v["run"]["mutant_limit"] != 128 {
        return Err("wrong computation limit".into());
    }
    let boundary = [
        "authority_dynamic_expiry",
        "authority_expiry_unnamed",
        "summary_reuse_scope",
        "quality_changes",
    ];
    let unchecked = v["unchecked"].as_array().unwrap();
    if unchecked.len() < 4
        || unchecked[unchecked.len() - 4..] != boundary.map(|word| serde_json::json!(word))
        || boundary
            .iter()
            .any(|word| unchecked.iter().filter(|x| **x == *word).count() != 1)
    {
        return Err("new mutation unchecked boundary order/duplicates".into());
    }
    if !v["original"].is_null() {
        validate_wire(&v["original"])?;
        if v["original_verdict"] != v["original"]["verdict"] {
            return Err("original verdict mismatch".into());
        }
    } else if !v["original_verdict"].is_null() {
        return Err("verdict without original".into());
    }
    let rows = v["mutations"].as_array().unwrap();
    if rows.len() != 10 {
        return Err("exactly ten mutation kinds required".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut trials = 0;
    let mut not_run = 0;
    let mut incomplete = false;
    for (i, row) in rows.iter().enumerate() {
        if row["kind"] != MUTATION_KINDS[i] || row["name"] != MUTATION_NAMES[i] {
            return Err("mutation kind order/identity".into());
        }
        if row["assigned_checks"].as_array().unwrap().is_empty() {
            return Err("missing assigned-check declaration".into());
        }
        let ms = row["mutants"].as_array().unwrap();
        let response_kind = matches!(i, 3 | 6)
            && (row.get("skipped_counts").is_some()
                || row.get("skipped_sites").is_some()
                || row["status"] == "blocked");
        if response_kind
            && row["status"] == "blocked"
            && (row.get("skipped_counts").is_some() || row.get("skipped_sites").is_some())
        {
            return Err("blocked response operator cannot claim enumeration".into());
        }
        if response_kind && row["status"] != "blocked" {
            let counts = row
                .get("skipped_counts")
                .and_then(Value::as_object)
                .ok_or("response skip counts missing")?;
            let skipped = row
                .get("skipped_sites")
                .and_then(Value::as_array)
                .ok_or("response skip sites missing")?;
            let mut counted = std::collections::BTreeMap::<&str, u64>::new();
            let mut sites = std::collections::BTreeSet::new();
            for s in skipped {
                if s.get("cohort").is_some() || s.get("site").is_none() {
                    return Err("wrong response skipped-site shape".into());
                }
                let site = &s["site"];
                if site["line"].as_u64() == Some(0)
                    || site["column"].as_u64() == Some(0)
                    || site["span"]["start"].as_u64() >= site["span"]["end"].as_u64()
                    || site["process"].as_str().is_none()
                    || site["actor"].as_str().is_none()
                    || site["capability"].as_str().is_none()
                    || site["affected_bindings"]
                        .as_array()
                        .is_none_or(|bs| bs.len() != 1)
                    || !sites.insert((
                        site["process"].as_str(),
                        site["span"]["start"].as_u64(),
                        site["span"]["end"].as_u64(),
                    ))
                {
                    return Err("invalid or duplicate response skipped site".into());
                }
                *counted.entry(s["reason"].as_str().unwrap()).or_default() += 1;
            }
            if counts.len() != counted.len()
                || counts.iter().any(|(k, n)| {
                    n.as_u64() != counted.get(k.as_str()).copied() || n.as_u64() == Some(0)
                })
            {
                return Err("response skipped sites/counts mismatch".into());
            }
            if row["status"] == "not_applicable"
                && (row["reason"] == "no_matching_site") != skipped.is_empty()
            {
                return Err("response absence/rejected-site reasons conflated".into());
            }
            if !ms.is_empty() {
                return Err("dynamic response mutation wire is outside this Phase 1 fixture validator; use the ADR0056 typed transport and hand oracle".into());
            }
        }
        if let Some((_, reason)) = MUTATION_UNSUPPORTED
            .iter()
            .find(|(n, _)| *n == i)
            .filter(|_| !response_kind)
        {
            if row["status"] != "unsupported" || row["reason"] != *reason || !ms.is_empty() {
                return Err("unsupported kind lost/replaced".into());
            }
        } else {
            match row["status"].as_str().unwrap() {
                "generated" if !ms.is_empty() && row["reason"].is_null() => {}
                "not_applicable"
                    if ms.is_empty()
                        && (row["reason"] == "no_matching_site"
                            || response_kind && row["reason"] == "no_supported_site") => {}
                "blocked"
                    if ms.is_empty()
                        && [
                            "original_not_decidable",
                            "invalid_arguments",
                            "input_failure",
                        ]
                        .contains(&row["reason"].as_str().unwrap_or("")) => {}
                _ => return Err("inconsistent generated/not_applicable/blocked row".into()),
            }
        }
        for m in ms {
            if m["result"] == "not_run" {
                not_run += 1;
            } else {
                trials += 1;
            }
            incomplete |= ["not_run", "inconclusive", "invalid_mutant"]
                .contains(&m["result"].as_str().unwrap());
            let id = m["id"].as_str().unwrap();
            if !ids.insert(id) || !id.starts_with(&format!("{}:", MUTATION_KINDS[i])) {
                return Err("duplicate/wrong mutant id".into());
            }
            let site = &m["site"];
            if site["line"].as_u64().unwrap() == 0
                || site["column"].as_u64().unwrap() == 0
                || site["span"]["start"].as_u64() >= site["span"]["end"].as_u64()
            {
                return Err("invalid source position".into());
            }
            let expected_operator = match i {
                0 | 9 => "remove_read",
                2 | 8 => "bypass_guard",
                1 => "expire_authority",
                4 => "duplicate_summary",
                _ => return Err("mutant in unsupported kind".into()),
            };
            if m["operator"] != expected_operator {
                return Err("kind/operator mismatch".into());
            }
            if i != 1 && site.get("authority").is_some()
                || i != 4 && site.get("duplicate_binding").is_some()
            {
                return Err("new site field on wrong operator".into());
            }
            if i == 1 {
                let a = site["authority"]
                    .as_str()
                    .ok_or("authority site lacks authority")?;
                if id != format!("authority_expired:{a}:expires")
                    || [
                        "process",
                        "actor",
                        "capability",
                        "source",
                        "statement_path",
                        "retained_branch",
                    ]
                    .iter()
                    .any(|k| !site[k].is_null())
                {
                    return Err("authority identity/site mismatch".into());
                }
            } else if i == 4 {
                let p = site["process"].as_str().ok_or("summary process")?;
                let path = site["statement_path"].as_str().ok_or("summary path")?;
                let bs = site["affected_bindings"].as_array().unwrap();
                if bs.len() != 2
                    || site["actor"].as_str().is_none()
                    || site["capability"].as_str().is_none()
                    || site["duplicate_binding"].as_str().is_none()
                    || !site["source"].is_null()
                    || !site["retained_branch"].is_null()
                    || id
                        != format!(
                            "summary_reused:{p}:{path}:{}->{}",
                            bs[0].as_str().unwrap(),
                            bs[1].as_str().unwrap()
                        )
                {
                    return Err("summary identity/site mismatch".into());
                }
            } else if m["operator"] == "remove_read" {
                let (a, c, s) = (
                    site["actor"].as_str(),
                    site["capability"].as_str(),
                    site["source"].as_str(),
                );
                if let (Some(a), Some(c), Some(s)) = (a, c, s) {
                    if id != format!("{}:{a}.{c}:reads:{s}", MUTATION_KINDS[i])
                        || !site["process"].is_null()
                        || !site["statement_path"].is_null()
                        || !site["retained_branch"].is_null()
                    {
                        return Err("read identity/site mismatch".into());
                    }
                } else {
                    return Err("read site lacks names".into());
                }
            } else if let (Some(p), Some(path), Some(branch)) = (
                site["process"].as_str(),
                site["statement_path"].as_str(),
                site["retained_branch"].as_str(),
            ) {
                if id != format!("{}:{p}:{path}:keep_{branch}", MUTATION_KINDS[i])
                    || ["actor", "capability", "source"]
                        .iter()
                        .any(|k| !site[k].is_null())
                {
                    return Err("guard identity/site mismatch".into());
                }
            } else {
                return Err("guard site lacks path".into());
            }
            let changes = m["assigned_changes"].as_array().unwrap();
            validate_completion_changes(&m["assigned_changes"], true)?;
            validate_completion_changes(&m["other_changes"], false)?;
            let worsened = changes.iter().any(|c| c["direction"] == "worsened");
            if i == 4 && m["reason"] == "independence_credited" {
                let consumer = site["affected_bindings"][1].as_str().unwrap();
                let line = |report: &Value| {
                    report["agency"]["lineage"]
                        .as_array()
                        .and_then(|a| a.iter().find(|l| l["call"] == consumer))
                        .cloned()
                };
                let before = line(&v["original"]).ok_or("credited without original lineage")?;
                let after = line(&m["report"]).ok_or("credited without mutant lineage")?;
                let set = |x: &Value| {
                    x.as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect::<std::collections::BTreeSet<_>>()
                };
                let old_independent = set(&before["independent_sources"]);
                let increased = set(&after["independent_sources"])
                    .iter()
                    .any(|s| !old_independent.contains(s));
                let subject = set(&before["subject_sources"]);
                let count = |report: &Value| {
                    report["agency"]["lineage_groups"]
                        .as_array()
                        .and_then(|a| a.iter().find(|g| set(&g["subject_sources"]) == subject))
                        .and_then(|g| g["judgements"].as_u64())
                        .unwrap_or(0)
                };
                if !increased && count(&m["report"]) >= count(&v["original"]) {
                    return Err(
                        "independence_credited without a new source or group reduction".into(),
                    );
                }
            }
            let held = changes.len() == 3
                && changes.iter().all(|c| c["direction"] == "held")
                && changes[0]["field"] == "subject_sources"
                && changes[1]["field"] == "independent_sources"
                && changes[2]["field"] == "judgements"
                && changes[..2].iter().all(|c| c["before"] == c["after"])
                && changes[2]["before"].as_u64().is_some()
                && changes[2]["before"].as_u64() <= changes[2]["after"].as_u64();
            if i == 4 && m["result"] == "detected" && held {
                let consumer = site["affected_bindings"][1].as_str().unwrap();
                let find_line = |report: &Value| {
                    report["agency"]["lineage"]
                        .as_array()
                        .and_then(|lines| lines.iter().find(|line| line["call"] == consumer))
                        .cloned()
                };
                let before = find_line(&v["original"]).ok_or("held without original consumer")?;
                let after = find_line(&m["report"]).ok_or("held without mutant consumer")?;
                let set = |x: &Value| -> Value {
                    serde_json::json!(
                        x.as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect::<std::collections::BTreeSet<_>>()
                    )
                };
                let key = format!(
                    "{consumer}:{}",
                    before["capability"].as_str().ok_or("lineage capability")?
                );
                for (n, field) in ["subject_sources", "independent_sources"]
                    .iter()
                    .enumerate()
                {
                    if changes[n]["check"] != "agency.lineage"
                        || changes[n]["key"] != key
                        || changes[n]["before"] != set(&before[field])
                        || changes[n]["after"] != set(&after[field])
                    {
                        return Err("held payload differs from lineage".into());
                    }
                }
                let subject = set(&before["subject_sources"]);
                if subject == serde_json::json!([]) {
                    return Err("held without nonempty subject".into());
                }
                let group_count = |report: &Value| {
                    report["agency"]["lineage_groups"]
                        .as_array()
                        .and_then(|groups| {
                            groups
                                .iter()
                                .find(|g| set(&g["subject_sources"]) == subject)
                        })
                        .map(|g| g["judgements"].clone())
                };
                if changes[2]["check"] != "agency.lineage_groups"
                    || changes[2]["key"] != serde_json::to_string(&subject).unwrap()
                    || Some(changes[2]["before"].clone()) != group_count(&v["original"])
                    || Some(changes[2]["after"].clone()) != group_count(&m["report"])
                {
                    return Err("held payload differs from group".into());
                }
            }
            if changes.iter().any(|c| c["direction"] == "held")
                && (i != 4 || m["result"] != "detected" || !held)
                || m["other_changes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["direction"] == "held")
            {
                return Err("held is only the three assigned summary fields".into());
            }
            for c in changes.iter().chain(m["other_changes"].as_array().unwrap()) {
                for key in ["before", "after"] {
                    if let Some(a) = c[key].as_array()
                        && a.windows(2)
                            .any(|pair| pair[0].as_str() >= pair[1].as_str())
                    {
                        return Err("source sets must be sorted unique".into());
                    }
                }
            }
            match m["result"].as_str().unwrap() {
                "not_run"
                    if m["reason"] == "mutant_limit"
                        && m["report"].is_null()
                        && changes.is_empty()
                        && m["other_changes"] == serde_json::json!([]) => {}
                "detected"
                    if (if i == 4 { held } else { worsened })
                        && m["reason"].is_null()
                        && !m["report"].is_null() => {}
                "undetected"
                    if !worsened
                        && (["no_worsening", "no_assigned_check"]
                            .contains(&m["reason"].as_str().unwrap_or(""))
                            || i == 4
                                && m["reason"] == "independence_credited"
                                && changes.iter().any(|c| c["direction"] == "improved"))
                        && !m["report"].is_null() => {}
                "invalid_mutant"
                    if ["static_invalid", "static_unsupported"]
                        .contains(&m["reason"].as_str().unwrap_or(""))
                        && !m["report"].is_null() => {}
                "inconclusive"
                    if [
                        "truncated",
                        "runtime_unsupported",
                        "original_truncated",
                        "assigned_check_unavailable",
                    ]
                    .contains(&m["reason"].as_str().unwrap_or(""))
                        && !m["report"].is_null() => {}
                _ => return Err("inconsistent mutant result/reason/report".into()),
            }
            if !m["report"].is_null() {
                validate_wire(&m["report"])?;
                if ["detected", "undetected"].contains(&m["result"].as_str().unwrap())
                    && (m["report"]["bounds"]["truncated"] != false
                        || !["satisfied", "violated", "vacuous"]
                            .contains(&m["report"]["verdict"].as_str().unwrap_or("")))
                {
                    return Err("incomplete/invalid report classified as comparison".into());
                }
                if m["result"] == "invalid_mutant"
                    && m["report"]["verdict"]
                        != if m["reason"] == "static_invalid" {
                            "invalid"
                        } else {
                            "unsupported"
                        }
                {
                    return Err("static reason/verdict mismatch".into());
                }
                if m["reason"] == "truncated" && m["report"]["bounds"]["truncated"] != true {
                    return Err("truncation reason without cut".into());
                }
            }
        }
    }
    let mut cut_seen = false;
    for i in [0, 2, 8, 9, 1, 4] {
        for m in rows[i]["mutants"].as_array().unwrap() {
            if m["result"] == "not_run" {
                cut_seen = true;
            } else if cut_seen {
                return Err("executed mutant after computation cut".into());
            }
        }
    }
    if trials > 128 || v["run"]["limit_reached"] != (not_run > 0) {
        return Err("computation budget inconsistent".into());
    }
    let blocked = v["original"].is_null()
        || !["satisfied", "violated"].contains(&v["original_verdict"].as_str().unwrap_or(""));
    for (index, word) in [(3, "command_resend"), (6, "receipt_unknown")] {
        if rows[index].get("skipped_counts").is_some() || rows[index]["status"] == "blocked" {
            let done = !blocked && not_run == 0;
            if v["checked"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|x| **x == word)
                .count()
                != usize::from(done)
                || unchecked.iter().filter(|x| **x == word).count() != usize::from(!done)
            {
                return Err("response operator stage mismatch".into());
            }
        }
    }
    let authority_done = !blocked
        && rows[1]["mutants"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["result"] != "not_run");
    if v["checked"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| **w == "authority_expiry")
        .count()
        != usize::from(authority_done)
        || v["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|w| **w == "authority_expiry")
            .count()
            != usize::from(!authority_done)
    {
        return Err("authority_expiry stage must match its own completion".into());
    }
    for stage in ["single_fault_generation", "assigned_check_comparison"] {
        let done = !blocked && not_run == 0;
        if v["checked"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|w| **w == stage)
            .count()
            != usize::from(done)
            || unchecked.iter().filter(|w| **w == stage).count() != usize::from(!done)
        {
            return Err("mutation whole-run stage mismatch".into());
        }
    }
    if blocked {
        if trials + not_run != 0
            || v["run"]["completion"] != "blocked"
            || [0, 1, 2, 4, 8, 9]
                .iter()
                .any(|&i| rows[i]["status"] != "blocked")
        {
            return Err("original gate must block generation".into());
        }
        if v["original"].is_null() {
            if v["verdict"] != "invalid"
                || !["invalid_arguments", "input_failure"]
                    .contains(&v["run"]["reason"].as_str().unwrap_or(""))
            {
                return Err("preflight verdict/reason".into());
            }
        } else if v["verdict"] != v["original_verdict"]
            || v["run"]["reason"] != "original_not_decidable"
        {
            return Err("original verdict gate overwritten".into());
        }
        if [0, 1, 2, 4, 8, 9]
            .iter()
            .any(|&i| rows[i]["reason"] != v["run"]["reason"])
        {
            return Err("blocked row reason differs from run".into());
        }
    } else {
        if v["verdict"] != "unsupported"
            || [0, 1, 2, 4, 8, 9]
                .iter()
                .any(|&i| rows[i]["status"] == "blocked")
        {
            return Err("completed mutation fold must preserve unsupported priority".into());
        }
        let completion = if incomplete {
            "inconclusive"
        } else {
            "complete"
        };
        let reason = if not_run > 0 {
            serde_json::json!("mutant_limit")
        } else if incomplete {
            serde_json::json!("mutant_incomplete")
        } else {
            Value::Null
        };
        if v["run"]["completion"] != completion || v["run"]["reason"] != reason {
            return Err("run completion/reason inconsistent".into());
        }
    }
    Ok(())
}

fn compare_mutation_values(e: &Value, a: &Value, path: &str) -> Result<(), String> {
    match (e, a) {
        (Value::Object(e), Value::Object(a)) => {
            for (k, v) in e {
                if k == "absent_diagnostics" {
                    for code in v.as_array().ok_or("absent_diagnostics array")? {
                        if a.get("diagnostics")
                            .and_then(Value::as_array)
                            .is_some_and(|ds| ds.iter().any(|d| d["code"] == *code))
                        {
                            return Err(format!(
                                "{path}.absent_diagnostics: forbidden diagnostic {code}"
                            ));
                        }
                    }
                    continue;
                }
                compare_mutation_values(
                    v,
                    a.get(k).ok_or_else(|| format!("{path}: missing {k}"))?,
                    &format!("{path}.{k}"),
                )?;
            }
            Ok(())
        }
        (Value::Array(e), Value::Array(a)) => {
            if e.len() != a.len() {
                return Err(format!("{path}: array length"));
            }
            for (i, (x, y)) in e.iter().zip(a).enumerate() {
                compare_mutation_values(x, y, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        _ if e == a => Ok(()),
        _ => Err(format!("{path}: {e} != {a}")),
    }
}

#[cfg(test)]
mod mutation_wire_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scoped_negative_diagnostics_are_metadata_not_an_expected_wire_key() {
        let e = json!({"original":{"absent_diagnostics":["W-DEADLOCK-0"]}});
        let a = json!({"original":{"diagnostics":[]},"other":{"diagnostics":[{"code":"W-DEADLOCK-0"}]}});
        assert!(compare_mutation_values(&e, &a, "mutation").is_ok());
        let mut bad = a.clone();
        bad["original"]["diagnostics"] = json!([{"code":"W-DEADLOCK-0"}]);
        assert_ne!(a, bad);
        assert!(compare_mutation_values(&e, &bad, "mutation").is_err());
    }

    fn full() -> Value {
        let mut original=serde_json::to_value(relca_core::report::check("model M version \"0.2\" { process Flow { end done } bounds { cases 1; live_tasks 1; clock 1min } verify safety no_resource_overcommit }")).unwrap();
        // Hand wire, not a conformance assertion on the producer. The separate
        // real-binary acceptances require the producer itself to drop this word.
        original["agency"]["unchecked"]
            .as_array_mut()
            .unwrap()
            .retain(|v| v != "W-LIN-2");
        let rows:Vec<_>=MUTATION_KINDS.iter().enumerate().map(|(i,k)|{
            let reason=MUTATION_UNSUPPORTED.iter().find(|(n,_)|*n==i).map(|(_,r)|*r);
                json!({"kind":k,"name":MUTATION_NAMES[i],"status":if reason.is_some(){"unsupported"}else{"not_applicable"},"reason":reason.unwrap_or("no_matching_site"),"assigned_checks":["fixture_assigned"],"mutants":[]})
        }).collect();
        let assumptions: Vec<_> = MUTATION_ASSUMPTIONS
            .iter()
            .enumerate()
            .map(|(i,(f, v))| json!({"field":f,"value":v,"affects":if i>=7 {MUTATION_SCOPE_AFFECTS[i-7]} else {"mutation"}}))
            .collect();
        json!({"ledger":ledger_shape::control(&json!({})),"uncertainty":ledger_shape::uncertainty_wire::project(&json!({})),"verdict":"unsupported","mode":"mutation","analysis":"exhaustive","set":"bpr-default",
            "original":original,"original_verdict":"satisfied","run":{"completion":"complete","reason":null,"mutant_limit":128,"limit_reached":false},
            "mutations":rows,"assumptions":assumptions,"diagnostics":[],"checked":["single_fault_generation","assigned_check_comparison","authority_expiry"],"unchecked":["coverage_quality","real_world_model_correctness","simulation_detection","via_model","command_resend","rule_version","receipt_unknown","quality_denominator","W-LIN-2","requires","guard_operator_scope","contest_route_handoff","contest_occurrence","contest_response_deadline","responsibility_time_frame","authority_dynamic_expiry","authority_expiry_unnamed","summary_reuse_scope","quality_changes"]})
    }
    fn generated() -> Value {
        let mut v = full();
        let report = v["original"].clone();
        v["mutations"][0]["status"] = json!("generated");
        v["mutations"][0]["reason"] = Value::Null;
        v["mutations"][0]["mutants"] = json!([{"id":"evidence_missing:A.read:reads:S","operator":"remove_read",
            "site":{"process":null,"actor":"A","capability":"read","source":"S","affected_bindings":["x"],"statement_path":null,"retained_branch":null,"span":{"start":1,"end":2},"line":1,"column":2},
            "result":"detected","reason":null,"report":report,
            "assigned_changes":[{"check":"agency.lineage","key":"x","field":"independent_sources","before":["S"],"after":[],"direction":"worsened"}],"other_changes":[]}]);
        v
    }
    #[test]
    fn response_skip_wire_is_closed_and_counts_each_source_site_once() {
        let mut v = full();
        for i in [3, 6] {
            v["mutations"][i]["status"] = json!("not_applicable");
            v["mutations"][i]["reason"] = json!("no_supported_site");
            v["mutations"][i]["skipped_counts"] = json!({"undeclared_site":1});
            v["mutations"][i]["skipped_sites"] = json!([{"status":"not_applicable","reason":"undeclared_site","site":{
                "process":"Flow","actor":"A","capability":"post","source":null,"affected_bindings":["x"],"statement_path":"body/1","retained_branch":null,
                "span":{"start":1,"end":2},"line":1,"column":2}}]);
        }
        for word in ["command_resend", "receipt_unknown"] {
            v["unchecked"]
                .as_array_mut()
                .unwrap()
                .retain(|v| *v != word);
            v["checked"].as_array_mut().unwrap().push(json!(word));
        }
        validate_mutation_wire(&v).unwrap();
        for fault in 0..9 {
            let mut bad = v.clone();
            match fault {
                0 => bad["mutations"][3]["skipped_counts"]["undeclared_site"] = json!(2),
                1 => bad["mutations"][3]["skipped_counts"]["unknown_reason"] = json!(1),
                2 => bad["mutations"][3]["skipped_sites"][0]["site"]["span"]["end"] = json!(0),
                3 => bad["mutations"][3]["skipped_sites"][0]["cohort"] = json!("not_a_call"),
                4 => bad["mutations"][0]["skipped_counts"] = json!({}),
                5 => {
                    bad["mutations"][3]
                        .as_object_mut()
                        .unwrap()
                        .remove("skipped_counts");
                }
                6 => bad["mutations"][3]["status"] = json!("unsupported"),
                7 => bad["mutations"][3]["status"] = json!("blocked"),
                _ => {
                    let site = bad["mutations"][3]["skipped_sites"][0].clone();
                    bad["mutations"][3]["skipped_sites"]
                        .as_array_mut()
                        .unwrap()
                        .push(site);
                    bad["mutations"][3]["skipped_counts"]["undeclared_site"] = json!(2);
                }
            }
            assert!(validate_mutation_wire(&bad).is_err(), "fault {fault}");
        }
    }
    #[test]
    fn mutation_invocation_is_closed_and_hyphenated_set_is_not_identifier() {
        let i = Invocation::parse(Some(&json!({"command":"mutate","set":"bpr-default"}))).unwrap();
        assert_eq!(
            i.args(std::path::Path::new("model.relca")),
            ["mutate", "model.relca", "--set", "bpr-default"]
        );
        for v in [
            json!({"command":"mutate","set":"other"}),
            json!({"command":"mutate","set":null}),
            json!({"command":"mutate","mode":"simulation"}),
        ] {
            assert!(Invocation::parse(Some(&v)).is_err())
        }
    }
    #[test]
    fn mutation_closed_wire_rejects_single_faults_at_every_object_depth() {
        let v = generated();
        validate_mutation_wire(&v).unwrap();
        fn paths(v: &Value, p: String, out: &mut Vec<String>) {
            match v {
                Value::Object(o) => {
                    out.push(p.clone());
                    for (k, x) in o {
                        paths(x, format!("{p}/{k}"), out)
                    }
                }
                Value::Array(a) => {
                    for (i, x) in a.iter().enumerate() {
                        paths(x, format!("{p}/{i}"), out)
                    }
                }
                _ => {}
            }
        }
        let mut ps = vec![];
        paths(&v, String::new(), &mut ps);
        for p in ps {
            let mut bad = v.clone();
            bad.pointer_mut(&p)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("coverage_score".into(), json!(1));
            assert!(validate_mutation_wire(&bad).is_err(), "unknown at {p}");
            let keys: Vec<_> = v
                .pointer(&p)
                .unwrap()
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            for key in keys {
                let mut bad = v.clone();
                bad.pointer_mut(&p)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(&key);
                assert!(validate_mutation_wire(&bad).is_err(), "missing {p}/{key}");
            }
        }
        for (pointer, replacement) in [
            ("/mutations/0/mutants", json!([])),
            ("/mutations/1/reason", json!("")),
            ("/original_verdict", json!("violated")),
            ("/run/completion", json!("inconclusive")),
            ("/verdict", json!("violated")),
            ("/mutations/0/assigned_checks", json!([])),
            (
                "/mutations/0/mutants/0/report/bounds/truncated",
                json!(true),
            ),
            ("/mutations/0/mutants/0/id", json!("other")),
            ("/mutations/0/mutants/0/site/line", json!(0)),
            ("/mutations/0/mutants/0/site/span/end", json!(1)),
            ("/mutations/0/mutants/0/assigned_changes", json!([])),
            ("/mutations/0/mutants/0/result", json!("undetected")),
            ("/mutations/0/mutants/0/result", json!("not_run")),
            (
                "/mutations/0/mutants/0/assigned_changes/0/before",
                json!({"sources":["S"]}),
            ),
            (
                "/mutations/0/mutants/0/assigned_changes/0/before",
                json!([1]),
            ),
            (
                "/mutations/0/mutants/0/assigned_changes/0/before",
                json!(["Z", "A"]),
            ),
        ] {
            let mut bad = v.clone();
            *bad.pointer_mut(pointer).unwrap() = replacement;
            assert!(validate_mutation_wire(&bad).is_err(), "{pointer}");
        }
        let mut bad = v.clone();
        bad["mutations"].as_array_mut().unwrap().pop();
        assert!(validate_mutation_wire(&bad).is_err());
        let mut bad = v.clone();
        bad["mutations"][1] = bad["mutations"][0].clone();
        assert!(validate_mutation_wire(&bad).is_err());
        let mut bad = v.clone();
        let m = bad["mutations"][0]["mutants"][0].clone();
        bad["mutations"][0]["mutants"]
            .as_array_mut()
            .unwrap()
            .push(m);
        assert!(validate_mutation_wire(&bad).is_err());
        let mut bad = full();
        bad["mutations"][0]["mutants"] = v["mutations"][0]["mutants"].clone();
        assert!(validate_mutation_wire(&bad).is_err());
    }
    #[test]
    fn mutation_partial_arrays_still_match_every_site_and_order() {
        let v = generated();
        let inv = Invocation::Mutate { set: None };
        inv.compare(&v, &v).unwrap();
        let mut bad = v.clone();
        bad["mutations"][0]["mutants"][0]["site"]["affected_bindings"] = json!([]);
        assert!(inv.compare(&v, &bad).is_err());
        for name in MUTATION_FIXTURES {
            let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures")
                .join(name)
                .join("expected.json");
            inv.validate_expected(
                &serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    fn mutation_summary_held_independence_and_optional_site_fields_are_closed() {
        let mut v = full();
        let source = include_str!("../../../fixtures/mut2-summary/model.relca");
        let report = serde_json::to_value(relca_core::report::check(source)).unwrap();
        v["original"] = report.clone();
        let oracle: Value = serde_json::from_str(include_str!(
            "../../relca-core/tests/support/mutation_default_oracle.json"
        ))
        .unwrap();
        let mut m = oracle["mut2-summary"]["rows"][1]["mutants"][0].clone();
        m["site"]["span"] = json!({"start":1,"end":2});
        m["site"]["line"] = json!(1);
        m["site"]["column"] = json!(2);
        m["report"] = report;
        m["other_changes"] = json!([]);
        v["mutations"][4]["status"] = json!("generated");
        v["mutations"][4]["reason"] = Value::Null;
        v["mutations"][4]["mutants"] = json!([m]);
        validate_mutation_wire(&v).unwrap();
        let mut increase = v.clone();
        increase["mutations"][4]["mutants"][0]["assigned_changes"][2]["after"] = json!(3);
        increase["mutations"][4]["mutants"][0]["report"]["agency"]["lineage_groups"][0]["judgements"] =
            json!(3);
        validate_mutation_wire(&increase).unwrap();
        let mut credited = v.clone();
        let m = &mut credited["mutations"][4]["mutants"][0];
        m["result"] = json!("undetected");
        m["reason"] = json!("independence_credited");
        m["assigned_changes"] = json!([{"check":"agency.lineage","key":"a:Approver.approve","field":"independent_sources","before":[],"after":["PurchaseOrder"],"direction":"improved"}]);
        m["report"]["agency"]["lineage"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|l| l["call"] == "a")
            .unwrap()["independent_sources"] = json!(["PurchaseOrder"]);
        validate_mutation_wire(&credited).unwrap();
        for (pointer, value) in [
            ("/mutations/4/mutants/0/site/authority", json!("G")),
            ("/mutations/4/mutants/0/site/duplicate_binding", Value::Null),
            ("/mutations/4/mutants/0/operator", json!("remove_read")),
            (
                "/mutations/4/mutants/0/assigned_changes/0/after",
                json!(["PurchaseOrder"]),
            ),
            ("/mutations/4/mutants/0/assigned_changes/2/after", json!(1)),
            (
                "/mutations/4/mutants/0/reason",
                json!("independence_credited"),
            ),
        ] {
            let mut bad = v.clone();
            if pointer.ends_with("/authority") {
                bad["mutations"][4]["mutants"][0]["site"]["authority"] = value;
            } else {
                *bad.pointer_mut(pointer).unwrap() = value;
            }
            assert!(validate_mutation_wire(&bad).is_err(), "{pointer}");
        }
        let mut old = generated();
        old["mutations"][0]["mutants"][0]["site"]["duplicate_binding"] = Value::Null;
        assert!(validate_mutation_wire(&old).is_err());
        let mut old = generated();
        old["mutations"][0]["mutants"][0]["assigned_changes"][0]["direction"] = json!("held");
        assert!(validate_mutation_wire(&old).is_err());
    }
}

/// Full site oracle separate from partial expected.json. Names and counts are
/// fixed by calc.md; no generated wire is used to construct expectations.
#[allow(dead_code)] // Other command integration targets include this same module.
pub fn compare_mutation_fixture_sites(name: &str, v: &Value) -> Result<(), String> {
    if mutation_default_fixture(name) {
        let oracle: Value = serde_json::from_str(include_str!(
            "../../relca-core/tests/support/mutation_default_oracle.json"
        ))
        .unwrap();
        for (n, i) in [1, 4].iter().enumerate() {
            compare(
                &oracle[name]["rows"][n],
                &v["mutations"][i],
                &format!("{name}/mutations/{i}"),
            )?;
        }
        return Ok(());
    }
    if !MUTATION_FIXTURES.contains(&name) {
        return Err(format!("unknown mutation fixture {name}"));
    }
    let blocked = name == "mut-empty.broken-invalid" || name.starts_with("mut-gate");
    let reads: Vec<(&str, &str, &str)> =
        if blocked || name == "mut-empty" || name.starts_with("mut-limit") {
            vec![]
        } else if name.starts_with("mut-evidence") {
            vec![
                ("Reader", "enter", "Invoice"),
                (
                    "Clerk",
                    "match_po",
                    if name.contains("broken") {
                        "Invoice"
                    } else {
                        "PurchaseOrder"
                    },
                ),
            ]
        } else if name.starts_with("mut-guard") {
            vec![("Reader", "enter", "Invoice")]
        } else if name == "mut-origin" {
            vec![
                ("Reader", "enter", "Invoice"),
                ("Approver", "approve", "Invoice"),
            ]
        } else {
            vec![
                ("Reader", "enter", "Invoice"),
                ("Clerk", "recheck", "Invoice"),
            ]
        };
    for (i, kind) in MUTATION_KINDS.iter().enumerate() {
        let row = &v["mutations"][i];
        let mut expected = vec![];
        if [0, 9].contains(&i) && !blocked {
            if name.starts_with("mut-limit") {
                let n = if name.contains("broken") { 65 } else { 64 };
                for j in 0..n {
                    expected.push((
                        format!("{kind}:Probe.read:reads:S{j:02}"),
                        if i == 9 && n == 65 && j >= 63 {
                            "not_run"
                        } else {
                            "undetected"
                        },
                    ));
                }
            } else {
                for (a, c, s) in &reads {
                    let detected = (i == 0 && name == "mut-evidence" && *a == "Clerk")
                        || (i == 9 && name == "mut-origin" && *a == "Approver");
                    expected.push((
                        format!("{kind}:{a}.{c}:reads:{s}"),
                        if detected { "detected" } else { "undetected" },
                    ));
                }
            }
        }
        if [2, 8].contains(&i) {
            let paths: &[&str] = if name.starts_with("mut-guard") {
                &["body/2", "body/4"]
            } else if name == "mut-origin" {
                &["body/2"]
            } else {
                &[]
            };
            for path in paths {
                expected.push((
                    format!("{kind}:Flow:{path}:keep_then"),
                    if i == 8 || name == "mut-guard" {
                        "detected"
                    } else {
                        "undetected"
                    },
                ));
            }
        }
        if i == 4 && name.starts_with("mut-guard") {
            expected.push((
                "summary_reused:Flow:body/0:entry->approval".into(),
                "detected",
            ));
        }
        let ms = row["mutants"].as_array().ok_or("missing mutants")?;
        if ms.len() != expected.len() {
            return Err(format!("{name}/{kind}: full candidate count"));
        }
        for (m, (id, result)) in ms.iter().zip(&expected) {
            if m["id"] != *id || m["result"] != *result {
                return Err(format!("{name}: wrong site/result {id}/{result}"));
            }
        }
    }
    Ok(())
}

/// ADR0046 is appended after the existing groups. Membership is closed by
/// an independently authored exact table, never by a mut2- prefix exemption.
#[allow(dead_code)]
pub fn mutation_default_fixture(name: &str) -> bool {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../../relca-core/tests/support/mutation_default_names.json"
            ))
            .unwrap()
        })
        .iter()
        .any(|n| n == name)
}

pub fn compare_report(expected: &Value, actual: &Value) -> Result<(), String> {
    validate_expected(expected)?;
    validate_wire(actual)?;
    // The schema must admit every real report, or it would reject true
    // expectations: check it against the actual output too.
    validate_expected(actual).map_err(|error| format!("schema: {error}"))?;
    let mut expected = expected.clone();
    let absent = expected
        .as_object_mut()
        .ok_or("expected must be an object")?
        .remove("absent_diagnostics");
    for code in absent.iter().flat_map(|codes| codes.as_array().unwrap()) {
        let code = code.as_str().unwrap();
        if actual["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"].as_str() == Some(code))
        {
            return Err(format!("absent_diagnostics: forbidden diagnostic {code}"));
        }
    }
    compare(&expected, actual, "$")
}

/// The expectation's own form, independent of any report. The runner checks it
/// even for undetected fixtures, whose comparison failures are only listed.
/// Every key must be an ADR 0002 / 0007 report key (or absent_diagnostics) and
/// every present value must have that key's type; partial objects are allowed.
pub fn validate_expected(expected: &Value) -> Result<(), String> {
    check(expected, &report_schema(), "$")
}

pub fn validate_diff_expected(expected: &Value) -> Result<(), String> {
    check(expected, &diff_schema(), "$")
}

pub fn compare_diff_report(expected: &Value, actual: &Value) -> Result<(), String> {
    for side in ["from", "to"] {
        validate_actor_capacity_reason(&actual["agency"][side], &actual[side]["report"]["actors"])?;
        validate_read_write_pin(
            &actual[side]["report"],
            actual[side]["configuration"].as_str(),
        )?;
    }
    validate_diff_expected(expected)?;
    for side in ["from", "to"] {
        validate_common_cause_agency(&actual["agency"][side]).map_err(|e| format!("wire: {e}"))?;
    }
    if actual.get("metrics").is_some() {
        validate_metric_tables(actual, false)?;
        let old = without_metric_comparison(actual, false)?;
        compare_diff_report(&serde_json::json!({}), &old)?;
        check(actual, &diff_schema(), "diff metrics wire")?;
        complete(actual, &diff_schema(), "diff metrics wire")?;
        for side in ["from", "to"] {
            validate_wire(&actual[side]["report"])?;
        }
        return compare(expected, actual, "$");
    }
    if actual["verdict"] != "invalid"
        && ["from", "to"]
            .iter()
            .any(|s| actual[s]["report"].get("metrics").is_some())
    {
        return Err("metrics declaration lost diff metric table".into());
    }
    for field in ["checked", "unchecked"] {
        if actual[field].as_array().is_some_and(|a| {
            a.iter().any(|x| {
                x.as_str()
                    .is_some_and(|s| s == "metrics" || s.starts_with("metric_"))
            })
        }) {
            return Err("metric diff words without metric table".into());
        }
    }
    // Validate the entire wire independently of the partial expectation. This
    // explicit schema also works before the production DiffReport type exists.
    check(actual, &diff_schema(), "$")
        .and_then(|()| complete(actual, &diff_schema(), "$"))
        .map_err(|error| format!("wire: {error}"))?;
    if let Some(table) = actual.get("quality") {
        validate_quality_table(table, false, false, None)?;
    }
    for side in ["from", "to"] {
        validate_wire(&actual[side]["report"])?;
    }
    validate_diff_cases(actual)?;
    validate_diff_extensions(actual)?;
    compare(expected, actual, "$")
}

fn valid_plan_name(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

const METRIC_REASONS: &[&str] = &[
    "not_configured",
    "truncated",
    "aborted",
    "range_stopped",
    "no_samples",
    "no_terminal",
    "rejected",
    "no_task_started",
    "never_created",
    "cancelled_before_start",
    "refused",
    "not_refused",
    "unfinished",
    "deadlock",
    "zero_capacity",
    "no_capacity",
];
const METRIC_CHECKED: &[&str] = &[
    "lead_time_terminal_only",
    "first_start_wait",
    "task_ready_wait",
    "actor_work",
    "named_people_work",
    "peak_tasks",
    "capacity_window_utilization",
    "declared_rates",
    "observed_processing_cost",
    "outcome_counts",
];
const METRIC_UNCHECKED: &[&str] = &[
    "quality",
    "business_value",
    "waiting_decomposition",
    "queue_length_statistics",
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
const METRIC_COMPARISON_WORDS: &[&str] = &[
    "metric_differences",
    "metric_ranking",
    "metric_semantic_alignment",
];
const METRIC_FIELDS: &[&str] = &[
    "metrics.window",
    "metrics.currency",
    "metrics.rates",
    "metrics.lead_time",
    "metrics.observation",
    "metrics.estimation",
];
const METRIC_AFFECTS: &[&str] = &[
    "全資源は共通の分析窓で利用可能と仮定する。windowは容量時間の分母であり、clockや到着を変えず、終了後のdrainを追加しない。",
    "費用はこの通貨の最小単位であり、他通貨へ換算しない。",
    "単価の出所はモデルの宣言であり、実際の賃金・価格ではない。未設定の費目を0円にせず、必要費目が一つでも欠ければ合計もnot_configuredとする。",
    "リードタイムは終端に達した案件だけの値である。未完了・受付拒否・deadlockの件数を並べ、業務上の成功や解決を終端名だけから認定しない。",
    "作業時間と費用は観測停止までに発生した占有と開始の値である。未完了案件の費用も含み、未観測の全期間の費用とは呼ばない。網羅で切断が一つでもあれば新指標の極値は全て未確定とする。",
    "点推定であり、全スケジュールの検証でも区間推定でもない。案件を独立な標本と数えず、反復ごとに観測して標本平均・最小・最大・標本数を出す。",
];

fn metric_scalar_schema() -> Ty {
    use Ty::*;
    let base = || {
        vec![
            ("min", Null(Box::new(MetricNumber))),
            ("max", Null(Box::new(MetricNumber))),
            ("reason", Null(Box::new(Lit(METRIC_REASONS)))),
        ]
    };
    let exhaustive = Obj(base());
    let mut fields = base();
    fields.extend([("mean", Null(Box::new(MetricNumber))), ("samples", Int)]);
    OneOf(vec![exhaustive, Obj(fields)])
}
fn shared_resources_schema() -> Ty {
    use Ty::*;
    Arr(Box::new(Obj(vec![
        ("resource", Str),
        ("capacity", Int),
        ("occupied", Int),
        (
            "holders",
            Arr(Box::new(Obj(vec![
                ("case_id", Int),
                ("binding", Str),
                ("amount", Int),
            ]))),
        ),
    ])))
}
/// Closed display calibration, not independent semantic replay.
#[allow(dead_code)]
pub fn validate_shared_resource_snapshots(v: &Value) -> Result<(), String> {
    check(v, &shared_resources_schema(), "shared_resources")?;
    complete(v, &shared_resources_schema(), "shared_resources")?;
    let rows = v.as_array().unwrap();
    if rows.is_empty() {
        return Err("empty shared resources must be omitted".into());
    }
    unique_composite(rows, &["resource"])?;
    for row in rows {
        if row["resource"].as_str() == Some("") {
            return Err("empty shared resource name".into());
        }
        let mut occupied = 0u64;
        let mut previous = None;
        for h in row["holders"].as_array().unwrap() {
            let case = h["case_id"].as_u64().ok_or("holder case")?;
            let binding = h["binding"].as_str().ok_or("holder binding")?;
            let amount = h["amount"].as_u64().ok_or("holder amount")?;
            let key = (case, binding);
            if case == 0 || binding.is_empty() || amount == 0 || previous.is_some_and(|p| p >= key)
            {
                return Err("invalid/duplicate/unordered holder".into());
            }
            previous = Some(key);
            occupied = occupied.checked_add(amount).ok_or("holder overflow")?;
        }
        if row["occupied"].as_u64() != Some(occupied) {
            return Err("display occupancy differs from holder quantities".into());
        }
    }
    Ok(())
}
fn resource_metrics_schema() -> Ty {
    use Ty::*;
    Arr(Box::new(Obj(vec![
        ("resource", Str),
        ("capacity", Int),
        ("occupancy_time", metric_scalar_schema()),
        ("peak_units", metric_scalar_schema()),
        ("utilization", metric_scalar_schema()),
    ])))
}
fn metric_count_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        ("stats", metric_scalar_schema()),
        ("observed_total", Null(Box::new(Int))),
    ])
}
fn metric_counts_schema(overall: bool) -> Ty {
    use Ty::*;
    let mut fields = vec![
        (
            "end_counts",
            Arr(Box::new(Obj(vec![
                ("end", Str),
                ("count", metric_count_schema()),
            ]))),
        ),
        ("rejected", metric_count_schema()),
        ("unfinished", metric_count_schema()),
        ("deadlocked", metric_count_schema()),
        ("not_arrived", metric_count_schema()),
        ("admitted", OptionalField(Box::new(metric_count_schema()))),
        ("overflowed", OptionalField(Box::new(metric_count_schema()))),
        ("cut_off", OptionalField(Box::new(metric_count_schema()))),
    ];
    if overall {
        fields.extend([
            ("offered", metric_count_schema()),
            ("received", metric_count_schema()),
            ("completed", metric_count_schema()),
        ]);
    }
    Obj(fields)
}
fn business_metrics_schema() -> Ty {
    use Ty::*;
    let scalar = metric_scalar_schema;
    let count = metric_count_schema;
    let rate = |id, amount| {
        Obj(vec![
            (id, Str),
            (amount, Null(Box::new(Int))),
            ("reason", Null(Box::new(Lit(&["not_configured"])))),
            ("required", Bool),
        ])
    };
    Obj(vec![
        (
            "rules",
            Lit(&["business-metrics-p1-1", "business-metrics-p1-resource-1"]),
        ),
        (
            "resources",
            OptionalField(Box::new(resource_metrics_schema())),
        ),
        (
            "status",
            Lit(&["complete", "truncated", "aborted", "range_stopped"]),
        ),
        ("window", Int),
        ("currency", Str),
        ("time_basis", Lit(&["arrival_to_declared_end"])),
        ("lead_time_scope", Lit(&["terminal_cases_only"])),
        (
            "observation_scope",
            Lit(&[
                "until_execution_stop_no_extra_drain",
                "until_horizon_or_execution_stop",
            ]),
        ),
        ("statistical_unit", Null(Box::new(Lit(&["replication"])))),
        (
            "estimate_note",
            Null(Box::new(Lit(&[
                "点推定であり、全スケジュールの検証でも区間推定でもない",
            ]))),
        ),
        (
            "rates",
            Obj(vec![
                ("actors", Arr(Box::new(rate("actor", "per_minute")))),
                (
                    "capabilities",
                    Arr(Box::new(rate("capability", "per_start"))),
                ),
            ]),
        ),
        (
            "cases",
            Arr(Box::new(Obj(vec![
                ("case_id", Int),
                ("arrival", Int),
                ("counts", metric_counts_schema(false)),
                ("first_start_wait", scalar()),
                ("external_wait", OptionalField(Box::new(scalar()))),
                ("intentional_wait", OptionalField(Box::new(scalar()))),
                ("dwell", OptionalField(Box::new(wait_dwell_schema()))),
                ("joins", OptionalField(Box::new(wait_joins_schema()))),
                (
                    "terminals",
                    Arr(Box::new(Obj(vec![
                        ("kind", Lit(&["end", "rejected", "overflow"])),
                        ("end", Null(Box::new(Str))),
                        ("lead_time", scalar()),
                        (
                            "breakdown",
                            OptionalField(Box::new(wait_breakdown_schema())),
                        ),
                        ("cost_per_case", scalar()),
                        ("count", count()),
                        (
                            "excluded",
                            Obj(vec![
                                ("unfinished", count()),
                                ("deadlocked", count()),
                                ("rejected", count()),
                                ("other_end", count()),
                                ("cut_off", OptionalField(Box::new(count()))),
                            ]),
                        ),
                    ]))),
                ),
                (
                    "tasks",
                    Arr(Box::new(Obj(vec![
                        ("binding", Str),
                        ("capability", Str),
                        ("wait", scalar()),
                        ("wait_causes", OptionalField(Box::new(wait_causes_schema()))),
                        (
                            "predecessor_wait",
                            OptionalField(Box::new(wait_scalar_schema())),
                        ),
                        ("refused_count", OptionalField(Box::new(scalar()))),
                        ("refusal_wait", OptionalField(Box::new(scalar()))),
                        ("work", scalar()),
                        ("starts", scalar()),
                        ("never_created_count", count()),
                        ("cancelled_before_start_count", count()),
                        ("not_started_count", count()),
                    ]))),
                ),
                ("work", scalar()),
                ("observed_cost_total", scalar()),
            ]))),
        ),
        (
            "terminals",
            Arr(Box::new(Obj(vec![
                ("end", Str),
                ("kind", OptionalField(Box::new(Lit(&["end", "overflow"])))),
                ("count", count()),
                ("cost_per_case", scalar()),
                ("completed_cases_total", Null(Box::new(Int))),
            ]))),
        ),
        (
            "actors",
            Arr(Box::new(Obj(vec![
                ("actor", Str),
                ("capacity", Null(Box::new(Int))),
                ("queue", OptionalField(Box::new(wait_actor_queue_schema()))),
                ("work", scalar()),
                ("peak_tasks", scalar()),
                ("utilization", scalar()),
                ("occupancy_cost", scalar()),
            ]))),
        ),
        (
            "people",
            Arr(Box::new(Obj(vec![
                ("person", Str),
                ("work", scalar()),
                ("peak_tasks", scalar()),
            ]))),
        ),
        (
            "capabilities",
            Arr(Box::new(Obj(vec![
                ("capability", Str),
                ("starts", scalar()),
                ("start_cost", scalar()),
            ]))),
        ),
        (
            "overall",
            Obj(vec![
                ("counts", metric_counts_schema(true)),
                ("observed_until", scalar()),
                (
                    "queue_wait_observed",
                    OptionalField(Box::new(wait_scalar_schema())),
                ),
                ("work", scalar()),
                ("named_people_work", scalar()),
                ("intentional_wait", OptionalField(Box::new(scalar()))),
                ("peak_tasks", scalar()),
                ("occupancy_cost", scalar()),
                ("start_cost", scalar()),
                ("observed_cost_total", scalar()),
                ("observed_cost_per_arrival", scalar()),
                (
                    "business_value",
                    Obj(vec![
                        ("value", Null(Box::new(MetricNumber))),
                        ("reason", Lit(&["not_configured"])),
                    ]),
                ),
            ]),
        ),
        (
            "checked",
            Arr(Box::new(OneOf(vec![
                Lit(METRIC_CHECKED),
                Lit(WAIT_METRIC_CHECKED),
                Lit(&[
                    "resource_occupancy_time",
                    "resource_peak_units",
                    "resource_capacity_window_utilization",
                ]),
            ]))),
        ),
        (
            "unchecked",
            Arr(Box::new(OneOf(vec![
                Lit(METRIC_UNCHECKED),
                Lit(&["queue_length_quantiles"]),
            ]))),
        ),
    ])
}
const WAIT_METRIC_CHECKED: &[&str] = &["waiting_decomposition", "queue_length_max_and_average"];
const WAIT_CAUSES: &[&str] = &[
    "actor_capacity",
    "resource",
    "executor",
    "combined",
    "unattributed",
];
const WAIT_AFFECTS: &str = "区分は時刻ごとに一つ。external、in_service、contest_route、queue、intentional、unattributedの順で決める。原因は全案件のTaskの容量・資源・人の占有から読む。異なる原因又は既知と不明の混在はcombinedとし、説明できない時間を隠さない。前工程待ちは受入から、first_start_waitは到着から数える。網羅の区分の範囲の和はlead timeの範囲と一致するとは限らない。dwellとhorizonで止めたqueueの和・時間平均のlowerは固定した観測期間内の観測量の印であり、打切り後まで延長した期間の平均の下界を主張しない。";

fn wait_scalar_schema() -> Ty {
    metric_scalar_schema()
}
fn wait_join_scalar_schema() -> Ty {
    use Ty::*;
    let base = || {
        vec![
            ("min", Null(Box::new(MetricNumber))),
            ("max", Null(Box::new(MetricNumber))),
            (
                "reason",
                Null(Box::new(OneOf(vec![
                    Lit(METRIC_REASONS),
                    Lit(&["join_any"]),
                ]))),
            ),
        ]
    };
    let mut sample = base();
    sample.extend([("mean", Null(Box::new(MetricNumber))), ("samples", Int)]);
    OneOf(vec![Obj(base()), Obj(sample)])
}
fn wait_causes_schema() -> Ty {
    Ty::Obj(
        WAIT_CAUSES
            .iter()
            .map(|&k| (k, wait_scalar_schema()))
            .collect(),
    )
}
fn wait_breakdown_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        ("external", wait_scalar_schema()),
        ("in_service", wait_scalar_schema()),
        ("contest_route", wait_scalar_schema()),
        ("intentional", wait_scalar_schema()),
        ("unattributed", wait_scalar_schema()),
        (
            "queue",
            Obj(WAIT_CAUSES[..4]
                .iter()
                .map(|&k| (k, wait_scalar_schema()))
                .collect()),
        ),
    ])
}
fn wait_dwell_schema() -> Ty {
    Ty::Obj(vec![
        ("until", wait_scalar_schema()),
        ("breakdown", wait_breakdown_schema()),
        ("bound", Ty::Lit(&["lower"])),
    ])
}
fn wait_joins_schema() -> Ty {
    use Ty::*;
    Arr(Box::new(Obj(vec![
        ("fork", Str),
        ("mode", Lit(&["all", "any"])),
        (
            "branches",
            Arr(Box::new(Obj(vec![
                ("binding", Str),
                ("join_wait", wait_join_scalar_schema()),
            ]))),
        ),
    ])))
}
fn wait_actor_queue_schema() -> Ty {
    use Ty::*;
    let observed = || {
        Obj(vec![
            ("stats", wait_scalar_schema()),
            ("bound", OptionalField(Box::new(Lit(&["lower"])))),
        ])
    };
    Obj(vec![
        ("wait_total", observed()),
        ("length_max", wait_scalar_schema()),
        ("length_time_average", observed()),
    ])
}
fn waiting_declared(v: &Value) -> bool {
    v["assumptions"]
        .as_array()
        .is_some_and(|a| a.iter().any(|r| r["field"] == "metrics.waiting"))
}
fn reject_unscoped_waiting(v: &Value) -> Result<(), String> {
    let m = &v["metrics"];
    let contains = m["overall"].get("queue_wait_observed").is_some()
        || m["actors"]
            .as_array()
            .is_some_and(|a| a.iter().any(|r| r.get("queue").is_some()))
        || m["cases"].as_array().is_some_and(|a| {
            a.iter().any(|c| {
                c.get("joins").is_some()
                    || c.get("dwell").is_some()
                    || c["terminals"]
                        .as_array()
                        .is_some_and(|ts| ts.iter().any(|t| t.get("breakdown").is_some()))
                    || c["tasks"].as_array().is_some_and(|ts| {
                        ts.iter().any(|t| {
                            t.get("wait_causes").is_some() || t.get("predecessor_wait").is_some()
                        })
                    })
            })
        });
    if contains {
        Err("waiting fields without explicit metrics.waiting declaration".into())
    } else {
        Ok(())
    }
}

/// Only named A7 scopes are projected. All old validation still runs afterwards;
/// a namesake key in any other scope is neither accepted nor erased.
fn waiting_projection(v: &Value) -> Value {
    let mut old = v.clone();
    let m = &mut old["metrics"];
    for c in m["cases"].as_array_mut().unwrap() {
        for k in ["dwell", "joins"] {
            c.as_object_mut().unwrap().remove(k);
        }
        for t in c["terminals"].as_array_mut().unwrap() {
            t.as_object_mut().unwrap().remove("breakdown");
        }
        for t in c["tasks"].as_array_mut().unwrap() {
            for k in ["wait_causes", "predecessor_wait"] {
                t.as_object_mut().unwrap().remove(k);
            }
        }
    }
    for a in m["actors"].as_array_mut().unwrap() {
        a.as_object_mut().unwrap().remove("queue");
    }
    m["overall"]
        .as_object_mut()
        .unwrap()
        .remove("queue_wait_observed");
    m["checked"]
        .as_array_mut()
        .unwrap()
        .retain(|x| !WAIT_METRIC_CHECKED.iter().any(|w| x == w));
    m["unchecked"] = serde_json::json!(METRIC_UNCHECKED);
    old["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["field"] != "metrics.waiting");
    old
}

fn waiting_scalar_tree(
    v: &Value,
    sim: bool,
    status: &str,
    reps: u64,
    zero_average: bool,
) -> Result<(), String> {
    if let Some(o) = v.as_object() {
        if o.contains_key("min") {
            let mut legacy = v.clone();
            if !sim && v["reason"] == "no_samples" && !zero_average {
                return Err(
                    "exhaustive no_samples is restricted to zero-observation queue average".into(),
                );
            }
            if v["reason"] == "join_any" || !sim && v["reason"] == "no_samples" {
                legacy["reason"] = Value::String("no_terminal".into());
            }
            return scalar_invariants(&legacy, sim, status, reps);
        }
        for (key, child) in o {
            waiting_scalar_tree(
                child,
                sim,
                status,
                reps,
                zero_average || key == "length_time_average",
            )?;
        }
    } else if let Some(a) = v.as_array() {
        for child in a {
            waiting_scalar_tree(child, sim, status, reps, zero_average)?;
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn validate_waiting_wire(v: &Value) -> Result<(), String> {
    if !waiting_declared(v) {
        return reject_unscoped_waiting(v);
    }
    let m = v
        .get("metrics")
        .ok_or("waiting assumption without metrics")?;
    check(m, &business_metrics_schema(), "waiting metrics")?;
    complete(m, &business_metrics_schema(), "waiting metrics")?;
    let assumptions = v["assumptions"].as_array().ok_or("waiting assumptions")?;
    let rows: Vec<_> = assumptions
        .iter()
        .enumerate()
        .filter(|(_, r)| r["field"] == "metrics.waiting")
        .collect();
    if rows.len() != 1
        || rows[0].1["value"] != "exclusive_interval_partition_v1"
        || rows[0].1["affects"] != WAIT_AFFECTS
    {
        return Err("waiting assumption value/text/inventory".into());
    }
    for (i, r) in assumptions.iter().enumerate() {
        let f = r["field"].as_str().ok_or("assumption field")?;
        if f == "quality.quantile" || f == "probability.scheduler" {
            if i <= rows[0].0 {
                return Err("waiting assumption must precede quantile/scheduler".into());
            }
        } else if f != "metrics.waiting" && i >= rows[0].0 {
            return Err("waiting assumption must follow existing rows".into());
        }
    }
    let checked = m["checked"].as_array().ok_or("waiting checked")?;
    let suffix = serde_json::json!(WAIT_METRIC_CHECKED);
    if !checked.ends_with(suffix.as_array().unwrap())
        || checked
            .iter()
            .filter(|x| WAIT_METRIC_CHECKED.iter().any(|w| *x == w))
            .count()
            != WAIT_METRIC_CHECKED.len()
    {
        return Err("waiting checked suffix".into());
    }
    let want: Vec<_> = METRIC_UNCHECKED
        .iter()
        .filter_map(|&w| match w {
            "waiting_decomposition" => None,
            "queue_length_statistics" => Some("queue_length_quantiles"),
            _ => Some(w),
        })
        .collect();
    if m["unchecked"] != serde_json::json!(want) {
        return Err("waiting unchecked vocabulary/order".into());
    }
    let sim = v["mode"] == "simulation";
    let status = m["status"].as_str().ok_or("waiting status")?;
    let reps = v["simulation"]["replications"].as_u64().unwrap_or(0);
    let scalar = |x: &Value| waiting_scalar_tree(x, sim, status, reps, false);
    let complete_field = |x: &Value, schema: Ty| -> Result<(), String> {
        check(x, &schema, "waiting field")?;
        complete(x, &schema, "waiting field")?;
        scalar(x)
    };
    complete_field(
        m["overall"]
            .get("queue_wait_observed")
            .ok_or("missing queue_wait_observed")?,
        wait_scalar_schema(),
    )?;
    for a in m["actors"].as_array().unwrap() {
        let q = a.get("queue").ok_or("Actor queue missing")?;
        complete_field(q, wait_actor_queue_schema())?;
        if q["wait_total"].get("bound") != q["length_time_average"].get("bound") {
            return Err("waiting lower flags disagree".into());
        }
        if q["wait_total"].get("bound").is_some() && !v["scenario"]["horizon"].is_u64() {
            return Err(
                "waiting lower requires a business horizon, not a computational clock".into(),
            );
        }
        if m["overall"]["counts"]["cut_off"]["stats"]["max"]
            .as_str()
            .is_some_and(|n| n != "0")
            && q["wait_total"]["bound"] != "lower"
        {
            return Err("actual horizon cutoff requires queue observation lower flags".into());
        }
        if (status == "aborted" || status == "range_stopped" || status != "complete" && !sim)
            && q["wait_total"].get("bound").is_some()
        {
            return Err("computational invalidation must remove lower".into());
        }
        if !sim
            && q["length_time_average"]["stats"]["reason"] == "no_samples"
            && m["overall"]["observed_until"]["max"] != "0"
        {
            return Err("exhaustive no_samples only at zero observation".into());
        }
    }
    for c in m["cases"].as_array().unwrap() {
        if let Some(dwell) = c.get("dwell") {
            complete_field(dwell, wait_dwell_schema())?;
            if status == "aborted" || status == "range_stopped" || !sim && status != "complete" {
                return Err("computational cut cannot publish dwell".into());
            }
            if c["counts"]["cut_off"]["stats"]["max"]
                .as_str()
                .is_none_or(|n| n == "0")
            {
                return Err("dwell requires an actually cut-off case observation".into());
            }
        }
        let joins = c.get("joins").ok_or("joins required even when empty")?;
        complete_field(joins, wait_joins_schema())?;
        unique_composite(joins.as_array().unwrap(), &["fork"])?;
        for j in joins.as_array().unwrap() {
            if j["fork"].as_str().unwrap().is_empty()
                || j["branches"].as_array().unwrap().is_empty()
            {
                return Err("empty fork/branches".into());
            }
            unique_composite(j["branches"].as_array().unwrap(), &["binding"])?;
            for b in j["branches"].as_array().unwrap() {
                if status != "aborted"
                    && status != "range_stopped"
                    && (sim || status == "complete")
                    && ((j["mode"] == "any") != (b["join_wait"]["reason"] == "join_any"))
                {
                    return Err("join any reason must be join_any, all must not be join_any".into());
                }
            }
        }
        for t in c["terminals"].as_array().unwrap() {
            let b = t.get("breakdown").ok_or("terminal breakdown missing")?;
            complete_field(b, wait_breakdown_schema())?;
            if t["lead_time"]["min"].is_null() {
                for (key, x) in b.as_object().unwrap() {
                    let xs: Vec<_> = if key == "queue" {
                        x.as_object().unwrap().values().collect()
                    } else {
                        vec![x]
                    };
                    if xs
                        .iter()
                        .any(|s| s["reason"] != t["lead_time"]["reason"] || !s["min"].is_null())
                    {
                        return Err(
                            "null lead time requires identical null breakdown reason".into()
                        );
                    }
                }
            }
        }
        for t in c["tasks"].as_array().unwrap() {
            let causes = t.get("wait_causes").ok_or("task causes missing")?;
            complete_field(causes, wait_causes_schema())?;
            complete_field(
                t.get("predecessor_wait").ok_or("predecessor missing")?,
                wait_scalar_schema(),
            )?;
            if t["wait"]["min"].is_null()
                && causes
                    .as_object()
                    .unwrap()
                    .values()
                    .any(|s| s["reason"] != t["wait"]["reason"] || !s["min"].is_null())
            {
                return Err("null task wait requires identical cause reasons".into());
            }
        }
    }
    Ok(())
}

fn metric_table_schema(compare: bool) -> Ty {
    use Ty::*;
    let key = Obj(vec![
        (
            "scope",
            Lit(&[
                "overall",
                "case",
                "case_terminal",
                "task",
                "terminal",
                "actor",
                "person",
                "capability",
                "resource",
            ]),
        ),
        ("case_id", Null(Box::new(Int))),
        ("end", Null(Box::new(Str))),
        ("subject", Null(Box::new(Str))),
        ("metric", Str),
    ]);
    let cell = || {
        Obj(vec![
            ("status", Lit(&["reported", "not_applicable"])),
            (
                "value",
                Null(Box::new(OneOf(vec![
                    metric_scalar_schema(),
                    metric_count_schema(),
                ]))),
            ),
        ])
    };
    let mut fields = vec![("key", key)];
    if compare {
        let Obj(mut vs) = cell() else { unreachable!() };
        vs.insert(0, ("configuration", Str));
        fields.push(("values", Arr(Box::new(Obj(vs)))));
    } else {
        fields.extend([("from", cell()), ("to", cell())]);
    }
    Arr(Box::new(Obj(fields)))
}

// Decimal natural arithmetic keeps this validator independent of production
// MetricNumber and of the [0,1]-only probability validator. No floats or u64 cap.
fn nat_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}
fn nat_sub(a: &str, b: &str) -> String {
    let mut out = Vec::new();
    let mut borrow = 0i16;
    let mut bs = b.bytes().rev();
    for x in a.bytes().rev() {
        let mut d = (x - b'0') as i16 - borrow - bs.next().map(|x| (x - b'0') as i16).unwrap_or(0);
        borrow = if d < 0 { 1 } else { 0 };
        if d < 0 {
            d += 10;
        }
        out.push(b'0' + d as u8);
    }
    while out.len() > 1 && out.last() == Some(&b'0') {
        out.pop();
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}
fn nat_mul(a: &str, b: &str) -> String {
    let mut ds = vec![0u32; a.len() + b.len()];
    for (i, x) in a.bytes().rev().enumerate() {
        for (j, y) in b.bytes().rev().enumerate() {
            ds[i + j] += (x - b'0') as u32 * (y - b'0') as u32;
        }
    }
    for i in 0..ds.len() - 1 {
        ds[i + 1] += ds[i] / 10;
        ds[i] %= 10;
    }
    while ds.len() > 1 && ds.last() == Some(&0) {
        ds.pop();
    }
    ds.into_iter()
        .rev()
        .map(|x| (b'0' + x as u8) as char)
        .collect()
}
fn nat_rem(a: &str, b: &str) -> String {
    let mut r = "0".to_owned();
    for d in a.chars() {
        if r == "0" {
            r.clear();
        }
        r.push(d);
        while nat_cmp(&r, b) != std::cmp::Ordering::Less {
            r = nat_sub(&r, b);
        }
    }
    r
}
fn metric_fraction(v: &Value) -> Result<(String, String), String> {
    let s = v
        .as_str()
        .ok_or("metric number must be a canonical rational string")?;
    let (n, d) = s.split_once('/').unwrap_or((s, "1"));
    let canonical = |x: &str| {
        !x.is_empty() && x.bytes().all(|x| x.is_ascii_digit()) && (x == "0" || !x.starts_with('0'))
    };
    if !canonical(n) || !canonical(d) || d == "0" || s.contains('/') && d == "1" {
        return Err("noncanonical metric number".into());
    }
    let (mut a, mut b) = (n.to_owned(), d.to_owned());
    while b != "0" {
        let r = nat_rem(&a, &b);
        a = b;
        b = r;
    }
    if a != "1" {
        return Err("unreduced metric fraction".into());
    }
    Ok((n.into(), d.into()))
}
fn metric_le(a: &Value, b: &Value) -> Result<bool, String> {
    let (an, ad) = metric_fraction(a)?;
    let (bn, bd) = metric_fraction(b)?;
    Ok(nat_cmp(&nat_mul(&an, &bd), &nat_mul(&bn, &ad)) != std::cmp::Ordering::Greater)
}
fn scalar_invariants(v: &Value, sim: bool, status: &str, r: u64) -> Result<(), String> {
    check(v, &metric_scalar_schema(), "metric scalar")?;
    complete(v, &metric_scalar_schema(), "metric scalar")?;
    if v.get("samples").is_some() != sim || v.get("mean").is_some() != sim {
        return Err("metric scalar analysis form".into());
    }
    let empty = v["min"].is_null();
    if empty != v["max"].is_null() || sim && empty != v["mean"].is_null() {
        return Err("partial metric null".into());
    }
    if empty {
        if v["reason"].is_null() || sim && v["samples"] != 0 {
            return Err("null requires reason and zero samples".into());
        }
    } else {
        if !v["reason"].is_null() || !metric_le(&v["min"], &v["max"])? {
            return Err("metric reason/extrema".into());
        }
        if sim
            && (v["samples"].as_u64().unwrap() == 0
                || v["samples"].as_u64().unwrap() > r
                || !metric_le(&v["min"], &v["mean"])?
                || !metric_le(&v["mean"], &v["max"])?)
        {
            return Err("metric samples/mean range".into());
        }
    }
    if (status == "aborted" || !sim && status == "truncated" || status == "range_stopped")
        && (!empty || v["reason"] != status)
    {
        return Err("partial extrema after cut/abort".into());
    }
    // ADR 0029 U2: the range word only under a range-stopped table.
    if v["reason"] == "range_stopped" && status != "range_stopped" {
        return Err("range_stopped reason outside a range-stopped table".into());
    }
    if !sim && v["reason"] == "no_samples" {
        return Err("no_samples only in simulation".into());
    }
    Ok(())
}
fn metric_tree(v: &Value, sim: bool, status: &str, r: u64) -> Result<(), String> {
    if let Some(o) = v.as_object() {
        if o.contains_key("min") {
            scalar_invariants(v, sim, status, r)?;
        } else if o.contains_key("stats") {
            scalar_invariants(&v["stats"], sim, status, r)?;
            for key in ["min", "max"] {
                if !v["stats"][key].is_null() && metric_fraction(&v["stats"][key])?.1 != "1" {
                    return Err("count extrema must be integers".into());
                }
            }
            if sim && status != "aborted" {
                let total = v["observed_total"]
                    .as_u64()
                    .ok_or("simulation count total")?;
                if v["stats"]["samples"] != r {
                    return Err("counts must observe all replications".into());
                }
                let (n, d) = metric_fraction(&v["stats"]["mean"])?;
                if nat_mul(&n, &r.to_string()) != nat_mul(&total.to_string(), &d) {
                    return Err("count mean/sample/total mismatch".into());
                }
            } else if !v["observed_total"].is_null() {
                return Err("exhaustive/aborted count total is null".into());
            }
        } else {
            for x in o.values() {
                metric_tree(x, sim, status, r)?;
            }
        }
    } else if let Some(a) = v.as_array() {
        for x in a {
            metric_tree(x, sim, status, r)?;
        }
    }
    Ok(())
}
fn without_metrics(v: &Value) -> Value {
    let mut old = v.clone();
    old.as_object_mut().unwrap().remove("metrics");
    if let Some(a) = old["assumptions"].as_array_mut() {
        a.retain(|a| {
            !a["field"]
                .as_str()
                .is_some_and(|x| x.starts_with("metrics."))
        });
    }
    old
}

/// ADR 0045 (plan B6 section 2.7): the closed quality assumption block. It is
/// present exactly when the report carries `quality`, is the four rows
/// `quality.definition`, `quality.denominator`, `quality.form` and
/// `quality.scheduler` (exhaustive) / `quality.estimation` (simulation) in
/// this order, contiguous, after every existing row and immediately before
/// `probability.scheduler` when that row exists (otherwise last). ADR 0052 removes
/// the historical `quality.rule` disclosure; its presence is now an error.
/// Rule state alone cannot withdraw numerical quality on a non-refused run. On success
/// the report is returned without `quality` and without the block, so every
/// pre-existing assumption check runs unchanged on the remaining rows.
fn without_quality(v: &Value) -> Result<Option<Value>, String> {
    // Quantile disclosure follows the old quality block and all other old
    // assumptions. Remove only its exact registered row before the unchanged
    // B6 block validator runs; no prefix-based quality erasure.
    if v.get("quality")
        .is_some_and(|q| q.get("quantiles").is_some())
    {
        let rows = v["assumptions"]
            .as_array()
            .ok_or("quantile assumptions missing")?;
        let positions: Vec<_> = rows
            .iter()
            .enumerate()
            .filter(|(_, a)| a["field"] == "quality.quantile")
            .map(|(i, _)| i)
            .collect();
        if positions.len() != 1 {
            return Err("exactly one quantile assumption required".into());
        }
        let i = positions[0];
        if rows[i]["value"] != "left-inverse-first-hit-no-interpolation"
            || !rows[i]["affects"].is_string()
            || rows[i + 1..]
                .iter()
                .any(|a| a["field"] != "probability.scheduler")
            || rows[..i]
                .iter()
                .any(|a| a["field"] == "probability.scheduler")
        {
            return Err("quantile assumption fields/order".into());
        }
        let mut old = v.clone();
        old["quality"].as_object_mut().unwrap().remove("quantiles");
        old["assumptions"].as_array_mut().unwrap().remove(i);
        return without_quality(&old);
    }
    const NAMES: [&str; 5] = [
        "quality.definition",
        "quality.denominator",
        "quality.form",
        "quality.scheduler",
        "quality.estimation",
    ];
    let Some(rows) = v["assumptions"].as_array() else {
        return if v.get("quality").is_some() {
            Err("quality without assumptions".into())
        } else {
            Ok(None)
        };
    };
    if rows.iter().any(|a| a["field"] == "quality.rule") {
        return Err("obsolete quality Rule disclosure".into());
    }
    let positions: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, a)| a["field"].as_str().is_some_and(|f| NAMES.contains(&f)))
        .map(|(i, _)| i)
        .collect();
    let Some(q) = v.get("quality") else {
        return if positions.is_empty() {
            Ok(None)
        } else {
            Err("quality assumptions without quality".into())
        };
    };
    let last = match q["analysis"].as_str() {
        Some("exhaustive") => "quality.scheduler",
        Some("simulation") => "quality.estimation",
        _ => return Err("quality analysis".into()),
    };
    let rule = v["actors"]
        .as_array()
        .is_some_and(|actors| actors.iter().any(|a| a.get("rule").is_some()));
    // Reasoned nulls (zero denominator, censoring, numerical/computational cuts)
    // remain valid. Only the old Rule-only semantic withdrawal is forbidden.
    // A real semantic refusal can coexist with an already-known violation.
    let semantic_refusal = v["verdict"] == "unsupported"
        || v["diagnostics"].as_array().is_some_and(|rows| {
            rows.iter().any(|d| {
                d["severity"] == "error"
                    && (d["code"] == "U-SEM-0"
                        || v["verdict"] == "invalid"
                            && d["code"]
                                .as_str()
                                .is_some_and(|code| code.starts_with("E-")))
            })
        });
    if rule
        && !semantic_refusal
        && q["properties"].as_array().is_some_and(|rows| {
            rows.iter()
                .any(|p| p["expected_count"]["reason"] == "aborted")
        })
    {
        return Err("Rule alone cannot withdraw quality".into());
    }
    let want = vec![NAMES[0], NAMES[1], NAMES[2], last];
    let size = want.len();
    let start = *positions.first().ok_or("quality assumptions missing")?;
    if positions != (start..start + size).collect::<Vec<_>>()
        || rows[start..start + size]
            .iter()
            .zip(want)
            .any(|(a, f)| a["field"] != f || !a["value"].is_string() || !a["affects"].is_string())
    {
        return Err("quality assumption fields/order".into());
    }
    let rest = &rows[start + size..];
    // The old simulation rows (including metrics.estimation) precede quality.
    // The former simulation-tail exception admitted the c13a4d66 regression;
    // it was not historical behavior (c7onto review ruling, 2026-10-31).
    if !(rest.is_empty() || rest.len() == 1 && rest[0]["field"] == "probability.scheduler") {
        return Err("quality assumptions must follow existing rows".into());
    }
    // With probability.scheduler present the block sits immediately before it,
    // never after it at the very end.
    if rows[..start]
        .iter()
        .any(|a| a["field"] == "probability.scheduler")
    {
        return Err("quality assumptions must precede probability.scheduler".into());
    }
    let mut old = v.clone();
    old.as_object_mut().unwrap().remove("quality");
    old["assumptions"]
        .as_array_mut()
        .unwrap()
        .drain(start..start + size);
    Ok(Some(old))
}

fn reject_unscoped_metric_assumptions(v: &Value) -> Result<(), String> {
    if v["assumptions"].as_array().is_some_and(|a| {
        a.iter().any(|x| {
            x["field"]
                .as_str()
                .is_some_and(|s| s.starts_with("metrics."))
        })
    }) {
        return Err("metric assumptions without metrics declaration".into());
    }
    Ok(())
}

pub fn validate_business_metrics_wire(v: &Value) -> Result<(), String> {
    if waiting_declared(v) {
        validate_waiting_wire(v)?;
        return validate_business_metrics_wire(&waiting_projection(v));
    }
    reject_unscoped_waiting(v)?;
    if let Some(old) = without_quality(v)? {
        return validate_business_metrics_wire(&old);
    }
    let m = v.get("metrics").ok_or("metrics missing")?;
    if let Some(rows) = v["assumptions"].as_array() {
        let limits = limits_declaration(rows)?;
        validate_limits_fields(v, &limits)?;
    }
    if let Some(rows) = m.get("resources") {
        check(rows, &resource_metrics_schema(), "metrics.resources")?;
        complete(rows, &resource_metrics_schema(), "metrics.resources")?;
        if rows.as_array().unwrap().is_empty() || m["rules"] != "business-metrics-p1-resource-1" {
            return Err("resource inventory/rules must be nonempty and versioned".into());
        }
        unique_composite(rows.as_array().unwrap(), &["resource"])?;
        if rows
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["resource"].as_str() == Some(""))
        {
            return Err("empty resource name".into());
        }
        let mut checked: Vec<Value> = METRIC_CHECKED
            .iter()
            .map(|x| Value::String((*x).into()))
            .collect();
        checked.extend(
            [
                "resource_occupancy_time",
                "resource_peak_units",
                "resource_capacity_window_utilization",
            ]
            .iter()
            .map(|x| Value::String((*x).into())),
        );
        if m["checked"] != Value::Array(checked) {
            return Err("resource checked vocabulary/order".into());
        }
        metric_tree(
            rows,
            v["mode"] == "simulation",
            m["status"].as_str().ok_or("status")?,
            v["simulation"]["replications"].as_u64().unwrap_or(0),
        )?;
        let mut old = v.clone();
        let oldm = old["metrics"].as_object_mut().unwrap();
        oldm.remove("resources");
        oldm.insert(
            "rules".into(),
            Value::String("business-metrics-p1-1".into()),
        );
        oldm.insert("checked".into(), serde_json::json!(METRIC_CHECKED));
        return validate_business_metrics_wire(&old);
    }
    if m["rules"] == "business-metrics-p1-resource-1" {
        return Err("resource rules without inventory".into());
    }
    check(m, &business_metrics_schema(), "metrics")?;
    complete(m, &business_metrics_schema(), "metrics")?;
    let sim = v["mode"] == "simulation";
    let status = m["status"].as_str().unwrap();
    let r = if sim {
        v["simulation"]["replications"]
            .as_u64()
            .ok_or("replications")?
    } else {
        0
    };
    if v["bounds"]["cases"].is_null() {
        return Err("metrics must be absent on static failure".into());
    }
    let window = m["window"].as_u64().unwrap();
    if window == 0
        || window < v["bounds"]["clock"].as_u64().ok_or("clock")?
        || m["currency"].as_str().unwrap().trim().is_empty()
    {
        return Err("metric window/currency".into());
    }
    if sim != (m["statistical_unit"] == "replication") || sim != m["estimate_note"].is_string() {
        return Err("metric statistical unit/note".into());
    }
    if m["checked"] != serde_json::json!(METRIC_CHECKED)
        || m["unchecked"] != serde_json::json!(METRIC_UNCHECKED)
    {
        return Err("metric obligation vocabulary/order".into());
    }
    if !m["overall"]["business_value"]["value"].is_null() {
        return Err("business value is not configured".into());
    }
    if !sim && (v["bounds"]["truncated"] == true) != (status == "truncated") {
        return Err("metric truncation status".into());
    }
    if (status == "range_stopped") != (!sim && exhaustive_word(v) == "range_stopped") {
        return Err("metric range-stop status".into());
    }
    metric_tree(m, sim, status, r)?;
    if sim && status != "aborted" {
        let mut prefixes = vec![&m["overall"]];
        for field in ["actors", "people", "capabilities", "cases"] {
            prefixes.extend(m[field].as_array().unwrap());
        }
        for prefix in prefixes {
            for (key, value) in prefix.as_object().unwrap() {
                if value.get("samples").is_some()
                    && !value["min"].is_null()
                    && value["samples"] != r
                {
                    // Case first-start waits are conditional; all other direct
                    // prefix measurements in these rows observe every replication.
                    if prefix.get("case_id").is_some()
                        && matches!(key.as_str(), "first_start_wait" | "external_wait")
                    {
                        continue;
                    }
                    return Err("prefix metrics must retain all replications".into());
                }
            }
        }
        let counts = &m["overall"]["counts"];
        let total = |key: &str| {
            counts[key]["observed_total"]
                .as_u64()
                .map(u128::from)
                .ok_or_else(|| format!("missing total {key}"))
        };
        let offered = total("offered")?;
        if offered != u128::from(v["bounds"]["cases"].as_u64().unwrap()) * u128::from(r)
            || total("completed")?
                + total("rejected")?
                + total("unfinished")?
                + total("deadlocked")?
                + counts["cut_off"]["observed_total"]
                    .as_u64()
                    .map(u128::from)
                    .unwrap_or(0)
                != offered
            || total("received")? + total("rejected")? > offered
            || total("not_arrived")? > total("unfinished")?
        {
            return Err("metric global count conservation".into());
        }
    }
    for (field, id) in [
        ("actors", "actor"),
        ("people", "person"),
        ("capabilities", "capability"),
        ("terminals", "end"),
    ] {
        unique_composite(m[field].as_array().unwrap(), &[id])?;
    }
    let cases = m["cases"].as_array().unwrap();
    if cases.len() as u64 != v["bounds"]["cases"].as_u64().unwrap() {
        return Err("metric case population".into());
    }
    let ends: Vec<_> = m["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["end"].clone())
        .collect();
    let global_counts = m["overall"]["counts"]["end_counts"].as_array().unwrap();
    if global_counts.len() != ends.len()
        || global_counts.iter().zip(&ends).any(|(c, e)| c["end"] != *e)
    {
        return Err("metric overall end-count inventory/order".into());
    }
    for (t, c) in m["terminals"].as_array().unwrap().iter().zip(global_counts) {
        if t["count"] != c["count"] {
            return Err("metric overall terminal counts disagree".into());
        }
        if sim && status != "aborted" {
            if t["completed_cases_total"] != t["count"]["observed_total"] {
                return Err("completed cases are not replication samples".into());
            }
        } else if !t["completed_cases_total"].is_null() {
            return Err("completed_cases_total is simulation only".into());
        }
    }
    for (i, c) in cases.iter().enumerate() {
        if c["case_id"] != i as u64 + 1 {
            return Err("metric case ID/order".into());
        }
        unique_composite(c["terminals"].as_array().unwrap(), &["kind", "end"])?;
        unique_composite(c["tasks"].as_array().unwrap(), &["binding"])?;
        let terminals = c["terminals"].as_array().unwrap();
        if terminals.len() != ends.len() + 1 {
            return Err("metric terminal/rejection inventory".into());
        }
        for (e, t) in ends.iter().zip(terminals) {
            let kind = m["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["end"] == *e)
                .and_then(|t| t.get("kind"))
                .cloned()
                .unwrap_or(Value::String("end".into()));
            if t["kind"] != kind || t["end"] != *e {
                return Err("metric terminal order".into());
            }
        }
        let rejected = terminals.last().unwrap();
        if rejected["kind"] != "rejected" || !rejected["end"].is_null() {
            return Err("metric rejection identity".into());
        }
        let cs = c["counts"]["end_counts"].as_array().unwrap();
        if cs.len() != ends.len() || cs.iter().zip(&ends).any(|(c, e)| c["end"] != *e) {
            return Err("metric case end-count inventory/order".into());
        }
        for (t, count) in terminals.iter().zip(cs) {
            if t["count"] != count["count"] {
                return Err("metric terminal and case counts disagree".into());
            }
        }
        if rejected["count"] != c["counts"]["rejected"] {
            return Err("metric rejection count mismatch".into());
        }
        if status != "aborted"
            && (sim || status != "truncated" && status != "range_stopped")
            && rejected["lead_time"]["reason"] != "rejected"
        {
            return Err("rejected lead is never a zero sample".into());
        }
        if sim && status != "aborted" {
            let old = &v["case_outcomes"][i];
            if old["case_id"] != c["case_id"] || old["arrival"] != c["arrival"] {
                return Err("metric case identity mismatch".into());
            }
            for (key, oldkey) in [
                ("unfinished", "unfinished_count"),
                ("deadlocked", "deadlocked_count"),
            ] {
                if c["counts"][key]["observed_total"] != old[oldkey] {
                    return Err("metric/old exclusion counts".into());
                }
            }
            if old.get("cut_off_count").is_some()
                && c["counts"]["cut_off"]["observed_total"] != old["cut_off_count"]
            {
                return Err("metric/sample cutoff count mismatch".into());
            }
            for t in terminals {
                let total = old["terminals"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|x| x["kind"] == t["kind"] && x["end"] == t["end"])
                    .map(|x| x["count"].clone())
                    .unwrap_or(serde_json::json!(0));
                if t["count"]["observed_total"] != total {
                    return Err("metric/old terminal count".into());
                }
            }
        }
    }
    for (field, id, ratekey) in [
        ("actors", "actor", "per_minute"),
        ("capabilities", "capability", "per_start"),
    ] {
        let rates = m["rates"][field].as_array().unwrap();
        unique_composite(rates, &[id])?;
        let required: Vec<_> = rates
            .iter()
            .filter(|x| x["required"] == true)
            .map(|x| x[id].clone())
            .collect();
        let actual: Vec<_> = m[field]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x[id].clone())
            .collect();
        if required != actual {
            return Err("required rates/execution inventory".into());
        }
        for rate in rates {
            if rate[ratekey].is_null() != (rate["reason"] == "not_configured") {
                return Err("missing rate reason".into());
            }
        }
    }
    if status != "aborted" && (sim || status != "truncated" && status != "range_stopped") {
        let mut any_missing = false;
        for (field, id, amount, cost) in [
            ("actors", "actor", "per_minute", "occupancy_cost"),
            ("capabilities", "capability", "per_start", "start_cost"),
        ] {
            let mut missing = false;
            for rate in m["rates"][field]
                .as_array()
                .unwrap()
                .iter()
                .filter(|x| x["required"] == true)
            {
                let entry = m[field]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|x| x[id] == rate[id])
                    .unwrap();
                let scope = deferred_wire_scope(v).map_err(|error| format!("wire: {error}"))?;
                let deferred = scope.checked
                    && (scope.global
                        || scope.business
                        || if field == "actors" {
                            rate[id].as_str().is_some_and(|n| scope.actors.contains(n))
                        } else {
                            rate[id]
                                .as_str()
                                .is_some_and(|n| scope.capabilities.contains(n))
                        });
                let absent = rate[amount].is_null() || deferred;
                missing |= rate[amount].is_null();
                if (entry[cost]["reason"] == "not_configured") != absent {
                    return Err("configured vs missing metric fee".into());
                }
            }
            let scope = deferred_wire_scope(v).map_err(|error| format!("wire: {error}"))?;
            missing |= scope.checked && scope.business;
            if (m["overall"][cost]["reason"] == "not_configured") != missing {
                return Err("partial fee subtotal is not a configured total".into());
            }
            any_missing |= missing;
        }
        for key in ["observed_cost_total", "observed_cost_per_arrival"] {
            if (m["overall"][key]["reason"] == "not_configured") != any_missing {
                return Err("total must retain every missing required rate".into());
            }
        }
    }
    let assumptions: Vec<_> = v["assumptions"]
        .as_array()
        .ok_or("assumptions")?
        .iter()
        .filter(|a| {
            !a["field"]
                .as_str()
                .is_some_and(|s| s == "scenario" || s.starts_with("scenario."))
        })
        .cloned()
        .collect();
    let metric_rows: Vec<_> = assumptions
        .iter()
        .filter(|a| {
            a["field"]
                .as_str()
                .is_some_and(|x| x.starts_with("metrics."))
        })
        .collect();
    let n = if sim { 6 } else { 5 };
    if metric_rows.len() != n {
        return Err("metric assumptions missing/duplicated".into());
    }
    let values = [
        format!("{window}min"),
        m["currency"].as_str().unwrap().into(),
        "model-declarations".into(),
        "terminal_cases_only".into(),
        m["observation_scope"].as_str().unwrap().into(),
        "point-estimate-only; unit=replication".into(),
    ];
    for (i, a) in metric_rows.iter().enumerate() {
        if a["field"] != METRIC_FIELDS[i]
            || a["value"] != values[i]
            || a["affects"] != METRIC_AFFECTS[i]
        {
            return Err("metric assumption text/order".into());
        }
    }
    // ADR 0044's closed Rule tail follows the pre-existing metrics block,
    // before simulation rows (if any). Do not silently filter arbitrary rows:
    // require the complete Rule inventory and its established inner order,
    // then match its exact position as part of the anchored tail below.
    let rule_fields: Vec<_> = assumptions
        .iter()
        .filter_map(|a| a["field"].as_str().filter(|f| rule_tail_assumption(f)))
        .collect();
    let expected_rules: Vec<_> = v["actors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a.get("rule").is_some_and(|r| !r.is_null()))
        .map(|a| format!("{}.rule", a["name"].as_str().unwrap()))
        .collect();
    let actual_rules: Vec<_> = rule_fields
        .iter()
        .filter(|f| !f.starts_with("authority.") && f.ends_with(".rule"))
        .copied()
        .collect();
    if actual_rules != expected_rules {
        return Err("metrics Rule assumption inventory".into());
    }
    validate_rule_tail_wire(v)?;
    let tail = if sim { 11 } else { 5 } + rule_fields.len();
    // The scheduler assumption of a probability model is appended after the
    // metrics block (report.rs), so the metrics block ends just before it.
    let end = assumptions.len()
        - usize::from(
            assumptions
                .last()
                .is_some_and(|a| a["field"] == "probability.scheduler"),
        );
    if end < tail {
        return Err("metric assumption tail".into());
    }
    let fields: Vec<_> = assumptions[end - tail..end]
        .iter()
        .map(|a| a["field"].as_str().unwrap())
        .collect();
    let mut want = METRIC_FIELDS[..5].to_vec();
    want.extend(rule_fields);
    if sim {
        want.extend(SIM_FIELDS);
        want.push(METRIC_FIELDS[5]);
    }
    if fields != want {
        return Err("metrics and simulation assumption placement".into());
    }
    Ok(())
}

type MetricProjection = BTreeMap<(usize, u64, String, String, String), (Value, Value)>;
fn project_metrics(m: &Value) -> Result<MetricProjection, String> {
    let mut result = BTreeMap::new();
    let scopes = [
        "overall",
        "case",
        "case_terminal",
        "task",
        "terminal",
        "actor",
        "person",
        "capability",
        "resource",
    ];
    let mut insert = |scope: usize,
                      c: Option<u64>,
                      end: Option<&str>,
                      subject: Option<&str>,
                      metric: String,
                      value: Value|
     -> Result<(), String> {
        let sort = (
            scope,
            c.unwrap_or(0),
            end.unwrap_or("").into(),
            subject.unwrap_or("").into(),
            metric.clone(),
        );
        let key = serde_json::json!({"scope":scopes[scope],"case_id":c,"end":end,"subject":subject,"metric":metric});
        if result.insert(sort, (key, value)).is_some() {
            return Err("duplicate metric projection key".into());
        }
        Ok(())
    };
    // Flatten only Scalar / Count leaves, never rate metadata or business value.
    fn leaves(v: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
        if v.get("min").is_some() || v.get("stats").is_some() {
            out.push((prefix.into(), v.clone()));
        } else if let Some(o) = v.as_object() {
            for (k, x) in o {
                if k == "business_value" {
                    continue;
                }
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                leaves(x, &p, out);
            }
        } else if let Some(a) = v.as_array() {
            for x in a {
                if let Some(e) = x["end"].as_str() {
                    leaves(&x["count"], &format!("{prefix}.{e}"), out);
                }
            }
        }
    }
    let mut fields = Vec::new();
    // The old metric table excludes only these scoped top-level fields.
    let mut overall = m["overall"].clone();
    if let Some(o) = overall.as_object_mut() {
        o.remove("queue_wait_observed");
    }
    leaves(&overall, "", &mut fields);
    for (k, x) in fields {
        insert(0, None, None, None, k, x)?;
    }
    for c in m["cases"].as_array().ok_or("metric cases")? {
        let id = c["case_id"].as_u64().ok_or("case ID")?;
        let mut fields = Vec::new();
        for k in [
            "counts",
            "first_start_wait",
            "work",
            "observed_cost_total",
            "intentional_wait",
        ] {
            leaves(&c[k], k, &mut fields);
        }
        for (k, x) in fields {
            insert(1, Some(id), None, None, k, x)?;
        }
        for t in c["terminals"].as_array().unwrap() {
            let mut fields = Vec::new();
            for k in ["lead_time", "cost_per_case", "count", "excluded"] {
                leaves(&t[k], k, &mut fields);
            }
            for (k, x) in fields {
                insert(2, Some(id), t["end"].as_str(), None, k, x)?;
            }
        }
        for t in c["tasks"].as_array().unwrap() {
            let mut fields = Vec::new();
            for k in [
                "wait",
                "work",
                "starts",
                "never_created_count",
                "cancelled_before_start_count",
                "not_started_count",
            ] {
                leaves(&t[k], k, &mut fields);
            }
            for (k, x) in fields {
                insert(3, Some(id), None, t["binding"].as_str(), k, x)?;
            }
        }
    }
    for (scope, field, id) in [
        (4, "terminals", "end"),
        (5, "actors", "actor"),
        (6, "people", "person"),
        (7, "capabilities", "capability"),
    ] {
        for t in m[field].as_array().ok_or("metric inventory")? {
            let mut fields = Vec::new();
            let mut t_old = t.clone();
            if scope == 5
                && let Some(o) = t_old.as_object_mut()
            {
                o.remove("queue");
            }
            leaves(&t_old, "", &mut fields);
            for (k, x) in fields {
                insert(
                    scope,
                    None,
                    if scope == 4 { t[id].as_str() } else { None },
                    if scope == 4 { None } else { t[id].as_str() },
                    k,
                    x,
                )?;
            }
        }
    }
    if let Some(rows) = m["resources"].as_array() {
        for row in rows {
            for metric in ["occupancy_time", "peak_units", "utilization"] {
                insert(
                    8,
                    None,
                    None,
                    row["resource"].as_str(),
                    metric.into(),
                    row[metric].clone(),
                )?;
            }
        }
    }
    Ok(result)
}
fn validate_metric_tables(v: &Value, compare: bool) -> Result<(), String> {
    let table = if compare {
        &v["tables"]["metrics"]
    } else {
        &v["metrics"]
    };
    check(table, &metric_table_schema(compare), "metric table")?;
    complete(table, &metric_table_schema(compare), "metric table")?;
    let reports: Vec<_> = if compare {
        v["plans"]
            .as_array()
            .ok_or("plans")?
            .iter()
            .map(|p| (&p["configuration"], &p["report"]))
            .collect()
    } else {
        vec![
            (&v["from"]["configuration"], &v["from"]["report"]),
            (&v["to"]["configuration"], &v["to"]["report"]),
        ]
    };
    let first = reports.first().ok_or("plans missing")?.1;
    for (_, r) in &reports {
        validate_business_metrics_wire(r)?;
        if r["metrics"]["window"] != first["metrics"]["window"]
            || r["metrics"]["currency"] != first["metrics"]["currency"]
        {
            return Err("metric comparison settings mismatch".into());
        }
    }
    let aborted = reports
        .iter()
        .any(|(_, r)| r["metrics"]["status"] == "aborted");
    let rows = table.as_array().unwrap();
    if aborted {
        if !rows.is_empty() {
            return Err("aborted metric table must be empty".into());
        }
        return Ok(());
    }
    let checked = v["checked"].as_array().ok_or("checked")?;
    let unchecked = v["unchecked"].as_array().ok_or("unchecked")?;
    if !checked.contains(&serde_json::json!("metric_tables"))
        || unchecked.contains(&serde_json::json!("metrics"))
        || METRIC_COMPARISON_WORDS
            .iter()
            .any(|w| !unchecked.contains(&serde_json::json!(w)))
    {
        return Err("metric table obligation vocabulary".into());
    }
    let projections: Vec<_> = reports
        .iter()
        .map(|(_, r)| project_metrics(&r["metrics"]))
        .collect::<Result<_, _>>()?;
    let union: BTreeMap<_, _> = projections
        .iter()
        .flat_map(|p| p.iter().map(|(k, (key, _))| (k.clone(), key.clone())))
        .collect();
    if union.len() != rows.len() {
        return Err("metric table missing/extra rows".into());
    }
    for ((sort, key), row) in union.iter().zip(rows) {
        if row["key"] != *key {
            return Err("metric table key/order/duplicate".into());
        }
        let cells: Vec<_> = if compare {
            row["values"].as_array().unwrap().iter().collect()
        } else {
            vec![&row["from"], &row["to"]]
        };
        if cells.len() != reports.len() {
            return Err("metric table not rectangular".into());
        }
        for (i, cell) in cells.iter().enumerate() {
            if compare && cell["configuration"] != *reports[i].0 {
                return Err("metric plan order".into());
            }
            match projections[i].get(sort) {
                Some((_, x)) => {
                    if cell["status"] != "reported" || cell["value"] != *x {
                        return Err("metric table value not raw report projection".into());
                    }
                }
                None => {
                    if cell["status"] != "not_applicable" || !cell["value"].is_null() {
                        return Err("missing metric is not_applicable/null, not zero".into());
                    }
                }
            }
        }
    }
    Ok(())
}
fn without_metric_comparison(v: &Value, compare: bool) -> Result<Value, String> {
    // ADR 0048 review m1: waiting_tables is closed against the original wire, then
    // leaves with the embedded metrics so the old projection is checked unchanged.
    let reports: Vec<&Value> = if compare {
        v["plans"]
            .as_array()
            .ok_or("plans")?
            .iter()
            .map(|p| &p["report"])
            .collect()
    } else {
        vec![&v["from"]["report"], &v["to"]["report"]]
    };
    let waits = reports
        .iter()
        .any(|r| r["metrics"]["overall"].get("queue_wait_observed").is_some());
    let words = v["unchecked"].as_array().ok_or("unchecked")?;
    let count = words.iter().filter(|x| *x == "waiting_tables").count();
    let at = |w: &str| words.iter().position(|x| x == w);
    let late = at("waiting_tables").is_some_and(|i| {
        (compare && i + 1 != words.len())
            || ["obligation_tables", "contest_runtime_tables"]
                .iter()
                .any(|w| at(w).is_some_and(|j| j > i))
    });
    if count != usize::from(waits) || late {
        return Err("waiting_tables unchecked presence/order".into());
    }
    let mut old = v.clone();
    if compare {
        old["tables"]
            .as_object_mut()
            .ok_or("tables")?
            .remove("metrics");
        for p in old["plans"].as_array_mut().ok_or("plans")? {
            p["report"] = without_metrics(&p["report"]);
        }
    } else {
        old.as_object_mut().unwrap().remove("metrics");
        for side in ["from", "to"] {
            old[side]["report"] = without_metrics(&old[side]["report"]);
        }
    }
    old["checked"]
        .as_array_mut()
        .ok_or("checked")?
        .retain(|x| x != "metric_tables");
    let unchecked = old["unchecked"].as_array_mut().ok_or("unchecked")?;
    unchecked.retain(|x| {
        !METRIC_COMPARISON_WORDS.iter().any(|w| x == w)
            && x != "metric_tables"
            && x != "waiting_tables"
    });
    if compare && !unchecked.contains(&serde_json::json!("metrics")) {
        let i = unchecked
            .iter()
            .position(|x| x == "terminal_meaning")
            .ok_or("terminal meaning")?;
        unchecked.insert(i, serde_json::json!("metrics"));
    }
    Ok(old)
}

fn exhaustive_value_schema() -> Ty {
    use Ty::*;
    let range = || Null(Box::new(Obj(vec![("min", Int), ("max", Int)])));
    Obj(vec![
        ("completed_at", range()),
        ("elapsed", range()),
        ("work", range()),
        ("result_at", range()),
        ("probability", Null(Box::new(Probability))),
    ])
}

fn property_value_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        ("verdict", Verdict),
        ("probability", OptionalField(Box::new(Probability))),
    ])
}

fn compare_schema() -> Ty {
    use Ty::*;
    let strings = || Arr(Box::new(Str));
    let property = Obj(vec![
        ("id", Str),
        ("case_id", Null(Box::new(Int))),
        ("arrival", Null(Box::new(Int))),
        (
            "values",
            Arr(Box::new(Obj(vec![
                ("configuration", Str),
                ("verdict", Verdict),
                ("probability", Null(Box::new(Probability))),
                (
                    "basis",
                    Null(Box::new(Lit(&[
                        "statistical",
                        "witness",
                        "sample",
                        "structural",
                    ]))),
                ),
                ("hits", Null(Box::new(Int))),
                ("unknown", Null(Box::new(Int))),
                ("trials", Null(Box::new(Int))),
                ("interval", Null(Box::new(SimulationInterval))),
                ("witness", Null(Box::new(Obj(vec![("replication", Int)])))),
            ]))),
        ),
    ]);
    let terminal = Obj(vec![
        ("kind", Lit(&["end", "rejected", "overflow", "cut_off"])),
        ("end", Null(Box::new(Str))),
        ("case_id", Int),
        ("arrival", Int),
        (
            "values",
            Arr(Box::new(Obj(vec![
                ("configuration", Str),
                ("status", Lit(&["reported", "not_reported"])),
                (
                    "value",
                    Null(Box::new(OneOf(vec![
                        exhaustive_value_schema(),
                        Obj(vec![("count", Int)]),
                    ]))),
                ),
            ]))),
        ),
    ]);
    let cut_difference = Obj(vec![
        (
            "field",
            Lit(&[
                "include",
                "exclude",
                "defer",
                "question",
                "externality",
                "reentry",
            ]),
        ),
        ("left", cut_payload_schema()),
        ("right", cut_payload_schema()),
    ]);
    Obj(vec![
        ("verdict", Verdict),
        ("mode", Lit(&["compare"])),
        ("ledger", Ledger),
        ("uncertainty", Uncertainty),
        ("scenario", OptionalField(Box::new(Str))),
        (
            "analysis",
            Null(Box::new(Lit(&["exhaustive", "simulation"]))),
        ),
        (
            "plans",
            Arr(Box::new(Obj(vec![
                ("configuration", Str),
                ("report", OneOf(vec![report_schema(), simulation_schema()])),
            ]))),
        ),
        (
            "tables",
            Obj(vec![
                ("properties", Arr(Box::new(property))),
                ("terminals", Arr(Box::new(terminal))),
                (
                    "quality",
                    OptionalField(Box::new(quality_table_schema(true))),
                ),
                (
                    "metrics",
                    OptionalField(Box::new(metric_table_schema(true))),
                ),
                (
                    "agency",
                    OptionalField(Box::new(evaluation_wire::agency_table_schema())),
                ),
            ]),
        ),
        (
            "evaluation",
            OptionalField(Box::new(evaluation_wire::schema())),
        ),
        (
            "mismatch",
            Null(Box::new(Obj(vec![(
                "pairs",
                Arr(Box::new(Obj(vec![
                    ("left", Str),
                    ("right", Str),
                    (
                        "cut",
                        Null(Box::new(Obj(vec![
                            ("left_cut", Str),
                            ("right_cut", Str),
                            ("differences", Arr(Box::new(cut_difference))),
                        ]))),
                    ),
                    (
                        "terminals",
                        Null(Box::new(Obj(vec![
                            ("left_only", strings()),
                            ("right_only", strings()),
                        ]))),
                    ),
                ]))),
            )]))),
        ),
        (
            "common_random",
            OptionalField(Box::new(Null(Box::new(Obj(vec![
                ("method", Lit(&["sha256-counter-v1"])),
                ("scope", Lit(&["initial-only"])),
                (
                    "shared",
                    Arr(Box::new(Obj(vec![
                        ("kind", Lit(&["case_initial", "actor_initial"])),
                        ("case_ids", Arr(Box::new(Int))),
                        ("sites", strings()),
                        ("occurrence", Int),
                    ]))),
                ),
                (
                    "independent",
                    Arr(Box::new(Obj(vec![
                        ("configuration", Str),
                        ("process", Str),
                        ("kernel_sites", strings()),
                        ("kernel_occurrence", Int),
                        ("duration_sites", strings()),
                        ("duration_occurrence", Lit(&["positive-support-order"])),
                    ]))),
                ),
                ("coupling", Lit(&["potential-draws-common-when-admitted"])),
                ("independence", Lit(&["domain-separated-sha256-assumption"])),
                ("confidence_scope", Lit(&["within-plan-only"])),
            ]))))),
        ),
        (
            "assumptions",
            Arr(Box::new(Obj(vec![
                ("field", Str),
                ("value", Str),
                ("affects", Str),
            ]))),
        ),
        ("diagnostics", Arr(Box::new(Diagnostic))),
        ("checked", strings()),
        ("unchecked", strings()),
    ])
}

fn cut_payload_schema() -> Ty {
    use Ty::*;
    OneOf(vec![
        Null(Box::new(Str)),
        Arr(Box::new(Str)),
        Arr(Box::new(Obj(vec![
            ("target", Str),
            ("actor", Bool),
            ("reason", Str),
            ("loses", Null(Box::new(Str))),
        ]))),
    ])
}

pub fn compare_compare_report(expected: &Value, actual: &Value) -> Result<(), String> {
    check(expected, &compare_schema(), "$")?;
    validate_compare_wire(actual)?;
    compare(expected, actual, "$")
}

fn unique_composite<'a>(
    values: &'a [Value],
    fields: &[&str],
) -> Result<BTreeMap<String, &'a Value>, String> {
    let mut result = BTreeMap::new();
    for v in values {
        let key = fields
            .iter()
            .map(|f| v.get(*f).cloned().ok_or_else(|| format!("missing key {f}")))
            .collect::<Result<Vec<_>, _>>()?;
        let key = serde_json::to_string(&key).unwrap();
        if result.insert(key, v).is_some() {
            return Err("duplicate composite row key".into());
        }
    }
    Ok(result)
}

fn validate_compare_metric_end_inventory(v: &Value) -> Result<(), String> {
    use std::collections::BTreeSet;
    if !v["checked"]
        .as_array()
        .ok_or("compare checked")?
        .iter()
        .any(|w| w == "terminal_tables")
    {
        return Ok(());
    }
    // Both collectors enumerate all lexical ends of the selected process in
    // counts, including unreachable ends and overflow. These are independent
    // of table rows and observed business terminals, and survive reason/null
    // withdrawal. Quality and Agency framing instead enumerate model-wide ends.
    let actual: BTreeSet<_> = v["tables"]["terminals"]
        .as_array()
        .ok_or("terminal rows")?
        .iter()
        .filter(|r| r["kind"] == "end" || r["kind"] == "overflow")
        .map(|r| r["end"].as_str().ok_or("terminal end name"))
        .collect::<Result<_, _>>()?;
    for plan in v["plans"].as_array().ok_or("plans")? {
        let rows = plan["report"]["metrics"]["overall"]["counts"]["end_counts"]
            .as_array()
            .ok_or("selected metric End inventory")?;
        let expected: BTreeSet<_> = rows
            .iter()
            .map(|r| r["end"].as_str().ok_or("metric End name"))
            .collect::<Result<_, _>>()?;
        if rows.len() != expected.len() || actual != expected {
            return Err("terminal table differs from selected metric End inventory".into());
        }
    }
    Ok(())
}

pub fn validate_compare_wire(v: &Value) -> Result<(), String> {
    for pair in v["tables"]["agency"].as_array().into_iter().flatten() {
        let actors = |side: &str| {
            v["plans"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|p| p["configuration"] == pair[side])
                .map(|p| &p["report"]["actors"])
                .unwrap_or(&Value::Null)
        };
        validate_capacity_difference_context(&pair["differences"], actors("from"), actors("to"))?;
    }
    if let Some(plans) = v["plans"].as_array() {
        for plan in plans {
            validate_read_write_pin(&plan["report"], plan["configuration"].as_str())?;
        }
    }
    if v.get("evaluation").is_some() {
        let old = evaluation_wire::inverse(v)?;
        return validate_compare_wire(&old);
    }
    if v["tables"].get("agency").is_some() {
        return Err("Agency comparison requires a declared evaluation".into());
    }
    if let Some(plans) = v["plans"].as_array() {
        for p in plans {
            if let Some(s) = p["report"].get("scenario") {
                if v.get("scenario") != Some(&s["name"]) {
                    return Err("compare scenario must reach every plan".into());
                }
            } else if v.get("scenario").is_some() && p["report"]["bounds"]["cases"].is_u64() {
                return Err("runtime comparison plan lost scenario".into());
            }
        }
    }
    if v["tables"].get("metrics").is_some() {
        validate_metric_tables(v, true)?;
        validate_compare_metric_end_inventory(v)?;
        let old = without_metric_comparison(v, true)?;
        validate_compare_wire(&old)?;
        check(v, &compare_schema(), "compare metrics wire")?;
        complete(v, &compare_schema(), "compare metrics wire")?;
        for p in v["plans"].as_array().ok_or("plans")? {
            if v["analysis"] == "simulation" {
                validate_simulation_wire(&p["report"])?;
            } else {
                validate_wire(&p["report"])?;
            }
        }
        return Ok(());
    }
    if v["verdict"] != "invalid"
        && v["plans"]
            .as_array()
            .is_some_and(|ps| ps.iter().any(|p| p["report"].get("metrics").is_some()))
    {
        return Err("metrics declaration lost compare metric table".into());
    }
    check(v, &compare_schema(), "compare wire")?;
    complete(v, &compare_schema(), "compare wire")?;
    let sim = v["analysis"] == "simulation";
    if let Some(table) = v["tables"].get("quality") {
        let built = v["checked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "property_tables");
        validate_quality_table(
            table,
            true,
            sim,
            if built {
                v["plans"].as_array().map(Vec::as_slice)
            } else {
                None
            },
        )?;
        if !built && !table.as_array().unwrap().is_empty() {
            return Err("unchecked comparison published quality rows".into());
        }
    } else if v["checked"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w == "property_tables")
        && v["plans"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["report"].get("quality").is_some())
    {
        return Err("quality declaration lost compare quality table".into());
    }
    if sim != v.get("common_random").is_some() {
        return Err("common_random analysis scope".into());
    }
    let plans = v["plans"].as_array().unwrap();
    let names: Vec<_> = plans
        .iter()
        .map(|p| p["configuration"].as_str().unwrap())
        .collect();
    let checked = v["checked"].as_array().unwrap();
    let unchecked = v["unchecked"].as_array().unwrap();
    let feature = plans
        .iter()
        .any(|p| p["report"].get("responsibilities").is_some());
    let mut feature_words = Vec::new();
    if feature {
        feature_words.extend(["responsibility", "contest"]);
    }
    if plans
        .iter()
        .any(|p| p["report"].get("obligations").is_some())
    {
        feature_words.push("obligation_tables");
    }
    if plans
        .iter()
        .any(|p| p["report"].get("contest_runtime").is_some())
    {
        feature_words.push("contest_runtime_tables");
    }
    // ADR 0048 review m1: a declared waits model names the excluded waiting tables last.
    if plans.iter().any(|p| {
        p["report"]["metrics"]["overall"]
            .get("queue_wait_observed")
            .is_some()
    }) {
        feature_words.push("waiting_tables");
    }
    let obligations_end = unchecked
        .len()
        .checked_sub(feature_words.len())
        .ok_or("missing feature unchecked words")?;
    if unchecked[obligations_end..]
        .iter()
        .map(|v| v.as_str().unwrap())
        .ne(feature_words.iter().copied())
    {
        return Err("feature unchecked order/presence".into());
    }
    let unchecked = &unchecked[..obligations_end];
    let applicable: Vec<_> = [
        "configuration_selection",
        "cut_compatibility",
        "terminal_compatibility",
        "simulation_clause",
        "random_key_separation",
        "property_tables",
        "terminal_tables",
    ]
    .into_iter()
    .filter(|x| sim || !["simulation_clause", "random_key_separation"].contains(x))
    .collect();
    let fixed = [
        "ranking",
        "hard_filter",
        "pareto",
        "weighted_score",
        "differences",
        "difference_intervals",
        "cross_plan_confidence",
        "operation_coupling",
        "metrics",
        "terminal_meaning",
    ];
    if checked.iter().any(|x| !applicable.iter().any(|k| x == *k))
        || checked
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != checked.len()
        || unchecked.len() < fixed.len()
        || unchecked[..fixed.len()]
            .iter()
            .zip(fixed)
            .any(|(x, k)| x != k)
        || unchecked[fixed.len()..]
            .iter()
            .any(|x| !applicable.iter().any(|k| x == *k))
    {
        return Err("compare checked/unchecked vocabulary".into());
    }
    let expected_checked: Vec<_> = applicable
        .iter()
        .filter(|k| checked.contains(&Value::String((***k).to_owned())))
        .map(|s| Value::String((*s).to_owned()))
        .collect();
    if *checked != expected_checked {
        return Err("checked order".into());
    }
    if unchecked[fixed.len()..]
        != applicable
            .iter()
            .filter(|k| !checked.contains(&Value::String((***k).to_owned())))
            .map(|s| Value::String((*s).to_owned()))
            .collect::<Vec<_>>()
    {
        return Err("unchecked must retain all applicable unperformed obligations".into());
    }
    if names.iter().any(|n| !valid_plan_name(n))
        || names
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != names.len()
    {
        return Err("plan identifiers must be unique".into());
    }
    for p in plans {
        if p["report"]["mode"] != if sim { "simulation" } else { "exhaustive" } {
            return Err("analysis/nested mode mismatch".into());
        }
        if sim {
            validate_simulation_wire(&p["report"])?;
        } else {
            validate_wire(&p["report"])?;
        }
        if p["report"]["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"].as_str().unwrap().starts_with("compare."))
        {
            return Err("comparison assumption leaked into raw report".into());
        }
    }
    let rank = |s: &str| {
        [
            "satisfied",
            "vacuous",
            "inconclusive",
            "unsupported",
            "violated",
            "invalid",
        ]
        .iter()
        .position(|x| *x == s)
        .unwrap()
    };
    let mut verdicts: Vec<_> = plans
        .iter()
        .map(|p| p["report"]["verdict"].as_str().unwrap())
        .collect();
    for d in v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
    {
        verdicts.push(if d["code"].as_str().unwrap().starts_with("U-") {
            "unsupported"
        } else {
            "invalid"
        });
    }
    if v["verdict"]
        != verdicts
            .into_iter()
            .max_by_key(|s| rank(s))
            .unwrap_or("unsupported")
    {
        return Err("compare verdict fold".into());
    }
    let properties = v["tables"]["properties"].as_array().unwrap();
    let terminals = v["tables"]["terminals"].as_array().unwrap();
    unique_composite(properties, &["id", "case_id"])?;
    unique_composite(terminals, &["case_id", "kind", "end"])?;
    for row in terminals {
        validate_terminal_identity(row)?;
    }
    if !v["mismatch"].is_null() && (!properties.is_empty() || !terminals.is_empty()) {
        return Err("mismatch published tables".into());
    }
    if let Some(pairs) = v["mismatch"].get("pairs") {
        let pairs = pairs.as_array().unwrap();
        if pairs.is_empty() {
            return Err("empty mismatch".into());
        }
        unique_composite(pairs, &["left", "right"])?;
        for pair in pairs {
            let left = names
                .iter()
                .position(|n| pair["left"] == *n)
                .ok_or("unknown left mismatch plan")?;
            let right = names
                .iter()
                .position(|n| pair["right"] == *n)
                .ok_or("unknown right mismatch plan")?;
            if left >= right || (pair["cut"].is_null() && pair["terminals"].is_null()) {
                return Err("invalid mismatch pair".into());
            }
            if let Some(ds) = pair["cut"].get("differences") {
                for d in ds.as_array().unwrap() {
                    let legacy =
                        serde_json::json!({"field":d["field"],"from":d["left"],"to":d["right"]});
                    check(&legacy, &Ty::CutDifference, "compare Cut difference")?;
                    complete(&legacy, &Ty::CutDifference, "compare Cut difference")?;
                }
            }
        }
    }
    if checked.contains(&serde_json::json!("property_tables")) {
        let mut inventory = Vec::new();
        for plan in plans {
            let mut rows = Vec::new();
            for p in plan["report"]["properties"].as_array().unwrap() {
                if let Some(cs) = p.get("cases") {
                    for c in cs.as_array().unwrap() {
                        rows.push(serde_json::json!([p["id"], c["case_id"], c["arrival"]]));
                    }
                } else {
                    rows.push(serde_json::json!([
                        p["id"],
                        if p["id"] == "safety no_resource_overcommit"
                            || (v["analysis"] == "simulation" && p["id"] == "state_in_range")
                            || conflict_property_id(&p["id"])
                            || static_wire_property(&plan["report"], &p["id"])
                        {
                            Value::Null
                        } else {
                            serde_json::json!(1)
                        },
                        if p["id"] == "safety no_resource_overcommit"
                            || (v["analysis"] == "simulation" && p["id"] == "state_in_range")
                            || conflict_property_id(&p["id"])
                            || static_wire_property(&plan["report"], &p["id"])
                        {
                            Value::Null
                        } else {
                            serde_json::json!(0)
                        }
                    ]));
                }
            }
            if !inventory.is_empty() && inventory != rows {
                return Err("raw property inventory disagreement".into());
            }
            inventory = rows;
        }
        let table: Vec<_> = properties
            .iter()
            .map(|p| serde_json::json!([p["id"], p["case_id"], p["arrival"]]))
            .collect();
        if table != inventory {
            return Err("property table inventory/order".into());
        }
    }
    if checked.contains(&serde_json::json!("terminal_tables")) && !plans.is_empty() {
        let cases: Vec<_> = plans[0]["report"]
            .get("case_outcomes")
            .map(|c| {
                c.as_array()
                    .unwrap()
                    .iter()
                    .map(|c| (c["case_id"].clone(), c["arrival"].clone()))
                    .collect()
            })
            .unwrap_or_else(|| vec![(serde_json::json!(1), serde_json::json!(0))]);
        let ends: std::collections::BTreeSet<_> = terminals
            .iter()
            .filter(|r| r["kind"] == "end" || r["kind"] == "overflow")
            .map(|r| r["end"].as_str().unwrap())
            .collect();
        // ADR0042's selected horizon adds a lexical CutOff key, alongside the
        // existing Rejected key. Do not infer CutOff from observations. End
        // inventory is additionally checked against selected-process metrics
        // before metric stripping; without metrics this wire has no complete
        // selected-process lexical inventory (quality/Agency are model-wide).
        let cutoff = !plans[0]["report"]["scenario"]["horizon"].is_null();
        if plans
            .iter()
            .any(|p| !p["report"]["scenario"]["horizon"].is_null() != cutoff)
        {
            return Err("terminal table horizon inventory disagreement".into());
        }
        if terminals.len() != cases.len() * (ends.len() + 1 + usize::from(cutoff)) {
            return Err("terminal table rectangular inventory".into());
        }
        for (case, arrival) in cases {
            for kind in std::iter::once("rejected").chain(cutoff.then_some("cut_off")) {
                if !terminals.iter().any(|r| {
                    r["case_id"] == case
                        && r["arrival"] == arrival
                        && r["kind"] == kind
                        && r["end"].is_null()
                }) {
                    return Err("missing terminal/case kind row".into());
                }
            }
            for end in ends.iter().map(|e| serde_json::json!(e)) {
                if !terminals
                    .iter()
                    .any(|r| r["case_id"] == case && r["arrival"] == arrival && r["end"] == end)
                {
                    return Err("missing terminal/case row".into());
                }
            }
        }
    }
    for row in properties.iter().chain(terminals) {
        let cells = row["values"].as_array().unwrap();
        if cells
            .iter()
            .map(|c| c["configuration"].as_str().unwrap())
            .collect::<Vec<_>>()
            != names
        {
            return Err("table plan order".into());
        }
        for (cell, plan) in cells.iter().zip(plans) {
            let raw = &plan["report"];
            if row.get("id").is_some() {
                let prop = raw["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["id"] == row["id"])
                    .ok_or("missing raw property")?;
                let source = if row["case_id"].is_null() || prop.get("cases").is_none() {
                    prop
                } else {
                    prop["cases"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|c| c["case_id"] == row["case_id"] && c["arrival"] == row["arrival"])
                        .ok_or("property case alignment")?
                };
                for field in [
                    "verdict",
                    "probability",
                    "basis",
                    "hits",
                    "unknown",
                    "trials",
                    "interval",
                    "witness",
                ] {
                    if cell[field] != source.get(field).cloned().unwrap_or(Value::Null) {
                        return Err(format!(
                            "table property field {field} differs from raw case"
                        ));
                    }
                }
            } else {
                validate_terminal_identity(row)?;
                if row["kind"] == "overflow"
                    && row["end"].as_str()
                        != limits_declaration(
                            raw["assumptions"].as_array().ok_or("raw assumptions")?,
                        )?
                        .overflow
                        .as_deref()
                {
                    return Err(
                        "comparison overflow terminal is not declared by the raw plan".into(),
                    );
                }
                let source = if let Some(cs) = raw.get("case_outcomes") {
                    cs.as_array()
                        .unwrap()
                        .iter()
                        .find(|c| c["case_id"] == row["case_id"] && c["arrival"] == row["arrival"])
                        .ok_or("terminal case alignment")?["terminals"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|t| t["kind"] == row["kind"] && t["end"] == row["end"])
                } else {
                    if row["case_id"] != 1 || row["arrival"] != 0 {
                        return Err("legacy normalization".into());
                    }
                    raw["terminals"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|t| row["kind"] == "end" && t["end"] == row["end"])
                };
                if sim {
                    let count = if row["kind"] == "cut_off" {
                        raw["case_outcomes"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|c| {
                                c["case_id"] == row["case_id"] && c["arrival"] == row["arrival"]
                            })
                            .ok_or("cutoff terminal case alignment")?["cut_off_count"]
                            .clone()
                    } else {
                        source
                            .map(|s| s["count"].clone())
                            .unwrap_or(serde_json::json!(0))
                    };
                    if cell["status"] != "reported" || cell["value"]["count"] != count {
                        return Err("simulation terminal count".into());
                    }
                    check(&cell["value"], &Ty::Obj(vec![("count", Ty::Int)]), "count")?;
                } else if let Some(source) = source {
                    if cell["status"] != "reported" {
                        return Err("lost reported terminal".into());
                    }
                    for f in [
                        "completed_at",
                        "elapsed",
                        "work",
                        "result_at",
                        "probability",
                    ] {
                        let expected = if f == "completed_at" && source.get(f).is_none() {
                            source["elapsed"].clone()
                        } else {
                            source.get(f).cloned().unwrap_or(Value::Null)
                        };
                        if cell["value"][f] != expected {
                            return Err(format!("terminal field {f}"));
                        }
                    }
                    check(
                        &cell["value"],
                        &exhaustive_value_schema(),
                        "exhaustive value",
                    )?;
                } else if cell["status"] != "not_reported" || !cell["value"].is_null() {
                    return Err("fabricated exhaustive zero".into());
                }
            }
        }
    }
    Ok(())
}

fn validate_terminal_identity(t: &Value) -> Result<(), String> {
    if ((t["kind"] == "end" || t["kind"] == "overflow") && t["end"].is_string())
        || (t["kind"] == "rejected" && t["end"].is_null())
        || (t["kind"] == "cut_off" && t["end"].is_null())
    {
        Ok(())
    } else {
        Err("terminal kind/end mismatch".into())
    }
}

fn validate_diff_cases(v: &Value) -> Result<(), String> {
    let Some(cases) = v.get("case_outcomes") else {
        if v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.get("cases").is_some())
        {
            return Err("legacy diff has cases".into());
        }
        return Ok(());
    };
    if v["from"]["report"].get("case_outcomes").is_none()
        || v["to"]["report"].get("case_outcomes").is_none()
    {
        return Err("legacy diff gained new fields".into());
    }
    if v["terminals"] != serde_json::json!([]) {
        return Err("new diff used top terminals".into());
    }
    unique_composite(cases.as_array().unwrap(), &["case_id"])?;
    let raw = [&v["from"]["report"], &v["to"]["report"]];
    let calculated =
        v["mismatch"].is_null() && raw.iter().all(|r| !r["bounds"]["truncated"].is_null());
    if calculated {
        let expected: Vec<_> = raw[0]["case_outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| serde_json::json!([c["case_id"], c["arrival"]]))
            .collect();
        let actual: Vec<_> = cases
            .as_array()
            .unwrap()
            .iter()
            .map(|c| serde_json::json!([c["case_id"], c["arrival"]]))
            .collect();
        if actual != expected {
            return Err("diff case inventory/order".into());
        }
    } else if !cases.as_array().unwrap().is_empty() {
        return Err("diff published unavailable cases".into());
    }
    for c in cases.as_array().unwrap() {
        unique_composite(c["terminals"].as_array().unwrap(), &["kind", "end"])?;
        let mut inventory = std::collections::BTreeSet::new();
        for r in raw {
            let rc = r["case_outcomes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["case_id"] == c["case_id"] && x["arrival"] == c["arrival"])
                .ok_or("diff raw case mismatch")?;
            for t in rc["terminals"].as_array().unwrap() {
                inventory.insert(
                    serde_json::to_string(&serde_json::json!([t["kind"], t["end"]])).unwrap(),
                );
            }
        }
        let actual: std::collections::BTreeSet<_> = c["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| serde_json::to_string(&serde_json::json!([t["kind"], t["end"]])).unwrap())
            .collect();
        if actual != inventory {
            return Err("diff terminal union".into());
        }
        for t in c["terminals"].as_array().unwrap() {
            validate_terminal_identity(t)?;
            for (i, side) in ["from", "to"].into_iter().enumerate() {
                let rc = raw[i]["case_outcomes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|x| x["case_id"] == c["case_id"])
                    .unwrap();
                let rt = rc["terminals"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|x| x["kind"] == t["kind"] && x["end"] == t["end"]);
                if let Some(rt) = rt {
                    for f in [
                        "completed_at",
                        "elapsed",
                        "work",
                        "result_at",
                        "probability",
                    ] {
                        if t[side].get(f) != rt.get(f) {
                            return Err(format!("diff {side} {f} changed"));
                        }
                    }
                } else if !t[side].is_null() {
                    return Err("diff invented absent side".into());
                }
            }
        }
    }
    for p in v["properties"].as_array().unwrap() {
        if p["id"] == "safety no_resource_overcommit"
            || conflict_property_id(&p["id"])
            || raw.iter().all(|r| static_wire_property(r, &p["id"]))
        {
            if p.get("cases").is_some() {
                return Err("global safety gained fake case rows".into());
            }
        } else if let Some(cs) = p.get("cases") {
            unique_composite(cs.as_array().unwrap(), &["case_id"])?;
            for (i, side) in ["from", "to"].into_iter().enumerate() {
                let rp = raw[i]["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|x| x["id"] == p["id"])
                    .ok_or("missing diff property")?;
                let rows = rp["cases"].as_array().ok_or("raw cases missing")?;
                if rows.len() != cs.as_array().unwrap().len() {
                    return Err("diff property case count".into());
                }
                for (cell, r) in cs.as_array().unwrap().iter().zip(rows) {
                    if cell["case_id"] != r["case_id"] || cell["arrival"] != r["arrival"] {
                        return Err("diff property alignment".into());
                    }
                    for f in ["verdict", "probability"] {
                        if cell[side].get(f) != r.get(f) {
                            return Err("diff property value".into());
                        }
                    }
                }
            }
        } else {
            return Err("new diff missing property cases".into());
        }
    }
    Ok(())
}

/// Wire types of the report, as partial-expectation schema.
#[derive(Clone)]
enum Ty {
    Ledger,
    Uncertainty,
    Str,
    Int,
    Bool,
    Verdict,
    Code,
    Severity,
    Lit(&'static [&'static str]),
    Null(Box<Ty>),
    Arr(Box<Ty>),
    /// An array matched as a set by this key: each item needs it, uniquely.
    Keyed(&'static str, Box<Ty>),
    Pairs(Box<Ty>),
    Obj(Vec<(&'static str, Ty)>),
    Diagnostic,
    OneOf(Vec<Ty>),
    CutDifference,
    /// Not required by complete(), but when present it is never null.
    OptionalField(Box<Ty>),
    /// A complete canonical, ordered two-endpoint rational range, even in
    /// partial expectations. Deliberately independent of production numeric code.
    Probability,
    RationalText,
    SimulationInterval,
    MetricNumber,
}

fn diff_subject_schema() -> Ty {
    Ty::Obj(vec![
        ("kind", Ty::Lit(&["person", "actor"])),
        ("name", Ty::Str),
    ])
}

fn diff_authority_schema() -> Ty {
    use Ty::*;
    let strings = || Arr(Box::new(Str));
    Null(Box::new(Obj(vec![
        ("permission", Null(Box::new(strings()))),
        (
            "grants",
            Keyed(
                "authority",
                Box::new(Obj(vec![
                    ("authority", Str),
                    ("persons", strings()),
                    ("basis", Str),
                    ("issued_by", Null(Box::new(Str))),
                    ("not_by", strings()),
                    ("scope", Null(Box::new(Str))),
                    ("expires", Null(Box::new(Int))),
                    ("effective_expires", Null(Box::new(Int))),
                    ("revocable_by", Null(Box::new(diff_subject_schema()))),
                    ("delegated_from", Null(Box::new(Str))),
                ])),
            ),
        ),
        (
            "authority_basis",
            Keyed("id", Box::new(Obj(vec![("id", Str), ("authority", Str)]))),
        ),
        (
            "responsibilities",
            Arr(Box::new(Obj(vec![
                ("id", Int),
                ("person", Str),
                (
                    "target",
                    Obj(vec![
                        (
                            "kind",
                            Lit(&["capability", "state_condition", "maintenance"]),
                        ),
                        ("value", Str),
                    ]),
                ),
                ("within", Null(Box::new(Int))),
            ]))),
        ),
    ])))
}

fn diff_annotation_schema() -> Ty {
    use Ty::*;
    let refs = || Arr(Box::new(Obj(vec![("id", Int), ("person", Str)])));
    OneOf(vec![
        Obj(vec![("kind", Lit(&["keep_fallback"]))]),
        Obj(vec![
            ("kind", Lit(&["preserve"])),
            ("selector", Str),
            ("global", Bool),
            ("properties", Arr(Box::new(Str))),
        ]),
        Obj(vec![
            ("kind", Lit(&["override"])),
            ("subject_kind", Lit(&["person", "actor"])),
            ("subject", Str),
            ("verdict", Verdict),
            ("reason", Null(Box::new(Str))),
            (
                "unchecked",
                Arr(Box::new(Lit(&["override_authority_validity"]))),
            ),
            (
                "effects",
                Arr(Box::new(Obj(vec![
                    ("effect", Str),
                    ("call", Str),
                    ("judgements", Arr(Box::new(Str))),
                    (
                        "stops",
                        Lit(&["all_paths", "conditional", "none", "unknown"]),
                    ),
                    ("bypasses", Null(Box::new(Int))),
                ]))),
            ),
        ]),
        Obj(vec![
            ("kind", Lit(&["maintain"])),
            ("person", Str),
            ("reason", Null(Box::new(Str))),
            (
                "unlinked_reason",
                Null(Box::new(Lit(&["no_actor_derived"]))),
            ),
            (
                "links",
                Keyed(
                    "actor",
                    Box::new(Obj(vec![
                        ("actor", Str),
                        ("from_duties", refs()),
                        ("to_duties", refs()),
                        ("status", Lit(&["linked", "unlinked", "conflict"])),
                    ])),
                ),
            ),
        ]),
    ])
}

fn preservation_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        (
            "properties",
            Keyed(
                "id",
                Box::new(Obj(vec![("id", Str), ("from", Verdict), ("to", Verdict)])),
            ),
        ),
        (
            "summary",
            Obj(vec![
                ("satisfied", Int),
                ("vacuous", Int),
                ("violated", Int),
                ("inconclusive", Int),
                ("unsupported", Int),
                ("invalid", Int),
            ]),
        ),
    ])
}

fn diff_row_schema() -> Ty {
    use Ty::*;
    let strings = || Arr(Box::new(Str));
    let value = |kind| match kind {
        "meaning" => Bool,
        "authority" => diff_authority_schema(),
        "resource" => Null(Box::new(Obj(vec![
            ("capacity", Null(Box::new(Int))),
            ("executors", Null(Box::new(strings()))),
        ]))),
        "time" => Arr(Box::new(Lit(&["sequential", "fork"]))),
        "visibility" => strings(),
        "rule" => rule_diff_cell_schema(),
        "contest" => Keyed(
            "id",
            Box::new(Obj(vec![
                ("id", Str),
                ("party", Str),
                ("route", Str),
                ("reversible_by", strings()),
                ("respond_by", Int),
            ])),
        ),
        _ => unreachable!(),
    };
    OneOf(
        [
            "actor",
            "relation",
            "meaning",
            "authority",
            "resource",
            "time",
            "visibility",
            "contest",
            "rule",
        ]
        .into_iter()
        .map(|kind| {
            // Each discriminator has its own closed shape: from on an old row
            // cannot be admitted through a permissive shared base object.
            let lit: &'static [&'static str] = match kind {
                "actor" => &["actor"],
                "relation" => &["relation"],
                "meaning" => &["meaning"],
                "authority" => &["authority"],
                "resource" => &["resource"],
                "time" => &["time"],
                "visibility" => &["visibility"],
                "contest" => &["contest"],
                "rule" => &["rule"],
                _ => unreachable!(),
            };
            let mut fields = vec![
                ("kind", Lit(lit)),
                ("target", Str),
                (
                    "change",
                    if ["actor", "relation", "meaning"].contains(&kind) {
                        Lit(&["added", "removed"])
                    } else if ["time", "visibility", "rule"].contains(&kind) {
                        Lit(&["modified"])
                    } else {
                        Lit(&["added", "removed", "modified"])
                    },
                ),
                ("reason", Null(Box::new(Str))),
                ("loses", Null(Box::new(Str))),
                (
                    "annotations",
                    OptionalField(Box::new(Arr(Box::new(diff_annotation_schema())))),
                ),
            ];
            if !["actor", "relation"].contains(&kind) {
                fields.extend([("from", value(kind)), ("to", value(kind))]);
            }
            Obj(fields)
        })
        // ADR 0051 adds a distinct closed relation form. Never make the old
        // added/removed capability form accept optional producer payloads.
        .chain(std::iter::once(Obj(vec![
            ("kind", Lit(&["relation"])),
            ("target", Str),
            ("change", Lit(&["modified"])),
            ("reason", Null(Box::new(Str))),
            ("loses", Null(Box::new(Str))),
            ("from", Obj(vec![("producer", Str)])),
            ("to", Obj(vec![("producer", Str)])),
            (
                "annotations",
                OptionalField(Box::new(Arr(Box::new(diff_annotation_schema())))),
            ),
        ])))
        .collect(),
    )
}

/// Semantic checks are independent of production Serde and partial fixture
/// comparison. Reached by both the ordinary and metric-recursive diff paths.
fn validate_diff_extensions(v: &Value) -> Result<(), String> {
    if let Some(d) = v.pointer("/agency/differences") {
        validate_agency_differences(d)?;
        validate_capacity_difference_context(
            d,
            &v["from"]["report"]["actors"],
            &v["to"]["report"]["actors"],
        )?;
        for row in d["rows"].as_array().into_iter().flatten() {
            if row["status"] == "changed"
                && ((row["viewpoint"] == "effects" && row["item"] == "u")
                    || (row["viewpoint"] == "refusal" && row["item"] == "judgements"))
            {
                for side in ["from", "to"] {
                    for item in row[side]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|j| j.get("kind").is_some())
                    {
                        validate_rule_item_actor(item, &v[side]["report"]["actors"])?;
                    }
                }
            }
        }
    }
    use std::collections::BTreeSet;
    let fail = |s: &str| Err(format!("diff extensions wire: {s}"));
    let unique = |xs: &Value| -> bool {
        xs.as_array()
            .is_some_and(|a| a.windows(2).all(|w| w[0].as_str() < w[1].as_str()))
    };
    let kinds = [
        "actor",
        "relation",
        "meaning",
        "authority",
        "resource",
        "time",
        "visibility",
        "contest",
        "rule",
    ];
    let producer_row = |row: &Value| row["kind"] == "relation" && row["change"] == "modified";
    let limited_words = ["relation_state_producers", "relation_binding_identity"];
    let unchecked = v["unchecked"].as_array().unwrap();
    // ADR0051: a completed static comparison must disclose both limitations,
    // including zero-producer-row comparisons. Presence-only checking would
    // accept a regression that silently drops both words.
    let completed = ["relation", "annotations"]
        .iter()
        .all(|w| v["checked"].as_array().unwrap().iter().any(|s| s == w));
    if completed
        || v["differences"]
            .as_array()
            .unwrap()
            .iter()
            .any(producer_row)
        || unchecked
            .iter()
            .any(|s| limited_words.iter().any(|w| s == w))
    {
        let tail: Vec<Value> = limited_words.iter().map(|w| Value::from(*w)).collect();
        if !unchecked.ends_with(&tail)
            || limited_words
                .iter()
                .any(|w| unchecked.iter().filter(|s| *s == w).count() != 1)
            || !["relation", "annotations"]
                .iter()
                .all(|w| v["checked"].as_array().unwrap().iter().any(|s| s == w))
        {
            return fail("producer limitations require a completed diff and exact word suffix");
        }
    }
    let mut previous = None;
    for row in v["differences"].as_array().unwrap() {
        let kind = row["kind"].as_str().unwrap();
        let producer = producer_row(row);
        let rank = if producer {
            kinds.len()
        } else {
            kinds.iter().position(|k| *k == kind).unwrap()
        };
        if producer && !valid_plan_name(row["target"].as_str().unwrap()) {
            return fail("producer relation requires a bare binding target");
        }
        let phase = if rank < 2 {
            usize::from(row["change"] == "removed")
        } else {
            0
        };
        let order = (rank, phase, row["target"].as_str().unwrap());
        if previous.is_some_and(|p| p >= order) {
            return fail("row ordering/identity");
        }
        previous = Some(order);
        if rank >= 2 {
            let a = &row["from"];
            let b = &row["to"];
            if a == b {
                return fail("equal from/to");
            }
            let absent = |x: &Value| match kind {
                "meaning" => *x == false,
                "contest" => x.as_array().unwrap().is_empty(),
                _ => x.is_null(),
            };
            let expected = if absent(a) {
                "added"
            } else if absent(b) {
                "removed"
            } else {
                "modified"
            };
            if row["change"] != expected {
                return fail("change/from/to disagreement");
            }
            for cell in [a, b] {
                match kind {
                    "relation" => {
                        let parts: Vec<_> = cell["producer"].as_str().unwrap().split('.').collect();
                        if parts.len() != 2 || !parts.iter().all(|p| valid_plan_name(p)) {
                            return fail("producer requires Actor.Capability");
                        }
                    }
                    "time" => {
                        if cell.as_array().unwrap().is_empty()
                            || (cell != &serde_json::json!(["sequential"])
                                && cell != &serde_json::json!(["fork"])
                                && cell != &serde_json::json!(["sequential", "fork"]))
                        {
                            return fail("empty/duplicate/unsorted modes");
                        }
                    }
                    "visibility" if !unique(cell) => return fail("reader set ordering/duplicates"),
                    "resource"
                        if !cell.is_null()
                            && !cell["executors"].is_null()
                            && !unique(&cell["executors"]) =>
                    {
                        return fail("executor set ordering/duplicates");
                    }
                    "contest" => {
                        let records = cell.as_array().unwrap();
                        if records
                            .windows(2)
                            .any(|w| w[0]["id"].as_str() >= w[1]["id"].as_str())
                            || records.iter().any(|r| !unique(&r["reversible_by"]))
                        {
                            return fail("contest record/people ordering");
                        }
                    }
                    "rule" => {
                        validate_rule_diff_cell_wire(cell)?;
                        if row["to"]["amendment"].is_null()
                            || row["to"]["amendment"]["from"] != row["from"]["version"]
                            || row["to"]["amendment"]["to"] != row["to"]["version"]
                        {
                            return fail("rule row has no direction-matching amendment");
                        }
                    }
                    "authority" if !cell.is_null() => {
                        for (field, key) in [("grants", "authority"), ("authority_basis", "id")] {
                            if cell[field]
                                .as_array()
                                .unwrap()
                                .windows(2)
                                .any(|w| w[0][key].as_str() >= w[1][key].as_str())
                            {
                                return fail("authority set ordering/duplicates");
                            }
                        }
                        if cell["responsibilities"]
                            .as_array()
                            .unwrap()
                            .windows(2)
                            .any(|w| w[0]["id"].as_u64() >= w[1]["id"].as_u64())
                        {
                            return fail("authority duty ordering/duplicates");
                        }
                        // Permission: null (anonymous, no permit) or a
                        // nonempty sorted unique person set, never [].
                        if !cell["permission"].is_null()
                            && (cell["permission"].as_array().unwrap().is_empty()
                                || !unique(&cell["permission"]))
                        {
                            return fail("permission set ordering/duplicates/empty");
                        }
                        if cell["permission"].is_null()
                            && ["grants", "authority_basis", "responsibilities"]
                                .iter()
                                .all(|k| cell[k].as_array().unwrap().is_empty())
                        {
                            return fail("empty authority must be null");
                        }
                        for g in cell["grants"].as_array().unwrap() {
                            if !unique(&g["persons"]) || !unique(&g["not_by"]) {
                                return fail("grant people set");
                            }
                        }
                        for d in cell["responsibilities"].as_array().unwrap() {
                            if d["id"] == 0 {
                                return fail("zero duty id");
                            }
                        }
                        for basis in cell["authority_basis"].as_array().unwrap() {
                            if !["from", "to"].iter().any(|side| {
                                v[side]["report"]["properties"]
                                    .as_array()
                                    .is_some_and(|ps| ps.iter().any(|p| p["id"] == basis["id"]))
                            }) {
                                return fail("unknown authority basis property");
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut seen = BTreeSet::new();
        let mut last_annotation = 0;
        let mut last_preserve = None;
        if row
            .get("annotations")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            return fail("empty annotations field");
        }
        for a in row
            .get("annotations")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let tag = a["kind"].as_str().unwrap();
            if producer && tag != "preserve" {
                return fail("producer relation supports preserve annotations only");
            }
            let index = ["preserve", "override", "maintain", "keep_fallback"]
                .iter()
                .position(|k| *k == tag)
                .unwrap();
            if index < last_annotation {
                return fail("annotation ordering");
            }
            last_annotation = index;
            let identity = if tag == "preserve" {
                format!("preserve:{}:{}", a["selector"], a["global"])
            } else {
                tag.into()
            };
            if !seen.insert(identity) {
                return fail("duplicate annotation");
            }
            match tag {
                "keep_fallback" => {
                    if row["kind"] != "relation" || row["change"] != "removed" {
                        return fail("keep_fallback requires removed Capability relation");
                    }
                }
                "preserve" => {
                    let order = (
                        !a["global"].as_bool().unwrap(),
                        a["selector"].as_str().unwrap(),
                    );
                    if last_preserve.is_some_and(|p| p >= order) {
                        return fail("preserve declaration ordering/duplicates");
                    }
                    last_preserve = Some(order);
                    let ps = v
                        .get("preservation")
                        .ok_or("preserve without preservation")?;
                    let mut ids = BTreeSet::new();
                    let mut last_property = None;
                    for id in a["properties"].as_array().unwrap() {
                        if !ids.insert(id.to_string())
                            || !ps["properties"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|p| p["id"] == *id)
                        {
                            return fail("unknown/duplicate preserved property");
                        }
                        let index = ps["properties"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .position(|p| p["id"] == *id)
                            .unwrap();
                        if last_property.is_some_and(|p| p >= index) {
                            return fail("preserve property inventory order");
                        }
                        last_property = Some(index);
                    }
                }
                "override" => {
                    if a["unchecked"] != serde_json::json!(["override_authority_validity"]) {
                        return fail("override authority limit missing");
                    }
                    let mut calls = BTreeSet::new();
                    let mut last_effect = None;
                    for effect in a["effects"].as_array().unwrap() {
                        let order = (
                            effect["effect"].as_str().unwrap(),
                            effect["call"].as_str().unwrap(),
                        );
                        if last_effect.is_some_and(|p| p >= order) || !unique(&effect["judgements"])
                        {
                            return fail("effect/judgement ordering");
                        }
                        last_effect = Some(order);
                        if !calls.insert((effect["effect"].to_string(), effect["call"].to_string()))
                        {
                            return fail("duplicate effect call");
                        }
                        if effect["stops"] == "unknown"
                            && (!effect["bypasses"].is_null()
                                || !effect["judgements"].as_array().unwrap().is_empty())
                        {
                            return fail("invented unknown witness");
                        }
                        if a["verdict"] == "satisfied"
                            && (effect["stops"] != "all_paths"
                                || effect["bypasses"] != 0
                                || effect["judgements"].as_array().unwrap().is_empty())
                        {
                            return fail("satisfied override without U witness");
                        }
                    }
                    if a["verdict"] == "satisfied" && calls.is_empty() {
                        return fail("empty override success");
                    }
                    let effects = a["effects"].as_array().unwrap();
                    // vacuous means no effect at all (a removed difference).
                    if a["verdict"] == "vacuous" && !effects.is_empty() {
                        return fail("vacuous override with effects");
                    }
                    // violated needs a witnessed failure, not only all_paths.
                    if a["verdict"] == "violated"
                        && !effects
                            .iter()
                            .any(|e| e["stops"] == "conditional" || e["stops"] == "none")
                    {
                        return fail("violated override without a failing effect");
                    }
                    // The override verdict is folded into the diff root. The
                    // precedence table is written here, not imported.
                    let rank = |x: &Value| {
                        [
                            "satisfied",
                            "vacuous",
                            "inconclusive",
                            "unsupported",
                            "violated",
                            "invalid",
                        ]
                        .iter()
                        .position(|k| x == *k)
                        .unwrap()
                    };
                    if rank(&v["verdict"]) < rank(&a["verdict"]) {
                        return fail("override verdict not folded into the root");
                    }
                }
                "maintain" => {
                    let links = a["links"].as_array().unwrap();
                    if links
                        .windows(2)
                        .any(|w| w[0]["actor"].as_str() >= w[1]["actor"].as_str())
                    {
                        return fail("maintenance Actor ordering");
                    }
                    if links.is_empty() != (a["unlinked_reason"] == "no_actor_derived") {
                        return fail("missing no-actor reason");
                    }
                    for link in links {
                        for side in ["from", "to"] {
                            if link[format!("{side}_duties")]
                                .as_array()
                                .unwrap()
                                .windows(2)
                                .any(|w| w[0]["id"].as_u64() >= w[1]["id"].as_u64())
                            {
                                return fail("maintenance duty ordering/duplicates");
                            }
                            let mut ids = BTreeSet::new();
                            for d in link[format!("{side}_duties")].as_array().unwrap() {
                                let found = v[side]["report"]["responsibilities"]
                                    .as_array()
                                    .is_some_and(|rs| {
                                        rs.iter().any(|r| {
                                            r["id"] == d["id"]
                                                && r["person"] == d["person"]
                                                && r["target"]["kind"] == "maintenance"
                                                && r["target"]["value"]
                                                    == format!(
                                                        "{}.maintenance",
                                                        link["actor"].as_str().unwrap()
                                                    )
                                        })
                                    });
                                if !found || !ids.insert(d["id"].to_string()) {
                                    return fail("unknown/duplicate maintenance duty reference");
                                }
                            }
                        }
                        let ds = link["to_duties"].as_array().unwrap();
                        let status = if ds.iter().any(|d| d["person"] == a["person"]) {
                            "linked"
                        } else if ds.is_empty() {
                            "unlinked"
                        } else {
                            "conflict"
                        };
                        if link["status"] != status {
                            return fail("maintenance status/ref mismatch");
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }
    if let Some(p) = v.get("preservation") {
        let mut counts = serde_json::json!({"satisfied":0,"vacuous":0,"violated":0,"inconclusive":0,"unsupported":0,"invalid":0});
        let inventory = v["properties"].as_array().unwrap();
        let mut last = None;
        for row in p["properties"].as_array().unwrap() {
            let Some(i) = inventory.iter().position(|r| {
                r["id"] == row["id"] && r["from"] == row["from"] && r["to"] == row["to"]
            }) else {
                return fail("unknown/mismatched preservation property");
            };
            if last.is_some_and(|old| old >= i) {
                return fail("preservation inventory order");
            }
            last = Some(i);
            let key = row["to"].as_str().unwrap();
            counts[key] = Value::from(counts[key].as_u64().unwrap() + 1);
        }
        if counts != p["summary"] {
            return fail("preservation six buckets do not match unique To verdicts");
        }
    }
    Ok(())
}

fn diff_schema() -> Ty {
    use Ty::*;
    let range = || Obj(vec![("min", Int), ("max", Int)]);
    let metrics = || {
        Null(Box::new(Obj(vec![
            ("elapsed", range()),
            ("work", range()),
            ("result_at", Null(Box::new(range()))),
        ])))
    };
    let side = || Obj(vec![("configuration", Str), ("report", report_schema())]);
    let case_value = || {
        let Obj(mut fields) = exhaustive_value_schema() else {
            unreachable!()
        };
        fields.retain(|(k, _)| *k != "probability");
        fields.push(("probability", OptionalField(Box::new(Probability))));
        Null(Box::new(Obj(fields)))
    };
    Obj(vec![
        ("ledger", Ledger),
        ("uncertainty", Uncertainty),
        ("verdict", Verdict),
        (
            "quality",
            OptionalField(Box::new(quality_table_schema(false))),
        ),
        (
            "metrics",
            OptionalField(Box::new(metric_table_schema(false))),
        ),
        (
            "agency",
            Obj(vec![
                ("from", nullable_agency()),
                ("to", nullable_agency()),
                (
                    "differences",
                    OptionalField(Box::new(agency_differences_schema())),
                ),
            ]),
        ),
        ("from", side()),
        ("to", side()),
        (
            "mismatch",
            Null(Box::new(Obj(vec![
                ("from_cut", Str),
                ("to_cut", Str),
                ("differences", Keyed("field", Box::new(CutDifference))),
            ]))),
        ),
        ("differences", Pairs(Box::new(diff_row_schema()))),
        (
            "preservation",
            OptionalField(Box::new(preservation_schema())),
        ),
        (
            "terminals",
            Keyed(
                "end",
                Box::new(Obj(vec![
                    ("end", Str),
                    ("from", metrics()),
                    ("to", metrics()),
                ])),
            ),
        ),
        (
            "properties",
            Keyed(
                "id",
                Box::new(Obj(vec![
                    ("id", Str),
                    ("from", Verdict),
                    ("to", Verdict),
                    (
                        "cases",
                        OptionalField(Box::new(Arr(Box::new(Obj(vec![
                            ("case_id", Int),
                            ("arrival", Int),
                            ("from", property_value_schema()),
                            ("to", property_value_schema()),
                        ]))))),
                    ),
                ])),
            ),
        ),
        (
            "case_outcomes",
            OptionalField(Box::new(Arr(Box::new(Obj(vec![
                ("case_id", Int),
                ("arrival", Int),
                (
                    "terminals",
                    Arr(Box::new(Obj(vec![
                        ("kind", Lit(&["end", "rejected", "overflow", "cut_off"])),
                        ("end", Null(Box::new(Str))),
                        ("from", case_value()),
                        ("to", case_value()),
                    ]))),
                ),
            ]))))),
        ),
        ("diagnostics", Arr(Box::new(Diagnostic))),
        (
            "checked",
            Arr(Box::new(Lit(&[
                "comparison_mismatch",
                "actor",
                "relation",
                "intent_claims",
                "retire_loses",
                "meaning",
                "authority",
                "resource",
                "time",
                "visibility",
                "contest",
                "preserve",
                "override",
                "maintain",
                "annotations",
                "rule",
                "case_outcomes",
                "case_properties",
                "metric_tables",
            ]))),
        ),
        (
            "unchecked",
            Arr(Box::new(Lit(&[
                "time_intent",
                "rule",
                "resource",
                "meaning",
                "authority",
                "visibility",
                "contest",
                "responsibility",
                "preserve",
                "override",
                "maintain",
                "comparison_mismatch",
                "actor",
                "relation",
                "intent_claims",
                "retire_loses",
                "case_outcomes",
                "case_properties",
                "terminals",
                "metrics",
                "metric_tables",
                "metric_differences",
                "metric_ranking",
                "metric_semantic_alignment",
                "obligation_tables",
                "contest_runtime_tables",
                "agency_differences",
                "waiting_tables",
                "relation_state_producers",
                "relation_binding_identity",
            ]))),
        ),
        ("absent_diagnostics", Arr(Box::new(Code))),
    ])
}

fn diagnostic_severity(code: &str) -> Option<relca_core::diag::Severity> {
    if ["W-UNREACH-0", "W-UNREACH-1", "W-UNREACH-2", "W-UNREACH-3"].contains(&code) {
        return Some(relca_core::diag::Severity::Warning);
    }
    if [
        "W-LIN-2",
        "W-RESP-0",
        "W-RESP-1",
        "W-RESP-2",
        "W-CONTEST-0",
        "W-AUTH-0",
        "W-DIFF-3",
        "W-AGY-2",
        "W-AGY-3",
    ]
    .contains(&code)
    {
        Some(relca_core::diag::Severity::Warning)
    } else {
        relca_core::diag::severity_of(code)
    }
}

fn static_wire_property(v: &Value, id: &Value) -> bool {
    v.get("contest_paths")
        .and_then(Value::as_array)
        .is_some_and(|rows| rows.iter().any(|r| r["id"] == *id))
}

fn responsibility_time_schema() -> Ty {
    use Ty::*;
    OneOf(vec![
        Obj(vec![("status", Lit(&["unchecked"]))]),
        Obj(vec![
            ("status", Lit(&["satisfied", "violated", "unchecked"])),
            ("within", Int),
            ("required_duration", Null(Box::new(Int))),
            ("basis", Lit(&["static_capability_path_upper"])),
            ("scope", Lit(&["static_responsibility_reachability"])),
            (
                "reason",
                Null(Box::new(Lit(&[
                    "observation_missing",
                    "observation_not_derived",
                    "refusal_missing",
                    "refusal_conditional",
                    "refusal_not_derived",
                    "no_reachability_path",
                    "path_not_reconstructed",
                    "duration_unknown",
                    "duration_overflow",
                    "derivation_limit",
                ]))),
            ),
            (
                "paths",
                Arr(Box::new(Obj(vec![
                    ("kind", Lit(&["observation", "refusal"])),
                    ("source", Null(Box::new(Str))),
                    ("judgement_call", Null(Box::new(Str))),
                    ("effect_call", Null(Box::new(Str))),
                    ("calls", Arr(Box::new(Str))),
                    ("duration_upper", Int),
                ]))),
            ),
        ]),
    ])
}

fn responsibility_schema() -> Ty {
    use Ty::*;
    let strings = || Arr(Box::new(Str));
    let reason = || {
        Null(Box::new(Lit(&[
            "target_not_called",
            "no_source_identified",
            "no_reachable_effect",
            "noncommit_state_write",
            "state_guard",
            "state_guards",
            "derivation_limit",
        ])))
    };
    Arr(Box::new(Obj(vec![
        ("id", Int),
        (
            "rules",
            Lit(&["responsibility-p1-1", "responsibility-p1-2"]),
        ),
        ("person", Str),
        (
            "target",
            Obj(vec![
                (
                    "kind",
                    Lit(&["capability", "state_condition", "maintenance"]),
                ),
                ("value", Str),
            ]),
        ),
        ("actors", strings()),
        (
            "observation",
            Obj(vec![
                ("status", Lit(&["present", "missing", "not_derived"])),
                ("required_sources", strings()),
                ("direct", strings()),
                ("indirect", strings()),
                ("missing_sources", strings()),
                ("reason", reason()),
            ]),
        ),
        (
            "refusal",
            Obj(vec![
                (
                    "status",
                    Lit(&["present", "missing", "conditional", "not_derived"]),
                ),
                (
                    "effects",
                    Arr(Box::new(Obj(vec![
                        ("capability", Str),
                        ("call", Str),
                        (
                            "status",
                            Lit(&[
                                "all_paths",
                                "conditional",
                                "none",
                                "not_applicable",
                                "not_derived",
                            ]),
                        ),
                        (
                            "judgements",
                            Arr(Box::new(Obj(vec![
                                ("actor", Str),
                                ("capability", Str),
                                ("call", Str),
                                ("stops", Lit(&["all_paths", "conditional", "none"])),
                                ("bypasses", Int),
                            ]))),
                        ),
                    ]))),
                ),
                ("reason", reason()),
            ]),
        ),
        ("time_frame", responsibility_time_schema()),
        ("warnings", Arr(Box::new(Lit(&["W-RESP-1", "W-RESP-2"])))),
    ])))
}

fn contest_schema() -> Ty {
    use Ty::*;
    let strings = || Arr(Box::new(Str));
    let check = || Obj(vec![("satisfied", Bool), ("reason", Null(Box::new(Str)))]);
    Arr(Box::new(Obj(vec![
        ("id", Str),
        ("rules", Lit(&["contest-p1-1", "contest-p1-2"])),
        ("party", Str),
        ("end", Null(Box::new(Str))),
        ("effect", OptionalField(Box::new(Str))),
        ("route", Str),
        ("reversible_by", strings()),
        ("respond_by", Int),
        (
            "checks",
            Obj(vec![
                ("route_exists", check()),
                (
                    "reversal_authority",
                    Obj(vec![
                        ("satisfied", Bool),
                        ("quantifier", Lit(&["exists_decision_per_person"])),
                        (
                            "people",
                            Arr(Box::new(Obj(vec![
                                ("person", Str),
                                ("satisfied", Bool),
                                (
                                    "decisions",
                                    Arr(Box::new(Obj(vec![
                                        ("capability", Str),
                                        ("executor", Bool),
                                        ("included", Bool),
                                        ("permission", Bool),
                                        ("authorities", strings()),
                                    ]))),
                                ),
                                (
                                    "reason",
                                    Null(Box::new(Lit(&[
                                        "Permission あり、Authority 未接続",
                                        "permission_missing",
                                        "executor_not_connected",
                                        "no_decision",
                                        "decision_excluded",
                                    ]))),
                                ),
                            ]))),
                        ),
                    ]),
                ),
                ("deadline_defined", check()),
            ]),
        ),
        ("verdict", Lit(&["satisfied", "violated"])),
        ("basis", Lit(&["構造上存在", "構造の不足"])),
        (
            "missing",
            Arr(Box::new(Lit(&[
                "route_exists",
                "reversal_authority",
                "deadline_defined",
            ]))),
        ),
    ])))
}

/// Independent closed validation: never relies on production DTO defaults,
/// prefix-based static-property recognition, or partial fixture expectations.
pub fn validate_responsibility_wire(v: &Value) -> Result<(), String> {
    let (Some(duties), Some(paths)) = (v.get("responsibilities"), v.get("contest_paths")) else {
        return if v.get("responsibilities").is_some() || v.get("contest_paths").is_some() {
            Err("both static tables must be present together".into())
        } else {
            Ok(())
        };
    };
    for (key, value, schema) in [
        ("responsibilities", duties, responsibility_schema()),
        ("contest_paths", paths, contest_schema()),
    ] {
        check(value, &schema, key)?;
        complete(value, &schema, key)?;
    }
    let unique = |rows: &[Value], key: &str| -> Result<(), String> {
        let mut seen = std::collections::BTreeSet::new();
        for r in rows {
            if !seen.insert(r[key].to_string()) {
                return Err(format!("duplicate {key}"));
            }
        }
        Ok(())
    };
    let unique_strings = |rows: &Value| -> Result<(), String> {
        let a = rows.as_array().ok_or("not string array")?;
        if a.iter()
            .map(Value::to_string)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != a.len()
        {
            Err("duplicate string".into())
        } else {
            Ok(())
        }
    };
    for (i, row) in duties.as_array().unwrap().iter().enumerate() {
        if row["id"] != json_value_u64(i as u64 + 1) {
            return Err("duty ids must be declaration-ordered, unique and one-based".into());
        }
        if row["person"].as_str() == Some("") || row["target"]["value"].as_str() == Some("") {
            return Err("empty responsibility identity".into());
        }
        let time = &row["time_frame"];
        if row["target"]["kind"] == "maintenance" && row["rules"] != "responsibility-p1-2" {
            return Err("maintenance requires the new responsibility rules".into());
        }
        if time.get("within").is_some() {
            if row["rules"] != "responsibility-p1-2" {
                return Err("explicit time requires new responsibility rules".into());
            }
            let within = time["within"]
                .as_u64()
                .ok_or("within is a nonnegative integer")?;
            let paths = time["paths"].as_array().ok_or("time paths")?;
            if time["status"] == "unchecked" {
                if !time["required_duration"].is_null()
                    || !paths.is_empty()
                    || !time["reason"].is_string()
                {
                    return Err("unchecked time has reason, no duration or witness".into());
                }
            } else {
                let required = time["required_duration"]
                    .as_u64()
                    .ok_or("derived duration required")?;
                if !time["reason"].is_null()
                    || paths.len() != 1
                    || (time["status"] == "satisfied") != (required <= within)
                {
                    return Err("time status/bound/reason/max witness disagreement".into());
                }
                let p = &paths[0];
                let calls = p["calls"].as_array().ok_or("path calls")?;
                unique_strings(&p["calls"])?;
                if calls.is_empty()
                    || calls.iter().any(|v| v.as_str().is_none_or(str::is_empty))
                    || p["duration_upper"] != required
                {
                    return Err(
                        "nonempty ordered max witness with consistent duration required".into(),
                    );
                }
                if p["kind"] == "observation" {
                    if p["source"].as_str().is_none_or(str::is_empty)
                        || !p["judgement_call"].is_null()
                        || !p["effect_call"].is_null()
                        || calls.len() != 1
                        || !row["observation"]["required_sources"]
                            .as_array()
                            .unwrap()
                            .contains(&p["source"])
                    {
                        return Err(
                            "observation witness is one direct reader of a required source".into(),
                        );
                    }
                } else if !p["source"].is_null()
                    || p["judgement_call"].as_str().is_none_or(str::is_empty)
                    || p["effect_call"].as_str().is_none_or(str::is_empty)
                    || calls.first() != Some(&p["judgement_call"])
                    || calls.last() != Some(&p["effect_call"])
                {
                    return Err("refusal witness includes both endpoints in order".into());
                } else if !row["refusal"]["effects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|effect| {
                        effect["call"] == p["effect_call"]
                            && effect["judgements"].as_array().unwrap().iter().any(|j| {
                                j["call"] == p["judgement_call"] && j["stops"] == "all_paths"
                            })
                    })
                {
                    return Err(
                        "refusal witness must identify a recorded stopping judgement/effect".into(),
                    );
                }
                if row["observation"]["status"] != "present"
                    || row["refusal"]["status"] != "present"
                {
                    return Err("derived time requires complete O and U evidence".into());
                }
            }
        }
        for k in ["actors", "warnings"] {
            unique_strings(&row[k])?;
        }
        for area in ["observation", "refusal"] {
            let x = &row[area];
            if (x["status"] == "not_derived") != x["reason"].is_string() {
                return Err("not_derived requires reason; derived requires null".into());
            }
        }
        let o = &row["observation"];
        for key in ["required_sources", "direct", "indirect", "missing_sources"] {
            unique_strings(&o[key])?;
        }
        if o["status"] != "not_derived" {
            let required = o["required_sources"].as_array().unwrap();
            let direct = o["direct"].as_array().unwrap();
            let missing: Vec<_> = required
                .iter()
                .filter(|s| !direct.contains(s))
                .cloned()
                .collect();
            if required.is_empty()
                || o["missing_sources"] != Value::Array(missing.clone())
                || (o["status"] == "missing") != (!missing.is_empty())
            {
                return Err("O subset/missing-source classification".into());
            }
        }
        let warnings = row["warnings"].as_array().unwrap();
        if warnings.contains(&Value::String("W-RESP-1".into())) != (o["status"] == "missing") {
            return Err("O warning mismatch".into());
        }
        if warnings.contains(&Value::String("W-RESP-2".into()))
            != ["missing", "conditional"]
                .iter()
                .any(|s| row["refusal"]["status"] == *s)
        {
            return Err("U warning mismatch".into());
        }
        let effects = row["refusal"]["effects"].as_array().unwrap();
        unique(effects, "call")?;
        for effect in effects {
            let judgements = effect["judgements"].as_array().unwrap();
            unique(judgements, "call")?;
            for j in judgements {
                if j["stops"] == "all_paths" && j["bypasses"] != 0 {
                    return Err("all_paths cannot have bypasses".into());
                }
            }
        }
    }
    let rows = paths.as_array().unwrap();
    unique(rows, "id")?;
    for row in rows {
        if row.get("effect").is_some() {
            if !row["end"].is_null()
                || !commit_target_name(&row["effect"])
                || row["rules"] != "contest-p1-2"
            {
                return Err(
                    "effect contest requires end:null, nonempty commit effect, new rules".into(),
                );
            }
        } else if row["end"].as_str().is_none_or(str::is_empty) || row["rules"] != "contest-p1-1" {
            return Err("terminal contest keeps end and old rules without an effect key".into());
        }
        for k in ["id", "party", "end", "route"] {
            if row[k].as_str() == Some("") {
                return Err("empty contest identity".into());
            }
        }
        unique_strings(&row["reversible_by"])?;
        let people = row["checks"]["reversal_authority"]["people"]
            .as_array()
            .unwrap();
        unique(people, "person")?;
        let who: Vec<_> = people.iter().map(|p| p["person"].clone()).collect();
        if who.is_empty() || Value::Array(who) != row["reversible_by"] {
            return Err("every reversible person needs an ordered result".into());
        }
        for p in people {
            let decisions = p["decisions"].as_array().unwrap();
            unique(decisions, "capability")?;
            let exists = decisions.iter().any(|d| {
                d["executor"] == true
                    && d["included"] == true
                    && d["permission"] == true
                    && !d["authorities"].as_array().unwrap().is_empty()
            });
            if p["satisfied"] != exists
                || (exists && !p["reason"].is_null())
                || (!exists && !p["reason"].is_string())
            {
                return Err("existential decision qualification/reason mismatch".into());
            }
        }
        if row["checks"]["reversal_authority"]["satisfied"]
            != people.iter().all(|p| p["satisfied"] == true)
        {
            return Err("all reversible people must qualify".into());
        }
        if row["checks"]["deadline_defined"]["satisfied"]
            != (row["respond_by"].as_u64().unwrap() > 0)
        {
            return Err("positive deadline required".into());
        }
        let missing: Vec<_> = ["route_exists", "reversal_authority", "deadline_defined"]
            .into_iter()
            .filter(|k| row["checks"][k]["satisfied"] == false)
            .map(|s| Value::String(s.into()))
            .collect();
        let satisfied = missing.is_empty();
        if row["missing"] != Value::Array(missing)
            || row["verdict"] != if satisfied { "satisfied" } else { "violated" }
            || row["basis"]
                != if satisfied {
                    "構造上存在"
                } else {
                    "構造の不足"
                }
        {
            return Err("contest checks/missing/verdict/basis disagree".into());
        }
        let props = v["properties"].as_array().ok_or("properties required")?;
        let matches: Vec<_> = props.iter().filter(|p| p["id"] == row["id"]).collect();
        if matches.len() != 1 || matches[0]["verdict"] != row["verdict"] {
            return Err("contest/property verdict or inventory disagreement".into());
        }
        let p = matches[0];
        if p.get("cases").is_some() || p.get("probability").is_some() {
            return Err("structural property is global and nonprobabilistic".into());
        }
        if v["mode"] == "simulation"
            && (p["basis"] != "structural" || p.as_object().unwrap().len() != 3)
        {
            return Err("structural simulation has only id/verdict/basis".into());
        }
    }
    // Static failure must not publish partial tables, even if empty.
    if v["agency"].is_null() {
        return Err("static tables on failed lowering".into());
    }
    Ok(())
}

fn json_value_u64(v: u64) -> Value {
    Value::Number(v.into())
}

#[cfg(test)]
mod responsibility_wire_tests {
    use super::*;
    use serde_json::json;

    fn control() -> Value {
        let mut c: Value =
            serde_json::from_str(include_str!("../../../fixtures/resp-contest/expected.json"))
                .unwrap();
        let d: Value =
            serde_json::from_str(include_str!("../../../fixtures/resp-origin/expected.json"))
                .unwrap();
        c["responsibilities"] = d["responsibilities"].clone();
        c["agency"] = json!({});
        c["mode"] = json!("exhaustive");
        c["diagnostics"] = json!([]);
        c
    }
    fn keys(v: &Value, path: &str, out: &mut Vec<(String, String)>) {
        if let Some(o) = v.as_object() {
            for (k, x) in o {
                out.push((path.into(), k.clone()));
                keys(x, &format!("{path}/{k}"), out);
            }
        }
        if let Some(a) = v.as_array() {
            for (i, x) in a.iter().enumerate() {
                keys(x, &format!("{path}/{i}"), out);
            }
        }
    }
    #[test]
    fn r_closed_static_tables_reject_each_missing_or_extra_nested_field() {
        let good = control();
        validate_responsibility_wire(&good).unwrap();
        let mut paths = Vec::new();
        for table in ["responsibilities", "contest_paths"] {
            keys(&good[table], &format!("/{table}"), &mut paths);
        }
        for (parent, key) in paths {
            let mut bad = good.clone();
            bad.pointer_mut(&parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(&key);
            assert!(
                validate_responsibility_wire(&bad).is_err(),
                "accepted missing {parent}/{key}"
            );
            let mut bad = good.clone();
            bad.pointer_mut(&parent).unwrap()["unknown"] = json!(true);
            assert!(
                validate_responsibility_wire(&bad).is_err(),
                "accepted unknown {parent}"
            );
        }
        for table in ["responsibilities", "contest_paths"] {
            let mut bad = good.clone();
            bad.as_object_mut().unwrap().remove(table);
            assert!(validate_responsibility_wire(&bad).is_err());
        }
    }
    #[test]
    fn r_static_wire_independent_mutations_and_comparator_inventory() {
        let good = control();
        for (path, value) in [
            ("/responsibilities/0/id", json!(0)),
            ("/responsibilities/0/target/kind", json!("norm")),
            (
                "/responsibilities/0/observation/status",
                json!("sufficient"),
            ),
            (
                "/responsibilities/0/observation/status",
                json!("not_derived"),
            ),
            ("/responsibilities/0/observation/direct", json!([])),
            ("/responsibilities/0/refusal/status", json!("missing")),
            ("/responsibilities/0/time_frame/status", json!("present")),
            ("/responsibilities/0/warnings", json!(["W-UNKNOWN-0"])),
            (
                "/responsibilities/0/refusal/effects/0/judgements/0/bypasses",
                json!(1),
            ),
            (
                "/contest_paths/0/checks/route_exists/satisfied",
                json!("true"),
            ),
            ("/contest_paths/0/respond_by", json!(-1)),
            ("/contest_paths/0/respond_by", json!(0)),
            ("/contest_paths/0/missing", json!(["route_exists"])),
            (
                "/contest_paths/0/checks/reversal_authority/quantifier",
                json!("all_decisions"),
            ),
            (
                "/contest_paths/0/checks/reversal_authority/people/0/decisions/0/authorities",
                json!([]),
            ),
            ("/contest_paths/0/reversible_by", json!(["a1", "a2"])),
            ("/contest_paths/0/basis", json!("当事者が受け入れた")),
            ("/properties/1/verdict", json!("violated")),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(path).unwrap() = value;
            assert!(
                validate_responsibility_wire(&bad).is_err(),
                "accepted {path}: {bad}"
            );
        }
        for table in ["responsibilities", "contest_paths"] {
            let mut bad = good.clone();
            let duplicate = bad[table][0].clone();
            bad[table].as_array_mut().unwrap().push(duplicate);
            assert!(validate_responsibility_wire(&bad).is_err(), "duplicate id");
            assert!(compare(&good, &bad, "$").is_err(), "extra table row");
        }
        let mut sim = good.clone();
        sim["mode"] = json!("simulation");
        sim["properties"][1]["basis"] = json!("structural");
        validate_responsibility_wire(&sim).unwrap();
        for key in ["witness", "hits", "unknown", "trials", "interval", "cases"] {
            let mut bad = sim.clone();
            bad["properties"][1][key] = json!(null);
            assert!(validate_responsibility_wire(&bad).is_err(), "fake {key}");
        }
        for code in ["W-RESP-0", "W-RESP-1", "W-RESP-2", "W-CONTEST-0"] {
            check(
                &json!({"code":code,"severity":"warning"}),
                &Ty::Diagnostic,
                "diag",
            )
            .unwrap();
            assert!(
                check(
                    &json!({"code":code,"severity":"error"}),
                    &Ty::Diagnostic,
                    "diag"
                )
                .is_err()
            );
        }
        assert!(check(&json!({"code":"W-RESP-3"}), &Ty::Diagnostic, "diag").is_err());
    }
    #[test]
    fn r_all_19_expected_schemas_are_authored_and_old_populations_unchanged() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut found: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| {
                e.file_type().unwrap().is_dir()
                    && e.file_name().to_string_lossy().starts_with("resp-")
            })
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        found.sort();
        assert_eq!(found, RESPONSIBILITY_FIXTURES);
        for name in found {
            let dir = root.join(name);
            let e: Value =
                serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).unwrap())
                    .unwrap();
            let inv = std::fs::read_to_string(dir.join("invocation.json"))
                .ok()
                .map(|s| serde_json::from_str(&s).unwrap());
            Invocation::parse(inv.as_ref())
                .unwrap()
                .validate_expected(&e)
                .unwrap();
        }
    }
}

fn effect_carrier_schema() -> Ty {
    use Ty::*;
    let strings = || Arr(Box::new(Str));
    Obj(vec![
        ("actors", strings()),
        ("people", strings()),
        ("unattributed_actors", strings()),
        ("permitted", strings()),
    ])
}
fn effect_judgement_schema() -> Ty {
    use Ty::*;
    let Obj(mut fields) = effect_carrier_schema() else {
        unreachable!()
    };
    fields.extend([
        ("call", Str),
        ("capability", Str),
        ("stops", Lit(&["all_paths", "conditional", "none"])),
        ("bypasses", Int),
    ]);
    Obj(fields)
}
fn effect_refuser_schema() -> Ty {
    Ty::OneOf(vec![effect_judgement_schema(), rule_judgement_schema()])
}

// Only the two explicit Rule-bearing list shapes call this helper. Process
// fields are still checked by their original closed schema, not by key prefixes.
fn validate_rule_items(items: &Value) -> Result<(), String> {
    let mut rules = std::collections::BTreeSet::new();
    let mut rule_seen = false;
    for item in items.as_array().ok_or("Rule union must be an array")? {
        if item.get("kind").is_some() {
            validate_rule_judgement_wire(item)?;
            rule_seen = true;
            if !rules.insert(item["actor"].as_str().unwrap()) {
                return Err("duplicate Rule item".into());
            }
        } else if rule_seen {
            return Err("process judgement after Rule items".into());
        }
    }
    Ok(())
}

fn validate_rule_item_actor(item: &Value, actors: &Value) -> Result<(), String> {
    let actor = actors
        .as_array()
        .and_then(|xs| xs.iter().find(|a| a["name"] == item["actor"]))
        .ok_or("Rule item Actor missing from report")?;
    let rule = actor
        .get("rule")
        .ok_or("Rule item requires declared Rule metadata")?;
    if rule["version"]["effective"] != item["version"] {
        return Err("Rule item effective version mismatch".into());
    }
    Ok(())
}
fn state_refusers_schema() -> Ty {
    use Ty::*;
    Arr(Box::new(Obj(vec![
        ("actor", Str),
        ("capability", Str),
        ("reads", Arr(Box::new(Str))),
        ("on_normal_path", OneOf(vec![Bool, Lit(&["unknown"])])),
    ])))
}
fn generation_roots_schema() -> Ty {
    use Ty::*;
    Arr(Box::new(Obj(vec![("call", Str), ("capability", Str)])))
}
fn effects_schema() -> Ty {
    use Ty::*;
    let unknown = || {
        Obj(vec![
            ("status", Lit(&["not_derived"])),
            ("reason", Lit(&["state_guards"])),
        ])
    };
    let concentration = |partial: bool| {
        let truth = || {
            if partial {
                OneOf(vec![Bool, Lit(&["unknown"])])
            } else {
                Bool
            }
        };
        Obj(vec![
            ("single_actor", truth()),
            ("single_person", OneOf(vec![Bool, Lit(&["unknown"])])),
            ("single_generation_root", truth()),
            (
                "generation_roots",
                if partial {
                    unknown()
                } else {
                    generation_roots_schema()
                },
            ),
            (
                "signal",
                Lit(&["判定・拒否・実行の担い手と生成根の集中の兆候"]),
            ),
            (
                "mechanism",
                Lit(&["同じ担い手又は生成根への依存が構成上重なる"]),
            ),
            ("check", Lit(&["モデル外の統制・原本参照・ログで確認する"])),
        ])
    };
    let row = |partial: bool| {
        let mut fields = vec![
            ("effect", Str),
            ("call", Str),
            ("status", Lit(&["derived"])),
            ("e", effect_carrier_schema()),
            ("state_refusers", state_refusers_schema()),
            (
                "process_judgements",
                if partial {
                    unknown()
                } else {
                    Lit(&["derived"])
                },
            ),
            ("concentration", concentration(partial)),
        ];
        if !partial {
            fields.extend([
                ("j", Arr(Box::new(effect_judgement_schema()))),
                ("u", Arr(Box::new(effect_refuser_schema()))),
            ]);
        }
        Obj(fields)
    };
    let unchecked = || {
        Arr(Box::new(Lit(&[
            "authority",
            "fallback_path",
            "state_write_effects",
        ])))
    };
    OneOf(vec![
        Obj(vec![
            ("status", Lit(&["derived"])),
            ("rows", Arr(Box::new(OneOf(vec![row(false), row(true)])))),
            ("unchecked", unchecked()),
        ]),
        Obj(vec![
            ("status", Lit(&["not_derived"])),
            ("reason", Lit(&["derivation_limit"])),
            ("unchecked", unchecked()),
        ]),
    ])
}
#[allow(dead_code)]
pub fn validate_effects(v: &Value) -> Result<(), String> {
    let schema = effects_schema();
    check(v, &schema, "effects")?;
    complete(v, &schema, "effects")?;
    if v["unchecked"] != serde_json::json!(["authority", "fallback_path"])
        && v["unchecked"]
            != serde_json::json!(["authority", "fallback_path", "state_write_effects"])
    {
        return Err("effects closed unchecked sequence".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for r in v["rows"].as_array().into_iter().flatten() {
        if !seen.insert((r["effect"].to_string(), r["call"].to_string())) {
            return Err("duplicate effect call".into());
        }
        for s in r["state_refusers"].as_array().unwrap() {
            if s["on_normal_path"] == true {
                return Err("normal-path true is not derived".into());
            }
        }
        if r["process_judgements"] == "derived" {
            validate_rule_items(&r["u"])?;
            let want: Vec<_> = r["j"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|j| j["stops"] != "none")
                .cloned()
                .collect();
            let process: Vec<_> = r["u"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|j| j.get("kind").is_none())
                .cloned()
                .collect();
            if process != want {
                return Err("process U must be ordered stopping subset of J".into());
            }
            let roots = r["concentration"]["generation_roots"].as_array().unwrap();
            if r["concentration"]["single_generation_root"] != (roots.len() == 1) {
                return Err("generation-root singleton flag".into());
            }
            let carriers = std::iter::once(&r["e"]).chain(r["j"].as_array().unwrap().iter());
            let mut people = std::collections::BTreeSet::new();
            let mut anonymous = false;
            for carrier in carriers {
                for key in ["actors", "people", "unattributed_actors", "permitted"] {
                    let xs = carrier[key].as_array().unwrap();
                    let unique: std::collections::BTreeSet<_> =
                        xs.iter().map(Value::to_string).collect();
                    if xs.len() != unique.len() {
                        return Err("duplicate carrier member".into());
                    }
                }
                people.extend(
                    carrier["people"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(Value::to_string),
                );
                anonymous |= !carrier["unattributed_actors"]
                    .as_array()
                    .unwrap()
                    .is_empty();
            }
            let person = if people.len() >= 2 {
                serde_json::json!(false)
            } else if anonymous {
                serde_json::json!("unknown")
            } else {
                serde_json::json!(people.len() == 1)
            };
            if r["concentration"]["single_person"] != person {
                return Err("known-two/anonymous singleton ordering".into());
            }
            if r["u"].as_array().unwrap().is_empty()
                && r["state_refusers"].as_array().unwrap().is_empty()
                && r["concentration"]["single_actor"] != false
            {
                return Err("single_actor requires process or state refusal".into());
            }
        } else if ["single_actor", "single_person", "single_generation_root"]
            .iter()
            .any(|k| r["concentration"][*k] == true)
        {
            return Err("incomplete judgements cannot establish a singleton".into());
        }
    }
    Ok(())
}

fn agency_differences_schema() -> Ty {
    use Ty::*;
    let strings = || Arr(Box::new(Str));
    let judgement = || Arr(Box::new(judgement_schema()));
    let unknown_side = |actor_capacity: bool| {
        let reasons = Lit(&[
            "state_guards",
            "resource_infeasible",
            "derivation_limit",
            "agency_unavailable",
            "state_guard",
            "target_not_called",
            "no_source_identified",
            "no_reachable_effect",
            "noncommit_state_write",
            "observation_missing",
            "observation_not_derived",
            "refusal_missing",
            "refusal_conditional",
            "refusal_not_derived",
            "no_reachability_path",
            "path_not_reconstructed",
            "duration_unknown",
        ]);
        let reasons = if actor_capacity {
            OneOf(vec![reasons, Lit(&["actor_capacity_infeasible"])])
        } else {
            reasons
        };
        Obj(vec![
            ("status", Lit(&["derived", "not_derived", "unchecked"])),
            ("reason", Null(Box::new(reasons))),
        ])
    };
    let mut rows = vec![];
    // Each (viewpoint,item) has its own key and value type, not arbitrary JSON.
    let mut add = |vp: &'static [&'static str],
                   item: &'static [&'static str],
                   keys: Vec<(&'static str, Ty)>,
                   value: Ty| {
        let mut base = vec![("viewpoint", Lit(vp)), ("item", Lit(item))];
        base.extend(keys);
        let mut changed = base.clone();
        changed.extend([
            ("status", Lit(&["changed"])),
            ("from", Null(Box::new(value.clone()))),
            ("to", Null(Box::new(value))),
        ]);
        let mut unknown = base;
        // Keys may be absent only when a whole block cannot enumerate its rows.
        for (key, ty) in unknown.iter_mut().skip(2) {
            let _ = key;
            *ty = OptionalField(Box::new(ty.clone()));
        }
        let actor_capacity = vp == ["framing"] || vp == ["retention"];
        unknown.extend([
            ("status", Lit(&["not_comparable"])),
            ("from", unknown_side(actor_capacity)),
            ("to", unknown_side(actor_capacity)),
        ]);
        rows.extend([Obj(changed), Obj(unknown)]);
    };
    add(
        &["observation"],
        &[
            "normal.actors",
            "normal.people",
            "normal.unattributed_actors",
            "contest.actors",
            "contest.people",
            "contest.unattributed_actors",
            "contest.routes",
        ],
        vec![("actor", Str)],
        strings(),
    );
    add(
        &["observation"],
        &["normal.direct", "normal.indirect", "contest.direct"],
        vec![("person", Str)],
        strings(),
    );
    add(
        &["origin"],
        &["direct", "indirect"],
        vec![("actor", Str)],
        strings(),
    );
    add(&["framing"], &["reachable"], vec![("end", Str)], Bool);
    add(
        &["framing"],
        &["selectors"],
        vec![("end", Str)],
        Arr(Box::new(Obj(vec![
            ("call", Str),
            ("capability", Str),
            ("selects", Lit(&["all_contexts", "some_contexts"])),
            ("blocked", OptionalField(Box::new(Bool))),
        ]))),
    );
    add(
        &["framing"],
        &["read_only"],
        vec![("end", Str)],
        generation_roots_schema(),
    );
    add(
        &["framing"],
        &["actors"],
        vec![],
        Obj(vec![("actors", strings()), ("complete", Bool)]),
    );
    add(
        &["retention"],
        &["normal"],
        vec![("capability", Str)],
        Obj(vec![("min", Int), ("max", Int)]),
    );
    add(
        &["retention"],
        &["fallback"],
        vec![("capability", Str)],
        Lit(&["unchecked", "kept_by_intent"]),
    );
    add(
        &["retention"],
        &["contest_route", "join_any_cancellation"],
        vec![("capability", Str)],
        Bool,
    );
    add(
        &["effects"],
        &["e"],
        vec![("effect", Str), ("call", Str)],
        effect_carrier_schema(),
    );
    add(
        &["effects"],
        &["j"],
        vec![("effect", Str), ("call", Str)],
        Arr(Box::new(effect_judgement_schema())),
    );
    add(
        &["effects"],
        &["u"],
        vec![("effect", Str), ("call", Str)],
        Arr(Box::new(effect_refuser_schema())),
    );
    add(
        &["effects"],
        &["state_refusers"],
        vec![("effect", Str), ("call", Str)],
        state_refusers_schema(),
    );
    add(
        &["effects"],
        &[
            "concentration.single_actor",
            "concentration.single_person",
            "concentration.single_generation_root",
        ],
        vec![("effect", Str), ("call", Str)],
        OneOf(vec![Bool, Lit(&["unknown"])]),
    );
    add(
        &["effects"],
        &["concentration.generation_roots"],
        vec![("effect", Str), ("call", Str)],
        generation_roots_schema(),
    );
    add(
        &["refusal"],
        &["judgements"],
        vec![("effect", Str), ("call", Str)],
        judgement(),
    );
    add(
        &["lineage_groups"],
        &["judgements", "actors"],
        vec![("subject_sources", strings())],
        Int,
    );
    add(
        &["lineage_groups"],
        &["independent_sources"],
        vec![("subject_sources", strings())],
        strings(),
    );
    let target = || {
        Obj(vec![
            (
                "kind",
                Lit(&["capability", "state_condition", "maintenance"]),
            ),
            ("value", Str),
        ])
    };
    let obs = Obj(vec![
        ("status", Lit(&["present", "missing", "not_derived"])),
        ("reason", Null(Box::new(Str))),
        ("required_sources", strings()),
        ("direct", strings()),
        ("indirect", strings()),
        ("missing_sources", strings()),
    ]);
    add(
        &["responsibility"],
        &["observation"],
        vec![("person", Str), ("target", target())],
        obs,
    );
    // Reuse the exact existing responsibility value subtable, not a looser DTO.
    let Arr(resps) = responsibility_schema() else {
        unreachable!()
    };
    let Obj(resps) = *resps else { unreachable!() };
    let refusal = resps.into_iter().find(|(k, _)| *k == "refusal").unwrap().1;
    add(
        &["responsibility"],
        &["refusal"],
        vec![("person", Str), ("target", target())],
        refusal,
    );
    add(
        &["responsibility"],
        &["time_frame"],
        vec![("person", Str), ("target", target())],
        responsibility_time_schema(),
    );
    let unchecked = || {
        Arr(Box::new(Lit(&[
            "fallback_path",
            "workload_transfer",
            "via_model_paths",
        ])))
    };
    OneOf(vec![
        Obj(vec![
            ("rules", Lit(&["agency-diff-p2-1"])),
            ("status", Lit(&["derived"])),
            ("rows", Arr(Box::new(OneOf(rows)))),
            ("unchecked", unchecked()),
        ]),
        Obj(vec![
            ("rules", Lit(&["agency-diff-p2-1"])),
            ("status", Lit(&["not_derived"])),
            (
                "reason",
                Lit(&[
                    "agency_unavailable",
                    "comparison_mismatch",
                    "derivation_limit",
                ]),
            ),
            ("unchecked", unchecked()),
        ]),
    ])
}
#[allow(dead_code)]
pub fn validate_agency_differences(v: &Value) -> Result<(), String> {
    let schema = agency_differences_schema();
    check(v, &schema, "agency.differences")?;
    complete(v, &schema, "agency.differences")?;
    if v["unchecked"]
        != serde_json::json!(["fallback_path", "workload_transfer", "via_model_paths"])
    {
        return Err("agency diff unchecked".into());
    }
    let groups: &[(&str, &[&str])] = &[
        (
            "observation",
            &[
                "normal.actors",
                "normal.people",
                "normal.unattributed_actors",
                "contest.actors",
                "contest.people",
                "contest.unattributed_actors",
                "contest.routes",
                "normal.direct",
                "normal.indirect",
                "contest.direct",
            ],
        ),
        ("origin", &["direct", "indirect"]),
        (
            "framing",
            &["reachable", "selectors", "read_only", "actors"],
        ),
        (
            "retention",
            &[
                "normal",
                "fallback",
                "contest_route",
                "join_any_cancellation",
            ],
        ),
        (
            "effects",
            &[
                "e",
                "j",
                "u",
                "state_refusers",
                "concentration.single_actor",
                "concentration.single_person",
                "concentration.single_generation_root",
                "concentration.generation_roots",
            ],
        ),
        ("refusal", &["judgements"]),
        (
            "lineage_groups",
            &["judgements", "actors", "independent_sources"],
        ),
        ("responsibility", &["observation", "refusal", "time_frame"]),
    ];
    let mut previous = None;
    let mut keys = std::collections::BTreeSet::new();
    for r in v["rows"].as_array().into_iter().flatten() {
        let rank = groups
            .iter()
            .flat_map(|(v, items)| items.iter().map(move |i| (*v, *i)))
            .position(|(v, i)| r["viewpoint"] == v && r["item"] == i)
            .unwrap();
        if previous.is_some_and(|p| p > rank) {
            return Err("meaning difference viewpoint/item order".into());
        }
        previous = Some(rank);
        let mut key = r.as_object().unwrap().clone();
        for field in ["from", "to", "status"] {
            key.remove(field);
        }
        if !keys.insert(serde_json::to_string(&key).unwrap()) {
            return Err("duplicate meaning difference key".into());
        }
        if r["status"] == "changed" && r["from"] == r["to"] {
            return Err("equal known values are not a difference".into());
        }
        if r["status"] == "changed"
            && ((r["viewpoint"] == "effects" && r["item"] == "u")
                || (r["viewpoint"] == "refusal" && r["item"] == "judgements"))
        {
            for side in ["from", "to"] {
                if !r[side].is_null() {
                    validate_rule_items(&r[side])?;
                }
            }
        }
        if r["status"] == "not_comparable" {
            for side in ["from", "to"] {
                if r[side]["reason"] == "actor_capacity_infeasible"
                    && r[side]["status"] != "not_derived"
                {
                    return Err("Actor capacity reason requires not_derived".into());
                }
                if r[side]["status"] == "derived" && !r[side]["reason"].is_null() {
                    return Err("known-side reason must be null".into());
                }
            }
        }
    }
    Ok(())
}

/// ADR 0034/0043 closed wire table. Missing payload is not an empty derived table.
fn viewpoints_schema() -> Ty {
    use Ty::*;
    let strings = || Arr(Box::new(Str));
    let calls = |selects| {
        let mut fs = vec![("call", Str), ("capability", Str)];
        if selects {
            fs.push(("selects", Lit(&["all_contexts", "some_contexts"])));
            fs.push(("blocked", OptionalField(Box::new(Bool))));
        }
        Arr(Box::new(Obj(fs)))
    };
    let failed = || {
        vec![
            ("status", Lit(&["not_derived"])),
            ("reason", Lit(&["derivation_limit"])),
        ]
    };
    let mut o_fail = failed();
    o_fail.push(("fallback", Lit(&["unchecked"])));
    let framing_tail = || vec![("complete", Bool), ("unchecked", strings())];
    let retention_tail = || {
        vec![
            ("notice", Lit(&["行使回数から不要とは判定しない"])),
            ("unchecked", strings()),
        ]
    };
    let mut f_fail = failed();
    f_fail.extend(framing_tail());
    let mut l_fail = failed();
    l_fail.extend(retention_tail());
    let mut f_ok = vec![
        ("status", Lit(&["derived"])),
        (
            "ends",
            Arr(Box::new(OneOf(vec![
                Obj(vec![
                    ("end", Str),
                    ("status", Lit(&["derived"])),
                    ("reachable", Bool),
                    ("selectors", calls(true)),
                    ("read_only", calls(false)),
                ]),
                Obj(vec![
                    ("end", Str),
                    ("status", Lit(&["not_derived"])),
                    (
                        "reason",
                        Lit(&[
                            "state_guards",
                            "resource_infeasible",
                            "actor_capacity_infeasible",
                        ]),
                    ),
                ]),
            ]))),
        ),
        ("actors", strings()),
    ];
    f_ok.extend(framing_tail());
    let cap_tail = || {
        vec![
            ("contest_route", Bool),
            ("basis", Lit(&["unknown"])),
            ("fallback", Lit(&["unchecked", "kept_by_intent"])),
            ("join_any_cancellation", Bool),
        ]
    };
    let mut cap_ok = vec![
        ("capability", Str),
        ("status", Lit(&["derived"])),
        ("normal", Obj(vec![("min", Int), ("max", Int)])),
    ];
    cap_ok.extend(cap_tail());
    let mut cap_fail = vec![
        ("capability", Str),
        ("status", Lit(&["not_derived"])),
        (
            "reason",
            Lit(&[
                "state_guards",
                "resource_infeasible",
                "actor_capacity_infeasible",
            ]),
        ),
    ];
    cap_fail.extend(cap_tail());
    let mut l_ok = vec![
        ("status", Lit(&["derived"])),
        (
            "capabilities",
            Arr(Box::new(OneOf(vec![Obj(cap_ok), Obj(cap_fail)]))),
        ),
    ];
    l_ok.extend(retention_tail());
    Obj(vec![
        (
            "rules",
            Lit(&["agency-viewpoints-p1-1", "agency-viewpoints-p1-2"]),
        ),
        (
            "scope",
            Lit(&["selected_process_lexical_calls_and_declared_contest_routes"]),
        ),
        (
            "observation",
            OneOf(vec![
                Obj(vec![
                    ("status", Lit(&["derived"])),
                    (
                        "sources",
                        Arr(Box::new(Obj(vec![
                            ("actor", Str),
                            (
                                "normal",
                                Obj(vec![
                                    ("actors", strings()),
                                    ("people", strings()),
                                    ("unattributed_actors", strings()),
                                ]),
                            ),
                            (
                                "contest",
                                Obj(vec![
                                    ("actors", strings()),
                                    ("people", strings()),
                                    ("unattributed_actors", strings()),
                                    ("routes", strings()),
                                ]),
                            ),
                        ]))),
                    ),
                    (
                        "people",
                        Arr(Box::new(Obj(vec![
                            ("person", Str),
                            (
                                "normal",
                                Obj(vec![("direct", strings()), ("indirect", strings())]),
                            ),
                            ("contest", Obj(vec![("direct", strings())])),
                        ]))),
                    ),
                    ("fallback", Lit(&["unchecked"])),
                ]),
                Obj(o_fail),
            ]),
        ),
        ("framing", OneOf(vec![Obj(f_ok), Obj(f_fail)])),
        (
            "proposal",
            Obj(vec![
                ("status", Lit(&["not_derived"])),
                ("reason", Lit(&["no_proposal_contract"])),
            ]),
        ),
        ("retention", OneOf(vec![Obj(l_ok), Obj(l_fail)])),
        ("effects", OptionalField(Box::new(effects_schema()))),
    ])
}

#[allow(dead_code)]
pub fn validate_viewpoints(v: &Value) -> Result<(), String> {
    let schema = viewpoints_schema();
    check(v, &schema, "viewpoints")?;
    complete(v, &schema, "viewpoints")?;
    if v["rules"] == "agency-viewpoints-p1-2" {
        validate_effects(v.get("effects").ok_or("missing effects for new rules")?)?;
    } else if v.get("effects").is_some() {
        return Err("effects requires new rules".into());
    }
    fn unique(v: &Value) -> Result<(), String> {
        match v {
            Value::Array(a) => {
                let key = a.first().and_then(|r| {
                    ["actor", "person", "end", "capability"]
                        .into_iter()
                        .find(|k| r.get(*k).is_some())
                });
                let mut seen = std::collections::BTreeSet::new();
                for r in a {
                    let identity = if r.is_string() {
                        r
                    } else if let Some(k) = key {
                        if r.get("call").is_some() { r } else { &r[k] }
                    } else {
                        r
                    };
                    if identity.as_str() == Some("") || !seen.insert(identity.to_string()) {
                        return Err("duplicate/empty viewpoints identity".into());
                    }
                    unique(r)?;
                }
            }
            Value::Object(o) => {
                for r in o.values() {
                    unique(r)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    unique(v)?;
    let f = &v["framing"];
    if f["unchecked"] != serde_json::json!(["type_domain_authority", "practical_framing"])
        || v["retention"]["unchecked"]
            != serde_json::json!(["fallback_dependency", "retention_basis", "actual_skill"])
    {
        return Err("viewpoints exact unchecked order".into());
    }
    if f["status"] == "not_derived" {
        if f["complete"] != false {
            return Err("budget failure is incomplete".into());
        }
    } else {
        let ends = f["ends"].as_array().unwrap();
        if f["complete"] != ends.iter().all(|e| e["status"] == "derived") {
            return Err("framing completeness".into());
        }
        for e in ends {
            if e["reachable"] == false
                && (e["selectors"] != serde_json::json!([])
                    || e["read_only"] != serde_json::json!([]))
            {
                return Err("unreachable end cannot have readers".into());
            }
            let mut calls = std::collections::BTreeSet::new();
            for r in e["selectors"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(e["read_only"].as_array().into_iter().flatten())
            {
                if r.get("blocked").is_some() && r["blocked"] != true {
                    return Err("blocked is optional true only".into());
                }
                if !calls.insert(r["call"].to_string()) {
                    return Err("duplicate selector/read-only call".into());
                }
            }
        }
        let mut actors = std::collections::BTreeSet::new();
        for e in ends.iter().filter(|e| e["status"] == "derived") {
            for selector in e["selectors"].as_array().unwrap() {
                let (actor, capability) = selector["capability"]
                    .as_str()
                    .unwrap()
                    .split_once('.')
                    .ok_or("selector capability identity")?;
                if actor.is_empty() || capability.is_empty() || capability.contains('.') {
                    return Err("selector capability identity".into());
                }
                actors.insert(actor);
            }
        }
        let aggregate: std::collections::BTreeSet<_> = f["actors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();
        if aggregate != actors {
            return Err("framing actors must equal derived selectors' Actor union".into());
        }
    }
    for r in v["retention"]["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if r["status"] == "derived" && r["normal"]["min"].as_u64() > r["normal"]["max"].as_u64() {
            return Err("retention min exceeds max".into());
        }
    }
    Ok(())
}

// A row's reason is not evidence for its own precondition. Use the independent
// Actor table at Report boundaries (also for diff's top-level Agency copies).
fn validate_actor_capacity_reason(agency: &Value, actors: &Value) -> Result<(), String> {
    let viewpoints = &agency["viewpoints"];
    let has_reason = [
        &viewpoints["framing"]["ends"],
        &viewpoints["retention"]["capabilities"],
    ]
    .into_iter()
    .flat_map(|rows| rows.as_array().into_iter().flatten())
    .any(|row| row["reason"] == "actor_capacity_infeasible");
    if has_reason {
        require_zero_capacity(actors)?;
    }
    Ok(())
}

fn require_zero_capacity(actors: &Value) -> Result<(), String> {
    if !actors
        .as_array()
        .is_some_and(|rows| rows.iter().any(|actor| actor["capacity"]["value"] == 0))
    {
        return Err("actor_capacity_infeasible requires an Actor with capacity 0".into());
    }
    Ok(())
}

fn validate_capacity_difference_context(d: &Value, from: &Value, to: &Value) -> Result<(), String> {
    for row in d["rows"].as_array().into_iter().flatten() {
        for (side, actors) in [("from", from), ("to", to)] {
            if row[side]["reason"] == "actor_capacity_infeasible" {
                require_zero_capacity(actors)?;
            }
        }
    }
    Ok(())
}

fn nullable_agency() -> Ty {
    use Ty::*;
    let strings = || Arr(Box::new(Str));
    let uncertainty = |kind, detail, detail_ty, notice| {
        Obj(vec![
            ("kind", Lit(kind)),
            ("call", Str),
            ("capability", Str),
            ("kernel", Str),
            ("targets", strings()),
            ("error_latents", strings()),
            (detail, detail_ty),
            ("notice", Lit(notice)),
        ])
    };
    Null(Box::new(Obj(vec![
        ("viewpoints", OptionalField(Box::new(viewpoints_schema()))),
        (
            "common_causes",
            OptionalField(Box::new(Obj(vec![
                ("rules", Lit(&["common-cause-p1-1"])),
                ("scope", Lit(&["selected_process_lexical_calls"])),
                ("notice", Lit(&[COMMON_CAUSE_NOTICE])),
                (
                    "causes",
                    Arr(Box::new(Obj(vec![
                        ("actor", Str),
                        ("latent", Str),
                        ("instance", OptionalField(Box::new(Lit(&["per_case"])))),
                        (
                            "targets",
                            Keyed(
                                "call",
                                Box::new(Obj(vec![
                                    ("call", Str),
                                    ("capability", Str),
                                    ("kernel", Str),
                                ])),
                            ),
                        ),
                    ]))),
                ),
            ]))),
        ),
        (
            "model_uncertainties",
            OptionalField(Box::new(Obj(vec![
                ("rules", Lit(&["wlin2-suppression-p1-1"])),
                ("scope", Lit(&["selected_process_lexical_calls"])),
                (
                    "items",
                    Keyed(
                        "call",
                        Box::new(OneOf(vec![
                            uncertainty(&["wlin2_basis"], "basis", Str, &[BASIS_NOTICE]),
                            uncertainty(&["wlin2_tacit"], "sources", strings(), &[TACIT_NOTICE]),
                            uncertainty(
                                &["wlin2_correlation"],
                                "judgements",
                                strings(),
                                &[CORRELATION_NOTICE],
                            ),
                        ])),
                    ),
                ),
            ]))),
        ),
        (
            "source_descriptions",
            OptionalField(Box::new(Keyed(
                "actor",
                Box::new(Obj(vec![
                    ("actor", Str),
                    ("kind", Lit(&["tacit"])),
                    ("description", Str),
                ])),
            ))),
        ),
        (
            "rules",
            Lit(&[
                "agency-p0-1",
                "agency-p1-state-1",
                "agency-p1-requires-1",
                "agency-p1-reads-1",
                "agency-p1-rule-1",
            ]),
        ),
        (
            "origin",
            Arr(Box::new(Obj(vec![
                ("actor", Str),
                ("direct", strings()),
                ("indirect", strings()),
            ]))),
        ),
        (
            "lineage",
            Arr(Box::new(Obj(vec![
                ("call", Str),
                ("capability", Str),
                ("subject_sources", strings()),
                ("independent_sources", strings()),
            ]))),
        ),
        (
            "lineage_groups",
            Arr(Box::new(Obj(vec![
                ("subject_sources", strings()),
                ("judgements", Int),
                ("actors", Int),
                ("independent_sources", strings()),
            ]))),
        ),
        (
            "refusal",
            Arr(Box::new(Obj(vec![
                ("effect", Str),
                ("call", Str),
                ("reachable", Bool),
                ("judgements", Arr(Box::new(judgement_schema()))),
            ]))),
        ),
        (
            "unchecked",
            Arr(Box::new(Lit(&[
                "W-RESP-1",
                "contest_path",
                "fallback_path",
                "executors",
                "via_model",
                "state_guards",
                "kernel_correlations",
                "multi_case_interference",
                "responsibility_time_frame",
                "responsibility_assignment_scope",
                "responsibility_observation",
                "responsibility_refusal",
                "responsibility_noncommit_writes",
                "responsibility_derivation_limit",
                "responsibility_people_undeclared",
                "contest_occurrence",
                "contest_response_time",
                "contest_response_deadline",
                "contest_route_handoff",
                "contest_decision_coverage",
                "obligation_debtor",
                "contest_real_world_occurrence",
                "contest_real_world_response_time",
                "contest_acceptance",
                "contest_remedy",
            ]))),
        ),
    ])))
}

fn cut_difference_schema(value: &Value) -> Result<Ty, String> {
    use Ty::*;
    let entry = || {
        Obj(vec![
            ("target", Str),
            ("actor", Bool),
            ("reason", Str),
            ("loses", Null(Box::new(Str))),
        ])
    };
    let field = identifier(value, "field", "cut difference")?;
    let wire = || match field {
        "question" => Ok(Null(Box::new(Str))),
        "include" | "externality" | "reentry" => Ok(Arr(Box::new(Str))),
        "horizon" => Ok(horizon_schema()),
        "exclude" | "defer" => Ok(Keyed("target", Box::new(entry()))),
        _ => Err(format!("unknown cut difference field {field}")),
    };
    Ok(Obj(vec![
        ("field", Str),
        ("from", wire()?),
        ("to", wire()?),
    ]))
}

/// Required-key validation for actual wire, not for partial expectations.
fn complete(value: &Value, ty: &Ty, path: &str) -> Result<(), String> {
    match ty {
        Ty::Obj(fields) => {
            for (key, field) in fields {
                if let Ty::OptionalField(inner) = field {
                    if let Some(item) = value.get(key) {
                        complete(item, inner, &format!("{path}.{key}"))?;
                    }
                    continue;
                }
                if *key == "absent_diagnostics" {
                    if value.get(key).is_some() {
                        return Err(format!("{path}: metadata on wire"));
                    }
                    continue;
                }
                let item = value
                    .get(key)
                    .ok_or_else(|| format!("{path}: missing {key}"))?;
                complete(item, field, &format!("{path}.{key}"))?;
            }
        }
        Ty::Null(inner) if !value.is_null() => complete(value, inner, path)?,
        Ty::Arr(inner) | Ty::Keyed(_, inner) | Ty::Pairs(inner) => {
            for (i, item) in value.as_array().unwrap().iter().enumerate() {
                complete(item, inner, &format!("{path}[{i}]"))?;
            }
        }
        Ty::Diagnostic => {
            for key in ["code", "severity", "message", "span"] {
                if value.get(key).is_none() {
                    return Err(format!("{path}: missing {key}"));
                }
            }
            if !value["span"].is_null() {
                complete(
                    &value["span"],
                    &Ty::Obj(vec![("start", Ty::Int), ("end", Ty::Int)]),
                    path,
                )?;
            }
        }
        Ty::OneOf(options) => {
            let option = options
                .iter()
                .find(|option| check(value, option, path).is_ok())
                .ok_or("no wire form")?;
            complete(value, option, path)?;
        }
        Ty::CutDifference => complete(value, &cut_difference_schema(value)?, path)?,
        _ => {}
    }
    Ok(())
}

fn scenario_schema() -> Ty {
    use Ty::*;
    Obj(vec![
        ("name", Str),
        (
            "arrivals",
            Obj(vec![
                ("form", Lit(&["fixed", "fixed_every"])),
                ("every", Null(Box::new(Int))),
                ("from", Int),
                ("until", Int),
                ("count", Int),
            ]),
        ),
        ("horizon", Null(Box::new(Int))),
        (
            "drain",
            Obj(vec![
                ("from", Int),
                ("until", OneOf(vec![Int, Lit(&["all_terminal_or_clock"])])),
            ]),
        ),
        (
            "aggregation",
            Obj(vec![
                ("from", Int),
                (
                    "until",
                    Lit(&["all_terminal_or_clock", "horizon_or_all_terminal_or_clock"]),
                ),
                ("observed_until", metric_scalar_schema()),
            ]),
        ),
        ("warmup", Int),
    ])
}

/// Independent closed ADR0042 wire guard. Never accepts cutoff fields on old
/// reports merely because their types happen to be correct.
#[allow(dead_code)]
pub fn validate_scenario_wire(v: &Value) -> Result<(), String> {
    let scenario = v.get("scenario");
    let horizon = scenario.and_then(|s| s["horizon"].as_u64());
    let sampled = v["mode"] == "simulation";
    if let Some(s) = scenario {
        check(s, &scenario_schema(), "scenario")?;
        complete(s, &scenario_schema(), "scenario")?;
        let cases = v["case_outcomes"]
            .as_array()
            .ok_or("scenario requires per-case route")?;
        let a = &s["arrivals"];
        if s["name"].as_str().unwrap().is_empty()
            || s["warmup"] != 0
            || s["aggregation"]["from"] != 0
            || cases.is_empty()
            || a["count"].as_u64() != Some(cases.len() as u64)
            || v["bounds"]["cases"] != a["count"]
            || a["from"] != cases[0]["arrival"]
            || s["drain"]["from"] != cases.last().unwrap()["arrival"]
            || horizon == Some(0)
            || horizon.is_some_and(|d| cases.iter().any(|c| c["arrival"].as_u64().unwrap() > d))
        {
            return Err("scenario identity/arrival/horizon mismatch".into());
        }
        if a["form"] == "fixed_every" {
            let step = a["every"].as_u64().ok_or("fixed_every step")?;
            let from = a["from"].as_u64().unwrap();
            let until = a["until"].as_u64().unwrap();
            if step == 0
                || until <= from
                || 1 + (until - from - 1) / step != cases.len() as u64
                || cases.len() > 10000
                || cases.iter().enumerate().any(|(i, c)| {
                    step.checked_mul(i as u64).and_then(|n| from.checked_add(n))
                        != c["arrival"].as_u64()
                })
            {
                return Err("fixed_every expansion mismatch".into());
            }
        } else if !a["every"].is_null() || a["until"] != cases.last().unwrap()["arrival"] {
            return Err("fixed arrival metadata mismatch".into());
        }
        let until = horizon.map_or_else(
            || serde_json::json!("all_terminal_or_clock"),
            |d| serde_json::json!(d),
        );
        if s["drain"]["until"] != until
            || s["aggregation"]["until"]
                != if horizon.is_some() {
                    "horizon_or_all_terminal_or_clock"
                } else {
                    "all_terminal_or_clock"
                }
        {
            return Err("drain declaration and observation rule differ".into());
        }
        let rows = v["assumptions"]
            .as_array()
            .ok_or("scenario assumptions missing")?;
        let expected = if horizon.is_some() {
            vec![
                "scenario",
                "scenario.horizon",
                "scenario.drain",
                "scenario.aggregation",
            ]
        } else {
            vec!["scenario", "scenario.drain", "scenario.aggregation"]
        };
        let observed: Vec<_> = rows
            .iter()
            .filter_map(|r| r["field"].as_str())
            .filter(|s| *s == "scenario" || s.starts_with("scenario."))
            .collect();
        if observed != expected
            || !rows
                .iter()
                .any(|r| r["field"] == "scenario" && r["value"] == s["name"])
        {
            return Err("scenario assumption inventory/name/order".into());
        }
        let observed = &s["aggregation"]["observed_until"];
        check(observed, &metric_scalar_schema(), "scenario observation")?;
        complete(observed, &metric_scalar_schema(), "scenario observation")?;
        let observation_status = if sampled {
            if v["simulation"]["status"] == "completed" {
                "complete"
            } else {
                "aborted"
            }
        } else {
            exhaustive_word(v)
        };
        metric_tree(
            observed,
            sampled,
            observation_status,
            v["simulation"]["replications"].as_u64().unwrap_or(0),
        )?;
        if sampled != observed.get("samples").is_some() {
            return Err("scenario observation mode mismatch".into());
        }
        if !sampled
            && v["bounds"]["truncated"] == true
            && (!observed["min"].is_null()
                || !observed["max"].is_null()
                || observed["reason"] != "truncated")
        {
            return Err("scenario observation must retain computational unknown".into());
        }
    } else if v["assumptions"].as_array().is_some_and(|rs| {
        rs.iter().any(|r| {
            r["field"]
                .as_str()
                .is_some_and(|s| s == "scenario" || s.starts_with("scenario."))
        })
    }) {
        return Err("scenario assumptions without scenario".into());
    }
    if v.get("simulation").is_some() {
        let field = v["simulation"].get("cut_off_replications");
        if horizon.is_some() != field.is_some()
            || field.is_some_and(|n| {
                if v["simulation"]["status"] == "completed" {
                    n.as_u64().is_none_or(|n| {
                        n > v["simulation"]["completed_replications"]
                            .as_u64()
                            .unwrap_or(0)
                    })
                } else {
                    !n.is_null()
                }
            })
        {
            return Err("cutoff replication presence/count".into());
        }
    }
    for c in v["case_outcomes"].as_array().into_iter().flatten() {
        if sampled && horizon.is_some() != c.get("cut_off_count").is_some() {
            return Err("cutoff case count presence".into());
        }
        if sampled && horizon.is_some() {
            let count = &c["cut_off_count"];
            if if v["simulation"]["status"] == "completed" {
                count.as_u64().is_none_or(|n| {
                    n > v["simulation"]["completed_replications"]
                        .as_u64()
                        .unwrap_or(0)
                })
            } else {
                !count.is_null()
            } {
                return Err("cutoff case count type/abort withdrawal".into());
            }
        }
        for t in c["terminals"].as_array().into_iter().flatten() {
            if t["kind"] == "cut_off"
                && (horizon.is_none()
                    || sampled
                    || ["end", "completed_at", "elapsed", "result_at"]
                        .iter()
                        .any(|k| !t[k].is_null())
                    || !t["work"]["min"].is_u64()
                    || t["work"]["min"].as_u64() > t["work"]["max"].as_u64()
                    || t.get("probability").is_none())
            {
                return Err("cutoff is not a business terminal or a clock cut".into());
            }
        }
    }
    if let Some(m) = v.get("metrics") {
        if (m["observation_scope"] == "until_horizon_or_execution_stop") != horizon.is_some() {
            return Err("horizon metric observation scope mismatch".into());
        }
        let mut counts = vec![&m["overall"]["counts"]];
        for c in m["cases"].as_array().into_iter().flatten() {
            counts.push(&c["counts"]);
            for t in c["terminals"].as_array().into_iter().flatten() {
                counts.push(&t["excluded"]);
            }
        }
        if counts
            .iter()
            .any(|r| r.get("cut_off").is_some() != horizon.is_some())
        {
            return Err("horizon metric cutoff count presence".into());
        }
    }
    Ok(())
}

fn arrival_value_matches(value: &Value, cases: &[Value]) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    let times = cases
        .iter()
        .map(|c| format!("{}min", c["arrival"].as_u64().unwrap()))
        .collect::<Vec<_>>()
        .join(", ");
    if value == format!("fixed[{times}]") {
        return true;
    }
    let Some(rest) = value.strip_prefix("fixed_every ") else {
        return false;
    };
    let Some((step, rest)) = rest.split_once("min during [") else {
        return false;
    };
    let Some((from, rest)) = rest.split_once("min, ") else {
        return false;
    };
    let Some((until, count)) = rest.split_once("min) (") else {
        return false;
    };
    let Some(count) = count.strip_suffix(" cases)") else {
        return false;
    };
    let nums = [step, from, until, count].map(str::parse::<u64>);
    let [Ok(step), Ok(from), Ok(until), Ok(count)] = nums else {
        return false;
    };
    step > 0
        && until > from
        && count == cases.len() as u64
        && count <= 10000
        && 1 + (until - from - 1) / step == count
        && cases.iter().enumerate().all(|(i, c)| {
            step.checked_mul(i as u64).and_then(|n| from.checked_add(n)) == c["arrival"].as_u64()
        })
}

fn report_schema() -> Ty {
    use Ty::*;
    let range = || Obj(vec![("min", Int), ("max", Int)]);
    let entry = || {
        Obj(vec![
            ("target", Str),
            ("actor", Bool),
            ("reason", Str),
            ("loses", Null(Box::new(Str))),
        ])
    };
    let strings = || Arr(Box::new(Str));
    Obj(vec![
        ("ledger", Ledger),
        ("uncertainty", Uncertainty),
        ("verdict", Verdict),
        ("scenario", OptionalField(Box::new(scenario_schema()))),
        ("quality", OptionalField(Box::new(quality_schema(false)))),
        ("actors", actor_table_schema()),
        ("cut_context", cut_context_schema()),
        (
            "unchecked",
            OptionalField(Box::new(Arr(Box::new(Lit(&[
                "unreachable",
                "completion_graph",
                "dynamic_authority_revocation",
            ]))))),
        ),
        (
            "via_model_paths",
            OptionalField(Box::new(via_model_schema())),
        ),
        (
            "metrics",
            OptionalField(Box::new(business_metrics_schema())),
        ),
        ("agency", nullable_agency()),
        (
            "capability_refusals",
            OptionalField(Box::new(capability_refusals_schema(false))),
        ),
        (
            "responsibilities",
            OptionalField(Box::new(responsibility_schema())),
        ),
        ("contest_paths", OptionalField(Box::new(contest_schema()))),
        ("obligations", OptionalField(Box::new(obligation_schema()))),
        (
            "contest_runtime",
            OptionalField(Box::new(contest_runtime_schema())),
        ),
        (
            "contest_summary",
            OptionalField(Box::new(contest_summary_schema())),
        ),
        ("mode", Lit(&["exhaustive"])),
        (
            "state_range_violations",
            OptionalField(Box::new(Arr(Box::new(state_range_violation_schema())))),
        ),
        ("unit", Lit(&["min"])),
        (
            "model",
            Obj(vec![
                ("name", Null(Box::new(Str))),
                ("sha256", Null(Box::new(Str))),
            ]),
        ),
        (
            "cut",
            OneOf(vec![
                Lit(&["undeclared", "unresolved"]),
                Obj(vec![
                    ("name", Str),
                    ("question", Null(Box::new(Str))),
                    ("include", strings()),
                    ("exclude", Arr(Box::new(entry()))),
                    ("defer", Arr(Box::new(entry()))),
                    ("externality", strings()),
                    ("reentry", strings()),
                    ("checked", strings()),
                    ("unchecked", strings()),
                ]),
            ]),
        ),
        (
            "assumptions",
            Arr(Box::new(Obj(vec![
                ("field", Str),
                ("value", Str),
                ("affects", Str),
            ]))),
        ),
        ("diagnostics", Arr(Box::new(Diagnostic))),
        (
            "bounds",
            Obj(vec![
                ("cases", Null(Box::new(Int))),
                ("live_tasks", Null(Box::new(Int))),
                ("clock", Null(Box::new(Int))),
                ("truncated", Null(Box::new(Bool))),
                ("range_violation_stops", OptionalField(Box::new(Bool))),
            ]),
        ),
        (
            "properties",
            Keyed(
                "id",
                Box::new(Obj(vec![
                    ("id", Str),
                    ("verdict", Verdict),
                    (
                        "notes",
                        OptionalField(Box::new(Arr(Box::new(Lit(&["mechanism_unmodeled"]))))),
                    ),
                    ("probability", OptionalField(Box::new(Probability))),
                    (
                        "cases",
                        OptionalField(Box::new(Arr(Box::new(Obj(vec![
                            ("case_id", Int),
                            ("arrival", Int),
                            ("verdict", Verdict),
                            ("probability", OptionalField(Box::new(Probability))),
                        ]))))),
                    ),
                ])),
            ),
        ),
        (
            "terminals",
            Keyed(
                "end",
                Box::new(Obj(vec![
                    ("end", Str),
                    ("elapsed", range()),
                    ("work", range()),
                    ("result_at", Null(Box::new(range()))),
                    ("probability", OptionalField(Box::new(Probability))),
                ])),
            ),
        ),
        ("absent_diagnostics", Arr(Box::new(Code))),
        (
            "case_outcomes",
            OptionalField(Box::new(Arr(Box::new(Obj(vec![
                ("case_id", Int),
                ("arrival", Int),
                (
                    "terminals",
                    Arr(Box::new(Obj(vec![
                        ("kind", Lit(&["end", "rejected", "overflow", "cut_off"])),
                        ("end", Null(Box::new(Str))),
                        ("completed_at", Null(Box::new(range()))),
                        ("elapsed", Null(Box::new(range()))),
                        ("work", Null(Box::new(range()))),
                        ("result_at", Null(Box::new(range()))),
                        ("probability", OptionalField(Box::new(Probability))),
                    ]))),
                ),
                (
                    "admitted_at",
                    OptionalField(Box::new(Null(Box::new(range())))),
                ),
            ]))))),
        ),
        (
            "admission",
            OptionalField(Box::new(Obj(vec![
                ("rejected_count", range()),
                ("overflow_count", OptionalField(Box::new(range()))),
                ("external_wait_max", OptionalField(Box::new(range()))),
            ]))),
        ),
    ])
}

fn check(value: &Value, ty: &Ty, path: &str) -> Result<(), String> {
    let bad = |what: &str| Err(format!("{path}: expected {what}, got {value}"));
    match ty {
        Ty::Ledger => ledger_shape::validate(value),
        Ty::Uncertainty => ledger_shape::uncertainty_wire::validate(value),
        Ty::MetricNumber => metric_fraction(value).map(|_| ()),
        Ty::RationalText => value
            .as_str()
            .and_then(fraction)
            .map(|_| ())
            .ok_or_else(|| format!("{path}: expected canonical probability string")),
        Ty::SimulationInterval => {
            check(
                value,
                &Ty::Obj(vec![
                    ("lower", Ty::RationalText),
                    ("upper", Ty::RationalText),
                ]),
                path,
            )?;
            complete(
                value,
                &Ty::Obj(vec![
                    ("lower", Ty::RationalText),
                    ("upper", Ty::RationalText),
                ]),
                path,
            )?;
            validate_probability(&serde_json::json!({"min": value["lower"], "max": value["upper"]}))
        }
        Ty::OptionalField(inner) => check(value, inner, path),
        Ty::Probability => validate_probability(value).map_err(|e| format!("{path}: {e}")),
        Ty::Str if value.is_string() => Ok(()),
        Ty::Int if value.is_u64() => Ok(()),
        Ty::Bool if value.is_boolean() => Ok(()),
        Ty::Str => bad("a string"),
        Ty::Int => bad("a nonnegative integer"),
        Ty::Bool => bad("a boolean"),
        Ty::Verdict => serde_json::from_value::<relca_core::Verdict>(value.clone())
            .map(|_| ())
            .map_err(|_| format!("{path}: unknown verdict {value}")),
        Ty::Code => match value.as_str() {
            Some(code) if diagnostic_severity(code).is_some() => Ok(()),
            Some(code) => Err(format!("{path}: unknown diagnostic code {code}")),
            None => bad("a diagnostic code string"),
        },
        Ty::Severity => check(value, &Ty::Lit(&["error", "warning"]), path),
        Ty::Lit(allowed) => match value.as_str() {
            Some(text) if allowed.contains(&text) => Ok(()),
            _ => bad(&format!("one of {allowed:?}")),
        },
        Ty::Null(inner) => {
            if value.is_null() {
                Ok(())
            } else {
                check(value, inner, path)
            }
        }
        Ty::Arr(inner) => {
            let items = value
                .as_array()
                .ok_or(format!("{path}: expected an array, got {value}"))?;
            for (i, item) in items.iter().enumerate() {
                check(item, inner, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        Ty::Keyed(key, inner) => {
            let items = value
                .as_array()
                .ok_or(format!("{path}: expected an array, got {value}"))?;
            let mut seen = std::collections::BTreeSet::new();
            for (i, item) in items.iter().enumerate() {
                let path = format!("{path}[{i}]");
                check(item, inner, &path)?;
                let id = item[*key]
                    .as_str()
                    .ok_or(format!("{path}: missing string identifier {key}"))?;
                if !seen.insert(id) {
                    return Err(format!("{path}: duplicate {key} {id}"));
                }
            }
            Ok(())
        }
        Ty::Pairs(inner) => {
            let items = value
                .as_array()
                .ok_or(format!("{path}: expected an array"))?;
            for (i, item) in items.iter().enumerate() {
                check(item, inner, &format!("{path}[{i}]"))?;
            }
            pair_keyed(items, path).map(|_| ())
        }
        Ty::CutDifference => check(value, &cut_difference_schema(value)?, path),
        Ty::Obj(fields) => {
            let object = value
                .as_object()
                .ok_or(format!("{path}: expected an object, got {value}"))?;
            for (key, item) in object {
                let (_, field) = fields
                    .iter()
                    .find(|(name, _)| name == key)
                    .ok_or(format!("{path}: unknown key {key}"))?;
                check(item, field, &format!("{path}.{key}"))?;
            }
            Ok(())
        }
        Ty::Diagnostic => {
            let span = Ty::Null(Box::new(Ty::Obj(vec![
                ("start", Ty::Int),
                ("end", Ty::Int),
            ])));
            let shape = Ty::Obj(vec![
                ("code", Ty::Code),
                ("severity", Ty::Severity),
                ("message", Ty::Str),
                ("span", span),
            ]);
            check(value, &shape, path)?;
            // Expected diagnostics are matched by code, so it is required.
            if value.get("code").is_none() {
                return Err(format!("{path}: missing string identifier code"));
            }
            // A code fixes its severity; an expectation cannot contradict it.
            if let (Some(code), Some(severity)) = (value["code"].as_str(), value.get("severity")) {
                let required = serde_json::to_value(diagnostic_severity(code)).unwrap();
                if *severity != required {
                    return Err(format!("{path}: {code} must have severity {required}"));
                }
            }
            Ok(())
        }
        Ty::OneOf(options) => {
            if options
                .iter()
                .any(|option| check(value, option, path).is_ok())
            {
                Ok(())
            } else {
                bad("one of the allowed forms")
            }
        }
    }
}

fn simulation_schema() -> Ty {
    use Ty::*;
    let range = || Obj(vec![("min", Int), ("max", Int)]);
    let witness = || Null(Box::new(Obj(vec![("replication", Int)])));
    let sample_fields = || {
        vec![
            (
                "verdict",
                Lit(&[
                    "satisfied",
                    "violated",
                    "inconclusive",
                    "unsupported",
                    "invalid",
                ]),
            ),
            ("basis", Lit(&["witness", "sample"])),
            ("witness", witness()),
        ]
    };
    let mut statistical = vec![
        ("case_id", Int),
        ("arrival", Int),
        ("verdict", Lit(&["satisfied", "violated", "inconclusive"])),
        ("basis", Lit(&["statistical"])),
        ("hits", Int),
        ("unknown", Int),
        ("trials", Int),
        ("interval", SimulationInterval),
    ];
    let mut sample = vec![("case_id", Int), ("arrival", Int)];
    sample.extend(sample_fields());
    let case = OneOf(vec![Obj(std::mem::take(&mut statistical)), Obj(sample)]);
    let mut global = vec![("id", Str)];
    global.extend(sample_fields());
    let property = OneOf(vec![
        Obj(vec![
            ("id", Str),
            ("verdict", Lit(&["satisfied", "violated"])),
            ("basis", Lit(&["structural"])),
        ]),
        Obj(global),
        Obj(vec![
            ("id", Str),
            ("verdict", Verdict),
            ("cases", Arr(Box::new(case))),
            (
                "notes",
                OptionalField(Box::new(Arr(Box::new(Lit(&["mechanism_unmodeled"]))))),
            ),
        ]),
    ]);
    let Ty::Obj(mut fields) = report_schema() else {
        unreachable!()
    };
    fields.retain(|(key, _)| {
        ![
            "mode",
            "properties",
            "terminals",
            "case_outcomes",
            "admission",
            "unchecked",
            "capability_refusals",
            "quality",
        ]
        .contains(key)
    });
    fields.extend([
        ("mode", Lit(&["simulation"])),
        ("quality", OptionalField(Box::new(quality_schema(true)))),
        (
            "capability_refusals",
            OptionalField(Box::new(capability_refusals_schema(true))),
        ),
        (
            "unchecked",
            OptionalField(Box::new(Arr(Box::new(Lit(&[
                "obligation_outcomes",
                "unreachable",
                "dynamic_authority_revocation",
            ]))))),
        ),
        ("properties", Keyed("id", Box::new(property))),
        ("terminals", Arr(Box::new(Obj(vec![])))),
        (
            "case_outcomes",
            Arr(Box::new(Obj(vec![
                ("case_id", Int),
                ("arrival", Int),
                (
                    "terminals",
                    Arr(Box::new(Obj(vec![
                        ("kind", Lit(&["end", "rejected", "overflow"])),
                        ("end", Null(Box::new(Str))),
                        ("count", Int),
                    ]))),
                ),
                ("unfinished_count", Null(Box::new(Int))),
                ("deadlocked_count", Null(Box::new(Int))),
                (
                    "cut_off_count",
                    OptionalField(Box::new(Null(Box::new(Int)))),
                ),
                (
                    "range_stopped_count",
                    OptionalField(Box::new(Null(Box::new(Int)))),
                ),
            ]))),
        ),
        (
            "admission",
            Null(Box::new(Obj(vec![
                ("rejected_count", Int),
                ("overflow_count", OptionalField(Box::new(range()))),
                ("external_wait_max", OptionalField(Box::new(range()))),
            ]))),
        ),
        (
            "simulation",
            Obj(vec![
                ("status", Lit(&["completed", "unsupported", "invalid"])),
                ("replications", Null(Box::new(Int))),
                ("seed", Null(Box::new(Int))),
                ("confidence", Null(Box::new(RationalText))),
                ("attempted_replications", Int),
                ("completed_replications", Int),
                ("truncated_replications", Null(Box::new(Int))),
                (
                    "cut_off_replications",
                    OptionalField(Box::new(Null(Box::new(Int)))),
                ),
                (
                    "range_stopped_replications",
                    OptionalField(Box::new(Null(Box::new(Int)))),
                ),
                ("interval_method", Lit(&["clopper-pearson"])),
                ("random_method", Lit(&["sha256-counter-v1"])),
                ("nondeterminism", Lit(&["reject-on-encounter"])),
                (
                    "multiplicity",
                    Null(Box::new(Obj(vec![
                        ("method", Lit(&["bonferroni"])),
                        ("rows", Int),
                        ("alpha_before", Null(Box::new(RationalText))),
                        ("alpha_after", Null(Box::new(RationalText))),
                    ]))),
                ),
                (
                    "deadlock_witnesses",
                    Arr(Box::new(Obj(vec![
                        ("case_id", Int),
                        ("replication", Int),
                        ("time", Int),
                    ]))),
                ),
            ]),
        ),
    ]);
    Obj(fields)
}

const SIM_FIELDS: [&str; 5] = [
    "simulation.random",
    "simulation.replications",
    "simulation.multiplicity",
    "simulation.nondeterminism",
    "simulation.guarantee",
];
const STRUCTURAL_SIM_REFUSAL: &str = "simulation cannot check structural completion questions";
const SIM_AFFECTS: [&str; 5] = [
    "draws use keys formed from seed, replication, case, site and occurrence; each categorical probability has absolute discretization bias at most 2^-64; SHA-256 is not used for cryptographic purposes",
    "replications are independently initialized; cases sharing one replication are not independent statistical trials",
    "two one-sided Clopper-Pearson errors are allocated to each probability-query case row; the reported confidence is simultaneous across these rows",
    "unresolved nondeterministic successor actions are not assigned probabilities or resolved by a fixed ordering",
    "statistical verdicts concern this model and load under the declared confidence; absence of sampled violations is not a safety proof",
];

// A7 intake ruling: these five OLD sibling rows survive two no-metrics
// simulation refusals. This is an inverse validation projection, not approval
// of that legacy disclosure inconsistency or a metrics.waiting exemption.
fn without_legacy_refusal_metric_rows(v: &Value) -> Result<Option<Value>, String> {
    if v.get("metrics").is_some() || v["mode"] != "simulation" {
        return Ok(None);
    }
    let rows = v["assumptions"].as_array().ok_or("refusal assumptions")?;
    let positions: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r["field"]
                .as_str()
                .is_some_and(|f| f.starts_with("metrics."))
        })
        .map(|(j, _)| j)
        .collect();
    if positions.is_empty() {
        return Ok(None);
    }
    let revoker = rows.iter().any(|r| {
        r["field"]
            .as_str()
            .is_some_and(|f| f.starts_with("authority.") && f.ends_with(".revocable_by"))
    });
    let refused = v["diagnostics"].as_array().is_some_and(|ds| {
        ds.iter().any(|d| {
            d["code"] == "U-SEM-0" && d["message"] == STRUCTURAL_SIM_REFUSAL
                || revoker
                    && d["code"] == "E-SIM-0"
                    && d["message"] == "simulation requires an explicit simulation clause"
        })
    });
    let start = positions[0];
    if !refused || positions != (start..start + 5).collect::<Vec<_>>() {
        return Err("legacy refusal metrics rows outside closed routes/order".into());
    }
    for (j, row) in rows[start..start + 5].iter().enumerate() {
        if row["field"] != METRIC_FIELDS[j]
            || row["affects"] != METRIC_AFFECTS[j]
            || !row["value"].is_string()
            || row.as_object().is_none_or(|r| r.len() != 3)
        {
            return Err("legacy refusal metrics sibling fields/text".into());
        }
    }
    let block = &rows[start..start + 5];
    if block[0]["value"]
        .as_str()
        .and_then(|s| s.strip_suffix("min"))
        .and_then(|s| s.parse::<u64>().ok())
        .is_none_or(|n| n == 0)
        || block[1]["value"].as_str().unwrap().trim().is_empty()
        || block[2]["value"] != "model-declarations"
        || block[3]["value"] != "terminal_cases_only"
        || !matches!(
            block[4]["value"].as_str(),
            Some("until_execution_stop_no_extra_drain" | "until_horizon_or_execution_stop")
        )
    {
        return Err("legacy refusal metrics sibling values".into());
    }
    let mut old = v.clone();
    old["assumptions"]
        .as_array_mut()
        .unwrap()
        .drain(start..start + 5);
    Ok(Some(old))
}

fn validate_structural_refusal_quality(q: &Value) -> Result<(), String> {
    fn withdrawn(v: &Value) -> bool {
        match v {
            Value::Array(xs) => xs.iter().all(withdrawn),
            Value::Object(o) => o.iter().all(|(k, x)| match k.as_str() {
                "hits" | "unknown" | "trials" | "samples" | "observations" | "censored" => *x == 0,
                "point" | "mean" | "min" | "max" | "lower_bound" | "bounds_with_undetermined" => {
                    x.is_null()
                }
                "reason"
                    if o.contains_key("samples")
                        || o.contains_key("observations")
                        || o.contains_key("point") =>
                {
                    x == "aborted"
                }
                _ => withdrawn(x),
            }),
            _ => true,
        }
    }
    let has_quantiles = q.get("quantiles").is_some();
    if !withdrawn(q)
        || has_quantiles
            && (q["checked"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x == "quantiles")
                || !q["unchecked"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|x| x == "quantiles"))
    {
        return Err("structural refusal quality must be fully withdrawn".into());
    }
    Ok(())
}

/// No production DTO or arithmetic: closed shape, counts and alpha independently.
pub fn validate_simulation_wire(v: &Value) -> Result<(), String> {
    validate_deferred_quantity_positions(v).map_err(|error| format!("simulation wire: {error}"))?;
    validate_simulation_wire_inner(v)
}
fn validate_simulation_wire_inner(v: &Value) -> Result<(), String> {
    // Integration order, as in validate_wire/validate_business_metrics_wire:
    // validate the original waiting rows before projecting them, then require
    // every pre-existing quality row through the unchanged quality validator.
    if waiting_declared(v) {
        validate_waiting_wire(v)?;
        return validate_simulation_wire_inner(&waiting_projection(v));
    }
    reject_unscoped_waiting(v)?;
    if let Some(old) = without_legacy_refusal_metric_rows(v)? {
        return validate_simulation_wire_inner(&old);
    }
    if let Some(q) = v.get("quality") {
        validate_quality_wire(q, true)?;
        if v["diagnostics"].as_array().is_some_and(|ds| {
            ds.iter()
                .any(|d| d["code"] == "U-SEM-0" && d["message"] == STRUCTURAL_SIM_REFUSAL)
        }) {
            validate_structural_refusal_quality(q)?;
        }
    }
    if let Some(old) = without_quality(v)? {
        return validate_simulation_wire_inner(&old);
    }
    validate_caseset_notes(v)?;
    if let Some(rows) = v["assumptions"].as_array() {
        let limits = limits_declaration(rows)?;
        validate_limits_fields(v, &limits)?;
    }
    validate_integer_wire(v)?;
    validate_requires_wire(v)?;
    validate_common_cause_agency(&v["agency"])?;
    validate_actor_context_wire(v)?;
    if v.get("metrics").is_some() {
        validate_business_metrics_wire(v)?;
        check(v, &simulation_schema(), "simulation metrics wire")?;
        complete(v, &simulation_schema(), "simulation metrics wire")?;
        validate_scenario_wire(v)?;
        return validate_simulation_wire_inner(&without_metrics(v));
    }
    reject_unscoped_metric_assumptions(v)?;
    validate_responsibility_wire(v)?;
    check(v, &simulation_schema(), "simulation wire")?;
    complete(v, &simulation_schema(), "simulation wire")?;
    validate_scenario_wire(v)?;
    validate_completion_wire(v).map_err(|e| format!("simulation wire: {e}"))?;
    let fail = |s: &str| Err(format!("simulation wire: {s}"));
    let ledger = v["assumptions"].as_array().is_some_and(|rows| {
        rows.iter().any(|a| {
            a["field"] == "obligations.scope" && a["value"] == "per_case_response_instances"
        })
    });
    let mut expected_words = vec![];
    if ledger {
        expected_words.push("obligation_outcomes");
    }
    if v["bounds"]["cases"].is_u64() {
        expected_words.push("unreachable");
    }
    if validate_authority_record(v)? {
        expected_words.push("dynamic_authority_revocation");
    }
    if (expected_words.is_empty() && v.get("unchecked").is_some())
        || (!expected_words.is_empty()
            && v.get("unchecked") != Some(&serde_json::json!(expected_words)))
    {
        return fail(
            "obligation_outcomes requires the model obligation ledger and an exact unique word",
        );
    }
    if v["terminals"] != serde_json::json!([]) {
        return fail("top terminals must be empty");
    }
    let s = &v["simulation"];
    let done = s["status"] == "completed";
    let cases = v["case_outcomes"].as_array().unwrap();
    let properties = v["properties"].as_array().unwrap();
    // Authority records have their own closed validation above; their position
    // does not weaken the relative order of the pre-existing simulation rows.
    let assumptions: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| {
            !a["field"]
                .as_str()
                .is_some_and(|f| f.starts_with("authority."))
        })
        .cloned()
        .collect();
    if assumptions
        .iter()
        .any(|a| a["field"] == "probability.scheduler")
    {
        return fail("simulation has no probability scheduler");
    }
    let is_static = v["bounds"]["cases"].is_null();
    if is_static {
        if !cases.is_empty()
            || !properties.is_empty()
            || !v["agency"].is_null()
            || !v["admission"].is_null()
            || s["attempted_replications"] != 0
            || s["completed_replications"] != 0
            || !s["truncated_replications"].is_null()
            || ["cases", "live_tasks", "clock", "truncated"]
                .iter()
                .any(|k| !v["bounds"][k].is_null())
            || done
            || v["verdict"] != s["status"]
        {
            return fail("static failure leaked runtime results");
        }
        return Ok(());
    }
    let n = v["bounds"]["cases"].as_u64().unwrap();
    let r = s["replications"].as_u64().ok_or("missing R")?;
    // This exact pre-sampling structural refusal has lowered bounds, but no
    // sampled case population. It is not an ordinary runtime-abort exemption.
    if v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "U-SEM-0" && d["message"] == STRUCTURAL_SIM_REFUSAL)
    {
        if v.get("metrics").is_some()
            || waiting_declared(v)
            || !properties.is_empty()
            || !cases.is_empty()
            || s["attempted_replications"] != 0
            || s["completed_replications"] != 0
            || s["status"] != "unsupported"
            || v["verdict"] != "unsupported"
            || !(1..=10000).contains(&r)
            || n == 0
            || !v["bounds"]["truncated"].is_null()
            || !s["truncated_replications"].is_null()
            || !v["admission"].is_null()
            || assumptions.iter().any(|a| {
                a["field"]
                    .as_str()
                    .is_some_and(|f| f.starts_with("simulation."))
            })
        {
            return fail("structural refusal must have no sampled output or waiting");
        }
        return Ok(());
    }
    if !(1..=10000).contains(&r) || cases.len() as u64 != n || n == 0 {
        return fail("invalid replication or case population");
    }
    let attempts = s["attempted_replications"].as_u64().unwrap();
    let completed = s["completed_replications"].as_u64().unwrap();
    if done {
        let k = s["truncated_replications"]
            .as_u64()
            .ok_or("missing cut count")?;
        if attempts != r || completed != r || k > r || v["bounds"]["truncated"] != (k > 0) {
            return fail("completion/cut accounting");
        }
        if !["satisfied", "violated", "inconclusive"]
            .iter()
            .any(|x| v["verdict"] == *x)
        {
            return fail("completed simulation verdict");
        }
    } else if s["status"] != "unsupported"
        || (v["verdict"] != "unsupported"
            && !(v["verdict"] == "violated"
                && properties
                    .iter()
                    .any(|p| static_wire_property(v, &p["id"]) && p["verdict"] == "violated")))
        || attempts == 0
        || attempts > r
        || completed != attempts - 1
        || !v["bounds"]["truncated"].is_null()
        || !s["truncated_replications"].is_null()
        || !v["admission"].is_null()
    {
        return fail("abort must withdraw all partial aggregates");
    }
    let m = (properties
        .iter()
        .filter(|p| {
            p["id"]
                .as_str()
                .unwrap()
                .starts_with("probability reached #")
        })
        .count() as u64)
        .checked_mul(n)
        .ok_or("row count overflow")?;
    let mult = &s["multiplicity"];
    if mult["rows"] != m {
        return fail("Bonferroni rows must include all cases");
    }
    let (gn, gd) =
        fraction(s["confidence"].as_str().ok_or("missing confidence")?).ok_or("confidence")?;
    if gn.0 == [0] || gn == gd {
        return fail("confidence must be open interval");
    }
    let mut complement = gd.clone();
    complement.subtract(&gn);
    for (field, factor) in [("alpha_before", 2), ("alpha_after", 2 * m)] {
        if m == 0 {
            if !mult[field].is_null() {
                return fail("alpha must be null for m0");
            }
        } else {
            let (an, ad) = fraction(mult[field].as_str().ok_or("missing alpha")?).ok_or("alpha")?;
            let f = Decimal::parse(&factor.to_string()).unwrap();
            if an.multiply(&gd).multiply(&f) != complement.multiply(&ad) {
                return fail("incorrect exact alpha");
            }
        }
    }
    if assumptions.len() < 5 {
        return fail("five simulation assumptions required");
    }
    let tail = &assumptions[assumptions.len() - 5..];
    let values = [
        "sha256-counter-v1; category-bias<=1/18446744073709551616".to_owned(),
        "independent; unit=replication".to_owned(),
        format!(
            "bonferroni; rows={m}; alpha_before={}; alpha_after={}",
            mult["alpha_before"].as_str().unwrap_or("none"),
            mult["alpha_after"].as_str().unwrap_or("none")
        ),
        "reject-on-encounter".to_owned(),
        "estimation-not-proof".to_owned(),
    ];
    if assumptions[..assumptions.len() - 5]
        .iter()
        .any(|a| a["field"].as_str().unwrap().starts_with("simulation."))
    {
        return fail("duplicate or misplaced simulation assumption");
    }
    for i in 0..5 {
        let valid_affects = if i == 0 && v.get("scenario").is_some() {
            tail[i]["affects"].as_str().is_some_and(|s| {
                s.starts_with(SIM_AFFECTS[i]) && s.contains("scenario") && s != SIM_AFFECTS[i]
            })
        } else {
            tail[i]["affects"] == SIM_AFFECTS[i]
        };
        if tail[i]["field"] != SIM_FIELDS[i] || tail[i]["value"] != values[i] || !valid_affects {
            return fail("simulation assumption text or order");
        }
    }
    let mut old = v.clone();
    old["assumptions"] = Value::Array(assumptions[..assumptions.len() - 5].to_vec());
    validate_duration_assumptions(&old)?;
    let mut arrival_order = None;
    let limits = limits_declaration(&assumptions)?;
    validate_policy_rows(&assumptions[..assumptions.len() - 5])
        .map_err(|e| format!("simulation wire: {e}"))?;
    let mut rule_tail = false;
    for a in &assumptions[..assumptions.len() - 5] {
        let field = a["field"].as_str().unwrap();
        if rule_tail_assumption(field) {
            // ADR 0044: the rule tail follows every existing row, as in the exhaustive wire;
            // its inner order and Rule Actors are checked by validate_rule_tail_wire.
            rule_tail = true;
        } else if rule_tail {
            return fail("old assumption follows rule rows");
        } else if let Some(index) = arrival_rank(field) {
            if arrival_order.is_some_and(|previous| previous >= index)
                || (!(limits.new && (index == 5 || index == 6) || limits.wait_or_end && index == 1)
                    && old_arrival_affects(index).is_some_and(|text| a["affects"] != text))
            {
                return fail("arrival assumption order/text");
            }
            arrival_order = Some(index);
            match index {
                0 => {
                    if !arrival_value_matches(&a["value"], cases) {
                        return fail("arrivals disagree with cases");
                    }
                }
                1 if a["value"]
                    != if limits.wait_or_end {
                        "independent-per-arrival"
                    } else {
                        "independent-per-admission"
                    } =>
                {
                    return fail("case independence assumption");
                }
                _ => {}
            }
        } else if arrival_order.is_some()
            && !(field == "scenario" || field.starts_with("scenario."))
        {
            return fail("old assumption follows arrival rows");
        }
    }
    if n > 1 && !assumptions.iter().any(|a| a["field"] == "arrivals") {
        return fail("multi-case arrivals assumption missing");
    }
    let mut rejected = 0u128;
    for (i, c) in cases.iter().enumerate() {
        if c["case_id"] != i as u64 + 1
            || (i > 0 && c["arrival"].as_u64() < cases[i - 1]["arrival"].as_u64())
        {
            return fail("case IDs/order/arrivals");
        }
        let ts = c["terminals"].as_array().unwrap();
        if !done {
            if !ts.is_empty()
                || !c["unfinished_count"].is_null()
                || !c["deadlocked_count"].is_null()
            {
                return fail("partial terminal aggregates after abort");
            }
            continue;
        }
        let mut count = u128::from(c["unfinished_count"].as_u64().ok_or("unfinished count")?)
            + u128::from(c["deadlocked_count"].as_u64().ok_or("deadlocked count")?);
        count += u128::from(c["range_stopped_count"].as_u64().unwrap_or(0));
        count += u128::from(c["cut_off_count"].as_u64().unwrap_or(0));
        let mut seen = std::collections::BTreeSet::new();
        let mut reject_seen = false;
        for t in ts {
            let key = (t["kind"].as_str().unwrap(), t["end"].as_str());
            let amount = t["count"].as_u64().unwrap();
            if amount == 0 || !seen.insert(key) || reject_seen {
                return fail("terminal duplicate/zero/order");
            }
            count += u128::from(amount);
            if t["kind"] == "rejected" {
                if !t["end"].is_null() {
                    return fail("reject must have null end");
                }
                reject_seen = true;
                rejected += u128::from(amount);
            } else if !t["end"].is_string() {
                return fail("end must have declared name");
            }
        }
        if count != u128::from(r) {
            return fail("case counts do not sum to R");
        }
    }
    if done && v["admission"]["rejected_count"].as_u64().map(u128::from) != Some(rejected) {
        return fail("admission must sum rejection counts");
    }
    for p in properties {
        if static_wire_property(v, &p["id"]) {
            continue;
        }
        let global = p["id"] == "safety no_resource_overcommit"
            || p["id"] == "state_in_range"
            || conflict_property_id(&p["id"]);
        let query = p["id"]
            .as_str()
            .unwrap()
            .starts_with("probability reached #");
        let rows: Vec<&Value> = if global {
            vec![p]
        } else {
            let cs = p["cases"].as_array().ok_or("property cases required")?;
            if cs.len() != cases.len() {
                return fail("property all IDs required");
            }
            for (c, out) in cs.iter().zip(cases) {
                if c["case_id"] != out["case_id"] || c["arrival"] != out["arrival"] {
                    return fail("property ID/arrival mismatch");
                }
            }
            cs.iter().collect()
        };
        if global && p.get("cases").is_some() {
            return fail("safety is global");
        }
        let mut folded = "satisfied";
        let rank = |x: &str| {
            [
                "satisfied",
                "inconclusive",
                "unsupported",
                "violated",
                "invalid",
            ]
            .iter()
            .position(|v| *v == x)
            .unwrap_or(usize::MAX)
        };
        for row in rows {
            if row["verdict"] == "vacuous" {
                return fail("simulation never vacuous");
            }
            if !done
                && (row["verdict"] != "unsupported"
                    || row["basis"] != "sample"
                    || !row["witness"].is_null())
            {
                return fail("all abort rows must be unsupported samples");
            }
            if done && query {
                let h = row["hits"].as_u64().ok_or("missing hits")?;
                let u = row["unknown"].as_u64().ok_or("missing unknown")?;
                if row["basis"] != "statistical"
                    || row["trials"] != r
                    || h.checked_add(u).is_none_or(|x| x > r)
                {
                    return fail("statistical counts/basis");
                }
                for key in ["lower", "upper"] {
                    let (_, d) = fraction(row["interval"][key].as_str().unwrap()).unwrap();
                    let q = Decimal::parse("1048576").unwrap();
                    if q.remainder(&d).0 != [0] {
                        return fail("interval is not 2^-20 grid");
                    }
                }
            } else if row["basis"] == "witness" {
                let wr = row["witness"]["replication"]
                    .as_u64()
                    .ok_or("missing witness replication")?;
                if wr == 0 || wr > r {
                    return fail("witness out of range");
                }
            } else if row["basis"] != "sample" || !row["witness"].is_null() {
                return fail("invalid sample basis");
            }
            if done && !query {
                let reachable = p["id"].as_str().unwrap().starts_with("reachable end.");
                let expected = if row["basis"] == "witness" {
                    if reachable { "satisfied" } else { "violated" }
                } else {
                    "inconclusive"
                };
                if row["verdict"] != expected {
                    return fail("witness/sample verdict rule");
                }
            }
            let verdict = row["verdict"].as_str().unwrap();
            if rank(verdict) > rank(folded) {
                folded = verdict;
            }
        }
        if p["verdict"] != folded {
            return fail("property case verdict fold");
        }
    }
    if done && properties.is_empty() {
        return fail("no vacuous empty success");
    }
    let mut seen = std::collections::BTreeSet::new();
    for w in s["deadlock_witnesses"].as_array().unwrap() {
        let c = w["case_id"].as_u64().unwrap();
        let wr = w["replication"].as_u64().unwrap();
        if !done
            || c == 0
            || c > n
            || wr == 0
            || wr > r
            || !seen.insert(c)
            || w["time"].as_u64() > v["bounds"]["clock"].as_u64()
            || cases[(c - 1) as usize]["deadlocked_count"].as_u64() == Some(0)
        {
            return fail("deadlock witness identity/range");
        }
    }
    if done {
        for c in cases {
            if c["deadlocked_count"].as_u64().unwrap() > 0
                && !seen.contains(&c["case_id"].as_u64().unwrap())
            {
                return fail("deadlocked case needs earliest witness");
            }
        }
        let ds = v["diagnostics"].as_array().unwrap();
        let violated = properties.iter().any(|p| p["verdict"] == "violated")
            || ds.iter().any(|d| d["code"] == "W-DEADLOCK-0");
        let inconclusive = properties.iter().any(|p| p["verdict"] == "inconclusive")
            || ds.iter().any(|d| d["code"] == "W-BUDGET-0");
        let expected = if violated {
            "violated"
        } else if inconclusive {
            "inconclusive"
        } else {
            "satisfied"
        };
        if v["verdict"] != expected {
            return fail("root verdict fold");
        }
    } else if !v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "U-SEM-0" && d["span"].is_object())
    {
        return fail("runtime refusal needs located diagnostic");
    }
    Ok(())
}

fn validate_wire(actual: &Value) -> Result<(), String> {
    validate_deferred_quantity_positions(actual).map_err(|error| format!("wire: {error}"))?;
    validate_wire_inner(actual)
}
fn validate_wire_inner(actual: &Value) -> Result<(), String> {
    if waiting_declared(actual) {
        validate_waiting_wire(actual)?;
        return validate_wire_inner(&waiting_projection(actual));
    }
    reject_unscoped_waiting(actual)?;
    if let Some(q) = actual.get("quality") {
        validate_quality_wire(q, false)?;
    }
    if let Some(old) = without_quality(actual)? {
        return validate_wire_inner(&old);
    }
    validate_caseset_notes(actual)?;
    validate_integer_wire(actual).map_err(|e| format!("wire: {e}"))?;
    validate_common_cause_agency(&actual["agency"]).map_err(|e| format!("wire: {e}"))?;
    validate_actor_context_wire(actual).map_err(|error| format!("wire: {error}"))?;
    validate_obligations_wire(actual)?;
    validate_contest_runtime_wire(actual)?;
    if actual.get("metrics").is_some() {
        validate_business_metrics_wire(actual)?;
        check(actual, &report_schema(), "metrics wire")?;
        complete(actual, &report_schema(), "metrics wire")?;
        validate_scenario_wire(actual)?;
        let mut dto_shape = limits_dto_projection(actual);
        dto_shape.as_object_mut().unwrap().remove("unchecked");
        ledger_shape::dto_projection(&mut dto_shape)?;
        let typed: relca_core::report::Report =
            serde_json::from_value(dto_shape.clone()).map_err(|e| format!("metrics DTO: {e}"))?;
        if serde_json::to_value(typed).map_err(|e| e.to_string())? != dto_shape {
            return Err("metrics DTO did not round-trip losslessly".into());
        }
        return validate_wire_inner(&without_metrics(actual));
    }
    reject_unscoped_metric_assumptions(actual)?;
    validate_responsibility_wire(actual)?;
    // Independent closed schema is required even when serde's production DTO
    // ignores extra fields or supplies omitted Option fields as None.
    check(actual, &report_schema(), "$")
        .and_then(|()| complete(actual, &report_schema(), "$"))
        .map_err(|error| format!("wire: {error}"))?;
    validate_scenario_wire(actual)?;
    validate_completion_wire(actual).map_err(|e| format!("wire: {e}"))?;
    validate_requires_wire(actual).map_err(|e| format!("wire: {e}"))?;
    let probability_mode = actual["properties"].as_array().unwrap().iter().any(|p| {
        p["id"]
            .as_str()
            .unwrap()
            .starts_with("probability reached #")
    }) || actual["assumptions"].as_array().unwrap().iter().any(|a| {
        a["field"].as_str().unwrap().starts_with("kernel.")
            || a["value"].as_str().unwrap().starts_with("dist(")
    });
    for property in actual["properties"].as_array().unwrap() {
        let query = property_probability(property["id"].as_str().unwrap());
        if property.get("probability").is_some() != (query && actual.get("case_outcomes").is_none())
        {
            return Err("wire: probability is required only on probability properties".into());
        }
    }
    for terminal in actual["terminals"].as_array().unwrap() {
        if terminal.get("probability").is_some() != probability_mode {
            return Err("wire: terminal probability presence must match probability mode".into());
        }
    }
    validate_duration_assumptions(actual)?;
    validate_arrivals_wire(actual)?;
    // The optional ADR0025 member is checked by the closed schema and the
    // independent cross-field validator above, not by the production DTO. This
    // also lets pre-implementation wire controls run without requiring a new
    // Rust Report field merely to express the new JSON acceptance oracle.
    let mut dto_shape = limits_dto_projection(actual);
    dto_shape.as_object_mut().unwrap().remove("unchecked");
    ledger_shape::dto_projection(&mut dto_shape)?;
    let report: relca_core::report::Report =
        serde_json::from_value(dto_shape.clone()).map_err(|error| format!("wire: {error}"))?;
    // Deserialize alone accepts omitted Option fields as None and ignores
    // unknown keys. Lossless round-trip also requires every nullable field to
    // be present explicitly and rejects keys outside the ADR report shape.
    let canonical = serde_json::to_value(&report).map_err(|error| format!("wire: {error}"))?;
    if canonical != dto_shape {
        return Err("wire: missing nullable fields or unexpected report keys".into());
    }
    for diagnostic in &report.diagnostics {
        match diagnostic_severity(&diagnostic.code) {
            None => return Err(format!("wire: unknown diagnostic code {}", diagnostic.code)),
            Some(severity) if severity != diagnostic.severity => {
                return Err(format!(
                    "wire: {} must have severity {severity:?}",
                    diagnostic.code
                ));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// The independent closed table validates the ADR0039 additions even before
/// production DTOs support them. The lossless DTO round-trip still checks every
/// old field, after removing ONLY these explicitly enumerated additions.
fn limits_dto_projection(v: &Value) -> Value {
    let mut out = v.clone();
    // ADR0044 Rule U is now part of the production DTO and must round-trip too.
    if let Some(cases) = out.get_mut("case_outcomes").and_then(Value::as_array_mut) {
        for c in cases {
            c.as_object_mut().unwrap().remove("admitted_at");
            if let Some(ts) = c["terminals"].as_array_mut() {
                for t in ts {
                    if t["kind"] == "overflow" {
                        t["kind"] = Value::String("end".into());
                    }
                }
            }
        }
    }
    if let Some(a) = out.get_mut("admission").and_then(Value::as_object_mut) {
        a.remove("external_wait_max");
        a.remove("overflow_count");
    }
    if let Some(m) = out.get_mut("metrics") {
        if let Some(cases) = m["cases"].as_array_mut() {
            for c in cases {
                c.as_object_mut().unwrap().remove("external_wait");
                if let Some(counts) = c["counts"].as_object_mut() {
                    counts.remove("admitted");
                    counts.remove("overflowed");
                }
                if let Some(ts) = c["terminals"].as_array_mut() {
                    for t in ts {
                        if t["kind"] == "overflow" {
                            t["kind"] = Value::String("end".into());
                        }
                    }
                }
            }
        }
        if let Some(counts) = m["overall"]["counts"].as_object_mut() {
            counts.remove("admitted");
            counts.remove("overflowed");
        }
        if let Some(ts) = m["terminals"].as_array_mut() {
            for t in ts {
                t.as_object_mut().unwrap().remove("kind");
            }
        }
    }
    out
}

#[cfg(test)]
mod rule_dto_projection_tests {
    use super::*;

    #[test]
    fn limits_projection_preserves_the_rule_u_arm_for_production_dto() {
        let rule =
            serde_json::json!({"kind":"rule","actor":"R","version":"r1","reads":["R.required"]});
        let wire = serde_json::json!({"agency":{"viewpoints":{"effects":{"rows":[{"u":[rule]}]}}}});
        assert_eq!(limits_dto_projection(&wire), wire);
        let mut corrupt = wire.clone();
        corrupt["agency"]["viewpoints"]["effects"]["rows"][0]["u"][0]["version"] =
            serde_json::json!("");
        assert_eq!(
            limits_dto_projection(&corrupt),
            corrupt,
            "projection must not hide malformed Rule data either"
        );
    }
}

const DURATION_AFFECTS: &str = "duration is drawn from the declared distribution, independently of outcomes and of other durations; the distribution is an analyst assumption";

fn validate_duration_assumptions(actual: &Value) -> Result<(), String> {
    let rows = actual["assumptions"].as_array().unwrap();
    let mut duration_seen = false;
    let mut scheduler_seen = false;
    let mut fields = std::collections::BTreeSet::new();
    for row in rows {
        let field = row["field"].as_str().unwrap();
        if field == "probability.scheduler" {
            scheduler_seen = true;
        } else if field.starts_with("duration.") {
            if scheduler_seen || !fields.insert(field) {
                return Err(
                    "wire: duplicate duration assumption or duration after scheduler".into(),
                );
            }
            let parts: Vec<_> = field.split('.').collect();
            if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
                return Err("wire: duration field must name Actor and Capability".into());
            }
            if row["affects"] != DURATION_AFFECTS {
                return Err("wire: duration independence assumption text differs".into());
            }
            let text = row["value"].as_str().unwrap();
            let entries = text
                .strip_prefix("dist(")
                .and_then(|s| s.strip_suffix(')'))
                .ok_or("wire: duration value must be dist(...)")?;
            let mut previous = None;
            for entry in entries.split(", ") {
                let (time, probability) = entry
                    .split_once("min=")
                    .ok_or("wire: duration entry must be integer min=probability")?;
                let tick: u64 = time.parse().map_err(|_| "wire: invalid duration time")?;
                if tick.to_string() != time || previous.is_some_and(|p| p >= tick) {
                    return Err("wire: duration times must be canonical and increasing".into());
                }
                validate_probability(&serde_json::json!({"min":probability,"max":probability}))?;
                previous = Some(tick);
            }
            duration_seen = true;
        } else if duration_seen
            && arrival_rank(field).is_none()
            // ADR 0042: the scenario rows follow; their exact inventory and
            // order are checked by the scenario validator.
            && field != "scenario"
            && !field.starts_with("scenario.")
        {
            return Err(
                "wire: duration assumptions must follow state and kernel assumptions".into(),
            );
        }
    }
    Ok(())
}

// The old four field names, kept for the self-tests; order is `arrival_rank`.
#[allow(dead_code)]
const ARRIVAL_FIELDS: [&str; 4] = [
    "arrivals",
    "initial.case_independence",
    "policy.queue",
    "queue.capacity",
];
const ARRIVAL_AFFECTS: [&str; 4] = [
    "arrival times are fixed analyst assumptions; case IDs follow list order",
    "case initial distributions are drawn independently for each admitted case at receipt; this independence is an analyst assumption",
    "each start compares only the tasks queued within the Actor at that moment, by queue-entry time then case ID; tasks entering the queue later in the same instant are not compared; ties within one case remain nondeterministic",
    "admitted nonterminal cases with no task ever started are counted before same-time starts; rejected cases retain their probability mass",
];

// Fixed-policy rows, independently specified, not
// from implementation output. They are admitted only on a report that itself
// carries the extension (policy.classes, emitted only when `then call_order`
// or `assign first_idle` is written); every other report keeps the old FIFO
// row and text above.
const POLICY_QUEUE_AFFECTS: &str = "方策の区分の待ちを、待ちに入った時刻・案件ID・呼出しの字句の順で比較し、開始可能な最小の1件を1遷移で開始または拒否する。Actorと資源の待ちの先頭を追い越さず、人は取り置かない。通常の呼出しはprocess本体の前順、contestの合成呼出しはその後のcontest宣言順とする。環境とpartyにはActorごとのFIFOだけが掛かる。同じ時刻に後から待ちに入るTaskは比べない。";
const POLICY_ASSIGN_VALUE: &str = "first_idle(executors order)";
const POLICY_ASSIGN_AFFECTS: &str = "方策の区分のActorには、executorsの宣言順の最初の空いている人を割り当てる。占有は全Actor・全案件で共有し、PermissionとAuthorityでは人を選ばない。";
const POLICY_ASSIGN_SUFFIXES: [&str; 2] = [
    " 方策の区分にexecutorsを持つActorが無いため、この割当規則の適用先は無い。",
    " 選択processと宣言されたcontest routeに、方策の区分の実行者つき呼出しは無い。",
];
const POLICY_CLASSES_AFFECTS: &str = "方策が解くのはpolicy区分の宣言した開始順と割当てだけである。解かない選択は非決定のまま残る。方策はmodelの規則でActorとして切っておらず、Cutのinclude/excludeとSilent Cut検出の対象外である。安全性・契約・時間の結論は、この固定方策が生む経路と環境の全選択についてであり、別の方策の順序・割当てを保証しない。";
const POLICY_CLASSES_EMPTY_SUFFIX: &str = " policy区分のActorが無いため、延長の適用先は無い。";
const POLICY_CLASSES_SUFFIXES: [&str; 2] = [
    " 選択processと宣言されたcontest routeにpolicy区分の呼出しは無い。",
    " 方策の区分で同時に待つ呼出しは高々1件であり、開始順の取り合いは無い。",
];
const POLICY_SCHEDULE_AFFECTS: &str = "宣言した固定方策で解けない開始・割当て、区分をまたぐ競合、終了・採用・開始の同時刻の順は非決定のまま全て展開する。即時遷移が尽きるまで時刻を進めない。";
const POLICY_FIXED_AFFECTS: &str = "方策の区分の選択は宣言した固定方策で決まる。min/maxは残りの環境の選択の全ての解き方での幅であり、環境は全状態を見られる。方策の最適値ではない。";

/// Closed order of the arrival-family rows: the old four, with the two
/// extension rows between policy.queue and queue.capacity.
fn arrival_rank(field: &str) -> Option<usize> {
    match field {
        "arrivals" => Some(0),
        "initial.case_independence" => Some(1),
        "policy.queue" => Some(2),
        "policy.assign" => Some(3),
        "policy.classes" => Some(4),
        "queue.capacity" => Some(5),
        "concurrent.cases" => Some(6),
        _ => None,
    }
}

/// Old affects text for the old arrival rows; the policy rows are checked by
/// `validate_policy_rows`.
fn old_arrival_affects(rank: usize) -> Option<&'static str> {
    match rank {
        0 => Some(ARRIVAL_AFFECTS[0]),
        1 => Some(ARRIVAL_AFFECTS[1]),
        5 => Some(ARRIVAL_AFFECTS[3]),
        _ => None,
    }
}

/// The text is the base, or the base followed by exactly one declared suffix.
fn base_with_suffix(text: &Value, base: &str, suffixes: &[&str]) -> bool {
    text.as_str().is_some_and(|t| {
        t.strip_prefix(base)
            .is_some_and(|rest| rest.is_empty() || suffixes.contains(&rest))
    })
}

/// Validates the policy rows of one assumptions array and returns whether the
/// report is extended. A legacy (or observable-only) report admits only the
/// old policy.queue row with the old value and text.
fn validate_policy_rows(rows: &[Value]) -> Result<bool, String> {
    let fail = |text: &str| Err(format!("wire policy: {text}"));
    let only = |field: &str| -> Result<Option<&Value>, String> {
        let mut found = rows.iter().filter(|r| r["field"] == field);
        let first = found.next();
        if found.next().is_some() {
            return Err(format!("wire policy: duplicate {field}"));
        }
        Ok(first)
    };
    let queue = only("policy.queue")?;
    let assign = only("policy.assign")?;
    let classes = only("policy.classes")?;
    let extended = classes.is_some();
    match queue {
        Some(q) => {
            let value = q["value"].as_str().ok_or("wire policy: queue value")?;
            let body = value.strip_prefix("observable ").unwrap_or(value);
            let call_order = match body {
                "fifo tie_by case_id" => false,
                "fifo tie_by case_id then call_order" => true,
                _ => return fail("queue value"),
            };
            if call_order && !extended {
                return fail("then call_order on a report without policy.classes");
            }
            if extended && !call_order && assign.is_none() {
                return fail("policy.classes without then call_order or policy.assign");
            }
            let affects = if call_order {
                POLICY_QUEUE_AFFECTS
            } else {
                ARRIVAL_AFFECTS[2]
            };
            if q["affects"] != affects {
                return fail("queue affects text");
            }
        }
        None if extended || assign.is_some() => return fail("policy rows without policy.queue"),
        None => {}
    }
    if let Some(a) = assign {
        if !extended {
            return fail("policy.assign without policy.classes");
        }
        if a["value"] != POLICY_ASSIGN_VALUE {
            return fail("assign value");
        }
        if !base_with_suffix(
            &a["affects"],
            POLICY_ASSIGN_AFFECTS,
            &POLICY_ASSIGN_SUFFIXES,
        ) {
            return fail("assign affects text");
        }
    }
    if let Some(c) = classes {
        let value = c["value"].as_str().ok_or("wire policy: classes value")?;
        let parts: Vec<_> = value.split("; ").collect();
        if parts.len() != 3 {
            return fail("classes value shape");
        }
        let mut names = std::collections::BTreeSet::new();
        let mut policy_empty = false;
        for (part, label) in parts.iter().zip(["policy", "environment", "party"]) {
            let inner = part
                .strip_prefix(label)
                .and_then(|s| s.strip_prefix('['))
                .and_then(|s| s.strip_suffix(']'))
                .ok_or("wire policy: classes value shape")?;
            if label == "policy" {
                policy_empty = inner.is_empty();
            }
            if inner.is_empty() {
                continue;
            }
            for name in inner.split(", ") {
                if name.is_empty()
                    || !name.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
                    || !names.insert(name)
                {
                    return fail("classes Actor names");
                }
            }
        }
        let affects = c["affects"]
            .as_str()
            .ok_or("wire policy: classes affects")?;
        let rest = affects
            .strip_prefix(POLICY_CLASSES_AFFECTS)
            .ok_or("wire policy: classes affects text")?;
        let ok = if policy_empty {
            rest == POLICY_CLASSES_EMPTY_SUFFIX
        } else {
            rest.is_empty() || POLICY_CLASSES_SUFFIXES.contains(&rest)
        };
        if !ok {
            return fail("classes affects text");
        }
    }
    for row in rows {
        if row["field"] == "schedule" && (row["affects"] == POLICY_SCHEDULE_AFFECTS) != extended {
            return fail("schedule affects text");
        }
        if row["field"] == "probability.scheduler"
            && (row["value"] == "fixed-policy" || row["affects"] == POLICY_FIXED_AFFECTS)
            && (!extended
                || row["value"] != "fixed-policy"
                || row["affects"] != POLICY_FIXED_AFFECTS)
        {
            return fail("fixed-policy scheduler row");
        }
    }
    Ok(extended)
}

#[derive(Default)]
struct LimitsDeclaration {
    new: bool,
    wait: bool,
    wait_or_end: bool,
    overflow: Option<String>,
}

const LIMITS_RULES_AFFECTS: &str = "Admission limits and overflow handling are model rules, not an Actor. Counts are tested before same-time starts and finishes. Concurrent cases stop counting at the business end, not contest closure.";
const LIMITS_WAIT_AFFECTS: &str = "Receipt and initial values occur at arrival; admission occurs when both limits have room. External FIFO may reuse slots released at the same time after immediate events settle; new arrivals do not. External wait is included in elapsed, within and receipt-based response deadlines. first_start_wait remains arrival-based and overlaps external_wait; do not add them.";

fn limits_declaration(rows: &[Value]) -> Result<LimitsDeclaration, String> {
    let mut limits = LimitsDeclaration::default();
    let mut action: Option<String> = None;
    for row in rows.iter().filter(|r| {
        matches!(
            r["field"].as_str(),
            Some("queue.capacity" | "concurrent.cases")
        )
    }) {
        let text = row["value"].as_str().ok_or("limit value")?;
        let (number, treatment) = text
            .split_once(" on_overflow ")
            .ok_or("limit action required")?;
        let n: u64 = number.parse().map_err(|_| "limit integer")?;
        if n.to_string() != number {
            return Err("limit noncanonical integer".into());
        }
        if let Some(a) = &action
            && a != treatment
        {
            return Err("limit actions disagree".into());
        }
        action = Some(treatment.to_owned());
        match treatment {
            "reject" => {}
            "wait_external fifo" => {
                if n == 0 {
                    return Err("zero wait limit".into());
                }
                limits.new = true;
                limits.wait = true;
                limits.wait_or_end = true;
            }
            end if end.starts_with("end ") => {
                let name = &end[4..];
                if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    return Err("invalid overflow end".into());
                }
                limits.new = true;
                limits.wait_or_end = true;
                limits.overflow = Some(name.to_owned());
            }
            _ => return Err("unknown overflow action".into()),
        }
        limits.new |= row["field"] == "concurrent.cases";
        if treatment != "reject" || row["field"] == "concurrent.cases" {
            let affects = row["affects"].as_str().ok_or("limit affects required")?;
            if !affects.contains(LIMITS_RULES_AFFECTS)
                || treatment == "wait_external fifo" && !affects.contains(LIMITS_WAIT_AFFECTS)
            {
                return Err("limit assumptions omit frozen semantic boundary text".into());
            }
        }
    }
    Ok(limits)
}

/// Closed addition to the ADR0015/0019 table. Old reports cannot opt in merely
/// by inventing a new output field; the declared assumption action is required.
fn validate_limits_fields(v: &Value, limits: &LimitsDeclaration) -> Result<(), String> {
    if let Some(a) = v.get("admission")
        && !a.is_null()
    {
        if a.get("external_wait_max").is_some() != limits.wait
            || a.get("overflow_count").is_some() != limits.overflow.is_some()
        {
            return Err("limit summary key presence differs from declaration".into());
        }
        for key in ["external_wait_max", "overflow_count"] {
            if let Some(r) = a.get(key)
                && (r["min"].as_u64() > r["max"].as_u64()
                    || r["max"].as_u64() > v["bounds"]["cases"].as_u64())
            {
                return Err("limit summary inverted/outside population".into());
            }
        }
    }
    if let Some(cases) = v["case_outcomes"].as_array() {
        for c in cases {
            if v["mode"] != "simulation" && c.get("admitted_at").is_some() != limits.wait {
                return Err("admitted_at presence differs from external-wait declaration".into());
            }
            if let Some(at) = c.get("admitted_at")
                && !at.is_null()
                && (at["min"].as_u64() < c["arrival"].as_u64()
                    || at["min"].as_u64() > at["max"].as_u64())
            {
                return Err("admitted_at before receipt/inverted".into());
            }
            if let Some(ts) = c["terminals"].as_array() {
                for t in ts {
                    if t["kind"] == "overflow" {
                        if t["end"].as_str() != limits.overflow.as_deref() {
                            return Err("undeclared overflow terminal".into());
                        }
                        if v["mode"] != "simulation"
                            && (t["completed_at"]
                                != serde_json::json!({"min":c["arrival"],"max":c["arrival"]})
                                || t["elapsed"] != serde_json::json!({"min":0,"max":0})
                                || t["work"] != serde_json::json!({"min":0,"max":0})
                                || !t["result_at"].is_null())
                        {
                            return Err(
                                "overflow must be immediate without task work or result".into()
                            );
                        }
                    }
                }
            }
        }
    }
    if let Some(m) = v.get("metrics") {
        let mut counts = vec![&m["overall"]["counts"]];
        if let Some(cases) = m["cases"].as_array() {
            for c in cases {
                if c.get("external_wait").is_some() != limits.wait {
                    return Err("external_wait presence differs from declaration".into());
                }
                counts.push(&c["counts"]);
                if let Some(ts) = c["terminals"].as_array() {
                    for t in ts {
                        if t["kind"] == "overflow"
                            && t["end"].as_str() != limits.overflow.as_deref()
                        {
                            return Err("metric overflow identity".into());
                        }
                    }
                }
            }
        }
        for c in counts {
            if c.get("admitted").is_some() != limits.new
                || c.get("overflowed").is_some() != limits.overflow.is_some()
            {
                return Err("metric count key presence differs from limit declaration".into());
            }
        }
        if let Some(ts) = m["terminals"].as_array() {
            for t in ts {
                if t.get("kind").is_some() != limits.overflow.is_some() {
                    return Err("global metric terminal kind presence".into());
                }
                if limits.overflow.is_some()
                    && (t["kind"] == "overflow")
                        != (t["end"].as_str() == limits.overflow.as_deref())
                {
                    return Err("global metric overflow kind identity".into());
                }
            }
        }
    }
    Ok(())
}

/// ADR 0041 round 9: canonical pins are independent test-IR derived; inline
/// pairs are hand-calculated. Both are bound to frozen source hashes/ordinals.
/// This validates the new rows in place; it does NOT normalize them away.
fn validate_read_write_assumptions(v: &Value) -> Result<(), String> {
    const FIELD: &str = "concurrency.read_write";
    let rows = v["assumptions"].as_array().unwrap();
    validate_read_write_excluded(rows)?;
    let start = rows.iter().position(|r| r["field"] == FIELD);
    static HAND: std::sync::OnceLock<Vec<Value>> = std::sync::OnceLock::new();
    let hand = HAND.get_or_init(|| {
        serde_json::from_str(include_str!("../tests/read_write_wire_expectations.json")).unwrap()
    });
    let source_registered = hand
        .iter()
        .any(|pin| pin["source_sha256"] == v["model"]["sha256"]);
    if start.is_none() {
        if source_registered {
            return Err("wire read_write: missing registered candidates".into());
        }
        return validate_read_write_pin_unselected(v);
    }
    let start = start.unwrap();
    let registered = hand.iter().any(|pin| {
        pin["source_sha256"] == v["model"]["sha256"]
            && pin["index"].as_u64() == Some(start as u64)
            && pin["rows"].as_array().is_some_and(|expected| {
                rows.get(start..start + expected.len()) == Some(expected.as_slice())
                    && rows.iter().filter(|r| r["field"] == FIELD).count() == expected.len()
            })
    });
    if !registered {
        return Err(
            "wire read_write: payload, source, position or order differs from hand table".into(),
        );
    }
    Ok(())
}

fn read_write_selection_pins() -> &'static [Value] {
    static PINS: std::sync::OnceLock<Vec<Value>> = std::sync::OnceLock::new();
    PINS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../tests/read_write_selection_expectations.json"
        ))
        .unwrap()
    })
}

fn validate_read_write_pin_rows(v: &Value, pins: &[&Value]) -> Result<(), String> {
    let Some(rows) = v["assumptions"].as_array() else {
        return Ok(());
    };
    let candidates: Vec<_> = rows
        .iter()
        .filter(|r| r["field"] == "concurrency.read_write")
        .cloned()
        .collect();
    let start = rows
        .iter()
        .position(|r| r["field"] == "concurrency.read_write");
    if pins.iter().any(|pin| {
        pin["rows"] == serde_json::json!(candidates)
            && (candidates.is_empty() || pin["index"].as_u64() == start.map(|i| i as u64))
    }) {
        return Ok(());
    }
    Err(
        "wire read_write: candidate set differs from source/configuration/selected-process pin"
            .into(),
    )
}

fn validate_read_write_pin_unselected(v: &Value) -> Result<(), String> {
    let pins: Vec<_> = read_write_selection_pins()
        .iter()
        .filter(|p| p["source_sha256"] == v["model"]["sha256"] && p["mode"] == v["mode"])
        .collect();
    if pins.is_empty() {
        return Ok(());
    }
    validate_read_write_pin_rows(v, &pins)
}

fn validate_read_write_pin(v: &Value, configuration: Option<&str>) -> Result<(), String> {
    let source: Vec<_> = read_write_selection_pins()
        .iter()
        .filter(|p| p["source_sha256"] == v["model"]["sha256"] && p["mode"] == v["mode"])
        .collect();
    if source.is_empty() {
        return Ok(());
    }
    let pins: Vec<_> = source
        .into_iter()
        .filter(|p| {
            p["configuration"].as_str() == configuration
                && p["process"].as_str().is_some_and(|s| !s.is_empty())
        })
        .collect();
    // Configuration fixes the selected process in this independently authored
    // table; its complete rows include the exact process name/If byte positions.
    if pins.is_empty() {
        return Err("wire read_write: unregistered configuration/selected process".into());
    }
    validate_read_write_pin_rows(v, &pins)
}

fn validate_read_write_excluded(rows: &[Value]) -> Result<(), String> {
    let excluded: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r["field"] == "concurrency.read_write_excluded")
        .collect();
    if excluded.is_empty() {
        return Ok(());
    }
    let fail = || Err("wire read_write: excluded disclosure shape/count/order".into());
    if excluded.len() != 1 {
        return fail();
    }
    let (at, row) = excluded[0];
    let Some(value) = row["value"].as_str() else {
        return fail();
    };
    let Some((single, local)) = value
        .strip_prefix("single_case=")
        .and_then(|s| s.split_once("; case_local="))
    else {
        return fail();
    };
    let parse = |s: &str| s.parse::<u64>().ok().filter(|n| n.to_string() == s);
    let (Some(single), Some(local)) = (parse(single), parse(local)) else {
        return fail();
    };
    if (single == 0 && local == 0)
        || row["affects"]
            != "一覧の対象外の静的な読取り/書込み対の種類と数。単一案件のfork兄弟もsingle_caseに含む。到達や同時実行を確定しない。潜在の読取りは数えない。候補の不在は競合なしの証明ではない。"
        || rows[at + 1..]
            .iter()
            .any(|r| r["field"] == "concurrency.read_write")
    {
        return fail();
    }
    Ok(())
}

/// Independent ADR 0015 closed wire constraints, not production DTO validation.
fn validate_arrivals_wire(v: &Value) -> Result<(), String> {
    validate_read_write_assumptions(v)?;
    let fail = |text: &str| Err(format!("wire arrivals: {text}"));
    let rows = v["assumptions"].as_array().unwrap();
    let limits = limits_declaration(rows)?;
    validate_limits_fields(v, &limits)?;
    let Some(cases) = v.get("case_outcomes") else {
        if v.get("admission").is_some()
            || rows
                .iter()
                .any(|r| arrival_rank(r["field"].as_str().unwrap()).is_some())
            || v["properties"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p.get("cases").is_some())
            || v["agency"]["unchecked"]
                .as_array()
                .is_some_and(|u| u.iter().any(|x| x == "multi_case_interference"))
        {
            return fail("new fields on legacy or static report");
        }
        return Ok(());
    };
    if v["terminals"] != serde_json::json!([]) {
        return fail("top-level terminals must be empty");
    }
    let n = v["bounds"]["cases"]
        .as_u64()
        .ok_or("wire arrivals: unknown case count")?;
    if n == 0 {
        return fail("case count must be positive");
    }
    let cases = cases.as_array().unwrap();
    if cases.len() as u64 != n {
        return fail("all case IDs are required");
    }
    let probability_mode = v["properties"].as_array().unwrap().iter().any(|p| {
        p["id"]
            .as_str()
            .unwrap()
            .starts_with("probability reached #")
    }) || rows.iter().any(|r| {
        r["field"].as_str().unwrap().starts_with("kernel.")
            || r["value"].as_str().unwrap().starts_with("dist(")
    });
    for (i, case) in cases.iter().enumerate() {
        if case["case_id"].as_u64() != Some(i as u64 + 1) {
            return fail("case IDs must be complete, unique, ascending");
        }
        if i > 0 && case["arrival"].as_u64() < cases[i - 1]["arrival"].as_u64() {
            return fail("arrival order differs");
        }
        let mut keys = std::collections::BTreeSet::new();
        let mut rejected = false;
        for t in case["terminals"].as_array().unwrap() {
            let kind = t["kind"].as_str().unwrap();
            let end = t["end"].as_str();
            if !keys.insert((kind, end)) {
                return fail("duplicate (kind,end)");
            }
            if rejected {
                return fail("rejected must be last");
            }
            if t.get("probability").is_some() != (probability_mode || kind == "cut_off") {
                return fail("terminal probability presence");
            }
            if kind == "rejected" {
                rejected = true;
                if !t["end"].is_null()
                    || ["completed_at", "elapsed", "work", "result_at"]
                        .iter()
                        .any(|k| !t[k].is_null())
                {
                    return fail("reject end and all four metrics must be null");
                }
            } else if kind != "cut_off" {
                if end.is_none() {
                    return fail("declared end name required");
                }
                for key in ["completed_at", "elapsed", "work"] {
                    if t[key].is_null() || t[key]["min"].as_u64() > t[key]["max"].as_u64() {
                        return fail("end metrics missing or inverted");
                    }
                }
                for bound in ["min", "max"] {
                    if t["elapsed"][bound]
                        .as_u64()
                        .and_then(|e| e.checked_add(case["arrival"].as_u64().unwrap()))
                        != t["completed_at"][bound].as_u64()
                    {
                        return fail("absolute and relative metrics disagree");
                    }
                }
            }
            if !t["result_at"].is_null()
                && t["result_at"]["min"].as_u64() > t["result_at"]["max"].as_u64()
            {
                return fail("result_at inverted");
            }
        }
    }
    let range = &v["admission"]["rejected_count"];
    let lo = range["min"]
        .as_u64()
        .ok_or("wire arrivals: admission required")?;
    let hi = range["max"]
        .as_u64()
        .ok_or("wire arrivals: admission required")?;
    if lo > hi || hi > n {
        return fail("rejected count range invalid");
    }
    for p in v["properties"].as_array().unwrap() {
        if p["id"] == "safety no_resource_overcommit"
            || conflict_property_id(&p["id"])
            || static_wire_property(v, &p["id"])
        {
            if p.get("cases").is_some() {
                return fail("safety is global");
            }
            continue;
        }
        let ps = p["cases"]
            .as_array()
            .ok_or("wire arrivals: property cases required")?;
        if ps.len() != cases.len() {
            return fail("property case IDs incomplete");
        }
        let query = property_probability(p["id"].as_str().unwrap());
        for (c, outcome) in ps.iter().zip(cases) {
            if c["case_id"] != outcome["case_id"] || c["arrival"] != outcome["arrival"] {
                return fail("property ID or arrival mismatch");
            }
            if c.get("probability").is_some() != query {
                return fail("case query probability presence");
            }
        }
        let order = [
            "satisfied",
            "vacuous",
            "inconclusive",
            "unsupported",
            "violated",
            "invalid",
        ];
        let worst = ps
            .iter()
            .map(|c| order.iter().position(|x| c["verdict"] == *x).unwrap())
            .max()
            .unwrap_or(0);
        if p["verdict"] != order[worst] {
            return fail("case verdict fold differs");
        }
    }
    let mut previous = None;
    let mut new_seen = false;
    let mut scheduler = false;
    let mut rule_tail = false;
    validate_policy_rows(rows)?;
    for row in rows {
        let field = row["field"].as_str().unwrap();
        if field == "concurrency.read_write" || field == "concurrency.read_write_excluded" {
            // Candidate payloads use source pins; excluded disclosure has the
            // separately closed count grammar above. Unknown
            // fields still take the pre-existing refusal below.
            continue;
        }
        if let Some(index) = arrival_rank(field) {
            if scheduler || rule_tail || previous.is_some_and(|p| p >= index) {
                return fail("new assumption order or duplicate");
            }
            previous = Some(index);
            new_seen = true;
            if !(limits.new && (index == 5 || index == 6) || limits.wait_or_end && index == 1)
                && old_arrival_affects(index).is_some_and(|text| row["affects"] != text)
            {
                return fail("new assumption affects text");
            }
            match index {
                0 => {
                    if !arrival_value_matches(&row["value"], cases) {
                        return fail("arrival assumption disagrees with cases");
                    }
                }
                1 if row["value"]
                    != if limits.wait_or_end {
                        "independent-per-arrival"
                    } else {
                        "independent-per-admission"
                    } =>
                {
                    return fail("initial independence value");
                }
                5 if !limits.new => {
                    let text = row["value"].as_str().unwrap();
                    let number = text
                        .strip_suffix(" on_overflow reject")
                        .ok_or("wire arrivals: queue value")?;
                    let count: u64 = number.parse().map_err(|_| "wire arrivals: queue integer")?;
                    if count.to_string() != number {
                        return fail("queue value not canonical");
                    }
                }
                _ => {}
            }
        } else if field == "probability.scheduler" {
            scheduler = true;
        } else if !scheduler && rule_tail_assumption(field) {
            // Rule/amendment and unused rule-change grant rows
            // follow the existing rows and precede the scheduler row.
            rule_tail = true;
        } else if rule_tail || new_seen && !(field == "scenario" || field.starts_with("scenario."))
        {
            return fail("old assumption after new assumptions");
        }
    }
    if !rows.iter().any(|r| r["field"] == "arrivals") {
        return fail("arrivals assumption required");
    }
    let initial_dist = rows.iter().any(|r| {
        r["field"].as_str().unwrap().starts_with("latent.")
            && r["value"].as_str().unwrap().starts_with("dist(")
    });
    if rows
        .iter()
        .any(|r| r["field"] == "initial.case_independence")
        != initial_dist
    {
        return fail("case independence assumption presence");
    }
    let unchecked = v["agency"]["unchecked"]
        .as_array()
        .ok_or("wire arrivals: agency required")?;
    let count = unchecked
        .iter()
        .filter(|x| *x == "multi_case_interference")
        .count();
    // ADR 0042 integration (a): report.rs apply_outcome appends
    // obligation_debtor after lower's trailing markers exactly when the report
    // carries obligation rows (the pre-B5 order). ADR 0043 (plan C04) makes
    // lower append responsibility_people_undeclared after its existing
    // markers when the model declares no people; it stays a lower marker, so
    // it precedes obligation_debtor (ADR 0043 integration (c)). Closed: no
    // other marker may follow, and never without ledger rows.
    let ledger = v
        .get("obligations")
        .and_then(Value::as_array)
        .is_some_and(|rows| !rows.is_empty());
    let people = unchecked
        .iter()
        .filter(|x| *x == "responsibility_people_undeclared")
        .count();
    if people > 1 {
        return fail("Agency unchecked marker");
    }
    let mut tail: Vec<&str> = Vec::new();
    if n > 1 {
        tail.push("multi_case_interference");
    }
    if people == 1 {
        tail.push("responsibility_people_undeclared");
    }
    if ledger {
        tail.push("obligation_debtor");
    }
    let ends_with_tail = unchecked.len() >= tail.len()
        && unchecked[unchecked.len() - tail.len()..]
            .iter()
            .zip(&tail)
            .all(|(x, t)| x == t);
    if (n > 1 && (count != 1 || !ends_with_tail)) || (n == 1 && count != 0) {
        return fail("Agency unchecked marker");
    }
    // Post-integration boundary review m1: obligation_debtor is pushed once,
    // last, and only with ledger rows, whatever the case count. Closed: never
    // twice, never before another marker, never without rows.
    let debtor = unchecked
        .iter()
        .filter(|x| *x == "obligation_debtor")
        .count();
    if debtor != usize::from(ledger)
        || (ledger && unchecked.last().is_none_or(|x| x != "obligation_debtor"))
        || !ends_with_tail
    {
        return fail("Agency unchecked marker");
    }
    Ok(())
}

// Decimal naturals, big-endian; schema checking has no machine-integer ceiling
// and shares neither the production Rational type nor its normalization.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Decimal(Vec<u8>);
impl Decimal {
    fn parse(s: &str) -> Option<Self> {
        if s.is_empty()
            || !s.bytes().all(|b| b.is_ascii_digit())
            || (s.len() > 1 && s.starts_with('0'))
        {
            return None;
        }
        Some(Self(s.bytes().map(|b| b - b'0').collect()))
    }
    fn trim(&mut self) {
        let n = self
            .0
            .iter()
            .position(|&d| d != 0)
            .unwrap_or(self.0.len() - 1);
        self.0.drain(..n);
    }
    fn cmp_num(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.cmp(&other.0))
    }
    fn subtract(&mut self, other: &Self) {
        let mut borrow = 0i16;
        for i in 0..self.0.len() {
            let j = self.0.len() - 1 - i;
            let rhs = if i < other.0.len() {
                other.0[other.0.len() - 1 - i] as i16
            } else {
                0
            };
            let d = self.0[j] as i16 - rhs - borrow;
            self.0[j] = d.rem_euclid(10) as u8;
            borrow = i16::from(d < 0);
        }
        self.trim();
    }
    fn remainder(&self, divisor: &Self) -> Self {
        let mut r = Self(vec![0]);
        for d in &self.0 {
            r.0.push(*d);
            r.trim();
            while !r.cmp_num(divisor).is_lt() {
                r.subtract(divisor);
            }
        }
        r
    }
    fn multiply(&self, other: &Self) -> Self {
        let mut result = vec![0u32; self.0.len() + other.0.len()];
        for (i, a) in self.0.iter().rev().enumerate() {
            for (j, b) in other.0.iter().rev().enumerate() {
                result[i + j] += u32::from(*a) * u32::from(*b);
            }
        }
        for i in 0..result.len() - 1 {
            let carry = result[i] / 10;
            result[i] %= 10;
            result[i + 1] += carry;
        }
        let mut n = Self(result.into_iter().rev().map(|d| d as u8).collect());
        n.trim();
        n
    }
}
fn fraction(s: &str) -> Option<(Decimal, Decimal)> {
    let one = Decimal(vec![1]);
    if s == "0" || s == "1" {
        return Some((Decimal::parse(s)?, one));
    }
    let (n, d) = s.split_once('/')?;
    let n = Decimal::parse(n)?;
    let d = Decimal::parse(d)?;
    if n.0 == [0] || !n.cmp_num(&d).is_lt() {
        return None;
    }
    let (mut a, mut b) = (n.clone(), d.clone());
    while b.0 != [0] {
        let r = a.remainder(&b);
        a = b;
        b = r;
    }
    (a == one).then_some((n, d))
}
fn validate_probability(v: &Value) -> Result<(), String> {
    let o = v
        .as_object()
        .ok_or("probability must be an object, not null")?;
    if o.len() != 2 || !o.contains_key("min") || !o.contains_key("max") {
        return Err("probability needs exactly min and max".into());
    }
    let parse = |key| {
        v[key]
            .as_str()
            .and_then(fraction)
            .ok_or_else(|| format!("{key} must be a canonical probability string"))
    };
    let (ln, ld) = parse("min")?;
    let (hn, hd) = parse("max")?;
    if ln.multiply(&hd).cmp_num(&hn.multiply(&ld)).is_gt() {
        return Err("probability min exceeds max".into());
    }
    Ok(())
}

fn compare(expected: &Value, actual: &Value, path: &str) -> Result<(), String> {
    match (expected, actual) {
        (Value::Array(expected), Value::Array(actual))
            if path.contains(".contest_runtime[") && path.ends_with(".alternatives") =>
        {
            if expected.len() != actual.len() {
                return Err(format!("{path}: alternative cardinality"));
            }
            let mut used = std::collections::BTreeSet::new();
            for e in expected {
                let index = actual
                    .iter()
                    .enumerate()
                    .position(|(i, a)| {
                        !used.contains(&i) && compare(e, a, &format!("{path}[]")).is_ok()
                    })
                    .ok_or_else(|| format!("{path}: missing alternative {e}"))?;
                used.insert(index);
            }
            Ok(())
        }
        (Value::Array(expected), Value::Array(actual))
            if path.contains(".obligations[") && path.ends_with(".outcomes") =>
        {
            // These are possible outcomes, not DFS order or path counts.
            if expected.len() != actual.len() {
                return Err(format!("{path}: outcome cardinality"));
            }
            let mut expected = expected.iter().collect::<Vec<_>>();
            let mut actual = actual.iter().collect::<Vec<_>>();
            expected.sort_by_key(|o| o.to_string());
            actual.sort_by_key(|o| o.to_string());
            for (e, a) in expected.into_iter().zip(actual) {
                compare(e, a, &format!("{path}[]"))?;
            }
            Ok(())
        }
        (Value::Array(expected), Value::Array(actual))
            if path.contains("metrics") && path.ends_with(".terminals") =>
        {
            let fields = if path.contains(".cases[") {
                &["kind", "end"][..]
            } else {
                &["end"][..]
            };
            let expected = unique_composite(expected, fields)?;
            let actual = unique_composite(actual, fields)?;
            if expected.keys().ne(actual.keys()) {
                return Err(format!("{path}: metric terminal identities differ"));
            }
            for (k, v) in expected {
                compare(v, actual[&k], &format!("{path}[{k}]"))?;
            }
            Ok(())
        }
        (Value::Array(expected), Value::Array(actual))
            if path == "$.tables.metrics" || path == "$.metrics" =>
        {
            if expected.len() != actual.len() {
                return Err(format!("{path}: metric table length"));
            }
            for (i, (e, a)) in expected.iter().zip(actual).enumerate() {
                if e["key"] != a["key"] {
                    return Err(format!("{path}[{i}]: metric key/order"));
                }
                compare(e, a, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        (Value::Array(expected), Value::Array(actual))
            if path == "$.tables.properties" || path == "$.tables.terminals" =>
        {
            let fields = if path.ends_with("properties") {
                &["id", "case_id"][..]
            } else {
                &["case_id", "kind", "end"][..]
            };
            let expected = unique_composite(expected, fields)?;
            let actual = unique_composite(actual, fields)?;
            if expected.keys().ne(actual.keys()) {
                return Err(format!("{path}: composite keys differ"));
            }
            for (key, value) in expected {
                compare(value, actual[&key], &format!("{path}[{key}]"))?;
            }
            Ok(())
        }
        (Value::Object(expected), Value::Object(actual)) => {
            for (key, value) in expected {
                if key == "absent_diagnostics" {
                    for code in value.as_array().unwrap() {
                        if actual["diagnostics"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|d| d["code"] == *code)
                        {
                            return Err(format!(
                                "{path}.absent_diagnostics: forbidden diagnostic {}",
                                code.as_str().unwrap()
                            ));
                        }
                    }
                    continue;
                }
                let path = format!("{path}.{key}");
                let actual = actual
                    .get(key)
                    .ok_or_else(|| format!("{path}: missing key"))?;
                compare(value, actual, &path)?;
            }
            Ok(())
        }
        (Value::Array(expected), Value::Array(actual)) if path == "$.differences" => {
            let expected = pair_keyed(expected, path)?;
            let actual = pair_keyed(actual, path)?;
            if expected.keys().ne(actual.keys()) {
                return Err(format!("{path}: (kind,target) sets differ"));
            }
            for (id, value) in expected {
                compare(value, actual[&id], path)?;
            }
            Ok(())
        }
        (Value::Array(expected), Value::Array(actual))
            if ["exclude", "defer"].iter().any(|field| {
                ["from", "to"]
                    .iter()
                    .any(|side| path == format!("$.mismatch.differences[{field}].{side}"))
            }) =>
        {
            compare_set(expected, actual, "target", path)
        }
        (Value::Array(expected), Value::Array(actual))
            if ["from", "to"]
                .iter()
                .any(|side| path == format!("$.mismatch.differences[include].{side}")) =>
        {
            let expected: std::collections::BTreeSet<_> =
                expected.iter().map(|v| v.as_str().unwrap()).collect();
            let actual: std::collections::BTreeSet<_> =
                actual.iter().map(|v| v.as_str().unwrap()).collect();
            if expected == actual {
                Ok(())
            } else {
                Err(format!("{path}: include sets differ"))
            }
        }
        (Value::Array(expected), Value::Array(actual)) => match path.rsplit('.').next() {
            Some("differences") if path == "$.mismatch.differences" => {
                compare_set(expected, actual, "field", path)
            }
            Some("terminals") if !path.contains("case_outcomes") => {
                compare_set(expected, actual, "end", path)
            }
            // Quality properties are named and in declaration
            // order (never sorted by name), so they keep length and order.
            Some("properties") if path.ends_with(".quality.properties") => {
                compare_ordered(expected, actual, path)
            }
            // Property rows are keyed by id; an annotation's property list is a
            // plain ordered list of id strings (ADR 0031) and falls through.
            Some("properties") if !path.contains(".annotations[") => {
                compare_set(expected, actual, "id", path)
            }
            Some("diagnostics") => {
                for value in expected {
                    let code = identifier(value, "code", path)?;
                    if !actual.iter().any(|candidate| {
                        candidate.get("code").and_then(Value::as_str) == Some(code)
                            && compare(value, candidate, path).is_ok()
                    }) {
                        return Err(format!("{path}: missing matching diagnostic {code}"));
                    }
                }
                Ok(())
            }
            _ => compare_ordered(expected, actual, path),
        },
        _ if expected == actual => Ok(()),
        _ => Err(format!("{path}: expected {expected}, got {actual}")),
    }
}

fn compare_ordered(expected: &[Value], actual: &[Value], path: &str) -> Result<(), String> {
    if expected.len() != actual.len() {
        return Err(format!(
            "{path}: array length {} != {}",
            expected.len(),
            actual.len()
        ));
    }
    for (i, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        compare(expected, actual, &format!("{path}[{i}]"))?;
    }
    Ok(())
}

fn identifier<'a>(value: &'a Value, key: &str, path: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{path}: missing string identifier {key}"))
}

fn keyed<'a>(
    values: &'a [Value],
    key: &str,
    path: &str,
) -> Result<BTreeMap<&'a str, &'a Value>, String> {
    let mut result = BTreeMap::new();
    for value in values {
        let id = identifier(value, key, path)?;
        if result.insert(id, value).is_some() {
            return Err(format!("{path}: duplicate {key} {id}"));
        }
    }
    Ok(result)
}

fn pair_keyed<'a>(
    values: &'a [Value],
    path: &str,
) -> Result<BTreeMap<(&'a str, &'a str), &'a Value>, String> {
    let mut result = BTreeMap::new();
    for value in values {
        let key = (
            identifier(value, "kind", path)?,
            identifier(value, "target", path)?,
        );
        if result.insert(key, value).is_some() {
            return Err(format!("{path}: duplicate (kind,target) {key:?}"));
        }
    }
    Ok(result)
}

fn compare_set(expected: &[Value], actual: &[Value], key: &str, path: &str) -> Result<(), String> {
    let expected = keyed(expected, key, path)?;
    let actual = keyed(actual, key, path)?;
    if expected.keys().ne(actual.keys()) {
        return Err(format!(
            "{path}: {key} sets differ: {:?} != {:?}",
            expected.keys(),
            actual.keys()
        ));
    }
    for (id, value) in expected {
        compare(value, actual[id], &format!("{path}[{id}]"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_business_metrics() -> Value {
        let x = |s: &str| serde_json::json!({"min":s,"max":s,"reason":null});
        let count = |n: &str| serde_json::json!({"stats":x(n),"observed_total":null});
        let zero = count("0");
        let one = count("1");
        let counts = serde_json::json!({"end_counts":[{"end":"done","count":one}],"rejected":zero,"unfinished":zero,"deadlocked":zero,"not_arrived":zero});
        let no_terminal = serde_json::json!({"min":null,"max":null,"reason":"rejected"});
        let m = serde_json::json!({
            "rules":"business-metrics-p1-1","status":"complete","window":10,"currency":"JPY",
            "time_basis":"arrival_to_declared_end","lead_time_scope":"terminal_cases_only",
            "observation_scope":"until_execution_stop_no_extra_drain","statistical_unit":null,"estimate_note":null,
            "rates":{"actors":[{"actor":"Office","per_minute":100,"reason":null,"required":true}],"capabilities":[{"capability":"Office.run","per_start":5,"reason":null,"required":true}]},
            "cases":[{"case_id":1,"arrival":0,"counts":counts,"first_start_wait":x("0"),
                "terminals":[{"kind":"end","end":"done","lead_time":x("8"),"cost_per_case":x("805"),"count":one,"excluded":{"unfinished":zero,"deadlocked":zero,"rejected":zero,"other_end":zero}},
                    {"kind":"rejected","end":null,"lead_time":no_terminal,"cost_per_case":no_terminal,"count":zero,"excluded":{"unfinished":zero,"deadlocked":zero,"rejected":zero,"other_end":one}}],
                "tasks":[{"binding":"job","capability":"Office.run","wait":x("0"),"work":x("8"),"starts":x("1"),"never_created_count":zero,"cancelled_before_start_count":zero,"not_started_count":zero}],"work":x("8"),"observed_cost_total":x("805")}],
            "terminals":[{"end":"done","count":one,"cost_per_case":x("805"),"completed_cases_total":null}],
            "actors":[{"actor":"Office","capacity":2,"work":x("8"),"peak_tasks":x("1"),"utilization":x("2/5"),"occupancy_cost":x("800")}],
            "people":[],"capabilities":[{"capability":"Office.run","starts":x("1"),"start_cost":x("5")}],
            "overall":{"counts":{"offered":one,"received":one,"completed":one,"rejected":zero,"unfinished":zero,"deadlocked":zero,"not_arrived":zero,"end_counts":[{"end":"done","count":one}]},
                "observed_until":x("8"),"work":x("8"),"named_people_work":x("0"),"peak_tasks":x("1"),"occupancy_cost":x("800"),"start_cost":x("5"),"observed_cost_total":x("805"),"observed_cost_per_arrival":x("805"),"business_value":{"value":null,"reason":"not_configured"}},
            "checked":METRIC_CHECKED,"unchecked":METRIC_UNCHECKED
        });
        let mut v = complete_report();
        v["metrics"] = m;
        let values = [
            "10min",
            "JPY",
            "model-declarations",
            "terminal_cases_only",
            "until_execution_stop_no_extra_drain",
        ];
        for i in 0..5 {
            v["assumptions"].as_array_mut().unwrap().push(serde_json::json!({"field":METRIC_FIELDS[i],"value":values[i],"affects":METRIC_AFFECTS[i]}));
        }
        v
    }

    #[test]
    fn m_metric_numbers_are_nonnegative_canonical_and_not_probability_bounded() {
        for s in [
            "0",
            "8",
            "805",
            "8/3",
            "36893488147419103230",
            "340282366920938463463374607431768211456",
            "100000000000000000000000000000000000001/3",
        ] {
            assert!(metric_fraction(&serde_json::json!(s)).is_ok(), "{s}");
        }
        for s in [
            "-1", "+1", "1.5", "01", "1/0", "1/-2", "2/4", "0/2", "8/1", "1//2", " 1", "1e2", "",
        ] {
            assert!(metric_fraction(&serde_json::json!(s)).is_err(), "{s}");
        }
        assert!(metric_le(&serde_json::json!("8/3"), &serde_json::json!("805")).unwrap());
        assert!(
            !metric_le(
                &serde_json::json!("36893488147419103230"),
                &serde_json::json!("805")
            )
            .unwrap()
        );
    }

    #[test]
    fn m_closed_metrics_reject_every_missing_and_unknown_nested_key() {
        let v = complete_business_metrics();
        validate_business_metrics_wire(&v).unwrap();
        let mut fields = Vec::new();
        object_fields(&v["metrics"], "/metrics", &mut fields);
        for field in fields {
            let (parent, key) = field.rsplit_once('/').unwrap();
            let mut missing = v.clone();
            missing
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                validate_business_metrics_wire(&missing).is_err(),
                "missing {field}"
            );
            let mut unknown = v.clone();
            unknown
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unrecognized_metric_key".into(), Value::Null);
            assert!(
                validate_business_metrics_wire(&unknown).is_err(),
                "unknown in {parent}"
            );
        }
        for (path, bad) in [
            ("/metrics/cases/0/case_id", serde_json::json!(2)),
            (
                "/metrics/overall/work/reason",
                serde_json::json!("not_configured"),
            ),
            ("/metrics/overall/work/min", Value::Null),
            ("/metrics/overall/work/max", serde_json::json!("7")),
            ("/metrics/rates/actors/0/per_minute", serde_json::json!(-1)),
            ("/metrics/window", serde_json::json!(0)),
            ("/metrics/window", serde_json::json!(9)),
            ("/metrics/overall/work/min", serde_json::json!("2/4")),
            (
                "/metrics/overall/counts/completed/observed_total",
                serde_json::json!(1),
            ),
            (
                "/metrics/statistical_unit",
                serde_json::json!("replication"),
            ),
            ("/metrics/status", serde_json::json!("truncated")),
        ] {
            let mut x = v.clone();
            *x.pointer_mut(path).unwrap() = bad;
            assert!(validate_business_metrics_wire(&x).is_err(), "{path}");
        }
        for path in [
            "/metrics/actors",
            "/metrics/capabilities",
            "/metrics/cases/0/tasks",
            "/metrics/cases/0/terminals",
            "/metrics/terminals",
            "/metrics/rates/actors",
        ] {
            let mut x = v.clone();
            let a = x.pointer_mut(path).unwrap().as_array_mut().unwrap();
            a.push(a[0].clone());
            assert!(
                validate_business_metrics_wire(&x).is_err(),
                "duplicate {path}"
            );
        }
        let mut x = v.clone();
        x["metrics"]["people"] = serde_json::json!([{"person":"p","work":{"min":"0","max":"0","reason":null},"peak_tasks":{"min":"0","max":"0","reason":null}},{"person":"p","work":{"min":"0","max":"0","reason":null},"peak_tasks":{"min":"0","max":"0","reason":null}}]);
        assert!(validate_business_metrics_wire(&x).is_err());
    }

    #[test]
    fn m_scalar_samples_null_reasons_modes_and_count_means_are_checked() {
        let good = serde_json::json!({"mean":"8/3","min":"2","max":"4","samples":3,"reason":null});
        scalar_invariants(&good, true, "complete", 3).unwrap();
        for (key, value) in [
            ("samples", serde_json::json!(0)),
            ("samples", serde_json::json!(4)),
            ("mean", serde_json::json!("5")),
            ("mean", Value::Null),
            ("reason", serde_json::json!("no_samples")),
        ] {
            let mut x = good.clone();
            x[key] = value;
            assert!(scalar_invariants(&x, true, "complete", 3).is_err());
        }
        assert!(scalar_invariants(&good, false, "complete", 3).is_err());
        assert!(scalar_invariants(&good, true, "aborted", 3).is_err());
        let count = serde_json::json!({"stats":{"mean":"1/2","min":"0","max":"1","samples":4,"reason":null},"observed_total":2});
        metric_tree(&count, true, "truncated", 4).unwrap();
        let mut wrong = count.clone();
        wrong["observed_total"] = serde_json::json!(3);
        assert!(metric_tree(&wrong, true, "complete", 4).is_err());
        for reason in ["aborted", "truncated"] {
            let null = serde_json::json!({"min":null,"max":null,"reason":reason});
            scalar_invariants(&null, false, reason, 0).unwrap();
        }
        let mut fractional = count;
        fractional["stats"]["min"] = serde_json::json!("1/2");
        assert!(metric_tree(&fractional, true, "complete", 4).is_err());
    }

    fn complete_sample_metrics() -> Value {
        let mut v = complete_business_metrics();
        fn sampled(v: &mut Value) {
            if v.get("min").is_some() {
                let value = v["min"].clone();
                let n = if value.is_null() { 0 } else { 4 };
                v["mean"] = value;
                v["samples"] = serde_json::json!(n);
            } else if v.get("stats").is_some() {
                let n = v["stats"]["min"].as_str().unwrap().parse::<u64>().unwrap();
                v["observed_total"] = serde_json::json!(4 * n);
                sampled(&mut v["stats"]);
            } else if let Some(o) = v.as_object_mut() {
                for x in o.values_mut() {
                    sampled(x);
                }
            } else if let Some(a) = v.as_array_mut() {
                for x in a {
                    sampled(x);
                }
            }
        }
        sampled(&mut v["metrics"]);
        v["mode"] = serde_json::json!("simulation");
        v["simulation"] = serde_json::json!({"replications":4});
        v["metrics"]["statistical_unit"] = serde_json::json!("replication");
        v["metrics"]["estimate_note"] =
            serde_json::json!("点推定であり、全スケジュールの検証でも区間推定でもない");
        v["metrics"]["terminals"][0]["completed_cases_total"] = serde_json::json!(4);
        v["case_outcomes"] = serde_json::json!([{"case_id":1,"arrival":0,"terminals":[{"kind":"end","end":"done","count":4}],"unfinished_count":0,"deadlocked_count":0}]);
        for field in SIM_FIELDS {
            v["assumptions"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"field":field,"value":"test","affects":"test"}));
        }
        v["assumptions"].as_array_mut().unwrap().push(serde_json::json!({"field":METRIC_FIELDS[5],"value":"point-estimate-only; unit=replication","affects":METRIC_AFFECTS[5]}));
        v
    }

    #[test]
    fn m_complete_sample_metrics_reject_lost_prefix_samples_and_bad_cohort_counts() {
        let v = complete_sample_metrics();
        validate_business_metrics_wire(&v).unwrap();
        for (path, bad) in [
            ("/metrics/overall/work/samples", serde_json::json!(2)),
            ("/metrics/overall/work/samples", serde_json::json!(0)),
            ("/metrics/overall/work/mean", serde_json::json!("9")),
            (
                "/metrics/cases/0/terminals/0/lead_time/samples",
                serde_json::json!(0),
            ),
            ("/metrics/cases/0/terminals/1/lead_time/reason", Value::Null),
            (
                "/metrics/overall/counts/completed/observed_total",
                serde_json::json!(5),
            ),
            (
                "/metrics/cases/0/counts/unfinished/observed_total",
                serde_json::json!(1),
            ),
            ("/metrics/actors/0/work/mean", serde_json::json!("1/0")),
        ] {
            let mut x = v.clone();
            *x.pointer_mut(path).unwrap() = bad;
            assert!(validate_business_metrics_wire(&x).is_err(), "{path}");
        }
        let mut x = v.clone();
        x["metrics"]["overall"]["work"] = serde_json::json!({"min":"8","max":"8","reason":null});
        assert!(validate_business_metrics_wire(&x).is_err());
    }

    fn metric_comparison_control(compare: bool) -> Value {
        let report = complete_business_metrics();
        let mut rows = Vec::new();
        for (_, (key, value)) in project_metrics(&report["metrics"]).unwrap() {
            rows.push(if compare {serde_json::json!({"key":key,"values":[{"configuration":"B","status":"reported","value":value},{"configuration":"A","status":"reported","value":value}]})}else{serde_json::json!({"key":key,"from":{"status":"reported","value":value},"to":{"status":"reported","value":value}})});
        }
        let words = serde_json::json!([
            "metric_differences",
            "metric_ranking",
            "metric_semantic_alignment"
        ]);
        if compare {
            serde_json::json!({"tables":{"metrics":rows},"plans":[{"configuration":"B","report":report},{"configuration":"A","report":report}],"checked":["metric_tables"],"unchecked":words})
        } else {
            serde_json::json!({"metrics":rows,"from":{"configuration":"B","report":report},"to":{"configuration":"A","report":report},"checked":["metric_tables"],"unchecked":words})
        }
    }

    #[test]
    fn m_metric_tables_reject_lost_duplicate_unordered_ranked_or_nonraw_cells() {
        for compare in [false, true] {
            let control = metric_comparison_control(compare);
            validate_metric_tables(&control, compare).unwrap();
            let path = if compare {
                "/tables/metrics"
            } else {
                "/metrics"
            };
            for fault in [
                "missing",
                "duplicate",
                "reverse",
                "rank",
                "delta",
                "unknown-key",
                "not-applicable-zero",
                "wrong-value",
                "plan-order",
            ] {
                if fault == "plan-order" && !compare {
                    continue;
                }
                let mut x = control.clone();
                let rows = x.pointer_mut(path).unwrap().as_array_mut().unwrap();
                match fault {
                    "missing" => {
                        rows.pop();
                    }
                    "duplicate" => rows.push(rows[0].clone()),
                    "reverse" => rows.reverse(),
                    "rank" | "delta" => rows[0][fault] = serde_json::json!(0),
                    "unknown-key" => rows[0]["key"]["unknown"] = Value::Null,
                    "plan-order" => rows[0]["values"].as_array_mut().unwrap().reverse(),
                    "not-applicable-zero" => {
                        let cell = if compare {
                            &mut rows[0]["values"][0]
                        } else {
                            &mut rows[0]["from"]
                        };
                        cell["status"] = serde_json::json!("not_applicable");
                        cell["value"] = serde_json::json!({"min":"0","max":"0","reason":null});
                    }
                    "wrong-value" => {
                        let cell = if compare {
                            &mut rows[0]["values"][0]
                        } else {
                            &mut rows[0]["from"]
                        };
                        cell["value"] = serde_json::json!({"min":"805","max":"805","reason":null});
                    }
                    _ => unreachable!(),
                }
                assert!(validate_metric_tables(&x, compare).is_err(), "{fault}");
            }
        }
    }

    #[test]
    fn m_fourteenth_fixture_schema_population_is_24_and_existing_13_total207() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut found: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| {
                e.file_type().unwrap().is_dir()
                    && e.file_name().to_string_lossy().starts_with("met-")
            })
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        found.sort();
        assert_eq!(found, METRICS_FIXTURES);
        assert_eq!(
            16 + 12 + 9 + 4 + 19 + 14 + 16 + 16 + 18 + 24 + 24 + 16 + 19,
            207
        );
        for name in found {
            let dir = root.join(name);
            let expected: Value =
                serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).unwrap())
                    .unwrap();
            let inv = std::fs::read_to_string(dir.join("invocation.json"))
                .ok()
                .map(|s| serde_json::from_str(&s).unwrap());
            Invocation::parse(inv.as_ref())
                .unwrap()
                .validate_expected(&expected)
                .unwrap();
        }
    }

    fn complete_compare(sim: bool) -> Value {
        let report = if sim {
            complete_simulation()
        } else {
            complete_report()
        };
        let source = if sim {
            &report["properties"][0]["cases"][0]
        } else {
            &report["properties"][0]
        };
        let mut cells = Vec::new();
        for name in ["B", "A"] {
            let mut cell = serde_json::json!({"configuration":name});
            for f in [
                "verdict",
                "probability",
                "basis",
                "hits",
                "unknown",
                "trials",
                "interval",
                "witness",
            ] {
                cell[f] = source.get(f).cloned().unwrap_or(Value::Null);
            }
            cells.push(cell);
        }
        let mut v = serde_json::json!({"ledger":ledger_shape::control(&serde_json::json!({})),"uncertainty":ledger_shape::uncertainty_wire::project(&serde_json::json!({})),"verdict":"satisfied","mode":"compare","analysis":if sim{"simulation"}else{"exhaustive"},
            "plans":[{"configuration":"B","report":report},{"configuration":"A","report":report}],
            "tables":{"properties":[{"id":report["properties"][0]["id"],"case_id":1,"arrival":0,"values":cells}],
                "terminals":[{"kind":"end","end":"done","case_id":1,"arrival":0,"values":[]},
                    {"kind":"rejected","end":null,"case_id":1,"arrival":0,"values":[]}]},
            "mismatch":null,"assumptions":[],"diagnostics":[],
            "checked":if sim {vec!["configuration_selection","cut_compatibility","terminal_compatibility","simulation_clause","random_key_separation","property_tables","terminal_tables"]}
                else {vec!["configuration_selection","cut_compatibility","terminal_compatibility","property_tables","terminal_tables"]},
            "unchecked":["ranking","hard_filter","pareto","weighted_score","differences","difference_intervals","cross_plan_confidence","operation_coupling","metrics","terminal_meaning"]});
        for (i, end) in [true, false].into_iter().enumerate() {
            let value = if sim {
                serde_json::json!({"count":if end{4}else{0}})
            } else if end {
                let t = &report["terminals"][0];
                serde_json::json!({"completed_at":t["elapsed"],"elapsed":t["elapsed"],"work":t["work"],"result_at":t["result_at"],"probability":null})
            } else {
                Value::Null
            };
            v["tables"]["terminals"][i]["values"] = serde_json::json!([{"configuration":"B","status":if sim||end{"reported"}else{"not_reported"},"value":value},
                {"configuration":"A","status":if sim||end{"reported"}else{"not_reported"},"value":value}]);
        }
        if sim {
            v["common_random"] = serde_json::json!({"method":"sha256-counter-v1","scope":"initial-only",
            "shared":[{"kind":"case_initial","case_ids":[1],"sites":[],"occurrence":1},{"kind":"actor_initial","case_ids":[0],"sites":[],"occurrence":1}],
            "independent":[{"configuration":"B","process":"P","kernel_sites":[],"kernel_occurrence":1,"duration_sites":[],"duration_occurrence":"positive-support-order"},
                {"configuration":"A","process":"Q","kernel_sites":[],"kernel_occurrence":1,"duration_sites":[],"duration_occurrence":"positive-support-order"}],
            "coupling":"potential-draws-common-when-admitted","independence":"domain-separated-sha256-assumption","confidence_scope":"within-plan-only"});
        }
        v
    }

    #[test]
    fn compare_range_inventory_is_global_only_in_simulation() {
        for sim in [false, true] {
            let mut good = complete_compare(sim);
            let range = if sim {
                serde_json::json!({"id":"state_in_range","verdict":"inconclusive","basis":"sample","witness":null})
            } else {
                serde_json::json!({"id":"state_in_range","verdict":"satisfied"})
            };
            for plan in good["plans"].as_array_mut().unwrap() {
                let report = &mut plan["report"];
                report["properties"]
                    .as_array_mut()
                    .unwrap()
                    .push(range.clone());
                if sim {
                    report["verdict"] = serde_json::json!("inconclusive");
                    report["simulation"]["range_stopped_replications"] = serde_json::json!(0);
                    report["case_outcomes"][0]["range_stopped_count"] = serde_json::json!(0);
                }
            }
            if sim {
                good["verdict"] = serde_json::json!("inconclusive");
            }
            // Hand-authored expected key and cells, not production projection.
            good["tables"]["properties"].as_array_mut().unwrap().push(serde_json::json!({
                "id":"state_in_range","case_id":if sim {Value::Null} else {serde_json::json!(1)},
                "arrival":if sim {Value::Null} else {serde_json::json!(0)},
                "values":[
                    {"configuration":"B","verdict":if sim {"inconclusive"} else {"satisfied"},"probability":null,"basis":if sim {serde_json::json!("sample")} else {Value::Null},"hits":null,"unknown":null,"trials":null,"interval":null,"witness":null},
                    {"configuration":"A","verdict":if sim {"inconclusive"} else {"satisfied"},"probability":null,"basis":if sim {serde_json::json!("sample")} else {Value::Null},"hits":null,"unknown":null,"trials":null,"interval":null,"witness":null}
                ]
            }));
            validate_compare_wire(&good).unwrap();
            for fault in ["scope", "missing", "reverse"] {
                let mut bad = good.clone();
                let rows = bad["tables"]["properties"].as_array_mut().unwrap();
                match fault {
                    "scope" => {
                        rows[1]["case_id"] = if sim {
                            serde_json::json!(1)
                        } else {
                            Value::Null
                        };
                        rows[1]["arrival"] = if sim {
                            serde_json::json!(0)
                        } else {
                            Value::Null
                        };
                    }
                    "missing" => {
                        rows.pop();
                    }
                    "reverse" => rows.reverse(),
                    _ => unreachable!(),
                }
                assert_eq!(
                    validate_compare_wire(&bad).unwrap_err(),
                    "property table inventory/order",
                    "sim={sim} fault={fault}"
                );
            }
        }
    }

    #[test]
    fn compare_closed_wire_and_order_comparator_have_independent_negative_controls() {
        for sim in [false, true] {
            let good = complete_compare(sim);
            validate_compare_wire(&good).unwrap();
            compare_compare_report(&good, &good).unwrap();
            for pointer in [
                "",
                "/tables",
                "/tables/properties/0",
                "/tables/properties/0/values/0",
                "/tables/terminals/0",
                "/tables/terminals/0/values/0",
                "/tables/terminals/0/values/0/value",
                "/plans/0",
            ] {
                for key in good.pointer(pointer).unwrap().as_object().unwrap().keys() {
                    let mut bad = good.clone();
                    bad.pointer_mut(pointer)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .remove(key);
                    assert!(
                        validate_compare_wire(&bad).is_err(),
                        "missing {pointer}/{key}"
                    );
                }
                let mut bad = good.clone();
                bad.pointer_mut(pointer).unwrap()["unknown_field"] = serde_json::json!(0);
                assert!(validate_compare_wire(&bad).is_err(), "unknown {pointer}");
            }
            let mut bad = good.clone();
            bad["plans"].as_array_mut().unwrap().reverse();
            assert!(compare_compare_report(&good, &bad).is_err());
            let mut bad = good.clone();
            bad["tables"]["properties"][0]["values"]
                .as_array_mut()
                .unwrap()
                .reverse();
            assert!(compare_compare_report(&good, &bad).is_err());
            let mut bad = good.clone();
            bad["analysis"] = serde_json::json!(if sim { "exhaustive" } else { "simulation" });
            assert!(validate_compare_wire(&bad).is_err());
            for pointer in ["/tables/properties", "/tables/terminals", "/plans"] {
                let mut bad = good.clone();
                let a = bad.pointer_mut(pointer).unwrap().as_array_mut().unwrap();
                a.push(a[0].clone());
                assert!(validate_compare_wire(&bad).is_err(), "duplicate {pointer}");
            }
            let mut bad = good.clone();
            bad["tables"]["properties"] = serde_json::json!([]);
            assert!(validate_compare_wire(&bad).is_err());
            let mut bad = good.clone();
            bad["tables"]["terminals"][0]["end"] = Value::Null;
            assert!(validate_compare_wire(&bad).is_err());
            if sim {
                let mut bad = good.clone();
                bad["plans"][0]["report"]["simulation"]["seed"] =
                    serde_json::json!("18446744073709551615");
                assert!(validate_compare_wire(&bad).is_err());
                for key in ["hits", "trials", "interval"] {
                    let mut bad = good.clone();
                    bad["tables"]["properties"][0]["values"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove(key);
                    assert!(validate_compare_wire(&bad).is_err());
                }
            } else {
                let mut bad = good.clone();
                bad["common_random"] = Value::Null;
                assert!(validate_compare_wire(&bad).is_err());
            }
        }
        let mut bad = complete_compare(false);
        bad["mismatch"] = serde_json::json!({"pairs":[{"left":"B","right":"A","cut":{"left_cut":"C","right_cut":"D","differences":[]},"terminals":null}]});
        assert!(
            validate_compare_wire(&bad).is_err(),
            "mismatch cannot keep tables"
        );
        let mut old = complete_diff();
        old["case_outcomes"] = serde_json::json!([]);
        assert!(
            compare_diff_report(&serde_json::json!({}), &old).is_err(),
            "legacy diff must not gain fields"
        );
    }

    #[test]
    fn compare_fixture_population_and_partial_schemas_are_frozen() {
        assert_eq!(
            (
                CONTRACT_FIXTURES.len(),
                EXECUTOR_FIXTURES.len(),
                SIM_FIXTURES.len()
            ),
            (19, 14, 24)
        );
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut names: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| {
                e.file_type().unwrap().is_dir()
                    && e.file_name().to_string_lossy().starts_with("compare-")
            })
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, COMPARE_FIXTURES);
        for name in names {
            let spec: Value = serde_json::from_str(
                &std::fs::read_to_string(root.join(&name).join("invocation.json")).unwrap(),
            )
            .unwrap();
            let h: Value = serde_json::from_str(
                &std::fs::read_to_string(root.join(&name).join("expected.json")).unwrap(),
            )
            .unwrap();
            Invocation::parse(Some(&spec))
                .unwrap()
                .validate_expected(&h)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn new_diff_closed_schema_alignment_and_nulls_have_negative_controls() {
        let raw = complete_arrivals();
        let mut good = complete_diff();
        good["from"]["report"] = raw.clone();
        good["to"]["report"] = raw.clone();
        good["agency"] = serde_json::json!({"from":raw["agency"],"to":raw["agency"]});
        good["mismatch"] = Value::Null;
        good["terminals"] = serde_json::json!([]);
        good["case_outcomes"]=serde_json::json!(raw["case_outcomes"].as_array().unwrap().iter().map(|c| {
            let ts:Vec<_>=c["terminals"].as_array().unwrap().iter().map(|t| {
                let mut value=t.clone();let o=value.as_object_mut().unwrap();o.remove("kind");o.remove("end");
                serde_json::json!({"kind":t["kind"],"end":t["end"],"from":value,"to":value})
            }).collect();serde_json::json!({"case_id":c["case_id"],"arrival":c["arrival"],"terminals":ts})
        }).collect::<Vec<_>>());
        good["properties"]=serde_json::json!(raw["properties"].as_array().unwrap().iter().map(|p| {
            let mut row=serde_json::json!({"id":p["id"],"from":p["verdict"],"to":p["verdict"]});
            if let Some(cs)=p.get("cases") {row["cases"]=serde_json::json!(cs.as_array().unwrap().iter().map(|c| {
                let mut value=c.clone();value.as_object_mut().unwrap().remove("case_id");value.as_object_mut().unwrap().remove("arrival");
                serde_json::json!({"case_id":c["case_id"],"arrival":c["arrival"],"from":value,"to":value})
            }).collect::<Vec<_>>());}row
        }).collect::<Vec<_>>());
        compare_diff_report(&good, &good).unwrap();
        for (pointer, value) in [
            ("/case_outcomes", serde_json::json!([])),
            ("/case_outcomes/1/case_id", serde_json::json!(1)),
            ("/case_outcomes/1/arrival", serde_json::json!(7)),
            ("/case_outcomes/1/terminals/0/end", serde_json::json!("bad")),
            (
                "/case_outcomes/1/terminals/0/from/work",
                serde_json::json!({"min":0,"max":0}),
            ),
            ("/properties/1/cases/1/case_id", serde_json::json!(1)),
            (
                "/properties/1/cases/0/to/probability",
                serde_json::json!({"min":"1","max":"1"}),
            ),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(
                compare_diff_report(&serde_json::json!({}), &bad).is_err(),
                "{pointer}"
            );
        }
        for pointer in [
            "/case_outcomes/0",
            "/case_outcomes/0/terminals/0",
            "/case_outcomes/0/terminals/0/from",
            "/properties/1/cases/0",
            "/properties/1/cases/0/from",
        ] {
            for key in good.pointer(pointer).unwrap().as_object().unwrap().keys() {
                if key == "probability" {
                    continue;
                } // Optional in non-probability models, but raw consistency below still protects it.
                let mut bad = good.clone();
                bad.pointer_mut(pointer)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(
                    compare_diff_report(&serde_json::json!({}), &bad).is_err(),
                    "missing {pointer}/{key}"
                );
            }
            let mut bad = good.clone();
            bad.pointer_mut(pointer).unwrap()["unknown"] = serde_json::json!(0);
            assert!(
                compare_diff_report(&serde_json::json!({}), &bad).is_err(),
                "unknown {pointer}"
            );
        }
    }

    #[test]
    fn compare_repeated_property_and_end_names_use_composite_case_keys() {
        let raw = complete_arrivals();
        let mut good = complete_compare(false);
        for p in good["plans"].as_array_mut().unwrap() {
            p["report"] = raw.clone();
        }
        let mut properties = Vec::new();
        for p in raw["properties"].as_array().unwrap() {
            let rows: Vec<_> = p
                .get("cases")
                .map(|cs| cs.as_array().unwrap().iter().collect())
                .unwrap_or_else(|| vec![p]);
            for r in rows {
                let values:Vec<_>=["B","A"].into_iter().map(|name|serde_json::json!({"configuration":name,"verdict":r["verdict"],
                    "probability":r.get("probability").cloned().unwrap_or(Value::Null),"basis":null,"hits":null,"unknown":null,"trials":null,"interval":null,"witness":null})).collect();
                properties.push(serde_json::json!({"id":p["id"],"case_id":r.get("case_id").cloned().unwrap_or(Value::Null),"arrival":r.get("arrival").cloned().unwrap_or(Value::Null),"values":values}));
            }
        }
        let mut terminals = Vec::new();
        for end in [Some("rejected"), None] {
            for c in raw["case_outcomes"].as_array().unwrap() {
                let t = c["terminals"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|t| t["end"] == serde_json::json!(end));
                let value=t.map(|t|serde_json::json!({"completed_at":t["completed_at"],"elapsed":t["elapsed"],"work":t["work"],"result_at":t["result_at"],"probability":t["probability"]}));
                terminals.push(serde_json::json!({"kind":if end.is_some(){"end"}else{"rejected"},"end":end,"case_id":c["case_id"],"arrival":c["arrival"],
                "values":[{"configuration":"B","status":if t.is_some(){"reported"}else{"not_reported"},"value":value},
                    {"configuration":"A","status":if t.is_some(){"reported"}else{"not_reported"},"value":value}]}));
            }
        }
        good["tables"] = serde_json::json!({"properties":properties,"terminals":terminals});
        validate_compare_wire(&good).unwrap();
        let mut expected = good.clone();
        expected["tables"]["properties"]
            .as_array_mut()
            .unwrap()
            .reverse();
        expected["tables"]["terminals"]
            .as_array_mut()
            .unwrap()
            .reverse();
        compare_compare_report(&expected, &good).unwrap();
        let mut bad = good.clone();
        bad["tables"]["properties"][2]["case_id"] = serde_json::json!(1);
        assert!(compare_compare_report(&serde_json::json!({}), &bad).is_err());
    }

    fn complete_simulation() -> Value {
        let mut v = complete_report();
        v["unchecked"] = serde_json::json!(["unreachable"]);
        v["verdict"] = serde_json::json!("satisfied");
        v["mode"] = serde_json::json!("simulation");
        v["bounds"] = serde_json::json!({"cases":1,"live_tasks":1,"clock":2,"truncated":false});
        v["terminals"] = serde_json::json!([]);
        v["properties"] = serde_json::json!([{"id":"probability reached #1","verdict":"satisfied","cases":[{"case_id":1,"arrival":0,"verdict":"satisfied","basis":"statistical","hits":4,"unknown":0,"trials":4,"interval":{"lower":"741455/1048576","upper":"1"}}]}]);
        v["simulation"] = serde_json::json!({"status":"completed","replications":4,"seed":18446744073709551615u64,"confidence":"1/2","attempted_replications":4,"completed_replications":4,"truncated_replications":0,"interval_method":"clopper-pearson","multiplicity":{"method":"bonferroni","rows":1,"alpha_before":"1/4","alpha_after":"1/4"},"random_method":"sha256-counter-v1","nondeterminism":"reject-on-encounter","deadlock_witnesses":[]});
        v["case_outcomes"] = serde_json::json!([{"case_id":1,"arrival":0,"terminals":[{"kind":"end","end":"done","count":4}],"unfinished_count":0,"deadlocked_count":0}]);
        v["admission"] = serde_json::json!({"rejected_count":0});
        v["assumptions"] = serde_json::json!([]);
        let values = [
            "sha256-counter-v1; category-bias<=1/18446744073709551616",
            "independent; unit=replication",
            "bonferroni; rows=1; alpha_before=1/4; alpha_after=1/4",
            "reject-on-encounter",
            "estimation-not-proof",
        ];
        for i in 0..5 {
            v["assumptions"].as_array_mut().unwrap().push(serde_json::json!({"field":SIM_FIELDS[i],"value":values[i],"affects":SIM_AFFECTS[i]}));
        }
        v
    }

    #[test]
    fn simulation_wire_closed_schema_has_independent_negative_controls() {
        let good = complete_simulation();
        validate_simulation_wire(&good).unwrap();
        let inv = Invocation::parse(Some(&serde_json::json!({"command":"simulate"}))).unwrap();
        inv.compare(&good, &good).unwrap();
        assert!(
            compare_report(&good, &good).is_err(),
            "do not widen check schema to accept simulation"
        );
        for pointer in [
            "",
            "/simulation",
            "/simulation/multiplicity",
            "/bounds",
            "/model",
            "/case_outcomes/0",
            "/case_outcomes/0/terminals/0",
            "/properties/0",
            "/properties/0/cases/0",
            "/properties/0/cases/0/interval",
            "/assumptions/0",
        ] {
            let object = good.pointer(pointer).unwrap().as_object().unwrap();
            for key in object.keys() {
                let mut bad = good.clone();
                bad.pointer_mut(pointer)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(
                    validate_simulation_wire(&bad).is_err(),
                    "missing required {pointer}/{key}"
                );
            }
            let mut bad = good.clone();
            bad.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("rogue".into(), serde_json::json!(null));
            assert!(validate_simulation_wire(&bad).is_err(), "closed {pointer}");
        }
        for (pointer, value) in [
            ("/mode", serde_json::json!("exhaustive")),
            ("/simulation/interval_method", serde_json::json!("wilson")),
            ("/simulation/random_method", serde_json::json!("uniform")),
            (
                "/simulation/seed",
                serde_json::json!("18446744073709551615"),
            ),
            ("/simulation/confidence", serde_json::json!("2/4")),
            ("/simulation/confidence", serde_json::json!("1")),
            ("/simulation/multiplicity/rows", serde_json::json!(2)),
            (
                "/simulation/multiplicity/alpha_after",
                serde_json::json!("1/8"),
            ),
            ("/simulation/truncated_replications", serde_json::json!(1)),
            ("/case_outcomes/0/case_id", serde_json::json!(2)),
            ("/case_outcomes/0/terminals/0/count", serde_json::json!(3)),
            ("/case_outcomes/0/unfinished_count", serde_json::json!(1)),
            ("/admission/rejected_count", serde_json::json!(1)),
            ("/properties/0/cases/0/case_id", serde_json::json!(2)),
            ("/properties/0/cases/0/arrival", serde_json::json!(1)),
            ("/properties/0/cases/0/hits", serde_json::json!(5)),
            ("/properties/0/cases/0/unknown", serde_json::json!(1)),
            ("/properties/0/cases/0/trials", serde_json::json!(3)),
            ("/properties/0/cases/0/basis", serde_json::json!("sample")),
            (
                "/properties/0/cases/0/verdict",
                serde_json::json!("vacuous"),
            ),
            (
                "/properties/0/cases/0/interval/lower",
                serde_json::json!("1/3"),
            ),
            (
                "/properties/0/cases/0/interval/upper",
                serde_json::json!("0"),
            ),
            ("/assumptions/0/value", serde_json::json!("pcg")),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(
                validate_simulation_wire(&bad).is_err(),
                "mutation {pointer}"
            );
        }
        for key in [
            "probability",
            "elapsed",
            "work",
            "result_at",
            "completed_at",
        ] {
            let mut bad = good.clone();
            bad["case_outcomes"][0]["terminals"][0][key] = Value::Null;
            assert!(
                validate_simulation_wire(&bad).is_err(),
                "forbidden time/probability {key}"
            );
        }
        for key in ["state_budget", "states", "transitions"] {
            let mut bad = good.clone();
            bad[key] = serde_json::json!(0);
            assert!(
                validate_simulation_wire(&bad).is_err(),
                "exhaustive-only {key}"
            );
        }
    }

    #[test]
    fn simulation_invocation_is_not_a_check_or_cli_override() {
        let v = serde_json::json!({"command":"simulate","configuration":"C"});
        let inv = Invocation::parse(Some(&v)).unwrap();
        assert_eq!(
            inv.args(std::path::Path::new("x.relca")),
            ["simulate", "x.relca", "--configuration", "C"]
        );
        for value in [
            serde_json::json!({"command":"simulate","seed":0}),
            serde_json::json!({"command":"simulate","configuration":null}),
            serde_json::json!({"command":"simulate","configuration":""}),
        ] {
            assert!(Invocation::parse(Some(&value)).is_err());
        }
    }

    #[test]
    fn simulation_static_and_runtime_refusal_forms_withdraw_statistics_not_case_ids() {
        let mut abort = complete_simulation();
        abort["verdict"] = serde_json::json!("unsupported");
        abort["bounds"]["truncated"] = Value::Null;
        abort["simulation"]["status"] = serde_json::json!("unsupported");
        abort["simulation"]["attempted_replications"] = serde_json::json!(3);
        abort["simulation"]["completed_replications"] = serde_json::json!(2);
        abort["simulation"]["truncated_replications"] = Value::Null;
        abort["properties"] = serde_json::json!([{"id":"probability reached #1","verdict":"unsupported","cases":[{"case_id":1,"arrival":0,"verdict":"unsupported","basis":"sample","witness":null}]}]);
        abort["case_outcomes"] = serde_json::json!([{"case_id":1,"arrival":0,"terminals":[],"unfinished_count":null,"deadlocked_count":null}]);
        abort["admission"] = Value::Null;
        abort["diagnostics"] = serde_json::json!([{"code":"U-SEM-0","severity":"error","message":"simulation encountered unresolved nondeterminism: replication 3, time 1, cases [1], sites [P.a]; at least two distinct successor actions remain","span":{"start":1,"end":2}}]);
        validate_simulation_wire(&abort).unwrap();
        for (pointer, value) in [
            ("/verdict", serde_json::json!("violated")),
            ("/bounds/truncated", serde_json::json!(false)),
            ("/simulation/completed_replications", serde_json::json!(3)),
            ("/case_outcomes/0/unfinished_count", serde_json::json!(2)),
            (
                "/case_outcomes/0/terminals",
                serde_json::json!([{"kind":"end","end":"done","count":2}]),
            ),
            (
                "/properties/0/cases/0/verdict",
                serde_json::json!("violated"),
            ),
            (
                "/properties/0/cases/0/witness",
                serde_json::json!({"replication":1}),
            ),
            ("/diagnostics/0/span", Value::Null),
        ] {
            let mut bad = abort.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(
                validate_simulation_wire(&bad).is_err(),
                "abort mutation {pointer}"
            );
        }
        for key in ["hits", "unknown", "trials", "interval"] {
            let mut bad = abort.clone();
            bad["properties"][0]["cases"][0][key] = serde_json::json!(0);
            assert!(validate_simulation_wire(&bad).is_err(), "partial {key}");
        }
        let mut static_failure = abort;
        static_failure.as_object_mut().unwrap().remove("unchecked");
        static_failure["verdict"] = serde_json::json!("invalid");
        static_failure["bounds"] =
            serde_json::json!({"cases":null,"live_tasks":null,"clock":null,"truncated":null});
        static_failure["agency"] = Value::Null;
        static_failure["assumptions"] = serde_json::json!([]);
        static_failure["properties"] = serde_json::json!([]);
        static_failure["case_outcomes"] = serde_json::json!([]);
        static_failure["simulation"]["status"] = serde_json::json!("invalid");
        for key in [
            "replications",
            "seed",
            "confidence",
            "multiplicity",
            "truncated_replications",
        ] {
            static_failure["simulation"][key] = Value::Null;
        }
        for key in ["attempted_replications", "completed_replications"] {
            static_failure["simulation"][key] = serde_json::json!(0);
        }
        static_failure["diagnostics"] = serde_json::json!([{"code":"E-SYN-0","severity":"error","message":"input syntax","span":null}]);
        validate_simulation_wire(&static_failure).unwrap();
        let mut bad = static_failure.clone();
        bad["bounds"]["cases"] = serde_json::json!(1);
        assert!(validate_simulation_wire(&bad).is_err());
        let mut bad = static_failure;
        bad["simulation"]["attempted_replications"] = serde_json::json!(1);
        assert!(validate_simulation_wire(&bad).is_err());
    }

    fn complete_arrivals() -> Value {
        let mut v = complete_report();
        v["bounds"]["cases"] = serde_json::json!(2);
        v["agency"]["unchecked"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!("multi_case_interference"));
        v["terminals"] = serde_json::json!([]);
        v["properties"] = serde_json::json!([
            {"id":"safety no_resource_overcommit","verdict":"satisfied"},
            {"id":"probability reached #1","verdict":"satisfied","cases":[
                {"case_id":1,"arrival":0,"verdict":"satisfied","probability":{"min":"1/2","max":"1/2"}},
                {"case_id":2,"arrival":1,"verdict":"satisfied","probability":{"min":"0","max":"0"}}
            ]}
        ]);
        v["case_outcomes"] = serde_json::json!([
            {"case_id":1,"arrival":0,"terminals":[{"kind":"end","end":"rejected","completed_at":{"min":2,"max":2},"elapsed":{"min":2,"max":2},"work":{"min":2,"max":2},"result_at":null,"probability":{"min":"1","max":"1"}}]},
            {"case_id":2,"arrival":1,"terminals":[{"kind":"rejected","end":null,"completed_at":null,"elapsed":null,"work":null,"result_at":null,"probability":{"min":"1","max":"1"}}]}
        ]);
        v["admission"] = serde_json::json!({"rejected_count":{"min":1,"max":1}});
        for (i, value) in [
            (0, "fixed[0min, 1min]"),
            (2, "fifo tie_by case_id"),
            (3, "1 on_overflow reject"),
        ] {
            v["assumptions"].as_array_mut().unwrap().push(serde_json::json!({"field":ARRIVAL_FIELDS[i],"value":value,"affects":ARRIVAL_AFFECTS[i]}));
        }
        v
    }
    fn independent_arrivals_wire(v: &Value) -> Result<(), String> {
        check(v, &report_schema(), "$")?;
        complete(v, &report_schema(), "$")?;
        validate_duration_assumptions(v)?;
        validate_arrivals_wire(v)
    }

    /// Fixed-policy rows built independently of the
    /// implementation: queue with `then call_order`, assign, classes, and the
    /// extended schedule text.
    fn extended_arrivals() -> Value {
        let mut v = complete_arrivals();
        let rows = v["assumptions"].as_array_mut().unwrap();
        assert_eq!(rows[0]["field"], "schedule");
        rows[0]["affects"] = serde_json::json!(POLICY_SCHEDULE_AFFECTS);
        let q = rows
            .iter()
            .position(|r| r["field"] == "policy.queue")
            .unwrap();
        rows[q]["value"] = serde_json::json!("fifo tie_by case_id then call_order");
        rows[q]["affects"] = serde_json::json!(POLICY_QUEUE_AFFECTS);
        rows.insert(q + 1, serde_json::json!({"field":"policy.assign","value":POLICY_ASSIGN_VALUE,"affects":POLICY_ASSIGN_AFFECTS}));
        rows.insert(q + 2, serde_json::json!({"field":"policy.classes","value":"policy[Clerk, Approver]; environment[Invoice]; party[Vendor]","affects":POLICY_CLASSES_AFFECTS}));
        v
    }
    fn edited(v: &Value, f: impl FnOnce(&mut Vec<Value>)) -> Value {
        let mut v = v.clone();
        f(v["assumptions"].as_array_mut().unwrap());
        v
    }
    fn at(rows: &[Value], field: &str) -> usize {
        rows.iter().position(|r| r["field"] == field).unwrap()
    }

    #[test]
    fn a_wire_policy_rows_are_admitted_only_with_the_extension_and_old_table_is_kept() {
        let legacy = complete_arrivals();
        independent_arrivals_wire(&legacy).unwrap();
        let classes = serde_json::json!({"field":"policy.classes","value":"policy[Clerk]; environment[]; party[]","affects":POLICY_CLASSES_AFFECTS});
        let assign = serde_json::json!({"field":"policy.assign","value":POLICY_ASSIGN_VALUE,"affects":POLICY_ASSIGN_AFFECTS});
        // A report without the extension keeps the old row and text: any one
        // new row or new text mixed in is rejected.
        let legacy_faults: Vec<(&str, Value)> = vec![
            (
                "classes row",
                edited(&legacy, |r| {
                    r.insert(at(r, "policy.queue") + 1, classes.clone())
                }),
            ),
            (
                "assign row",
                edited(&legacy, |r| {
                    r.insert(at(r, "policy.queue") + 1, assign.clone())
                }),
            ),
            (
                "then value with new text",
                edited(&legacy, |r| {
                    let q = at(r, "policy.queue");
                    r[q]["value"] = serde_json::json!("fifo tie_by case_id then call_order");
                    r[q]["affects"] = serde_json::json!(POLICY_QUEUE_AFFECTS);
                }),
            ),
            (
                "then value with old text",
                edited(&legacy, |r| {
                    let q = at(r, "policy.queue");
                    r[q]["value"] = serde_json::json!("fifo tie_by case_id then call_order");
                }),
            ),
            (
                "new queue text",
                edited(&legacy, |r| {
                    let q = at(r, "policy.queue");
                    r[q]["affects"] = serde_json::json!(POLICY_QUEUE_AFFECTS);
                }),
            ),
            (
                "extended schedule text",
                edited(&legacy, |r| {
                    r[0]["affects"] = serde_json::json!(POLICY_SCHEDULE_AFFECTS);
                }),
            ),
            (
                "fixed-policy scheduler",
                edited(&legacy, |r| {
                    r.push(serde_json::json!({"field":"probability.scheduler","value":"fixed-policy","affects":POLICY_FIXED_AFFECTS}));
                }),
            ),
            (
                "both rows with old queue and schedule",
                edited(&legacy, |r| {
                    let q = at(r, "policy.queue");
                    r.insert(q + 1, assign.clone());
                    r.insert(q + 2, classes.clone());
                }),
            ),
            (
                "unknown queue value",
                edited(&legacy, |r| {
                    let q = at(r, "policy.queue");
                    r[q]["value"] = serde_json::json!("fifo tie_by case_id assign first_idle");
                }),
            ),
        ];
        for (name, bad) in &legacy_faults {
            assert!(independent_arrivals_wire(bad).is_err(), "legacy: {name}");
        }
        let ext = extended_arrivals();
        independent_arrivals_wire(&ext).unwrap();
        let good: Vec<(&str, Value)> = vec![
            (
                "assign only",
                edited(&ext, |r| {
                    let q = at(r, "policy.queue");
                    r[q]["value"] = serde_json::json!("fifo tie_by case_id");
                    r[q]["affects"] = serde_json::json!(ARRIVAL_AFFECTS[2]);
                }),
            ),
            (
                "call_order only",
                edited(&ext, |r| {
                    r.remove(at(r, "policy.assign"));
                }),
            ),
            (
                "observable",
                edited(&ext, |r| {
                    let q = at(r, "policy.queue");
                    r[q]["value"] =
                        serde_json::json!("observable fifo tie_by case_id then call_order");
                }),
            ),
            (
                "empty policy class",
                edited(&ext, |r| {
                    let c = at(r, "policy.classes");
                    r[c]["value"] = serde_json::json!("policy[]; environment[Clerk]; party[]");
                    r[c]["affects"] = serde_json::json!(format!(
                        "{POLICY_CLASSES_AFFECTS}{POLICY_CLASSES_EMPTY_SUFFIX}"
                    ));
                }),
            ),
            (
                "fixed-policy scheduler",
                edited(&ext, |r| {
                    r.push(serde_json::json!({"field":"probability.scheduler","value":"fixed-policy","affects":POLICY_FIXED_AFFECTS}));
                }),
            ),
        ];
        let mut good = good;
        for suffix in POLICY_CLASSES_SUFFIXES {
            good.push((
                "classes suffix",
                edited(&ext, |r| {
                    let c = at(r, "policy.classes");
                    r[c]["affects"] =
                        serde_json::json!(format!("{POLICY_CLASSES_AFFECTS}{suffix}"));
                }),
            ));
        }
        for suffix in POLICY_ASSIGN_SUFFIXES {
            good.push((
                "assign suffix",
                edited(&ext, |r| {
                    let a = at(r, "policy.assign");
                    r[a]["affects"] = serde_json::json!(format!("{POLICY_ASSIGN_AFFECTS}{suffix}"));
                }),
            ));
        }
        for (name, v) in &good {
            independent_arrivals_wire(v).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        let ext_faults: Vec<(&str, Value)> = vec![
            (
                "no classes",
                edited(&ext, |r| {
                    r.remove(at(r, "policy.classes"));
                }),
            ),
            (
                "assign after classes",
                edited(&ext, |r| {
                    let a = at(r, "policy.assign");
                    r.swap(a, a + 1);
                }),
            ),
            (
                "classes after capacity",
                edited(&ext, |r| {
                    let c = at(r, "policy.classes");
                    let row = r.remove(c);
                    r.push(row);
                }),
            ),
            (
                "duplicate classes",
                edited(&ext, |r| {
                    let c = at(r, "policy.classes");
                    let row = r[c].clone();
                    r.insert(c, row);
                }),
            ),
            (
                "assign value",
                edited(&ext, |r| {
                    let a = at(r, "policy.assign");
                    r[a]["value"] = serde_json::json!("first_idle");
                }),
            ),
            (
                "assign text",
                edited(&ext, |r| {
                    let a = at(r, "policy.assign");
                    r[a]["affects"] = serde_json::json!(format!(
                        "{POLICY_ASSIGN_AFFECTS}{POLICY_CLASSES_EMPTY_SUFFIX}"
                    ));
                }),
            ),
            (
                "empty policy class without its suffix",
                edited(&ext, |r| {
                    let c = at(r, "policy.classes");
                    r[c]["value"] = serde_json::json!("policy[]; environment[Clerk]; party[]");
                }),
            ),
            (
                "empty-class suffix on a nonempty class",
                edited(&ext, |r| {
                    let c = at(r, "policy.classes");
                    r[c]["affects"] = serde_json::json!(format!(
                        "{POLICY_CLASSES_AFFECTS}{POLICY_CLASSES_EMPTY_SUFFIX}"
                    ));
                }),
            ),
            (
                "Actor in two classes",
                edited(&ext, |r| {
                    let c = at(r, "policy.classes");
                    r[c]["value"] = serde_json::json!("policy[Clerk]; environment[Clerk]; party[]");
                }),
            ),
            (
                "classes shape",
                edited(&ext, |r| {
                    let c = at(r, "policy.classes");
                    r[c]["value"] = serde_json::json!("policy[Clerk]; environment[]");
                }),
            ),
            (
                "old schedule text",
                edited(&ext, |r| {
                    r[0]["affects"] = serde_json::json!("time");
                }),
            ),
            (
                "then with old queue text",
                edited(&ext, |r| {
                    let q = at(r, "policy.queue");
                    r[q]["affects"] = serde_json::json!(ARRIVAL_AFFECTS[2]);
                }),
            ),
            (
                "full-information value with fixed text",
                edited(&ext, |r| {
                    r.push(serde_json::json!({"field":"probability.scheduler","value":"full-information","affects":POLICY_FIXED_AFFECTS}));
                }),
            ),
        ];
        for (name, bad) in &ext_faults {
            assert!(independent_arrivals_wire(bad).is_err(), "extended: {name}");
        }
        // The simulation wire uses the same rows before its fixed tail.
        let sim = complete_simulation();
        let head = |rows: Vec<Value>| {
            let mut v = sim.clone();
            let tail = v["assumptions"].as_array().unwrap().clone();
            v["assumptions"] = serde_json::json!(rows.into_iter().chain(tail).collect::<Vec<_>>());
            v
        };
        let queue = |value: &str, affects: &str| serde_json::json!({"field":"policy.queue","value":value,"affects":affects});
        validate_simulation_wire(&head(vec![queue(
            "fifo tie_by case_id",
            ARRIVAL_AFFECTS[2],
        )]))
        .unwrap();
        validate_simulation_wire(&head(vec![
            queue("fifo tie_by case_id then call_order", POLICY_QUEUE_AFFECTS),
            assign.clone(),
            classes.clone(),
        ]))
        .unwrap();
        for (name, rows) in [
            (
                "classes row",
                vec![
                    queue("fifo tie_by case_id", ARRIVAL_AFFECTS[2]),
                    classes.clone(),
                ],
            ),
            (
                "assign row",
                vec![
                    queue("fifo tie_by case_id", ARRIVAL_AFFECTS[2]),
                    assign.clone(),
                ],
            ),
            (
                "then without classes",
                vec![queue(
                    "fifo tie_by case_id then call_order",
                    POLICY_QUEUE_AFFECTS,
                )],
            ),
            (
                "then with old text",
                vec![
                    queue("fifo tie_by case_id then call_order", ARRIVAL_AFFECTS[2]),
                    classes.clone(),
                ],
            ),
            ("classes without queue", vec![classes.clone()]),
        ] {
            assert!(
                validate_simulation_wire(&head(rows)).is_err(),
                "simulation: {name}"
            );
        }
    }

    #[test]
    fn a_wire_closed_arrivals_schema_rejects_each_single_wire_fault() {
        let good = complete_arrivals();
        independent_arrivals_wire(&good).unwrap();
        // One mutation at a time, independently of production DTOs/serializers.
        for (pointer, replacement) in [
            ("/case_outcomes", serde_json::json!([])),
            ("/case_outcomes/1/case_id", serde_json::json!(1)),
            ("/case_outcomes/1/case_id", serde_json::json!(3)),
            ("/case_outcomes/1/arrival", serde_json::json!(0)),
            (
                "/case_outcomes/1/terminals/0/kind",
                serde_json::json!("overflow"),
            ),
            (
                "/case_outcomes/1/terminals/0/end",
                serde_json::json!("done"),
            ),
            (
                "/case_outcomes/1/terminals/0/work",
                serde_json::json!({"min":0,"max":0}),
            ),
            (
                "/case_outcomes/0/terminals/0/completed_at",
                serde_json::json!({"min":1,"max":2}),
            ),
            (
                "/properties/1/cases/0/probability/min",
                serde_json::json!("2/4"),
            ),
            ("/properties/1/cases/1/arrival", serde_json::json!(2)),
            (
                "/properties/1/cases/1/verdict",
                serde_json::json!("vacuous"),
            ),
            ("/properties/1/cases", serde_json::json!([])),
            (
                "/admission/rejected_count",
                serde_json::json!({"min":2,"max":1}),
            ),
            (
                "/admission/rejected_count",
                serde_json::json!({"min":1,"max":3}),
            ),
            (
                "/terminals",
                serde_json::json!([{"end":"done","elapsed":{"min":0,"max":0},"work":{"min":0,"max":0},"result_at":null}]),
            ),
            (
                "/assumptions/1/value",
                serde_json::json!("fixed[00min, 1min]"),
            ),
            (
                "/assumptions/2/affects",
                serde_json::json!("FIFO always deterministic"),
            ),
            (
                "/assumptions/3/value",
                serde_json::json!("01 on_overflow reject"),
            ),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                independent_arrivals_wire(&bad).is_err(),
                "accepted fault {pointer}: {bad}"
            );
        }
        for pointer in [
            "/admission",
            "/case_outcomes/1/terminals/0/result_at",
            "/properties/1/cases/0/probability",
        ] {
            let (parent, key) = pointer.rsplit_once('/').unwrap();
            let mut bad = good.clone();
            bad.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                independent_arrivals_wire(&bad).is_err(),
                "accepted omission {pointer}"
            );
        }
        let mut bad = good.clone();
        bad["case_outcomes"][0]["bogus"] = serde_json::json!(true);
        assert!(independent_arrivals_wire(&bad).is_err());
        let mut bad = good.clone();
        let duplicate = bad["case_outcomes"][0]["terminals"][0].clone();
        bad["case_outcomes"][0]["terminals"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(independent_arrivals_wire(&bad).is_err());
        let mut bad = good.clone();
        bad["assumptions"].as_array_mut().unwrap().swap(1, 2);
        assert!(independent_arrivals_wire(&bad).is_err());
        let mut old = complete_report();
        old["admission"] = Value::Null;
        assert!(validate_wire(&old).is_err(), "legacy null leakage");
        let mut old = complete_report();
        old["case_outcomes"] = Value::Null;
        assert!(validate_wire(&old).is_err());
    }

    #[test]
    fn arrivals_inventory_and_all_hand_expectations_are_closed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut names: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| {
                e.file_type().unwrap().is_dir()
                    && e.file_name().to_string_lossy().starts_with("arrivals-")
            })
            .map(|e| e.file_name().to_str().unwrap().to_owned())
            .collect();
        names.sort();
        assert_eq!(names, ARRIVALS_FIXTURES);
        let mut runtime = 0;
        for name in names {
            let h: Value = serde_json::from_str(
                &std::fs::read_to_string(root.join(&name).join("expected.json")).unwrap(),
            )
            .unwrap();
            validate_expected(&h).unwrap_or_else(|e| panic!("{name}: {e}"));
            if h.get("case_outcomes").is_some() {
                runtime += 1;
                assert_eq!(h["terminals"], serde_json::json!([]));
                assert_eq!(
                    h["case_outcomes"].as_array().unwrap().len() as u64,
                    h["bounds"]["cases"].as_u64().unwrap()
                );
            }
        }
        assert_eq!(runtime, 20);
    }

    #[test]
    fn arrivals_hand_shapes_calibrate_independently_of_pending_e_arr_registration() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let (mut runtime, mut rejected) = (0, 0);
        for name in ARRIVALS_FIXTURES {
            let mut h: Value = serde_json::from_str(
                &std::fs::read_to_string(root.join(name).join("expected.json")).unwrap(),
            )
            .unwrap();
            if h.get("case_outcomes").is_none() {
                rejected += 1;
                // This schema-control projection is not a changed expectation:
                // registration remains a separate RED obligation above.
                for d in h["diagnostics"].as_array_mut().unwrap() {
                    if d["code"] == "E-ARR-0" {
                        d["code"] = serde_json::json!("E-SYN-0");
                    }
                }
                validate_expected(&h).unwrap();
                continue;
            }
            runtime += 1;
            validate_expected(&h).unwrap_or_else(|e| panic!("{name}: {e}"));
            let source = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
            let mut full = complete_report();
            for k in [
                "verdict",
                "mode",
                "unit",
                "cut",
                "bounds",
                "properties",
                "terminals",
                "case_outcomes",
                "admission",
            ] {
                full[k] = h[k].clone();
            }
            full["agency"]["unchecked"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!("multi_case_interference"));
            let mut rows = vec![
                serde_json::json!({"field":"schedule","value":"work-conserving","affects":"control"}),
            ];
            let initial = name == "arrivals-initial-product";
            if initial {
                rows.push(serde_json::json!({"field":"latent.x","value":"dist(short=1/2, long=1/2)","affects":"control"}));
            }
            let arrivals = h["case_outcomes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| format!("{}min", c["arrival"].as_u64().unwrap()))
                .collect::<Vec<_>>()
                .join(", ");
            rows.push(serde_json::json!({"field":ARRIVAL_FIELDS[0],"value":format!("fixed[{arrivals}]"),"affects":ARRIVAL_AFFECTS[0]}));
            if initial {
                rows.push(serde_json::json!({"field":ARRIVAL_FIELDS[1],"value":"independent-per-admission","affects":ARRIVAL_AFFECTS[1]}));
            }
            if source.contains("policy fifo") {
                rows.push(serde_json::json!({"field":ARRIVAL_FIELDS[2],"value":"fifo tie_by case_id","affects":ARRIVAL_AFFECTS[2]}));
            }
            if let Some(tail) = source.split("queue capacity ").nth(1) {
                rows.push(serde_json::json!({"field":ARRIVAL_FIELDS[3],"value":format!("{} on_overflow reject",tail.split_whitespace().next().unwrap()),"affects":ARRIVAL_AFFECTS[3]}));
            }
            full["assumptions"] = serde_json::json!(rows);
            independent_arrivals_wire(&full).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        assert_eq!((runtime, rejected), (20, 4));
    }

    #[test]
    fn duration_inventory_and_hand_shapes_are_fixed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut names: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| {
                e.file_type().unwrap().is_dir()
                    && e.file_name().to_string_lossy().starts_with("duration-")
            })
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, DURATION_FIXTURES);
        let mut rejected = 0;
        for name in DURATION_FIXTURES {
            let h: Value = serde_json::from_str(
                &std::fs::read_to_string(root.join(name).join("expected.json")).unwrap(),
            )
            .unwrap();
            validate_expected(&h).unwrap_or_else(|e| panic!("{name}: {e}"));
            rejected += usize::from(h["verdict"] == "invalid");
        }
        assert_eq!(rejected, 2);
    }

    #[test]
    fn duration_assumption_validator_kills_bad_text_order_and_values() {
        let old = complete_report();
        let mut v = old.clone();
        let row = serde_json::json!({"field":"duration.A.sample","value":"dist(0min=0, 3min=7/10, 5min=3/10)","affects":DURATION_AFFECTS});
        v["assumptions"].as_array_mut().unwrap().push(row.clone());
        v["terminals"][0]["probability"] = serde_json::json!({"min":"1","max":"1"});
        validate_wire(&v).unwrap(); // duration alone, no probability query
        for text in [
            "dist()",
            "dist(5min=3/10, 3min=7/10)",
            "dist(3min=7/10,3min=3/10)",
            "dist(3min=14/20)",
            "dist(03min=1)",
            "dist(3sec=1)",
        ] {
            let mut bad = v.clone();
            bad["assumptions"]
                .as_array_mut()
                .unwrap()
                .last_mut()
                .unwrap()["value"] = text.into();
            assert!(validate_wire(&bad).is_err(), "{text}");
        }
        let mut bad = v.clone();
        bad["assumptions"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()["affects"] = "independent".into();
        assert!(validate_wire(&bad).is_err());
        let mut bad = v.clone();
        bad["assumptions"].as_array_mut().unwrap().push(row);
        assert!(validate_wire(&bad).is_err());
        let mut bad = v.clone();
        bad["assumptions"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"field":"kernel.Late","value":"given[]","affects":"x"}));
        assert!(validate_wire(&bad).is_err());
        let mut bad = v.clone();
        bad["properties"][0]["probability"] = serde_json::json!({"min":"1","max":"1"});
        assert!(validate_wire(&bad).is_err());
        let mut q = v.clone();
        q["properties"][0] = serde_json::json!({"id":"probability reached #1","verdict":"satisfied","probability":{"min":"1/2","max":"1/2"}});
        validate_wire(&q).unwrap(); // within queries use the same closed row schema
        for key in ["within", "expected", "mean", "quantile"] {
            let mut bad = q.clone();
            bad["properties"][0][key] = 3.into();
            assert!(validate_wire(&bad).is_err());
        }
        validate_wire(&old).unwrap();
        assert_eq!(ledger_shape::original_field_count(&old), 13);
    }

    #[test]
    fn probability_code_is_registered_error_not_warning_or_near_miss() {
        let expected = serde_json::json!({"diagnostics":[{"code":"E-PROB-0","severity":"error"}]});
        validate_expected(&expected).expect("ADR 0013 E-PROB-0 registration");
        let mut actual = complete_report();
        actual["diagnostics"][0]["code"] = "E-PROB-0".into();
        actual["diagnostics"][0]["severity"] = "error".into();
        compare_report(&expected, &actual).unwrap();
        for (code, severity) in [("E-PROB-0", "warning"), ("E-PROB-1", "error")] {
            let bad = serde_json::json!({"diagnostics":[{"code":code,"severity":severity}]});
            assert!(validate_expected(&bad).is_err());
            actual["diagnostics"][0]["code"] = code.into();
            actual["diagnostics"][0]["severity"] = severity.into();
            assert!(validate_wire(&actual).is_err());
        }
    }

    #[test]
    fn probability_schema_is_strict_in_partial_expectations_and_complete_shapes() {
        for (min, max) in [
            ("0", "1"),
            ("1/4", "3/4"),
            ("93/125000", "93/125000"),
            ("1", "1"),
        ] {
            let p = serde_json::json!({"min":min,"max":max});
            validate_probability(&p).unwrap();
            validate_expected(&serde_json::json!({"properties":[{"id":"probability reached #1","probability":p}]})).unwrap();
        }
        for bad in [
            serde_json::json!(null),
            serde_json::json!({}),
            serde_json::json!({"min":"0"}),
            serde_json::json!({"max":"1"}),
            serde_json::json!({"min":"0","max":"1","mean":"1/2"}),
            serde_json::json!({"min":0,"max":1}),
            serde_json::json!({"min":"0/1","max":"1"}),
            serde_json::json!({"min":"2/4","max":"1"}),
            serde_json::json!({"min":"1/0","max":"1"}),
            serde_json::json!({"min":"-1/2","max":"1"}),
            serde_json::json!({"min":"1/-2","max":"1"}),
            serde_json::json!({"min":"0.5","max":"1"}),
            serde_json::json!({"min":"01/2","max":"1"}),
            serde_json::json!({"min":"1/02","max":"1"}),
            serde_json::json!({"min":"1/1","max":"1"}),
            serde_json::json!({"min":"1/2","max":"1/3"}),
            serde_json::json!({"min":"2","max":"2"}),
            serde_json::json!({"min":"+0","max":"1"}),
        ] {
            assert!(validate_probability(&bad).is_err(), "{bad}");
            for key in ["properties", "terminals"] {
                let id = if key == "properties" { "id" } else { "end" };
                let mut row = serde_json::json!({"probability":bad});
                row[id] = "x".into();
                let mut e = serde_json::json!({});
                e[key] = serde_json::json!([row]);
                assert!(validate_expected(&e).is_err(), "{e}");
            }
        }
        // 10^1197: valid wire cannot be constrained to u64/u128 by the validator.
        let denominator = format!("1{}", "0".repeat(1197));
        let tiny = format!("1/{denominator}");
        validate_probability(&serde_json::json!({"min":tiny,"max":"1"})).unwrap();
        let even_numerator = format!("2/{}", denominator);
        assert!(fraction(&even_numerator).is_none());
        let just_below_half = format!("4{}/1{}", "9".repeat(90), "0".repeat(91));
        validate_probability(&serde_json::json!({"min":just_below_half,"max":"1/2"})).unwrap();
        assert!(
            validate_probability(&serde_json::json!({"min":"1/2","max":just_below_half})).is_err()
        );
    }

    #[test]
    fn probability_optional_wire_and_nonprobability_key_absence_are_not_null() {
        let old = complete_report();
        validate_wire(&old).unwrap();
        assert!(old["properties"][0].get("probability").is_none());
        assert!(old["terminals"][0].get("probability").is_none());
        for key in ["properties", "terminals"] {
            let mut bad = old.clone();
            bad[key][0]["probability"] = serde_json::json!({"min":"0","max":"1"});
            assert!(validate_wire(&bad).is_err());
            bad[key][0]["probability"] = Value::Null;
            assert!(validate_wire(&bad).is_err());
        }
        let mut new = old.clone();
        new["properties"][0]["id"] = "probability reached #1".into();
        for key in ["properties", "terminals"] {
            new[key][0]["probability"] = serde_json::json!({"min":"1/4","max":"3/4"});
        }
        check(&new, &report_schema(), "$").unwrap();
        complete(&new, &report_schema(), "$").unwrap();
        // Production round-trip must preserve, not silently discard, new fields.
        validate_wire(&new).expect("ADR 0013 production DTO probability fields");
        for key in ["properties", "terminals"] {
            let mut bad = new.clone();
            bad[key][0].as_object_mut().unwrap().remove("probability");
            assert!(validate_wire(&bad).is_err());
        }
    }

    #[test]
    fn probability_agency_vocabulary_is_closed_and_rules_version_unchanged() {
        let mut actual = complete_report();
        actual["agency"]["unchecked"]
            .as_array_mut()
            .unwrap()
            .push("kernel_correlations".into());
        compare_report(&serde_json::json!({}), &actual).unwrap();
        assert_eq!(actual["agency"]["rules"], "agency-p0-1");
        actual["agency"]["unchecked"] = serde_json::json!(["kernel_correlation"]);
        assert!(validate_wire(&actual).is_err());
        actual["agency"]["rules"] = "agency-p1-prob-1".into();
        assert!(validate_wire(&actual).is_err());
    }

    #[test]
    fn probability_inventory_and_all_hand_shapes_are_fixed_before_implementation() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut names: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| {
                e.file_type().unwrap().is_dir()
                    && e.file_name().to_string_lossy().starts_with("prob-")
            })
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, PROB_FIXTURES);
        assert_eq!(PROB_FIXTURES.len(), 16);
        let mut inputs = 0;
        for name in PROB_FIXTURES {
            for f in ["model.relca", "calc.md", "expected.json"] {
                assert!(root.join(name).join(f).is_file());
            }
            let literal = std::fs::read_to_string(root.join(name).join("expected.json")).unwrap();
            let h: Value = serde_json::from_str(&literal).unwrap();
            let invocation = if name.starts_with("prob-invoice") {
                inputs += 2;
                let v: Value = serde_json::from_str(
                    &std::fs::read_to_string(root.join(name).join("invocation.json")).unwrap(),
                )
                .unwrap();
                Invocation::parse(Some(&v)).unwrap()
            } else {
                inputs += 1;
                Invocation::LegacyCheck
            };
            // Schema-control copy alone uses a registered code to separate shape
            // calibration from the production E-PROB registration RED test.
            let control: Value =
                serde_json::from_str(&literal.replace("E-PROB-0", "E-CON-0")).unwrap();
            invocation
                .validate_expected(&control)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(h.get("verdict").is_some());
        }
        assert_eq!(inputs, 18);
    }

    #[test]
    fn state_policy_code_is_known_on_expected_and_wire_not_warning_or_near_miss() {
        let expected =
            serde_json::json!({"diagnostics":[{"code":"E-POL-LATENT","severity":"error"}]});
        validate_expected(&expected).expect("ADR 0012 diagnostic vocabulary");
        let mut actual = complete_report();
        actual["diagnostics"][0]["code"] = "E-POL-LATENT".into();
        actual["diagnostics"][0]["severity"] = "error".into();
        compare_report(&expected, &actual).expect("known error on complete wire");
        for (code, severity) in [("E-POL-LATENT", "warning"), ("E-POL-LATENT-1", "error")] {
            let bad = serde_json::json!({"diagnostics":[{"code":code,"severity":severity}]});
            assert!(validate_expected(&bad).is_err());
            actual["diagnostics"][0]["code"] = code.into();
            actual["diagnostics"][0]["severity"] = severity.into();
            assert!(compare_report(&serde_json::json!({}), &actual).is_err());
        }
    }

    #[test]
    fn state_agency_vocabulary_is_closed_including_complete_wire() {
        let mut actual = complete_report();
        actual["agency"]["rules"] = "agency-p1-state-1".into();
        actual["agency"]["unchecked"]
            .as_array_mut()
            .unwrap()
            .push("state_guards".into());
        compare_report(&serde_json::json!({}), &actual).unwrap();
        for (key, value) in [
            ("rules", serde_json::json!("agency-p1-state-next")),
            ("unchecked", serde_json::json!(["state_guard"])),
        ] {
            let mut bad = actual.clone();
            bad["agency"][key] = value;
            assert!(compare_report(&serde_json::json!({}), &bad).is_err());
        }
        for key in [
            "rules",
            "origin",
            "lineage",
            "lineage_groups",
            "refusal",
            "unchecked",
        ] {
            let mut bad = actual.clone();
            bad["agency"].as_object_mut().unwrap().remove(key);
            assert!(
                compare_report(&serde_json::json!({}), &bad).is_err(),
                "missing {key}"
            );
        }
        let mut bad = actual.clone();
        bad["agency"]["state"] = serde_json::json!([]);
        assert!(compare_report(&serde_json::json!({}), &bad).is_err());
    }

    #[test]
    fn state_fixture_inventory_and_all_hand_expectations_are_well_formed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut actual: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| {
                e.file_type().unwrap().is_dir()
                    && e.file_name().to_string_lossy().starts_with("state-")
            })
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        actual.sort();
        assert_eq!(actual, STATE_FIXTURES);
        assert_eq!(STATE_FIXTURES.len(), 16);
        for name in STATE_FIXTURES {
            for file in ["model.relca", "calc.md", "expected.json"] {
                assert!(root.join(name).join(file).is_file(), "{name}/{file}");
            }
            let expected: Value = serde_json::from_str(
                &std::fs::read_to_string(root.join(name).join("expected.json")).unwrap(),
            )
            .unwrap();
            validate_expected(&expected).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn state_hand_shapes_calibrate_independently_of_pending_diagnostic_registration() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        for name in STATE_FIXTURES {
            let literal = std::fs::read_to_string(root.join(name).join("expected.json")).unwrap();
            // Only this schema-control copy uses the already registered error.
            // Expectations and the runtime comparator still require E-POL-LATENT.
            let control: Value =
                serde_json::from_str(&literal.replace("E-POL-LATENT", "E-CON-0")).unwrap();
            validate_expected(&control).unwrap_or_else(|e| panic!("{name}: shape {e}"));
            if name.starts_with("state-agency") {
                check(&control["agency"], &nullable_agency(), "agency").unwrap();
                complete(&control["agency"], &nullable_agency(), "agency").unwrap();
            }
        }
    }

    #[test]
    fn finite_contract_code_is_known_error_and_not_a_warning() {
        let expected = serde_json::json!({"diagnostics":[{"code":"E-CON-0","severity":"error"}]});
        validate_expected(&expected).expect("ADR 0010 extends diagnostic vocabulary");
        assert!(
            validate_expected(&serde_json::json!({
                "diagnostics":[{"code":"E-CON-0","severity":"warning"}]
            }))
            .is_err()
        );
        assert!(
            validate_expected(&serde_json::json!({
                "diagnostics":[{"code":"E-CON-1","severity":"error"}]
            }))
            .is_err()
        );
    }

    fn complete_agency() -> Value {
        // ADR 0009/F3 hand literal, never obtained by serializing production.
        serde_json::json!({
            "rules":"agency-p0-1",
            "origin":[
                {"actor":"Invoice","direct":[],"indirect":[]},
                {"actor":"Reader","direct":["Invoice"],"indirect":[]},
                {"actor":"Reviewer","direct":["Invoice"],"indirect":["Invoice"]},
                {"actor":"Ledger","direct":[],"indirect":["Invoice"]}
            ],
            "lineage":[{"call":"approval","capability":"Reviewer.review","subject_sources":["Invoice"],"independent_sources":[]}],
            "lineage_groups":[{"subject_sources":["Invoice"],"judgements":1,"actors":1,"independent_sources":[]}],
            "refusal":[{"effect":"Ledger.post","call":"posted","reachable":true,"judgements":[
                {"call":"approval","capability":"Reviewer.review","stops":"all_paths","bypasses":0}
            ]}],
            "unchecked":["W-RESP-1","contest_path","fallback_path","executors","via_model"]
        })
    }

    #[test]
    fn agency_schema_independently_rejects_every_missing_wrong_and_extra_key() {
        let literal = complete_agency();
        let schema = nullable_agency();
        let valid = |value: &Value| {
            check(value, &schema, "agency").and_then(|()| complete(value, &schema, "agency"))
        };
        assert!(valid(&literal).is_ok());
        assert!(valid(&Value::Null).is_ok());
        let mut fields = Vec::new();
        object_fields(&literal, "", &mut fields);
        for path in fields {
            let (parent, key) = path.rsplit_once('/').unwrap();
            let mut changed = literal.clone();
            changed
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(valid(&changed).is_err(), "missing {path}");
            let mut changed = literal.clone();
            let value = changed.pointer_mut(&path).unwrap();
            *value = if value.is_string() {
                Value::Bool(true)
            } else {
                json_string()
            };
            assert!(valid(&changed).is_err(), "type {path}");
            let mut changed = literal.clone();
            changed
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unknown".into(), Value::Null);
            assert!(valid(&changed).is_err(), "extra at {parent}");
        }
        for (path, value) in [
            ("/rules", serde_json::json!("agency-p0-next")),
            ("/refusal/0/judgements/0/stops", serde_json::json!("always")),
            ("/refusal/0/judgements/0/bypasses", serde_json::json!(-1)),
            ("/refusal/0/judgements/0/bypasses", serde_json::json!(1.5)),
            ("/unchecked/0", serde_json::json!("agency")),
        ] {
            let mut changed = literal.clone();
            *changed.pointer_mut(path).unwrap() = value;
            assert!(valid(&changed).is_err(), "value {path}");
        }
    }

    fn json_string() -> Value {
        Value::String("wrong type".into())
    }

    #[test]
    fn agency_fixture_inventory_literals_and_single_faults_are_independent() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut actual: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| {
                e.file_type().unwrap().is_dir()
                    && e.file_name().to_string_lossy().starts_with("agency-")
            })
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        actual.sort();
        assert_eq!(actual, AGENCY_FIXTURES);
        assert_eq!(actual.len(), 9);
        for name in AGENCY_FIXTURES {
            let expected: Value = serde_json::from_str(
                &std::fs::read_to_string(root.join(name).join("expected.json")).unwrap(),
            )
            .unwrap();
            if name.starts_with("agency-refusal") {
                // Vocabulary registration is tested separately by the runner;
                // schema calibration must work even before production exists.
                for side in ["from", "to"] {
                    assert_eq!(expected["agency"][side], expected[side]["report"]["agency"]);
                    check(&expected["agency"][side], &nullable_agency(), "agency").unwrap();
                    complete(&expected["agency"][side], &nullable_agency(), "agency").unwrap();
                }
            } else {
                check(&expected["agency"], &nullable_agency(), "agency").unwrap();
                complete(&expected["agency"], &nullable_agency(), "agency").unwrap();
            }
            if let Some((base, _)) = name.split_once(".broken-") {
                let before = std::fs::read_to_string(root.join(base).join("model.relca")).unwrap();
                let after = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
                assert_eq!(before.lines().count(), after.lines().count(), "{name}");
                assert_eq!(
                    before
                        .lines()
                        .zip(after.lines())
                        .filter(|(a, b)| a != b)
                        .count(),
                    1,
                    "{name}: exactly one edited line"
                );
            }
        }
    }

    fn complete_report() -> Value {
        // Independent literal exercises every ADR 0002 object, including non-null
        // optional ranges/spans. Do not obtain the schema oracle from serialization.
        let mut v = serde_json::json!({
            "verdict": "satisfied", "mode": "exhaustive", "unit": "min",
            "actors": [], "cut_context": {"notices":CUT_NOTICES,"reentry":[],"horizon":[]},
            "agency": complete_agency(),
            "model": {"name": "Example", "sha256": "abc"}, "cut": "undeclared",
            "assumptions": [{"field": "schedule", "value": "work-conserving", "affects": "time"}],
            "diagnostics": [{"code": "W-CUT-0", "severity": "warning", "message": "Cut undeclared",
                             "span": {"start": 0, "end": 1}}],
            "bounds": {"cases": 1, "live_tasks": 2, "clock": 10, "truncated": false},
            "properties": [{"id": "reachable end.done", "verdict": "satisfied"}],
            "terminals": [{"end": "done", "elapsed": {"min": 4, "max": 4},
                           "work": {"min": 7, "max": 7}, "result_at": {"min": 3, "max": 3}}]
        });
        v["ledger"] = ledger_shape::control(&v);
        v["uncertainty"] = ledger_shape::uncertainty_wire::project(&v);
        v["ledger"] = ledger_shape::control(&v);
        v
    }

    #[test]
    fn ledger_migration_requires_closed_metadata_without_weakening_old_wire() {
        let good = complete_report();
        compare_report(&serde_json::json!({}), &good).unwrap();
        let mut missing = good.clone();
        missing["ledger"].as_object_mut().unwrap().remove("schema");
        assert!(compare_report(&serde_json::json!({}), &missing).is_err());
        let mut extra = good;
        extra["ledger"]["extra"] = serde_json::json!(0);
        assert!(compare_report(&serde_json::json!({}), &extra).is_err());
    }

    fn sync_completion_unchecked(v: &mut Value) {
        if v["bounds"]["cases"].is_u64()
            && (v["bounds"]["truncated"] == true || v["bounds"]["truncated"].is_null())
        {
            v["unchecked"] = serde_json::json!(["unreachable"]);
        } else {
            v.as_object_mut().unwrap().remove("unchecked");
        }
    }

    fn complete_diff() -> Value {
        serde_json::json!({
            "ledger": ledger_shape::control(&serde_json::json!({})),
            "uncertainty": ledger_shape::uncertainty_wire::project(&serde_json::json!({})),
            "verdict":"satisfied",
            "agency":{"from":complete_agency(),"to":complete_agency()},
            "from":{"configuration":"AsIs","report":complete_report()},
            "to":{"configuration":"ToBe","report":complete_report()},
            "mismatch":{"from_cut":"C","to_cut":"D","differences":[
                {"field":"question","from":null,"to":"new question"},
                {"field":"include","from":["Office"],"to":["Office","Extra"]},
                {"field":"exclude","from":[],"to":[{"target":"Payment","actor":false,"reason":"outside","loses":null}]}
            ]},
            "differences":[
                {"kind":"actor","target":"Extra","change":"added","reason":"new actor","loses":null},
                {"kind":"relation","target":"Extra.run","change":"removed","reason":null,"loses":"lost step"}
            ],
            "terminals":[{"end":"done","from":{"elapsed":{"min":8,"max":8},"work":{"min":8,"max":8},"result_at":null},"to":null}],
            "properties":[{"id":"reachable end.done","from":"satisfied","to":"violated"}],
            "diagnostics":[{"code":"U-SEM-0","severity":"error","message":"literal","span":null}],
            "checked":["comparison_mismatch"],"unchecked":["time_intent"]
        })
    }

    #[test]
    fn invocation_is_closed_and_dispatch_is_not_inferred_from_report_shape() {
        let path = std::path::Path::new("model.relca");
        assert_eq!(
            Invocation::parse(None).unwrap().args(path),
            ["check", "model.relca"]
        );
        for (value, args) in [
            (
                serde_json::json!({"command":"check","configuration":"A"}),
                vec!["check", "model.relca", "--configuration", "A"],
            ),
            (
                serde_json::json!({"command":"diff","from":"A","to":"B"}),
                vec!["diff", "model.relca", "--from", "A", "--to", "B"],
            ),
        ] {
            assert_eq!(Invocation::parse(Some(&value)).unwrap().args(path), args);
        }
        for value in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({"command":"compare","from":"A","to":"B"}),
            serde_json::json!({"command":"check","configuration":null}),
            serde_json::json!({"command":"check","configuration":1}),
            serde_json::json!({"command":"check","configuration":"A","to":"B"}),
            serde_json::json!({"command":"diff","from":"A"}),
            serde_json::json!({"command":"diff","from":"A","to":null}),
            serde_json::json!({"command":"diff","from":"A","to":"B","args":[]}),
        ] {
            assert!(Invocation::parse(Some(&value)).is_err(), "{value}");
        }
        let check = Invocation::LegacyCheck;
        assert!(
            check
                .validate_expected(&serde_json::json!({"from":{}}))
                .is_err()
        );
        let diff = Invocation::Diff {
            from: "A".into(),
            to: "B".into(),
        };
        assert!(
            diff.validate_expected(&serde_json::json!({"bounds":{}}))
                .is_err()
        );
    }

    #[test]
    fn diff_wire_requires_every_key_type_and_nullable_before_partial_matching() {
        let actual = complete_diff();
        assert!(compare_diff_report(&serde_json::json!({}), &actual).is_ok());
        assert_eq!(ledger_shape::original_field_count(&actual), 11);
        let mut fields = Vec::new();
        object_fields(&actual, "", &mut fields);
        for field in fields {
            let (parent, key) = field.rsplit_once('/').unwrap();
            let mut missing = actual.clone();
            missing
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                compare_diff_report(&serde_json::json!({}), &missing)
                    .unwrap_err()
                    .starts_with("wire:"),
                "missing {field}"
            );
            let mut wrong = actual.clone();
            let item = wrong.pointer_mut(&field).unwrap();
            *item = if item.is_null() {
                serde_json::json!([])
            } else if item.is_string() {
                Value::Bool(true)
            } else {
                Value::String("wrong type".into())
            };
            assert!(
                compare_diff_report(&serde_json::json!({}), &wrong).is_err(),
                "wrong type {field}"
            );
            let mut extra = actual.clone();
            extra
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unexpected".into(), Value::Null);
            assert!(
                compare_diff_report(&serde_json::json!({}), &extra).is_err(),
                "unknown key at {parent}"
            );
        }
        for path in [
            "/mismatch",
            "/differences/0/reason",
            "/differences/1/loses",
            "/terminals/0/from",
            "/from/report/bounds/truncated",
        ] {
            let mut nullable = actual.clone();
            *nullable.pointer_mut(path).unwrap() = Value::Null;
            if path == "/from/report/bounds/truncated" {
                sync_completion_unchecked(&mut nullable["from"]["report"]);
            }
            assert!(
                compare_diff_report(&serde_json::json!({}), &nullable).is_ok(),
                "null {path}"
            );
        }
    }

    #[test]
    fn diff_wire_validates_root_and_both_nested_diagnostic_vocabularies() {
        for scope in [
            "/diagnostics/0",
            "/from/report/diagnostics/0",
            "/to/report/diagnostics/0",
        ] {
            for code in relca_core::diag::CODES {
                let mut actual = complete_diff();
                let diagnostic = actual.pointer_mut(scope).unwrap();
                diagnostic["code"] = (*code).into();
                diagnostic["severity"] =
                    serde_json::to_value(relca_core::diag::severity_of(code)).unwrap();
                assert!(
                    compare_diff_report(&serde_json::json!({}), &actual).is_ok(),
                    "{scope} {code}"
                );
                let diagnostic = actual.pointer_mut(scope).unwrap();
                diagnostic["severity"] = if code.starts_with("W-") {
                    "error"
                } else {
                    "warning"
                }
                .into();
                assert!(
                    compare_diff_report(&serde_json::json!({}), &actual).is_err(),
                    "{scope} {code}"
                );
            }
            let mut actual = complete_diff();
            actual.pointer_mut(scope).unwrap()["code"] = "W-NOT-REGISTERED".into();
            assert!(
                compare_diff_report(&serde_json::json!({}), &actual).is_err(),
                "{scope}"
            );
        }
    }

    #[test]
    fn diff_wire_checks_verdict_kind_change_and_checked_vocabularies() {
        for from in [
            "satisfied",
            "violated",
            "inconclusive",
            "invalid",
            "unsupported",
            "vacuous",
        ] {
            for to in [
                "satisfied",
                "violated",
                "inconclusive",
                "invalid",
                "unsupported",
                "vacuous",
            ] {
                let mut actual = complete_diff();
                actual["from"]["report"]["verdict"] = from.into();
                actual["to"]["report"]["verdict"] = to.into();
                actual["properties"][0]["from"] = from.into();
                actual["properties"][0]["to"] = to.into();
                // Wire admissibility, not a claim to test the parent's fold.
                assert!(compare_diff_report(&serde_json::json!({}), &actual).is_ok());
            }
        }
        for (path, word) in [
            ("/verdict", "success"),
            ("/from/report/verdict", "success"),
            ("/properties/0/to", "success"),
            ("/differences/0/kind", "time"),
            ("/differences/0/change", "retired"),
            ("/checked/0", "agency"),
            ("/unchecked/0", "anything_else"),
        ] {
            let mut actual = complete_diff();
            *actual.pointer_mut(path).unwrap() = word.into();
            assert!(
                compare_diff_report(&serde_json::json!({}), &actual).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    fn diff_sets_use_kind_target_not_target_or_order_and_enforce_exact_membership() {
        let mut actual = complete_diff();
        // Deliberately same target, different kind: a structural wire test,
        // independent of what semantic target spellings lower will produce.
        actual["differences"][1]["target"] = "Extra".into();
        let mut expected = serde_json::json!({"differences":actual["differences"]});
        expected["differences"].as_array_mut().unwrap().reverse();
        assert!(compare_diff_report(&expected, &actual).is_ok());
        for side in ["expected", "actual"] {
            for fault in [
                "missing-key",
                "duplicate",
                "missing-item",
                "extra-item",
                "direction",
            ] {
                let mut expected = expected.clone();
                let mut actual = actual.clone();
                let changed = if side == "expected" {
                    &mut expected
                } else {
                    &mut actual
                };
                let items = changed["differences"].as_array_mut().unwrap();
                match fault {
                    "missing-key" => {
                        items[0].as_object_mut().unwrap().remove("kind");
                    }
                    "duplicate" => items.push(items[0].clone()),
                    "missing-item" => {
                        items.pop();
                    }
                    "extra-item" => {
                        let mut item = items[0].clone();
                        item["target"] = "Other".into();
                        items.push(item);
                    }
                    "direction" => {
                        let change = &mut items[0]["change"];
                        *change = if *change == "added" {
                            "removed"
                        } else {
                            "added"
                        }
                        .into();
                    }
                    _ => unreachable!(),
                }
                assert!(
                    compare_diff_report(&expected, &actual).is_err(),
                    "{side} {fault}"
                );
            }
        }
    }

    #[test]
    fn diff_cut_details_are_separate_from_actor_relation_differences() {
        let actual = complete_diff();
        let expected = serde_json::json!({"mismatch":{"differences":[
            {"field":"exclude","to":[{"target":"Payment","loses":null}]},
            {"field":"include","from":["Office"]},
            {"field":"question","from":null,"to":"new question"}
        ]}});
        assert!(compare_diff_report(&expected, &actual).is_ok());
        for details in [
            serde_json::json!([{"kind":"actor","target":"Office"}]),
            serde_json::json!([{"field":"include","from":"Office"}]),
            serde_json::json!([{"field":"question","to":[]}]),
            serde_json::json!([{"field":"unknown"}]),
        ] {
            assert!(
                validate_diff_expected(&serde_json::json!({"mismatch":{"differences":details}}))
                    .is_err()
            );
        }
        let mut reordered = actual.clone();
        reordered["mismatch"]["differences"][1]["to"] = serde_json::json!(["Extra", "Office"]);
        reordered["mismatch"]["differences"][2]["to"]
            .as_array_mut()
            .unwrap()
            .push(
                serde_json::json!({"target":"Tax","actor":false,"reason":"outside","loses":null}),
            );
        let expected = serde_json::json!({"mismatch":{"differences":[
            {"field":"question","from":null,"to":"new question"},
            {"field":"include","from":["Office"],"to":["Office","Extra"]},
            {"field":"exclude","from":[],"to":[{"target":"Tax"},{"target":"Payment"}]}
        ]}});
        assert!(compare_diff_report(&expected, &reordered).is_ok());
        reordered["mismatch"]["differences"][2]["to"][1]["target"] = "Payment".into();
        assert!(
            compare_diff_report(&expected, &reordered).is_err(),
            "duplicate target"
        );
    }

    #[test]
    fn diff_negative_diagnostics_are_scoped_and_metadata_never_appears_on_wire() {
        let mut actual = complete_diff();
        let expected = serde_json::json!({"from":{"report":{"absent_diagnostics":["W-DEADLOCK-0"]}},"absent_diagnostics":["W-DEADLOCK-0"]});
        assert!(compare_diff_report(&expected, &actual).is_ok());
        let diagnostic = serde_json::json!({"code":"W-DEADLOCK-0","severity":"warning","message":"injected","span":null});
        actual["to"]["report"]["diagnostics"]
            .as_array_mut()
            .unwrap()
            .push(diagnostic.clone());
        assert!(
            compare_diff_report(&expected, &actual).is_ok(),
            "to diagnostic does not violate from/root exclusion"
        );
        actual["from"]["report"]["diagnostics"]
            .as_array_mut()
            .unwrap()
            .push(diagnostic);
        assert!(
            compare_diff_report(&expected, &actual)
                .unwrap_err()
                .contains("$.from.report.absent_diagnostics")
        );
        for scope in ["", "/from/report", "/to/report"] {
            let mut actual = complete_diff();
            actual.pointer_mut(scope).unwrap()["absent_diagnostics"] = serde_json::json!([]);
            assert!(
                compare_diff_report(&serde_json::json!({}), &actual).is_err(),
                "{scope}"
            );
        }
    }

    #[test]
    fn configuration_fixture_inventory_schema_echo_and_truncation_are_calibrated() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut count = 0;
        for entry in std::fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if !name.to_string_lossy().starts_with("config-diff-") {
                continue;
            }
            count += 1;
            let directory = entry.path();
            let expected: Value = serde_json::from_str(
                &std::fs::read_to_string(directory.join("expected.json")).unwrap(),
            )
            .unwrap();
            let invocation_path = directory.join("invocation.json");
            let invocation_value: Option<Value> = invocation_path.exists().then(|| {
                serde_json::from_str(&std::fs::read_to_string(invocation_path).unwrap()).unwrap()
            });
            let invocation = Invocation::parse(invocation_value.as_ref()).unwrap();
            assert!(
                invocation
                    .compare(&serde_json::json!({}), &expected)
                    .unwrap_err()
                    .starts_with("wire:"),
                "echo {name:?}"
            );
            match invocation {
                Invocation::Scenario { .. } => {
                    panic!("scenario cannot enter fixed configuration population")
                }
                Invocation::Explain => {
                    panic!("explain cannot enter fixed configuration population");
                }
                Invocation::Mutate { .. } => {
                    panic!("mutation cannot enter fixed configuration population")
                }
                Invocation::Diff { .. } => {
                    for side in ["from", "to"] {
                        let truncated = &expected[side]["report"]["bounds"]["truncated"];
                        assert!(
                            expected[side]["report"]["bounds"]
                                .get("truncated")
                                .is_some(),
                            "{name:?} {side}"
                        );
                        let expectation =
                            serde_json::json!({side:{"report":{"bounds":{"truncated":truncated}}}});
                        let mut actual = complete_diff();
                        actual[side]["report"]["bounds"]["truncated"] = truncated.clone();
                        sync_completion_unchecked(&mut actual[side]["report"]);
                        assert!(compare_diff_report(&expectation, &actual).is_ok());
                        actual[side]["report"]["bounds"]["truncated"] = truncated
                            .as_bool()
                            .map(|v| Value::Bool(!v))
                            .unwrap_or(Value::Bool(false));
                        sync_completion_unchecked(&mut actual[side]["report"]);
                        assert!(
                            compare_diff_report(&expectation, &actual)
                                .unwrap_err()
                                .contains("bounds.truncated")
                        );
                    }
                }
                Invocation::LegacyCheck => assert!(expected["bounds"].get("truncated").is_some()),
                Invocation::Check { .. } => panic!("unexpected fixture invocation"),
                Invocation::Simulate { .. } => {
                    panic!("simulation cannot enter the fixed configuration population")
                }
                Invocation::Compare { .. } => {
                    panic!("compare cannot enter fixed configuration population")
                }
            }
        }
        assert_eq!(count, 12, "P1-P5 plus B1-B7");
    }

    #[test]
    fn all_configuration_expectations_validate_with_registered_vocabulary() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        for entry in std::fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            if !entry
                .file_name()
                .to_string_lossy()
                .starts_with("config-diff-")
            {
                continue;
            }
            let directory = entry.path();
            let expected: Value = serde_json::from_str(
                &std::fs::read_to_string(directory.join("expected.json")).unwrap(),
            )
            .unwrap();
            let invocation = directory.join("invocation.json");
            let value: Option<Value> = invocation.exists().then(|| {
                serde_json::from_str(&std::fs::read_to_string(invocation).unwrap()).unwrap()
            });
            Invocation::parse(value.as_ref())
                .unwrap()
                .validate_expected(&expected)
                .unwrap_or_else(|error| panic!("{}: {error}", directory.display()));
        }
    }

    fn fixture_expectations() -> Vec<(std::path::PathBuf, Value)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut expectations = Vec::new();
        for name in LEGACY_FIXTURES {
            let directory = root.join(name);
            assert!(
                !directory.join("invocation.json").exists(),
                "legacy fixture must use default check: {name}"
            );
            let path = directory.join("expected.json");
            let expected = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            expectations.push((path, expected));
        }
        assert_eq!(expectations.len(), 16, "Phase 0 fixture inventory changed");
        expectations
    }

    fn object_fields(value: &Value, path: &str, fields: &mut Vec<String>) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    let path = format!("{path}/{key}");
                    fields.push(path.clone());
                    object_fields(value, &path, fields);
                }
            }
            Value::Array(array) => {
                for (i, value) in array.iter().enumerate() {
                    object_fields(value, &format!("{path}/{i}"), fields);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn wire_requires_every_top_level_and_nested_key_and_type_before_partial_matching() {
        let actual = complete_report();
        assert!(compare_report(&serde_json::json!({}), &actual).is_ok());
        let mut fields = Vec::new();
        object_fields(&actual, "", &mut fields);
        assert_eq!(ledger_shape::original_field_count(&actual), 13);
        for field in fields {
            // This is the unchanged old-field type matrix. Ledger's tagged,
            // nullable unions have their own closed-shape and Q3 wire matrix;
            // replacing a requested null with a string is not a type fault.
            if field.starts_with("/ledger/") {
                continue;
            }
            let (parent, key) = field.rsplit_once('/').unwrap();
            let mut missing = actual.clone();
            missing
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            let error = compare_report(&serde_json::json!({}), &missing).unwrap_err();
            assert!(error.starts_with("wire:"), "missing {field}: {error}");

            let mut wrong_type = actual.clone();
            let value = wrong_type.pointer_mut(&field).unwrap();
            *value = if value.is_string() {
                Value::Bool(true)
            } else {
                Value::String("wrong type".into())
            };
            let error = compare_report(&serde_json::json!({}), &wrong_type).unwrap_err();
            assert!(error.starts_with("wire:"), "wrong type {field}: {error}");
        }
        for (field, value) in [
            ("/verdict", "success"),
            ("/mode", "simulation"),
            ("/unit", "hour"),
            ("/cut", "declared"),
            ("/diagnostics/0/severity", "info"),
            ("/properties/0/verdict", "success"),
        ] {
            let mut invalid = actual.clone();
            *invalid.pointer_mut(field).unwrap() = Value::String(value.into());
            assert!(
                compare_report(&serde_json::json!({}), &invalid)
                    .unwrap_err()
                    .starts_with("wire:")
            );
        }
        let mut extra = actual.clone();
        extra["absent_diagnostics"] = serde_json::json!([]);
        assert!(
            compare_report(&serde_json::json!({}), &extra)
                .unwrap_err()
                .starts_with("wire:")
        );
    }

    #[test]
    fn wire_requires_explicit_nullable_keys_but_accepts_null() {
        let mut actual = complete_report();
        for field in [
            "/agency",
            "/model/name",
            "/model/sha256",
            "/bounds/cases",
            "/bounds/live_tasks",
            "/bounds/clock",
            "/bounds/truncated",
            "/diagnostics/0/span",
            "/terminals/0/result_at",
        ] {
            *actual.pointer_mut(field).unwrap() = Value::Null;
        }
        assert!(compare_report(&serde_json::json!({}), &actual).is_ok());
        let mut fields = Vec::new();
        object_fields(&actual, "", &mut fields);
        for field in fields
            .into_iter()
            .filter(|field| actual.pointer(field).unwrap().is_null())
        {
            let mut missing = actual.clone();
            let (parent, key) = field.rsplit_once('/').unwrap();
            missing
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                compare_report(&serde_json::json!({}), &missing)
                    .unwrap_err()
                    .starts_with("wire:"),
                "{field}"
            );
        }
    }

    #[test]
    fn fake_implementation_echoing_each_expected_json_fails_wire_validation() {
        for (path, expected) in fixture_expectations() {
            let error = compare_report(&expected, &expected).unwrap_err();
            assert!(error.starts_with("wire:"), "{}: {error}", path.display());
        }
    }

    #[test]
    fn each_fixture_truncation_expectation_rejects_only_that_field_changing() {
        for (path, expected) in fixture_expectations() {
            let truncated = expected
                .pointer("/bounds/truncated")
                .expect("truncation expectation required");
            let expected = serde_json::json!({"bounds": {"truncated": truncated}});
            let mut actual = complete_report();
            actual["bounds"]["truncated"] = truncated.clone();
            sync_completion_unchecked(&mut actual);
            assert!(
                compare_report(&expected, &actual).is_ok(),
                "{}",
                path.display()
            );
            actual["bounds"]["truncated"] = match truncated.as_bool() {
                Some(value) => Value::Bool(!value),
                None => Value::Bool(false),
            };
            sync_completion_unchecked(&mut actual);
            let error = compare_report(&expected, &actual).unwrap_err();
            assert!(
                error.starts_with("$.bounds.truncated:"),
                "{}: {error}",
                path.display()
            );
        }
    }

    #[test]
    fn deadlock_exclusions_reject_only_the_added_diagnostic() {
        let mut checked = 0;
        for (path, expected) in fixture_expectations() {
            let Some(absent) = expected.get("absent_diagnostics") else {
                continue;
            };
            // Static-error Cut fixtures exclude only W-CUT-0; the rest of the
            // fixtures with exclusions must exclude deadlock.
            if !absent
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("W-DEADLOCK-0"))
            {
                assert_eq!(
                    expected["verdict"],
                    serde_json::json!("invalid"),
                    "{}",
                    path.display()
                );
                continue;
            }
            let absent = serde_json::json!(["W-DEADLOCK-0"]);
            let expected = serde_json::json!({"diagnostics": [{"code": "W-CUT-0"}], "absent_diagnostics": absent});
            let mut actual = complete_report();
            assert!(compare_report(&expected, &actual).is_ok());
            actual["diagnostics"].as_array_mut().unwrap().push(serde_json::json!({
            "code": "W-DEADLOCK-0", "severity": "warning", "message": "fake deadlock", "span": null
        }));
            let error = compare_report(&expected, &actual).unwrap_err();
            assert_eq!(
                error,
                "absent_diagnostics: forbidden diagnostic W-DEADLOCK-0"
            );
            // Inclusion remains permissive when no negative constraint is specified.
            assert!(
                compare_report(
                    &serde_json::json!({"diagnostics": [{"code": "W-CUT-0"}]}),
                    &actual
                )
                .is_ok()
            );
            checked += 1;
        }
        assert_eq!(checked, 10);
    }

    #[test]
    fn absent_diagnostics_is_optional_strictly_typed_and_checks_every_code() {
        let mut actual = complete_report();
        actual["diagnostics"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "code": "W-DEADLOCK-0", "severity": "warning", "message": "deadlock", "span": null
            }));
        for expected in [
            serde_json::json!({}),
            serde_json::json!({"absent_diagnostics": []}),
        ] {
            assert!(compare_report(&expected, &actual).is_ok());
        }
        let expected = serde_json::json!({"absent_diagnostics": ["E-CAP-0", "W-DEADLOCK-0"]});
        assert!(
            compare_report(&expected, &actual)
                .unwrap_err()
                .contains("forbidden diagnostic W-DEADLOCK-0")
        );
        for absent in [
            Value::Null,
            Value::Bool(true),
            serde_json::json!("W-DEADLOCK-0"),
            serde_json::json!([1]),
            serde_json::json!([{}]),
        ] {
            let expected = serde_json::json!({"absent_diagnostics": absent});
            assert!(
                compare_report(&expected, &actual)
                    .unwrap_err()
                    .starts_with("$.absent_diagnostics")
            );
        }
    }

    #[test]
    fn wire_rejects_codes_outside_the_vocabulary_and_wrong_severity() {
        assert!(compare_report(&serde_json::json!({}), &complete_report()).is_ok());
        for (code, severity, error) in [
            ("W-FOO-0", "warning", "unknown diagnostic code W-FOO-0"),
            ("W-CUT-0", "error", "W-CUT-0 must have severity \"warning\""),
            ("E-CAP-0", "warning", "E-CAP-0 must have severity \"error\""),
            ("U-SEM-0", "warning", "U-SEM-0 must have severity \"error\""),
        ] {
            let mut actual = complete_report();
            actual["diagnostics"][0]["code"] = code.into();
            actual["diagnostics"][0]["severity"] = severity.into();
            // The closed
            // wire schema now runs first and prefixes the JSON path, so pin
            // the class of error and its location, not the exact prefix.
            let message = compare_report(&serde_json::json!({}), &actual).unwrap_err();
            assert!(
                message.starts_with("wire: $.diagnostics[0]") && message.ends_with(error),
                "{message}"
            );
        }
    }

    #[test]
    fn expectations_name_only_vocabulary_codes() {
        for code in relca_core::diag::CODES {
            let expected = serde_json::json!({
                "absent_diagnostics": [code], "diagnostics": [{"code": code}]
            });
            assert!(validate_expected(&expected).is_ok(), "{code}");
        }
        for expected in [
            serde_json::json!({"absent_diagnostics": ["W-DEADLOK-0"]}),
            serde_json::json!({"diagnostics": [{"code": "E-CAPS-0"}]}),
            serde_json::json!({"diagnostics": [{"severity": "error"}]}),
            serde_json::json!({"diagnostics": {}}),
            serde_json::json!([]),
        ] {
            assert!(validate_expected(&expected).is_err(), "{expected}");
            assert!(compare_report(&expected, &complete_report()).is_err());
        }
    }

    #[test]
    fn every_expectation_field_is_type_checked_before_comparison() {
        for expected in [
            serde_json::json!({"verdict": 17}),
            serde_json::json!({"verdict": "fine"}),
            serde_json::json!({"verdict": "satisfied", "diagnostics": [{"code": "W-CUT-0", "severity": 17}]}),
            serde_json::json!({"diagnostics": [{"code": "W-CUT-0", "severity": "error"}]}),
            serde_json::json!({"diagnostics": [{"code": "W-CUT-0", "span": {"start": "0"}}]}),
            serde_json::json!({"bounds": {"truncated": "no"}}),
            serde_json::json!({"bounds": {"horizon": 3}}),
            serde_json::json!({"terminals": [{"end": "done", "elapsed": {"min": -1}}]}),
            serde_json::json!({"terminals": [{"end": "done", "result_at": 3}]}),
            serde_json::json!({"properties": [{"id": "x", "verdict": "ok"}]}),
            serde_json::json!({"cut": "declared"}),
            serde_json::json!({"cut": {"name": "C", "scope": []}}),
            serde_json::json!({"cut": {"exclude": [{"target": "P", "actor": "no"}]}}),
            serde_json::json!({"mode": "simulation"}),
            serde_json::json!({"undetected": true}),
            serde_json::json!({"properties": [{"verdict": "satisfied"}]}),
            serde_json::json!({"properties": [{"id": "a"}, {"id": "a"}]}),
            serde_json::json!({"terminals": [{"end": "done"}, {"end": "done"}]}),
            serde_json::json!({"terminals": [{"elapsed": {"min": 1, "max": 1}}]}),
        ] {
            assert!(validate_expected(&expected).is_err(), "{expected}");
        }
        for expected in [
            serde_json::json!({}),
            serde_json::json!({"verdict": "vacuous", "bounds": {"truncated": null}}),
            serde_json::json!({"terminals": [{"end": "done", "result_at": null}]}),
            serde_json::json!({"cut": {"name": "C", "exclude": [{"loses": null}]}}),
            serde_json::json!({"diagnostics": [{"code": "E-CUT-1", "severity": "error"}]}),
        ] {
            assert!(validate_expected(&expected).is_ok(), "{expected}");
        }
        assert!(validate_expected(&complete_report()).is_ok());
    }

    #[test]
    fn partial_objects_require_keys_and_strict_values() {
        let actual = serde_json::json!({"value":8,"extra":true,"nested":{"a":null}});
        assert!(compare(&serde_json::json!({"value":8}), &actual, "$").is_ok());
        for expected in [
            serde_json::json!({"missing":null}),
            serde_json::json!({"value":9}),
            serde_json::json!({"value":"8"}),
            serde_json::json!({"nested":{"a":0}}),
        ] {
            assert!(compare(&expected, &actual, "$").is_err());
        }
    }

    #[test]
    fn terminal_arrays_match_by_end_and_require_exact_sets() {
        let actual = serde_json::json!({"terminals":[{"end":"b","work":{"min":7}}, {"end":"a","work":{"min":8}}]});
        let expected = serde_json::json!({"terminals":[{"end":"a","work":{"min":8}}, {"end":"b"}]});
        assert!(compare(&expected, &actual, "$").is_ok());
        for expected in [
            serde_json::json!({"terminals":[]}),
            serde_json::json!({"terminals":[{"end":"a"}]}),
            serde_json::json!({"terminals":[{"end":"a"},{"end":"c"}]}),
            serde_json::json!({"terminals":[{"end":"a"},{"end":"a"}]}),
            serde_json::json!({"terminals":[{"end":"a","work":{"min":9}},{"end":"b"}]}),
        ] {
            assert!(compare(&expected, &actual, "$").is_err());
        }
        assert!(
            compare(
                &expected,
                &serde_json::json!({"terminals":[{"end":"a"}]}),
                "$"
            )
            .is_err()
        );
        assert!(
            compare(
                &expected,
                &serde_json::json!({"terminals":[{"end":"a"},{"end":"a"}]}),
                "$"
            )
            .is_err()
        );
    }

    #[test]
    fn properties_match_by_id_as_exact_sets() {
        let expected =
            serde_json::json!({"properties":[{"id":"a","verdict":"satisfied"},{"id":"b"}]});
        let actual =
            serde_json::json!({"properties":[{"id":"b"},{"id":"a","verdict":"satisfied"}]});
        assert!(compare(&expected, &actual, "$").is_ok());
        assert!(compare(&serde_json::json!({"properties":[]}), &actual, "$").is_err());
        assert!(
            compare(
                &expected,
                &serde_json::json!({"properties":[{"id":"a","verdict":"violated"},{"id":"b"}]}),
                "$"
            )
            .is_err()
        );
    }

    #[test]
    fn diagnostics_require_code_inclusion_but_allow_extras() {
        let actual = serde_json::json!({"diagnostics":[{"code":"W-CUT-0","severity":"warning"},{"code":"E-CAP-0"}]});
        assert!(
            compare(
                &serde_json::json!({"diagnostics":[{"code":"E-CAP-0"},{"code":"W-CUT-0"}]}),
                &actual,
                "$"
            )
            .is_ok()
        );
        assert!(
            compare(
                &serde_json::json!({"diagnostics":[{"code":"W-CUT-0"}]}),
                &actual,
                "$"
            )
            .is_ok()
        );
        assert!(compare(&serde_json::json!({"diagnostics":[]}), &actual, "$").is_ok());
        assert!(
            compare(
                &serde_json::json!({"diagnostics":[{"code":"E-TYPE-0"}]}),
                &actual,
                "$"
            )
            .is_err()
        );
        assert!(
            compare(
                &serde_json::json!({"diagnostics":[{"code":"W-CUT-0","severity":"error"}]}),
                &actual,
                "$"
            )
            .is_err()
        );
        assert!(compare(&serde_json::json!({"diagnostics":[{}]}), &actual, "$").is_err());
    }

    #[test]
    fn other_arrays_preserve_length_and_order() {
        assert!(
            compare(
                &serde_json::json!([1, null, "a"]),
                &serde_json::json!([1, null, "a"]),
                "$"
            )
            .is_ok()
        );
        assert!(compare(&serde_json::json!([1, 2]), &serde_json::json!([2, 1]), "$").is_err());
        assert!(compare(&serde_json::json!([]), &serde_json::json!([1]), "$").is_err());
        assert!(
            compare(
                &serde_json::json!({"terminals":[]}),
                &serde_json::json!({"terminals":[]}),
                "$"
            )
            .is_ok()
        );
    }

    #[test]
    fn actor_cut_required_wire_keys_and_declared_debtor_are_independently_calibrated() {
        use serde_json::json;
        let good = json!({"actors":[],"cut":"undeclared","cut_context":{"notices":CUT_NOTICES,"reentry":[],"horizon":[]}});
        validate_actor_context_wire(&good).unwrap();
        for key in ["actors", "cut_context"] {
            let mut bad = good.clone();
            bad.as_object_mut().unwrap().remove(key);
            assert!(
                validate_actor_context_wire(&bad).is_err(),
                "required {key}, not OptionalField"
            );
        }
        for key in ["notices", "reentry", "horizon"] {
            let mut bad = good.clone();
            bad["cut_context"].as_object_mut().unwrap().remove(key);
            assert!(
                validate_actor_context_wire(&bad).is_err(),
                "missing context.{key}"
            );
        }
        let mut bad = good;
        bad["cut_context"]["extra"] = json!(true);
        assert!(validate_actor_context_wire(&bad).is_err());
        let ledger = json!({"bounds":{"cases":1,"truncated":false},"properties":[{"id":"contract C #1 response","verdict":"satisfied"}],"obligations":[{"id":"case1:response1:received","response_id":"contract C #1 response","response_label":null,"case_id":1,"trigger_site":"received","trigger":"case.received","targets":["A.run.finish"],"debtor":"q","debtor_status":"declared","non_waivable":false,"coverage":"complete","outcomes":[{"status":"fulfilled","triggered_at":0,"deadline":2,"resolved_at":1,"reason":null,"by_clause":null,"executor":null,"authority":null}]}]});
        validate_obligations_wire(&ledger).unwrap();
        for (key, value) in [
            ("debtor", Value::Null),
            ("debtor_status", json!("not_configured")),
            ("debtor_status", json!("inferred")),
            ("debtor", json!("")),
        ] {
            let mut bad = ledger.clone();
            bad["obligations"][0][key] = value;
            assert!(
                validate_obligations_wire(&bad).is_err(),
                "debtor/status consistency {key}"
            );
        }
        let mut old = ledger;
        old["obligations"][0]["debtor"] = Value::Null;
        old["obligations"][0]["debtor_status"] = json!("not_configured");
        validate_obligations_wire(&old).unwrap();
    }
}

/// ADR 0030: appended finite population, seventeen pairs.
#[allow(dead_code)]
pub const PERCASE_FIXTURES: [&str; 34] = [
    "pcase-capability",
    "pcase-capability.broken-capability",
    "pcase-capacity",
    "pcase-capacity.broken-capacity",
    "pcase-case-product",
    "pcase-case-product.broken-fixed-case",
    "pcase-common",
    "pcase-common.broken-one-reader",
    "pcase-executors",
    "pcase-executors.broken-executors",
    "pcase-explain",
    "pcase-explain.broken-invariant",
    "pcase-joint",
    "pcase-joint.broken-half",
    "pcase-keys",
    "pcase-keys.broken-unmarked-single",
    "pcase-late",
    "pcase-late.broken-clock",
    "pcase-latent-policy",
    "pcase-latent-policy.broken-policy",
    "pcase-marginal",
    "pcase-marginal.broken-error-row",
    "pcase-nondet",
    "pcase-nondet.broken-fixed",
    "pcase-optin",
    "pcase-optin.broken-unmarked",
    "pcase-reject",
    "pcase-reject.broken-zero-queue",
    "pcase-same-document",
    "pcase-same-document.broken-second-kernel",
    "pcase-shared-fixed",
    "pcase-shared-fixed.broken-marker",
    "pcase-write-ir",
    "pcase-write-ir.broken-override",
];

/// ADR 0032: appended finite population, twenty-four hand-calculated pairs.
#[allow(dead_code)]
pub const ACTOR_OBSERVED_FIXTURES: [&str; 48] = [
    "aobs-cancel",
    "aobs-cancel.broken-finish-after-drain",
    "aobs-capacity",
    "aobs-capacity.broken-two-writers",
    "aobs-counter",
    "aobs-counter.broken-overflow",
    "aobs-cut",
    "aobs-cut.broken-excluded-read",
    "aobs-integer-compare",
    "aobs-integer-compare.broken-noninteger",
    "aobs-kernel",
    "aobs-kernel.broken-observed-read",
    "aobs-labels",
    "aobs-labels.broken-renamed",
    "aobs-late",
    "aobs-late.broken-no-requires",
    "aobs-names",
    "aobs-names.broken-case-clash",
    "aobs-optional",
    "aobs-optional.broken-missing-default",
    "aobs-owner",
    "aobs-owner.broken-external-writer",
    "aobs-per-case",
    "aobs-per-case.broken-false",
    "aobs-policy",
    "aobs-policy.broken-latent",
    "aobs-postwrite",
    "aobs-postwrite.broken-same-tick",
    "aobs-read-write-order",
    "aobs-read-write-order.broken-later-arrival",
    "aobs-reserved",
    "aobs-reserved.broken-set",
    "aobs-responsibility",
    "aobs-responsibility.broken-no-read",
    "aobs-scope",
    "aobs-scope.broken-latent",
    "aobs-shared-order",
    "aobs-shared-order.broken-no-requires",
    "aobs-simulation",
    "aobs-simulation.broken-nondeterminism",
    "aobs-start-visible",
    "aobs-start-visible.broken-no-start",
    "aobs-symbolic-initial",
    "aobs-symbolic-initial.broken-initial-no",
    "aobs-symbolic-writer",
    "aobs-symbolic-writer.broken-case-storage",
    "aobs-truncation",
    "aobs-truncation.broken-clock",
];

/// ADR 0034: appended closed population, populations[29] in tests/fixtures.rs
/// (after ADR 0033 [27] and ADR 0032 [28]); internal budget and saved-output
/// faults have identical DSL controls and separate test obligations.
#[allow(dead_code)]
pub const AGENCY_VIEWPOINTS_FIXTURES: [&str; 46] = [
    "agy-o-people",
    "agy-o-people.broken-no-read",
    "agy-o-permit",
    "agy-o-permit.broken-one-executor",
    "agy-o-unattributed",
    "agy-o-unattributed.broken-declared-executor",
    "agy-o-contest",
    "agy-o-contest.broken-route-no-read",
    "agy-f-select",
    "agy-f-select.broken-merged-ends",
    "agy-f-or",
    "agy-f-or.broken-and",
    "agy-f-state",
    "agy-f-state.broken-binding-guard",
    "agy-f-singleton",
    "agy-f-singleton.broken-two-results",
    "agy-l-counts",
    "agy-l-counts.broken-unconditional",
    "agy-l-requires",
    "agy-l-requires.broken-no-requires",
    "agy-l-fork",
    "agy-l-fork.broken-missing-second-call",
    "agy-l-any",
    "agy-l-any.broken-sequential",
    "agy-p-name",
    "agy-p-name.broken-proposal-name",
    "agy-label",
    "agy-label.broken-actor-rename",
    "agy-l-state-local",
    "agy-l-state-local.broken-binding-guard",
    "agy-soft-budget",
    "agy-soft-budget.broken-one-unit-short",
    "agy-old-budget",
    "agy-old-budget.broken-legacy-limit",
    "agy-invoice",
    "agy-invoice.broken-ai-transfer",
    "agy-report-compat",
    "agy-report-compat.broken-old-key",
    "agy-f-new-binding",
    "agy-f-new-binding.broken-only-fail",
    "agy-mutate",
    "agy-mutate.broken-off-path-viewpoints",
    "agy-cut-source",
    "agy-cut-source.broken-excluded-caller",
    "agy-no-end-after-end",
    "agy-no-end-after-end.broken-enable-suffix",
];

/// ADR0036: appended closed population, populations[30] in tests/fixtures.rs
/// (after ADR 0034 [29]), never a permissive prefix exclusion.
#[allow(dead_code)]
pub const POLICY_FIXTURES: [&str; 74] = [
    "pol-assign",
    "pol-assign.broken-change",
    "pol-configuration",
    "pol-configuration.broken-change",
    "pol-control",
    "pol-control.broken-change",
    "pol-cross-class-person",
    "pol-cross-class-person.broken-change",
    "pol-cross-class-resource",
    "pol-cross-class-resource.broken-change",
    "pol-deadlock",
    "pol-deadlock.broken-change",
    "pol-label-cofinish",
    "pol-label-cofinish.broken-change",
    "pol-label-cut",
    "pol-label-cut.broken-change",
    "pol-label-dtmc",
    "pol-label-dtmc.broken-change",
    "pol-label-env",
    "pol-label-env.broken-change",
    "pol-late-enqueue",
    "pol-late-enqueue.broken-change",
    "pol-latent",
    "pol-latent.broken-change",
    "pol-names",
    "pol-names.broken-change",
    "pol-no-effect",
    "pol-no-effect.broken-change",
    "pol-no-person-reservation",
    "pol-no-person-reservation.broken-change",
    "pol-observable-assign",
    "pol-observable-assign.broken-change",
    "pol-observable-fifo",
    "pol-observable-fifo.broken-change",
    "pol-permission",
    "pol-permission.broken-change",
    "pol-refuse",
    "pol-refuse.broken-change",
    "pol-resource",
    "pol-resource-hol",
    "pol-resource-hol.broken-change",
    "pol-resource.broken-change",
    "pol-separation",
    "pol-separation.broken-change",
    "pol-single-winner",
    "pol-single-winner.broken-change",
    "pol-static-block",
    "pol-static-block.broken-change",
    "pol-static-duplicate-assign",
    "pol-static-duplicate-assign.broken-change",
    "pol-static-duplicate-policy",
    "pol-static-duplicate-policy.broken-change",
    "pol-static-duplicate-then",
    "pol-static-duplicate-then.broken-change",
    "pol-static-optimize",
    "pol-static-optimize.broken-change",
    "pol-static-priority",
    "pol-static-priority.broken-change",
    "pol-static-random",
    "pol-static-random.broken-change",
    "pol-static-round-robin",
    "pol-static-round-robin.broken-change",
    "pol-static-skill",
    "pol-static-skill.broken-change",
    "pol-static-synthesize",
    "pol-static-synthesize.broken-change",
    "pol-tie-ack",
    "pol-tie-ack.broken-change",
    "pol-tie-actor",
    "pol-tie-actor.broken-change",
    "pol-tie-nondet",
    "pol-tie-nondet.broken-change",
    "pol-tie-person",
    "pol-tie-person.broken-change",
];
/// ADR0037: appended closed state-invariant group, populations[31] in tests/fixtures.rs
/// (after ADR 0036 [30]); not an open prefix exemption.
#[path = "../../relca-core/tests/support/state_invariant_population.rs"]
mod state_invariant_population;
#[allow(unused_imports)]
pub use state_invariant_population::NAMES as STATE_INVARIANT_FIXTURES;
/// ADR0035: appended, closed, independently hand-authored population,
/// populations[32] in tests/fixtures.rs (after ADR 0037 [31]).
#[allow(dead_code)]
pub const CASESET_FIXTURES: &[&str] =
    include!("../../relca-core/tests/support/caseset_wire_names.rs");

/// ADR0041: appended closed shared-update conflict population, populations[36]
/// (after ADR 0038 [35]); renumbered from 33 when rebased onto ADR 0035/0037/0038/0039/0040.
#[allow(dead_code)]
pub const CONFLICT_FIXTURES: &[&str] =
    include!("../../relca-core/tests/support/conflict_wire_names.rs");

fn conflict_property_id(id: &Value) -> bool {
    id.as_str()
        .is_some_and(|s| s.starts_with("no_lost_update "))
}

/// Closed global-property rows. A sampled non-violation is never a proof.
#[allow(dead_code)]
pub fn validate_conflict_properties(v: &Value) -> Result<(), String> {
    let Some(rows) = v["properties"].as_array() else {
        return Ok(());
    };
    let mut seen = std::collections::BTreeSet::new();
    for row in rows.iter().filter(|r| conflict_property_id(&r["id"])) {
        let id = row["id"].as_str().unwrap();
        let name = id.strip_prefix("no_lost_update ").unwrap();
        let parts: Vec<_> = name.split('.').collect();
        let ident = |s: &str| {
            !s.is_empty()
                && s.bytes().enumerate().all(|(i, b)| {
                    b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit())
                })
        };
        if parts.len() != 2 || !parts.iter().all(|p| ident(p)) || !seen.insert(id) {
            return Err("malformed/duplicate no_lost_update identity".into());
        }
        let keys: std::collections::BTreeSet<_> = row
            .as_object()
            .ok_or("property row")?
            .keys()
            .map(String::as_str)
            .collect();
        if v["mode"] == "simulation" {
            if keys != ["id", "verdict", "basis", "witness"].into_iter().collect() {
                return Err("sampled no_lost_update is one global row".into());
            }
            if row["verdict"] == "violated" {
                if row["basis"] != "witness"
                    || row["witness"].as_object().is_none_or(|w| w.len() != 1)
                    || row["witness"]["replication"].as_u64().is_none_or(|n| {
                        n == 0
                            || n > v["simulation"]["completed_replications"]
                                .as_u64()
                                .unwrap_or(0)
                    })
                {
                    return Err("loss witness requires a completed replication".into());
                }
            } else if !["inconclusive", "unsupported"]
                .iter()
                .any(|s| row["verdict"] == *s)
                || row["basis"] != "sample"
                || !row["witness"].is_null()
            {
                return Err("sample absence is inconclusive, not satisfied".into());
            }
        } else if keys != ["id", "verdict"].into_iter().collect() {
            return Err("exhaustive no_lost_update has no cases/probability/notes".into());
        }
    }
    Ok(())
}

/// Closed ADR0035 property-note vocabulary, not a general annotation channel.
pub fn validate_caseset_notes(v: &Value) -> Result<(), String> {
    validate_conflict_properties(v)?;
    if let Some(rows) = v["properties"].as_array() {
        for row in rows {
            if let Some(notes) = row.get("notes") {
                let id = row["id"].as_str().ok_or("notes property id")?;
                if !id.starts_with("contract ")
                    || !id.ends_with(" at_most_once")
                    || notes != &serde_json::json!(["mechanism_unmodeled"])
                {
                    return Err("notes require an at_most_once property and one known mechanism_unmodeled word".into());
                }
            }
        }
    }
    Ok(())
}

fn validate_caseset_value(value: &Value, cases: Option<u64>) -> Result<(), String> {
    if let Some(ids) = value["value"].as_array() {
        let n = cases.ok_or("set display without finite case bound")?;
        if value["visibility"] != "observed"
            || !value["name"].as_str().is_some_and(|s| s.contains('.'))
        {
            return Err("case sets are shared Actor observations".into());
        }
        let mut previous = 0;
        for id in ids {
            let id = id.as_u64().ok_or("set display requires integer case IDs")?;
            if id <= previous || id > n {
                return Err(
                    "set display must be sorted, distinct and one-based within bounds".into(),
                );
            }
            previous = id;
        }
    }
    Ok(())
}

/// ADR0038: appended closed record/inline-enum population, populations[35]
/// (after ADR 0040 [34]); renumbered from 31 when rebased onto ADR 0035/0037/0039/0040.
#[allow(dead_code)]
pub const RECORD_FIXTURES: &[&str] =
    include!("../../relca-core/tests/support/record_fixture_names.rs");
/// ADR0045: appended after the existing populations, never a prefix exemption.
#[allow(dead_code)]
pub const QUALITY_FIXTURES: &[&str] = &include!("../../relca-core/tests/support/quality_names.rs");

/// ADR0047 appended closed population. Unknown eval-/cmp7- names are not skipped.
#[allow(dead_code)]
pub const EVALUATION_FIXTURES: &[&str] =
    &include!("../../relca-core/tests/support/evaluation_names.rs");

/// ADR0049: appended closed population, including static cases, not a prefix.
#[allow(dead_code)]
pub const QUANTILE_FIXTURES: &[&str] =
    &include!("../../relca-core/tests/support/quantile_names.rs");
