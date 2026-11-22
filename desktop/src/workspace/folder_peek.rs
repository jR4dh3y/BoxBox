//! What a folder holds, drawn inside its tile as Windows shows a folder's
//! contents: a few of its items, pictures first. An empty folder stays plain.
//!
//! Read on the thumbnail threads and cached by the folder's modified time, which
//! changes when an entry is added to it or removed from it. Editing a picture in
//! place does not change it, so a folder listed again reads its preview again,
//! showing the one it has until then.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{Context as _, Result};
use gpui::Context;

use super::favorites::FavoriteProvider;
use super::operations::PaneTarget;
use super::thumbnails::has_inline_thumbnail;
use super::Workspace;
use crate::api::{Api, FileInfo};

/// The items drawn inside a folder.
pub(crate) const SHOWN: usize = 3;
/// The entries read to choose them, so a huge folder costs no more than a small one.
const SCANNED: usize = 64;
/// More folders than a screen of tiles shows; past it the cache starts again.
const CAPACITY: usize = 1024;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct PeekKey {
    provider: FavoriteProvider,
    path: String,
    mod_time: Option<i64>,
    show_hidden: bool,
}

#[derive(Default)]
pub(crate) struct FolderPeeks {
    peeks: HashMap<PeekKey, Vec<FileInfo>>,
    pending: HashSet<PeekKey>,
    /// Read since a listing last loaded.
    fresh: HashSet<PeekKey>,
}

impl FolderPeeks {
    pub fn clear(&mut self) {
        self.peeks.clear();
        self.pending.clear();
        self.fresh.clear();
    }

    /// A listing loaded: the previews it shows are read again when drawn.
    pub fn revalidate(&mut self) {
        self.fresh.clear();
    }
}

/// The same items, unchanged since read.
fn same(a: &[FileInfo], b: &[FileInfo]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.path == b.path && a.mod_time == b.mod_time && a.size == b.size)
}

/// Up to `SHOWN` items of `folder`, pictures and videos first, then by name,
/// leaving out hidden ones as the folder's listing does.
fn read(api: &Api, folder: &str, show_hidden: bool) -> Result<Vec<FileInfo>> {
    let real = api.local_path(folder)?.context("not a local folder")?;
    let mut names: Vec<(bool, String)> = std::fs::read_dir(real)?
        .flatten()
        .take(SCANNED)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| show_hidden || !name.starts_with('.'))
        .map(|name| (!has_inline_thumbnail(&name), name))
        .collect();
    names.sort_by(|(a_plain, a), (b_plain, b)| a_plain.cmp(b_plain).then_with(|| a.to_lowercase().cmp(&b.to_lowercase())));
    Ok(names
        .into_iter()
        .filter_map(|(_, name)| api.stat(&format!("{folder}/{name}")).ok())
        .filter(|item| show_hidden || !item.is_hidden())
        .take(SHOWN)
        .collect())
}

impl Workspace {
    /// Previews draw on BoxBox's folder; the desktop's icon theme keeps its own folders.
    pub(crate) fn folder_previews_shown(&self) -> bool {
        self.preferences.folder_previews && self.preferences.file_icons != super::IconStyle::System
    }

    fn peek_key(&self, api: &Api, folder: &FileInfo) -> PeekKey {
        PeekKey {
            provider: FavoriteProvider::for_api(api),
            path: folder.path.clone(),
            mod_time: folder.mod_time,
            show_hidden: self.preferences.show_hidden_files,
        }
    }

    /// The items to draw inside `folder`'s tile, once they are read; empty for an empty folder.
    pub(crate) fn folder_peek(&self, api: &Api, folder: &FileInfo) -> Option<&[FileInfo]> {
        self.folder_peeks.peeks.get(&self.peek_key(api, folder)).map(Vec::as_slice)
    }

    /// Reads what a local folder holds, unless it is known or being read, and asks
    /// for the thumbnails of what it shows.
    pub(crate) fn request_folder_peek(&mut self, target: PaneTarget, folder: &FileInfo, cx: &mut Context<Self>) {
        if !self.folder_previews_shown() {
            return;
        }
        let Some(api) = self.pane_by_id(target.0).map(|pane| pane.api.clone()).filter(|api| api.is_local() && folder.is_dir) else { return };
        let key = self.peek_key(&api, folder);
        if let Some(items) = self.folder_peeks.peeks.get(&key) {
            for item in items.clone() {
                self.request_thumbnail(target, item, cx);
            }
            if self.folder_peeks.fresh.contains(&key) {
                return;
            }
        }
        if !self.folder_peeks.pending.insert(key.clone()) {
            return;
        }
        let (path, show_hidden, reader) = (folder.path.clone(), key.show_hidden, Arc::clone(&api));
        let read = crate::thumbnail_queue::render(move || read(&reader, &path, show_hidden));
        cx.spawn(async move |this, cx| {
            let result = read.await;
            this.update(cx, |this, cx| {
                // A cleared cache no longer waits for it.
                if !this.folder_peeks.pending.remove(&key) {
                    return;
                }
                let peeks = &mut this.folder_peeks;
                match result {
                    Some(Ok(items)) => {
                        peeks.fresh.insert(key.clone());
                        if peeks.peeks.get(&key).is_some_and(|known| same(known, &items)) {
                            return;
                        }
                        if peeks.peeks.len() >= CAPACITY {
                            peeks.peeks.clear();
                            peeks.fresh.clear();
                        }
                        peeks.peeks.insert(key, items);
                        cx.notify();
                    }
                    // Unreadable: drawn as a plain folder, or as it last was.
                    Some(Err(_)) => {
                        peeks.fresh.insert(key.clone());
                        peeks.peeks.entry(key).or_default();
                    }
                    // Dropped from the queue: asked again when drawn.
                    None => {}
                }
            })
            .ok();
        })
        .detach();
    }
}
