//! Input-only adapters. No CDF, probability fold, inverse or reason algorithm.
use crate::{
    ir,
    probability::Rational,
    quality::{Arrival, FirstView},
};
use num_rational::BigRational;
use serde_json::{Value, json};

pub(crate) fn rational(v: &Value) -> Rational {
    Rational(v.as_str().unwrap().parse::<BigRational>().unwrap())
}
pub(crate) struct Graph {
    pub views: Vec<FirstView>,
    pub actions: Vec<Vec<Vec<(Rational, usize)>>>,
}
impl Graph {
    fn insert(&mut self, view: FirstView) -> usize {
        let id = self.views.len();
        self.views.push(view);
        self.actions.push(vec![]);
        id
    }
    fn node(&mut self, v: &Value, within: Option<u128>) -> usize {
        let id = self.insert(FirstView::default());
        if v.is_null() {
            self.views[id].closed = true;
        } else if let Some(t) = v.as_u64() {
            self.views[id].hit = Some(t as u128);
        } else if let Some(a) = v.get("choice") {
            self.actions[id] = a
                .as_array()
                .unwrap()
                .iter()
                .map(|v| vec![(Rational::one(), self.node(v, within))])
                .collect();
        } else if let Some(a) = v.get("chance") {
            self.actions[id] = vec![
                a.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| (rational(&v[0]), self.node(&v[1], within)))
                    .collect(),
            ];
        } else if let Some(a) = v.get("path") {
            let mut prev = id;
            for v in a.as_array().unwrap() {
                let j = self.insert(FirstView {
                    hit: v["hit"]
                        .as_bool()
                        .unwrap()
                        .then(|| v["elapsed"].as_u64().unwrap() as u128),
                    ..Default::default()
                });
                self.actions[prev] = vec![vec![(Rational::one(), j)]];
                prev = j;
            }
            self.views[prev].closed = true;
        } else {
            let c = &v["censored"];
            let earliest = c["earliest"].as_u64().unwrap() as u128;
            let latest = c["latest"].as_u64().map(u128::from).or(within);
            self.views[id].censor = Some(Arrival::Censored {
                earliest,
                latest,
                computational: c["cause"] != "horizon",
            });
        }
        id
    }
}
pub(crate) fn graph(v: &Value) -> Graph {
    let mut g = Graph {
        views: vec![],
        actions: vec![],
    };
    g.node(&v["tree"], v["within"].as_u64().map(u128::from));
    g
}
pub(crate) fn queries(v: &Value) -> Vec<ir::QuantileMetric> {
    v["queries"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, c)| ir::QuantileMetric {
            name: format!("Q{i}"),
            p: rational(&c["p"]),
            property: 0,
            given_reached: c["given_reached"].as_bool().unwrap(),
        })
        .collect()
}
pub(crate) fn flat(value: Value, lb: Value) -> Value {
    let mut v = value;
    v.as_object_mut().unwrap().insert("lower_bound".into(), lb);
    v
}
pub(crate) fn sample(v: &Value) -> Vec<Arrival> {
    v["replications"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|a| a.as_array().unwrap())
        .map(|v| {
            if v.is_null() {
                Arrival::Never
            } else if let Some(t) = v.as_u64() {
                Arrival::Hit(t as u128)
            } else {
                Arrival::Censored {
                    earliest: v["earliest"].as_u64().unwrap() as u128,
                    latest: v["latest"].as_u64().map(u128::from),
                    computational: v["cause"] != "horizon",
                }
            }
        })
        .collect()
}
pub(crate) fn worlds(
    v: &Value,
    mut observe: impl FnMut(&mut [Option<u128>], &[u64], u64, bool, &[(bool, bool, Option<u64>)]),
) -> Value {
    let arrivals = v["arrivals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_u64().unwrap())
        .collect::<Vec<_>>();
    let mut first = vec![None; arrivals.len()];
    let mut events = v["views"].as_array().unwrap().clone();
    if let Some(candidates) = v["candidates"].as_array() {
        for candidate in candidates {
            let mut c = candidate.clone();
            c["received"] = v["views"].as_array().unwrap().last().unwrap()["received"].clone();
            events.push(c);
        }
    }
    for view in events {
        let t = view["time"].as_u64().unwrap();
        let views = arrivals
            .iter()
            .enumerate()
            .map(|(k, _)| {
                let received = view["received"].as_array().unwrap().contains(&json!(k + 1));
                // A shared-state view is not necessarily this case's own event.
                let own_event = view["actor_case"] == json!(k + 1)
                    || (k == 1
                        && v["case2_trace"]
                            .as_array()
                            .is_some_and(|trace| trace.contains(&json!(t))));
                (received, view["flag"] == true, own_event.then_some(t))
            })
            .collect::<Vec<_>>();
        observe(
            &mut first,
            &arrivals,
            t,
            view["selected"].as_bool().unwrap_or(true),
            &views,
        );
    }
    json!({"first_hits":first})
}
pub(crate) fn source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/qtl-sample/model.relca"
    ))
    .unwrap()
}
pub(crate) fn model() -> ir::Model {
    crate::lower::lower(&crate::syntax::parse(&source()).unwrap()).unwrap()
}
pub(crate) fn forge(base: &ir::Model, row: &Value) -> ir::Model {
    let mut m = base.clone();
    let q = m.quality.as_mut().unwrap();
    let v = &row["mutation"];
    if let Some(p) = v.get("p") {
        q.quantiles[0].p = rational(p);
    }
    if let Some(a) = v["p_raw"].as_array() {
        q.quantiles[0].p = Rational(BigRational::new_raw(
            a[0].as_i64().unwrap().into(),
            a[1].as_i64().unwrap().into(),
        ));
    }
    if v.get("property").is_some() {
        q.quantiles[0].property = q.properties.len();
    }
    if let Some(n) = v["name"].as_str() {
        q.quantiles[0].name = n.into();
    }
    if v["duplicate"] == "quantile" {
        q.quantiles.push(q.quantiles[0].clone());
    }
    if v["duplicate"] == "cohort" {
        q.metrics.push(ir::CohortMetric {
            name: q.quantiles[0].name.clone(),
            numerator: 0,
            denominator: ir::Denominator::Arrivals,
        });
    }
    m
}
