//! Internal-seam acceptance with independently fixed expectations.
//! The integration target compiles this file but cannot call the private driver.
//! A cfg(test) module in report.rs can pass simulate_using with reference::simulate.
#![allow(dead_code)]
use super::{hand, input};
use serde_json::Value;

/// `run(source)` must return the actual full report produced by the production
/// report collector using the independent reference SimulationOutcome, not an
/// engine report, a hand-built table, or a second test-side reducer.
pub fn assert_reference_aggregate(mut run: impl FnMut(&str) -> Value) {
    for (name, in_route) in [("range-before-target", false), ("range-in-route", true)] {
        hand::range_report(&run(&input::source(name)), in_route);
    }
}
