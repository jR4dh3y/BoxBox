//! The Trash page's table: its order and what is selected, kept as a folder's
//! are, so Trash reads and selects like any other listing, as in Thunar.

use std::cmp::Ordering;
use std::collections::HashSet;

use gpui::{Context, Modifiers, UniformListScrollHandle};

use super::{Load, Workspace};
use crate::api::TrashItem;
use crate::components::Column;
use crate::format::{deletion_millis, type_description};
use crate::sort::{natural_cmp, SortDir};

pub(crate) struct TrashTable {
    pub sort: (Column, SortDir),
    /// Selected items, by Trash id.
    pub selected: HashSet<String>,
    /// Where a Shift+click range starts, and the keyboard cursor.
    pub anchor: Option<usize>,
    pub scroll: UniformListScrollHandle,
}

impl Default for TrashTable {
    /// Newest first, as Thunar and Nautilus open Trash.
    fn default() -> Self {
        Self { sort: (Column::Deleted, SortDir::Desc), selected: HashSet::new(), anchor: None, scroll: UniformListScrollHandle::new() }
    }
}

/// The folder an item came from, with the home folder as `~`.
pub(crate) fn location(original: &str) -> String {
    std::path::Path::new(original).parent().map(crate::local::display_location).unwrap_or_default()
}

/// `items` in `sort`'s order; folders first, as in a folder listing.
pub(crate) fn sort_trash(items: &mut [TrashItem], (column, dir): (Column, SortDir)) {
    let key = |item: &TrashItem| -> (i64, String) {
        match column {
            Column::Deleted => (deletion_millis(&item.deleted_at).unwrap_or(0), String::new()),
            Column::Location => (0, location(&item.original_path)),
            Column::Type => (0, if item.is_dir { "Folder".into() } else { type_description(&item.name).into_owned() }),
            Column::Name | Column::Size | Column::Modified => (0, String::new()),
        }
    };
    items.sort_by(|left, right| {
        if left.is_dir != right.is_dir {
            return if left.is_dir { Ordering::Less } else { Ordering::Greater };
        }
        let ((left_value, left_text), (right_value, right_text)) = (key(left), key(right));
        let ordering = left_value.cmp(&right_value).then_with(|| natural_cmp(&left_text, &right_text)).then_with(|| natural_cmp(&left.name, &right.name));
        match dir {
            SortDir::Asc => ordering,
            SortDir::Desc => ordering.reverse(),
        }
    });
}

impl Workspace {
    pub(crate) fn trash_items(&self) -> &[TrashItem] {
        match &self.trash {
            Load::Loaded(items) => items,
            _ => &[],
        }
    }

    /// Puts a fresh listing in the table's order and drops what is no longer there
    /// from the selection.
    pub(crate) fn show_trash_items(&mut self, mut items: Vec<TrashItem>) {
        sort_trash(&mut items, self.trash_table.sort);
        let present: HashSet<&str> = items.iter().map(|item| item.id.as_str()).collect();
        self.trash_table.selected.retain(|id| present.contains(id.as_str()));
        self.trash_table.anchor = self.trash_table.anchor.filter(|&anchor| anchor < items.len());
        self.trash = Load::Loaded(items);
    }

    /// A header click: that column ascending, or the other way if it already sorts.
    pub(crate) fn sort_trash_by(&mut self, column: Column, cx: &mut Context<Self>) {
        let (current, dir) = self.trash_table.sort;
        let dir = match (current == column, dir) {
            (true, SortDir::Asc) => SortDir::Desc,
            _ => SortDir::Asc,
        };
        self.trash_table.sort = (column, dir);
        if let Load::Loaded(items) = &mut self.trash {
            sort_trash(items, (column, dir));
        }
        self.trash_table.anchor = None;
        cx.notify();
    }

    /// A row click, with the modifiers a folder row takes.
    pub(crate) fn click_trash(&mut self, index: usize, modifiers: Modifiers, cx: &mut Context<Self>) {
        let items = match &self.trash {
            Load::Loaded(items) => items,
            _ => return,
        };
        let Some(item) = items.get(index) else { return };
        let table = &mut self.trash_table;
        if modifiers.shift {
            let anchor = table.anchor.filter(|&anchor| anchor < items.len()).unwrap_or(index);
            let (start, end) = if anchor <= index { (anchor, index) } else { (index, anchor) };
            if !(modifiers.control || modifiers.platform) {
                table.selected.clear();
            }
            table.selected.extend(items[start..=end].iter().map(|item| item.id.clone()));
        } else if modifiers.control || modifiers.platform {
            if !table.selected.remove(&item.id) {
                table.selected.insert(item.id.clone());
            }
            table.anchor = Some(index);
        } else {
            table.selected.clear();
            table.selected.insert(item.id.clone());
            table.anchor = Some(index);
        }
        cx.notify();
    }

    /// A right click selects the row first unless it is already selected.
    pub(crate) fn trash_menu(&mut self, index: usize, position: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        let Some(id) = self.trash_items().get(index).map(|item| item.id.clone()) else { return };
        if !self.trash_table.selected.contains(&id) {
            self.trash_table.selected.clear();
            self.trash_table.selected.insert(id);
            self.trash_table.anchor = Some(index);
        }
        self.open_place_menu(super::place_menu::Place::TrashItems, position, cx);
    }

    pub(crate) fn selected_trash(&self) -> Vec<TrashItem> {
        self.trash_items().iter().filter(|item| self.trash_table.selected.contains(&item.id)).cloned().collect()
    }

    pub(crate) fn select_all_trash(&mut self, cx: &mut Context<Self>) {
        self.trash_table.selected = self.trash_items().iter().map(|item| item.id.clone()).collect();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, from: &str, deleted: &str, is_dir: bool) -> TrashItem {
        TrashItem { id: name.into(), name: name.into(), original_path: format!("{from}/{name}"), deleted_at: deleted.into(), is_dir, can_restore: true }
    }

    #[test]
    fn newest_first_with_folders_on_top() {
        let mut items = vec![
            item("a.txt", "/srv", "2026-01-01T00:00:00Z", false),
            item("b.txt", "/srv", "2026-03-01T00:00:00Z", false),
            item("photos", "/srv", "2025-01-01T00:00:00Z", true),
        ];
        sort_trash(&mut items, TrashTable::default().sort);
        assert_eq!(items.iter().map(|item| item.name.as_str()).collect::<Vec<_>>(), ["photos", "b.txt", "a.txt"]);
    }

    #[test]
    fn by_location_then_name() {
        let mut items = vec![item("b", "/srv/z", "", false), item("a", "/srv/z", "", false), item("c", "/srv/a", "", false)];
        sort_trash(&mut items, (Column::Location, SortDir::Asc));
        assert_eq!(items.iter().map(|item| item.name.as_str()).collect::<Vec<_>>(), ["c", "a", "b"]);
    }

    #[cfg(unix)]
    #[test]
    fn locations_show_the_folder_with_home_as_tilde() {
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
        assert_eq!(location(home.join("Desktop").join("a.pdf").to_str().unwrap()), "~/Desktop");
        assert_eq!(location("/srv/media/a.mkv"), "/srv/media");
    }
}
