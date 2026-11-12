//! Closed scn-* membership, separate from every historical population.
#[allow(dead_code)]
pub const NAMES: &[&str] = include!("scenario_names.rs");

#[allow(dead_code)]
pub fn fixture(name: &str) -> bool {
    assert!(
        !name.starts_with("scn-") || NAMES.contains(&name),
        "unclassified scenario fixture {name}"
    );
    NAMES.contains(&name)
}
