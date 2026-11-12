//! Review finding on ADR 0020 (addendum 2): a run cut by the mutant limit does
//! not claim the generation and comparison stages as checked.
use relca_core::mutation;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../fixtures/{name}/model.relca",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

const STAGES: [&str; 2] = ["single_fault_generation", "assigned_check_comparison"];

#[test]
fn r_limit_cut_keeps_generation_and_comparison_unchecked() {
    let cut = mutation::mutate(&fixture("mut-limit.broken-many-sites"));
    assert!(cut.run.limit_reached);
    for stage in STAGES {
        assert!(cut.unchecked.iter().any(|s| s == stage), "{stage}");
        assert!(!cut.checked.iter().any(|s| s == stage), "{stage}");
    }
    let whole = mutation::mutate(&fixture("mut-guard"));
    assert!(!whole.run.limit_reached);
    for stage in STAGES {
        assert!(whole.checked.iter().any(|s| s == stage), "{stage}");
        assert!(!whole.unchecked.iter().any(|s| s == stage), "{stage}");
    }
}
