use super::*;
#[test]
fn postprocessing_limit_is_not_execution_abort_and_prior_witnesses_are_preserved() {
    let src = "model M version \"0.2\" {process Flow {end done} bounds {cases 1;live_tasks 1;clock 1min} verify safety no_resource_overcommit verify reachable end.done verify recoverable end verify terminating end verify almost_sure end}";
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    for budget in [0, 1] {
        let mut out = engine::explore_with_budget(&m, budget);
        let old: Vec<_> = out
            .properties
            .iter()
            .filter(|p| !p.property.completion())
            .cloned()
            .collect();
        let graph = crate::completion::Graph {
            exceeded: true,
            ..Default::default()
        };
        crate::completion::apply(&mut out, &m, &graph, false);
        assert_eq!(
            out.properties
                .iter()
                .filter(|p| !p.property.completion())
                .cloned()
                .collect::<Vec<_>>(),
            old
        );
        assert!(
            out.postprocessing_diagnostics
                .iter()
                .all(|d| d.code == diag::U_SEM_0 && d.span.is_some())
        );
        let base = empty_report(src);
        let report = apply_outcome(base.clone(), &m, out.clone());
        assert_eq!(report.bounds.truncated, Some(budget == 0));
        assert!(report.unchecked.iter().any(|u| u == "completion_graph"));
        assert_eq!(
            report.properties.last().unwrap().probability,
            Some(crate::probability::ProbabilityRange {
                min: crate::probability::Rational::zero(),
                max: crate::probability::Rational::one()
            })
        );
        assert_eq!(
            report.unchecked.iter().any(|u| u == "unreachable"),
            budget == 0
        );
        if budget == 1 {
            assert_eq!(report.properties[1].verdict, Verdict::Satisfied);
        }
        // A real execution error still makes truncated unknown, even when the
        // same outcome also contains the unrelated postprocessing error.
        out.diagnostics.push(diag::Diagnostic {
            code: diag::U_SEM_0.into(),
            severity: diag::Severity::Error,
            message: "injected execution arithmetic abort".into(),
            span: None,
        });
        if !out.unchecked.iter().any(|u| u == "unreachable") {
            out.unchecked.insert(0, "unreachable".into());
        }
        assert_eq!(apply_outcome(base, &m, out).bounds.truncated, None);
    }
}
