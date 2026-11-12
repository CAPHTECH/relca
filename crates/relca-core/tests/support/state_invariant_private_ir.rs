//! Implementation-side linkage after ContractKind::StateInvariant is defined.
//! Include from lib.rs under cfg(test); not a silently skipped integration test.
use crate::{engine, ir, lower, reference, syntax};
fn model() -> ir::Model {
    lower::lower(
        &syntax::parse(include_str!(
            "../../../../fixtures/sinv-approval/model.relca"
        ))
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn state_invariant_public_ir_rejects_each_invalid_child_on_both_sides_before_execution() {
    let base = model();
    let truth = base.types[base.state_variables[0].base.0]
        .members
        .iter()
        .position(|v| v == "true")
        .unwrap();
    let f = ir::Cond::StateEq(ir::StateMatch {
        variable: ir::StateVarId(0),
        value: ir::StateValue::Value(ir::ValueId(truth)),
    });
    let invalid = [
        ir::Cond::Eq(ir::ResultMatch {
            binding: ir::BindingId(0),
            value: ir::ValueId(0),
        }),
        ir::Cond::StateEq(ir::StateMatch {
            variable: ir::StateVarId(usize::MAX),
            value: ir::StateValue::None,
        }),
        ir::Cond::StateEq(ir::StateMatch {
            variable: ir::StateVarId(0),
            value: ir::StateValue::Value(ir::ValueId(usize::MAX)),
        }),
        ir::Cond::StateEq(ir::StateMatch {
            variable: ir::StateVarId(0),
            value: ir::StateValue::None,
        }),
        ir::Cond::IntCompare {
            variable: ir::StateVarId(0),
            operator: ir::IntComparison::Le,
            value: 1,
        },
    ];
    for bad in invalid {
        for left in [false, true] {
            for nested in [false, true] {
                let mut m = base.clone();
                let bad = if nested {
                    ir::Cond::Or(vec![f.clone(), bad.clone()])
                } else {
                    bad.clone()
                };
                m.contracts[0].kind = ir::ContractKind::StateInvariant {
                    antecedent: Some(if left { bad.clone() } else { f.clone() }),
                    condition: if left { f.clone() } else { bad },
                };
                // f is false initially. It must not hide an invalid right-hand child.
                let e = engine::explore(&m);
                assert!(e.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
                let r = reference::explore(&m);
                assert_eq!(r.stop, reference::StopReason::SemanticAbort);
            }
        }
    }
}
