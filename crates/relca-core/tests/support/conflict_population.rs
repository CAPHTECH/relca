#![allow(dead_code)]
pub const NAMES: &[&str] = include!("conflict_wire_names.rs");
pub fn fixture(name: &str) -> bool {
    let found = NAMES.contains(&name);
    assert!(
        !name.starts_with("cf-") || found,
        "unclassified conflict fixture {name}"
    );
    found
}
