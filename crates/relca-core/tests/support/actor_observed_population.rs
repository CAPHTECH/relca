//! Closed ADR0032 population; unknown aobs names may not become exemptions.
pub fn fixture(name: &str) -> bool {
    let names: Vec<String> =
        serde_json::from_str(include_str!("actor_observed_names.json")).unwrap();
    let found = names.iter().any(|n| n == name);
    assert!(
        !name.starts_with("aobs-") || found,
        "unclassified Actor observed fixture {name}"
    );
    found
}
