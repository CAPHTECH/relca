//! Closed exclusion from historical populations; unknown policy names fail.
pub fn fixture(name: &str) -> bool {
    if !name.starts_with("pol-") {
        return false;
    }
    let names: Vec<String> = serde_json::from_str(include_str!("policy_names.json")).unwrap();
    assert!(
        names.iter().any(|n| n == name),
        "unclassified policy fixture {name}"
    );
    true
}
