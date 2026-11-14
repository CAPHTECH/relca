#[path = "support/agency_vp_snapshot_oracle.rs"]
mod oracle;
use serde_json::Value;
use std::{path::Path, process::Command};

#[test]
fn hand_calculated_agency_viewpoint_fixture_pairs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in oracle::NAMES {
        let dir = root.join("fixtures").join(name);
        assert!(dir.join("calc.md").is_file());
        let mut args = vec!["check".to_owned(), format!("fixtures/{name}/model.relca")];
        if dir.join("invocation.json").is_file() {
            let inv: Value =
                serde_json::from_slice(&std::fs::read(dir.join("invocation.json")).unwrap())
                    .unwrap();
            args.extend([
                "--agency-limit-scale".into(),
                inv["agency_limit_scale"].as_str().unwrap().into(),
            ]);
        }
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .current_dir(&root)
            .output()
            .unwrap();
        let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
        oracle::check(
            &format!("fixture-{name}"),
            &args,
            &doc,
            out.status.code().unwrap(),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        let text = String::from_utf8(out.stderr).unwrap();
        let json_count = doc["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-AGY-5")
            .count();
        assert_eq!(
            text.lines().filter(|s| s.starts_with("W-AGY-5:")).count(),
            json_count
        );
    }
}
