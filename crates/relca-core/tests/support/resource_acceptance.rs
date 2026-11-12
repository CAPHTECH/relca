//! Hand resources population. No production-output collection.
#![allow(dead_code)]
use serde_json::Value;
use std::path::PathBuf;
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn pairs() -> Value {
    serde_json::from_str(include_str!("resource_pairs.json")).unwrap()
}
pub fn names() -> Vec<String> {
    pairs()
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| {
            [
                p["good"].as_str().unwrap().to_owned(),
                p["bad"].as_str().unwrap().to_owned(),
            ]
        })
        .collect()
}
pub fn fixture(name: &str) -> bool {
    if !name.starts_with("rsrc-") {
        return false;
    }
    assert!(
        names().iter().any(|n| n == name),
        "unclassified resource fixture {name}"
    );
    true
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(root().join(name).join("expected.json")).unwrap())
        .unwrap()
}
pub fn edit(s: &str, from: &str, to: &str) -> String {
    assert!(s.contains(from), "edit absent: {from}");
    let t = s.replace(from, to);
    assert_ne!(s, t, "empty control edit");
    t
}
pub fn static_case(name: &str) -> bool {
    matches!(
        name,
        "rsrc-members.broken-members"
            | "rsrc-actor-use.broken-actor-use"
            | "rsrc-collision.broken-same-name"
            | "rsrc-zero-amount.broken-zero"
            | "rsrc-unknown.broken-unknown"
            | "rsrc-duplicate.broken-duplicate"
            | "rsrc-second-use.broken-second-use"
            | "rsrc-actor-capacity.broken-no-capacity"
            | "rsrc-all-start.broken-no-proof"
    )
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                subset(v, a.get(k).unwrap_or_else(|| panic!("missing {k}: {a}")));
            }
        }
        Value::Array(es) => {
            let actual = a
                .as_array()
                .unwrap_or_else(|| panic!("expected array: {a}"));
            assert_eq!(es.len(), actual.len(), "closed hand array: {e} vs {a}");
            let key = [
                "resource",
                "id",
                "case_id",
                "binding",
                "actor",
                "capability",
                "end",
            ]
            .into_iter()
            .find(|k| !es.is_empty() && es.iter().all(|v| v.get(*k).is_some()));
            if let Some(k) = key {
                for h in es {
                    let found: Vec<_> = actual.iter().filter(|r| r[k] == h[k]).collect();
                    assert_eq!(found.len(), 1, "unique {k} in {a}");
                    subset(h, found[0]);
                }
            } else {
                for (x, y) in es.iter().zip(actual) {
                    subset(x, y);
                }
            }
        }
        _ => assert_eq!(e, a, "hand oracle"),
    }
}
/// Remove only resource syntax. For static cases preflight the positive control.
/// This projection checks the pre-existing DSL, not resource semantics.
pub fn old_skeleton(s: &str) -> String {
    fn outside(s: &str, needle: &str) -> Option<usize> {
        let mut quoted = false;
        let mut escaped = false;
        for (i, c) in s.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if quoted && c == '\\' {
                escaped = true;
                continue;
            }
            if c == '"' {
                quoted = !quoted;
            }
            if !quoted && s[i..].starts_with(needle) {
                return Some(i);
            }
        }
        None
    }
    let mut t = s.to_owned();
    while let Some(start) = outside(&t, "resource ") {
        let after = &t[start + 9..];
        let mut words = after.split_whitespace();
        let _name = words.next().unwrap();
        assert_eq!(words.next(), Some("capacity"));
        let n = words.next().unwrap().trim_end_matches(';');
        let end = start + 9 + after.find(n).unwrap() + n.len();
        t.replace_range(start..end, "");
    }
    while let Some(start) = outside(&t, "uses ") {
        let end = start + t[start..].find(';').unwrap() + 1;
        t.replace_range(start..end, "");
    }
    t
}
