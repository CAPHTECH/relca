//! Q6b test-first acceptance. Future DSL uses only already-public Rust APIs.
#[path = "support/q6b_oracle.rs"]
mod hand;
#[path = "support/q6b_inputs.rs"]
mod input;
use relca_core::{engine, explain, ir, lower, reference, report, syntax};
use serde_json::{Value, json};

fn model(name: &str, substrate: bool) -> ir::Model {
    let src = if substrate {
        input::substrate(name)
    } else {
        input::source(name)
    };
    let ast = syntax::parse(&src).unwrap_or_else(|d| panic!("{name}: parse: {d:?}"));
    if let Some(config) = input::configuration(name) {
        lower::lower_configuration(&ast, config)
            .unwrap_or_else(|d| panic!("{name}: lower: {d:?}"))
            .model
    } else {
        lower::lower(&ast).unwrap_or_else(|d| panic!("{name}: lower: {d:?}"))
    }
}
fn check(name: &str) -> Value {
    let src = input::source(name);
    json!(if let Some(c) = input::configuration(name) {
        report::check_configuration(&src, c)
    } else {
        report::check(&src)
    })
}

fn terminal_projection(o: &relca_core::outcome::Outcome) -> Vec<(usize, u64, u64, u64, u64)> {
    let mut rows = if let Some(cases) = &o.case_outcomes {
        assert_eq!(
            cases.len(),
            1,
            "this projection is only for these single-case controls"
        );
        cases[0]
            .terminals
            .iter()
            .map(|t| {
                let relca_core::outcome::CaseTerminalKind::End(id) = t.kind else {
                    panic!("expected business End")
                };
                let elapsed = t.elapsed.unwrap();
                let work = t.work.unwrap();
                (id.0, elapsed.min, elapsed.max, work.min, work.max)
            })
            .collect::<Vec<_>>()
    } else {
        o.terminals
            .iter()
            .map(|t| {
                (
                    t.end.0,
                    t.elapsed.min,
                    t.elapsed.max,
                    t.work.min,
                    t.work.max,
                )
            })
            .collect()
    };
    rows.sort();
    rows
}

fn dynamic(name: &str, substrate: bool) {
    let m = model(name, substrate);
    if !substrate {
        assert!(
            !m.legacy_route(),
            "effective degrade must use arrivals path"
        );
    }
    let r = reference::explore(&m);
    let e = engine::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "{name}: reference exclusions=0"
    );
    for (side, o) in [("engine", &e), ("reference", &r.outcome)] {
        assert!(!o.truncated, "{side}/{name}");
        let (min, max) = if matches!(name, "keep" | "no-keep") {
            (1, 1)
        } else {
            hand::bounds(name)
        };
        let (times, work): (Vec<_>, Vec<_>) = if let Some(cases) = &o.case_outcomes {
            cases
                .iter()
                .flat_map(|c| &c.terminals)
                .map(|t| {
                    (
                        t.completed_at.expect("complete time"),
                        t.work.expect("complete work"),
                    )
                })
                .unzip()
        } else {
            o.terminals.iter().map(|t| (t.elapsed, t.work)).unzip()
        };
        assert!(!times.is_empty(), "{side}/{name}: missing terminals");
        assert_eq!(
            times.iter().map(|x| x.min).min(),
            Some(min),
            "{side}/{name}: elapsed lower"
        );
        assert_eq!(
            times.iter().map(|x| x.max).max(),
            Some(max),
            "{side}/{name}: elapsed upper"
        );
        let wmax = if name == "calendar" { 4 } else { max };
        assert_eq!(
            work.iter().map(|x| x.min).min(),
            Some(min),
            "{side}/{name}: work lower"
        );
        assert_eq!(
            work.iter().map(|x| x.max).max(),
            Some(wmax),
            "{side}/{name}: work upper"
        );
    }
    assert_eq!(
        e.terminals, r.outcome.terminals,
        "{name}: after separate hand checks"
    );
    assert_eq!(e.properties, r.outcome.properties, "{name}");
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes, "{name}");
}

#[test]
fn green_old_if_substrates_are_valid_and_match_hand_values_in_both_interpreters() {
    for name in input::RUNTIME
        .iter()
        .copied()
        .filter(|n| !matches!(*n, "qualified" | "clock" | "defer"))
    {
        dynamic(name, true);
    }
    for name in ["keep", "no-keep"] {
        dynamic(name, true);
    }
}

#[test]
fn green_a9_keep_is_only_intent_not_structural_dependency() {
    for (name, kept) in [("keep", true), ("no-keep", false)] {
        hand::legacy(&check(name), kept).unwrap();
    }
}

#[test]
fn green_wire_controls_are_independent_and_single_faults_are_rejected() {
    for name in input::RUNTIME {
        hand::validate_table(&hand::expected(name)).unwrap();
    }
    for (label, bad) in hand::wire_faults() {
        assert!(hand::validate_table(&bad).is_err(), "{label}");
    }
    let mut limit = hand::expected_limit();
    hand::validate_table(&limit).unwrap();
    limit["routes"] = json!([]);
    assert!(
        hand::validate_table(&limit).is_err(),
        "no partial or empty complete table at the derivation limit"
    );
}

#[test]
fn green_a7_a8_and_a11_pairs_are_exact_single_edits() {
    for (a, b, from, to) in [
        ("a7", "other", "c = Clerk.enter()", "c = Other.enter()"),
        ("a8", "must", "x = Other.enter()", "x = Clerk.enter()"),
        ("guarded", "guarded-open", "if v == fail", "if v == pass"),
        (
            "known-none",
            "unknown",
            "response_loss none",
            "response_loss sequence [unknown]",
        ),
        (
            "latent-ok",
            "latent",
            "when status == fail",
            "when secret == true",
        ),
        (
            "keep",
            "no-keep",
            "retire Clerk.enter from normal_path keep fallback",
            "retire Clerk.enter reason \"not retained\"",
        ),
        (
            "effect",
            "explain",
            "invariant manual == true -> marked == true",
            "invariant manual == true -> marked == false",
        ),
    ] {
        assert_eq!(input::edit(&input::source(a), from, to), input::source(b));
    }
}

macro_rules! runtime {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            dynamic($name, false);
        }
    };
}
runtime!(i12_i13_a7_explicit_degrade_has_if_execution, "a7");
runtime!(i13_a7_changed_call_changes_work_not_only_label, "other");
runtime!(i13_a8_bypass_stays_nondeterministic, "a8");
runtime!(i13_a8_replaced_bypass_makes_clerk_necessary, "must");
runtime!(i13_a11_none_produces_known_binding, "known-none");
runtime!(i13_a11_kernel_may_read_latent, "latent-ok");
runtime!(i13_state_and_effects_are_not_rolled_back, "effect");
runtime!(i13_i15_common_suffix_runs_on_both_sides, "suffix");
runtime!(i13_i15_requires_refusal_is_not_a_start, "requires");
runtime!(i13_i15_calendar_changes_wall_time_not_work, "calendar");
runtime!(i13_i15_priority_minimal_connection, "priority");

#[test]
fn i13_degrade_to_if_preserves_dynamic_outcomes_and_contracts_not_just_elapsed_extrema() {
    for name in [
        "a7",
        "other",
        "a8",
        "must",
        "guarded",
        "guarded-open",
        "suffix",
        "known-none",
        "effect",
    ] {
        let new = model(name, false);
        let old = model(name, true);
        let n = reference::explore(&new);
        let o = reference::explore(&old);
        assert_eq!(n.stop, reference::StopReason::Complete);
        assert_eq!(o.stop, reference::StopReason::Complete);
        for (n, o) in [
            (engine::explore(&new), engine::explore(&old)),
            (n.outcome, o.outcome),
        ] {
            assert_eq!(
                terminal_projection(&n),
                terminal_projection(&o),
                "{name}: End/time/work after normalizing old legacy versus new arrivals representation"
            );
            let properties = |out: &relca_core::outcome::Outcome| {
                out.properties
                    .iter()
                    .map(|p| (p.property, p.verdict, p.probability.clone()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                properties(&n),
                properties(&o),
                "{name}: same declared question verdicts, including marker invariant"
            );
            assert_eq!(n.truncated, o.truncated);
        }
    }
}

macro_rules! table {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            let v = check($name);
            hand::validate_report($name, &v)
                .unwrap_or_else(|d| panic!("{d}; diagnostics={}", v["diagnostics"]));
        }
    };
}
table!(i14_a7_direct_dependency_and_common_prefix, "a7");
table!(i14_a7_other_removes_clerk_and_doc, "other");
table!(i14_m8_a8_may_is_not_must, "a8");
table!(i14_m8_a8_clerk_on_both_paths_is_must, "must");
table!(i14_a8_guard_satisfiability_and_end_partition, "guarded");
table!(
    i14_a8_single_guard_change_opens_impossible_end,
    "guarded-open"
);
table!(i14_suffix_is_in_each_side_not_prefix, "suffix");
table!(i15_requires_side_is_not_fabricated_start_one, "requires");
table!(i15_shared_state_guard_is_not_known_zero, "state-guard");
table!(
    i15_calendar_availability_is_not_a_static_guarantee,
    "calendar"
);
table!(
    i15_priority_availability_is_not_a_static_guarantee,
    "priority"
);
table!(
    i15_observation_separates_structural_qualified_permitted,
    "qualified"
);
table!(
    i15_m8_keep_does_not_fabricate_clerk_dependency,
    "keep-route"
);
table!(
    i15_m8_keep_remains_irrelevant_after_another_call_edit,
    "keep-other"
);
table!(
    i14_structural_domain_does_not_use_runtime_probabilities,
    "latent-ok"
);
table!(i14_runtime_clock_does_not_cut_static_paths, "clock");
table!(i15_cut_does_not_remove_structural_dependency, "defer");

#[test]
fn i14_old_retention_keeps_whole_process_scope() {
    let v = check("a7");
    let rows = v["agency"]["viewpoints"]["retention"]["capabilities"]
        .as_array()
        .expect("lowered");
    let clerk = rows
        .iter()
        .find(|r| r["capability"] == "Clerk.enter")
        .unwrap();
    assert_eq!(clerk["normal"], json!({"min":0,"max":1}));
    assert!(v["assumptions"].to_string().contains("whole_process_paths"));
}

#[test]
fn i13_a11_simulate_does_not_invent_probability_for_nondet_probe() {
    let src = input::source("a7");
    let v = json!(report::simulate(&src));
    hand::diagnostic(&v, "U-SEM-0", "").unwrap();
    let m = model("a7", false);
    for o in [engine::simulate(&m), reference::simulate(&m).outcome] {
        assert_eq!(o.completed_replications, 0, "never choose a uniform branch");
        assert!(o.replications.is_empty());
    }
}

#[test]
fn i13_known_kernel_sample_matches_hand_time_and_has_no_selector_draw() {
    let m = model("latent-ok", false);
    let r = reference::simulate(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for o in [engine::simulate(&m), r.outcome] {
        assert_eq!(o.completed_replications, 4);
        for r in o.replications {
            let t = r.cases[0].terminal.as_ref().unwrap();
            assert_eq!(t.end.as_deref(), Some("done"));
            assert_eq!(
                json!(r.metrics)["overall"]["work"],
                json!({"min":"2","max":"2","reason":null})
            );
            assert!(
                r.draws.iter().all(|d| !d.key.contains("degrade")),
                "route is not a new draw site"
            );
        }
    }
}

#[test]
fn i13_explanation_replays_degraded_branch_and_preserves_original_source_location() {
    let src = input::source("explain");
    let v = json!(explain::explain(&src));
    assert_eq!(v["verdict"], "violated", "{}", v["diagnostics"]);
    let rows = v["explanation"]["counterexamples"].as_array().unwrap();
    let c = rows
        .iter()
        .find(|c| c["id"] == "contract C #1 invariant")
        .expect("invariant counterexample");
    assert_eq!(c["replay"]["status"], "verified");
    assert!(c.to_string().contains("Clerk.enter"));
    fn has_location(v: &Value, src: &str) -> bool {
        match v {
            Value::Object(o) => {
                let own = v
                    .pointer("/span/start")
                    .and_then(Value::as_u64)
                    .zip(v.pointer("/span/end").and_then(Value::as_u64))
                    .is_some_and(|(a, b)| {
                        a <= b
                            && b <= src.len() as u64
                            && src[a as usize..b as usize].contains("degrade Manual")
                    });
                own || o.values().any(|x| has_location(x, src))
            }
            Value::Array(a) => a.iter().any(|x| has_location(x, src)),
            _ => false,
        }
    }
    assert!(
        has_location(c, &src),
        "counterexample includes original degrade span, not generated If bytes"
    );
}

#[test]
fn i12_a11_static_rejections_use_specific_codes_before_running_any_prefix() {
    syntax::parse(&input::base())
        .expect("positive degrade grammar recognized before negative classifications");
    for (name, code, reason) in input::STATIC {
        let src = input::source(name);
        for v in [
            json!(report::check(&src)),
            json!(report::simulate(&src)),
            json!(explain::explain(&src)),
        ] {
            hand::diagnostic(&v, code, reason).unwrap_or_else(|d| panic!("{name}: {d}"));
        }
    }
}

#[test]
fn i15_m10_public_decoder_rejects_single_faults_and_broken_ledger_reference() {
    let good = check("a8");
    hand::validate_report("a8", &good).unwrap();
    serde_json::from_value::<report::Report>(good.clone()).expect("positive production decode");
    for (label, bad) in hand::wire_faults() {
        let mut v = good.clone();
        v["agency"]["degradation"] = bad;
        assert!(
            serde_json::from_value::<report::Report>(v).is_err(),
            "{label}"
        );
    }
    let encoded = good.to_string();
    assert!(encoded.contains("/agency/degradation/rules"));
    let bad = encoded.replace(
        "/agency/degradation/rules",
        "/agency/degradation/missing_rules",
    );
    assert!(serde_json::from_str::<report::Report>(&bad).is_err());
}

#[test]
fn i15_m8_legacy_decoder_rejects_fabricated_must_dependency() {
    let good = check("keep");
    hand::legacy(&good, true).unwrap();
    serde_json::from_value::<report::Report>(good.clone()).unwrap();
    let mut bad = good;
    let rows = bad["agency"]["viewpoints"]["retention"]["capabilities"]
        .as_array_mut()
        .unwrap();
    rows.iter_mut()
        .find(|r| r["capability"] == "Clerk.enter")
        .unwrap()["dependency"] = json!("must");
    assert!(
        serde_json::from_value::<report::Report>(bad).is_err(),
        "keep never authorizes a dependency claim"
    );
}

#[test]
fn i13_i15_clock_and_cut_never_turn_unknown_quantities_into_success_or_zero() {
    for (name, reason) in [("clock", "truncated"), ("defer", "not_configured")] {
        let v = check(name);
        hand::validate_report(name, &v).unwrap();
        let work = &v["metrics"]["overall"]["work"];
        assert_eq!(work["reason"], reason);
        assert!(work["min"].is_null() && work["max"].is_null());
        if name == "clock" {
            assert_eq!(v["verdict"], "inconclusive");
            let m = model(name, false);
            let r = reference::explore(&m);
            assert!(r.outcome.truncated);
            assert!(engine::explore(&m).truncated);
        }
    }
}
