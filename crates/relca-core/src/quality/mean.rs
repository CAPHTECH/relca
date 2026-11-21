//! Expected first-hit rewards on the DAG. None is +infinity, never zero.
use super::*;

#[derive(Debug, Clone)]
pub(crate) struct MeanView {
    pub first: FirstView,
    pub amount: Option<Rational>,
}
impl MeanView {
    pub(crate) fn elapsed(first: FirstView) -> Self {
        let t = first
            .hit
            .or(match first.censor {
                Some(Arrival::Censored { earliest, .. }) => Some(earliest),
                _ => None,
            })
            .unwrap_or(0);
        Self {
            first,
            amount: Some(Rational(BigRational::from_integer(t.into()))),
        }
    }
}

pub(crate) fn mean_row(m: &ir::Model, c: &ir::MeanMetric, value: Value, lb: Value) -> Value {
    json!({"name":c.name,"measure":c.measure.name(),
        "property":c.property.map(|i| &m.quality.as_ref().unwrap().properties[i].name),
        "form":if c.given_reached {"reached_only"} else {"all_arrivals"},
        "value":value,"lower_bound":lb})
}

pub(crate) fn sample_mean_row(
    m: &ir::Model,
    c: &ir::MeanMetric,
    point: Option<String>,
    observations: u64,
    censored: u64,
    reason: Option<&str>,
) -> Value {
    mean_row(
        m,
        c,
        json!({"point":point,"observations":observations,"censored":censored,"reason":reason}),
        Value::Null,
    )
}

pub(crate) fn graph_mean(
    c: &ir::MeanMetric,
    order: &[usize],
    actions: impl Fn(usize) -> Vec<Vec<(Rational, usize)>>,
    views: &[Vec<MeanView>],
    bits: u64,
) -> (Value, Value) {
    let fits = |x: &BigRational| x.numer().bits() <= bits && x.denom().bits() <= bits;
    let mut totals = [Some(BigRational::zero()), Some(BigRational::zero())];
    let mut den = [BigRational::zero(), BigRational::zero()];
    let mut reason = None;
    for vs in views {
        let mut table = vec![[None, None]; vs.len()];
        let mut reasons = vec![None; vs.len()];
        for &i in order.iter().rev() {
            let v = &vs[i].first;
            let Some(amount) = &vs[i].amount else {
                reasons[i] = Some("not_configured");
                continue;
            };
            let amount = &amount.0;
            if v.hit.is_some() {
                if !fits(amount) {
                    table[i] = [Some(BigRational::zero()), None];
                    reasons[i] = Some("truncated");
                    continue;
                }
                let x = amount.clone();
                table[i] = [Some(x.clone()), Some(x)];
                continue;
            }
            let miss = if c.given_reached {
                Some(BigRational::zero())
            } else {
                None
            };
            if v.closed {
                table[i] = [miss.clone(), miss];
                continue;
            }
            let mut choices = Vec::new();
            if let Some(Arrival::Censored { computational, .. }) = &v.censor {
                reasons[i] = Some(if *computational {
                    "truncated"
                } else {
                    "undetermined"
                });
                choices.push([
                    Some(if c.given_reached {
                        BigRational::zero()
                    } else {
                        amount.clone()
                    }),
                    None,
                ]);
            }
            for action in actions(i) {
                let mut b = [Some(BigRational::zero()), Some(BigRational::zero())];
                let mut overflow = false;
                for (p, j) in action.into_iter().filter(|(p, _)| !p.is_zero()) {
                    if reasons[j] == Some("not_configured")
                        || reasons[i] != Some("not_configured")
                            && (reasons[j] == Some("truncated") || reasons[i].is_none())
                    {
                        reasons[i] = reasons[j];
                    }
                    overflow |= !p.within_bits(bits);
                    for z in 0..2 {
                        b[z] = b[z].as_ref().zip(table[j][z].as_ref()).map(|(a, x)| {
                            let term = &p.0 * x;
                            let sum = a + &term;
                            overflow |= !fits(&term) || !fits(&sum);
                            sum
                        });
                    }
                }
                if overflow {
                    if reasons[i] != Some("not_configured") {
                        reasons[i] = Some("truncated");
                    }
                    b = [Some(BigRational::zero()), None];
                }
                choices.push(b);
            }
            table[i] = if choices.is_empty() {
                [miss.clone(), miss]
            } else {
                [
                    choices.iter().filter_map(|b| b[0].clone()).min(),
                    if choices.iter().any(|b| b[1].is_none()) {
                        None
                    } else {
                        choices.iter().filter_map(|b| b[1].clone()).max()
                    },
                ]
            };
        }
        for z in 0..2 {
            totals[z] = totals[z]
                .as_ref()
                .zip(table[0][z].as_ref())
                .map(|(a, b)| a + b);
        }
        if reasons[0] == Some("not_configured")
            || reason != Some("not_configured")
                && (reasons[0] == Some("truncated") || reason.is_none())
        {
            reason = reasons[0];
        }
        if c.given_reached {
            let first: Vec<_> = vs.iter().map(|v| v.first.clone()).collect();
            let d = first_reduce(order, &actions, &first, 0, 2, bits);
            den[0] += &d.b[0].0;
            den[1] += &d.b[3].0;
            if reason != Some("not_configured")
                && (d.reason == Some("truncated") || reason.is_none())
            {
                reason = d.reason;
            }
        }
        if reason != Some("not_configured") && totals.iter().flatten().chain(&den).any(|x| !fits(x))
        {
            totals = [Some(BigRational::zero()), None];
            reason = Some("truncated");
        }
    }
    let missing = |r, lb: Option<BigRational>| (absent(r), json!(lb.map(|x| x.to_string())));
    if reason == Some("not_configured") {
        return missing("not_configured", None);
    }
    if c.given_reached {
        if let Some(r) = reason {
            return missing(r, None);
        }
        if den[1].is_zero() {
            return missing("zero_denominator", None);
        }
        if den[0].is_zero() {
            return missing("denominator_may_be_zero", None);
        }
    } else {
        let n = number(views.len() as u64);
        den = [n.clone(), n];
        if totals[0].is_none() {
            return missing("beyond_reached_mass", None);
        }
    }
    let lower = totals[0].as_ref().map(|x| x / &den[1]);
    if let Some(r) = reason {
        return missing(r, if c.given_reached { None } else { lower });
    }
    match (lower, totals[1].as_ref()) {
        (Some(l), Some(u)) => {
            let u = u / &den[0];
            (range(&l, &u, l == u, None), Value::Null)
        }
        (l, _) => missing("beyond_reached_mass", l),
    }
}

#[cfg(test)]
#[test]
fn mean_numeric_cut_is_local_and_never_a_value() {
    let c = ir::MeanMetric {
        name: "Cost".into(),
        measure: ir::MeanMeasure::Cost,
        property: None,
        given_reached: false,
    };
    let view = MeanView {
        first: FirstView {
            hit: Some(0),
            ..Default::default()
        },
        amount: Some(Rational(BigRational::from_integer(83.into()))),
    };
    let run = |bits| graph_mean(&c, &[0], |_| vec![], &[vec![view.clone()]], bits);
    assert_eq!(run(4096).0["lower"], "83");
    let (v, lb) = run(2);
    assert_eq!(v["reason"], "truncated");
    assert!(v["lower"].is_null());
    assert_eq!(lb, "0");
}

pub(super) fn mean_wire(q: &Quality) -> Result<(), &'static str> {
    let Some(rows) = &q.means else {
        if q.checked.iter().chain(&q.unchecked).any(|s| s == "means") {
            return Err("mean inventory without declaration");
        }
        return Ok(());
    };
    if rows.is_empty() {
        return Err("empty means");
    }
    let mut names: std::collections::BTreeSet<_> = q
        .cohort
        .iter()
        .chain(q.quantiles.iter().flatten())
        .filter_map(|r| r["name"].as_str())
        .collect();
    for row in rows {
        keys(
            row,
            &[
                "name",
                "measure",
                "property",
                "form",
                "value",
                "lower_bound",
            ],
        )?;
        if !row["name"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && names.insert(s))
            || !matches!(
                row["measure"].as_str(),
                Some("elapsed" | "work" | "named_people_work" | "cost")
            )
            || !(row["property"].is_null()
                && row["measure"] != "elapsed"
                && row["form"] == "all_arrivals"
                || row["property"]
                    .as_str()
                    .is_some_and(|s| q.properties.iter().any(|p| p["name"] == s)))
            || !matches!(row["form"].as_str(), Some("all_arrivals" | "reached_only"))
        {
            return Err("invalid mean definition");
        }
        let v = &row["value"];
        if q.analysis == "simulation" {
            keys(v, &["point", "observations", "censored", "reason"])?;
            let n = v["observations"]
                .as_u64()
                .ok_or("invalid mean observations")?;
            let censored = v["censored"].as_u64().ok_or("invalid censored means")?;
            let valid = match v["reason"].as_str() {
                None => {
                    v["reason"].is_null()
                        && n > 0
                        && censored == 0
                        && quantity_value(&v["point"]).is_some_and(|p| p >= BigRational::zero())
                }
                Some("aborted" | "no_samples" | "zero_denominator") => n == 0 && censored == 0,
                Some("truncated" | "undetermined") => censored > 0,
                Some("not_configured") => true,
                Some("beyond_reached_mass") => row["form"] == "all_arrivals" && censored == 0,
                _ => false,
            };
            if !valid
                || !v["reason"].is_null() && !v["point"].is_null()
                || !row["lower_bound"].is_null()
                || v["reason"] == "zero_denominator" && row["form"] != "reached_only"
            {
                return Err("invalid sampled mean");
            }
            continue;
        }
        keys(v, &["lower", "upper", "basis", "reason"])?;
        let lb = &row["lower_bound"];
        if !lb.is_null() && quantity_value(lb).is_none_or(|x| x < BigRational::zero()) {
            return Err("invalid mean bound");
        }
        if v["lower"].is_null() {
            if !v["upper"].is_null()
                || !v["basis"].is_null()
                || !matches!(
                    v["reason"].as_str(),
                    Some(
                        "aborted"
                            | "truncated"
                            | "undetermined"
                            | "zero_denominator"
                            | "denominator_may_be_zero"
                            | "beyond_reached_mass"
                            | "not_configured"
                    )
                )
            {
                return Err("invalid missing mean");
            }
            if !lb.is_null()
                && (row["form"] == "reached_only"
                    || !matches!(
                        v["reason"].as_str(),
                        Some("truncated" | "undetermined" | "beyond_reached_mass")
                    ))
            {
                return Err("spurious mean bound");
            }
            if row["form"] == "all_arrivals"
                && matches!(
                    v["reason"].as_str(),
                    Some("zero_denominator" | "denominator_may_be_zero")
                )
                || row["form"] == "reached_only" && v["reason"] == "beyond_reached_mass"
            {
                return Err("mean reason/form mismatch");
            }
        } else {
            let l = quantity_value(&v["lower"]).ok_or("invalid mean lower")?;
            let u = quantity_value(&v["upper"]).ok_or("invalid mean upper")?;
            if l < BigRational::zero()
                || l > u
                || !v["reason"].is_null()
                || !lb.is_null()
                || !matches!(v["basis"].as_str(), Some("exact" | "per_case_relaxation"))
                || (v["basis"] == "exact") != (l == u)
            {
                return Err("invalid mean interval");
            }
        }
    }
    let aborted = rows.iter().all(|r| {
        matches!(
            r["value"]["reason"].as_str(),
            Some("aborted" | "no_samples")
        )
    });
    if q.checked.iter().filter(|s| *s == "means").count() != usize::from(!aborted)
        || q.unchecked.iter().filter(|s| *s == "means").count() != usize::from(aborted)
    {
        return Err("mean inventory mismatch");
    }
    Ok(())
}
