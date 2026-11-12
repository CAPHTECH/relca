//! Shared hand inputs, never a solver or a collector of production expectations.
#![allow(dead_code)]
use serde_json::Value;
use std::path::PathBuf;
pub const NAMES: [&str; 32] = [
    "req-after-queue",
    "req-after-queue.broken-close",
    "req-agency",
    "req-agency.broken-remove-guard",
    "req-bypass",
    "req-bypass.broken-conjunction",
    "req-cancel-immediate",
    "req-cancel-immediate.broken-cooperative",
    "req-explicit-else",
    "req-explicit-else.broken-no-else",
    "req-kernel",
    "req-kernel.broken-enable",
    "req-no-capacity",
    "req-no-capacity.broken-zero",
    "req-observed",
    "req-observed.broken-latent",
    "req-same-time",
    "req-same-time.broken-no-close",
    "req-start",
    "req-start-kept",
    "req-start-kept.broken-forbidden",
    "req-start-siblings",
    "req-start-siblings.broken-same-target",
    "req-start.broken-false",
    "req-wait-admission",
    "req-wait-admission.broken-small-queue",
    "req-wait-arrivals",
    "req-wait-arrivals.broken-no-wait",
    "req-wait-clock",
    "req-wait-clock.broken-short-clock",
    "req-wait-gap",
    "req-wait-gap.broken-zero",
];
pub fn fixture(name: &str) -> bool {
    if !name.starts_with("req-") {
        return false;
    }
    assert!(
        NAMES.contains(&name),
        "unclassified requires fixture {name}"
    );
    true
}
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(root().join(name).join("expected.json")).unwrap())
        .unwrap()
}
pub fn static_case(name: &str) -> bool {
    matches!(
        name,
        "req-explicit-else.broken-no-else"
            | "req-observed.broken-latent"
            | "req-start-siblings.broken-same-target"
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
            // Match declarative tables by identity, never rely on DFS end order.
            let key = [
                "id",
                "case_id",
                "binding",
                "actor",
                "capability",
                "call",
                "code",
                "end",
            ]
            .into_iter()
            .find(|k| !es.is_empty() && es.iter().all(|v| v.get(*k).is_some()));
            if key != Some("code") {
                assert_eq!(es.len(), actual.len(), "closed hand inventory: {e} vs {a}");
            }
            if let Some(k) = key {
                for h in es {
                    let found: Vec<_> = actual.iter().filter(|r| r[k] == h[k]).collect();
                    assert_eq!(found.len(), 1, "unique {k}={} in {a}", h[k]);
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
/// Validate every old-language name, bound, type and parameter without requiring
/// the parser under development to understand any ADR0026 clause.
pub fn old_skeleton(s: &str) -> String {
    let mut s = s.to_owned();
    while let Some(start) = s.find("requires ") {
        let end = s[start..].find([';', '}']).unwrap() + start;
        s.replace_range(start..end, "");
    }
    while let Some(start) = s.find("start {") {
        let end = s[start..].find('}').unwrap() + start + 1;
        s.replace_range(start..end, "");
    }
    s = s.replace("cancel immediate", "cancel cooperative ack_within 1min");
    while let Some(start) = s.find("wait ") {
        let end = s[start..].find("min").unwrap() + start + 3;
        s.replace_range(start..end, "");
    }
    s
}
