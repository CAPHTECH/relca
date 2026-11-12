//! ADR 0025 private seam tests. The legacy census tests live elsewhere.
use super::*;
fn model(src: &str) -> ir::Model {
    crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap()
}
#[test]
fn end_target_grouping_multiline_and_existing_depth_limit() {
    let base = "model M version \"0.2\" {process Flow {end done} bounds {cases 1;live_tasks 1;clock 1min} verify probability reached TARGET within 0min >= 1}";
    for target in ["end.done", "((end.done))", "(\n(\nend.done\n)\n)"] {
        let m = model(&base.replace("TARGET", target));
        assert_eq!(explore(&m).properties[0].verdict, Verdict::Satisfied);
    }
    for target in [
        "(end.done && end.done)",
        "((end.done) || end.done)",
        "(end.done == true)",
    ] {
        let ds = crate::syntax::parse(&base.replace("TARGET", target)).unwrap_err();
        assert_eq!(ds[0].code, diag::U_SYN_0);
    }
    for (n, accepted) in [(127, true), (128, false)] {
        let target = format!("{}end.done{}", "(".repeat(n), ")".repeat(n));
        assert_eq!(
            crate::syntax::parse(&base.replace("TARGET", &target)).is_ok(),
            accepted
        );
    }
}
#[test]
fn lexical_paths_disambiguate_direct_ir_if_sites_even_when_source_spans_coincide() {
    let mut m = model(
        "model M version \"0.2\" {type T=enum {yes} actor A {capacity 1;capability run()->T {duration fixed 1min;result nondet {yes}}} process Flow {a=A.run();if a==yes {if a==yes {} else {}} else {if a==yes {} else {}};end done} bounds {cases 1;live_tasks 1;clock 2min} verify safety no_resource_overcommit}",
    );
    fn spans(block: &mut ir::Block) {
        for s in block {
            s.span = diag::Span { start: 0, end: 1 };
            if let ir::Stmt::If {
                then_body,
                else_body,
                ..
            } = &mut s.kind
            {
                spans(then_body);
                spans(else_body);
            }
        }
    }
    spans(&mut m.process.body);
    let e = explore(&m);
    let r = crate::reference::explore(&m);
    assert_eq!(r.stop, crate::reference::StopReason::Complete);
    for ds in [&e.diagnostics, &r.outcome.diagnostics] {
        assert_eq!(ds.iter().filter(|d| d.code == diag::W_UNREACH_1).count(), 4);
    }
}
#[test]
fn malformed_completion_ir_and_probability_zero_budget_fail_closed() {
    let base = model(include_str!(
        "../../../../fixtures/done-probability/model.relca"
    ));
    let mut bad = base.clone();
    bad.verify.push(bad.verify[1].clone());
    assert!(
        explore(&bad)
            .diagnostics
            .iter()
            .any(|d| d.code == diag::U_SEM_0)
    );
    assert_eq!(
        crate::reference::explore(&bad).stop,
        crate::reference::StopReason::SemanticAbort
    );
    let mut bad = base.clone();
    bad.probability_queries[0].condition = ir::ProbabilityCondition::End(ir::EndId(usize::MAX));
    assert!(
        explore(&bad)
            .diagnostics
            .iter()
            .any(|d| d.code == diag::U_SEM_0)
    );
    assert_eq!(
        crate::reference::explore(&bad).stop,
        crate::reference::StopReason::SemanticAbort
    );
    let o = explore_with_budget(&base, 0);
    for p in o.properties.iter().filter(|p| p.property.completion()) {
        assert_eq!(p.verdict, Verdict::Inconclusive);
        if p.property == ir::Property::AlmostSureEnd {
            assert_eq!(
                p.probability,
                Some(ProbabilityRange {
                    min: Rational::zero(),
                    max: Rational::one()
                })
            );
        }
    }
}

#[test]
fn known_runtime_witnesses_survive_budget_series_and_later_sidecar_refusal() {
    let m = model(include_str!(
        "../../../../fixtures/done-recovery.broken-zero-capacity/model.relca"
    ));
    let mut control = m.clone();
    control.verify.retain(|v| !v.property.completion());
    let mut cut_with_deadlock = 0;
    for budget in 0..40 {
        let mut actual = explore_with_budget(&m, budget);
        let old = explore_with_budget(&control, budget);
        if actual.truncated
            && actual
                .diagnostics
                .iter()
                .any(|d| d.code == diag::W_DEADLOCK_0)
        {
            cut_with_deadlock += 1;
            for p in actual.properties.iter().filter(|p| p.property.completion()) {
                assert_eq!(p.verdict, Verdict::Violated);
            }
        }
        actual.properties.retain(|p| !p.property.completion());
        assert_eq!(
            actual, old,
            "unchanged runtime evidence at state budget {budget}"
        );
    }
    assert!(
        cut_with_deadlock > 0,
        "test must actually encounter a known failure before another branch is cut"
    );
    let mut e = Exploration::default();
    probability_graph(&m, DEFAULT_STATE_BUDGET, &mut e).unwrap();
    let mut graph = e.completion.take().unwrap();
    assert!(graph.known_failure);
    let mut out = e.outcome(&m);
    let old: Vec<_> = out
        .properties
        .iter()
        .filter(|p| !p.property.completion())
        .cloned()
        .collect();
    // Exercise refusal after a real runtime witness. The separate k/k+1 test
    // materializes the actual 2M retained edges; this seam avoids doing it twice.
    graph.edges = crate::completion::EDGE_LIMIT;
    graph.action(0, vec![(Rational::one(), 0)]);
    crate::completion::apply(&mut out, &m, &graph, false);
    assert_eq!(
        out.properties
            .iter()
            .filter(|p| !p.property.completion())
            .cloned()
            .collect::<Vec<_>>(),
        old
    );
    assert!(out.diagnostics.iter().any(|d| d.code == diag::W_DEADLOCK_0));
    assert!(
        out.properties
            .iter()
            .filter(|p| p.property.completion())
            .all(|p| p.verdict == Verdict::Violated)
    );
}
#[test]
fn completion_sidecar_census_and_timing_small_and_large_models() {
    for (name, src) in [
        (
            "small",
            include_str!("../../../../fixtures/done-call-kind/model.relca"),
        ),
        (
            "large-invoice",
            include_str!("../../../../examples/invoice/as-is.relca"),
        ),
    ] {
        let mut m = model(src);
        if !m.completion_requested() {
            m.verify.push(ir::Verify {
                property: ir::Property::RecoverableEnd,
                span: m.process.span,
            });
        }
        let mut e = Exploration::default();
        let start = std::time::Instant::now();
        if m.uses_probability() {
            probability_graph(&m, DEFAULT_STATE_BUDGET, &mut e).unwrap();
        } else {
            explore_into(
                &m,
                true,
                DEFAULT_STATE_BUDGET,
                std::convert::identity,
                &mut e,
            )
            .unwrap();
        }
        let build = start.elapsed();
        let g = e.completion.as_ref().unwrap();
        let start = std::time::Instant::now();
        let a = g.analyze(false);
        let reduce = start.elapsed();
        eprintln!(
            "completion measurement {name}: nodes={} additional_edges={} explore_and_capture_us={} postprocess_us={} exceeded={}",
            g.nodes.len(),
            g.edges,
            build.as_micros(),
            reduce.as_micros(),
            g.exceeded
        );
        assert!(!g.exceeded);
        assert_eq!(a.recoverable, Verdict::Satisfied);
        let mut stripped = m.clone();
        stripped.verify.retain(|v| !v.property.completion());
        let mut old = Exploration::default();
        if m.uses_probability() {
            let (n, _) = probability_graph(&stripped, DEFAULT_STATE_BUDGET, &mut old).unwrap();
            assert_eq!(n.len(), g.nodes.len());
        } else {
            explore_into(
                &stripped,
                true,
                DEFAULT_STATE_BUDGET,
                std::convert::identity,
                &mut old,
            )
            .unwrap();
            assert_eq!(old.census, e.census);
        }
        assert!(old.completion.is_none());
    }
}

#[test]
fn route_deadlock_after_business_end_is_not_a_known_failure_on_the_probability_graph() {
    // ADR 0025 追記: business end absorbs the contest route; a route that can
    // never start (capacity 0) deadlocks only after the case completed. The
    // retained known_failure decides R/A/P when the edge limit is exceeded,
    // so it must not count that leaf.
    let src = include_str!("../../../../fixtures/done-end-vs-close/model.relca")
        .replace(
            "actor Route { executors [p]; capability respond() -> T { duration fixed 10min;",
            "actor Route { capacity 0; capability respond() -> T { duration fixed 1min;",
        )
        .replace("  permit p to Route.respond\n", "")
        .replace("clock 2min", "clock 30min");
    assert!(src.contains("actor Route { capacity 0;"));
    let m = model(&src);
    let mut e = Exploration::default();
    probability_graph(&m, DEFAULT_STATE_BUDGET, &mut e).unwrap();
    let graph = e.completion.take().unwrap();
    assert!(!graph.known_failure);
    let out = e.outcome(&m);
    assert!(out.diagnostics.iter().any(|d| d.code == diag::W_DEADLOCK_0));
}
