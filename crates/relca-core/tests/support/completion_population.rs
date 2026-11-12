//! ADR0025 is a separate, closed 24-dir population, never old148/old124.
#![allow(dead_code)]
pub const FIXTURES: [&str; 24] = [
    "done-arrivals",
    "done-arrivals.broken-arrival-after-clock",
    "done-branches",
    "done-branches.broken-enable-then",
    "done-call-kind",
    "done-call-kind.broken-call-unused",
    "done-clock",
    "done-clock.broken-short-clock",
    "done-end-vs-close",
    "done-end-vs-close.broken-pre-end-delay",
    "done-nondet",
    "done-nondet.broken-nondet-option",
    "done-people",
    "done-people.broken-remove-executor",
    "done-probability",
    "done-probability.broken-block-failure",
    "done-recovery",
    "done-recovery.broken-zero-capacity",
    "done-support",
    "done-support.broken-positive-support",
    "done-vacuity",
    "done-vacuity.broken-enable-registration",
    "done-within",
    "done-within.broken-tight-deadline",
];
pub fn fixture(name: &str) -> bool {
    if !name.starts_with("done-") {
        return false;
    }
    assert!(
        FIXTURES.contains(&name),
        "unclassified completion fixture {name}"
    );
    true
}
