use super::*;
use serde_json::{Value, json};

const MODEL: &str = include_str!("../../../../fixtures/mean-metric-check/model.relca");

#[test]
fn hand_calculated_metric_checks() {
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/mean-metric-check/expected.json"
    ))
    .unwrap();
    let cases = [
        (MODEL.to_owned(), expected["metric_checks"][0].clone(), 0),
        (
            include_str!("../../../../fixtures/mean-metric-check.broken-nondet/model.relca").into(),
            json!({"constraint":"mean.Cost <= 50","verdict":"inconclusive","basis":"per_case_relaxation","interval":{"lower":"31","upper":"93"},"reason":null}),
            2,
        ),
        (
            include_str!("../../../../fixtures/mean-metric-check.broken-tight/model.relca").into(),
            json!({"constraint":"mean.Cost <= 45","verdict":"violated","basis":"exact","interval":{"lower":"93/2","upper":"93/2"},"reason":null}),
            1,
        ),
        (
            once(
                MODEL,
                "include [A, B]",
                "include [A, B]; defer Foo reason \"unknown\" loses \"cost\"",
            ),
            json!({"constraint":"mean.Cost <= 50","verdict":"inconclusive","basis":null,"interval":null,"reason":"not_configured"}),
            2,
        ),
    ];
    for (source, row, exit) in cases {
        check(&source, &row, exit);
    }
    for (op, bound, verdict, exit) in [
        ("<=", 4, "inconclusive", 2),
        ("<=", 3, "violated", 1),
        (">=", 3, "satisfied", 0),
    ] {
        let constraint = format!("mean.Lead {op} {bound}");
        let source = once(
            &once(MODEL, "clock 20min", "clock 5min"),
            "mean.Cost <= 50",
            &format!("{constraint}min"),
        );
        check(
            &source,
            &json!({"constraint":constraint,"verdict":verdict,"basis":"lower_bound","interval":{"lower":"29/8","upper":null},"reason":"truncated"}),
            exit,
        );
    }
}

fn check(source: &str, row: &Value, exit: i32) {
    syntax::parse(source).unwrap();
    let reference = check_using(source, |m| crate::reference::explore(m).outcome);
    for r in [super::check(source), reference] {
        assert_eq!(r.verdict.exit_code(), exit);
        let json = serde_json::to_value(r).unwrap();
        assert_eq!(&json["metric_checks"][0], row);
        serde_json::from_value::<Report>(json).unwrap();
    }
    let explained = crate::explain::explain(source);
    assert_eq!(explained.report.verdict.exit_code(), exit);
    assert_eq!(
        &serde_json::to_value(explained).unwrap()["metric_checks"][0],
        row
    );
}

#[test]
fn sampled_cohort_uses_confidence_not_the_point() {
    let s = once(
        &MODEL
            .lines()
            .filter(|l| !l.trim_start().starts_with("metric mean."))
            .collect::<Vec<_>>()
            .join("\n"),
        "verify metric mean.Cost <= 50",
        "metric cohort.All = expected(count(case.Any)) / arrivals\n  simulation replications 20 seed 1 confidence 0.95\n  verify metric cohort.All >= 0.5\n  verify metric cohort.All >= 1",
    );
    for r in [
        simulate(&s),
        simulate_using(&s, None, |m| crate::reference::simulate(m).outcome),
    ] {
        let rows = &r.metric_checks;
        assert_eq!(r.simulation["multiplicity"]["rows"], 2);
        assert_eq!(r.simulation["multiplicity"]["alpha_after"], "1/80");
        assert_eq!(rows[0].verdict, Verdict::Satisfied);
        assert_eq!(rows[1].verdict, Verdict::Inconclusive);
        assert_eq!(rows[0].basis.as_deref(), Some("statistical"));
        assert_eq!(rows[0].interval, rows[1].interval);
        assert_eq!(r.verdict, Verdict::Inconclusive);
    }
}

#[test]
fn metric_thresholds_are_resolved_against_the_declaration() {
    for (old, new, code) in [
        ("mean.Cost <= 50", "mean.Missing <= 50", "E-NAME-0"),
        ("mean.Cost <= 50", "mean.Lead <= 50", "U-SYN-0"),
        ("mean.Cost <= 50", "mean.Cost <= 50min", "U-SYN-0"),
        ("verify reachable end.done", "", "U-SEM-0"),
    ] {
        assert!(
            super::check(&once(MODEL, old, new))
                .diagnostics
                .iter()
                .any(|d| d.code == code)
        );
    }
    let r = super::check(&once(MODEL, "mean.Cost <= 50", "mean.Lead < 1h"));
    assert_eq!(r.metric_checks[0].constraint, "mean.Lead < 60");
    assert_eq!(r.verdict, Verdict::Satisfied);
}

#[test]
fn mutate_does_not_gate_on_unassigned_metric_checks() {
    let s = once(MODEL, "outcomes kernel K", "result nondet { pass, fail }");
    let r = crate::mutation::mutate_set_with_scenario(&s, "bpr-default", None);
    assert_eq!(r.original_verdict, Some(Verdict::Satisfied));
    assert!(r.unchecked.iter().any(|s| s == "metric_constraints"));
}

#[test]
fn probability_and_cohort_share_the_same_planned_family() {
    let mixed = include_str!("../../../../fixtures/mean-mixed-family/model.relca");
    let probability = "  verify probability reached end.done >= 0.9\n";
    let metric = "  verify metric cohort.OnTime >= 0.9\n";
    assert_eq!(mixed.matches(probability).count(), 1);
    assert_eq!(mixed.matches(metric).count(), 1);
    for (source, rows, alpha, verdict, checks) in [
        (mixed.to_owned(), 2, "1/80", Verdict::Inconclusive, 1),
        (
            include_str!(
                "../../../../fixtures/mean-mixed-family.broken-no-probability/model.relca"
            )
            .into(),
            1,
            "1/40",
            Verdict::Inconclusive,
            1,
        ),
        (
            include_str!(
                "../../../../fixtures/mean-mixed-family.broken-no-cohort-check/model.relca"
            )
            .into(),
            1,
            "1/40",
            Verdict::Satisfied,
            0,
        ),
    ] {
        for r in [
            simulate(&source),
            simulate_using(&source, None, |m| crate::reference::simulate(m).outcome),
        ] {
            assert_eq!(r.simulation["multiplicity"]["rows"], rows);
            assert_eq!(r.simulation["multiplicity"]["alpha_after"], alpha);
            assert_eq!(r.verdict, verdict);
            assert_eq!(r.metric_checks.len(), checks);
            for check in r.metric_checks {
                assert_eq!(check.alpha.unwrap().to_string(), alpha);
                assert_eq!(
                    check.verdict,
                    if rows == 2 {
                        Verdict::Inconclusive
                    } else {
                        Verdict::Satisfied
                    }
                );
            }
        }
    }
}

fn once(s: &str, from: &str, to: &str) -> String {
    assert_eq!(s.matches(from).count(), 1, "{from}");
    s.replacen(from, to, 1)
}

#[test]
fn strict_comparisons_at_exact_endpoints_and_missing_values() {
    // Cost = (3·21+3·41+83+103)/8 = 93/2, exactly the bound.
    for (op, verdict, exit) in [
        ("<=", "satisfied", 0),
        ("<", "violated", 1),
        (">=", "satisfied", 0),
        (">", "violated", 1),
    ] {
        let source = once(MODEL, "mean.Cost <= 50", &format!("mean.Cost {op} 46.5"));
        check(
            &source,
            &json!({"constraint":format!("mean.Cost {op} 93/2"),"verdict":verdict,"basis":"exact","interval":{"lower":"93/2","upper":"93/2"},"reason":null}),
            exit,
        );
    }
    // A true Done is reached only on the pass branch (3/4). At clock 5,
    // Lead given reached must withhold both the interval and the lower bound.
    let done = once(
        MODEL,
        "  property case.Any",
        "  property case.Done = reached(end.done)\n  property case.Any",
    );
    for (s, reason) in [
        (
            once(&done, "mean(elapsed, case.Any)", "mean(elapsed, case.Done)"),
            "beyond_reached_mass",
        ),
        (
            once(
                &once(MODEL, "clock 20min", "clock 5min"),
                "mean(elapsed, case.Any)",
                "mean(elapsed, case.Any given reached)",
            ),
            "truncated",
        ),
        (
            once(
                &once(
                    MODEL,
                    "  bounds { cases 1; live_tasks 1; clock 20min }",
                    "  scenario Day { arrivals fixed [0min]; horizon 5min; bounds { cases 1; live_tasks 1; clock 20min } }",
                ),
                "mean(elapsed, case.Any)",
                "mean(elapsed, case.Any given reached)",
            ),
            "undetermined",
        ),
        (
            once(
                &once(
                    &done,
                    "reached(end.done)\n",
                    "reached(end.done) within 1min\n",
                ),
                "mean(elapsed, case.Any)",
                "mean(elapsed, case.Done given reached)",
            ),
            "zero_denominator",
        ),
    ] {
        let source = once(&s, "mean.Cost <= 50", "mean.Lead <= 50min");
        if reason == "undetermined" {
            let r = check_with_selection(&source, None, Some("Day"));
            assert_eq!(r.metric_checks[0].reason.as_deref(), Some(reason));
            assert_eq!(r.metric_checks[0].verdict, Verdict::Inconclusive);
            assert!(r.metric_checks[0].interval.is_none());
            assert_eq!(r.verdict, Verdict::Inconclusive);
        } else {
            check(
                &source,
                &json!({"constraint":"mean.Lead <= 50","verdict":"inconclusive","basis":null,"interval":null,"reason":reason}),
                2,
            );
        }
    }
    let s = once(MODEL, "; rate B.fix per_start 2", "");
    check(
        &s,
        &json!({"constraint":"mean.Cost <= 50","verdict":"inconclusive","basis":null,"interval":null,"reason":"not_configured"}),
        2,
    );
    // Any is reached on every complete path, so the exhaustive cohort is [1,1].
    let s = once(
        MODEL,
        "verify metric mean.Cost <= 50",
        "metric cohort.All = expected(count(case.Any)) / arrivals\n  verify metric cohort.All >= 1",
    );
    check(
        &s,
        &json!({"constraint":"cohort.All >= 1","verdict":"satisfied","basis":"exact","interval":{"lower":"1","upper":"1"},"reason":null}),
        0,
    );
}

#[test]
fn cohort_components_count_both_properties_every_case_and_every_check() {
    let double = include_str!("../../tests/support/cohort-ratio-rows.relca");
    let one_row = once(double, "  verify metric cohort.Ratio >= 0\n", "");
    let one_case = once(
        &once(&one_row, "cases 2", "cases 1"),
        "fixed [0min, 2min]",
        "fixed [0min]",
    );
    for (source, rows, alpha, upper, checks, verdict) in [
        (one_case.as_str(), 2, "1/8", "8", 1, Verdict::Satisfied),
        (one_row.as_str(), 4, "1/16", "16", 1, Verdict::Inconclusive),
        (double, 8, "1/32", "32", 2, Verdict::Inconclusive),
    ] {
        for r in [
            simulate(source),
            simulate_using(source, None, |m| crate::reference::simulate(m).outcome),
        ] {
            assert_eq!(r.simulation["multiplicity"]["rows"], rows);
            assert_eq!(r.simulation["multiplicity"]["alpha_after"], alpha);
            assert_eq!(r.metric_checks.len(), checks);
            assert_eq!(r.metric_checks[0].verdict, verdict);
            if checks == 2 {
                assert_eq!(r.metric_checks[1].verdict, Verdict::Satisfied);
            }
            for c in r.metric_checks {
                assert_eq!(c.alpha.unwrap().to_string(), alpha);
                assert_eq!(
                    serde_json::to_value(c.interval).unwrap(),
                    json!({"lower":alpha, "upper":upper})
                );
            }
        }
    }
}

#[test]
fn absent_samples_and_denominator_and_equal_lower_endpoints_are_not_fabricated() {
    let s = once(MODEL, "outcomes kernel K", "result nondet { pass, fail }");
    for (op, verdict, exit) in [(">=", "satisfied", 0), (">", "inconclusive", 2)] {
        let source = once(&s, "mean.Cost <= 50", &format!("mean.Cost {op} 31"));
        check(
            &source,
            &json!({"constraint":format!("mean.Cost {op} 31"),"verdict":verdict,"basis":"per_case_relaxation","interval":{"lower":"31","upper":"93"},"reason":null}),
            exit,
        );
    }
    let s = once(
        &s,
        "metric mean.Lead = mean(elapsed, case.Any)",
        "property case.Done = reached(end.done)\n  metric mean.Lead = mean(elapsed, case.Done given reached)",
    );
    check(
        &once(&s, "mean.Cost <= 50", "mean.Lead >= 0min"),
        &json!({"constraint":"mean.Lead >= 0","verdict":"inconclusive","basis":null,"interval":null,"reason":"denominator_may_be_zero"}),
        2,
    );

    let source = include_str!("../../../../fixtures/mean-first-hit-fork.broken-clock/model.relca");
    for (op, verdict) in [
        (">=", Verdict::Satisfied),
        (">", Verdict::Inconclusive),
        ("<", Verdict::Violated),
        ("<=", Verdict::Inconclusive),
    ] {
        let source = once(
            source,
            "metric mean.Work =",
            &format!("verify metric mean.Work {op} 2min\n  metric mean.Work ="),
        );
        for r in [
            super::check(&source),
            check_using(&source, |m| crate::reference::explore(m).outcome),
        ] {
            let row = &r.metric_checks[0];
            assert_eq!(row.verdict, verdict);
            assert_eq!(
                serde_json::to_value(&row.interval).unwrap(),
                json!({"lower":"2","upper":null})
            );
        }
    }
    let m = lower::lower(
        &syntax::parse(include_str!(
            "../../../../fixtures/mean-mixed-family/model.relca"
        ))
        .unwrap(),
    )
    .unwrap();
    for (abort, reason) in [(true, "aborted"), (false, "no_samples")] {
        let q = crate::quality::simulation(&m, &[], abort);
        let rows = crate::evaluation::metric_checks(&m, Some(&q), true, None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].verdict, Verdict::Unsupported);
        assert_eq!(rows[0].reason.as_deref(), Some(reason));
        assert!(rows[0].interval.is_none());
    }
}
