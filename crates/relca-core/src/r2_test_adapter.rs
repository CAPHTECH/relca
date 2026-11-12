//! Test-only observation projection. No acceptance expectation tables are read.
use crate::{ir, probability::Rational};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};
thread_local! {static ACTIVE:Cell<bool>=const{Cell::new(false)};static RECORDS:RefCell<Vec<Value>>=const{RefCell::new(Vec::new())};}
pub(crate) fn active() -> bool {
    ACTIVE.get()
}
pub(crate) fn record(value: Value) {
    if active() {
        RECORDS.with_borrow_mut(|v| v.push(value));
    }
}
fn observations(system: &str, source: &str, scenario: Option<&str>) -> Vec<Value> {
    let m =
        crate::lower::lower_with_selection(&crate::syntax::parse(source).unwrap(), None, scenario)
            .unwrap();
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ACTIVE.set(false);
        }
    }
    assert!(!ACTIVE.replace(true));
    let _reset = Reset;
    RECORDS.with_borrow_mut(Vec::clear);
    match system {
        "engine" => {
            crate::engine::explore(&m);
        }
        "reference" => {
            crate::reference::explore(&m);
        }
        _ => panic!("unknown system"),
    }
    RECORDS.with_borrow_mut(std::mem::take)
}
pub fn project(system: &str, scenario: &str) -> Value {
    match system {
        "engine" => crate::engine::r2_project(scenario),
        "reference" => crate::reference::r2_project(scenario),
        _ => panic!("system"),
    }
}
pub fn resolve(system: &str, fault: &str) -> Value {
    match system {
        "engine" => crate::engine::r2_resolve(fault),
        "reference" => crate::reference::r2_resolve(fault),
        _ => panic!("system"),
    }
}
pub fn replay(fault: &str) -> Value {
    crate::reference::r2_replay(fault)
}
pub fn cut(system: &str, scenario: &str) -> Value {
    let id = match scenario {
        "clock2" => "T04",
        "clock4" => "T06",
        "horizon2" => "T05",
        _ => panic!("scenario"),
    };
    let values = observations(
        system,
        &crate::r2_inputs::source(id),
        (id == "T05").then_some("Day"),
    );
    let r = values.first().expect("physical stop observation");
    json!({"at":r["at"],"observed_work":r["work"],"charge_commits":r["charge_commits"],"query_evidence":r["query_evidence"],"decision_recorded":r["decision_recorded"],"resolved":r["resolved"],"end":r["end_at"],"stop":r["stop"]})
}
pub fn history(system: &str, scenario: &str) -> Value {
    let id = scenario.strip_suffix("_person_query").unwrap_or(scenario);
    let mut s = crate::r2_inputs::source(id);
    if id != scenario {
        s = crate::r2_inputs::edit(
            &s,
            "capacity 1; control policy; state keys",
            "executors [q]; capacity 1; control policy; state keys",
        );
    }
    let rows = observations(system, &s, None);
    let r = rows.first().unwrap();
    if id == "R08" {
        return json!({"attempts":r["attempts"],"handler_entries":r["entries"],"arrivals":r["arrivals"],"charge_commits":r["charge_commits"],"end_at":r["end_at"],"work":r["work"]});
    }
    json!({"charge_commits":r["charge_commits"],"refund_commits":r["refund_commits"],"total_commits":r["total_commits"],"end_at":r["end_at"],"work":r["work"],"people":r["people"],"key_present":r["key_present"],"original_receipt":r["original_receipt"],"pay_visible":r["pay_visible"]})
}
pub fn probability(system: &str, available: bool) -> Value {
    let rows = observations(
        system,
        &crate::r2_inputs::probability(available, "0.1"),
        None,
    );
    let mut paths = BTreeMap::new();
    for r in rows {
        let name = r["path"].as_str().unwrap().to_owned();
        if let Some(old) = paths.insert(name, r.clone()) {
            assert_eq!(
                old, r,
                "schedule copies must have identical observations and chance mass"
            );
        }
    }
    let (mut work, mut tasks, mut attempts, mut query, mut decision, mut complete, mut unknown) = (
        Rational::zero(),
        Rational::zero(),
        Rational::zero(),
        Rational::zero(),
        Rational::zero(),
        Rational::zero(),
        Rational::zero(),
    );
    let mut projected = Vec::new();
    for name in ["D", "UD", "UU"] {
        let r = &paths[name];
        let p: Rational = serde_json::from_value(r["mass"].clone()).unwrap();
        for (sum, key) in [
            (&mut work, "work"),
            (&mut tasks, "tasks"),
            (&mut attempts, "attempts"),
            (&mut query, "query_work"),
            (&mut decision, "decision_work"),
        ] {
            sum.0 += &p.0 * num_bigint::BigInt::from(r[key].as_u64().unwrap());
        }
        if r["end_at"].is_null() {
            unknown.0 += &p.0;
        } else {
            complete.0 += &p.0;
        }
        projected.push(json!({"path":name,"mass":p.to_string(),"end_at":r["end_at"],"work":r["work"],"tasks":r["tasks"]}));
    }
    if available {
        json!({"paths":projected,"expected_work":work.to_string(),"expected_tasks":tasks.to_string(),"expected_attempts":attempts.to_string(),"query_work":query.to_string(),"decision_work":decision.to_string(),"completed":complete.to_string(),"unknown":unknown.to_string()})
    } else {
        let r = &paths["UU"];
        json!({"expected_observed_work":work.to_string(),"completed":complete.to_string(),"unknown":unknown.to_string(),"uu_observed_at":r["at"],"uu_observed_work":r["work"],"uu_end":r["end_at"]})
    }
}
pub fn validate_ir(system: &str, fault: &str) -> Value {
    let mut m =
        crate::lower::lower(&crate::syntax::parse(&crate::r2_inputs::source("R04")).unwrap())
            .unwrap();
    corrupt(&mut m, fault);
    match system {
        "engine" => {
            let o = crate::engine::explore(&m);
            if fault == "valid" {
                assert!(
                    o.diagnostics
                        .iter()
                        .all(|d| d.severity != crate::diag::Severity::Error)
                );
                return Value::Null;
            }
            assert!(
                o.response_receipts.as_ref().is_none_or(Vec::is_empty),
                "IR must fail before execution: {fault}"
            );
            let d = o
                .diagnostics
                .iter()
                .find(|d| d.severity == crate::diag::Severity::Error)
                .expect("IR refusal");
            assert!(
                d.message.starts_with("invalid_recovery_ir"),
                "{fault}: {d:?}"
            );
            json!({"reason":"invalid_recovery_ir","stop":"unsupported"})
        }
        "reference" => {
            let r = crate::reference::explore(&m);
            if fault == "valid" {
                assert_eq!(r.stop, crate::reference::StopReason::Complete);
                Value::Null
            } else {
                assert_eq!(
                    r.stop,
                    crate::reference::StopReason::SemanticAbort,
                    "{fault}"
                );
                assert_eq!(
                    r.statistics.frames, 0,
                    "IR must fail before enumeration: {fault}"
                );
                json!({"reason":"invalid_recovery_ir","stop":"semantic_abort"})
            }
        }
        _ => panic!("system"),
    }
}
fn corrupt(m: &mut ir::Model, fault: &str) {
    let (command, body) = m.recovery().unwrap();
    let query = if let ir::Stmt::Query(c, _) = &body[0].kind {
        c.clone()
    } else {
        panic!("query")
    };
    let target = m.capabilities[query.capability.0]
        .receipt_query
        .as_ref()
        .unwrap()
        .target;
    let ir::Cond::Contains { set, .. } =
        m.capabilities[target.0].atomic.as_ref().unwrap().condition
    else {
        panic!("G")
    };
    let refund = m
        .capabilities
        .iter()
        .position(|c| c.name == "refund")
        .unwrap();
    let status_ty = m.capabilities[query.capability.0].return_type;
    let unknown = ir::ValueId(
        m.types[status_ty.0]
            .members
            .iter()
            .position(|v| v == "still_unknown")
            .unwrap(),
    );
    let other = ir::BindingId(m.process.bindings.len());
    if fault == "valid" {
        return;
    }
    match fault {
        "initial_nonempty" => {
            m.state_variables[set.0].initial = ir::InitialValue::Fixed(ir::StateValue::Set(1))
        }
        "different_set" => {
            let mut v = m.state_variables[set.0].clone();
            v.name = "other_keys".into();
            let new = ir::StateVarId(m.state_variables.len());
            m.state_variables.push(v);
            let ir::Cond::Contains { set, .. } =
                &mut m.capabilities[target.0].atomic.as_mut().unwrap().condition
            else {
                panic!()
            };
            *set = new;
        }
        "non_case_key" => m.capabilities[target.0].params[0].ty = status_ty,
        "extra_writer" | "key_delete" => m.capabilities[refund].writes.push(ir::StateWrite {
            variable: set,
            source: ir::WriteSource::Constant(ir::StateValue::Set(u64::from(
                fault == "extra_writer",
            ))),
        }),
        "different_provider" | "wrong_provider" => {
            m.capabilities[query.capability.0].actor = ir::ActorId(1)
        }
        "ordinary_enum_source" => m.capabilities[query.capability.0].receipt_query = None,
        "query_commits" => m.capabilities[query.capability.0].commits = true,
        "query_writes" => {
            let v = m
                .state_variables
                .iter()
                .position(|v| v.name == "refunded")
                .unwrap();
            let ir::InitialValue::Fixed(x) = m.state_variables[v].initial else {
                panic!()
            };
            m.capabilities[query.capability.0]
                .writes
                .push(ir::StateWrite {
                    variable: ir::StateVarId(v),
                    source: ir::WriteSource::Constant(x),
                });
        }
        "query_kernel" => {
            m.capabilities[query.capability.0].kernel = Some(ir::Kernel {
                name: "query_bad".into(),
                basis: None,
                given: Vec::new(),
                rows: vec![ir::KernelRow {
                    values: Vec::new(),
                    branches: (0..3)
                        .map(|v| ir::KernelBranch {
                            probability: Rational(num_rational::BigRational::new(
                                1.into(),
                                3.into(),
                            )),
                            result: ir::ValueId(v),
                            overrides: Vec::new(),
                        })
                        .collect(),
                }],
            })
        }
        "query_requires" => {
            let v = m
                .state_variables
                .iter()
                .position(|v| v.name == "refunded")
                .unwrap();
            let ir::InitialValue::Fixed(x) = m.state_variables[v].initial else {
                panic!()
            };
            m.capabilities[query.capability.0].requires = Some(ir::Requires {
                condition: ir::Cond::StateEq(ir::StateMatch {
                    variable: ir::StateVarId(v),
                    value: x,
                }),
                otherwise: unknown,
                span: query.span,
            });
        }
        "wrong_key" => {
            m.capabilities[query.capability.0]
                .receipt_query
                .as_mut()
                .unwrap()
                .key_parameter = 1
        }
        "b_handler" => {
            let a = m.capabilities[target.0].atomic.take().unwrap();
            m.capabilities[target.0].writes =
                a.writes.into_iter().filter(|w| w.variable != set).collect();
            for s in &mut m.process.body {
                if let ir::Stmt::Response(_, r) = &mut s.kind {
                    r.idempotent = false;
                    r.idempotency = None;
                    r.effect_modeled = true;
                }
            }
        }
        _ => {}
    }
    let original = m.process.body[0].clone();
    let ir::Stmt::Response(_, r) = &mut m.process.body[0].kind else {
        panic!("response")
    };
    let b = r.recovery.as_mut().unwrap();
    match fault {
        "wrong_case" => {
            if let ir::Stmt::Query(c, _) = &mut b[0].kind {
                c.args = vec![other];
            }
        }
        "non_dominating" => {
            let q = b.remove(0);
            b.push(q);
        }
        "unknown_fact" => {
            if let ir::Stmt::If { condition, .. } = &mut b[1].kind {
                *condition = ir::Cond::Eq(ir::ResultMatch {
                    binding: query.binding,
                    value: unknown,
                });
            }
        }
        "query_count33" => {
            for n in 0..32 {
                let id = ir::BindingId(m.process.bindings.len());
                let mut binding = m.process.bindings[query.binding.0].clone();
                binding.name = format!("extra{n}");
                m.process.bindings.push(binding);
                let mut q = query.clone();
                q.binding = id;
                b.insert(
                    0,
                    ir::Statement {
                        span: q.span,
                        kind: ir::Stmt::Query(q, command),
                    },
                );
            }
        }
        "nested_handler" => b.insert(0, original.clone()),
        _ => {}
    }
    fn edit(b: &mut ir::Block, fault: &str, command: ir::BindingId, query: &ir::Call) {
        let mut i = 0;
        while i < b.len() {
            match &mut b[i].kind {
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    edit(then_body, fault, command, query);
                    edit(else_body, fault, command, query);
                }
                ir::Stmt::ResolveEffect {
                    command: c,
                    evidence,
                } => match fault {
                    "missing_evidence" => *evidence = command,
                    "wrong_command" => *c = query.binding,
                    "double_resolve" => {
                        let copy = b[i].clone();
                        b.insert(i, copy);
                        i += 1;
                    }
                    "unresolved_end" => {
                        b.remove(i);
                        continue;
                    }
                    _ => {}
                },
                ir::Stmt::End(_) if fault == "resolved_stop" => {
                    b[i].kind = ir::Stmt::StopReceiptUnknown
                }
                _ => {}
            }
            i += 1;
        }
    }
    edit(b, fault, command, &query);
    if fault == "shared_command_key" {
        let mut new = original;
        let ir::Stmt::Response(c, r) = &mut new.kind else {
            panic!()
        };
        let alias = ir::BindingId(m.process.bindings.len());
        let mut binding = m.process.bindings[command.0].clone();
        binding.name = "other".into();
        m.process.bindings.push(binding.clone());
        let site = ir::BindingId(m.process.bindings.len());
        binding.name = format!("@response:{}:1", alias.0);
        m.process.bindings.push(binding);
        c.binding = site;
        r.command = alias;
        r.command_call.binding = alias;
        r.recovery = None;
        m.process.body.push(new);
    }
}
