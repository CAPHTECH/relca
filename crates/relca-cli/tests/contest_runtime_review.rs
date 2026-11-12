//! Closed-wire regression for the D2 overdue, unanswered cut reason.
#[path = "../src/fixture_match.rs"]
mod fixture_match;

#[test]
fn minor3_overdue_unresolved_reason_is_in_the_closed_vocabulary() {
    let mut hand = serde_json::json!({
        "bounds":{"cases":1,"truncated":true},
        "contest_summary":{"business_ended_open":{"min":null,"max":null,"reason":"truncated"}},
        "properties":[{"id":"contract Controls #1 contest","verdict":"satisfied"},{"id":"contract Controls #1 contest_response","verdict":"violated"}],
        "contest_runtime":[{"id":"case1:contest1","static_id":"contract Controls #1 contest","case_id":1,"party":"Party","target":"returned","route":"Route.respond","respond_by":2,"basis":"モデル内での期限内応答","coverage":"truncated","alternatives":[
            {"filed":true,"target_at":1,"filed_at":1,"deadline":3,"status":"unresolved","response_at":null,"closed_at":null,"executor":"p","result":null,"violation_reason":"deadline_passed_unresolved","route_work":null,"route_wait":null,"route_cost":{"min":null,"max":null,"reason":"truncated"}}
        ]}]
    });
    fixture_match::validate_contest_runtime_wire(&hand).unwrap();
    hand["contest_runtime"][0]["alternatives"][0]["violation_reason"] =
        "unknown_deadline_reason".into();
    assert!(fixture_match::validate_contest_runtime_wire(&hand).is_err());
}
