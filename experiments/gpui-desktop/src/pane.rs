//! One tab's browse state: location, history, listing, sort, view and selection.
//! Mirrors `lib/browse/location.svelte.ts` and `selection.svelte.ts`.

use std::collections::HashSet;

use gpui::{Modifiers, UniformListScrollHandle};

use crate::api::FileInfo;
use crate::sort::{sort_files, Sort};

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

pub struct Pane {
    pub id: usize,
    /// Virtual path; "" is This PC.
    pub path: String,
    back: Vec<String>,
    forward: Vec<String>,
    pub listing: Listing,
    /// Visible entries in display order (hidden files removed, then sorted).
    pub items: Vec<FileInfo>,
    pub sort: Sort,
    pub view_mode: ViewMode,
    pub drive_view_mode: ViewMode,
    pub selected: HashSet<String>,
    anchor: Option<usize>,
    pub active: Option<usize>,
    /// Bumped per request, so a late response for an old folder is ignored.
    pub generation: u64,
    pub list_scroll: UniformListScrollHandle,
    pub grid_scroll: UniformListScrollHandle,
}

impl Pane {
    pub fn new(id: usize, path: String) -> Self {
        Self {
            id,
            listing: if path.is_empty() {
                Listing::Root
            } else {
                Listing::Loading
            },
            path,
            back: Vec::new(),
            forward: Vec::new(),
            items: Vec::new(),
            sort: Sort::default(),
            view_mode: ViewMode::List,
            drive_view_mode: ViewMode::Grid,
            selected: HashSet::new(),
            anchor: None,
            active: None,
            generation: 0,
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
        self.items.clear();
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

    pub fn finish_load(&mut self, result: anyhow::Result<Vec<FileInfo>>) {
        match result {
            Ok(items) => {
                // settings.showHiddenFiles defaults to false.
                self.items = items
                    .into_iter()
                    .filter(|item| !item.name.starts_with('.'))
                    .collect();
                sort_files(&mut self.items, self.sort);
                let paths: HashSet<&str> =
                    self.items.iter().map(|item| item.path.as_str()).collect();
                self.selected.retain(|path| paths.contains(path.as_str()));
                self.listing = Listing::Loaded;
            }
            Err(error) => self.listing = Listing::Failed(format!("{error:#}")),
        }
    }

    pub fn set_sort(&mut self, sort: Sort) {
        self.sort = sort;
        sort_files(&mut self.items, sort);
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

    /// Click selection: plain replaces, Ctrl toggles, Shift extends from the anchor.
    pub fn click(&mut self, index: usize, modifiers: Modifiers) {
        let Some(item) = self.items.get(index) else {
            return;
        };
        let path = item.path.clone();
        if modifiers.shift {
            let anchor = self.anchor.unwrap_or(index);
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
        let mut pane = Pane::new(0, String::new());
        assert!(pane.navigate("d".into()));
        pane.finish_load(Ok(vec![file("a"), file("b"), file("c"), file(".h")]));
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
}
