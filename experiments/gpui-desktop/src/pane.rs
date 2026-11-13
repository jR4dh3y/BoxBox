//! One tab's browse state: location, history, listing, sort, view and selection.
//! Mirrors `lib/browse/location.svelte.ts` and `selection.svelte.ts`.

use std::collections::HashSet;
use std::sync::Arc;

use gpui::{Modifiers, UniformListScrollHandle};

use crate::api::{Api, DriveStats, FileInfo, MountPoint};
use crate::search;
use crate::sort::{sort_files, Sort};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct View {
    pub sort: Sort,
    pub show_hidden_files: bool,
}

/// Drops hidden entries unless shown, then sorts. Runs off the UI thread.
pub fn arrange(items: Vec<FileInfo>, view: View) -> Vec<FileInfo> {
    let mut items: Vec<FileInfo> =
        items.into_iter().filter(|item| view.show_hidden_files || !item.name.starts_with('.')).collect();
    sort_files(&mut items, view.sort);
    items
}

pub struct SortJob {
    items: Arc<Vec<FileInfo>>,
    folder_items: Arc<Vec<FileInfo>>,
    pub sort: Sort,
}

impl SortJob {
    /// Sorted copies of both lists.
    pub fn run(&self) -> (Vec<FileInfo>, Vec<FileInfo>) {
        let sorted = |items: &Vec<FileInfo>| {
            let mut items = items.clone();
            sort_files(&mut items, self.sort);
            items
        };
        (sorted(&self.items), sorted(&self.folder_items))
    }
}

pub enum Load<T> {
    Loading,
    Loaded(T),
    Failed(String),
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Grid,
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
    /// Virtual path; "" is This PC.
    pub path: String,
    /// Provider identity and operations for this pane; another pane may use another provider.
    pub(crate) api: std::sync::Arc<Api>,
    pub(crate) roots: Load<Vec<MountPoint>>,
    pub(crate) drives: Load<Vec<DriveStats>>,
    back: Vec<String>,
    forward: Vec<String>,
    pub listing: Listing,
    /// Visible entries in display order (hidden files removed, then sorted): the folder, or the
    /// search results while a search is on. Shared, so sorting can run on a
    /// background thread without copying on the UI thread.
    pub items: Arc<Vec<FileInfo>>,
    pub search_input: String,
    pub search: Search,
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
    /// Bumped per input change and request, so a late search response is ignored.
    pub search_generation: u64,
    pub list_scroll: UniformListScrollHandle,
    pub grid_scroll: UniformListScrollHandle,
}

impl Pane {
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
            roots: Load::Loading,
            drives: Load::Loading,
            back: Vec::new(),
            forward: Vec::new(),
            items: Arc::default(),
            search_input: String::new(),
            search: Search::Off,
            folder_items: Arc::default(),
            sort,
            view_mode,
            show_hidden_files,
            drive_view_mode: ViewMode::Grid,
            selected: HashSet::new(),
            anchor: None,
            active: None,
            generation: 0,
            search_generation: 0,
            list_scroll: UniformListScrollHandle::new(),
            grid_scroll: UniformListScrollHandle::new(),
        }
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
    pub fn navigate(&mut self, path: String) -> bool {
        if path == self.path {
            return false;
        }
        self.back.push(std::mem::replace(&mut self.path, path));
        self.forward.clear();
        self.reset();
        true
    }

    pub fn go_back(&mut self) -> bool {
        let Some(path) = self.back.pop() else {
            return false;
        };
        self.forward.push(std::mem::replace(&mut self.path, path));
        self.reset();
        true
    }

    pub fn go_forward(&mut self) -> bool {
        let Some(path) = self.forward.pop() else {
            return false;
        };
        self.back.push(std::mem::replace(&mut self.path, path));
        self.reset();
        true
    }

    fn reset(&mut self) {
        self.items = Arc::default();
        self.folder_items = Arc::default();
        self.search_input.clear();
        self.search = Search::Off;
        // Invalidates a search still in flight for the old folder.
        self.search_generation += 1;
        self.clear_selection();
        self.listing = if self.path.is_empty() {
            Listing::Root
        } else {
            Listing::Loading
        };
    }

    pub fn begin_load(&mut self) -> u64 {
        self.generation += 1;
        if !self.is_at_root() && self.items.is_empty() {
            self.listing = Listing::Loading;
        }
        self.generation
    }

    /// Takes entries `arrange`d off the UI thread. Returns whether the view changed while
    /// they were prepared, so the caller should sort again.
    pub fn finish_load(&mut self, result: anyhow::Result<Vec<FileInfo>>, view: View) -> bool {
        match result {
            Ok(items) => {
                if self.is_searching() {
                    self.folder_items = Arc::new(items);
                } else {
                    self.replace_items(items);
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

    /// What `arrange` needs, read on the UI thread before work is sent off it.
    pub fn view(&self) -> View {
        View { sort: self.sort, show_hidden_files: self.show_hidden_files }
    }

    /// The current entries and how to sort them, for `apply_sort`.
    pub fn sort_job(&self) -> SortJob {
        SortJob { items: self.items.clone(), folder_items: self.folder_items.clone(), sort: self.sort }
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
        self.search_generation += 1;
        if search::active_query(&self.search_input).is_some() {
            if !self.is_searching() {
                self.folder_items = std::mem::take(&mut self.items);
                self.clear_selection();
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
        self.search_generation += 1;
        self.end_search();
    }

    fn end_search(&mut self) {
        if self.is_searching() {
            self.search = Search::Off;
            self.items = std::mem::take(&mut self.folder_items);
            self.clear_selection();
        }
    }

    /// The query and request generation for a search about to be sent.
    pub fn begin_search(&mut self) -> Option<(u64, String)> {
        let query = search::active_query(&self.search_input)?.to_string();
        self.search_generation += 1;
        self.search = Search::Loading;
        Some((self.search_generation, query))
    }

    /// Like `finish_load`, for search results.
    pub fn finish_search(&mut self, result: anyhow::Result<Vec<FileInfo>>, view: View) -> bool {
        match result {
            Ok(items) => {
                self.replace_items(items);
                self.search = Search::Done;
                view != self.view()
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
        self.set_sort(sort);
        self.set_view_mode(view_mode);
        hidden_changed
    }

    /// Records the sort; the workspace re-sorts the entries off the UI thread.
    pub fn set_sort(&mut self, sort: Sort) {
        self.sort = sort;
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

    /// Moves the active row and updates selection; Shift extends from the current anchor.
    pub fn move_selection(&mut self, delta: isize, extend: bool) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let next = match self.active {
            Some(index) => (index as isize + delta).clamp(0, self.items.len() as isize - 1),
            None if delta < 0 => self.items.len() as isize - 1,
            None => 0,
        } as usize;
        if extend {
            let anchor = self.anchor.filter(|&index| index < self.items.len()).unwrap_or(next);
            let (start, end) = if anchor <= next { (anchor, next) } else { (next, anchor) };
            self.selected.clear();
            self.selected
                .extend(self.items[start..=end].iter().map(|item| item.path.clone()));
            self.anchor = Some(anchor);
        } else {
            self.selected.clear();
            self.selected.insert(self.items[next].path.clone());
            self.anchor = Some(next);
        }
        self.active = Some(next);
        Some(next)
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
        let view = pane.view();
        assert!(!pane.finish_search(Ok(arrange(items, view)), view));
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
            mod_time: String::new(),
        }
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
        assert!(pane.go_back());
        assert!(pane.is_at_root() && pane.selected.is_empty() && pane.can_go_forward());
        assert!(pane.go_forward());
        assert_eq!(pane.path, "d");
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
        pane.go_back();
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

        let (generation, query) = pane.begin_search().unwrap();
        assert_eq!(query, "ca");
        assert_eq!(generation, pane.search_generation);
        found(&mut pane, vec![hit("d/x/cat"), hit("d/y/car")]);
        assert_eq!(names(&pane), ["car", "cat"]);
        assert!(pane.placeholder().is_none());
        // Results carry their own paths, so nested hits stay selectable.
        pane.click(1, Modifiers::default());
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
        let (stale, _) = pane.begin_search().unwrap();
        pane.set_search_input("cat".into());
        assert_ne!(stale, pane.search_generation);
        // The workspace drops responses whose generation no longer matches.
        let (current, query) = pane.begin_search().unwrap();
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
        assert_eq!(names(&pane), ["aab", "zab"]);
        pane.set_sort(Sort {
            field: crate::sort::SortField::Name,
            dir: crate::sort::SortDir::Desc,
        });
        let job = pane.sort_job();
        let (items, folder_items) = job.run();
        assert!(pane.apply_sort(&job, items, folder_items));
        assert_eq!(names(&pane), ["zab", "aab"]);
        pane.clear_search();
        assert_eq!(names(&pane), ["b", "a"]);
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
        let (stale, _) = pane.begin_search().unwrap();
        assert!(pane.navigate("e".into()));
        assert!(!pane.is_searching() && pane.search_input.is_empty());
        assert_ne!(stale, pane.search_generation);
        assert!(pane.items.is_empty());
    }
}
