//! Policy order must not reorder static tables.
use relca_core::{lower, report, syntax};

#[test]
fn declared_first_idle_order_is_private_to_policy_ir_not_actor_or_agency_projection() {
    let src = include_str!("../../../fixtures/pol-separation.broken-change/model.relca");
    let old = src.replace(" assign first_idle", "");
    assert_ne!(src, old);
    let extended = report::check(src);
    let baseline = report::check(&old);
    let extended = serde_json::to_value(extended).unwrap();
    let baseline = serde_json::to_value(baseline).unwrap();
    assert_eq!(extended["actors"], baseline["actors"]);
    assert_eq!(extended["agency"], baseline["agency"]);
    let clerk = extended["actors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "Clerk")
        .unwrap();
    assert_eq!(
        clerk["executors"]["value"]["ids"],
        serde_json::json!(["c1", "c2"])
    );
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    assert_eq!(
        m.actors[0].executors,
        Some(vec![
            relca_core::ir::PersonId(0),
            relca_core::ir::PersonId(1)
        ])
    );
    assert_eq!(
        m.start_policy.unwrap().executor_order[0],
        Some(vec![
            relca_core::ir::PersonId(1),
            relca_core::ir::PersonId(0)
        ])
    );
}
