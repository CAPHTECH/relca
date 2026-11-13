//! Closed C6 population for historical census exclusions and later linkage.
#![allow(dead_code)]
// Round 1's independent positive-time pair is tested by mutation_review_round1
// and the canonical CLI population, not fed to the frozen historical invocations.
pub fn review_fixture(name: &str) -> bool {
    matches!(
        name,
        "mut3-expiry-window" | "mut3-expiry-window.broken-late-arrival"
    )
}
pub fn fixture(name: &str) -> bool {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let known = NAMES
        .get_or_init(|| {
            let names: Vec<String> =
                serde_json::from_str(include_str!("mutation_default_names.json")).unwrap();
            let declared: std::collections::BTreeSet<_> = names.iter().cloned().collect();
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
            let actual: std::collections::BTreeSet<_> = std::fs::read_dir(root)
                .unwrap()
                .map(|p| p.unwrap().path())
                .filter(|p| p.join("model.relca").is_file())
                .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
                .filter(|n| n.starts_with("mut2-"))
                .collect();
            assert_eq!(names.len(), 79);
            assert_eq!(declared.len(), names.len());
            assert_eq!(declared, actual, "closed C6 table must equal real fixtures");
            names
        })
        .iter()
        .any(|n| n == name);
    assert!(
        !name.starts_with("mut2-") || known,
        "unclassified C6 fixture {name}"
    );
    known
}
