//! Independent lexical M1 re-enumeration: transitive source sets, peer union,
//! descendant exclusion, declared suppressions. No production Agency walker.
#![allow(dead_code)]
use relca_core::ir;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn calls<'a>(block: &'a ir::Block, out: &mut Vec<&'a ir::Call>) {
    for statement in block {
        match &statement.kind {
            ir::Stmt::Query(_, _)
            | ir::Stmt::ResolveEffect { .. }
            | ir::Stmt::StopReceiptUnknown => panic!("receipt recovery is outside this oracle"),
            ir::Stmt::WaitUntil(_) => panic!("common-cause oracle does not handle condition waits"),
            ir::Stmt::Response(_, _) => {
                panic!("independent common-cause oracle does not cover receipt or retry control")
            }
            ir::Stmt::Call(call) => out.push(call),
            ir::Stmt::Fork { branches, .. } => out.extend(branches),
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                calls(then_body, out);
                calls(else_body, out);
            }
            ir::Stmt::Wait(_) | ir::Stmt::End(_) => {}
        }
    }
}

fn must_before(
    block: &ir::Block,
    mut facts: Option<BTreeSet<ir::BindingId>>,
    out: &mut BTreeMap<ir::BindingId, BTreeSet<ir::BindingId>>,
) -> Option<BTreeSet<ir::BindingId>> {
    for s in block {
        match &s.kind {
            ir::Stmt::Query(_, _)
            | ir::Stmt::ResolveEffect { .. }
            | ir::Stmt::StopReceiptUnknown => panic!("receipt recovery is outside this oracle"),
            ir::Stmt::WaitUntil(_) => panic!("must-before oracle does not handle condition waits"),
            ir::Stmt::Response(_, _) => {
                panic!("independent must-before oracle does not cover receipt or retry control")
            }
            ir::Stmt::Call(c) => {
                if let Some(f) = &mut facts {
                    out.insert(c.binding, f.clone());
                    f.insert(c.binding);
                }
            }
            ir::Stmt::Fork { branches, join, .. } => {
                if let Some(f) = &mut facts {
                    for c in branches {
                        out.insert(c.binding, f.clone());
                    }
                    if matches!(
                        join,
                        ir::Join::All
                            | ir::Join::Any {
                                remaining: ir::Remaining::Drain,
                                ..
                            }
                    ) || branches.len() == 1
                    {
                        f.extend(branches.iter().map(|c| c.binding));
                    }
                }
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                let a = must_before(then_body, facts.clone(), out);
                let b = must_before(else_body, facts, out);
                facts = match (a, b) {
                    (Some(a), Some(b)) => Some(a.intersection(&b).copied().collect()),
                    (a, b) => a.or(b),
                };
            }
            ir::Stmt::End(_) => facts = None,
            ir::Stmt::Wait(_) => {}
        }
    }
    facts
}

pub fn enumerate(m: &ir::Model) -> (Value, Vec<Value>) {
    let mut all = vec![];
    calls(&m.process.body, &mut all);
    let index: BTreeMap<_, _> = all
        .iter()
        .enumerate()
        .map(|(i, c)| (c.binding, i))
        .collect();
    let mut parents: Vec<BTreeSet<usize>> = all
        .iter()
        .map(|c| {
            c.args
                .iter()
                .filter(|a| **a != ir::CURRENT_CASE)
                .map(|a| index[a])
                .collect()
        })
        .collect();
    for (i, c) in all.iter().enumerate() {
        for variable in &m.capabilities[c.capability.0].read_variables {
            let writers: Vec<_> = all
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    m.capabilities[p.capability.0]
                        .writes
                        .iter()
                        .any(|w| w.variable == *variable && w.source == ir::WriteSource::Result)
                })
                .map(|(j, _)| j)
                .collect();
            if writers.len() == 1 && writers[0] != i {
                parents[i].insert(writers[0]);
            }
        }
    }
    loop {
        let previous = parents.clone();
        for row in &mut parents {
            for p in row.clone() {
                row.extend(previous[p].iter().copied());
            }
        }
        if previous == parents {
            break;
        }
    }
    let evidence: Vec<BTreeSet<_>> = all
        .iter()
        .enumerate()
        .map(|(i, c)| {
            std::iter::once(c)
                .chain(parents[i].iter().map(|j| &all[*j]))
                .flat_map(|c| m.capabilities[c.capability.0].reads.iter().copied())
                .collect()
        })
        .collect();
    let mut before = BTreeMap::new();
    must_before(&m.process.body, Some(BTreeSet::new()), &mut before);
    let target = |i: usize| {
        let c = all[i];
        let cap = &m.capabilities[c.capability.0];
        json!({"call":m.process.bindings[c.binding.0].name,"capability":format!("{}.{}",m.actors[cap.actor.0].name,cap.name),"kernel":cap.kernel.as_ref().unwrap().name})
    };
    let latent = |id: ir::StateVarId| {
        let v = &m.state_variables[id.0];
        match v.kind {
            ir::StateKind::ActorLatent(a) => format!("{}.{}", m.actors[a.0].name, v.name),
            _ => v.name.clone(),
        }
    };
    let mut users = BTreeMap::<ir::StateVarId, Vec<usize>>::new();
    let mut groups = BTreeMap::<usize, BTreeSet<usize>>::new();
    let mut errors = BTreeMap::<(usize, usize), BTreeSet<ir::StateVarId>>::new();
    let mut result_vars = vec![BTreeSet::new(); all.len()];
    for (i, c) in all.iter().enumerate() {
        let cap = &m.capabilities[c.capability.0];
        let Some(k) = &cap.kernel else { continue };
        for given in &k.given {
            if matches!(
                m.state_variables[given.0].kind,
                ir::StateKind::ActorLatent(_)
            ) {
                users.entry(*given).or_default().push(i);
            }
        }
        if k.given.is_empty() {
            continue;
        }
        for w in &cap.writes {
            if m.state_variables[w.variable.0].kind == ir::StateKind::ObservedCase
                && m.state_variables[w.variable.0].base == cap.return_type
                && w.source == ir::WriteSource::Result
                && k.rows.iter().flat_map(|r| &r.branches).all(|b| {
                    b.overrides.iter().all(|(v, value)| {
                        *v != w.variable || *value == ir::StateValue::Value(b.result)
                    })
                })
            {
                result_vars[i].insert(w.variable);
            }
        }
        for p in &parents[i] {
            for v in &m.capabilities[all[*p].capability.0].kernel_writes {
                if k.given.contains(v) {
                    groups.entry(*p).or_default().insert(i);
                    errors.entry((i, *p)).or_default().insert(*v);
                }
            }
        }
    }
    let causes:Vec<_>=users.into_iter().filter(|(_,u)|u.len()>1).map(|(id,u)|{
        let ir::StateKind::ActorLatent(a)=m.state_variables[id.0].kind else{unreachable!()};
        let mut row=json!({"actor":m.actors[a.0].name,"latent":latent(id),"targets":u.into_iter().map(&target).collect::<Vec<_>>()});
        if let Some(instance)=m.actors[a.0].metadata.instance {row["instance"]=json!(instance);}
        row
    }).collect();
    let mut agency = serde_json::Map::new();
    if !causes.is_empty() {
        agency.insert("common_causes".into(),json!({"rules":"common-cause-p1-1","scope":"selected_process_lexical_calls","notice":"条件付き独立は given の下でだけ仮定され、無条件には独立でない","causes":causes}));
    }
    let descriptions: Vec<_> = m
        .actors
        .iter()
        .filter_map(|a| {
            a.metadata
                .source_description
                .as_ref()
                .map(|d| json!({"actor":a.name,"kind":"tacit","description":d}))
        })
        .collect();
    if !descriptions.is_empty() {
        agency.insert("source_descriptions".into(), json!(descriptions));
    }
    let mut warnings = vec![];
    let mut uncertainty = vec![];
    for (i, c) in all.iter().enumerate() {
        let Some(k) = &m.capabilities[c.capability.0].kernel else {
            continue;
        };
        let mut targets = [BTreeSet::new(), BTreeSet::new(), BTreeSet::new()];
        let mut latents = [BTreeSet::new(), BTreeSet::new(), BTreeSet::new()];
        let mut tacit = BTreeSet::new();
        let mut correlations = BTreeSet::new();
        for ((consumer, p), error) in &errors {
            if *consumer != i || groups[p].len() < 2 {
                continue;
            }
            let peer_sources: BTreeSet<_> = groups[p]
                .iter()
                .filter(|j| **j != i && !parents[**j].contains(&i))
                .flat_map(|j| evidence[*j].iter().copied())
                .collect();
            let new: Vec<_> = evidence[i].difference(&peer_sources).copied().collect();
            if new
                .iter()
                .any(|s| m.actors[s.0].source != Some(ir::SourceKind::Tacit))
            {
                continue;
            }
            let correlated: BTreeSet<_> = groups[p]
                .iter()
                .filter(|j| **j != i)
                .filter(|j| {
                    let other = m.capabilities[all[**j].capability.0]
                        .kernel
                        .as_ref()
                        .unwrap();
                    !result_vars[i].iter().any(|v| other.given.contains(v))
                        && (parents[i].contains(j)
                            || before
                                .get(&c.binding)
                                .is_some_and(|f| f.contains(&all[**j].binding)))
                        && result_vars[**j].iter().any(|v| k.given.contains(v))
                })
                .copied()
                .collect();
            let category = if !new.is_empty() {
                tacit.extend(new);
                1
            } else if !correlated.is_empty() {
                correlations.extend(correlated);
                2
            } else {
                0
            };
            targets[category].insert(*p);
            latents[category].extend(error.iter().copied());
        }
        for category in [1usize, 2, 0] {
            if targets[category].is_empty() {
                continue;
            }
            let names: Vec<_> = targets[category]
                .iter()
                .map(|p| m.process.bindings[all[*p].binding.0].name.clone())
                .collect();
            let t = target(i);
            if category == 0 && k.basis.is_none() {
                warnings.push(json!({"code":"W-LIN-2","severity":"warning","span":{"start":c.span.start,"end":c.span.end},
                    "message":format!("{} {} kernel {} targets [{}]: conditional independence under given assumes detection without a new evidence source",t["call"].as_str().unwrap(),t["capability"].as_str().unwrap(),t["kernel"].as_str().unwrap(),names.join(", "))}));
            } else {
                let mut item = t;
                item["targets"] = json!(names);
                item["error_latents"] = json!(
                    latents[category]
                        .iter()
                        .copied()
                        .map(&latent)
                        .collect::<Vec<_>>()
                );
                let (kind, notice) = match category {
                    1 => {
                        item["sources"] = json!(
                            tacit
                                .iter()
                                .map(|s| m.actors[s.0].name.clone())
                                .collect::<Vec<_>>()
                        );
                        (
                            "wlin2_tacit",
                            "tacit は分析者の宣言であり、根拠源の妥当性は検査していない",
                        )
                    }
                    2 => {
                        item["judgements"] = json!(
                            correlations
                                .iter()
                                .map(|j| m.process.bindings[all[*j].binding.0].name.clone())
                                .collect::<Vec<_>>()
                        );
                        (
                            "wlin2_correlation",
                            "相関は先行する判定結果への条件付けであり、残余相関の推定ではない",
                        )
                    }
                    _ => {
                        item["basis"] = json!(k.basis.as_ref().unwrap());
                        (
                            "wlin2_basis",
                            "basis は分析者の宣言であり、測定根拠の正しさは検査していない",
                        )
                    }
                };
                item["kind"] = json!(kind);
                item["notice"] = json!(notice);
                uncertainty.push(item);
            }
        }
    }
    if !uncertainty.is_empty() {
        agency.insert("model_uncertainties".into(),json!({"rules":"wlin2-suppression-p1-1","scope":"selected_process_lexical_calls","items":uncertainty}));
    }
    (Value::Object(agency), warnings)
}

pub fn check(report: &Value, model: &ir::Model) -> Result<(), String> {
    if report["agency"].is_null() {
        // A successfully lowered model normally publishes Agency. The public
        // simulation front end is the exception: a missing simulation clause
        // is refused before the lowered Agency is transported (ADR 0016).
        // Require BOTH the independent IR condition and the specific refusal;
        // an arbitrary error or a dropped Agency must not skip M1 silently.
        let missing_simulation = model.simulation.is_none()
            && report["mode"] == "simulation"
            && report["verdict"] == "invalid"
            && report["diagnostics"].as_array().is_some_and(|rows| {
                rows.iter().any(|d| {
                    d["code"] == "E-SIM-0"
                        && d["severity"] == "error"
                        && d["message"]
                            .as_str()
                            .is_some_and(|s| s.contains("explicit simulation clause"))
                })
            });
        return if missing_simulation {
            Ok(())
        } else {
            Err("independent M1: lowered IR requires Agency; no justified missing-simulation refusal".into())
        };
    }
    let (expected, warnings) = enumerate(model);
    for field in [
        "common_causes",
        "model_uncertainties",
        "source_descriptions",
    ] {
        if report["agency"].get(field) != expected.get(field) {
            return Err(format!("independent M1 {field}"));
        }
    }
    let actual: Vec<_> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-LIN-2")
        .cloned()
        .collect();
    if actual != warnings {
        return Err(format!(
            "independent M1 warning/span/order: {}!={}",
            actual.len(),
            warnings.len()
        ));
    }
    Ok(())
}
