#![allow(dead_code)]
pub fn fixture(name: &str) -> bool {
    let names: Vec<String> = serde_json::from_str(include_str!("caseset_names.json")).unwrap();
    let found = names.iter().any(|n| n == name);
    assert!(
        !name.starts_with("aset-") || found,
        "unclassified caseset fixture {name}"
    );
    found
}
