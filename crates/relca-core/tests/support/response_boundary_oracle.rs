// ADR0056 boundary oracle: inspect declarations and lexical calls only. Never
// call the production mutation site enumerator, generators or comparison code.
use relca_core::{ast, syntax};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[allow(dead_code)]
pub fn expected(source: &str) -> (Value, Vec<Value>) {
    let model = syntax::parse(source).expect("old-language boundary source must parse");
    let commits: BTreeSet<_> = model
        .items
        .iter()
        .filter_map(|i| match i {
            ast::Item::Actor(a) => Some(a),
            _ => None,
        })
        .flat_map(|a| {
            a.items.iter().filter_map(move |i| match i {
                ast::ActorItem::Capability(c)
                    if c.items
                        .iter()
                        .any(|i| matches!(i.value, ast::CapItem::Commits)) =>
                {
                    Some((a.name.value.clone(), c.name.value.clone()))
                }
                _ => None,
            })
        })
        .collect();
    let processes: Vec<_> = model
        .items
        .iter()
        .filter_map(|i| match i {
            ast::Item::Process(p) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(
        processes.len(),
        1,
        "this oracle covers only an unambiguously selected old process"
    );
    let mut sites = Vec::new();
    fn call(
        c: &ast::Call,
        fork: bool,
        process: &str,
        commits: &BTreeSet<(String, String)>,
        sites: &mut Vec<Value>,
    ) {
        assert!(
            c.retry.is_none() && c.response_loss.is_none(),
            "new declarations require the separate Q1 hand oracle"
        );
        if commits.contains(&(c.actor.value.clone(), c.capability.value.clone())) {
            sites.push(json!({"reason":if fork {"fork_branch"}else{"undeclared_site"},"binding":c.binding.value,"actor":c.actor.value,"capability":c.capability.value,"process":process,"start":c.span.start,"end":c.span.end}));
        }
    }
    fn walk(
        b: &ast::Block,
        process: &str,
        commits: &BTreeSet<(String, String)>,
        sites: &mut Vec<Value>,
    ) {
        for s in b {
            match &s.value {
                ast::Stmt::WaitUntil(_) => {
                    panic!("response boundary oracle does not handle condition waits")
                }
                ast::Stmt::Call(c) => call(c, false, process, commits, sites),
                ast::Stmt::Fork { branches, .. } => {
                    for c in branches {
                        call(c, true, process, commits, sites)
                    }
                }
                ast::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    walk(then_body, process, commits, sites);
                    walk(else_body, process, commits, sites)
                }
                ast::Stmt::Wait(_) | ast::Stmt::End(_) => {}
            }
        }
    }
    walk(
        &processes[0].body,
        &processes[0].name.value,
        &commits,
        &mut sites,
    );
    let mut counts = BTreeMap::<&str, usize>::new();
    for site in &sites {
        *counts.entry(site["reason"].as_str().unwrap()).or_default() += 1;
    }
    (json!(counts), sites)
}

#[allow(dead_code)]
pub fn row(source: &str, kind: &str) -> Value {
    let (counts, sites) = expected(source);
    json!({"kind":kind,"status":"not_applicable","reason":if sites.is_empty(){"no_matching_site"}else{"no_supported_site"},"mutants":[],"skipped_counts":counts})
}

#[allow(dead_code)]
pub fn assert_not_applicable(source: &str, rows: &Value) {
    assert_no_declared_sites(rows);
    let (counts, sites) = expected(source);
    for index in [3, 6] {
        let r = &rows[index];
        assert_eq!(r["status"], "not_applicable");
        assert_eq!(
            r["reason"],
            if sites.is_empty() {
                "no_matching_site"
            } else {
                "no_supported_site"
            }
        );
        assert_eq!(r["mutants"], json!([]));
        assert_eq!(r["skipped_counts"], counts);
        let actual = r["skipped_sites"].as_array().unwrap();
        assert_eq!(actual.len(), sites.len());
        let mut observed = BTreeMap::<&str, usize>::new();
        for (a, e) in actual.iter().zip(&sites) {
            assert_eq!(a["status"], "not_applicable");
            assert_eq!(a["reason"], e["reason"]);
            for key in ["process", "actor", "capability"] {
                assert_eq!(a["site"][key], e[key]);
            }
            assert_eq!(a["site"]["affected_bindings"], json!([e["binding"]]));
            assert_eq!(
                a["site"]["span"],
                json!({"start":e["start"],"end":e["end"]})
            );
            *observed.entry(a["reason"].as_str().unwrap()).or_default() += 1;
        }
        assert_eq!(json!(observed), r["skipped_counts"]);
    }
}

// For closed pre-Q1 input families whose constructors contain no channel or
// retry declarations. This checks only the new boundary metadata, not an old
// numeric oracle or an inferred number of execution instances.
#[allow(dead_code)]
pub fn assert_no_declared_sites(rows: &Value) {
    for index in [3, 6] {
        let r = &rows[index];
        assert_eq!(r["status"], "not_applicable");
        assert_eq!(r["mutants"], json!([]));
        let skipped = r["skipped_sites"].as_array().unwrap();
        assert_eq!(
            r["reason"],
            if skipped.is_empty() {
                "no_matching_site"
            } else {
                "no_supported_site"
            }
        );
        let mut counts = BTreeMap::<&str, usize>::new();
        for s in skipped {
            let reason = s["reason"].as_str().unwrap();
            assert!(["undeclared_site", "fork_branch"].contains(&reason));
            assert_eq!(s["status"], "not_applicable");
            *counts.entry(reason).or_default() += 1;
        }
        assert_eq!(json!(counts), r["skipped_counts"]);
    }
}

#[allow(dead_code)]
pub fn assert_blocked(rows: &Value) {
    for index in [3, 6] {
        let r = &rows[index];
        assert_eq!(r["status"], "blocked");
        assert_eq!(r["reason"], "original_not_decidable");
        assert_eq!(r["mutants"], json!([]));
        assert!(r.get("skipped_counts").is_none());
        assert!(r.get("skipped_sites").is_none());
    }
}
