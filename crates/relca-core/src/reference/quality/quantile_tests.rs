use super::*;
use crate::{quantile_private_contract as contract, quantile_test_support as input};
fn tree(g: &input::Graph, i: usize) -> FirstTree {
    let children = g.actions[i]
        .iter()
        .map(|a| {
            let children = a.iter().map(|(p, j)| (p.clone(), tree(g, *j))).collect();
            (
                Rational::one(),
                FirstTree {
                    random: true,
                    children,
                    ..Default::default()
                },
            )
        })
        .collect();
    FirstTree {
        views: vec![vec![g.views[i].clone()]],
        random: false,
        children,
    }
}
#[test]
fn graph_contract() {
    contract::graph_contract(|row| {
        let g = input::graph(row);
        let t = tree(&g, 0);
        let endpoints = |x, q| t.evaluate(0, 0, x, q, 4096).0.map(|v| v.to_string());
        let at = row["at"].as_array().cloned().unwrap_or_default();
        let quantiles = input::queries(row)
            .iter()
            .map(|c| {
                let (v, b) = tree_quantile(&t, c, 1, 4096);
                input::flat(v, b)
            })
            .collect::<Vec<_>>();
        json!({"A":at.iter().map(|x|endpoints(x.as_u64().unwrap() as u128,0)).collect::<Vec<_>>(),"B":at.iter().map(|x|endpoints(x.as_u64().unwrap() as u128,1)).collect::<Vec<_>>(),"D":endpoints(0,2),"quantiles":quantiles})
    });
}
#[test]
fn sample_contract() {
    contract::sample_contract(|row| {
        let (v, b) = sample_inverse(
            &input::rational(&row["p"]),
            row["given_reached"] == true,
            &input::sample(row),
            row["aborted"] == true,
        );
        input::flat(v, b)
    });
}
#[test]
fn world_contract() {
    contract::world_contract(|row| {
        let mut result = input::worlds(row, observe_first_world);
        let pool = result["first_hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                v.as_u64()
                    .map_or(Arrival::Never, |t| Arrival::Hit(t as u128))
            })
            .collect::<Vec<_>>();
        for (given, key) in [(false, "all"), (true, "given")] {
            let (v, _) = sample_inverse(&Rational::one(), given, &pool, false);
            result[format!("{key}_q1")] = v["point"].clone();
            result[format!("{key}_reason")] = v["reason"].clone();
        }
        result
    });
}
