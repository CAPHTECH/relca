use relca_core::{engine, lower, reference, syntax};
use serde_json::Value;
use std::path::PathBuf;

fn fixtures() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut paths: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("mean-")
                && ![
                    "mean-close-contest",
                    "mean-compare",
                    "mean-conditional-arrivals",
                    "mean-first-hit-fork",
                    "mean-metric-check",
                    "mean-mixed-family",
                    "mean-repeat",
                    "mean-simulate",
                ]
                .iter()
                .any(|n| p.file_name().unwrap().to_str().unwrap().starts_with(n))
        })
        .collect();
    paths.sort();
    paths
}

#[test]
fn means_match_hand_values_in_both_independent_interpreters() {
    for path in fixtures() {
        let source = std::fs::read_to_string(path.join("model.relca")).unwrap();
        let expected: Value =
            serde_json::from_str(&std::fs::read_to_string(path.join("expected.json")).unwrap())
                .unwrap();
        let ast = syntax::parse(&source).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
        let m = lower::lower(&ast).unwrap();
        let e = engine::explore(&m);
        let mut r = reference::explore(&m);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "{}",
            path.display()
        );
        for q in [&e.quality, &r.outcome.quality] {
            assert_eq!(
                serde_json::to_value(q).unwrap()["means"],
                expected["quality"]["means"],
                "{}",
                path.display()
            );
        }
        assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
        r.outcome.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, r.outcome, "{}", path.display());
    }
}

#[test]
fn mean_static_boundaries_and_reserved_word_policy() {
    let s = std::fs::read_to_string(
        fixtures()
            .into_iter()
            .find(|p| p.ends_with("mean-chance"))
            .unwrap()
            .join("model.relca"),
    )
    .unwrap();
    for (old, new, code) in [
        ("mean(elapsed, case.Any)", "mean(elapsed)", "U-SYN-0"),
        ("given reached", "given case.Done", "U-SYN-0"),
        ("case.Any)", "case.Unknown)", "E-NAME-0"),
        ("mean.Done =", "mean.Lead =", "U-SEM-0"),
        (
            "metric mean.Done = mean(elapsed, case.Done given reached)",
            "metric quantile.Lead = quantile(0.5, case.Done)",
            "U-SEM-0",
        ),
    ] {
        let source = s.replacen(old, new, 1);
        let ds = match syntax::parse(&source) {
            Err(ds) => ds,
            Ok(a) => lower::lower(&a).unwrap_err(),
        };
        assert!(ds.iter().any(|d| d.code == code), "{new}: {ds:?}");
    }
    let source = s
        .replace("actor A", "actor mean")
        .replace("A.run", "mean.run")
        .replace("[A, B]", "[mean, B]")
        .replace("rate A per", "rate mean per");
    lower::lower(&syntax::parse(&source).unwrap()).unwrap();
}

#[test]
fn mean_only_adds_declared_observations_and_explain_transports_them() {
    for path in fixtures() {
        let s = std::fs::read_to_string(path.join("model.relca")).unwrap();
        let plain = s
            .lines()
            .filter(|s| !s.trim_start().starts_with("metric mean."))
            .collect::<Vec<_>>()
            .join("\n");
        let mut m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
        let p = lower::lower(&syntax::parse(&plain).unwrap()).unwrap();
        let mut observed = engine::explore(&m);
        let original = engine::explore(&p);
        let explained = serde_json::to_value(relca_core::explain::explain(&s)).unwrap();
        assert_eq!(
            explained["quality"]["means"],
            serde_json::to_value(&observed.quality).unwrap()["means"]
        );
        let q = observed.quality.as_mut().unwrap();
        q.means = None;
        q.checked.retain(|s| s != "means");
        q.unchecked.retain(|s| s != "means");
        assert_eq!(observed, original, "{}", path.display());
        m.quality.as_mut().unwrap().means[0].property = Some(usize::MAX);
        assert!(
            engine::explore(&m)
                .diagnostics
                .iter()
                .any(|d| d.code == "U-SEM-0")
        );
        assert_eq!(
            reference::explore(&m).stop,
            reference::StopReason::SemanticAbort
        );
    }
}

#[test]
fn mean_simulation_with_unresolved_nondeterminism_fails_closed() {
    let s = std::fs::read_to_string(
        fixtures()
            .into_iter()
            .find(|p| p.ends_with("mean-chance"))
            .unwrap()
            .join("model.relca"),
    )
    .unwrap();
    let s = s
        .replace("outcomes kernel K", "result nondet { pass, fail }")
        .replace(
            "  verify reachable",
            "  simulation replications 4 seed 1 confidence 0.95\n  verify reachable",
        );
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    let report = relca_core::report::simulate(&s);
    assert_eq!(report.verdict, relca_core::Verdict::Unsupported);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    for out in [&e, &r.outcome] {
        assert_eq!(out.attempted_replications, 1);
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.message.contains("nondeterminism"))
        );
        let q = out.quality.as_ref().unwrap();
        assert!(
            q.means
                .as_ref()
                .unwrap()
                .iter()
                .all(|r| r["value"]["reason"] == "aborted"
                    && r["value"]["point"].is_null()
                    && r["lower_bound"].is_null())
        );
        assert!(!q.checked.iter().any(|s| s == "means"));
    }
}

#[test]
fn reference_mean_tree_fold_does_not_name_the_dag_reducers() {
    let src = include_str!("../src/reference/quality/mean.rs");
    for forbidden in [
        "graph_mean",
        "crate::engine",
        "crate::quality::range",
        "crate::quality::first_reduce",
    ] {
        assert!(!src.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn mean_rewards_use_the_same_observation_boundaries_on_both_routes() {
    let mut faults = Vec::new();
    for (name, src, expected) in [
        (
            "close-contest",
            include_str!("../../../fixtures/mean-close-contest/model.relca"),
            include_str!("../../../fixtures/mean-close-contest/expected.json"),
        ),
        (
            "first-hit-fork",
            include_str!("../../../fixtures/mean-first-hit-fork/model.relca"),
            include_str!("../../../fixtures/mean-first-hit-fork/expected.json"),
        ),
    ] {
        let want: Value = serde_json::from_str(expected).unwrap();
        for arrivals in [false, true] {
            let source = if arrivals {
                src.replace("  bounds", "  arrivals fixed [0min]\n  bounds")
            } else {
                src.to_owned()
            };
            let m = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
            assert_eq!(m.legacy_route(), !arrivals, "{name}");
            for (interpreter, rows) in ["engine", "reference"].into_iter().zip(both_means(&source))
            {
                let actual: Vec<_> = rows
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| &r["value"])
                    .collect();
                eprintln!("{name}, explicit_arrivals={arrivals}, {interpreter}: {actual:?}");
                if rows != want["quality"]["means"] {
                    faults.push(format!(
                        "{name}, explicit_arrivals={arrivals}, {interpreter}"
                    ));
                }
            }
            let short = if name == "first-hit-fork" {
                source.replace("clock 5min", "clock 1min")
            } else {
                continue;
            };
            for rows in both_means(&short) {
                for (i, lb) in ["2", "2", "8"].into_iter().enumerate() {
                    assert_eq!(rows[i]["value"]["reason"], "truncated");
                    assert!(rows[i]["value"]["lower"].is_null());
                    assert_eq!(rows[i]["lower_bound"], lb);
                }
            }
        }
    }
    assert!(faults.is_empty(), "{faults:?}");
}

#[test]
fn conditional_arrivals_use_a_ratio_of_expected_totals() {
    let src = include_str!("../../../fixtures/mean-conditional-arrivals/model.relca");
    let expected: Value = serde_json::from_str(include_str!(
        "../../../fixtures/mean-conditional-arrivals/expected.json"
    ))
    .unwrap();
    let deferred = src.replace("  cut C { include [A] }", "  actor Idle { capacity 1 }\n  cut C { include [A,Idle]; defer Idle reason \"unused\" loses \"idle analysis\" }");
    for s in [src, &deferred] {
        for rows in both_means(s) {
            assert_eq!(rows, expected["quality"]["means"]);
        }
    }
    for rows in both_means(&src.replace("within 4min", "within 1min")) {
        assert_eq!(rows[0]["value"]["reason"], "zero_denominator");
        assert!(rows[0]["value"]["lower"].is_null() && rows[0]["lower_bound"].is_null());
    }
}

#[test]
fn public_ir_rejects_a_shared_cohort_and_mean_name() {
    let src = include_str!("../../../fixtures/mean-conditional-arrivals/model.relca");
    let mut m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    m.quality.as_mut().unwrap().metrics[0].name = "Lead".into();
    assert!(
        engine::explore(&m)
            .diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0")
    );
    assert_eq!(
        reference::explore(&m).stop,
        reference::StopReason::SemanticAbort
    );
}

fn both_means(source: &str) -> Vec<Value> {
    let m = lower::lower(&syntax::parse(source).unwrap()).unwrap();
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    vec![
        serde_json::to_value(e.quality).unwrap()["means"].clone(),
        serde_json::to_value(r.outcome.quality).unwrap()["means"].clone(),
    ]
}

#[test]
fn mean_work_and_cost_freeze_at_first_hit_before_same_tick_later_start() {
    let source = r#"model FirstReward version "0.2" {
      type T = enum { ok }
      type Flag = enum { off,on }
      actor A { capacity 1; state flag: Flag = off
        capability run() -> T { duration fixed 2min; result nondet { ok }; writes flag; finish { flag=on } } }
      actor B { capacity 1; capability fix() -> T { duration fixed 3min; result nondet { ok } } }
      process P { a=A.run(); b=B.fix(); end done }
      cut C { include [A,B] }
      bounds { cases 1; live_tasks 1; clock 10min }
      metrics { window 10min; currency "JPY"; rate A per_minute 10; rate A.run per_start 1; rate B per_minute 20; rate B.fix per_start 2 }
      verify reachable end.done
      property case.Early = reached(A.flag == on)
      metric mean.Work = mean(work, case.Early)
      metric mean.Cost = mean(cost, case.Early given reached)
      metric mean.Closed = mean(cost)
    }"#;
    for rows in both_means(source) {
        assert_eq!(rows[0]["value"]["lower"], "2");
        assert_eq!(rows[1]["value"]["lower"], "21");
        assert_eq!(rows[2]["value"]["lower"], "83");
        assert!(
            rows.as_array()
                .unwrap()
                .iter()
                .all(|r| r["value"]["basis"] == "exact")
        );
    }
}

#[test]
fn mean_partial_work_and_cost_are_bounds_not_complete_or_clock_plus_one_work() {
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/mean-work/model.relca"),
    )
    .unwrap()
    .replace("clock 20min", "clock 5min");
    for rows in both_means(&source) {
        for (i, lb) in [(0, "29/8"), (1, "7/2"), (2, "0"), (3, "83/2")] {
            assert_eq!(rows[i]["value"]["reason"], "truncated");
            assert!(rows[i]["value"]["lower"].is_null());
            assert_eq!(rows[i]["lower_bound"], lb);
        }
        assert_eq!(rows[4]["value"]["reason"], "truncated");
        assert!(rows[4]["lower_bound"].is_null());
    }
}

#[test]
fn mean_close_can_be_declared_without_case_properties_and_rates_are_not_invented() {
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/mean-people/model.relca"),
    )
    .unwrap();
    let source = source
        .lines()
        .filter(|s| !s.contains("property case.") && !s.contains("mean.Cost"))
        .collect::<Vec<_>>()
        .join("\n");
    for rows in both_means(&source) {
        assert_eq!(rows[0]["value"]["lower"], "2");
        assert_eq!(rows[1]["value"]["lower"], "5");
    }
    let source = source
        .lines()
        .filter(|s| !s.trim_start().starts_with("metrics {"))
        .collect::<Vec<_>>()
        .join("\n");
    for rows in both_means(&source) {
        assert!(
            rows.as_array()
                .unwrap()
                .iter()
                .all(|r| r["value"]["reason"] == "not_configured" && r["lower_bound"].is_null())
        );
    }
}

#[test]
fn mean_horizon_withdraws_unfinished_rewards_without_calling_them_closed() {
    let source = r#"model HorizonReward version "0.2" {
      type T = enum { ok }
      actor A { capacity 1; capability run() -> T { duration fixed 10min; result nondet { ok } } }
      process P { a=A.run(); end done }
      cut C { include [A] }
      scenario Day { arrivals fixed [0min]; horizon 5min; bounds { cases 1; live_tasks 1; clock 20min } }
      metrics { window 20min; currency "JPY"; rate A per_minute 10; rate A.run per_start 1 }
      verify reachable end.done
      property case.Done = reached(end.done)
      metric mean.Lead = mean(elapsed, case.Done)
      metric mean.Work = mean(work)
      metric mean.Cost = mean(cost)
    }"#;
    let m =
        lower::lower_with_selection(&syntax::parse(source).unwrap(), None, Some("Day")).unwrap();
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for q in [e.quality, r.outcome.quality] {
        let q = q.unwrap();
        let rows = q.means.unwrap();
        for (i, lb) in [(0, "6"), (1, "5"), (2, "51")] {
            assert_eq!(rows[i]["value"]["reason"], "undetermined");
            assert!(rows[i]["value"]["lower"].is_null());
            assert_eq!(rows[i]["lower_bound"], lb);
        }
    }
}

#[test]
fn mean_tariff_uses_the_same_occupancy_definition_and_missing_overtime_is_not_zero() {
    let source = r#"model TariffReward version "0.2" {
      type T = enum { ok }
      calendar Day through 11min on_close continue { [0min, 2min) capacity 1; [2min, 11min) capacity 0 }
      actor A { capacity 1; calendar Day; capability run() -> T { duration fixed 5min; result nondet { ok } } }
      process P { a=A.run(); end done }
      cut C { include [A] }
      bounds { cases 1; live_tasks 1; clock 10min }
      metrics { window 10min; currency "JPY"; rate A per_minute 10; rate A overtime_per_minute 20; rate A.run per_start 1 }
      verify reachable end.done
      metric mean.Cost = mean(cost)
    }"#;
    for rows in both_means(source) {
        assert_eq!(rows[0]["value"]["lower"], "81");
    }
    for rows in both_means(&source.replace("rate A overtime_per_minute 20;", "")) {
        assert_eq!(rows[0]["value"]["reason"], "not_configured");
        assert!(rows[0]["lower_bound"].is_null());
    }
    // Missing pricing strictly after an absorbing first hit must not revoke
    // an already known zero reward at receipt. The closed cost is still unknown.
    let early=source.replace("rate A overtime_per_minute 20;", "")
        .replace("  type T", "  observed case { ready: bool = true }\n  type T")
        .replace("  metric mean.Cost", "  property case.Early = reached(ready == true)\n  metric mean.Early = mean(cost, case.Early)\n  metric mean.Cost");
    for rows in both_means(&early) {
        assert_eq!(rows[0]["value"]["lower"], "0");
        assert_eq!(rows[0]["value"]["basis"], "exact");
        assert_eq!(rows[1]["value"]["reason"], "not_configured");
    }
}

#[test]
fn mean_closed_rewards_include_the_contest_route_but_first_end_rewards_do_not() {
    let source = include_str!("../../relca-cli/tests/snapshots/inputs/q9-rates.relca");
    let source = source.replace(
        "  verify reachable",
        r#"  property case.Returned = reached(end.returned)
  metric mean.EndCost = mean(cost, case.Returned)
  metric mean.CloseCost = mean(cost)
  metric mean.CloseWork = mean(work)
  verify reachable"#,
    );
    // Judge costs zero. Route work E=1/4*(3/4*2+1/4*4)=5/8.
    // Route fee E=1/4*(10*5/2+2)=27/4. Judge work adds 1.
    for rows in both_means(&source) {
        for (i, want) in [(0, "0"), (1, "27/4"), (2, "13/8")] {
            assert_eq!(rows[i]["value"]["lower"], want);
            assert_eq!(rows[i]["value"]["upper"], want);
            assert_eq!(rows[i]["value"]["basis"], "exact");
        }
    }
}

#[test]
fn mean_rule_models_leave_means_unchecked_without_withdrawing_existing_quality() {
    let source = include_str!("../../relca-cli/tests/snapshots/inputs/q9-rates.relca");
    // A fixed Rule is valid today; ADR0078 nevertheless leaves mean unconnected.
    let source = source
        .replace(
            "  type T",
            "  actor Rule { version \"r1\"; decides enabled: bool = true }\n  type T",
        )
        .replace("[Judge,Reviewer,Party]", "[Judge,Reviewer,Party,Rule]")
        .replace(
            "  verify reachable",
            "  metric mean.Cost = mean(cost)\n  verify reachable",
        );
    let m = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for q in [e.quality, r.outcome.quality] {
        let q = q.unwrap();
        assert!(q.checked.iter().any(|s| s == "outcomes_per_arrival"));
        assert!(!q.checked.iter().any(|s| s == "means"));
        assert!(q.unchecked.iter().any(|s| s == "means"));
        assert_eq!(q.means.as_ref().unwrap()[0]["value"]["reason"], "aborted");
        let _: relca_core::quality::Quality =
            serde_json::from_value(serde_json::to_value(q).unwrap()).unwrap();
    }
}
