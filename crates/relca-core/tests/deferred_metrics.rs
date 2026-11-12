#[path = "support/deferred_metrics_inputs.rs"]
mod h;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn outcome(name: &str, sim: bool, independent: bool) -> (Value, Value) {
    let s = h::source(name);
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    match (independent, sim) {
        (true, true) => {
            let x = reference::simulate(&m);
            assert_eq!(x.stop, reference::StopReason::Complete);
            assert_eq!(x.outcome.replications.len(), 3);
            for r in &x.outcome.replications {
                h::assert_metrics(&json!(r.metrics), name, false);
            }
            (
                json!(x.outcome.replications[0].metrics),
                json!(x.outcome.quality),
            )
        }
        (true, false) => {
            let x = reference::explore(&m);
            assert_eq!(x.stop, reference::StopReason::Complete);
            (json!(x.outcome.metrics), json!(x.outcome.quality))
        }
        (false, true) => {
            let x = engine::simulate(&m);
            assert_eq!(x.replications.len(), 3);
            for r in &x.replications {
                h::assert_metrics(&json!(r.metrics), name, false);
            }
            (json!(x.replications[0].metrics), json!(x.quality))
        }
        (false, false) => {
            let x = engine::explore(&m);
            (json!(x.metrics), json!(x.quality))
        }
    }
}
#[test]
fn no_defer_control_is_numeric_in_both_interpreters_and_modes() {
    for sim in [false, true] {
        for independent in [false, true] {
            let (m, q) = outcome("none", sim, independent);
            h::assert_metrics(&m, "none", false);
            h::assert_quality(&q, "none", sim);
        }
    }
}
#[test]
fn resolved_actor_capability_opaque_and_unused_layers_have_hand_values() {
    for name in [
        "actor",
        "capability",
        "opaque",
        "unrelated",
        "unused",
        "quality_only",
        "no_indicators",
        "dead_branch",
        "two_caps",
        "duplicate",
    ] {
        for sim in [false, true] {
            let e = outcome(name, sim, false);
            let r = outcome(name, sim, true);
            for (m, q) in [&e, &r] {
                h::assert_metrics(m, name, false);
                h::assert_quality(q, name, sim);
            }
            assert_eq!(e, r, "{name} mode={sim}");
        }
    }
}
#[test]
fn public_report_quality_only_and_no_indicators_checked_boundary() {
    for name in h::NAMES {
        if name != "truncated" {
            let v = json!(report::check(&h::source(name)));
            h::assert_report(&v, name, false);
            let _: report::Report = serde_json::from_value(v.clone()).unwrap();
            if name == "actor" {
                let mut bad = v;
                bad["quality"]["cohort"][0]["value"]["reason"] = json!("not_configureX");
                assert!(serde_json::from_value::<report::Report>(bad).is_err());
            }
            h::assert_report(&json!(report::simulate(&h::source(name))), name, true);
        }
    }
}
#[test]
fn computational_cut_reason_is_not_hidden_by_defer() {
    let v = json!(report::check(&h::source("truncated")));
    assert_eq!(v["bounds"]["truncated"], true);
    assert_eq!(v["metrics"]["overall"]["work"]["reason"], "truncated");
    assert_eq!(v["quality"]["cohort"][0]["value"]["reason"], "truncated");
}
#[test]
fn hand_oracle_rejects_a_single_wrong_non_deferred_cell() {
    let mut m = outcome("none", false, false).0;
    m["actors"][0]["work"]["min"] = json!("4");
    assert!(std::panic::catch_unwind(|| h::assert_metrics(&m, "none", false)).is_err());
    assert!(std::panic::catch_unwind(|| h::row(&m["actors"], "missing_key", "A")).is_err());
}
#[test]
fn compare_transports_missing_work_quality_quantile_and_never_invents_dominance() {
    for hard in [false, true] {
        let s = h::comparison_source(hard);
        h::assert_comparison(
            &json!(relca_core::compare::compare(
                &s,
                &["Included", "Unrelated"],
                relca_core::compare::Analysis::Exhaustive
            )),
            hard,
            false,
        );
        h::assert_comparison(
            &json!(relca_core::compare::compare(
                &s,
                &["Included", "Unrelated"],
                relca_core::compare::Analysis::Simulation
            )),
            hard,
            true,
        );
        let d = json!(relca_core::diff::diff(&s, "Included", "Unrelated"));
        h::assert_report(&d["from"]["report"], "actor", false);
        h::assert_report(&d["to"]["report"], "unrelated", false);
        let work = d["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| {
                r["key"]["scope"] == "overall"
                    && r["key"].get("metric").expect("MetricKey.metric") == "work"
            })
            .unwrap();
        assert_eq!(work["from"]["value"], h::absent("not_configured", false));
        assert_eq!(work["to"]["value"], h::scalar("3", false));
    }
}
#[test]
fn comparison_substrate_without_defer_is_a_positive_control() {
    let s = h::comparison_control();
    let v = json!(relca_core::compare::compare(
        &s,
        &["Included", "Unrelated"],
        relca_core::compare::Analysis::Exhaustive
    ));
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(v["evaluation"]["objectives"][0]["values"][0]["lower"], "5");
    assert_eq!(v["evaluation"]["objectives"][0]["values"][1]["lower"], "3");
}
#[test]
fn mutation_original_and_explain_are_not_an_escape_from_deferred_layers() {
    let s = h::source("actor");
    let v = json!(relca_core::mutation::mutate(&s));
    h::assert_report(&v["original"], "actor", false);
    let v = json!(relca_core::explain::explain(&s));
    h::assert_report(&v, "actor", false);
}
#[test]
fn defer_never_changes_core_terminal_work_contract_or_exploration_result() {
    for independent in [false, true] {
        let mut runs = Vec::new();
        for name in ["none", "actor", "capability", "opaque", "duplicate"] {
            let s = h::source(name);
            let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
            let mut out = if independent {
                reference::explore(&m).outcome
            } else {
                engine::explore(&m)
            };
            out.metrics = None;
            out.quality = None;
            runs.push(out);
        }
        for other in &runs[1..] {
            assert!(other == &runs[0], "only analyst indicators may change");
        }
    }
}
#[test]
fn a_static_cut_error_cannot_claim_deferred_metrics_checked() {
    let s = h::static_error_source();
    let v = json!(report::check(&s));
    assert_eq!(v["verdict"], "invalid");
    assert!(v["metrics"].is_null());
    assert!(
        !v["cut"]["checked"]
            .as_array()
            .is_some_and(|a| a.contains(&json!("deferred_metrics")))
    );
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CUT-1")
    );
}
#[test]
fn existing_semantic_refusal_reason_wins_but_the_defer_check_is_applied() {
    let s = h::refusal_source();
    let v = json!(report::simulate(&s));
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(v["metrics"]["overall"]["work"]["reason"], "aborted");
    assert_eq!(v["quality"]["cohort"][0]["value"]["reason"], "aborted");
    assert_eq!(
        v["cut"]["checked"].as_array().unwrap().last(),
        Some(&json!("deferred_metrics"))
    );
}
#[test]
fn actor_names_and_grain_labels_do_not_select_a_metric_layer() {
    let s = h::relabeled_source();
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    for independent in [false, true] {
        let x = if independent {
            reference::explore(&m).outcome
        } else {
            engine::explore(&m)
        };
        let metrics: Value =
            serde_json::from_str(&json!(x.metrics).to_string().replace("Unit", "A")).unwrap();
        h::assert_metrics(&metrics, "capability", false);
        h::assert_quality(&json!(x.quality), "capability", false);
    }
}
#[test]
fn shared_person_aggregate_withdraws_other_actor_work_conservatively() {
    let s = h::shared_person_source();
    assert!(s.contains("actor R { executors [p]"));
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    for independent in [false, true] {
        let out = if independent {
            reference::explore(&m).outcome
        } else {
            engine::explore(&m)
        };
        let metrics = json!(out.metrics);
        assert_eq!(
            h::row(&metrics["actors"], "actor", "A")["work"],
            h::absent("not_configured", false)
        );
        assert_eq!(
            h::row(&metrics["people"], "person", "p")["work"],
            h::absent("not_configured", false)
        );
        assert_eq!(
            h::row(&metrics["people"], "person", "q")["work"],
            h::absent("not_configured", false)
        );
    }
}
#[test]
fn closed_new_inline_population_has_nineteen_named_constructions() {
    let cases = h::all_sources();
    assert_eq!(
        cases.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        h::NEW_INLINE_CASES
    );
    assert_eq!(
        cases
            .iter()
            .map(|(_, s)| s)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        18,
        "unrelated and unused deliberately share one source"
    );
    let mut eligible = 0;
    for (name, src) in cases {
        let ast = syntax::parse(&src).unwrap();
        let indicator = ast.items.iter().any(|i| {
            matches!(
                i,
                relca_core::ast::Item::Metrics(_) | relca_core::ast::Item::QualityProperty(_)
            )
        });
        let defer=ast.items.iter().any(|i|matches!(i,relca_core::ast::Item::Cut(c) if c.items.iter().any(|x|matches!(x.value,relca_core::ast::CutItem::Defer(_)))));
        let result = lower::lower_with_selection(
            &ast,
            if name.starts_with("compare_") {
                Some("Included")
            } else {
                None
            },
            None,
        );
        if name == "static_cut_error" {
            assert!(result.unwrap_err().iter().any(|d| d.code == "E-CUT-1"));
        } else {
            assert!(result.is_ok(), "{name}: {:?}", result.err());
            eligible += usize::from(indicator && defer);
        }
    }
    assert_eq!((eligible, 19 - eligible), (15, 4));
}
#[test]
fn forged_deferred_actor_id_fails_closed_before_both_collectors() {
    let src = h::source("actor");
    let mut m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
    m.cut.as_mut().unwrap().defer[0].target =
        relca_core::ir::CutTarget::Actor(relca_core::ir::ActorId(usize::MAX));
    let e = engine::explore(&m);
    assert!(e.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
    assert!(e.metrics.is_none());
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    assert!(r.outcome.metrics.is_none());
    let e = engine::simulate(&m);
    assert!(e.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
    assert!(e.replications.is_empty());
    let r = reference::simulate(&m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    assert!(r.outcome.replications.is_empty());
}
