//! Closed hand-oracle route for the three ADR 0056 snapshot additions.
//! Legacy common/completion graph oracles intentionally reject Response IR.
#[path = "../../../relca-core/tests/support/response_loss_inputs.rs"]
pub mod input;
#[path = "../../../relca-core/tests/support/response_loss_oracle.rs"]
mod oracle;
use serde_json::Value;

pub fn check(job: &Value, args: &[String], source: &str, report: &Value, exit: Option<i32>) {
    let id = job["id"].as_str().unwrap();
    let name = id
        .strip_prefix("response-")
        .expect("unregistered response job");
    assert!(["g-unknown", "g-probability", "max-thirty-two"].contains(&name));
    assert_eq!(
        args,
        &[
            "check".to_string(),
            format!("crates/relca-cli/tests/snapshots/inputs/{id}.relca")
        ]
    );
    assert_eq!(
        source.split_whitespace().collect::<Vec<_>>(),
        input::source(name).split_whitespace().collect::<Vec<_>>(),
        "response hand route must bind the actual source, not just its job name"
    );
    oracle::check(report, name);
    assert_eq!(exit, Some(if name == "max-thirty-two" { 0 } else { 2 }));
    let agency = &report["agency"];
    assert_eq!(agency["rules"], "agency-p2-response-1");
    // These three sources have no source lineage or kernel-written latent
    // producer, and no pair reading an Actor latent. Derive absence from those
    // closed sources rather than extending the old graph to unknown control.
    assert!(agency.get("common_causes").is_none());
    assert!(agency.get("model_uncertainties").is_none());
    assert_eq!(
        agency["viewpoints"]["framing"]["reason"],
        "response_control"
    );
    assert!(report.get("via_model_paths").is_none());
    for field in ["via_model_paths", "agency.diagnostics.W-LIN-2"] {
        assert!(
            report["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["field"] == field
                    && a["value"].as_str().unwrap().contains("response_control"))
        );
    }
    assert!(
        !report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "W-LIN-2")
    );
}
