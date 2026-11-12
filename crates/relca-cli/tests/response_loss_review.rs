//! Real CLI acceptance for the Q1 review rulings (not core-as-CLI substitution).
#[path = "../../relca-core/tests/support/response_loss_review_oracle.rs"]
mod hand;
#[path = "../../relca-core/tests/support/response_loss_review_inputs.rs"]
mod input;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::Value;
#[path = "support/ledger_shape.rs"]
mod ledger_shape;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
fn run(command: &str, name: &str, source: &str, config: Option<&str>) -> (Value, i32) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let tmp = test_tmp::root();
    let dir = tmp.join(format!("q1review-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!(
        "{name}-{command}-{}.relca",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&file, source).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_relca"));
    cmd.current_dir(root)
        .arg(command)
        .arg(&file)
        .env("CARGO_NET_OFFLINE", "true");
    if let Some(c) = config {
        cmd.args(["--configuration", c]);
    }
    let o = cmd.output().unwrap();
    std::fs::write(file.with_extension("stdout.json"), &o.stdout).unwrap();
    std::fs::write(file.with_extension("stderr.txt"), &o.stderr).unwrap();
    std::fs::write(
        file.with_extension("exit.txt"),
        format!("{}\n", o.status.code().expect("normal process exit")),
    )
    .unwrap();
    assert!(o.stdout.ends_with(b"\n"), "complete JSON output");
    let v = serde_json::from_slice(&o.stdout).expect("JSON stdout");
    (v, o.status.code().expect("normal process exit"))
}

macro_rules! m1 {
    ($test:ident, $kind:literal, $source:expr) => {
        #[test]
        fn $test() {
            let (v, code) = run("check", $kind, &$source, None);
            assert_eq!(code, 0, "{}", v["diagnostics"]);
            hand::m1(&v, $kind);
        }
    };
}
m1!(
    cli_m1_common_cause_excludes_alias,
    "common",
    input::common(true)
);
m1!(
    cli_m1_uncertainty_judgement_excludes_alias,
    "judgement",
    input::basis(Some("judgement"))
);
m1!(
    cli_m1_uncertainty_target_excludes_alias,
    "target",
    input::basis(Some("target"))
);

macro_rules! suppressed {
    ($test:ident, $warning:literal) => {
        #[test]
        fn $test() {
            let (v, code) = run("check", $warning, &input::warning($warning, true), None);
            assert_eq!(code, 0, "{}", v["diagnostics"]);
            hand::suppressed(&v, $warning);
        }
    };
}
suppressed!(cli_m1_agy0_reasoned_unchecked, "W-AGY-0");
suppressed!(cli_m1_agy2_reasoned_unchecked, "W-AGY-2");
suppressed!(cli_m1_lin2_reasoned_unchecked, "W-LIN-2");
suppressed!(cli_m1_resp2_reasoned_unchecked, "W-RESP-2");

#[test]
fn cli_m1_ordinary_controls_really_warn() {
    for warning in input::SUPPRESSED {
        let (v, code) = run("check", warning, &input::warning(warning, false), None);
        assert_eq!(code, 0);
        assert!(hand::warning(&v, warning));
        assert!(!hand::has_mark(&v, warning));
    }
}
#[test]
fn cli_m1_agy1_retained_and_single_edit_control() {
    for single in [false, true] {
        let (v, code) = run("check", "bypass", &input::bypass(single), None);
        assert_eq!(code, 0, "{}", v["diagnostics"]);
        assert_eq!(hand::warning(&v, "W-AGY-1"), !single);
    }
}
#[test]
fn cli_m1_agy3_retained_and_single_edit_control() {
    for keep in [true, false] {
        let (v, code) = run("check", "fallback", &input::fallback(keep), Some("To"));
        assert_eq!(code, 0, "{}", v["diagnostics"]);
        assert_eq!(hand::warning(&v, "W-AGY-3"), keep);
    }
}
#[test]
fn cli_m2_via_model_path_withdrawal_is_reasoned() {
    let (v, code) = run("check", "via", &input::via(true, true), None);
    assert_eq!(code, 0);
    hand::via(&v);
}
#[test]
fn cli_m2_no_via_declaration_is_still_explicitly_unchecked() {
    let (v, code) = run("check", "no-via", &input::via(true, false), None);
    assert_eq!(code, 0);
    hand::via(&v);
}
#[test]
fn cli_m2_ordinary_call_preserves_nonempty_via_path_control() {
    let (v, code) = run("check", "ordinary-via", &input::via(false, true), None);
    assert_eq!(code, 0);
    assert!(!hand::rows(&v["via_model_paths"]["paths"]).is_empty());
    assert!(!hand::has_mark(&v, "via_model_paths"));
}

macro_rules! reach {
    ($test:ident, $command:literal, $receipt:literal, $fail:literal, $violation:literal) => {
        #[test]
        fn $test() {
            let (v, code) = run(
                $command,
                stringify!($test),
                &input::reachable($receipt, $fail, $violation),
                None,
            );
            let expected = hand::reachable(&v, $receipt, $fail, $violation);
            assert_eq!(code, expected, "actual CLI exit");
            let mut dto = v.clone();
            if $command == "explain" {
                dto.as_object_mut().unwrap().remove("explanation");
                ledger_shape::dto_projection(&mut dto).unwrap();
            }
            let typed: relca_core::report::Report = serde_json::from_value(dto).unwrap();
            let round = serde_json::to_value(typed).unwrap();
            assert_eq!(round["properties"], v["properties"]);
            assert_eq!(round["case_outcomes"], v["case_outcomes"]);
        }
    };
}
reach!(
    cli_m3_unknown_only_reachable_exit_two,
    "check",
    "unknown",
    false,
    false
);
reach!(
    cli_m3_explain_unknown_only_reachable_exit_two,
    "explain",
    "unknown",
    false,
    false
);
reach!(
    cli_m3_delivered_control_exit_zero,
    "check",
    "delivered",
    false,
    false
);
reach!(
    cli_m3_known_rejected_control_exit_one,
    "check",
    "delivered",
    true,
    false
);
reach!(
    cli_m3_unknown_keeps_independent_violation_exit_one,
    "check",
    "unknown",
    false,
    true
);
