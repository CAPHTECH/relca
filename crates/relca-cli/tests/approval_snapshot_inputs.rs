#[path = "support/approval_snapshot_jobs.rs"]
mod jobs;
#[test]
fn snapshot_inputs_and_live_values_remain_the_three_accepted_positive_examples() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for id in jobs::IDS {
        jobs::check_inputs(id, &root);
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(jobs::arguments(id))
            .current_dir(&root)
            .output()
            .unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        jobs::check(id, &doc, out.status.code().unwrap()).unwrap();
        let mut bad = doc.clone();
        bad["approval_burden"]["rate"]["value"] = "0".into();
        assert!(jobs::check(id, &bad, out.status.code().unwrap()).is_err());
        let mut bad = doc.clone();
        bad["approval_burden"]["reinvestigation"]["total"] = "99".into();
        assert!(jobs::check(id, &bad, out.status.code().unwrap()).is_err());
    }
}
