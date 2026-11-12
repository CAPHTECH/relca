//! ADR0064 Q6a test-first public acceptance; no new Rust APIs are assumed.
#[path = "support/q6a_oracle.rs"]
mod hand;
#[path = "support/q6a_inputs.rs"]
mod input;
#[path = "support/q6a_private.rs"]
mod private;
use relca_core::{engine, explain, ir, lower, outcome, reference, report, syntax};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn model(name: &str) -> ir::Model {
    let src = input::source(name);
    let ast =
        syntax::parse(&src).unwrap_or_else(|e| panic!("Q6a/{name} parse not implemented: {e:?}"));
    lower::lower_with_selection(&ast, None, input::scenario(name))
        .unwrap_or_else(|e| panic!("Q6a/{name} lower not implemented: {e:?}"))
}
fn public(name: &str) -> Value {
    json!(report::check_with_selection(
        &input::source(name),
        None,
        input::scenario(name)
    ))
}
fn accept(name: &str) {
    let v = public(name);
    hand::validate(name, &v)
        .unwrap_or_else(|e| panic!("Q6a/{name}: {e}; diagnostics={}", v["diagnostics"]));
}
macro_rules! accepts {($($test:ident=>$name:literal),+$(,)?)=>{$(#[test]fn $test(){accept($name);})+};}
accepts! {
    a1_filter_before_first_idle=>"qualified",
    a1_uncontrolled_start_filters_qualification=>"uncontrolled",
    a1_permission_is_not_eligibility=>"forbidden",
    a2_wait_for_qualified_shift=>"shift",
    a2_nonfixed_candidate_frontier=>"shift-unassigned",
    a2_empty_qualification_does_not_fabricate_future_person=>"empty",
    a3_first_person_performance=>"people",
    a3_executor_order_not_qualification_order=>"reverse",
    a3_unassigned_is_interval_not_uniform=>"unassigned",
    a4_three_backlogs_count_and_ratio=>"burst",
    a4_one_kernel_change_not_time_change=>"lighter",
    a5_external_held_not_queued=>"held",
    a6_later_arrival_cannot_reselect=>"later",
    a6_simultaneous_arrival_changes_frozen_band=>"simultaneous",
    a4_self_not_in_backlog=>"single",
    a10_clock_not_false_success=>"clock",
    a10_horizon_is_undetermined_not_truncated=>"horizon",
    a13_calendar_qualification_and_priority=>"priority",
    a13_reversed_rank_preserves_people_work=>"priority-reverse",
    a14_requires_false_no_start_no_work=>"refused",
    a14_requires_true_uses_profile=>"admitted",
    a10_defer_still_dominates_quality_and_cost=>"defer",
    i16_response_loss_none_known_result=>"known-none",
    i16_actor_grain_not_skill=>"label",
    i5_unused_declaration_no_scheduler_change=>"unused"
}

#[test]
fn a12_static_matrix_rejects_the_actual_fault_not_the_new_keyword() {
    let mut errors = vec![];
    for &(name, code, reason) in input::STATIC {
        for sim in [false, true] {
            let v = if sim {
                json!(report::simulate(&input::source(name)))
            } else {
                public(name)
            };
            if let Err(e) = hand::diagnostic(&v, code, reason) {
                errors.push(format!("{name}/sim={sim}: {e}"));
            }
            if v.pointer("/ledger/execution/status") != Some(&json!("not_run")) {
                errors.push(format!(
                    "{name}/sim={sim}: static refusal must precede execution"
                ));
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

fn cases(out: &outcome::Outcome, m: &ir::Model) -> Value {
    json!(out.case_outcomes.as_ref().expect("Q6 effective path must be arrivals").iter().map(|c|
        json!({"case_id":c.case_id.0+1,"arrival":c.arrival,"terminals":c.terminals.iter().map(|t| {
            let (kind,end)=match t.kind {
                outcome::CaseTerminalKind::End(id)=>("end",Some(m.ends[id.0].clone())),
                outcome::CaseTerminalKind::CutOff=>("cut_off",None),
                outcome::CaseTerminalKind::ReceiptUnknown=>("receipt_unknown",None),
                _=>("other",None),
            };
            json!({"kind":kind,"end":end,"completed_at":t.completed_at,"elapsed":t.elapsed,"work":t.work})
        }).collect::<Vec<_>>()})
    ).collect::<Vec<_>>())
}
macro_rules! engines {($($test:ident=>$name:literal),+$(,)?)=>{$(#[test]fn $test(){both($name);})+};}
fn both(name: &str) {
    let m = model(name);
    assert!(!m.legacy_route(), "effective Q6 routes through arrivals");
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "{name}: {:?}",
        r.outcome.diagnostics
    );
    for (side, out) in [("engine", &e), ("reference", &r.outcome)] {
        assert_eq!(out.truncated, name == "clock", "{side}/{name}");
        hand::execution(
            name,
            &cases(out, &m),
            &json!(out.metrics),
            &json!(out.quality),
        )
        .unwrap_or_else(|err| panic!("{side}/{name}: {err}; diagnostics={:?}", out.diagnostics));
    }
}
engines! {
    a1_both_qualified=>"qualified", a1_both_forbidden=>"forbidden",
    a1_both_uncontrolled=>"uncontrolled",
    a2_both_shift=>"shift", a2_both_nonfixed=>"shift-unassigned", a2_both_empty=>"empty",
    a3_both_people=>"people", a3_both_reverse=>"reverse", a3_both_unassigned=>"unassigned",
    a4_both_burst=>"burst", a4_both_lighter=>"lighter", a5_both_held=>"held",
    a6_both_later=>"later", a6_both_simultaneous=>"simultaneous", a4_both_single=>"single",
    a10_both_clock=>"clock", a10_both_horizon=>"horizon", a10_both_defer=>"defer",
    a13_both_priority=>"priority", a13_both_priority_reverse=>"priority-reverse",
    a14_both_refused=>"refused", a14_both_admitted=>"admitted"
}

fn sampled(name: &str) {
    let m = model(name);
    let r = reference::simulate(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for (side, o) in [("engine", engine::simulate(&m)), ("reference", r.outcome)] {
        assert_eq!(
            o.completed_replications, 16,
            "{side}/{name}: {:?}",
            o.diagnostics
        );
        hand::sample_quality(name, &json!(o.quality)).unwrap();
        assert_eq!(o.replications.len(), 16);
        for (i, rep) in o.replications.iter().enumerate() {
            let mut want = vec![];
            if name != "refused" {
                for case in 1..=hand::bank(name).len() {
                    let site = if name == "known-none" {
                        "kernel.Flow.r.attempt1"
                    } else {
                        "kernel.Flow.r"
                    };
                    want.push(format!(
                        "relca-sim-v1|seed=1|rep={}|case={case}|site={site}|n=1",
                        i + 1
                    ));
                }
            }
            let mut actual: Vec<_> = rep.draws.iter().map(|d| d.key.clone()).collect();
            want.sort();
            actual.sort();
            assert_eq!(
                actual,
                want,
                "{side}/{name}/rep{}: no selector or early draw",
                i + 1
            );
        }
    }
    hand::validate_sim(name, &json!(report::simulate(&input::source(name)))).unwrap();
}
macro_rules! sims {($($test:ident=>$name:literal),+$(,)?)=>{$(#[test]fn $test(){sampled($name);})+};}
sims! {
    a3_sim_people=>"people",a3_sim_reverse=>"reverse",a4_sim_burst=>"burst",a4_sim_lighter=>"lighter",
    a5_sim_held=>"held",a6_sim_later=>"later",a6_sim_simultaneous=>"simultaneous",
    a14_sim_refused=>"refused",a14_sim_admitted=>"admitted",i16_sim_known_none=>"known-none"
}
#[test]
fn a3_both_simulators_refuse_unassigned_not_uniform() {
    let m = model("unassigned");
    let r = reference::simulate(&m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    for o in [engine::simulate(&m), r.outcome] {
        assert_eq!(o.completed_replications, 0);
        assert!(
            o.diagnostics
                .iter()
                .any(|d| d.code == "U-SEM-0" && d.message.contains("unresolved"))
        );
    }
}

#[test]
fn i3_qualification_only_simulation_preserves_assignment_without_new_draws() {
    for name in ["qualified", "shift"] {
        let m = model(name);
        let r = reference::simulate(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for o in [engine::simulate(&m), r.outcome] {
            assert_eq!(o.completed_replications, 16);
            for rep in &o.replications {
                assert!(rep.draws.is_empty(), "qualification is not a random site");
                let metrics = json!(rep.metrics);
                for (person, work) in [("p", 0), ("q", 2)] {
                    hand::subset(
                        &hand::metric(work),
                        &hand::by(&metrics["people"], "person", &json!(person)).unwrap()["work"],
                        name,
                    )
                    .unwrap();
                }
            }
        }
        let v = json!(report::simulate(&input::source(name)));
        assert_eq!(
            v["verdict"], "inconclusive",
            "safety samples are not exhaustive safety"
        );
        assert_eq!(v["work_profiles"], hand::table(name));
        hand::ledger(&v).unwrap();
    }
}
#[test]
fn a1_a2_explain_uses_independent_qualified_replay() {
    for name in ["forbidden", "shift"] {
        let src = if name == "shift" {
            input::append(
                &input::source(name),
                "contract Deadline { response case.received -> one_of(end.done) by 4min after case.received otherwise unresolved }",
            )
        } else {
            input::source(name)
        };
        let v = json!(explain::explain(&src));
        assert_eq!(v["verdict"], "violated", "{name}: {}", v["diagnostics"]);
        let rows = v
            .pointer("/explanation/counterexamples")
            .and_then(Value::as_array)
            .expect("counterexamples");
        assert!(!rows.is_empty());
        for row in rows {
            assert_eq!(row["replay"]["status"], "verified", "{row}");
        }
    }
}
#[test]
fn i11_real_report_decoder_and_ledger_reject_single_fault_wires() {
    let v = public("people");
    hand::validate("people", &v).unwrap();
    serde_json::from_value::<report::Report>(v.clone()).expect("positive decode");
    for (base, label, table) in hand::wire_faults() {
        let mut bad = public(base);
        hand::validate(base, &bad).unwrap();
        bad["work_profiles"] = table;
        assert!(
            serde_json::from_value::<report::Report>(bad).is_err(),
            "{label}"
        );
    }
    let mut bad = v.clone();
    let rows = bad
        .pointer_mut("/ledger/derivation/components")
        .unwrap()
        .as_array_mut()
        .unwrap();
    let c = rows
        .iter_mut()
        .find(|r| r["at"] == "/work_profiles/rules")
        .unwrap();
    c["version"] = json!("work-profiles-v2");
    assert!(
        serde_json::from_value::<report::Report>(bad).is_err(),
        "ledger version must match wire rule"
    );
    let mut missing_kernel = v.clone();
    missing_kernel["work_profiles"]["profiles"][0]["rows"][0]["kernel"] = json!("Missing");
    assert!(hand::validate("people", &missing_kernel).is_err());
    assert!(
        serde_json::from_value::<report::Report>(missing_kernel).is_err(),
        "kernel name absent from effective aleatory declarations"
    );
    let mut missing_model_ref = v.clone();
    missing_model_ref
        .pointer_mut("/uncertainty/model/items")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .retain(|r| r["at"] != "/work_profiles");
    assert!(hand::validate("people", &missing_model_ref).is_err());
    assert!(
        serde_json::from_value::<report::Report>(missing_model_ref).is_err(),
        "missing model assumption reference"
    );
}
#[test]
fn green_old_language_substrates_have_no_unrelated_parse_or_lower_failure() {
    for name in input::RUNTIME.iter().filter(|n| **n != "unused") {
        let src = input::substrate(&input::source(name));
        let ast = syntax::parse(&src).unwrap_or_else(|e| panic!("{name}: {e:?}\n{src}"));
        lower::lower_with_selection(&ast, None, input::scenario(name))
            .unwrap_or_else(|e| panic!("{name}: {e:?}\n{src}"));
    }
}
#[test]
fn green_single_edit_pairs_and_closed_names() {
    let mut names = std::collections::BTreeSet::new();
    for n in input::RUNTIME
        .iter()
        .copied()
        .chain(input::CONDITIONAL.iter().copied())
        .chain(input::CAPACITY_CONTROLS.iter().copied())
        .chain(input::STATIC.iter().map(|r| r.0))
    {
        assert!(names.insert(n), "duplicate {n}");
        assert!(input::source(n).starts_with("model "));
    }
    for &(a, b, from, to) in input::PAIRS {
        assert_eq!(
            input::edit(&input::source(a), from, to),
            input::source(b),
            "{a}->{b}"
        );
    }
}
#[test]
fn green_independent_decoder_rejects_each_wire_fault() {
    for n in input::RUNTIME {
        hand::decode_table(&hand::table(n)).unwrap_or_else(|e| panic!("{n}: {e}"));
    }
    for (base, label, v) in hand::wire_faults() {
        hand::decode_table(&hand::table(base)).unwrap();
        assert!(hand::decode_table(&v).is_err(), "{label}");
    }
}
#[test]
fn green_independent_sha_bank_matches_fixed_masks() {
    for name in [
        "people",
        "known-none",
        "reverse",
        "burst",
        "lighter",
        "held",
        "later",
        "simultaneous",
    ] {
        for (case, ((n, d), mask)) in hand::probabilities(name)
            .iter()
            .map(|p| {
                let (n, d) = p.0.split_once('/').unwrap();
                (n.parse::<u128>().unwrap(), d.parse::<u128>().unwrap())
            })
            .zip(hand::bank(name))
            .enumerate()
        {
            for rep in 1..=16 {
                let site = if name == "known-none" {
                    "kernel.Flow.r.attempt1"
                } else {
                    "kernel.Flow.r"
                };
                let key = format!(
                    "relca-sim-v1|seed=1|rep={rep}|case={}|site={site}|n=1",
                    case + 1
                );
                let bytes = Sha256::digest(key.as_bytes());
                let x = u128::from(u64::from_be_bytes(bytes[..8].try_into().unwrap()));
                assert_eq!(
                    x * d >= (d - n) * (1u128 << 64),
                    (mask >> (rep - 1)) & 1 == 1,
                    "{name}/{key}"
                );
            }
        }
    }
}
#[test]
fn green_existing_calendar_permission_and_kernel_contracts_are_saved() {
    let calendar = json!(report::check(&input::legacy("calendar")));
    assert_eq!(
        calendar["verdict"], "violated",
        "old first_idle chooses unpermitted p, not q"
    );
    let ordinary = json!(report::check(&input::legacy("ordinary")));
    hand::quality("people", &ordinary["quality"]).unwrap();
    assert!(ordinary.get("work_profiles").is_none());
    let words = json!(report::check(&input::legacy("words")));
    assert!(
        words.get("work_profiles").is_none(),
        "binding name is not a feature declaration"
    );
    let clock = json!(report::check(&input::legacy("clock")));
    assert_eq!(clock["verdict"], "inconclusive");
    hand::quality("clock", &clock["quality"]).unwrap();
    let horizon = json!(report::check_with_selection(
        &input::legacy("horizon"),
        None,
        Some("Window")
    ));
    assert_eq!(horizon["bounds"]["truncated"], false);
    hand::quality("horizon", &horizon["quality"]).unwrap();
    let refused = json!(report::check(&input::legacy("refused")));
    let task = hand::by(
        &refused["metrics"]["cases"][0]["tasks"],
        "binding",
        &json!("r"),
    )
    .unwrap();
    hand::subset(&json!({"wait":{"min":null,"max":null,"reason":"refused"},
        "refusal_wait":hand::metric(0),"refused_count":hand::metric(1),"starts":hand::metric(0),"work":hand::metric(0)}),task,"old refusal").unwrap();
}

#[test]
fn i9_i10_selector_freeze_does_not_freeze_existing_kernel_given_values() {
    for (name, p, verdict) in [
        ("given-finish", "1/2", "satisfied"),
        ("given-later", "0", "violated"),
    ] {
        let m = model(name);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for (side, o) in [("engine", engine::explore(&m)), ("reference", r.outcome)] {
            let q = json!(o.quality);
            let property = hand::by(&q["properties"], "name", &json!("Hit")).unwrap();
            hand::subset(&hand::exact(p), &property["expected_count"], side).unwrap();
        }
        let v = public(name);
        assert_eq!(v["verdict"], verdict, "{}", v["diagnostics"]);
        let ratio = hand::by(&v["quality"]["cohort"], "name", &json!("HitPerArrival")).unwrap();
        hand::subset(&hand::exact(p), &ratio["value"], name).unwrap();
        hand::decode_table(&v["work_profiles"]).unwrap();
    }
}

#[test]
fn green_conditional_profile_substrates_preserve_finish_time_given_evaluation() {
    for (name, p) in [("given-finish", "1/2"), ("given-later", "0")] {
        let src = input::substrate(&input::source(name))
            .replace("outcomes kernel Low", "outcomes kernel K");
        let v = json!(report::check(&src));
        let property = hand::by(&v["quality"]["properties"], "name", &json!("Hit")).unwrap();
        hand::subset(&hand::exact(p), &property["expected_count"], name).unwrap();
    }
}

#[test]
fn i5_exact_profile_and_row_declaration_limits_are_not_runtime_truncations() {
    for name in input::CAPACITY_CONTROLS {
        let src = input::source(name);
        let a = syntax::parse(&src).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        lower::lower(&a).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    }
}

#[test]
fn i16_quantile_keeps_fixed_duration_despite_person_performance_difference() {
    for (base, slow, minutes) in [
        ("people", false, "2"),
        ("reverse", false, "2"),
        ("people", true, "3"),
    ] {
        let src = input::quantile_source(base, slow);
        let a = syntax::parse(&src).unwrap_or_else(|d| panic!("{base}: {d:?}"));
        let m = lower::lower(&a).unwrap();
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for (side, o) in [("engine", engine::explore(&m)), ("reference", r.outcome)] {
            hand::subset(
                &hand::quantiles(minutes),
                &json!(o.quality)["quantiles"],
                side,
            )
            .unwrap();
        }
        let v = json!(report::check(&src));
        assert_eq!(v["verdict"], "satisfied");
        hand::subset(&hand::quantiles(minutes), &v["quality"]["quantiles"], base).unwrap();
    }
}
