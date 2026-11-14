use relca_core::explain;

#[test]
fn definite_missing_source_keeps_the_excluded_conditional_edge_visible() {
    let src = include_str!("evidence_basis/m5.relca")
        .replace(
            "actor PurchaseOrder",
            "actor Missing { source record } actor PurchaseOrder",
        )
        .replace("include [PurchaseOrder", "include [Missing,PurchaseOrder");
    let end = src.rfind('}').unwrap();
    let src = format!(
        "{} contract Basis {{ evidence_basis Approver.approve [PurchaseOrder,Missing] }} }}",
        &src[..end]
    );
    let value = serde_json::to_value(explain::explain(&src)).unwrap();
    let trace = &value["explanation"]["counterexamples"][0];
    assert_eq!(trace["replay"]["status"], "verified");
    let evidence = &trace["evidence"];
    assert_eq!(
        evidence["required"],
        serde_json::json!(["Missing", "PurchaseOrder"])
    );
    assert_eq!(evidence["sources"], serde_json::json!(["Invoice"]));
    assert_eq!(
        evidence["missing"],
        serde_json::json!(["Missing", "PurchaseOrder"])
    );
    assert_eq!(evidence["excluded_edges"], serde_json::json!(["p -> a"]));
}
