//! Separate eleventh population; independent per-replication hand/SHA oracle.
//! Proposed public outcome wire: replications[].{replication,cases,draws};
//! cases[].{case_id,arrival,terminal:{kind,end}|null,queries:[hit|miss|unknown]}.
//! A reference limit is a failure, never an allowlisted exclusion.
use relca_core::{engine, lower, reference, syntax};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
#[path = "support/actor_observed_population.rs"]
mod aobs_population;
#[path = "support/authority_population.rs"]
mod authority_population;
#[path = "support/conflict_population.rs"]
mod conflict_population;
#[path = "support/configuration_diff_population.rs"]
mod diff2_population;
#[path = "support/completion_population.rs"]
mod done_population;
#[path = "support/effect_standing_population.rs"]
mod effect_population;
#[path = "support/integer_acceptance.rs"]
mod integer_population;
#[path = "support/limits_population.rs"]
mod limits_population;
#[path = "support/common_cause_population.rs"]
mod lin_population;
#[path = "support/load_confluent_migration.rs"]
mod load_confluent_migration;
#[path = "support/mutation_default_population.rs"]
mod mutation_default_population;
#[path = "support/percase_acceptance.rs"]
mod pcase_population;
#[path = "support/policy_population.rs"]
mod policy_population;
#[path = "support/record_population.rs"]
mod record_population;
#[path = "support/requires_acceptance.rs"]
mod req_population;
#[path = "support/resource_acceptance.rs"]
mod resource_population;
#[path = "support/responsibility_extension_population.rs"]
mod resp2_population;
#[path = "support/scenario_population.rs"]
mod scenario_population;
#[path = "support/state_invariant_population.rs"]
mod sinv_population;

const FIXTURES: [&str; 24] = [
    "sim-actor-initial",
    "sim-actor-initial.broken-missing-distribution",
    "sim-contract",
    "sim-contract.broken-short-response",
    "sim-cut",
    "sim-cut.broken-short-clock",
    "sim-deadlock",
    "sim-deadlock.broken-zero-capacity",
    "sim-fifo",
    "sim-fifo.broken-no-policy",
    "sim-hazard",
    "sim-hazard.broken-short-clock",
    "sim-keys",
    "sim-keys.broken-flipped-query",
    "sim-range",
    "sim-range.broken-zero-replications",
    "sim-reachable",
    "sim-reachable.broken-unseen-end",
    "sim-rejection",
    "sim-rejection.broken-positive-queue",
    "sim-required",
    "sim-required.broken-missing-simulation",
    "sim-statistical",
    "sim-statistical.broken-tight-threshold",
];
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
fn src(name: &str) -> String {
    fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}

#[test]
fn ld_merge_mean_elapsed_and_same_time() {
    let input = src("ld-confluent").replace(
        "  metric quantile",
        "  metric mean.Lead = mean(elapsed, case.Done)\n  metric quantile",
    );
    let m = syntax::parse(&input)
        .and_then(|ast| lower::lower(&ast))
        .unwrap();
    for out in [engine::simulate(&m), reference::simulate(&m).outcome] {
        assert_eq!(out.completed_replications, 4);
        assert!(out.same_time);
        let q = serde_json::to_value(out.quality).unwrap();
        assert_eq!(q["means"][0]["value"]["point"], "19/3");
        assert_eq!(q["means"][0]["value"]["observations"], 12);
    }
}

#[test]
fn ld_merge_same_time_cost_observation_changes_even_when_closed_state_agrees() {
    // A sets the observed flag at start. B's start fee (2 JPY) is included
    // at the first hit only if B starts before A, giving 1 vs 3 JPY.
    let input = src("mean-first-hit-fork")
        .replace("writes flag; finish { flag=on }", "start { flag=on }")
        .replace("clock 5min", "clock 20min")
        .replace("window 10min", "window 20min")
        .replace(
            "  verify safety",
            "  simulation replications 2 seed 0 confidence 0.5\n  verify safety",
        );
    for with_arrivals in [false, true] {
        let input = if with_arrivals {
            input.replace("  bounds", "  arrivals fixed [0min]\n  bounds")
        } else {
            input.clone()
        };
        for with_mean in [false, true] {
            let source = input
                .lines()
                .filter(|l| !l.contains("metric mean.") || (with_mean && l.contains("mean.Cost")))
                .collect::<Vec<_>>()
                .join("\n");
            let m = syntax::parse(&source)
                .and_then(|ast| lower::lower(&ast))
                .unwrap();
            assert_eq!(m.legacy_route(), !with_arrivals);
            let actual = engine::simulate(&m);
            let independent = reference::simulate(&m).outcome;
            assert_eq!(
                relca_test_support::diagnostic::facts(&actual.diagnostics),
                relca_test_support::diagnostic::facts(&independent.diagnostics),
                "arrivals={with_arrivals}, mean={with_mean}"
            );
            for out in [actual, independent] {
                assert_eq!(
                    out.completed_replications,
                    if with_mean { 0 } else { 2 },
                    "arrivals={with_arrivals}, mean={with_mean}: {:?}",
                    out.diagnostics
                );
                if with_mean {
                    assert_eq!(out.diagnostics[0].code, "U-SEM-0");
                    assert!(
                        out.diagnostics[0]
                            .message
                            .contains("time 0, cases [1], sites [P.x,P.y]")
                    );
                } else {
                    assert!(out.same_time);
                }
            }
        }
    }
}

#[test]
fn ld_merge_repeat_after_same_time_fork() {
    let input = src("agy-vp-repeat-control")
        .replace("result nondet { pass,fail }", "result nondet { pass }")
        .replace("process Main {", "actor Left { capacity 1; capability work() -> V { duration fixed 1min; result nondet { pass } } }\nactor Right { capacity 1; capability work() -> V { duration fixed 1min; result nondet { pass } } }\nprocess Main {\n  fork f { preleft = Left.work(); preright = Right.work() }; join all f")
        .replace("include [Office]", "include [Office,Left,Right]")
        .replace("live_tasks 1", "live_tasks 2")
        .replace("verify reachable", "simulation replications 2 seed 0 confidence 0.5\nverify reachable");
    let m = syntax::parse(&input)
        .and_then(|ast| lower::lower(&ast))
        .unwrap();
    let actual = engine::simulate(&m);
    let spec = reference::simulate(&m).outcome;
    for out in [&actual, &spec] {
        assert_eq!(out.completed_replications, 2, "{:?}", out.diagnostics);
        assert!(out.same_time);
    }
    assert_eq!(actual, spec);
}

#[test]
fn ld_published_confluence_assumption_and_legacy_hand_values() {
    let r = relca_core::report::simulate(&src("ld-confluent"));
    let row = r
        .assumptions
        .iter()
        .find(|a| a.field == "simulation.same_time")
        .unwrap();
    assert_eq!(row.value, "instant_confluence(max_states=4096)");
    let m = syntax::parse(&src("ld-legacy-fork"))
        .and_then(|a| lower::lower(&a))
        .unwrap();
    assert!(m.legacy_route());
    for out in [engine::simulate(&m), reference::simulate(&m).outcome] {
        assert_eq!(out.completed_replications, 4);
        for rep in out.replications {
            let value = serde_json::to_value(rep.metrics.unwrap()).unwrap();
            assert_eq!(value["overall"]["observed_until"]["min"], "2");
            assert_eq!(value["overall"]["work"]["min"], "4");
        }
    }
}

#[test]
fn ld_sampled_instant_closure() {
    for (name, passed, at) in [
        ("ld-confluent", true, ""),
        (
            "ld-confluent.broken-people",
            false,
            "time 3, cases [1,2], sites [P.e]",
        ),
        ("ld-legacy-fork", true, ""),
        ("ld-instant-observation", true, ""),
        (
            "ld-instant-observation.broken-transient",
            false,
            "time 2, cases [1], sites [P.a,P.b]",
        ),
        (
            "ld-legacy-fork.broken-same-actor",
            false,
            "time 0, cases [1], sites [P.a,P.b]",
        ),
    ] {
        let m = syntax::parse(&src(name))
            .and_then(|a| lower::lower(&a))
            .unwrap();
        let engine = engine::simulate(&m);
        let reference = reference::simulate(&m);
        assert_eq!(reference.stop, reference::StopReason::Complete, "{name}");
        for out in [&engine, &reference.outcome] {
            assert_eq!(
                out.completed_replications,
                if passed { 4 } else { 0 },
                "{name}: {:?}",
                out.diagnostics
            );
            assert_eq!(out.same_time, passed, "{name}");
            if passed {
                assert!(out.diagnostics.is_empty(), "{name}: {:?}", out.diagnostics);
                for rep in &out.replications {
                    assert!(rep.draws.is_empty());
                    assert_eq!(rep.cut, None);
                    assert_eq!(rep.cases.len(), m.bounds.cases as usize);
                    assert!(rep.cases.iter().all(|c| {
                        c.terminal.as_ref().and_then(|t| t.end.as_deref()) == Some("done")
                    }));
                }
            } else {
                assert_eq!(out.diagnostics[0].code, "U-SEM-0");
                assert!(
                    out.diagnostics[0].message.contains(at),
                    "{name}: {:?}",
                    out.diagnostics
                );
            }
        }
        let facts = relca_test_support::diagnostic::simulation_facts;
        assert_eq!(facts(engine), facts(reference.outcome), "{name}");
    }
}

#[test]
fn ld_dispatch_after_finishes() {
    use relca_core::Verdict;
    for (name, work, completed) in [
        ("ld-after-finishes", [(8, 8), (7, 7)], 4),
        ("ld-after-finishes.broken-off", [(7, 8), (7, 8)], 0),
        ("ld-priority-dispatch", [(8, 8), (7, 7)], 2),
        ("ld-priority-dispatch.broken-off", [(7, 8), (7, 8)], 0),
    ] {
        let m = syntax::parse(&src(name))
            .and_then(|a| lower::lower(&a))
            .unwrap();
        let sampled = engine::simulate(&m);
        assert_eq!(sampled.completed_replications, completed, "{name}");
        let facts = relca_test_support::diagnostic::simulation_facts;
        assert_eq!(
            facts(sampled.clone()),
            facts(reference::simulate(&m).outcome),
            "{name}"
        );
        for result in [engine::explore(&m), reference::explore(&m).outcome] {
            let metrics = serde_json::to_value(&result.metrics).unwrap();
            let people = metrics["people"].as_array().unwrap();
            assert_eq!(people.len(), work.len(), "{name}");
            for ((row, person), (min, max)) in people.iter().zip(["c1", "c2"]).zip(work) {
                assert_eq!(row["person"], person);
                assert_eq!(row["work"]["min"], min.to_string());
                assert_eq!(row["work"]["max"], max.to_string());
            }
        }
    }
    for (name, verdict, at) in [
        (
            "ld-late-enqueue",
            Verdict::Satisfied,
            Some([(5, 5), (7, 7)]),
        ),
        (
            "ld-late-enqueue.broken-off",
            Verdict::Satisfied,
            Some([(5, 7), (5, 7)]),
        ),
        ("ld-environment-start", Verdict::Violated, None),
        (
            "ld-environment-start.broken-controlled",
            Verdict::Satisfied,
            None,
        ),
    ] {
        let m = syntax::parse(&src(name))
            .and_then(|a| lower::lower(&a))
            .unwrap();
        for result in [engine::explore(&m), reference::explore(&m).outcome] {
            use relca_core::ir;
            let target = if at.is_some() {
                ir::Property::ReachableEnd(ir::EndId(
                    m.ends.iter().position(|end| end == "done").unwrap(),
                ))
            } else {
                let clause = m
                    .contracts
                    .iter()
                    .enumerate()
                    .find(|(i, c)| c.property_id(ir::ClauseId(*i)) == "contract K #1 invariant")
                    .unwrap();
                ir::Property::Contract(ir::ClauseId(clause.0))
            };
            let row = result
                .properties
                .iter()
                .find(|p| p.property == target)
                .unwrap_or_else(|| panic!("{name}: missing {target:?}"));
            assert_eq!(row.verdict, verdict, "{name}: {target:?}");
            if let Some(at) = at {
                let cases = result.case_outcomes.as_ref().unwrap();
                assert_eq!(cases.len(), 2);
                for (case, (lo, hi)) in cases.iter().zip(at) {
                    assert_eq!(
                        case.terminals[0].completed_at,
                        Some(relca_core::outcome::Range { min: lo, max: hi })
                    );
                    assert_eq!(
                        case.terminals[0].work,
                        Some(relca_core::outcome::Range { min: 5, max: 5 })
                    );
                }
            }
        }
    }
}

#[test]
fn ld_nondeterminism_inside_instant_uses_the_root_diagnostic() {
    let input = r#"model NestedChoice version "0.2" {
        type T = enum { ok, failed }
        actor A { capacity 1; capability work() -> T { duration fixed 0min; result nondet { ok } } }
        actor B { capacity 1; capability work() -> T { duration fixed 0min; result nondet { ok } } }
        actor C { capacity 1; capability work() -> T { duration fixed 0min; result nondet { ok, failed } } }
        process P { fork f { a = A.work(); b = B.work() }; join all f; c = C.work(); end done }
        cut Scope { include [A,B,C] }
        bounds { cases 1; live_tasks 2; clock 10min }
        simulation replications 2 seed 0 confidence 0.5
        verify reachable end.done
    }"#;
    for source in [
        input.to_owned(),
        input.replace("bounds {", "arrivals fixed [0min]\n        bounds {"),
    ] {
        let m = syntax::parse(&source)
            .and_then(|a| lower::lower(&a))
            .unwrap();
        let prod = engine::simulate(&m);
        let spec = reference::simulate(&m).outcome;
        assert_eq!(prod.completed_replications, 0);
        assert_eq!(spec.completed_replications, 0);
        let facts = relca_test_support::diagnostic::facts;
        assert_eq!(facts(&prod.diagnostics), facts(&spec.diagnostics));
        assert_eq!(prod.diagnostics[0].code, "U-SEM-0");
        assert_eq!(
            prod.diagnostics[0].message,
            "simulation encountered unresolved nondeterminism: replication 1, time 0, cases [1], sites [P.a,P.b]; at least two distinct successor actions remain"
        );
    }
}

#[test]
fn ld_range_and_normal_candidate_inside_instant_keep_the_root_diagnostic() {
    let input = r#"model NestedRange version "0.2" {
        type T = enum { ok }
        observed case { n: 0..1 = 1 }
        actor A { capacity 1; capability work() -> T { duration fixed 0min; result nondet { ok } } }
        actor B { capacity 1; capability work() -> T { duration fixed 0min; result nondet { ok } } }
        actor C { capacity 1; capability work() -> T { duration fixed 1min; result nondet { ok }; start { n += 1 } } }
        actor D { capacity 1; capability work() -> T { duration fixed 1min; result nondet { ok } } }
        process P { fork f { a = A.work(); b = B.work() }; join all f;
                    fork g { c = C.work(); d = D.work() }; join all g; end done }
        cut Scope { include [A,B,C,D] }
        bounds { cases 1; live_tasks 4; clock 10min }
        simulation replications 2 seed 0 confidence 0.5
        verify reachable end.done
    }"#;
    for source in [
        input.to_owned(),
        input.replace("bounds {", "arrivals fixed [0min]\n        bounds {"),
    ] {
        let m = syntax::parse(&source)
            .and_then(|a| lower::lower(&a))
            .unwrap();
        let prod = engine::simulate(&m);
        let spec = reference::simulate(&m).outcome;
        assert_eq!(prod.completed_replications, 0);
        assert_eq!(spec.completed_replications, 0);
        let facts = relca_test_support::diagnostic::facts;
        assert_eq!(facts(&prod.diagnostics), facts(&spec.diagnostics));
        assert_eq!(prod.diagnostics[0].code, "U-SEM-0");
        assert_eq!(
            prod.diagnostics[0].message,
            "simulation encountered unresolved nondeterminism: replication 1, time 0, cases [1], sites [P.a,P.b]; at least two distinct successor actions remain"
        );
    }
}

#[test]
fn ld_priority_admission_orders() {
    for (name, complete) in [
        ("ld-priority-admission", 2),
        ("ld-priority-admission.broken-one-slot", 0),
    ] {
        let s = src(name);
        let m = syntax::parse(&s).and_then(|a| lower::lower(&a)).unwrap();
        let engine = engine::simulate(&m);
        let reference = reference::simulate(&m).outcome;
        assert_eq!(
            engine.completed_replications, complete,
            "engine: {:?}",
            engine.diagnostics
        );
        assert_eq!(
            reference.completed_replications, complete,
            "reference: {:?}",
            reference.diagnostics
        );
        if complete != 0 {
            assert!(engine.same_time && reference.same_time);
            for out in [&engine, &reference] {
                for rep in &out.replications {
                    assert_eq!(rep.cases.len(), 4);
                    let metrics = serde_json::to_value(rep.metrics.as_ref().unwrap()).unwrap();
                    for id in [3, 4] {
                        let case = metrics["cases"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|c| c["case_id"] == id)
                            .unwrap();
                        assert_eq!(case["arrival"], 1);
                        let done = case["terminals"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|t| t["end"] == "done")
                            .unwrap();
                        assert_eq!(done["lead_time"]["min"], "3");
                        assert_eq!(done["lead_time"]["max"], "3");
                    }
                }
            }
        } else {
            let facts = relca_test_support::diagnostic::facts;
            assert_eq!(facts(&engine.diagnostics), facts(&reference.diagnostics));
            assert_eq!(engine.diagnostics[0].code, "U-SEM-0");
            assert_eq!(
                engine.diagnostics[0].message,
                "unresolved simulation nondeterminism: priority_tie_remaining: replication 1, time 2, cases [], sites []; at least two distinct successor actions remain"
            );
        }
    }
}

#[test]
fn ld_zero_duration_executor_is_part_of_closure() {
    let source = r#"model ZeroPerson version "0.2" {
        type T = enum { ok }
        people [a,b]
        actor A { executors [a,b]; capability work() -> T { duration fixed 0min; result nondet { ok } } }
        process P { x = A.work(); wait 1min; end done }
        cut C { include [A] }
        bounds { cases 1; live_tasks 1; clock 8min }
        simulation replications 2 seed 1 confidence 0.5
        verify reachable end.done
    }"#;
    for s in [
        source.to_owned(),
        source.replace(
            "cut C",
            "metrics { window 8min; currency \"JPY\" }\n        cut C",
        ),
    ] {
        let m = syntax::parse(&s).and_then(|a| lower::lower(&a)).unwrap();
        for (name, out) in [
            ("engine", engine::simulate(&m)),
            ("reference", reference::simulate(&m).outcome),
        ] {
            assert_eq!(
                out.completed_replications,
                0,
                "{name}, metrics={}",
                m.metrics.is_some()
            );
            assert_eq!(out.diagnostics[0].code, "U-SEM-0");
        }
    }
}

#[test]
fn ld_transient_invariant_is_seen_before_same_time_closure() {
    let source = src("ld-instant-observation.broken-transient");
    let original = "property case.Hit = reached(left == true && right == false)\n  metric quantile.First = quantile(1, case.Hit)";
    assert_eq!(source.matches(original).count(), 1);
    for (expect, guard) in [
        (4, "right == true || left == false || left == true"),
        (0, "right == true || left == false"),
    ] {
        let s = source.replace(
            original,
            &format!("contract CGuard {{ invariant {guard} }}"),
        );
        let m = syntax::parse(&s).and_then(|a| lower::lower(&a)).unwrap();
        for out in [engine::simulate(&m), reference::simulate(&m).outcome] {
            assert_eq!(
                out.completed_replications, expect,
                "{guard}: {:?}",
                out.diagnostics
            );
        }
    }
}

#[test]
fn ld_contest_not_filed_contract() {
    use relca_core::{Verdict, report};
    let declared = src("ld-contest-not-filed");
    let undeclared = src("ld-contest-not-filed.broken-undeclared");
    let lower = |text: &str| {
        lower::lower_with_selection(&syntax::parse(text).unwrap(), None, Some("Day")).unwrap()
    };
    let model = lower(&declared);
    for outcome in [
        engine::simulate(&model),
        reference::simulate(&model).outcome,
    ] {
        assert_eq!(outcome.completed_replications, 4);
        for rep in outcome.replications {
            assert!(rep.draws.is_empty());
            assert_eq!(
                rep.contests
                    .iter()
                    .map(|c| (c.case_id, c.filed))
                    .collect::<Vec<_>>(),
                [(1, Some(false)), (2, Some(false))]
            );
            let metrics = serde_json::to_value(rep.metrics.unwrap()).unwrap();
            for (i, (lead, wait)) in [("2", "0"), ("3", "1")].into_iter().enumerate() {
                let case = &metrics["cases"][i];
                assert_eq!(case["case_id"], i + 1);
                let returned = case["terminals"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|t| t["end"] == "returned")
                    .unwrap();
                assert_eq!(returned["lead_time"]["min"], lead);
                let task = case["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|t| t["binding"] == "v")
                    .unwrap();
                assert_eq!(task["wait"]["min"], wait);
            }
            let actors = metrics["actors"].as_array().unwrap();
            assert_eq!(actors.len(), 2);
            for (name, work) in [("Approver", "4"), ("Clerk", "0")] {
                let actor = actors.iter().find(|a| a["actor"] == name).unwrap();
                assert_eq!(actor["work"]["min"], work);
                assert_eq!(actor["work"]["max"], work);
            }
        }
    }
    let checked = report::check_with_selection(&declared, None, Some("Day"));
    let sampled = report::simulate_with_selection(&declared, None, Some("Day"));
    assert_eq!(sampled.verdict, Verdict::Inconclusive);
    let response = sampled
        .properties
        .iter()
        .find(|p| p["id"] == "contract K #2 contest_response")
        .unwrap();
    assert_eq!(response["verdict"], "inconclusive");
    assert_eq!(response["excluded"], "scenario_contest_not_filed");
    assert_eq!(checked.verdict, Verdict::Inconclusive);
    for (m, max) in [(&model, "0"), (&lower(&undeclared), "2")] {
        for out in [engine::explore(m), reference::explore(m).outcome] {
            let metrics = serde_json::to_value(out.metrics.unwrap()).unwrap();
            let clerk = metrics["actors"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["actor"] == "Clerk")
                .unwrap();
            assert_eq!(clerk["work"]["min"], "0");
            assert_eq!(clerk["work"]["max"], max);
        }
    }
    let m = lower(&undeclared);
    let production = engine::simulate(&m);
    let independent = reference::simulate(&m).outcome;
    for (outcome, provenance, span) in [
        (production, "cases [], sites []", (1031, 1114)),
        (independent, "cases [1], sites [P.v,contest2]", (940, 1026)),
    ] {
        assert_eq!(outcome.completed_replications, 0);
        let error = &outcome.diagnostics[0];
        assert_eq!(error.code, "U-SEM-0");
        assert_eq!(
            error.message,
            format!(
                "simulation encountered unresolved nondeterminism: replication 1, time 2, {provenance}; at least two distinct successor actions remain"
            )
        );
        let actual = error.span.unwrap();
        assert_eq!((actual.start, actual.end), span);
    }
    assert_eq!(
        report::check_with_selection(&undeclared, None, Some("Day")).verdict,
        Verdict::Satisfied
    );
}

#[test]
fn ld_compare_not_filed_keeps_the_terminal_table() {
    use relca_core::{Verdict, compare};
    let source = src("ld-contest-not-filed");
    let input = source.trim_end().strip_suffix('}').unwrap().to_owned()
        + "process Q { v = Approver.approve(); if v == pass { end ok } else { end returned } }\nconfiguration A { cut C; process P }\nconfiguration B { cut C; process Q }\n}";
    let report = compare::compare_with_scenario(
        &input,
        &["A", "B"],
        compare::Analysis::Simulation,
        Some("Day"),
    );
    assert_eq!(
        report.verdict,
        Verdict::Inconclusive,
        "{:?}",
        report.diagnostics
    );
    assert!(!report.tables.terminals.is_empty());
    let wire = serde_json::to_value(&report).unwrap();
    let rows: Vec<_> = wire["tables"]["properties"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["id"] == "contract K #2 contest_response")
        .collect();
    assert!(!rows.is_empty());
    for row in rows {
        let values = row["values"].as_array().unwrap();
        assert_eq!(values.len(), 2);
        assert!(
            values
                .iter()
                .all(|v| v["excluded"] == "scenario_contest_not_filed")
        );
    }
    assert!(
        report
            .diagnostics
            .iter()
            .all(|d| d.severity != relca_core::diag::Severity::Error)
    );
}

#[test]
fn ld_excluded_contest_does_not_mark_normal_work_on_the_same_capability() {
    let original = src("ld-contest-not-filed");
    assert_eq!(original.matches("process P { v").count(), 1);
    let input = original.replacen("process P { v", "process P { r = Clerk.recheck(); v", 1);
    let out = relca_core::report::simulate_with_selection(&input, None, Some("Day"));
    assert_eq!(out.simulation["completed_replications"], 4);
    let metrics = serde_json::to_value(out.metrics.as_ref().unwrap()).unwrap();
    let row = metrics["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["capability"] == "Clerk.recheck")
        .unwrap();
    assert_eq!(row["starts"]["mean"], "2");
    assert!(row.get("excluded").is_none());
    assert_eq!(
        out.cut_context
            .reentry
            .iter()
            .filter(|row| row.contains("Clerk.recheck (not filed in scenario Day)"))
            .count(),
        1
    );
    let response = out
        .properties
        .iter()
        .find(|p| p["id"] == "contract K #2 contest_response")
        .unwrap();
    assert_eq!(response["excluded"], "scenario_contest_not_filed");
}

#[test]
fn ld_excluded_contest_does_not_mark_filed_work_on_the_same_capability() {
    let input = src("ld-contest-not-filed")
        .replace("standing over [end.returned]", "standing over [end.returned,end.ok]")
        .replace("result nondet { fail }", "result nondet { pass }")
        .replace("respond_by 10min }", "respond_by 10min; contest Vendor on end.ok route Clerk.recheck reversible_by [a1] respond_by 10min rates { occurrence probability 1 basis \"sample\" service_time basis \"sample\" independence per_case_per_clause } }");
    let m =
        lower::lower_with_selection(&syntax::parse(&input).unwrap(), None, Some("Day")).unwrap();
    for out in [engine::simulate(&m), reference::simulate(&m).outcome] {
        assert_eq!(out.completed_replications, 4);
        for rep in &out.replications {
            assert_eq!(
                rep.contests
                    .iter()
                    .filter(|c| c.contest == 1 && c.filed == Some(true))
                    .count(),
                2
            );
        }
    }
    let out = relca_core::report::simulate_with_selection(&input, None, Some("Day"));
    assert!(
        out.diagnostics
            .iter()
            .all(|d| d.severity != relca_core::diag::Severity::Error),
        "{:?}",
        out.diagnostics
    );
    let metrics = serde_json::to_value(out.metrics.as_ref().unwrap()).unwrap();
    let row = metrics["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["capability"] == "Clerk.recheck")
        .unwrap();
    assert_eq!(row["starts"]["mean"], "2");
    assert!(row.get("excluded").is_none());
    let responses = metrics["contests"]["rows"].as_array().unwrap();
    assert_eq!(responses.len(), 2);
    assert!(
        responses
            .iter()
            .all(|r| r["target"] == "ok" && r["responded"] == 4)
    );
    assert!(
        out.cut_context
            .reentry
            .iter()
            .any(|row| row.contains("not filed in scenario Day"))
    );
}

#[test]
fn ld_aborted_check_has_unsupported_for_not_filed_response() {
    use relca_core::{Verdict, ir};
    let ast = syntax::parse(&src("ld-contest-not-filed")).unwrap();
    let mut m = lower::lower_with_selection(&ast, None, Some("Day")).unwrap();
    m.arrivals = Some(vec![1, 2]);
    m.capabilities
        .iter_mut()
        .find(|c| c.name == "approve")
        .unwrap()
        .duration = u64::MAX;
    for out in [engine::explore(&m), reference::explore(&m).outcome] {
        assert!(!out.diagnostics.is_empty(), "overflow must abort check");
        let response = out.properties.iter().find(|p| matches!(p.property, ir::Property::Contract(id) if matches!(m.contracts[id.0].kind, ir::ContractKind::ContestResponse { .. }))).unwrap();
        assert_eq!(response.verdict, Verdict::Unsupported);
        let cases = response.cases.as_ref().unwrap();
        assert_eq!(cases.len(), 2);
        assert!(
            cases
                .iter()
                .all(|case| case.verdict == Verdict::Unsupported)
        );
    }
}

#[test]
fn ld_cut_defer_of_not_filed_route_keeps_process_metrics() {
    use relca_core::report;
    let source = src("ld-contest-not-filed");
    let marker = "reentry [contest Vendor -> Clerk.recheck]";
    assert_eq!(source.matches(marker).count(), 1);
    let input = source.replace(
        marker,
        &format!("{marker}; defer Clerk.recheck reason \"unknown effort\" loses \"route work\""),
    );
    let out = report::simulate_with_selection(&input, None, Some("Day"));
    let wire = serde_json::to_value(out).unwrap();
    assert_eq!(wire["simulation"]["completed_replications"], 4);
    assert_eq!(wire["metrics"]["overall"]["work"]["mean"], "4");
    let actors = wire["metrics"]["actors"].as_array().unwrap();
    let approver = actors.iter().find(|a| a["actor"] == "Approver").unwrap();
    assert_eq!(approver["work"]["mean"], "4");
    let caps = wire["metrics"]["capabilities"].as_array().unwrap();
    let recheck = caps
        .iter()
        .find(|c| c["capability"] == "Clerk.recheck")
        .unwrap();
    assert_eq!(recheck["excluded"], "scenario_contest_not_filed");
    assert_eq!(recheck["starts"]["mean"], "0");
    assert_eq!(recheck["start_cost"]["reason"], "not_configured");
}

#[test]
fn ld_contest_not_filed_static() {
    let source = src("ld-contest-not-filed");
    let decl = "contest_not_filed Vendor on end.returned reason \"contest frequency unknown\"";
    let clause =
        "contest Vendor on end.returned route Clerk.recheck reversible_by [a1] respond_by 10min";
    for (text, code, message) in [
        (source.replace(decl, "").replace("  contract K", &format!("  {decl}\n  contract K")), "U-SYN-0", "contest_not_filed outside scenario"),
        (source.replace(decl, &decl.replace("end.returned", "end.ok")), "E-NAME-0", "no matching contest"),
        (source.replace(clause, &format!("{clause} rates {{ occurrence probability 0.5 basis \"x\" service_time basis \"x\" independence per_case_per_clause }}")), "U-SEM-0", "conflicts with declared rates"),
        (source.replace("contest frequency unknown", " "), "E-SYN-0", "reason must not be empty"),
        (source.replace(decl, &format!("{decl}\n    {decl}")), "U-SEM-0", "duplicate contest_not_filed"),
        (source.replace(clause, &format!("{clause}; {clause}")), "U-SEM-0", "ambiguous contest_not_filed"),
    ] {
        let diagnostics = syntax::parse(&text).and_then(|ast| lower::lower_with_selection(&ast, None, Some("Day"))).unwrap_err();
        assert!(diagnostics.iter().any(|d| d.code == code && d.message.contains(message)), "{diagnostics:?}");
    }
}

#[test]
fn ld_invalid_non_filing_with_rates_cannot_bypass_public_ir_guards() {
    let ast = syntax::parse(&src("ld-contest-not-filed")).unwrap();
    let mut m = lower::lower_with_selection(&ast, None, Some("Day")).unwrap();
    m.responsibility.as_mut().unwrap().contests[0].rates = Some(relca_core::contest::Rates {
        probability: relca_core::probability::Rational::decimal("0.5").unwrap(),
        occurrence_basis: "fixture".into(),
        service_time_basis: "fixture".into(),
        independence: "per_case_per_clause".into(),
    });
    for out in [engine::simulate(&m), reference::simulate(&m).outcome] {
        assert_eq!(out.attempted_replications, 0);
        assert_eq!(out.diagnostics[0].code, "U-SEM-0");
    }
}

#[test]
fn ld_scenario_exclusion_keys_two_clauses_by_end_not_route() {
    let input = src("ld-contest-not-filed")
        .replace("standing over [end.returned]", "standing over [end.returned,end.ok]")
        .replace("respond_by 10min }", "respond_by 10min; contest Vendor on end.ok route Clerk.recheck reversible_by [a1] respond_by 10min }")
        .replace("reason \"contest frequency unknown\"", "reason \"contest frequency unknown\"\n    contest_not_filed Vendor on end.ok reason \"second reason\"");
    let ast = syntax::parse(&input).unwrap();
    let m = lower::lower_with_selection(&ast, None, Some("Day")).unwrap();
    assert_eq!(m.contests().len(), 2);
    let s = m.scenario.as_ref().unwrap();
    for (id, target, reason) in [
        (0, "returned", "contest frequency unknown"),
        (1, "ok", "second reason"),
    ] {
        let relca_core::contest::ContestTarget::End(end) = m.contests()[id].target else {
            panic!("expected end target")
        };
        assert_eq!(m.ends[end.0], target);
        assert_eq!(s.contests_not_filed[&id], reason);
    }
    let report = relca_core::report::simulate_with_selection(&input, None, Some("Day"));
    let marks: Vec<_> = report
        .properties
        .iter()
        .filter(|r| r["excluded"] == "scenario_contest_not_filed")
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        marks,
        [
            "contract K #2 contest_response",
            "contract K #3 contest_response"
        ]
    );
}

#[test]
fn ld_dispatch_does_not_resolve_adoption() {
    let original = src("pol-tie-actor");
    let from = "policy fifo tie_by case_id then call_order";
    assert_eq!(original.matches(from).count(), 1);
    let input = original.replacen(
        from,
        "policy fifo tie_by case_id then call_order dispatch after_finishes",
        1,
    );
    let m = syntax::parse(&input)
        .and_then(|a| lower::lower(&a))
        .unwrap();
    for result in [engine::explore(&m), reference::explore(&m).outcome] {
        assert_eq!(
            result.case_outcomes.unwrap()[0].terminals[0].completed_at,
            Some(relca_core::outcome::Range { min: 3, max: 4 })
        );
    }
}
fn key(r: u64, c: u64, site: &str, n: u64) -> String {
    format!("relca-sim-v1|seed=0|rep={r}|case={c}|site={site}|n={n}")
}
fn first_half(key: &str) -> bool {
    let digest = Sha256::digest(key.as_bytes());
    u64::from_be_bytes(digest[..8].try_into().unwrap()) < 1u64 << 63
}
fn reps(v: &Value) -> &[Value] {
    v["replications"]
        .as_array()
        .expect("public replication-level outcomes")
}
fn draws(rep: &Value) -> Vec<String> {
    rep["draws"]
        .as_array()
        .expect("public per-path draw audit")
        .iter()
        .map(|d| d["key"].as_str().expect("full key").to_owned())
        .collect()
}
fn replication_projection(v: &Value) -> Value {
    let rows: Vec<_> = reps(v)
        .iter()
        .map(|rep| {
            let cases: Vec<_> = rep["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    json!({"case_id":c["case_id"],"arrival":c["arrival"],"terminal":c["terminal"],
                "queries":c["queries"],"witnesses":c["witnesses"],"deadlock":c["deadlock"]})
                })
                .collect();
            let mut keys = draws(rep);
            keys.sort();
            json!({"replication":rep["replication"],"cases":cases,"draw_keys":keys,
            "cut":rep["cut"],"witnesses":rep["witnesses"]})
        })
        .collect();
    json!(rows)
}
fn semantic_projection(v: &Value) -> Value {
    let mut out = json!({"replications":replication_projection(v)});
    for key in [
        "attempted_replications",
        "completed_replications",
        "truncated_replications",
        "properties",
        "case_outcomes",
        "admission",
    ] {
        out[key] = v[key].clone();
    }
    let diagnostics: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| {
            ["U-SEM-0", "W-BUDGET-0", "W-DEADLOCK-0"]
                .iter()
                .any(|c| d["code"] == *c)
        })
        .cloned()
        .collect();
    out["diagnostics"] = json!(
        diagnostics
            .into_iter()
            .map(relca_test_support::diagnostic::row_facts)
            .collect::<Vec<_>>()
    );
    out
}
fn hand(name: &str, v: &Value) {
    let expected_r = if name.starts_with("sim-range") {
        1
    } else if name.starts_with("sim-statistical") {
        10000
    } else {
        4
    };
    assert_eq!(
        reps(v).len(),
        expected_r,
        "{name}: every replication retained"
    );
    for (i, rep) in reps(v).iter().enumerate() {
        let r = i as u64 + 1;
        assert_eq!(rep["replication"], r, "{name}: one-based repetition");
        let cs = rep["cases"]
            .as_array()
            .expect("every declared case, including rejected/unarrived");
        let nc = if name.starts_with("sim-fifo") || name.starts_with("sim-rejection") {
            2
        } else {
            1
        };
        assert_eq!(cs.len(), nc);
        for (j, c) in cs.iter().enumerate() {
            assert_eq!(c["case_id"], j as u64 + 1);
            let queries = c["queries"]
                .as_array()
                .expect("query truth per case/replication");
            let expected_queries: Vec<&str> = if name.starts_with("sim-keys") {
                vec![
                    if name.contains("broken") {
                        "miss"
                    } else {
                        "hit"
                    },
                    if r <= 2 { "miss" } else { "hit" },
                ]
            } else if name.starts_with("sim-actor-initial") {
                vec![if r <= 2 { "miss" } else { "hit" }]
            } else if name.starts_with("sim-hazard") {
                vec![if name.contains("broken") && r <= 2 {
                    "unknown"
                } else {
                    "hit"
                }]
            } else if name.starts_with("sim-cut") {
                vec![
                    if name.contains("broken") {
                        "unknown"
                    } else {
                        "hit"
                    };
                    2
                ]
            } else if name.starts_with("sim-fifo") {
                vec!["hit"]
            } else if name.starts_with("sim-rejection") {
                vec![if name.contains("broken") {
                    "hit"
                } else {
                    "miss"
                }]
            } else if name.starts_with("sim-statistical") {
                vec![
                    if first_half(&key(r, 1, "kernel.P.a", 1)) {
                        "hit"
                    } else {
                        "miss"
                    };
                    3
                ]
            } else {
                vec![]
            };
            assert_eq!(
                queries,
                &serde_json::to_value(expected_queries)
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .clone(),
                "{name}/r{r}/case{}",
                j + 1
            );
            let expected_terminal = if name == "sim-deadlock.broken-zero-capacity"
                || name == "sim-cut.broken-short-clock"
                || (name == "sim-hazard.broken-short-clock" && r <= 2)
            {
                None
            } else if name == "sim-rejection" {
                Some(("rejected", None))
            } else if name.starts_with("sim-keys") {
                Some(("end", Some(if r <= 2 { "no" } else { "yes" })))
            } else {
                Some(("end", Some("done")))
            };
            match expected_terminal {
                None => assert!(
                    c["terminal"].is_null(),
                    "{name}/r{r}: not a success/failure terminal"
                ),
                Some((kind, end)) => {
                    assert_eq!(c["terminal"]["kind"], kind);
                    assert_eq!(c["terminal"]["end"], serde_json::to_value(end).unwrap());
                }
            }
        }
        let mut expected_draws = if name.starts_with("sim-keys") {
            vec![key(r, 1, "init.case.z", 1), key(r, 1, "kernel.P.a", 1)]
        } else if name.starts_with("sim-hazard") {
            let mut ks = vec![key(r, 1, "duration.P.a", 1)];
            if r <= 2 && !name.contains("broken") {
                ks.push(key(r, 1, "duration.P.a", 2));
            }
            ks
        } else if name.starts_with("sim-actor-initial") {
            vec![key(r, 0, "init.actor.A.z", 1)]
        } else if name.starts_with("sim-statistical") {
            vec![key(r, 1, "kernel.P.a", 1)]
        } else {
            vec![]
        };
        let mut actual = draws(rep);
        actual.sort();
        expected_draws.sort();
        assert_eq!(
            actual, expected_draws,
            "{name}/r{r}: full key audit, no extra draws"
        );
    }
}

#[test]
fn simulation_population_24_lower22_static2_runtime_u2_complete20_excluded0() {
    let mut names: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| {
            e.file_type().unwrap().is_dir() && e.file_name().to_string_lossy().starts_with("sim-")
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, FIXTURES);
    let (mut complete, mut runtime_u, mut static_invalid) = (0, 0, 0);
    for name in FIXTURES {
        let source = src(name);
        let h: Value = serde_json::from_str(
            &fs::read_to_string(root().join(name).join("expected.json")).unwrap(),
        )
        .unwrap();
        if h["verdict"] == "invalid" {
            static_invalid += 1;
            // Missing simulation is rejected by report::simulate BEFORE lower; check still accepts it.
            let v = serde_json::to_value(relca_core::report::simulate(&source)).unwrap();
            assert_eq!(v["verdict"], "invalid");
            assert!(
                v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "E-SIM-0")
            );
            continue;
        }
        let model = syntax::parse(&source)
            .and_then(|a| lower::lower(&a))
            .unwrap();
        let e = engine::simulate(&model);
        let r = reference::simulate(&model);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "{name}: no reference exclusions/limits"
        );
        let ev = serde_json::to_value(&e).unwrap();
        let rv = serde_json::to_value(&r.outcome).unwrap();
        if h["verdict"] == "unsupported" {
            runtime_u += 1;
            for v in [&ev, &rv] {
                assert_eq!(v["attempted_replications"], 1);
                assert_eq!(v["completed_replications"], 0);
                let ds = v["diagnostics"].as_array().unwrap();
                let d = ds.iter().find(|d| d["code"] == "U-SEM-0").unwrap();
                assert!(d["span"].is_object());
                assert!(
                    d["message"]
                        .as_str()
                        .unwrap()
                        .contains("replication 1, time 0")
                );
            }
        } else {
            complete += 1;
            hand(name, &ev);
            hand(name, &rv); // each interpreter FIRST against independent oracle
        }
        assert_eq!(
            semantic_projection(&ev),
            semantic_projection(&rv),
            "{name}: per-replication equality after independent expectations"
        );
    }
    assert_eq!((complete, runtime_u, static_invalid), (20, 2, 2));
}

fn both(s: &str) -> (Value, Value) {
    let m = syntax::parse(s).and_then(|a| lower::lower(&a)).unwrap();
    let e = serde_json::to_value(engine::simulate(&m)).unwrap();
    let r = reference::simulate(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    (e, serde_json::to_value(r.outcome).unwrap())
}

#[test]
fn s_key_sites_public_serial_bindings_and_actor_case0_old_new_routes() {
    let s = src("sim-keys")
        .replace("a = A.c(); if", "a = A.c(); b = A.c(); if")
        .replace("clock 2min", "clock 3min");
    let (e, r) = both(&s);
    for v in [&e, &r] {
        for (i, rep) in reps(v).iter().enumerate() {
            let mut actual = draws(rep);
            actual.sort();
            let mut expected = vec![
                key(i as u64 + 1, 1, "init.case.z", 1),
                key(i as u64 + 1, 1, "kernel.P.a", 1),
                key(i as u64 + 1, 1, "kernel.P.b", 1),
            ];
            expected.sort();
            assert_eq!(
                actual, expected,
                "same kernel/capability does not mean same site"
            );
            assert_eq!(
                rep["cases"][0]["queries"][1],
                ["miss", "hit", "hit", "hit"][i],
                "b selects first only in rep2; reached hits stay sticky"
            );
        }
    }
    assert_eq!(semantic_projection(&e), semantic_projection(&r));
    let old = src("sim-actor-initial");
    let new = old.replace("  bounds", "  arrivals fixed [0min]\n  bounds");
    let (e1, r1) = both(&old);
    let (e2, r2) = both(&new);
    for v in [&e1, &r1, &e2, &r2] {
        for (i, rep) in reps(v).iter().enumerate() {
            assert_eq!(draws(rep), vec![key(i as u64 + 1, 0, "init.actor.A.z", 1)]);
        }
    }
    assert_eq!(replication_projection(&e1), replication_projection(&e2));
    assert_eq!(replication_projection(&r1), replication_projection(&r2));
    let old = src("sim-keys");
    let new = old.replace("  bounds", "  arrivals fixed [0min]\n  bounds");
    let (e1, r1) = both(&old);
    let (e2, r2) = both(&new);
    hand("sim-keys", &e1);
    hand("sim-keys", &r1);
    hand("sim-keys", &e2);
    hand("sim-keys", &r2);
    assert_eq!(replication_projection(&e1), replication_projection(&e2));
    assert_eq!(replication_projection(&r1), replication_projection(&r2));
}

#[test]
fn s_key_occurrences_public_three_hazards_final_q1_and_rep_reset() {
    let s = src("sim-hazard")
        .replace("{1min:0.5,3min:0.5}", "{1min:0.25,2min:0.25,3min:0.5}")
        .replace("hit == true >= 0", "hit == true within 2min >= 0");
    let (e, r) = both(&s);
    for v in [&e, &r] {
        for (i, rep) in reps(v).iter().enumerate() {
            let expected_n = [3, 2, 1, 3][i];
            assert_eq!(
                draws(rep),
                (1..=expected_n)
                    .map(|n| key(i as u64 + 1, 1, "duration.P.a", n))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                rep["cases"][0]["queries"][0],
                ["miss", "hit", "hit", "miss"][i]
            );
        }
    }
    assert_eq!(semantic_projection(&e), semantic_projection(&r));
    // An unused duration/kernel declaration must not consume a key.
    let s=s.replace("  bounds", "  kernel Unused { * -> {1:ok} }\n  actor UnusedActor {capacity 1;capability x() -> T {duration {1min:1};outcomes kernel Unused}}\n  bounds");
    let (a, b) = both(&s);
    assert_eq!(replication_projection(&a), replication_projection(&e));
    assert_eq!(replication_projection(&b), replication_projection(&r));
}

/// Explicit, source-reasoned old-input allowlist, not discovered by running simulate.
/// The rest of the old populations retain their own exhaustive differential tests.
/// These entries cover old/new route, case/Actor initial/kernel/hazard/cut/reject/contract.
const OLD_ALLOWLIST: [(&str, Option<&str>, &str); 22] = [
    (
        "serial-time",
        None,
        "one-valued serial tasks, anonymous unique executor",
    ),
    (
        "cut-declared",
        None,
        "serial one-valued calls under explicit Cut",
    ),
    (
        "config-diff-add-actor",
        Some("AsIs"),
        "selected serial one-valued Before",
    ),
    (
        "config-diff-add-actor",
        Some("ToBe"),
        "selected serial one-valued After",
    ),
    (
        "contract-response",
        None,
        "single one-valued task, timely contract",
    ),
    (
        "contract-response.broken-late-finish",
        None,
        "same unique path, late contract witness",
    ),
    (
        "contract-response.broken-short-clock",
        None,
        "same unique path, model clock cut",
    ),
    (
        "state-invoice-write",
        None,
        "serial single-valued finish writes",
    ),
    (
        "state-invoice-write.broken-failed-approval",
        None,
        "same single path, deterministic invariant violation",
    ),
    (
        "prob-sum",
        None,
        "one kernel call, probability is chance not nondeterminism",
    ),
    (
        "prob-rows",
        None,
        "shared Actor initial distribution plus conditional kernel",
    ),
    (
        "prob-zero",
        None,
        "zero branches omitted, one positive kernel effect",
    ),
    ("duration-sum", None, "one task, hazard chance only"),
    (
        "duration-zero",
        None,
        "one task, probability-one positive duration",
    ),
    (
        "duration-clock.broken-short-clock",
        None,
        "one task and clock cut, no competing actions",
    ),
    (
        "arrivals-count",
        None,
        "FIFO capacity1 serializes two cases",
    ),
    (
        "prob-invoice",
        Some("AsIs"),
        "serial kernel calls; Actor chance and fixed case latent, no competing starts",
    ),
    (
        "arrivals-shared-fixed",
        None,
        "shared Actor fixed state, FIFO unique starts",
    ),
    (
        "arrivals-contracts",
        None,
        "FIFO capacity1 single task each, per-case contracts",
    ),
    (
        "arrivals-zero-queue",
        None,
        "all rejected before task creation",
    ),
    (
        "arrivals-zero-queue.broken-positive-queue",
        None,
        "staggered capacity1 FIFO unique path",
    ),
    (
        "arrivals-relative-within",
        None,
        "staggered arrivals avoid simultaneous starts despite capacity2",
    ),
];

// Full migration supplement: every declared configuration, including Hold and
// the otherwise unselected third invoice configuration. This is a NEW 179-input
// population, not a change to any old 28/39, 9/11, 4/8, etc. denominator.
// Classified from the old public IR before any simulate implementation existed:
// 50 direct + 30 selected deterministic inputs; 80 unresolved-choice inputs;
// 19 static rejections. The detailed 22-entry list above explains key examples.
const COMPLETE_DIRECT: [&str; 50] = [
    "serial-time",
    "cut-declared",
    "cut-declared.broken-missing-loses",
    "resource-contention.broken-zero-capacity",
    "arrivals-contracts",
    "arrivals-contracts.broken-short-response",
    "arrivals-count",
    "arrivals-order",
    "arrivals-reject-mass",
    "arrivals-reject-mass.broken-larger-queue",
    "arrivals-relative-within",
    "arrivals-relative-within.broken-tight-deadline",
    "arrivals-required",
    "arrivals-same-time-slot",
    "arrivals-same-time-slot.broken-later-arrival",
    "arrivals-shared-fixed",
    "arrivals-simultaneous",
    "arrivals-staggered",
    "arrivals-staggered.broken-extra-capacity",
    "arrivals-zero-queue",
    "arrivals-zero-queue.broken-positive-queue",
    "contract-cut",
    "contract-cut.broken-late-target",
    "contract-once",
    "contract-order",
    "contract-response",
    "contract-response.broken-late-finish",
    "contract-response.broken-short-clock",
    "contract-vacuous",
    "duration-clock",
    "duration-clock.broken-short-clock",
    "duration-duplicate",
    "duration-serial",
    "duration-serial.broken-tight-upper",
    "duration-sum",
    "duration-zero",
    "duration-zero.broken-late-positive",
    "exec-anonymous",
    "exec-separation",
    "prob-cut",
    "prob-cut.broken-short-clock",
    "prob-rows",
    "prob-sum",
    "prob-zero",
    "prob-zero.broken-positive-write",
    "state-invoice-write",
    "state-invoice-write.broken-failed-approval",
    "state-name-alias",
    "state-policy",
    "state-writer-order",
];
fn migration_complete(name: &str, cfg: Option<&str>) -> bool {
    if load_confluent_migration::accepted(name, cfg) {
        return true;
    }
    let Some(c) = cfg else {
        return COMPLETE_DIRECT.contains(&name);
    };
    match name {
        "config-diff-add-actor"
        | "config-diff-add-actor.broken-missing-intent"
        | "config-diff-add-actor.broken-other-cut" => ["AsIs", "ToBe", "Hold"].contains(&c),
        "config-diff-parallel"
        | "config-diff-parallel.broken-intent-only"
        | "config-diff-parallel.broken-no-configuration" => c == "AsIs",
        "config-diff-retire"
        | "config-diff-retire.broken-missing-loses"
        | "config-diff-terminal-union"
        | "config-diff-unreachable"
        | "config-diff-unreachable.broken-intent-only" => ["AsIs", "ToBe"].contains(&c),
        "contract-diff" | "contract-diff.broken-zero-deadline" => ["Before", "After"].contains(&c),
        "exec-vacuous" => ["AsIs", "ToBeA"].contains(&c),
        "prob-invoice" | "prob-invoice.broken-ai-transfer" => c == "AsIs",
        _ => false,
    }
}
fn migration_static(name: &str) -> Option<&'static str> {
    match name {
        "any-drain.broken-missing-cancel" => Some("E-CANCEL-0"),
        "arrivals-count.broken-case-count"
        | "arrivals-order.broken-descending"
        | "arrivals-required.broken-missing-arrivals" => Some("E-ARR-0"),
        "arrivals-shared-fixed.broken-unset-shared"
        | "exec-anonymous.broken-no-executor"
        | "state-name-alias.broken-missing-write"
        | "state-writer-order.broken-parallel-writers" => Some("U-SEM-0"),
        "config-diff-terminal-union.broken-unknown-terminal" => Some("E-NAME-0"),
        "cut-declared.broken-excluded-call" | "cut-declared.broken-missing-member" => {
            Some("E-CUT-1")
        }
        "duration-duplicate.broken-duplicate-value"
        | "duration-sum.broken-mass"
        | "prob-rows.broken-missing-difficult"
        | "prob-sum.broken-mass" => Some("E-PROB-0"),
        "parallel-time.broken-missing-join" => Some("E-SYN-0"),
        "serial-time.broken-missing-capacity" => Some("E-CAP-0"),
        "state-policy.broken-latent-difficulty" => Some("E-POL-LATENT"),
        _ => None,
    }
}
fn migration_reason(name: &str, cfg: Option<&str>) -> &'static str {
    if migration_static(name).is_some() {
        return "existing static rejection, no simulation default repairs it";
    }
    if let Some(reason) = load_confluent_migration::reason(name, cfg) {
        return reason;
    }
    if migration_complete(name, cfg) {
        if name == "resource-contention.broken-zero-capacity" {
            return "no start is enabled; genuine deadlock, not unresolved choice";
        }
        if name.starts_with("arrivals-") {
            return "one task per case; FIFO single resource or staggered starts; no simultaneous distinct enabled actions";
        }
        return "selected serial one-valued calls or declared chance only; fixed latent/single executor; model cut/deadlock remains a completed replication";
    }
    if name == "arrivals-simultaneous.broken-no-policy" {
        return "same-time cross-case starts without FIFO";
    }
    if name.starts_with("state-latent-product") {
        return "unspecified multi-valued initial latent is unresolved nondeterminism";
    }
    if name.starts_with("exec-authority")
        || name.starts_with("exec-permitted")
        || name.starts_with("exec-separation.broken")
        || name.starts_with("exec-vacuous.broken")
    {
        return "multiple eligible executor identities; permission/authority/separation monitor does not resolve allocation";
    }
    "reached fork has two distinct immediate start states, or a reached result nondet has multiple values; later equal terminals do not deduplicate"
}

#[test]
fn all_old_models_migration_148dirs_179inputs_160lower_106complete_54nondet_19static() {
    const METRICS_FIXTURES: [&str; 24] = [
        "met-cancel",
        "met-cancel.broken-drain",
        "met-compare",
        "met-compare.broken-longer-task",
        "met-diff",
        "met-diff.broken-free-start",
        "met-exclusions",
        "met-exclusions.broken-larger-queue",
        "met-fifo",
        "met-fifo.broken-shorter-second-step",
        "met-legacy",
        "met-legacy.broken-enabled",
        "met-missing-rate",
        "met-missing-rate.broken-free-rate",
        "met-parallel",
        "met-parallel.broken-one-slot",
        "met-person",
        "met-person.broken-shared-person",
        "met-positive-support",
        "met-positive-support.broken-positive-early-time",
        "met-sample",
        "met-sample.broken-short-clock",
        "met-serial",
        "met-serial.broken-longer-task",
    ];
    let mut metrics_names = Vec::new();
    const RESPONSIBILITY_FIXTURES: [&str; 19] = [
        "resp-contest",
        "resp-contest.broken-no-authority",
        "resp-contest.broken-no-permission",
        "resp-contest.broken-route-excluded",
        "resp-contest.broken-zero-deadline",
        "resp-cut",
        "resp-cut.broken-no-explicit-cut",
        "resp-diff",
        "resp-diff.broken-no-authorized-decision",
        "resp-missing-duty",
        "resp-missing-duty.broken-no-duty",
        "resp-origin",
        "resp-origin.broken-no-direct-read",
        "resp-refusal",
        "resp-refusal.broken-cut-guard",
        "resp-standing",
        "resp-standing.broken-not-party",
        "resp-state",
        "resp-state.broken-no-direct-read",
    ];
    let mut responsibility_names = Vec::new();
    use relca_core::ast;
    const COMPARE_FIXTURES: [&str; 16] = [
        "compare-arrivals",
        "compare-arrivals.broken-slow-plan",
        "compare-case-diff",
        "compare-case-diff.broken-same-kernel",
        "compare-cut",
        "compare-cut.broken-other-cut",
        "compare-legacy",
        "compare-legacy.broken-longer-task",
        "compare-random",
        "compare-random.broken-alias-process",
        "compare-reject",
        "compare-reject.broken-zero-queue",
        "compare-terminal",
        "compare-terminal.broken-other-end",
        "compare-unlock",
        "compare-unlock.broken-excluded-call",
    ];
    let mut compare_names = Vec::new();
    let mut dirs: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir() && !p.file_name().unwrap().to_str().unwrap().starts_with("sim-"))
        .filter(|p| {
            let name = p.file_name().unwrap().to_str().unwrap();
            if name.starts_with("ld-")
                || relca_test_support::fixture_match::intent_fixture(name)
                || relca_test_support::fixture_match::agency_limit_fixture(name)
                || relca_test_support::fixture_match::contract_scope_fixture(name)
            {
                return false;
            }
            if scenario_population::fixture(name)
                || (include!("support/waits_names.rs") as &[&str]).contains(&name)
            {
                return false;
            }
            if sinv_population::NAMES.contains(&name) {
                return false;
            }
            if lin_population::fixture(name)
                || aobs_population::fixture(name)
                || conflict_population::fixture(name)
                || serde_json::from_str::<Vec<String>>(include_str!("support/caseset_names.json"))
                    .unwrap()
                    .iter()
                    .any(|n| n == name)
                || done_population::fixture(name)
                || req_population::fixture(name)
                || authority_population::fixture(name)
                || resp2_population::fixture(name)
                || pcase_population::fixture(name)
                || integer_population::fixture(name)
                || diff2_population::fixture(name)
                || resource_population::fixture(name)
                || agency_viewpoints_population::fixture(name)
                || policy_population::fixture(name)
                || limits_population::fixture(name)
                || effect_population::fixture(name)
                || record_population::fixture(name)
                || mutation_default_population::fixture(name)
                || mutation_default_population::review_fixture(name)
                || include!("support/quality_names.rs").contains(&name)
                || serde_json::from_str::<Vec<String>>(include_str!("support/rule_names.json"))
                    .unwrap()
                    .iter()
                    .any(|n| n == name)
                || include!("support/evaluation_names.rs").contains(&name)
                || include!("support/quantile_names.rs").contains(&name)
                || include!("support/mean_names.rs").contains(&name)
                || include!("support/acceptance_gap_names.rs").contains(&name)
            {
                return false;
            }
            // ADR 0023 is independently accepted, not part of the old 148 dirs.
            const ACTOR_FIXTURES: [&str; 28] = [
                "act-arrival-units",
                "act-arrival-units.broken-later-first",
                "act-composite",
                "act-composite.broken-team-work",
                "act-control",
                "act-control.broken-explicit-environment",
                "act-cut-context",
                "act-cut-context.broken-no-cut",
                "act-debtor",
                "act-debtor-actor",
                "act-debtor-actor.broken-omitted",
                "act-debtor-value",
                "act-debtor-value.broken-other-person",
                "act-debtor.broken-unknown",
                "act-defaults",
                "act-defaults.broken-explicit-boundary",
                "act-grain",
                "act-grain.broken-swapped",
                "act-horizon",
                "act-horizon.broken-short-horizon",
                "act-legacy",
                "act-legacy.broken-longer-task",
                "act-overlap",
                "act-overlap.broken-relation",
                "act-units",
                "act-units.broken-duration-day",
                "act-via",
                "act-via.broken-upstream-only",
            ];
            if ACTOR_FIXTURES.contains(&name) {
                return false;
            }
            // Keep the old 148-dir migration oracle, not D1's new 12 inputs.
            const D1: [&str; 24] = [
                "ctx-excludes",
                "ctx-excludes.broken-start",
                "ctx-nonwaivable",
                "ctx-nonwaivable.broken-waive",
                "ctx-obligation",
                "ctx-obligation.broken-excludes-target",
                "ctx-waive",
                "ctx-waive-authority",
                "ctx-waive-authority.broken-no-authority",
                "ctx-waive-names",
                "ctx-waive-names.broken-unknown-response",
                "ctx-waive.broken-after-deadline",
                "ctx-contest",
                "ctx-contest-branches",
                "ctx-contest-branches.broken-zero-capacity",
                "ctx-contest-clock",
                "ctx-contest-clock.broken-short-clock",
                "ctx-contest-metrics",
                "ctx-contest-metrics.broken-missing-route-rate",
                "ctx-contest-origin",
                "ctx-contest-origin.broken-arrival-clock",
                "ctx-contest-resource",
                "ctx-contest-resource.broken-slow-route",
                "ctx-contest.broken-late",
            ];
            if D1.contains(&name) {
                return false;
            }
            if name.starts_with("explain-") {
                const EXPLAIN_FIXTURES: [&str; 12] = [
                    "explain-deadlock",
                    "explain-deadlock.broken-zero-capacity",
                    "explain-executors",
                    "explain-executors.broken-shared-worker",
                    "explain-invariant",
                    "explain-invariant.broken-cut-guard",
                    "explain-once",
                    "explain-once.broken-repeat-result",
                    "explain-requires",
                    "explain-requires.broken-cut-guard",
                    "explain-response",
                    "explain-response.broken-late-finish",
                ];
                assert!(
                    EXPLAIN_FIXTURES.contains(&name),
                    "unclassified explain fixture {name}"
                );
                return false;
            }
            if name.starts_with("mut-") {
                const MUTATION_FIXTURES: [&str; 12] = [
                    "mut-empty",
                    "mut-empty.broken-invalid",
                    "mut-evidence",
                    "mut-evidence.broken-no-independent-read",
                    "mut-gate",
                    "mut-gate.broken-unsupported",
                    "mut-guard",
                    "mut-guard.broken-no-contract",
                    "mut-limit",
                    "mut-limit.broken-many-sites",
                    "mut-origin",
                    "mut-origin.broken-route-only",
                ];
                assert!(
                    MUTATION_FIXTURES.contains(&name),
                    "unclassified mutation fixture {name}"
                );
                return false;
            }
            if name.starts_with("met-") {
                assert!(
                    METRICS_FIXTURES.contains(&name),
                    "unclassified metrics fixture {name}"
                );
                metrics_names.push(name.to_owned());
                false
            } else if name.starts_with("resp-") {
                assert!(
                    RESPONSIBILITY_FIXTURES.contains(&name),
                    "unclassified responsibility fixture {name}"
                );
                responsibility_names.push(name.to_owned());
                false
            } else if name.starts_with("compare-") {
                assert!(
                    COMPARE_FIXTURES.contains(&name),
                    "unclassified compare fixture {name}"
                );
                compare_names.push(name.to_owned());
                false
            } else {
                true
            }
        })
        .collect();
    compare_names.sort();
    assert_eq!(compare_names, COMPARE_FIXTURES);
    responsibility_names.sort();
    assert_eq!(responsibility_names, RESPONSIBILITY_FIXTURES);
    metrics_names.sort();
    assert_eq!(metrics_names, METRICS_FIXTURES);
    dirs.sort();
    assert_eq!(dirs.len(), 148);
    let (mut inputs, mut complete, mut nondet, mut rejected) = (0, 0, 0, 0);
    for dir in dirs {
        let name = dir.file_name().unwrap().to_str().unwrap();
        let s = fs::read_to_string(dir.join("model.relca")).unwrap();
        let old = match syntax::parse(&s) {
            Ok(a) => a,
            Err(ds) => {
                inputs += 1;
                rejected += 1;
                assert!(
                    ds.iter()
                        .any(|d| Some(d.code.as_str()) == migration_static(name))
                );
                continue;
            }
        };
        let mut configs: Vec<Option<String>> = old
            .items
            .iter()
            .filter_map(|i| {
                if let ast::Item::Configuration(c) = i {
                    Some(Some(c.name.value.clone()))
                } else {
                    None
                }
            })
            .collect();
        if configs.is_empty() {
            configs.push(None);
        }
        let i = s.rfind('}').unwrap();
        let added = format!(
            "{}\nsimulation replications 4 seed 0 confidence 0.5\n{}",
            &s[..i],
            &s[i..]
        );
        let ast = syntax::parse(&added).unwrap();
        for cfg in configs {
            inputs += 1;
            let cfg = cfg.as_deref();
            let model = if let Some(c) = cfg {
                lower::lower_configuration(&ast, c).map(|s| s.model)
            } else {
                lower::lower(&ast)
            };
            let model = match model {
                Ok(m) => {
                    assert!(
                        migration_static(name).is_none(),
                        "{name}: static input must stay rejected"
                    );
                    m
                }
                Err(ds) => {
                    rejected += 1;
                    assert!(
                        ds.iter()
                            .any(|d| Some(d.code.as_str()) == migration_static(name)),
                        "{name}/{cfg:?}: {ds:?}"
                    );
                    continue;
                }
            };
            let e = serde_json::to_value(engine::simulate(&model)).unwrap();
            let r = reference::simulate(&model);
            assert_eq!(
                r.stop,
                reference::StopReason::Complete,
                "{name}/{cfg:?}: no enumeration exclusions"
            );
            let rv = serde_json::to_value(r.outcome).unwrap();
            let reason = migration_reason(name, cfg);
            for v in [&e, &rv] {
                if migration_complete(name, cfg) {
                    assert_eq!(reps(v).len(), 4, "{name}/{cfg:?}: {reason}");
                    assert_eq!(v["completed_replications"], 4);
                } else {
                    assert_eq!(v["attempted_replications"], 1, "{name}/{cfg:?}: {reason}");
                    assert_eq!(v["completed_replications"], 0);
                    assert!(
                        v["diagnostics"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|d| d["code"] == "U-SEM-0"),
                        "{name}/{cfg:?}: {reason}"
                    );
                }
            }
            if migration_complete(name, cfg) {
                complete += 1;
            } else {
                nondet += 1;
            }
            assert_eq!(
                semantic_projection(&e),
                semantic_projection(&rv),
                "{name}/{cfg:?}: {reason}"
            );
        }
    }
    assert_eq!((inputs, complete, nondet, rejected), (179, 106, 54, 19));
}

#[path = "support/agency_viewpoints_acceptance.rs"]
mod agency_viewpoints_population;

#[test]
fn old_input_allowlist_22_complete22_excluded0_and_reasons_are_frozen() {
    assert_eq!(OLD_ALLOWLIST.len(), 22);
    let mut seen = std::collections::BTreeSet::new();
    for (name, configuration, reason) in OLD_ALLOWLIST {
        assert!(seen.insert((name, configuration)));
        assert!(!reason.is_empty());
        let s = src(name);
        let i = s.rfind('}').unwrap();
        let s = format!(
            "{}\nsimulation replications 4 seed 0 confidence 0.5\n{}",
            &s[..i],
            &s[i..]
        );
        let ast = syntax::parse(&s).unwrap();
        let m = if let Some(c) = configuration {
            lower::lower_configuration(&ast, c).map(|selected| selected.model)
        } else {
            lower::lower(&ast)
        }
        .unwrap();
        let e = serde_json::to_value(engine::simulate(&m)).unwrap();
        let r = reference::simulate(&m);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "{name}/{configuration:?}"
        );
        let r = serde_json::to_value(r.outcome).unwrap();
        assert_eq!(reps(&e).len(), 4, "{name}: no unanticipated nondeterminism");
        assert_eq!(reps(&r).len(), 4);
        assert_eq!(
            semantic_projection(&e),
            semantic_projection(&r),
            "{name}/{configuration:?}: {reason}"
        );
    }
}
