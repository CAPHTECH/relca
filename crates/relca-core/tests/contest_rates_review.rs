//! Q9 merge-review acceptance; production is deliberately not repaired here.
#[path = "support/q9_review_oracle.rs"]
mod hand;
#[path = "support/q9_review_inputs.rs"]
mod input;
#[path = "support/q9_review_reference_aggregate.rs"]
mod private_reference_aggregate;
use relca_core::{engine, ir, lower, metrics, outcome, reference, report, syntax};
use serde_json::json;

fn model(name: &str) -> ir::Model {
    lower::lower(&syntax::parse(&input::source(name)).expect("review syntax"))
        .expect("review lowering")
}

fn range_observations(o: &outcome::SimulationOutcome, in_route: bool) {
    assert_eq!(o.completed_replications, 16);
    assert_eq!(
        o.range_stops.len(),
        16,
        "must actually reach out-of-range write"
    );
    assert_eq!(
        o.truncated_replications,
        Some(0),
        "range stop is not a computational cut"
    );
    for (i, (rep, ranges)) in o.range_stops.iter().enumerate() {
        assert_eq!(*rep, i as u64 + 1);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].attempted, "2");
        assert_eq!(ranges[0].time, if in_route { 3 } else { 1 });
    }
    for r in &o.replications {
        assert_eq!(r.cut, None);
        assert_eq!(r.contests.len(), 1);
        let c = &r.contests[0];
        assert_eq!(
            c.target_known, in_route,
            "range stop does not prove target absence"
        );
        assert_eq!(c.target_at, in_route.then_some(1));
        assert_eq!(c.filed, in_route.then_some(true));
        assert_eq!(c.started_at, in_route.then_some(1));
        assert_eq!(c.response_at, None);
        assert!(!c.failed, "range stop is unresolved, not route_failed");
        assert_eq!(
            c.closed_at, None,
            "range stop must not fabricate route closure"
        );
    }
}
macro_rules! range_test {
    ($name:ident,$source:literal,$in_route:literal,$reference:literal) => {
        #[test]
        fn $name() {
            let m = model($source);
            if $reference {
                let r = reference::simulate(&m);
                assert_eq!(r.stop, reference::StopReason::Complete);
                range_observations(&r.outcome, $in_route);
            } else {
                range_observations(&engine::simulate(&m), $in_route);
            }
        }
    };
}
range_test!(
    mj1_before_target_engine,
    "range-before-target",
    false,
    false
);
range_test!(
    mj1_before_target_reference,
    "range-before-target",
    false,
    true
);
range_test!(mj1_in_route_engine, "range-in-route", true, false);
range_test!(mj1_in_route_reference, "range-in-route", true, true);
#[test]
fn mj1_before_target_aggregate() {
    hand::range_report(
        &json!(report::simulate(&input::source("range-before-target"))),
        false,
    );
}
#[test]
fn mj1_in_route_aggregate() {
    hand::range_report(
        &json!(report::simulate(&input::source("range-in-route"))),
        true,
    );
}

fn permission_cut(o: outcome::SimulationOutcome) {
    assert_eq!(o.completed_replications, 16);
    for rep in o.replications {
        assert_eq!(rep.cut, Some(outcome::SimulationCut::Clock));
        let c = &rep.contests[0];
        assert_eq!(c.started_at, Some(1));
        assert_eq!(c.response_at, None);
        assert!(
            !c.failed,
            "running route stays unresolved even with permission witness"
        );
    }
}
#[test]
fn m1_running_permission_violation_engine() {
    permission_cut(engine::simulate(&model("permission-cut")));
}
#[test]
fn m1_running_permission_violation_reference() {
    let r = reference::simulate(&model("permission-cut"));
    assert_eq!(r.stop, reference::StopReason::Complete);
    permission_cut(r.outcome);
}

#[test]
fn m3_check_discloses_unprovided_sample_aggregation() {
    hand::check_assumption(&json!(report::check(&input::source("fixed"))), true);
    hand::check_assumption(&json!(report::check(&input::source("legacy"))), false);
}

fn legacy_outcome(o: outcome::SimulationOutcome) {
    assert_eq!(o.completed_replications, 0);
    assert_eq!(o.truncated_replications, None);
    assert!(
        o.diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0" && d.message.contains("unresolved nondeterminism")),
        "{:?}",
        o.diagnostics
    );
}
#[test]
fn m4_legacy_encounter_precedes_deadline_overflow_engine() {
    legacy_outcome(engine::simulate(&model("legacy-overflow")));
    hand::legacy_refusal(
        &json!(report::simulate(&input::source("legacy-overflow"))),
        1,
    );
}
#[test]
fn m4_legacy_encounter_precedes_deadline_overflow_reference() {
    legacy_outcome(reference::simulate(&model("legacy-overflow")).outcome);
}

fn legacy_live(reference: bool) {
    for name in ["legacy-live-control", "legacy-live"] {
        let m = model(name);
        let (side, o) = if reference {
            ("reference", reference::simulate(&m).outcome)
        } else {
            ("engine", engine::simulate(&m))
        };
        assert_eq!(
            o.completed_replications,
            0,
            "{name}/{side}; cuts={:?}",
            o.replications
                .iter()
                .map(|r| (r.replication, r.cut))
                .collect::<Vec<_>>()
        );
        assert_eq!(o.truncated_replications, None, "{name}/{side}");
        assert!(
            o.diagnostics.iter().any(|d| d.code == "U-SEM-0"
                && d.message.contains("unresolved nondeterminism")
                && d.message.contains("time 3,")),
            "{name}/{side}: {:?}",
            o.diagnostics
        );
    }
}
#[test]
fn u2_legacy_encounter_precedes_live_cut_engine() {
    legacy_live(false);
}
#[test]
fn u2_legacy_encounter_precedes_live_cut_reference() {
    legacy_live(true);
}

#[test]
fn u1_end_with_cancelled_unstarted_task_matches_reference() {
    let m = model("end-unstarted");
    assert!(m.contests().is_empty());
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for o in [&e, &r.outcome] {
        assert_eq!(o.completed_replications, 16, "{:?}", o.diagnostics);
        assert_eq!(o.truncated_replications, Some(0));
        for rep in &o.replications {
            assert!(rep.cut.is_none());
            assert!(rep.cases[0].deadlock.is_none());
            assert_eq!(
                rep.cases[0].terminal.as_ref().unwrap().end.as_deref(),
                Some("done")
            );
            let v = json!(rep.metrics);
            let tasks = v["cases"][0]["tasks"].as_array().unwrap();
            let queued = tasks.iter().find(|t| t["binding"] == "b").unwrap();
            assert_eq!(queued["starts"]["min"], "0");
            assert_eq!(queued["cancelled_before_start_count"]["stats"]["min"], "1");
        }
    }
    for (a, b) in e.replications.iter().zip(&r.outcome.replications) {
        assert_eq!(
            json!(a),
            json!(b),
            "same observations and metrics, not just root verdict"
        );
    }
}

macro_rules! wire_test {
    ($name:ident,$case:literal,$reason:expr,$accepted:literal) => {
        #[test]
        fn $name() {
            let result =
                serde_json::from_value::<metrics::contests::Table>(hand::wire($case, $reason));
            assert_eq!(result.is_ok(), $accepted, "{result:?}");
        }
    };
}
wire_test!(m2_known_positive_control, "known", None, true);
wire_test!(
    m2_unresolved_positive_control,
    "unresolved",
    Some("unfinished"),
    true
);
wire_test!(
    m2_target_unknown_positive_control,
    "target_unknown",
    Some("unfinished"),
    true
);
wire_test!(
    m2_unresolved_cannot_have_numeric_times,
    "unresolved",
    None,
    false
);
wire_test!(
    m2_target_unknown_cannot_have_numeric_times,
    "target_unknown",
    None,
    false
);
wire_test!(
    m2_known_cannot_have_truncated_times,
    "known",
    Some("truncated"),
    false
);
wire_test!(
    m2_known_cannot_have_unfinished_times,
    "known",
    Some("unfinished"),
    false
);

#[test]
fn m2_known_cannot_have_truncated_fraction() {
    fraction_without_unknown("truncated");
}
#[test]
fn m2_known_cannot_have_unfinished_fraction() {
    fraction_without_unknown("unfinished");
}
fn fraction_without_unknown(reason: &str) {
    let mut t = hand::wire("known", None);
    t["rows"][0]["filing_fraction"]["value"] = serde_json::Value::Null;
    t["rows"][0]["filing_fraction"]["reason"] = json!(reason);
    assert!(
        serde_json::from_value::<metrics::contests::Table>(t).is_err(),
        "{reason} fraction requires unknown target or choice"
    );
}
