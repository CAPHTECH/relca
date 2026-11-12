//! Phase 2 cleanup acceptance. No snapshots supply semantic expectations.
#[path = "support/p2_cleanup_contests.rs"]
mod contests;
#[path = "support/p2_cleanup_inputs.rs"]
mod input;
#[path = "support/q9_review_inputs.rs"]
mod prior_q9;
#[path = "support/p2_cleanup_private.rs"]
mod private;
use relca_core as core;
use relca_core::{agency, diff, lower, report, sensitivity::inputs::EffectiveInputs, syntax};
use serde_json::{Value, json};

fn model(s: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(s).expect("cleanup syntax")).expect("cleanup lower")
}
#[test]
fn q7_public_stochastic_check_has_only_mode_refusal_and_consistent_ledger() {
    let r = report::check(&input::stochastic());
    let errors: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.severity == core::diag::Severity::Error)
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, "U-SEM-0");
    assert_eq!(errors[0].message, "stochastic_arrivals_mode_unsupported");
    assert_eq!(r.verdict, core::Verdict::Unsupported);
    let v = json!(r);
    serde_json::from_value::<report::Report>(v).unwrap();
}
fn compare(a: &report::Report, z: &report::Report) -> Value {
    fn side(r: &report::Report) -> agency::ComparisonSide<'_> {
        agency::ComparisonSide {
            agency: r.agency.as_ref(),
            responsibilities: None,
        }
    }
    json!(agency::differences(side(a), side(z)))
}
fn rows(v: &Value) -> Vec<Value> {
    v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["viewpoint"] == "degradation")
        .cloned()
        .collect()
}
fn counts(n: u64) -> Value {
    json!([{"capability":"Probe.check","min":n,"max":n},{"capability":"Clerk.run","min":0,"max":0},{"capability":"Fast.run","min":0,"max":0}])
}
fn whole_process_retention(r: &report::Report, n: u64) {
    let v = json!(r);
    let scopes: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["field"] == "agency.retention.normal.scope")
        .collect();
    assert_eq!(scopes.len(), 1);
    assert_eq!(scopes[0]["value"], "whole_process_paths");
    let caps = v["agency"]["viewpoints"]["retention"]["capabilities"]
        .as_array()
        .unwrap();
    for (cap, min, max) in [
        ("Probe.check", n, n),
        ("Clerk.run", 0, 1),
        ("Fast.run", 0, 1),
    ] {
        let c = caps.iter().find(|r| r["capability"] == cap).unwrap();
        assert_eq!(c["normal"], json!({"min":min,"max":max}));
    }
}

#[test]
fn c1_prefix_only_change_has_one_typed_count_row_in_both_directions() {
    let a = report::check(input::DEGRADE);
    let z = report::check(&input::prefix_two());
    whole_process_retention(&a, 1);
    whole_process_retention(&z, 2);
    assert_eq!(
        a.agency
            .as_ref()
            .unwrap()
            .degradation
            .as_ref()
            .unwrap()
            .routes
            .as_ref()
            .unwrap()[0]
            .common_prefix
            .len(),
        3
    );
    for reverse in [false, true] {
        let v = if reverse {
            compare(&z, &a)
        } else {
            compare(&a, &z)
        };
        let (x, y) = if reverse { (2, 1) } else { (1, 2) };
        assert_eq!(
            rows(&v),
            vec![
                json!({"viewpoint":"degradation","item":"common_prefix","call":"Manual","status":"changed","from":counts(x),"to":counts(y)})
            ]
        );
        assert_eq!(
            json!(serde_json::from_value::<agency::Differences>(v.clone()).unwrap()),
            v
        );
    }
    assert!(rows(&compare(&a, &a)).is_empty());
}

#[test]
fn c1_missing_table_adds_prefix_not_comparable_not_a_side_payload() {
    let a = report::check(input::DEGRADE);
    let mut z = a.clone();
    z.agency.as_mut().unwrap().degradation = None;
    for reverse in [false, true] {
        let v = if reverse {
            compare(&z, &a)
        } else {
            compare(&a, &z)
        };
        let r = rows(&v);
        assert_eq!(r.len(), 3);
        let p = r.iter().find(|r| r["item"] == "common_prefix").unwrap();
        assert_eq!(p["call"], "Manual");
        assert_eq!(p["status"], "not_comparable");
        let (missing, known) = if reverse {
            ("from", "to")
        } else {
            ("to", "from")
        };
        assert_eq!(
            p[missing],
            json!({"status":"not_derived","reason":format!("{missing}.agency.degradation:null")})
        );
        assert_eq!(p[known], json!({"status":"derived","reason":null}));
        serde_json::from_value::<agency::Differences>(v).unwrap();
    }
}

#[test]
fn c1_route_rename_is_two_unknown_prefixes_not_a_known_change() {
    let a = report::check(input::DEGRADE);
    let z = report::check(&input::edit(
        input::DEGRADE,
        "degrade Manual",
        "degrade Renamed",
    ));
    let v = compare(&a, &z);
    let p: Vec<_> = rows(&v)
        .into_iter()
        .filter(|r| r["item"] == "common_prefix")
        .collect();
    assert_eq!(p.len(), 2);
    for (name, missing) in [("Manual", "to"), ("Renamed", "from")] {
        let r = p.iter().find(|r| r["call"] == name).unwrap();
        assert_eq!(r["status"], "not_comparable");
        assert_eq!(
            r[missing],
            json!({"status":"not_derived","reason":format!("{missing}.agency.degradation.{name}.common_prefix:null")})
        );
    }
}

fn prefix_wire() -> Value {
    json!({"rules":"agency-diff-p2-1","status":"derived","unchecked":["fallback_path","workload_transfer","via_model_paths"],"rows":[{"viewpoint":"degradation","item":"common_prefix","call":"Manual","status":"changed","from":counts(1),"to":counts(2)}]})
}
#[test]
fn c1_prefix_decoder_accepts_counts() {
    let v = prefix_wire();
    assert_eq!(
        json!(serde_json::from_value::<agency::Differences>(v.clone()).unwrap()),
        v
    );
}
#[test]
fn c1_prefix_decoder_rejects_range_extra_key_and_side_object() {
    for fault in ["range", "key", "side"] {
        let mut v = prefix_wire();
        match fault {
            "range" => v["rows"][0]["to"][0]["min"] = json!(3),
            "key" => v["rows"][0]["to"][0]["invented"] = json!(true),
            "side" => {
                v["rows"][0]["to"] =
                    json!({"status":"derived","reason":null,"ends":[],"observation":[]})
            }
            _ => unreachable!(),
        }
        // First establish the valid control so an old rules gate is not counted
        // as successful calibration of the payload validator.
        serde_json::from_value::<agency::Differences>(prefix_wire()).unwrap();
        assert!(
            serde_json::from_value::<agency::Differences>(v).is_err(),
            "{fault}"
        );
    }
}

#[test]
fn c4_whole_table_withdrawal_has_exact_owner_reason_with_and_without_call() {
    let a = report::check(input::DEGRADE);
    let z = report::check(&input::withdrawn());
    assert_eq!(
        json!(z.agency.as_ref().unwrap().degradation)["reason"],
        "multiple_case_interference"
    );
    for (from, to, both) in [(&a, &z, false), (&z, &a, false), (&z, &z, true)] {
        let v = compare(from, to);
        let rs = rows(&v);
        assert_eq!(rs.len(), 3);
        for item in ["normal", "degraded", "common_prefix"] {
            let r = rs.iter().find(|r| r["item"] == item).unwrap();
            assert_eq!(r["status"], "not_comparable");
            assert_eq!(
                r.get("call").cloned(),
                if both { None } else { Some(json!("Manual")) }
            );
            for (owner, rep) in [("from", from), ("to", to)] {
                if json!(rep.agency.as_ref().unwrap().degradation)["status"] == "not_derived" {
                    assert_eq!(
                        r[owner],
                        json!({"status":"not_derived","reason":format!("{owner}.agency.degradation:multiple_case_interference")})
                    );
                } else {
                    assert_eq!(r[owner], json!({"status":"derived","reason":null}));
                }
            }
        }
        serde_json::from_value::<agency::Differences>(v).unwrap();
    }
}
#[test]
fn c4_whole_table_reason_and_cell_keys_are_closed() {
    let z = report::check(&input::withdrawn());
    let good = compare(&z, &z);
    serde_json::from_value::<agency::Differences>(good.clone()).unwrap();
    for fault in ["reason", "key"] {
        let mut bad = good.clone();
        let r = bad["rows"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["viewpoint"] == "degradation" && r["item"] == "normal")
            .unwrap();
        if fault == "reason" {
            r["from"]["reason"] = json!("from.agency.degradation:invented_reason");
        } else {
            r["from"]["invented"] = json!(true);
        }
        assert!(
            serde_json::from_value::<agency::Differences>(bad).is_err(),
            "{fault}"
        );
    }
}

#[test]
fn c2_local_rules_and_bundle_accept_new_reject_old_and_one_sided_versions() {
    let mut v = json!(diff::diff(input::WORK, "From", "To"));
    assert_eq!(v["agency"]["differences"]["rules"], "agency-diff-p2-1");
    assert_eq!(
        v["ledger"]["derivation"]["bundle"],
        "relca-conclusions-p2-uncertainty-2"
    );
    serde_json::from_value::<diff::DiffReport>(v.clone()).unwrap();
    // Include every ledger owner, not merely the aggregate root.
    for p in [
        "/ledger/derivation/bundle",
        "/from/report/ledger/derivation/bundle",
        "/to/report/ledger/derivation/bundle",
    ] {
        assert_eq!(v.pointer(p).unwrap(), "relca-conclusions-p2-uncertainty-2");
        for old in [
            "relca-conclusions-p2-uncertainty-1",
            "relca-conclusions-p2-1",
            "future",
        ] {
            let mut bad = v.clone();
            *bad.pointer_mut(p).unwrap() = json!(old);
            assert!(
                serde_json::from_value::<diff::DiffReport>(bad).is_err(),
                "{p}/{old}"
            );
        }
    }
    for old in ["agency-diff-p1-1", "future"] {
        let mut bad = v.clone();
        bad["agency"]["differences"]["rules"] = json!(old);
        assert!(serde_json::from_value::<diff::DiffReport>(bad).is_err());
    }
    let components = v["ledger"]["derivation"]["components"]
        .as_array_mut()
        .unwrap();
    let c = components
        .iter_mut()
        .find(|c| c["version"] == "agency-diff-p2-1")
        .expect("agency component");
    c["version"] = json!("agency-diff-p1-1");
    assert!(
        serde_json::from_value::<diff::DiffReport>(v).is_err(),
        "component-only old version"
    );
}
#[test]
fn c2_new_local_generation_is_accepted_and_old_unknown_are_rejected() {
    let good = json!({"rules":"agency-diff-p2-1","status":"not_derived","reason":"agency_unavailable","unchecked":["fallback_path","workload_transfer","via_model_paths"]});
    assert_eq!(
        json!(serde_json::from_value::<agency::Differences>(good.clone()).unwrap()),
        good
    );
    for old in ["agency-diff-p1-1", "future"] {
        let mut v = good.clone();
        v["rules"] = json!(old);
        assert!(serde_json::from_value::<agency::Differences>(v).is_err());
    }
}
#[test]
fn c2_new_bundle_is_accepted_and_old_unknown_are_rejected() {
    let mut good = json!(report::check(input::SIMPLE));
    good["ledger"]["derivation"]["bundle"] = json!("relca-conclusions-p2-uncertainty-2");
    assert_eq!(
        json!(serde_json::from_value::<report::Report>(good.clone()).unwrap()),
        good
    );
    for old in [
        "relca-conclusions-p2-uncertainty-1",
        "relca-conclusions-p2-1",
        "future",
    ] {
        let mut bad = good.clone();
        bad["ledger"]["derivation"]["bundle"] = json!(old);
        assert!(serde_json::from_value::<report::Report>(bad).is_err());
    }
}

fn qualification(from: Value, to: Value) -> Value {
    let change = if from.is_null() {
        "added"
    } else if to.is_null() {
        "removed"
    } else {
        "modified"
    };
    json!({"kind":"qualification","target":"A.run","change":change,"from":from,"to":to,"reason":null,"loses":null})
}
#[test]
fn c6_qualification_canonical_set_controls() {
    for (a, z) in [
        (json!(["p", "q"]), json!(["p"])),
        (json!([]), Value::Null),
        (Value::Null, json!([])),
    ] {
        let v = qualification(a, z);
        assert_eq!(
            json!(serde_json::from_value::<diff::Difference>(v.clone()).unwrap()),
            v
        );
    }
}
#[test]
fn c6_qualification_rejects_unsorted_duplicate_and_empty_names_on_either_side() {
    for bad in [json!(["q", "p"]), json!(["p", "p"]), json!([""])] {
        for reverse in [false, true] {
            let v = if reverse {
                qualification(json!([]), bad.clone())
            } else {
                qualification(bad.clone(), json!([]))
            };
            assert!(
                serde_json::from_value::<diff::Difference>(v).is_err(),
                "{bad}/{reverse}"
            );
        }
    }
}
#[test]
fn c6_effective_omission_and_performance_object_roundtrip_without_normalization() {
    let v = json!(EffectiveInputs::from_model(&model(&input::point())).unwrap());
    let ops = v["capabilities"].as_array().unwrap();
    assert!(
        ops.iter()
            .any(|r| r["capability"] == "A.old" && r.get("performance").is_none())
    );
    assert!(ops.iter().any(|r| r["capability"] == "A.run"
        && r["performance"].is_object()
        && r["kernel"].is_null()));
    assert_eq!(
        json!(serde_json::from_value::<EffectiveInputs>(v.clone()).unwrap()),
        v
    );
}
#[test]
fn c6_effective_explicit_null_is_not_omission() {
    let mut v = json!(EffectiveInputs::from_model(&model(&input::point())).unwrap());
    let row = v["capabilities"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["capability"] == "A.old")
        .unwrap();
    assert!(row.get("performance").is_none());
    row["performance"] = Value::Null;
    assert!(serde_json::from_value::<EffectiveInputs>(v).is_err());
}
#[test]
fn c6_effective_rejects_dual_source_unknown_key_and_missing_row_kernel() {
    let good = json!(EffectiveInputs::from_model(&model(&input::point())).unwrap());
    serde_json::from_value::<EffectiveInputs>(good.clone()).unwrap();
    let old = good["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["capability"] == "A.old")
        .unwrap()["kernel"]
        .clone();
    for fault in ["dual", "key", "kernel"] {
        let mut v = good.clone();
        let row = v["capabilities"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["capability"] == "A.run")
            .unwrap();
        match fault {
            "dual" => row["kernel"] = old.clone(),
            "key" => row["performance"]["extra"] = json!(true),
            "kernel" => {
                row["performance"]["rows"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("kernel");
            }
            _ => unreachable!(),
        }
        assert!(
            serde_json::from_value::<EffectiveInputs>(v).is_err(),
            "{fault}"
        );
    }
}

#[test]
fn q9_nc_legal_controls_keep_distinct_filing_and_response_knowledge() {
    for kind in [
        "known",
        "unresolved",
        "target_unknown",
        "choice_unknown",
        "zero",
        "aborted",
        "failed",
    ] {
        contests::decode(contests::wire(kind), true);
    }
    for kind in ["known", "unresolved"] {
        let mut v = contests::wire(kind);
        v["rows"][0]["filing_fraction"]["value"] = Value::Null;
        v["rows"][0]["filing_fraction"]["reason"] = json!("not_configured");
        if kind == "known" {
            for k in ["service_time", "queue_wait", "response_elapsed"] {
                v["rows"][0][k] = contests::absent("not_configured");
            }
        }
        contests::decode(v, true);
    }
}
#[test]
fn q9_nc_cannot_mask_unknown_target_or_choice_fraction() {
    for kind in ["target_unknown", "choice_unknown"] {
        let mut v = contests::wire(kind);
        contests::decode(v.clone(), true);
        v["rows"][0]["filing_fraction"]["reason"] = json!("not_configured");
        contests::decode(v, false);
    }
}
#[test]
fn q9_nc_cannot_mask_unresolved_response_times() {
    for kind in ["target_unknown", "choice_unknown", "unresolved"] {
        let mut v = contests::wire(kind);
        contests::decode(v.clone(), true);
        // All three reason fields form one equality class. Change that class,
        // not a count, sample value or partition.
        for k in ["service_time", "queue_wait", "response_elapsed"] {
            v["rows"][0][k]["reason"] = json!("not_configured");
        }
        contests::decode(v, false);
    }
}
#[test]
fn q9_nc_cannot_replace_known_no_samples() {
    let mut v = contests::wire("zero");
    contests::decode(v.clone(), true);
    v["rows"][0]["filing_fraction"]["reason"] = json!("not_configured");
    contests::decode(v, false);
    for kind in ["zero", "failed"] {
        let mut v = contests::wire(kind);
        contests::decode(v.clone(), true);
        for k in ["service_time", "queue_wait", "response_elapsed"] {
            v["rows"][0][k]["reason"] = json!("not_configured");
        }
        contests::decode(v, false);
    }
}
fn known_failure(reference: bool) {
    let m = model(input::KNOWN_FAILURE);
    let o = if reference {
        let r = core::reference::simulate(&m);
        assert_eq!(r.stop, core::reference::StopReason::Complete);
        r.outcome
    } else {
        core::engine::simulate(&m)
    };
    assert_eq!(o.completed_replications, 2, "{:?}", o.diagnostics);
    assert_eq!(o.truncated_replications, Some(0));
    assert_eq!(o.range_stops.len(), 2);
    for (i, (rep, vs)) in o.range_stops.iter().enumerate() {
        assert_eq!(*rep, i as u64 + 1);
        assert_eq!(vs.len(), 1);
        assert_eq!(vs[0].attempted, "2");
        assert_eq!(vs[0].time, 3);
    }
    for r in &o.replications {
        assert!(r.cut.is_none());
        assert_eq!(r.contests.len(), 2);
        let rejected = &r.contests[0];
        let running = &r.contests[1];
        for c in [rejected, running] {
            assert_eq!(c.target_at, Some(1));
            assert_eq!(c.filed, Some(true));
            assert!(c.response_at.is_none());
        }
        assert!(
            rejected.failed,
            "the already rejected route remains known failed"
        );
        assert_eq!(rejected.closed_at, Some(1), "not the later range time 3");
        assert_eq!(
            rejected.started_at, None,
            "requires refusal consumes no service"
        );
        assert!(!running.failed);
        assert_eq!(running.closed_at, None);
        assert_eq!(running.started_at, Some(1));
    }
}
#[test]
fn q9_known_failure_before_range_engine() {
    known_failure(false);
}
#[test]
fn q9_known_failure_before_range_reference() {
    known_failure(true);
}
#[test]
fn q9_known_failure_report_preserves_core_witness_and_partition() {
    let v = json!(report::simulate(input::KNOWN_FAILURE));
    assert_eq!(v["verdict"], "violated");
    assert_eq!(v["simulation"]["range_stopped_replications"], 2);
    let rs = v["metrics"]["contests"]["rows"].as_array().unwrap();
    assert_eq!(rs.len(), 2);
    for (i, failed, unresolved) in [(0, 2, 0), (1, 0, 2)] {
        let r = &rs[i];
        assert_eq!(r["filed"], 2);
        assert_eq!(r["route_failed"], failed);
        assert_eq!(r["unresolved"], unresolved);
        assert_eq!(r["filing_fraction"]["value"], "1");
        for k in ["service_time", "queue_wait", "response_elapsed"] {
            assert_eq!(
                r[k],
                contests::absent(if i == 0 { "no_samples" } else { "unfinished" })
            );
        }
    }
}
fn management(reference: bool) {
    for renamed in [false, true] {
        let source = if renamed {
            input::MANAGEMENT
                .replace("[0min,2min]", "[0min,4min]")
                .replace("Judge", "Decider")
                .replace("Reply", "Responder")
        } else {
            input::MANAGEMENT.replace("[0min,2min]", "[0min,4min]")
        };
        let m = model(&source);
        let o = if reference {
            core::reference::simulate(&m).outcome
        } else {
            core::engine::simulate(&m)
        };
        assert_eq!(o.completed_replications, 2, "{:?}", o.diagnostics);
        assert_eq!(o.truncated_replications, Some(0));
        for r in &o.replications {
            assert!(r.cut.is_none());
            assert_eq!(r.contests.len(), 2);
            assert_eq!(r.cases.len(), 2);
            for (c, target, response) in [(&r.contests[0], 1, 3), (&r.contests[1], 5, 7)] {
                assert_eq!(c.target_at, Some(target));
                assert_eq!(c.started_at, Some(target));
                assert_eq!(c.response_at, Some(response));
                assert_eq!(c.closed_at, Some(response));
                assert!(!c.failed);
            }
            assert_eq!(
                r.draws.len(),
                2,
                "one occurrence draw per case, no extra stabilization draw"
            );
            for (case, draw) in r.draws.iter().enumerate() {
                assert_eq!(
                    draw.key,
                    format!(
                        "relca-sim-v1|seed=1|rep={}|case={}|site=contest.Flow.clause1.occurrence|n=1",
                        r.replication,
                        case + 1
                    )
                );
            }
            for c in &r.cases {
                assert_eq!(c.terminal.as_ref().unwrap().end.as_deref(), Some("done"));
            }
        }
    }
}
#[test]
fn q9_narrow_management_nonoverlap_engine() {
    management(false);
}
#[test]
fn q9_narrow_management_nonoverlap_reference() {
    management(true);
}
#[test]
fn q9_management_simultaneous_finishes_are_not_randomly_ordered() {
    let m = model(input::MANAGEMENT);
    for o in [
        core::engine::simulate(&m),
        core::reference::simulate(&m).outcome,
    ] {
        assert_eq!(o.completed_replications, 0);
        assert_eq!(o.truncated_replications, None);
        assert!(o.diagnostics.iter().any(|d| d.code == "U-SEM-0"
            && d.message.contains("time 3,")
            && d.message.contains("Flow.a,Flow.contest:1")
            && d.message.contains("unresolved nondeterminism")));
    }
}
#[test]
fn q9_management_legacy_encounter_precedes_other_case_live_cut() {
    for name in ["legacy-live", "legacy-live-control"] {
        let m = model(&prior_q9::source(name));
        for o in [
            core::engine::simulate(&m),
            core::reference::simulate(&m).outcome,
        ] {
            assert_eq!(o.completed_replications, 0);
            assert_eq!(o.truncated_replications, None);
            assert!(o.diagnostics.iter().any(|d| d.code == "U-SEM-0"
                && d.message.contains("time 3,")
                && d.message.contains("unresolved nondeterminism")));
        }
    }
}
