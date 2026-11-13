//! Settings files under `$XDG_CONFIG_HOME/boxbox-gpui`, written off the UI thread.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, LazyLock, Mutex,
    },
};

pub(crate) fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("boxbox-gpui")
}

/// Writes through a temporary file and a rename, so a crash leaves the old or the new file.
pub(crate) fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    static TEMP_ID: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
    fs::create_dir_all(parent)?;
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("file");
    let (temporary, mut file) = loop {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".{name}.{}-{id}.tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&temporary) {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// One settings file. Writes run on a background thread one at a time, and a
/// write that a newer one has replaced is skipped, so the newest state wins.
pub(crate) struct Store {
    path: PathBuf,
    latest: Arc<AtomicU64>,
    lock: Arc<Mutex<()>>,
}

/// What a background write did.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Written {
    Saved,
    /// A newer write was queued, so this one was skipped.
    Superseded,
}

impl Store {
    fn new(name: &str) -> Self {
        Self { path: config_dir().join(name), latest: Arc::default(), lock: Arc::default() }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Call on the UI thread; run the returned work on a background thread.
    pub(crate) fn write(&self, bytes: Vec<u8>) -> impl FnOnce() -> io::Result<Written> + Send + 'static {
        let id = self.latest.fetch_add(1, Ordering::SeqCst) + 1;
        let (path, latest, lock) = (self.path.clone(), self.latest.clone(), self.lock.clone());
        move || {
            let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if latest.load(Ordering::SeqCst) != id {
                return Ok(Written::Superseded);
            }
            atomic_write(&path, &bytes).map(|()| Written::Saved)
        }
    }
}

pub(crate) static PREFERENCES: LazyLock<Store> = LazyLock::new(|| Store::new("settings.json"));
pub(crate) static DESKTOP_SETTINGS: LazyLock<Store> = LazyLock::new(|| Store::new("desktop-settings.json"));
pub(crate) static FAVORITES: LazyLock<Store> = LazyLock::new(|| Store::new("favorites.json"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_newest_queued_write_lands() {
        let dir = std::env::temp_dir().join(format!("boxbox-persist-{}", std::process::id()));
        let store = Store { path: dir.join("a.json"), latest: Arc::default(), lock: Arc::default() };
        let first = store.write(b"1".to_vec());
        let second = store.write(b"2".to_vec());
        assert_eq!(second().unwrap(), Written::Saved);
        assert_eq!(first().unwrap(), Written::Superseded);
        assert_eq!(fs::read(store.path()).unwrap(), b"2");
        let _ = fs::remove_dir_all(dir);
    }
}
