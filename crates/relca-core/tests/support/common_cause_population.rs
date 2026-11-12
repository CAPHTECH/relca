//! Closed ADR0024 population. Historical differential denominators do not grow.
pub const NAMES: [&str; 24] = [
    "lin-basis",
    "lin-basis.broken-no-basis",
    "lin-case-latent",
    "lin-case-latent.broken-actor-local",
    "lin-correlation",
    "lin-correlation.broken-unrelated-given",
    "lin-five-kernels",
    "lin-five-kernels.broken-given-only-read",
    "lin-invoice",
    "lin-invoice.broken-po-evidence",
    "lin-no-error-writer",
    "lin-no-error-writer.broken-connect-error-writer",
    "lin-numeric",
    "lin-numeric.broken-zero-detection",
    "lin-same-actor",
    "lin-same-actor.broken-read-removed",
    "lin-same-source-different-target",
    "lin-same-source-different-target.broken-use-same-target",
    "lin-shared-kernel",
    "lin-shared-kernel.broken-drop-call",
    "lin-single",
    "lin-single.broken-second-call",
    "lin-tacit",
    "lin-tacit.broken-unused-tacit",
];
#[allow(dead_code)]
pub fn fixture(name: &str) -> bool {
    let member = NAMES.contains(&name);
    assert!(
        !name.starts_with("lin-") || member,
        "unclassified ADR0024 fixture {name}"
    );
    member
}
