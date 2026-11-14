//! Private conversions for the independently fixed acceptance population.
use super::*;
use serde_json::{Value, json};

pub(super) fn adr0072_run(r: &agency_viewpoint_limit_acceptance::Request) -> Value {
    let ast = match crate::syntax::parse(&r.source) {
        Ok(ast) => ast,
        Err(diagnostics) => return json!({"status":"rejected", "diagnostics":diagnostics}),
    };
    let mut m = match crate::lower::lower_with_agency_limit(
        &ast,
        r.configuration.as_deref(),
        r.scenario.as_deref(),
        if r.reference && r.scale > MAX_LIMIT_SCALE {
            None
        } else {
            Some(r.scale)
        },
    ) {
        Ok(m) => m,
        Err(diagnostics) => return json!({"status":"rejected", "diagnostics":diagnostics}),
    };
    if r.domain_probe {
        return viewpoints::domain_probe(&m);
    }
    if r.budget_profile {
        return json!({"status":"ok", "budgets":budgets_for_model(&m).unwrap(), "degradation_work":DEGRADATION_WORK});
    }
    if r.revalidate {
        let validation = crate::responsibility::validate(&m);
        return json!({"status":"ok", "responsibility_valid":validation.is_ok(), "validation_error":validation.err(),
            "model_scale":m.agency_limit_scale, "agency_derived":m.agency.as_derived().is_some()});
    }
    let mut a = m
        .agency
        .as_derived()
        .expect("main derivation for viewpoint input")
        .clone();
    if r.corruption.as_deref() == Some("call_capability") {
        fn corrupt(block: &mut ir::Block, id: ir::BindingId) {
            for s in block {
                match &mut s.kind {
                    ir::Stmt::Call(c) if c.binding == id => {
                        c.capability = ir::CapabilityId(usize::MAX)
                    }
                    ir::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        corrupt(then_body, id);
                        corrupt(else_body, id);
                    }
                    _ => {}
                }
            }
        }
        let id = ir::BindingId(
            m.process
                .bindings
                .iter()
                .position(|b| b.name == "j1")
                .unwrap(),
        );
        corrupt(&mut m.process.body, id);
    }
    if r.corruption.as_deref() == Some("origin_actor") {
        let mut origin = a.origin.last().unwrap().clone();
        origin.direct = vec!["Doc".into()];
        a.origin.push(origin);
    }
    let mut limits = [DEFAULT; 3];
    let mut effects = DEFAULT;
    if let Some(block) = &r.block {
        let l = match block.as_str() {
            "observation" => &mut limits[0],
            "framing" => &mut limits[1],
            "retention" => &mut limits[2],
            "effects" => &mut effects,
            _ => panic!("unknown block"),
        };
        match r.counter.as_str() {
            "work" => l.work = r.limit,
            "cells" => l.cells = r.limit,
            "contexts" => l.contexts = r.limit,
            "paths" => l.paths = r.limit,
            "depth" => l.depth = r.limit,
            _ => panic!("unknown counter"),
        }
    }
    for l in limits.iter_mut().chain(std::iter::once(&mut effects)) {
        *l = l.scaled(r.scale).unwrap();
    }
    let (v, diagnostics) = if r.reference {
        viewpoints::with_limits_and_warnings_reference(&m, &a, limits, effects)
    } else {
        viewpoints::with_limits_and_warnings(&m, &a, limits, effects)
    };
    json!({"status":"ok", "viewpoints":v, "viewpoints_bytes":serde_json::to_string(&v).unwrap(), "diagnostics":diagnostics})
}
