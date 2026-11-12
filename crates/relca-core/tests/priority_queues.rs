//! ADR0062 acceptance. RED expectations are intentional, never ignored.
#[path = "support/q5_oracle.rs"]
mod hand;
#[path = "support/q5_inputs.rs"]
mod input;
use relca_core::{engine, explain, lower, outcome, reference, report, syntax};
use serde_json::{Value, json};

fn public(name: &str) -> Value {
    let src = input::source(name);
    match input::command(name) {
        "simulate" => json!(report::simulate_with_selection(
            &src,
            None,
            input::scenario(name)
        )),
        "explain" => json!(explain::explain_with_scenario(&src, input::scenario(name))),
        _ => json!(report::check_with_selection(
            &src,
            None,
            input::scenario(name)
        )),
    }
}
fn accept(name: &str) {
    let v = public(name);
    hand::validate(name, &v)
        .unwrap_or_else(|e| panic!("{name}: {e}; diagnostics {}", v["diagnostics"]));
}
macro_rules! accepts{($($test:ident=>$name:literal),+$(,)?)=>{$(#[test]fn $test(){accept($name);})+};}
accepts! {
    p01_larger_rank_starts_first_and_later_high_overtakes=>"start",
    p01_equal_low_tail_remains_nondeterministic=>"start-tie",
    p02_omitted_tie_is_not_fifo_or_half_probability=>"equal",
    p02_explicit_nondet_has_same_extrema=>"equal-explicit",
    p02_explicit_fifo_orders_case_id=>"equal-fifo",
    p02_simulate_refuses_unresolved_tie=>"equal-sim",
    p02_call_order_within_explicit_fifo=>"call-order",
    p02_reversed_call_declaration_reverses_wait=>"call-order-reverse",
    p03_policy_people_compete_without_double_occupancy=>"people",
    p03_environment_person_competition_is_not_fixed=>"people-environment",
    p04_closed_calendar_blocks_high_priority_too=>"calendar",
    p04_later_opening_changes_wait_not_work=>"calendar-later",
    p05_only_held_readmission_uses_external_priority=>"external",
    p05_explicit_external_fifo_control=>"external-fifo",
    p06_external_omitted_tie_has_both_orders=>"external-equal",
    p06_external_explicit_fifo_has_one_order=>"external-equal-fifo",
    p06_external_tie_simulation_refuses=>"external-equal-sim",
    p06_initial_admission_stays_case_id_order=>"initial",
    p07_due_version_finish_rank_selects_one_writer=>"conflict",
    p07_start_policy_does_not_imply_finish_priority=>"conflict-no-resolve",
    p08_omitted_finish_tie_keeps_both_winners=>"conflict-equal",
    p08_explicit_finish_case_id_selects_lower_id=>"conflict-id",
    p09_past_low_commit_stales_future_high=>"future",
    p09_same_due_time_changes_winner_not_duration=>"future-tied",
    p10_stale_high_rejects_without_staling_fresh_low=>"stale",
    p11_disjoint_checked_sets_keep_both_finish_orders=>"unrelated",
    p11_finish_ack_race_is_not_removed_by_priority=>"ack",
    p12_clock_keeps_low_case_held_and_unfinished=>"clock",
    p12_horizon_is_cut_off_not_computational_truncation=>"horizon",
    p13_actor_head_blocks_later_future_calendar=>"actor-hol",
    p13_common_uses_head_blocks_other_actor=>"resource-hol",
    p13_independent_actor_can_use_future_calendar=>"independent-hol",
    p13_priority_wait_without_physical_block_is_unattributed=>"unattributed",
    p13_adding_real_resource_claim_changes_cause=>"physical",
    p15_retry_attempts_keep_original_case_priority=>"retry",
    p15_unknown_keeps_effect_and_never_becomes_failed=>"retry-unknown",
    p16_unused_table_does_not_enable_start_priority=>"unused",
    p16_full_u64_max_does_not_overflow_sort=>"max-rank",
    p16_actor_name_and_grain_do_not_decide_priority=>"label",
    p16_selected_scenario_uses_model_rank_table=>"scenario"
}

#[test]
fn p16_every_static_failure_has_its_reason_not_generic_new_keyword_failure() {
    let mut failures = Vec::new();
    for &(name, code, reason) in input::STATIC {
        let v = public(name);
        if let Err(e) = hand::diagnostic(&v, code, reason) {
            failures.push(format!("{name}: {e}"));
        }
        let expected = if code.starts_with("E-") {
            "invalid"
        } else {
            "unsupported"
        };
        if v["verdict"] != expected {
            failures.push(format!("{name}: wrong verdict {}", v["verdict"]));
        }
        if !v["case_outcomes"].as_array().is_some_and(Vec::is_empty) {
            failures.push(format!("{name}: executed static refusal"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn green_model_substrates_parse_and_lower_without_priority_not_unrelated_errors() {
    let mut failures = Vec::new();
    for &name in input::RUNTIME {
        let source = input::substrate(name);
        let parsed = syntax::parse(&source);
        match parsed {
            Err(d) => failures.push(format!("{name}: parse {d:?}")),
            Ok(ast) => {
                if let Err(d) = lower::lower_with_selection(&ast, None, input::scenario(name)) {
                    failures.push(format!("{name}: lower {d:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn green_closed_inventory_and_exact_single_edit_pairs() {
    let mut names = std::collections::BTreeSet::new();
    for name in input::RUNTIME
        .iter()
        .copied()
        .chain(input::STATIC.iter().map(|r| r.0))
    {
        assert!(names.insert(name), "duplicate {name}");
        assert!(input::source(name).starts_with("model "));
    }
    assert_eq!(input::RUNTIME.len(), 43);
    assert_eq!(input::STATIC.len(), 13);
    for &(a, b, from, to) in input::PAIRS {
        assert_eq!(
            input::edit(&input::source(a), from, to),
            input::source(b),
            "{a}->{b}"
        );
    }
}

#[test]
fn green_five_required_broken_wires_and_version_faults_are_detected() {
    for (name, description, bad) in hand::faults() {
        hand::validate(name, &hand::witness(name)).unwrap();
        assert!(
            hand::validate(name, &bad).is_err(),
            "blind oracle: {description}"
        );
    }
    let mut with_uncertainty = hand::witness("start");
    with_uncertainty["uncertainty"] = json!({"future_q7_field":"not the Q5 oracle scope"});
    hand::validate("start", &with_uncertainty).unwrap();
}

// Project only specified Outcome paths; each interpreter is checked against
// hand arithmetic BEFORE comparing it to the other implementation.
fn outcome_cases(out: &outcome::Outcome, m: &relca_core::ir::Model) -> Value {
    json!(out.case_outcomes.as_ref().unwrap().iter().map(|c|{
        let mut row=json!({"case_id":c.case_id.0+1,"arrival":c.arrival,"terminals":c.terminals.iter().map(|t|{
            let(kind,end)=match t.kind{
                outcome::CaseTerminalKind::End(id)=>("end",Some(m.ends[id.0].clone())),
                outcome::CaseTerminalKind::CutOff=>("cut_off",None),
                outcome::CaseTerminalKind::ReceiptUnknown=>("receipt_unknown",None),
                _=>("other",None)};
            json!({"kind":kind,"end":end,"completed_at":t.completed_at,"elapsed":t.elapsed,"work":t.work,"result_at":t.result_at,
                "observed_until":t.observed_until,"observed_elapsed":t.observed_elapsed,"observed_work":t.observed_work})
        }).collect::<Vec<_>>()});
        if let Some(at)=c.admitted_at{row["admitted_at"]=json!(at);}row
    }).collect::<Vec<_>>())
}
#[test]
fn p01_p16_both_interpreters_match_hand_cases_and_metrics_independently() {
    for &name in input::RUNTIME
        .iter()
        .filter(|n| input::command(n) == "check")
    {
        let ast = syntax::parse(&input::source(name)).unwrap_or_else(|d| panic!("{name}: {d:?}"));
        let m = lower::lower_with_selection(&ast, None, input::scenario(name)).unwrap();
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        let want = hand::expected(name);
        for (which, out) in [("engine", &e), ("reference", &r.outcome)] {
            hand::subset(
                &want["case_outcomes"],
                &outcome_cases(out, &m),
                &format!("{which}/{name}/case_outcomes"),
            )
            .unwrap();
            if let Some(metrics) = want.get("metrics") {
                hand::subset(
                    metrics,
                    &json!(out.metrics),
                    &format!("{which}/{name}/metrics"),
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn p14_public_explanations_have_independently_verified_replay() {
    for name in ["explain-start", "explain-external", "explain-conflict"] {
        let v = public(name);
        hand::validate(name, &v).unwrap();
        let rows = v["explanation"]["counterexamples"]
            .as_array()
            .expect("counterexamples");
        assert!(
            !rows.is_empty(),
            "{name}: required structural counterexample"
        );
        for row in rows {
            assert_eq!(
                row["replay"]["status"], "verified",
                "{name}: {}",
                row["replay"]
            );
        }
    }
}

#[test]
fn p16_priority_wire_is_closed_and_local_rule_link_round_trips() {
    for name in ["start", "unused", "external", "conflict"] {
        let v = public(name);
        hand::validate(name, &v).unwrap();
        let decoded: relca_core::report::Report = serde_json::from_value(v.clone()).unwrap();
        let wire = json!(decoded);
        assert_eq!(wire["priority_policy"], hand::policy(name));
        let ledger: relca_core::ledger::Ledger =
            serde_json::from_value(wire["ledger"].clone()).unwrap();
        ledger.validate(&wire).unwrap();
    }
}

#[test]
fn p02_p06_each_simulator_rejects_unresolved_ties_without_random_priority_draws() {
    for name in ["equal-sim", "external-equal-sim"] {
        let ast = syntax::parse(&input::source(name)).unwrap_or_else(|d| panic!("{name}: {d:?}"));
        let m = lower::lower(&ast).unwrap();
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        assert_eq!(r.stop, reference::StopReason::SemanticAbort);
        for (which, out) in [("engine", &e), ("reference", &r.outcome)] {
            assert!(
                out.diagnostics
                    .iter()
                    .any(|d| d.code == "U-SEM-0" && d.message.contains("unresolved")),
                "{name}/{which}: must refuse a choice, not invalid IR or missing metrics"
            );
            assert_eq!(out.completed_replications, 0);
            assert!(
                out.replications
                    .iter()
                    .flat_map(|rep| &rep.draws)
                    .all(|draw| !draw.key.contains("priority"))
            );
        }
    }
}

#[test]
fn green_legacy_version_retry_stays_unsupported_without_priority() {
    let source = input::substrate("version-retry");
    let v = json!(report::check(&source));
    assert_eq!(v["verdict"], "unsupported");
    hand::diagnostic(&v, "U-SEM-0", "retry_feature_combination").unwrap();
}
