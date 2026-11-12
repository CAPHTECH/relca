#![allow(dead_code)]
//! Hand-authored ADR0040 inventory and recursive partial oracle (no production DTO defaults).
use serde_json::Value;
use std::path::PathBuf;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn names() -> Vec<String> {
    serde_json::from_str(include_str!("effect_standing_names.json")).unwrap()
}
pub fn pairs() -> Value {
    serde_json::from_str(include_str!("effect_standing_pairs.json")).unwrap()
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(root().join(name).join("expected.json")).unwrap())
        .unwrap()
}
pub fn invocation(name: &str) -> Option<Value> {
    let p = root().join(name).join("invocation.json");
    p.exists()
        .then(|| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap())
}
pub fn subset(e: &Value, a: &Value, path: &str) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                if k == "absent_diagnostics" {
                    for code in v.as_array().unwrap() {
                        assert!(
                            !a["diagnostics"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|d| d["code"] == *code),
                            "{path}: absent {code}"
                        );
                    }
                } else {
                    subset(
                        v,
                        a.get(k).unwrap_or_else(|| panic!("{path}: missing {k}")),
                        &format!("{path}/{k}"),
                    );
                }
            }
        }
        Value::Array(es) => {
            let actual = a
                .as_array()
                .unwrap_or_else(|| panic!("{path}: array expected, got {a}"));
            assert_eq!(es.len(), actual.len(), "{path}: inventory");
            if path.ends_with("/alternatives") {
                // Each tuple is unique; compare possible outcomes, not DFS order.
                fn contains(e: &Value, a: &Value) -> bool {
                    match e {
                        Value::Object(o) => o
                            .iter()
                            .all(|(k, v)| a.get(k).is_some_and(|x| contains(v, x))),
                        _ => e == a,
                    }
                }
                let mut used = std::collections::BTreeSet::new();
                for e in es {
                    let i = actual
                        .iter()
                        .enumerate()
                        .find(|(i, a)| !used.contains(i) && contains(e, a))
                        .map(|(i, _)| i)
                        .unwrap_or_else(|| panic!("{path}: missing alternative {e} in {a}"));
                    used.insert(i);
                }
            } else {
                for (i, (e, a)) in es.iter().zip(actual).enumerate() {
                    subset(e, a, &format!("{path}/{i}"));
                }
            }
        }
        _ => assert_eq!(e, a, "{path}"),
    }
}
pub fn replace(src: &str, old: &str, new: &str) -> String {
    assert_eq!(src.matches(old).count(), 1, "unique edit {old}");
    let changed = src.replacen(old, new, 1);
    assert_ne!(src, changed);
    changed
}

/// Remove the *new feature*, not syntax failures elsewhere, for old parser preflight.
pub fn skeleton(src: &str) -> String {
    let mut s = src.to_owned();
    while let Some(start) = s.find("standing over [") {
        let basis = start + s[start..].find("basis \"").unwrap() + 7;
        let end = basis + s[basis..].find('"').unwrap() + 1;
        s.replace_range(start..end, "");
    }
    s.lines()
        .filter(|l| !l.trim_start().starts_with("contract Controls"))
        .collect::<Vec<_>>()
        .join("\n")
}
