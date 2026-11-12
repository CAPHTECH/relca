#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/deferred_review_inputs.rs"]
mod h;
#[path = "../../relca-core/tests/support/deferred_metrics_inputs.rs"]
mod previous;
#[path = "../../relca-core/tests/support/deferred_second_inputs.rs"]
mod second;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::Value;
use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn run(src: &str, compare: bool, sim: bool, scenario: Option<&str>, selected: bool) -> Value {
    let dir = test_tmp::root().join(format!("k2-review-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}.relca", NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&path, src).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_relca"));
    cmd.arg(if compare {
        "compare"
    } else if sim {
        "simulate"
    } else {
        "check"
    })
    .arg(&path);
    if compare {
        cmd.args([
            "--plans",
            "Left,Right",
            "--mode",
            if sim { "simulation" } else { "exhaustive" },
        ]);
    } else if selected {
        cmd.args(["--configuration", "Left"]);
    }
    if let Some(day) = scenario {
        cmd.args(["--scenario", day]);
    }
    let o = cmd.output().unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(
        !["invalid", "unsupported"].contains(&v["verdict"].as_str().unwrap()),
        "{v}"
    );
    assert_eq!(
        o.status.code(),
        Some(match v["verdict"].as_str().unwrap() {
            "satisfied" => 0,
            "violated" => 1,
            "inconclusive" => 2,
            "vacuous" => 5,
            _ => panic!("unexpected verdict"),
        })
    );
    v
}
macro_rules! quality_case {
    ($test:ident,$name:literal,$sim:literal) => {
        #[test]
        fn $test() {
            for deferred in [false, true] {
                let s = h::source($name, deferred);
                let v = run(&s, true, $sim, h::scenario($name), true);
                h::assert_comparison(&v, $name, $sim, deferred);
                fixture_match::validate_compare_wire(&v).unwrap();
                for (j, plan) in ["Left", "Right"].iter().enumerate() {
                    let report = &v["plans"][j]["report"];
                    let wire = fixture_match::validate_deferred_metrics_wire(report);
                    let source = fixture_match::validate_deferred_metrics_with_source(
                        report,
                        &s,
                        Some(plan),
                    );
                    assert_eq!(wire.is_ok(), source.is_ok());
                    wire.unwrap();
                    source.unwrap();
                }
            }
        }
    };
}
quality_case!(real_truncated_compare, "truncated", false);
quality_case!(real_truncated_sample_compare, "truncated", true);
quality_case!(real_horizon_compare, "undetermined", false);
quality_case!(real_horizon_sample_compare, "undetermined", true);
quality_case!(real_zero_denominator_sample, "zero_denominator", true);
quality_case!(
    real_denominator_may_be_zero,
    "denominator_may_be_zero",
    false
);
quality_case!(real_beyond_reached_mass, "beyond_reached_mass", false);
#[test]
fn real_existing_clock_fixture_with_hard() {
    let v = run(&h::clock_fixture(true), true, false, None, true);
    for p in v["plans"].as_array().unwrap() {
        assert!(p["report"]["quality"]["cohort"][0]["bounds_with_undetermined"].is_null());
    }
    assert_eq!(
        v["evaluation"]["hard"][0]["values"][0]["verdict"],
        "inconclusive"
    );
    assert!(v["evaluation"]["hard"][0]["values"][0]["interval"].is_null());
}
macro_rules! dependency_case {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            for deferred in [false, true] {
                let s = h::dependency_source($name, deferred);
                let v = run(&s, false, false, None, false);
                // Same actual Report, two independent classification entries.
                let wire = fixture_match::validate_deferred_metrics_wire(&v);
                let source = fixture_match::validate_deferred_metrics_with_source(&v, &s, None);
                assert_eq!(
                    wire.is_ok(),
                    source.is_ok(),
                    "wire={wire:?}; source={source:?}"
                );
                h::assert_dependency(&v["metrics"], $name, deferred);
                if deferred {
                    second::assert_all_quantities(&v["metrics"]);
                }
                wire.unwrap();
                source.unwrap();
            }
        }
    };
}
dependency_case!(real_contest_route_dependency, "contest");
dependency_case!(real_shared_resource_dependency, "shared_resource");
dependency_case!(real_shared_person_dependency, "shared_person");

#[test]
fn real_deferred_hard_is_not_reconstructed_from_prior_reason_or_structural_trials() {
    let mut failures = Vec::new();
    for name in ["truncated", "undetermined"] {
        for sim in [false, true] {
            let v = run(&h::source(name, true), true, sim, h::scenario(name), true);
            for row in v["evaluation"]["hard"].as_array().unwrap() {
                for c in row["values"].as_array().unwrap() {
                    if !c["interval"].is_null()
                        || !c["basis"].is_null()
                        || c["verdict"] != "inconclusive"
                    {
                        failures.push((name, sim, c.clone()));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "hard numeric resurrection: {failures:?}"
    );
}

#[test]
fn real_no_defer_controls_are_numeric_or_keep_their_original_reason() {
    for name in h::QUALITY_CASES {
        let s = h::source(name, false);
        let v = run(&s, true, false, h::scenario(name), true);
        h::assert_comparison(&v, name, false, false);
    }
    for name in h::DEPENDENCY_CASES {
        let v = run(
            &h::dependency_source(name, false),
            false,
            false,
            None,
            false,
        );
        h::assert_dependency(&v["metrics"], name, false);
    }
}

#[test]
fn raw_wires_agree_at_both_matchers_for_every_materialized_input() {
    let mut cases = Vec::new();
    for (name, src) in previous::all_sources() {
        cases.push((
            src,
            name.starts_with("compare_").then_some("Included,Unrelated"),
            None,
        ));
    }
    for deferred in [false, true] {
        for name in h::QUALITY_CASES {
            cases.push((
                h::source(name, deferred),
                Some("Left,Right"),
                h::scenario(name),
            ));
        }
        cases.push((h::clock_fixture(deferred), Some("Left,Right"), None));
        for name in h::DEPENDENCY_CASES {
            cases.push((h::dependency_source(name, deferred), None, None));
        }
    }
    assert_eq!(cases.len(), 37);
    let mut jobs = std::collections::BTreeSet::new();
    for (src, plans, scenario) in cases {
        let commands: &[&str] = if plans.is_some() {
            &["compare"]
        } else {
            &["check", "simulate", "explain"]
        };
        for command in commands {
            let modes: &[&str] = if plans.is_some() {
                &["exhaustive", "simulation"]
            } else {
                &[""]
            };
            for mode in modes {
                let mut extra = Vec::new();
                if let Some(plans) = plans {
                    extra.extend(["--plans", plans, "--mode", mode]);
                }
                if let Some(day) = scenario {
                    extra.extend(["--scenario", day]);
                }
                jobs.insert((
                    src.clone(),
                    (*command).to_owned(),
                    extra.into_iter().map(str::to_owned).collect::<Vec<_>>(),
                ));
            }
        }
    }
    assert_eq!(jobs.len(), 93);
    let mut failures = Vec::new();
    for (src, command, extra) in jobs {
        let dir = test_tmp::root().join(format!("k2-materialized-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}.relca", NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(&path, &src).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg(&command)
            .arg(&path)
            .args(&extra)
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        let mut stderr = String::new();
        let mut diagnostics = |prefix: &str, rows: &Value| {
            for d in rows.as_array().unwrap() {
                stderr.push_str(&format!(
                    "{prefix}{}: {}\n",
                    d["code"].as_str().unwrap(),
                    d["message"].as_str().unwrap()
                ));
            }
        };
        if command == "compare" {
            for p in v["plans"].as_array().unwrap() {
                diagnostics(
                    &format!("configuration {}: ", p["configuration"].as_str().unwrap()),
                    &p["report"]["diagnostics"],
                );
            }
            diagnostics("compare: ", &v["diagnostics"]);
        } else {
            diagnostics("", &v["diagnostics"]);
            if command == "explain" {
                diagnostics("explain: ", &v["explanation"]["diagnostics"]);
            }
        }
        assert_eq!(out.stderr, stderr.as_bytes());
        assert_eq!(
            out.status.code(),
            Some(match v["verdict"].as_str().unwrap() {
                "satisfied" => 0,
                "violated" => 1,
                "inconclusive" => 2,
                "invalid" => 3,
                "unsupported" => 4,
                "vacuous" => 5,
                _ => panic!("unexpected verdict"),
            })
        );
        let reports: Vec<_> = if let Some(plans) = v["plans"].as_array() {
            plans
                .iter()
                .map(|p| (&p["report"], p["configuration"].as_str()))
                .collect()
        } else {
            vec![(&v, None)]
        };
        for (report, selection) in reports {
            let wire = fixture_match::validate_deferred_metrics_wire(report);
            let source =
                fixture_match::validate_deferred_metrics_with_source(report, &src, selection);
            if wire.is_err() || source.is_err() || wire.is_ok() != source.is_ok() {
                failures.push(format!(
                    "{} {selection:?} wire={wire:?} source={source:?}",
                    command
                ));
            }
            if report["quality"].is_object()
                && let Err(e) = fixture_match::validate_quality_wire(
                    &report["quality"],
                    report["mode"] == "simulation",
                )
            {
                failures.push(e);
            }
        }
        if command == "compare"
            && let Err(e) = fixture_match::validate_compare_wire(&v)
        {
            failures.push(format!("{extra:?} compare: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
