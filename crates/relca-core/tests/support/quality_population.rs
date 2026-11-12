#![allow(dead_code)]
pub const NAMES: &[&str] = &include!("quality_names.rs");
pub fn fixture(name: &str) -> bool {
    NAMES.contains(&name)
}
