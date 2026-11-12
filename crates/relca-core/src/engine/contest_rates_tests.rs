use super::*;
use crate::contest::q9_input as input;
#[path = "../../tests/support/q9_oracle.rs"]
mod hand;

fn model(name: &str) -> ir::Model {
    crate::lower::lower(&crate::syntax::parse(&input::source(name)).unwrap()).unwrap()
}

#[test]
fn q9_i8_reducer_matches_hand_values_for_each_independent_interpreter() {
    for name in [
        "c",
        "half",
        "fixed",
        "deadline",
        "late",
        "clock",
        "deadlock",
        "two-spaced",
        "two-burst",
        "calendar",
        "calendar-later",
        "normal-call",
        "ordinal",
        "conditional-target",
        "mixed-unreached",
    ] {
        let m = model(name);
        for out in [
            super::super::simulate(&m),
            crate::reference::simulate(&m).outcome,
        ] {
            assert_eq!(out.completed_replications, 16, "{name}");
            let table = crate::metrics::contests::summarize(&m, &out, false)
                .unwrap()
                .unwrap();
            hand::subset(
                &hand::expected_table(name),
                &serde_json::to_value(&table).unwrap(),
                name,
            )
            .unwrap();
            let mut metrics = crate::report::summarize_metrics(&out, false).unwrap();
            metrics.contests = Some(Box::new(table));
            serde_json::from_value::<crate::metrics::BusinessMetrics>(
                serde_json::to_value(metrics).unwrap(),
            )
            .unwrap();
        }
    }
    for name in ["same-tick", "two-clauses", "mixed", "route-nondet"] {
        let m = model(name);
        for out in [
            super::super::simulate(&m),
            crate::reference::simulate(&m).outcome,
        ] {
            assert!(
                out.diagnostics
                    .iter()
                    .any(|d| d.severity == diag::Severity::Error)
            );
            let table = crate::metrics::contests::summarize(&m, &out, true)
                .unwrap()
                .unwrap();
            hand::validate_sim(
                name,
                &serde_json::json!({"mode":"simulation","verdict":"unsupported",
                "diagnostics":[{"code":"U-SEM-0"}],"metrics":{"contests":table}}),
            )
            .unwrap();
        }
    }
}

#[test]
fn q9_i8_real_reducer_accepts_hand_assembled_histories_not_dedup_frequencies() {
    use crate::outcome::{ContestObservation, ReplicationOutcome, SimulationOutcome};
    let m = model("c");
    let mut out = SimulationOutcome {
        completed_replications: 16,
        ..SimulationOutcome::default()
    };
    for replication in 1..=16 {
        let duration = if replication <= 12 {
            0
        } else if replication <= 15 {
            2
        } else {
            4
        };
        let filed = duration != 0;
        out.replications.push(ReplicationOutcome {
            contests: vec![ContestObservation {
                contest: 0,
                case_id: 1,
                target_known: true,
                target_at: Some(1),
                filed: Some(filed),
                started_at: filed.then_some(1),
                response_at: filed.then_some(1 + duration),
                closed_at: Some(1 + duration),
                failed: false,
            }],
            lost_updates: Vec::new(),
            admission: None,
            scenario_observed_until: None,
            metrics: None,
            replication,
            draws: Vec::new(),
            cases: Vec::new(),
            cut: None,
            overcommit: false,
        });
    }
    let table = crate::metrics::contests::summarize(&m, &out, false)
        .unwrap()
        .unwrap();
    let expected = hand::table(vec![hand::row(
        1,
        "1/4",
        16,
        4,
        1,
        hand::sample("5/2", 2, 4, 4),
        hand::sample("0", 0, 0, 4),
        hand::sample("5/2", 2, 4, 4),
    )]);
    assert_eq!(serde_json::to_value(table).unwrap(), expected);
    out.replications[0].contests.clear();
    assert!(crate::metrics::contests::summarize(&m, &out, false).is_err());
}

#[test]
fn q9_i8_decoder_checks_hand_tables_against_real_metric_inventory() {
    let report = crate::report::simulate(&input::source("fixed"));
    let mut metrics = serde_json::to_value(report.metrics.unwrap()).unwrap();
    metrics["contests"] = hand::expected_table("fixed");
    serde_json::from_value::<crate::metrics::contests::Table>(metrics["contests"].clone()).unwrap();
    serde_json::from_value::<crate::metrics::BusinessMetrics>(metrics.clone()).unwrap_or_else(
        |e| {
            panic!(
                "{e}; tasks={}; status={}; unit={}",
                metrics["cases"][0]["tasks"], metrics["status"], metrics["statistical_unit"]
            )
        },
    );
    for (label, bad) in hand::wire_faults() {
        let mut changed = metrics.clone();
        changed["contests"] = bad;
        assert!(
            serde_json::from_value::<crate::metrics::BusinessMetrics>(changed).is_err(),
            "{label}"
        );
    }
}

#[test]
fn q9_i7_each_interpreter_records_actual_route_history_before_reduction() {
    for (name, bank) in [
        ("c", hand::C),
        ("half", hand::HALF),
        ("ordinal", hand::ORDINAL),
    ] {
        let m = model(name);
        for out in [
            super::super::simulate(&m),
            crate::reference::simulate(&m).outcome,
        ] {
            assert_eq!(out.completed_replications, 16);
            for (rep, duration) in out.replications.iter().zip(bank) {
                assert_eq!(rep.contests.len(), 1);
                let row = &rep.contests[0];
                assert_eq!((row.contest, row.case_id), (0, 1));
                assert!(row.target_known);
                assert_eq!(row.target_at, Some(1));
                assert_eq!(row.filed, Some(duration != 0));
                assert_eq!(row.started_at, (duration != 0).then_some(1));
                assert_eq!(row.response_at, (duration != 0).then_some(1 + duration));
                assert_eq!(row.closed_at, Some(1 + duration));
                assert!(!row.failed);
            }
        }
    }
    for name in ["clock", "normal-call", "deadlock"] {
        let m = model(name);
        let e = super::super::simulate(&m);
        let r = crate::reference::simulate(&m).outcome;
        for (e, r) in e.replications.iter().zip(&r.replications) {
            assert_eq!(e.contests, r.contests, "{name}");
            let row = &e.contests[0];
            assert_eq!(row.filed, Some(true));
            if name == "clock" {
                assert!(row.response_at.is_none());
                assert!(!row.failed);
            } else if name == "normal-call" {
                assert_eq!(
                    (row.target_at, row.started_at, row.response_at),
                    (Some(3), Some(3), Some(5))
                );
            } else {
                assert!(row.failed);
                assert!(row.response_at.is_none());
            }
        }
    }
}

#[test]
fn q9_i4_sampled_management_uses_clause_key_and_disjoint_route_site() {
    for (name, ordinal) in [("c", 1), ("ordinal", 2), ("fixed", 1)] {
        let m = model(name);
        let mut s = State::initial(&m, 0);
        s.contest.as_mut().unwrap().routes[0].target_at = Some(1);
        for rep in 1..=16 {
            let mut random = simulation::Sample::new(1, rep);
            let Some(Step::Next(states)) =
                contest_control(&s, &m, &mut random, &mut Exploration::default()).unwrap()
            else {
                panic!()
            };
            assert_eq!(states.len(), 1);
            assert_eq!(random.draws.len(), 1, "only occurrence is sampled");
            assert_eq!(
                random.draws[0].key,
                format!(
                    "relca-sim-v1|seed=1|rep={rep}|case=1|site=contest.Flow.clause{ordinal}.occurrence|n=1"
                )
            );
            if name == "fixed" {
                assert_eq!(
                    states[0].contest.as_ref().unwrap().routes[0].filed,
                    Some(true)
                );
            }
        }
        let site = m.contest_site(0);
        assert_eq!(
            simulation::Sample::task_site(&m, site, "duration"),
            "duration.Flow.contest:1"
        );
        let mut random = simulation::Sample::new(1, 1);
        random.note_task(&m, site, false);
        assert_eq!(random.provenance[0].1, "Flow.contest:1");
        assert_eq!(random.provenance[0].2, m.contests()[0].span);
    }
}

#[test]
fn q9_i3_management_transports_one_chance_action_not_two_choices() {
    for (name, weights) in [
        ("c", vec![vec!["3/4", "1/4"]]),
        ("fixed", vec![vec!["1"]]),
        ("legacy", vec![vec!["1"], vec!["1"]]),
    ] {
        let m = model(name);
        if name == "fixed" {
            assert!(m.uses_probability());
            assert!(!m.legacy_route());
        }
        let end = m.contests()[0].target.end().unwrap();
        let mut local = State::initial(&m, 0);
        local.time = 1;
        local.contest.as_mut().unwrap().business = Some(BusinessEnd {
            end,
            at: 1,
            work: 1,
            result_at: None,
        });
        local.contest.as_mut().unwrap().routes[0].target_at = Some(1);
        let mut s = initial(&m, 1);
        s.time = 1;
        s.cursor = 1;
        s.phase = Phase::Mixed;
        s.cases[0] = Slot::Admitted(
            Case {
                local,
                queued_at: Vec::new(),
                ever_started: true,
                received: true,
                admitted_at: Some(0),
                terminal: Some(end),
            }
            .into(),
        );
        let actions = transition(&s, &m, &[0], &[], 0, &mut Room(100), &mut Facts::new(1))
            .unwrap()
            .unwrap();
        let actual: Vec<Vec<_>> = actions
            .iter()
            .map(|a| a.iter().map(|(p, _)| p.to_string()).collect())
            .collect();
        assert_eq!(actual, weights, "{name}");
        for (_, next) in actions.iter().flatten() {
            let c = next.cases[0].payload().unwrap();
            assert!(c.local.contest.as_ref().unwrap().routes[0].filed.is_some());
            assert!(
                c.local.tasks.is_empty(),
                "dispatch cannot precede management"
            );
        }
    }
}
