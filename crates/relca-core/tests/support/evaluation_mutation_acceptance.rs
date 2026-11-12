//! Hand oracles and witnessed edits, derived in ../evaluation_mutation_fixes.md.
use relca_core::compare::Analysis;
use serde_json::{Value, json};

fn edit(source: &str, before: &str, after: &str) -> String {
    assert_eq!(source.matches(before).count(), 1, "unique edit: {before}");
    source.replacen(before, after, 1)
}

const PROBE: &str = r#"model Probe version "0.2" {
  type T = enum { pass }
  actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet { pass } } }
  process Flow { x = A.run(); end done }
  cut C { include [A] }
  cut D { include [A] }
  configuration Left { cut C; process Flow }
  configuration Right { cut D; process Flow }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.done
  compare [Left, Right]
}
"#;

pub fn source(case: &str) -> String {
    match case {
        "missing-value" => edit(
            include_str!("../../../../fixtures/cmp7-cost.broken-missing-rate/model.relca"),
            "minimize cohort.WrongPerArrival; ",
            "",
        ),
        "configured-cost" => edit(
            &source("missing-value"),
            "rate Checker.quick per_start 0;",
            "rate Checker.check per_start 0; rate Checker.quick per_start 0;",
        ),
        "null-bounds" => {
            let s = include_str!("../../../../fixtures/cmp7-clock/model.relca");
            let s = edit(
                s,
                "arrivals fixed [0min, 6min, 12min]",
                "arrivals fixed [0min]",
            );
            let s = edit(
                &s,
                "cases 3; live_tasks 2; clock 30min",
                "cases 1; live_tasks 2; clock 3min",
            );
            edit(
                &s,
                "hard { cohort.WrongPerArrival <= 0.3 }",
                "hard { cohort.WrongPerRegistered <= 0.3 }",
            )
        }
        "finite-bounds" => edit(&source("null-bounds"), "clock 3min", "clock 30min"),
        "unknown-cache" => {
            let s = edit(
                include_str!("../../../../fixtures/cmp7-case-multiplicity/model.relca"),
                "clock 20min",
                "clock 12min",
            );
            edit(&s, "<= 0.9", "<= 0.95")
        }
        "known-cache" => edit(&source("unknown-cache"), "clock 12min", "clock 20min"),
        "hard-families" => include_str!("../../../../fixtures/eval-sim/model.relca").into(),
        "hard-quantifier" => include_str!("../../../../fixtures/eval-hard/model.relca").into(),
        "plans" => edit(PROBE, "compare [Left, Right]", "compare [Right, Left]"),
        "under" => edit(
            PROBE,
            "compare [Left, Right]",
            "compare [Left, Right] under [Allowed]\n  scenario Allowed { horizon 10min }\n  scenario Outside { horizon 10min }",
        ),
        "cut" => edit(
            PROBE,
            "compare [Left, Right]",
            "compare [Left, Right] cut C",
        ),
        "missing-plan" => PROBE.into(),
        "failed-lower" => edit(
            PROBE,
            "configuration Right { cut D; process Flow }",
            "process Broken { x = Missing.run(); end done }\n  configuration Right { cut D; process Broken }",
        ),
        "plans-only" => edit(
            &source("plans"),
            "configuration Right { cut D;",
            "configuration Right { cut C;",
        ),
        "under-only" => edit(
            &source("under"),
            "configuration Right { cut D;",
            "configuration Right { cut C;",
        ),
        "cut-only" => edit(
            &source("cut"),
            "configuration Left { cut C;",
            "configuration Left { cut D;",
        ),
        _ => panic!("unlisted mutation acceptance {case}"),
    }
}

pub fn plans(case: &str) -> &'static [&'static str] {
    match case {
        "missing-value" | "configured-cost" | "null-bounds" | "finite-bounds"
        | "hard-quantifier" => &["Check", "Fast"],
        "unknown-cache" | "known-cache" | "hard-families" => &["Good", "Bad"],
        "missing-plan" => &["Left", "Missing"],
        _ => &["Left", "Right"],
    }
}
pub fn analysis(case: &str) -> Analysis {
    if matches!(case, "unknown-cache" | "known-cache" | "hard-families") {
        Analysis::Simulation
    } else {
        Analysis::Exhaustive
    }
}
pub fn scenario(case: &str) -> Option<&'static str> {
    matches!(case, "under" | "under-only").then_some("Outside")
}
pub fn exit(case: &str) -> i32 {
    match case {
        "missing-value" | "configured-cost" | "finite-bounds" | "known-cache" => 0,
        "null-bounds" | "unknown-cache" | "hard-families" => 2,
        "hard-quantifier" => 1,
        _ => 3,
    }
}

fn assumption(out: &Value, text: &str) {
    let rows: Vec<_> = out["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["field"] == "compare.hard")
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["value"], "conservative-endpoints");
    assert!(
        rows[0]["affects"].as_str().unwrap().contains(text),
        "missing hard caveat: {text}"
    );
}

pub fn hand(case: &str, out: &Value) {
    assert_eq!(
        out["verdict"],
        match exit(case) {
            0 => "satisfied",
            1 => "violated",
            2 => "inconclusive",
            _ => "invalid",
        }
    );
    match case {
        "missing-value" | "configured-cost" => {
            let e = &out["evaluation"];
            assert_eq!(e["objectives"].as_array().unwrap().len(), 1);
            let v = &e["objectives"][0]["values"];
            assert_eq!(
                v[1],
                json!({"configuration":"Fast","lower":"4","upper":"4","basis":"path_range","reason":null})
            );
            if case == "missing-value" {
                assert_eq!(
                    v[0],
                    json!({"configuration":"Check","lower":null,"upper":null,"basis":null,"reason":"not_configured"})
                );
                assert_eq!(e["pareto"]["non_dominated"], json!(["Check", "Fast"]));
                assert_eq!(e["pareto"]["dominated"], json!([]));
                assert_eq!(
                    e["pareto"]["undetermined_pairs"],
                    json!([{"from":"Check","to":"Fast","reason":"missing_value"}])
                );
            } else {
                assert_eq!(v[0]["lower"], "6");
                assert_eq!(v[0]["upper"], "8");
                assert_eq!(e["pareto"]["non_dominated"], json!(["Fast"]));
                assert_eq!(
                    e["pareto"]["dominated"],
                    json!([{"configuration":"Check","dominated_by":["Fast"]}])
                );
                assert_eq!(e["pareto"]["undetermined_pairs"], json!([]));
            }
            assert_eq!(e["pareto"]["among"], json!(["Check", "Fast"]));
        }
        "null-bounds" | "finite-bounds" => {
            for i in 0..2 {
                let c = &out["evaluation"]["hard"][0]["values"][i];
                let p = &out["evaluation"]["plans"][i];
                let q = &out["plans"][i]["report"]["quality"]["cohort"][1];
                if case == "null-bounds" {
                    assert!(q["bounds_with_undetermined"].is_null());
                    assert!(q["value"]["lower"].is_null());
                    assert!(q["value"]["upper"].is_null());
                    assert_eq!(q["value"]["reason"], "truncated");
                    assert_eq!(
                        c,
                        &json!({"configuration":plans(case)[i],"verdict":"inconclusive","basis":null,"interval":null,"reason":"truncated","source_reason":null})
                    );
                    assert_eq!(p["status"], "undetermined");
                    assert_eq!(
                        p["reasons"],
                        json!([{"kind":"verify","constraint":null,"verdict":"inconclusive","reason":null},{"kind":"hard","constraint":"cohort.WrongPerRegistered <= 3/10","verdict":"inconclusive","reason":"truncated"}])
                    );
                } else {
                    assert_eq!(c["verdict"], "satisfied");
                    assert_eq!(c["interval"], json!({"lower":"1/46","upper":"1/46"}));
                    assert_eq!(p["status"], "admissible");
                }
            }
        }
        "unknown-cache" | "known-cache" => {
            let e = &out["evaluation"];
            assert_eq!(e["statistics"]["multiplicity"]["m"], 4);
            assert_eq!(e["statistics"]["multiplicity"]["alpha"], "1/16");
            for i in 0..2 {
                let unknown = case == "unknown-cache" && i == 0;
                let rows = out["plans"][i]["report"]["quality"]["properties"][0]["cases"]
                    .as_array()
                    .unwrap();
                assert_eq!(rows.len(), 2);
                for (k, r) in rows.iter().enumerate() {
                    assert_eq!(r["hits"], 0);
                    assert_eq!(r["trials"], 1);
                    assert_eq!(r["unknown"], u64::from(unknown && k == 1));
                }
                let c = &e["hard"][0]["values"][i];
                assert_eq!(
                    c["interval"],
                    json!({"lower":"0","upper":if unknown {"31/32"} else {"15/16"}})
                );
                assert_eq!(
                    c["verdict"],
                    if unknown { "inconclusive" } else { "satisfied" }
                );
                assert_eq!(c["point"], if unknown { Value::Null } else { json!("0") });
                assert_eq!(
                    c["reason"],
                    if unknown {
                        json!("truncated")
                    } else {
                        Value::Null
                    }
                );
            }
        }
        "hard-families" | "hard-quantifier" => {
            assumption(
                out,
                "The verify and hard families together do not have simultaneous confidence equal to confidence",
            );
            assumption(
                out,
                "violated means violation for every schedule and every resolution of unknown mass",
            );
        }
        _ => mismatch_hand(case, out),
    }
}

fn mismatch_hand(case: &str, out: &Value) {
    assert!(out.get("evaluation").is_none());
    assert!(out["tables"].get("agency").is_none());
    if matches!(case, "missing-plan" | "failed-lower") {
        assert!(
            out["mismatch"].is_null(),
            "never fabricate failed metadata pairs"
        );
        assert_eq!(out["plans"][0]["report"]["verdict"], "satisfied");
        assert_eq!(out["plans"][1]["report"]["verdict"], "invalid");
        assert!(
            !out["checked"]
                .as_array()
                .unwrap()
                .contains(&json!("cut_compatibility"))
        );
        if case == "failed-lower" {
            let ast = relca_core::syntax::parse(&source(case)).unwrap();
            assert!(relca_core::lower::lower_configuration(&ast, "Left").is_ok());
            assert!(relca_core::lower::lower_configuration(&ast, "Right").is_err());
        }
        return;
    }
    for p in out["plans"].as_array().unwrap() {
        assert_eq!(p["report"]["verdict"], "satisfied");
    }
    let declaration_only = case.ends_with("-only");
    let declared = if case.starts_with("plans") {
        ("E-CONF-0", "plans differ from the compare declaration")
    } else if case.starts_with("under") {
        ("E-CONF-0", "scenario is not under the compare declaration")
    } else {
        ("E-CMP-0", "compare cut differs from plan cut")
    };
    let mut diagnostics = vec![json!({"code":declared.0,"message":declared.1})];
    if !declaration_only {
        diagnostics.push(
            json!({"code":"E-CMP-0","message":"configurations Left and Right have different Cuts"}),
        );
    }
    let got: Vec<_> = out["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| json!({"code":d["code"],"message":d["message"]}))
        .collect();
    assert_eq!(
        got, diagnostics,
        "declaration diagnostics cannot hide real pairs"
    );
    if declaration_only {
        assert!(out["mismatch"].is_null());
    } else {
        assert_eq!(
            out["mismatch"]["pairs"],
            json!([{"left":"Left","right":"Right","cut":{"left_cut":"C","right_cut":"D","differences":[]},"terminals":null}])
        );
    }
    for key in [
        "configuration_selection",
        "cut_compatibility",
        "terminal_compatibility",
    ] {
        assert!(out["checked"].as_array().unwrap().contains(&json!(key)));
    }
    for key in [
        "property_tables",
        "terminal_tables",
        "metric_tables",
        "hard_filter",
        "admissibility",
    ] {
        assert!(!out["checked"].as_array().unwrap().contains(&json!(key)));
    }
}

// Same finite cases in the public core and real CLI transports; no test reads .task.
macro_rules! mutation_acceptance_tests {
    () => {
        #[test]
        fn missing_value_is_not_dominated() {
            run("missing-value");
        }
        #[test]
        fn configured_cost_can_be_dominated() {
            run("configured-cost");
        }
        #[test]
        fn null_bounds_remain_inconclusive() {
            run("null-bounds");
        }
        #[test]
        fn finite_bounds_can_satisfy_hard() {
            run("finite-bounds");
        }
        #[test]
        fn unknown_count_separates_cp_cache_entries() {
            run("unknown-cache");
        }
        #[test]
        fn known_counts_share_the_right_cp_interval() {
            run("known-cache");
        }
        #[test]
        fn simulation_hard_assumption_preserves_both_caveats() {
            run("hard-families");
        }
        #[test]
        fn exhaustive_hard_assumption_preserves_both_caveats() {
            run("hard-quantifier");
        }
        #[test]
        fn plans_mismatch_keeps_interplan_pairs() {
            run("plans");
        }
        #[test]
        fn outside_scenario_keeps_real_lowered_pairs() {
            run("under");
        }
        #[test]
        fn declared_cut_mismatch_keeps_interplan_pairs() {
            run("cut");
        }
        #[test]
        fn declaration_only_failures_do_not_invent_pairs() {
            run("plans-only");
            run("under-only");
            run("cut-only");
        }
        #[test]
        fn missing_plan_does_not_invent_pairs() {
            run("missing-plan");
        }
        #[test]
        fn failed_lower_does_not_invent_pairs() {
            run("failed-lower");
        }
    };
}
pub(crate) use mutation_acceptance_tests;
