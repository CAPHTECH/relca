use super::*;
fn model() -> ir::Model {
    crate::lower::lower(
        &crate::syntax::parse(include_str!("../../../tests/data/sensitivity/base.relca")).unwrap(),
    )
    .unwrap()
}
#[test]
fn actor_grain_is_not_an_operational_contract_or_parameter_instruction() {
    let base = include_str!("../../../tests/data/sensitivity/base.relca");
    let project = |grain: &str| {
        let source = base.replace(
            "actor Check { capacity 1",
            &format!("actor Check {{ grain {grain}; capacity 1"),
        );
        Contract::from_model(&crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap())
            .unwrap()
    };
    assert!(
        project("person")
            .differences(&project("service"))
            .is_empty()
    );
}
#[test]
fn declaration_table_order_does_not_change_metrics_or_policy_sets() {
    let source=include_str!("../../../tests/data/sensitivity/abort.relca").replace("  bounds", "  metrics {window 2min; currency \"JPY\"; rate A per_minute 2; rate B per_minute 3}\n  policy fifo tie_by case_id then call_order assign first_idle\n  bounds");
    let a = "actor A { capacity 1; capability c() -> T { duration fixed 1min; outcomes kernel K; writes y } }";
    let b = "actor B { capacity 1; capability c() -> T { duration fixed 1min; result nondet { first, second } } }";
    assert_eq!(source.matches(a).count(), 1);
    assert_eq!(source.matches(b).count(), 1);
    let swapped = source.replace(a, "@A@").replace(b, a).replace("@A@", b);
    let project = |s: &str| {
        Contract::from_model(&crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap())
            .unwrap()
    };
    let differences = project(&source).differences(&project(&swapped));
    assert!(
        differences.is_empty(),
        "accidental table IDs changed contracts: {differences:?}"
    );
}
#[test]
fn quality_denominators_and_metrics_are_part_of_the_question() {
    let mut m = model();
    m.quality = Some(ir::QualitySpec {
        properties: vec![ir::QualityProperty {
            name: "wrong".into(),
            target: ir::QualityTarget::Ends(vec![ir::EndId(0)]),
            within: Some(2),
        }],
        metrics: vec![ir::CohortMetric {
            name: "rate".into(),
            numerator: 0,
            denominator: ir::Denominator::Arrivals,
        }],
        quantiles: vec![],
    });
    let base = Contract::from_model(&m).unwrap();
    let mut changed = m.clone();
    changed.quality.as_mut().unwrap().metrics[0].denominator = ir::Denominator::Property(0);
    assert!(
        base.differences(&Contract::from_model(&changed).unwrap())
            .contains(&"/quality".into())
    );
    changed = m.clone();
    changed.capabilities[0].duration += 2;
    assert!(
        base.differences(&Contract::from_model(&changed).unwrap())
            .is_empty()
    );
    m.metrics = Some(ir::MetricsSpec {
        breakdown_waits: true,
        window: 4,
        currency: "JPY".into(),
        actor_rates: vec![Some(7); m.actors.len()],
        start_rates: vec![Some(3); m.capabilities.len()],
        required_actors: vec![],
        required_capabilities: vec![],
        span: m.process.span,
    });
    let base = Contract::from_model(&m).unwrap();
    m.metrics.as_mut().unwrap().actor_rates[0] = Some(8);
    assert!(
        base.differences(&Contract::from_model(&m).unwrap())
            .contains(&"/metrics".into())
    );
}

#[test]
fn finite_contract_deadlines_are_not_duration_parameters() {
    let source = include_str!("../../../../../fixtures/contract-response/model.relca");
    let lower = |s: &str| crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap();
    let m = lower(source);
    let c = Contract::from_model(&m).unwrap();
    let changed = lower(&source.replace("duration fixed 5min", "duration fixed 6min"));
    assert!(
        c.differences(&Contract::from_model(&changed).unwrap())
            .is_empty()
    );
    let changed = lower(&source.replace("by 5min", "by 4min"));
    assert!(
        c.differences(&Contract::from_model(&changed).unwrap())
            .contains(&"/contracts".into())
    );
}

#[test]
fn integer_effects_ranges_and_parameter_operands_are_named_contracts() {
    let source = r#"model IntegerGate version "0.2" {
      type R = enum { ok }
      observed case { count: 0..2 = 0 }
      actor Work { capacity 1
        capability run() -> R {duration fixed 1min;result nondet {ok};writes count;finish {count += 1}}
      }
      process Flow {x = Work.run();end done}
      cut C {include [Work]}
      bounds {cases 1;live_tasks 1;clock 4min}
      verify probability reached count >= 1 >= 1
    }"#;
    let lower = |s: &str| crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap();
    let c = Contract::from_model(&lower(source)).unwrap();
    let changed =
        Contract::from_model(&lower(&source.replace("count += 1", "count += 2"))).unwrap();
    assert!(c.differences(&changed).contains(&"/capabilities".into()));
    let changed = Contract::from_model(&lower(&source.replace("0..2", "0..3"))).unwrap();
    assert!(c.differences(&changed).contains(&"/state".into()));
}

#[test]
fn grants_and_person_identity_are_not_accidental_table_ids() {
    let mut m = model();
    let span = m.process.span;
    m.people = vec![
        ir::Person {
            name: "alice".into(),
            span,
        },
        ir::Person {
            name: "bob".into(),
            span,
        },
    ];
    m.actors[0].executors = Some(vec![ir::PersonId(0), ir::PersonId(1)]);
    m.capabilities[0].permission = Some(vec![ir::PersonId(0)]);
    m.authorities = vec![ir::Authority {
        name: "Permit".into(),
        basis: "reviewed policy".into(),
        grants: vec![ir::Grant {
            capability: ir::CapabilityId(0),
            persons: vec![ir::PersonId(0)],
        }],
        rule_grants: vec![],
        under: None,
        issued_by: None,
        not_by: vec![],
        restrictions: Default::default(),
        span,
    }];
    let c = Contract::from_model(&m).unwrap();
    let mut changed = m.clone();
    changed.capabilities[0].permission = Some(vec![ir::PersonId(1)]);
    assert!(
        c.differences(&Contract::from_model(&changed).unwrap())
            .contains(&"/capabilities".into())
    );
    let mut reordered = m.clone();
    reordered.people.swap(0, 1);
    reordered.capabilities[0].permission = Some(vec![ir::PersonId(1)]);
    reordered.actors[0].executors = Some(vec![ir::PersonId(1), ir::PersonId(0)]);
    reordered.authorities[0].grants[0].persons = vec![ir::PersonId(1)];
    assert!(
        c.differences(&Contract::from_model(&reordered).unwrap())
            .is_empty()
    );
    m.authorities[0].restrictions.expires = Some(2);
    assert!(
        c.differences(&Contract::from_model(&m).unwrap())
            .contains(&"/authorities".into())
    );
}
