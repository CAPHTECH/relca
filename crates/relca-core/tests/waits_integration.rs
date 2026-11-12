//! Integrated PRE-A7 cross-feature obligations. See waits_integration.md.
#[path = "support/waits_integration_inputs.rs"]
mod i;
#[path = "support/waits_refusal_private.rs"]
mod refusal_private;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn model(s: &str, config: Option<&str>, scenario: Option<&str>) -> relca_core::ir::Model {
    let a = syntax::parse(s).unwrap_or_else(|e| panic!("parse substrate: {e:?}"));
    lower::lower_with_selection(&a, config, scenario)
        .unwrap_or_else(|e| panic!("lower substrate: {e:?}"))
}
fn independent(m: &relca_core::ir::Model, reference_side: bool, sim: bool) -> Value {
    if sim {
        let o = if reference_side {
            let r = reference::simulate(m);
            assert_eq!(
                r.stop,
                reference::StopReason::Complete,
                "zero reference exclusions"
            );
            r.outcome
        } else {
            engine::simulate(m)
        };
        assert_eq!(o.replications.len(), 4);
        for row in &o.replications {
            assert_eq!(
                row.metrics, o.replications[0].metrics,
                "deterministic replication oracle"
            );
        }
        json!({"quality":o.quality,"metrics":o.replications[0].metrics})
    } else {
        let o = if reference_side {
            let r = reference::explore(m);
            assert_eq!(
                r.stop,
                reference::StopReason::Complete,
                "zero reference exclusions"
            );
            r.outcome
        } else {
            engine::explore(m)
        };
        json!({"quality":o.quality,"metrics":o.metrics,"truncated":o.truncated,"diagnostics":o.diagnostics,
            "terminals":format!("{:?}",o.terminals),"properties":format!("{:?}",o.properties)})
    }
}
macro_rules! pair {
    ($e:ident,$r:ident,$body:expr) => {
        #[test]
        fn $e() {
            ($body)(false);
        }
        #[test]
        fn $r() {
            ($body)(true);
        }
    };
}

#[test]
fn closed_wait_inventory_matches_all_real_directories_and_both_tables() {
    let names: Vec<String> =
        serde_json::from_str(include_str!("support/waits_names.json")).unwrap();
    let rust: &[&str] = include!("support/waits_names.rs");
    assert_eq!(names, rust);
    assert_eq!(names.len(), 43);
    let mut actual: Vec<_> =
        std::fs::read_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.join("model.relca").is_file())
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .filter(|n: &String| n.starts_with("wait-"))
            .collect();
    actual.sort();
    assert_eq!(
        names, actual,
        "prefix detects unknowns, never grants an exemption"
    );
}

fn hq(reference_side: bool, clock: bool, sim: bool) {
    let s = i::hq(clock, sim);
    let a = independent(&model(&s, None, Some("Day")), reference_side, sim);
    let b = independent(
        &model(&i::masked(&s), None, Some("Day")),
        reference_side,
        sim,
    );
    assert_eq!(
        a["quality"], b["quality"],
        "waiting must not change quality"
    );
    let q = i::ratio(&a);
    let stopped = clock || !sim;
    let property = a["quality"]["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Done")
        .unwrap();
    if sim {
        assert_eq!(
            property["expected_count"]["mean"],
            if stopped { "0" } else { "2" }
        );
        assert_eq!(
            property["unknown_count"]["mean"],
            if stopped { "2" } else { "0" }
        );
        assert_eq!(property["expected_count"]["samples"], 4);
    } else {
        assert_eq!(property["expected_count"]["lower"], "0");
        assert_eq!(property["expected_count"]["upper"], "2");
        assert_eq!(
            property["expected_count"]["reason"],
            if clock { "truncated" } else { "undetermined" }
        );
        assert_eq!(property["cases"].as_array().unwrap().len(), 2);
    }
    if sim && !clock {
        assert_eq!(q["value"]["point"], "1");
    } else {
        assert_eq!(
            q["value"]["reason"],
            if clock && !sim {
                "truncated"
            } else {
                "undetermined"
            }
        );
        assert_eq!(
            q["bounds_with_undetermined"],
            json!({"lower":"0","upper":"1"})
        );
    }
    let m = &a["metrics"];
    assert_eq!(
        m["status"],
        if clock && !sim {
            "truncated"
        } else {
            "complete"
        }
    );
    if !sim && !clock {
        assert_eq!(
            m["cases"][0]["dwell"]["breakdown"]["in_service"],
            i::range("3")
        );
        assert_eq!(
            m["cases"][1]["dwell"]["breakdown"]["queue"]["actor_capacity"],
            i::range("3")
        );
        assert_eq!(
            m["actors"][0]["queue"]["wait_total"],
            json!({"stats":i::range("3"),"bound":"lower"})
        );
        assert_eq!(
            m["actors"][0]["queue"]["length_time_average"],
            json!({"stats":i::range("1"),"bound":"lower"})
        );
    } else if clock && !sim {
        fn truncated(v: &Value) {
            match v {
                Value::Object(o) if o.contains_key("min") && o.contains_key("reason") => {
                    assert_eq!(*v, json!({"min":null,"max":null,"reason":"truncated"}))
                }
                Value::Object(o) => {
                    assert!(!o.contains_key("bound"));
                    for x in o.values() {
                        truncated(x);
                    }
                }
                Value::Array(xs) => {
                    for x in xs {
                        truncated(x);
                    }
                }
                _ => {}
            }
        }
        truncated(&m["overall"]["queue_wait_observed"]);
        for row in m["cases"].as_array().unwrap() {
            assert!(row.get("dwell").is_none());
            for t in row["terminals"].as_array().unwrap() {
                truncated(&t["breakdown"]);
            }
            for t in row["tasks"].as_array().unwrap() {
                truncated(&t["wait_causes"]);
                truncated(&t["predecessor_wait"]);
            }
            truncated(&row["joins"]);
        }
        for actor in m["actors"].as_array().unwrap() {
            truncated(&actor["queue"]);
        }
    } else {
        let n = if stopped { "3" } else { "5" };
        let avg = if stopped { "1" } else { "1/2" };
        assert_eq!(m["actors"][0]["queue"]["wait_total"]["stats"], i::range(n));
        assert_eq!(
            m["actors"][0]["queue"]["length_time_average"]["stats"],
            i::range(avg)
        );
    }
}
pair!(b6_horizon_engine, b6_horizon_reference, |r| hq(
    r, false, false
));
pair!(b6_clock_engine, b6_clock_reference, |r| hq(r, true, false));
pair!(b6_sample_engine, b6_sample_reference, |r| hq(
    r, false, true
));
pair!(b6_sample_horizon_engine, b6_sample_horizon_reference, |r| {
    hq(r, true, true)
});

#[test]
fn f36_normal_exhaustive_quality_rows_precede_waiting() {
    i::assumption_order(
        &json!(report::check_with_selection(
            &i::hq(false, false),
            None,
            Some("Day")
        )),
        false,
    );
}
#[test]
fn f36_normal_simulation_quality_rows_precede_waiting() {
    i::assumption_order(
        &json!(report::simulate_with_selection(
            &i::hq(false, true),
            None,
            Some("Day")
        )),
        true,
    );
}
#[test]
fn f36_early_refusal_keeps_waiting_after_four_quality_rows() {
    let v = json!(report::simulate_with_selection(
        &i::refusal(),
        None,
        Some("Day")
    ));
    assert_eq!(v["verdict"], "unsupported");
    i::assumption_order(&v, true);
}

fn missing_simulation_clause_refusal(day: bool) {
    let s = if day {
        i::hq(false, false)
    } else {
        i::quality(&i::fixture("wait-intentional"))
    };
    let v = json!(report::simulate_with_selection(
        &s,
        None,
        day.then_some("Day")
    ));
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-SIM-0")
    );
    i::missing_simulation_disclosure(&v);
}
#[test]
fn f36_missing_simulation_clause_refusal_without_scenario_has_no_waiting() {
    missing_simulation_clause_refusal(false);
}
#[test]
fn f36_missing_simulation_clause_refusal_with_scenario_has_no_waiting() {
    missing_simulation_clause_refusal(true);
}

fn rule(reference_side: bool) {
    let s = i::rule();
    for (cfg, lead, intentional) in [("Before", "10", "2"), ("After", "8", "0")] {
        let m = model(&s, Some(cfg), None);
        let actual = independent(&m, reference_side, false);
        let old = independent(
            &model(&i::masked(&s), Some(cfg), None),
            reference_side,
            false,
        );
        for key in ["terminals", "properties", "diagnostics", "truncated"] {
            assert_eq!(actual[key], old[key], "{cfg}/{key}");
        }
        let t = &actual["metrics"]["cases"][0]["terminals"][0];
        assert_eq!(t["lead_time"], i::range(lead));
        assert_eq!(t["breakdown"]["in_service"], i::range("8"));
        assert_eq!(t["breakdown"]["intentional"], i::range(intentional));
        assert_eq!(
            actual["metrics"]["overall"]["queue_wait_observed"],
            i::range("0")
        );
        let people = actual["metrics"]["people"].as_array().unwrap();
        assert_eq!(people.len(), 2, "declared people inventory is preserved");
        assert_eq!(
            people.iter().find(|p| p["person"] == "p").unwrap()["work"],
            i::range("8")
        );
        assert_eq!(
            people.iter().find(|p| p["person"] == "cfo").unwrap()["work"],
            i::range("0"),
            "Rule grant cfo is not a dispatch candidate"
        );
        let a = json!(report::check_configuration(&s, cfg));
        let b = json!(report::check_configuration(&i::masked(&s), cfg));
        assert_eq!(i::disclosure(&a), i::disclosure(&b));
        assert_eq!(a["actors"], b["actors"]);
        assert_eq!(a["authorities"], b["authorities"]);
    }
}
pair!(a6_rule_engine, a6_rule_reference, rule);

fn crossings(r: bool) {
    for (name, ends) in [
        (
            "rec-approval",
            vec![("posted", "4", "7"), ("returned", "3", "6")],
        ),
        (
            "agy2-jue",
            vec![("posted", "3", "3"), ("returned", "2", "2")],
        ),
        (
            "cf-version-atomic",
            vec![("done", "1", "1"), ("conflict_end", "1", "1")],
        ),
    ] {
        let s = i::crossing(name);
        let a = independent(&model(&s, None, None), r, false);
        let b = independent(&model(&i::masked(&s), None, None), r, false);
        for (end, min, max) in ends {
            i::partition_equals_service(&a["metrics"], end, min, max);
        }
        for key in [
            "quality",
            "diagnostics",
            "terminals",
            "properties",
            "truncated",
        ] {
            assert_eq!(a[key], b[key], "{name}/{key}");
        }
        let a = json!(report::check(&s));
        let b = json!(report::check(&i::masked(&s)));
        assert_eq!(
            i::disclosure(&a),
            i::disclosure(&b),
            "{name}: new inventory cannot change Agency/obligations"
        );
    }
    let s = i::crossing("lim-overflow");
    let a = independent(&model(&s, None, None), r, false);
    assert_eq!(a["metrics"]["cases"].as_array().unwrap().len(), 3);
    i::partition_equals_service(&a["metrics"], "done", "2", "2");
    assert_eq!(
        a["metrics"]["overall"]["queue_wait_observed"],
        i::range("0"),
        "Overflow is not an admitted queued Task"
    );
}
pair!(
    record_conflict_c5_overflow_engine,
    record_conflict_c5_overflow_reference,
    crossings
);

#[test]
fn t17_waiting_tables_stays_last_and_quality_rows_do_not_enter_comparison() {
    for sim in [false, true] {
        let s = i::quality(&i::fixture("wait-compare-diff"));
        let s = if sim {
            i::append(&s, "simulation replications 4 seed 1 confidence 0.5")
        } else {
            s
        };
        let analysis = if sim {
            relca_core::compare::Analysis::Simulation
        } else {
            relca_core::compare::Analysis::Exhaustive
        };
        let a = json!(relca_core::compare::compare(
            &s,
            &["Left", "Right"],
            analysis
        ));
        let b = json!(relca_core::compare::compare(
            &i::masked(&s),
            &["Left", "Right"],
            analysis
        ));
        assert_eq!(a["tables"], b["tables"]);
        let mut words = b["unchecked"].as_array().unwrap().clone();
        words.push(json!("waiting_tables"));
        assert_eq!(a["unchecked"], json!(words));
    }
}

fn a7fix_revocation(quality_on: bool, rule_on: bool, revocable: bool, waits: bool) {
    let s = i::revocation(quality_on, rule_on, revocable);
    let s = if waits { s } else { i::masked(&s) };
    // Prove the refusal substrate is valid before invoking the missing-clause path.
    model(&s, None, None);
    let v = json!(report::simulate(&s));
    i::refusal_status(&v, "invalid", "E-SIM-0", "explicit simulation clause");
    i::legacy_revocation(&v, revocable);
    if waits {
        let control = json!(report::simulate(&i::masked(&s)));
        i::legacy_refusal_equal(&v, &control);
    }
    i::no_metric_disclosure(&v, true, quality_on, rule_on);
}
macro_rules! revocation_cases {
    ($on:ident, $off:ident, $q:expr, $rule:expr, $rev:expr) => {
        #[test]
        fn $on() {
            a7fix_revocation($q, $rule, $rev, true);
        }
        #[test]
        fn $off() {
            a7fix_revocation($q, $rule, $rev, false);
        }
    };
}
revocation_cases!(
    a7fix_revocable_no_quality_waits,
    a7fix_revocable_no_quality_control,
    false,
    false,
    true
);
revocation_cases!(
    a7fix_revocable_quality_waits,
    a7fix_revocable_quality_control,
    true,
    false,
    true
);
revocation_cases!(
    a7fix_revocable_rule_quality_waits,
    a7fix_revocable_rule_quality_control,
    true,
    true,
    true
);
revocation_cases!(
    a7fix_no_revoker_no_quality_waits,
    a7fix_no_revoker_no_quality_control,
    false,
    false,
    false
);
revocation_cases!(
    a7fix_no_revoker_quality_waits,
    a7fix_no_revoker_quality_control,
    true,
    false,
    false
);
revocation_cases!(
    a7fix_no_revoker_rule_quality_waits,
    a7fix_no_revoker_rule_quality_control,
    true,
    true,
    false
);

fn a7fix_completion(question: &str, rule_on: bool, waits: bool) {
    let s = i::completion(question, rule_on);
    let s = if waits { s } else { i::masked(&s) };
    model(&s, None, Some("Day"));
    let v = json!(report::simulate_with_selection(&s, None, Some("Day")));
    i::refusal_status(&v, "unsupported", "U-SEM-0", "structural completion");
    i::no_metric_disclosure(&v, true, true, rule_on);
}
macro_rules! completion_cases {
    ($on:ident, $off:ident, $question:expr, $rule:expr) => {
        #[test]
        fn $on() {
            a7fix_completion($question, $rule, true);
        }
        #[test]
        fn $off() {
            a7fix_completion($question, $rule, false);
        }
    };
}
completion_cases!(
    a7fix_terminating_waits,
    a7fix_terminating_control,
    "terminating",
    false
);
completion_cases!(
    a7fix_recoverable_waits,
    a7fix_recoverable_control,
    "recoverable",
    false
);
completion_cases!(
    a7fix_almost_sure_waits,
    a7fix_almost_sure_control,
    "almost_sure",
    false
);
completion_cases!(
    a7fix_terminating_rule_waits,
    a7fix_terminating_rule_control,
    "terminating",
    true
);

fn a7fix_rule_order(sim: bool) {
    for cfg in ["Before", "After"] {
        let s = i::rule_quality(sim);
        for waits in [false, true] {
            let s = if waits { s.clone() } else { i::masked(&s) };
            model(&s, Some(cfg), None);
            let v = if sim {
                json!(report::simulate_configuration(&s, cfg))
            } else {
                json!(report::check_configuration(&s, cfg))
            };
            assert!(v.get("metrics").is_some());
            i::completed_rule_quality(&v, sim);
            i::quality_block(&v, sim, true);
            if waits {
                i::assumption_order(&v, sim);
            } else {
                assert!(!i::fields(&v).contains(&"metrics.waiting"));
            }
        }
    }
}
#[test]
fn a7fix_rule_quality_waits_exhaustive_order() {
    a7fix_rule_order(false);
}
#[test]
fn a7fix_rule_quality_waits_simulation_order() {
    a7fix_rule_order(true);
}

#[test]
fn a7fix_diff_unchecked_literal_inventory() {
    for contest in [false, true] {
        for mismatch in [false, true] {
            let s = i::diff_input(contest, mismatch);
            for waits in [false, true] {
                let s = if waits { s.clone() } else { i::masked(&s) };
                model(&s, Some("Left"), None);
                model(&s, Some("Right"), None);
                let v = json!(relca_core::diff::diff(&s, "Left", "Right"));
                assert_eq!(v["verdict"], if mismatch { "invalid" } else { "satisfied" });
                assert_eq!(
                    v["unchecked"],
                    i::diff_unchecked(contest, mismatch, waits),
                    "contest={contest} mismatch={mismatch} waits={waits}"
                );
            }
        }
    }
}

#[test]
fn a7fix_diff_aborted_legacy_unchecked_places_waiting_before_terminals() {
    let s = i::diff_aborted_input();
    for waits in [false, true] {
        let s = if waits { s.clone() } else { i::masked(&s) };
        model(&s, Some("Left"), None);
        model(&s, Some("Right"), None);
        let v = json!(relca_core::diff::diff(&s, "Left", "Right"));
        assert_eq!(v["verdict"], "unsupported");
        for side in ["from", "to"] {
            assert_eq!(v[side]["report"]["metrics"]["status"], "aborted");
            assert!(v[side]["report"]["bounds"]["truncated"].is_null());
            assert!(
                v[side]["report"]["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "U-SEM-0")
            );
        }
        assert_eq!(v["unchecked"], i::diff_aborted_unchecked(waits));
    }
}

#[test]
fn a7fix_frontend_refusals_never_publish_quality_or_waiting() {
    for sim in [false, true] {
        let s = i::hq(false, sim);
        for waits in [false, true] {
            let s = if waits { s.clone() } else { i::masked(&s) };
            for parse_error in [false, true] {
                let bad = if parse_error {
                    format!("{s}\n)")
                } else {
                    i::quality(&s)
                }; // duplicate quality definition, not executable IR
                let v = if sim {
                    json!(report::simulate_with_selection(&bad, None, Some("Day")))
                } else {
                    json!(report::check_with_selection(&bad, None, Some("Day")))
                };
                assert!(
                    v["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d["severity"] == "error")
                );
                assert!(v.get("quality").is_none());
                i::no_metric_disclosure(&v, sim, false, false);
            }
        }
    }
    let v = json!(report::simulation_input_failure("input unavailable"));
    i::refusal_status(&v, "invalid", "E-SYN-0", "input unavailable");
    i::no_metric_disclosure(&v, true, false, false);
}

#[test]
fn a7fix_metrics_undeclared_normal_and_runtime_refusal_have_no_waiting() {
    for (sim, refuse) in [(false, false), (true, false), (true, true)] {
        let s = if refuse {
            i::refusal()
        } else {
            i::hq(false, sim)
        };
        let s = s.replace(
            "metrics { window 20min; currency \"JPY\"; breakdown waits }",
            "",
        );
        assert!(!s.contains("metrics {"));
        let v = if sim {
            json!(report::simulate_with_selection(&s, None, Some("Day")))
        } else {
            json!(report::check_with_selection(&s, None, Some("Day")))
        };
        i::no_metric_disclosure(&v, sim, true, false);
    }
}

#[test]
fn a7fix_private_refusal_substrates_lower_but_are_not_claimed_connected() {
    let probes = refusal_private::probes();
    assert_eq!(
        probes.len(),
        112,
        "7 routes x quality on/off x Rule on/off x waits on/off x revocable on/off"
    );
    for p in probes {
        let m = model(&p.source, None, None);
        assert_eq!(m.quality.is_some(), p.quality);
        assert_eq!(m.metrics.as_ref().unwrap().breakdown_waits, p.waits);
        assert_eq!(m.actors.iter().any(|a| a.metadata.rule.is_some()), p.rule);
    }
    // run() is intentionally NOT invoked with a stand-in producer here.
}

#[test]
fn a7fix_structural_refusal_must_not_leave_waiting_before_quality() {
    let v = json!(report::simulate_with_selection(
        &i::completion("terminating", false),
        None,
        Some("Day")
    ));
    i::refusal_status(&v, "unsupported", "U-SEM-0", "structural completion");
    if i::fields(&v).contains(&"metrics.waiting") {
        i::assumption_order(&v, true);
    }
    i::no_metric_disclosure(&v, true, true, false);
}

#[test]
fn a7fix_selected_configuration_frontend_returns_have_no_quality_or_waiting() {
    for sim in [false, true] {
        let s = i::rule_quality(sim);
        for waits in [false, true] {
            let s = if waits { s.clone() } else { i::masked(&s) };
            for parse_error in [false, true] {
                let s = if parse_error {
                    format!("{s}\n)")
                } else {
                    i::quality(&s)
                };
                let v = if sim {
                    json!(report::simulate_configuration(&s, "Before"))
                } else {
                    json!(report::check_configuration(&s, "Before"))
                };
                assert!(
                    v["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d["severity"] == "error")
                );
                assert!(v.get("quality").is_none());
                i::no_metric_disclosure(&v, sim, false, false);
            }
        }
    }
}

#[test]
fn a7fix_missing_clause_rejected_lowering_copies_no_rows() {
    let s = i::quality(&i::revocation(true, true, true));
    for waits in [false, true] {
        let s = if waits { s.clone() } else { i::masked(&s) };
        let a = syntax::parse(&s).unwrap();
        assert!(
            lower::lower(&a).is_err(),
            "duplicate quality must invalidate lowering"
        );
        let v = json!(report::simulate(&s));
        i::refusal_status(&v, "invalid", "E-SIM-0", "explicit simulation clause");
        assert!(v.get("quality").is_none());
        assert!(i::fields(&v).is_empty());
        i::no_metric_disclosure(&v, true, false, false);
    }
}

#[test]
fn a7fix_missing_clause_scenario_keeps_rule_after_quality_without_waiting() {
    let s = i::missing_clause_rule_scenario();
    for waits in [false, true] {
        let s = if waits { s.clone() } else { i::masked(&s) };
        model(&s, None, Some("Day"));
        let v = json!(report::simulate_with_selection(&s, None, Some("Day")));
        i::refusal_status(&v, "invalid", "E-SIM-0", "explicit simulation clause");
        i::no_metric_disclosure(&v, true, true, true);
    }
}
