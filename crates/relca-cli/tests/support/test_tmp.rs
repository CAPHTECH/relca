//! Local temporary root for CLI integration tests, including direct cargo test.
use std::path::{Component, Path, PathBuf};

pub(super) fn select(workspace: &Path, requested: &Path) -> PathBuf {
    let mut directory = workspace.to_path_buf();
    for component in workspace.join(requested).components() {
        match component {
            Component::ParentDir => {
                directory.pop();
            }
            Component::CurDir => {}
            Component::RootDir => directory = PathBuf::from(component.as_os_str()),
            other => directory.push(other.as_os_str()),
        }
    }
    // Resolve existing ancestors before creating anything; a symlink must not
    // turn a lexical child into an external write.
    let mut ancestor = directory.as_path();
    while !ancestor.exists() {
        ancestor = ancestor.parent().expect("absolute temporary directory");
    }
    if directory.starts_with(workspace) && ancestor.canonicalize().unwrap().starts_with(workspace) {
        directory
    } else {
        workspace.join(".tmp")
    }
}

pub fn root() -> PathBuf {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let requested = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".tmp"));
    let directory = select(&workspace, &requested);
    if directory.exists() {
        assert!(directory.canonicalize().unwrap().starts_with(&workspace));
    }
    std::fs::create_dir_all(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    assert!(directory.starts_with(workspace));
    directory
}
