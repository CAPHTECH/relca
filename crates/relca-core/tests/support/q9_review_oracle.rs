//! Review hand expectations. See contest_rates_review_calculations.md.
#![allow(dead_code)]
use serde_json::{Value, json};
#[path = "q9_oracle.rs"]
mod original;

pub fn table(in_route: bool) -> Value {
    let mut t = original::expected_table("fixed");
    let r = &mut t["rows"][0];
    r["responded"] = json!(0);
    if in_route {
        r["unresolved"] = json!(16);
    } else {
        r["target_observed"] = json!(0);
        r["target_unknown"] = json!(16);
        r["filed"] = json!(0);
        r["filing_fraction"] =
            json!({"value":null,"numerator":0,"denominator":0,"reason":"unfinished"});
    }
    for k in ["service_time", "queue_wait", "response_elapsed"] {
        r[k] = original::absent("unfinished");
    }
    t
}

pub fn range_report(v: &Value, in_route: bool) {
    assert_eq!(v["mode"], "simulation");
    assert_eq!(v["verdict"], "violated", "state_in_range witness");
    assert_eq!(v["simulation"]["range_stopped_replications"], 16);
    assert_eq!(v["simulation"]["truncated_replications"], 0);
    original::subset(
        &table(in_route),
        &v["metrics"]["contests"],
        "metrics.contests",
    )
    .unwrap();
}

pub fn check_assumption(v: &Value, rates: bool) {
    let rows: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["field"] == "metrics.contests")
        .collect();
    assert_eq!(
        rows.len(),
        usize::from(rates),
        "check must disclose unprovided sample aggregation only for rates"
    );
    if rates {
        assert_eq!(rows[0]["value"], "not_provided");
    }
    assert!(
        v.pointer("/metrics/contests").is_none(),
        "no sample table in check"
    );
}

pub fn legacy_refusal(v: &Value, at: u64) {
    assert_eq!(
        v["verdict"], "unsupported",
        "reject entire sample, not a live_tasks inconclusive"
    );
    assert_eq!(v["simulation"]["status"], "unsupported");
    assert_eq!(v["simulation"]["completed_replications"], 0);
    assert!(v["simulation"]["truncated_replications"].is_null());
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0"
                && d["message"]
                    .as_str()
                    .unwrap()
                    .contains("unresolved nondeterminism")
                && d["message"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("time {at},"))),
        "encounter rejection must precede other management/arithmetic at target: {}",
        v["diagnostics"]
    );
    assert!(v.pointer("/metrics/contests").is_none());
}

/// Positive rows differ only in declared known/unknown cohorts. Identity and
/// every partition remain valid, so negative pairs isolate reason consistency.
pub fn wire(case: &str, reason: Option<&str>) -> Value {
    let mut t = original::expected_table("fixed");
    let r = &mut t["rows"][0];
    match case {
        "unresolved" => {
            r["responded"] = json!(15);
            r["unresolved"] = json!(1);
        }
        "target_unknown" => {
            r["target_observed"] = json!(15);
            r["target_unknown"] = json!(1);
            r["filed"] = json!(15);
            r["responded"] = json!(15);
            r["filing_fraction"] =
                json!({"value":null,"numerator":15,"denominator":15,"reason":"unfinished"});
        }
        "known" => (),
        _ => panic!("unknown wire case"),
    }
    for k in ["service_time", "queue_wait", "response_elapsed"] {
        r[k] = match reason {
            Some(reason) => original::absent(reason),
            None => original::sample(
                if k == "queue_wait" { "0" } else { "2" },
                if k == "queue_wait" { 0 } else { 2 },
                if k == "queue_wait" { 0 } else { 2 },
                if case == "known" { 16 } else { 15 },
            ),
        };
    }
    t
}
