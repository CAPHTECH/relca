//! Review findings on ADR 0019 (addendum 2): same-time completion and start,
//! and the rejected row of a case that is never rejected.
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

const TIE: &str = r#"model PeakTie version "0.2" {
  type T = enum { pass }
  actor A { capacity 2; capability run() -> T { duration fixed 3min; result nondet { pass } } }
  process P { job = A.run(); end done }
  cut C { include [A] }
  arrivals fixed [0min, 3min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 2; clock 10min }
  metrics { window 10min; currency "JPY"; rate A per_minute 1; rate A.run per_start 1 }
  verify reachable end.done
}"#;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../fixtures/{name}/model.relca",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn rejected_row(case: &Value) -> &Value {
    case["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["kind"] == "rejected")
        .unwrap()
}

#[test]
fn r_same_time_completion_and_start_do_not_overlap() {
    let m = lower::lower(&syntax::parse(TIE).unwrap()).unwrap();
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    let ev = serde_json::to_value(&e.metrics).unwrap();
    let rv = serde_json::to_value(&r.outcome.metrics).unwrap();
    let one = json!({"min":"1","max":"1","reason":null});
    for v in [&ev, &rv] {
        assert_eq!(v["overall"]["peak_tasks"], one);
        assert_eq!(v["actors"][0]["peak_tasks"], one);
    }
    assert_eq!(ev, rv);
}

#[test]
fn r_rejected_row_cost_is_null_unless_the_case_is_rejected() {
    let v = serde_json::to_value(report::check(&fixture("met-serial"))).unwrap();
    assert_eq!(
        rejected_row(&v["metrics"]["cases"][0])["cost_per_case"],
        json!({"min":null,"max":null,"reason":"no_terminal"})
    );
    // Cases 2 and 4 are rejected in every replication; 1 and 3 never are.
    let v = serde_json::to_value(report::simulate(&fixture("met-exclusions"))).unwrap();
    let cases = v["metrics"]["cases"].as_array().unwrap();
    for (i, rejected) in [(0, false), (1, true), (2, false), (3, true)] {
        let cost = &rejected_row(&cases[i])["cost_per_case"];
        if rejected {
            assert_eq!(cost["mean"], "0", "case {}", i + 1);
            assert_eq!(cost["samples"], 4, "case {}", i + 1);
        } else {
            assert_eq!(
                cost,
                &json!({"mean":null,"min":null,"max":null,"samples":0,"reason":"no_samples"}),
                "case {}",
                i + 1
            );
        }
    }
}
