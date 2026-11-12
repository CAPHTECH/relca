//! Mount under diff::work_profiles. The hook withholds a real table; it does
//! not manufacture the Err or the outer diagnostic under test.
use crate::p2_cleanup_shared::input;
use std::cell::Cell;
thread_local! {static MISSING:Cell<bool>=const {Cell::new(false)};}
pub(super) fn missing() -> bool {
    MISSING.with(Cell::get)
}
#[test]
fn p2_missing_record_reaches_real_diff_and_compare_adapters() {
    for comparison in [false, true] {
        MISSING.with(|c| c.set(true));
        let v = if comparison {
            serde_json::to_value(crate::compare::compare(
                input::WORK,
                &["From", "To"],
                crate::compare::Analysis::Exhaustive,
            ))
            .unwrap()
        } else {
            serde_json::to_value(crate::diff::diff(input::WORK, "From", "To")).unwrap()
        };
        MISSING.with(|c| c.set(false));
        assert_eq!(v["verdict"], "unsupported");
        let errors: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["code"], "U-SEM-0");
        assert!(
            errors[0]["message"]
                .as_str()
                .unwrap()
                .starts_with("invalid_q6_record:"),
            "{}",
            errors[0]
        );
    }
}
