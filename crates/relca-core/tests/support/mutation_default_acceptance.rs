//! Hand oracle shared by core and CLI tests; never calls generation helpers.
#![allow(dead_code)]
use relca_core::{ast, syntax};
use serde_json::{Value, json};
use std::path::PathBuf;
#[path = "response_boundary_oracle.rs"]
mod response_boundary;
#[path = "responsibility_defaults_oracle.rs"]
mod responsibility_oracle;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn oracle() -> Value {
    serde_json::from_str(include_str!("mutation_default_oracle.json")).unwrap()
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}

fn expected_span(s: &str, m: &Value) -> relca_core::diag::Span {
    let ast = syntax::parse(s).unwrap();
    if m["operator"] == "expire_authority" {
        let a = ast
            .items
            .iter()
            .find_map(|item| match item {
                ast::Item::Authority(a)
                    if a.name.value == m["site"]["authority"].as_str().unwrap() =>
                {
                    Some(a)
                }
                _ => None,
            })
            .unwrap();
        return a
            .items
            .iter()
            .find_map(|item| {
                matches!(item.value, ast::AuthorityItem::Expires(_)).then_some(item.span)
            })
            .unwrap_or(a.span);
    }
    fn find(block: &ast::Block, binding: &str) -> Option<relca_core::diag::Span> {
        for stmt in block {
            match &stmt.value {
                ast::Stmt::Call(c) if c.binding.value == binding => return Some(c.span),
                ast::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    if let Some(span) =
                        find(then_body, binding).or_else(|| find(else_body, binding))
                    {
                        return Some(span);
                    }
                }
                _ => {}
            }
        }
        None
    }
    ast.items
        .iter()
        .find_map(|item| match item {
            ast::Item::Process(p) if p.name.value == m["site"]["process"].as_str().unwrap() => {
                find(&p.body, m["site"]["affected_bindings"][0].as_str().unwrap())
            }
            _ => None,
        })
        .unwrap()
}
pub fn partial(expected: &Value, actual: &Value, path: &str) {
    match expected {
        Value::Object(o) => {
            for (k, v) in o {
                partial(
                    v,
                    actual
                        .get(k)
                        .unwrap_or_else(|| panic!("missing {path}/{k}")),
                    &format!("{path}/{k}"),
                );
            }
        }
        Value::Array(a) => {
            let b = actual
                .as_array()
                .unwrap_or_else(|| panic!("{path}: expected array"));
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                partial(a, b, &format!("{path}/{i}"));
            }
        }
        _ => assert_eq!(expected, actual, "{path}"),
    }
}
pub fn check(name: &str, v: &Value) {
    let all = oracle();
    let e = &all[name];
    assert!(e.is_object(), "unknown fixture {name}");
    let mut hand = e["expected"].clone();
    if let Some(original) = hand.get_mut("original")
        && let Ok(ast) = syntax::parse(&source(name))
        && let Ok(model) = relca_core::lower::lower(&ast)
    {
        // ADR 0054: the original is a canonical Report, not the mutation root.
        responsibility_oracle::assert_report(&model, &v["original"]);
        *original = responsibility_oracle::with_diagnostics(&model, original);
    }
    partial(&hand, v, name);
    for (n, i) in [1, 4].iter().enumerate() {
        partial(&e["rows"][n], &v["mutations"][i], &format!("{name}/row{i}"));
        for m in v["mutations"][i]["mutants"].as_array().unwrap() {
            let site = &m["site"];
            let s = source(name);
            let start = site["span"]["start"].as_u64().unwrap() as usize;
            let end = site["span"]["end"].as_u64().unwrap() as usize;
            let span = expected_span(&s, m);
            assert_eq!(
                site["span"],
                json!({"start":span.start,"end":span.end}),
                "{name}: original AST item/call provenance"
            );
            assert!(start < end && end <= s.len());
            let prefix = &s[..start];
            assert_eq!(
                site["line"],
                json!(prefix.bytes().filter(|&b| b == b'\n').count() + 1)
            );
            assert_eq!(
                site["column"],
                json!(prefix.rsplit('\n').next().unwrap().chars().count() + 1)
            );
            if m["result"] == "detected" && *i == 1 {
                assert!(
                    m["assigned_changes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|c| c["direction"] == "worsened" && c["field"] == "verdict")
                );
            }
            if m["result"] == "not_run" {
                assert!(m["report"].is_null());
                assert_eq!(m["assigned_changes"], json!([]));
                assert_eq!(m["other_changes"], json!([]));
            } else {
                assert_eq!(
                    m["report"]["model"]["sha256"], v["original"]["model"]["sha256"],
                    "{name}: mutant retains original source provenance"
                );
            }
        }
    }
    for (i, reason) in [
        (5, "rule_version_unavailable"),
        (7, "quality_denominator_unavailable"),
    ] {
        assert_eq!(v["mutations"][i]["status"], "unsupported");
        assert_eq!(v["mutations"][i]["reason"], reason);
        assert_eq!(v["mutations"][i]["mutants"], json!([]));
    }
    if e["expected"]["run"]["completion"] == "blocked" {
        response_boundary::assert_blocked(&v["mutations"]);
    } else {
        response_boundary::assert_not_applicable(&source(name), &v["mutations"]);
    }
    let unchecked = v["unchecked"].as_array().unwrap();
    assert_eq!(
        &unchecked[unchecked.len() - 4..],
        &json!([
            "authority_dynamic_expiry",
            "authority_expiry_unnamed",
            "summary_reuse_scope",
            "quality_changes"
        ])
        .as_array()
        .unwrap()[..]
    );
    let assumptions = v["assumptions"].as_array().unwrap();
    assert_eq!(assumptions.len(), 9);
    assert_eq!(assumptions[7]["field"], "mutation.authority_scope");
    assert_eq!(assumptions[7]["value"], "expires_zero_on_basis_authorities");
    assert_eq!(assumptions[8]["field"], "mutation.summary_scope");
    assert_eq!(assumptions[8]["value"], "duplicate_producer_one_consumer");
    if let Some(old) = e["old_sites"].as_u64() {
        let old_ms: Vec<_> = [0, 2, 8, 9]
            .into_iter()
            .flat_map(|i| v["mutations"][i]["mutants"].as_array().unwrap())
            .collect();
        assert_eq!(old_ms.len(), old as usize);
        if name.starts_with("mut2-limit") {
            let pending = old_ms.iter().filter(|m| m["result"] == "not_run").count();
            assert_eq!(pending, old.saturating_sub(128) as usize);
            let authority_done = name == "mut2-limit" || name.starts_with("mut2-limit-summary");
            assert_eq!(
                v["checked"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("authority_expiry")),
                authority_done
            );
            assert_eq!(
                unchecked.contains(&json!("authority_expiry")),
                !authority_done
            );
        }
    }
    if name == "mut2-expiry-cases" {
        let cs = v["mutations"][1]["mutants"][0]["assigned_changes"]
            .as_array()
            .unwrap();
        assert!(!cs.is_empty());
        assert!(cs.iter().all(|c| c["check"] != "properties"
            || c["field"] != "verdict"
            || c["before"] != "violated"));
        assert!(cs.iter().any(|c| c["direction"] == "worsened"));
    }
    for (end, times) in e["terminals"].as_object().unwrap() {
        let terminal = |r: &Value| {
            r["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["end"] == *end)
                .unwrap()
                .clone()
        };
        let before = terminal(&v["original"]);
        let after = terminal(&v["mutations"][4]["mutants"][0]["report"]);
        for (r, elapsed, work) in [
            (&before, &times[0], &times[2]),
            (&after, &times[1], &times[3]),
        ] {
            assert_eq!(&r["elapsed"]["min"], elapsed);
            assert_eq!(&r["elapsed"]["max"], elapsed);
            assert_eq!(&r["work"]["min"], work);
            assert_eq!(&r["work"]["max"], work);
        }
    }
}
