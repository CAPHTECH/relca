//! ADR0045 × ADR0038–0043. Hand acceptance first; no production meaning fixes.
#[path = "support/quality_integration_inputs.rs"]
mod i;
#[path = "support/quality_mutation_inputs.rs"]
mod mutations;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn model(src: &str, day: bool) -> relca_core::ir::Model {
    let a = syntax::parse(src).unwrap_or_else(|d| panic!("parse: {d:?}"));
    lower::lower_with_selection(&a, None, day.then_some("Day"))
        .unwrap_or_else(|d| panic!("lower: {d:?}"))
}
fn quality(src: &str, day: bool, simulation: bool, reference_side: bool) -> Value {
    let m = model(src, day);
    if simulation {
        if reference_side {
            let r = reference::simulate(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            json!(r.outcome.quality.unwrap())
        } else {
            json!(engine::simulate(&m).quality.unwrap())
        }
    } else if reference_side {
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        json!(r.outcome.quality.unwrap())
    } else {
        json!(engine::explore(&m).quality.unwrap())
    }
}
fn deferred(name: &str, reference_side: bool) {
    let v = i::deferred(name);
    let src = v["source"].as_str().unwrap();
    let day = v["invocation"]["scenario"] == "Day";
    let sim = v["invocation"]["command"] == "simulate";
    assert_eq!(
        quality(src, day, sim, reference_side),
        v["expected"]["quality"],
        "{name}: complete hand oracle"
    );
}
macro_rules! pair {
    ($engine:ident, $reference:ident, $body:expr) => {
        #[test]
        fn $engine() {
            ($body)(false);
        }
        #[test]
        fn $reference() {
            ($body)(true);
        }
    };
}
pair!(t06_horizon_engine, t06_horizon_reference, |r| deferred(
    "qual-horizon",
    r
));
pair!(t06_clock_engine, t06_clock_reference, |r| deferred(
    "qual-horizon.broken-clock",
    r
));
pair!(t10_horizon_engine, t10_horizon_reference, |r| deferred(
    "qual-sim-horizon",
    r
));
pair!(t10_clock_engine, t10_clock_reference, |r| deferred(
    "qual-sim-horizon.broken-clock",
    r
));

pair!(
    d05_clock_with_horizon_engine,
    d05_clock_with_horizon_reference,
    |r| {
        let s = i::deferred("qual-horizon.broken-clock")["source"]
            .as_str()
            .unwrap()
            .to_owned();
        let s = i::scenario(&s, 30);
        let q = quality(&s, true, false, r);
        i::unknown(&q, "WrongPerArrival", "truncated", "1/75", "26/75");
        i::unknown(&q, "WrongPerRegistered", "truncated", "1/71", "13/23");
        // Computation stopped before the later business observation; the
        // final cut_off classification is not yet known.
        assert_eq!(
            i::outcome(&q, "cut_off")["expected_count"],
            i::range("0", "1", Some("truncated"))
        );
        assert_eq!(i::outcome(&q, "cut_off")["rate"]["reason"], "truncated");
    }
);
pair!(
    d05_live_tasks_with_horizon_engine,
    d05_live_tasks_with_horizon_reference,
    |r| {
        let s = i::scenario(&i::fixture("qual-live-tasks.broken-one-task"), 30);
        let q = quality(&s, true, false, r);
        i::unknown(&q, "Rate", "truncated", "0", "1");
    }
);
pair!(
    d05_simulation_clock_with_horizon_engine,
    d05_simulation_clock_with_horizon_reference,
    |r| {
        let s = i::deferred("qual-sim-horizon.broken-clock")["source"]
            .as_str()
            .unwrap()
            .to_owned();
        let s = i::scenario(&s, 30);
        let q = quality(&s, true, true, r);
        let ratio = i::ratio(&q, "WrongPerArrival");
        assert_eq!(ratio["value"]["reason"], "truncated");
        assert_eq!(
            ratio["bounds_with_undetermined"],
            json!({"lower":"0","upper":"1/3"})
        );
    }
);
pair!(
    horizon_future_deadline_simulation_engine,
    horizon_future_deadline_simulation_reference,
    |r| {
        // Arrivals0,6,12; all service+post paths take5. The first two within3
        // windows have closed. Last deadline15 is AFTER H14, so it is not a miss
        // merely because it lies before the unrelated computational clock30.
        let v = i::deferred("qual-sim-horizon");
        let s = i::append(
            v["source"].as_str().unwrap(),
            "property case.Early = reached(registered == true) within 3min\nmetric cohort.EarlyRate = expected(count(case.Early)) / arrivals",
        );
        let q = quality(&s, true, true, r);
        let p = i::property(&q, "Early");
        assert_eq!(p["cases"][2]["hits"], 0);
        assert_eq!(p["cases"][2]["unknown"], 4);
        assert_eq!(i::ratio(&q, "EarlyRate")["value"]["reason"], "undetermined");
        assert_eq!(
            i::ratio(&q, "EarlyRate")["bounds_with_undetermined"],
            json!({"lower":"0","upper":"1/3"})
        );
    }
);
pair!(
    quality_only_numeric_with_horizon_engine,
    quality_only_numeric_with_horizon_reference,
    |r| {
        // Same explicit reference stack as quality_mutation_fixes::deep.
        std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || {
                // p has denominator10^63. p^20 needs about4186 bits, beyond4096;
                // p^19 fits. Every execution finishes by20, before H25/clock30. Thus the
                // quality-only unknown is numeric/truncated, never business/undetermined.
                let input = mutations::quality_only_numeric_arrivals();
                let s = i::scenario(&input.source, 25);
                let q = quality(&s, true, false, r);
                i::unknown(&q, "AllRate", "truncated", "0", "1");
                assert_eq!(i::ratio(&q, "DoneRate")["value"], i::exact("1"));
                let m = model(&s, true);
                let old = model(&i::strip_quality(&s), true);
                if r {
                    let a = reference::explore(&m);
                    let b = reference::explore(&old);
                    assert_eq!(a.stop, reference::StopReason::Complete);
                    assert_eq!(a.outcome.truncated, b.outcome.truncated);
                    assert_eq!(a.outcome.diagnostics, b.outcome.diagnostics);
                } else {
                    let a = engine::explore(&m);
                    let b = engine::explore(&old);
                    assert_eq!(a.truncated, b.truncated);
                    assert_eq!(a.diagnostics, b.diagnostics);
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
);
pair!(
    mixed_horizon_and_computational_engine,
    mixed_horizon_and_computational_reference,
    |r| {
        i::unknown(
            &quality(i::MIXED, true, false, r),
            "Rate",
            "truncated",
            "0",
            "1",
        );
    }
);
pair!(held_horizon_engine, held_horizon_reference, |r| {
    let q = quality(i::HELD, true, false, r);
    i::unknown(&q, "Rate", "undetermined", "0", "1");
    assert_eq!(i::property(&q, "Hit")["cases"].as_array().unwrap().len(), 3);
    assert_eq!(i::outcome(&q, "cut_off")["expected_count"], i::exact("3"));
    assert_eq!(i::outcome(&q, "cut_off")["rate"], i::exact("1"));
    assert_eq!(i::outcome(&q, "unfinished")["rate"], i::exact("0"));
});
fn closed_deadlines(sim: bool, r: bool) {
    let v = i::deferred(if sim {
        "qual-sim-horizon"
    } else {
        "qual-horizon"
    });
    for (condition, n) in [("registered == false", "3"), ("registered == true", "0")] {
        for deadline in [1, 2] {
            let s = i::append(
                v["source"].as_str().unwrap(),
                &format!(
                    "property case.Early = reached({condition}) within {deadline}min\nmetric cohort.EarlyRate = expected(count(case.Early)) / arrivals"
                ),
            );
            let q = quality(&s, true, sim, r);
            let p = i::property(&q, "Early");
            if sim {
                assert_eq!(p["expected_count"]["mean"], n);
                assert_eq!(p["unknown_count"]["mean"], "0");
                assert_eq!(
                    i::ratio(&q, "EarlyRate")["value"]["point"],
                    if n == "3" { "1" } else { "0" }
                );
            } else {
                assert_eq!(p["expected_count"], i::exact(n));
                assert_eq!(
                    i::ratio(&q, "EarlyRate")["value"],
                    i::exact(if n == "3" { "1" } else { "0" })
                );
            }
        }
    }
}
pair!(
    d04_closed_deadlines_engine,
    d04_closed_deadlines_reference,
    |r| closed_deadlines(false, r)
);
pair!(
    d04_closed_deadlines_simulation_engine,
    d04_closed_deadlines_simulation_reference,
    |r| closed_deadlines(true, r)
);
pair!(
    horizon_exact_deadline_engine,
    horizon_exact_deadline_reference,
    |r| {
        // At H3 no run can finish before5. The inclusive within 1min window is closed,
        // so unknown work does not make this bounded property unknown.
        let s = i::append(
            i::HELD,
            "property case.Early = reached(hit == true) within 1min\nmetric cohort.EarlyRate = expected(count(case.Early)) / arrivals",
        );
        // Case3 arrival2 +1 = horizon3 (inclusive). Others expired earlier.
        let q = quality(&s, true, false, r);
        assert_eq!(i::property(&q, "Early")["expected_count"], i::exact("0"));
    }
);
pair!(
    min_max_unknown_with_horizon_engine,
    min_max_unknown_with_horizon_reference,
    |r| {
        for n in [
            "qual-min-only-unknown.broken-short-clock",
            "qual-max-only-unknown.broken-short-clock",
        ] {
            let s = i::scenario(&i::fixture(n), 30);
            i::unknown(&quality(&s, true, false, r), "Rate", "truncated", "0", "1");
        }
    }
);
pair!(
    record_field_and_scalar_latent_engine,
    record_field_and_scalar_latent_reference,
    |r| {
        let q = quality(i::RECORD, false, false, r);
        for (name, value) in [
            ("WrongRate", "1/50"),
            ("Conditional", "1/46"),
            ("PassRate", "23/25"),
        ] {
            assert_eq!(i::ratio(&q, name)["value"], i::exact(value));
        }
        let changed = i::RECORD.replace("entry.confidence == pass)", "entry.confidence == fail)");
        assert_eq!(
            i::ratio(&quality(&changed, false, false, r), "PassRate")["value"],
            i::exact("2/25")
        );
    }
);

pair!(
    post_end_route_horizon_engine,
    post_end_route_horizon_reference,
    |r| {
        // Business End at1 is final for outcome classification, while a contest
        // route can still write reviewed at3; H2 observes the latter as unknown.
        let s = i::scenario(&i::fixture("qual-post-end-state"), 2);
        let q = quality(&s, true, false, r);
        assert_eq!(i::ratio(&q, "ReturnedRate")["value"], i::exact("1"));
        assert_eq!(i::outcome(&q, "cut_off")["rate"], i::exact("0"));
        assert_eq!(i::outcome(&q, "unfinished")["rate"], i::exact("0"));
        i::unknown(&q, "ReviewedRate", "undetermined", "0", "1");
    }
);
pair!(
    overflow_and_horizon_are_disjoint_engine,
    overflow_and_horizon_are_disjoint_reference,
    |r| {
        // At receipt0 queue1 admits case1, case2 takes spill immediately. H1
        // cuts case1 before Checker3. Never classify spill as ordinary End too.
        let s = i::scenario(&i::fixture("qual-overflow"), 1);
        let q = quality(&s, true, false, r);
        assert_eq!(i::outcome(&q, "overflow")["rate"], i::exact("1/2"));
        assert!(
            !q["outcomes_per_arrival"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["kind"] == "end" && v["end"] == "spill")
        );
        assert_eq!(i::outcome(&q, "cut_off")["rate"], i::exact("1/2"));
        assert_eq!(i::outcome(&q, "unfinished")["rate"], i::exact("0"));
    }
);
pair!(
    held_simulation_horizon_engine,
    held_simulation_horizon_reference,
    |r| {
        let s = i::HELD.replace(
            "horizon 3min;",
            "horizon 3min; simulation replications 4 seed 1 confidence 0.5;",
        );
        let q = quality(&s, true, true, r);
        assert_eq!(
            i::outcome(&q, "cut_off")["expected_count"],
            json!({"mean":"3","min":"3","max":"3","samples":4,"reason":null})
        );
        assert_eq!(i::outcome(&q, "cut_off")["rate"]["point"], "1");
        assert_eq!(i::outcome(&q, "unfinished")["rate"]["point"], "0");
        assert_eq!(i::ratio(&q, "Rate")["value"]["reason"], "undetermined");
        assert_eq!(
            i::ratio(&q, "Rate")["bounds_with_undetermined"],
            json!({"lower":"0","upper":"1"})
        );
    }
);

pair!(
    d08_sticky_lost_update_engine,
    d08_sticky_lost_update_reference,
    |r| {
        let s = i::fixture("cf-lww.broken-overlap")
            .replace("clock 10min", "clock 2min")
            .replace("r = Ledger.claim()", "r = Ledger.claim(); wait 5min");
        let s = i::append(
            &s,
            "property case.Done = reached(end.claimed)\nmetric cohort.Rate = expected(count(case.Done)) / arrivals",
        );
        let m = model(&s, false);
        let out = if r {
            let run = reference::explore(&m);
            assert_eq!(run.stop, reference::StopReason::Complete);
            run.outcome
        } else {
            engine::explore(&m)
        };
        assert!(
            out.properties
                .iter()
                .any(|p| p.verdict == relca_core::Verdict::Violated)
        );
        i::unknown(&json!(out.quality.unwrap()), "Rate", "truncated", "0", "1");
    }
);

#[test]
fn d01_d07_static_boundaries_do_not_expand_record_semantics() {
    for (s, code) in [
        (
            i::RECORD.replace(
                "reached(entry.confidence == pass)",
                "reached(v.confidence == pass)",
            ),
            "U-SEM-0",
        ),
        (
            i::RECORD.replace("if v.confidence == pass", "if correct == true"),
            "E-POL-LATENT",
        ),
        (
            i::append(
                &i::fixture("rec-s01-latent-case.broken-unsupported"),
                "property case.P = reached(entry.confidence == high)",
            ),
            "U-SYN-0",
        ),
        (
            i::RECORD.replace(
                "reached(entry.confidence == pass)",
                "reached(entry == Verdict { confidence = pass })",
            ),
            "U-SYN-0",
        ),
    ] {
        let v = json!(report::check(&s));
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code),
            "{v}"
        );
        assert!(
            v.get("quality").is_none(),
            "refusal must not publish fake quality"
        );
    }
}

#[test]
fn d02_nested_scenario_quality_is_rejected_at_scope() {
    for decl in [
        "property case.P = reached(registered == true)",
        "metric cohort.R = expected(count(case.Wrong)) / arrivals",
    ] {
        let v = i::deferred("qual-horizon");
        let s = v["source"]
            .as_str()
            .unwrap()
            .replace("scenario Day {", &format!("scenario Day {{ {decl};"));
        let parsed = syntax::parse(&s).unwrap_err();
        assert!(parsed.iter().any(|d| d.code == "U-SYN-0"));
    }
}
#[test]
fn d03_unused_scenarios_do_not_duplicate_quality_diagnostics_or_move_span() {
    let s = i::append(
        &i::scenario(&i::fixture("qual-base"), 30),
        "property case.Wrong = reached(registered == true)",
    );
    let plain = json!(report::check_with_selection(&s, None, Some("Day")));
    let s = i::append(
        &s,
        "scenario Unused { arrivals fixed [0min,10min]; bounds { cases 2; live_tasks 2; clock 30min } }",
    );
    let with = json!(report::check_with_selection(&s, None, Some("Day")));
    assert_eq!(plain["diagnostics"], with["diagnostics"]);
    let errors: Vec<_> = with["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["code"], "U-SEM-0");
    assert_eq!(errors[0]["message"], "duplicate quality property");
}

fn held_full_drain(sim: bool, r: bool) {
    // Hand oracle BEFORE execution: receipts 0,1,2, concurrent limit1, service5.
    // Admissions 0,5,10; finishes 5,10,15; all before clock20 and control H16.
    // Hit per case=1, E[count]=3, Rate=1, end.done=3/3; all other outcomes=0.
    // With no Horizon there is NO cut_off row; H16 adds exactly a zero row.
    // ADR 0042 "意味 / drain(U3)", "出力 / scenario", and M2c distinguish
    // declared drain (2..all_terminal_or_clock or 2..16) from actual stop15;
    // this records drain per design v0.1 §11.2, not a new execution bound.
    for horizon in [None, Some(16)] {
        let replacement = format!(
            "{} {}",
            horizon.map_or_else(String::new, |h| format!("horizon {h}min;")),
            if sim {
                "simulation replications 4 seed 1 confidence 0.5;"
            } else {
                ""
            }
        );
        let src = i::HELD.replace("horizon 3min;", &replacement);
        let m = model(&src, true);
        assert_eq!(m.horizon.is_some(), horizon.is_some());
        let q = if sim {
            let out = if r {
                let run = reference::simulate(&m);
                assert_eq!(run.stop, reference::StopReason::Complete);
                run.outcome
            } else {
                engine::simulate(&m)
            };
            assert_eq!(out.replications.len(), 4);
            assert_eq!(out.truncated_replications, Some(0));
            for rep in &out.replications {
                assert_eq!(rep.scenario_observed_until, Some(15));
                assert_eq!(rep.cut, None);
                for c in &rep.cases {
                    assert_eq!(c.terminal.as_ref().unwrap().end.as_deref(), Some("done"));
                    assert_eq!(c.deadlock, None);
                }
            }
            let old = model(&i::strip_quality(&src), true);
            let old_out = if r {
                reference::simulate(&old).outcome
            } else {
                engine::simulate(&old)
            };
            assert_eq!(
                i::old_outcome_projection(json!(out)),
                i::old_outcome_projection(json!(old_out))
            );
            json!(out.quality.unwrap())
        } else {
            let out = if r {
                let run = reference::explore(&m);
                assert_eq!(run.stop, reference::StopReason::Complete);
                run.outcome
            } else {
                engine::explore(&m)
            };
            assert!(!out.truncated);
            let cases = out.case_outcomes.as_ref().unwrap();
            assert_eq!(cases.len(), 3);
            for (k, c) in cases.iter().enumerate() {
                assert_eq!(c.terminals.len(), 1);
                assert_eq!(
                    c.terminals[0].completed_at,
                    Some(relca_core::outcome::Range {
                        min: 5 * (k as u64 + 1),
                        max: 5 * (k as u64 + 1)
                    })
                );
            }
            assert_eq!(
                json!(out.scenario_observed_until),
                json!({"min":"15","max":"15","reason":null})
            );
            let old = model(&i::strip_quality(&src), true);
            let old_out = if r {
                reference::explore(&old).outcome
            } else {
                engine::explore(&old)
            };
            assert_eq!(out.case_outcomes, old_out.case_outcomes);
            assert_eq!(out.scenario_observed_until, old_out.scenario_observed_until);
            assert_eq!(out.properties, old_out.properties);
            assert_eq!(out.diagnostics, old_out.diagnostics);
            json!(out.quality.unwrap())
        };
        let hit = i::property(&q, "Hit");
        assert_eq!(hit["cases"].as_array().unwrap().len(), 3);
        for c in hit["cases"].as_array().unwrap() {
            if sim {
                assert_eq!(c["hits"], 4);
                assert_eq!(c["unknown"], 0);
                assert_eq!(c["trials"], 4);
            } else {
                assert_eq!(c["min"], "1");
                assert_eq!(c["max"], "1");
                assert!(c["reason"].is_null());
            }
        }
        if sim {
            assert_eq!(
                hit["expected_count"],
                json!({"mean":"3","min":"3","max":"3","samples":4,"reason":null})
            );
            assert_eq!(hit["unknown_count"]["mean"], "0");
            assert_eq!(i::ratio(&q, "Rate")["value"]["point"], "1");
            assert_eq!(i::ratio(&q, "Rate")["value"]["samples"], 4);
        } else {
            assert_eq!(hit["expected_count"], i::exact("3"));
            assert_eq!(i::ratio(&q, "Rate")["value"], i::exact("1"));
        }
        let rows = q["outcomes_per_arrival"].as_array().unwrap();
        assert_eq!(rows.len(), if horizon.is_some() { 5 } else { 4 });
        assert_eq!(
            rows.iter().any(|v| v["kind"] == "cut_off"),
            horizon.is_some()
        );
        for row in rows {
            let (count, rate) = if row["kind"] == "end" {
                assert_eq!(row["end"], "done");
                ("3", "1")
            } else {
                ("0", "0")
            };
            if sim {
                assert_eq!(
                    row["expected_count"],
                    json!({"mean":count,"min":count,"max":count,"samples":4,"reason":null})
                );
                assert_eq!(row["rate"], json!({"point":rate,"samples":4,"reason":null}));
            } else {
                assert_eq!(row["expected_count"], i::exact(count));
                assert_eq!(row["rate"], i::exact(rate));
            }
        }
        // Report projection also retains scenario.drain (declared interval) and
        // aggregation.observed_until (actual finish15), not just quality rows.
        let report = if sim {
            json!(report::simulate_with_selection(&src, None, Some("Day")))
        } else {
            json!(report::check_with_selection(&src, None, Some("Day")))
        };
        let old_src = i::strip_quality(&src);
        let old_report = if sim {
            json!(report::simulate_with_selection(&old_src, None, Some("Day")))
        } else {
            json!(report::check_with_selection(&old_src, None, Some("Day")))
        };
        assert_eq!(report["scenario"]["drain"]["from"], 2);
        assert_eq!(
            report["scenario"]["drain"]["until"],
            horizon.map_or(json!("all_terminal_or_clock"), |h| json!(h))
        );
        assert_eq!(
            report["scenario"]["aggregation"]["observed_until"],
            if sim {
                json!({"mean":"15","min":"15","max":"15","samples":4,"reason":null})
            } else {
                json!({"min":"15","max":"15","reason":null})
            }
        );
        assert_eq!(i::old_projection(report), i::old_projection(old_report));
    }
}
pair!(held_full_drain_engine, held_full_drain_reference, |r| {
    held_full_drain(false, r)
});
pair!(
    held_full_drain_simulation_engine,
    held_full_drain_simulation_reference,
    |r| held_full_drain(true, r)
);
