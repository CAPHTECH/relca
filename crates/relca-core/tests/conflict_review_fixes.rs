//! ADR 0041 boundary review M1 and m2: the presence of `version_check` must not
//! hide a diagnostic, and an unrelated U-SYN-0 inside the same Capability must
//! not hide the version_check checks. Each expectation is a hand derivation; the
//! legacy rows (without version_check) were also run on the pre-change binary
//! (56d4d0b), which gives the same codes, messages and exit code.
#[path = "support/actor_observed_acceptance.rs"]
mod h;
use relca_core::{Verdict, diag, lower, report, syntax};

const CHECK: &str = "      version_check [holder] else conflict\n";
const OTHER: &str = "writes to another Actor's state are unsupported";

fn errors(s: &str) -> Vec<(String, String)> {
    let ds = syntax::parse(s)
        .and_then(|a| lower::lower(&a))
        .expect_err("static failure, not deferred to the engine");
    let mut v: Vec<_> = ds
        .iter()
        .filter(|d| d.severity == diag::Severity::Error)
        .map(|d| (d.code.clone(), d.message.clone()))
        .collect();
    v.sort();
    v
}

fn rows(xs: &[(&str, &str)]) -> Vec<(String, String)> {
    xs.iter()
        .map(|(c, m)| ((*c).to_owned(), (*m).to_owned()))
        .collect()
}

fn verdict(s: &str) -> Verdict {
    report::check(s).verdict
}

/// M1. finish writes Other.x instead of holder, so holder (declared in writes,
/// not assigned by finish) remains a result writer: R into H is E-TYPE-0. The
/// other Actor's write is U-SYN-0. E- outranks U-: invalid, exit 3. The legacy
/// path (no version_check) gives the same two rows.
#[test]
fn other_actor_finish_keeps_result_type_error_with_or_without_version_check() {
    let with = h::source("cf-s19-other-finish.broken-owner");
    let without = h::change(&with, CHECK, "");
    let want = rows(&[
        ("E-TYPE-0", "result writer has different base type"),
        ("U-SYN-0", OTHER),
    ]);
    for s in [&with, &without] {
        assert_eq!(errors(s), want, "{s}");
        assert_eq!(verdict(s), Verdict::Invalid, "{s}");
        assert_eq!(verdict(s).exit_code(), 3);
    }
}

/// m2 (direct consequence). writes is [Other.x, claims], so holder, the
/// version_check target, is not among the declared writes. The U-SYN-0 of the
/// other Actor's write does not hide that U-SEM-0. The legacy path has no
/// version_check, hence only U-SYN-0. Both are U-: unsupported, exit 4.
#[test]
fn other_actor_writes_does_not_hide_unwritten_version_check_target() {
    let with = h::source("cf-s19-other-writes.broken-owner");
    let without = h::change(&with, CHECK, "");
    assert_eq!(
        errors(&with),
        rows(&[
            ("U-SEM-0", "version_check target is not written"),
            ("U-SYN-0", OTHER),
        ])
    );
    assert_eq!(errors(&without), rows(&[("U-SYN-0", OTHER)]));
    for s in [&with, &without] {
        assert_eq!(verdict(s), Verdict::Unsupported, "{s}");
        assert_eq!(verdict(s).exit_code(), 4);
    }
}

/// m2. The S08 fault (else value overlaps the result committed) inside a
/// Capability that also writes another Actor's state at start: both U-SYN-0
/// and the overlap U-SEM-0, unsupported, exit 4. `retry 1` (cf-s15-retry) is a
/// parse error, so lowering never runs and it cannot carry this check.
#[test]
fn other_actor_start_does_not_hide_version_check_overlap() {
    let s = h::change(
        &h::source("cf-s19-other-start.broken-owner"),
        "else conflict",
        "else committed",
    );
    assert_eq!(
        errors(&s),
        rows(&[
            ("U-SEM-0", "version_check else value overlaps a result"),
            ("U-SYN-0", OTHER),
        ])
    );
    assert_eq!(verdict(&s), Verdict::Unsupported);
    assert_eq!(verdict(&s).exit_code(), 4);
}

/// m2. The S09 fault (start writes the checked state) next to the other
/// Actor's start write: both U-SYN-0 and the start U-SEM-0.
#[test]
fn other_actor_start_does_not_hide_version_check_start_write() {
    let s = h::change(
        &h::source("cf-s19-other-start.broken-owner"),
        "start {Other.x=taken}",
        "start {Other.x=taken; holder=taken}",
    );
    assert_eq!(
        errors(&s),
        rows(&[
            (
                "U-SEM-0",
                "version_check with start writes to checked state is unsupported"
            ),
            ("U-SYN-0", OTHER),
        ])
    );
    assert_eq!(verdict(&s), Verdict::Unsupported);
}
