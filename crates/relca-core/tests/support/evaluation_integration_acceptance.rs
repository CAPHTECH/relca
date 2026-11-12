use serde_json::{Value, json};
#[path = "evaluation_integration_inputs.rs"]
pub(super) mod i;
#[path = "quality_integration_inputs.rs"]
pub(super) mod q;
#[path = "evaluation_quality_table_oracle.rs"]
pub(super) mod quality_table;
#[path = "rule_integration_inputs.rs"]
mod rule;

#[derive(Clone, Copy)]
pub(super) enum Op<'a> {
    Compare(&'a [&'a str], bool),
    Single(&'a str, bool),
}

fn compared(s: &str, plans: &[&str], sampled: bool, day: bool) -> Value {
    super::run(s, Op::Compare(plans, sampled), day)
}
fn no_errors(v: &Value) {
    assert_eq!(v["diagnostics"], json!([]), "{v}");
    assert!(v["evaluation"].is_object(), "{v}");
}
fn cells(v: &Value) -> &[Value] {
    v["evaluation"]["hard"][0]["values"].as_array().unwrap()
}
fn statuses(v: &Value, status: &str, verify: &str) {
    for p in v["evaluation"]["plans"].as_array().unwrap() {
        assert_eq!(p["status"], status, "{p}");
        assert_eq!(p["verify_verdict"], verify, "{p}");
    }
}
fn transport(s: &str, v: &Value, sampled: bool, day: bool) {
    for p in v["plans"].as_array().unwrap() {
        let one = super::run(
            s,
            Op::Single(p["configuration"].as_str().unwrap(), sampled),
            day,
        );
        assert_eq!(
            p["report"], one,
            "single-command report must remain unchanged"
        );
    }
}

#[test]
fn d01_complete_and_horizon_conservative_endpoints() {
    for (h, bound, root, hard, status) in [
        (30, "0.3", "satisfied", "satisfied", "admissible"),
        (14, "0.3", "inconclusive", "inconclusive", "undetermined"),
        (14, "0.5", "inconclusive", "satisfied", "undetermined"),
        (14, "0.01", "violated", "violated", "excluded"),
    ] {
        let s = i::horizon(h, bound, false);
        let v = compared(&s, &["Check", "Fast"], false, true);
        no_errors(&v);
        assert_eq!(v["verdict"], root);
        if h == 14 {
            assert_eq!(v["tables"]["quality"], quality_table::horizon14());
        }
        statuses(
            &v,
            status,
            if h == 30 { "satisfied" } else { "inconclusive" },
        );
        for c in cells(&v) {
            assert_eq!(c["verdict"], hard);
            assert_eq!(
                c["interval"],
                if h == 30 {
                    i::interval("1/50", "1/50")
                } else {
                    i::interval("1/75", "26/75")
                }
            );
            assert_eq!(
                c["reason"],
                if h == 30 {
                    Value::Null
                } else {
                    json!("undetermined")
                }
            );
            assert_eq!(
                c["basis"],
                if h == 30 {
                    "exact"
                } else {
                    "with_undetermined"
                }
            );
        }
        transport(&s, &v, false, true);
    }
}

#[test]
fn d01_objective_does_not_borrow_hard_unknown_bounds() {
    let s = i::horizon(14, "0.3", true);
    let v = compared(&s, &["Check", "Fast"], false, true);
    no_errors(&v);
    for c in v["evaluation"]["objectives"][0]["values"]
        .as_array()
        .unwrap()
    {
        assert!(c["lower"].is_null() && c["upper"].is_null());
        assert_eq!(c["reason"], "undetermined");
    }
    assert_eq!(v["evaluation"]["pareto"]["reason"], "no_admissible_plans");
    assert_eq!(v["evaluation"]["pareto"]["among"], json!([]));
}

#[test]
fn d01_computational_clock_wins_over_declared_later_horizon() {
    let s = i::edit(&i::horizon(30, "0.3", false), "clock 30min", "clock 14min");
    let v = compared(&s, &["Check", "Fast"], false, true);
    no_errors(&v);
    for c in cells(&v) {
        assert_eq!(c["reason"], "truncated");
        assert_eq!(c["interval"], i::interval("1/75", "26/75"));
    }
    for p in v["plans"].as_array().unwrap() {
        assert_eq!(p["report"]["bounds"]["truncated"], true);
    }
}

#[test]
fn d02_simulation_unknown_enters_cp_upper_tail() {
    for h in [1, 5] {
        let s = i::simulation(h, false);
        let v = compared(&s, &["Good", "Bad"], true, true);
        no_errors(&v);
        assert_eq!(
            v["verdict"],
            if h == 1 { "inconclusive" } else { "satisfied" }
        );
        assert_eq!(
            v["evaluation"]["statistics"]["multiplicity"],
            json!({"method":"bonferroni","m":2,"alpha":"1/8"})
        );
        for c in cells(&v) {
            assert_eq!(
                c["interval"],
                i::interval("0", if h == 1 { "1" } else { "1/2" })
            );
            assert_eq!(c["point"], if h == 1 { Value::Null } else { json!("0") });
            assert_eq!(
                c["reason"],
                if h == 1 {
                    json!("undetermined")
                } else {
                    Value::Null
                }
            );
        }
        for p in v["plans"].as_array().unwrap() {
            let c = &p["report"]["quality"]["properties"][0]["cases"][0];
            assert_eq!(c["hits"], 0);
            assert_eq!(c["unknown"], if h == 1 { 3 } else { 0 });
            assert_eq!(c["trials"], 3);
            assert_eq!(p["report"]["simulation"]["truncated_replications"], 0);
        }
        transport(&s, &v, true, true);
    }
}

#[test]
fn d02_unknown_denominator_keeps_source_reason_and_doubled_m() {
    let s = i::simulation(1, true);
    let v = compared(&s, &["Good", "Bad"], true, true);
    no_errors(&v);
    assert_eq!(
        v["evaluation"]["statistics"]["multiplicity"],
        json!({"method":"bonferroni","m":4,"alpha":"1/16"})
    );
    for c in cells(&v) {
        assert!(c["interval"].is_null());
        assert_eq!(c["reason"], "denominator_may_be_zero");
        assert_eq!(c["source_reason"], "undetermined");
        assert_eq!(c["verdict"], "inconclusive");
    }
}

#[test]
fn d02_scenario_case_and_row_multiplicity_are_not_deduplicated() {
    for cases in [false, true] {
        let s = i::simulation(10, false);
        let s = if cases {
            i::edit(
                &i::edit(&s, "arrivals fixed [0min]", "arrivals fixed [0min,5min]"),
                "cases 1;",
                "cases 2;",
            )
        } else {
            i::edit(
                &s,
                "cohort.WrongPerArrival <= 0.6 }",
                "cohort.WrongPerArrival <= 0.6; cohort.WrongPerArrival < 1 }",
            )
        };
        let v = compared(&s, &["Good", "Bad"], true, true);
        no_errors(&v);
        assert_eq!(
            v["evaluation"]["statistics"]["multiplicity"],
            json!({"method":"bonferroni","m":4,"alpha":"1/16"})
        );
        transport(&s, &v, true, true);
    }
}

#[test]
fn record_quality_flows_through_filter_objectives_and_tables() {
    let s = i::append(
        q::RECORD,
        "configuration Left { cut C; process P }\nconfiguration Right { cut C; process P }\ncompare [Left,Right] hard { cohort.WrongRate <= 0.02 } objectives { minimize cohort.Conditional }",
    );
    let v = compared(&s, &["Left", "Right"], false, false);
    no_errors(&v);
    assert_eq!(v["verdict"], "satisfied");
    statuses(&v, "admissible", "satisfied");
    for c in cells(&v) {
        assert_eq!(c["interval"], i::interval("1/50", "1/50"));
    }
    for c in v["evaluation"]["objectives"][0]["values"]
        .as_array()
        .unwrap()
    {
        assert_eq!(c["lower"], "1/46");
        assert_eq!(c["upper"], "1/46");
    }
    assert_eq!(
        v["evaluation"]["pareto"]["non_dominated"],
        json!(["Left", "Right"])
    );
    transport(&s, &v, false, false);
    assert_eq!(v["tables"]["quality"], quality_table::record());
}

#[test]
fn record_latent_guard_is_located_static_failure_not_spanless_evaluation() {
    let s = i::edit(q::RECORD, "if v.confidence == pass", "if correct == true");
    let s = i::append(
        &s,
        "configuration Left { cut C; process P }\nconfiguration Right { cut C; process P }\ncompare [Left,Right] hard { cohort.WrongRate <= 0.02 }",
    );
    let v = compared(&s, &["Left", "Right"], false, false);
    assert_eq!(v["verdict"], "invalid");
    assert!(v.get("evaluation").is_none());
    for p in v["plans"].as_array().unwrap() {
        let ds = p["report"]["diagnostics"].as_array().unwrap();
        assert!(
            ds.iter()
                .any(|d| d["code"] == "E-POL-LATENT" && d["span"].is_object())
        );
    }
    assert!(
        !v["checked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "hard_filter" || x == "property_tables")
    );
}

#[test]
fn held_horizon_and_complete_drain_keep_all_arrivals() {
    for h in [Some(3), Some(16), None] {
        let replacement = h.map_or_else(String::new, |h| format!("horizon {h}min;"));
        let s = if h == Some(3) {
            q::HELD.into()
        } else {
            i::edit(q::HELD, "horizon 3min;", &replacement)
        };
        let s = i::with_plans(&s, true);
        let v = compared(&s, &["Left", "Right"], false, true);
        no_errors(&v);
        let cut = h == Some(3);
        if cut {
            assert_eq!(v["tables"]["quality"], quality_table::held());
        }
        assert_eq!(v["verdict"], if cut { "inconclusive" } else { "satisfied" });
        statuses(
            &v,
            if cut { "undetermined" } else { "admissible" },
            if cut { "inconclusive" } else { "satisfied" },
        );
        for c in cells(&v) {
            assert_eq!(c["verdict"], "satisfied");
            assert_eq!(c["interval"], i::interval(if cut { "0" } else { "1" }, "1"));
        }
        for p in v["plans"].as_array().unwrap() {
            let rows = p["report"]["quality"]["outcomes_per_arrival"]
                .as_array()
                .unwrap();
            assert_eq!(rows.iter().any(|r| r["kind"] == "cut_off"), h.is_some());
            if let Some(row) = rows.iter().find(|r| r["kind"] == "cut_off") {
                assert_eq!(row["rate"], i::exact(if cut { "1" } else { "0" }));
            }
        }
        transport(&s, &v, false, true);
    }
}

#[test]
fn overflow_and_cut_off_do_not_become_ordinary_end() {
    let s = i::scenario(&i::fixture("qual-overflow"), Some(1));
    // This existing fixture has process Flow, not the Held substrate's P.
    let s = i::append(
        &s,
        "configuration Left { cut C; process Flow }\nconfiguration Right { cut C; process Flow }\ncompare [Left,Right] under [Day] hard { cohort.WrongPerArrival <= 0.5 }",
    );
    let v = compared(&s, &["Left", "Right"], false, true);
    no_errors(&v);
    assert_eq!(v["tables"]["quality"], quality_table::overflow());
    for p in v["plans"].as_array().unwrap() {
        let rows = p["report"]["quality"]["outcomes_per_arrival"]
            .as_array()
            .unwrap();
        assert!(
            !rows
                .iter()
                .any(|r| r["kind"] == "end" && r["end"] == "spill")
        );
        for kind in ["overflow", "cut_off"] {
            assert_eq!(
                rows.iter().find(|r| r["kind"] == kind).unwrap()["rate"],
                i::exact("1/2")
            );
        }
    }
    for c in cells(&v) {
        assert_eq!(c["interval"], i::interval("0", "1/2"));
        assert_eq!(c["verdict"], "satisfied");
    }
    transport(&s, &v, false, true);
}

#[test]
fn d04_rule_quality_is_withdrawn_not_invented_zero_or_one() {
    for sampled in [false, true] {
        let s = if sampled {
            i::edit(rule::QUALITY, "process P; amend", "process Q; amend")
        } else {
            rule::QUALITY.into()
        };
        let s = if sampled {
            i::append(&s, "process Q { x = A.run(); end done }")
        } else {
            s
        };
        let s = i::append(
            &s,
            "compare [AsIs,ToBeB] hard { cohort.Rate <= 0.5 } objectives { minimize cohort.Rate }",
        );
        let v = compared(&s, &["AsIs", "ToBeB"], sampled, false);
        no_errors(&v);
        assert_eq!(
            v["verdict"],
            if sampled { "inconclusive" } else { "violated" }
        );
        assert_eq!(v["tables"]["quality"], quality_table::rule(sampled));
        for (j, c) in cells(&v).iter().enumerate() {
            assert_eq!(
                c["verdict"],
                if sampled {
                    "inconclusive"
                } else if j == 0 {
                    "violated"
                } else {
                    "satisfied"
                }
            );
            assert_eq!(c["reason"], Value::Null);
            assert!(c["interval"].is_object());
        }
        for (j, p) in v["evaluation"]["plans"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            assert_eq!(
                p["status"],
                if sampled {
                    "undetermined"
                } else if j == 0 {
                    "excluded"
                } else {
                    "admissible"
                }
            );
        }
        for (j, p) in v["plans"].as_array().unwrap().iter().enumerate() {
            rule::assert_fixed_quality(&p["report"]["quality"], j == 0, sampled);
            assert!(
                !p["report"]["assumptions"]
                    .as_array()
                    .unwrap()
                    .contains(&rule::quality_rule_assumption())
            );
        }
        transport(&s, &v, sampled, false);
    }
}

#[test]
fn d04_rule_c5_complete_ten_rows_transport_without_quality_claim() {
    let s = i::append(
        &i::fixture("rule-diff"),
        "compare [AsIs,ToBeB] objectives { minimize metrics.overall.work }",
    );
    let v = compared(&s, &["AsIs", "ToBeB"], false, false);
    no_errors(&v);
    assert_eq!(v["evaluation"]["plans"][0]["status"], "admissible");
    assert_eq!(v["evaluation"]["plans"][1]["status"], "excluded");
    assert_eq!(
        v["tables"]["agency"][0]["differences"]["rows"],
        rule::differences()
    );
    assert_eq!(v["tables"]["agency"][0]["from"], "AsIs");
    assert_eq!(v["tables"]["agency"][0]["to"], "ToBeB");
    for p in v["plans"].as_array().unwrap() {
        assert_eq!(
            p["report"]["agency"]["viewpoints"]["effects"]["rows"][0]["j"],
            json!([])
        );
    }
    transport(&s, &v, false, false);
}

#[test]
fn declared_evaluation_checked_and_unchecked_are_hand_closed() {
    for sampled in [false, true] {
        let s = i::simulation(5, false);
        let v = compared(&s, &["Good", "Bad"], sampled, true);
        no_errors(&v);
        let mut checked = vec![
            "configuration_selection",
            "cut_compatibility",
            "terminal_compatibility",
        ];
        if sampled {
            checked.extend(["simulation_clause", "random_key_separation"]);
        }
        // ADR0017 U5 suppresses terminals only on runtime abort, not merely
        // because this is a simulation. Both controls complete normally.
        checked.extend(["property_tables", "terminal_tables"]);
        checked.extend([
            "metric_tables",
            "hard_filter",
            "admissibility",
            if sampled {
                "hard_cross_plan_bonferroni"
            } else {
                "pareto_sure_dominance"
            },
        ]);
        assert_eq!(v["checked"], json!(checked));
        let mut unchecked = vec!["ranking"];
        if sampled {
            unchecked.push("pareto");
        }
        unchecked.extend([
            "weighted_score",
            "differences",
            "difference_intervals",
            "cross_plan_confidence",
            "operation_coupling",
            "terminal_meaning",
            "metric_differences",
            "metric_ranking",
            "metric_semantic_alignment",
        ]);
        assert_eq!(v["unchecked"], json!(unchecked));
    }
}

fn cutoff_rows_contract(declared: bool, sampled: bool) {
    for h in [1, 5] {
        let s = i::simulation(h, false);
        let s = if declared { s } else { i::without_compare(&s) };
        let v = compared(&s, &["Good", "Bad"], sampled, true);
        assert_eq!(v["diagnostics"], json!([]));
        assert!(
            v["checked"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w == "terminal_tables")
        );
        let rows: Vec<_> = v["tables"]["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "cut_off")
            .collect();
        assert_eq!(
            rows.len(),
            1,
            "selected scenario declares H{h}: cutoff inventory is lexical, never selected from observed rows"
        );
        let values: Vec<_> = ["Good", "Bad"].iter().map(|c| json!({"configuration":c,
            "status":if sampled || h == 1 { "reported" } else { "not_reported" },
            "value":if sampled { json!({"count":if h == 1 {3} else {0}}) } else if h == 1 {
                json!({"probability":{"min":"1","max":"1"},"completed_at":null,"elapsed":null,"work":{"min":1,"max":1},"result_at":null})
            } else { Value::Null }})).collect();
        assert_eq!(
            *rows[0],
            json!({"kind":"cut_off","end":null,"case_id":1,"arrival":0,"values":values})
        );
    }
}
#[test]
fn silent_cutoff_declared_exhaustive_rectangle() {
    cutoff_rows_contract(true, false);
}
#[test]
fn silent_cutoff_declared_simulation_rectangle() {
    cutoff_rows_contract(true, true);
}
#[test]
fn silent_cutoff_legacy_exhaustive_rectangle() {
    cutoff_rows_contract(false, false);
}
#[test]
fn silent_cutoff_legacy_simulation_rectangle() {
    cutoff_rows_contract(false, true);
}
#[test]
fn no_horizon_control_never_acquires_cutoff_table() {
    for sampled in [false, true] {
        let s = i::edit(&i::simulation(5, false), "horizon 5min", "");
        for s in [&s, &i::without_compare(&s)] {
            let v = compared(s, &["Good", "Bad"], sampled, true);
            assert_eq!(v["diagnostics"], json!([]));
            assert!(
                !v["tables"]["terminals"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["kind"] == "cut_off")
            );
        }
    }
}

#[test]
fn atomic_version_check_quality_is_a_per_case_relaxation_not_uniform_schedule_mass() {
    for removed in [false, true] {
        let s = i::fixture("cf-version-atomic");
        let s = if removed {
            i::edit(&s, "version_check [keys] else conflict", "")
        } else {
            s
        };
        let s = i::append(
            &s,
            "property case.Done = reached(end.done)\nmetric cohort.Rate = expected(count(case.Done)) / arrivals\nconfiguration Left { cut K; process Flow }\nconfiguration Right { cut K; process Flow }\ncompare [Left,Right] hard { cohort.Rate <= 1 } objectives { maximize cohort.Rate }",
        );
        let v = compared(&s, &["Left", "Right"], false, false);
        no_errors(&v);
        for p in v["plans"].as_array().unwrap() {
            let rate = i::row(&p["report"]["quality"], "cohort", "Rate");
            assert_eq!(
                rate["value"],
                if removed {
                    i::exact("1")
                } else {
                    json!({"lower":"0","upper":"1","basis":"per_case_relaxation","reason":null})
                }
            );
        }
        for c in cells(&v) {
            assert_eq!(c["verdict"], "satisfied");
        }
        transport(&s, &v, false, false);
    }
}

#[test]
fn sticky_lost_update_survives_hard_satisfied_and_unknown_quality() {
    let s = i::edit(
        &i::fixture("cf-lww.broken-overlap"),
        "clock 10min",
        "clock 2min",
    );
    let s = i::edit(&s, "r = Ledger.claim()", "r = Ledger.claim(); wait 5min");
    let s = i::append(
        &s,
        "property case.Done = reached(end.claimed)\nmetric cohort.Rate = expected(count(case.Done)) / arrivals\nconfiguration Left { cut C; process P }\nconfiguration Right { cut C; process P }\ncompare [Left,Right] hard { cohort.Rate <= 1 }",
    );
    let v = compared(&s, &["Left", "Right"], false, false);
    no_errors(&v);
    assert_eq!(v["verdict"], "violated");
    statuses(&v, "excluded", "violated");
    for c in cells(&v) {
        assert_eq!(c["verdict"], "satisfied");
        assert_eq!(c["reason"], "truncated");
    }
    for p in v["plans"].as_array().unwrap() {
        assert!(
            p["report"]["properties"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["id"].as_str().unwrap().starts_with("no_lost_update ")
                    && p["verdict"] == "violated")
        );
    }
    transport(&s, &v, false, false);
}

#[test]
fn standing_effect_route_work_remains_in_overall_objective_not_business_end() {
    let s = i::append(
        &i::fixture("sov-metrics"),
        "configuration Left { cut Scope; process Flow }\nconfiguration Right { cut Scope; process Flow }\ncompare [Left,Right] objectives { minimize metrics.overall.work }",
    );
    let v = compared(&s, &["Left", "Right"], false, false);
    no_errors(&v);
    for c in v["evaluation"]["objectives"][0]["values"]
        .as_array()
        .unwrap()
    {
        assert_eq!(c["lower"], "2");
        assert_eq!(c["upper"], "4");
        assert_eq!(c["basis"], "path_range");
    }
    assert_eq!(
        v["evaluation"]["pareto"]["undetermined_pairs"],
        json!([{"from":"Left","to":"Right","reason":"overlapping_intervals"}])
    );
    for p in v["plans"].as_array().unwrap() {
        let posted = p["report"]["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["end"] == "posted")
            .unwrap();
        assert_eq!(posted["work"], json!({"min":3,"max":3}));
        assert!(p["report"]["contest_runtime"].is_array());
    }
    transport(&s, &v, false, false);
}

#[test]
fn exact_horizon_oracle_rejects_one_input_truth_fault() {
    let good = i::horizon(30, "0.3", false);
    let bad = i::edit(
        &good,
        "registered == true && correct == false",
        "registered == true && correct == true",
    );
    let assert_original = |v: &Value| {
        for c in cells(v) {
            assert_eq!(c["interval"], i::interval("1/50", "1/50"));
        }
    };
    let v = compared(&good, &["Check", "Fast"], false, true);
    assert_original(&v);
    let altered = compared(&bad, &["Check", "Fast"], false, true);
    no_errors(&altered);
    assert!(std::panic::catch_unwind(|| assert_original(&altered)).is_err());
    assert_eq!(altered["verdict"], "violated");
    for c in cells(&altered) {
        assert_eq!(c["interval"], i::interval("9/10", "9/10"));
    }
}
