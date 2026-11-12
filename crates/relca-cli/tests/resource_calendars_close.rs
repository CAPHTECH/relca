#[path = "../../relca-core/tests/support/q4_close.rs"]
mod close;
#[path = "../../relca-core/tests/support/q4_review.rs"]
mod review;
use serde_json::Value;
use std::{path::PathBuf, process::Command};
#[path = "../../relca-core/tests/support/sensitivity_oracle.rs"]
mod uncertainty_oracle;

fn invoke(name: &str, source: &str, diff: bool, exit: i32) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root
        .join(".tmp/q4-close-cli")
        .join(format!("{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, source).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_relca"));
    command
        .arg(if diff { "diff" } else { "check" })
        .arg(path)
        .current_dir(root);
    if diff {
        command.args(["--from", "From", "--to", "To"]);
    }
    let output = command.output().unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{name}: {}",
        value["diagnostics"]
    );
    let mut stderr = String::new();
    let groups = if diff {
        vec![
            ("from: ", &value["from"]["report"]["diagnostics"]),
            ("to: ", &value["to"]["report"]["diagnostics"]),
            ("diff: ", &value["diagnostics"]),
        ]
    } else {
        vec![("", &value["diagnostics"])]
    };
    for (prefix, diagnostics) in groups {
        for d in diagnostics.as_array().unwrap() {
            stderr.push_str(&format!(
                "{prefix}{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            ));
        }
    }
    assert_eq!(output.stderr, stderr.as_bytes());
    value
}

#[test]
fn cli_mixed_if_wait_keeps_whole_row_withdrawal_and_fixed_wait_control() {
    for shape in 0..3 {
        for condition in [false, true] {
            let value = invoke(
                &format!("mixed-{shape}-{condition}"),
                &close::mixed(shape, condition),
                false,
                i32::from(shape == 1 && condition),
            );
            close::mixed_assert(&value, shape, condition);
        }
    }
}

#[test]
fn cli_diff_wait_until_keeps_refusal_diagnostic_and_confirmed_child_violation() {
    let control = invoke("diff-control", &close::diff(false, false), true, 0);
    assert_eq!(control["verdict"], "satisfied");
    for (from, to) in [(true, false), (false, true), (true, true)] {
        let value = invoke(
            &format!("diff-{from}-{to}"),
            &close::diff(from, to),
            true,
            1,
        );
        assert_eq!(value["verdict"], "violated");
        assert!(value["diagnostics"].as_array().unwrap().iter().any(|d| {
            d["code"] == "U-SEM-0"
                && d["message"]
                    .as_str()
                    .unwrap()
                    .contains("condition_wait_diff_unsupported")
        }));
        uncertainty_oracle::references(&value, &["/from/report".into(), "/to/report".into()])
            .unwrap();
    }
}

#[test]
fn cli_contest_wait_until_is_explicitly_unsupported_for_end_and_commit() {
    for effect in [false, true] {
        invoke(
            &format!("contest-control-{effect}"),
            &close::contest(false, false, 1, effect),
            false,
            0,
        );
        for restricted in [false, true] {
            let value = invoke(
                &format!("contest-{effect}-{restricted}"),
                &close::contest(true, restricted, 1, effect),
                false,
                4,
            );
            close::refused(&value, "condition_wait_contest_unsupported");
            uncertainty_oracle::uncertainty(
                &value,
                [
                    ("unavailable", Some("not_lowered")),
                    ("not_applicable", Some("exhaustive_analysis")),
                    ("unavailable", Some("not_lowered")),
                ],
            )
            .unwrap();
        }
    }
}
