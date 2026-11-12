//! Link from lib.rs after the ADR0035 IR shape exists. Field spelling follows
//! the approved plan; mechanical adaptation may not change any hand oracle.
use crate::{engine, ir, lower, reference, syntax};
fn model(name: &str) -> ir::Model {
    let s = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap();
    syntax::parse(&s).and_then(|a| lower::lower(&a)).unwrap()
}
fn refused(m: &ir::Model) {
    let e = engine::explore(m);
    let r = reference::explore(m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    for o in [&e, &r.outcome] {
        assert!(o.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
        assert!(o.terminals.is_empty());
    }
}

#[test]
fn aset_public_ir_set_shape_bits_and_parameter_context_are_validated_independently() {
    let good = model("aset-dup");
    let set = good
        .state_variables
        .iter()
        .position(|v| v.name == "keys")
        .unwrap();
    let cap = good
        .capabilities
        .iter()
        .position(|c| c.name == "post")
        .unwrap();
    for fault in 0..12 {
        let mut m = good.clone();
        match fault {
            0 => m.state_variables[set].case_set_size = Some(65),
            1 => m.state_variables[set].case_set_size = Some(2),
            2 => m.state_variables[set].initial = ir::InitialValue::Fixed(ir::StateValue::Set(2)),
            3 => {
                m.capabilities[cap].atomic.as_mut().unwrap().condition = ir::Cond::Contains {
                    set: ir::StateVarId(set),
                    element: ir::Element::Parameter(usize::MAX),
                }
            }
            4 => {
                m.capabilities[cap].atomic.as_mut().unwrap().condition = ir::Cond::Contains {
                    set: ir::StateVarId(0),
                    element: ir::Element::Parameter(0),
                }
            }
            5 => {
                let commit = m.capabilities[cap].atomic.as_ref().unwrap().commit;
                m.capabilities[cap].atomic.as_mut().unwrap().reject = commit;
            }
            6 => m.capabilities[cap].atomic.as_mut().unwrap().commit = ir::ValueId(usize::MAX),
            7 => m.capabilities[cap].atomic.as_mut().unwrap().writes.clear(),
            8 => {
                let write = m.capabilities[cap].atomic.as_ref().unwrap().writes[0];
                m.capabilities[cap]
                    .atomic
                    .as_mut()
                    .unwrap()
                    .writes
                    .push(write);
            }
            9 => {
                m.capabilities[cap].atomic.as_mut().unwrap().writes[0].source =
                    ir::WriteSource::InsertCase {
                        parameter: usize::MAX,
                    }
            }
            10 => m.capabilities[cap].results.clear(),
            11 => m.state_variables[set].initial = ir::InitialValue::Fixed(ir::StateValue::Set(1)),
            _ => unreachable!(),
        }
        refused(&m);
    }
}

#[test]
fn aset_ir_atomic_writer_ownership_and_anonymous_executor_are_not_trusted_from_lowering() {
    let mut m = model("aset-dup");
    let id = m
        .state_variables
        .iter()
        .position(|v| v.name == "keys")
        .unwrap();
    m.state_variables[id].kind = ir::StateKind::ActorObserved(ir::ActorId(usize::MAX));
    refused(&m);
    let mut m = model("aset-executor");
    m.actors[0].executors = None;
    refused(&m);
}
