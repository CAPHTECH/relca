//! Private, opt-in diagnostics. Never part of a state, budget, or public wire.
//! Set RELCA_PRIVATE_PERF_LOG to a caller-owned file to collect inclusive wall
//! times. Nested stages overlap; these are not additive CPU measurements.
//!
//! Only a regular file (or a new file created exclusively) is a supported sink.
//! Existing symlinks, devices, pipes, sockets and directories are rejected before
//! opening; sink errors are ignored rather than reported on stdout or stderr.
//!
//! Coverage is intentionally limited: samples are thread-local and only
//! report::seal flushes them. report::seal_simulation and compare's own seal do
//! not flush, although an exhaustive child report may do so. Direct engine or
//! reference calls need a later report::seal on that thread to emit samples.
//! Spans still open at the last flush remain buffered; reference worker-thread
//! samples are not merged into the parent and are lost when the worker exits.
//! The parent's reference.total and frame counter still include that work.
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, Instant},
};

fn destination() -> Option<&'static PathBuf> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| std::env::var_os("RELCA_PRIVATE_PERF_LOG").map(PathBuf::from))
        .as_ref()
}

#[derive(Default, serde::Serialize)]
struct Sample {
    calls: u64,
    wall_ns: u128,
    counters: BTreeMap<&'static str, usize>,
}
thread_local! {
    static SAMPLES: RefCell<BTreeMap<&'static str, Sample>> = RefCell::default();
}

pub(crate) struct Span {
    stage: &'static str,
    start: Option<Instant>,
}
pub(crate) fn span(stage: &'static str) -> Span {
    Span {
        stage,
        start: destination().map(|_| Instant::now()),
    }
}
fn add(stage: &'static str, elapsed: Duration) {
    SAMPLES.with_borrow_mut(|samples| {
        let sample = samples.entry(stage).or_default();
        sample.calls += 1;
        sample.wall_ns += elapsed.as_nanos();
    });
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            add(self.stage, start.elapsed());
        }
    }
}
pub(crate) fn count(stage: &'static str, name: &'static str, value: usize) {
    if destination().is_some() {
        SAMPLES.with_borrow_mut(|samples| {
            *samples
                .entry(stage)
                .or_default()
                .counters
                .entry(name)
                .or_default() += value;
        });
    }
}
pub(crate) fn enabled() -> bool {
    destination().is_some()
}

fn open_sink(path: &Path) -> Option<std::fs::File> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            // Check before open: opening a FIFO for append can block even if
            // writing is later refused. Do not create a disappeared existing
            // file, and check the opened handle again before any write.
            let file = std::fs::OpenOptions::new().append(true).open(path).ok()?;
            file.metadata().ok()?.is_file().then_some(file)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Exclusive creation also refuses a file/symlink installed after
            // the absence check, instead of following it to a special sink.
            std::fs::OpenOptions::new()
                .append(true)
                .create_new(true)
                .open(path)
                .ok()
        }
        Ok(_) | Err(_) => None,
    }
}

pub(crate) fn flush() {
    let Some(path) = destination() else { return };
    // Diagnostics are best effort: an unavailable sink cannot change a verdict,
    // stderr, exploration, random draw, or the caller's exit status.
    SAMPLES.with_borrow_mut(|samples| {
        if let Some(mut file) = open_sink(path) {
            for (stage, sample) in samples.iter() {
                let row = serde_json::json!({"stage": stage, "sample": sample});
                let _ = writeln!(file, "{row}");
            }
        }
        samples.clear();
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn aggregation_preserves_counts_and_accumulates_inclusive_time() {
        super::SAMPLES.with_borrow_mut(|s| s.clear());
        super::add("test", std::time::Duration::from_nanos(7));
        super::add("test", std::time::Duration::from_nanos(11));
        super::SAMPLES.with_borrow_mut(|s| {
            let sample = s.remove("test").unwrap();
            assert_eq!(sample.calls, 2);
            assert_eq!(sample.wall_ns, 18);
        });
    }
}
