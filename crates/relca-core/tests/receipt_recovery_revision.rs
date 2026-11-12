//! Review reproductions. These expectations precede the round-one fixes.
#[path = "support/r2_inputs.rs"]
mod i;
use relca_core::{Verdict, diag, engine, ir, lower, mutation, reference, report, syntax};
use serde_json::json;

fn lower_reject(source: &str, code: &str, reason: &str) {
    let ast = syntax::parse(source).expect("reproduction must be valid syntax");
    let result = lower::lower(&ast);
    assert!(result.is_err(), "lower accepted the review reproduction");
    let ds = result.unwrap_err();
    assert!(
        ds.iter().any(|d| d.code == code && d.message == reason),
        "{ds:?}"
    );
}
fn before_pay(source: &str, text: &str) -> String {
    i::edit(
        source,
        "process Flow { pay",
        &format!("process Flow {{ {text}\npay"),
    )
}
#[test]
fn a_blocker1_bare_g_in_handler() {
    lower_reject(
        &i::body(
            Some("again = Psp.charge_keyed(case.id)\nstop receipt_unknown"),
            i::LOSS,
        ),
        "E-CON-0",
        "invalid_recovery_contract",
    );
}
#[test]
fn a_blocker1_bare_g_in_body() {
    lower_reject(
        &before_pay(&i::source("R01"), "again = Psp.charge_keyed(case.id)"),
        "E-CON-0",
        "invalid_recovery_contract",
    );
}
#[test]
fn a_blocker2_atomic_condition() {
    let s = i::edit(
        &i::source("R01"),
        "capability query_status",
        "state probes: 0..2 = 0; capability probe(key: CaseId) -> Pay { duration fixed 1min; writes probes; atomic { if key in keys { result rejected_duplicate } else { probes += 1; result charged } } }\ncapability query_status",
    );
    lower_reject(
        &before_pay(&s, "p = Psp.probe(case.id)"),
        "E-TYPE-0",
        "recovery_provider_state_leak",
    );
}
#[test]
fn a_blocker2_wait_until() {
    lower_reject(
        &before_pay(&i::source("R01"), "wait until case.id in Psp.keys"),
        "E-TYPE-0",
        "recovery_provider_state_leak",
    );
}
#[test]
fn a_major1_previously_received_observation_is_allowed_by_m2() {
    let s = i::edit(
        &i::source("R01"),
        "actor Shop {",
        "actor Source { capacity 1; capability observe() -> PaymentStatus {duration fixed 1min; result nondet {still_unknown}} }\nactor Shop {",
    );
    let s = before_pay(&s, "order = Source.observe()");
    let s = i::edit(
        &s,
        "include [Psp,CS,Finance,Shop]",
        "include [Psp,CS,Finance,Shop,Source]",
    );
    let s = i::edit(
        &s,
        "decision = CS.decide(status)",
        "decision = CS.decide(order)",
    );
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    let e = engine::explore(&m);
    assert!(
        !e.diagnostics
            .iter()
            .any(|d| d.severity == diag::Severity::Error),
        "{:?}",
        e.diagnostics
    );
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.terminals[0].elapsed.min, 8);
    assert_eq!(r.outcome.terminals[0].elapsed.min, 8);
}
#[test]
fn a_major1_bare_query_handler() {
    lower_reject(
        &i::body(
            Some("status = Psp.query_status(case.id)\nstop receipt_unknown"),
            i::LOSS,
        ),
        "E-TYPE-0",
        "recovery_evidence_mismatch",
    );
}
#[test]
fn a_major1_bare_query_body() {
    lower_reject(
        &before_pay(&i::source("R01"), "other = Psp.query_status(case.id)"),
        "E-TYPE-0",
        "recovery_evidence_mismatch",
    );
}
#[test]
fn a_major1_query_declaration_without_handler() {
    let s = i::edit(
        &i::source("R03"),
        "result nondet {still_unknown}",
        "receipt_query charge_keyed key key availability available",
    );
    lower_reject(&s, "U-SEM-0", "recovery_query_unmodeled");
}
#[test]
fn a_major1_query_command_in_body() {
    lower_reject(
        &before_pay(
            &i::source("R01"),
            "other = Psp.query_status(case.id) for command pay",
        ),
        "U-SEM-0",
        "recovery_feature_combination",
    );
}
#[test]
fn a_major1_query_command_in_fork() {
    lower_reject(
        &before_pay(
            &i::source("R01"),
            "fork xs { other = Psp.query_status(case.id) for command pay }; join all xs",
        ),
        "U-SEM-0",
        "recovery_feature_combination",
    );
}
#[test]
fn a_major1_recovery_in_fork() {
    let s = i::edit(
        &i::source("R03"),
        "pay = Psp.charge_keyed(case.id) response_loss sequence [unknown]",
        "fork xs { pay = Psp.charge_keyed(case.id) response_loss none on receipt_unknown {stop receipt_unknown} }; join all xs",
    );
    lower_reject(&s, "U-SEM-0", "recovery_feature_combination");
}
#[test]
fn a_major1_query_suffix_is_not_silently_dropped() {
    let s = before_pay(
        &i::source("R01"),
        "other = Psp.query_status(case.id) response_loss none for command pay",
    );
    lower_reject(&s, "U-SEM-0", "recovery_feature_combination");
}
#[test]
fn a_major1_nonempty_initializer_is_already_statically_closed() {
    let s = i::edit(
        &i::source("R01"),
        "state keys: set<CaseId>",
        "state keys: set<CaseId> = none",
    );
    let ds = syntax::parse(&s).unwrap_err();
    assert_eq!(ds[0].code, "U-SYN-0");
    assert_eq!(
        ds[0].message,
        "unsupported syntax: unsupported set initializer"
    );
}
#[test]
fn b_major1_mutation_is_unsupported() {
    let r = mutation::mutate(&i::source("R04"));
    assert_eq!(r.verdict, Verdict::Unsupported);
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0" && d.message == "recovery_feature_combination"),
        "{:?}",
        r.diagnostics
    );
}
#[test]
fn b_major3_simulation_analysis_is_sampled() {
    let v = json!(report::simulate(&i::source("R01")));
    assert_eq!(v["response_analysis"]["coverage"], "sampled");
    assert!(
        v["response_analysis"]["checked"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| !v.as_str().unwrap().starts_with("exact_"))
    );
}
#[test]
fn b_major3_simulation_decoder_validates_analysis() {
    let v = json!(report::simulate(&i::source("R01")));
    for field in ["rules", "checked"] {
        let mut bad = v.clone();
        bad["response_analysis"][field] = if field == "rules" {
            json!("unrecognized")
        } else {
            json!(["invented"])
        };
        assert!(
            serde_json::from_value::<report::SimulationReport>(bad).is_err(),
            "{field}"
        );
    }
    let mut bad = v;
    bad.as_object_mut().unwrap().remove("response_analysis");
    assert!(serde_json::from_value::<report::SimulationReport>(bad).is_err());
}
#[test]
fn check_decoder_requires_receipt_and_analysis_with_recovery() {
    let mut v = json!(report::check(&i::source("R01")));
    v.as_object_mut().unwrap().remove("response_receipts");
    v.as_object_mut().unwrap().remove("response_analysis");
    assert!(serde_json::from_value::<report::Report>(v).is_err());
}
#[test]
fn unreachable_handler_suffix_is_rejected() {
    lower_reject(
        &i::body(Some("stop receipt_unknown\nwait 1min"), i::LOSS),
        "E-SYN-0",
        "invalid_recovery_syntax",
    );
}
#[test]
fn unreachable_end_suffix_is_rejected() {
    let s = i::edit(
        &i::source("R01"),
        "end shipped\n   }",
        "end shipped\nwait 1min\n   }",
    );
    lower_reject(&s, "E-SYN-0", "invalid_recovery_syntax");
}
#[test]
fn malformed_handler_keeps_the_original_error_token() {
    let s = i::body(Some("stop receipt_unknown\nwait }"), i::LOSS);
    let ds = syntax::parse(&s).unwrap_err();
    let start = s.find("wait }").unwrap() + 5;
    assert_eq!(ds[0].code, "E-SYN-0");
    assert_eq!(ds[0].message, "invalid_recovery_syntax");
    assert_eq!(
        ds[0].span,
        Some(diag::Span {
            start,
            end: start + 1
        })
    );
}
#[test]
fn dotted_recovery_rhs_is_not_an_extension_to_the_grammar() {
    let s = i::body(
        Some("if Psp.keys == Psp.keys {stop receipt_unknown} else {stop receipt_unknown}"),
        i::LOSS,
    );
    assert!(syntax::parse(&s).is_err());
    let s = i::body(
        Some("if case.id in Psp.keys {stop receipt_unknown} else {stop receipt_unknown}"),
        i::LOSS,
    );
    lower_reject(&s, "E-TYPE-0", "recovery_provider_state_leak");
}
#[test]
fn old_for_extension_retains_diagnostic_and_position() {
    let s = i::edit(
        &i::old_binding("on"),
        "on = A.run();",
        "on = A.run()\nfor item extension;",
    );
    let ds = syntax::parse(&s).unwrap_err();
    assert_eq!(ds[0].code, "U-SYN-0");
    assert_eq!(ds[0].message, "unsupported syntax: for");
    let start = s.find("for item").unwrap();
    assert_eq!(
        ds[0].span,
        Some(diag::Span {
            start,
            end: start + 3
        })
    );
}

// Build structurally valid public IR without going through the DSL boundary.
fn corrupt_g(location: &str) -> ir::Model {
    let mut m = lower::lower(&syntax::parse(&i::source("R01")).unwrap()).unwrap();
    let (_, r) = m.response_attempts()[0];
    let mut call = r.command_call.clone();
    let mut binding = m.process.bindings[call.binding.0].clone();
    binding.name = "second_command".into();
    call.binding = ir::BindingId(m.process.bindings.len());
    m.process.bindings.push(binding);
    let stmt = ir::Statement {
        span: call.span,
        kind: ir::Stmt::Call(call.clone()),
    };
    match location {
        "body" => m.process.body.insert(0, stmt),
        "fork" => {
            m.process.forks.push("extra".into());
            m.process.body.insert(
                0,
                ir::Statement {
                    span: call.span,
                    kind: ir::Stmt::Fork {
                        id: ir::ForkId(0),
                        branches: vec![call],
                        join: ir::Join::All,
                    },
                },
            )
        }
        "handler" => {
            if let ir::Stmt::Response(_, r) = &mut m.process.body[0].kind {
                r.recovery.as_mut().unwrap().insert(0, stmt)
            } else {
                panic!("response")
            }
        }
        _ => panic!("location"),
    }
    m
}
macro_rules! ir_g_test {
    ($name:ident,$location:literal,$system:ident) => {#[test]fn $name(){
        let m=corrupt_g($location);
        let result=$system::explore(&m);
        ir_g_test!(@check result,$system);
    }};
    (@check $r:ident,engine)=>{assert!($r.diagnostics.iter().any(|d|d.message=="invalid_recovery_ir"),"{:?}",$r.diagnostics)};
    (@check $r:ident,reference)=>{assert_eq!($r.stop,reference::StopReason::SemanticAbort)};
}
ir_g_test!(b_major2_reference_g_body, "body", reference);
ir_g_test!(b_major2_reference_g_fork, "fork", reference);
ir_g_test!(a_blocker1_engine_g_handler, "handler", engine);
ir_g_test!(b_major2_engine_g_fork, "fork", engine);
ir_g_test!(
    a_blocker1_reference_g_handler_already_closed,
    "handler",
    reference
);

fn private_ir(kind: &str) -> ir::Model {
    let s = i::edit(
        &i::source("R01"),
        "capability query_status",
        "state probes: 0..2 = 0; capability probe(key: CaseId) -> Pay { duration fixed 1min; writes probes; atomic { if key in keys { result rejected_duplicate } else { probes += 1; result charged } } }\ncapability query_status",
    );
    let mut m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    // Lexical fee inventory is not the boundary exercised by this corruption.
    m.metrics = None;
    let (_, r) = m.response_attempts()[0];
    let mut call = r.command_call.clone();
    let ir::Cond::Contains { set, .. } = m.capabilities[call.capability.0]
        .atomic
        .as_ref()
        .unwrap()
        .condition
    else {
        panic!("G")
    };
    let condition = ir::Cond::Contains {
        set,
        element: ir::Element::CurrentCase,
    };
    let stmt = if kind == "wait" {
        ir::Statement {
            span: call.span,
            kind: ir::Stmt::WaitUntil(condition),
        }
    } else {
        call.capability = ir::CapabilityId(
            m.capabilities
                .iter()
                .position(|c| c.name == "probe")
                .unwrap(),
        );
        if kind == "version" {
            let cap = &mut m.capabilities[call.capability.0];
            cap.atomic.as_mut().unwrap().condition = ir::Cond::IntCompare {
                variable: cap.atomic.as_ref().unwrap().writes[0].variable,
                operator: relca_core::ast::IntComparison::Eq,
                value: 0,
            };
            cap.version_check = Some(ir::VersionCheck {
                resolve: None,
                variables: vec![set],
                reject: ir::ValueId(1),
                span: call.span,
            });
        }
        let mut binding = m.process.bindings[call.binding.0].clone();
        binding.name = "probe".into();
        call.binding = ir::BindingId(m.process.bindings.len());
        m.process.bindings.push(binding);
        ir::Statement {
            span: call.span,
            kind: ir::Stmt::Call(call),
        }
    };
    m.process.body.insert(0, stmt);
    m
}
macro_rules! private_test {
    ($name:ident,$kind:literal,$system:ident)=>{#[test]fn $name(){let m=private_ir($kind);let result=$system::explore(&m);ir_g_test!(@check result,$system);}};
}
private_test!(a_blocker2_engine_atomic_reader, "atomic", engine);
private_test!(a_blocker2_reference_atomic_reader, "atomic", reference);
private_test!(a_blocker2_engine_wait_reader, "wait", engine);
private_test!(a_blocker2_reference_wait_reader, "wait", reference);

#[test]
fn stop_reasons_cannot_disagree_with_case_completion() {
    let v = json!(report::check(&i::source("R01")));
    for reason in ["state_range", "semantic_abort", "deadlock"] {
        let mut recovery = v["response_recovery"].clone();
        recovery["paths"][0]["commands"][0]["stop_reason"] = json!(reason);
        let r: relca_core::recovery::Recovery = serde_json::from_value(recovery).unwrap();
        assert!(r.validate_host(&v).is_err(), "{reason}");
    }
}
#[test]
fn pre_receipt_cut_is_pending_not_resolved_in_both_interpreters() {
    let s = i::edit(
        &i::source("R01"),
        "writes [keys,charged_truth,charges]; duration fixed 1min",
        "writes [keys,charged_truth,charges]; duration fixed 5min",
    );
    let s = i::edit(&s, "clock 40min", "clock 2min");
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    for o in [engine::explore(&m), reference::explore(&m).outcome] {
        let recovery = o.response_recovery.unwrap();
        let c = &recovery.paths[0].commands[0];
        assert!(c.unresolved);
        assert!(c.queries.is_empty() && c.resolutions.is_empty());
    }
    let v = json!(report::check(&s));
    serde_json::from_value::<report::Report>(v.clone()).unwrap();
    let mut recovery = v["response_recovery"].clone();
    recovery["paths"][0]["commands"][0]["unresolved"] = json!(false);
    let r: relca_core::recovery::Recovery = serde_json::from_value(recovery).unwrap();
    assert!(r.validate_host(&v).is_err());
}
