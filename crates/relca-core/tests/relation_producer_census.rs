//! Independent lexical IR oracle calibrated to hand values, not old output bytes.
use relca_core::{ast, ir, lower, syntax};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[path = "support/relation_producer_read_oracle.rs"]
mod read_oracle;
use read_oracle::inventory;
#[path = "support/relation_producers.rs"]
mod hand;
fn census(src: &str, a: &str, b: &str) -> Value {
    census_with_scenario(src, a, b, None)
}
fn census_with_scenario(src: &str, a: &str, b: &str, scenario: Option<&str>) -> Value {
    let Ok(ast) = syntax::parse(src) else {
        return Value::Null;
    };
    let (Ok(a), Ok(b)) = (
        lower::lower_configuration_with_scenario(&ast, a, scenario),
        lower::lower_configuration_with_scenario(&ast, b, scenario),
    ) else {
        return Value::Null;
    };
    let mut annotations = vec![];
    for i in &b.intent {
        if let ast::Intent::Preserve {
            selector, on: None, ..
        } = i
        {
            let selected: BTreeSet<_> = match selector {
                ast::PropertySelector::Exact(s) => [s.value.clone()].into_iter().collect(),
                ast::PropertySelector::Contract(n) => b
                    .model
                    .contracts
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.name.as_deref() == Some(&n.value))
                    .map(|(i, c)| c.property_id(ir::ClauseId(i)))
                    .chain(
                        b.model
                            .contests()
                            .iter()
                            .filter(|c| c.name.as_deref() == Some(&n.value))
                            .map(|c| c.property_id()),
                    )
                    .collect(),
            };
            // The independent old verification inventory, not contracts followed
            // by contests, determines property order (contest precedes response).
            let properties: Vec<_> = b
                .model
                .verify
                .iter()
                .filter_map(|v| match v.property {
                    ir::Property::Contract(c) => Some(b.model.contracts[c.0].property_id(c)),
                    ir::Property::Contest(c) => Some(b.model.contests()[c].property_id()),
                    _ => None,
                })
                .filter(|id| selected.contains(id))
                .collect();
            annotations.push(json!({"kind":"preserve","selector":selector.value(),"global":true,"properties":properties}));
        }
    }
    annotations.sort_by_key(|a| a["selector"].as_str().unwrap().to_owned());
    let (before, after) = (inventory(&a.model), inventory(&b.model));
    let rows:Vec<_>=before.iter().filter_map(|(name,from)|{
        let to=after.get(name)?;
        if from==to{return None}
        let mut row=json!({"kind":"relation","target":name,"change":"modified","reason":null,"loses":null,"from":{"producer":from},"to":{"producer":to}});
        if !annotations.is_empty(){row["annotations"]=json!(annotations)}
        Some(row)
    }).collect();
    json!({"from":before,"to":after,"rows":rows})
}
#[test]
fn census_calibrated_to_hand_not_production_diff() {
    let src = hand::source(hand::BEFORE, hand::AFTER, "");
    assert_eq!(
        census(&hand::join_source(false, false, ""), "From", "To")["rows"],
        json!([
            hand::row("spare", "A.two", "A.one"),
            hand::row("x", "A.one", "A.two")
        ])
    );
    for (same, all) in [(true, false), (false, true)] {
        assert_eq!(
            census(&hand::join_source(same, all, ""), "From", "To")["rows"],
            json!([])
        );
    }
    assert_eq!(
        census(&src, "From", "To")["rows"],
        json!([hand::row("x", "A.one", "A.two")])
    );
    assert_eq!(
        census(&hand::source(hand::BEFORE, hand::BEFORE, ""), "From", "To")["rows"],
        json!([])
    );
    let invoice = include_str!("../../../examples/invoice/phase1.relca");
    for (a, b, n) in [
        ("AsIs", "ToBeA", 1),
        ("AsIs", "ToBeP", 0),
        ("ToBeP", "ToBeA", 1),
    ] {
        let c = census(invoice, a, b);
        assert_eq!(c["rows"].as_array().unwrap().len(), n);
        if n == 1 {
            assert_eq!(c["rows"][0]["target"], "entry");
            assert_eq!(c["rows"][0]["from"]["producer"], "Clerk.enter");
            assert_eq!(c["rows"][0]["to"]["producer"], "Extractor.extract");
        }
    }
    let annotated = census(
        include_str!("../../../fixtures/diff2-invoice/model.relca"),
        "AsIs",
        "ToBeA",
    );
    let ps = annotated["rows"][0]["annotations"][0]["properties"]
        .as_array()
        .unwrap();
    assert_eq!(
        &ps[6..],
        &[
            json!("contract InvoiceControls #7 contest"),
            json!("contract InvoiceControls #7 contest_response")
        ]
    );
}
#[test]
fn independent_oracle_classifies_every_ir_variant_and_not_tie_ids() {
    // Artificial IR tests the lexical oracle only, not lowering/execution of
    // these sentinel state IDs. Hand source supplies real result definitions.
    let parsed = syntax::parse(&hand::join_source(false, false, "")).unwrap();
    let mut model = lower::lower_configuration(&parsed, "From").unwrap().model;
    assert_eq!(
        inventory(&model),
        BTreeMap::from([
            ("spare".into(), "A.two".into()),
            ("x".into(), "A.one".into())
        ])
    );
    let x = model
        .process
        .bindings
        .iter()
        .position(|b| b.name == "x")
        .unwrap();
    let x = ir::BindingId(x);
    let ir::Stmt::Fork { join, .. } = &mut model.process.body[0].kind else {
        panic!("fork")
    };
    let ir::Join::Any { matches, .. } = join else {
        panic!("any")
    };
    matches.clear(); // tie By still names both branches: not a value read.
    assert!(inventory(&model).is_empty());
    let ir::Stmt::Fork { join, .. } = &mut model.process.body[0].kind else {
        panic!("fork")
    };
    *join = ir::Join::All;
    let no_reads = vec![
        ir::Cond::StateEq(ir::StateMatch {
            variable: ir::StateVarId(0),
            value: ir::StateValue::Integer(0),
        }),
        ir::Cond::IntCompare {
            variable: ir::StateVarId(0),
            operator: ir::IntComparison::Eq,
            value: 0,
        },
        ir::Cond::Contains {
            set: ir::StateVarId(0),
            element: ir::Element::CurrentCase,
        },
        ir::Cond::Contains {
            set: ir::StateVarId(0),
            element: ir::Element::Parameter(0),
        },
    ];
    let mut statement = model.process.body[1].clone(); // End; keep real span.
    statement.kind = ir::Stmt::Wait(1);
    model.process.body.push(statement.clone());
    statement.kind = ir::Stmt::If {
        condition: ir::Cond::Or(no_reads.clone()),
        then_body: vec![],
        else_body: vec![],
    };
    model.process.body.push(statement.clone());
    assert!(inventory(&model).is_empty());
    statement.kind = ir::Stmt::If {
        condition: ir::Cond::And(vec![
            ir::Cond::Or(no_reads),
            ir::Cond::Eq(ir::ResultMatch {
                binding: x,
                value: ir::ValueId(0),
            }),
        ]),
        then_body: vec![],
        else_body: vec![],
    };
    model.process.body.push(statement);
    assert_eq!(
        inventory(&model),
        BTreeMap::from([("x".into(), "A.one".into())])
    );
    // Sequential Call.args reads the result; CURRENT_CASE never becomes a producer.
    let parsed = syntax::parse(&hand::source(hand::BEFORE, hand::AFTER, "")).unwrap();
    let mut model = lower::lower_configuration(&parsed, "From").unwrap().model;
    assert_eq!(
        inventory(&model),
        BTreeMap::from([("x".into(), "A.one".into())])
    );
    let ir::Stmt::Call(reader) = &mut model.process.body[2].kind else {
        panic!("reader")
    };
    reader.args = vec![ir::CURRENT_CASE];
    assert!(inventory(&model).is_empty());
}
