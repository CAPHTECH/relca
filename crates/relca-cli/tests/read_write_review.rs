#[path = "../../relca-core/tests/support/read_write_review.rs"]
mod review;
use serde_json::Value;
use std::{path::PathBuf, process::Command};
fn run(src: &str, command: &str, name: &str) -> Value {
    let out = run_raw(src, command, name);
    serde_json::from_slice(&out.stdout).unwrap()
}
fn run_raw(src: &str, command: &str, name: &str) -> std::process::Output {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(format!(".tmp/k5fix-{name}-{}.relca", std::process::id()));
    std::fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args([command, path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!((0..=5).contains(&out.status.code().unwrap()));
    out
}
#[test]
fn real_cli_empty_relation_keeps_hand_disclosure_and_exit() {
    let (_, src, counts, n) = review::cases()
        .into_iter()
        .find(|(n, _, _, _)| *n == "empty")
        .unwrap();
    let out = run_raw(&src, "check", "empty-bytes");
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stderr.is_empty());
    review::assert_disclosure(&serde_json::from_slice(&out.stdout).unwrap(), counts, n);
}
#[test]
fn real_cli_outer_candidate_disappearance_survives_other_changes() {
    let (_, src, _) = review::h::cases()
        .into_iter()
        .find(|(n, _, _)| *n == "two-if-sites")
        .unwrap();
    review::assert_outer_removed(&run(&src, "mutate", "outer"));
}
macro_rules! excluded { ($($test:ident => $name:literal),+) => { $(
    #[test] fn $test() {
        let (name,src,counts,n)=review::cases().into_iter().find(|(name,_,_,_)|*name==$name).unwrap();
        let v=run(&src,"check",name);
        assert_ne!(v["verdict"],"invalid","{name}: {v}");
        assert_ne!(v["verdict"],"unsupported","{name}: {v}");
        review::assert_disclosure(&v,counts,n);
    }
)+ }; }
excluded! {single_fork=>"single-fork",two_fork=>"two-fork",case_local=>"case-local",per_case=>"per-case",mixed=>"mixed",empty=>"empty",case_local_repeated=>"case-local-repeated",case_local_two=>"case-local-two",case_local_three=>"case-local-three",single_two=>"single-two",mixed_single_five=>"mixed-single-five"}
#[test]
fn real_cli_per_case_writer_forms_remain_rejected() {
    for (i, src) in review::per_case_refusals().iter().enumerate() {
        let v = run(src, "check", &format!("per-case-refusal-{i}"));
        assert_eq!(v["verdict"], "unsupported", "{v}");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == if i == 0 { "U-SEM-0" } else { "U-SYN-0" })
        );
        review::assert_disclosure(&v, [0, 0], 0);
    }
}
#[test]
fn real_cli_contains_or_atomic_writer() {
    let src = review::set_source();
    review::h::assert_rows(
        &src,
        &run(&src, "check", "set"),
        &["A.keys: A.r -> A.w", "A.keys: A.w -> A.w"],
    );
}
#[test]
fn real_cli_excluded_row_keeps_fixed_tails() {
    let s = review::surface_source();
    for command in ["check", "simulate", "explain"] {
        let v = run(&s, command, &format!("tail-{command}"));
        review::assert_disclosure(&v, [0, 1], 1);
    }
}

#[test]
fn guard_removal_deletes_the_three_hand_candidate_leaves_at_both_sites() {
    // Flow reads keys and only Ledger.post writes keys. Bypassing either arm
    // removes the lexical If read: field/value/affects each become absent.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source =
        std::fs::read_to_string(root.join("fixtures/aset-branch-sharing/model.relca")).unwrap();
    assert_eq!(source.find("if case.id in Ledger.keys").unwrap(), 538);
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["mutate", "fixtures/aset-branch-sharing/model.relca"])
        .current_dir(root)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(4));
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    let hand: Value =
        serde_json::from_str(include_str!("read_write_mutation_positions.json")).unwrap();
    for site in hand.as_array().unwrap() {
        let rows: Vec<_> = actual.pointer(site["pointer"].as_str().unwrap()).unwrap()
            .as_array().unwrap().iter().filter(|row| row["key"].as_str()
                .is_some_and(|key| key.starts_with("/field=\"concurrency.read_write\";value=\"Ledger.keys: process.Flow@if:538 -> Ledger.post\"/")))
            .cloned().collect();
        assert_eq!(serde_json::json!(rows), site["rows"]);
    }
}
