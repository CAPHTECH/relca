#![allow(dead_code)]
use relca_core::{engine, ir, lower, reference, report, syntax};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
#[path = "responsibility_defaults_oracle.rs"]
mod responsibility_oracle;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn names() -> Vec<String> {
    serde_json::from_str(include_str!("record_names.json")).unwrap()
}
pub fn fixture(name: &str) -> bool {
    let found = names().iter().any(|n| n == name);
    assert!(
        !name.starts_with("rec-") || found,
        "unclassified record fixture {name}"
    );
    found
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
pub fn expected_for_source(name: &str, source: &str) -> Value {
    if let Some(model) = syntax::parse(source)
        .ok()
        .and_then(|ast| lower::lower(&ast).ok())
    {
        responsibility_oracle::with_diagnostics(&model, &expected(name))
    } else {
        expected(name)
    }
}
pub fn oracle() -> Value {
    serde_json::from_str(include_str!("record_oracle.json")).unwrap()
}
pub fn model(s: &str) -> ir::Model {
    lower::lower(
        &syntax::parse(s).unwrap_or_else(|e| panic!("new syntax not implemented: {e:?}\n{s}")),
    )
    .unwrap_or_else(|e| panic!("new lowering not implemented: {e:?}\n{s}"))
}
pub fn changed(s: &str, from: &str, to: &str) -> String {
    assert!(s.contains(from), "missing edit {from}");
    let t = s.replace(from, to);
    assert_ne!(s, t, "edit must happen");
    t
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                assert!(a.get(k).is_some(), "missing {k}: {a}");
                subset(v, &a[k]);
            }
        }
        Value::Array(es) => {
            let rows = a
                .as_array()
                .unwrap_or_else(|| panic!("expected array: {e} vs {a}"));
            assert_eq!(es.len(), rows.len(), "closed row inventory {e} vs {a}");
            for (e, a) in es.iter().zip(rows) {
                subset(e, a);
            }
        }
        _ => assert_eq!(e, a),
    }
}
/// ADR 0043 (C5) migration, counted by hand from the source (never from
/// output): in these four the only effect Work.run is executed by Work
/// [executors p] alone, the legacy refusal row is derived (no judgement
/// candidate: Seed.seed() has no non-case argument) and no executor-less
/// carrier exists, so single_person is true and W-AGY-2 appears once (ADR 0043
/// ruling M4). p has a duty, so W-RESP-0 does not. Every other record input
/// must have no W-AGY-2. The authored expected files stay unchanged.
const C5_SINGLE_PERSON: &[&str] = &[
    "rec-context-duty-and",
    "rec-context-duty-compare",
    "rec-context-duty-implies",
    "rec-context-duty-or",
];
pub fn report_oracle(name: &str, actual: &Value) {
    report_oracle_for_source(name, &source(name), actual);
}
pub fn report_oracle_for_source(name: &str, source: &str, actual: &Value) {
    let mut actual = actual.clone();
    let diagnostics = actual["diagnostics"].as_array_mut().unwrap();
    let concentration: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["code"] == "W-AGY-2")
        .cloned()
        .collect();
    if C5_SINGLE_PERSON.contains(&name) {
        assert_eq!(concentration.len(), 1, "{name}: one W-AGY-2 (C5)");
        assert_eq!(concentration[0]["severity"], "warning");
        assert!(
            concentration[0]["message"]
                .as_str()
                .unwrap()
                .starts_with("Work.run "),
            "{name}: {concentration:?}"
        );
        diagnostics.retain(|d| d["code"] != "W-AGY-2");
    } else {
        assert!(concentration.is_empty(), "{name}: no W-AGY-2 (C5)");
    }
    let actual = &actual;
    let e = if let Some(model) = syntax::parse(source)
        .ok()
        .and_then(|ast| lower::lower(&ast).ok())
    {
        // ADR 0054: original record syntax and legacy skeletons may have
        // different Actor spans, so derive from this exact source, not output.
        responsibility_oracle::assert_report(&model, actual);
        responsibility_oracle::with_diagnostics(&model, &expected(name))
    } else {
        expected(name)
    };
    assert_eq!(e["verdict"], actual["verdict"], "{name}: hand verdict");
    let mut ec: Vec<_> = e["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (&d["code"], &d["severity"]))
        .collect();
    let mut ac: Vec<_> = actual["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (&d["code"], &d["severity"]))
        .collect();
    ec.sort_by_key(|(c, _)| c.as_str());
    ac.sort_by_key(|(c, _)| c.as_str());
    assert_eq!(
        ec, ac,
        "{name}: exact diagnostics, no secondary fallback errors"
    );
    let mut e = e.clone();
    e.as_object_mut().unwrap().remove("diagnostics");
    // End rows have identity, not an author-chosen serialization order.
    if let Some(rows) = e.get_mut("terminals").and_then(Value::as_array_mut) {
        rows.sort_by_key(|v| v["end"].as_str().unwrap().to_owned());
    }
    let mut a = actual.clone();
    if let Some(rows) = a.get_mut("terminals").and_then(Value::as_array_mut) {
        rows.sort_by_key(|v| v["end"].as_str().unwrap().to_owned());
    }
    subset(&e, &a);
}
pub fn acceptance(name: &str) {
    let s = source(name);
    let e = expected(name);
    let phase = oracle()[name]["phase"].as_str().unwrap().to_owned();
    let checked = serde_json::to_value(report::check(&s)).unwrap();
    report_oracle(name, &checked);
    if phase != "runtime" {
        let errors: Vec<_> = checked["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(errors.len(), 1, "{name}: one intended primary error");
        if let Some(message) = oracle()[name]["message"].as_str() {
            assert!(
                errors[0]["message"].as_str().unwrap().contains(message),
                "{name}: {errors:?}"
            );
        }
        let start = errors[0]["span"]["start"].as_u64().unwrap() as usize;
        let end = errors[0]["span"]["end"].as_u64().unwrap() as usize;
        assert!(
            start < end && end <= s.len() && s.is_char_boundary(start) && s.is_char_boundary(end)
        );
        let token = if name == "rec-s20-missing.broken-invalid" {
            Some(("Entry {}", 0))
        } else if name == "rec-s21-duplicate.broken-invalid" {
            Some(("confidence", 2))
        } else if name == "rec-s22-unknown.broken-invalid"
            || name == "rec-s23-undefined-field.broken-invalid"
        {
            Some(("tone", 0))
        } else if name == "rec-s24-ambiguous.broken-invalid" {
            Some(("e.confidence", 0))
        } else if name == "rec-s25-member.broken-invalid" {
            Some(("medium", 0))
        } else {
            None
        };
        if let Some((token, ordinal)) = token {
            let at = s.match_indices(token).nth(ordinal).unwrap().0;
            let width = if token == "e.confidence" {
                1
            } else {
                token.len()
            };
            assert_eq!(
                (start, end),
                (at, at + width),
                "{name}: independently located primary token"
            );
        }
        let parsed = syntax::parse(&s);
        if phase == "parse" {
            assert!(parsed.is_err(), "{name}: parser must reject");
        } else {
            assert!(
                lower::lower(&parsed.unwrap_or_else(|e| panic!("{name}: must reach lower: {e:?}")))
                    .is_err()
            );
        }
        return;
    }
    let m = model(&s);
    if name == "rec-record-refusal" {
        assert_eq!(
            m.capabilities[0].results,
            vec![ir::ValueId(0)],
            "refusal literal is not a normal result"
        );
        assert_eq!(
            m.capabilities[0].requires.as_ref().unwrap().otherwise,
            ir::ValueId(1)
        );
    }
    let a = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "{name}: no silent reference exclusion"
    );
    for (label, outcome) in [("engine", &a), ("reference", &r.outcome)] {
        assert_eq!(
            outcome.truncated,
            e["bounds"]["truncated"].as_bool().unwrap(),
            "{name}/{label}"
        );
        assert_eq!(
            outcome.properties.len(),
            checked["properties"].as_array().unwrap().len(),
            "{name}/{label}"
        );
        for (p, expected) in outcome
            .properties
            .iter()
            .zip(checked["properties"].as_array().unwrap())
        {
            assert_eq!(
                serde_json::to_value(p.verdict).unwrap(),
                expected["verdict"],
                "{name}/{label}: properties after hand report assertion"
            );
        }
        if let Some(rows) = e["terminals"].as_array() {
            assert_eq!(
                outcome.terminals.len(),
                rows.len(),
                "{name}/{label}: no invented end"
            );
            for t in &outcome.terminals {
                let want = rows.iter().find(|e| e["end"] == m.ends[t.end.0]).unwrap();
                assert_eq!(
                    json!({"min":t.elapsed.min,"max":t.elapsed.max}),
                    want["elapsed"],
                    "{name}/{label}"
                );
                assert_eq!(
                    json!({"min":t.work.min,"max":t.work.max}),
                    want["work"],
                    "{name}/{label}"
                );
                if let Some(p) = want.get("probability") {
                    assert_eq!(
                        serde_json::to_value(&t.probability).unwrap(),
                        *p,
                        "{name}/{label}: terminal mass"
                    );
                }
            }
        }
        if let Some(p) = e
            .get("properties")
            .and_then(Value::as_array)
            .and_then(|p| p.iter().find(|p| p.get("probability").is_some()))
        {
            let actual = outcome
                .properties
                .iter()
                .find_map(|p| p.probability.as_ref())
                .unwrap();
            assert_eq!(
                actual.min.to_string(),
                p["probability"]["min"].as_str().unwrap(),
                "{name}/{label}: independent rational"
            );
            assert_eq!(
                actual.max.to_string(),
                p["probability"]["max"].as_str().unwrap(),
                "{name}/{label}: independent rational"
            );
        }
        if let Some(rows) = e.get("case_outcomes").and_then(Value::as_array) {
            let actual = outcome.case_outcomes.as_ref().unwrap();
            assert_eq!(
                actual.len(),
                rows.len(),
                "{name}/{label}: arrival denominator"
            );
            for (a, e) in actual.iter().zip(rows) {
                assert_eq!(
                    a.case_id.0 as u64 + 1,
                    e["case_id"].as_u64().unwrap(),
                    "{name}/{label}: wire case_id is 1-based"
                );
                assert_eq!(a.arrival, e["arrival"].as_u64().unwrap());
                assert_eq!(a.terminals.len(), e["terminals"].as_array().unwrap().len());
                for t in &a.terminals {
                    let relca_core::outcome::CaseTerminalKind::End(end) = t.kind else {
                        panic!("{name}: no rejected case")
                    };
                    let want = e["terminals"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|r| r["end"] == m.ends[end.0])
                        .unwrap();
                    let elapsed = t.elapsed.unwrap();
                    let work = t.work.unwrap();
                    assert_eq!(
                        json!({"min":elapsed.min,"max":elapsed.max}),
                        want["elapsed"]
                    );
                    assert_eq!(json!({"min":work.min,"max":work.max}), want["work"]);
                    assert_eq!(
                        serde_json::to_value(&t.probability).unwrap(),
                        want["probability"]
                    );
                }
            }
        }
        if let Some(want) = e
            .get("properties")
            .and_then(Value::as_array)
            .and_then(|rows| rows.iter().find_map(|p| p.get("cases")))
        {
            let actual = outcome
                .properties
                .iter()
                .find_map(|p| p.cases.as_ref())
                .unwrap();
            assert_eq!(actual.len(), 1, "{name}/{label}");
            assert_eq!(
                serde_json::to_value(&actual[0].probability).unwrap(),
                want[0]["probability"]
            );
        }
    }
    // engine excludes lower diagnostics; only reference prepends them.
    assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
    let rd = &r.outcome.diagnostics[m.diagnostics.len()..];
    let key = |ds: &[relca_core::diag::Diagnostic]| {
        ds.iter()
            .map(|d| (d.code.clone(), d.severity, d.span))
            .collect::<Vec<_>>()
    };
    assert_eq!(key(&a.diagnostics), key(rd), "{name}: runtime diagnostics");
    assert_eq!(
        a.properties, r.outcome.properties,
        "{name}: both backends checked against hand oracle first"
    );
}
pub fn terminal_map(
    o: &relca_core::outcome::Outcome,
    m: &ir::Model,
) -> BTreeMap<String, (u64, u64)> {
    o.terminals
        .iter()
        .map(|t| (m.ends[t.end.0].clone(), (t.elapsed.min, t.work.min)))
        .collect()
}
