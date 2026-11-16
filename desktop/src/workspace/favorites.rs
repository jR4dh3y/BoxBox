//! Favorites in the sidebar. Local ones are the GTK bookmarks file
//! (`$XDG_CONFIG_HOME/gtk-3.0/bookmarks`), which Nautilus, Thunar and the GTK
//! file chooser share; favorites on a server are BoxBox's own.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::api::Api;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Favorite {
    pub name: String,
    /// A virtual path; for a local bookmark of a network location, its address.
    pub path: String,
    pub provider: FavoriteProvider,
    #[serde(default)]
    pub connection_id: Option<String>,
    /// The bookmark's URI as GTK wrote it, kept so a folder that is away (an
    /// unplugged drive) is written back unchanged.
    #[serde(skip)]
    pub uri: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "server_url", rename_all = "snake_case")]
pub(crate) enum FavoriteProvider {
    Local,
    Remote(String),
}

impl FavoriteProvider {
    pub fn for_api(api: &Api) -> Self {
        match api.server_url() {
            Some(url) => Self::Remote(url.to_string()),
            None => Self::Local,
        }
    }
}

/// Local bookmarks first, in the file's order, then favorites on servers.
#[derive(Default)]
pub(crate) struct Favorites {
    pub entries: Vec<Favorite>,
}

impl Favorites {
    /// Favorites on servers. Local bookmarks are read with the local provider.
    pub fn load() -> Self {
        let entries = fs::read(crate::persist::FAVORITES.path())
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<Favorite>>(&bytes).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|favorite| favorite.provider != FavoriteProvider::Local)
            .collect();
        Self { entries }
    }

    /// Replaces the local bookmarks, keeping favorites on servers after them.
    pub fn set_local(&mut self, local: Vec<Favorite>) {
        self.entries.retain(|favorite| favorite.provider != FavoriteProvider::Local);
        self.entries.splice(0..0, local);
    }

    pub fn local(&self) -> Vec<Favorite> {
        self.entries.iter().filter(|favorite| favorite.provider == FavoriteProvider::Local).cloned().collect()
    }

    pub fn add(&mut self, favorite: Favorite) {
        if let Some(existing) = self.entries.iter_mut().find(|existing| {
            existing.path == favorite.path && existing.provider == favorite.provider
        }) {
            existing.name = favorite.name;
            existing.connection_id = favorite.connection_id;
        } else {
            // A new local bookmark goes after the other local ones.
            let at = match favorite.provider {
                FavoriteProvider::Local => self.entries.iter().filter(|existing| existing.provider == FavoriteProvider::Local).count(),
                FavoriteProvider::Remote(_) => self.entries.len(),
            };
            self.entries.insert(at, favorite);
        }
    }

    pub fn remove(&mut self, favorite: &Favorite) {
        self.entries.retain(|existing| {
            existing.path != favorite.path || existing.provider != favorite.provider
        });
    }

    /// Moves entry `from` to where entry `to` is, within its own kind (local or server).
    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        let (Some(moving), Some(target)) = (self.entries.get(from), self.entries.get(to)) else { return false };
        let local = |favorite: &Favorite| favorite.provider == FavoriteProvider::Local;
        if from == to || local(moving) != local(target) {
            return false;
        }
        let entry = self.entries.remove(from);
        self.entries.insert(to, entry);
        true
    }

    /// The favorites on servers, for BoxBox's own file.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let remote: Vec<&Favorite> = self.entries.iter().filter(|favorite| favorite.provider != FavoriteProvider::Local).collect();
        Ok(serde_json::to_vec_pretty(&remote)?)
    }
}

/// Where GTK keeps bookmarks, read when needed so tests can point it elsewhere.
pub(crate) fn bookmarks_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_default()
        .join("gtk-3.0/bookmarks")
}

/// The bookmarks as favorites: a folder by its virtual path, another location by
/// its address. Each line is a URI, then optionally a space and a label. `None`
/// when the file exists but cannot be read, so it is not overwritten.
pub(crate) fn read_bookmarks(api: &Api) -> Option<Vec<Favorite>> {
    let text = match fs::read_to_string(bookmarks_path()) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            log::warn!("Could not read GTK bookmarks: {error}");
            return None;
        }
    };
    let favorites = text
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let (uri, label) = line.split_once(' ').map_or((line, None), |(uri, label)| (uri, Some(label.trim().to_string())));
            if uri.is_empty() {
                return None;
            }
            let real = crate::os_clipboard::path_from_uri(uri);
            let path = real.as_deref().and_then(|real| api.virtual_path(real)).unwrap_or_else(|| uri.to_string());
            let fallback = match &real {
                Some(real) => real.file_name().map_or_else(|| real.display().to_string(), |name| name.to_string_lossy().into_owned()),
                None => uri.split("://").nth(1).unwrap_or(uri).trim_end_matches('/').to_string(),
            };
            let name = label.filter(|label| !label.is_empty()).unwrap_or(fallback);
            Some(Favorite { name, path, provider: FavoriteProvider::Local, connection_id: None, uri: Some(uri.to_string()) })
        })
        .collect();
    Some(favorites)
}

/// Writes the bookmarks file from the local favorites. Run off the UI thread; of
/// writes queued together only the newest lands.
pub(crate) fn write_bookmarks(api: &Api, local: &[Favorite], generation: u64) -> Result<()> {
    static LOCK: Mutex<()> = Mutex::new(());
    let _turn = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if BOOKMARK_WRITES.load(Ordering::SeqCst) != generation {
        return Ok(());
    }
    let mut text = String::new();
    for favorite in local {
        let uri = match (&favorite.uri, api.local_path(&favorite.path)) {
            (Some(uri), _) => uri.clone(),
            (None, Ok(Some(real))) => crate::os_clipboard::file_uri(&real),
            (None, _) => continue,
        };
        // A label only when it differs from the name GTK would show anyway.
        let shown = crate::os_clipboard::path_from_uri(&uri).and_then(|real| real.file_name().map(|name| name.to_string_lossy().into_owned()));
        match shown.as_deref() == Some(favorite.name.as_str()) {
            true => text.push_str(&uri),
            false => text.push_str(&format!("{uri} {}", favorite.name)),
        }
        text.push('\n');
    }
    crate::persist::atomic_write(&bookmarks_path(), text.as_bytes())?;
    Ok(())
}

/// Numbers bookmark writes, so a write that a newer one replaced is skipped.
pub(crate) static BOOKMARK_WRITES: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
mod tests {
    use super::*;

    fn favorite(name: &str, provider: FavoriteProvider) -> Favorite {
        Favorite { name: name.into(), path: name.into(), provider, connection_id: None, uri: None }
    }

    #[test]
    fn local_and_server_favorites_keep_to_their_own_part() {
        let server = FavoriteProvider::Remote("http://nas".into());
        let mut favorites = Favorites::default();
        favorites.add(favorite("s", server.clone()));
        favorites.add(favorite("a", FavoriteProvider::Local));
        favorites.add(favorite("b", FavoriteProvider::Local));
        let names = |favorites: &Favorites| favorites.entries.iter().map(|favorite| favorite.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&favorites), ["a", "b", "s"]);
        assert!(favorites.reorder(1, 0));
        assert_eq!(names(&favorites), ["b", "a", "s"]);
        assert!(!favorites.reorder(2, 0), "a server favorite does not move among bookmarks");
        favorites.set_local(vec![favorite("c", FavoriteProvider::Local)]);
        assert_eq!(names(&favorites), ["c", "s"]);
        let saved: Vec<Favorite> = serde_json::from_slice(&favorites.encode().unwrap()).unwrap();
        assert_eq!(saved.len(), 1, "only server favorites go in BoxBox's file");
    }
}
