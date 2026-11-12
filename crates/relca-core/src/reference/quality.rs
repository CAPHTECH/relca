//! Independent reference aggregation. No call to the engine's quality reducers.
use crate::{ir::*, outcome::QueryTruth, probability::Rational, quality::Quality};
use num_rational::BigRational as Q;
use num_traits::Zero;
use serde_json::{Value, json};
mod quantile;
pub(super) use quantile::*;
fn integer(n: u64) -> Q {
    Q::from_integer(n.into())
}
fn empty(r: &str) -> Value {
    json!({"lower":null,"upper":null,"basis":null,"reason":r})
}
fn interval(l: &Q, u: &Q, exact: bool, r: Option<&str>) -> Value {
    json!({"lower":l.to_string(),"upper":u.to_string(),"basis":if exact {"exact"} else {"per_case_relaxation"},"reason":r})
}
fn total(cases: &[([Rational; 4], bool)]) -> (Q, Q, bool, Option<&'static str>) {
    let mut low = Q::zero();
    let mut high = Q::zero();
    let mut exact = true;
    let mut unknown = false;
    let mut computational = false;
    for (b, cut) in cases {
        low += &b[0].0;
        high += &b[3].0;
        exact &= b[0] == b[3];
        unknown |= b[0] != b[1] || b[2] != b[3];
        computational |= *cut && (b[0] != b[1] || b[2] != b[3]);
    }
    (
        low,
        high,
        exact,
        unknown.then_some(if computational {
            "truncated"
        } else {
            "undetermined"
        }),
    )
}
pub(super) fn exhaustive(
    m: &Model,
    ps: &[Vec<([Rational; 4], bool)>],
    os: &[Vec<([Rational; 4], bool)>],
    abort: bool,
) -> Quality {
    let mut out = crate::quality::inventory(m, "exhaustive", abort);
    let spec = m.quality.as_ref().unwrap();
    let times = m.arrivals.clone().unwrap_or_else(|| vec![0]);
    let sums = ps.iter().map(|v| total(v)).collect::<Vec<_>>();
    for (i, p) in spec.properties.iter().enumerate() {
        let (l, u, e, r) = &sums[i];
        let cases=ps[i].iter().enumerate().map(|(k,(b,cut))|json!({"case_id":k+1,"arrival":times[k],"min":if abort {Value::Null} else {json!(b[0].to_string())},"max":if abort {Value::Null} else {json!(b[3].to_string())},"reason":if abort {Some("aborted")} else if b[0]!=b[1]||b[2]!=b[3] {Some(if *cut {"truncated"} else {"undetermined"})} else {None}})).collect::<Vec<_>>();
        out.properties.push(json!({"name":p.name,"condition":crate::quality::definition(m,p),"within":p.within,"cases":cases,"expected_count":if abort {empty("aborted")} else {interval(l,u,*e,*r)}}));
    }
    for metric in &spec.metrics {
        let (nl, nu, ne, nr) = &sums[metric.numerator];
        let (dl, du, de, dr) = match metric.denominator {
            Denominator::Arrivals => (integer(m.bounds.cases), integer(m.bounds.cases), true, None),
            Denominator::Property(k) => sums[k].clone(),
        };
        let reason = if abort {
            Some("aborted")
        } else if *nr == Some("truncated") || dr == Some("truncated") {
            Some("truncated")
        } else if nr.is_some() || dr.is_some() {
            Some("undetermined")
        } else if du.is_zero() {
            Some("zero_denominator")
        } else if dl.is_zero() {
            Some("denominator_may_be_zero")
        } else {
            None
        };
        let bounds = if !abort && (nr.is_some() || dr.is_some()) && !dl.is_zero() {
            json!({"lower":(nl/&du).to_string(),"upper":(nu/&dl).to_string()})
        } else {
            Value::Null
        };
        let value = if let Some(r) = reason {
            empty(r)
        } else {
            interval(&(nl / du), &(nu / dl), *ne && de, None)
        };
        out.cohort.push(json!({"name":metric.name,"form":"ratio_of_expected_counts","numerator":spec.properties[metric.numerator].name,"denominator":crate::quality::denominator(m,&metric.denominator),"value":value,"bounds_with_undetermined":bounds}));
    }
    for ((kind, end), cases) in crate::quality::outcome_keys(m).into_iter().zip(os) {
        let (l, u, e, r) = total(cases);
        let count = if abort {
            empty("aborted")
        } else {
            interval(&l, &u, e, r)
        };
        let rate = if abort {
            empty("aborted")
        } else if let Some(r) = r {
            empty(r)
        } else {
            interval(
                &(l / integer(m.bounds.cases)),
                &(u / integer(m.bounds.cases)),
                e,
                None,
            )
        };
        out.outcomes_per_arrival
            .push(json!({"kind":kind,"end":end,"expected_count":count,"rate":rate}));
    }
    out
}
fn sample(xs: &[u64], reason: Option<&str>) -> Value {
    if let Some(r) = reason {
        return json!({"mean":null,"min":null,"max":null,"samples":0,"reason":r});
    }
    let sum = xs.iter().sum::<u64>();
    json!({"mean":(integer(sum)/integer(xs.len() as u64)).to_string(),"min":xs.iter().min().unwrap().to_string(),"max":xs.iter().max().unwrap().to_string(),"samples":xs.len(),"reason":null})
}
fn ratio(ns: &[u64], ds: &[u64], us: &[u64], vs: &[u64], abort: bool) -> (Value, Value) {
    ratio_stopped(ns, ds, us, vs, abort, true)
}
fn ratio_stopped(
    ns: &[u64],
    ds: &[u64],
    us: &[u64],
    vs: &[u64],
    abort: bool,
    computational: bool,
) -> (Value, Value) {
    let sum = |xs: &[u64]| xs.iter().sum::<u64>();
    let (n, d, u, v) = (sum(ns), sum(ds), sum(us), sum(vs));
    let missing = if abort {
        Some("aborted")
    } else if ns.is_empty() {
        Some("no_samples")
    } else {
        None
    };
    let reason = missing.or(if u > 0 || v > 0 {
        Some(if computational {
            "truncated"
        } else {
            "undetermined"
        })
    } else if d == 0 {
        Some("zero_denominator")
    } else {
        None
    });
    let bounds = if missing.is_none() && (u > 0 || v > 0) && d > 0 {
        json!({"lower":(integer(n)/integer(d+v)).to_string(),"upper":(integer(n+u)/integer(d)).to_string()})
    } else {
        Value::Null
    };
    (
        json!({"point":if reason.is_some() {Value::Null} else {json!((integer(n)/integer(d)).to_string())},"numerator":sample(ns,missing),"denominator":sample(ds,missing),"samples":if missing.is_some() {0} else {ns.len()},"reason":reason}),
        bounds,
    )
}
pub(super) struct Observation {
    pub truths: Vec<Vec<QueryTruth>>,
    pub outcomes: Vec<usize>,
    pub computational: bool,
    pub arrivals: Vec<Vec<crate::quality::Arrival>>,
}
pub(super) fn simulation(m: &Model, observations: &[Observation], abort: bool) -> Quality {
    let missing = if abort {
        Some("aborted")
    } else if observations.is_empty() {
        Some("no_samples")
    } else {
        None
    };
    let mut out = crate::quality::inventory(m, "simulation", missing.is_some());
    let spec = m.quality.as_ref().unwrap();
    let times = m.arrivals.clone().unwrap_or_else(|| vec![0]);
    let mut counts = Vec::new();
    for (i, p) in spec.properties.iter().enumerate() {
        let hits = observations
            .iter()
            .map(|o| {
                o.truths[i]
                    .iter()
                    .filter(|t| **t == QueryTruth::Hit)
                    .count() as u64
            })
            .collect::<Vec<_>>();
        let unknown = observations
            .iter()
            .map(|o| {
                o.truths[i]
                    .iter()
                    .filter(|t| **t == QueryTruth::Unknown)
                    .count() as u64
            })
            .collect::<Vec<_>>();
        let cases=times.iter().enumerate().map(|(k,a)|json!({"case_id":k+1,"arrival":a,"hits":if abort {0} else {observations.iter().filter(|o|o.truths[i][k]==QueryTruth::Hit).count()},"unknown":if abort {0} else {observations.iter().filter(|o|o.truths[i][k]==QueryTruth::Unknown).count()},"trials":if abort {0} else {observations.len()}})).collect::<Vec<_>>();
        out.properties.push(json!({"name":p.name,"condition":crate::quality::definition(m,p),"within":p.within,"cases":cases,"expected_count":sample(&hits,missing),"unknown_count":sample(&unknown,missing)}));
        counts.push((hits, unknown));
    }
    for metric in &spec.metrics {
        let (ns, us) = &counts[metric.numerator];
        let (ds, vs) = match metric.denominator {
            Denominator::Arrivals => (
                vec![m.bounds.cases; observations.len()],
                vec![0; observations.len()],
            ),
            Denominator::Property(k) => counts[k].clone(),
        };
        let computational = observations.iter().any(|o| {
            o.computational && (o.truths[metric.numerator].contains(&QueryTruth::Unknown)
                || matches!(metric.denominator, Denominator::Property(i) if o.truths[i].contains(&QueryTruth::Unknown)))
        });
        let (value, bounds) = if computational {
            ratio(ns, &ds, us, &vs, abort)
        } else {
            ratio_stopped(ns, &ds, us, &vs, abort, false)
        };
        out.cohort.push(json!({"name":metric.name,"form":"ratio_of_expected_counts","numerator":spec.properties[metric.numerator].name,"denominator":crate::quality::denominator(m,&metric.denominator),"value":value,"bounds_with_undetermined":bounds}));
    }
    let keys = crate::quality::outcome_keys(m);
    let unresolved = keys.iter().position(|(name, _)| *name == "receipt_unknown");
    for (i, (kind, end)) in keys.into_iter().enumerate() {
        let counts = observations
            .iter()
            .map(|o| {
                o.outcomes
                    .iter()
                    .filter(|x| **x == i || kind == "unfinished" && Some(**x) == unresolved)
                    .count() as u64
            })
            .collect::<Vec<_>>();
        out.outcomes_per_arrival.push(json!({"kind":kind,"end":end,"expected_count":sample(&counts,missing),"rate":{"point":if missing.is_some() {Value::Null} else {json!((integer(counts.iter().sum())/integer(observations.len() as u64*m.bounds.cases)).to_string())},"samples":if missing.is_some() {0} else {observations.len()},"reason":missing}}));
    }
    if !spec.quantiles.is_empty() {
        out.quantiles = Some(
            spec.quantiles
                .iter()
                .map(|c| {
                    let pool = observations
                        .iter()
                        .flat_map(|o| o.arrivals[c.property].iter().cloned())
                        .collect::<Vec<_>>();
                    let (v, lb) = sample_inverse(&c.p, c.given_reached, &pool, abort);
                    crate::quality::quantile_row(m, c, v, lb)
                })
                .collect(),
        );
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_numerator_precedes_zero_denominator_and_abort_withdraws_both() {
        for abort in [false, true] {
            let (value, bounds) = ratio(&[0], &[0], &[1], &[0], abort);
            assert_eq!(value["reason"], if abort { "aborted" } else { "truncated" });
            assert!(value["point"].is_null());
            assert!(bounds.is_null());
            assert_eq!(value["samples"], if abort { 0 } else { 1 });
        }
    }
    #[test]
    fn endpoints() {
        crate::quality_private_contract::endpoint_contract(|v| {
            let b = std::array::from_fn(|i| Rational(v[i].as_str().unwrap().parse::<Q>().unwrap()));
            let (_, _, exact, r) = total(&[(b, true)]);
            json!({"unknown":r.is_some(),"basis":if exact {"exact"} else {"per_case_relaxation"},"reason":r})
        });
    }
    #[test]
    fn replications() {
        crate::quality_private_contract::replication_contract(|v| {
            let a = |k: &str| {
                v[k].as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap())
                    .collect::<Vec<_>>()
            };
            let (r, b) = ratio(
                &a("numerator"),
                &a("denominator"),
                &a("unknown_numerator"),
                &a("unknown_denominator"),
                false,
            );
            json!({"point":r["point"],"numerator_sample":r["numerator"],"denominator_sample":r["denominator"],"reason":r["reason"],"bounds":b})
        });
    }
}
