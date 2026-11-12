#[path = "support/test_tmp.rs"]
mod test_tmp;

use std::path::{Path, PathBuf};

#[test]
fn external_tmp_is_ignored_without_writing_to_it() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let fallback = workspace.join(".tmp");
    for requested in [
        PathBuf::from("/tmp/relca-external"),
        workspace.join("../outside"),
        PathBuf::from(format!("{}-other/tmp", workspace.display())),
    ] {
        assert_eq!(test_tmp::select(&workspace, &requested), fallback);
    }
    assert_eq!(
        test_tmp::select(&workspace, Path::new(".tmp/inside")),
        fallback.join("inside")
    );
    assert!(test_tmp::root().starts_with(workspace));
}

#[cfg(unix)]
#[test]
fn escaping_symlink_is_rejected_before_creating_children() {
    let root = test_tmp::root();
    let directory = root.join(format!("tmp-link-control-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let link = directory.join("escape");
    std::os::unix::fs::symlink(root.parent().unwrap(), &link).unwrap();
    assert_eq!(
        test_tmp::select(&directory, Path::new("escape/not-created")),
        directory.join(".tmp")
    );
    std::fs::remove_file(link).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
