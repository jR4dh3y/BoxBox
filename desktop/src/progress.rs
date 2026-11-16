//! Progress and cancellation for a file transfer. A transfer runs start to
//! finish on one background thread, so it sets a scope for that thread and the
//! copy loop reports bytes into it and stops when it is cancelled, without
//! threading a handle through every provider call. The UI reads the counters.

use std::cell::RefCell;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{bail, Result};

#[derive(Clone, Default)]
pub struct Progress {
    done: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    cancelled: Arc<AtomicBool>,
}

impl Progress {
    pub fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }

    /// Zero until the sources are measured.
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Runs `work` with this progress as the thread's current transfer.
    pub fn scope<T>(&self, work: impl FnOnce() -> T) -> T {
        CURRENT.with(|current| *current.borrow_mut() = Some(self.clone()));
        let result = work();
        CURRENT.with(|current| *current.borrow_mut() = None);
        result
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Progress>> = const { RefCell::new(None) };
}

fn with_current(apply: impl FnOnce(&Progress)) {
    CURRENT.with(|current| {
        if let Some(progress) = current.borrow().as_ref() {
            apply(progress);
        }
    });
}

/// Counts copied bytes towards the current transfer.
pub fn advance(bytes: u64) {
    with_current(|progress| {
        progress.done.fetch_add(bytes, Ordering::Relaxed);
    });
}

/// Fails once the current transfer is cancelled; copy loops call it between chunks.
pub fn check() -> Result<()> {
    let mut cancelled = false;
    with_current(|progress| cancelled = progress.is_cancelled());
    if cancelled {
        bail!("Cancelled");
    }
    Ok(())
}

/// Adds the size of everything under `path` (without following links) to the total.
pub fn measure(path: &Path) {
    let mut bytes = 0u64;
    let mut pending = vec![path.to_path_buf()];
    while let Some(path) = pending.pop() {
        let Ok(metadata) = std::fs::symlink_metadata(&path) else { continue };
        if metadata.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                pending.extend(entries.flatten().map(|entry| entry.path()));
            }
        } else if metadata.is_file() {
            bytes += metadata.len();
        }
    }
    with_current(|progress| {
        progress.total.fetch_add(bytes, Ordering::Relaxed);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scope_counts_and_cancels_only_its_own_thread() {
        let progress = Progress::default();
        progress.scope(|| {
            advance(10);
            assert!(check().is_ok());
            progress.cancel();
            assert!(check().is_err());
        });
        advance(5);
        assert!(check().is_ok());
        assert_eq!(progress.done(), 10);
    }
}
