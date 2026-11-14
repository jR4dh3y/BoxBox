//! Local favorites as GTK bookmarks: read at start and whenever another app
//! changes them, written when they change here, reordered by dragging.

use std::sync::atomic::Ordering;

use gpui::Context;

use super::favorites::{read_bookmarks, write_bookmarks, BOOKMARK_WRITES};
use super::Workspace;

/// The directory watch that stands for the bookmarks file's folder.
pub(crate) const BOOKMARKS_WATCH: usize = usize::MAX - 1;

/// A favorite being dragged to a new place in the list.
#[derive(Clone, Copy)]
pub(crate) struct FavoriteDrag(pub usize);

impl Workspace {
    pub(crate) fn load_bookmarks(&mut self, cx: &mut Context<Self>) {
        let Some(api) = self.local_api() else { return };
        if let Some(folder) = super::favorites::bookmarks_path().parent() {
            self.directory_watches.watch(BOOKMARKS_WATCH, folder.to_path_buf());
        }
        cx.spawn(async move |this, cx| {
            let local = cx.background_executor().spawn(async move { read_bookmarks(&api) }).await;
            this.update(cx, |this, cx| {
                this.bookmarks_read = local.is_some();
                this.favorites.set_local(local.unwrap_or_default());
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn save_bookmarks(&mut self, cx: &mut Context<Self>) {
        // Until the file is read, writing it would lose what it holds.
        if !self.bookmarks_read {
            return;
        }
        let Some(api) = self.local_api() else { return };
        let local = self.favorites.local();
        let generation = BOOKMARK_WRITES.fetch_add(1, Ordering::SeqCst) + 1;
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { write_bookmarks(&api, &local, generation) }).await;
            if let Err(error) = result {
                this.update(cx, |this, cx| this.report(Err(format!("Could not save bookmarks: {error:#}")), cx)).ok();
            }
        })
        .detach();
    }

    /// A favorite dropped on another moves to its place.
    pub(crate) fn drop_favorite(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if self.favorites.reorder(from, to) {
            self.save_favorites(None, cx);
            cx.notify();
        }
    }
}
