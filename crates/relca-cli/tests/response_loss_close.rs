#[path = "../../relca-core/tests/support/response_loss_close_inputs.rs"]
mod input;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

#[test]
fn mixed_known_other_end_and_unknown_reachable_exits_two() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let tmp = test_tmp::root();
    std::fs::create_dir_all(&tmp).unwrap();
    let file = tmp.join(format!("r1close-mixed-{}.relca", std::process::id()));
    std::fs::write(&file, input::MIXED).unwrap();
    for command in ["check", "explain"] {
        let output = Command::new(env!("CARGO_BIN_EXE_relca"))
            .current_dir(&root)
            .arg(command)
            .arg(&file)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        // CLI mirrors warnings to stderr; their presence is not an error exit.
        let stderr = std::str::from_utf8(&output.stderr).unwrap();
        assert!(stderr.contains("W-UNREACH-1:"));
        assert!(stderr.contains("W-UNREACH-2:"));
        let v: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(v["verdict"], "inconclusive");
        for id in ["reachable end.done", "receipt_resolution"] {
            let p = v["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"] == id)
                .unwrap();
            assert_eq!(p["verdict"], "inconclusive");
            assert_eq!(p["cases"][0]["verdict"], "inconclusive");
        }
        assert_eq!(v["bounds"]["truncated"], false);
        let ts = v["case_outcomes"][0]["terminals"].as_array().unwrap();
        assert_eq!(ts.len(), 2);
        let known = ts.iter().find(|t| t["kind"] == "end").unwrap();
        assert_eq!(known["end"], "rejected");
        assert_eq!(known["work"], json!({"min":1,"max":1}));
        let unknown = ts.iter().find(|t| t["kind"] == "receipt_unknown").unwrap();
        assert!(unknown["end"].is_null());
        assert!(unknown["completed_at"].is_null());
        assert_eq!(unknown["observed_work"], json!({"min":1,"max":1}));
    }
}
