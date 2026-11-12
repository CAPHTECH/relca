#![allow(dead_code)]
use relca_core::{engine, ir, lower, outcome, reference, report, syntax};
use serde_json::{Value, json};
use std::path::PathBuf;
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn names() -> Vec<String> {
    serde_json::from_str(include_str!("state_invariant_names.json")).unwrap()
}
pub fn pairs() -> Vec<Value> {
    serde_json::from_str(include_str!("state_invariant_pairs.json")).unwrap()
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join("fixtures").join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(root().join("fixtures").join(name).join("expected.json")).unwrap(),
    )
    .unwrap()
}
pub fn invocation(name: &str) -> Option<Value> {
    let p = root().join("fixtures").join(name).join("invocation.json");
    p.exists()
        .then(|| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap())
}
pub fn change(s: &str, a: &str, b: &str) -> String {
    let t = s.replace(a, b);
    assert_ne!(s, t, "replacement must occur: {a}");
    t
}
pub fn model(s: &str) -> ir::Model {
    let a = syntax::parse(s).unwrap_or_else(|d| {
        panic!("state-invariant syntax not implemented or malformed: {d:?}\n{s}")
    });
    lower::lower(&a).unwrap_or_else(|d| {
        panic!("state-invariant lowering not implemented or malformed: {d:?}\n{s}")
    })
}
pub fn subset(e: &Value, a: &Value) {
    if e.is_object() && e.get("verdict").is_some() {
        assert_eq!(
            e["verdict"], a["verdict"],
            "expected classification before details"
        );
    }
    match e {
        Value::Object(fs) => {
            for (k, v) in fs {
                if k == "diagnostics" {
                    let actual = a[k].as_array().unwrap();
                    let mut want: Vec<_> = v
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|d| d["code"].as_str().unwrap())
                        .collect();
                    let mut got: Vec<_> =
                        actual.iter().map(|d| d["code"].as_str().unwrap()).collect();
                    want.sort();
                    got.sort();
                    assert_eq!(want, got, "exact diagnostic multiset: {a}");
                } else {
                    assert!(a.get(k).is_some(), "missing {k}: {a}");
                    subset(v, &a[k]);
                }
            }
        }
        Value::Array(es) => {
            let actual = a
                .as_array()
                .unwrap_or_else(|| panic!("expected array: {a}"));
            assert_eq!(es.len(), actual.len(), "closed array {e} vs {a}");
            for (e, a) in es.iter().zip(actual) {
                subset(e, a);
            }
        }
        _ => assert_eq!(e, a),
    }
}
pub fn report(name: &str) -> Value {
    serde_json::to_value(report::check(&source(name))).unwrap()
}
pub fn property<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("missing {id}: {v}"))
}
pub fn reject(s: &str, code: &str) {
    let ds = syntax::parse(s)
        .and_then(|a| lower::lower(&a))
        .expect_err("must be statically refused, not delegated to runtime");
    let errors: Vec<_> = ds
        .iter()
        .filter(|d| d.severity == relca_core::diag::Severity::Error)
        .collect();
    assert!(!errors.is_empty());
    assert!(errors.iter().all(|d| d.code == code), "only {code}: {ds:?}");
    for d in errors {
        let p = d.span.unwrap();
        assert!(p.start < p.end && p.end <= s.len());
        assert!(s.is_char_boundary(p.start) && s.is_char_boundary(p.end));
    }
}
fn range(r: outcome::Range) -> Value {
    json!({"min":r.min,"max":r.max})
}
pub fn projection(m: &ir::Model, o: &outcome::Outcome) -> Value {
    let properties: Vec<_> = o
        .properties
        .iter()
        .map(|p| {
            let id = match p.property {
                ir::Property::Contract(c) => m.contracts[c.0].property_id(c),
                ir::Property::StateInRange => "state_in_range".into(),
                _ => panic!("unexpected property in closed sinv runtime population"),
            };
            json!({"id":id,"verdict":p.verdict})
        })
        .collect();
    let terminals:Vec<_>=o.terminals.iter().map(|t|json!({"end":m.ends[t.end.0],"elapsed":range(t.elapsed),"work":range(t.work),"result_at":t.result_at.map(range)})).collect();
    let cases=o.case_outcomes.as_ref().map(|cs|cs.iter().map(|c|json!({"case_id":c.case_id.0+1,"arrival":c.arrival,"terminals":c.terminals.iter().map(|t|json!({"kind":if matches!(t.kind,outcome::CaseTerminalKind::End(_)){"end"}else{"rejected"},"end":match t.kind{outcome::CaseTerminalKind::ReceiptUnknown=>panic!("state-invariant projection does not cover receipt observations"),outcome::CaseTerminalKind::End(e)=>Some(&m.ends[e.0]),_=>None},"completed_at":t.completed_at.map(range),"elapsed":t.elapsed.map(range),"work":t.work.map(range),"result_at":t.result_at.map(range)})).collect::<Vec<_>>()})).collect::<Vec<_>>());
    json!({"properties":properties,"terminals":terminals,"case_outcomes":cases,"bounds":{"truncated":o.truncated},"state_range_violations":o.state_range_violations})
}
pub fn hand_outcome(name: &str, m: &ir::Model, o: &outcome::Outcome) {
    let mut e = expected(name);
    let a = projection(m, o);
    e.as_object_mut().unwrap().remove("verdict");
    e.as_object_mut().unwrap().remove("diagnostics");
    e["bounds"]
        .as_object_mut()
        .unwrap()
        .remove("range_violation_stops");
    // Property case rows are checked directly below and in Report; no fake
    // lower diagnostic prefix is expected from engine::explore.
    for p in e["properties"].as_array_mut().unwrap() {
        p.as_object_mut().unwrap().remove("cases");
    }
    subset(&e, &a);
    assert_eq!(
        !o.state_range_violations.is_empty(),
        expected(name)["bounds"]["range_violation_stops"] == true,
        "range stop is not truncation: {name}"
    );
    assert!(
        !o.diagnostics
            .iter()
            .any(|d| d.severity == relca_core::diag::Severity::Error),
        "unexpected runtime refusal {name}: {:?}",
        o.diagnostics
    );
}
pub fn both(name: &str) {
    let s = source(name);
    let m = model(&s);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "no silent reference exclusion: {name}"
    );
    hand_outcome(name, &m, &e);
    hand_outcome(name, &m, &r.outcome);
    assert_eq!(json!(r.verdict()), expected(name)["verdict"], "{name}");
    assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
    assert_eq!(
        e.diagnostics,
        r.outcome.diagnostics[m.diagnostics.len()..],
        "only reference prepends lower diagnostics"
    );
    assert_eq!(e.properties, r.outcome.properties, "{name}");
    assert_eq!(e.terminals, r.outcome.terminals, "{name}");
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes, "{name}");
    assert_eq!(
        e.state_range_violations, r.outcome.state_range_violations,
        "{name}"
    );
    assert_eq!(
        e.capability_refusals, r.outcome.capability_refusals,
        "{name}"
    );
}
pub fn case_terminal(v: &Value, case: u64, end: &str, time: (u64, u64), work: (u64, u64)) {
    let c = v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["case_id"] == case)
        .unwrap();
    let t = c["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["end"] == end)
        .unwrap();
    assert_eq!(t["completed_at"], json!({"min":time.0,"max":time.1}));
    assert_eq!(t["elapsed"], json!({"min":time.0,"max":time.1}));
    assert_eq!(t["work"], json!({"min":work.0,"max":work.1}));
}
