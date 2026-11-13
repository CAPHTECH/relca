//! Independent ADR0054 candidate assembly. E comes from an exhaustive IR walk,
//! never from effect.e. U derivation itself is unchanged and is an input here.
#![allow(dead_code)]
use relca_core::{agency, diag, ir};
use serde_json::json;
use std::collections::BTreeSet;

fn effects(body: &ir::Block, m: &ir::Model, out: &mut BTreeSet<String>) {
    fn call(c: &ir::Call, m: &ir::Model, out: &mut BTreeSet<String>) {
        let cap = &m.capabilities[c.capability.0];
        if cap.commits {
            out.insert(m.actors[cap.actor.0].name.clone());
        }
    }
    for statement in body {
        match &statement.kind {
            ir::Stmt::LoopControl(_) => panic!("bounded repetition is outside this oracle"),
            ir::Stmt::Query(_, _)
            | ir::Stmt::ResolveEffect { .. }
            | ir::Stmt::StopReceiptUnknown => panic!("receipt recovery is outside this oracle"),
            ir::Stmt::WaitUntil(_) => {
                panic!("responsibility defaults oracle does not handle condition waits")
            }
            ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => call(c, m, out),
            ir::Stmt::Fork { branches, join, .. } => {
                match join {
                    ir::Join::All => {}
                    ir::Join::Any { .. } => {}
                }
                for c in branches {
                    call(c, m, out);
                }
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                effects(then_body, m, out);
                effects(else_body, m, out);
            }
            ir::Stmt::Wait(_) | ir::Stmt::End(_) => {}
        }
    }
}
pub fn expected(m: &ir::Model) -> Vec<diag::Diagnostic> {
    let mut actors = BTreeSet::new();
    for row in &m.agency.as_derived().unwrap().refusal {
        if row.reachable {
            actors.insert(row.effect.split('.').next().unwrap().to_owned());
            for p in row.judgements.iter().filter_map(agency::Judgement::process) {
                if p.stops != agency::Stops::None {
                    actors.insert(p.capability.split('.').next().unwrap().to_owned());
                }
            }
        }
    }
    if let Some(view) = &m.agency.as_derived().unwrap().viewpoints {
        // The public field's private enum is serialized to inspect only the
        // unchanged status/U surface. The new E candidate set above is IR-only.
        let block = json!(view.effects);
        if block["status"] == "derived" {
            effects(&m.process.body, m, &mut actors);
            for row in block["rows"].as_array().unwrap() {
                for r in row["state_refusers"].as_array().unwrap() {
                    actors.insert(r["actor"].as_str().unwrap().to_owned());
                }
                for r in row["u"].as_array().into_iter().flatten() {
                    if r["kind"] == "rule" {
                        actors.insert(r["actor"].as_str().unwrap().to_owned());
                    } else {
                        assert!(r["actors"].is_array(), "unknown process-U carrier");
                        for a in r["actors"].as_array().unwrap() {
                            actors.insert(a.as_str().unwrap().to_owned());
                        }
                    }
                }
            }
        }
    }
    m.actors
        .iter()
        .filter(|a| {
            actors.contains(&a.name)
                && !a.executors.as_ref().is_some_and(|people| {
                    m.responsibility
                        .as_ref()
                        .is_some_and(|r| r.duties.iter().any(|d| people.contains(&d.person)))
                })
        })
        .map(|a| diag::Diagnostic {
            code: diag::W_RESP_0.into(),
            severity: diag::Severity::Warning,
            message: format!("{} has no declared responsibility", a.name),
            span: Some(a.span),
        })
        .collect()
}

/// ADR 0054, "取り込み前レビューで追加する開示": an incomplete effects
/// derivation is not an empty responsibility candidate set.
pub fn expected_disclosures(m: &ir::Model) -> Vec<String> {
    let effects = json!(m.agency.as_derived().unwrap().viewpoints)
        .get("effects")
        .cloned()
        .unwrap_or_default();
    if effects["status"] == "not_derived" {
        vec![format!(
            "responsibility_default_candidates: process {}: U/E not_derived ({})",
            m.process.name,
            effects["reason"].as_str().expect("closed effects reason")
        )]
    } else {
        vec![]
    }
}

/// Preserve a closed, exact K4 warning/disclosure contract in older acceptance
/// gates (ADR 0054 "範囲", "意味", and the pre-intake disclosure section).
pub fn assert_report(m: &ir::Model, actual: &serde_json::Value) {
    let warnings: Vec<_> = actual["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-RESP-0")
        .collect();
    assert_eq!(
        json!(warnings),
        json!(expected(m)),
        "independent IR responsibility rows"
    );
    let notices: Vec<_> = actual["agency"]["unchecked"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|s| s.starts_with("responsibility_default_candidates:"))
        .collect();
    assert_eq!(
        notices,
        expected_disclosures(m),
        "independent IR candidate disclosure"
    );
}

/// ADR 0054 changes only this selected Report's K4 diagnostic inventory. Keep
/// the generic exact-array matcher and every non-K4 hand expectation intact.
/// This function never receives the actual Report or reads its warning vector.
pub fn with_diagnostics(m: &ir::Model, hand: &serde_json::Value) -> serde_json::Value {
    let mut hand = hand.clone();
    if let Some(rows) = hand
        .get_mut("diagnostics")
        .and_then(serde_json::Value::as_array_mut)
    {
        rows.retain(|row| row["code"] != "W-RESP-0");
        // Lower's responsibility warnings precede runtime deadlock/budget and
        // unreachable warnings. Do not infer their position from live output.
        let at = rows
            .iter()
            .position(|row| {
                let code = row["code"].as_str().unwrap();
                code.starts_with("W-UNREACH-") || matches!(code, "W-DEADLOCK-0" | "W-BUDGET-0")
            })
            .unwrap_or(rows.len());
        rows.splice(at..at, expected(m).into_iter().map(|row| json!(row)));
    }
    hand
}
