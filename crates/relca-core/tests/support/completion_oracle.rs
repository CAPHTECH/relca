//! Independent ADR0025 inventory over the pre-ADR0025 IR.
//!
//! No production completion/unreachable helper is used. Start is tested by a
//! false invariant on the existing Start event; executor assignment by the old
//! permission monitor (permission does NOT filter dispatch). If arms are tested
//! by replacing just that arm with a fresh absorbing end: the first reach of
//! that marker has exactly the original prefix. Unreached probes cannot change
//! any prefix. End inventory is lexical, observations are business terminals.
//! This is a counterfactual-query oracle, not a second transition generator.
#![allow(dead_code)]
use relca_core::{Verdict, diag::Span, engine, ir::*, outcome::CaseTerminalKind};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn contract_probe(m: &mut Model, kind: ContractKind) -> bool {
    let label = format!("{kind:?}");
    let id = ClauseId(m.contracts.len());
    m.contracts.push(ContractClause {
        ordinal: None,
        name: None,
        kind,
        span: m.process.span,
    });
    m.verify.push(Verify {
        property: Property::Contract(id),
        span: m.process.span,
    });
    let out = engine::explore(m);
    assert!(
        !out.diagnostics.iter().any(|d| d.code.starts_with("U-")),
        "invalid oracle probe {label}: {:?}",
        out.diagnostics
    );
    let p = out
        .properties
        .iter()
        .find(|p| p.property == Property::Contract(id))
        .unwrap();
    p.verdict == Verdict::Violated
        || p.cases
            .as_ref()
            .is_some_and(|cs| cs.iter().any(|c| c.verdict == Verdict::Violated))
}
fn started(model: &Model, cap: CapabilityId) -> bool {
    let mut m = model.clone();
    let ty = TypeId(m.types.len());
    m.types.push(EnumType {
        name: "OracleBool".into(),
        record: None,
        members: vec!["no".into(), "yes".into()],
        span: m.process.span,
    });
    let variable = StateVarId(m.state_variables.len());
    m.state_variables.push(StateVariable {
        last_write_wins: false,
        case_set_size: None,
        name: "oracle_false".into(),
        kind: StateKind::ObservedCase,
        base: ty,
        optional: false,
        integer_max: None,
        initial: InitialValue::Fixed(StateValue::Value(ValueId(0))),
        span: m.process.span,
    });
    contract_probe(
        &mut m,
        ContractKind::Invariant {
            event: Event {
                capability: cap,
                kind: EventKind::Start,
            },
            condition: Some(Cond::StateEq(StateMatch {
                variable,
                value: StateValue::Value(ValueId(1)),
            })),
        },
    )
}
fn assigned(model: &Model, person: PersonId) -> bool {
    for (i, c) in model.capabilities.iter().enumerate() {
        let Some(pool) = &model.actors[c.actor.0].executors else {
            continue;
        };
        if !pool.contains(&person) {
            continue;
        }
        let mut m = model.clone();
        // Permission need not be a subset of the executor pool and never gates
        // Start. A fresh non-executor avoids an invalid empty permission when
        // p is the sole executor, without adding any dispatch candidate.
        let old_people: Vec<_> = m.people.iter().map(|p| p.name.clone()).collect();
        // PersonId was added after this oracle. Keep its declaration roster
        // valid when adding an unreachable non-executor sentinel. A nondet
        // PersonId initial state would gain an initial world, so refuse that
        // unsupported instrumentation rather than silently changing prefixes.
        assert!(
            !m.state_variables.iter().any(|v| m
                .types
                .get(v.base.0)
                .is_some_and(|t| t.name == "PersonId" && t.members == old_people)
                && v.initial == InitialValue::Nondet),
            "nondet PersonId needs a non-expanding assignment probe"
        );
        let dummy = PersonId(m.people.len());
        for ty in &mut m.types {
            if ty.name == "PersonId" && ty.members == old_people {
                ty.members.push("OracleNonExecutor".into());
            }
        }
        m.people.push(Person {
            name: "OracleNonExecutor".into(),
            span: m.process.span,
        });
        m.capabilities[i].permission = Some(
            pool.iter()
                .copied()
                .filter(|p| *p != person)
                .chain([dummy])
                .collect(),
        );
        // These certificates contain permission booleans, not execution state.
        // Drop duties (no runtime effect) and update only the permission-derived
        // fields of the old contest certificate; retain every runtime contest.
        if let Some(meta) = &mut m.responsibility {
            meta.duties.clear();
            meta.responsibilities.clear();
            let name = format!("{}.{}", m.actors[c.actor.0].name, c.name);
            for path in &mut meta.contest_paths {
                let reversal = &mut path.checks.reversal_authority;
                for p in &mut reversal.people {
                    for d in &mut p.decisions {
                        if d.capability == name {
                            d.permission = m.capabilities[i]
                                .permission
                                .as_ref()
                                .unwrap()
                                .iter()
                                .any(|id| m.people[id.0].name == p.person);
                        }
                    }
                    p.satisfied = p.decisions.iter().any(|d| {
                        d.executor && d.included && d.permission && !d.authorities.is_empty()
                    });
                    p.reason = if p.satisfied {
                        None
                    } else {
                        Some(
                            if p.decisions.is_empty() {
                                "no_decision"
                            } else if !p.decisions.iter().any(|d| d.executor) {
                                "executor_not_connected"
                            } else if !p.decisions.iter().any(|d| d.executor && d.included) {
                                "decision_excluded"
                            } else if !p
                                .decisions
                                .iter()
                                .any(|d| d.executor && d.included && d.permission)
                            {
                                "permission_missing"
                            } else {
                                "Permission あり、Authority 未接続"
                            }
                            .into(),
                        )
                    };
                }
                reversal.satisfied = reversal.people.iter().all(|p| p.satisfied);
                path.missing = [
                    ("route_exists", path.checks.route_exists.satisfied),
                    ("reversal_authority", reversal.satisfied),
                    ("deadline_defined", path.checks.deadline_defined.satisfied),
                ]
                .into_iter()
                .filter(|(_, v)| !*v)
                .map(|(s, _)| s.into())
                .collect();
                path.verdict = if path.missing.is_empty() {
                    Verdict::Satisfied
                } else {
                    Verdict::Violated
                };
                path.basis = if path.missing.is_empty() {
                    "構造上存在"
                } else {
                    "構造の不足"
                }
                .into();
            }
        }
        if contract_probe(
            &mut m,
            ContractKind::Permitted {
                capability: CapabilityId(i),
            },
        ) {
            return true;
        }
    }
    false
}
fn target_seen(m: &Model, target: EndId) -> bool {
    let out = engine::explore(m);
    assert!(
        !out.diagnostics.iter().any(|d| d.code.starts_with("U-")),
        "invalid arm probe: {:?}",
        out.diagnostics
    );
    out.terminals.iter().any(|t| t.end == target)
        || out.case_outcomes.as_ref().is_some_and(|cs| {
            cs.iter().any(|c| {
                c.terminals
                    .iter()
                    .any(|t| t.kind == CaseTerminalKind::End(target))
            })
        })
}
type Arm = (Vec<(usize, bool)>, usize, bool, Span);
fn inventory(
    block: &Block,
    path: &mut Vec<(usize, bool)>,
    arms: &mut Vec<Arm>,
    ends: &mut BTreeMap<EndId, Span>,
    calls: &mut BTreeSet<CapabilityId>,
) {
    for (i, s) in block.iter().enumerate() {
        match &s.kind {
            Stmt::LoopControl(_) => panic!("bounded repetition is outside this oracle"),
            Stmt::Query(_, _) | Stmt::ResolveEffect { .. } | Stmt::StopReceiptUnknown => {
                panic!("receipt recovery is outside this oracle")
            }
            Stmt::WaitUntil(_) => panic!("completion oracle does not handle condition waits"),
            Stmt::Response(_, _) => {
                panic!("completion oracle does not cover receipt or retry semantics")
            }
            Stmt::Call(c) => {
                calls.insert(c.capability);
            }
            Stmt::Wait(_) => {}
            Stmt::Fork { branches, join, .. } => {
                calls.extend(branches.iter().map(|c| c.capability));
                if let Join::Any { if_none, .. } = join {
                    ends.entry(*if_none).or_insert(s.span);
                }
            }
            Stmt::End(e) => {
                ends.entry(*e).or_insert(s.span);
            }
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                for side in [true, false] {
                    arms.push((path.clone(), i, side, s.span));
                }
                path.push((i, true));
                inventory(then_body, path, arms, ends, calls);
                path.pop();
                path.push((i, false));
                inventory(else_body, path, arms, ends, calls);
                path.pop();
            }
        }
    }
}
fn arm_seen(model: &Model, (path, index, side, span): &Arm) -> bool {
    let mut m = model.clone();
    let marker = EndId(m.ends.len());
    m.ends.push("OracleMarker".into());
    let mut block = &mut m.process.body;
    for (i, side) in path {
        let Stmt::If {
            then_body,
            else_body,
            ..
        } = &mut block[*i].kind
        else {
            unreachable!()
        };
        block = if *side { then_body } else { else_body };
    }
    let Stmt::If {
        then_body,
        else_body,
        ..
    } = &mut block[*index].kind
    else {
        unreachable!()
    };
    *(if *side { then_body } else { else_body }) = vec![Statement {
        kind: Stmt::End(marker),
        span: *span,
    }];
    // Existing contracts may refer to a binding in the removed suffix; these
    // monitors are not transition guards. Remove them for this reach-only probe.
    m.contracts.clear();
    m.responsibility = None;
    m.metrics = None;
    // Newer finite domains require their intrinsic monitors even in a lexical
    // arm probe. They stop exactly the same invalid-state prefixes as before;
    // removing them would make the independent counterfactual IR invalid.
    let intrinsic: Vec<_> = m
        .verify
        .iter()
        .filter(|v| {
            matches!(
                v.property,
                Property::StateInRange | Property::NoLostUpdate(_)
            )
        })
        .cloned()
        .collect();
    m.verify = vec![Verify {
        property: Property::ReachableEnd(marker),
        span: *span,
    }];
    m.verify.extend(intrinsic);
    target_seen(&m, marker)
}
fn row(code: &str, model: &Model, target: String, reason: &str, span: Span) -> Value {
    json!({"code":code,"severity":"warning","process":model.process.name,"target":target,"reason":reason,"span":{"start":span.start,"end":span.end}})
}
pub fn warnings(model: &Model) -> Vec<Value> {
    let out = engine::explore(model);
    assert!(
        !out.truncated && !out.diagnostics.iter().any(|d| d.code.starts_with("U-")),
        "oracle requires complete uncut input"
    );
    let mut arms = vec![];
    let mut ends = BTreeMap::new();
    let mut calls = BTreeSet::new();
    inventory(
        &model.process.body,
        &mut vec![],
        &mut arms,
        &mut ends,
        &mut calls,
    );
    calls.extend(model.contests().iter().map(|c| c.route));
    let included: BTreeSet<_> = model.cut.as_ref().map_or_else(
        || (0..model.actors.len()).map(ActorId).collect(),
        |c| c.include.iter().copied().collect(),
    );
    let mut rows = vec![];
    for (i, c) in model.capabilities.iter().enumerate() {
        let id = CapabilityId(i);
        if included.contains(&c.actor) && !started(model, id) {
            rows.push(row(
                "W-UNREACH-0",
                model,
                format!("{}.{}", model.actors[c.actor.0].name, c.name),
                if calls.contains(&id) {
                    "never_started"
                } else {
                    "not_called"
                },
                c.span,
            ));
        }
    }
    for arm in arms {
        if !arm_seen(model, &arm) {
            rows.push(row(
                "W-UNREACH-1",
                model,
                if arm.2 { "then" } else { "else" }.into(),
                "never_taken",
                arm.3,
            ));
        }
    }
    for (end, span) in ends {
        if !target_seen(model, end) {
            rows.push(row(
                "W-UNREACH-2",
                model,
                model.ends[end.0].clone(),
                "never_reached",
                span,
            ));
        }
    }
    let pool: BTreeSet<_> = model
        .actors
        .iter()
        .enumerate()
        .filter(|(i, _)| included.contains(&ActorId(*i)))
        .flat_map(|(_, a)| a.executors.iter().flatten().copied())
        .collect();
    for p in pool {
        if !assigned(model, p) {
            rows.push(row(
                "W-UNREACH-3",
                model,
                model.people[p.0].name.clone(),
                "never_assigned",
                model.people[p.0].span,
            ));
        }
    }
    rows.sort_by_key(|r| {
        (
            r["code"].as_str().unwrap().to_owned(),
            r["span"]["start"].as_u64().unwrap(),
            if r["code"] == "W-UNREACH-1" && r["target"] == "else" {
                1
            } else {
                0
            },
        )
    });
    rows
}
