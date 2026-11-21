use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

const MODEL: &str = include_str!("../../../fixtures/mean-simulate/model.relca");
const POOLED: &str = include_str!("../../../fixtures/mean-simulate/pooled.relca");

fn runs(s: &str) -> [relca_core::outcome::SimulationOutcome; 2] {
    let m = lower::lower_with_selection(&syntax::parse(s).unwrap(), None, Some("Busy")).unwrap();
    [engine::simulate(&m), reference::simulate(&m).outcome]
}

fn means(o: &relca_core::outcome::SimulationOutcome) -> &[Value] {
    assert_eq!(o.completed_replications, 2, "{:?}", o.diagnostics);
    o.quality.as_ref().unwrap().means.as_deref().unwrap()
}

#[test]
fn deterministic_busy_scenario_observes_all_arrivals_and_first_hits() {
    let want: Value = serde_json::from_str(include_str!(
        "../../../fixtures/mean-simulate/expected.json"
    ))
    .unwrap();
    for o in runs(MODEL) {
        for (i, row) in means(&o).iter().enumerate() {
            assert_eq!(row["value"], want["quality"]["means"][i]["value"]);
            assert!(row["lower_bound"].is_null());
        }
    }
    let r = report::simulate_with_selection(MODEL, None, Some("Busy"));
    assert_eq!(json!(r.verdict), want["verdict"]);
    let plain = MODEL
        .lines()
        .filter(|l| !l.contains("metric mean."))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        report::simulate_with_selection(&plain, None, Some("Busy")).verdict,
        r.verdict
    );
    let wire = serde_json::to_value(&r).unwrap();
    serde_json::from_value::<report::SimulationReport>(wire).unwrap();
}

#[test]
fn simulation_means_pool_cases_not_replication_averages() {
    let plain = POOLED
        .lines()
        .filter(|l| !l.contains("metric mean."))
        .collect::<Vec<_>>()
        .join("\n");
    for (o, p) in runs(POOLED).iter().zip(runs(&plain)) {
        assert_eq!(
            means(o)[0]["value"],
            json!({"point":"10/3","observations":3,"censored":0,"reason":null})
        );
        assert_eq!(o.replications, p.replications, "including keyed draws");
    }
}

#[test]
fn busy_horizon_and_clock_withhold_points() {
    for (old, new, reason) in [
        ("horizon 12min", "horizon 8min", "undetermined"),
        ("clock 20min", "clock 8min", "truncated"),
    ] {
        for o in runs(&MODEL.replace(old, new)) {
            for row in means(&o) {
                assert_eq!(row["value"]["reason"], reason);
                assert!(row["value"]["point"].is_null() && row["lower_bound"].is_null());
                assert_eq!(row["value"]["censored"], 6);
            }
        }
    }
}

#[test]
fn sampled_mean_check_does_not_use_its_point_estimate() {
    let s = MODEL.replace(
        "  metric mean.Lead =",
        "  verify metric mean.Lead <= 100min\n  metric mean.Lead =",
    );
    let r = report::simulate_with_selection(&s, None, Some("Busy"));
    assert_eq!(r.verdict, relca_core::Verdict::Unsupported);
    assert_eq!(
        r.metric_checks[0].reason.as_deref(),
        Some("no_statistical_bound")
    );
    assert_eq!(r.metric_checks[0].interval, None);
    assert_eq!(r.quality.unwrap().means.unwrap()[0]["value"]["point"], "5");
}

#[test]
fn sampled_missing_and_out_of_scope_values_remain_null() {
    for (source, i, reason) in [
        (
            MODEL.replace("rate A.run per_start 1", ""),
            4,
            "not_configured",
        ),
        (
            POOLED.replace("within 4min", "within 1min"),
            0,
            "zero_denominator",
        ),
        (
            POOLED.replace(" given reached", ""),
            0,
            "beyond_reached_mass",
        ),
        (
            MODEL.replace(
                "include [A]",
                "include [A]; defer Foo reason \"unknown\" loses \"time\"",
            ),
            0,
            "not_configured",
        ),
    ] {
        for o in runs(&source) {
            let row = &means(&o)[i];
            assert_eq!(row["value"]["reason"], reason);
            assert!(row["value"]["point"].is_null());
        }
    }
}

#[test]
fn range_stop_exactly_at_horizon_is_not_a_business_censor() {
    let src = r#"model RangeAtHorizon version "0.2" {
      type T = enum { ok }
      observed case { n: 0..1 = 1 }
      actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok }; writes n; finish { n += 1 } } }
      process P { x=A.run(); end done }
      cut C { include [A] }
      scenario Busy { arrivals fixed [0min]; horizon 1min; bounds { cases 1; live_tasks 1; clock 10min } }
      simulation replications 2 seed 3 confidence 0.95
      verify safety no_resource_overcommit
      property case.Done = reached(end.done)
      metric mean.Lead = mean(elapsed, case.Done)
    }"#;
    for out in runs(src) {
        assert_eq!(out.range_stops.len(), 2);
        assert!(out.range_stops.iter().all(|(_, rs)| rs[0].time == 1));
        assert!(out.replications.iter().all(|r| r.cut.is_none()));
        assert_eq!(
            means(&out)[0]["value"],
            json!({"point":null,"observations":0,"censored":2,"reason":"truncated"})
        );
    }
    let multi = src
        .replacen("capacity 1", "capacity 2", 1)
        .replacen("fixed 1min", "fixed 2min", 1)
        .replacen(
            "[0min]; horizon 1min; bounds { cases 1; live_tasks 1",
            "[0min,1min]; horizon 2min; bounds { cases 2; live_tasks 2",
            1,
        );
    assert!(multi.contains("[0min,1min]; horizon 2min"));
    for out in runs(&multi) {
        assert_eq!(out.range_stops.len(), 2);
        assert!(
            out.range_stops
                .iter()
                .all(|(_, rs)| rs.len() == 1 && rs[0].time == 2)
        );
        assert!(
            out.replications
                .iter()
                .all(|r| r.cut.is_none() && r.cases[1].terminal.is_none())
        );
        assert_eq!(
            means(&out)[0]["value"],
            json!({"point":null,"observations":0,"censored":4,"reason":"truncated"})
        );
    }
}
