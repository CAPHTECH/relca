//! Fixed private acceptance. Include as a cfg(test) child of
//! comparison_projection; no new public API and no replacement validator.
use super::{exhaustive, simulation};
use crate::{ir, lower, report, syntax};
use serde_json::json;
const SOURCE: &str = r#"model PrivateReceipt version "0.2" {
 type Ack = enum { committed, duplicate }
 actor Ledger { capacity 1; state keys: set<CaseId>
   capability post(key: CaseId) -> Ack { commits; idempotency key key; writes keys; duration fixed 1min;
     atomic { if key in keys { result duplicate } else { keys += key; result committed } }
     cancel forbidden_after_start }
 }
 process Flow { ack = Ledger.post(case.id) response_loss sequence [unknown,unknown]
   retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed; end done }
 cut C { include [Ledger] }
 arrivals fixed [0min,4min]
 policy observable fifo tie_by case_id then call_order assign first_idle
 bounds { cases 2; live_tasks 2; clock 20min }
 simulation replications 4 seed 7 confidence 0.5
 verify safety no_resource_overcommit
}"#;
fn model() -> ir::Model {
    lower::lower(&syntax::parse(SOURCE).unwrap()).unwrap()
}
fn control() -> report::Report {
    // Only producer metadata is borrowed. The entire compared inventory and
    // every compared number is independently assembled from the two timelines.
    let mut r = report::check(SOURCE);
    r.terminals.clear();
    r.bounds.truncated = Some(false);
    r.properties = serde_json::from_value(json!([
        {"id":"safety no_resource_overcommit","verdict":"satisfied"},
        {"id":"receipt_resolution","verdict":"inconclusive","cases":[
            {"case_id":1,"arrival":0,"verdict":"inconclusive"},
            {"case_id":2,"arrival":4,"verdict":"inconclusive"}]}]))
    .unwrap();
    r.case_outcomes=Some((1..=2).map(|id| {
        let a=(id-1)*4;
        serde_json::from_value(json!({"case_id":id,"arrival":a,"terminals":[
            {"kind":"receipt_unknown","end":null,"completed_at":null,"elapsed":null,"work":null,"result_at":null,
            "observed_until":{"min":a+2,"max":a+2},"observed_elapsed":{"min":2,"max":2},"observed_work":{"min":2,"max":2}}
        ]})).unwrap()
    }).collect());
    r.response_receipts=Some((1..=2).flat_map(|id| (1..=2).map(move |attempt| {
        let t=(id-1)*4+attempt;
        serde_json::from_value(json!({"case_id":id,"command":"ack","attempt":attempt,"capability":"Ledger.post","receipt":"unknown","at":{"min":t,"max":t},"observed_result":null,"resolved_effect":false})).unwrap()
    })).collect());
    r
}
#[test]
fn p3a_projection_accepts_hand_unknown_and_preserves_observation_fields() {
    let view = exhaustive(&control(), &model()).expect("valid Q1 hand control must project");
    assert_eq!(view.arrivals.len(), 2);
    assert_eq!(view.terminals.len(), 2);
    let rows:Vec<_>=view.terminals.iter().map(|((id,key),value)|json!({"case_id":id,"kind":key.fields().0,"end":key.fields().1,"value":value})).collect();
    for (index, t) in [2, 6].into_iter().enumerate() {
        assert_eq!(rows[index]["kind"], "receipt_unknown");
        assert_eq!(rows[index]["end"], serde_json::Value::Null);
        assert_eq!(
            rows[index]["value"]["observed_until"],
            json!({"min":t,"max":t})
        );
        assert_eq!(
            rows[index]["value"]["observed_elapsed"],
            json!({"min":2,"max":2})
        );
        assert_eq!(
            rows[index]["value"]["observed_work"],
            json!({"min":2,"max":2})
        );
    }
}
#[test]
fn p3a_projection_rejects_single_faults_not_merely_all_q1_reports() {
    exhaustive(&control(), &model()).expect("establish valid control before negative mutations");
    for fault in [
        "kind",
        "completion",
        "observed_work",
        "case",
        "all_cases",
        "property",
        "notes",
        "command",
        "alias",
        "attempt",
    ] {
        let mut r = control();
        match fault {
            "kind" => {
                r.case_outcomes.as_mut().unwrap()[0].terminals[0].kind =
                    report::CaseTerminalKind::End
            }
            "completion" => {
                r.case_outcomes.as_mut().unwrap()[0].terminals[0].completed_at =
                    Some(report::Range { min: 2, max: 2 })
            }
            "observed_work" => {
                r.case_outcomes.as_mut().unwrap()[0].terminals[0].observed_work = None
            }
            "case" => r.case_outcomes.as_mut().unwrap()[0].case_id = 2,
            "all_cases" => r.case_outcomes = Some(Vec::new()),
            "property" => {
                r.properties.pop();
            }
            "notes" => r.properties[1].notes = Some(vec!["invented receipt proof".into()]),
            "command" => r.response_receipts.as_mut().unwrap()[0].command = "other_command".into(),
            "alias" => {
                let mut alias = r.response_receipts.as_ref().unwrap()[0].clone();
                alias.command = "ack_alias".into();
                r.response_receipts.as_mut().unwrap().push(alias);
            }
            "attempt" => r.response_receipts.as_mut().unwrap()[0].attempt = 3,
            _ => unreachable!(),
        }
        assert!(exhaustive(&r, &model()).is_err(), "accepted {fault}");
    }
}
#[test]
fn p3a_projection_reordered_cases_are_keyed_by_id_not_position() {
    let m = model();
    let r = control();
    let expected = exhaustive(&r, &m).expect("valid keyed control");
    let mut reversed = r;
    reversed.case_outcomes.as_mut().unwrap().reverse();
    reversed.response_receipts.as_mut().unwrap().reverse();
    reversed.properties[1].cases.as_mut().unwrap().reverse();
    let actual = exhaustive(&reversed, &m).unwrap();
    assert_eq!(actual.arrivals, expected.arrivals);
    assert_eq!(actual.terminals, expected.terminals);
}
#[test]
fn p3a_projection_sample_unknown_is_subset_of_unfinished_not_completed_twice() {
    let m = model();
    let mut r = report::simulate(SOURCE);
    // Hand-assemble the compared sampling inventory as well. The runtime
    // endpoint controls are separate real-CLI tests; this seam validates DTOs.
    r.bounds.truncated = Some(false);
    r.diagnostics.clear();
    r.terminals.clear();
    r.simulation["status"] = json!("completed");
    r.simulation["completed_replications"] = json!(4);
    r.properties = vec![
        json!({"id":"safety no_resource_overcommit","verdict":"inconclusive","basis":"sample","witness":null}),
        json!({"id":"receipt_resolution","verdict":"inconclusive"}),
    ];
    r.case_outcomes=(1..=2).map(|id|json!({"case_id":id,"arrival":(id-1)*4,"terminals":[{"kind":"receipt_unknown","end":null,"count":4}],"unfinished_count":4,"deadlocked_count":0})).collect();
    let v = simulation(&r, &m)
        .expect("Q1 simulation projection connects receipt_resolution and unknown outcome");
    assert_eq!(v.arrivals.len(), 2);
    assert_eq!(v.terminals.len(), 2);
    for (id, case) in r.case_outcomes.iter().enumerate() {
        assert_eq!(case["case_id"], id + 1);
        assert_eq!(case["unfinished_count"], 4);
        assert_eq!(
            case["terminals"],
            json!([{"kind":"receipt_unknown","end":null,"count":4}])
        );
    }
    for fault in [
        "unknown_zero",
        "unfinished_zero",
        "wrong_case",
        "row_missing",
        "property_missing",
    ] {
        let mut bad = r.clone();
        match fault {
            "unknown_zero" => bad.case_outcomes[0]["terminals"][0]["count"] = json!(0),
            "unfinished_zero" => bad.case_outcomes[0]["unfinished_count"] = json!(0),
            "wrong_case" => bad.case_outcomes[0]["case_id"] = json!(2),
            "row_missing" => bad.case_outcomes[0]["terminals"] = json!([]),
            "property_missing" => {
                bad.properties.pop();
            }
            _ => unreachable!(),
        }
        assert!(simulation(&bad, &m).is_err(), "sample fault {fault}");
    }
}
