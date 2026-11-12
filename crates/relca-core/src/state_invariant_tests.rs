use crate::{engine, explain, ir, lower, reference, syntax};

#[test]
fn state_invariant_lookahead_keeps_legacy_three_segment_case_named_actor_events() {
    let src = r#"model M version "0.2" {
        type T=enum{ok}
        actor case{capacity 1;capability received()->T{duration fixed 1min;result nondet{ok}}}
        process P{x=case.received();end done}
        cut C{include[case]} bounds{cases 1;live_tasks 1;clock 2min}
        contract Controls{invariant case.received.finish -> x == ok}
    }"#;
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    assert!(matches!(
        m.contracts[0].kind,
        ir::ContractKind::Invariant { .. }
    ));
    assert_eq!(crate::report::check(src).verdict, crate::Verdict::Satisfied);
    assert_eq!(reference::explore(&m).stop, reference::StopReason::Complete);
}

#[test]
fn state_invariant_missing_qualified_operands_are_invalid_not_unsupported_bare_atoms() {
    let src = include_str!("../../../fixtures/sinv-initial/model.relca");
    for clause in [
        "invariant Fix.",
        "invariant (Fix.)",
        "invariant end",
        "invariant end.",
        "invariant ok ->",
        "invariant ok &&",
        "invariant (ok",
    ] {
        let input = src.replace("invariant ok == true", clause);
        assert_ne!(input, src);
        let errors = syntax::parse(&input).unwrap_err();
        assert_eq!(errors[0].code, "E-SYN-0", "{clause}: {errors:?}");
    }
    for clause in [
        "invariant Fix.flag",
        "invariant end.done -> ok == true",
        "invariant case.received -> ok == true",
    ] {
        let input = src.replace("invariant ok == true", clause);
        assert_eq!(
            syntax::parse(&input).unwrap_err()[0].code,
            "U-SYN-0",
            "{clause}"
        );
    }
}

#[test]
fn state_invariant_qualified_latents_and_case_names_replay_by_declaration_not_bare_name() {
    let src = r#"model M version "0.2" {
        observed case{flag:bool=true}
        actor A{latent flag:bool=false}
        actor B{latent flag:bool=true}
        process P{end done}
        cut C{include[A,B]} bounds{cases 1;live_tasks 1;clock 2min}
        contract Controls{invariant A.flag == true && B.flag == true}
    }"#;
    for arrivals in [false, true] {
        let src = if arrivals {
            src.replace("bounds{", "arrivals fixed[0min] bounds{")
        } else {
            src.into()
        };
        let r = explain::explain(&src);
        assert_eq!(r.report.verdict, crate::Verdict::Violated);
        assert_eq!(
            r.explanation.status,
            explain::Status::Complete,
            "{:#?}",
            r.explanation
        );
        let c = &r.explanation.counterexamples[0];
        let d = c
            .steps
            .last()
            .unwrap()
            .violation
            .as_ref()
            .unwrap()
            .state_invariant
            .as_ref()
            .unwrap();
        assert_eq!(
            d.values
                .iter()
                .map(|v| (v.name.as_str(), v.value.as_str()))
                .collect::<Vec<_>>(),
            [("A.flag", "false"), ("B.flag", "true")]
        );
        assert!(
            d.values
                .iter()
                .all(|v| v.visibility == explain::Visibility::Latent)
        );
    }
}

#[test]
fn state_invariant_post_end_contest_route_has_exact_finish_source_and_keeps_business_end_time() {
    let src = include_str!("../../../fixtures/ctx-contest/model.relca")
        .replace(
            "  people",
            "  observed case { flag: bool = true }\n  people",
        )
        .replace(
            "duration fixed 2min; result nondet { pass }",
            "duration fixed 2min; result nondet { pass }; writes flag; finish { flag = false }",
        )
        .replace(
            "  bounds",
            "  contract StateProbe { invariant flag }\n  bounds",
        );
    for arrivals in [false, true] {
        let src = if arrivals {
            src.replace("  bounds", "  arrivals fixed [0min]\n  bounds")
        } else {
            src.clone()
        };
        let m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
        let reference = reference::explore(&m);
        assert_eq!(reference.stop, reference::StopReason::Complete);
        let report = explain::explain(&src);
        assert_eq!(report.report.verdict, crate::Verdict::Violated);
        assert_eq!(report.explanation.status, explain::Status::Complete);
        let c = report
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id.ends_with(" invariant"))
            .unwrap();
        let last = c.steps.last().unwrap();
        assert_eq!((last.time, last.kind), (3, explain::Kind::ContestResponse));
        let d = last
            .violation
            .as_ref()
            .unwrap()
            .state_invariant
            .as_ref()
            .unwrap();
        assert_eq!(d.values[0].value, "false");
        assert!(
            c.related_locations
                .iter()
                .any(|l| &src[l.span.start..l.span.end] == "flag = false")
        );
        for kind in [
            explain::Kind::End,
            explain::Kind::ContestFiled,
            explain::Kind::ContestQueue,
            explain::Kind::Start,
        ] {
            assert!(c.steps.iter().any(|s| s.kind == kind));
        }
        let elapsed = if arrivals {
            report.report.case_outcomes.as_ref().unwrap()[0]
                .terminals
                .iter()
                .find(|t| t.end.as_deref() == Some("returned"))
                .unwrap()
                .elapsed
                .unwrap()
        } else {
            report
                .report
                .terminals
                .iter()
                .find(|t| t.end == "returned")
                .unwrap()
                .elapsed
        };
        assert_eq!((elapsed.min, elapsed.max), (1, 1));
    }
}

#[test]
fn state_invariant_finish_includes_kernel_overrides_and_the_selected_source_row() {
    let src = r#"model M version "0.2" {
        type T=enum{ok}
        observed case{hit:bool=false}
        latent case{truth:bool=true}
        kernel K given[truth]{
          true -> {1:ok with hit=true, truth=false}
          false -> {1:ok with hit=false, truth=false}
        }
        actor A{capacity 1;capability c()->T{duration fixed 1min;outcomes kernel K;kernel_reads latent[truth];writes[hit,truth]}}
        process P{x=A.c();end done}
        cut C{include[A]} bounds{cases 1;live_tasks 1;clock 3min}
        contract Controls{invariant hit -> truth == true}
    }"#;
    for arrivals in [false, true] {
        let src = if arrivals {
            src.replace("bounds{", "arrivals fixed[0min] bounds{")
        } else {
            src.into()
        };
        let report = explain::explain(&src);
        assert_eq!(report.report.verdict, crate::Verdict::Violated);
        assert_eq!(
            report.explanation.status,
            explain::Status::Complete,
            "{:#?}",
            report.explanation
        );
        let c = &report.explanation.counterexamples[0];
        let last = c.steps.last().unwrap();
        assert_eq!((last.time, last.kind), (1, explain::Kind::Finish));
        let d = last
            .violation
            .as_ref()
            .unwrap()
            .state_invariant
            .as_ref()
            .unwrap();
        assert_eq!(
            d.values
                .iter()
                .map(|v| (v.name.as_str(), v.value.as_str()))
                .collect::<Vec<_>>(),
            [("hit", "true"), ("truth", "false")]
        );
        for text in ["hit=true", "truth=false"] {
            let start = src.find(text).unwrap();
            assert!(c.related_locations.iter().any(|l| l.span
                == crate::diag::Span {
                    start,
                    end: start + text.len()
                }));
        }
    }
}

#[test]
fn state_invariant_read_identity_is_not_a_shared_declaration_span() {
    let src = r#"model M version "0.2" {
        type R=enum{done}
        observed case{a,b:bool=false}
        actor A{capacity 1;capability change()->R{duration fixed 1min;result nondet{done};writes[a,b];finish{a=true;b=false}}}
        process P{x=A.change();end done}
        cut C{include[A]} bounds{cases 1;live_tasks 1;clock 3min}
        contract Controls{invariant a -> b}
    }"#;
    let r = explain::explain(src);
    assert_eq!(r.report.verdict, crate::Verdict::Violated);
    assert_eq!(r.explanation.status, explain::Status::Complete);
    let c = &r.explanation.counterexamples[0];
    let d = c
        .steps
        .last()
        .unwrap()
        .violation
        .as_ref()
        .unwrap()
        .state_invariant
        .as_ref()
        .unwrap();
    assert_eq!(
        d.values
            .iter()
            .map(|v| (v.name.as_str(), v.value.as_str()))
            .collect::<Vec<_>>(),
        [("a", "true"), ("b", "false")]
    );
    assert_eq!(d.values[0].location, d.values[1].location);
    assert!(
        c.related_locations
            .iter()
            .any(|l| &src[l.span.start..l.span.end] == "a=true")
    );
    assert!(
        c.related_locations
            .iter()
            .any(|l| &src[l.span.start..l.span.end] == "b=false")
    );
}

#[test]
fn state_invariant_replay_rejects_property_span_writer_valuation_and_late_prefix() {
    for src in [
        include_str!("../../../fixtures/sinv-initial.broken-initial/model.relca"),
        include_str!("../../../fixtures/sinv-ended-clean.broken-increment/model.relca"),
    ] {
        let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
        let report = explain::explain(src);
        let c = report
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id == "contract Controls #1 invariant")
            .unwrap();
        assert_eq!(c.status, explain::ReplayStatus::Verified);
        let mut forged = c.steps.clone();
        forged
            .last_mut()
            .unwrap()
            .violation
            .as_mut()
            .unwrap()
            .property_location
            .as_mut()
            .unwrap()
            .span
            .start += 1;
        assert!(reference::replay::replay(&m, &forged, &c.id, c.case_id).is_err());
        let mut forged = c.steps.clone();
        let mut extra = forged.last().unwrap().clone();
        extra.index += 1;
        forged.push(extra);
        assert!(
            reference::replay::replay(&m, &forged, &c.id, c.case_id).is_err(),
            "witness must stop at first violation"
        );
        let expected = explain::state_invariant_locations(src, &m, &c.id, &c.steps, None).unwrap();
        assert_eq!(expected, c.related_locations);
        let mut forged_locations = c.related_locations.clone();
        forged_locations.last_mut().unwrap().span.start += 1;
        assert!(
            reference::replay::validate_state_invariant_locations(
                src,
                &m,
                &c.id,
                &c.steps,
                &forged_locations,
                None
            )
            .is_err()
        );
        if c.steps.last().unwrap().case_id != c.case_id {
            let mut forged = c.steps.clone();
            let last = forged.last_mut().unwrap();
            let d = last
                .violation
                .as_mut()
                .unwrap()
                .state_invariant
                .as_mut()
                .unwrap();
            // A valid writer event with the writer's pass observation is not
            // evidence about the ended target reader's fail valuation.
            for row in &mut d.values {
                if let Some(writer) = last
                    .values
                    .observed
                    .iter()
                    .chain(&last.values.latent)
                    .find(|v| v.name == row.name)
                {
                    row.value.clone_from(&writer.value);
                }
            }
            assert!(reference::replay::replay(&m, &forged, &c.id, c.case_id).is_err());
        }
    }
}

#[test]
fn state_invariant_public_ir_depth_and_excluded_local_reads_fail_closed_on_each_side() {
    let src = include_str!("../../../fixtures/sinv-cut/model.relca")
        .replace(
            "Invoice.difficulty == routine && registered -> approval == pass",
            "registered -> approval == pass",
        )
        .replace(
            "include [Approver, Ledger, Invoice]",
            "include [Approver, Ledger]; exclude Invoice reason \"outside\" loses \"local\"",
        );
    let base = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
    let (variable, v) = base
        .state_variables
        .iter()
        .enumerate()
        .find(|(_, v)| matches!(v.kind, ir::StateKind::ActorLatent(_)))
        .unwrap();
    let local = ir::Cond::StateEq(ir::StateMatch {
        variable: ir::StateVarId(variable),
        value: match v.initial {
            ir::InitialValue::Fixed(x) => x,
            _ => unreachable!(),
        },
    });
    for side in [false, true] {
        let mut m = base.clone();
        let ir::ContractKind::StateInvariant {
            antecedent,
            condition,
        } = &mut m.contracts[0].kind
        else {
            panic!()
        };
        if side {
            *antecedent = Some(local.clone());
        } else {
            *condition = local.clone();
        }
        assert!(
            engine::explore(&m)
                .diagnostics
                .iter()
                .any(|d| d.code == "U-SEM-0")
        );
        assert_eq!(
            reference::explore(&m).stop,
            reference::StopReason::SemanticAbort
        );
    }
    let mut m = lower::lower(
        &syntax::parse(include_str!("../../../fixtures/sinv-initial/model.relca")).unwrap(),
    )
    .unwrap();
    let ir::ContractKind::StateInvariant { condition, .. } = &m.contracts[0].kind else {
        panic!()
    };
    let leaf = condition.clone();
    let mut deep = leaf.clone();
    for _ in 0..128 {
        deep = ir::Cond::And(vec![leaf.clone(), deep]);
    }
    m.contracts[0].kind = ir::ContractKind::StateInvariant {
        antecedent: None,
        condition: deep,
    };
    assert!(
        engine::explore(&m)
            .diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0")
    );
    assert_eq!(
        reference::explore(&m).stop,
        reference::StopReason::EnumerationLimit
    );
}

#[test]
fn state_invariant_replay_rejects_a_legal_step_appended_after_an_initial_violation() {
    let src = include_str!("../../../fixtures/sinv-initial.broken-initial/model.relca");
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    let report = explain::explain(src);
    let c = report
        .explanation
        .counterexamples
        .iter()
        .find(|c| c.id == "contract Controls #1 invariant")
        .unwrap();
    assert_eq!(c.status, explain::ReplayStatus::Verified);
    assert_eq!(c.steps.len(), 1, "receipt itself is the first bad state");
    // A second clause after the first keeps every earlier byte offset, so its
    // witness supplies the legal queue stage that follows receipt in this model.
    let carrier = src.replacen(
        "invariant ok == true }",
        "invariant ok == true; invariant Fix.fix.finish -> ok == false }",
        1,
    );
    assert_ne!(src, carrier);
    let other = explain::explain(&carrier);
    let d = other
        .explanation
        .counterexamples
        .iter()
        .find(|c| c.id == "contract Controls #2 invariant")
        .unwrap();
    assert_eq!(d.status, explain::ReplayStatus::Verified);
    let queue = d.steps[1].clone();
    assert_eq!(queue.kind, explain::Kind::Queue);
    assert!(queue.violation.is_none());
    let mut first = c.steps[0].clone();
    let mut violation = first.violation.take().unwrap();
    violation.at_step = 1;
    violation.related_steps = vec![1];
    let mut moved = queue;
    moved.violation = Some(violation);
    let forged = vec![first, moved];
    assert!(
        reference::replay::replay(&m, &forged, &c.id, c.case_id).is_err(),
        "a violated receipt cannot be reported one legal step later"
    );
}

#[test]
fn state_invariant_replay_rejects_a_row_located_at_another_declaration_even_when_no_stage_lists_it()
{
    // With arrivals the receipt-time bad state is a global receive stage whose
    // values list is empty, so only the declaration span identifies the row.
    let src = r#"model M version "0.2" {
  type R = enum { ok }
  observed case { flag: bool = false; other: bool = false }
  actor Fix { capacity 1; capability fix() -> R { duration fixed 1min; result nondet { ok }; start { flag = true } } }
  process Flow { r = Fix.fix(); end done }
  cut C { include [Fix] }
  bounds { cases 1; live_tasks 2; clock 10min }
  contract Controls { invariant flag == true }
}
"#;
    for arrivals in [false, true] {
        let src = if arrivals {
            let t = src.replace("  bounds", "  arrivals fixed [0min]\n  bounds");
            assert_ne!(src, t);
            t
        } else {
            src.into()
        };
        let m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
        let r = explain::explain(&src);
        let c = &r.explanation.counterexamples[0];
        assert_eq!(c.status, explain::ReplayStatus::Verified);
        reference::replay::replay(&m, &c.steps, &c.id, c.case_id).unwrap();
        let start = src.find("other: bool = false").unwrap();
        let other = explain::Location::new(
            &src,
            crate::diag::Span {
                start,
                end: start + "other: bool = false".len(),
            },
        )
        .unwrap();
        let mut forged = c.steps.clone();
        let row = &mut forged
            .last_mut()
            .unwrap()
            .violation
            .as_mut()
            .unwrap()
            .state_invariant
            .as_mut()
            .unwrap()
            .values[0];
        assert_eq!((row.name.as_str(), row.value.as_str()), ("flag", "false"));
        assert_ne!(row.location, other);
        row.location = other;
        assert!(
            reference::replay::replay(&m, &forged, &c.id, c.case_id).is_err(),
            "the flag row must keep flag's declaration (arrivals {arrivals})"
        );
    }
}
