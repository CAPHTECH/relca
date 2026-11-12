//! ADR 0028 is a closed 30-dir population, not part of historical denominators.
pub const NAMES: [&str; 30] = [
    "resp2-atomic",
    "resp2-atomic.broken-consequent",
    "resp2-bool",
    "resp2-bool.broken-enum",
    "resp2-classification",
    "resp2-classification.broken-operator",
    "resp2-collision",
    "resp2-collision.broken-capability",
    "resp2-design",
    "resp2-design.broken-unknown",
    "resp2-direction",
    "resp2-direction.broken-reversed",
    "resp2-duration",
    "resp2-duration.broken-short",
    "resp2-equivalence",
    "resp2-equivalence.broken-wrong-antecedent",
    "resp2-maintenance",
    "resp2-maintenance.broken-indirect-only",
    "resp2-refusal",
    "resp2-refusal.broken-bypass",
    "resp2-relay",
    "resp2-relay.broken-short",
    "resp2-unchecked",
    "resp2-unchecked.broken-within",
    "resp2-unknown-origin",
    "resp2-unknown-origin.broken-no-source",
    "resp2-unknown-time",
    "resp2-unknown-time.broken-no-direct",
    "resp2-within",
    "resp2-within.broken-short",
];
#[allow(dead_code)]
pub fn fixture(name: &str) -> bool {
    let found = NAMES.contains(&name);
    assert!(
        !name.starts_with("resp2-") || found,
        "unclassified responsibility extension fixture {name}"
    );
    found
}
