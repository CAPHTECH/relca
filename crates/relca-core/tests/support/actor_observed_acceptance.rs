#![allow(dead_code)]
use relca_core::{engine, ir, lower, outcome, reference, report, syntax};
use serde_json::{Value, json};
use std::path::PathBuf;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn names() -> Vec<String> {
    serde_json::from_str(include_str!("actor_observed_names.json")).unwrap()
}
pub fn pairs() -> Vec<Value> {
    serde_json::from_str(include_str!("actor_observed_pairs.json")).unwrap()
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
pub fn change(s: &str, from: &str, to: &str) -> String {
    let t = s.replace(from, to);
    assert_ne!(s, t, "replacement must affect the control: {from}");
    t
}
pub fn model(s: &str) -> ir::Model {
    lower::lower(
        &syntax::parse(s)
            .unwrap_or_else(|d| panic!("new syntax not implemented or malformed: {d:?}\n{s}")),
    )
    .unwrap_or_else(|d| panic!("lower {d:?}\n{s}"))
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(fields) => {
            for (k, v) in fields {
                match k.as_str() {
                    "absent_diagnostics" => {
                        for code in v.as_array().unwrap() {
                            assert!(
                                !a["diagnostics"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .any(|d| d["code"] == *code),
                                "forbidden {code}: {a}"
                            );
                        }
                    }
                    "diagnostics" => {
                        for d in v.as_array().unwrap() {
                            assert!(
                                a[k].as_array().unwrap().iter().any(|actual| d
                                    .as_object()
                                    .unwrap()
                                    .iter()
                                    .all(|(k, v)| actual[k] == *v)),
                                "diagnostic {d}: {a}"
                            );
                        }
                    }
                    _ => {
                        assert!(a.get(k).is_some(), "missing {k}: {a}");
                        subset(v, &a[k]);
                    }
                }
            }
        }
        Value::Array(rows) => {
            let actual = a.as_array().unwrap_or_else(|| panic!("not array: {a}"));
            assert_eq!(rows.len(), actual.len(), "closed rows {e} vs {a}");
            for (e, a) in rows.iter().zip(actual) {
                subset(e, a);
            }
        }
        _ => assert_eq!(e, a),
    }
}
pub fn check(name: &str) -> Value {
    let s = source(name);
    if name.starts_with("aobs-simulation") {
        serde_json::to_value(report::simulate(&s)).unwrap()
    } else {
        serde_json::to_value(report::check(&s)).unwrap()
    }
}
pub fn reject(s: &str, code: &str) {
    let ds = syntax::parse(s)
        .and_then(|a| lower::lower(&a))
        .expect_err("must be statically refused, not deferred to engine");
    let errors: Vec<_> = ds
        .iter()
        .filter(|d| d.severity == relca_core::diag::Severity::Error)
        .collect();
    assert!(!errors.is_empty());
    assert!(
        errors.iter().all(|d| d.code == code),
        "expected only {code}: {ds:?}\n{s}"
    );
    for d in errors {
        let span = d.span.unwrap();
        assert!(
            span.start < span.end
                && span.end <= s.len()
                && s.is_char_boundary(span.start)
                && s.is_char_boundary(span.end)
        );
    }
}
fn range(r: outcome::Range) -> Value {
    json!({"min":r.min,"max":r.max})
}
pub fn projection(m: &ir::Model, o: &outcome::Outcome) -> Value {
    let terminals:Vec<_>=o.terminals.iter().map(|t|json!({"end":m.ends[t.end.0],"elapsed":range(t.elapsed),"work":range(t.work),"result_at":t.result_at.map(range)})).collect();
    let cases=o.case_outcomes.as_ref().map(|cs|cs.iter().map(|c|json!({"case_id":c.case_id.0+1,"arrival":c.arrival,"terminals":c.terminals.iter().map(|t|json!({"kind":if matches!(t.kind,outcome::CaseTerminalKind::End(_)){"end"}else{"rejected"},"end":match t.kind {outcome::CaseTerminalKind::ReceiptUnknown=>panic!("actor-state projection does not cover receipt observations"),outcome::CaseTerminalKind::End(e)=>Some(&m.ends[e.0]),_=>None},"completed_at":t.completed_at.map(range),"elapsed":t.elapsed.map(range),"work":t.work.map(range),"result_at":t.result_at.map(range)})).collect::<Vec<_>>()})).collect::<Vec<_>>());
    json!({"terminals":terminals,"case_outcomes":cases,"state_range_violations":o.state_range_violations,"capability_refusals":o.capability_refusals})
}
pub fn terminal(
    v: &Value,
    case: u64,
    end: &str,
    time: (u64, u64),
    elapsed: (u64, u64),
    work: (u64, u64),
) {
    let c = v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["case_id"] == case)
        .unwrap();
    let ts = c["terminals"].as_array().unwrap();
    let t = ts
        .iter()
        .find(|t| t["end"] == end)
        .unwrap_or_else(|| panic!("missing case{case} end {end}: {v}"));
    assert_eq!(t["kind"], "end");
    assert_eq!(t["completed_at"], json!({"min":time.0,"max":time.1}));
    assert_eq!(t["elapsed"], json!({"min":elapsed.0,"max":elapsed.1}));
    assert_eq!(t["work"], json!({"min":work.0,"max":work.1}));
}
pub fn hand_outcome(name: &str, m: &ir::Model, o: &outcome::Outcome) {
    let h = expected(name);
    let v = projection(m, o);
    assert!(
        !o.diagnostics
            .iter()
            .any(|d| d.severity == relca_core::diag::Severity::Error),
        "{name}: {o:?}"
    );
    assert_eq!(o.truncated, h["bounds"]["truncated"].as_bool().unwrap());
    if let Some(rows) = h.get("terminals") {
        subset(rows, &v["terminals"]);
    }
    if let Some(rows) = h.get("state_range_violations") {
        subset(rows, &v["state_range_violations"]);
    }
    assert_eq!(
        !o.state_range_violations.is_empty(),
        h["bounds"]["range_violation_stops"] == true,
        "{name}: not computational truncation"
    );
    for p in &o.properties {
        if matches!(p.property, ir::Property::StateInRange) {
            assert_eq!(
                json!(p.verdict),
                if !o.state_range_violations.is_empty() {
                    json!("violated")
                } else if o.truncated {
                    json!("inconclusive")
                } else {
                    json!("satisfied")
                }
            );
        }
    }
    let broken = name.contains(".broken-");
    if [
        "aobs-shared-order",
        "aobs-capacity",
        "aobs-labels",
        "aobs-labels.broken-renamed",
    ]
    .contains(&name)
    {
        for c in 1..=2 {
            terminal(&v, c, "posted", (1, 1), (1, 1), (1, 1));
            terminal(&v, c, "denied", (1, 1), (1, 1), (0, 0));
            assert_eq!(
                v["case_outcomes"][(c - 1) as usize]["terminals"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
        }
        let rs = o.capability_refusals.as_ref().unwrap();
        assert_eq!(rs.len(), 2);
        for (i, r) in rs.iter().enumerate() {
            assert_eq!(r.case_id, i as u64 + 1);
            assert_eq!(r.at.min, 1);
            assert_eq!(r.at.max, 1);
            assert_eq!(r.coverage, "complete");
        }
        assert_eq!(o.admission.as_ref().unwrap().rejected_count.min, 0);
        assert_eq!(o.admission.as_ref().unwrap().rejected_count.max, 0);
    }
    if name.starts_with("aobs-start-visible") {
        terminal(&v, 1, "done", (5, 5), (5, 5), (5, 5));
        terminal(
            &v,
            2,
            "done",
            if broken { (10, 10) } else { (5, 5) },
            if broken { (8, 8) } else { (3, 3) },
            if broken { (5, 5) } else { (3, 3) },
        );
    }
    if name == "aobs-late" {
        terminal(&v, 1, "posted", (1, 1), (1, 1), (1, 1));
        terminal(&v, 2, "denied", (3, 3), (0, 0), (0, 0));
        let rs = o.capability_refusals.as_ref().unwrap();
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].case_id, 2);
        assert_eq!(rs[0].at.min, 3);
        assert_eq!(rs[0].at.max, 3);
    }
    if name.starts_with("aobs-read-write-order") {
        terminal(&v, 1, "done", (2, 2), (2, 2), (2, 2));
        terminal(
            &v,
            2,
            "done",
            if broken { (4, 4) } else { (3, 4) },
            if broken { (1, 1) } else { (1, 2) },
            if broken { (1, 1) } else { (1, 2) },
        );
    }
    if name.starts_with("aobs-per-case") {
        terminal(&v, 1, "done", (1, 1), (1, 1), (1, 1));
        terminal(&v, 2, "done", (4, 4), (1, 1), (1, 1));
    }
    if [
        "aobs-shared-order.broken-no-requires",
        "aobs-late.broken-no-requires",
    ]
    .contains(&name)
    {
        assert!(o.capability_refusals.is_none());
        let ws = &o.state_range_violations;
        assert_eq!(ws.len(), if name.starts_with("aobs-late") { 1 } else { 2 });
        for w in ws {
            assert_eq!(w.variable, "Ledger.posted");
            assert_eq!(w.before, "1");
            assert_eq!(w.attempted, "2");
            assert_eq!(w.time, if name.starts_with("aobs-late") { 4 } else { 2 });
        }
    }
}
pub fn both(name: &str) {
    let s = source(name);
    let m = model(&s);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "no reference exclusion {name}: {r:?}"
    );
    hand_outcome(name, &m, &e);
    hand_outcome(name, &m, &r.outcome);
    assert_eq!(json!(r.verdict()), expected(name)["verdict"], "{name}");
    assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
    assert_eq!(
        e.diagnostics,
        r.outcome.diagnostics[m.diagnostics.len()..],
        "only reference prepends exact lower prefix"
    );
    assert_eq!(e.properties, r.outcome.properties, "{name}");
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes, "{name}");
    assert_eq!(e.terminals, r.outcome.terminals, "{name}");
    assert_eq!(
        e.state_range_violations, r.outcome.state_range_violations,
        "{name}"
    );
    assert_eq!(
        e.capability_refusals, r.outcome.capability_refusals,
        "{name}"
    );
    assert_eq!(
        e.unchecked, r.outcome.unchecked,
        "range-stop coverage must not be silently normalized"
    );
}
