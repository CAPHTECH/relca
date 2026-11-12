//! Agency integration with records, limits, Standing, conflicts and scenarios.
//!
//! Obligation (c) of the integration: the order of the Agency `unchecked`
//! markers. ADR 0042's integration ruling: in a report with obligation rows,
//! report.rs `apply_outcome` appends `obligation_debtor` once, last, after
//! every marker lower produced (the pre-B5 and pre-C5 order). C5 (plan C04)
//! makes lower append `responsibility_people_undeclared` when the model
//! declares no people; it is a lower marker, so it precedes
//! `obligation_debtor`. Boundary-review m7 of C5 said the input could not be
//! built; this people-less, two-case, named-response model builds it.
//!
//! The expected marker list is written by hand from lower.rs, report.rs and
//! the C5 plan (C03: `executors` leaves when effects are derived; C04: the
//! people marker). The pre-C5 list was read once from the integrated pre-C5
//! binary (a5b3fa6, sha256 2efc5881f3b21885…) and is fixed here as
//! data, never learned from the post-C5 binary.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(s: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = root().join("target").join(format!(
        "agency-second-half-integration-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{}.relca", N.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&p, s).unwrap();
    p
}

fn run(args: &[&str], src: &str) -> (i32, Vec<u8>, Vec<u8>) {
    let path = scratch(src);
    let mut all = vec![args[0], path.to_str().unwrap()];
    all.extend(&args[1..]);
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(all)
        .output()
        .unwrap();
    (o.status.code().unwrap(), o.stdout, o.stderr)
}

const PEOPLELESS: &str = r#"model M version "0.2" {
type T = enum { ok }
actor A {
  capacity 1
  capability run() -> T {
    duration fixed 5min
    result nondet { ok }
  }
}
process Flow { r = A.run(); end done }
cut C { include [A] }
policy fifo tie_by case_id
arrivals fixed [0min, 2min]
bounds { cases 2; live_tasks 1; clock 30min }
contract K { response delivery: case.received -> one_of(end.done) by 9min after case.received otherwise unresolved }
}
"#;

fn markers(v: &Value) -> Vec<String> {
    v["agency"]["unchecked"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}

fn assert_marker_refused(v: &Value) {
    let e = fixture_match::compare_report(&json!({}), v).unwrap_err();
    assert!(e.contains("Agency unchecked marker"), "{e}");
}

#[test]
fn c_people_marker_precedes_the_trailing_obligation_debtor() {
    let (code, out, err) = run(&["check"], PEOPLELESS);
    assert_eq!(code, 0);
    assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["obligations"].as_array().unwrap().len(), 2);
    assert_eq!(v["agency"]["viewpoints"]["rules"], "agency-viewpoints-p1-2");
    assert_eq!(
        markers(&v),
        [
            "W-RESP-1",
            "contest_path",
            "fallback_path",
            "via_model",
            "multi_case_interference",
            "responsibility_people_undeclared",
            "obligation_debtor",
        ],
        "lower's markers, then report.rs's obligation_debtor once and last"
    );
    // The pre-C5 list (integrated pre-C5 binary, read once): removing only the
    // C5 people marker and restoring only the C03 executors word gives it back.
    let pre_c5 = [
        "W-RESP-1",
        "contest_path",
        "fallback_path",
        "executors",
        "via_model",
        "multi_case_interference",
        "obligation_debtor",
    ];
    let mut projected = markers(&v);
    projected.retain(|x| x != "responsibility_people_undeclared");
    projected.insert(3, "executors".into());
    assert_eq!(projected, pre_c5);
    fixture_match::compare_report(&json!({}), &v)
        .unwrap_or_else(|e| panic!("the wire matcher must accept the rule's order: {e}"));
}

#[test]
fn c_matcher_refuses_the_people_marker_after_obligation_debtor_or_twice() {
    let (_, out, _) = run(&["check"], PEOPLELESS);
    let v: Value = serde_json::from_slice(&out).unwrap();
    let n = markers(&v).len();
    // [.., obligation_debtor, responsibility_people_undeclared]: refused.
    let mut late = v.clone();
    late["agency"]["unchecked"]
        .as_array_mut()
        .unwrap()
        .swap(n - 2, n - 1);
    assert_eq!(late["agency"]["unchecked"][n - 2], "obligation_debtor");
    assert_marker_refused(&late);
    // The people marker twice: refused.
    let mut twice = v.clone();
    twice["agency"]["unchecked"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!("responsibility_people_undeclared"));
    assert_marker_refused(&twice);
    // Without ledger rows the people marker is last: obligation_debtor may not
    // follow it.
    let mut no_rows = v.clone();
    no_rows["obligations"] = json!([]);
    assert_marker_refused(&no_rows);
    no_rows["agency"]["unchecked"].as_array_mut().unwrap().pop();
    fixture_match::compare_report(&json!({}), &no_rows)
        .unwrap_or_else(|e| panic!("people marker last without rows: {e}"));
}

/// Post-integration boundary review m2 (positive): one case, no people, one
/// ledger row. Hand: with `cases 1` lower adds no multi_case_interference
/// (it is the marker of two or more cases), so the closed tail is the people
/// marker then obligation_debtor; the other lower markers are those of the
/// two-case list above (they depend on the model's structure, not on the case
/// count). One case, one response obligation; A.run ends done at 5min, within
/// 9min, so the report is satisfied and exit 0.
#[test]
fn m2_matcher_accepts_one_case_people_marker_then_obligation_debtor() {
    let one = PEOPLELESS
        .replace("arrivals fixed [0min, 2min]", "arrivals fixed [0min]")
        .replace("bounds { cases 2;", "bounds { cases 1;");
    assert_eq!(one.matches("arrivals fixed [0min]").count(), 1);
    assert_eq!(one.matches("cases 1;").count(), 1);
    let (code, out, err) = run(&["check"], &one);
    assert_eq!(code, 0);
    assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["bounds"]["cases"], 1);
    assert_eq!(v["obligations"].as_array().unwrap().len(), 1);
    assert_eq!(
        markers(&v),
        [
            "W-RESP-1",
            "contest_path",
            "fallback_path",
            "via_model",
            "responsibility_people_undeclared",
            "obligation_debtor",
        ]
    );
    fixture_match::compare_report(&json!({}), &v)
        .unwrap_or_else(|e| panic!("the wire matcher must accept [.., rpu, od]: {e}"));
}

/// Post-integration boundary review m2 (negative): the two-case wire with
/// multi_case_interference and the people marker swapped, i.e. the tail
/// `[.., responsibility_people_undeclared, multi_case_interference,
/// obligation_debtor]`, breaks the closed lower order and is refused.
#[test]
fn m2_matcher_refuses_people_marker_before_multi_case_interference() {
    let (_, out, _) = run(&["check"], PEOPLELESS);
    let v: Value = serde_json::from_slice(&out).unwrap();
    let n = markers(&v).len();
    let mut swapped = v.clone();
    swapped["agency"]["unchecked"]
        .as_array_mut()
        .unwrap()
        .swap(n - 3, n - 2);
    assert_eq!(
        markers(&swapped)[n - 3..],
        [
            "responsibility_people_undeclared",
            "multi_case_interference",
            "obligation_debtor"
        ]
    );
    assert_marker_refused(&swapped);
}
