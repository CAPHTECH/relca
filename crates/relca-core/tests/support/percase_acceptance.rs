//! ADR 0030 finite population and specification-derived expectations.
#![allow(dead_code)]
use relca_core::{ir, lower, outcome::Outcome, syntax};
use serde_json::{Value, json};
use std::path::PathBuf;

pub const NAMES: [&str; 34] = [
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
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn fixture(name: &str) -> bool {
    assert!(
        !name.starts_with("pcase-") || NAMES.contains(&name),
        "unclassified per-case fixture {name}"
    );
    NAMES.contains(&name)
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(root().join(name).join("expected.json")).unwrap())
        .unwrap()
}
pub fn model(s: &str) -> ir::Model {
    syntax::parse(s)
        .and_then(|a| lower::lower(&a))
        .unwrap_or_else(|d| panic!("per_case input must lower: {d:?}\n{s}"))
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                subset(v, a.get(k).unwrap_or_else(|| panic!("missing {k}: {a}")));
            }
        }
        Value::Array(es) => {
            let rows = a
                .as_array()
                .unwrap_or_else(|| panic!("array required: {a}"));
            assert_eq!(es.len(), rows.len(), "exact inventory");
            for (x, y) in es.iter().zip(rows) {
                subset(x, y);
            }
        }
        _ => assert_eq!(e, a),
    }
}
pub fn probability(out: &Outcome, query: usize, case: usize, lo: &str, hi: &str) {
    let p = &out.properties[query];
    let value = if let Some(cs) = &p.cases {
        assert_eq!(cs[case - 1].case_id.0 + 1, case);
        cs[case - 1].probability.as_ref().unwrap()
    } else {
        assert_eq!(case, 1);
        p.probability.as_ref().unwrap()
    };
    assert_eq!(
        (value.min.to_string(), value.max.to_string()),
        (lo.into(), hi.into()),
        "independent probability oracle query {query} case {case}"
    );
}
pub fn hand_outcome(name: &str, m: &ir::Model, out: &Outcome) {
    let h = expected(name);
    if name.starts_with("pcase-reject") {
        let rejected = if name.contains("broken") { 2 } else { 1 };
        let admission = out
            .admission
            .as_ref()
            .expect("rejections retain their mass");
        assert_eq!(
            (admission.rejected_count.min, admission.rejected_count.max),
            (rejected, rejected)
        );
        let cases = out.case_outcomes.as_ref().unwrap();
        assert_eq!(cases.len(), 2);
        for (i, c) in cases.iter().enumerate() {
            assert_eq!(c.case_id.0, i);
            assert_eq!(c.terminals.len(), 1);
            let t = &c.terminals[0];
            if i == 1 || rejected == 2 {
                assert_eq!(t.kind, relca_core::outcome::CaseTerminalKind::Rejected);
                assert!(t.completed_at.is_none() && t.elapsed.is_none() && t.work.is_none());
            }
            let p = t.probability.as_ref().unwrap();
            assert_eq!(
                (p.min.to_string(), p.max.to_string()),
                ("1".into(), "1".into())
            );
        }
    }
    assert_eq!(out.properties.len(), m.verify.len());
    for (p, v) in out.properties.iter().zip(&m.verify) {
        assert_eq!(p.property, v.property);
    }
    if let Some(b) = h.get("bounds") {
        assert_eq!(json!(out.truncated), b["truncated"]);
    }
    if let Some(props) = h["properties"].as_array() {
        assert_eq!(props.len(), out.properties.len());
        for (i, p) in props.iter().enumerate() {
            if let Some(cs) = p["cases"].as_array() {
                assert_eq!(out.properties[i].cases.as_ref().unwrap().len(), cs.len());
                for (j, c) in cs.iter().enumerate() {
                    probability(
                        out,
                        i,
                        j + 1,
                        c["probability"]["min"].as_str().unwrap(),
                        c["probability"]["max"].as_str().unwrap(),
                    );
                }
            } else if p["probability"].is_object() {
                probability(
                    out,
                    i,
                    1,
                    p["probability"]["min"].as_str().unwrap(),
                    p["probability"]["max"].as_str().unwrap(),
                );
            }
        }
    }
}
pub fn grammar_control(s: &str) -> String {
    s.replace("instance per_case; ", "")
        .replace("instance per_case;", "")
        .replace("cases 2", "cases 1")
        .replace("arrivals fixed [0min, 0min]", "arrivals fixed [0min]")
        .replace("arrivals fixed [0min, 10min]", "arrivals fixed [0min]")
        .replace("arrivals fixed [0min, 3min]", "arrivals fixed [0min]")
        .replace("if Invoice.difficulty == difficult", "if obs == difficult")
}
pub const INSTANCE_AFFECTS: &str =
    "局所値は受付ごとの案件に属し、容量・実行者・Capabilityを持たない";
pub const INDEPENDENCE_AFFECTS: &str = "案件ごとの潜在は案件の間で独立と仮定。同じ時刻の案件の初期分布と一層の積で引く。この独立は分析者の仮定であり、現実の根拠ではない";
pub fn assumptions(v: &Value, actors: &[&str], distribution: bool) {
    let rows = v["assumptions"].as_array().expect("static assumptions");
    let actual: Vec<_> = rows
        .iter()
        .filter(|r| {
            r["field"].as_str().unwrap().ends_with(".instance")
                || r["field"] == "initial.actor_case_independence"
        })
        .cloned()
        .collect();
    let mut expected: Vec<_>=actors.iter().map(|a| json!({"field":format!("{a}.instance"),"value":"per_case","affects":INSTANCE_AFFECTS})).collect();
    if distribution {
        expected.push(json!({"field":"initial.actor_case_independence","value":"independent-per-admission","affects":INDEPENDENCE_AFFECTS}));
    }
    assert_eq!(
        actual, expected,
        "complete ordered new assumption inventory"
    );
}
