//! Independent ADR0047 wire tables and invariants. This is NOT the evaluator.
//! Included by fixture_match; existing old-wire validation runs after a narrow,
//! validated inverse projection. No entire arbitrary subtree is ignored.
use super::*;
use serde_json::json;
use std::collections::BTreeSet;

fn strings() -> Ty {
    Ty::Arr(Box::new(Ty::Str))
}
fn nullable(t: Ty) -> Ty {
    Ty::Null(Box::new(t))
}
fn reason() -> Ty {
    nullable(Ty::Lit(&[
        "zero_denominator",
        "denominator_may_be_zero",
        "undetermined",
        "truncated",
        "aborted",
        "no_samples",
        "not_configured",
        "no_terminal",
        "rejected",
        "no_task_started",
        "never_created",
        "cancelled_before_start",
        "unfinished",
        "deadlock",
        "zero_capacity",
        "no_capacity",
        "range_stopped",
        "refused",
        "not_refused",
    ]))
}
pub(super) fn agency_table_schema() -> Ty {
    Ty::Arr(Box::new(Ty::Obj(vec![
        ("from", Ty::Str),
        ("to", Ty::Str),
        ("differences", agency_differences_schema()),
    ])))
}
pub(super) fn schema() -> Ty {
    use Ty::*;
    let cell = Obj(vec![
        ("configuration", Str),
        ("verdict", Verdict),
        (
            "basis",
            nullable(Lit(&[
                "exact",
                "per_case_relaxation",
                "with_undetermined",
                "statistical",
            ])),
        ),
        (
            "interval",
            nullable(Obj(vec![("lower", MetricNumber), ("upper", MetricNumber)])),
        ),
        ("reason", reason()),
        ("source_reason", quality_reason()),
        ("point", OptionalField(Box::new(nullable(MetricNumber)))),
    ]);
    let objective_cell = OneOf(vec![
        Obj(vec![
            ("configuration", Str),
            ("lower", nullable(MetricNumber)),
            ("upper", nullable(MetricNumber)),
            (
                "basis",
                nullable(Lit(&["exact", "per_case_relaxation", "path_range"])),
            ),
            ("reason", reason()),
        ]),
        Obj(vec![
            ("configuration", Str),
            ("point", nullable(MetricNumber)),
            ("reason", reason()),
        ]),
        Obj(vec![
            ("configuration", Str),
            ("sample", metric_scalar_schema()),
            ("reason", reason()),
        ]),
        quantile_objective_cell_schema(),
    ]);
    let plan_reason = Obj(vec![
        ("kind", Lit(&["verify", "hard"])),
        ("constraint", nullable(Str)),
        ("verdict", Verdict),
        ("reason", reason()),
    ]);
    Obj(vec![
        ("rules", Lit(&["compare-evaluation-p1-1"])),
        (
            "declared",
            Obj(vec![
                ("plans", strings()),
                ("under", nullable(strings())),
                ("scenario", nullable(Str)),
                ("cut", nullable(Str)),
            ]),
        ),
        (
            "hard",
            Arr(Box::new(Obj(vec![
                ("constraint", Str),
                ("cohort", Str),
                ("op", Lit(&["<=", "<", ">=", ">"])),
                ("bound", RationalText),
                ("values", Arr(Box::new(cell))),
            ]))),
        ),
        (
            "statistics",
            OptionalField(Box::new(Obj(vec![
                ("interval_method", Lit(&["clopper-pearson-per-case-sum"])),
                (
                    "multiplicity",
                    Obj(vec![
                        ("method", Lit(&["bonferroni"])),
                        ("m", Int),
                        ("alpha", nullable(RationalText)),
                    ]),
                ),
                ("confidence", RationalText),
            ]))),
        ),
        (
            "plans",
            Arr(Box::new(Obj(vec![
                ("configuration", Str),
                ("verify_verdict", Verdict),
                ("hard_verdict", Verdict),
                ("status", Lit(&["admissible", "excluded", "undetermined"])),
                ("reasons", Arr(Box::new(plan_reason))),
            ]))),
        ),
        (
            "objectives",
            Arr(Box::new(Obj(vec![
                ("objective", Str),
                ("direction", Lit(&["minimize", "maximize"])),
                ("values", Arr(Box::new(objective_cell))),
            ]))),
        ),
        (
            "pareto",
            Obj(vec![
                ("status", Lit(&["computed", "not_computed"])),
                (
                    "reason",
                    nullable(Lit(&[
                        "simulation_estimates",
                        "no_objectives",
                        "no_admissible_plans",
                    ])),
                ),
                ("among", strings()),
                ("non_dominated", strings()),
                (
                    "dominated",
                    Arr(Box::new(Obj(vec![
                        ("configuration", Str),
                        ("dominated_by", strings()),
                    ]))),
                ),
                (
                    "undetermined_pairs",
                    Arr(Box::new(Obj(vec![
                        ("from", Str),
                        ("to", Str),
                        ("reason", Lit(&["missing_value", "overlapping_intervals"])),
                    ]))),
                ),
                (
                    "not_evaluated",
                    Arr(Box::new(Obj(vec![
                        ("configuration", Str),
                        ("status", Lit(&["excluded", "undetermined"])),
                    ]))),
                ),
            ]),
        ),
    ])
}
fn rank(s: &str) -> usize {
    [
        "satisfied",
        "vacuous",
        "inconclusive",
        "unsupported",
        "violated",
        "invalid",
    ]
    .iter()
    .position(|w| *w == s)
    .unwrap()
}
fn fold<'a>(xs: impl Iterator<Item = &'a str>) -> &'a str {
    xs.max_by_key(|s| rank(s)).unwrap_or("satisfied")
}
fn number_cmp(a: &Value, b: &Value) -> Result<std::cmp::Ordering, String> {
    let (an, ad) = metric_fraction(a)?;
    let (bn, bd) = metric_fraction(b)?;
    Ok(nat_cmp(&nat_mul(&an, &bd), &nat_mul(&bn, &ad)))
}
fn nonempty_unique(xs: &[Value]) -> bool {
    !xs.is_empty()
        && xs.iter().all(|n| n.as_str().is_some_and(valid_plan_name))
        && xs
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>()
            .len()
            == xs.len()
}
fn field_ref(s: &str) -> bool {
    s.strip_prefix("cohort.").is_some_and(valid_plan_name)
        || s.strip_prefix("quantile.").is_some_and(valid_plan_name)
        || [
            "metrics.overall.work",
            "metrics.overall.named_people_work",
            "metrics.overall.peak_tasks",
            "metrics.overall.observed_cost_total",
            "metrics.overall.observed_cost_per_arrival",
        ]
        .contains(&s)
}
fn quantile_objective_cell_schema() -> Ty {
    use Ty::*;
    let reason = || {
        nullable(Lit(&[
            "zero_denominator",
            "denominator_may_be_zero",
            "undetermined",
            "truncated",
            "aborted",
            "no_samples",
            "beyond_reached_mass",
            "not_configured",
        ]))
    };
    OneOf(vec![
        Obj(vec![
            ("configuration", Str),
            ("lower", nullable(MetricNumber)),
            ("upper", nullable(MetricNumber)),
            ("basis", nullable(Lit(&["exact", "per_case_relaxation"]))),
            ("reason", reason()),
        ]),
        Obj(vec![
            ("configuration", Str),
            ("point", nullable(MetricNumber)),
            ("reason", reason()),
        ]),
    ])
}
pub(super) fn validate(v: &Value) -> Result<(), String> {
    check(v, &compare_schema(), "evaluation wrapper")?;
    complete(v, &compare_schema(), "evaluation wrapper")?;
    let ev = &v["evaluation"];
    check(ev, &schema(), "evaluation")?;
    complete(ev, &schema(), "evaluation")?;
    let sim = v["analysis"] == "simulation";
    let plans = v["plans"].as_array().unwrap();
    let names: Vec<_> = plans.iter().map(|p| p["configuration"].clone()).collect();
    if names.len() < 2
        || !nonempty_unique(&names)
        || ev["declared"]["plans"] != json!(names)
        || !v["mismatch"].is_null()
    {
        return Err("evaluation declaration/plan identity".into());
    }
    if let Some(under) = ev["declared"]["under"].as_array()
        && (!nonempty_unique(under) || !under.contains(&ev["declared"]["scenario"]))
    {
        return Err("under selector identity".into());
    }
    if ev["declared"]["scenario"] != v.get("scenario").cloned().unwrap_or(Value::Null) {
        return Err("selected scenario mismatch".into());
    }
    let hard = ev["hard"].as_array().unwrap();
    let objectives = ev["objectives"].as_array().unwrap();
    let ep = ev["plans"].as_array().unwrap();
    if ep.len() != plans.len() || sim != ev.get("statistics").is_some() {
        return Err("evaluation analysis/inventory".into());
    }
    if sim {
        let r = plans[0]["report"]["simulation"]["replications"]
            .as_u64()
            .ok_or("replications")?;
        if r == 0 {
            return Err("zero replication inventory".into());
        }
        for p in plans {
            let raw = &p["report"];
            if raw["simulation"]["replications"] != r
                || raw["simulation"]["confidence"] != ev["statistics"]["confidence"]
            {
                return Err("cross-plan simulation parameters".into());
            }
            let n = raw["bounds"]["cases"].as_u64().ok_or("case bound")?;
            if n == 0 {
                return Err("zero case inventory".into());
            }
            for property in raw["quality"]["properties"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let cases = property["cases"].as_array().ok_or("property cases")?;
                if cases.len() as u64 != n {
                    return Err("quality cases incomplete".into());
                }
                let withdrawn = ["aborted", "no_samples"]
                    .iter()
                    .any(|s| property["expected_count"]["reason"] == *s);
                for (i, c) in cases.iter().enumerate() {
                    let h = c["hits"].as_u64().ok_or("hits")?;
                    let u = c["unknown"].as_u64().ok_or("unknown")?;
                    let t = c["trials"].as_u64().ok_or("trials")?;
                    if c["case_id"].as_u64() != Some(i as u64 + 1)
                        || t != if withdrawn { 0 } else { r }
                        || h.checked_add(u).is_none_or(|sum| sum > t)
                    {
                        return Err("case IDs,trials or checked hit sum".into());
                    }
                    if let Some(outcomes) = raw["case_outcomes"].as_array()
                        && outcomes.len() == cases.len()
                        && c["arrival"] != outcomes[i]["arrival"]
                    {
                        return Err("quality arrival inventory".into());
                    }
                }
            }
        }
    }
    let mut constraints = BTreeSet::new();
    let mut m = 0u64;
    for h in hard {
        let name = h["cohort"].as_str().unwrap();
        let op = h["op"].as_str().unwrap();
        let bound = h["bound"].as_str().unwrap();
        if !valid_plan_name(name)
            || !metric_le(&h["bound"], &json!("1"))?
            || h["constraint"] != format!("cohort.{name} {op} {bound}")
            || !constraints.insert(h["constraint"].as_str().unwrap())
        {
            return Err("hard normalized identity/duplicates".into());
        }
        let cells = h["values"].as_array().unwrap();
        if cells.len() != names.len() {
            return Err("hard rectangular inventory".into());
        }
        for (j, c) in cells.iter().enumerate() {
            if c["configuration"] != names[j] || sim != c.get("point").is_some() {
                return Err("hard cell order/mode".into());
            }
            let q = plans[j]["report"]["quality"]["cohort"]
                .as_array()
                .ok_or("hard source cohorts")?
                .iter()
                .find(|q| q["name"] == name)
                .ok_or("unknown hard cohort")?;
            super::validate_deferred_hard_cell(&plans[j]["report"], q, c)?;
            let n = plans[j]["report"]["bounds"]["cases"]
                .as_u64()
                .ok_or("case bound")?;
            let count = if q["denominator"]["kind"] == "arrivals" {
                1
            } else {
                2
            };
            m = m
                .checked_add(n.checked_mul(count).ok_or("multiplicity overflow")?)
                .ok_or("multiplicity overflow")?;
            if c["interval"].is_null() {
                if !c["basis"].is_null() || c["reason"].is_null() {
                    return Err("null hard interval needs reason,not basis".into());
                }
                let verdict = if ["aborted", "no_samples"].iter().any(|r| c["reason"] == *r) {
                    "unsupported"
                } else {
                    "inconclusive"
                };
                if c["verdict"] != verdict {
                    return Err("null hard verdict".into());
                }
            } else {
                let lo = &c["interval"]["lower"];
                let hi = &c["interval"]["upper"];
                if !metric_le(lo, hi)?
                    || c["basis"].is_null()
                    || sim != (c["basis"] == "statistical")
                {
                    return Err("hard interval bounds/basis".into());
                }
                let lower = number_cmp(lo, &h["bound"])?;
                let upper = number_cmp(hi, &h["bound"])?;
                let (satisfied, violated) = match op {
                    "<=" => (!upper.is_gt(), lower.is_gt()),
                    "<" => (upper.is_lt(), !lower.is_lt()),
                    ">=" => (!lower.is_lt(), upper.is_lt()),
                    ">" => (lower.is_gt(), !upper.is_gt()),
                    _ => unreachable!(),
                };
                let verdict = if satisfied {
                    "satisfied"
                } else if violated {
                    "violated"
                } else {
                    "inconclusive"
                };
                if c["verdict"] != verdict {
                    return Err("hard interval decision".into());
                }
                if !sim {
                    let unknown = q["value"]["lower"].is_null();
                    let src = if unknown {
                        &q["bounds_with_undetermined"]
                    } else {
                        &q["value"]
                    };
                    if !["truncated", "undetermined"]
                        .iter()
                        .any(|r| q["value"]["reason"] == *r)
                        && unknown
                    {
                        return Err("unavailable quality is not an interval".into());
                    }
                    if lo != &src["lower"]
                        || hi != &src["upper"]
                        || c["reason"] != q["value"]["reason"]
                        || c["basis"]
                            != if unknown {
                                json!("with_undetermined")
                            } else {
                                q["value"]["basis"].clone()
                            }
                    {
                        return Err("exhaustive hard source transport".into());
                    }
                }
            }
            if sim && c["point"] != q["value"]["point"] {
                return Err("simulation point transport".into());
            }
            let original_reason = &q["value"]["reason"];
            let want_source = if original_reason == &c["reason"] {
                Value::Null
            } else {
                original_reason.clone()
            };
            if c["source_reason"] != want_source {
                return Err("source reason lost/fabricated".into());
            }
        }
    }
    if sim {
        let st = &ev["statistics"];
        if st["multiplicity"]["m"] != m {
            return Err("Bonferroni case/row/plan/denominator census".into());
        }
        let alpha = &st["multiplicity"]["alpha"];
        if m == 0 {
            if !alpha.is_null() {
                return Err("empty family alpha".into());
            }
        } else {
            let (cn, cd) = metric_fraction(&st["confidence"])?;
            if nat_cmp(&cn, &cd) != std::cmp::Ordering::Less || cn == "0" {
                return Err("confidence range".into());
            }
            let (an, ad) = metric_fraction(alpha)?;
            if nat_mul(
                &an,
                &nat_mul(
                    &cd,
                    &m.checked_mul(2).ok_or("multiplicity overflow")?.to_string(),
                ),
            ) != nat_mul(&nat_sub(&cd, &cn), &ad)
            {
                return Err("Bonferroni alpha".into());
            }
        }
    }
    let mut identities = BTreeSet::new();
    for o in objectives {
        let direction = o["direction"].as_str().unwrap();
        let target = o["objective"]
            .as_str()
            .unwrap()
            .strip_prefix(&format!("{direction} "))
            .ok_or("objective direction")?;
        if !field_ref(target) || !identities.insert(o["objective"].as_str().unwrap()) {
            return Err("objective closed identity/duplicates".into());
        }
        let cells = o["values"].as_array().unwrap();
        if cells.len() != names.len() {
            return Err("objective rectangular inventory".into());
        }
        for (j, c) in cells.iter().enumerate() {
            if c["configuration"] != names[j] {
                return Err("objective plan order".into());
            }
            let quantile = target.strip_prefix("quantile.");
            if let Some(name) = quantile {
                validate_quantile_objective_cell(c, &plans[j]["report"], name, sim)?;
            } else {
                // The new quantile alternative must not widen old objective reasons.
                check(&c["reason"], &reason(), "objective reason")?;
            }
            if sim {
                let cohort = target.starts_with("cohort.") || quantile.is_some();
                if cohort != c.get("point").is_some()
                    || !cohort != c.get("sample").is_some()
                    || c.get("lower").is_some()
                {
                    return Err("sample objective form".into());
                }
                if !cohort {
                    scalar_invariants(
                        &c["sample"],
                        true,
                        plans[j]["report"]["metrics"]["status"]
                            .as_str()
                            .unwrap_or("complete"),
                        plans[j]["report"]["simulation"]["replications"]
                            .as_u64()
                            .unwrap_or(10000),
                    )?;
                } else if c["point"].is_null() && c["reason"].is_null() {
                    return Err("missing point reason".into());
                }
            } else if c.get("lower").is_none()
                || c.get("upper").is_none()
                || c["lower"].is_null() != c["upper"].is_null()
                || c["lower"].is_null() != c["basis"].is_null()
                || c["lower"].is_null() == c["reason"].is_null()
                || !c["lower"].is_null() && !metric_le(&c["lower"], &c["upper"])?
            {
                return Err("objective null/bounds/reason".into());
            }
        }
    }
    let mut all = vec![];
    for (j, p) in ep.iter().enumerate() {
        let verify = plans[j]["report"]["verdict"]
            .as_str()
            .ok_or("verify verdict")?;
        let hv = fold(
            hard.iter()
                .map(|h| h["values"][j]["verdict"].as_str().unwrap()),
        );
        let status = if verify == "violated" || hv == "violated" {
            "excluded"
        } else if verify == "satisfied" && hv == "satisfied" {
            "admissible"
        } else {
            "undetermined"
        };
        if p["configuration"] != names[j]
            || p["verify_verdict"] != verify
            || p["hard_verdict"] != hv
            || p["status"] != status
        {
            return Err("admissibility/verify/hard fold".into());
        }
        let mut reasons = vec![];
        if verify != "satisfied" {
            reasons.push(json!({"kind":"verify","constraint":null,"verdict":verify,"reason":null}));
        }
        for h in hard {
            let c = &h["values"][j];
            if c["verdict"] != "satisfied" {
                reasons.push(json!({"kind":"hard","constraint":h["constraint"],"verdict":c["verdict"],"reason":c["reason"]}));
            }
        }
        if p["reasons"] != json!(reasons) {
            return Err("admissibility reason inventory/order".into());
        }
        all.extend([verify, hv]);
    }
    for d in v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
    {
        all.push(if d["code"].as_str().unwrap().starts_with("U-") {
            "unsupported"
        } else {
            "invalid"
        });
    }
    if v["verdict"] != fold(all.into_iter()) {
        return Err("root hard fold".into());
    }
    validate_pareto(ev, sim)?;
    let pairs = v["tables"]["agency"]
        .as_array()
        .ok_or("missing Agency pairs")?;
    if pairs.len() != names.len() * (names.len() - 1) / 2 {
        return Err("Agency all pairs inventory".into());
    }
    let mut k = 0;
    for i in 0..names.len() {
        for j in i + 1..names.len() {
            if pairs[k]["from"] != names[i] || pairs[k]["to"] != names[j] {
                return Err("Agency declaration direction/order".into());
            }
            validate_agency_differences(&pairs[k]["differences"])?;
            k += 1;
        }
    }
    // Validate exact suffix before projecting only these four stage words.
    let suffix = if sim {
        vec![
            json!("hard_filter"),
            json!("admissibility"),
            json!("hard_cross_plan_bonferroni"),
        ]
    } else {
        vec![
            json!("hard_filter"),
            json!("admissibility"),
            json!("pareto_sure_dominance"),
        ]
    };
    let checked = v["checked"].as_array().unwrap();
    if !checked.ends_with(&suffix)
        || checked[..checked.len() - suffix.len()]
            .iter()
            .any(|w| suffix.contains(w))
    {
        return Err("evaluation checked suffix".into());
    }
    let unchecked = v["unchecked"].as_array().unwrap();
    if unchecked.contains(&json!("hard_filter"))
        || unchecked.contains(&json!("admissibility"))
        || sim != unchecked.contains(&json!("pareto"))
    {
        return Err("evaluation unchecked transitions".into());
    }
    let aa = v["assumptions"].as_array().unwrap();
    let fields: Vec<_> = aa
        .iter()
        .filter_map(|a| a["field"].as_str())
        .filter(|s| s.starts_with("compare."))
        .collect();
    let want = if sim {
        vec![
            "compare.purpose",
            "compare.common_random",
            "compare.confidence",
            "compare.hard",
            "compare.pareto",
        ]
    } else {
        vec!["compare.purpose", "compare.hard", "compare.pareto"]
    };
    if fields != want
        || aa
            .iter()
            .find(|a| a["field"] == "compare.purpose")
            .ok_or("purpose")?["value"]
            != "declared-evaluation"
    {
        return Err("evaluation assumptions presence/order".into());
    }
    Ok(())
}
fn validate_quantile_objective_cell(
    cell: &Value,
    report: &Value,
    name: &str,
    simulation: bool,
) -> Result<(), String> {
    let ty = quantile_objective_cell_schema();
    check(cell, &ty, "quantile objective")?;
    complete(cell, &ty, "quantile objective")?;
    let rows = report["quality"]["quantiles"]
        .as_array()
        .ok_or("missing objective quantiles")?;
    let mut found = rows.iter().filter(|q| q["name"] == name);
    let source = found.next().ok_or("unknown objective quantile")?;
    if found.next().is_some() {
        return Err("duplicate objective quantile".into());
    }
    let fields: &[&str] = if simulation {
        &["point", "reason"]
    } else {
        &["lower", "upper", "basis", "reason"]
    };
    for field in fields {
        if cell.get(*field).is_none() || cell.get(*field) != source["value"].get(*field) {
            return Err("quantile objective value transport".into());
        }
    }
    if simulation != cell.get("point").is_some() {
        return Err("quantile objective analysis".into());
    }
    Ok(())
}

fn validate_pareto(ev: &Value, sim: bool) -> Result<(), String> {
    let plans = ev["plans"].as_array().unwrap();
    let os = ev["objectives"].as_array().unwrap();
    let admissible: Vec<_> = plans
        .iter()
        .enumerate()
        .filter(|(_, p)| p["status"] == "admissible")
        .map(|(i, _)| i)
        .collect();
    let names: Vec<_> = admissible
        .iter()
        .map(|i| plans[*i]["configuration"].clone())
        .collect();
    let no: Vec<_> = plans
        .iter()
        .filter(|p| p["status"] != "admissible")
        .map(|p| json!({"configuration":p["configuration"],"status":p["status"]}))
        .collect();
    let p = &ev["pareto"];
    let reason = if sim {
        Some("simulation_estimates")
    } else if os.is_empty() {
        Some("no_objectives")
    } else if admissible.is_empty() {
        Some("no_admissible_plans")
    } else {
        None
    };
    if p["among"] != json!(names)
        || p["not_evaluated"] != json!(no)
        || p["reason"] != json!(reason)
        || p["status"]
            != if reason.is_some() {
                "not_computed"
            } else {
                "computed"
            }
    {
        return Err("Pareto domain/reason".into());
    }
    if reason.is_some() {
        if ["non_dominated", "dominated", "undetermined_pairs"]
            .iter()
            .any(|k| p[*k] != json!([]))
        {
            return Err("not computed Pareto has results".into());
        }
        return Ok(());
    }
    let dominates = |a: usize, b: usize| -> Result<bool, String> {
        let mut strict = false;
        for o in os {
            let x = &o["values"][a];
            let y = &o["values"][b];
            if x["lower"].is_null() || y["lower"].is_null() {
                return Ok(false);
            }
            let order = if o["direction"] == "minimize" {
                number_cmp(&x["upper"], &y["lower"])?
            } else {
                number_cmp(&y["upper"], &x["lower"])?
            };
            if order.is_gt() {
                return Ok(false);
            }
            strict |= order.is_lt();
        }
        Ok(strict)
    };
    let mut non = vec![];
    let mut dominated = vec![];
    let mut unknown = vec![];
    for &b in &admissible {
        let mut by = vec![];
        for &a in &admissible {
            if a != b && dominates(a, b)? {
                by.push(plans[a]["configuration"].clone());
            }
        }
        if by.is_empty() {
            non.push(plans[b]["configuration"].clone());
        } else {
            dominated.push(json!({"configuration":plans[b]["configuration"],"dominated_by":by}));
        }
    }
    for (k, &a) in admissible.iter().enumerate() {
        for &b in &admissible[k + 1..] {
            if dominates(a, b)? || dominates(b, a)? {
                continue;
            }
            let missing = os
                .iter()
                .any(|o| o["values"][a]["lower"].is_null() || o["values"][b]["lower"].is_null());
            let mut overlap = false;
            if !missing {
                for o in os {
                    let x = &o["values"][a];
                    let y = &o["values"][b];
                    overlap |= (x["lower"] != x["upper"] || y["lower"] != y["upper"])
                        && metric_le(&x["lower"], &y["upper"])?
                        && metric_le(&y["lower"], &x["upper"])?;
                }
            }
            if missing || overlap {
                unknown.push(
                    json!({"from":plans[a]["configuration"],"to":plans[b]["configuration"],
            "reason":if missing {"missing_value"} else {"overlapping_intervals"}}),
                );
            }
        }
    }
    if p["non_dominated"] != json!(non)
        || p["dominated"] != json!(dominated)
        || p["undetermined_pairs"] != json!(unknown)
    {
        return Err("Pareto sure dominance/unknown partition/order".into());
    }
    Ok(())
}
pub(super) fn inverse(v: &Value) -> Result<Value, String> {
    validate(v)?;
    let mut old = v.clone();
    old.as_object_mut().unwrap().remove("evaluation");
    old["tables"].as_object_mut().unwrap().remove("agency");
    old["checked"].as_array_mut().unwrap().retain(|s| {
        ![
            "hard_filter",
            "admissibility",
            "hard_cross_plan_bonferroni",
            "pareto_sure_dominance",
        ]
        .iter()
        .any(|w| s == *w)
    });
    let unchecked = old["unchecked"].as_array_mut().unwrap();
    unchecked.insert(1, json!("hard_filter"));
    if v["analysis"] != "simulation" {
        unchecked.insert(2, json!("pareto"));
    }
    let mut vs: Vec<_> = v["plans"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["report"]["verdict"].as_str().unwrap())
        .collect();
    for d in v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
    {
        vs.push(if d["code"].as_str().unwrap().starts_with("U-") {
            "unsupported"
        } else {
            "invalid"
        });
    }
    old["verdict"] = json!(fold(vs.into_iter()));
    // Only these two checked evaluation assumptions are new. Purpose's value
    // is not constrained by the pre-B7 old schema and remains visible there.
    old["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|a| a["field"] != "compare.hard" && a["field"] != "compare.pareto");
    Ok(old)
}

pub(super) fn mutation_inverse(v: &Value) -> Result<Value, String> {
    check(v, &mutation_schema(), "kind7 mutation wire")?;
    complete(v, &mutation_schema(), "kind7 mutation wire")?;
    if !["satisfied", "violated"]
        .iter()
        .any(|s| v["original_verdict"] == *s)
    {
        return Err("kind7 must preserve original gate".into());
    }
    let cohorts = v["original"]["quality"]["cohort"]
        .as_array()
        .ok_or("kind7 needs declared cohort ratios")?;
    if cohorts.is_empty() {
        return Err("property-only quality cannot enable kind7".into());
    }
    let rows = v["mutations"].as_array().unwrap();
    if rows.len() != 10 {
        return Err("ten kinds".into());
    }
    for (i, row) in rows.iter().enumerate() {
        if i == 7 {
            continue;
        }
        if (row.get("skipped_sites").is_some() && !matches!(i, 3 | 6))
            || row["mutants"].as_array().unwrap().iter().any(|m| {
                m["operator"] == "remove_denominator_end"
                    || m["site"].get("cohort").is_some()
                    || m["site"].get("removed_end").is_some()
            })
        {
            return Err("kind7-only extension leaked to another kind".into());
        }
    }
    let row = &rows[7];
    if row["kind"] != "old_promise_denominator_removed"
        || row["name"] != MUTATION_NAMES[7]
        || row["assigned_checks"] != json!(["quality_denominator", "comparison_mismatch"])
    {
        return Err("kind7 identity/assigned checks".into());
    }
    let skipped = row["skipped_sites"]
        .as_array()
        .ok_or("enabled kind7 needs skipped_sites")?;
    let mut seen = BTreeSet::new();
    let mut skipped_order = vec![];
    for s in skipped {
        let c = cohorts
            .iter()
            .position(|c| c["name"] == s["cohort"])
            .ok_or("unknown skipped cohort")?;
        if !seen.insert(s["cohort"].as_str().unwrap()) {
            return Err("duplicate skipped cohort".into());
        }
        skipped_order.push(c);
        let d = &cohorts[c]["denominator"];
        if s["reason"] == "state_condition" && (d["kind"] != "property" || !d["covers"].is_null()) {
            return Err("state skip requires State property denominator".into());
        }
    }
    if skipped_order.windows(2).any(|w| w[0] >= w[1]) {
        return Err("skipped declaration order".into());
    }
    let ms = row["mutants"].as_array().unwrap();
    if ms.is_empty() {
        if row["status"] != "not_applicable"
            || row["reason"]
                != if skipped.iter().all(|s| s["reason"] == "state_condition")
                    && !skipped.is_empty()
                {
                    json!("state_condition")
                } else {
                    json!("no_matching_site")
                }
        {
            return Err("kind7 no-site classification".into());
        }
    } else if row["status"] != "generated" || !row["reason"].is_null() {
        return Err("kind7 generation classification".into());
    }
    let mut ids = BTreeSet::new();
    let mut finished = 0usize;
    let mut not_run = 0usize;
    let mut previous = 0usize;
    for mutant in ms {
        let site = &mutant["site"];
        let cohort = site["cohort"].as_str().ok_or("kind7 missing cohort")?;
        let removed = site["removed_end"]
            .as_str()
            .ok_or("kind7 missing removed end")?;
        let ci = cohorts
            .iter()
            .position(|c| c["name"] == cohort)
            .ok_or("unknown kind7 cohort")?;
        if ci < previous || seen.contains(cohort) {
            return Err("kind7 declaration ordering/skipped conflict".into());
        }
        previous = ci;
        let id = format!("old_promise_denominator_removed:cohort.{cohort}:remove:{removed}");
        if mutant["id"] != id
            || !ids.insert(id)
            || mutant["operator"] != "remove_denominator_end"
            || !mutant["report"].is_null()
            || mutant["other_changes"] != json!([])
            || [
                "actor",
                "capability",
                "source",
                "statement_path",
                "retained_branch",
            ]
            .iter()
            .any(|k| !site[*k].is_null())
            || site["affected_bindings"] != json!([])
            || site["span"]["start"].as_u64() >= site["span"]["end"].as_u64()
            || site["line"] == 0
            || site["column"] == 0
        {
            return Err("kind7 structural site/report shape".into());
        }
        if cohorts[ci]["denominator"]["kind"] == "property" && !site["process"].is_null()
            || cohorts[ci]["denominator"]["kind"] == "arrivals" && !site["process"].is_string()
        {
            return Err("kind7 process location".into());
        }
        if mutant["result"] == "not_run" {
            not_run += 1;
            if mutant["reason"] != "mutant_limit" || mutant["assigned_changes"] != json!([]) {
                return Err("kind7 not_run fields".into());
            }
            continue;
        }
        if not_run > 0 || mutant["result"] != "detected" || !mutant["reason"].is_null() {
            return Err("kind7 comparison result/budget prefix".into());
        }
        finished += 1;
        let changes = mutant["assigned_changes"].as_array().unwrap();
        if changes.len() != 2 {
            return Err("two kind7 assigned definitions".into());
        }
        let before = changes[0]["before"]
            .as_array()
            .ok_or("denominator before list")?;
        let after = changes[0]["after"]
            .as_array()
            .ok_or("denominator after list")?;
        if before.len() < 2
            || !nonempty_unique(before)
            || !nonempty_unique(after)
            || before.iter().filter(|e| *e == removed).count() != 1
            || before.iter().filter(|e| *e != removed).ne(after.iter())
        {
            return Err("exactly one independent denominator removal".into());
        }
        let d = &cohorts[ci]["denominator"];
        if d["kind"] == "property" && d["covers"] != json!(before) {
            return Err("property denominator definition transport".into());
        }
        for (change, check_name) in changes
            .iter()
            .zip(["quality_denominator", "comparison_mismatch"])
        {
            if change["check"] != check_name
                || change["key"] != format!("cohort.{cohort}")
                || change["field"] != "denominator.terminals"
                || change["direction"] != "changed"
                || change["before"] != json!(before)
                || change["after"] != json!(after)
            {
                return Err("kind7 closed definition comparison".into());
            }
        }
    }
    let complete = !ms.is_empty() && not_run == 0;
    let checked = v["checked"].as_array().unwrap();
    let unchecked = v["unchecked"].as_array().unwrap();
    if checked
        .iter()
        .filter(|w| *w == "quality_denominator")
        .count()
        != usize::from(complete)
        || unchecked
            .iter()
            .filter(|w| *w == "quality_denominator")
            .count()
            != usize::from(!complete)
        || unchecked
            .iter()
            .filter(|w| *w == "numerator_subset_of_denominator")
            .count()
            != usize::from(!ms.is_empty())
    {
        return Err("kind7 checked completion/unchecked subset boundary".into());
    }
    if complete && checked.last() != Some(&json!("quality_denominator"))
        || !ms.is_empty() && unchecked.last() != Some(&json!("numerator_subset_of_denominator"))
    {
        return Err("kind7 boundary words append only".into());
    }
    let assumptions = v["assumptions"].as_array().unwrap();
    let kind7: Vec<_> = assumptions
        .iter()
        .filter(|a| a["field"] == "mutation.kind7")
        .collect();
    if kind7.len() != usize::from(!ms.is_empty())
        || !kind7.is_empty() && assumptions.last() != kind7.last().copied()
    {
        return Err("kind7 assumption presence/order".into());
    }
    let old_mutants: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 7)
        .flat_map(|(_, r)| r["mutants"].as_array().unwrap())
        .collect();
    let old_not_run = old_mutants
        .iter()
        .filter(|m| m["result"] == "not_run")
        .count();
    let old_tried = old_mutants.len() - old_not_run;
    if old_tried + finished > 128
        || (old_not_run > 0 && finished > 0)
        || v["run"]["limit_reached"] != (not_run + old_not_run > 0)
    {
        return Err("kind7 shares final budget,not a new budget".into());
    }
    let other_incomplete = old_mutants.iter().any(|m| {
        ["not_run", "inconclusive", "invalid_mutant"]
            .iter()
            .any(|r| m["result"] == *r)
    });
    let incomplete = other_incomplete || not_run > 0;
    if v["run"]["completion"]
        != if incomplete {
            "inconclusive"
        } else {
            "complete"
        }
        || v["run"]["reason"]
            != if not_run + old_not_run > 0 {
                json!("mutant_limit")
            } else if incomplete {
                json!("mutant_incomplete")
            } else {
                Value::Null
            }
    {
        return Err("kind7 run fold".into());
    }
    let mut old = v.clone();
    old["mutations"][7]["status"] = json!("unsupported");
    old["mutations"][7]["reason"] = json!("quality_denominator_unavailable");
    old["mutations"][7]["mutants"] = json!([]);
    old["mutations"][7]
        .as_object_mut()
        .unwrap()
        .remove("skipped_sites");
    old["checked"]
        .as_array_mut()
        .unwrap()
        .retain(|w| w != "quality_denominator");
    old["unchecked"]
        .as_array_mut()
        .unwrap()
        .retain(|w| w != "numerator_subset_of_denominator");
    if complete {
        // C6 closes its four-word suffix. Restore kind7's historical position
        // before W-LIN-2, never append it after C6's suffix.
        let at = old["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .position(|w| w == "W-LIN-2")
            .ok_or("missing historical W-LIN-2 boundary")?;
        old["unchecked"]
            .as_array_mut()
            .unwrap()
            .insert(at, json!("quality_denominator"));
    }
    old["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|a| a["field"] != "mutation.kind7");
    old["run"]["completion"] = json!(if other_incomplete {
        "inconclusive"
    } else {
        "complete"
    });
    old["run"]["reason"] = if old_not_run > 0 {
        json!("mutant_limit")
    } else if other_incomplete {
        json!("mutant_incomplete")
    } else {
        Value::Null
    };
    old["run"]["limit_reached"] = json!(old_not_run > 0);
    Ok(old)
}

#[cfg(test)]
mod quantile_objective_tests {
    use super::*;

    #[test]
    fn closed_quantile_objective_cells_reject_missing_extra_and_lower_bound_values() {
        // Hand-built source values, deliberately not produced by the evaluator.
        for (simulation, value, lower_bound) in [
            (
                false,
                json!({"lower":"3","upper":"6","basis":"per_case_relaxation","reason":null}),
                Value::Null,
            ),
            (
                false,
                json!({"lower":null,"upper":null,"basis":null,"reason":"beyond_reached_mass"}),
                json!(2),
            ),
            (
                true,
                json!({"point":"3","observations":2,"censored":0,"reason":null}),
                Value::Null,
            ),
            (
                true,
                json!({"point":null,"observations":2,"censored":1,"reason":"truncated"}),
                json!(4),
            ),
        ] {
            let report = json!({"quality":{"quantiles":[{"name":"Q","value":value,"lower_bound":lower_bound}]}});
            let mut good = json!({"configuration":"A","reason":value["reason"]});
            let fields: &[&str] = if simulation {
                &["point"]
            } else {
                &["lower", "upper", "basis"]
            };
            for field in fields {
                good[*field] = value[*field].clone();
            }
            validate_quantile_objective_cell(&good, &report, "Q", simulation).unwrap();
            for key in good.as_object().unwrap().keys() {
                let mut bad = good.clone();
                bad.as_object_mut().unwrap().remove(key);
                assert!(
                    validate_quantile_objective_cell(&bad, &report, "Q", simulation).is_err(),
                    "missing {key}"
                );
            }
            let mut extra = good.clone();
            extra["lower_bound"] = json!(2);
            assert!(validate_quantile_objective_cell(&extra, &report, "Q", simulation).is_err());
            let mut substituted = good.clone();
            if simulation {
                substituted["point"] = json!(2);
            } else {
                substituted["lower"] = json!(2);
                substituted["upper"] = json!(2);
                substituted["basis"] = json!("exact");
            }
            substituted["reason"] = Value::Null;
            assert!(
                validate_quantile_objective_cell(&substituted, &report, "Q", simulation).is_err()
            );
            let mut unknown = good.clone();
            unknown["reason"] = json!("not_configured");
            assert!(validate_quantile_objective_cell(&unknown, &report, "Q", simulation).is_err());
            let mut wrong_basis = good.clone();
            if !simulation {
                wrong_basis["basis"] = json!("path_range");
                assert!(
                    validate_quantile_objective_cell(&wrong_basis, &report, "Q", simulation)
                        .is_err()
                );
            }
        }
        assert!(field_ref("quantile.Q"));
        assert!(!field_ref("quantile."));
        assert!(!field_ref("quantile.Q.extra"));
    }
}
