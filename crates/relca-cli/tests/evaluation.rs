//! Real CLI, independent full evaluation oracles and fault-calibrated wire seam.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/evaluation_acceptance.rs"]
mod h;
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn all_evaluation_commands_collect_exit_and_hand_failures() {
    let mut failures = vec![];
    for n in h::names() {
        let d = h::root().join("fixtures").join(&n);
        let iv = &h::inventory()[&n]["invocation"];
        let parsed = fixture_match::Invocation::parse(Some(iv)).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(parsed.args(&d.join("model.relca")))
            .output()
            .unwrap();
        if std::panic::catch_unwind(|| {
            let v: Value = serde_json::from_slice(&out.stdout).unwrap();
            h::hand(&n, &v);
            let verdict: relca_core::Verdict =
                serde_json::from_value(h::expected(&n)["verdict"].clone()).unwrap();
            assert_eq!(
                out.status.code(),
                Some(verdict.exit_code()),
                "{n}:real CLI exit"
            );
            parsed.compare(&h::expected(&n), &v).unwrap();
        })
        .is_err()
        {
            failures.push(n);
        }
    }
    assert!(
        failures.is_empty(),
        "all CLI ADR0047 failures: {failures:?}"
    );
}

fn hand_wire(n: &str) -> Value {
    let s = h::source(n);
    let mut v = h::run(n, &h::without_declaration(&s));
    let e = h::expected(n);
    v["verdict"] = e["verdict"].clone();
    v["evaluation"] = e["evaluation"].clone();
    let mut pairs = vec![];
    let ps = v["plans"].as_array().unwrap();
    for i in 0..ps.len() {
        for j in i + 1..ps.len() {
            let aa = serde_json::from_value(ps[i]["report"]["agency"].clone()).ok();
            let bb = serde_json::from_value(ps[j]["report"]["agency"].clone()).ok();
            let ar: Option<Vec<relca_core::responsibility::Responsibility>> =
                serde_json::from_value(ps[i]["report"]["responsibilities"].clone()).ok();
            let br: Option<Vec<relca_core::responsibility::Responsibility>> =
                serde_json::from_value(ps[j]["report"]["responsibilities"].clone()).ok();
            let d = relca_core::agency::differences(
                relca_core::agency::ComparisonSide {
                    agency: aa.as_ref(),
                    responsibilities: ar.as_deref(),
                },
                relca_core::agency::ComparisonSide {
                    agency: bb.as_ref(),
                    responsibilities: br.as_deref(),
                },
            );
            pairs.push(
                json!({"from":ps[i]["configuration"],"to":ps[j]["configuration"],"differences":d}),
            );
        }
    }
    v["tables"]["agency"] = json!(pairs);
    let sampled = v["analysis"] == "simulation";
    let checked = v["checked"].as_array_mut().unwrap();
    checked.extend([
        json!("hard_filter"),
        json!("admissibility"),
        json!(if sampled {
            "hard_cross_plan_bonferroni"
        } else {
            "pareto_sure_dominance"
        }),
    ]);
    v["unchecked"]
        .as_array_mut()
        .unwrap()
        .retain(|w| w != "hard_filter" && (sampled || w != "pareto"));
    let aa = v["assumptions"].as_array_mut().unwrap();
    aa.iter_mut()
        .find(|a| a["field"] == "compare.purpose")
        .unwrap()["value"] = json!("declared-evaluation");
    aa.extend([json!({"field":"compare.hard","value":"interval","affects":"hard intervals, not point estimates"}),
        json!({"field":"compare.pareto","value":"sure-dominance","affects":"objectives are not added"})]);
    v
}

#[test]
fn independent_hand_wire_accepts_valid_intervals_above_one_and_rejects_single_faults() {
    for n in [
        "eval-hard",
        "eval-hard.broken-no-hard",
        "eval-contention.broken-no-policy",
        "cmp7-clock.broken-short-clock",
        "eval-zero.broken-all-fail",
        "eval-sim",
        "eval-sim-ratio",
        "eval-pareto",
        "cmp7-cost.broken-missing-rate",
        "eval-agency",
        "eval-agency.broken-swapped",
        "eval-vacuous",
        "cmp7-verify-excluded",
        "cmp7-case-multiplicity",
        "cmp7-nondyadic",
        "cmp7-nondyadic.broken-tight",
        "cmp7-row-multiplicity.broken-another-row",
        "cmp7-sim-zero",
        "cmp7-overlap",
        "cmp7-maximize",
        "cmp7-no-admissible",
        "cmp7-overall",
        "cmp7-responsibility",
        "cmp7-responsibility.broken-no-inspect",
        "cmp7-three-plans",
        "cmp7-no-metrics",
        "cmp7-named-work",
        "cmp7-shared-cohort",
    ] {
        let good = hand_wire(n);
        if n == "eval-agency" || n == "eval-agency.broken-swapped" {
            assert_eq!(
                good["tables"]["agency"][0]["differences"],
                json!({"rules":"agency-diff-p2-1","status":"derived","rows":[],
                "unchecked":["fallback_path","workload_transfer","via_model_paths"]})
            );
        }
        fixture_match::validate_compare_wire(&good).unwrap_or_else(|e| panic!("{n}:hand wire:{e}"));
        for pointer in [
            "/evaluation",
            "/evaluation/declared",
            "/evaluation/hard/0",
            "/evaluation/hard/0/values/0",
            "/evaluation/statistics",
            "/evaluation/statistics/multiplicity",
            "/evaluation/plans/0",
            "/evaluation/objectives/0",
            "/evaluation/objectives/0/values/0",
            "/evaluation/pareto",
            "/tables/agency/0",
        ] {
            if let Some(obj) = good.pointer(pointer).and_then(Value::as_object) {
                let mut bad = good.clone();
                bad.pointer_mut(pointer)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("score".into(), json!(0));
                assert!(
                    fixture_match::validate_compare_wire(&bad).is_err(),
                    "{n}:extra {pointer}"
                );
                for key in obj.keys() {
                    let mut bad = good.clone();
                    bad.pointer_mut(pointer)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .remove(key);
                    assert!(
                        fixture_match::validate_compare_wire(&bad).is_err(),
                        "{n}:missing {pointer}/{key}"
                    );
                }
            }
        }
        for (pointer, value) in [
            ("/evaluation/declared/plans/0", json!("wrong")),
            ("/evaluation/hard/0/bound", json!("2")),
            ("/evaluation/hard/0/values/0/interval/lower", json!("2/4")),
            (
                "/evaluation/hard/0/values/0/source_reason",
                json!("invented"),
            ),
            ("/evaluation/hard/0/values/0/verdict", json!("vacuous")),
            ("/evaluation/plans/0/status", json!("invented")),
            ("/evaluation/statistics/multiplicity/m", json!(999)),
            ("/evaluation/statistics/multiplicity/alpha", json!("1/2")),
            ("/evaluation/pareto/among", json!([])),
            (
                "/evaluation/pareto/not_evaluated",
                json!([{ "configuration":"fake","status":"excluded" }]),
            ),
            ("/tables/agency/0/from", json!("fake")),
        ] {
            if good.pointer(pointer).is_some() && good.pointer(pointer).unwrap() != &value {
                let mut bad = good.clone();
                *bad.pointer_mut(pointer).unwrap() = value;
                assert!(
                    fixture_match::validate_compare_wire(&bad).is_err(),
                    "{n}:semantic fault {pointer}"
                );
            }
        }
    }
}

#[test]
fn all_partial_expectations_are_closed_before_running_the_binary() {
    for n in h::names() {
        let iv = &h::inventory()[&n]["invocation"];
        fixture_match::Invocation::parse(Some(iv))
            .unwrap()
            .validate_expected(&h::expected(&n))
            .unwrap_or_else(|e| panic!("{n}:{e}"));
    }
}

#[test]
fn t12_real_stdout_byte_projection_does_not_sort_evaluation_or_pareto() {
    let mut outputs = vec![];
    for n in ["eval-contention", "eval-label"] {
        let d = h::root().join("fixtures").join(n);
        let iv = &h::inventory()[n]["invocation"];
        let parsed = fixture_match::Invocation::parse(Some(iv)).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(parsed.args(&d.join("model.relca")))
            .output()
            .unwrap();
        let v = serde_json::from_slice(&out.stdout).unwrap();
        h::hand(n, &v);
        assert!(out.status.success());
        outputs.push(out.stdout);
    }
    h::label_byte_projection(&outputs[0], &outputs[1]);
}

fn hand_kind7_wire() -> Value {
    let source = h::source("eval-mut7");
    let mut v = serde_json::to_value(relca_core::mutation::mutate(&h::source(
        "eval-mut7.broken-no-cohort",
    )))
    .unwrap();
    v["original"] = serde_json::to_value(relca_core::report::check(&source)).unwrap();
    let mut mutants = vec![];
    for (cohort, ends, process) in [
        ("WrongPerArrival", vec!["returned", "posted"], json!("Flow")),
        ("WrongPerResolved", vec!["posted", "returned"], Value::Null),
    ] {
        let start = source.find(&format!("metric cohort.{cohort} =")).unwrap();
        let end = start + source[start..].find('\n').unwrap();
        let begin = start + source[start..end].find(" / ").unwrap() + 3;
        let line = source[..begin].bytes().filter(|b| *b == b'\n').count() + 1;
        let column = begin - source[..begin].rfind('\n').map_or(0, |p| p + 1) + 1;
        for removed in &ends {
            let after: Vec<_> = ends.iter().filter(|e| e != &removed).collect();
            mutants.push(json!({
                "id":format!("old_promise_denominator_removed:cohort.{cohort}:remove:{removed}"),
                "site":{"process":process,"actor":null,"capability":null,"source":null,"affected_bindings":[],
                    "statement_path":null,"retained_branch":null,"span":{"start":begin,"end":end},"line":line,"column":column,
                    "cohort":cohort,"removed_end":removed},
                "operator":"remove_denominator_end","result":"detected","reason":null,"report":null,
                "other_changes":[],"assigned_changes":(["quality_denominator","comparison_mismatch"].iter().map(|c|
                    json!({"check":c,"key":format!("cohort.{cohort}"),"field":"denominator.terminals","before":ends,"after":after,"direction":"changed"})).collect::<Vec<_>>())
            }));
        }
    }
    v["mutations"][7]["status"] = json!("generated");
    // Two removable ends in each of two terminal-denominator cohorts.
    v["mutations"][7]["candidates"] = json!(4);
    v["run"]["candidates_total"] = json!(v["run"]["candidates_total"].as_u64().unwrap() + 4);
    v["mutations"][7]["reason"] = Value::Null;
    v["mutations"][7]["mutants"] = json!(mutants);
    v["mutations"][7]["skipped_sites"] = json!([{"cohort":"WrongPerRegistered","status":"not_applicable","reason":"state_condition"}]);
    v["checked"]
        .as_array_mut()
        .unwrap()
        .push(json!("quality_denominator"));
    v["unchecked"]
        .as_array_mut()
        .unwrap()
        .retain(|w| w != "quality_denominator");
    v["unchecked"]
        .as_array_mut()
        .unwrap()
        .push(json!("numerator_subset_of_denominator"));
    v["assumptions"].as_array_mut().unwrap().push(
        json!({"field":"mutation.kind7","value":"definition-comparison",
        "affects":"no mutant execution, no ratio values, no relca compare"}),
    );
    v
}

#[test]
fn kind7_closed_wire_checks_structural_definitions_not_reexecution_or_other_changes() {
    let good = hand_kind7_wire();
    fixture_match::validate_mutation_wire(&good).unwrap();
    h::mutation_hand("eval-mut7", &good);
    for (pointer, value) in [
        ("/mutations/7/mutants/0/report", good["original"].clone()),
        ("/mutations/7/mutants/0/assigned_changes", json!([])),
        (
            "/mutations/7/mutants/0/assigned_changes/0/check",
            json!("other_changes"),
        ),
        (
            "/mutations/7/mutants/0/assigned_changes/0/after",
            json!(["returned", "posted"]),
        ),
        (
            "/mutations/7/mutants/0/assigned_changes/1/after",
            json!(["returned"]),
        ),
        ("/mutations/7/mutants/0/site/cohort", json!("missing")),
        ("/mutations/7/mutants/0/site/removed_end", json!("missing")),
        ("/mutations/7/mutants/0/result", json!("undetected")),
        (
            "/mutations/7/skipped_sites/0/cohort",
            json!("WrongPerArrival"),
        ),
        ("/mutations/7/status", json!("not_applicable")),
        ("/run/limit_reached", json!(true)),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            fixture_match::validate_mutation_wire(&bad).is_err(),
            "kind7 wire fault:{pointer}"
        );
    }
    for pointer in [
        "/mutations/7",
        "/mutations/7/mutants/0",
        "/mutations/7/mutants/0/site",
        "/mutations/7/skipped_sites/0",
    ] {
        let mut bad = good.clone();
        bad.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("ratio".into(), json!("0"));
        assert!(
            fixture_match::validate_mutation_wire(&bad).is_err(),
            "forbidden mutant ratio:{pointer}"
        );
        for key in good.pointer(pointer).unwrap().as_object().unwrap().keys() {
            let mut bad = good.clone();
            bad.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                fixture_match::validate_mutation_wire(&bad).is_err(),
                "kind7 missing:{pointer}/{key}"
            );
        }
    }
    let mut bad = good.clone();
    bad["mutations"][0]["skipped_sites"] = json!([]);
    assert!(
        fixture_match::validate_mutation_wire(&bad).is_err(),
        "another kind cannot borrow exception"
    );
}

#[test]
fn malformed_case_inventory_is_rejected_without_overflow_panic_or_partial_zip() {
    let good = hand_wire("eval-sim");
    for fault in 0..6 {
        let mut bad = good.clone();
        let cases = bad["plans"][0]["report"]["quality"]["properties"][0]["cases"]
            .as_array_mut()
            .unwrap();
        match fault {
            0 => {
                cases.push(cases[0].clone());
            }
            1 => {
                cases.clear();
            }
            2 => cases[0]["arrival"] = json!(99),
            3 => cases[0]["trials"] = json!(2),
            4 => {
                cases[0]["hits"] = json!(3);
                cases[0]["unknown"] = json!(1);
            }
            5 => {
                cases[0]["hits"] = json!(u64::MAX);
                cases[0]["unknown"] = json!(1);
            }
            _ => unreachable!(),
        }
        let rejected = std::panic::catch_unwind(|| fixture_match::validate_compare_wire(&bad));
        assert!(
            rejected.is_ok(),
            "fault{fault}:must return refusal,not panic"
        );
        assert!(
            rejected.unwrap().is_err(),
            "fault{fault}:missing/duplicate/trials/arrival/overflow"
        );
    }
}
