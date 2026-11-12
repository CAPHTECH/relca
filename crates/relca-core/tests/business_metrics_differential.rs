//! Each implementation first meets hand arithmetic, then all metric fields agree.
//! No limits/exclusions may silently reduce the 24-dir / 28-input population.
#[allow(dead_code)]
#[path = "business_metrics.rs"]
mod acceptance;
use acceptance::{FIXTURES, case, eq, hand, row, source, terminal};
use relca_core::{engine, lower, reference, syntax};
use serde_json::{Value, json};

fn model(s: &str, cfg: Option<&str>) -> relca_core::ir::Model {
    let a = syntax::parse(s).expect("fixture syntax must be accepted");
    match cfg {
        Some(c) => lower::lower_configuration(&a, c).map(|x| x.model),
        None => lower::lower(&a),
    }
    .expect("fixture must lower")
}

#[test]
fn m_differential_24_dirs_28_inputs_24_exhaustive_4_simulation_complete28_exclusions0() {
    let (mut selected, mut exhaustive, mut simulation, mut complete) = (0, 0, 0, 0);
    for name in FIXTURES {
        let s = source(name);
        let sim = name.starts_with("met-sample") || name.starts_with("met-exclusions");
        let choices: &[Option<&str>] =
            if name.starts_with("met-compare") || name.starts_with("met-diff") {
                &[Some("AsIs"), Some("ToBeP")]
            } else {
                &[None]
            };
        for &cfg in choices {
            selected += 1;
            let m = model(&s, cfg);
            if sim {
                simulation += 1;
                let e = engine::simulate(&m);
                let r = reference::simulate(&m);
                assert_eq!(
                    r.stop,
                    reference::StopReason::Complete,
                    "{name}: no exclusions"
                );
                complete += 1;
                let ev = serde_json::to_value(&e).unwrap();
                let rv = serde_json::to_value(&r.outcome).unwrap();
                per_rep_hand(name, &ev);
                per_rep_hand(name, &rv);
                let er = &ev["replications"];
                let rr = &rv["replications"];
                assert_eq!(
                    er, rr,
                    "{name}: full per-rep observations including every metrics field"
                );
                assert_eq!(e.completed_replications, r.outcome.completed_replications);
                hand(name, cfg, &acceptance::run(&s, cfg, true)["metrics"]);
            } else {
                exhaustive += 1;
                let e = engine::explore(&m);
                let r = reference::explore(&m);
                assert_eq!(
                    r.stop,
                    reference::StopReason::Complete,
                    "{name}: no exclusions"
                );
                complete += 1;
                // Public Outcome.metrics is the proposed ADR 0019 seam. Do not
                // compare only report, which would miss reference-collector bugs.
                let ev = serde_json::to_value(&e.metrics).unwrap();
                let rv = serde_json::to_value(&r.outcome.metrics).unwrap();
                hand(name, cfg, &ev);
                hand(name, cfg, &rv);
                assert_eq!(
                    ev, rv,
                    "{name}/{cfg:?}: every new metric, null reason and count"
                );
                assert_eq!(e, r.outcome, "legacy semantics also agree");
            }
        }
    }
    assert_eq!(
        (selected, exhaustive, simulation, complete),
        (28, 24, 4, 28)
    );
}

/// A replication metric is a single observation, not a pooled cohort sample.
/// Accept raw canonical number or a singleton exhaustive scalar as the DTO
/// representation; never accept an absent field or a simulation aggregate.
fn observed<'a>(v: &'a Value, key: &str) -> &'a Value {
    let value = v
        .get(key)
        .unwrap_or_else(|| panic!("missing observation {key}: {v}"));
    if value.is_object() {
        assert_eq!(value.get("min"), value.get("max"));
        assert!(value.get("min").is_some());
        assert!(value.get("mean").is_none());
        assert!(value["reason"].is_null());
        &value["min"]
    } else {
        value
    }
}
fn obs(v: &Value, key: &str, want: &str) {
    assert_eq!(observed(v, key), &json!(want), "{key}");
}
fn per_rep_hand(name: &str, v: &Value) {
    let reps = v["replications"].as_array().expect("replication audit");
    assert_eq!(reps.len(), 4);
    let short = name.ends_with("broken-short-clock");
    let larger = name.ends_with("broken-larger-queue");
    for (i, rep) in reps.iter().enumerate() {
        let r = i + 1;
        let m = &rep["metrics"];
        assert!(m.is_object(), "{name}/r{r}: replication metrics");
        let o = &m["overall"];
        if name.starts_with("met-sample") {
            let t = if short || i % 2 == 0 { "2" } else { "4" };
            obs(o, "work", t);
            obs(o, "observed_until", t);
            obs(o, "observed_cost_total", if t == "2" { "17" } else { "31" });
            let mut keys = vec![format!(
                "relca-sim-v1|seed=41020|rep={r}|case=1|site=duration.P.job|n=1"
            )];
            if !short && i % 2 == 1 {
                keys.push(format!(
                    "relca-sim-v1|seed=41020|rep={r}|case=1|site=duration.P.job|n=2"
                ));
            }
            let actual: Vec<_> = rep["draws"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d["key"].as_str().unwrap().to_owned())
                .collect();
            assert_eq!(
                actual, keys,
                "golden full keys, including q=1, no unused n2"
            );
        } else {
            obs(o, "work", "3");
            obs(o, "observed_until", "3");
            obs(o, "observed_cost_total", "16");
            obs(
                row(&m["capabilities"], "capability", "A.run"),
                "starts",
                "2",
            );
            assert_eq!(rep["draws"], json!([]));
            let cs = rep["cases"].as_array().unwrap();
            assert_eq!(cs[0]["terminal"]["kind"], "end");
            assert!(cs[if larger { 1 } else { 2 }]["terminal"].is_null());
            assert_eq!(cs[3]["terminal"]["kind"], "rejected");
        }
    }
}

#[test]
fn m_independent_prefix_history_peak_and_correlation_public_engines() {
    for input in acceptance::boundary_inputs() {
        let sim = matches!(
            input.label.as_str(),
            "clock-prefix" | "zero-duration" | "three-reps" | "abort" | "abort-late"
        );
        let cfg = if input.label == "unselected-process-rate" {
            Some("Selected")
        } else {
            None
        };
        let m = model(&input.source, cfg);
        if sim {
            let e = engine::simulate(&m);
            let r = reference::simulate(&m);
            assert_eq!(r.stop, reference::StopReason::Complete, "{}", input.label);
            let ev = serde_json::to_value(e).unwrap();
            let rv = serde_json::to_value(r.outcome).unwrap();
            assert_eq!(ev["replications"], rv["replications"], "{}", input.label);
            if input.label == "clock-prefix" {
                for v in [&ev, &rv] {
                    for rep in v["replications"].as_array().unwrap() {
                        obs(&rep["metrics"]["overall"], "work", "3");
                        obs(&rep["metrics"]["overall"], "observed_cost_total", "11");
                    }
                }
            }
        } else {
            let e = engine::explore(&m);
            let r = reference::explore(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            let ev = serde_json::to_value(&e.metrics).unwrap();
            let rv = serde_json::to_value(&r.outcome.metrics).unwrap();
            for v in [&ev, &rv] {
                match input.label.as_str() {
                    "history-identity" => {
                        eq(&v["overall"], "named_people_work", "3", "3");
                        for p in ["p", "q"] {
                            eq(row(&v["people"], "person", p), "work", "0", "3");
                        }
                    }
                    "correlation" => {
                        eq(&v["overall"], "work", "4", "6");
                        eq(&v["overall"], "observed_cost_total", "6", "15");
                    }
                    "global-peak" => eq(&v["overall"], "peak_tasks", "2", "2"),
                    "unstarted-cancel" => {
                        eq(terminal(case(v, 1)), "lead_time", "3", "10");
                        eq(&v["overall"], "observed_cost_total", "11", "32");
                    }
                    "mixed-cut" => acceptance::assert_all_truncated(v),
                    _ => {}
                }
            }
            assert_eq!(ev, rv, "{}: full metrics equality", input.label);
        }
    }
}
