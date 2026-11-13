//! Internal linkage seam for the mutation comparator/editor.
//! This file owns the oracle, not the production implementation. See the adjacent
//! obligations document for the exact adapter signature and finite case list.
#![allow(dead_code)]
use serde_json::{Value, json};

fn line_mut<'a>(v: &'a mut Value, call: &str) -> &'a mut Value {
    v["agency"]["lineage"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|x| x["call"] == call)
        .unwrap()
}
fn group_mut(v: &mut Value) -> &mut Value {
    v["agency"]["lineage_groups"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|g| g["subject_sources"] == json!(["Invoice"]))
        .unwrap()
}

/// Adapter returns the *real private comparison's* result/reason/assigned_changes/
/// other_changes as JSON. It must not implement a second comparator in the test.
pub fn comparison_cases(mut probe: impl FnMut(&Value, &Value, &[&str], &str, &str) -> Value) {
    let source = include_str!("../../../../fixtures/mut2-summary/model.relca");
    let base = serde_json::to_value(crate::report::check(source)).unwrap();
    assert_eq!(base["verdict"], "satisfied");
    let check = |got: Value, result: &str, reason: Value| {
        assert_eq!(got["result"], result);
        assert_eq!(got["reason"], reason);
        got
    };
    let got = check(
        probe(&base, &base, &["Invoice"], "a", "Approver.approve"),
        "detected",
        Value::Null,
    );
    assert_eq!(got["assigned_changes"].as_array().unwrap().len(), 3);
    assert!(
        got["assigned_changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["direction"] == "held")
    );
    // P01: order and duplicates are sets, not list identity.
    let mut before = base.clone();
    let mut after = base.clone();
    for v in [&mut before, &mut after] {
        line_mut(v, "a")["subject_sources"] = json!(["Invoice", "Other"]);
        group_mut(v)["subject_sources"] = json!(["Invoice", "Other"]);
    }
    line_mut(&mut after, "a")["subject_sources"] = json!(["Other", "Invoice", "Invoice"]);
    after["agency"]["lineage_groups"][0]["subject_sources"] = json!(["Other", "Invoice"]);
    check(
        probe(&before, &after, &["Invoice"], "a", "Approver.approve"),
        "detected",
        Value::Null,
    );
    // P02: equal counts with a *new* source must not be held.
    let mut before = base.clone();
    let mut after = base.clone();
    line_mut(&mut before, "a")["independent_sources"] = json!(["C"]);
    line_mut(&mut after, "a")["independent_sources"] = json!(["D"]);
    let got = check(
        probe(&before, &after, &["Invoice"], "a", "Approver.approve"),
        "undetected",
        json!("independence_credited"),
    );
    assert!(
        got["assigned_changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["direction"] == "improved")
    );
    // P03: same-subject count reduction and explicit split each over-credit.
    for split in [false, true] {
        let mut after = base.clone();
        group_mut(&mut after)["judgements"] = json!(1);
        if split {
            line_mut(&mut after, "a")["subject_sources"] = json!(["Split"]);
            let mut group = after["agency"]["lineage_groups"][0].clone();
            group["subject_sources"] = json!(["Split"]);
            group["judgements"] = json!(1);
            after["agency"]["lineage_groups"]
                .as_array_mut()
                .unwrap()
                .push(group);
        }
        let got = check(
            probe(&base, &after, &["Invoice"], "a", "Approver.approve"),
            "undetected",
            json!("independence_credited"),
        );
        // X-held-decrease (split=false): subject and independent are equal
        // before and after, so only the group row; 2 -> 1 is a reduction,
        // never held -> one improved row.
        if !split {
            assert_eq!(
                got["assigned_changes"],
                json!([{"check":"agency.lineage_groups","key":"[\"Invoice\"]","field":"judgements","before":2,"after":1,"direction":"improved"}])
            );
        }
    }
    // P04: absent consumer/group/agency is unavailable, never equality.
    for missing in ["consumer", "group", "agency"] {
        let mut after = base.clone();
        match missing {
            "consumer" => after["agency"]["lineage"]
                .as_array_mut()
                .unwrap()
                .retain(|x| x["call"] != "a"),
            "group" => after["agency"]["lineage_groups"] = json!([]),
            _ => after["agency"] = Value::Null,
        }
        check(
            probe(&base, &after, &["Invoice"], "a", "Approver.approve"),
            "inconclusive",
            json!("assigned_check_unavailable"),
        );
    }
    // P04b (X-missing-before-group): a missing *before* group is unavailable
    // too, never a zero count that the after group would hold.
    let mut before = base.clone();
    before["agency"]["lineage_groups"] = json!([]);
    let got = check(
        probe(&before, &base, &["Invoice"], "a", "Approver.approve"),
        "inconclusive",
        json!("assigned_check_unavailable"),
    );
    assert_eq!(got["assigned_changes"], json!([]));
    // P05: empty or only partially included producer sources are unassigned.
    for sources in [vec![], vec!["Invoice", "Absent"]] {
        let got = check(
            probe(&base, &base, &sources, "a", "Approver.approve"),
            "undetected",
            json!("no_assigned_check"),
        );
        assert_eq!(got["assigned_changes"], json!([]));
    }
    // P06/Q4: held means nondecrease, with actual values, not invented equality.
    let mut after = base.clone();
    group_mut(&mut after)["judgements"] = json!(3);
    let got = check(
        probe(&base, &after, &["Invoice"], "a", "Approver.approve"),
        "detected",
        Value::Null,
    );
    let c = &got["assigned_changes"][2];
    assert_eq!(
        (
            c["before"].clone(),
            c["after"].clone(),
            c["direction"].clone()
        ),
        (json!(2), json!(3), json!("held"))
    );
    // P07: a changed subject without a count reduction isn't conservation.
    let mut after = base.clone();
    line_mut(&mut after, "a")["subject_sources"] = json!(["Changed"]);
    let mut group = after["agency"]["lineage_groups"][0].clone();
    group["subject_sources"] = json!(["Changed"]);
    after["agency"]["lineage_groups"]
        .as_array_mut()
        .unwrap()
        .push(group);
    check(
        probe(&base, &after, &["Invoice"], "a", "Approver.approve"),
        "inconclusive",
        json!("assigned_check_unavailable"),
    );
    // P08: pre-comparison static and computational failure priority.
    for (verdict, truncated, result, reason) in [
        ("invalid", Value::Null, "invalid_mutant", "static_invalid"),
        (
            "unsupported",
            Value::Null,
            "invalid_mutant",
            "static_unsupported",
        ),
        ("inconclusive", json!(true), "inconclusive", "truncated"),
        (
            "unsupported",
            Value::Null,
            "inconclusive",
            "runtime_unsupported",
        ),
    ] {
        let mut after = base.clone();
        after["verdict"] = json!(verdict);
        after["bounds"]["truncated"] = truncated;
        if reason == "static_unsupported" {
            after["agency"] = Value::Null;
        }
        check(
            probe(&base, &after, &["Invoice"], "a", "Approver.approve"),
            result,
            json!(reason),
        );
    }
    // P10 (X-credited-loose): a *smaller* independent set is not new evidence.
    // {C} is a subset of {C,D} and the group stays 2 -> not credited; the
    // independent sets differ -> not held. One changed independent row.
    let mut before = base.clone();
    line_mut(&mut before, "a")["independent_sources"] = json!(["C", "D"]);
    let mut after = base.clone();
    line_mut(&mut after, "a")["independent_sources"] = json!(["C"]);
    let got = check(
        probe(&before, &after, &["Invoice"], "a", "Approver.approve"),
        "inconclusive",
        json!("assigned_check_unavailable"),
    );
    assert_eq!(
        got["assigned_changes"],
        json!([{"check":"agency.lineage","key":"a:Approver.approve","field":"independent_sources","before":["C","D"],"after":["C"],"direction":"changed"}])
    );
    // P11 (M1-subject-empty): an empty consumer subject is unassigned even
    // when the producer sources lie in its independent set and an empty-subject
    // group exists.
    let mut empty = base.clone();
    line_mut(&mut empty, "a")["subject_sources"] = json!([]);
    line_mut(&mut empty, "a")["independent_sources"] = json!(["Invoice"]);
    empty["agency"]["lineage_groups"]
        .as_array_mut()
        .unwrap()
        .push(json!({"subject_sources": [], "judgements": 1, "actors": 1, "independent_sources": ["Invoice"]}));
    let got = check(
        probe(&empty, &empty, &["Invoice"], "a", "Approver.approve"),
        "undetected",
        json!("no_assigned_check"),
    );
    assert_eq!(got["assigned_changes"], json!([]));
    // P09: another consumer with equal old payload is not assigned to a.
    let mut before = base.clone();
    line_mut(&mut before, "p")["independent_sources"] = json!([]);
    let mut after = before.clone();
    line_mut(&mut after, "p")["independent_sources"] = json!(["New"]);
    let got = check(
        probe(&before, &after, &["Invoice"], "a", "Approver.approve"),
        "detected",
        Value::Null,
    );
    assert!(
        got["other_changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["after"] == json!(["New"]))
    );
    // K5 M1: the real private comparison must not collapse repeated fields.
    // Exercise loss AND addition, retaining the final row, and row-order identity.
    let mut repeated_failures = vec![];
    for field in ["concurrency.read_write", "arrivals"] {
        let row = |value| json!({"field":field,"value":value,"affects":"fixed disclosure"});
        let mut many = base.clone();
        many["assumptions"] = json!([row("outer"), row("inner")]);
        let mut one = many.clone();
        one["assumptions"] = json!([row("inner")]);
        for (before, after, added) in [(&many, &one, false), (&one, &many, true)] {
            let got = probe(before, after, &["Invoice"], "a", "Approver.approve");
            let expected:Vec<_>=[("affects","fixed disclosure"),("field",field),("value","outer")].into_iter().map(|(leaf,text)| {
                json!({"check":"assumptions","key":format!("/field={};value=\"outer\"/{leaf}",json!(field)),"field":"value","before":if added{Value::Null}else{json!(text)},"after":if added{json!(text)}else{Value::Null},"direction":"changed"})
            }).collect();
            if got["other_changes"] != json!(expected) {
                repeated_failures.push(format!(
                    "repeated {field}, added={added}: actual={} expected={}",
                    got["other_changes"],
                    json!(expected)
                ));
            }
        }
        let mut reversed = many.clone();
        reversed["assumptions"].as_array_mut().unwrap().reverse();
        let got = probe(&many, &reversed, &["Invoice"], "a", "Approver.approve");
        if got["other_changes"] != json!([]) {
            repeated_failures.push(format!(
                "display order is not identity for {field}: {}",
                got["other_changes"]
            ));
        }
    }
    assert!(
        repeated_failures.is_empty(),
        "{}",
        repeated_failures.join("\n")
    );
}

fn structure(ast: &crate::ast::Model) -> String {
    let text = format!("{ast:?}");
    let mut rest = text.as_str();
    let mut out = String::new();
    while let Some(at) = rest.find("span: Span { start: ") {
        out.push_str(&rest[..at]);
        let suffix = &rest[at..];
        let end = suffix.find('}').unwrap();
        out.push_str("span: POSITION");
        rest = &suffix[end + 1..];
    }
    out.push_str(rest);
    out
}

/// T17/T22/F19–F21/F33: generated AST must match *each independent* hand edit.
/// Adapter receives original source and expected candidate id, and returns the
/// production editor's single edited AST (not a reimplementation).
pub fn ast_cases(mut edit: impl FnMut(&str, &str) -> crate::ast::Model) {
    let oracle: Value = serde_json::from_str(include_str!("mutation_default_oracle.json")).unwrap();
    for name in [
        "mut2-summary",
        "mut2-three-consumers",
        "mut2-guard-unchanged",
        "mut2-nested-then",
        "mut2-nested-else",
        "mut2-summary-fork",
        "mut2-repeated-argument",
    ] {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures")
                .join(name)
                .join("model.relca"),
        )
        .unwrap();
        for manual in oracle[name]["hand_edits"].as_array().unwrap() {
            let id = manual["id"].as_str().unwrap();
            if !id.starts_with("summary_reused:") {
                continue;
            }
            let got = edit(&source, id);
            let want = crate::syntax::parse(manual["source"].as_str().unwrap()).unwrap();
            assert_eq!(
                structure(&got),
                structure(&want),
                "{name}/{id}: full AST, ignoring source positions only"
            );
        }
    }
    // C6 intake: inline enum record, repeated arguments and a field guard.
    let source = integration_inputs::record();
    let got = edit(&source, "summary_reused:Flow:body/0:e->a");
    let want = crate::syntax::parse(&integration_inputs::manual(&source)).unwrap();
    assert_eq!(
        structure(&got),
        structure(&want),
        "A4 full AST, not just report equality"
    );
}

#[path = "mutation_integration_inputs.rs"]
mod integration_inputs;

#[test]
fn c6int_quality_only_report_pair_is_not_a_comparison_signal() {
    let before = crate::report::check(&integration_inputs::quality());
    let mut after = before.clone();
    let changed = crate::report::check(&integration_inputs::manual(&integration_inputs::quality()));
    assert_ne!(before.quality, changed.quality);
    after.quality = changed.quality;
    let sources = std::collections::BTreeSet::from(["Invoice".into()]);
    let equal = super::compare_summary(&before, &before, &sources, "a", "Approver.approve");
    let delta = super::compare_summary(&before, &after, &sources, "a", "Approver.approve");
    assert_eq!(equal.result, delta.result);
    assert_eq!(equal.reason, delta.reason);
    assert_eq!(
        serde_json::to_value(equal.assigned).unwrap(),
        serde_json::to_value(delta.assigned).unwrap()
    );
    assert!(equal.other.is_empty());
    assert!(delta.other.is_empty());
}

#[test]
fn c7int_quantile_only_report_pair_is_not_a_signal_in_any_of_six_comparators() {
    let s = integration_inputs::add(
        &integration_inputs::quality(),
        "metric quantile.First = quantile(1, case.Timely)",
    );
    let before = crate::report::check(&s);
    assert_eq!(
        before.quality.as_ref().unwrap().quantiles.as_ref().unwrap()[0]["value"]["lower"],
        "4"
    );
    let changed = crate::report::check(&integration_inputs::manual(&s));
    let mut after = before.clone();
    after.quality.as_mut().unwrap().quantiles = changed.quality.unwrap().quantiles;
    assert_ne!(
        before.quality, after.quality,
        "nonempty isolated quantile difference"
    );
    for kind in [0, 1, 2, 4, 8, 9] {
        let equal = super::compare(kind, &before, &before);
        let delta = super::compare(kind, &before, &after);
        assert_eq!(equal.result, delta.result, "kind {kind}");
        assert_eq!(equal.reason, delta.reason);
        assert_eq!(json!(equal.assigned), json!(delta.assigned));
        assert_eq!(json!(equal.other), json!(delta.other));
    }
    let sources = std::collections::BTreeSet::from(["Invoice".into()]);
    let equal = super::compare_summary(&before, &before, &sources, "a", "Approver.approve");
    let delta = super::compare_summary(&before, &after, &sources, "a", "Approver.approve");
    assert_eq!(equal.result, delta.result);
    assert_eq!(equal.reason, delta.reason);
    assert_eq!(json!(equal.assigned), json!(delta.assigned));
    assert_eq!(json!(equal.other), json!(delta.other));
}

#[test]
fn c6int_version_check_alone_is_a_defensive_summary_exclusion() {
    let src = integration_inputs::version(true);
    let ast = crate::syntax::parse(&src).unwrap();
    let mut context = crate::lower::lower(&ast).unwrap();
    let cap = context
        .capabilities
        .iter_mut()
        .find(|c| c.name == "make")
        .unwrap();
    assert!(cap.version_check.is_some());
    // Deliberately forged IR isolates the exclusion. Real DSL already has a
    // shared writer, so this is NOT public-source mutation detection strength.
    cap.start_writes.clear();
    cap.writes.clear();
    cap.kernel_writes.clear();
    assert!(
        super::summary_shared_effect(cap, &crate::lower::lower(&ast).unwrap()),
        "version_check must independently forbid duplication"
    );
}

#[test]
fn c6int_expiry_editor_clones_rule_change_grants_and_not_by_exactly() {
    let src = integration_inputs::rule(true, false);
    let ast = crate::syntax::parse(&src).unwrap();
    let candidates = super::authority_sites(&src, &ast, None);
    assert_eq!(candidates.len(), 1);
    let got = candidates[0].apply(&ast);
    let want = integration_inputs::replace(&src, "not_by [a1] }", "not_by [a1]; expires 0min }");
    let want = crate::syntax::parse(&want).unwrap();
    assert_eq!(
        structure(&got),
        structure(&want),
        "all A6 authority metadata, not just the expiry verdict"
    );
}

#[test]
fn c6int_rule_judgement_is_not_reinterpreted_as_a_process_call() {
    let mut before = crate::report::check(&integration_inputs::rule(false, false));
    assert_eq!(before.verdict, crate::Verdict::Satisfied);
    let refusal = &mut before
        .agency
        .as_mut()
        .unwrap()
        .as_derived_mut()
        .unwrap()
        .refusal;
    for r in refusal.iter_mut() {
        r.judgements
            .retain(|j| matches!(j, crate::agency::Judgement::Rule(_)));
    }
    assert!(refusal.iter().any(|r| !r.judgements.is_empty()));
    let mut after = before.clone();
    for r in &mut after
        .agency
        .as_mut()
        .unwrap()
        .as_derived_mut()
        .unwrap()
        .refusal
    {
        for j in &mut r.judgements {
            if let crate::agency::Judgement::Rule(rule) = j {
                rule.version = "r2".into();
            }
        }
    }
    let comparison = super::compare(8, &before, &after);
    assert_eq!(comparison.result, "undetected");
    assert_eq!(comparison.reason, Some("no_assigned_check"));
    assert!(comparison.assigned.is_empty());
    assert!(
        comparison
            .other
            .iter()
            .any(|c| c.check == "agency" && c.field == "value")
    );
}

#[test]
fn c6int_business_horizon_inconclusive_is_not_counted_as_summary_detection() {
    let source = integration_inputs::quality_horizon(4);
    let before = crate::report::check_with_selection(&source, None, Some("Day"));
    let after = crate::report::check_with_selection(
        &integration_inputs::manual(&source),
        None,
        Some("Day"),
    );
    assert_eq!(before.verdict, crate::Verdict::Satisfied);
    assert_eq!(after.verdict, crate::Verdict::Inconclusive);
    assert_eq!(after.bounds.truncated, Some(false));
    let sources = std::collections::BTreeSet::from(["Invoice".into()]);
    let comparison = super::compare_summary(&before, &after, &sources, "a", "Approver.approve");
    assert_eq!(
        comparison.result, "inconclusive",
        "business cutoff is not a completed mutation detection"
    );
    assert_eq!(comparison.reason, Some("assigned_check_unavailable"));
}

#[test]
fn c6fix_summary_candidate_sources_are_the_exact_all_writer_union() {
    use std::collections::BTreeSet;
    for (source, multiple, expected) in [
        (
            true,
            true,
            BTreeSet::from(["Archive".into(), "Invoice".into()]),
        ),
        (true, false, BTreeSet::from(["Invoice".into()])),
        (false, false, BTreeSet::new()),
    ] {
        let src = integration_inputs::observed(source, multiple, false);
        let ast = crate::syntax::parse(&src).unwrap();
        let context = crate::lower::lower(&ast).unwrap();
        let candidates = super::summary_sites(&src, &ast, &context);
        assert_eq!(
            candidates.len(),
            1,
            "empty sources must not erase the candidate"
        );
        let candidate = &candidates[0];
        assert_eq!(candidate.site.affected_bindings, ["e", "a"]);
        match &candidate.edit {
            super::Edit::Summary {
                sources, consumer, ..
            } => {
                assert_eq!(consumer.binding, "a");
                assert_eq!(
                    sources, &expected,
                    "candidate sources, not Report held rows"
                );
            }
            _ => panic!("expected the real Summary candidate"),
        }
    }
}

#[test]
fn c6fix_start_only_writer_contributes_to_the_actual_summary_candidate() {
    let src = integration_inputs::start_only_writer();
    let ast = crate::syntax::parse(&src).unwrap();
    let context = crate::lower::lower(&ast).expect("public DSL, not forged IR");
    let snap = context
        .state_variables
        .iter()
        .position(|v| v.name == "snap")
        .unwrap();
    let writer = context
        .capabilities
        .iter()
        .find(|c| context.actors[c.actor.0].name == "Writer" && c.name == "make")
        .unwrap();
    assert!(writer.start_writes.iter().any(|w| w.variable.0 == snap));
    assert!(writer.finish_writes().all(|w| w.variable.0 != snap));
    assert!(writer.kernel_writes.is_empty());
    assert!(writer.kernel.is_none());
    let candidates = super::summary_sites(&src, &ast, &context);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].site.affected_bindings, ["e", "a"]);
    let super::Edit::Summary { sources, .. } = &candidates[0].edit else {
        panic!("expected the real Summary candidate");
    };
    assert_eq!(
        sources,
        &std::collections::BTreeSet::from(["Invoice".into()])
    );
}

/// Deliberately forged Report pairs isolate the public wire's precondition.
/// Public reachability of every pair is NOT claimed. In particular Unsupported
/// retains real Agency while the root verdict alone is changed.
fn c6fix_classification_matrix(
    mut compare: impl FnMut(&crate::report::Report, &crate::report::Report) -> super::Comparison,
    summary: bool,
) {
    let before = crate::report::check(integration_inputs::S);
    classification_matrix_with_report(&before, &mut compare, summary);
}

fn classification_matrix_with_report(
    before: &crate::report::Report,
    mut compare: impl FnMut(&crate::report::Report, &crate::report::Report) -> super::Comparison,
    summary: bool,
) {
    use crate::Verdict::{Inconclusive, Invalid, Satisfied, Unsupported, Vacuous, Violated};
    assert_eq!(before.verdict, Satisfied);
    assert!(before.agency.is_some());
    let mut errors = vec![];
    let mut observed = 0;
    for verdict in [
        Satisfied,
        Violated,
        Vacuous,
        Inconclusive,
        Invalid,
        Unsupported,
    ] {
        for truncated in [Some(false), Some(true), None] {
            for agency in [true, false] {
                let mut after = before.clone();
                after.verdict = verdict;
                after.bounds.truncated = truncated;
                if !agency {
                    after.agency = None;
                }
                let expected = if verdict == Invalid {
                    Some(("invalid_mutant", "static_invalid"))
                } else if verdict == Unsupported && !agency {
                    Some(("invalid_mutant", "static_unsupported"))
                } else if truncated.is_none() {
                    Some(("inconclusive", "runtime_unsupported"))
                } else if truncated == Some(true) {
                    Some(("inconclusive", "truncated"))
                } else if verdict == Unsupported {
                    // The check could not execute, not a
                    // business horizon. Preserve the existing reason vocabulary.
                    Some(("inconclusive", "runtime_unsupported"))
                } else if verdict == Inconclusive {
                    Some(("inconclusive", "assigned_check_unavailable"))
                } else {
                    None
                };
                let c = compare(before, &after);
                observed += 1;
                if let Some((result, reason)) = expected {
                    if c.result != result || c.reason != Some(reason) {
                        errors.push(format!("{verdict:?}/{truncated:?}/agency={agency}: {}/ {:?}, expected {result}/{reason}", c.result, c.reason));
                    }
                    // The early failure must not smuggle static held/worsened
                    // rows through a non-comparison classification.
                    if !c.assigned.is_empty() || !c.other.is_empty() {
                        errors.push(format!(
                            "{verdict:?}/{truncated:?}/agency={agency}: comparison payload leaked"
                        ));
                    }
                } else if agency {
                    assert_eq!(c.result, if summary { "detected" } else { "undetected" });
                    if summary {
                        assert!(c.reason.is_none());
                        assert_eq!(c.assigned.len(), 3);
                    } else {
                        assert!(matches!(
                            c.reason,
                            Some("no_worsening" | "no_assigned_check")
                        ));
                    }
                }
            }
        }
    }
    assert_eq!(observed, 36);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

macro_rules! c6fix_kind_matrix {
    ($test:ident, $kind:expr) => {
        #[test]
        fn $test() {
            c6fix_classification_matrix(|b, a| super::compare($kind, b, a), false);
        }
    };
}
c6fix_kind_matrix!(c6fix_evidence_missing_report_classification_matrix, 0);
c6fix_kind_matrix!(c6fix_authority_expired_report_classification_matrix, 1);
c6fix_kind_matrix!(c6fix_hold_delayed_report_classification_matrix, 2);
c6fix_kind_matrix!(c6fix_refusal_path_cut_report_classification_matrix, 8);
c6fix_kind_matrix!(
    c6fix_original_access_removed_report_classification_matrix,
    9
);

#[test]
fn c6fix_summary_reused_report_classification_matrix() {
    let sources = std::collections::BTreeSet::from(["Invoice".into()]);
    c6fix_classification_matrix(
        |b, a| super::compare_summary(b, a, &sources, "a", "Approver.approve"),
        true,
    );
}

#[test]
fn c6fix_incomplete_rows_make_the_run_inconclusive_for_every_generated_kind() {
    let mut seen = std::collections::BTreeSet::new();
    let mut errors = vec![];
    for src in [integration_inputs::S, integration_inputs::X] {
        let base = crate::report::check(src);
        assert_eq!(base.verdict, crate::Verdict::Satisfied);
        for (verdict, reason) in [
            (crate::Verdict::Inconclusive, "assigned_check_unavailable"),
            (crate::Verdict::Unsupported, "runtime_unsupported"),
        ] {
            let mut incomplete = base.clone();
            incomplete.verdict = verdict;
            incomplete.bounds.truncated = Some(false);
            assert!(incomplete.agency.is_some());
            let r =
                super::mutate_using(src, 128, None, |_| base.clone(), |_, _| incomplete.clone());
            for (kind, row) in r.mutations.iter().enumerate() {
                for m in &row.mutants {
                    seen.insert(kind);
                    if m.result != "inconclusive" || m.reason != Some(reason) {
                        errors.push(format!("{verdict:?}/{}: {}/{:?}", m.id, m.result, m.reason));
                    }
                }
            }
            if r.run.completion != "inconclusive" || r.run.reason != Some("mutant_incomplete") {
                errors.push(format!(
                    "{verdict:?}/run: {}/{:?}, expected inconclusive/mutant_incomplete",
                    r.run.completion, r.run.reason
                ));
            }
        }
    }
    assert_eq!(seen, std::collections::BTreeSet::from([0, 1, 2, 4, 8, 9]));
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

fn a7onto_waiting_source() -> String {
    // Same hand-authored one-case S shape as waits_mutation_inputs::summary;
    // keep this private seam independent of public-API helper crate imports.
    let src =
        integration_inputs::S.replace("result nondet { pass, fail }", "result nondet { pass }");
    let src =
        integration_inputs::replace(&src, "bounds { cases 1; live_tasks 2; clock 10min }", "");
    let src = integration_inputs::replace(
        &src,
        "actor Extract { via model \"doc@v3\"; capacity 1;",
        "actor Extract { via model \"doc@v3\"; executors [p];",
    );
    integration_inputs::add(
        &src,
        "policy fifo tie_by case_id\nscenario Day { arrivals fixed [0min]; horizon 12min; bounds { cases 1; live_tasks 1; clock 20min } }\nmetrics { window 20min; currency \"JPY\"; breakdown waits }\nproperty case.Done = reached(end.done)\nmetric cohort.DoneRate = expected(count(case.Done)) / arrivals\npeople [p,cfo]\nauthority G { basis \"prior permission\"; grants p to Extract.make; issued_by cfo }\ncontract K { authority_basis Extract.make G }",
    )
}

#[test]
fn a7onto_waiting_reports_cross_all_six_classification_matrices() {
    let src = a7onto_waiting_source();
    let before = crate::report::check_with_selection(&src, None, Some("Day"));
    assert!(before.metrics.is_some());
    assert!(
        before
            .assumptions
            .iter()
            .any(|a| a.field == "metrics.waiting")
    );
    for kind in [0, 1, 2, 4, 8, 9] {
        classification_matrix_with_report(
            &before,
            |b, a| {
                if kind == 4 {
                    super::compare_summary(
                        b,
                        a,
                        &std::collections::BTreeSet::from(["Invoice".into()]),
                        "a",
                        "Approver.approve",
                    )
                } else {
                    super::compare(kind, b, a)
                }
            },
            kind == 4,
        );
    }
}

#[test]
fn a7onto_waiting_incomplete_run_uses_every_real_generated_kind() {
    let src = a7onto_waiting_source();
    let base = crate::report::check_with_selection(&src, None, Some("Day"));
    assert_eq!(base.verdict, crate::Verdict::Satisfied);
    for (verdict, reason) in [
        (crate::Verdict::Inconclusive, "assigned_check_unavailable"),
        (crate::Verdict::Unsupported, "runtime_unsupported"),
    ] {
        let mut incomplete = base.clone();
        incomplete.verdict = verdict;
        let r = super::mutate_using(
            &src,
            128,
            Some("Day"),
            |_| base.clone(),
            |_, _| incomplete.clone(),
        );
        for (kind, count) in [(0, 2), (1, 1), (2, 1), (4, 1), (8, 1), (9, 2)] {
            let rows = &r.mutations[kind].mutants;
            assert_eq!(rows.len(), count);
            for row in rows {
                assert_eq!(row.result, "inconclusive", "{}", row.id);
                assert_eq!(row.reason, Some(reason));
                assert!(row.assigned_changes.is_empty());
                assert!(row.other_changes.is_empty());
                let report = serde_json::to_value(row.report.as_ref().unwrap()).unwrap();
                assert_eq!(
                    report["assumptions"],
                    serde_json::to_value(&base.assumptions).unwrap()
                );
                assert!(
                    report["metrics"]["overall"]
                        .get("queue_wait_observed")
                        .is_some()
                );
            }
        }
        assert_eq!(r.run.completion, "inconclusive");
        assert_eq!(r.run.reason, Some("mutant_incomplete"));
    }
}
