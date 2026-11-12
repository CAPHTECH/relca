//! Counterfactual-query oracle retained from the independent acceptance suite.
#![allow(dead_code)]
#[path = "../../../relca-core/tests/support/completion_oracle.rs"]
mod counterfactual;
use relca_core::{contest::RuntimeStatus, engine, ir, outcome::CaseTerminalKind};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
type Cached = (bool, Vec<Value>);

fn execution_key(model: &ir::Model) -> String {
    // IR documents Agency and assumption descriptions as non-executable.
    // remove_read mutants differ only in these static lineage surfaces; do not
    // repeat every counterfactual execution for each of 128 identical edits.
    for cap in &model.capabilities {
        assert!(cap.reads.iter().all(|a| a.0 < model.actors.len()));
        assert!(
            cap.read_variables
                .iter()
                .all(|v| v.0 < model.state_variables.len())
        );
    }
    assert!(
        model
            .diagnostics
            .iter()
            .all(|d| d.severity == relca_core::diag::Severity::Warning)
    );
    let mut key = model.clone();
    key.agency = relca_core::agency::Agency::empty().into();
    key.assumptions.clear();
    key.diagnostics.clear();
    for cap in &mut key.capabilities {
        cap.reads.clear();
        cap.read_variables.clear();
    }
    format!("{key:?}")
}

fn independent(model: &ir::Model) -> Cached {
    static CACHE: OnceLock<Mutex<BTreeMap<String, Cached>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let key = execution_key(model);
    if let Some(found) = cache.lock().unwrap().get(&key).cloned() {
        return found;
    }
    let out = engine::explore(model);
    let complete = !out.truncated
        && out.state_range_violations.is_empty()
        && !out.diagnostics.iter().any(|d| d.code.starts_with("U-"))
        && !out.case_outcomes.as_ref().is_some_and(|cs| {
            cs.iter().any(|c| {
                c.terminals
                    .iter()
                    .any(|t| t.kind == CaseTerminalKind::CutOff)
            })
        })
        && !out.contest_runtime.as_ref().is_some_and(|rs| {
            rs.iter().any(|r| {
                r.alternatives
                    .iter()
                    .any(|a| a.status == RuntimeStatus::Unresolved)
            })
        });
    let warnings = if complete {
        counterfactual::warnings(model)
    } else {
        vec![]
    };
    let found = (complete, warnings);
    cache.lock().unwrap().insert(key, found.clone());
    found
}

#[test]
fn execution_cache_does_not_equate_changed_operational_capacity() {
    let source = include_str!("../../../../fixtures/serial-time/model.relca");
    let model = relca_core::lower::lower(&relca_core::syntax::parse(source).unwrap()).unwrap();
    let mut metadata = model.clone();
    metadata.agency.as_derived_mut().unwrap().rules = "different static description".into();
    metadata.assumptions.clear();
    assert_eq!(execution_key(&model), execution_key(&metadata));
    metadata.actors[0].capacity = Some(99);
    assert_ne!(execution_key(&model), execution_key(&metadata));
}

pub fn check(report: &Value, model: &ir::Model, simulation: bool) -> Result<Vec<Value>, String> {
    let runtime = report["bounds"]["cases"].is_u64();
    let (complete, wanted) = if !runtime || simulation {
        (false, vec![])
    } else {
        independent(model)
    };
    let unchecked = report["unchecked"]
        .as_array()
        .map_or(0, |a| a.iter().filter(|w| *w == "unreachable").count());
    if unchecked != usize::from(runtime && (simulation || !complete)) {
        return Err("independent completion coverage marker".into());
    }
    let actual: Vec<_> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("W-UNREACH-"))
        })
        .collect();
    if actual.len() != wanted.len() {
        return Err(format!(
            "independent completion inventory {}!={}",
            actual.len(),
            wanted.len()
        ));
    }
    for (a, h) in actual.iter().zip(&wanted) {
        if a["code"] != h["code"]
            || a["severity"] != "warning"
            || a["span"] != h["span"]
            || !["process", "target"].iter().all(|key| {
                a["message"]
                    .as_str()
                    .unwrap()
                    .contains(h[*key].as_str().unwrap())
            })
            || h["code"] == "W-UNREACH-0"
                && !a["message"]
                    .as_str()
                    .unwrap()
                    .contains(h["reason"].as_str().unwrap())
        {
            return Err("independent completion process/target/reason/span/order".into());
        }
    }
    Ok(wanted)
}
