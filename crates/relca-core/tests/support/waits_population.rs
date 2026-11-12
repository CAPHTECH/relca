#![allow(dead_code)]
pub const NAMES: &[&str] = include!("waits_names.rs");
pub fn fixture(name: &str) -> bool {
    static CENSUS: std::sync::LazyLock<()> = std::sync::LazyLock::new(|| {
        use std::collections::BTreeSet;
        let declared: BTreeSet<_> = NAMES.iter().map(|n| (*n).to_owned()).collect();
        assert_eq!(NAMES.len(), 43);
        assert_eq!(declared.len(), 43);
        let actual: BTreeSet<_> = std::fs::read_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"),
        )
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("model.relca").is_file())
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("wait-"))
        .collect();
        assert_eq!(declared, actual, "prefix checks inventory, never excludes");
    });
    std::sync::LazyLock::force(&CENSUS);
    NAMES.contains(&name)
}
