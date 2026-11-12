//! Implementation-side guards complementary to the immutable acceptance suite.
use crate::{Verdict, engine, ir, lower, reference, report, syntax};

fn model(s: &str) -> ir::Model {
    lower::lower(&syntax::parse(s).unwrap()).unwrap()
}
fn refused(m: &ir::Model) {
    let e = engine::explore(m);
    let r = reference::explore(m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    assert!(e.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
    assert!(e.terminals.is_empty());
}

#[test]
fn sentinel_has_a_typed_position_and_cannot_be_an_enum_binding() {
    let mut m = model(include_str!("../../../fixtures/aset-dup/model.relca"));
    let ir::Stmt::Call(c) = &mut m.process.body[0].kind else {
        panic!()
    };
    c.args[0] = ir::BindingId(0);
    refused(&m);
    let mut m = model(
        "model M version \"0.2\" {type T=enum{ok};actor A{capacity 1;capability a()->T{duration fixed 1min;result nondet{ok}};capability b(x:T)->T{duration fixed 1min;result nondet{ok}}};process P{a=A.a();b=A.b(a);end done};bounds{cases 1;live_tasks 1;clock 5min};verify reachable end.done}",
    );
    let ir::Stmt::Call(c) = &mut m.process.body[1].kind else {
        panic!()
    };
    c.args[0] = ir::CURRENT_CASE;
    refused(&m);
}

#[test]
fn case_paths_and_case_set_type_keep_token_whitespace_rules() {
    let s = include_str!("../../../fixtures/aset-dup/model.relca")
        .replace("set<CaseId>", "set<\nCaseId\n>")
        .replace("post(case.id)", "post(case\n.\nid)");
    assert_eq!(report::check(&s).verdict, Verdict::Satisfied);
    let s = include_str!("../../../fixtures/aset-dup/model.relca").replace("registered", "atomic");
    assert_eq!(report::check(&s).verdict, Verdict::Satisfied);
}

#[test]
fn actor_named_case_keeps_ordinary_qualified_state_access() {
    let s = "model M version \"0.2\" {type T=enum{ok};actor case{capacity 1;state flag:bool=true;capability go()->T{duration fixed 1min;result nondet{ok}}};process P{a=case.go();if case.flag==true{end done}else{end stopped}};bounds{cases 1;live_tasks 1;clock 3min};verify reachable end.done}";
    let m = model(s);
    assert_eq!(report::check(s).verdict, Verdict::Satisfied);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.terminals, r.outcome.terminals);
}

#[test]
fn binding_responsibilities_never_disappear_in_unselected_processes() {
    for selected in [false, true] {
        for formula in [false, true] {
            let p = if selected {
                "approved=A.check();end done"
            } else {
                "end done"
            };
            let q = if selected {
                "end done"
            } else {
                "approved=A.check();end done"
            };
            let duty = if formula {
                "approved == yes -> flag == true"
            } else {
                "approved == yes"
            };
            let s = format!(
                "model Duty version \"0.2\" {{type T=enum{{yes,no}};people[p];observed case{{flag:bool=true}};actor A{{executors[p];capability check()->T{{duration fixed 1min;result nondet{{yes}}}}}};process P{{{p}}};process Q{{{q}}};cut K{{include[A]}};configuration Active{{cut K;process P}};configuration Hidden{{cut K;process Q}};responsible p for ({duty});bounds{{cases 1;live_tasks 1;clock 3min}};verify safety no_resource_overcommit}}"
            );
            let ds = lower::lower_configuration(&syntax::parse(&s).unwrap(), "Active")
                .err()
                .unwrap();
            let start = s.rfind("approved == yes").unwrap();
            assert_eq!(
                ds,
                vec![crate::diag::Diagnostic {
                    code: "U-SEM-0".into(),
                    severity: crate::diag::Severity::Error,
                    message: "probability conditions require state".into(),
                    span: Some(crate::diag::Span {
                        start,
                        end: start + "approved".len()
                    })
                }]
            );
        }
    }
}

#[test]
fn unknown_executor_members_and_atomic_words_remain_unsupported_not_malformed() {
    for body in [
        "start{flag=executor.name}",
        "writes flag;finish{flag=executor.name}",
        "atomic{foo}",
    ] {
        let s = format!(
            "model M version \"0.2\" {{type T=enum{{ok}};people[p];observed case{{flag:bool=false}};actor A{{executors[p];capability go()->T{{duration fixed 1min;result nondet{{ok}};{body}}}}};process P{{end done}};bounds{{cases 1;live_tasks 1;clock 3min}};verify safety no_resource_overcommit}}"
        );
        let ds = syntax::parse(&s).unwrap_err();
        assert!(ds.iter().all(|d| d.code == "U-SYN-0"), "{ds:?}");
    }
    let s = "model M version \"0.2\" {type T=enum{ok,executor};observed case{x,y:T=ok};actor A{capacity 1;capability go()->T{writes[x,y];finish{x=executor\ny=ok};duration fixed 1min;result nondet{ok}}};process P{a=A.go();end done};bounds{cases 1;live_tasks 1;clock 3min};verify safety no_resource_overcommit}";
    assert_eq!(report::check(s).verdict, Verdict::Satisfied);
}

#[test]
fn simultaneous_same_capability_calls_keep_their_own_case_in_event_invariants() {
    let s = "model M version \"0.2\" {type T=enum{ok,no};actor Ledger{capacity 1;state keys:set<CaseId>;state gate:bool=false;capability post(k:CaseId)->T{commits;writes[keys,gate];atomic{if gate==false{keys+=k;gate=true;result ok}else{result no}};duration fixed 1min}};process P{a=Ledger.post(case.id);end done};arrivals fixed[0min,0min];policy fifo tie_by case_id;bounds{cases 2;live_tasks 1;clock 5min};contract C{invariant Ledger.post.finish -> case.id in Ledger.keys};verify safety no_resource_overcommit}";
    let m = model(s);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.properties, r.outcome.properties);
    let p = e
        .properties
        .iter()
        .find(|p| p.property == ir::Property::Contract(ir::ClauseId(0)))
        .unwrap();
    let cases = p.cases.as_ref().unwrap();
    assert_eq!(
        cases.iter().map(|p| p.verdict).collect::<Vec<_>>(),
        [Verdict::Satisfied, Verdict::Violated]
    );
    assert_eq!(
        cases.iter().map(|p| p.case_id).collect::<Vec<_>>(),
        [ir::CaseId(0), ir::CaseId(1)]
    );
}

#[test]
fn public_atomic_fork_footprints_cannot_be_forged_to_overlap() {
    let mut m = model(include_str!(
        "../../../fixtures/aset-static-fork-target/model.relca"
    ));
    let first = m.capabilities[0].atomic.as_ref().unwrap().writes[0].variable;
    m.capabilities[1].atomic.as_mut().unwrap().writes[0].variable = first;
    refused(&m);
    let s = "model M version \"0.2\" {type T=enum{ok};people[p,q];observed case{who,other:optional<PersonId>};actor A{executors[p,q];capability a()->T{writes who;finish{who=executor.id};duration fixed 1min;result nondet{ok}};capability b()->T{writes other;finish{other=executor.id};duration fixed 1min;result nondet{ok}}};process P{fork f{a=A.a();b=A.b()};join all f;end done};bounds{cases 1;live_tasks 2;clock 5min};verify reachable end.done}";
    let mut m = model(s);
    m.capabilities[1].writes[0].variable = m.capabilities[0].writes[0].variable;
    refused(&m);
}

#[test]
fn sets_cannot_be_smuggled_through_scalar_predicates_or_constant_writes() {
    let good = model(include_str!("../../../fixtures/aset-dup/model.relca"));
    let set = ir::StateVarId(
        good.state_variables
            .iter()
            .position(|v| v.case_set_size.is_some())
            .unwrap(),
    );
    for kind in 0..4 {
        let mut m = good.clone();
        match kind {
            0 => {
                let ir::Stmt::If { condition, .. } = &mut m.process.body[2].kind else {
                    panic!()
                };
                *condition = ir::Cond::StateEq(ir::StateMatch {
                    variable: set,
                    value: ir::StateValue::Set(0),
                });
            }
            1 => {
                m.capabilities[0].atomic.as_mut().unwrap().condition =
                    ir::Cond::StateEq(ir::StateMatch {
                        variable: set,
                        value: ir::StateValue::Set(0),
                    });
            }
            2 => {
                m.capabilities[0].atomic.as_mut().unwrap().writes[0].source =
                    ir::WriteSource::Constant(ir::StateValue::Set(0));
            }
            3 => {
                m.capabilities[0].start_writes.push(ir::StateWrite {
                    variable: set,
                    source: ir::WriteSource::Constant(ir::StateValue::Set(0)),
                });
            }
            _ => unreachable!(),
        }
        refused(&m);
    }
}

#[test]
fn atomic_declaration_duplicates_and_kernel_metadata_are_not_silently_dropped() {
    let s = include_str!("../../../fixtures/aset-dup/model.relca");
    for changed in [
        s.replace(
            "writes [keys, registered]",
            "writes [keys, keys, registered]",
        ),
        s.replace(
            "writes [keys, registered]",
            "writes [keys, registered]; writes keys",
        ),
        s.replace("commits", "commits;kernel_reads latent []"),
    ] {
        let ds = syntax::parse(&changed)
            .and_then(|a| lower::lower(&a))
            .unwrap_err();
        assert!(ds.iter().any(|d| d.code == "U-SEM-0"), "{ds:?}");
    }
}

#[test]
fn case_person_kernel_inputs_and_overrides_are_rejected_by_both_ir_guards() {
    let s = "model M version \"0.2\" {type T=enum{ok};people[p];observed case{who:optional<PersonId>;flag:bool=false};kernel K given[flag]{false->{1:ok};true->{1:ok}};actor A{capacity 1;capability a()->T{duration fixed 1min;outcomes kernel K}};process P{a=A.a();end done};bounds{cases 1;live_tasks 1;clock 5min};verify reachable end.done}";
    let good = model(s);
    for inputs in [true, false] {
        let mut m = good.clone();
        let who = ir::StateVarId(
            m.state_variables
                .iter()
                .position(|v| v.name == "who")
                .unwrap(),
        );
        let mut k = m.kernels[0].clone();
        if inputs {
            k.given = vec![who];
            k.rows[0].values = vec![ir::StateValue::None];
            k.rows[1].values = vec![ir::StateValue::Value(ir::ValueId(0))];
        } else {
            for row in &mut k.rows {
                row.branches[0].overrides = vec![(who, ir::StateValue::Value(ir::ValueId(0)))];
            }
            m.capabilities[0].writes.push(ir::StateWrite {
                variable: who,
                source: ir::WriteSource::Constant(ir::StateValue::None),
            });
        }
        m.kernels[0] = k.clone();
        m.capabilities[0].kernel = Some(k);
        refused(&m);
    }
}

#[test]
fn case_id_only_argument_is_not_a_judgement_input() {
    let s = "model M version \"0.2\" {type T=enum{ok,no};actor Doc{source record};actor A{capacity 1;capability check(k:CaseId)->T{reads[Doc];duration fixed 1min;result nondet{ok,no}};capability post()->T{commits;duration fixed 1min;result nondet{ok}}};process P{a=A.check(case.id);if a==ok{b=A.post();end done}else{end no}};bounds{cases 1;live_tasks 1;clock 5min};verify safety no_resource_overcommit}";
    let m = model(s);
    assert!(m.agency.lineage.is_empty());
    assert!(m.agency.refusal.iter().all(|r| r.judgements.is_empty()));
}

#[test]
fn membership_requires_is_unknown_not_an_initial_empty_set_constant() {
    let m = model(include_str!("../../../fixtures/aset-agency/model.relca"));
    assert!(m.agency.refusal.iter().all(|r| r.effect != "Poster.post"));
    let control = model(include_str!(
        "../../../fixtures/aset-agency.broken-scalar-requires/model.relca"
    ));
    assert!(
        control
            .agency
            .refusal
            .iter()
            .find(|r| r.effect == "Poster.post")
            .unwrap()
            .judgements
            .iter()
            .filter_map(crate::agency::Judgement::process)
            .any(|j| j.stops == crate::agency::Stops::AllPaths)
    );
}

#[test]
fn start_person_writer_keeps_sources_but_cannot_certify_refusal() {
    let s = "model M version \"0.2\" {type T=enum{ok};people[c1,c2];observed case{who:optional<PersonId>};actor Doc{source record};actor A{executors[c1,c2];capability set()->T{reads[Doc];start{who=executor.id};duration fixed 1min;result nondet{ok}}};process P{a=A.set();end done};bounds{cases 1;live_tasks 1;clock 5min};responsible c1 for(who==c1);verify safety no_resource_overcommit}";
    let r = report::check(s);
    assert_eq!(r.verdict, Verdict::Satisfied);
    let v = serde_json::to_value(r).unwrap();
    assert_eq!(
        v["responsibilities"][0]["observation"]["required_sources"],
        serde_json::json!(["Doc"])
    );
    assert_eq!(v["responsibilities"][0]["refusal"]["reason"], "state_guard");
}

#[test]
fn forged_probability_membership_is_outside_the_admitted_ir_domain() {
    let mut m = model(include_str!("../../../fixtures/aset-dup/model.relca"));
    let set = ir::StateVarId(
        m.state_variables
            .iter()
            .position(|v| v.case_set_size.is_some())
            .unwrap(),
    );
    m.probability_queries.push(ir::ProbabilityQuery {
        condition: ir::ProbabilityCondition::State(ir::Cond::Contains {
            set,
            element: ir::Element::CurrentCase,
        }),
        within: None,
        comparison: ir::Comparison::Le,
        bound: crate::probability::Rational::one(),
    });
    m.verify.push(ir::Verify {
        property: ir::Property::ProbabilityReached(0),
        span: m.process.span,
    });
    refused(&m);
}

#[test]
fn explicit_null_notes_cannot_be_laundered_into_an_absent_key() {
    let mut v = serde_json::to_value(report::check(include_str!(
        "../../../fixtures/aset-dup/model.relca"
    )))
    .unwrap();
    v["properties"][0]["notes"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<report::Report>(v).is_err());
}

#[test]
fn concurrent_atomic_writers_and_any_results_remain_unknown_for_later_refusal() {
    let s = "model M version \"0.2\" {type T=enum{ok,no};observed case{d:T=ok};actor A{capacity 2;capability choose()->T{writes d;atomic{if d==ok{d=no;result ok}else{result no}};duration fixed 1min;cancel forbidden_after_start};capability post()->T{requires d==ok else no;commits;duration fixed 1min;result nondet{ok};cancel forbidden_after_start}};process P{fork f{a=A.choose();b=A.post()};join all f;end done};bounds{cases 1;live_tasks 2;clock 5min};verify safety no_resource_overcommit}";
    let m = model(s);
    assert!(m.agency.refusal.iter().all(|r| r.call != "b"));
    let s = "model M version \"0.2\" {type T=enum{ok,no};observed case{d:T=ok};actor A{capacity 2;capability choose()->T{writes d;atomic{if d==ok{d=no;result ok}else{result no}};duration fixed 1min;cancel forbidden_after_start};capability fast()->T{duration fixed 1min;result nondet{ok};cancel forbidden_after_start};capability post()->T{commits;duration fixed 1min;result nondet{ok}}};process P{fork f{a=A.choose();b=A.fast()};join any f where result==ok tie by[b,a] remaining drain if none -> no_match;c=A.post();end done};bounds{cases 1;live_tasks 2;clock 5min};verify safety no_resource_overcommit}";
    let s = s.replace(
        "c=A.post();end done",
        "c=A.post();if d==ok{end done}else{end no_match}",
    );
    let m = model(&s);
    assert!(m.agency.refusal.iter().all(|r| r.call != "c"));
}
