//! Internal adapters to immutable hand oracles.
use super::*;
use serde_json::{Value, json};

const NONDYADIC: &str = include_str!("../../../fixtures/cmp7-nondyadic/model.relca");
const RATIO: &str = include_str!("../../../fixtures/eval-sim-ratio/model.relca");

#[test]
fn compare_parser_keeps_cut_declarations_distinct_and_requires_row_separators() {
    let src = include_str!("../../../fixtures/eval-agency/model.relca");
    let no_decl = src.replace("  compare [AsIs, ToBeP] cut C\n", "");
    assert_ne!(src, no_decl);
    let early = no_decl.replace("  cut C", "  compare [AsIs, ToBeP]\n  cut C");
    let out = compare(&early, &["AsIs", "ToBeP"], Analysis::Exhaustive);
    assert!(out.evaluation.is_some(), "{out:?}");
    for (legal, bad) in [
        (
            "cohort.WrongPerArrival <= 0.6",
            "cohort.WrongPerArrival < = 0.6",
        ),
        (
            "cohort.WrongPerArrival <= 0.6",
            "cohort.WrongPerArrival <= 0.6 cohort.WrongPerArrival <= 0.7",
        ),
        (
            "minimize metrics.overall.work",
            "minimize metrics.overall.work maximize metrics.overall.work",
        ),
    ] {
        let source = include_str!("../../../fixtures/eval-sim/model.relca");
        assert!(crate::syntax::parse(source).is_ok());
        assert!(
            crate::syntax::parse(&source.replace(legal, bad)).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn compare_scenario_name_fault_does_not_hide_other_static_errors() {
    let duplicate = include_str!("../../../fixtures/cmp7-under.broken-duplicate/model.relca");
    let broken = duplicate.replace(
        "process Quick {",
        "process Quick { if correct == true {} else {};",
    );
    assert_ne!(duplicate, broken);
    let out = compare_with_scenario(
        &broken,
        &["Check", "Fast"],
        Analysis::Simulation,
        Some("Day"),
    );
    assert_eq!(out.verdict, Verdict::Invalid);
    for p in out.plans {
        assert!(
            p.report.diagnostics().iter().any(
                |d| d.code == diag::U_SEM_0 && d.message.contains("duplicate compare scenario")
            )
        );
        assert!(
            p.report
                .diagnostics()
                .iter()
                .any(|d| d.code == diag::E_POL_LATENT)
        );
    }
}
fn oracles() -> Value {
    serde_json::from_str(include_str!(
        "../tests/support/evaluation_private_oracles.json"
    ))
    .unwrap()
}
fn injected(
    src: &str,
    mut alter: impl FnMut(&str, &mut report::SimulationReport, &mut lower::SelectedConfiguration),
) -> CompareReport {
    compare_using(&["Good", "Bad"], Analysis::Simulation, |name, _| {
        let (mut r, m) = report::simulate_selected_metadata(src, name);
        let mut m = m.unwrap();
        alter(name, &mut r, &mut m);
        (SelectedReport::Simulation(r), Some(m))
    })
}
#[test]
fn t14_all_six_hand_interval_controls_bind_the_real_comparator() {
    for c in oracles()["strict_interval_controls"].as_array().unwrap() {
        let parse = |v: &Value| v.as_str().unwrap().parse().unwrap();
        let op = match c["op"].as_str().unwrap() {
            "<=" => ir::HardComparison::Le,
            "<" => ir::HardComparison::Lt,
            ">=" => ir::HardComparison::Ge,
            ">" => ir::HardComparison::Gt,
            _ => unreachable!(),
        };
        let verdict = crate::evaluation::interval_verdict(
            op,
            &parse(&c["interval"]["lower"]),
            &parse(&c["interval"]["upper"]),
            &parse(&c["bound"]),
        );
        assert_eq!(json!(verdict), c["verdict"], "{c}");
    }
}
#[test]
fn f14_fixed_nondyadic_observations_pass_through_real_aggregate() {
    let o = &oracles()["cp_nondyadic"];
    let result = injected(NONDYADIC, |_, r, _| {
        // This injection is hand data, never an interval-function oracle.
        r.quality.as_mut().unwrap().properties[0]["cases"] = o["observations"].clone();
    });
    let result = json!(result);
    assert!(result["diagnostics"].as_array().unwrap().is_empty());
    let e = &result["evaluation"];
    assert_eq!(e["statistics"]["multiplicity"]["m"], o["m"]);
    assert_eq!(e["statistics"]["multiplicity"]["alpha"], o["alpha"]);
    for c in e["hard"][0]["values"].as_array().unwrap() {
        assert_eq!(c["interval"], o["interval"]);
        assert_eq!(c["point"], o["point"]);
        assert_eq!(c["basis"], o["basis"]);
        assert_eq!(c["verdict"], o["verdict"]);
    }
}

#[test]
fn t23_each_malformed_ir_and_report_is_rejected_without_partial_evaluation() {
    // Property denominator guarantees that both observation inventories are read.
    let src = RATIO.replace("replications 1", "replications 4");
    let normal = injected(&src, |_, _, _| {});
    assert!(normal.evaluation.is_some(), "{normal:?}");
    let oracle = oracles();
    for fault in oracle["malformed"].as_array().unwrap() {
        let id = fault["id"].as_str().unwrap();
        for property in [0usize, 1] {
            let run = std::panic::catch_unwind(|| {
                injected(&src, |name, r, s| {
                    if name != "Bad" && !matches!(id, "multiplicity-overflow" | "zero-cases") {
                        return;
                    }
                    let m = &mut s.model;
                    let q = r.quality.as_mut().unwrap();
                    let cases = q.properties[property]["cases"].as_array_mut().unwrap();
                    match id {
                        "case-duplicate" => cases.push(cases[0].clone()),
                        "case-missing" => {
                            cases.pop();
                        }
                        "arrival-mismatch" => cases[0]["arrival"] = json!(1),
                        "trials-mismatch" => cases[0]["trials"] = json!(3),
                        "hits-plus-unknown" => {
                            cases[0]["hits"] = json!(3);
                            cases[0]["unknown"] = json!(2);
                        }
                        "checked-add-overflow" => {
                            cases[0]["hits"] = json!(u64::MAX);
                            cases[0]["unknown"] = json!(1);
                        }
                        "no-reason-null" => {
                            q.cohort[0]["value"]["point"] = Value::Null;
                            q.cohort[0]["value"]["reason"] = Value::Null;
                        }
                        "reverse-range" => {
                            q.cohort[0]["bounds_with_undetermined"] =
                                json!({"lower":"1","upper":"0"});
                        }
                        "cohort-index" => {
                            m.comparison.as_mut().unwrap().hard[0].cohort = usize::MAX
                        }
                        "numerator-index" => {
                            m.quality.as_mut().unwrap().metrics[0].numerator = usize::MAX
                        }
                        "denominator-index" => {
                            m.quality.as_mut().unwrap().metrics[0].denominator =
                                ir::Denominator::Property(usize::MAX)
                        }
                        "bound-range" => {
                            m.comparison.as_mut().unwrap().hard[0].bound =
                                crate::probability::Rational("3/2".parse().unwrap())
                        }
                        "plan-inconsistency" => m.comparison.as_mut().unwrap().plans.reverse(),
                        "multiplicity-overflow" => m.bounds.cases = u64::MAX,
                        "zero-replications" => m.simulation.as_mut().unwrap().replications = 0,
                        "zero-cases" => m.bounds.cases = 0,
                        _ => panic!("unbound oracle {id}"),
                    }
                })
            });
            let result = run.unwrap_or_else(|_| panic!("panic: {id}/{property}"));
            assert_eq!(
                result.verdict,
                Verdict::Unsupported,
                "{id}/{property}: {result:?}"
            );
            assert!(result.evaluation.is_none(), "{id}/{property}");
            assert!(result.tables.agency.is_none(), "{id}/{property}");
            assert!(
                result.diagnostics.iter().any(|d| d.code == diag::U_SEM_0),
                "{id}/{property}"
            );
        }
    }
}

#[test]
fn f43_nested_nonempty_verify_family_keeps_its_own_alpha_and_intervals() {
    let src = include_str!("../../../fixtures/eval-sim/model.relca").replace(
        "  compare [",
        "  verify probability reached registered == true >= 0\n  compare [",
    );
    let result = compare(&src, &["Good", "Bad"], Analysis::Simulation);
    let e = json!(result.evaluation.as_ref().unwrap());
    assert_eq!(e["statistics"]["multiplicity"]["alpha"], "1/8");
    for p in result.plans {
        let alone = report::simulate_configuration(&src, &p.configuration);
        let nested = json!(p.report);
        assert_eq!(nested, json!(alone));
        assert_eq!(nested["simulation"]["multiplicity"]["alpha_after"], "1/4");
    }
}

#[test]
fn f11_injected_unknown_observations_enter_the_cp_upper_tail_without_horizon_repair() {
    let src = include_str!("../../../fixtures/eval-sim/model.relca");
    let out = injected(src, |_, r, _| {
        let q = r.quality.as_mut().unwrap();
        let zero = json!({"mean":"0","min":"0","max":"0","samples":3,"reason":null});
        let one = json!({"mean":"1","min":"1","max":"1","samples":3,"reason":null});
        q.properties[0]["cases"] =
            json!([{"case_id":1,"arrival":0,"hits":0,"unknown":3,"trials":3}]);
        q.properties[0]["expected_count"] = zero.clone();
        q.properties[0]["unknown_count"] = one.clone();
        q.cohort[0]["value"] = json!({"point":null,"numerator":zero,"denominator":one,"samples":3,"reason":"truncated"});
        q.cohort[0]["bounds_with_undetermined"] = json!({"lower":"0","upper":"1"});
        r.verdict = Verdict::Inconclusive;
    });
    assert_eq!(out.verdict, Verdict::Inconclusive);
    let e = json!(out.evaluation.unwrap());
    for c in e["hard"][0]["values"].as_array().unwrap() {
        assert_eq!(c["interval"], json!({"lower":"0","upper":"1"}));
        assert_eq!(c["verdict"], "inconclusive");
        assert_eq!(c["reason"], "truncated");
        assert!(c["point"].is_null() && c["source_reason"].is_null());
    }
}
