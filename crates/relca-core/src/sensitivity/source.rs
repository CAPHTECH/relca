//! I02: bounded local inputs, captured once before any model is executed.
use super::input::{FailureKind, InputError, MAX_MANIFEST_BYTES, Manifest, Mode, Point};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

pub const MAX_SOURCE_BYTES: usize = 4 << 20;
pub const MAX_TOTAL_SOURCE_BYTES: usize = 64 << 20;

#[derive(Debug, Clone)]
pub struct ManifestSnapshot {
    manifest: Manifest,
    sha256: String,
    root: PathBuf,
    mode: Mode,
}

#[derive(Debug, Clone)]
struct Content {
    text: Arc<str>,
    sha256: String,
}

#[derive(Debug, Clone)]
pub struct SourceSnapshot {
    point: Point,
    content: Content,
}
impl SourceSnapshot {
    pub fn point(&self) -> &Point {
        &self.point
    }
    pub fn source(&self) -> &str {
        &self.content.text
    }
    pub fn sha256(&self) -> &str {
        &self.content.sha256
    }
}

fn unavailable(detail: impl Into<String>) -> InputError {
    InputError::new(FailureKind::InputUnavailable, detail)
}
fn capacity(detail: &str) -> InputError {
    InputError::new(FailureKind::CapacityLimit, detail)
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Verify the actual opened object before reading, not just a previously
/// canonicalized name. An unavailable fd-root check is not silently skipped.
#[cfg(target_os = "linux")]
fn check_opened(file: &File, root: &Path) -> Result<(), InputError> {
    use std::os::fd::AsRawFd;
    let actual = fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).map_err(|e| {
        InputError::new(
            FailureKind::FeatureUnsupported,
            format!("fd_root_check_unavailable: {e}"),
        )
    })?;
    if !actual.starts_with(root) || actual.to_string_lossy().ends_with(" (deleted)") {
        return Err(unavailable(
            "opened input is no longer within the manifest root",
        ));
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn check_opened(_: &File, _: &Path) -> Result<(), InputError> {
    Err(InputError::new(
        FailureKind::FeatureUnsupported,
        "fd_root_check_unavailable on this backend",
    ))
}

fn read_bounded(path: &Path, root: &Path, limit: usize) -> Result<Vec<u8>, InputError> {
    let metadata =
        fs::metadata(path).map_err(|e| unavailable(format!("cannot inspect input: {e}")))?;
    if !metadata.is_file() {
        return Err(unavailable("input must be a regular local file"));
    }
    if metadata.len() > limit as u64 {
        return Err(capacity("input byte limit exceeded"));
    }
    let file = File::open(path).map_err(|e| unavailable(format!("cannot open input: {e}")))?;
    check_opened(&file, root)?;
    if !file
        .metadata()
        .map_err(|e| unavailable(e.to_string()))?
        .is_file()
    {
        return Err(unavailable("opened input is not a regular file"));
    }
    // File growth cannot turn a bounded request into an unbounded read.
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| unavailable(format!("cannot read input: {e}")))?;
    if bytes.len() > limit {
        return Err(capacity("input byte limit exceeded"));
    }
    Ok(bytes)
}

impl ManifestSnapshot {
    /// Capture/decode only: keep declared IDs even if validation later refuses
    /// the suite. load_sources always validates the whole manifest first.
    pub fn read(path: &Path, mode: Mode) -> Result<Self, InputError> {
        let path = path
            .canonicalize()
            .map_err(|e| unavailable(format!("cannot locate manifest: {e}")))?;
        let root = path
            .parent()
            .ok_or_else(|| unavailable("manifest has no parent"))?
            .to_owned();
        let bytes = read_bounded(&path, &root, MAX_MANIFEST_BYTES)?;
        let manifest = Manifest::decode(&bytes)?;
        Ok(Self {
            manifest,
            sha256: hash(&bytes),
            root,
            mode,
        })
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn validate(&self) -> Result<(), InputError> {
        self.manifest.validate(self.mode)
    }

    /// All-or-error: never return a successful prefix. The caller retains this
    /// manifest snapshot, hence all planned IDs, even when a source is refused.
    pub fn load_sources(&self) -> Result<Vec<SourceSnapshot>, InputError> {
        self.validate()?;
        let mut cache: BTreeMap<PathBuf, Content> = BTreeMap::new();
        let mut sources = Vec::with_capacity(self.manifest.points.len());
        let mut total = 0usize;
        for point in &self.manifest.points {
            let load = || -> Result<PathBuf, InputError> {
                let relative = Path::new(&point.model);
                if point.model.contains("://")
                    || relative.is_absolute()
                    || relative
                        .components()
                        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
                {
                    return Err(unavailable(
                        "model path must be local and relative, without parent components",
                    ));
                }
                let path = self
                    .root
                    .join(relative)
                    .canonicalize()
                    .map_err(|e| unavailable(format!("cannot locate model: {e}")))?;
                if !path.starts_with(&self.root) {
                    return Err(unavailable("model symlink escapes manifest root"));
                }
                Ok(path)
            };
            let path = load().map_err(|e| e.at_point(&point.id))?;
            let content = if let Some(content) = cache.get(&path) {
                content.clone()
            } else {
                let bytes = read_bounded(&path, &self.root, MAX_SOURCE_BYTES)
                    .map_err(|e| e.at_point(&point.id))?;
                let sha256 = hash(&bytes);
                let text = String::from_utf8(bytes).map_err(|e| {
                    unavailable(format!("model is not UTF-8: {e}")).at_point(&point.id)
                })?;
                let content = Content {
                    text: Arc::from(text),
                    sha256,
                };
                cache.insert(path, content.clone());
                content
            };
            // The limit counts the full source for every declared model point,
            // even when physical bytes can be shared by the read-once cache.
            total = total
                .checked_add(content.text.len())
                .filter(|n| *n <= MAX_TOTAL_SOURCE_BYTES)
                .ok_or_else(|| {
                    capacity("total declared source bytes exceed 64MiB").at_point(&point.id)
                })?;
            sources.push(SourceSnapshot {
                point: point.clone(),
                content,
            });
        }
        Ok(sources)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Directory {
        root: PathBuf,
    }
    impl Directory {
        fn new() -> Self {
            let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp");
            fs::create_dir_all(&parent).unwrap();
            let root = parent.join(format!(
                "q7-source-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            fs::create_dir(root.join("models")).unwrap();
            Self { root }
        }
        fn manifest(&self, paths: &[&str]) -> PathBuf {
            let path = self.root.join("models/manifest.json");
            let value = json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"hand path controls"},
                "points":paths.iter().enumerate().map(|(i,p)| json!({"id":format!("p{i}"),"model":p,"configuration":null,"scenario":null})).collect::<Vec<_>>(),
                "targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":null});
            fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            path
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
    #[test]
    fn exact_bytes_and_internal_aliases_are_captured_once_and_remain_immutable() {
        let d = Directory::new();
        let bytes = b"hand source with trailing spaces  \n";
        fs::write(d.root.join("models/base.relca"), bytes).unwrap();
        std::os::unix::fs::symlink("base.relca", d.root.join("models/link.relca")).unwrap();
        let path = d.manifest(&["base.relca", "./base.relca", "link.relca"]);
        let manifest_bytes = fs::read(&path).unwrap();
        let snapshot = ManifestSnapshot::read(&path, Mode::Exhaustive).unwrap();
        assert_eq!(snapshot.sha256(), hash(&manifest_bytes));
        let sources = snapshot.load_sources().unwrap();
        assert_eq!(sources.len(), 3);
        assert!(Arc::ptr_eq(
            &sources[0].content.text,
            &sources[2].content.text
        ));
        fs::write(d.root.join("models/base.relca"), "changed after capture").unwrap();
        for source in sources {
            assert_eq!(source.source().as_bytes(), bytes);
            assert_eq!(source.sha256(), hash(bytes));
        }
    }
    #[test]
    fn parent_absolute_url_missing_directory_and_escaping_symlink_are_refused() {
        let d = Directory::new();
        fs::write(d.root.join("outside.relca"), "outside manifest root").unwrap();
        std::os::unix::fs::symlink("../outside.relca", d.root.join("models/escape.relca")).unwrap();
        for path in [
            "../outside.relca",
            "/absolute.relca",
            "https://example.invalid/model.relca",
            "missing.relca",
            ".",
            "escape.relca",
        ] {
            let manifest = ManifestSnapshot::read(&d.manifest(&[path]), Mode::Exhaustive).unwrap();
            let error = manifest.load_sources().unwrap_err();
            assert_eq!(error.kind, FailureKind::InputUnavailable, "{path}");
            assert_eq!(error.point.as_deref(), Some("p0"));
            assert_eq!(manifest.manifest().points.len(), 1);
        }
    }
    #[test]
    fn individual_and_total_byte_caps_are_inclusive_without_returning_a_prefix() {
        let d = Directory::new();
        let mut bytes = vec![b' '; MAX_SOURCE_BYTES];
        fs::write(d.root.join("models/base.relca"), &bytes).unwrap();
        let snapshot =
            ManifestSnapshot::read(&d.manifest(&["base.relca"]), Mode::Exhaustive).unwrap();
        assert_eq!(
            snapshot.load_sources().unwrap()[0].source().len(),
            MAX_SOURCE_BYTES
        );
        bytes.push(b' ');
        fs::write(d.root.join("models/base.relca"), bytes).unwrap();
        assert_eq!(
            snapshot.load_sources().unwrap_err().kind,
            FailureKind::CapacityLimit
        );
        fs::write(
            d.root.join("models/base.relca"),
            vec![b' '; MAX_SOURCE_BYTES],
        )
        .unwrap();
        let paths = vec!["base.relca"; 16];
        let exact = ManifestSnapshot::read(&d.manifest(&paths), Mode::Exhaustive).unwrap();
        assert_eq!(exact.load_sources().unwrap().len(), 16);
        let paths = vec!["base.relca"; 17];
        let over = ManifestSnapshot::read(&d.manifest(&paths), Mode::Exhaustive).unwrap();
        assert_eq!(
            over.load_sources().unwrap_err().kind,
            FailureKind::CapacityLimit
        );
        assert_eq!(over.manifest().points.len(), 17);
    }
    #[test]
    fn bad_utf8_and_replaced_opened_paths_are_not_successful_inputs() {
        let d = Directory::new();
        let path = d.root.join("models/base.relca");
        fs::write(&path, [255]).unwrap();
        let manifest =
            ManifestSnapshot::read(&d.manifest(&["base.relca"]), Mode::Exhaustive).unwrap();
        assert_eq!(
            manifest.load_sources().unwrap_err().kind,
            FailureKind::InputUnavailable
        );
        let file = File::open(&path).unwrap();
        fs::rename(&path, d.root.join("moved-outside-model-root.relca")).unwrap();
        assert_eq!(
            check_opened(&file, &manifest.root).unwrap_err().kind,
            FailureKind::InputUnavailable
        );
    }
    #[test]
    fn full_manifest_gate_precedes_source_io_and_keeps_known_point_ids() {
        let d = Directory::new();
        // The source does not exist, but the invalid simulation confidence must
        // refuse the manifest before any source lookup is attempted.
        let snapshot =
            ManifestSnapshot::read(&d.manifest(&["missing.relca"]), Mode::Simulation).unwrap();
        assert_eq!(snapshot.manifest().points[0].id, "p0");
        assert_eq!(
            snapshot.validate().unwrap_err().kind,
            FailureKind::ManifestInvalid
        );
        assert_eq!(
            snapshot.load_sources().unwrap_err().kind,
            FailureKind::ManifestInvalid
        );
    }
}
