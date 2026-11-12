//! Snapshot inputs must remain the independently accepted positive models.
#[path = "support/response_snapshot_oracle.rs"]
mod hand;

#[test]
fn response_snapshot_models_match_three_existing_positive_acceptance_sources() {
    for (name, saved) in [
        (
            "g-unknown",
            include_str!("snapshots/inputs/response-g-unknown.relca"),
        ),
        (
            "g-probability",
            include_str!("snapshots/inputs/response-g-probability.relca"),
        ),
        (
            "max-thirty-two",
            include_str!("snapshots/inputs/response-max-thirty-two.relca"),
        ),
    ] {
        let accepted = hand::input::source(name);
        assert_eq!(
            saved.split_whitespace().collect::<Vec<_>>(),
            accepted.split_whitespace().collect::<Vec<_>>(),
            "snapshot source drifted from hand acceptance {name}"
        );
    }
}

#[test]
fn response_population_route_checks_hand_values_and_rejects_single_faults() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["g-unknown", "g-probability", "max-thirty-two"] {
        let id = format!("response-{name}");
        let args = vec![
            "check".to_string(),
            format!("crates/relca-cli/tests/snapshots/inputs/{id}.relca"),
        ];
        let source = std::fs::read_to_string(root.join(&args[1])).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .current_dir(&root)
            .output()
            .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let job = serde_json::json!({"id":id});
        hand::check(&job, &args, &source, &report, output.status.code());
        let rejects = |v: &serde_json::Value, s: &str, code| {
            std::panic::catch_unwind(|| hand::check(&job, &args, s, v, code)).is_err()
        };
        let mut broken = report.clone();
        broken["verdict"] = serde_json::json!("violated");
        assert!(rejects(&broken, &source, output.status.code()));
        let mut broken = report.clone();
        broken["agency"]["common_causes"] = serde_json::json!({"causes":[]});
        assert!(rejects(&broken, &source, output.status.code()));
        assert!(rejects(
            &report,
            &source.replace("duration fixed 1min", "duration fixed 2min"),
            output.status.code()
        ));
        assert!(rejects(&report, &source, Some(3)));
    }
}
