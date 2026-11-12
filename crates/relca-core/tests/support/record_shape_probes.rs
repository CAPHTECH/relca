//! API-dependent continuation of T24, linked from tests/records.rs now that
//! the implementation adds EnumType.record / RecordShape / RecordField.
//! It does not compile until that API exists.
//! Existing-public-field shape corruptions already run in records.rs after parse.
use super::hand;
use relca_core::{engine, ir::*, reference};

fn rejected(m: &Model, fault: &str) {
    let e = engine::explore(m);
    assert!(e.diagnostics.iter().any(|d| d.code == "U-SEM-0"), "{fault}");
    assert!(e.properties.is_empty() && e.terminals.is_empty(), "{fault}");
    assert_eq!(
        reference::explore(m).stop,
        reference::StopReason::SemanticAbort,
        "{fault}"
    );
}

#[test]
fn t24_shape_field_identity_names_and_domains_are_independently_guarded() {
    let good = hand::model(&hand::source("rec-product"));
    let id = good.types.iter().position(|t| t.name == "Pair").unwrap();
    let color = good.types.iter().position(|t| t.name == "Color").unwrap();
    let shape = good.types[id].record.as_ref().unwrap();
    assert_eq!(shape.fields.len(), 2);
    assert_eq!(shape.fields[0].name, "color");
    assert_eq!(shape.fields[0].ty, TypeId(color));
    assert_eq!(shape.fields[1].name, "flag");
    let mut m = good.clone();
    m.types[id].record.as_mut().unwrap().fields[0].ty = TypeId(usize::MAX);
    rejected(&m, "out-of-range field TypeId must not panic");
    let mut m = good.clone();
    m.types[id].record.as_mut().unwrap().fields[0].ty = TypeId(id);
    rejected(&m, "recursive field");
    let mut m = good.clone();
    m.types[id].record.as_mut().unwrap().fields[0].name.clear();
    rejected(&m, "empty field name");
    let mut m = good.clone();
    m.types[id].record.as_mut().unwrap().fields[1].name = "color".into();
    rejected(&m, "duplicate field name");
    let mut m = good.clone();
    m.types[id].record.as_mut().unwrap().fields.clear();
    rejected(&m, "zero fields");
    let mut m = good.clone();
    m.types[id].record.as_mut().unwrap().fields.swap(0, 1);
    rejected(&m, "field order disagrees with canonical members");
}

#[test]
fn t24_record_latent_state_is_rejected_by_each_backend() {
    let good = hand::model(&hand::source("rec-prob-field"));
    let state = good
        .state_variables
        .iter()
        .position(|v| v.name == "pair")
        .unwrap();
    let mut m = good.clone();
    m.state_variables[state].kind = StateKind::LatentCase;
    rejected(&m, "latent record state");
    // ActorObserved owner construction, Distribution and kernel-given payload
    // constructors must follow the actual integrated ADR0035/0037 API. Their
    // expected rejection is already fixed in record_obligations.md, not guessed
    // from a future public type spelling here.
}

#[test]
fn one_grouped_inline_position_is_shared_but_two_written_positions_are_nominal() {
    let s = r#"model Grouped version "0.2" {
      type R=record { f,g:enum {a,b}; h:enum {a,b} }
      process P {end done}
      bounds {cases 1;live_tasks 1;clock 1min}
      verify safety no_resource_overcommit
    }"#;
    let m = hand::model(s);
    let record = m.types.iter().find(|t| t.name == "R").unwrap();
    assert_eq!(record.members.len(), 8);
    let fields = &record.record.as_ref().unwrap().fields;
    assert_eq!(
        fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        vec!["f", "g", "h"]
    );
    assert_eq!(fields[0].ty, fields[1].ty);
    assert_ne!(fields[1].ty, fields[2].ty);
}
