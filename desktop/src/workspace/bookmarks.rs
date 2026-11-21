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
        self.watch_bookmarks();
        let edits = self.bookmark_edits;
        cx.spawn(async move |this, cx| {
            let read = cx.background_executor().spawn(async move { read_bookmarks(&api) }).await;
            this.update(cx, |this, cx| {
                let edited = this.bookmark_edits != edits;
                match read {
                    // Edited here meanwhile: the edit is newer and was saved over this read.
                    Some(_) if edited && this.bookmarks_read => {}
                    Some(read) => {
                        // Pins made before the first read finished join what the file holds.
                        let pinned = if edited { this.favorites.local() } else { Vec::new() };
                        this.bookmarks_read = true;
                        this.favorites.set_local(read);
                        for favorite in pinned {
                            this.favorites.add(favorite);
                        }
                        if edited {
                            this.save_bookmarks(cx);
                        }
                    }
                    None => {
                        this.bookmarks_read = false;
                        this.favorites.set_local(Vec::new());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Watches the bookmarks file's folder, which GTK creates with the first bookmark.
    fn watch_bookmarks(&mut self) {
        if let Some(folder) = super::favorites::bookmarks_path().parent() {
            self.directory_watches.watch(BOOKMARKS_WATCH, folder.to_path_buf());
        }
    }

    pub(crate) fn save_bookmarks(&mut self, cx: &mut Context<Self>) {
        self.bookmark_edits += 1;
        // Until the file is read, writing it would lose what it holds; the read saves them.
        if !self.bookmarks_read {
            return;
        }
        let Some(api) = self.local_api() else { return };
        let local = self.favorites.local();
        let generation = BOOKMARK_WRITES.fetch_add(1, Ordering::SeqCst) + 1;
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { write_bookmarks(&api, &local, generation) }).await;
            this.update(cx, |this, cx| match result {
                // The first bookmark may have just made the folder, which can only be watched now.
                Ok(()) => this.watch_bookmarks(),
                Err(error) => this.report(Err(format!("Could not save bookmarks: {error:#}")), cx),
            })
            .ok();
        })
        .detach();
    }

    /// A favorite dropped on another moves to its place.
    pub(crate) fn drop_favorite(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if self.favorites.reorder(from, to) {
            self.save_favorites(false, cx);
            cx.notify();
        }
    }
}
