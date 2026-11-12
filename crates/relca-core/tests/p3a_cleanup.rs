//! Phase 2 review carryovers, scoped to P3a by the checkpoint ruling.
#[path = "support/p2_cleanup_inputs.rs"]
mod prior;
use relca_core::{agency, engine, lower, reference, syntax};
use serde_json::{Value, json};
fn wire(reason: &str) -> Value {
    json!({"rules":"agency-diff-p2-1","status":"derived","unchecked":["fallback_path","workload_transfer","via_model_paths"],"rows":[{"viewpoint":"degradation","item":"common_prefix","call":"Manual","status":"not_comparable","from":{"status":"derived","reason":null},"to":{"status":"not_derived","reason":reason}}]})
}
#[test]
fn m1_prefix_decoder_accepts_only_missing_item_or_whole_table_withdrawal() {
    for reason in [
        "to.agency.degradation:null",
        "to.agency.degradation:multiple_case_interference",
        "to.agency.degradation.Manual.common_prefix:null",
    ] {
        let v = wire(reason);
        assert_eq!(
            json!(serde_json::from_value::<agency::Differences>(v.clone()).unwrap()),
            v
        );
    }
}
#[test]
fn m1_prefix_decoder_rejects_side_only_reasons_in_item_position() {
    serde_json::from_value::<agency::Differences>(wire(
        "to.agency.degradation.Manual.common_prefix:null",
    ))
    .unwrap();
    for reason in [
        "requires",
        "shared_state_guard",
        "calendar",
        "arrival_coverage",
        "priority",
        "resource_impossible",
        "qualification_unavailable",
        "unknown_binding",
        "unsupported_control",
        "invalid_q6_ir",
        "derivation_limit",
        "multiple_case_interference",
    ] {
        let v = wire(&format!(
            "to.agency.degradation.Manual.common_prefix:{reason}"
        ));
        assert!(
            serde_json::from_value::<agency::Differences>(v).is_err(),
            "item common_prefix is always counts, never a side withdrawal: {reason}"
        );
    }
}
#[test]
fn m2_fixture_generator_emits_current_agency_difference_version() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let s = include_str!("support/generate_agency_second_half_fixtures.py");
    // Evaluate only the actual diff_e constructor expression, with an empty
    // hand row list. Never run main/materialization or rewrite fixed fixtures.
    let mut child=Command::new("python3").args(["-B","-c",r#"import ast,json,sys
tree=ast.parse(sys.stdin.read())
nodes=[n for n in ast.walk(tree) if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='diff_e' for t in n.targets)]
assert len(nodes)==1
expression=ast.Expression(body=nodes[0].value)
value=eval(compile(expression,'<generator-constructor>','eval'),{'__builtins__':{'dict':dict}},{'differences':[]})
print(json.dumps(value['agency']['differences']['rules']))
"#]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(s.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        "agency-diff-p2-1",
        "actual generator constructor, not a comment/string census"
    );
}
#[test]
fn u_contest_observations_with_rates_without_metrics_match_both_interpreters() {
    for source in [
        prior::KNOWN_FAILURE.to_string(),
        prior::MANAGEMENT.replace("[0min,2min]", "[0min,4min]"),
    ] {
        let source = prior::edit(&source, "metrics { window 10min; currency \"JPY\" }", "");
        let m = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
        assert!(m.metrics.is_none());
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(e.completed_replications, 2);
        assert_eq!(r.outcome.completed_replications, 2);
        for (a, b) in e.replications.iter().zip(&r.outcome.replications) {
            assert_eq!(
                a.contests, b.contests,
                "full ContestObservation PartialEq without metric_history"
            );
            assert_eq!(a.contests.len(), 2);
            if source.contains("KnownFailure") {
                assert_eq!(a.contests[0].closed_at, Some(1));
                assert_eq!(a.contests[0].started_at, None);
                assert!(a.contests[0].failed);
                assert_eq!(a.contests[1].closed_at, None);
                assert_eq!(a.contests[1].started_at, Some(1));
            } else {
                assert_eq!(a.contests[0].closed_at, Some(3));
                assert_eq!(a.contests[1].closed_at, Some(7));
            }
        }
    }
}
