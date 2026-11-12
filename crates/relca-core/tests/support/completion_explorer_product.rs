//! Product-side observation adapter, not part of the independent explorer.
//! Public Outcome lacks Start/assignment/arm sets. Read them with monitor and
//! absorbing-marker probes, and compare to independently generated sets.
use super::completion_explorer::*;
use relca_core::{Verdict, engine, ir, outcome::Outcome};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn probe(m: &ir::Model, kind: ir::ContractKind) -> bool {
    let mut m = m.clone();
    let id = ir::ClauseId(m.contracts.len());
    m.contracts.push(ir::ContractClause {
        ordinal: None,
        name: None,
        kind,
        span: m.process.span,
    });
    m.verify.push(ir::Verify {
        property: ir::Property::Contract(id),
        span: m.process.span,
    });
    let o = engine::explore(&m);
    assert!(
        !o.diagnostics.iter().any(|d| d.code.starts_with("U-")),
        "invalid product probe {o:?}"
    );
    o.properties
        .iter()
        .find(|p| p.property == ir::Property::Contract(id))
        .unwrap()
        .verdict
        == Verdict::Violated
}
fn observed_arms(m: &ir::Model) -> BTreeSet<(Location, bool)> {
    fn walk(
        m: &ir::Model,
        b: &ir::Block,
        path: Vec<(usize, bool)>,
        out: &mut BTreeSet<(Location, bool)>,
    ) {
        for (i, s) in b.iter().enumerate() {
            if let ir::Stmt::If {
                then_body,
                else_body,
                ..
            } = &s.kind
            {
                for take in [true, false] {
                    let mut changed = m.clone();
                    let marker = ir::EndId(changed.ends.len());
                    changed.ends.push("ProductObservationMarker".into());
                    let mut body = &mut changed.process.body;
                    for (j, yes) in &path {
                        let ir::Stmt::If {
                            then_body,
                            else_body,
                            ..
                        } = &mut body[*j].kind
                        else {
                            unreachable!()
                        };
                        body = if *yes { then_body } else { else_body };
                    }
                    let ir::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } = &mut body[i].kind
                    else {
                        unreachable!()
                    };
                    *(if take { then_body } else { else_body }) = vec![ir::Statement {
                        kind: ir::Stmt::End(marker),
                        span: s.span,
                    }];
                    changed.verify = vec![ir::Verify {
                        property: ir::Property::ReachableEnd(marker),
                        span: s.span,
                    }];
                    let o = engine::explore(&changed);
                    assert!(
                        !o.diagnostics.iter().any(|d| d.code.starts_with("U-")),
                        "invalid product arm probe {o:?}"
                    );
                    if o.terminals.iter().any(|t| t.end == marker) {
                        out.insert(((s.span.start, s.span.end), take));
                    }
                }
                for (yes, child) in [(true, then_body), (false, else_body)] {
                    let mut p = path.clone();
                    p.push((i, yes));
                    walk(m, child, p, out);
                }
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(m, &m.process.body, vec![], &mut out);
    out
}
pub fn observations(m: &ir::Model, o: &Outcome) -> Observation {
    assert!(
        !o.diagnostics.iter().any(|d| d.code.starts_with("U-")),
        "unexpected refusal {o:?}"
    );
    let mut actual = Observation {
        cut: o.truncated,
        deadlock: o.diagnostics.iter().any(|d| d.code == "W-DEADLOCK-0"),
        arms: observed_arms(m),
        ..Observation::default()
    };
    for t in &o.terminals {
        // All hand population terminals are singleton time/work sets. Do not
        // expand extrema into a fictional set of intermediate measurements.
        assert_eq!(
            t.elapsed.min, t.elapsed.max,
            "non-singleton outside comparison lane"
        );
        assert_eq!(
            t.work.min, t.work.max,
            "non-singleton outside comparison lane"
        );
        assert!(t.result_at.is_none());
        let end = m.ends[t.end.0].clone();
        actual.ends.insert(end.clone());
        actual.terminals.insert((end, t.elapsed.min, t.work.min));
    }
    for (i, c) in m.capabilities.iter().enumerate() {
        let id = ir::CapabilityId(i);
        // First Start of a Capability cannot have its own preceding finish.
        if probe(
            m,
            ir::ContractKind::RequiresEvent {
                before: ir::Event {
                    capability: id,
                    kind: ir::EventKind::Finish(None),
                },
                start: ir::Event {
                    capability: id,
                    kind: ir::EventKind::Start,
                },
            },
        ) {
            actual
                .started
                .insert(format!("{}.{}", m.actors[c.actor.0].name, c.name));
        }
    }
    for (i, p) in m.people.iter().enumerate() {
        for (ci, c) in m.capabilities.iter().enumerate() {
            let Some(pool) = &m.actors[c.actor.0].executors else {
                continue;
            };
            if !pool.contains(&ir::PersonId(i)) {
                continue;
            }
            let mut changed = m.clone();
            let dummy = ir::PersonId(changed.people.len());
            changed.people.push(ir::Person {
                name: "ProductNonExecutor".into(),
                span: m.process.span,
            });
            changed.capabilities[ci].permission = Some(
                pool.iter()
                    .copied()
                    .filter(|p| p.0 != i)
                    .chain([dummy])
                    .collect(),
            );
            if probe(
                &changed,
                ir::ContractKind::Permitted {
                    capability: ir::CapabilityId(ci),
                },
            ) {
                actual.assigned.insert(p.name.clone());
                break;
            }
        }
    }
    actual
}
pub fn verdict(t: Truth) -> Verdict {
    match t {
        Truth::Yes => Verdict::Satisfied,
        Truth::No => Verdict::Violated,
        Truth::Unknown => Verdict::Inconclusive,
    }
}
pub fn check_properties(m: &ir::Model, o: &Outcome, expected: &Observation) {
    assert_eq!(m.verify.len(), o.properties.len());
    for (v, p) in m.verify.iter().zip(&o.properties) {
        assert_eq!(v.property, p.property);
        let truth = match v.property {
            ir::Property::RecoverableEnd
            | ir::Property::TerminatingEnd
            | ir::Property::AlmostSureEnd => expected.completion(),
            ir::Property::ReachableEnd(e) => expected.reachable(&m.ends[e.0]),
            _ => panic!("not in test query lane"),
        };
        assert_eq!(p.verdict, verdict(truth), "{:?}", v.property);
        if v.property == ir::Property::AlmostSureEnd {
            let (min, max) = completion_probability(expected);
            let range = p.probability.as_ref().expect("almost-sure bounds");
            assert_eq!(
                (range.min.to_string(), range.max.to_string()),
                (min.into(), max.into())
            );
        } else {
            assert!(p.probability.is_none());
        }
    }
}
fn message(w: &Warning) -> String {
    let detail = match w.code.as_str() {
        "W-UNREACH-0" => format!("{} {}", w.target, w.reason),
        "W-UNREACH-1" => format!("if {} arm is never taken", w.target),
        "W-UNREACH-2" => format!("end.{} is never reached", w.target),
        "W-UNREACH-3" => format!("executor {} is never assigned", w.target),
        _ => panic!("unknown warning"),
    };
    format!("process P: {detail}")
}
pub fn check_report(report: &Value, m: &OwnModel, expected: &Observation) -> Result<(), String> {
    let wanted=warnings(m,expected).iter().map(|w| json!({"code":w.code,"severity":"warning","message":message(w),"span":{"start":w.location.0,"end":w.location.1}})).collect::<Vec<_>>();
    let mut actual = report["diagnostics"]
        .as_array()
        .ok_or("diagnostics missing")?
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("W-UNREACH-"))
        })
        .cloned()
        .collect::<Vec<_>>();
    // Ordering differs from set semantics, but duplicate/missing/payload edits
    // cannot be hidden by sorting (vector cardinality is retained).
    let sort = |v: &Value| v.to_string();
    let mut wanted = wanted;
    actual.sort_by_key(sort);
    wanted.sort_by_key(sort);
    if actual != wanted {
        return Err(format!(
            "warning code/target/reason/span mismatch: {actual:?} != {wanted:?}"
        ));
    }
    if report["bounds"]["truncated"] != json!(expected.cut) {
        return Err("clock cut coverage mismatch".into());
    }
    let unchecked = report["unchecked"]
        .as_array()
        .map_or(0, |a| a.iter().filter(|v| **v == "unreachable").count());
    if unchecked != usize::from(expected.cut) {
        return Err("unreachable coverage mismatch".into());
    }
    let deadlock = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "W-DEADLOCK-0");
    if deadlock != expected.deadlock {
        return Err("deadlock mismatch".into());
    }
    let properties = report["properties"]
        .as_array()
        .ok_or("properties missing")?;
    let wanted_ids: BTreeSet<String> = ["recoverable end", "terminating end", "almost_sure end"]
        .into_iter()
        .map(str::to_owned)
        .chain(
            declared_ends(m)
                .into_iter()
                .map(|e| format!("reachable end.{e}")),
        )
        .collect();
    let actual_ids = properties
        .iter()
        .filter_map(|p| p["id"].as_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    if actual_ids != wanted_ids || properties.len() != wanted_ids.len() {
        return Err("completion property inventory mismatch".into());
    }
    if report["verdict"] != json!(verdict(overall(m, expected))) {
        return Err("root completion verdict mismatch".into());
    }
    for p in properties {
        let id = p["id"].as_str().ok_or("property id missing")?;
        let t = if let Some(end) = id.strip_prefix("reachable end.") {
            expected.reachable(end)
        } else if ["recoverable end", "terminating end", "almost_sure end"].contains(&id) {
            expected.completion()
        } else {
            return Err(format!("unexpected property {id}"));
        };
        if p["verdict"] != json!(verdict(t)) {
            return Err(format!("property {id} mismatch"));
        }
        if id == "almost_sure end" {
            let (min, max) = completion_probability(expected);
            if p["probability"] != json!({"min": min, "max": max}) {
                return Err("almost-sure probability envelope mismatch".into());
            }
        } else if p.get("probability").is_some() {
            return Err("unexpected probability envelope".into());
        }
    }
    let mut terms = BTreeSet::new();
    for t in report["terminals"].as_array().ok_or("terminals missing")? {
        if t["elapsed"]["min"] != t["elapsed"]["max"]
            || t["work"]["min"] != t["work"]["max"]
            || !t["result_at"].is_null()
        {
            return Err("non-singleton or any terminal".into());
        }
        if !terms.insert((
            t["end"].as_str().ok_or("end missing")?.to_owned(),
            t["elapsed"]["min"].as_u64().ok_or("time missing")?,
            t["work"]["min"].as_u64().ok_or("work missing")?,
        )) {
            return Err("duplicate terminal".into());
        }
    }
    if terms != expected.terminals {
        return Err("terminal time/work set mismatch".into());
    }
    Ok(())
}
