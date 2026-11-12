//! ADR0059 stage-C acceptance: public API plus each independent interpreter.
#[path = "support/q4_oracle.rs"]
mod hand;
#[path = "support/q4_inputs.rs"]
mod input;
use relca_core::{engine, explain, lower, reference, report, syntax};
use serde_json::{Value, json};

fn public(name: &str) -> Value {
    let s = input::source(name);
    match input::command(name) {
        "simulate" => json!(report::simulate_with_selection(
            &s,
            None,
            input::scenario(name)
        )),
        "explain" => json!(explain::explain(&s)),
        _ => json!(report::check_with_selection(
            &s,
            None,
            input::scenario(name)
        )),
    }
}

fn runtime(name: &str) {
    let v = public(name);
    hand::validate(name, &v)
        .unwrap_or_else(|e| panic!("{e}; observed diagnostics {}", v["diagnostics"]));
    assert_eq!(v["ledger"]["schema"], "relca-conclusion-ledger/v1");
}

macro_rules! runtime_test {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            runtime($name);
        }
    };
}
runtime_test!(a4_a9_opening_is_clock_event_and_calendar_wait, "open");
runtime_test!(a4_later_opening_moves_end_not_work, "open-later");
runtime_test!(a4_a11_continue_keeps_work_and_missing_cost_null, "continue");
runtime_test!(
    a9_earlier_close_changes_overtime_not_completion,
    "continue-earlier-close"
);
runtime_test!(
    a3_a4_global_people_shift_does_not_transfer_running_task,
    "shift"
);
runtime_test!(
    a3_later_shift_does_not_reuse_off_shift_person,
    "shift-later"
);
runtime_test!(a4_all_resources_are_acquired_together, "resource");
runtime_test!(
    a4_resource_opening_moves_start_and_holds_nothing_early,
    "resource-later"
);
runtime_test!(a5_cancel_ack_keeps_occupancy_across_close, "ack");
runtime_test!(
    a5_ack_upper_bound_has_both_one_and_two_tick_delays,
    "ack-later"
);
runtime_test!(a5_horizon_is_cut_off_and_dwell_is_lower, "horizon");
runtime_test!(
    a5_clock_is_truncated_and_calendar_quantities_are_null,
    "clock"
);
runtime_test!(b2_b4_condition_wait_is_not_task_wait, "condition");
runtime_test!(
    b2_later_signal_moves_condition_not_task_work,
    "condition-later"
);
runtime_test!(
    b4_condition_and_calendar_wait_are_disjoint,
    "condition-open"
);
runtime_test!(
    b4_later_opening_does_not_change_condition_time,
    "condition-open-later"
);
runtime_test!(
    b1_initial_true_wait_is_single_zero_time_progress,
    "condition-true"
);
runtime_test!(
    b2_false_without_writer_is_genuine_deadlock,
    "condition-deadlock"
);
runtime_test!(
    b2_same_tick_true_false_preserves_both_orders,
    "condition-race"
);
runtime_test!(
    b3_simulation_rejects_unresolved_condition_race,
    "condition-race-sim"
);
runtime_test!(
    b3_simulation_without_race_has_four_normal_ends,
    "condition-sim"
);
runtime_test!(a5_a11_schedule_drop_is_not_physical_overcommit, "drop");
runtime_test!(a11_zero_schedule_and_explicit_overtime_rate, "drop-zero");
runtime_test!(
    a11_zero_schedule_without_rate_keeps_null,
    "drop-zero-missing-rate"
);
runtime_test!(a11_explicit_overtime_rate_gives_twenty_two, "explicit-rate");
runtime_test!(a11_explicit_zero_rate_is_not_missing_rate, "free-overtime");
runtime_test!(
    c2_c3_finite_arrivals_have_sampled_not_first_arrival,
    "arrivals"
);
runtime_test!(
    c5_changed_arrivals_change_relative_deadline_hits,
    "arrivals-burst"
);
runtime_test!(
    c5_calendar_and_sampled_arrivals_use_realized_ready_times,
    "arrivals-calendar"
);
runtime_test!(green_fixed_support_burst_has_hand_values, "fixed-burst");
runtime_test!(green_fixed_support_spaced_has_hand_values, "fixed-spaced");
runtime_test!(a4_q1_each_retry_obeys_calendar_and_keeps_effect, "retry");
runtime_test!(
    a4_q1_later_opening_delays_second_attempt_only,
    "retry-later"
);

macro_rules! refusal_test {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            let (_, code, reason) = input::STATIC.iter().find(|r| r.0 == $name).unwrap();
            let v = public($name);
            hand::diagnostic(&v, code, reason).unwrap();
            assert!(v["terminals"].as_array().unwrap().is_empty());
            assert!(v["case_outcomes"].as_array().unwrap().is_empty());
        }
    };
}
refusal_test!(
    a2_missing_on_close_is_u_sem_not_implicit_continue,
    "missing-close"
);
refusal_test!(a2_suspend_is_explicitly_semantically_unsupported, "suspend");
refusal_test!(a2_gap_is_invalid_not_assumed_holiday_zero, "gap");
refusal_test!(a2_overlap_is_invalid_not_silently_repaired, "overlap");
refusal_test!(a2_table_covers_clock_boundary, "short-coverage");
refusal_test!(
    a3_missing_target_calendar_is_not_assumed_24_hours,
    "missing-target"
);
refusal_test!(
    a2_unselected_scenario_coverage_is_checked,
    "unselected-coverage"
);
refusal_test!(a2_unknown_calendar_is_name_error, "unknown-calendar");
refusal_test!(b1_latent_condition_is_static_error, "latent-condition");
refusal_test!(
    c1_mass_one_point_one_is_probability_error,
    "bad-probability"
);
refusal_test!(c1_arrival_basis_must_be_explicit, "missing-basis");
refusal_test!(c1_all_support_rows_have_same_cohort_count, "wrong-count");
refusal_test!(c6_exact_mode_is_reasoned_unsupported, "arrivals-check");
refusal_test!(c6_explain_is_reasoned_unsupported, "arrivals-explain");

#[test]
fn green_inputs_and_single_edit_witnesses_have_closed_inventory() {
    let mut names = std::collections::BTreeSet::new();
    for n in input::RUNTIME
        .iter()
        .copied()
        .chain(input::STATIC.iter().map(|r| r.0))
    {
        assert!(names.insert(n), "duplicate {n}");
        assert!(input::source(n).starts_with("model "));
    }
    assert_eq!(names.len(), 47);
    for (a, b, from, to) in input::PAIRS {
        assert_eq!(
            input::edit(&input::source(a), from, to),
            input::source(b),
            "{a}->{b}"
        );
    }
}

#[test]
fn green_old_language_substrates_have_no_hidden_type_or_policy_errors() {
    for name in input::RUNTIME
        .iter()
        .filter(|n| !n.starts_with("arrivals") && **n != "condition-race-sim")
    {
        let source = input::substrate(&input::source(name));
        let ast = syntax::parse(&source)
            .unwrap_or_else(|d| panic!("substrate parse {name}: {d:?}\n{source}"));
        lower::lower_with_selection(&ast, None, input::scenario(name))
            .unwrap_or_else(|d| panic!("substrate lower {name}: {d:?}\n{source}"));
    }
}

#[test]
fn a1_calendar_syntax_is_accepted_before_engine_lowering() {
    for name in ["open", "shift", "resource", "drop"] {
        syntax::parse(&input::source(name)).unwrap();
    }
}

#[test]
fn b1_observed_wait_until_syntax_is_accepted_before_engine_lowering() {
    syntax::parse(&input::source("condition-true")).unwrap();
}

#[test]
fn c1_finite_distribution_syntax_is_accepted_before_sampling() {
    syntax::parse(&input::source("arrivals")).unwrap();
}

#[test]
fn green_independent_schedule_hash_and_hand_arithmetic_are_not_new_engine_output() {
    use sha2::{Digest, Sha256};
    let hex = [
        "1ab95de28da79f2e",
        "3a037e898da85e28",
        "89722ae085a119fa",
        "b2b842ec0c312694",
    ];
    let mut choices = Vec::new();
    for (i, expected) in hex.iter().enumerate() {
        let key = format!(
            "relca-sim-v1|seed=1|rep={}|case=0|site=arrival.schedule|n=1",
            i + 1
        );
        let digest = Sha256::digest(key.as_bytes());
        assert_eq!(
            digest[..8]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            *expected
        );
        choices.push(u64::from_be_bytes(digest[..8].try_into().unwrap()) >= (1u64 << 63));
    }
    assert_eq!(choices, [false, false, true, true]);
    let arrival = [0u64, 0, 4, 4];
    let lead = [6u64, 6, 3, 3];
    let wait = [3u64, 3, 0, 0];
    assert_eq!(arrival.iter().sum::<u64>(), 8);
    assert_eq!(lead.iter().sum::<u64>(), 18);
    assert_eq!(wait.iter().sum::<u64>(), 6);
    assert_eq!(2 * 8 + 3 * 2, 22, "normal plus overtime, not default-zero");
    assert_eq!(
        2 * 2 + 3 * 4,
        16,
        "partial capacity is not tariff allocation"
    );
}

#[test]
fn green_oracle_accepts_hand_witness_and_rejects_each_semantic_single_fault() {
    for (name, description, bad) in hand::mutation_controls() {
        let good = hand::expected(name);
        hand::validate(name, &good).unwrap();
        assert!(
            hand::validate(name, &bad).is_err(),
            "oracle blind to {description}"
        );
    }
    // The omitted holiday is a source fault and must be rejected by production,
    // not by a synthetic decoder. Its attached RED test is a2_gap_... above.
}

#[test]
fn a6_a7_a10_b3_b5_each_interpreter_meets_hand_metrics_before_differential() {
    for name in input::RUNTIME
        .iter()
        .filter(|n| input::command(n) == "check")
    {
        let ast = syntax::parse(&input::source(name)).unwrap_or_else(|d| panic!("{name}: {d:?}"));
        let m = lower::lower_with_selection(&ast, None, input::scenario(name)).unwrap();
        let e = engine::explore(&m);
        let mut r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        for (which, out) in [("engine", &e), ("reference", &r.outcome)] {
            if let Some(want) = hand::expected(name).get("metrics") {
                let actual = json!(out.metrics);
                hand::subset(want, &actual, &format!("{which}/{name}/metrics")).unwrap();
            }
            if let Some(want) = hand::expected(name).get("case_outcomes") {
                let actual:Vec<_>=out.case_outcomes.as_ref().unwrap().iter().map(|c|json!({
                    "case_id":c.case_id.0 + 1,"arrival":c.arrival,
                    "terminals":c.terminals.iter().map(|t|json!({
                        "kind":match t.kind {relca_core::outcome::CaseTerminalKind::End(_)=>"end",relca_core::outcome::CaseTerminalKind::CutOff=>"cut_off",_=>"other"},
                        "end":match t.kind {relca_core::outcome::CaseTerminalKind::End(id)=>Some(&m.ends[id.0]),_=>None},
                        "completed_at":t.completed_at,"elapsed":t.elapsed,"work":t.work
                    })).collect::<Vec<_>>()
                })).collect();
                hand::subset(want, &json!(actual), &format!("{which}/{name}/outcomes")).unwrap();
            }
        }
        assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
        r.outcome.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(
            e, r.outcome,
            "{name}: whole Outcome after separate hand checks"
        );
    }
}

#[test]
fn c3_c4_independent_draw_keys_and_every_realized_arrival_are_fixed() {
    use sha2::{Digest, Sha256};
    let hex = [
        "1ab95de28da79f2e",
        "3a037e898da85e28",
        "89722ae085a119fa",
        "b2b842ec0c312694",
    ];
    for name in ["arrivals", "arrivals-burst"] {
        let ast = syntax::parse(&input::source(name)).unwrap();
        let m = lower::lower(&ast).unwrap();
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for (which, out) in [("engine", &e), ("reference", &r.outcome)] {
            assert_eq!(out.replications.len(), 4, "{which}");
            for (i, rep) in out.replications.iter().enumerate() {
                let key = format!(
                    "relca-sim-v1|seed=1|rep={}|case=0|site=arrival.schedule|n=1",
                    i + 1
                );
                let digest = Sha256::digest(key.as_bytes());
                let prefix: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
                assert_eq!(prefix, hex[i], "independent SHA contract");
                assert_eq!(
                    rep.draws.iter().filter(|d| d.key == key).count(),
                    1,
                    "{which}: one schedule draw"
                );
                assert_eq!(rep.cases[0].arrival, 0);
                assert_eq!(
                    rep.cases[1].arrival,
                    if i < 2 {
                        0
                    } else if name == "arrivals" {
                        4
                    } else {
                        1
                    }
                );
                assert!(!rep.overcommit);
            }
        }
    }
}

fn words(value: &Value) -> String {
    match value {
        Value::Array(a) => a.iter().map(words).collect::<Vec<_>>().join(" "),
        Value::Object(o) => o.values().map(words).collect::<Vec<_>>().join(" "),
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

fn limitations(value: &Value) -> String {
    match value {
        Value::Object(o) => o
            .iter()
            .map(|(key, v)| {
                if matches!(key.as_str(), "unchecked" | "not_checked")
                    || (key == "reason" && value["status"] == "not_derived")
                {
                    words(v)
                } else {
                    limitations(v)
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        Value::Array(a) => a.iter().map(limitations).collect::<Vec<_>>().join(" "),
        _ => String::new(),
    }
}

#[test]
fn d1_agency_does_not_certify_calendar_or_condition_feasibility() {
    for (name, reason) in [("open", "calendar"), ("condition-true", "condition")] {
        let v = public(name);
        assert_eq!(v["verdict"], "satisfied");
        let disclosure = format!("{} {}", limitations(&v["agency"]), words(&v["unchecked"]));
        assert!(
            disclosure.contains(reason),
            "{name}: missing explicit Agency limitation"
        );
    }
}

#[test]
fn d1_conform_does_not_promote_missing_calendar_and_condition_observations() {
    for (name, reason, start, finish) in [
        ("open", "calendar", 5, 7),
        ("condition-true", "condition", 0, 2),
    ] {
        let header = json!({"kind":"header","schema":"relca-log/v1","clock":"fixture-clock","time_unit":"min",
            "window":{"from":0,"to":20},"cases":[{"case_id":"c","from_case_start":true,"through_case_end":true}],
            "actor_map":[{"source_id":"desk","actor":"Desk"}],"executor_map":[]});
        let mut log = header.to_string() + "\n";
        for (kind, at) in [("start", start), ("finish", finish)] {
            log.push_str(&json!({"kind":"event","event_id":kind,"case_id":"c","task_id":"r",
                "operation":"Desk.run","lifecycle":kind,"cancel_phase":null,"time":{"earliest":at,"latest":at},
                "after":[],"actor_id":"desk","executor_id":null,"input_version":"v1",
                "output_version":if kind=="finish" {Some("v2")} else {None},
                "result":if kind=="finish" {Some("ok")} else {None}}).to_string());
            log.push('\n');
        }
        let v = json!(relca_core::conformance::conform(
            &input::source(name),
            log.as_bytes(),
            None,
            None
        ));
        assert!(
            words(&v["not_checked"]).contains(reason),
            "{name}: missing not_checked feature, got {v}"
        );
    }
}

#[test]
fn d2_comparison_does_not_silently_promote_new_calendar_quantities() {
    let s = input::source("open");
    let end = s.rfind('}').unwrap();
    let s = format!(
        "{}\nconfiguration Left {{ cut C; process Flow }}\nconfiguration Right {{ cut C; process Flow }}\n}}",
        &s[..end]
    );
    let v = json!(relca_core::compare::compare(
        &s,
        &["Left", "Right"],
        relca_core::compare::Analysis::Exhaustive
    ));
    assert_eq!(v["verdict"], "satisfied");
    for plan in v["plans"].as_array().unwrap() {
        hand::validate("open", &plan["report"]).unwrap();
    }
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "waiting_tables")
    );
    let table = v["tables"]["metrics"].to_string();
    for name in [
        "scheduled_capacity_time",
        "overtime_occupancy_time",
        "tariff_overtime_work",
    ] {
        assert!(
            !table.contains(name),
            "new quantity automatically entered old comparison table"
        );
    }
}

#[test]
fn c6_mutate_compare_both_modes_and_diff_do_not_run_stochastic_arrivals() {
    let source = input::source("arrivals");
    let end = source.rfind('}').unwrap();
    let selected = format!(
        "{}\nconfiguration Left {{ cut C; process Flow }}\nconfiguration Right {{ cut C; process Flow }}\n}}",
        &source[..end]
    );
    let results = [
        json!(relca_core::mutation::mutate(&source)),
        json!(relca_core::compare::compare(
            &selected,
            &["Left", "Right"],
            relca_core::compare::Analysis::Exhaustive
        )),
        json!(relca_core::compare::compare(
            &selected,
            &["Left", "Right"],
            relca_core::compare::Analysis::Simulation
        )),
        json!(relca_core::diff::diff(&selected, "Left", "Right")),
    ];
    for result in results {
        assert_eq!(result["verdict"], "unsupported");
        fn reason(v: &Value) -> bool {
            match v {
                Value::Object(o) => {
                    (v["code"] == "U-SEM-0"
                        && v["message"]
                            .as_str()
                            .is_some_and(|s| s.contains("stochastic_arrivals_mode_unsupported")))
                        || o.values().any(reason)
                }
                Value::Array(a) => a.iter().any(reason),
                _ => false,
            }
        }
        assert!(
            reason(&result),
            "mode refusal must not be an unrelated parser error"
        );
    }
}
