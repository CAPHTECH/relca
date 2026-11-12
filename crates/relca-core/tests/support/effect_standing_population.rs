#![allow(dead_code)]
pub fn fixture(name: &str) -> bool {
    let names: Vec<String> =
        serde_json::from_str(include_str!("effect_standing_names.json")).unwrap();
    let member = names.iter().any(|n| n == name);
    assert!(
        !name.starts_with("sov-") || member,
        "unclassified effect Standing fixture {name}"
    );
    member
}
