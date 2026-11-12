//! Separate closed C7 population; never folded into a frozen earlier census.
#![allow(dead_code)]
pub const NAMES: &[&str] = &include!("quantile_names.rs");
pub fn fixture(name: &str) -> bool {
    NAMES.contains(&name)
}
