use super::*;
use serde_json::{Value, json};
const SOURCE: &str = include_str!("../../../fixtures/eval-mut7/model.relca");
fn oracle() -> Value {
    serde_json::from_str(include_str!(
        "../tests/support/evaluation_private_oracles.json"
    ))
    .unwrap()
}

#[test]
fn t26_identical_and_one_end_records_use_the_real_definition_comparator() {
    let oracle = oracle();
    let original: DenominatorDefinition =
        serde_json::from_value(oracle["definition_original"].clone()).unwrap();
    for control in oracle["definition_controls"].as_array().unwrap() {
        let mut after = original.clone();
        if !control["edit"].is_null() {
            after.terminals = serde_json::from_value(control["edit"]["terminals"].clone()).unwrap();
        }
        let (c, counts) = crate::evaluation_spy::observe(|| compare_definition(&original, &after));
        assert_eq!(counts, [0; 7]);
        assert_eq!(json!(c.result), control["result"]);
        assert_eq!(json!(c.reason), control["reason"]);
        assert!(c.other.is_empty());
        if control["edit"].is_null() {
            assert_eq!(json!(c.assigned), control["assigned_changes"]);
        } else {
            assert_eq!(c.assigned.len(), 2);
            for (change, name) in c
                .assigned
                .iter()
                .zip(["quality_denominator", "comparison_mismatch"])
            {
                assert_eq!(change.check, name);
                assert_eq!(change.key, format!("cohort.{}", original.cohort_name));
                assert_eq!(change.field, "denominator.terminals");
                assert_eq!(change.before, Payload::Strings(original.terminals.clone()));
                assert_eq!(change.after, Payload::Strings(after.terminals.clone()));
                assert_eq!(change.direction, "changed");
            }
        }
        let mut fixed = after;
        fixed.terminals = original.terminals.clone();
        assert_eq!(fixed, original);
    }
}
#[test]
fn t26_kind7_generation_and_comparison_never_enter_any_execution_api() {
    let ast = syntax::parse(SOURCE).unwrap();
    let baseline = report::check(SOURCE);
    assert_eq!(baseline.verdict, Verdict::Satisfied);
    let mut r = uncomputed("original_not_decidable", 128);
    r.original = Some(baseline);
    let (_, counts) = crate::evaluation_spy::observe(|| {
        kind7(SOURCE, &ast, &mut r, &mut 0, &Options::default(), None)
    });
    assert_eq!(counts, [0; 7], "all seven execution entries");
    assert_eq!(r.mutations[7].mutants.len(), 4);
    assert!(
        r.mutations[7]
            .mutants
            .iter()
            .all(|m| m.result == "detected" && m.report.is_none() && m.other_changes.is_empty())
    );
}
#[test]
fn t26_all_seven_spy_hooks_have_positive_controls() {
    let ast = syntax::parse(SOURCE).unwrap();
    let m = crate::lower::lower(&ast).unwrap();
    let simulation = crate::lower::lower(
        &syntax::parse(include_str!("../../../fixtures/met-sample/model.relca")).unwrap(),
    )
    .unwrap();
    let (_, counts) = crate::evaluation_spy::observe(|| {
        crate::lower::lower(&ast).unwrap();
        engine::explore(&m);
        crate::reference::explore(&m);
        crate::compare::compare(SOURCE, &[], crate::compare::Analysis::Exhaustive);
        assert_eq!(engine::simulate(&simulation).completed_replications, 4);
        assert_eq!(
            crate::reference::simulate(&simulation)
                .outcome
                .completed_replications,
            4
        );
        crate::explain::explain(include_str!(
            "../../../fixtures/explain-invariant/model.relca"
        ));
    });
    assert!(
        counts.iter().all(|n| *n > 0),
        "each hook must detect a real call: {counts:?}"
    );
}
#[test]
fn t24_four_hand_limits_share_the_old_budget_and_keep_every_structural_candidate() {
    let oracle = oracle();
    let selected_source = SOURCE.replace(
        "arrivals fixed [0min, 10min]\n  bounds { cases 2; live_tasks 2; clock 30min }",
        "scenario Day { arrivals fixed [0min, 10min]; bounds { cases 2; live_tasks 2; clock 30min } }",
    );
    assert_ne!(selected_source, SOURCE, "Day must be a real source edit");
    for control in oracle["budget"].as_array().unwrap() {
        let limit = control["limit"].as_u64().unwrap() as usize;
        let mut executed = 0;
        let r = mutate_using(
            &selected_source,
            limit,
            Some("Day"),
            |s| report::check_with_selection(s, None, Some("Day")),
            |s, ast| {
                executed += 1;
                let r = report::check_ast_with_scenario_using(s, ast, Some("Day"), engine::explore);
                assert_eq!(r.verdict, Verdict::Invalid);
                assert!(r.diagnostics.iter().any(|d| d.code == "E-NAME-0"));
                r
            },
        );
        assert_eq!(
            executed,
            control["ordinary_guards"].as_u64().unwrap() as usize
        );
        let row = &r.mutations[7];
        assert_eq!(row.mutants.len(), 4);
        assert_eq!(
            row.mutants
                .iter()
                .filter(|m| m.result == "detected")
                .count(),
            control["kind7_detected"].as_u64().unwrap() as usize
        );
        assert_eq!(
            row.mutants.iter().filter(|m| m.result == "not_run").count(),
            control["kind7_not_run"].as_u64().unwrap() as usize
        );
        for m in &row.mutants {
            if m.result == "not_run" {
                assert_eq!(m.reason, Some("mutant_limit"));
                assert!(
                    m.report.is_none()
                        && m.assigned_changes.is_empty()
                        && m.other_changes.is_empty()
                );
            } else {
                assert!(m.report.is_none());
                assert_eq!(m.assigned_changes.len(), 2);
                assert!(m.other_changes.is_empty());
            }
        }
        assert_eq!(
            r.checked.iter().any(|s| s == "quality_denominator"),
            control["quality_denominator_checked"] == true
        );
        assert_eq!(r.run.limit_reached, limit < 6);
        assert_eq!(r.run.completion, "inconclusive");
        assert_eq!(
            r.run.reason,
            if limit < 6 {
                Some("mutant_limit")
            } else {
                Some("mutant_incomplete")
            }
        );
        for i in [2, 8] {
            let m = &r.mutations[i].mutants[0];
            assert_eq!(
                m.result,
                if limit == 0 {
                    "not_run"
                } else {
                    "invalid_mutant"
                }
            );
            assert_eq!(
                m.reason,
                Some(if limit == 0 {
                    "mutant_limit"
                } else {
                    "static_invalid"
                })
            );
        }
        assert!(r.unchecked.iter().any(|x| x == "quality_changes"));
        assert_eq!(r.verdict, Verdict::Unsupported);
        assert_eq!(
            row.skipped_sites.as_ref().unwrap().len(),
            1,
            "State site consumes no slot"
        );
    }
}
