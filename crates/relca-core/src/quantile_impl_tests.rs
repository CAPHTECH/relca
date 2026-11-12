use crate::{
    engine, quantile_private_contract as contract, quantile_test_support as input, reference,
};
use serde_json::json;

#[test]
fn forged_ir_contract_both_independent_entrances() {
    let base = input::model();
    contract::ir_contract(|row| {
        let m = input::forge(&base, row);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        json!({"engine_unsupported":e.diagnostics.iter().any(|d|d.code==crate::diag::U_SEM_0),"reference_stop":format!("{:?}",r.stop)})
    });
}
#[test]
fn engine_numeric_isolation_contract() {
    let m = input::model();
    let mut normal = engine::explore(&m);
    let census = engine::quantile_private_census(&m);
    contract::numeric_isolation_contract(|bits| {
        crate::quality::TEST_BITS.with(|b| b.set(Some(bits)));
        let mut limited = engine::explore(&m);
        let limited_census = engine::quantile_private_census(&m);
        crate::quality::TEST_BITS.with(|b| b.set(None));
        let reason = limited
            .quality
            .as_ref()
            .unwrap()
            .quantiles
            .as_ref()
            .unwrap()[0]["value"]["reason"]
            .clone();
        normal.quality.as_mut().unwrap().quantiles = None;
        limited.quality.as_mut().unwrap().quantiles = None;
        json!({"quantile_reason":reason,"old_outcome_unchanged":normal==limited,"old_diagnostics_unchanged":normal.diagnostics==limited.diagnostics,"old_census_unchanged":census==limited_census})
    });
}
#[test]
fn reference_numeric_isolation_contract() {
    reference::quantile_numeric_test();
}
#[test]
fn engine_sparse_graph_contract() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/qtl-static/model.relca"
    ))
    .unwrap();
    let mut m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    contract::sparse_graph_contract(|clock| {
        m.bounds.clock = clock;
        crate::quality::GRAPH_BUILDS.with(|n| n.set(0));
        let out = engine::explore(&m);
        json!({"graph_builds":crate::quality::GRAPH_BUILDS.with(|n|n.get()),"quantile":out.quality.unwrap().quantiles.unwrap()[0]["value"]["lower"]})
    });
}
#[test]
fn reference_sparse_graph_contract() {
    reference::quantile_sparse_test();
}

#[test]
fn quantile_empty_simulation_wire_is_reasoned_and_closed() {
    let m = input::model();
    for abort in [false, true] {
        let e = crate::quality::simulation(&m, &[], abort);
        let r = reference::quality_empty(&m, abort);
        assert_eq!(e, r);
        let wire = serde_json::to_string(&e).unwrap();
        assert_eq!(
            serde_json::from_str::<crate::quality::Quality>(&wire).unwrap(),
            e
        );
    }
}

#[test]
#[ignore = "explicit 120-case by 1000-replication performance observation"]
fn quantile_simulation_performance_probe() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/qtl-nondeterminism/model.relca"
    ))
    .unwrap();
    let mut m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    m.bounds.cases = 120;
    m.bounds.clock = 400;
    m.arrivals = Some(vec![0; 120]);
    m.simulation.as_mut().unwrap().replications = 1000;
    let mut control = m.clone();
    control.quality.as_mut().unwrap().quantiles.clear();
    let start = std::time::Instant::now();
    let baseline = engine::simulate(&control);
    let without = start.elapsed();
    let start = std::time::Instant::now();
    let mut observed = engine::simulate(&m);
    let with = start.elapsed();
    let quantiles = observed
        .quality
        .as_ref()
        .unwrap()
        .quantiles
        .as_ref()
        .unwrap();
    assert_eq!(quantiles[0]["value"]["observations"], 120000);
    assert_eq!(quantiles[0]["value"]["censored"], 0);
    observed.quality.as_mut().unwrap().quantiles = None;
    observed
        .quality
        .as_mut()
        .unwrap()
        .checked
        .retain(|s| s != "quantiles");
    assert_eq!(baseline, observed);
    eprintln!(
        "120 cases x 1000 replications: without_quantiles={without:?},with_quantiles={with:?}; whole selected-path outcomes and draw audits equal after only quantile fields removed"
    );
}
