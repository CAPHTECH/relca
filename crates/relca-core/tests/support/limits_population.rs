//! Exact new population; no prefix-only exclusion of unknown future fixtures.
#![allow(dead_code)]
pub fn names() -> Vec<String> {
    serde_json::from_str(include_str!("limits_names.json")).unwrap()
}
pub fn fixture(name: &str) -> bool {
    if !name.starts_with("lim-") {
        return false;
    }
    assert!(
        names().iter().any(|n| n == name),
        "unclassified limits fixture {name}"
    );
    true
}
