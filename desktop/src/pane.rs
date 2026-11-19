//! One tab's browse state: location, history, listing, sort, view and selection.
//! Mirrors `lib/browse/location.svelte.ts` and `selection.svelte.ts`.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gpui::{Modifiers, Pixels, Point, UniformListScrollHandle};

use crate::api::{Api, DriveStats, FileInfo, MountPoint};
use crate::search;
use crate::sort::{sort_files, Sort};

/// A folder in a pane's history, on the provider that holds it: a tab moves
/// between this computer and paired servers.
#[derive(Clone)]
pub struct Place {
    pub api: Arc<Api>,
    pub path: String,
    /// How the folder looked when the pane left it; not part of what the place is.
    pub view: ViewMemo,
}

/// A folder's selection, cursor and scroll position, kept in history so Back and
/// Forward return to them, as other file managers do.
#[derive(Clone, Default, Debug)]
pub struct ViewMemo {
    selected: HashSet<String>,
    active: Option<String>,
    list_offset: Point<Pixels>,
    grid_offset: Point<Pixels>,
}

impl PartialEq for Place {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.api, &other.api) && self.path == other.path
    }
}

/// What `Pane::navigate_to` did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Moved {
    /// The place was already open.
    No,
    Within,
    /// To another provider, whose drives the pane must load.
    Across,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct View {
    pub sort: Sort,
    pub show_hidden_files: bool,
    /// Search results in the order the search ranked them; `sort` is not applied.
    pub ranked: bool,
}

/// Drops hidden entries unless shown, then sorts unless the entries are ranked.
/// Runs off the UI thread.
pub fn arrange(items: Vec<FileInfo>, view: View) -> Vec<FileInfo> {
    let mut items: Vec<FileInfo> =
        items.into_iter().filter(|item| view.show_hidden_files || !item.is_hidden()).collect();
    if !view.ranked {
        sort_files(&mut items, view.sort);
    }
    items
}

pub struct SortJob {
    items: Arc<Vec<FileInfo>>,
    folder_items: Arc<Vec<FileInfo>>,
    pub sort: Sort,
    /// `items` are ranked search results, which keep their order.
    ranked: bool,
}

impl SortJob {
    /// Sorted copies of both lists.
    pub fn run(&self) -> (Vec<FileInfo>, Vec<FileInfo>) {
        let sorted = |items: &Vec<FileInfo>| {
            let mut items = items.clone();
            sort_files(&mut items, self.sort);
            items
        };
        let items = if self.ranked { self.items.to_vec() } else { sorted(&self.items) };
        (items, sorted(&self.folder_items))
    }
}

/// How a keyboard move treats the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    /// Select only the item moved to.
    Select,
    /// Shift: select from the anchor to it.
    Extend,
    /// Ctrl: move the cursor and keep the selection.
    Focus,
}

#[derive(Clone)]
pub enum Load<T> {
    Loading,
    Loaded(T),
    Failed(String),
}


#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewMode {
    List,
    Grid,
}

impl ViewMode {
    pub const OPTIONS: [(Self, &'static str); 2] = [(Self::List, "List"), (Self::Grid, "Grid")];
}

pub enum Listing {
    /// The root shows drives, which the workspace loads; no directory request.
    Root,
    Loading,
    Loaded,
    Failed(String),
}

/// Whether `Pane::items` hold the folder or the results of a name search below it.
pub enum Search {
    Off,
    Loading,
    Done,
    Failed(String),
}

pub struct Pane {
    pub id: usize,
    /// Virtual path; "" is the Overview.
    pub path: String,
    /// Provider identity and operations for this pane; another pane may use another provider.
    pub(crate) api: std::sync::Arc<Api>,
    pub(crate) roots: Load<Vec<MountPoint>>,
    pub(crate) drives: Load<Vec<DriveStats>>,
    back: Vec<Place>,
    forward: Vec<Place>,
    pub listing: Listing,
    /// Whether the open folder takes new entries, renames and deletions.
    pub writable: bool,
    /// Where the open folder is on this computer, for a local provider.
    pub real_path: Option<std::path::PathBuf>,
    /// Visible entries in display order (hidden files removed, then sorted): the folder, or the
    /// search results while a search is on. Shared, so sorting can run on a
    /// background thread without copying on the UI thread.
    pub items: Arc<Vec<FileInfo>>,
    pub search_input: String,
    pub search: Search,
    /// Results stay in the search's own order, best match first, until a column is
    /// clicked; then they sort like a folder.
    pub search_ranked: bool,
    /// Stops the walk of the search in flight.
    search_cancel: Option<Arc<AtomicBool>>,
    /// The folder's entries while `items` hold search results.
    folder_items: Arc<Vec<FileInfo>>,
    pub sort: Sort,
    pub view_mode: ViewMode,
    pub show_hidden_files: bool,
    pub drive_view_mode: ViewMode,
    pub selected: HashSet<String>,
    anchor: Option<usize>,
    pub active: Option<usize>,
    /// Bumped per request, so a late response for an old folder is ignored.
    pub generation: u64,
    /// A folder listing was asked for and has not arrived.
    pub busy: bool,
    /// The provider's drive list was asked for and has not arrived.
    pub drives_busy: bool,
    /// Bumped per input change and request, so a late search response is ignored.
    pub search_generation: u64,
    pub list_scroll: UniformListScrollHandle,
    pub grid_scroll: UniformListScrollHandle,
    /// A history entry's view, put back when its listing arrives.
    restore: Option<ViewMemo>,
    /// Set on a copy of another pane, until it leaves the folder it opened on:
    /// the screen did not change, so its contents do not fade up.
    pub copied: bool,
    /// Which way the pane last moved, and a count of moves, so the contents
    /// arrive from that side once per move.
    pub arrival: crate::components::motion::Arrival,
    pub moves: u64,
    /// When the first listing after the last move landed, for its items to
    /// cascade in; a refresh of the same folder does not set it again.
    pub listed_at: Option<std::time::Instant>,
    listed_move: u64,
}

impl Pane {
    #[cfg(test)]
    pub fn new(
        id: usize,
        path: String,
        show_hidden_files: bool,
        sort: Sort,
        view_mode: ViewMode,
    ) -> Self {
        Self::with_api(
            id,
            path,
            std::sync::Arc::new(Api::local().expect("local API initialization failed")),
            show_hidden_files,
            sort,
            view_mode,
        )
    }

    pub(crate) fn with_api(
        id: usize,
        path: String,
        api: std::sync::Arc<Api>,
        show_hidden_files: bool,
        sort: Sort,
        view_mode: ViewMode,
    ) -> Self {
        Self {
            id,
            listing: if path.is_empty() {
                Listing::Root
            } else {
                Listing::Loading
            },
            path,
            api,
            writable: true,
            real_path: None,
            roots: Load::Loading,
            drives: Load::Loading,
            back: Vec::new(),
            forward: Vec::new(),
            items: Arc::default(),
            search_input: String::new(),
            search: Search::Off,
            search_ranked: true,
            search_cancel: None,
            folder_items: Arc::default(),
            sort,
            view_mode,
            show_hidden_files,
            drive_view_mode: ViewMode::Grid,
            selected: HashSet::new(),
            anchor: None,
            active: None,
            generation: 0,
            busy: false,
            drives_busy: false,
            search_generation: 0,
            list_scroll: UniformListScrollHandle::new(),
            grid_scroll: UniformListScrollHandle::new(),
            restore: None,
            copied: false,
            arrival: Default::default(),
            moves: 0,
            listed_at: None,
            listed_move: 0,
        }
    }

    /// A new pane at this one's place, as a new tab opens: the same folder,
    /// listing, drives and view, shown at once while the caller reads them again
    /// behind it. History, selection and search stay with this pane.
    pub(crate) fn duplicate(&self, id: usize) -> Self {
        let mut pane = Self::with_api(id, self.path.clone(), self.api.clone(), self.show_hidden_files, self.sort, self.view_mode);
        pane.drive_view_mode = self.drive_view_mode;
        pane.roots = self.roots.clone();
        pane.drives = self.drives.clone();
        pane.writable = self.writable;
        pane.real_path = self.real_path.clone();
        // While searching, `items` are the results; the copy shows the folder from
        // its top. Otherwise it keeps this pane's scroll, so the screen holds still.
        pane.items = if self.is_searching() { self.folder_items.clone() } else { self.items.clone() };
        if !self.is_searching() {
            let memo = self.view_memo();
            pane.set_scroll(memo.list_offset, memo.grid_offset);
        }
        pane.listing = match self.listing {
            Listing::Root => Listing::Root,
            Listing::Loaded => Listing::Loaded,
            // Not shown yet, or failed: the copy reads it like any folder.
            Listing::Loading | Listing::Failed(_) => Listing::Loading,
        };
        pane.copied = true;
        pane
    }

    pub fn is_at_root(&self) -> bool {
        self.path.is_empty()
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Returns false when `path` is already open.
    #[cfg(test)]
    pub fn navigate(&mut self, path: String) -> bool {
        self.navigate_to(self.api.clone(), path) != Moved::No
    }

    pub fn place(&self) -> Place {
        Place { api: self.api.clone(), path: self.path.clone(), view: ViewMemo::default() }
    }

    fn view_memo(&self) -> ViewMemo {
        let offset = |handle: &UniformListScrollHandle| handle.0.borrow().base_handle.offset();
        ViewMemo {
            selected: self.selected.clone(),
            active: self.active.and_then(|index| self.items.get(index)).map(|item| item.path.clone()),
            list_offset: offset(&self.list_scroll),
            grid_offset: offset(&self.grid_scroll),
        }
    }

    /// Puts back a folder's selection, cursor and scroll position once its
    /// entries are in; an entry gone since is left out.
    fn restore_view(&mut self, memo: ViewMemo) {
        self.selected = memo.selected.into_iter().filter(|path| self.index_of(path).is_some()).collect();
        self.active = memo.active.and_then(|path| self.index_of(&path));
        self.anchor = self.active;
        self.set_scroll(memo.list_offset, memo.grid_offset);
    }

    fn set_scroll(&self, list: Point<Pixels>, grid: Point<Pixels>) {
        self.list_scroll.0.borrow().base_handle.set_offset(list);
        self.grid_scroll.0.borrow().base_handle.set_offset(grid);
    }

    /// Opens `path` on `api`, which may be another provider than the current one.
    pub fn navigate_to(&mut self, api: Arc<Api>, path: String) -> Moved {
        let place = Place { api, path, view: ViewMemo::default() };
        if place == self.place() {
            return Moved::No;
        }
        let previous = self.enter(place);
        self.back.push(previous);
        self.forward.clear();
        self.moved_from(&self.back.last().expect("just pushed").api.clone())
    }

    /// Makes `place` current and returns the place it replaces.
    fn enter(&mut self, place: Place) -> Place {
        self.arrival = crate::components::motion::Arrival::between(&self.path, &place.path, Arc::ptr_eq(&self.api, &place.api));
        self.moves += 1;
        let view = self.view_memo();
        let previous = Place { api: std::mem::replace(&mut self.api, place.api), path: std::mem::replace(&mut self.path, place.path), view };
        self.reset();
        // A folder opens at its top; `travel` puts one from history back.
        self.set_scroll(Point::default(), Point::default());
        self.restore = None;
        previous
    }

    fn moved_from(&mut self, previous: &Arc<Api>) -> Moved {
        if Arc::ptr_eq(previous, &self.api) {
            return Moved::Within;
        }
        self.roots = Load::Loading;
        self.drives = Load::Loading;
        Moved::Across
    }

    /// Places Back would go to, nearest first, and the same for Forward.
    pub fn history(&self) -> (Vec<Place>, Vec<Place>) {
        (self.back.iter().rev().cloned().collect(), self.forward.iter().rev().cloned().collect())
    }

    /// Drops `missing` places from both histories, then moves `steps` places back or
    /// forward.
    pub fn travel(&mut self, back: bool, steps: usize, missing: &[Place]) -> Moved {
        self.back.retain(|place| !missing.contains(place));
        self.forward.retain(|place| !missing.contains(place));
        let from = if back { &mut self.back } else { &mut self.forward };
        if steps == 0 || from.len() < steps {
            return Moved::No;
        }
        let mut skipped = Vec::with_capacity(steps);
        for _ in 1..steps {
            skipped.push(from.pop().expect("checked above"));
        }
        let mut destination = from.pop().expect("checked above");
        let view = std::mem::take(&mut destination.view);
        let previous = self.enter(destination);
        // History has a direction even between unrelated folders.
        if self.arrival == crate::components::motion::Arrival::InPlace {
            self.arrival = if back { crate::components::motion::Arrival::FromLeft } else { crate::components::motion::Arrival::FromRight };
        }
        // Where it was, once its entries load: an empty list cannot hold a scroll position.
        self.restore = Some(view);
        let to = if back { &mut self.forward } else { &mut self.back };
        to.push(previous.clone());
        to.extend(skipped);
        self.moved_from(&previous.api)
    }

    /// Shows the Overview in place of the folder the pane started on, keeping no
    /// history: for a remembered start folder that is gone.
    pub fn start_on_overview(&mut self) {
        self.path.clear();
        self.reset();
    }

    fn reset(&mut self) {
        self.copied = false;
        self.items = Arc::default();
        self.folder_items = Arc::default();
        self.search_input.clear();
        self.search = Search::Off;
        // A search still in flight is for the old folder.
        self.invalidate_search();
        self.clear_selection();
        self.listing = if self.path.is_empty() {
            Listing::Root
        } else {
            Listing::Loading
        };
    }

    pub fn begin_load(&mut self) -> u64 {
        self.generation += 1;
        self.busy = !self.is_at_root();
        if !self.is_at_root() && self.items.is_empty() {
            self.listing = Listing::Loading;
        }
        self.generation
    }

    /// Takes entries `arrange`d off the UI thread. Returns whether the view changed while
    /// they were prepared, so the caller should sort again.
    pub fn finish_load(&mut self, result: anyhow::Result<Vec<FileInfo>>, view: View) -> bool {
        self.busy = false;
        match result {
            Ok(items) => {
                if self.is_searching() {
                    self.folder_items = Arc::new(items);
                } else {
                    self.replace_items(items);
                    if let Some(memo) = self.restore.take() {
                        self.restore_view(memo);
                    }
                    if self.listed_move != self.moves {
                        self.listed_move = self.moves;
                        self.listed_at = Some(std::time::Instant::now());
                    }
                }
                self.listing = Listing::Loaded;
                view != self.view()
            }
            Err(error) => {
                self.listing = Listing::Failed(format!("{error:#}"));
                false
            }
        }
    }

    /// Whether the items of a listing that just arrived still cascade in: not
    /// on a copied tab, whose screen did not change.
    pub fn cascading(&self) -> bool {
        !self.copied && self.listed_at.is_some_and(|at| at.elapsed() < crate::components::motion::CASCADE_WINDOW)
    }

    /// Whether anything this pane asked for is still out: its listing, its
    /// drives, or a search.
    pub fn loading(&self) -> bool {
        self.busy || self.drives_busy || matches!(self.search, Search::Loading)
    }

    /// What `arrange` needs for a folder listing, read on the UI thread before work
    /// is sent off it.
    pub fn view(&self) -> View {
        View { sort: self.sort, show_hidden_files: self.show_hidden_files, ranked: false }
    }

    /// The same for search results, which may keep the search's order.
    pub fn search_view(&self) -> View {
        View { ranked: self.search_ranked, ..self.view() }
    }

    /// The current entries and how to sort them, for `apply_sort`.
    pub fn sort_job(&self) -> SortJob {
        SortJob {
            items: self.items.clone(),
            folder_items: self.folder_items.clone(),
            sort: self.sort,
            ranked: self.is_searching() && self.search_ranked,
        }
    }

    /// Whether the list shows results in the search's order rather than a column's.
    pub fn shows_ranked(&self) -> bool {
        self.is_searching() && self.search_ranked
    }

    /// Makes any search in flight stale and stops its walk.
    fn invalidate_search(&mut self) {
        self.search_generation += 1;
        if let Some(cancel) = self.search_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Applies a finished `SortJob` unless the entries or the sort changed meanwhile.
    pub fn apply_sort(&mut self, job: &SortJob, items: Vec<FileInfo>, folder_items: Vec<FileInfo>) -> bool {
        if !Arc::ptr_eq(&job.items, &self.items) || !Arc::ptr_eq(&job.folder_items, &self.folder_items) || job.sort != self.sort {
            return false;
        }
        self.items = Arc::new(items);
        self.folder_items = Arc::new(folder_items);
        true
    }

    /// Swaps in new entries, keeping the selection, anchor and active row by path.
    fn replace_items(&mut self, items: Vec<FileInfo>) {
        let path_at = |index: Option<usize>| -> Option<String> {
            index
                .and_then(|i| self.items.get(i))
                .map(|item| item.path.clone())
        };
        let (anchor, active) = (path_at(self.anchor), path_at(self.active));
        self.items = Arc::new(items);
        if !self.selected.is_empty() {
            let paths: HashSet<&str> = self.items.iter().map(|item| item.path.as_str()).collect();
            self.selected.retain(|path| paths.contains(path.as_str()));
        }
        self.anchor = anchor.and_then(|path| self.index_of(&path));
        self.active = active.and_then(|path| self.index_of(&path));
    }

    /// Whether rows say which folder each item is in: search results and Recent,
    /// whose items come from many folders.
    pub fn shows_locations(&self) -> bool {
        self.is_searching() || self.path == crate::local::RECENT
    }

    pub fn is_searching(&self) -> bool {
        !matches!(self.search, Search::Off)
    }

    /// Applies new field text. Returns whether a (debounced) request should follow.
    pub fn set_search_input(&mut self, input: String) -> bool {
        if input == self.search_input {
            return false;
        }
        self.search_input = input;
        // Whatever search was pending or running is for the old text.
        self.invalidate_search();
        if search::active_query(&self.search_input).is_some() {
            if !self.is_searching() {
                self.folder_items = std::mem::take(&mut self.items);
                self.clear_selection();
                self.search_ranked = true;
            }
            self.search = Search::Loading;
            true
        } else {
            self.end_search();
            false
        }
    }

    pub fn clear_search(&mut self) {
        self.search_input.clear();
        self.invalidate_search();
        self.end_search();
    }

    fn end_search(&mut self) {
        if self.is_searching() {
            self.search = Search::Off;
            self.items = std::mem::take(&mut self.folder_items);
            self.clear_selection();
        }
    }

    /// The query, request generation and stop flag for a search about to be sent.
    pub fn begin_search(&mut self) -> Option<(u64, String, Arc<AtomicBool>)> {
        let query = search::active_query(&self.search_input)?.to_string();
        self.invalidate_search();
        self.search = Search::Loading;
        let cancel = Arc::new(AtomicBool::new(false));
        self.search_cancel = Some(cancel.clone());
        Some((self.search_generation, query, cancel))
    }

    /// Shows the matches found so far while the search goes on. Returns whether the
    /// view changed while they were prepared, like `finish_search`.
    pub fn show_partial_search(&mut self, items: Vec<FileInfo>, view: View) -> bool {
        self.replace_items(items);
        view != self.search_view()
    }

    /// Like `finish_load`, for search results.
    pub fn finish_search(&mut self, result: anyhow::Result<Vec<FileInfo>>, view: View) -> bool {
        self.search_cancel = None;
        match result {
            Ok(items) => {
                self.replace_items(items);
                self.search = Search::Done;
                view != self.search_view()
            }
            Err(error) => {
                self.items = Arc::default();
                self.clear_selection();
                self.search = Search::Failed(format!("{error:#}"));
                false
            }
        }
    }

    /// What replaces an empty list: an icon and its message.
    pub fn placeholder(&self) -> Option<(&'static str, String)> {
        if !self.items.is_empty() {
            return None;
        }
        match &self.search {
            Search::Off if matches!(self.listing, Listing::Loaded) => {
                Some(("folder-open", "This folder is empty".into()))
            }
            Search::Off => None,
            Search::Loading => Some(("search", "Searching...".into())),
            Search::Done => Some((
                "search",
                format!(
                    "No matches for \"{}\" in this folder",
                    search::active_query(&self.search_input).unwrap_or_default()
                ),
            )),
            Search::Failed(error) => Some(("search", error.clone())),
        }
    }

    fn index_of(&self, path: &str) -> Option<usize> {
        self.items.iter().position(|item| item.path == path)
    }

    pub fn apply_preferences(
        &mut self,
        show_hidden_files: bool,
        sort: Sort,
        view_mode: ViewMode,
    ) -> bool {
        let hidden_changed = self.show_hidden_files != show_hidden_files;
        self.show_hidden_files = show_hidden_files;
        // An unrelated preference must not take search results out of their ranking.
        if sort != self.sort {
            self.set_sort(sort);
        }
        self.set_view_mode(view_mode);
        hidden_changed
    }

    /// Records the sort; the workspace re-sorts the entries off the UI thread.
    pub fn set_sort(&mut self, sort: Sort) {
        self.sort = sort;
        self.search_ranked = false;
        self.anchor = None;
        self.active = None;
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if self.is_at_root() {
            self.drive_view_mode = mode;
        } else {
            self.view_mode = mode;
        }
    }

    pub fn current_view_mode(&self) -> ViewMode {
        if self.is_at_root() {
            self.drive_view_mode
        } else {
            self.view_mode
        }
    }

    pub fn is_selected(&self, path: &str) -> bool {
        self.selected.contains(path)
    }

    /// The index `delta` items from the keyboard cursor, clamped to the list. With no
    /// cursor, Down starts at the first item and Up at the last.
    pub fn cursor_step(&self, delta: isize) -> Option<usize> {
        let last = self.items.len().checked_sub(1)?;
        Some(match self.active.filter(|&index| index <= last) {
            Some(index) => (index as isize + delta).clamp(0, last as isize) as usize,
            None if delta < 0 => last,
            None => 0,
        })
    }

    /// Puts the keyboard cursor on `index`: selecting only it, extending the selection
    /// from the anchor to it, or leaving the selection as it is.
    pub fn move_cursor(&mut self, index: usize, how: Move) {
        let Some(item) = self.items.get(index) else { return };
        match how {
            Move::Select => {
                self.selected.clear();
                self.selected.insert(item.path.clone());
                self.anchor = Some(index);
            }
            Move::Extend => {
                let anchor = self.anchor.or(self.active).filter(|&anchor| anchor < self.items.len()).unwrap_or(index);
                let (start, end) = if anchor <= index { (anchor, index) } else { (index, anchor) };
                self.selected.clear();
                self.selected.extend(self.items[start..=end].iter().map(|item| item.path.clone()));
                self.anchor = Some(anchor);
            }
            Move::Focus => {}
        }
        self.active = Some(index);
    }

    /// Ctrl+Space: selects or deselects the item under the keyboard cursor.
    pub fn toggle_active(&mut self) {
        let Some(index) = self.active else { return };
        let Some(path) = self.items.get(index).map(|item| item.path.clone()) else { return };
        if !self.selected.remove(&path) {
            self.selected.insert(path);
        }
        self.anchor = Some(index);
    }

    /// The first item whose name starts with `prefix`, ignoring case, looking from
    /// `from` and wrapping around.
    pub fn find_prefix(&self, prefix: &str, from: usize) -> Option<usize> {
        let prefix = prefix.to_lowercase();
        let count = self.items.len();
        (0..count).map(|offset| (from + offset) % count).find(|&index| self.items[index].name.to_lowercase().starts_with(&prefix))
    }

    pub fn active_item(&self) -> Option<FileInfo> {
        self.active.and_then(|index| self.items.get(index)).cloned()
    }

    /// Click selection: plain replaces, Ctrl toggles, Shift extends from the anchor.
    pub fn click(&mut self, index: usize, modifiers: Modifiers) {
        let Some(item) = self.items.get(index) else {
            return;
        };
        let path = item.path.clone();
        if modifiers.shift {
            let anchor = self
                .anchor
                .filter(|&a| a < self.items.len())
                .unwrap_or(index);
            let (start, end) = if anchor <= index {
                (anchor, index)
            } else {
                (index, anchor)
            };
            if !(modifiers.control || modifiers.platform) {
                self.selected.clear();
            }
            self.selected
                .extend(self.items[start..=end].iter().map(|item| item.path.clone()));
        } else if modifiers.control || modifiers.platform {
            if !self.selected.remove(&path) {
                self.selected.insert(path);
            }
            self.anchor = Some(index);
        } else {
            self.selected.clear();
            self.selected.insert(path);
            self.anchor = Some(index);
        }
        self.active = Some(index);
    }

    /// Dragging a selected item carries the whole selection; any other item goes alone.
    pub fn selected_items(&self) -> Vec<FileInfo> {
        self.items
            .iter()
            .filter(|item| self.selected.contains(&item.path))
            .cloned()
            .collect()
    }

    pub fn drag_items(&self, index: usize) -> Vec<FileInfo> {
        match self.items.get(index) {
            Some(item) if self.is_selected(&item.path) => self
                .items
                .iter()
                .filter(|item| self.is_selected(&item.path))
                .cloned()
                .collect(),
            Some(item) => vec![item.clone()],
            None => Vec::new(),
        }
    }

    pub fn select_all(&mut self) {
        self.selected = self.items.iter().map(|item| item.path.clone()).collect();
    }

    /// Selects what was not selected, and only that.
    pub fn invert_selection(&mut self) {
        self.selected = self.items.iter().map(|item| item.path.clone()).filter(|path| !self.selected.contains(path)).collect();
    }

    /// Selects the entries whose names match, in place of the selection, which
    /// stays as it was when nothing matches. Returns how many matched.
    pub fn select_matching(&mut self, matches: impl Fn(&str) -> bool) -> usize {
        let matched: HashSet<String> = self.items.iter().filter(|item| matches(&item.name)).map(|item| item.path.clone()).collect();
        let count = matched.len();
        if count > 0 {
            self.selected = matched;
        }
        count
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.anchor = None;
        self.active = None;
    }

    /// Combined size of selected files; folders count as 0.
    pub fn selected_bytes(&self) -> u64 {
        self.items
            .iter()
            .filter(|item| !item.is_dir && self.selected.contains(&item.path))
            .map(|item| item.size)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the workspace does: arrange off the UI thread, then apply.
    fn load(pane: &mut Pane, items: Vec<FileInfo>) {
        let view = pane.view();
        assert!(!pane.finish_load(Ok(arrange(items, view)), view));
    }

    fn found(pane: &mut Pane, items: Vec<FileInfo>) {
        let view = pane.search_view();
        assert!(!pane.finish_search(Ok(arrange(items, view)), view));
    }

    #[test]
    fn a_folder_is_busy_from_its_request_until_its_listing() {
        let mut pane = Pane::new(0, "d".into(), false, Sort::default(), ViewMode::List);
        pane.begin_load();
        assert!(pane.busy);
        pane.finish_load(Err(anyhow::anyhow!("gone")), pane.view());
        assert!(!pane.busy, "a failure ends it too");
        let mut root = Pane::new(0, String::new(), false, Sort::default(), ViewMode::List);
        root.begin_load();
        assert!(!root.busy, "the Overview asks for no listing");
    }

    #[test]
    fn a_new_tab_shows_the_folder_at_once_and_reloads_it_quietly() {
        let mut pane = Pane::new(0, "d".into(), false, Sort::default(), ViewMode::List);
        load(&mut pane, ["a", "b"].map(file).to_vec());
        let mut tab = pane.duplicate(1);
        assert_eq!((tab.id, tab.path.as_str()), (1, "d"));
        assert!(matches!(tab.listing, Listing::Loaded) && Arc::ptr_eq(&tab.items, &pane.items), "shown at once, not re-read");
        assert!(tab.copied);
        tab.begin_load();
        assert!(matches!(tab.listing, Listing::Loaded), "the reload keeps the entries on screen");
        load(&mut tab, ["a", "b", "c"].map(file).to_vec());
        assert_eq!(tab.items.len(), 3);
        assert!(tab.copied, "still the same folder, so it does not fade up");
        assert!(tab.navigate("d/a".into()));
        assert!(!tab.copied);
    }

    #[test]
    fn a_sort_is_dropped_when_entries_change_first() {
        let mut pane = Pane::new(0, "d".into(), false, Sort::default(), ViewMode::List);
        load(&mut pane, ["b", "a"].map(file).to_vec());
        pane.set_sort(Sort { field: crate::sort::SortField::Name, dir: crate::sort::SortDir::Asc });
        let job = pane.sort_job();
        let (items, folder_items) = job.run();
        load(&mut pane, ["c", "a"].map(file).to_vec());
        assert!(!pane.apply_sort(&job, items, folder_items));

        let job = pane.sort_job();
        let (items, folder_items) = job.run();
        assert!(pane.apply_sort(&job, items, folder_items));
        let names: Vec<_> = pane.items.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, ["a", "c"]);
    }

    fn file(name: &str) -> FileInfo {
        FileInfo {
            name: name.into(),
            path: format!("d/{name}"),
            size: 10,
            is_dir: false,
            hidden: false,
            mod_time: None,
        }
    }

    #[test]
    fn back_returns_to_the_selection_cursor_and_scroll_it_left() {
        let mut pane = Pane::new(0, String::new(), false, Sort::default(), ViewMode::List);
        assert!(pane.navigate("d".into()));
        load(&mut pane, vec![file("a"), file("b"), file("c")]);
        pane.click(0, Modifiers::default());
        pane.click(2, Modifiers { control: true, ..Default::default() });
        let scrolled = gpui::point(gpui::px(0.), gpui::px(-480.));
        pane.list_scroll.0.borrow().base_handle.set_offset(scrolled);
        let offset = |pane: &Pane| pane.list_scroll.0.borrow().base_handle.offset();

        // A folder opened from here starts at its top, with nothing selected.
        assert!(pane.navigate("e".into()));
        assert_eq!(offset(&pane), gpui::Point::default());
        load(&mut pane, vec![file("x")]);
        assert!(pane.selected.is_empty());

        // Back: once the entries are in, the same two are selected, the cursor is
        // on the last one clicked, and the list is where it was. "a" went meanwhile.
        assert_eq!(pane.travel(true, 1, &[]), Moved::Within);
        assert_eq!(offset(&pane), gpui::Point::default(), "an empty list cannot hold the position yet");
        load(&mut pane, vec![file("b"), file("c")]);
        assert_eq!(pane.selected, HashSet::from(["d/c".to_string()]));
        assert_eq!(pane.active.map(|index| pane.items[index].name.as_str()), Some("c"));
        assert_eq!(offset(&pane), scrolled);

        // Refreshing keeps them too.
        load(&mut pane, vec![file("b"), file("c"), file("d")]);
        assert_eq!(pane.selected, HashSet::from(["d/c".to_string()]));
    }

    #[test]
    fn history_and_selection() {
        let mut pane = Pane::new(0, String::new(), false, Sort::default(), ViewMode::List);
        assert!(pane.navigate("d".into()));
        load(&mut pane, vec![file("a"), file("b"), file("c"), file(".h")]);
        assert_eq!(pane.items.len(), 3);
        pane.click(0, Modifiers::default());
        pane.click(
            2,
            Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        assert_eq!(pane.selected.len(), 3);
        assert_eq!(pane.selected_bytes(), 30);
        pane.click(
            1,
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        assert_eq!(pane.selected.len(), 2);
        assert_eq!(pane.travel(true, 1, &[]), Moved::Within);
        assert!(pane.is_at_root() && pane.selected.is_empty() && pane.can_go_forward());
        assert_eq!(pane.travel(false, 1, &[]), Moved::Within);
        assert_eq!(pane.path, "d");
    }

    #[test]
    fn the_keyboard_cursor_selects_extends_or_only_moves() {
        let mut pane = Pane::new(0, "d".into(), false, Sort::default(), ViewMode::List);
        load(&mut pane, ["a", "b", "c", "d", "e"].map(file).to_vec());
        assert_eq!(pane.cursor_step(1), Some(0));
        assert_eq!(pane.cursor_step(-1), Some(4));
        pane.move_cursor(1, Move::Select);
        pane.move_cursor(pane.cursor_step(2).unwrap(), Move::Focus);
        assert_eq!((pane.active, pane.selected.len()), (Some(3), 1));
        pane.toggle_active();
        assert!(pane.is_selected("d/b") && pane.is_selected("d/d"));
        pane.move_cursor(4, Move::Extend);
        let mut selected: Vec<_> = pane.selected.iter().cloned().collect();
        selected.sort();
        assert_eq!(selected, ["d/d", "d/e"]);
        assert_eq!(pane.cursor_step(10), Some(4));
        assert_eq!(pane.find_prefix("C", 0), Some(2));
        assert_eq!(pane.find_prefix("a", 1), Some(0));
        assert_eq!(pane.find_prefix("z", 0), None);
    }

    #[test]
    fn travel_skips_places_that_are_gone() {
        let mut pane = Pane::new(0, String::new(), false, Sort::default(), ViewMode::List);
        for path in ["a", "b", "c", "d"] {
            pane.navigate(path.into());
        }
        let gone = [Place { api: pane.api.clone(), path: "c".into(), view: ViewMemo::default() }];
        assert_eq!(pane.travel(true, 1, &gone), Moved::Within);
        assert_eq!(pane.path, "b");
        let paths = |places: Vec<Place>| places.into_iter().map(|place| place.path).collect::<Vec<_>>();
        let (back, forward) = pane.history();
        assert_eq!((paths(back), paths(forward)), (vec!["a".into(), String::new()], vec!["d".into()]));
        assert_eq!(pane.travel(true, 2, &[]), Moved::Within);
        assert_eq!(pane.path, "");
        assert_eq!(pane.travel(false, 3, &[]), Moved::Within);
        assert_eq!(pane.path, "d");
        assert_eq!(pane.travel(false, 1, &[]), Moved::No);
    }

    #[test]
    fn history_remembers_each_places_provider() {
        let mut pane = Pane::new(0, String::new(), false, Sort::default(), ViewMode::List);
        let home = pane.api.clone();
        let server = Arc::new(Api::new("http://nas:8080", None));
        pane.navigate("a".into());
        assert_eq!(pane.navigate_to(server.clone(), "media".into()), Moved::Across);
        assert!(Arc::ptr_eq(&pane.api, &server));
        assert!(matches!(pane.drives, Load::Loading), "the server's drives load next");
        assert_eq!(pane.navigate_to(server.clone(), "media/films".into()), Moved::Within);
        // Two steps back crosses to this computer again.
        assert_eq!(pane.travel(true, 2, &[]), Moved::Across);
        assert!(Arc::ptr_eq(&pane.api, &home));
        assert_eq!(pane.path, "a");
        assert_eq!(pane.travel(false, 1, &[]), Moved::Across);
        assert_eq!((pane.path.as_str(), Arc::ptr_eq(&pane.api, &server)), ("media", true));
    }

    #[test]
    fn refresh_keeps_anchor_by_path() {
        let shift = Modifiers {
            shift: true,
            ..Default::default()
        };
        let mut pane = Pane::new(0, "d".into(), false, Sort::default(), ViewMode::List);
        load(&mut pane, ["a", "b", "c", "d", "e"].map(file).to_vec());
        pane.click(4, Modifiers::default());
        load(&mut pane, vec![file("a"), file("b")]);
        pane.click(1, shift);
        assert_eq!(pane.selected.len(), 1);

        pane.click(1, Modifiers::default());
        load(&mut pane, vec![file("0"), file("a"), file("b"), file("z")]);
        pane.click(3, shift);
        let mut selected: Vec<_> = pane.selected.iter().cloned().collect();
        selected.sort();
        assert_eq!(selected, ["d/b", "d/z"]);
    }

    #[test]
    fn drag_carries_selection_or_single_item() {
        let mut pane = Pane::new(0, "d".into(), false, Sort::default(), ViewMode::List);
        load(&mut pane, ["a", "b", "c"].map(file).to_vec());
        pane.click(0, Modifiers::default());
        pane.click(
            2,
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        let names =
            |items: Vec<FileInfo>| items.into_iter().map(|item| item.name).collect::<Vec<_>>();
        assert_eq!(names(pane.drag_items(2)), ["a", "c"]);
        assert_eq!(names(pane.drag_items(1)), ["b"]);
    }

    #[test]
    fn every_load_invalidates_older_requests() {
        let mut pane = Pane::new(0, "d".into(), false, Sort::default(), ViewMode::List);
        let pending = pane.begin_load();
        pane.travel(true, 1, &[]);
        assert!(pane.navigate(String::new()));
        assert_ne!(pane.begin_load(), pending);
        assert!(matches!(pane.listing, Listing::Root));
    }

    fn hit(path: &str) -> FileInfo {
        let name = path.rsplit('/').next().unwrap();
        FileInfo {
            path: path.into(),
            ..file(name)
        }
    }

    fn pane_in_folder() -> Pane {
        let mut pane = Pane::new(0, "d".into(), false, Sort::default(), ViewMode::List);
        load(&mut pane, ["a", "b"].map(file).to_vec());
        pane
    }

    fn names(pane: &Pane) -> Vec<&str> {
        pane.items.iter().map(|item| item.name.as_str()).collect()
    }

    #[test]
    fn one_character_does_not_search() {
        let mut pane = pane_in_folder();
        assert!(!pane.set_search_input("a".into()));
        assert!(!pane.is_searching());
        assert!(pane.begin_search().is_none());
        assert_eq!(names(&pane), ["a", "b"]);
    }

    #[test]
    fn results_replace_the_folder_and_clearing_restores_it() {
        let mut pane = pane_in_folder();
        pane.click(0, Modifiers::default());
        assert!(pane.set_search_input(" ca ".into()));
        assert!(pane.items.is_empty() && pane.selected.is_empty());
        assert_eq!(pane.placeholder().unwrap().1, "Searching...");

        let (generation, query, _) = pane.begin_search().unwrap();
        assert_eq!(query, "ca");
        assert_eq!(generation, pane.search_generation);
        found(&mut pane, vec![hit("d/x/cat"), hit("d/y/car")]);
        // The search's own order, not the folder's sort.
        assert_eq!(names(&pane), ["cat", "car"]);
        assert!(pane.placeholder().is_none());
        // Results carry their own paths, so nested hits stay selectable.
        pane.click(0, Modifiers::default());
        assert!(pane.is_selected("d/x/cat"));

        pane.clear_search();
        assert!(!pane.is_searching() && pane.search_input.is_empty());
        assert_eq!(names(&pane), ["a", "b"]);
        assert!(pane.selected.is_empty());
    }

    #[test]
    fn shortening_the_text_below_the_minimum_leaves_search() {
        let mut pane = pane_in_folder();
        pane.set_search_input("ab".into());
        pane.begin_search();
        found(&mut pane, vec![hit("d/ab")]);
        assert!(!pane.set_search_input("a".into()));
        assert_eq!(names(&pane), ["a", "b"]);
        assert!(matches!(pane.search, Search::Off));
    }

    #[test]
    fn later_input_invalidates_the_request_in_flight() {
        let mut pane = pane_in_folder();
        pane.set_search_input("ca".into());
        let (stale, _, cancel) = pane.begin_search().unwrap();
        pane.set_search_input("cat".into());
        assert_ne!(stale, pane.search_generation);
        // The walk for the old text is told to stop.
        assert!(cancel.load(Ordering::Relaxed));
        // The workspace drops responses whose generation no longer matches.
        let (current, query, cancel) = pane.begin_search().unwrap();
        assert!(!cancel.load(Ordering::Relaxed));
        assert_eq!(query, "cat");
        assert_eq!(current, pane.search_generation);
        assert!(matches!(pane.search, Search::Loading));
    }

    #[test]
    fn empty_and_failed_searches_explain_themselves() {
        let mut pane = pane_in_folder();
        pane.set_search_input("zz".into());
        pane.begin_search();
        found(&mut pane, vec![]);
        assert_eq!(
            pane.placeholder().unwrap().1,
            "No matches for \"zz\" in this folder"
        );
        pane.finish_search(Err(anyhow::anyhow!("Search path not found")), pane.view());
        assert_eq!(pane.placeholder().unwrap().1, "Search path not found");
        assert!(pane.items.is_empty());
    }

    #[test]
    fn results_follow_hidden_files_and_sort() {
        let mut pane = pane_in_folder();
        pane.set_search_input("ab".into());
        pane.begin_search();
        found(&mut pane, vec![hit("d/b/zab"), hit("d/.ab"), hit("d/aab")]);
        assert!(pane.shows_ranked());
        assert_eq!(names(&pane), ["zab", "aab"]);
        // A column click sorts the results like a folder.
        pane.set_sort(Sort {
            field: crate::sort::SortField::Name,
            dir: crate::sort::SortDir::Desc,
        });
        let job = pane.sort_job();
        let (items, folder_items) = job.run();
        assert!(pane.apply_sort(&job, items, folder_items));
        assert!(!pane.shows_ranked());
        assert_eq!(names(&pane), ["zab", "aab"]);
        pane.clear_search();
        assert_eq!(names(&pane), ["b", "a"]);
        // The next search is ranked again.
        pane.set_search_input("ab".into());
        assert!(pane.search_ranked);
    }

    #[test]
    fn matches_show_while_the_search_runs_and_a_resort_keeps_their_rank() {
        let mut pane = pane_in_folder();
        pane.set_search_input("ab".into());
        pane.begin_search();
        let view = pane.search_view();
        assert!(!pane.show_partial_search(vec![hit("d/zab")], view));
        assert_eq!(names(&pane), ["zab"]);
        assert!(matches!(pane.search, Search::Loading));
        assert!(pane.placeholder().is_none());
        found(&mut pane, vec![hit("d/zab"), hit("d/aab")]);
        // A background sort of the folder underneath leaves the ranked results alone.
        let job = pane.sort_job();
        let (items, folder_items) = job.run();
        assert!(pane.apply_sort(&job, items, folder_items));
        assert_eq!(names(&pane), ["zab", "aab"]);
    }

    #[test]
    fn a_folder_reload_during_search_waits_behind_the_results() {
        let mut pane = pane_in_folder();
        pane.set_search_input("ab".into());
        pane.begin_search();
        found(&mut pane, vec![hit("d/ab")]);
        load(&mut pane, vec![file("a"), file("b"), file("c")]);
        assert_eq!(names(&pane), ["ab"]);
        pane.clear_search();
        assert_eq!(names(&pane), ["a", "b", "c"]);
    }

    #[test]
    fn opening_another_folder_leaves_search() {
        let mut pane = pane_in_folder();
        pane.set_search_input("ab".into());
        let (stale, _, cancel) = pane.begin_search().unwrap();
        assert!(pane.navigate("e".into()));
        assert!(cancel.load(Ordering::Relaxed));
        assert!(!pane.is_searching() && pane.search_input.is_empty());
        assert_ne!(stale, pane.search_generation);
        assert!(pane.items.is_empty());
    }
}
