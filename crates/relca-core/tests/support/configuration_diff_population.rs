//! ADR 0031: nineteen pairs, never absorbed into historical populations.
pub const NAMES: [&str; 38] = [
    "diff2-time",
    "diff2-time.broken-sequential",
    "diff2-resource",
    "diff2-resource.broken-no-call",
    "diff2-meaning",
    "diff2-meaning.broken-same-end",
    "diff2-authority",
    "diff2-authority.broken-no-call",
    "diff2-visibility",
    "diff2-visibility.broken-same-reader",
    "diff2-contest",
    "diff2-contest.broken-same-end",
    "diff2-contest-reader",
    "diff2-contest-reader.broken-no-route-read",
    "diff2-preserve",
    "diff2-preserve.broken-restored-call",
    "diff2-preserve-union",
    "diff2-preserve-union.broken-other-selector",
    "diff2-override",
    "diff2-override.broken-bypass",
    "diff2-override-person",
    "diff2-override-person.broken-other-person",
    "diff2-maintain",
    "diff2-maintain.broken-conflict",
    "diff2-surplus",
    "diff2-surplus.broken-intent-only",
    "diff2-warning-intent",
    "diff2-warning-intent.broken-no-change",
    "diff2-warning-annotation",
    "diff2-warning-annotation.broken-no-preserve",
    "diff2-capacity",
    "diff2-capacity.broken-one-slot",
    "diff2-mismatch",
    "diff2-mismatch.broken-other-cut",
    "diff2-dead-end",
    "diff2-dead-end.broken-same-end",
    "diff2-invoice",
    "diff2-invoice.broken-missing-resource-claim",
];

#[allow(dead_code)]
pub fn fixture(name: &str) -> bool {
    let yes = NAMES.contains(&name);
    assert!(
        !name.starts_with("diff2-") || yes,
        "unknown diff2 fixture {name}"
    );
    yes
}

/// W1/W3 from the calc.md sets and declaration coverage, not CLI output.
#[allow(dead_code)]
pub fn hand_warnings(name: &str) -> (usize, usize) {
    let index = NAMES
        .iter()
        .position(|n| *n == name)
        .expect("closed diff2 population");
    [
        (3, 3),
        (2, 2),
        (4, 4),
        (0, 0),
        (1, 1),
        (0, 0),
        (2, 2),
        (0, 0),
        (3, 3),
        (2, 2),
        (2, 2),
        (0, 0),
        (6, 6),
        (6, 6),
        (1, 0),
        (0, 0),
        (1, 0),
        (1, 0),
        (2, 2),
        (2, 2),
        (2, 2),
        (2, 2),
        (3, 3),
        (3, 3),
        (0, 0),
        (0, 0),
        (0, 0),
        (1, 0),
        (0, 0),
        (0, 1),
        (2, 2),
        (2, 2),
        (2, 2),
        (0, 0),
        (1, 1),
        (0, 0),
        // ADR0051: entry's producer transfer adds W1; global preserve keeps W3=0.
        (4, 0),
        (5, 0),
    ][index]
}
