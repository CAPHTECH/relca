// ADR0049's closed quantile wire table, included in fixture_match's scope.
fn quantile_schema(sampled: bool) -> Ty {
    use Ty::*;
    let reason = || {
        Null(Box::new(Lit(&[
            "zero_denominator",
            "denominator_may_be_zero",
            "undetermined",
            "truncated",
            "aborted",
            "no_samples",
            "beyond_reached_mass",
            "not_configured",
        ])))
    };
    let value = if sampled {
        Obj(vec![
            ("point", Null(Box::new(Str))),
            ("observations", Int),
            ("censored", Int),
            ("reason", reason()),
        ])
    } else {
        Obj(vec![
            ("lower", Null(Box::new(Str))),
            ("upper", Null(Box::new(Str))),
            (
                "basis",
                Null(Box::new(Lit(&["exact", "per_case_relaxation"]))),
            ),
            ("reason", reason()),
        ])
    };
    Obj(vec![
        ("name", Str),
        ("p", RationalText),
        ("property", Str),
        ("form", Lit(&["all_arrivals", "reached_only"])),
        ("value", value),
        ("lower_bound", Null(Box::new(Str))),
    ])
}

fn quantile_time(value: &Value) -> Result<(), String> {
    if !value.is_null() && metric_fraction(value)?.1 != "1" {
        return Err("quantile time must be a canonical nonnegative integer string".into());
    }
    Ok(())
}

fn validate_quantiles<'a>(
    q: &'a Value,
    sampled: bool,
    names: &std::collections::BTreeSet<&str>,
    metrics: &mut std::collections::BTreeSet<&'a str>,
) -> Result<(), String> {
    let Some(rows) = q.get("quantiles") else {
        if ["checked", "unchecked"]
            .iter()
            .any(|k| q[*k].as_array().unwrap().iter().any(|v| v == "quantiles"))
        {
            return Err("quantile inventory without declaration".into());
        }
        return Ok(());
    };
    let rows = rows.as_array().ok_or("quantile array required")?;
    if rows.is_empty() {
        return Err("quantile declarations must be nonempty".into());
    }
    for row in rows {
        if row["name"].as_str().unwrap().is_empty()
            || !metrics.insert(row["name"].as_str().unwrap())
            || !names.contains(row["property"].as_str().unwrap())
        {
            return Err("quantile identity/property".into());
        }
        let (p, _) = metric_fraction(&row["p"])?;
        if p == "0" || !metric_le(&row["p"], &serde_json::json!("1"))? {
            return Err("quantile level outside (0,1]".into());
        }
        let v = &row["value"];
        let reason = &v["reason"];
        let bound = &row["lower_bound"];
        if row["form"] == "reached_only" && reason == "beyond_reached_mass"
            || row["form"] == "all_arrivals"
                && (reason == "zero_denominator" || reason == "denominator_may_be_zero")
            || sampled && reason == "denominator_may_be_zero"
        {
            return Err("quantile reason/form".into());
        }
        quantile_time(bound)?;
        let missing = if sampled {
            v["point"].is_null()
        } else {
            v["lower"].is_null()
        };
        if missing == reason.is_null() {
            return Err("quantile value/reason".into());
        }
        if !missing && !bound.is_null() {
            return Err("finite quantile has no lower_bound".into());
        }
        if sampled {
            quantile_time(&v["point"])?;
            let n = v["observations"].as_u64().unwrap();
            let c = v["censored"].as_u64().unwrap();
            if c > n || !missing && n == 0 {
                return Err("quantile observation counts".into());
            }
            if row["form"] == "reached_only" && !missing && c == n
                || reason == "zero_denominator" && c > 0
            {
                return Err(
                    "quantile conditional point/denominator needs uncensored evidence".into(),
                );
            }
            if reason == "aborted" || reason == "no_samples" {
                if n != 0 || c != 0 {
                    return Err("quantile withdrawn/empty observations".into());
                }
            } else if n == 0 {
                return Err("quantile empty observations need no_samples/aborted".into());
            }
            if (reason == "truncated" || reason == "undetermined") && c == 0 {
                return Err("quantile censor reason needs censored observations".into());
            }
        } else {
            quantile_time(&v["lower"])?;
            quantile_time(&v["upper"])?;
            if missing != v["upper"].is_null() || missing != v["basis"].is_null() {
                return Err("quantile partial null".into());
            }
            if !missing
                && (!metric_le(&v["lower"], &v["upper"])?
                    || v["basis"] == "exact" && v["lower"] != v["upper"])
            {
                return Err("quantile range/exact".into());
            }
            if reason == "no_samples" {
                return Err("no_samples is simulation only".into());
            }
        }
        if [
            "zero_denominator",
            "denominator_may_be_zero",
            "aborted",
            "no_samples",
            "not_configured",
        ]
        .iter()
        .any(|r| reason.as_str() == Some(*r))
            && !bound.is_null()
        {
            return Err("quantile reason forbids lower_bound".into());
        }
    }
    let aborted = rows.iter().all(|r| {
        matches!(
            r["value"]["reason"].as_str(),
            Some("aborted" | "no_samples")
        )
    });
    let key = if aborted { "unchecked" } else { "checked" };
    let other = if aborted { "checked" } else { "unchecked" };
    if q[key]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| *v == "quantiles")
        .count()
        != 1
        || q[other]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "quantiles")
    {
        return Err("quantile checked/unchecked inventory".into());
    }
    if !aborted && q[key].as_array().unwrap().last() != Some(&serde_json::json!("quantiles")) {
        return Err("quantiles follows old checked inventory".into());
    }
    Ok(())
}
