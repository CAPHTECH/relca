use std::{path::PathBuf, process::Command};
#[test]
fn mutation_scopes_do_not_open_a_fork_inside_bounded_repetition() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root
        .join(".tmp")
        .join(format!("round2-loop-gates-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = include_str!("../../relca-core/tests/bounded_repetition/models/L1.relca");
    assert_eq!(source.matches("r = Rev.review()").count(), 1);
    let source = source.replace(
        "r = Rev.review()",
        "fork f { r = Rev.review() }; join all f",
    );
    let source = format!(
        "{} configuration Plan {{cut C;process P}} }}",
        source.trim_end().strip_suffix('}').unwrap()
    );
    let file = dir.join("model.relca");
    std::fs::write(&file, source).unwrap();
    for args in [
        vec![],
        vec!["--kinds", "evidence_missing"],
        vec!["--sites", "M1"],
        vec!["--configuration", "Plan"],
        vec!["--mutant-limit", "1"],
        vec![
            "--kinds",
            "evidence_missing",
            "--sites",
            "M1",
            "--configuration",
            "Plan",
            "--mutant-limit",
            "1",
        ],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("mutate")
            .arg(&file)
            .args(&args)
            .output()
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(4), "{args:?}: {value}");
        assert_eq!(value["verdict"], "unsupported");
        assert!(
            value["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SEM-0"
                    && d["message"]
                        == "loop_feature_combination: repeat mutation sites are not connected"),
            "{args:?}: {value}"
        );
    }
}
