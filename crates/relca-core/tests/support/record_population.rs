#![allow(dead_code)]
/// Closed ADR0038 population. Unknown rec-* names are errors, never silently
/// excluded from an older compatibility or census denominator.
/// Membership is by the closed name table, not by the prefix.
pub fn fixture(name: &str) -> bool {
    let names: Vec<String> = serde_json::from_str(include_str!("record_names.json")).unwrap();
    let member = names.iter().any(|n| n == name);
    assert!(
        !name.starts_with("rec-") || member,
        "unclassified record fixture {name}"
    );
    member
}
