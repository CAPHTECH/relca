#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/deferred_metrics_inputs.rs"]
mod h;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::Value;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

fn run(name: &str, command: &str) -> (Value, i32) {
    let dir = test_tmp::root().join(format!("k2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "{name}-{command}-{}.relca",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, h::source(name)).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(path)
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    fixture_match::validate_deferred_metrics_with_source(
        if command == "mutate" {
            &v["original"]
        } else {
            &v
        },
        &h::source(name),
        None,
    )
    .unwrap();
    (v, o.status.code().unwrap())
}
#[test]
fn real_no_defer_positive_pair_preserves_numeric_output() {
    for command in ["check", "simulate", "explain"] {
        let (v, code) = run("none", command);
        h::assert_report(&v, "none", command == "simulate");
        assert_eq!(code, 0);
    }
}
#[test]
fn real_defer_actor_capability_opaque_and_quality_only_are_not_numeric() {
    for name in h::NAMES {
        if name != "truncated" {
            for command in ["check", "simulate", "explain"] {
                let (v, code) = run(name, command);
                h::assert_report(&v, name, command == "simulate");
                assert_eq!(code, 0);
            }
        }
    }
}
#[test]
fn real_cut_keeps_truncated_not_not_configured() {
    let (v, code) = run("truncated", "check");
    assert_eq!(code, 2);
    assert_eq!(v["metrics"]["overall"]["work"]["reason"], "truncated");
}
#[test]
fn real_compare_objectives_and_quality_hard_keep_missing_cells() {
    let dir = test_tmp::root().join(format!("k2-compare-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for hard in [false, true] {
        for sim in [false, true] {
            let p = dir.join(format!("{hard}.relca"));
            std::fs::write(&p, h::comparison_source(hard)).unwrap();
            let o = Command::new(env!("CARGO_BIN_EXE_relca"))
                .arg("compare")
                .arg(p)
                .args([
                    "--plans",
                    "Included,Unrelated",
                    "--mode",
                    if sim { "simulation" } else { "exhaustive" },
                ])
                .output()
                .unwrap();
            let v: Value = serde_json::from_slice(&o.stdout).unwrap();
            for (j, plan) in ["Included", "Unrelated"].iter().enumerate() {
                fixture_match::validate_deferred_metrics_with_source(
                    &v["plans"][j]["report"],
                    &h::comparison_source(hard),
                    Some(plan),
                )
                .unwrap();
            }
            h::assert_comparison(&v, hard, sim);
            assert_eq!(o.status.code(), Some(if hard { 2 } else { 0 }));
        }
    }
}
#[test]
fn real_mutation_original_is_subject_to_the_same_cut() {
    let (v, code) = run("actor", "mutate");
    assert_eq!(code, 4);
    h::assert_report(&v["original"], "actor", false);
}
#[test]
fn real_invoice_and_busyday_keep_cut_and_existing_refusal_reasons_separate() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (model, plan, exit) in [
        ("phase1", "AsIs", 0),
        ("phase1", "ToBeP", 0),
        ("phase1", "ToBeA", 1),
        ("rules", "ToBeA", 5),
        ("rules", "ToBeB", 1),
    ] {
        let o = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args([
                "check",
                &format!("examples/invoice/{model}.relca"),
                "--configuration",
                plan,
            ])
            .current_dir(&root)
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(o.status.code(), Some(exit));
        assert_eq!(
            v["quality"]["cohort"][0]["value"]["reason"],
            "not_configured"
        );
        assert_eq!(v["metrics"]["overall"]["work"]["reason"], "not_configured");
        assert_eq!(
            v["cut"]["checked"].as_array().unwrap().last().unwrap(),
            "deferred_metrics"
        );
    }
    for plan in ["AsIs", "ToBeP", "ToBeA"] {
        let o = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args([
                "simulate",
                "examples/invoice/busyday.relca",
                "--configuration",
                plan,
                "--scenario",
                "BusyDay",
            ])
            .current_dir(&root)
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(o.status.code(), Some(4));
        assert_eq!(v["metrics"]["overall"]["work"]["reason"], "aborted");
        assert_eq!(
            v["cut"]["checked"].as_array().unwrap().last().unwrap(),
            "deferred_metrics"
        );
        assert!(
            !v["cut"]["unchecked"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("deferred_metrics"))
        );
    }
}
