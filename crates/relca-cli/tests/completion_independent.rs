#[path = "../../relca-core/tests/completion_independent.rs"]
mod acceptance;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use acceptance::{completion_explorer_population::population, completion_explorer_product};
use std::process::Command;

#[test]
fn independent_population_matches_real_cli_without_snapshot_expectations() {
    let dir = test_tmp::root().join(format!(
        "relca-independent-completion-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for e in population() {
        let _m = acceptance::model(&e);
        let wanted = acceptance::expected(&e);
        let file = dir.join(format!("{}.relca", e.name));
        std::fs::write(&file, &e.source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("check")
            .arg(file)
            .output()
            .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        completion_explorer_product::check_report(&report, &e.own, &wanted)
            .unwrap_or_else(|error| panic!("{} {error}", e.name));
        let expected_exit = completion_explorer_product::verdict(
            acceptance::completion_explorer::overall(&e.own, &wanted),
        )
        .exit_code();
        assert_eq!(output.status.code(), Some(expected_exit), "{}", e.name);
    }
    // Deliberately keep the small inputs under the caller's TMPDIR for audit;
    // runtime comparison has no dependency on these files or a previous run.
}
