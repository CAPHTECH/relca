//! Fixed private acceptance. Connect at crate root with cfg(test) + path.
//! Adapter contract and fault meanings are in receipt_recovery_calculations.md.
//! This file MUST NOT be edited to implement the adapter.
use super::r2_test_adapter as a;
use serde_json::{Value, json};
#[test]
fn r2_private_query_is_finish_projection_not_snapshot_or_enum_draw() {
    for system in ["engine", "reference"] {
        for (scenario, status, evidence) in [
            ("empty_available", "not_committed", true),
            ("present_available", "committed_once", true),
            ("empty_unavailable", "still_unknown", false),
            ("present_unavailable", "still_unknown", false),
            ("insert_after_start_before_finish", "committed_once", true),
            ("other_case_key_only", "not_committed", true),
        ] {
            let v = a::project(system, scenario);
            assert_eq!(
                v,
                json!({"status":status,"evidence":evidence,"at":3,"commits":0,"writes":0,"work":2}),
                "{system}/{scenario}"
            );
        }
        for scenario in ["empty_nondet", "present_nondet"] {
            let v = a::project(system, scenario);
            let rows = v.as_array().unwrap();
            let expected = if scenario.starts_with("empty") {
                "not_committed"
            } else {
                "committed_once"
            };
            assert_eq!(rows.len(), 2);
            assert!(rows.iter().any(|r| r["status"] == expected));
            assert!(rows.iter().any(|r| r["status"] == "still_unknown"));
            assert!(rows.iter().all(|r| r.get("probability").is_none()));
        }
    }
}
#[test]
fn r2_private_public_ir_has_two_independent_fail_closed_validators() {
    for system in ["engine", "reference"] {
        assert_eq!(
            a::validate_ir(system, "valid"),
            Value::Null,
            "valid control first"
        );
        for fault in [
            "initial_nonempty",
            "different_set",
            "non_case_key",
            "extra_writer",
            "key_delete",
            "shared_command_key",
            "different_provider",
            "ordinary_enum_source",
            "query_writes",
            "query_commits",
            "query_kernel",
            "query_requires",
            "missing_evidence",
            "wrong_case",
            "wrong_command",
            "wrong_key",
            "wrong_provider",
            "unknown_fact",
            "non_dominating",
            "double_resolve",
            "resolved_stop",
            "unresolved_end",
            "query_count33",
            "nested_handler",
            "b_handler",
        ] {
            assert_eq!(
                a::validate_ir(system, fault),
                json!({"reason":"invalid_recovery_ir","stop":if system=="engine" {"unsupported"}else{"semantic_abort"}}),
                "{system}/{fault}"
            );
        }
    }
}
#[test]
fn r2_private_runtime_identity_and_replay_are_checked_not_trusted() {
    for system in ["engine", "reference"] {
        assert_eq!(
            a::resolve(system, "valid"),
            json!({"unresolved":["other"],"pay_visible":false,"charge_commits":1,"refund_commits":0,"work_added":0,"time_added":0})
        );
        for fault in [
            "wrong_case",
            "wrong_command",
            "wrong_key",
            "wrong_provider",
            "wrong_query_site",
            "forged_enum",
            "still_unknown",
            "evidence_before_query",
            "double_resolve",
        ] {
            assert_eq!(
                a::resolve(system, fault),
                json!({"rejected":true}),
                "{system}/{fault}"
            );
        }
    }
    assert_eq!(
        a::replay("valid"),
        json!({"verified":true,"query_at":3,"decision_at":6,"resolve_at":6,"pay_visible":false})
    );
    for fault in [
        "swap_case",
        "swap_command",
        "swap_key",
        "swap_evidence",
        "omit_query",
        "change_status",
        "change_resolve_time",
        "publish_unknown_ack",
    ] {
        assert_eq!(
            a::replay(fault),
            json!({"verified":false,"reason":"unsupported"}),
            "{fault}"
        );
    }
}
#[test]
fn r2_private_cut_observations_are_physical_not_complete_metric_extrema() {
    for system in ["engine", "reference"] {
        for (scenario, t, w, evidence) in [
            ("clock2", 2, 2, false),
            ("horizon2", 2, 2, false),
            ("clock4", 4, 4, true),
        ] {
            assert_eq!(
                a::cut(system, scenario),
                json!({"at":t,"observed_work":w,"charge_commits":1,"query_evidence":evidence,"decision_recorded":false,"resolved":false,"end":null,"stop":if scenario=="horizon2" {"cut_off"}else{"truncated"}}),
                "{system}/{scenario}"
            );
        }
    }
}
#[test]
fn r2_private_commit_and_people_counts_are_not_inferred_from_query_labels() {
    for system in ["engine", "reference"] {
        for (scenario, t, refund, c, f, q) in [
            ("R01", 7, 0, 3, 0, 0),
            ("R04", 9, 1, 4, 1, 0),
            ("R05", 10, 1, 5, 1, 0),
            ("R01_person_query", 7, 0, 3, 0, 3),
            ("R04_person_query", 9, 1, 4, 1, 4),
            ("R05_person_query", 10, 1, 5, 1, 4),
        ] {
            assert_eq!(
                a::history(system, scenario),
                json!({"charge_commits":1,"refund_commits":refund,"total_commits":1+refund,"end_at":t,"work":t,"people":{"c":c,"f":f,"q":q},"key_present":true,"original_receipt":"unknown","pay_visible":false}),
                "{system}/{scenario}"
            );
        }
        assert_eq!(
            a::history(system, "R08"),
            json!({"attempts":2,"handler_entries":1,"arrivals":1,"charge_commits":1,"end_at":8,"work":8})
        );
    }
}
#[test]
fn r2_private_probability_path_mass_and_reward_are_not_sample_or_schedule_counts() {
    for system in ["engine", "reference"] {
        assert_eq!(
            a::probability(system, true),
            json!({"paths":[{"path":"D","mass":"9/10","end_at":2,"work":2,"tasks":2},{"path":"UD","mass":"9/100","end_at":3,"work":3,"tasks":3},{"path":"UU","mass":"1/100","end_at":8,"work":8,"tasks":5}],"expected_work":"43/20","expected_tasks":"53/25","expected_attempts":"11/10","query_work":"1/50","decision_work":"3/100","completed":"1","unknown":"0"})
        );
        assert_eq!(
            a::probability(system, false),
            json!({"expected_observed_work":"211/100","completed":"99/100","unknown":"1/100","uu_observed_at":4,"uu_observed_work":4,"uu_end":null})
        );
    }
}
